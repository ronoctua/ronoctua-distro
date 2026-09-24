//! The client's half: what the shell links — never the engine.

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

/// `systemctl` by path where the build baked it, since a unit's `PATH` is
/// minimal; by name elsewhere.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(p) => p,
    None => "systemctl",
};

/// The daemon's binary, found NEXT DOOR — the same workspace build ships both
/// (`ronoctua_studio::client`'s reason).
fn beside_me() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    let it = me.parent()?.join("ronoctua-player");
    it.exists().then_some(it)
}

/// The player is not on this machine: its unit does not exist.
///
/// A profile can leave it out (`ronoctua.player.enable`), and "not answering,
/// it starts on its own" would then be a sentence that is not true. The
/// shell asks for this by type and says so.
#[derive(Debug)]
pub struct NotInstalled;

impl std::fmt::Display for NotInstalled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the player is not installed on this machine")
    }
}

impl std::error::Error for NotInstalled {}

/// `systemctl`'s answer for a unit it does not know.
const UNIT_NOT_FOUND: i32 = 5;

/// One spawn at a time in this process: two threads asking at once would
/// race two daemons for one socket.
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
        use std::os::unix::process::CommandExt as _;
        let mut daemon = std::process::Command::new(daemon);
        // THE SHELL THAT STARTED IT, for MPRIS `Raise`: the unit names the
        // installed one, and here the one asking is the one to raise.
        if let Ok(me) = std::env::current_exe() {
            daemon.env("RONOCTUA_SHELL", me);
        }
        let _ = daemon
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .process_group(0)
            .spawn();
    } else if ronoctua_xdg::instance().is_none() {
        // THE INSTALLED SYSTEM'S UNIT, asked to come back: the daemon is not
        // beside this program there (`rust.player` is a package of its own),
        // and a player that quit — MPRIS `Quit` — or crashed past its restarts
        // would otherwise stay down until the next login. A user unit: nothing
        // is asked of anyone. The nested loop never asks, because the unit is
        // the INSTALLED player and the loop is a second copy with its own.
        let asked = std::process::Command::new(SYSTEMCTL)
            .args(["--user", "start", "--no-block", "ronoctua-player.service"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if asked.is_ok_and(|s| s.code() == Some(UNIT_NOT_FOUND)) {
            return Err(NotInstalled.into());
        }
    }
    let until = Instant::now() + Duration::from_secs(3);
    while Instant::now() < until {
        if UnixStream::connect(&path).is_ok() {
            return Ok(path);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    anyhow::bail!("the player did not start")
}

/// Whether the daemon is up, without starting it.
#[must_use]
pub fn running() -> bool {
    socket_path().is_ok_and(|p| UnixStream::connect(p).is_ok())
}

/// Sends one ask and does not wait.
pub fn tell(what: &Ask) -> Result<()> {
    let stream = UnixStream::connect(ensure()?).context("the player is not running")?;
    let mut out = BufWriter::new(stream);
    write_line(&mut out, what)
}

/// Sends one ask and reads the answer.
pub fn ask(what: &Ask) -> Result<Say> {
    let stream = UnixStream::connect(ensure()?)?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut out = BufWriter::new(stream.try_clone()?);
    write_line(&mut out, what)?;
    read_line(&mut BufReader::new(stream))?.context("the player did not answer")
}

/// Watches the state on a thread of its own: `on_state` gets every state and
/// `None` when the player went away. It does NOT start the player: a footer
/// watching is not a reason for a player to exist, and the mark on the
/// modeline is exactly "there is a player".
pub fn watch(on_state: impl Fn(Option<State>) + Send + 'static) -> Arc<AtomicBool> {
    let stop = Arc::new(AtomicBool::new(false));
    let gone = stop.clone();
    let _ = std::thread::Builder::new()
        .name("player-watch".into())
        .spawn(move || {
            let mut was_there = true;
            while !gone.load(Ordering::SeqCst) {
                let run = || -> Result<()> {
                    let stream = UnixStream::connect(socket_path()?)?;
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
                let r = run();
                // SAY IT ONCE when it goes away, not every second it stays gone.
                if r.is_err() && was_there {
                    on_state(None);
                }
                was_there = r.is_ok();
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    stop
}

/// Frames of `width × height` of `of` (see [`Ask::Preview`]), starting the
/// daemon.
pub fn preview(width: u32, height: u32, of: Option<usize>) -> Result<ronoctua_frames::Feed> {
    let path = ensure()?;
    let mut socket = UnixStream::connect(&path)?;
    write_line(&mut socket, &Ask::Preview { width, height, of })?;
    let (line, fd) = ronoctua_frames::receive(&socket)?;
    let say: Say = serde_json::from_str(&line).context("the answer is not a preview")?;
    let Say::Preview {
        width,
        height,
        slots,
    } = say
    else {
        anyhow::bail!("the player refused the preview: {line}");
    };
    let fd = fd.context("the preview came without its memory")?;
    Ok(ronoctua_frames::Feed::new(socket, fd, width, height, slots))
}
