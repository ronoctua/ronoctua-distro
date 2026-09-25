//! The other side of the socket: how the shell asks, and how it listens.
//!
//! # Why there are two ways to connect, and only one of them may be slow
//!
//! Whoever sends a command — *run it now*, *not tonight* — is the loop that
//! DRAWS. It gets [`connect_quick`], which either finds the socket in
//! microseconds or gives up: a connect that could take two seconds because the
//! daemon needs starting would be two seconds of frozen footer, and this
//! project has paid for that lesson twice already (`systemd-run --scope`, and
//! the child process a buffer waited on).
//!
//! Whoever LISTENS runs on a thread of its own and has all the time in the
//! world. It gets [`watch`], which starts the daemon if nobody is home and
//! keeps trying if it goes away.
//!
//! # Why the client may start the daemon at all
//!
//! On the installed system it never needs to: a user service has it running
//! before the shell does. The starting is for the **nested development loop**,
//! where there is no unit at all — and a brain that only works on the installed
//! machine is a brain nobody tries until it is too late to change.

use std::io::{BufReader, BufWriter};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};

use crate::mind::Mind;
use crate::wire::{Ask, Say, read_line, socket_path, write_line};

/// How long to keep trying after starting the daemon.
const PATIENCE: Duration = Duration::from_secs(3);

/// How long between tries.
const AGAIN: Duration = Duration::from_millis(50);

/// The daemon's binary, found NEXT DOOR.
///
/// # Why not an `option_env!` with a store path
///
/// Because it would be a derivation referencing itself: the daemon and
/// everything that starts it come out of the SAME workspace build, so a
/// `RONOCTUA_BRAIN` set at compile time would have to name the output that is
/// still being produced. It is the wall `ronoctua-transfers::client` already
/// hit, with the same answer — what ships them together is that they are one
/// `bin/`, and asking the filesystem is the honest way to say so.
///
/// It is still never a bare NAME: under a systemd unit the `PATH` is minimal
/// and a name fails far from its cause. This resolves to an absolute path
/// before anything is spawned.
fn beside_me() -> Option<std::path::PathBuf> {
    let me = std::env::current_exe().ok()?;
    let it = me.parent()?.join("ronoctua-brain");
    it.exists().then_some(it)
}

/// Connects, or fails at once. For whoever is holding a frame.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, or nobody listening.
pub fn connect_quick() -> Result<UnixStream> {
    let path = socket_path()?;
    UnixStream::connect(&path)
        .with_context(|| format!("there is no brain listening at {}", path.display()))
}

/// Sends one ask and reads the one answer.
///
/// # Errors
///
/// Nobody listening, or the brain refused.
pub fn ask(what: &Ask) -> Result<Say> {
    ask_at(&socket_path()?, what)
}

/// The same, at a given path.
///
/// Split off for the reason `leader::listen_on` gives: `XDG_RUNTIME_DIR` is
/// process-global, and a test that changes it races every other test in the
/// same binary.
///
/// # Errors
///
/// Nobody listening there, or the brain refused.
pub fn ask_at(path: &Path, what: &Ask) -> Result<Say> {
    let stream = UnixStream::connect(path)
        .with_context(|| format!("there is no brain listening at {}", path.display()))?;
    // A daemon that stopped answering must not take the frame with it. On the
    // other side every ask is a lock, a change and a `writeln!`.
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let mut out = BufWriter::new(stream.try_clone().context("duplicate the socket")?);
    write_line(&mut out, what)?;
    let mut input = BufReader::new(stream);
    read_line(&mut input)?.context("the brain did not answer")
}

/// Sends one ask and does not wait to be told it worked.
///
/// # Errors
///
/// Nobody listening.
pub fn tell(what: &Ask) -> Result<()> {
    let mut out = BufWriter::new(connect_quick()?);
    write_line(&mut out, what)
}

/// The same, at a given path. See [`ask_at`].
///
/// # Errors
///
/// Nobody listening there.
pub fn tell_at(path: &Path, what: &Ask) -> Result<()> {
    let mut out = BufWriter::new(
        UnixStream::connect(path)
            .with_context(|| format!("there is no brain listening at {}", path.display()))?,
    );
    write_line(&mut out, what)
}

/// Listens forever, calling `each` with the whole mind every time it changes.
///
/// It starts the daemon when nobody is home and reconnects when it goes away,
/// so the caller's thread is a `loop` it never has to write. It only comes back
/// when `each` says to stop — which is what the shell does when it is quitting.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, which is the one failure retrying cannot fix.
pub fn watch(mut each: impl FnMut(Mind) -> bool) -> Result<()> {
    let path = socket_path()?;
    loop {
        match listen_once(&path, &mut each) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => tracing::debug!(error = %e, "the brain went down; trying again"),
        }
        // A daemon that is down is not an emergency: the screen simply has
        // nothing to draw. Sleeping between tries is what keeps a missing unit
        // from becoming a spin on one core for the length of the session.
        std::thread::sleep(AGAIN * 10);
    }
}

/// One connection's worth of listening. `true` when the caller asked to stop.
fn listen_once(path: &Path, each: &mut impl FnMut(Mind) -> bool) -> Result<bool> {
    let stream = connect_or_start(path)?;
    let mut out = BufWriter::new(stream.try_clone().context("duplicate the socket")?);
    write_line(&mut out, &Ask::Watch)?;
    let mut input = BufReader::new(stream);
    while let Some(said) = read_line::<Say>(&mut input)? {
        match said {
            Say::Mind { mind } => {
                if each(*mind) {
                    return Ok(true);
                }
            }
            // Nothing else belongs on a watch stream. Saying so in the log is
            // what keeps a protocol drift from looking like "the screen stopped
            // updating".
            other => tracing::warn!(?other, "an unexpected answer on the brain's stream"),
        }
    }
    Ok(false)
}

/// Follows the daemon's own log forever, calling `each` with every batch of
/// lines — the whole ring first, then only what is new.
///
/// It reconnects the way [`watch`] does, and starts the daemon the same way.
/// After a reconnection the ring arrives WHOLE again (a restarted daemon
/// numbers from one), so the caller replaces what it held when the first
/// line of a batch is not after its last.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, which is the one failure retrying cannot fix.
pub fn tail(mut each: impl FnMut(Vec<crate::tail::Line>) -> bool) -> Result<()> {
    let path = socket_path()?;
    loop {
        match tail_once(&path, &mut each) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => tracing::debug!(error = %e, "the brain went down; trying again"),
        }
        std::thread::sleep(AGAIN * 10);
    }
}

/// One connection's worth of following. `true` when the caller asked to stop.
fn tail_once(path: &Path, each: &mut impl FnMut(Vec<crate::tail::Line>) -> bool) -> Result<bool> {
    let stream = connect_or_start(path)?;
    let mut out = BufWriter::new(stream.try_clone().context("duplicate the socket")?);
    write_line(&mut out, &Ask::Tail)?;
    let mut input = BufReader::new(stream);
    while let Some(said) = read_line::<Say>(&mut input)? {
        match said {
            Say::Lines { lines } => {
                if each(lines) {
                    return Ok(true);
                }
            }
            other => tracing::warn!(?other, "an unexpected answer on the brain's log"),
        }
    }
    Ok(false)
}

/// Connects, starting the daemon if nobody is listening.
fn connect_or_start(path: &Path) -> Result<UnixStream> {
    if let Ok(s) = UnixStream::connect(path) {
        return Ok(s);
    }
    let Some(daemon) = beside_me() else {
        bail!("there is no `ronoctua-brain` beside this binary");
    };
    start(&daemon)?;
    let until = Instant::now() + PATIENCE;
    loop {
        if let Ok(s) = UnixStream::connect(path) {
            return Ok(s);
        }
        if Instant::now() >= until {
            bail!("the brain did not come up within {PATIENCE:?}");
        }
        std::thread::sleep(AGAIN);
    }
}

/// Starts the daemon, in a process group of its own.
///
/// The group matters: without it the daemon is in the group of whoever started
/// it, and a development loop that restarts the footer would take a rebuild in
/// flight with it — which is one of the things the daemon exists to prevent.
fn start(daemon: &Path) -> Result<()> {
    use std::os::unix::process::CommandExt as _;
    tracing::info!(daemon = %daemon.display(), "bringing the brain up");
    std::process::Command::new(daemon)
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("start {}", daemon.display()))?;
    Ok(())
}
