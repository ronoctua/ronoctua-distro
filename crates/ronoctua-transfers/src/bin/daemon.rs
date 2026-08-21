//! The queue's own process — and the two doors that lead into it.
//!
//! # Three shapes, one binary
//!
//! - no argument: **the unit**. It listens on the socket and owns the ledger.
//! - `open <address>`: what `xdg-open` runs when a **magnet** is clicked, or a
//!   `.torrent` opened. It adds and leaves; it does not wait for the download.
//! - `browser`: the **native-messaging host** Chrome starts. It translates
//!   Chrome's length-prefixed framing into the queue's own asks.
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

    match std::env::args().nth(1).as_deref() {
        None => serve::listen_on(&wire::socket_path()?, &Arc::new(Shared::fresh_one())),
        Some("open") => open(),
        Some("browser") => ronoctua_transfers::browser::serve_stdio(),
        Some(other) => {
            anyhow::bail!("nao sei fazer `{other}`; use `open <endereco>`, `browser`, ou nada")
        }
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
        .context("uso: ronoctua-transfers open <magnet|arquivo.torrent|endereco>")?;
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
        .context("a fila de transferencias nao aceitou o pedido")
}
