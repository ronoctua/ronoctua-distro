//! One call whose bodies are STREAMED: the request's read from a reader as it
//! goes out, the answer's written to a writer as it comes in — never held.
//!
//! # Why it is here
//!
//! Because the bodies are as big as a disk. Incus moves an instance's backup,
//! and a file pushed into or pulled out of an instance, over its unix socket
//! as an HTTP body: [`crate::over_unix`] holds the whole answer as TEXT, which
//! is right for a daemon's JSON and wrong twice for a tarball (held, and
//! turned into text); [`crate::tls::download`] streams, but over TLS and only
//! a `GET`. The framing — the head, the chunks, the announced length — is
//! this crate's (`one-http-writer`), so the stream's framing is here too and
//! the caller brings only the stream.
//!
//! # Generic over the stream
//!
//! A `UnixStream` in the product and a canned one in the tests, as
//! [`crate::ws`] is: the rules below are testable with no daemon.
//!
//! # What it answers for
//!
//! **A refusal is kept, never written.** Only a 2xx body goes to the writer —
//! the file being made. A daemon's 4xx is its sentence, kept in the reply (at
//! most [`REFUSAL_MOST_BYTES`]) for the caller to read, and a file that was
//! about to be written never receives an error page as its contents.
//!
//! **The announced length ends the body.** Waiting for the peer to close is
//! waiting on a peer that may keep the socket; a chunked body ends at its zero
//! chunk; only a body with neither is read to the end.
//!
//! **A stop is honoured between chunks.** `heard` is told every piece in both
//! directions and returning `false` stops the call — how a cancel reaches the
//! socket.

use std::io::{BufRead, BufReader, Read, Write};

use anyhow::{Context as _, Result, bail};

use crate::{Call, Reply};

/// A body that is NOT text: what a file is sent as.
pub const BYTES: &str = "application/octet-stream";

/// The most of a refusal's body that is kept: a daemon's sentence, never a
/// file.
pub const REFUSAL_MOST_BYTES: u64 = 1 << 20;

/// The longest head an answer may have.
const HEAD_MOST_BYTES: usize = 64 << 10;

/// The size of one piece read or written.
const PIECE_BYTES: usize = 64 << 10;

/// A request body, read as it is sent.
pub struct Body<'a> {
    /// Where its bytes come from.
    pub from: &'a mut dyn Read,
    /// How many there are — announced before the first is sent.
    pub length: u64,
}

impl std::fmt::Debug for Body<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Body")
            .field("length", &self.length)
            .finish()
    }
}

/// Where a streamed call is, as `heard` is told it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// Bytes of the request's body sent so far, of its length.
    Sent {
        /// So far.
        done: u64,
        /// Of.
        total: u64,
    },
    /// Bytes of the answer's body received so far, of its length when the
    /// answer announced one.
    Received {
        /// So far.
        done: u64,
        /// Of, when known.
        total: Option<u64>,
    },
}

/// Exchanges one call over `stream`.
///
/// The request's head, then its body: `body` when given (read as it goes,
/// `length` bytes exactly), else the call's own text. Then the answer: a 2xx
/// body is written to `to` as it arrives (at most `most` bytes) and the reply
/// comes back with its headers and an EMPTY body; any other answer comes back
/// with its body kept, at most [`REFUSAL_MOST_BYTES`], and `to` untouched.
///
/// # Errors
///
/// The stream broke or timed out, `body` gave fewer bytes than its length,
/// the answer was not HTTP, its body passed `most`, or `heard` asked to stop.
pub fn exchange<S: Read + Write>(
    mut stream: S,
    call: &Call<'_>,
    body: Option<Body<'_>>,
    to: &mut dyn Write,
    most: u64,
    heard: &mut dyn FnMut(Moved) -> bool,
) -> Result<Reply> {
    match body {
        Some(body) => {
            stream
                .write_all(crate::head_of(call, body.length).as_bytes())
                .context("the request could not be sent")?;
            send(&mut stream, body, heard)?;
        }
        None => stream
            .write_all(crate::request_of(call).as_bytes())
            .context("the request could not be sent")?,
    }
    stream.flush().ok();

    let mut reader = BufReader::new(stream);
    let head = head(&mut reader)?;
    let status = crate::status_of(&head).ok_or_else(|| anyhow::anyhow!("no status line"))?;
    let mut reply = Reply {
        status,
        headers: crate::headers_of(&head),
        body: Vec::new(),
    };
    let framing = Framing::of(&reply);
    if reply.went_through() {
        if let Some(length) = framing.length()
            && length > most
        {
            bail!("the answer announced {length} bytes, more than {most}");
        }
        receive(&mut reader, framing, to, most, heard)?;
    } else {
        let mut kept = Vec::new();
        // A REFUSAL IS READ TO ITS END OR ITS CEILING, and a longer one is
        // cut there: what matters of it is its first words.
        let _ = receive(
            &mut reader,
            framing,
            &mut kept,
            REFUSAL_MOST_BYTES,
            &mut |_| true,
        );
        reply.body = kept;
    }
    Ok(reply)
}

/// The request's body, piece by piece.
fn send(to: &mut dyn Write, body: Body<'_>, heard: &mut dyn FnMut(Moved) -> bool) -> Result<()> {
    let total = body.length;
    let mut done = 0u64;
    let mut piece = vec![0u8; PIECE_BYTES];
    while done < total {
        let want = usize::try_from((total - done).min(PIECE_BYTES as u64)).unwrap_or(PIECE_BYTES);
        let n = body
            .from
            .read(&mut piece[..want])
            .context("the body could not be read")?;
        if n == 0 {
            // THE HEAD PROMISED `total`: a body that ends early would leave
            // the daemon waiting for bytes that never come.
            bail!("the body ended after {done} of the {total} bytes it announced");
        }
        to.write_all(&piece[..n])
            .context("the body could not be sent")?;
        done += n as u64;
        if !heard(Moved::Sent { done, total }) {
            bail!("stopped");
        }
    }
    Ok(())
}

/// The answer's head, up to and without the blank line.
fn head(reader: &mut dyn BufRead) -> Result<String> {
    let mut head = Vec::new();
    loop {
        let before = head.len();
        let n = reader
            .read_until(b'\n', &mut head)
            .context("the answer could not be read")?;
        if n == 0 {
            bail!("the peer closed before its answer's head ended");
        }
        if head.len() > HEAD_MOST_BYTES {
            bail!("the answer's head is longer than {HEAD_MOST_BYTES} bytes");
        }
        // A LINE OF ITS OWN THAT IS EMPTY ends the head.
        if head[before..] == *b"\r\n" || head[before..] == *b"\n" {
            head.truncate(before);
            return Ok(String::from_utf8_lossy(&head).into_owned());
        }
    }
}

/// How the answer's body ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Framing {
    /// In chunks, at the zero chunk.
    Chunked,
    /// After this many bytes.
    Length(u64),
    /// When the peer closes.
    Close,
}

impl Framing {
    /// What the headers say.
    fn of(reply: &Reply) -> Self {
        if reply
            .header("transfer-encoding")
            .is_some_and(|v| v.to_ascii_lowercase().contains("chunked"))
        {
            return Self::Chunked;
        }
        reply
            .header("content-length")
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map_or(Self::Close, Self::Length)
    }

    /// The announced length, when there is one.
    const fn length(self) -> Option<u64> {
        match self {
            Self::Length(n) => Some(n),
            Self::Chunked | Self::Close => None,
        }
    }
}

/// The answer's body, to `to`, as it arrives.
fn receive(
    reader: &mut dyn BufRead,
    framing: Framing,
    to: &mut dyn Write,
    most: u64,
    heard: &mut dyn FnMut(Moved) -> bool,
) -> Result<()> {
    let total = framing.length();
    let mut done = 0u64;
    let mut piece = vec![0u8; PIECE_BYTES];
    let mut take = |reader: &mut dyn BufRead, left: Option<u64>, done: &mut u64| -> Result<usize> {
        let want = left.map_or(PIECE_BYTES, |l| {
            usize::try_from(l.min(PIECE_BYTES as u64)).unwrap_or(PIECE_BYTES)
        });
        let n = match reader.read(&mut piece[..want]) {
            Ok(n) => n,
            // A PEER THAT CLOSES WITHOUT A TLS-STYLE GOODBYE is still a close
            // when nothing announced a length.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof && left.is_none() => 0,
            Err(e) => return Err(anyhow::Error::new(e).context("the answer could not be read")),
        };
        if n == 0 {
            return Ok(0);
        }
        *done += n as u64;
        if *done > most {
            bail!("the answer sent more than {most} bytes");
        }
        to.write_all(&piece[..n])
            .context("the answer could not be written")?;
        if !heard(Moved::Received { done: *done, total }) {
            bail!("stopped");
        }
        Ok(n)
    };
    match framing {
        Framing::Length(length) => {
            while done < length {
                if take(reader, Some(length - done), &mut done)? == 0 {
                    bail!("the peer closed after {done} of the {length} bytes it announced");
                }
            }
        }
        Framing::Close => while take(reader, None, &mut done)? > 0 {},
        Framing::Chunked => loop {
            // ONE HEXADECIMAL LENGTH PER LINE, the chunk, a CRLF; zero ends
            // it. Through the buffered reader, so a length line split across
            // two reads is still one line.
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .context("the answer could not be read")?;
            if line.is_empty() {
                bail!("the peer closed inside a chunked answer");
            }
            let size = u64::from_str_radix(line.trim().split(';').next().unwrap_or(""), 16)
                .context("a chunk length is not a number")?;
            if size == 0 {
                break;
            }
            let mut left = size;
            while left > 0 {
                let before = done;
                if take(reader, Some(left), &mut done)? == 0 {
                    bail!("the peer closed inside a chunk");
                }
                left -= done - before;
            }
            let mut crlf = [0u8; 2];
            reader.read_exact(&mut crlf).ok();
        },
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stream that hands out canned bytes and keeps what was written.
    #[derive(Debug, Default)]
    struct Fake {
        gives: Vec<u8>,
        at: usize,
        wrote: Vec<u8>,
        /// At most this many bytes per read, to split frames and chunks.
        drip: usize,
    }

    impl Read for Fake {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let left = self.gives.len() - self.at;
            let mut n = left.min(out.len());
            if self.drip > 0 {
                n = n.min(self.drip);
            }
            out[..n].copy_from_slice(&self.gives[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    impl Write for Fake {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.wrote.extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn fake(answer: &[u8]) -> Fake {
        Fake {
            gives: answer.to_vec(),
            ..Fake::default()
        }
    }

    #[test]
    fn a_body_is_streamed_out_measured_by_its_length_and_counted() {
        // THE DEFECT THIS PREVENTS: a body whose bytes are counted by its
        // text — or not counted at all — makes the daemon wait for the rest.
        let mut stream = fake(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
        let mut file: &[u8] = &[7u8; 150_000];
        let mut sent = Vec::new();
        let mut answer = Vec::new();
        let call = Call {
            content_type: BYTES,
            ..Call::new("POST", "/1.0/instances/a/files?path=%2Fx", "incus")
        };
        let reply = exchange(
            &mut stream,
            &call,
            Some(Body {
                from: &mut file,
                length: 150_000,
            }),
            &mut answer,
            1 << 20,
            &mut |m| {
                if let Moved::Sent { done, .. } = m {
                    sent.push(done);
                }
                true
            },
        )
        .unwrap();
        assert_eq!(reply.status, 200);
        assert_eq!(answer, b"{}");
        let wrote = String::from_utf8_lossy(&stream.wrote[..200]).into_owned();
        assert!(wrote.contains("Content-Length: 150000\r\n"), "{wrote}");
        assert!(
            wrote.contains("Content-Type: application/octet-stream\r\n"),
            "{wrote}"
        );
        assert_eq!(sent.last(), Some(&150_000));
        assert!(sent.len() >= 3, "the body went in one piece: {sent:?}");
    }

    #[test]
    fn a_short_body_is_an_error_and_not_a_hang() {
        let mut stream = fake(b"");
        let mut file: &[u8] = b"abc";
        let said = exchange(
            &mut stream,
            &Call::new("POST", "/x", "incus"),
            Some(Body {
                from: &mut file,
                length: 10,
            }),
            &mut Vec::new(),
            10,
            &mut |_| true,
        )
        .unwrap_err()
        .to_string();
        assert!(said.contains("3 of the 10"), "{said}");
    }

    #[test]
    fn a_chunked_answer_reaches_the_file_whole_and_binary() {
        // A chunk split across reads, a length line split across reads, and
        // bytes that are not UTF-8.
        let mut body = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\n".to_vec();
        body.extend_from_slice(&[0x00, 0xff, 0x89]);
        body.extend_from_slice(b"\r\n2;ext=1\r\nab\r\n0\r\n\r\n");
        let mut stream = fake(&body);
        stream.drip = 2;
        let mut file = Vec::new();
        let reply = exchange(
            &mut stream,
            &Call::new("GET", "/export", "incus"),
            None,
            &mut file,
            100,
            &mut |_| true,
        )
        .unwrap();
        assert!(reply.went_through());
        assert!(reply.body.is_empty(), "a 2xx body goes to the file");
        assert_eq!(file, [0x00, 0xff, 0x89, b'a', b'b']);
    }

    #[test]
    fn the_announced_length_ends_the_body_and_a_longer_one_is_refused() {
        let mut stream = fake(b"HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nabcEXTRA");
        let mut file = Vec::new();
        let mut last = None;
        exchange(
            &mut stream,
            &Call::new("GET", "/f", "incus"),
            None,
            &mut file,
            100,
            &mut |m| {
                last = Some(m);
                true
            },
        )
        .unwrap();
        assert_eq!(file, b"abc", "the bytes after the length are not the body");
        assert_eq!(
            last,
            Some(Moved::Received {
                done: 3,
                total: Some(3)
            })
        );
        let mut big = fake(b"HTTP/1.1 200 OK\r\nContent-Length: 300\r\n\r\n");
        let said = exchange(
            &mut big,
            &Call::new("GET", "/f", "incus"),
            None,
            &mut Vec::new(),
            100,
            &mut |_| true,
        )
        .unwrap_err()
        .to_string();
        assert!(said.contains("more than 100"), "{said}");
    }

    #[test]
    fn a_refusal_is_kept_and_never_written_to_the_file() {
        // THE DEFECT THIS PREVENTS: an error page saved as the backup the
        // person asked for, with nothing on the screen.
        let mut stream = fake(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 30\r\n\r\n{\"error\":\"Instance not found\"}",
        );
        let mut file = Vec::new();
        let reply = exchange(
            &mut stream,
            &Call::new("GET", "/1.0/instances/x/backups/b/export", "incus"),
            None,
            &mut file,
            100,
            &mut |_| true,
        )
        .unwrap();
        assert_eq!(reply.status, 404);
        assert!(file.is_empty());
        assert_eq!(reply.body, b"{\"error\":\"Instance not found\"}");
    }

    #[test]
    fn a_stop_ends_the_call_between_pieces() {
        let mut answer = b"HTTP/1.1 200 OK\r\n\r\n".to_vec();
        answer.extend(std::iter::repeat_n(b'x', 300_000));
        let mut stream = fake(&answer);
        let mut file = Vec::new();
        let said = exchange(
            &mut stream,
            &Call::new("GET", "/f", "incus"),
            None,
            &mut file,
            1 << 30,
            &mut |m| !matches!(m, Moved::Received { done, .. } if done > 100_000),
        )
        .unwrap_err()
        .to_string();
        assert_eq!(said, "stopped");
        assert!(file.len() < 300_000);
    }

    #[test]
    fn a_body_with_no_length_is_read_to_the_close() {
        let mut stream = fake(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nto the end");
        let mut file = Vec::new();
        exchange(
            &mut stream,
            &Call::new("GET", "/f", "incus"),
            None,
            &mut file,
            100,
            &mut |_| true,
        )
        .unwrap();
        assert_eq!(file, b"to the end");
        assert!(
            exchange(
                &mut fake(b"not http\r\n\r\n"),
                &Call::new("GET", "/f", "incus"),
                None,
                &mut Vec::new(),
                100,
                &mut |_| true,
            )
            .is_err()
        );
    }
}
