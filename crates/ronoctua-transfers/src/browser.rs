//! The browser's door into the queue — Chrome native messaging.
//!
//! # Why native messaging and not a port
//!
//! Because the alternative is an HTTP server on the loopback, and then the
//! question is who else on the machine may talk to it — which needs a shared
//! secret, which needs somewhere to keep it, which the extension can only read
//! by having it written into its own files. Native messaging has none of that:
//! Chrome starts this process itself, as the person, with a pipe. Nothing is
//! listening, so nothing can be reached.
//!
//! # The framing is Chrome's, and it is four bytes
//!
//! Each message is a little-endian `u32` length followed by that many bytes of
//! UTF-8 JSON, in both directions. It is not line-delimited like the queue's
//! own socket — a length prefix is what Chrome speaks, and translating between
//! the two is the whole job of this file.
//!
//! # What it may be asked for
//!
//! Adding to the queue, and nothing else. It cannot pause, cancel, list or
//! sweep: a page that got hold of the extension could otherwise cancel a copy
//! of forty gibibytes, and there is no reason for a browser to be able to do
//! that. The narrowness is the design.

use crate::job::{Identity, Want, Work};
use crate::wire::Ask;
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// The biggest message Chrome will send. Anything larger is a bug or an
/// attempt, and either way is refused before it is allocated.
const CEILING: u32 = 1024 * 1024;

/// What the extension may ask for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum Request {
    /// A plain link the browser was about to download itself.
    Fetch {
        /// Where to get it.
        url: String,
        /// Whose session. Absent means anonymous, which is the default and
        /// what an older build of the extension will send.
        #[serde(default)]
        identity: Identity,
    },
    /// The page in front of the person, for yt-dlp.
    Media {
        /// The page's address.
        url: String,
        /// Video, audio alone, or subtitles alone.
        want: Want,
        /// Whose session.
        #[serde(default)]
        identity: Identity,
    },
    /// A magnet or a `.torrent`.
    Torrent {
        /// The magnet, or an address that ends in `.torrent`.
        source: String,
    },
    /// Is anybody there? The extension asks this once, so its button can say
    /// whether the queue is up before the person clicks it.
    Ping,
}

/// What it is told back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Answer {
    /// Whether it was taken.
    pub ok: bool,
    /// Why not, when not. Never a path and never a directory listing: the
    /// browser has no business learning the shape of the disk.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

impl Answer {
    const fn yes() -> Self {
        Self {
            ok: true,
            why: None,
        }
    }

    fn no(why: impl Into<String>) -> Self {
        Self {
            ok: false,
            why: Some(why.into()),
        }
    }
}

/// Reads one message, or `None` at the end of the stream.
///
/// # Errors
///
/// On a short read, a length past [`CEILING`], or JSON that does not parse.
pub fn read_message<T: serde::de::DeserializeOwned>(from: &mut impl Read) -> Result<Option<T>> {
    let mut header = [0_u8; 4];
    match from.read_exact(&mut header) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_le_bytes(header);
    if len > CEILING {
        bail!("a message of {len} bytes; the ceiling is {CEILING}");
    }
    let mut body = vec![0_u8; len as usize];
    from.read_exact(&mut body)
        .context("the message ended before the announced size")?;
    Ok(Some(serde_json::from_slice(&body)?))
}

/// Writes one message with Chrome's framing.
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

/// What one request becomes, or nothing for a ping.
///
/// Kept apart from the plumbing so the mapping can be tested without a pipe —
/// and so the one place that decides which engine takes an address stays
/// [`crate::where_to::work_for`].
#[must_use]
pub fn work_of(request: &Request) -> Option<Work> {
    let into = crate::where_to::landing();
    match request {
        // A URL the browser was going to fetch could still be a `.torrent`
        // link, which is why this goes through the same resolver as every
        // other door rather than assuming.
        Request::Fetch { url, identity } => Some(crate::where_to::work_for(
            url,
            into,
            crate::where_to::Wish {
                want: None,
                identity: *identity,
            },
        )),
        Request::Media {
            url,
            want,
            identity,
        } => Some(crate::where_to::work_for(
            url,
            into,
            crate::where_to::Wish {
                want: Some(*want),
                identity: *identity,
            },
        )),
        Request::Torrent { source } => Some(Work::Torrent {
            source: source.clone(),
            into,
        }),
        Request::Ping => None,
    }
}

/// Reads requests off stdin and answers on stdout until the browser goes away.
///
/// # Errors
///
/// Only when the pipes themselves fail: a request that cannot be honoured is
/// answered with `ok: false` and the loop carries on, because Chrome keeps the
/// process for the life of the port and one bad message must not end it.
pub fn serve_stdio() -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut input = stdin.lock();
    let mut output = stdout.lock();

    while let Some(request) = read_message::<Request>(&mut input)? {
        let answer = match work_of(&request) {
            None => Answer::yes(),
            Some(work) => match crate::client::tell(&Ask::Add { work }) {
                Ok(()) => Answer::yes(),
                // The message is ours and says what is wrong with the QUEUE,
                // never what is on the disk: a browser that learned a path from
                // an error message learned something the person did not send.
                Err(e) => {
                    tracing::error!(error = %e, "the queue refused the browser's request");
                    Answer::no("the transfer queue is not up")
                }
            },
        };
        write_message(&mut output, &answer)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_framing_round_trips_and_stops_at_the_end() {
        let mut pipe = Vec::new();
        write_message(&mut pipe, &Request::Ping).expect("write");
        write_message(
            &mut pipe,
            &Request::Media {
                url: "https://exemplo/v".to_owned(),
                want: Want::Subtitles,
                identity: Identity::Anonymous,
            },
        )
        .expect("write");

        let mut reading = pipe.as_slice();
        assert_eq!(
            read_message::<Request>(&mut reading).expect("read"),
            Some(Request::Ping)
        );
        assert_eq!(
            read_message::<Request>(&mut reading).expect("read"),
            Some(Request::Media {
                url: "https://exemplo/v".to_owned(),
                want: Want::Subtitles,
                identity: Identity::Anonymous,
            })
        );
        // End of stream is `None` and NOT an error: Chrome closes the pipe when
        // the port closes, and a process that treated that as a failure would
        // fill the log every time a tab is shut.
        assert_eq!(
            read_message::<Request>(&mut reading).expect("the end"),
            None
        );
    }

    #[test]
    fn an_announced_length_past_the_ceiling_is_refused_before_it_is_allocated() {
        // Four bytes claiming four gibibytes. Without the ceiling this is a
        // `vec![0; 4_294_967_295]`, which is the whole attack.
        let mut absurd: Vec<u8> = u32::MAX.to_le_bytes().to_vec();
        absurd.extend_from_slice(b"{}");
        let mut reading = absurd.as_slice();
        assert!(read_message::<Request>(&mut reading).is_err());
    }

    #[test]
    fn a_truncated_message_is_an_error_and_not_a_silent_empty_one() {
        let mut short: Vec<u8> = 99_u32.to_le_bytes().to_vec();
        short.extend_from_slice(b"{\"kind\":\"ping\"}");
        let mut reading = short.as_slice();
        assert!(read_message::<Request>(&mut reading).is_err());
    }

    #[test]
    fn the_browser_can_only_add_and_never_act_on_a_row() {
        // The request type has four cases and none of them names a job id.
        // That is the guard: a page that got hold of the extension cannot
        // cancel a copy, because there is no message that says so.
        let text = serde_json::to_string(&Request::Ping).expect("json");
        assert_eq!(text, r#"{"kind":"ping"}"#);
        for forbidden in ["pause", "cancel", "forget", "sweep", "list"] {
            let attempt = format!("{{\"kind\":\"{forbidden}\",\"id\":1}}");
            assert!(
                serde_json::from_str::<Request>(&attempt).is_err(),
                "`{forbidden}` cannot exist at the browser's door"
            );
        }
    }

    #[test]
    fn a_torrent_link_from_the_browser_reaches_the_torrent_engine() {
        // The browser says "fetch" for everything it was about to download,
        // including a `.torrent` file. Sending that to aria2 would save the
        // file instead of downloading what it describes.
        let work = work_of(&Request::Fetch {
            url: "https://exemplo/debian.torrent".to_owned(),
            identity: Identity::Anonymous,
        });
        assert!(matches!(work, Some(Work::Torrent { .. })));
    }
}
