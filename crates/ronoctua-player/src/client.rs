//! The client's half: what the shell links — never the engine.

use std::{
    io::{BufRead, BufReader, BufWriter},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};

use crate::queue::Outline;
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
        if ask_the_unit() == Some(UNIT_NOT_FOUND) {
            return Err(NotInstalled.into());
        }
        return wait_for(&path, true);
    }
    wait_for(&path, false)
}

/// Asks the installed unit to start, without waiting for it; `systemctl`'s
/// exit code.
fn ask_the_unit() -> Option<i32> {
    std::process::Command::new(SYSTEMCTL)
        .args(["--user", "start", "--no-block", "ronoctua-player.service"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()
        .and_then(|s| s.code())
}

/// How long a player that said farewell may still be a process: its socket
/// is off the disk at once, and the process leaves a moment later (the
/// engine's `LEAVING`), with margin.
const STILL_LEAVING: Duration = Duration::from_millis(500);

/// Waits up to three seconds for somebody to answer at `path`.
///
/// `unit`: the installed unit was asked, and is asked ONCE MORE when
/// [`STILL_LEAVING`] has passed with nobody answering — a player closing
/// everywhere takes its socket off the disk before its process leaves, and a
/// start asked in between reached a unit that was still active, so it did
/// nothing, and the door waited for a player nobody was starting.
fn wait_for(path: &std::path::Path, unit: bool) -> Result<PathBuf> {
    let began = Instant::now();
    let mut asked_again = !unit;
    while began.elapsed() < Duration::from_secs(3) {
        if UnixStream::connect(path).is_ok() {
            return Ok(path.to_path_buf());
        }
        if !asked_again && began.elapsed() >= STILL_LEAVING {
            asked_again = true;
            let _ = ask_the_unit();
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

/// Closes the player that runs through MPRIS `Quit` on the session bus —
/// what a door does to a player OLDER than itself ([`restart`]): a daemon
/// from before the socket's `quit` does not know that ask, and every one of
/// them answers MPRIS's.
///
/// BLOCKING, as a D-Bus call is: call it off a loop.
///
/// # Errors
///
/// No session bus, or no player on it under [`crate::mpris_name`].
pub fn quit_by_mpris() -> Result<()> {
    let bus = zbus::blocking::Connection::session().context("the session bus")?;
    bus.call_method(
        Some(crate::mpris_name().as_str()),
        "/org/mpris/MediaPlayer2",
        Some("org.mpris.MediaPlayer2"),
        "Quit",
        &(),
    )
    .context("the player's MPRIS `Quit`")?;
    Ok(())
}

/// How long an older player is given to leave after its `Quit`: its own
/// leaving takes a fifth of a second, and a unit's stop a little more.
const LEAVING_AT_MOST: Duration = Duration::from_secs(3);

/// Starts the player again: the one that runs — OLDER than this door, which
/// cannot speak to it — closed through MPRIS ([`quit_by_mpris`]), and once
/// nobody answers its socket, this build's started ([`ensure`]).
///
/// BLOCKING, for as long as the old one takes to leave and the new one to
/// answer: call it off a loop.
///
/// # Errors
///
/// The old player could not be told, did not leave in time, or the new one
/// did not start.
pub fn restart() -> Result<PathBuf> {
    quit_by_mpris()?;
    let began = Instant::now();
    while running() {
        anyhow::ensure!(
            began.elapsed() < LEAVING_AT_MOST,
            "the older player did not leave"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    ensure()
}

/// Sends one ask ONLY TO A PLAYER THAT RUNS, and does not wait: `false`
/// when nobody listens, and nothing is started to hear it. What closing the
/// player sends — starting a daemon to tell it to leave would be the one
/// thing worse than not telling it — and what a preference written asks
/// (`reload`: a player that starts later reads the preferences itself).
///
/// NO ANSWER IS READ: both are asks the player cannot refuse, and a door's
/// loop that waited for the word would freeze for as long as a busy player
/// took to say it.
///
/// # Errors
///
/// A player answered the connection and the ask could not be written.
pub fn tell_if_running(what: &Ask) -> Result<bool> {
    tell_if_running_at(&socket_path()?, what)
}

/// [`tell_if_running`], on the socket at `path`.
fn tell_if_running_at(path: &std::path::Path, what: &Ask) -> Result<bool> {
    let Ok(stream) = UnixStream::connect(path) else {
        return Ok(false);
    };
    write_line(&mut BufWriter::new(stream), what)?;
    Ok(true)
}

/// Sends one ask and does not wait.
pub fn tell(what: &Ask) -> Result<()> {
    let stream = UnixStream::connect(ensure()?).context("the player is not running")?;
    let mut out = BufWriter::new(stream);
    write_line(&mut out, what)
}

/// Sends one ask and reads the answer.
pub fn ask(what: &Ask) -> Result<Say> {
    ask_within(what, Duration::from_secs(3))
}

/// The same, waiting up to `patience` for the answer — for an ask the player
/// answers only after a round trip of its own: pairing waits for the other
/// machine, and three seconds were less than that trip may take. Call it off
/// a loop.
///
/// # Errors
///
/// The player could not be started or reached, or did not answer in time —
/// the last is an `std::io::Error` of kind `WouldBlock` or `TimedOut` inside.
pub fn ask_within(what: &Ask, patience: Duration) -> Result<Say> {
    ask_at(&ensure()?, what, patience)?.context("the player did not answer")
}

/// The same, of a player ALREADY FOUND at `path` ([`ensure`]) — for a teller
/// that asks one thing after another and finds the player once — and with
/// what came back told apart: `None` is a connection the player closed
/// without a word, which an older player does with an ask it cannot read.
///
/// # Errors
///
/// Nobody answers at `path`, the ask could not be written, or the answer did
/// not come within `patience` — an `std::io::Error` of kind `WouldBlock` or
/// `TimedOut` inside.
pub fn ask_at(path: &std::path::Path, what: &Ask, patience: Duration) -> Result<Option<Say>> {
    let stream = UnixStream::connect(path)?;
    let _ = stream.set_read_timeout(Some(patience));
    let mut out = BufWriter::new(stream.try_clone()?);
    write_line(&mut out, what)?;
    read_line(&mut BufReader::new(stream))
}

/// How long a door waits for a sheet ([`sheet`]): up to four programs read
/// a file one after the other, each within ten seconds
/// (`ronoctua_tags`' reading ceiling), and a sheet asked while another is
/// read waits for it.
pub const SHEET_PATIENCE: Duration = Duration::from_secs(45);

/// Everything a file says about itself — [`Say::Sheet`], or the refusal
/// ([`Say::Refused`]) — from a player that RUNS: a sheet is asked because
/// something plays, and is no reason to start one. Blocks up to
/// [`SHEET_PATIENCE`]: call it off a loop, when
/// [`State::sheet_generation`](crate::wire::State) moves.
///
/// # Errors
///
/// No player runs, or it did not answer in time.
pub fn sheet(location: &str) -> Result<Say> {
    from_a_running_player(
        &Ask::Sheet {
            location: location.to_owned(),
        },
        SHEET_PATIENCE,
    )
}

/// A desk's page — [`Say::Desk`], or the refusal — from a player that RUNS:
/// a page is asked because its generation moved in a state a door heard, and
/// is no reason to start one. The page is in the daemon's memory, so this
/// is quick; call it off a loop all the same.
///
/// # Errors
///
/// No player runs, or it did not answer in time.
pub fn desk(kind: crate::wire::DeskKind) -> Result<Say> {
    from_a_running_player(&Ask::Desk { kind }, Duration::from_secs(3))
}

/// Has a player that RUNS fetch the cover a suggestion names, to be seen
/// beside the file's own ([`Ask::SuggestedCover`]) — what a door asks once a
/// person opened the candidate. Answered at once: the daemon fetches on the
/// desk's thread, and the picture lands on the candidate. Call it off a
/// loop all the same.
///
/// # Errors
///
/// No player runs, or it did not answer in time.
pub fn suggested_cover(candidate: crate::find::CandidateId) -> Result<Say> {
    from_a_running_player(&Ask::SuggestedCover { candidate }, Duration::from_secs(3))
}

/// How long a door waits for a key to be tried ([`key_check`]): the service's
/// answer, and for OpenSubtitles a login after it — each within the web's
/// own patience.
pub const KEY_CHECK_PATIENCE: Duration = Duration::from_secs(60);

/// Whether the service takes the key the person kept for it
/// ([`Ask::KeyCheck`]): [`Say::Done`], or [`Say::Refused`] saying why. It
/// STARTS the player — a key was just pasted, and trying it is the player's
/// to do. Blocks up to [`KEY_CHECK_PATIENCE`]: call it off a loop.
///
/// # Errors
///
/// The player could not be started or reached, or did not answer in time.
pub fn key_check(service: ronoctua_lookup::Service) -> Result<Say> {
    ask_within(&Ask::KeyCheck { service }, KEY_CHECK_PATIENCE)
}

/// `what` to a player that runs, waiting up to `patience` for its answer.
fn from_a_running_player(what: &Ask, patience: Duration) -> Result<Say> {
    let stream = UnixStream::connect(socket_path()?).context("the player is not running")?;
    let _ = stream.set_read_timeout(Some(patience));
    let mut out = BufWriter::new(stream.try_clone()?);
    write_line(&mut out, what)?;
    read_line(&mut BufReader::new(stream))?.context("the player did not answer")
}

/// What a watch heard.
#[derive(Debug, Clone, PartialEq)]
pub enum Heard {
    /// The whole state.
    State(Box<State>),
    /// The whole outline of the queue: on connecting, and whenever its shape
    /// changes — a door keeps the last one and draws the queue from it.
    Queue(Box<Outline>),
    /// The player is closing because somebody asked ([`Say::Farewell`]):
    /// every door closes.
    Farewell,
    /// Nobody answers any more — a crash, a unit stopped, a player never
    /// started. A door stays and says the player is not running: only a
    /// farewell closes one.
    Gone,
}

/// Watches the state on a thread of its own: `on_heard` gets every state,
/// every outline of the queue, the farewell of a player asked to close, and
/// [`Heard::Gone`] once when
/// nobody answers any more. It does NOT start the player: a footer watching
/// is not a reason for a player to exist, and the mark on the modeline is
/// exactly "there is a player".
pub fn watch(on_heard: impl Fn(Heard) + Send + 'static) -> Arc<AtomicBool> {
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
                    let input = BufReader::new(stream);
                    let ask_queue = || match from_a_running_player(&Ask::Queue, QUEUE_PATIENCE) {
                        Ok(Say::Queue(q)) => Some(q),
                        _ => None,
                    };
                    hear_lines(input, &gone, ask_queue, &on_heard)
                };
                let r = run();
                // SAY IT ONCE when it goes away, not every second it stays gone.
                if r.is_err() && was_there {
                    on_heard(Heard::Gone);
                }
                was_there = r.is_ok();
                std::thread::sleep(Duration::from_secs(1));
            }
        });
    stop
}

/// How long a watch waits for the outline it asked for ([`Ask::Queue`]):
/// answered under the queue's lock, at once.
const QUEUE_PATIENCE: Duration = Duration::from_secs(3);

/// The lines of one watch's connection, handed on as they come — and THE
/// OUTLINE ASKED FOR when a state's `revision` passes the last outline's: the
/// daemon sends an outline before the state that goes with it, so a state
/// past it means a line was lost, and the door would draw a queue of another
/// shape than the state's (`ask_queue`, on another connection, at most once
/// for each revision). What was asked is handed on before the state.
fn hear_lines(
    mut input: impl BufRead,
    gone: &AtomicBool,
    ask_queue: impl Fn() -> Option<Box<Outline>>,
    on_heard: &impl Fn(Heard),
) -> Result<()> {
    let mut kept: Option<u64> = None;
    let mut asked: Option<u64> = None;
    while let Some(say) = read_line::<Say>(&mut input)? {
        if gone.load(Ordering::SeqCst) {
            return Ok(());
        }
        match &say {
            Say::Queue(q) => kept = Some(q.revision),
            Say::State(s) if outline_owed(kept, asked, s.revision) => {
                asked = Some(s.revision);
                if let Some(q) = ask_queue() {
                    kept = Some(q.revision);
                    on_heard(Heard::Queue(q));
                }
            }
            _ => {}
        }
        if let Some(heard) = heard_of(say) {
            on_heard(heard);
        }
    }
    Ok(())
}

/// Whether a state of `revision` finds the door's outline behind it — the
/// one it `kept` older, or none at all — and it was not asked for that
/// revision already.
const fn outline_owed(kept: Option<u64>, asked: Option<u64>, revision: u64) -> bool {
    let behind = match kept {
        Some(k) => revision > k,
        None => true,
    };
    let again = match asked {
        Some(a) => a == revision,
        None => false,
    };
    behind && !again
}

/// What a line on a watch means to a door: a state, an outline, a farewell,
/// or nothing a watch is told (an answer to somebody else's ask never comes
/// here).
fn heard_of(say: Say) -> Option<Heard> {
    match say {
        Say::State(s) => Some(Heard::State(s)),
        Say::Queue(q) => Some(Heard::Queue(q)),
        Say::Farewell => Some(Heard::Farewell),
        Say::Preview { .. }
        | Say::Done
        | Say::Refused(_)
        | Say::Sheet(_)
        | Say::Desk(_)
        | Say::Draft { .. } => None,
    }
}

/// Frames of `width × height` of `lane`'s picture, or of the first picture
/// shown when `None` (see [`Ask::Preview`]) — from a player that RUNS. It
/// never starts one: a picture's thread asks again every second, and one that
/// started the daemon brought back a player somebody had just closed
/// everywhere.
pub fn preview(
    width: u32,
    height: u32,
    lane: Option<crate::own::LaneId>,
) -> Result<ronoctua_frames::Feed> {
    let path = socket_path()?;
    let mut socket = UnixStream::connect(&path).context("the player is not running")?;
    write_line(
        &mut socket,
        &Ask::Preview {
            width,
            height,
            lane,
        },
    )?;
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

/// The player reads the sound outputs ([`State::sinks`](crate::wire::State))
/// for as long as the answer is held — a door holds it while it shows them,
/// and drops it when it stops ([`Ask::Sinks`]). From a player that RUNS: the
/// list of outputs is not a reason to start one.
///
/// # Errors
///
/// No player runs, or the ask could not be written.
pub fn hold_sinks() -> Result<UnixStream> {
    let stream = UnixStream::connect(socket_path()?).context("the player is not running")?;
    write_line(&mut BufWriter::new(stream.try_clone()?), &Ask::Sinks)?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: telling a player that is not there starts one, or waits for
    /// it — closing the player everywhere would bring it back to close it —
    /// or a player that IS there is not told, or the teller waits for an
    /// answer a busy player has not given (a door's loop would freeze).
    #[test]
    fn a_quit_is_told_only_to_a_player_that_runs() {
        let dir = std::env::temp_dir().join(format!("ronoctua-ask-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a directory");
        let path = dir.join("player.sock");
        let _ = std::fs::remove_file(&path);

        // NOBODY LISTENS: nothing to tell, and nothing started to hear it.
        assert!(matches!(tell_if_running_at(&path, &Ask::Quit), Ok(false)));
        assert!(!path.exists(), "nothing listens there now either");

        // A PLAYER THAT RUNS is told — and one that never answers does not
        // keep the teller: it is back before the player has said a word.
        let listener = std::os::unix::net::UnixListener::bind(&path).expect("binds");
        let (said, hear) = std::sync::mpsc::channel::<()>();
        let heard = std::thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accepts");
            let mut input = BufReader::new(stream);
            let ask: Option<Ask> = read_line(&mut input).expect("reads");
            // SILENT until the teller is back.
            let _ = hear.recv();
            ask
        });
        let began = Instant::now();
        assert!(tell_if_running_at(&path, &Ask::Quit).expect("tells"));
        assert!(
            began.elapsed() < Duration::from_secs(1),
            "the teller waited for an answer"
        );
        said.send(()).expect("the fake player waits");
        assert_eq!(heard.join().expect("the fake player"), Some(Ask::Quit));
        std::fs::remove_dir_all(&dir).expect("cleans up");
    }

    /// FAILS IF: a state whose revision passes the outline the door holds —
    /// a line lost — leaves the door drawing the old queue, instead of the
    /// outline being asked and handed on before that state; or the door asks
    /// again and again for one revision, or asks when nothing was lost.
    #[test]
    fn a_state_past_the_outline_asks_for_it() {
        use std::cell::RefCell;
        let state = |revision: u64| {
            Say::State(Box::new(State {
                revision,
                ..State::default()
            }))
        };
        let outline = |revision: u64| {
            Say::Queue(Box::new(Outline {
                revision,
                entries: Vec::new(),
            }))
        };
        let lines = |says: &[Say]| {
            let mut text = String::new();
            for s in says {
                text.push_str(&serde_json::to_string(s).expect("writes"));
                text.push('\n');
            }
            text
        };
        let run = |says: &[Say], answer: Option<u64>| {
            let heard = RefCell::new(Vec::new());
            let asked = RefCell::new(0_u32);
            let ask = || {
                *asked.borrow_mut() += 1;
                answer.map(|revision| {
                    Box::new(Outline {
                        revision,
                        entries: Vec::new(),
                    })
                })
            };
            hear_lines(
                std::io::Cursor::new(lines(says)),
                &AtomicBool::new(false),
                ask,
                &|h| heard.borrow_mut().push(h),
            )
            .expect("reads");
            (heard.into_inner(), asked.into_inner())
        };
        let revision_of = |h: &Heard| match h {
            Heard::State(s) => (0, s.revision),
            Heard::Queue(q) => (1, q.revision),
            _ => (9, 0),
        };
        // NOTHING LOST: the outline, then its states — never asked.
        let (heard, asked) = run(&[outline(4), state(4), state(4)], Some(9));
        assert_eq!(asked, 0);
        assert_eq!(heard.len(), 3);
        // THE OUTLINE OF 5 LOST: asked, and handed on BEFORE the state.
        let (heard, asked) = run(&[outline(4), state(4), state(5), state(5)], Some(5));
        assert_eq!(asked, 1, "once for revision 5");
        let order: Vec<(u8, u64)> = heard.iter().map(revision_of).collect();
        assert_eq!(order, vec![(1, 4), (0, 4), (1, 5), (0, 5), (0, 5)]);
        // A PLAYER THAT DOES NOT ANSWER is asked once a revision, not on
        // every state.
        let (heard, asked) = run(&[outline(4), state(5), state(5), state(6)], None);
        assert_eq!(asked, 2, "once for 5, once for 6");
        assert_eq!(heard.len(), 4);
        // NO OUTLINE AT ALL before a state: asked.
        let (_, asked) = run(&[state(1)], Some(1));
        assert_eq!(asked, 1);
        assert!(
            !outline_owed(Some(7), None, 3),
            "an older state owes nothing"
        );
    }

    /// FAILS IF: a watch stops telling a farewell from a state or an
    /// outline, or hands a door the answer to an ask it never made.
    #[test]
    fn a_watch_hears_states_outlines_and_the_farewell() {
        assert_eq!(heard_of(Say::Farewell), Some(Heard::Farewell));
        assert_eq!(
            heard_of(Say::State(Box::default())),
            Some(Heard::State(Box::default()))
        );
        assert_eq!(
            heard_of(Say::Queue(Box::default())),
            Some(Heard::Queue(Box::default()))
        );
        assert_eq!(heard_of(Say::Done), None);
        // A DESK'S PAGE is an answer to the door that asked, never news.
        assert_eq!(
            heard_of(Say::Desk(Box::new(crate::find::Desk::empty(
                crate::wire::DeskKind::Lyrics
            )))),
            None
        );
        // A SHEET is an answer to the door that asked, never news.
        assert_eq!(
            heard_of(Say::Sheet(Box::new(ronoctua_tags::stream::sheet(
                "https://x",
                ronoctua_tags::Kind::Music,
                &[],
                Vec::new()
            )))),
            None
        );
        assert_eq!(
            heard_of(Say::Refused(crate::wire::Trouble::Remote {
                why: "x".into()
            })),
            None
        );
    }
}
