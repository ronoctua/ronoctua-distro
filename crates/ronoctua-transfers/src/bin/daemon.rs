//! The queue's own process — and the two doors that lead into it.
//!
//! # Three shapes, one binary
//!
//! - no argument: **the unit**. It listens on the socket and owns the ledger.
//! - `open <address>`: what `xdg-open` runs when a **magnet** is clicked, or a
//!   `.torrent` opened. It adds and leaves; it does not wait for the download.
//! - `chrome-extension://<id>/`: the **native-messaging host**, as Chrome
//!   starts it — the caller's origin is the first argument, because the
//!   manifest's `path` carries none. It translates Chrome's length-prefixed
//!   framing into the queue's own asks. `browser` is the same door by hand.
//!
//! One binary because all three need the same model, the same socket and the
//! same resolver for "which engine takes this" — and three binaries would be
//! three closures of the same size in the ISO.

use anyhow::{Context as _, Result};
use ronoctua_transfers::{Shared, serve, wire};
use std::sync::Arc;

fn main() -> Result<()> {
    // The unit's log goes to the journal; the two short-lived doors are quiet
    // unless something is wrong, because `xdg-open` has nowhere to show a line
    // and Chrome treats anything on stdout as a message.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_transfers=info".into()),
        )
        .init();

    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    match mode_of(&args) {
        Mode::Unit => serve::listen_on(&wire::socket_path()?, &Arc::new(Shared::fresh_one())),
        Mode::Open => open(),
        Mode::Browser => ronoctua_transfers::browser::serve_stdio(),
        Mode::Unknown(other) => {
            anyhow::bail!(
                "I do not know how to do `{other}`; use `open <address>`, `browser`, or nothing"
            )
        }
    }
}

/// Which of the three shapes this process is.
#[derive(Debug, PartialEq, Eq)]
enum Mode {
    /// No argument: the unit.
    Unit,
    /// `open <address>`.
    Open,
    /// The browser's door: Chrome's origin as the first argument (what a real
    /// browser passes), or `browser` by hand.
    Browser,
    /// Anything else, as it was given.
    Unknown(String),
}

/// The shape, from the WHOLE argument vector (program first).
///
/// THE ORIGIN IS THE BROWSER'S ARGUMENT. Chrome starts a host with the
/// caller's origin as argv[1] and the manifest cannot add a word of ours, so
/// accepting only `browser` meant the host died on every message from a real
/// browser — hidden by a test that passed `browser` by hand.
fn mode_of(args: &[std::ffi::OsString]) -> Mode {
    use ronoctua_browser::native::{Caller, caller};
    match caller(args) {
        Caller::Chrome { .. } | Caller::Hand => return Mode::Browser,
        Caller::Unknown => {}
    }
    match args.get(1) {
        None => Mode::Unit,
        Some(a) if a == "open" => Mode::Open,
        Some(other) => Mode::Unknown(other.to_string_lossy().into_owned()),
    }
}

/// Adds whatever address was handed over, and leaves.
///
/// It does NOT wait: a magnet clicked in a browser or a chat window must not
/// leave a process hanging around for the life of the download. What watches it
/// is the unit, which is already running or is started by the client.
fn open() -> Result<()> {
    let address = std::env::args()
        .nth(2)
        .context("usage: ronoctua-transfers open <magnet|file.torrent|address>")?;
    let work = ronoctua_transfers::where_to::work_for(
        &address,
        ronoctua_transfers::where_to::landing(),
        // PLAIN, AND ANONYMOUS. A `.desktop` handler is given an address and
        // never an intent: a page with a video on it is fetched as a page, and
        // nothing here says who is asking. Both of those are choices somebody
        // makes at the keyboard — `LEADER t v` and `LEADER t i v` — and a
        // handler that guessed either would be guessing about anonymity.
        ronoctua_transfers::where_to::Wish::plain(),
    );
    ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work })
        .context("the transfer queue did not take the request")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(args: &[&str]) -> Vec<std::ffi::OsString> {
        std::iter::once("ronoctua-transfers")
            .chain(args.iter().copied())
            .map(std::ffi::OsString::from)
            .collect()
    }

    #[test]
    fn chromes_origin_opens_the_browsers_door() {
        let origin = "chrome-extension://abcdefghijklmnopabcdefghijklmnop/";
        assert_eq!(mode_of(&argv(&[origin])), Mode::Browser);
        assert_eq!(
            mode_of(&argv(&[origin, "--parent-window=0"])),
            Mode::Browser
        );
        assert_eq!(mode_of(&argv(&["browser"])), Mode::Browser);
    }

    #[test]
    fn the_other_shapes_are_what_they_were() {
        assert_eq!(mode_of(&argv(&[])), Mode::Unit);
        assert_eq!(mode_of(&argv(&["open", "magnet:?xt=x"])), Mode::Open);
        assert_eq!(
            mode_of(&argv(&["chrome-extension://nope/"])),
            Mode::Unknown("chrome-extension://nope/".into())
        );
    }
}
