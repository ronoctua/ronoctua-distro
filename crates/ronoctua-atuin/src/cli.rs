//! The `atuin` binary, and what it says.
//!
//! # Why the CLI and not the protocol
//!
//! Atuin's sync is end to end encrypted, and the key never leaves the machine.
//! Speaking the protocol would mean re-implementing the key derivation, the
//! record store and the crypto — to reach a server the binary this distro
//! already installs reaches in one call.
//!
//! The ClipCascade crate goes the other way, for the opposite reason: its
//! upstream client WATCHES THE CLIPBOARD, and a second watcher is a second
//! history. `atuin` watches nothing; it is asked.
//!
//! # Nothing here decides
//!
//! Every function is one call and one parse. The parsing is PURE, so the whole
//! of what this crate believes is testable against captured text with no
//! `atuin` on the machine — which a build sandbox is.

use anyhow::{Context as _, Result, bail};

/// The binary, baked at build time.
///
/// By its STORE PATH and not by name: this runs from a shell process whose
/// `PATH` is whatever the session has, and a bare name fails far from the
/// cause (AGENTS.md).
pub const ATUIN: &str = match option_env!("RONOCTUA_ATUIN") {
    Some(p) => p,
    None => "atuin",
};

/// What `atuin status` said.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Status {
    /// Is there an account on a server?
    pub logged_in: bool,
    /// The server's address, as Atuin prints it.
    pub address: String,
    /// The account's name.
    pub username: String,
    /// How often it syncs by itself.
    pub frequency: String,
    /// When it last did.
    pub last: String,
}

/// Reads `atuin status`.
///
/// A REFUSAL IS NOT AN ERROR HERE. `atuin status` on a machine that has never
/// logged in prints the local half and nothing else, which is a perfectly good
/// answer — and the view says "not logged in" rather than "something failed".
#[must_use]
pub fn status_of(said: &str) -> Status {
    let mut out = Status::default();
    let mut remote = false;
    for line in said.lines() {
        let line = line.trim();
        if line.eq_ignore_ascii_case("[remote]") {
            remote = true;
            continue;
        }
        if line.starts_with('[') {
            remote = false;
            continue;
        }
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        match name.trim().to_ascii_lowercase().as_str() {
            // THE ADDRESS CARRIES A COLON OF ITS OWN (`http://host:8888`), and
            // `split_once` is what keeps it whole: splitting on every colon
            // would leave `http` as the value.
            "address" if remote => out.address = value.to_owned(),
            "username" if remote => out.username = value.to_owned(),
            "sync frequency" => out.frequency = value.to_owned(),
            "last sync" => out.last = value.to_owned(),
            _ => {}
        }
    }
    out.logged_in = !out.username.is_empty();
    out
}

/// Asks the binary.
///
/// # Errors
///
/// The binary is not there. A NON-ZERO EXIT IS NOT AN ERROR: `atuin status`
/// exits non-zero when there is no account, and reading that as a failure
/// would make "not logged in" indistinguishable from "atuin is not installed".
pub fn status() -> Result<Status> {
    let out = std::process::Command::new(ATUIN)
        .arg("status")
        .output()
        .with_context(|| format!("{ATUIN} could not be run"))?;
    Ok(status_of(&String::from_utf8_lossy(&out.stdout)))
}

/// Syncs now.
///
/// # Errors
///
/// The binary is not there, or it refused — and its own words come back,
/// because "sync failed" sends a person nowhere.
pub fn sync() -> Result<()> {
    run(&["sync"])
}

/// Logs out, which forgets the key on this machine.
///
/// # Errors
///
/// As [`sync`].
pub fn logout() -> Result<()> {
    run(&["logout"])
}

/// Logs in to `address` as `username`.
///
/// # The key is a third thing
///
/// Atuin's login takes the password AND the encryption key, and they are not
/// the same: the password reaches the server, the key never does. A second
/// machine needs the key from the first — which is what `atuin key` prints and
/// what the view shows behind a question.
///
/// # Errors
///
/// As [`sync`].
pub fn login(address: &str, username: &str, password: &str, key: &str) -> Result<()> {
    // THE PASSWORD AND THE KEY DO NOT GO ON THE COMMAND LINE. `argv` is
    // readable by every process on the machine through `/proc/*/cmdline`, and
    // this is the whole of somebody's shell history. Atuin reads both from
    // stdin when they are not given as flags.
    let mut child = std::process::Command::new(ATUIN)
        .args(["login", "--username", username])
        .args(["--"])
        .env("ATUIN_SYNC_ADDRESS", address)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("{ATUIN} could not be run"))?;
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().context("no stdin for atuin")?;
        writeln!(stdin, "{password}")?;
        writeln!(stdin, "{key}")?;
    }
    let out = child.wait_with_output()?;
    if out.status.success() {
        return Ok(());
    }
    refuse(&out)
}

/// The encryption key, for moving to another machine.
///
/// # Errors
///
/// As [`sync`]. A machine that has not logged in has no key, and says so.
pub fn key() -> Result<String> {
    let out = std::process::Command::new(ATUIN)
        .arg("key")
        .output()
        .with_context(|| format!("{ATUIN} could not be run"))?;
    if !out.status.success() {
        return refuse(&out).map(|()| String::new());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// How many entries the history holds.
///
/// # Errors
///
/// As [`sync`].
pub fn how_many() -> Result<u64> {
    let out = std::process::Command::new(ATUIN)
        .args(["history", "list", "--cmd-only"])
        .output()
        .with_context(|| format!("{ATUIN} could not be run"))?;
    Ok(out.stdout.iter().filter(|b| **b == b'\n').count() as u64)
}

/// One call, and the binary's own words when it says no.
fn run(args: &[&str]) -> Result<()> {
    let out = std::process::Command::new(ATUIN)
        .args(args)
        .output()
        .with_context(|| format!("{ATUIN} could not be run"))?;
    if out.status.success() {
        return Ok(());
    }
    refuse(&out)
}

/// The refusal, in Atuin's words rather than in a status code.
fn refuse(out: &std::process::Output) -> Result<()> {
    let said = String::from_utf8_lossy(&out.stderr);
    let said = said.trim();
    if said.is_empty() {
        bail!("atuin exited with {}", out.status);
    }
    bail!("{said}");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from a real `atuin status` — WITH THE ADDRESS AND THE ACCOUNT
    /// REPLACED. The machine this was read on has a real server and a real
    /// username, and neither enters this repository (invariant 13). What is
    /// being held is the SHAPE.
    const SAID: &str = "Atuin v18.21.0 - Build rev NO_GIT\n\
                        \n\
                        [Local]\n\
                        Sync frequency: 5m\n\
                        Last sync: 2026-01-02 3:04:05.000000000 +00:00:00\n\
                        [Remote]\n\
                        Address: http://history.example.org:8888/\n\
                        Username: someone\n";

    #[test]
    fn the_address_survives_its_own_colon() {
        // `http://host:8888` HAS TWO COLONS, and splitting on every one of
        // them would leave `http` as the address — which reads as a server
        // that is not answering.
        let status = status_of(SAID);
        assert_eq!(status.address, "http://history.example.org:8888/");
        assert_eq!(status.username, "someone");
        assert_eq!(status.frequency, "5m");
        assert!(status.logged_in);
    }

    #[test]
    fn a_machine_that_never_logged_in_is_not_a_failure() {
        // `atuin status` prints the local half and nothing else, and exits
        // non-zero. Reading that as an error would make "not logged in"
        // indistinguishable from "atuin is not installed", which are acted on
        // very differently.
        let local = "Atuin v18.21.0\n\n[Local]\nSync frequency: 5m\nLast sync: never\n";
        let status = status_of(local);
        assert!(!status.logged_in);
        assert!(status.address.is_empty());
        assert_eq!(status.frequency, "5m");
    }

    #[test]
    fn a_local_field_is_not_read_as_a_remote_one() {
        // The two halves have keys that could collide; the section header is
        // what tells them apart, and a parser that ignored it would report a
        // local value as the server's.
        let odd = "[Local]\nAddress: not-the-server\n[Remote]\nAddress: http://real:1/\n";
        assert_eq!(status_of(odd).address, "http://real:1/");
    }

    #[test]
    fn nothing_at_all_is_read_without_panicking() {
        assert_eq!(status_of(""), Status::default());
        assert_eq!(status_of("garbage"), Status::default());
    }

    #[test]
    fn the_binary_is_a_path_when_nix_built_this() {
        // Outside Nix the fallback is a bare name, and that is deliberate:
        // `cargo build` keeps working. Inside, it must be a store path.
        if option_env!("RONOCTUA_ATUIN").is_some() {
            assert!(
                ATUIN.starts_with('/'),
                "a bare name fails at EXEC where PATH is minimal"
            );
        }
    }
}
