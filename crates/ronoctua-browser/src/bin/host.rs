//! `ronoctua-browser` — the native-messaging host Chrome starts for our
//! extension while a kiosk window exists (see [`ronoctua_browser::host`]).
//!
//! Chrome runs it with the extension's origin as the only argument, and
//! nothing else is accepted: this program has no use by hand, and a word it
//! does not know is a usage line and status 2, never a guess. Its log goes to
//! stderr, which Chrome passes through to its own; stdout is Chrome's pipe
//! and carries nothing but frames.

use ronoctua_browser::native::{self, Caller};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<std::ffi::OsString> = std::env::args_os().collect();
    let Caller::Chrome { origin } = native::caller(&args) else {
        eprintln!("usage: ronoctua-browser chrome-extension://<id>/  (started by Chrome)");
        return ExitCode::from(2);
    };
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_browser=info".into()),
        )
        .init();
    tracing::info!(%origin, "started by Chrome");
    // The end of stdin is the end of the port, and returning from `main`
    // ends the footer's thread with it.
    match ronoctua_browser::host::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            tracing::error!(error = %e, "the extension's pipe broke");
            ExitCode::FAILURE
        }
    }
}
