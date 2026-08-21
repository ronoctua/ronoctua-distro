//! The queue's daemon: one process, one socket, one list.
//!
//! # Why it is a `[[bin]]` and not a mode of the shell
//!
//! Two reasons, and each holds on its own.
//!
//! A derivation references what its BYTES cite. The shell's binary carries the
//! renderer, wgpu, the wasm runtime and every font path baked in; a unit that
//! only copies files has no business dragging any of that into its closure —
//! the same reasoning that made `ronoctua-files-root` and `ronoctua-rescue`
//! binaries of their own.
//!
//! And it has to outlive the shell. A copy of forty gibibytes must not die
//! because the footer was restarted, and in the nested development loop the
//! footer is restarted on every file edit.
//!
//! Everything it actually does is in `ronoctua_transfers::serve`, where a test
//! can reach it. What is left here is the two decisions a binary makes: where
//! the log goes and where the socket is.

use std::sync::Arc;

use anyhow::Result;
use ronoctua_transfers::ledger::Shared;
use ronoctua_transfers::{serve, wire};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_transfers=info".into()),
        )
        .init();

    serve::listen_on(&wire::socket_path()?, &Arc::new(Shared::fresh_one()))
}
