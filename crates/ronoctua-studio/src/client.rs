//! The other side of the socket: how the shell asks and how it listens.
//!
//! The rule `ronoctua-transfers::client` states and this copies: whoever holds
//! a frame gets [`tell`], which finds the socket in microseconds or gives up;
//! whoever LISTENS runs on a thread of its own and gets [`watch`], which starts
//! the daemon when nobody is home (the nested loop has no unit) and keeps
//! trying when it goes away.

use std::{
    io::{BufReader, BufWriter},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};

use crate::wire::{Ask, Say, State, read_line, socket_path, write_line};

/// The daemon's binary, found NEXT DOOR — the same workspace build ships both,
/// and a store path baked in at compile time would name the output still
/// being produced (`ronoctua_transfers::client::beside_me`'s reason).
fn beside_me() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    let it = me.parent()?.join("ronoctua-studio");
    it.exists().then_some(it)
}

/// One spawn at a time in this process: a window asks from two threads (its
/// state and its preview) in the same instant, and two daemons would race for
/// one socket — the second refuses, but only after a spawn nobody needed.
static SPAWNING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Starts the daemon if nobody answers, and waits a moment for it.
pub fn ensure() -> Result<PathBuf> {
    let path = socket_path()?;
    if UnixStream::connect(&path).is_ok() {
        return Ok(path);
    }
    let _one = SPAWNING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if UnixStream::connect(&path).is_ok() {
        return Ok(path);
    }
    // On the installed system the unit is already running and the connect
    // above answered; this is the nested loop's path, and a unit that died.
    if let Some(daemon) = beside_me() {
        let _ = std::process::Command::new(daemon)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .process_group(0)
            .spawn();
    }
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        if UnixStream::connect(&path).is_ok() {
            return Ok(path);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::bail!("the studio did not start")
}

use std::os::unix::process::CommandExt as _;

/// Sends one ask and does not wait: for whoever is holding a frame.
pub fn tell(what: &Ask) -> Result<()> {
    let stream = UnixStream::connect(socket_path()?).context("the studio is not running")?;
    let mut out = BufWriter::new(stream);
    write_line(&mut out, what)
}

/// Sends one ask and reads the answer.
pub fn ask(what: &Ask) -> Result<Say> {
    let stream = UnixStream::connect(ensure()?)?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut out = BufWriter::new(stream.try_clone()?);
    write_line(&mut out, what)?;
    read_line(&mut BufReader::new(stream))?.context("the studio did not answer")
}

/// Watches the state on a thread of its own: `on_state` gets every state and
/// `None` when the studio went away (it is then started again).
pub fn watch(on_state: impl Fn(Option<State>) + Send + 'static) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let gone = stop.clone();
    let _ = std::thread::Builder::new()
        .name("studio-watch".into())
        .spawn(move || {
            while !gone.load(Ordering::SeqCst) {
                let run = || -> Result<()> {
                    let stream = UnixStream::connect(ensure()?)?;
                    let mut out = BufWriter::new(stream.try_clone()?);
                    write_line(&mut out, &Ask::Watch)?;
                    let mut input = BufReader::new(stream);
                    while let Some(say) = read_line::<Say>(&mut input)? {
                        if gone.load(Ordering::SeqCst) {
                            return Ok(());
                        }
                        if let Say::State(s) = say {
                            on_state(Some(*s));
                        }
                    }
                    Ok(())
                };
                if let Err(e) = run() {
                    tracing::debug!(error = %e, "studio watch");
                }
                on_state(None);
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    stop
}

/// Opens the program's preview (see `preview.rs`), starting the daemon.
pub fn preview() -> Result<crate::preview::Feed> {
    crate::preview::open(&ensure()?)
}
