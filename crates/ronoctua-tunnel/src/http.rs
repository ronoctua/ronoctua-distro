//! HTTP/1.1 over a Unix socket, written by hand — the shape
//! `shell::syncthing` already has, and for its reason.
//!
//! # Why not a client crate
//!
//! There are five calls, all of them to a socket in `/run`, none of them with
//! TLS, redirection, proxying or a persistent connection. A real HTTP client
//! would bring dozens of crates to solve problems that do not exist here. The
//! project's cut is the same everywhere: the dependency comes in when its
//! problem shows up.
//!
//! # What it does answer for
//!
//! A deadline on every read and write. This runs on a thread of the footer's
//! process, and a socket that accepts a connection and never answers would
//! otherwise hold that thread for ever — which is the `systemd-run --scope`
//! trap AGENTS.md records, one layer down.

use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// How long a local daemon is given to answer.
///
/// Two seconds: it is a socket in `/run`, so either it answers in
/// milliseconds or it is not running.
pub const PATIENCE: Duration = Duration::from_secs(2);

/// One request, and the body that came back.
///
/// `Host` is a parameter because Tailscale checks it: its local API refuses a
/// request whose `Host` is not the name it expects, with a 403 that reads
/// exactly like a permission problem.
///
/// # Errors
///
/// The socket is not there, the daemon did not answer inside [`PATIENCE`], or
/// it answered something that is not an HTTP response.
pub fn call(
    socket: &Path,
    host: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> anyhow::Result<String> {
    let mut stream =
        UnixStream::connect(socket).map_err(|e| anyhow::anyhow!("{}: {e}", socket.display()))?;
    stream.set_read_timeout(Some(PATIENCE))?;
    stream.set_write_timeout(Some(PATIENCE))?;
    let content = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\
         Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{content}",
        content.len()
    );
    stream.write_all(request.as_bytes())?;
    let mut answer = Vec::new();
    stream.read_to_end(&mut answer)?;
    // THE FRAMING IS BYTES, AND ONLY THE FINISHED BODY IS TEXT.
    //
    // Read as a `String` first, a chunk boundary that falls INSIDE a
    // multi-byte character turned it into one replacement character of a
    // different length — every later offset was then wrong, and slicing a
    // `&str` by those byte counts panicked ("byte index N is not a char
    // boundary"). That panic killed the tunnel thread, which is also the
    // thread that carries out orders, so both marks froze at their last value
    // and every tunnel key stopped working, silently, for the rest of the
    // session. A machine on the tailnet with an accented name reaches it.
    let split = answer
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| anyhow::anyhow!("no HTTP response"))?;
    let head = String::from_utf8_lossy(&answer[..split]).into_owned();
    let status = status_of(&head).ok_or_else(|| anyhow::anyhow!("no status line"))?;
    if !(200..300).contains(&status) {
        anyhow::bail!("the daemon answered {status}");
    }
    Ok(String::from_utf8_lossy(&dechunked(&head, &answer[split + 4..])).into_owned())
}

/// The number out of `HTTP/1.1 200 OK`.
#[must_use]
pub fn status_of(head: &str) -> Option<u16> {
    head.lines().next()?.split_whitespace().nth(1)?.parse().ok()
}

/// The body, with the chunk framing taken off when there is any.
///
/// `Connection: close` does NOT stop a server from answering in chunks —
/// Tailscale's does — and a body read raw then carries the hexadecimal
/// lengths inside it, which lands in the JSON parser as a syntax error a long
/// way from the cause.
///
/// BYTES IN, BYTES OUT: a chunk may end in the middle of a character, so
/// nothing here may be a `&str`. See [`call`] for the panic that taught it.
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
}
