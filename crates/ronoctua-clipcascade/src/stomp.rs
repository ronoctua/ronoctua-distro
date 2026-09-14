//! STOMP 1.2, the four frames this needs, over a WebSocket.
//!
//! # What a frame is
//!
//! `COMMAND\n` then `name:value\n` per header, then a blank line, then the
//! body, then a NUL. Read out of `stomp_ws/frame.py` of the 3.2.0 this system
//! pins, which is fifty-one lines and is the whole encoding.
//!
//! # The heartbeat is a bare newline, and it is not a frame
//!
//! The client announces `heart-beat: 0,20000` — it sends none, and asks the
//! server for one every twenty seconds. What arrives is a WebSocket message
//! containing exactly `\n`, and the server expects one back. Treating it as a
//! frame gets a parse error every twenty seconds; ignoring it gets a
//! connection the server drops, and both look like a network problem.
//!
//! # What is NOT here
//!
//! Transactions, receipts, `ACK`/`NACK`, and the SockJS envelope. ClipCascade
//! uses a raw STOMP endpoint and none of the rest.

use std::collections::BTreeMap;

/// What this system announces to the server.
///
/// `0,20000`: this sends no heartbeat and wants one from the server every
/// twenty seconds. Sending our own would mean a timer, and the whole point of
/// this transport is that the socket is the clock.
pub const HEARTBEAT: &str = "0,20000";

/// The STOMP version this speaks.
pub const VERSION: &str = "1.2";

/// One frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    /// `CONNECT`, `MESSAGE`, `ERROR`, …
    pub command: String,
    /// Its headers, in name order.
    pub headers: BTreeMap<String, String>,
    /// The body, which is a JSON message here or nothing.
    pub body: String,
}

impl Frame {
    /// A frame with no body.
    #[must_use]
    pub fn of(command: &str, headers: &[(&str, &str)]) -> Self {
        Self {
            command: command.to_owned(),
            headers: headers
                .iter()
                .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
                .collect(),
            body: String::new(),
        }
    }

    /// The same, carrying one.
    #[must_use]
    pub fn with_body(mut self, body: &str) -> Self {
        self.body = body.to_owned();
        self
    }

    /// One header.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(String::as_str)
    }

    /// The frame, as it goes on the wire.
    ///
    /// `content-length` IS IN BYTES, not characters — a clipboard full of
    /// accented text is longer than it looks, and a server that trusted a
    /// character count would read the next frame's first bytes as this one's
    /// tail.
    #[must_use]
    pub fn to_wire(&self) -> String {
        let mut out = String::with_capacity(self.body.len() + 128);
        out.push_str(&self.command);
        out.push('\n');
        for (name, value) in &self.headers {
            out.push_str(name);
            out.push(':');
            out.push_str(value);
            out.push('\n');
        }
        out.push_str("content-length:");
        out.push_str(&self.body.len().to_string());
        out.push('\n');
        out.push('\n');
        out.push_str(&self.body);
        out.push('\0');
        out
    }

    /// A frame read off the wire.
    ///
    /// `None` for anything that is not one, the heartbeat included — the
    /// caller answers that, and telling them apart here would make a bare
    /// newline into a parse error every twenty seconds.
    #[must_use]
    pub fn of_wire(raw: &str) -> Option<Self> {
        let raw = raw.trim_end_matches('\0');
        if raw.trim().is_empty() {
            return None;
        }
        let mut lines = raw.split('\n');
        let command = lines.next()?.trim().to_owned();
        if command.is_empty() {
            return None;
        }
        let mut headers = BTreeMap::new();
        let mut body = String::new();
        let mut in_body = false;
        for line in lines {
            if in_body {
                if !body.is_empty() {
                    // A BODY MAY CONTAIN NEWLINES, and the JSON this carries
                    // does not — but a server that pretty-printed it would
                    // otherwise arrive truncated at the first one.
                    body.push('\n');
                }
                body.push_str(line);
                continue;
            }
            if line.is_empty() {
                in_body = true;
                continue;
            }
            // ONLY THE FIRST COLON SPLITS. A `destination` is a path and a
            // `message-id` can carry one, so splitting on every colon would
            // cut a value in half.
            if let Some((name, value)) = line.split_once(':') {
                headers.insert(name.trim().to_owned(), value.to_owned());
            }
        }
        Some(Self {
            command,
            headers,
            body,
        })
    }

    /// Is this what the server sends when it has nothing to say?
    #[must_use]
    pub fn is_heartbeat(raw: &str) -> bool {
        !raw.is_empty() && raw.chars().all(|c| c == '\n' || c == '\r')
    }
}

/// The frame that opens a session.
#[must_use]
pub fn connect(host: &str) -> Frame {
    Frame::of(
        "CONNECT",
        &[
            ("accept-version", VERSION),
            ("heart-beat", HEARTBEAT),
            // THE VIRTUAL HOST, which Spring ignores and the standard wants.
            ("host", host),
        ],
    )
}

/// The frame that asks for the clipboard.
///
/// The id is a string the server echoes back on every `MESSAGE`; one
/// subscription is all this needs, so it is a constant rather than a counter.
#[must_use]
pub fn subscribe(destination: &str) -> Frame {
    Frame::of(
        "SUBSCRIBE",
        &[
            ("id", "ronoctua-0"),
            ("destination", destination),
            // `auto` IS THE ONLY ACKNOWLEDGEMENT MODE HERE. `client` would
            // mean sending an `ACK` per message, and a clipboard entry that
            // arrived is already in the history by then — there is nothing a
            // refusal could undo.
            ("ack", "auto"),
        ],
    )
}

/// The frame that sends one.
#[must_use]
pub fn send(destination: &str, body: &str) -> Frame {
    Frame::of(
        "SEND",
        &[
            ("destination", destination),
            ("content-type", "application/json"),
        ],
    )
    .with_body(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_goes_out_in_the_shape_the_upstream_writes() {
        let wire = send("/app/cliptext", r#"{"a":1}"#).to_wire();
        assert!(wire.starts_with("SEND\n"), "{wire:?}");
        assert!(wire.contains("destination:/app/cliptext\n"), "{wire:?}");
        assert!(wire.contains("content-length:7\n"), "{wire:?}");
        assert!(wire.ends_with("\n\n{\"a\":1}\0"), "{wire:?}");
    }

    #[test]
    fn the_length_is_in_bytes_and_not_in_characters() {
        // A CLIPBOARD FULL OF ACCENTED TEXT is longer than it looks, and a
        // server that trusted a character count would read the next frame's
        // first bytes as this one's tail.
        let body = "café";
        let wire = send("/d", body).to_wire();
        assert_eq!(body.chars().count(), 4);
        assert!(wire.contains("content-length:5\n"), "{wire:?}");
    }

    #[test]
    fn a_frame_comes_back_with_its_headers_and_its_body() {
        let raw = "MESSAGE\nsubscription:ronoctua-0\ndestination:/user/queue/cliptext\n\n{\"nonce\":\"x\"}\0";
        let frame = Frame::of_wire(raw).unwrap();
        assert_eq!(frame.command, "MESSAGE");
        assert_eq!(frame.header("subscription"), Some("ronoctua-0"));
        // ONLY THE FIRST COLON SPLITS: a destination is a path, and splitting
        // on every colon would cut it in half.
        assert_eq!(frame.header("destination"), Some("/user/queue/cliptext"));
        assert_eq!(frame.body, "{\"nonce\":\"x\"}");
    }

    #[test]
    fn a_bare_newline_is_a_heartbeat_and_not_a_frame() {
        // THE DEFECT THIS PREVENTS, twice over: treating it as a frame is a
        // parse error every twenty seconds, and ignoring it is a connection
        // the server drops. Both read as a network problem.
        assert!(Frame::is_heartbeat("\n"));
        assert!(Frame::is_heartbeat("\r\n"));
        assert!(!Frame::is_heartbeat("MESSAGE\n"));
        assert!(!Frame::is_heartbeat(""));
        assert_eq!(Frame::of_wire("\n"), None);
        assert_eq!(Frame::of_wire("\0"), None);
    }

    #[test]
    fn a_body_with_newlines_in_it_survives() {
        let raw = "MESSAGE\nx:y\n\nfirst\nsecond\0";
        assert_eq!(Frame::of_wire(raw).unwrap().body, "first\nsecond");
    }

    #[test]
    fn the_connect_frame_says_what_this_speaks_and_what_it_wants() {
        let frame = connect("clip.example.org");
        assert_eq!(frame.header("accept-version"), Some(VERSION));
        // `0,20000`: THIS SENDS NONE and asks the server for one every twenty
        // seconds. Sending our own would mean a timer, and the whole point of
        // this transport is that the socket is the clock.
        assert_eq!(frame.header("heart-beat"), Some("0,20000"));
        assert_eq!(frame.header("host"), Some("clip.example.org"));
    }

    #[test]
    fn what_goes_out_comes_back_the_same() {
        let one = send(
            "/app/cliptext",
            r#"{"nonce":"a","ciphertext":"b","tag":"c"}"#,
        );
        let back = Frame::of_wire(&one.to_wire()).unwrap();
        assert_eq!(back.command, one.command);
        assert_eq!(back.body, one.body);
        assert_eq!(back.header("destination"), Some("/app/cliptext"));
    }
}
