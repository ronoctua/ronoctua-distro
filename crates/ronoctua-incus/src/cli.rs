//! The CLI, for what lives in the CLIENT's configuration: its remotes, and
//! a remote's image catalogue (which the client fetches, not the daemon).
//!
//! # Why the CLI here and not the socket
//!
//! The remotes are the person's `~/.config/incus/config.yml`, which the
//! daemon never sees; and a simplestreams catalogue is an index the CLIENT
//! downloads and filters. Reading either by hand would be a second Incus
//! client. Everything else goes through the socket.
//!
//! # A deadline and a ceiling
//!
//! A catalogue is a request to a host on the internet, by a program that sets
//! no timeout of its own: the child is killed at [`CATALOGUE_PATIENCE`], and
//! its output is read up to [`OUTPUT_MOST_BYTES`] — a door's thread is never
//! held for ever, and an answer is never held whole past what is shown.

use std::io::Read as _;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use ronoctua_consent::Pass;

use crate::{Offered, Refusal, Remote, Why, parse};

/// How long the remotes may take: a file read.
const REMOTES_PATIENCE: Duration = Duration::from_secs(10);

/// How long a catalogue may take: a download of an index.
const CATALOGUE_PATIENCE: Duration = Duration::from_secs(120);

/// The most of the CLI's output read.
const OUTPUT_MOST_BYTES: u64 = 64 << 20;

/// The most of its stderr kept, for the log.
const ERR_MOST_BYTES: u64 = 64 << 10;

/// How often a running child is looked at.
const LOOK: Duration = Duration::from_millis(50);

/// The architecture as the daemon names it (`x86_64`, `aarch64`): the
/// kernel's, which is Rust's for the two this distro builds for.
#[must_use]
pub(crate) const fn architecture() -> &'static str {
    std::env::consts::ARCH
}

/// Runs the CLI to its end, or to `patience`: its stdout.
///
/// # Errors
///
/// [`Why::NoIncus`] when it cannot start; [`Why::Failed`] when it failed,
/// outlived `patience` (killed) or said more than the ceiling — its own
/// words to the log.
pub(crate) fn run(cli: &Path, args: &[&str], patience: Duration) -> Result<Vec<u8>, Refusal> {
    let mut child = Command::new(cli)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            tracing::warn!(error = %e, "the incus CLI could not start");
            Refusal::of(Why::NoIncus)
        })?;
    // READERS ON THREADS, so a child that fills a pipe while this waits is
    // never a deadlock, and the deadline stays answerable.
    let out = child
        .stdout
        .take()
        .map(|o| reading(o, OUTPUT_MOST_BYTES + 1));
    let err = child.stderr.take().map(|e| reading(e, ERR_MOST_BYTES));
    let until = Instant::now() + patience;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < until => std::thread::sleep(LOOK),
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let verb = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
    let Some(status) = status else {
        // THE READERS ARE LEFT, not joined: something the child started may
        // still hold its pipe, and joining would be the wait the deadline
        // exists to end. They end when the pipe does.
        tracing::warn!(verb, "the incus CLI outlived its patience and was killed");
        return Err(Refusal::of(Why::Failed));
    };
    let out = out.and_then(|h| h.join().ok()).unwrap_or_default();
    let err = err.and_then(|h| h.join().ok()).unwrap_or_default();
    if status.success() && (out.len() as u64) <= OUTPUT_MOST_BYTES {
        return Ok(out);
    }
    tracing::warn!(
        verb,
        %status,
        said = String::from_utf8_lossy(&err).trim(),
        "the incus CLI failed"
    );
    Err(Refusal::of(Why::Failed))
}

/// A pipe read to its end (or its ceiling) on a thread of its own.
fn reading<R: std::io::Read + Send + 'static>(
    from: R,
    most: u64,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        let _ = from.take(most).read_to_end(&mut kept);
        kept
    })
}

/// The client's remotes that serve images.
///
/// # Errors
///
/// As [`run`]; [`Why::Failed`] for an answer this build does not read.
pub(crate) fn remotes(cli: &Path) -> Result<Vec<Remote>, Refusal> {
    parse::remotes_of(&run(
        cli,
        &["remote", "list", "--format", "json"],
        REMOTES_PATIENCE,
    )?)
}

/// A remote's catalogue, for this machine's architecture — asked of the
/// remote by the CLI, so ONLY under passes for every host its client asks
/// ([`crate::hosts_of`]): `pass` for the remote's address, and one `minter`
/// answers for each other host. Checked before the CLI starts.
///
/// # The architecture is OURS to filter
///
/// The CLI is asked for the whole catalogue, and [`parse::catalogue_of`]
/// keeps this machine's architecture by the image's top-level field
/// (`x86_64`). A `key=value` argument would not mean that: for a
/// simplestreams remote the CLI filters on the client, against the image's
/// PROPERTIES, whose `architecture` is the distribution's word (`amd64`) —
/// `architecture=x86_64` there keeps nothing, and the view would have nothing
/// to launch and no refusal to say.
///
/// # Errors
///
/// [`Why::Gone`] for a remote the client does not have; [`Why::NotLet`]
/// when a host of the remote's is not let; as [`remotes`].
pub(crate) fn catalogue(
    cli: &Path,
    pass: &Pass,
    remote: &str,
    minter: &dyn Fn(&str) -> Option<Pass>,
) -> Result<Vec<Offered>, Refusal> {
    let found = remotes(cli)?
        .into_iter()
        .find(|r| r.name == remote)
        .ok_or(Refusal::of(Why::Gone))?;
    let hosts = crate::hosts_of(&found.address, found.protocol);
    let others = hosts.get(1..).unwrap_or_default();
    crate::client::covered(&hosts, &crate::client::passes_for(pass, others, minter))?;
    let named = format!("{}:", found.name);
    let out = run(
        cli,
        &["image", "list", &named, "--format", "json"],
        CATALOGUE_PATIENCE,
    )?;
    parse::catalogue_of(&out, architecture())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use std::path::PathBuf;

    /// An executable file written by a child `/bin/sh` and never by this
    /// process: a file this process holds open for writing while another
    /// test's thread forks is inherited by that fork until it execs, and
    /// running the file in that window fails with `ETXTBSY`.
    fn write_executable(path: &Path, body: &str) {
        let status = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "printf '%s' \"$2\" > \"$1\" && chmod 700 \"$1\"",
                "sh",
            ])
            .arg(path)
            .arg(body)
            .status()
            .unwrap();
        assert!(status.success(), "{}", path.display());
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-incus-cli-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A stand-in CLI: a script that answers the remotes from the fixture,
    /// records every call, and answers a catalogue from the other.
    fn stand_in(dir: &Path) -> PathBuf {
        std::fs::write(dir.join("remotes.json"), fixtures::REMOTES).unwrap();
        std::fs::write(dir.join("catalogue.json"), fixtures::CATALOGUE).unwrap();
        let program = dir.join("incus");
        write_executable(
            &program,
            &format!(
                "#!/bin/sh\necho \"$*\" >> '{d}/calls'\ncase \"$1 $2\" in\n  'remote list') cat '{d}/remotes.json' ;;\n  'image list') cat '{d}/catalogue.json' ;;\n  *) echo \"Error: unknown\" >&2; exit 1 ;;\nesac\n",
                d = dir.display()
            ),
        );
        program
    }

    fn pass_for(dir: &Path, host: &str) -> Pass {
        let store = ronoctua_consent::store_at(Some(dir.join("consent")));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        ronoctua_consent::pass_in(&store, host).unwrap()
    }

    /// FAILS IF: the CLI is asked for a remote's catalogue — a request to
    /// that remote — under a pass for another host, or for Docker Hub's
    /// under its address's alone; or the CLI is handed a `key=value` filter,
    /// which it reads against the image's properties (`amd64`), not the
    /// architecture the daemon says (`x86_64`).
    #[test]
    fn a_catalogue_is_fetched_only_under_the_remote_s_pass() {
        let dir = scratch("catalogue");
        let cli = stand_in(&dir);
        let nobody = |_: &str| None;
        let other = pass_for(&dir, "example.org");
        assert_eq!(
            catalogue(&cli, &other, "images", &nobody),
            Err(Refusal::of(Why::NotLet))
        );
        let hub = pass_for(&dir, "docker.io");
        assert_eq!(
            catalogue(&cli, &hub, "docker", &nobody),
            Err(Refusal::of(Why::NotLet)),
            "Docker Hub's client asks two hosts more"
        );
        let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
        assert!(
            !calls.contains("image list"),
            "the CLI was asked anyway:\n{calls}"
        );
        let right = pass_for(&dir, "images.linuxcontainers.org");
        assert_eq!(
            catalogue(&cli, &right, "nowhere", &nobody),
            Err(Refusal::of(Why::Gone))
        );
        let offered = catalogue(&cli, &right, "images", &nobody).unwrap();
        if architecture() == "x86_64" {
            assert_eq!(
                offered.len(),
                3,
                "the architecture is kept here, by the top-level field"
            );
        }
        let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
        let listed: Vec<&str> = calls
            .lines()
            .filter(|c| c.starts_with("image list"))
            .collect();
        assert_eq!(
            listed,
            ["image list images: --format json"],
            "the whole catalogue, no filter handed to the CLI"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_cli_that_fails_hangs_or_is_missing_says_so() {
        let dir = scratch("fails");
        let cli = stand_in(&dir);
        assert_eq!(
            run(&cli, &["nonsense"], Duration::from_secs(5)),
            Err(Refusal::of(Why::Failed))
        );
        assert_eq!(
            run(
                &dir.join("absent"),
                &["remote", "list"],
                Duration::from_secs(5)
            ),
            Err(Refusal::of(Why::NoIncus))
        );
        let slow = dir.join("slow");
        write_executable(&slow, "#!/bin/sh\nexec sleep 30\n");
        let started = Instant::now();
        assert_eq!(
            run(&slow, &[], Duration::from_millis(300)),
            Err(Refusal::of(Why::Failed))
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the deadline held"
        );
        assert_eq!(remotes(&cli).unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
