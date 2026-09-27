//! Chrome's native-messaging framing, and who is calling.
//!
//! # The framing is Chrome's, and it is four bytes
//!
//! Each message is a little-endian `u32` length followed by that many bytes of
//! UTF-8 JSON, in both directions. It is not line-delimited like our own
//! sockets — a length prefix is what Chrome speaks. Both hosts of this system
//! (the queue's and the kiosk's) read and write it through the two functions
//! here, so there is one reader to get right.
//!
//! # Who is calling: the first argument
//!
//! Chrome starts a host with the caller's ORIGIN as its first argument,
//! `chrome-extension://<id>/` (on Windows a `--parent-window=` follows; on
//! Linux nothing does). The manifest's `path` takes no arguments, so a host
//! that expected a word of its own there — the queue's did, `browser` — dies
//! on the first message from a real browser, and a test that passes the word
//! by hand never sees it.

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::ffi::OsString;
use std::io::{Read, Write};

/// The biggest message read by default: Chrome's own limit on what a host
/// may SEND is 1 MiB, and the queue's requests are a few hundred bytes.
/// Anything larger is a bug or an attempt, refused before it is allocated.
pub const CEILING: u32 = 1024 * 1024;

/// Reads one message of at most [`CEILING`] bytes, or `None` at the end of
/// the stream.
///
/// # Errors
///
/// On a short read, a length past the ceiling, or JSON that does not parse.
pub fn read_message<T: DeserializeOwned>(from: &mut impl Read) -> Result<Option<T>> {
    read_message_within(from, CEILING)
}

/// [`read_message`] with a ceiling of the caller's: the kiosk's state, with a
/// few hundred tabs, is larger than the queue's requests.
///
/// # Errors
///
/// On a short read, a length past `ceiling`, or JSON that does not parse.
pub fn read_message_within<T: DeserializeOwned>(
    from: &mut impl Read,
    ceiling: u32,
) -> Result<Option<T>> {
    let mut header = [0_u8; 4];
    match from.read_exact(&mut header) {
        Ok(()) => {}
        // End of stream is `None` and NOT an error: Chrome closes the pipe
        // when the port closes, and a process that treated that as a failure
        // would fill the log every time a tab is shut.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_le_bytes(header);
    if len > ceiling {
        bail!("a message of {len} bytes; the ceiling is {ceiling}");
    }
    let mut body = vec![0_u8; len as usize];
    from.read_exact(&mut body)
        .context("the message ended before the announced size")?;
    Ok(Some(serde_json::from_slice(&body)?))
}

/// Writes one message with Chrome's framing, and flushes it.
///
/// # Errors
///
/// When the pipe is gone, or the value does not serialise.
pub fn write_message<T: Serialize>(to: &mut impl Write, value: &T) -> Result<()> {
    let body = serde_json::to_vec(value)?;
    let len = u32::try_from(body.len()).context("a message too large for the framing")?;
    to.write_all(&len.to_le_bytes())?;
    to.write_all(&body)?;
    to.flush()?;
    Ok(())
}

/// Who started this process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// Chrome, for the extension with this origin (`chrome-extension://<id>/`).
    Chrome {
        /// The caller's origin, as Chrome passed it.
        origin: String,
    },
    /// A person or a test, with the word `browser` — the queue's host keeps
    /// it for use by hand.
    Hand,
    /// Anything else.
    Unknown,
}

/// Reads the caller from the WHOLE argument vector (program first): the
/// first argument decides, and whatever follows it is ignored — Chrome adds
/// `--parent-window=` on Windows only, and a later Chrome may add more.
#[must_use]
pub fn caller(args: &[OsString]) -> Caller {
    match args.get(1).and_then(|a| a.to_str()) {
        Some(o) if is_origin(o) => Caller::Chrome {
            origin: o.to_owned(),
        },
        Some("browser") => Caller::Hand,
        _ => Caller::Unknown,
    }
}

/// Whether this is an extension's origin as Chrome writes it: the scheme, 32
/// letters from `a` to `p`, and the closing slash — nothing before, nothing
/// after.
#[must_use]
pub fn is_origin(s: &str) -> bool {
    s.strip_prefix("chrome-extension://")
        .and_then(|rest| rest.strip_suffix('/'))
        .is_some_and(crate::url::is_extension_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Probe {
        kind: String,
        n: u32,
    }

    const ORIGIN: &str = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";

    #[test]
    fn the_framing_round_trips_and_stops_at_the_end() {
        let mut pipe = Vec::new();
        let one = Probe {
            kind: "ping".into(),
            n: 1,
        };
        let two = Probe {
            kind: "état".into(),
            n: 2,
        };
        write_message(&mut pipe, &one).expect("write");
        write_message(&mut pipe, &two).expect("write");
        // `{"kind":"ping","n":1}` is 21 bytes.
        assert_eq!(&pipe[..4], &21_u32.to_le_bytes(), "little-endian length");
        let mut reading = pipe.as_slice();
        assert_eq!(
            read_message::<Probe>(&mut reading).expect("read"),
            Some(one)
        );
        assert_eq!(
            read_message::<Probe>(&mut reading).expect("read"),
            Some(two)
        );
        assert_eq!(read_message::<Probe>(&mut reading).expect("the end"), None);
    }

    #[test]
    fn a_length_past_the_ceiling_is_refused_before_it_is_allocated() {
        // Four bytes claiming four gibibytes. Without the ceiling this is a
        // `vec![0; 4_294_967_295]`, which is the whole attack.
        let mut absurd: Vec<u8> = u32::MAX.to_le_bytes().to_vec();
        absurd.extend_from_slice(b"{}");
        assert!(read_message::<serde_json::Value>(&mut absurd.as_slice()).is_err());
        // And a ceiling of the caller's is the one that holds.
        let mut small = Vec::new();
        write_message(&mut small, &serde_json::json!({"a": "0123456789"})).expect("write");
        assert!(read_message_within::<serde_json::Value>(&mut small.as_slice(), 8).is_err());
        assert!(read_message_within::<serde_json::Value>(&mut small.as_slice(), 64).is_ok());
    }

    #[test]
    fn a_truncated_message_is_an_error_and_not_a_silent_empty_one() {
        let mut short: Vec<u8> = 99_u32.to_le_bytes().to_vec();
        short.extend_from_slice(b"{\"kind\":\"ping\"}");
        assert!(read_message::<serde_json::Value>(&mut short.as_slice()).is_err());
    }

    fn argv(args: &[&str]) -> Vec<OsString> {
        std::iter::once("/nix/store/x/bin/host")
            .chain(args.iter().copied())
            .map(OsString::from)
            .collect()
    }

    #[test]
    fn chromes_origin_is_the_caller_and_what_follows_is_ignored() {
        assert_eq!(
            caller(&argv(&[ORIGIN])),
            Caller::Chrome {
                origin: ORIGIN.into()
            }
        );
        assert_eq!(
            caller(&argv(&[ORIGIN, "--parent-window=0"])),
            Caller::Chrome {
                origin: ORIGIN.into()
            }
        );
        assert_eq!(caller(&argv(&["browser"])), Caller::Hand);
    }

    #[test]
    fn anything_else_is_no_caller() {
        for garbage in [
            "",
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop",
            "chrome-extension://abcdefghijklmnopabcdefghijklmnoq/",
            "chrome-extension://short/",
            "https://abcdefghijklmnopabcdefghijklmnop/",
            " chrome-extension://abcdefghijklmnopabcdefghijklmnop/",
            "chrome-extension://abcdefghijklmnopabcdefghijklmnop/x",
            "open",
        ] {
            assert_eq!(caller(&argv(&[garbage])), Caller::Unknown, "{garbage:?}");
        }
        assert_eq!(caller(&argv(&[])), Caller::Unknown);
        assert!(is_origin(ORIGIN));
    }
}
