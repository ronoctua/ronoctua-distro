//! RFC 6455, as much of it as one duplex protocol needs.
//!
//! # Why it is here and not in the crate that uses it
//!
//! A WebSocket is an HTTP request that is answered `101` and then stops being
//! HTTP. The handshake is a request line, a `Host`, and four headers — which
//! is to say it is a second request builder, and this crate exists because the
//! FIRST one had been written three times. Putting it in
//! `ronoctua-clipcascade` would have put that copy in the crate least able to
//! test it.
//!
//! # What it does, and the four things that each cost something
//!
//! **The handshake is verified.** `Sec-WebSocket-Accept` is
//! `base64(SHA-1(key + GUID))`, and checking it is what tells a real WebSocket
//! server from a proxy that answered `101` to be agreeable. A server that gets
//! it wrong is one this must not then read frames from.
//!
//! **A ping is answered.** Not optional and not cosmetic: a server that pings
//! and hears nothing back closes the connection, and the symptom is a sync
//! that works for exactly the server's idle timeout and then stops — which
//! reads as a network problem and is not one.
//!
//! **A timeout is not an error.** This socket is read from a loop that may
//! wait minutes between messages, and the underlying socket has a deadline on
//! it so the thread can never hang for ever. So "the deadline fired and
//! nothing arrived" is [`Message::Nothing`], and only a broken stream is an
//! `Err`. Getting this wrong means a reconnect every `patience` seconds, for
//! ever, on a connection that was fine.
//!
//! **A partial frame is kept.** Because a deadline can fire in the MIDDLE of
//! one. The bytes read so far stay in the buffer and the next call continues
//! where this one stopped; dropping them would desynchronise the stream, and
//! a desynchronised WebSocket does not fail — it reads the next message's
//! length out of the middle of this message's payload, and then waits for
//! bytes that will never come.
//!
//! # What it does NOT do
//!
//! Extensions (`permessage-deflate`), subprotocol negotiation beyond naming
//! one, and server mode. Nothing here ever accepts a connection.

use std::io::{Read, Write};

use anyhow::{Context as _, Result, bail};
use base64::Engine as _;

/// The constant RFC 6455 concatenates with the key before hashing.
///
/// It is in the standard, letter for letter, and its whole job is to make a
/// `101` from something that is not a WebSocket server impossible to fake by
/// accident.
const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The most one message may be, reassembled, before the socket is failed.
///
/// Sixteen megabytes. A clipboard entry that big is not a clipboard entry, and
/// the ceiling exists because the length field is 64 bits wide: without it, a
/// server — or something pretending to be one — announces an 8 EiB payload and
/// this reserves it.
pub const MOST: usize = 16 << 20;

/// What a read produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// A whole text message, continuation frames already joined.
    Text(String),
    /// A whole binary message.
    Binary(Vec<u8>),
    /// The deadline fired and nothing had arrived.
    ///
    /// NOT an error, and the distinction is the whole reason this enum has a
    /// third arm: a socket that is idle is a socket that is working.
    Nothing,
    /// The other side closed, or we did.
    Closed,
}

/// The frame opcodes this speaks.
mod opcode {
    /// A continuation of the frame before it.
    pub const CONTINUATION: u8 = 0x0;
    /// UTF-8.
    pub const TEXT: u8 = 0x1;
    /// Bytes.
    pub const BINARY: u8 = 0x2;
    /// Goodbye.
    pub const CLOSE: u8 = 0x8;
    /// Are you there.
    pub const PING: u8 = 0x9;
    /// I am here.
    pub const PONG: u8 = 0xA;
}

/// An open WebSocket.
///
/// Generic over the transport so `ws://` over a [`std::net::TcpStream`] and
/// `wss://` over a [`crate::tls::Stream`] are the same code. The framing does
/// not care, and having it care would have been two copies of the masking.
#[derive(Debug)]
pub struct Socket<S: Read + Write> {
    stream: S,
    /// Bytes read but not yet a whole frame. See the header: a deadline can
    /// fire in the middle of one, and these have to survive that.
    held: Vec<u8>,
    /// The message being reassembled across continuation frames.
    joining: Vec<u8>,
    /// What the first frame of that message said it was.
    joining_kind: Option<u8>,
}

/// Sixteen random bytes, base64 — the client's half of the handshake.
fn nonce() -> Result<String> {
    let mut raw = [0u8; 16];
    aws_lc_rs::rand::SecureRandom::fill(&aws_lc_rs::rand::SystemRandom::new(), &mut raw)
        .map_err(|_| anyhow::anyhow!("the system refused to give random bytes"))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(raw))
}

/// What the server must answer for a given key.
///
/// Pure, so the check is testable against the example in the RFC — which is
/// the only way to know the hash and the encoding are both right, since a
/// wrong answer here and a server that is not a WebSocket look identical.
#[must_use]
pub fn accept_for(key: &str) -> String {
    let digest = aws_lc_rs::digest::digest(
        &aws_lc_rs::digest::SHA1_FOR_LEGACY_USE_ONLY,
        format!("{key}{GUID}").as_bytes(),
    );
    base64::engine::general_purpose::STANDARD.encode(digest.as_ref())
}

impl<S: Read + Write> Socket<S> {
    /// Does the handshake over an already-open stream, and returns the socket.
    ///
    /// `headers` is for whatever the server needs to recognise the caller — a
    /// session cookie, most often. It is not the place for a body: a handshake
    /// has none.
    ///
    /// # Errors
    ///
    /// The stream broke, the server did not answer `101`, or its
    /// `Sec-WebSocket-Accept` is not the one this key requires.
    pub fn connect(
        mut stream: S,
        host: &str,
        path: &str,
        headers: &[(&str, &str)],
    ) -> Result<Self> {
        let key = nonce()?;
        // NOT `request_of`, AND THE REASON IS IN THE HEADERS. That function
        // writes `Connection: close` and a `Content-Length`, which are the two
        // things a handshake must not say — one closes the socket this is
        // trying to keep, and the other announces a body an upgrade has no
        // room for. It is a different message, not a second copy of the same
        // one.
        let mut request = format!("GET {path} HTTP/1.1\r\n");
        request.push_str(&format!("Host: {host}\r\n"));
        request.push_str("Upgrade: websocket\r\n");
        request.push_str("Connection: Upgrade\r\n");
        request.push_str(&format!("Sec-WebSocket-Key: {key}\r\n"));
        request.push_str("Sec-WebSocket-Version: 13\r\n");
        for (name, value) in headers {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        stream
            .write_all(request.as_bytes())
            .context("the WebSocket handshake could not be sent")?;
        stream.flush().ok();

        // THE ANSWER IS READ BYTE BY BYTE UP TO THE BLANK LINE, and not with
        // `read_to_end`: everything after that line is already FRAMES, and
        // reading past it would eat the first message of the session.
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            let read = stream
                .read(&mut byte)
                .context("the WebSocket handshake was not answered")?;
            if read == 0 {
                bail!("the server closed during the handshake");
            }
            head.push(byte[0]);
            if head.len() > 8192 {
                bail!("the handshake answer has no end; this is not a WebSocket server");
            }
        }
        let head = String::from_utf8_lossy(&head).into_owned();
        match crate::status_of(&head) {
            Some(101) => {}
            Some(status) => bail!("the server answered {status} to the upgrade"),
            None => bail!("the upgrade answer has no status line"),
        }
        // THE ACCEPT IS CHECKED. A proxy that answers `101` to be agreeable
        // cannot produce this, and reading frames off one that could not is
        // how a session ends up waiting for a message that will never parse.
        let wanted = accept_for(&key);
        let said = head
            .lines()
            .find_map(|l| {
                let (name, value) = l.split_once(':')?;
                name.trim()
                    .eq_ignore_ascii_case("sec-websocket-accept")
                    .then(|| value.trim().to_owned())
            })
            .context("the server did not send Sec-WebSocket-Accept")?;
        if said != wanted {
            bail!("the server's Sec-WebSocket-Accept does not match the key it was given");
        }
        Ok(Self {
            stream,
            held: Vec::new(),
            joining: Vec::new(),
            joining_kind: None,
        })
    }

    /// Sends a text message.
    ///
    /// # Errors
    ///
    /// The stream broke.
    pub fn send_text(&mut self, text: &str) -> Result<()> {
        self.frame(opcode::TEXT, text.as_bytes())
    }

    /// Says goodbye properly, then stops.
    ///
    /// # Errors
    ///
    /// The stream broke — which, on a close, is not worth acting on.
    pub fn close(&mut self) -> Result<()> {
        // 1000: a normal closure. The code is two bytes, big-endian, and a
        // close with no code at all is legal but tells the other side nothing.
        self.frame(opcode::CLOSE, &1000u16.to_be_bytes())
    }

    /// Writes one frame, masked.
    ///
    /// EVERY client frame is masked — the RFC requires it, and a server that
    /// receives an unmasked one is required to close the connection. The mask
    /// is four fresh random bytes per frame; reusing one would defeat the
    /// point of having it.
    fn frame(&mut self, code: u8, payload: &[u8]) -> Result<()> {
        let mut out = Vec::with_capacity(payload.len() + 14);
        // FIN is always set: nothing here sends a message in pieces. Receiving
        // one in pieces is a different matter, and is handled.
        out.push(0x80 | code);
        let len = payload.len();
        // THE THREE LENGTH FORMS, and the boundaries are exact: 125 is the
        // last one that fits in seven bits, because 126 and 127 are the two
        // escapes.
        if len < 126 {
            out.push(0x80 | u8::try_from(len).unwrap_or(125));
        } else if let Ok(short) = u16::try_from(len) {
            out.push(0x80 | 126);
            out.extend_from_slice(&short.to_be_bytes());
        } else {
            out.push(0x80 | 127);
            out.extend_from_slice(&(len as u64).to_be_bytes());
        }
        let mut mask = [0u8; 4];
        aws_lc_rs::rand::SecureRandom::fill(&aws_lc_rs::rand::SystemRandom::new(), &mut mask)
            .map_err(|_| anyhow::anyhow!("the system refused to give random bytes"))?;
        out.extend_from_slice(&mask);
        out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.stream
            .write_all(&out)
            .context("a WebSocket frame could not be sent")?;
        self.stream.flush().ok();
        Ok(())
    }

    /// The next whole message, or [`Message::Nothing`] if none had arrived.
    ///
    /// Pings are answered here rather than handed to the caller: a caller that
    /// forgot to would lose the connection at the server's idle timeout, and
    /// there is no reason for every caller to remember.
    ///
    /// # Errors
    ///
    /// The stream broke, a frame announced more than [`MOST`], or a text
    /// message was not UTF-8.
    pub fn read(&mut self) -> Result<Message> {
        loop {
            // WHAT IS ALREADY HELD IS TRIED FIRST. Two frames can arrive in
            // one read, and a loop that always read first would leave the
            // second sitting in the buffer until a third arrived to push it
            // out — which on an idle connection is for ever.
            if let Some((fin, code, payload)) = self.take_frame()? {
                match code {
                    opcode::PING => {
                        self.frame(opcode::PONG, &payload)?;
                        continue;
                    }
                    // A pong we did not ask for is legal and means nothing.
                    opcode::PONG => continue,
                    opcode::CLOSE => {
                        // ECHO IT. The RFC wants the close acknowledged, and a
                        // server that does not get one keeps the session
                        // around until its own timeout.
                        let _ = self.frame(opcode::CLOSE, &payload);
                        return Ok(Message::Closed);
                    }
                    _ => {}
                }
                if code != opcode::CONTINUATION {
                    self.joining.clear();
                    self.joining_kind = Some(code);
                }
                if self.joining.len() + payload.len() > MOST {
                    bail!("a WebSocket message announced more than {MOST} bytes");
                }
                self.joining.extend_from_slice(&payload);
                if !fin {
                    continue;
                }
                let whole = std::mem::take(&mut self.joining);
                return match self.joining_kind.take() {
                    Some(opcode::TEXT) => Ok(Message::Text(
                        String::from_utf8(whole)
                            .context("a WebSocket text message was not UTF-8")?,
                    )),
                    Some(opcode::BINARY) => Ok(Message::Binary(whole)),
                    // 0x3 TO 0x7 ARE RESERVED, and the RFC says to fail the
                    // connection rather than guess. Calling them binary would
                    // hand the caller a message the server never sent.
                    other => bail!("a WebSocket message arrived with opcode {other:?}"),
                };
            }
            // NOTHING WHOLE YET. Read more, and treat a deadline as silence.
            let mut chunk = [0u8; 8192];
            match self.stream.read(&mut chunk) {
                Ok(0) => return Ok(Message::Closed),
                Ok(n) => self.held.extend_from_slice(&chunk[..n]),
                Err(e)
                    if matches!(
                        e.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    // THE DEADLINE FIRED. Whatever is in `held` stays there —
                    // see the header for what dropping it costs.
                    return Ok(Message::Nothing);
                }
                Err(e) => return Err(anyhow::Error::new(e).context("the WebSocket broke")),
            }
        }
    }

    /// One whole frame out of what is held, if there is one.
    ///
    /// Returns `(fin, opcode, payload)`. `None` means the bytes held so far are
    /// not yet a frame, and NOTHING is consumed in that case.
    #[allow(clippy::type_complexity)]
    fn take_frame(&mut self) -> Result<Option<(bool, u8, Vec<u8>)>> {
        if self.held.len() < 2 {
            return Ok(None);
        }
        let first = self.held[0];
        let second = self.held[1];
        let fin = first & 0x80 != 0;
        let code = first & 0x0F;
        let masked = second & 0x80 != 0;
        let short = usize::from(second & 0x7F);
        let mut at = 2usize;
        let length = match short {
            126 => {
                if self.held.len() < at + 2 {
                    return Ok(None);
                }
                let n = usize::from(u16::from_be_bytes([self.held[at], self.held[at + 1]]));
                at += 2;
                n
            }
            127 => {
                if self.held.len() < at + 8 {
                    return Ok(None);
                }
                let mut wide = [0u8; 8];
                wide.copy_from_slice(&self.held[at..at + 8]);
                let n = u64::from_be_bytes(wide);
                // THE CEILING IS CHECKED BEFORE THE CAST, not after. On a
                // 64-bit host `as usize` would succeed and then this would try
                // to hold eight exabytes.
                if n > MOST as u64 {
                    bail!("a WebSocket frame announced {n} bytes");
                }
                at += 8;
                usize::try_from(n).unwrap_or(MOST)
            }
            n => n,
        };
        if length > MOST {
            bail!("a WebSocket frame announced more than {MOST} bytes");
        }
        // A SERVER FRAME MUST NOT BE MASKED, but one that is gets unmasked
        // rather than refused: being strict here would break a session over a
        // detail the payload does not depend on.
        let mask = if masked {
            if self.held.len() < at + 4 {
                return Ok(None);
            }
            let m = [
                self.held[at],
                self.held[at + 1],
                self.held[at + 2],
                self.held[at + 3],
            ];
            at += 4;
            Some(m)
        } else {
            None
        };
        if self.held.len() < at + length {
            return Ok(None);
        }
        let mut payload = self.held[at..at + length].to_vec();
        if let Some(m) = mask {
            for (i, b) in payload.iter_mut().enumerate() {
                *b ^= m[i % 4];
            }
        }
        self.held.drain(..at + length);
        Ok(Some((fin, code, payload)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stream that hands out canned bytes and remembers what was written.
    ///
    /// It is what makes every rule in this file testable with no server: a
    /// WebSocket test that needs one is a test that does not run in the
    /// sandbox this project builds in.
    #[derive(Debug, Default)]
    struct Fake {
        gives: Vec<u8>,
        at: usize,
        wrote: Vec<u8>,
        /// How many reads should time out before any byte is handed over.
        stalls: usize,
        /// Hand out at most this many bytes per read, to force partial frames.
        drip: usize,
    }

    impl Read for Fake {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.stalls > 0 {
                self.stalls -= 1;
                return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
            }
            let left = self.gives.len() - self.at;
            if left == 0 {
                return Err(std::io::Error::from(std::io::ErrorKind::WouldBlock));
            }
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

    /// A server frame: never masked, always final unless told otherwise.
    fn served(code: u8, payload: &[u8], fin: bool) -> Vec<u8> {
        let mut out = vec![if fin { 0x80 | code } else { code }];
        let len = payload.len();
        if len < 126 {
            out.push(u8::try_from(len).unwrap());
        } else {
            out.push(126);
            out.extend_from_slice(&u16::try_from(len).unwrap().to_be_bytes());
        }
        out.extend_from_slice(payload);
        out
    }

    fn socket_over(gives: Vec<u8>) -> Socket<Fake> {
        Socket {
            stream: Fake {
                gives,
                ..Fake::default()
            },
            held: Vec::new(),
            joining: Vec::new(),
            joining_kind: None,
        }
    }

    #[test]
    fn the_accept_matches_the_example_in_the_rfc() {
        // RFC 6455 §1.3, letter for letter. This is the only way to know the
        // SHA-1 and the base64 are BOTH right: a wrong answer here and a
        // server that is not a WebSocket look identical from the outside.
        assert_eq!(
            accept_for("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn a_nonce_is_sixteen_random_bytes_and_never_repeats() {
        let one = nonce().unwrap();
        let two = nonce().unwrap();
        assert_ne!(one, two, "the key is not random");
        assert_eq!(
            base64::engine::general_purpose::STANDARD
                .decode(&one)
                .unwrap()
                .len(),
            16
        );
    }

    #[test]
    fn a_deadline_is_silence_and_not_an_error() {
        // THE DEFECT THIS PREVENTS: treating a timeout as a broken socket
        // means reconnecting every `patience` seconds for ever, on a
        // connection that was working.
        let mut socket = socket_over(Vec::new());
        socket.stream.stalls = 3;
        assert_eq!(socket.read().unwrap(), Message::Nothing);
        assert_eq!(socket.read().unwrap(), Message::Nothing);
    }

    #[test]
    fn a_frame_split_across_reads_survives_a_deadline_in_the_middle() {
        // THE DEFECT THIS PREVENTS: dropping the partial frame desynchronises
        // the stream, and a desynchronised WebSocket does not fail — it reads
        // the next length out of the middle of this payload and then waits for
        // bytes that never come.
        let mut socket = socket_over(served(opcode::TEXT, b"hello world", true));
        socket.stream.drip = 3;
        let mut said = Message::Nothing;
        for _ in 0..10 {
            said = socket.read().unwrap();
            if said != Message::Nothing {
                break;
            }
        }
        assert_eq!(said, Message::Text("hello world".to_owned()));
    }

    #[test]
    fn two_frames_in_one_read_both_come_out() {
        // THE DEFECT THIS PREVENTS: a loop that reads before parsing leaves
        // the second message in the buffer until a third arrives to push it
        // out — which on an idle connection is for ever.
        let mut both = served(opcode::TEXT, b"one", true);
        both.extend(served(opcode::TEXT, b"two", true));
        let mut socket = socket_over(both);
        assert_eq!(socket.read().unwrap(), Message::Text("one".to_owned()));
        assert_eq!(socket.read().unwrap(), Message::Text("two".to_owned()));
        assert_eq!(socket.read().unwrap(), Message::Nothing);
    }

    #[test]
    fn a_ping_is_answered_without_the_caller_hearing_about_it() {
        // THE DEFECT THIS PREVENTS: a server that pings and hears nothing
        // closes the connection, and the symptom is a sync that works for
        // exactly the idle timeout and then stops.
        let mut gives = served(opcode::PING, b"beat", true);
        gives.extend(served(opcode::TEXT, b"after", true));
        let mut socket = socket_over(gives);
        assert_eq!(socket.read().unwrap(), Message::Text("after".to_owned()));
        // AND THE PONG WENT OUT, masked, carrying the ping's payload back.
        let wrote = &socket.stream.wrote;
        assert_eq!(wrote[0], 0x80 | opcode::PONG, "not a final pong frame");
        assert_eq!(wrote[1] & 0x80, 0x80, "a client frame must be masked");
        let mask = [wrote[2], wrote[3], wrote[4], wrote[5]];
        let back: Vec<u8> = wrote[6..10]
            .iter()
            .enumerate()
            .map(|(i, b)| b ^ mask[i % 4])
            .collect();
        assert_eq!(back, b"beat");
    }

    #[test]
    fn continuation_frames_are_joined_into_one_message() {
        let mut gives = served(opcode::TEXT, b"half ", false);
        gives.extend(served(opcode::CONTINUATION, b"a message", true));
        let mut socket = socket_over(gives);
        assert_eq!(
            socket.read().unwrap(),
            Message::Text("half a message".to_owned())
        );
    }

    #[test]
    fn a_close_is_echoed_and_ends_the_socket() {
        let mut socket = socket_over(served(opcode::CLOSE, &1000u16.to_be_bytes(), true));
        assert_eq!(socket.read().unwrap(), Message::Closed);
        assert_eq!(socket.stream.wrote[0], 0x80 | opcode::CLOSE);
    }

    #[test]
    fn what_we_send_is_always_masked_and_measures_itself() {
        let mut socket = socket_over(Vec::new());
        socket.send_text("hi").unwrap();
        let wrote = &socket.stream.wrote;
        assert_eq!(wrote[0], 0x80 | opcode::TEXT);
        assert_eq!(wrote[1], 0x80 | 2, "masked, length two");
        assert_eq!(wrote.len(), 2 + 4 + 2);

        // AND THE LONG FORM at the exact boundary: 125 fits in seven bits, 126
        // is the first that does not.
        let mut small = socket_over(Vec::new());
        small.send_text(&"x".repeat(125)).unwrap();
        assert_eq!(small.stream.wrote[1], 0x80 | 125);
        let mut big = socket_over(Vec::new());
        big.send_text(&"x".repeat(126)).unwrap();
        assert_eq!(big.stream.wrote[1], 0x80 | 126);
        assert_eq!(&big.stream.wrote[2..4], &126u16.to_be_bytes());
    }

    #[test]
    fn an_enormous_announced_length_is_refused_before_it_is_reserved() {
        // THE DEFECT THIS PREVENTS: the length field is 64 bits. Without a
        // ceiling checked BEFORE the cast, a server — or something pretending
        // to be one — announces eight exabytes and this reserves them.
        let mut socket = socket_over(vec![
            0x81, 127, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        ]);
        let said = socket.read().unwrap_err().to_string();
        assert!(said.contains("announced"), "{said}");
    }

    #[test]
    fn a_server_that_answers_the_wrong_accept_is_refused() {
        // A proxy that answers `101` to be agreeable cannot produce the hash,
        // and reading frames off one that could not is how a session ends up
        // waiting for a message that will never parse.
        let answer = b"HTTP/1.1 101 Switching Protocols\r\n\
                       Upgrade: websocket\r\n\
                       Sec-WebSocket-Accept: not-the-right-hash\r\n\r\n";
        let fake = Fake {
            gives: answer.to_vec(),
            ..Fake::default()
        };
        let said = Socket::connect(fake, "h", "/s", &[])
            .unwrap_err()
            .to_string();
        assert!(said.contains("Sec-WebSocket-Accept"), "{said}");
    }

    #[test]
    fn a_status_that_is_not_101_says_what_it_was() {
        let fake = Fake {
            gives: b"HTTP/1.1 401 Unauthorized\r\n\r\n".to_vec(),
            ..Fake::default()
        };
        let said = Socket::connect(fake, "h", "/s", &[])
            .unwrap_err()
            .to_string();
        assert!(said.contains("401"), "{said}");
    }

    #[test]
    fn the_handshake_carries_the_headers_and_never_says_close() {
        // `Connection: close` would close the socket this is trying to keep,
        // and a `Content-Length` announces a body an upgrade has no room for.
        // The handshake is a different message, not a second copy of
        // `request_of`.
        let answer = {
            // A well-formed answer, so `connect` gets as far as the checks.
            let mut head = String::from("HTTP/1.1 101 Switching Protocols\r\n");
            head.push_str("Sec-WebSocket-Accept: placeholder\r\n\r\n");
            head.into_bytes()
        };
        let fake = Fake {
            gives: answer,
            ..Fake::default()
        };
        // It fails on the accept — the point here is what went OUT.
        let mut kept = Vec::new();
        {
            let mut probe = Fake {
                gives: fake.gives,
                ..Fake::default()
            };
            let _ = Socket::connect(
                &mut probe,
                "host.example",
                "/clipsocket",
                &[("Cookie", "SESSION=abc")],
            );
            kept.append(&mut probe.wrote);
        }
        let sent = String::from_utf8_lossy(&kept).into_owned();
        assert!(sent.starts_with("GET /clipsocket HTTP/1.1\r\n"), "{sent}");
        assert!(sent.contains("Host: host.example\r\n"), "{sent}");
        assert!(sent.contains("Upgrade: websocket\r\n"), "{sent}");
        assert!(sent.contains("Connection: Upgrade\r\n"), "{sent}");
        assert!(sent.contains("Sec-WebSocket-Version: 13\r\n"), "{sent}");
        assert!(sent.contains("Cookie: SESSION=abc\r\n"), "{sent}");
        assert!(!sent.contains("Connection: close"), "{sent}");
        assert!(!sent.contains("Content-Length"), "{sent}");
    }
}
