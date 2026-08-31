//! The other side of the socket: how the shell asks and how it listens.
//!
//! # Why there are two ways to connect, and only one of them may be slow
//!
//! Whoever sends a command — *pause 7*, *paste these here* — is the loop that
//! draws. It gets [`connect_quick`], which either finds the socket in
//! microseconds or gives up: a connect that could take two seconds because the
//! daemon needs starting would be two seconds of frozen footer, and this
//! project has paid for that lesson twice already (`systemd-run --scope`, and
//! the child process the buffer waited on).
//!
//! Whoever LISTENS runs on a thread of its own and has all the time in the
//! world. It gets [`watch`], which starts the daemon if nobody is home and
//! keeps trying if it goes away. A command that arrives while the daemon is
//! down fails once and says so; by the next one the listener has it back up.
//!
//! # Why the client may start the daemon at all
//!
//! On the installed system it never needs to: `nix/modules/transfers.nix`
//! installs a user service and systemd has it running before the shell does.
//! The starting is for the **nested development loop**, where there is no unit
//! at all — and a queue that only works on the installed machine is a queue
//! that is never tried until it is too late to change.

use std::io::{BufReader, BufWriter};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};

use crate::job::Job;
use crate::wire::{Ask, Say, read_line, socket_path, write_line};

/// The daemon's binary, found NEXT DOOR.
///
/// # Why not an `option_env!` with a store path
///
/// Because it would be a derivation referencing itself, which Nix cannot
/// express: the daemon and everything that starts it come out of the SAME
/// workspace build, so a `RONOCTUA_TRANSFERS` set at compile time would have to
/// name the output that is still being produced. It is the same wall
/// `open_rescue` hit, and the same answer — what ships them together is that
/// they are one `bin/`, and asking the filesystem is the honest way to say so.
///
/// It is still never a bare NAME. Under a systemd unit the `PATH` is minimal
/// and a name fails far from its cause; this resolves to an absolute path
/// before anything is spawned.
fn beside_me() -> Option<std::path::PathBuf> {
    let me = std::env::current_exe().ok()?;
    let it = me.parent()?.join("ronoctua-transfers");
    it.exists().then_some(it)
}

/// How long to keep trying after starting the daemon.
const PATIENCE: Duration = Duration::from_secs(3);

/// How long between tries.
const AGAIN: Duration = Duration::from_millis(50);

/// Connects, or fails at once. For whoever is holding a frame.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, or nobody listening.
pub fn connect_quick() -> Result<UnixStream> {
    let path = socket_path()?;
    UnixStream::connect(&path)
        .with_context(|| format!("there is no transfer queue listening at {}", path.display()))
}

/// Sends one ask and reads the one answer.
///
/// # Errors
///
/// Nobody listening, or the daemon refused.
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
/// Nobody listening there, or the daemon refused.
pub fn ask_at(path: &Path, what: &Ask) -> Result<Say> {
    let stream = UnixStream::connect(path)
        .with_context(|| format!("there is no queue listening at {}", path.display()))?;
    // A daemon that stopped answering must not take the frame with it. On the
    // other side every ask is a lock, a push and a `writeln!`.
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let mut out = BufWriter::new(stream.try_clone().context("duplicate the socket")?);
    write_line(&mut out, what)?;
    let mut input = BufReader::new(stream);
    read_line(&mut input)?.context("the transfer queue did not answer")
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
            .with_context(|| format!("there is no queue listening at {}", path.display()))?,
    );
    write_line(&mut out, what)
}

/// Listens forever, calling `each` with the whole list every time it changes.
///
/// It starts the daemon when nobody is home and reconnects when it goes away,
/// so the caller's thread is a `loop` it never has to write. It only comes back
/// when `each` says to stop — which is what the shell does when it is quitting.
///
/// # Errors
///
/// `XDG_RUNTIME_DIR` absent, which is the one failure retrying cannot fix.
pub fn watch(mut each: impl FnMut(Vec<Job>) -> bool) -> Result<()> {
    let path = socket_path()?;
    loop {
        match listen_once(&path, &mut each) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => {
                tracing::debug!(error = %e, "the transfer queue went down; trying again")
            }
        }
        // A daemon that is down is not an emergency: the modeline simply has no
        // segment. Sleeping between tries is what keeps a missing unit from
        // becoming a spin on one core for the length of the session.
        std::thread::sleep(AGAIN * 10);
    }
}

/// One connection's worth of listening. `true` when the caller asked to stop.
fn listen_once(path: &Path, each: &mut impl FnMut(Vec<Job>) -> bool) -> Result<bool> {
    let stream = connect_or_start(path)?;
    let mut out = BufWriter::new(stream.try_clone().context("duplicate the socket")?);
    write_line(&mut out, &Ask::Watch)?;
    let mut input = BufReader::new(stream);
    while let Some(said) = read_line::<Say>(&mut input)? {
        match said {
            Say::Ledger { jobs } => {
                if each(jobs) {
                    return Ok(true);
                }
            }
            // Nothing else belongs on a watch stream. Saying so in the log is
            // what keeps a protocol drift from looking like "the panel stopped
            // updating".
            other => tracing::warn!(?other, "an unexpected answer on the transfers stream"),
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
        bail!("there is no `ronoctua-transfers` beside this binary");
    };
    start(&daemon)?;
    let until = Instant::now() + PATIENCE;
    loop {
        if let Ok(s) = UnixStream::connect(path) {
            return Ok(s);
        }
        if Instant::now() >= until {
            bail!("the transfer queue did not come up within {PATIENCE:?}");
        }
        std::thread::sleep(AGAIN);
    }
}

/// Starts the daemon, in a process group of its own.
///
/// The group matters: without it the daemon is in the group of whoever started
/// it, and a development loop that restarts the footer would take the copy with
/// it — which is the one thing the daemon exists to prevent.
fn start(daemon: &Path) -> Result<()> {
    use std::os::unix::process::CommandExt as _;
    tracing::info!(daemon = %daemon.display(), "bringing the transfer queue up");
    std::process::Command::new(daemon)
        .process_group(0)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("start {}", daemon.display()))?;
    Ok(())
}
