//! `ronoctua-ssh-askpass` / `ronoctua-sudo-askpass` — what ssh and sudo run as
//! their askpass to ask a secret in the footer's OWN bar
//! (`ronoctua_shell_footer::sudo_bar`'s ask), never a floating window.
//!
//! ONE PROGRAM, TWO NAMES. Its own name says the KIND: a name containing
//! `sudo` asks the system password, any other an SSH key's passphrase. It
//! writes one line to the footer's control socket — `askpass <kind> <prompt>`
//! (`ronoctua_shell_footer::leader::ASKPASS`, spelled again here: built on its
//! own, with no crate to share the word from) — and reads one line back: the
//! secret, which it prints and nothing else. A closed connection — the person
//! cancelled, or there is no footer — prints nothing and ends in failure, so
//! ssh/sudo gives up cleanly.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

/// The footer's control socket (`ronoctua_shell_kit::ipc::socket_path`,
/// spelled again — no `/tmp` fallback, by section 9).
fn socket_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_owned());
    Some(PathBuf::from(dir).join(format!("ronoctua-{display}.sock")))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    // THE KIND IS THIS PROGRAM'S OWN NAME: `…-sudo-askpass` asks the system
    // password, any other name an SSH key's passphrase. The footer titles the
    // bar by it (`sudo_bar::Kind`).
    let kind = args
        .first()
        .and_then(|a0| a0.rsplit('/').next())
        .map_or("ssh", |name| {
            if name.contains("sudo") { "sudo" } else { "ssh" }
        });
    // ssh/sudo pass the prompt as the one argument; keep it to a single line,
    // as the socket is line-based.
    let prompt = args
        .get(1)
        .map(|p| p.replace(['\n', '\r'], " "))
        .unwrap_or_default();
    let Some(path) = socket_path() else {
        return ExitCode::FAILURE;
    };
    let Ok(mut stream) = UnixStream::connect(&path) else {
        return ExitCode::FAILURE;
    };
    if writeln!(stream, "askpass {kind} {prompt}")
        .and_then(|()| stream.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    let mut line = String::new();
    match BufReader::new(&stream).read_line(&mut line) {
        // A secret came back on its own line (maybe empty, as some keys have
        // none): print just it.
        Ok(read) if read > 0 => {
            let secret = line.strip_suffix('\n').unwrap_or(&line);
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{secret}").and_then(|()| out.flush()).is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        // Cancelled, or no footer answered: nothing is printed, and ssh/sudo
        // gives up its prompt.
        _ => ExitCode::FAILURE,
    }
}
