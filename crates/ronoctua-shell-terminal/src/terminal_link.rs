//! How a window reaches its terminal, and what comes back.
//!
//! # A window is a viewer
//!
//! The terminal lives in a HOLDER (`ronoctua_term::holder`): the PTY, the
//! program, the authoritative engine. A window attaches to it ([`start`],
//! [`Link::attach`]), receives a snapshot, decodes it into a MIRROR and
//! follows the bytes the holder's engine is fed — one ordered connection, a
//! frame at a time (`ronoctua_terminals::wire`).
//!
//! Two ways to the holder, ONE viewer for both ([`Reach`]):
//!
//! - **held**: a holder of its own, in a scope of its own, started by the
//!   spawner (`ronoctua_terminals::spawn::start`) — the default. It outlives
//!   the window, the footer and a `switch`; closing the window only sends the
//!   terminal to the background.
//! - **here**: the same holder's core in THIS process, over a socket pair
//!   (`Holder::connect`) — for what must live and die with its window (the
//!   manual page, the root shell, a software trial, the monitor, the rescue
//!   console), and the fallback when the session gives no scope, which the
//!   window says on its line.
//!
//! # Threads, and the one lock
//!
//! - the **launch**: starting a holder blocks (a scope, up to ten seconds),
//!   so it runs on a thread of its own, and the window hears
//!   [`Heard::Started`] — one launch per terminal of a group the window
//!   opens with ([`Plan::reachings`]);
//! - the **reader**: frames from the holder. The ones that change the screen
//!   (bytes, a resize, a mode, colours) are applied to the mirror RIGHT HERE,
//!   in the stream's order, and the window is only told that the screen moved
//!   (a ping, which coalesces); the rest become [`Heard`]s;
//! - the **writer**: frames to the holder, from a queue, so a holder that
//!   stops reading blocks this thread and never the window's loop.
//!
//! The mirror is behind ONE lock ([`Mirrored`]), taken by the reader to feed
//! it and by the window to read a frame from it — and never held while a
//! frame is read: the render state's two-phase update reads the terminal
//! under the lock and finishes without it.

use std::io::{self, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use anyhow::{Context as _, Result};
use ronoctua_term::ghostty::{self, Decoder, Mirror, Mode, RenderState};
use ronoctua_term::holder::{Holder, Setup};
use ronoctua_terminals::registry::Registry;
use ronoctua_terminals::spawn;
use ronoctua_terminals::state::TerminalState;
use ronoctua_terminals::wire::{
    self, Ask, Attach, AttachMode, Attached, ClipboardWrite, Consent, Size, ToHolder, ToViewer,
};
use ronoctua_terminals::{GroupId, PROTOCOL, TerminalId};

/// A TERMINAL'S MIRROR AND THE RENDER STATE THAT READS IT, behind the one
/// lock the reader and the window share.
pub struct Mirrored {
    mirror: Mutex<Option<Mirror>>,
    render: RenderState,
}

impl Mirrored {
    /// Nothing shown yet.
    ///
    /// # Errors
    ///
    /// The library could not make a render state.
    pub fn new() -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            mirror: Mutex::new(None),
            render: RenderState::new().context("a render state")?,
        }))
    }

    fn lock(&self) -> MutexGuard<'_, Option<Mirror>> {
        self.mirror.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The mirror a snapshot was decoded into, shown from now on — and the
    /// frame a program holds the screen on (synchronized output) captured at
    /// the very byte the hold begins.
    pub fn install(&self, mut mirror: Mirror) {
        mirror.capture_holds(&self.render);
        *self.lock() = Some(mirror);
    }

    /// Runs `f` on the mirror, when there is one.
    pub fn with<R>(&self, f: impl FnOnce(&mut Mirror) -> R) -> Option<R> {
        self.lock().as_mut().map(f)
    }

    /// Whether a snapshot has been decoded.
    #[must_use]
    pub fn shown(&self) -> bool {
        self.lock().is_some()
    }

    /// Reads the mirror into the render state: the first phase under the
    /// lock, the second without it. `false` with no mirror yet.
    pub fn update(&self) -> bool {
        let read = {
            let mirror = self.lock();
            match mirror.as_ref() {
                Some(m) => self.render.begin(m),
                None => return false,
            }
        };
        if let Err(e) = read.and_then(|()| self.render.end()) {
            tracing::warn!(error = %e, "the terminal's frame could not be read");
            return false;
        }
        true
    }

    /// The frame [`Mirrored::update`] read. Drop it before asking the mirror
    /// anything: the reader takes the mirror's lock and then this one.
    #[must_use]
    pub fn frame(&self) -> ghostty::Frame<'_> {
        self.render.frame()
    }
}

/// What the window hears from its terminal, off the loop.
#[derive(Debug)]
pub enum Heard {
    /// The launch thread is done: the terminal reached, or why not.
    Started(Box<Result<Started, String>>),
    /// The holder said what follows.
    Attached(Attached),
    /// The snapshot is decoded: the mirror is shown.
    Ready,
    /// The terminal's whole state.
    State(Box<TerminalState>),
    /// The program wrote a clipboard.
    Clipboard(ClipboardWrite),
    /// The program asks to read a clipboard — and the way to answer it,
    /// which is the reader's own: a read can arrive before the window has
    /// heard the attachment started, and it is answered all the same.
    Consent(Consent, Answering),
    /// Another window took the terminal.
    Taken,
    /// This window fell behind and was let go: attach again.
    Lagging,
    /// The program ended.
    Exited(i32),
    /// The holder sent a form this window cannot decode (another build).
    Unreadable,
    /// The connection ended without saying why.
    Gone,
    /// The window's search found something (`crate::terminal_search`).
    Found(Box<crate::terminal_search::Found>),
    /// Files were let go over the window, their list read off the loop
    /// (`crate::terminal_drop`).
    Dropped(Vec<PathBuf>),
    /// A paste this window sent was not written, and why
    /// ([`ToViewer::PasteRefused`]).
    PasteRefused(wire::PasteRefusal),
}

/// THE WAY BACK TO ONE HOLDER, for an answer that cannot wait for the
/// window's own [`Link`]: the attachment's queue, cloned.
#[derive(Debug, Clone)]
pub struct Answering(mpsc::Sender<ToHolder>);

impl Answering {
    /// The way back over `queue` — the reader's own, or a test's.
    #[must_use]
    pub const fn new(queue: mpsc::Sender<ToHolder>) -> Self {
        Self(queue)
    }

    /// Sends the holder a frame.
    pub fn send(&self, frame: ToHolder) {
        let _ = self.0.send(frame);
    }
}

/// One [`Heard`], and which attachment it came from — so a reader of an
/// attachment that was replaced is never taken for the new one's.
#[derive(Debug)]
pub struct Arrival {
    /// The attachment's number ([`Link::generation`]).
    pub generation: u64,
    /// What was heard.
    pub heard: Heard,
}

/// How the threads tell the window: what they heard, and that the screen
/// moved (which coalesces — a flood is one paint per frame, not one per
/// read).
pub struct Tell {
    /// A message for the window.
    pub heard: Box<dyn Fn(Arrival) + Send + Sync>,
    /// The mirror changed.
    pub moved: Box<dyn Fn() + Send + Sync>,
}

/// WHERE THE TERMINAL IS — see the head of the module.
pub enum Reach {
    /// A holder of its own, found in the registry by its id.
    Held {
        /// The registry its socket is in.
        registry: Registry,
        /// The terminal.
        id: TerminalId,
    },
    /// A holder in this process.
    Here {
        /// The holder.
        holder: Holder,
        /// The terminal.
        id: TerminalId,
    },
}

impl Reach {
    /// The terminal's id.
    #[must_use]
    pub const fn id(&self) -> &TerminalId {
        match self {
            Self::Held { id, .. } | Self::Here { id, .. } => id,
        }
    }

    /// Whether the terminal lives and dies with this window.
    #[must_use]
    pub const fn in_process(&self) -> bool {
        matches!(self, Self::Here { .. })
    }

    /// Connects and sends the attach, blocking for as long as a connect to a
    /// local socket takes.
    fn connect(&self, attach: &Attach) -> io::Result<UnixStream> {
        match self {
            Self::Held { registry, id } => registry.attach(id, attach),
            Self::Here { holder, .. } => {
                let mut stream = holder.connect()?;
                let line = Ask::Attach(attach.clone())
                    .line()
                    .map_err(io::Error::other)?;
                stream.write_all(&line)?;
                Ok(stream)
            }
        }
    }

    /// The window is going: a terminal of its own process is hung up with
    /// it (its program ends, as a window's own terminal always did); a held
    /// one goes to the background when the connection closes.
    pub fn window_gone(&self) {
        if let Self::Here { holder, .. } = self {
            holder.hang_up();
        }
    }
}

/// What a new window's terminal is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// A new terminal running `program`.
    New {
        /// The program; `$SHELL` when the window was given none.
        program: String,
        /// Its arguments.
        args: Vec<String>,
        /// Where it starts; the window's own folder when `None`.
        cwd: Option<PathBuf>,
        /// It lives in the window's process, and dies with it.
        here: bool,
    },
    /// Terminals already running: their group, one of them in front.
    Show {
        /// The group's terminals, in their order.
        show: Vec<TerminalId>,
        /// The one shown.
        front: TerminalId,
    },
}

impl Plan {
    /// WHAT THE WINDOW REACHES TO OPEN WITH: the one new terminal, or every
    /// terminal shown, in the strip's order — each its own launch, so one
    /// that ended since does not keep the others from coming.
    #[must_use]
    pub fn reachings(&self) -> Vec<Reaching> {
        match self {
            Self::New {
                program,
                args,
                cwd,
                here,
            } => vec![Reaching::New {
                program: program.clone(),
                args: args.clone(),
                cwd: cwd.clone(),
                here: *here,
            }],
            Self::Show { show, .. } => show.iter().cloned().map(Reaching::Held).collect(),
        }
    }
}

/// ONE TERMINAL TO REACH: a new one, or one that runs already.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reaching {
    /// A new terminal running `program`.
    New {
        /// The program.
        program: String,
        /// Its arguments.
        args: Vec<String>,
        /// Where it starts; the window's own folder when `None`.
        cwd: Option<PathBuf>,
        /// It lives in the window's process, and dies with it.
        here: bool,
    },
    /// A terminal that runs already, held: attached, and so taken from the
    /// window that showed it, if one did.
    Held(TerminalId),
}

/// A terminal reached: where it is, the attachment, and whether it had to
/// run in the window's process when it was meant to be held (said on the
/// window's line).
pub struct Started {
    /// Where the terminal is.
    pub reach: Reach,
    /// The attachment.
    pub link: Link,
    /// It fell back to the window's process.
    pub fell_back: bool,
}

impl std::fmt::Debug for Started {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Started")
            .field("id", self.reach.id())
            .field("in_process", &self.reach.in_process())
            .field("fell_back", &self.fell_back)
            .finish_non_exhaustive()
    }
}

/// What every attach says of the window: its process, its size for the
/// terminal, the colours of its page, and where in its group it shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Showing {
    /// The terminal's size in the window.
    pub size: Size,
    /// The page's colours.
    pub colors: wire::Colors,
    /// The group.
    pub group: GroupId,
    /// The place in it.
    pub order: u32,
}

impl Showing {
    /// The attach line's body.
    fn attach(&self) -> Attach {
        Attach {
            protocol: PROTOCOL,
            engine: ghostty::ENGINE.to_owned(),
            pid: std::process::id(),
            size: self.size,
            colors: self.colors.clone(),
            group: self.group.clone(),
            order: self.order,
        }
    }
}

/// A terminal in this process.
fn here(
    program: &str,
    args: &[String],
    cwd: Option<PathBuf>,
    showing: &Showing,
    born: spawn::Born,
) -> Result<Reach> {
    let id = TerminalId::fresh().context("a fresh terminal id")?;
    let holder = Holder::start(Setup {
        id: id.clone(),
        group: showing.group.clone(),
        order: showing.order,
        size: showing.size,
        cwd,
        program: program.to_owned(),
        args: args.to_vec(),
        version: crate::terminal_sheet::VERSION.to_owned(),
        registry: None,
        born,
    })
    .with_context(|| format!("start {program}"))?;
    Ok(Reach::Here { holder, id })
}

/// Where a terminal is, BLOCKING (a scope may take its time): the
/// held terminal started or found, or one in this process — and whether
/// that was a fallback. A new one is `born` with what the person chose
/// (`terminal.json`); one that runs already keeps what it was born with.
///
/// # Errors
///
/// The program could not start even in this process, or a terminal to show
/// is not there.
fn reach_of(reaching: &Reaching, showing: &Showing, born: spawn::Born) -> Result<(Reach, bool)> {
    match reaching {
        Reaching::New {
            program,
            args,
            cwd,
            here: true,
        } => Ok((here(program, args, cwd.clone(), showing, born)?, false)),
        Reaching::New {
            program,
            args,
            cwd,
            here: false,
        } => {
            let held = Registry::of_session()
                .filter(|_| spawn::scopes_work())
                .context("the session gives a terminal no scope")
                .and_then(|registry| {
                    let id = spawn::start(&spawn::Spawn {
                        program: program.clone(),
                        args: args.clone(),
                        cwd: cwd.clone(),
                        group: showing.group.clone(),
                        order: showing.order,
                        size: showing.size,
                        born,
                    })?;
                    Ok(Reach::Held { registry, id })
                });
            match held {
                Ok(reach) => Ok((reach, false)),
                Err(e) => {
                    // THE TERMINAL STILL OPENS, in this process — and the
                    // window says so, because closing it now ends the
                    // program, which a held one never does.
                    tracing::warn!(
                        error = format!("{e:#}"),
                        "no holder of its own; the terminal runs in its window"
                    );
                    Ok((here(program, args, cwd.clone(), showing, born)?, true))
                }
            }
        }
        Reaching::Held(id) => {
            let registry = Registry::of_session().context("no runtime folder, so no terminals")?;
            Ok((
                Reach::Held {
                    registry,
                    id: id.clone(),
                },
                false,
            ))
        }
    }
}

/// THE LAUNCH: reaches one terminal and attaches to it. Blocks — call it on
/// a thread of its own ([`launch`]).
///
/// # Errors
///
/// The terminal could not be reached or attached to.
pub fn start(
    reaching: &Reaching,
    showing: &Showing,
    born: spawn::Born,
    mirrored: &Arc<Mirrored>,
    tell: &Arc<Tell>,
    generation: u64,
) -> Result<Started> {
    let (reach, fell_back) = reach_of(reaching, showing, born)?;
    let link = Link::attach(&reach, showing, mirrored, tell, generation)
        .with_context(|| format!("attach to terminal {}", reach.id()))?;
    Ok(Started {
        reach,
        link,
        fell_back,
    })
}

/// [`start`] on a thread of its own, the answer told as [`Heard::Started`].
pub fn launch(
    reaching: Reaching,
    showing: Showing,
    born: spawn::Born,
    mirrored: Arc<Mirrored>,
    tell: Arc<Tell>,
    generation: u64,
) {
    let told = Arc::clone(&tell);
    let spawned = std::thread::Builder::new()
        .name("term-launch".to_owned())
        .spawn(move || {
            let started = start(&reaching, &showing, born, &mirrored, &tell, generation)
                .map_err(|e| format!("{e:#}"));
            (tell.heard)(Arrival {
                generation,
                heard: Heard::Started(Box::new(started)),
            });
        });
    if let Err(e) = spawned {
        (told.heard)(Arrival {
            generation,
            heard: Heard::Started(Box::new(Err(format!("no launch thread: {e}")))),
        });
    }
}

/// ONE ATTACHMENT: the queue to the holder, and its number.
pub struct Link {
    out: mpsc::Sender<ToHolder>,
    /// Which attachment of the window this is: every reattach is a new one,
    /// and what an old reader still says is not this one's.
    pub generation: u64,
}

impl Link {
    /// Attaches to the terminal at `reach`: the attach sent, the reader
    /// following it into `mirrored`, the writer taking the queue.
    ///
    /// # Errors
    ///
    /// The holder is not there, or a thread could not start.
    pub fn attach(
        reach: &Reach,
        showing: &Showing,
        mirrored: &Arc<Mirrored>,
        tell: &Arc<Tell>,
        generation: u64,
    ) -> Result<Self> {
        let stream = reach.connect(&showing.attach())?;
        let reading = stream.try_clone().context("the stream's reading half")?;
        let (out, queue) = mpsc::channel::<ToHolder>();
        {
            let (mirrored, tell) = (Arc::clone(mirrored), Arc::clone(tell));
            let answering = Answering(out.clone());
            std::thread::Builder::new()
                .name("term-read".to_owned())
                .spawn(move || follow(reading, &mirrored, &tell, generation, &answering))
                .context("the reader's thread")?;
        }
        std::thread::Builder::new()
            .name("term-write".to_owned())
            .spawn(move || write_out(stream, &queue))
            .context("the writer's thread")?;
        Ok(Self { out, generation })
    }

    /// Sends the holder a frame. A link whose writer is gone says nothing:
    /// its reader has told the window the connection ended.
    pub fn send(&self, frame: ToHolder) {
        let _ = self.out.send(frame);
    }

    /// The queue to the holder, for a thread that answers on its own (a
    /// clipboard read once the clipboard is read).
    #[must_use]
    pub fn sender(&self) -> mpsc::Sender<ToHolder> {
        self.out.clone()
    }

    /// A link with no holder behind it, and what is sent down it — a test's
    /// ear on what a window tells a terminal.
    #[cfg(test)]
    #[must_use]
    pub fn listened(generation: u64) -> (Self, mpsc::Receiver<ToHolder>) {
        let (out, heard) = mpsc::channel();
        (Self { out, generation }, heard)
    }
}

/// The writer: each frame built whole and written with one `write_all`,
/// until the window drops its queue or the holder stops taking them.
fn write_out(mut stream: UnixStream, queue: &mpsc::Receiver<ToHolder>) {
    for frame in queue {
        if let Err(e) = stream.write_all(&frame.encode()) {
            tracing::debug!(error = %e, "the terminal's holder took no more frames");
            break;
        }
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
}

/// The reader: frames applied to the mirror at their point in the stream,
/// the rest told — until the connection ends, which is told too.
fn follow(
    stream: UnixStream,
    mirrored: &Mirrored,
    tell: &Tell,
    generation: u64,
    answering: &Answering,
) {
    let say = |heard| (tell.heard)(Arrival { generation, heard });
    let mut reader = BufReader::new(stream);
    let mut snapshot = Vec::new();
    let mut readable = false;
    loop {
        let (kind, payload) = match wire::read_frame(&mut reader) {
            Ok(Some(frame)) => frame,
            Ok(None) => {
                say(Heard::Gone);
                return;
            }
            Err(e) => {
                tracing::info!(error = %e, "the terminal's stream ended");
                say(Heard::Gone);
                return;
            }
        };
        let frame = match ToViewer::decode(kind, payload) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(error = %e, "the holder sent what is no frame; let go");
                say(Heard::Gone);
                return;
            }
        };
        match frame {
            ToViewer::Attached(a) => {
                readable = a.protocol == PROTOCOL && a.mode == AttachMode::Exact;
                if !readable {
                    say(Heard::Unreadable);
                }
                say(Heard::Attached(a));
            }
            ToViewer::Snapshot(bytes) => {
                if readable {
                    snapshot.extend(bytes);
                }
            }
            ToViewer::Ready => {
                if !readable {
                    continue;
                }
                // EVERY PIECE IS HERE ALREADY (the holder sends READY after
                // the last), so decoding waits on nothing but the decoding.
                match Decoder::decode(std::mem::take(&mut snapshot)) {
                    Ok(mirror) => {
                        mirrored.install(mirror);
                        (tell.moved)();
                        say(Heard::Ready);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "the terminal's snapshot does not decode");
                        say(Heard::Unreadable);
                        readable = false;
                    }
                }
            }
            ToViewer::Bytes(bytes) => {
                if mirrored.with(|m| m.feed(&bytes)).is_some() {
                    (tell.moved)();
                }
            }
            ToViewer::Resize(s) => {
                let resized = mirrored.with(|m| {
                    m.resize(
                        s.columns,
                        s.rows,
                        u32::from(s.cell_width_px),
                        u32::from(s.cell_height_px),
                    )
                });
                if let Some(Err(e)) = resized {
                    tracing::warn!(error = %e, "the mirror refused the holder's size");
                }
                (tell.moved)();
            }
            ToViewer::Mode(m) => {
                let mode = if m.ansi {
                    Mode::ansi(m.number)
                } else {
                    Mode::dec(m.number)
                };
                if let Some(Err(e)) = mirrored.with(|mirror| mirror.set_mode(mode, m.on)) {
                    tracing::warn!(error = %e, "the mirror refused the holder's mode");
                }
                (tell.moved)();
            }
            ToViewer::Colors(c) => {
                match crate::terminal_sheet::engine_colors(&c) {
                    Some(colors) => {
                        if let Some(Err(e)) = mirrored.with(|m| m.set_colors(&colors)) {
                            tracing::warn!(error = %e, "the mirror refused the holder's colours");
                        }
                    }
                    None => tracing::warn!("the holder sent a palette that is not 256 colours"),
                }
                (tell.moved)();
            }
            ToViewer::State(s) => say(Heard::State(s)),
            ToViewer::Clipboard(c) => say(Heard::Clipboard(c)),
            ToViewer::Consent(c) => say(Heard::Consent(c, answering.clone())),
            ToViewer::Taken => say(Heard::Taken),
            ToViewer::Lagging => say(Heard::Lagging),
            ToViewer::Exited(e) => say(Heard::Exited(e.code)),
            ToViewer::PasteRefused(why) => say(Heard::PasteRefused(why)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::time::Duration;

    /// Writes an executable with a `/bin/sh` child, never this process (a
    /// file it held open for writing while another test's thread forked
    /// would be `ETXTBSY` to run).
    fn write_executable(path: &Path, body: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let status = std::process::Command::new("/bin/sh")
            .args([
                "-c",
                "printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"",
                "sh",
            ])
            .arg(path)
            .arg(body)
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-term-window-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A window's ears, as a test hears them: every arrival on one channel,
    /// every move of the screen on another.
    fn ears() -> (Arc<Tell>, mpsc::Receiver<Arrival>, mpsc::Receiver<()>) {
        let (tx, rx) = mpsc::channel();
        let (moved_tx, moved_rx) = mpsc::channel();
        let (tx, moved_tx) = (Mutex::new(tx), Mutex::new(moved_tx));
        let tell = Tell {
            heard: Box::new(move |a| {
                let _ = tx.lock().unwrap().send(a);
            }),
            moved: Box::new(move || {
                let _ = moved_tx.lock().unwrap().send(());
            }),
        };
        (Arc::new(tell), rx, moved_rx)
    }

    /// Waits until the mirror shows `text`, a move of the screen at a time —
    /// each wait 20 s at most, a bound on a broken test.
    fn until_shown(mirrored: &Mirrored, moved: &mpsc::Receiver<()>, text: &str) {
        while !screen_text(mirrored).contains(text) {
            moved
                .recv_timeout(Duration::from_secs(20))
                .unwrap_or_else(|_| {
                    panic!(
                        "{text:?} never reached the mirror: {:?}",
                        screen_text(mirrored)
                    )
                });
        }
    }

    fn showing() -> Showing {
        Showing {
            size: Size {
                columns: 60,
                rows: 10,
                cell_width_px: 10,
                cell_height_px: 20,
            },
            colors: crate::terminal_sheet::Palette::of_the_page().wire(),
            group: GroupId::fresh().unwrap(),
            order: 0,
        }
    }

    /// Waits for an arrival that `wanted` takes — every read waits 20 s at
    /// most, a bound on a broken test, never a measure of the program.
    fn until(rx: &mpsc::Receiver<Arrival>, mut wanted: impl FnMut(&Heard) -> bool) -> Heard {
        loop {
            let a = rx
                .recv_timeout(Duration::from_secs(20))
                .expect("an arrival in time");
            if wanted(&a.heard) {
                return a.heard;
            }
        }
    }

    fn screen_text(mirrored: &Mirrored) -> String {
        mirrored
            .with(|m| {
                String::from_utf8_lossy(&m.format(ghostty::Format::Plain).unwrap()).into_owned()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_new_terminal_is_born_with_what_the_window_was_told() {
        // FAILS IF the person's choices for a new terminal stop at the
        // window: its holder — here, in the window's process — starts with
        // the engine's block, and the mirror decoded from its snapshot draws
        // it.
        let dir = scratch("born");
        let program = dir.join("program");
        write_executable(
            &program,
            "#!/bin/sh
printf 'born\n'
exec cat
",
        );
        let reaching = Reaching::New {
            program: program.to_string_lossy().into_owned(),
            args: Vec::new(),
            cwd: Some(dir),
            here: true,
        };
        let mirrored = Mirrored::new().unwrap();
        let (tell, rx, moved) = ears();
        let born = spawn::Born {
            scrollback_bytes: Some(1_000_000),
            cursor: Some((spawn::CursorShape::Underline, false)),
        };
        let started = start(&reaching, &showing(), born, &mirrored, &tell, 3).expect("started");
        until(&rx, |h| matches!(h, Heard::Ready));
        until_shown(&mirrored, &moved, "born");
        let _ = mirrored.update();
        assert_eq!(
            mirrored.frame().cursor().shape,
            ghostty::CursorShape::Underline,
            "the terminal was not born with the person's cursor"
        );
        // AND ITS MIRROR GOES BACK WHERE THE HOLDER DOES — the snapshot
        // carries the default: a program's own cursor, then its reset, is the
        // holder's underline, never the engine's block.
        mirrored.with(|m| m.feed(b"\x1b[6 q\x1b[0 q"));
        let _ = mirrored.update();
        assert_eq!(
            mirrored.frame().cursor().shape,
            ghostty::CursorShape::Underline,
            "the mirror went back to another cursor than its holder"
        );
        drop(started);
    }

    #[test]
    fn a_window_attaches_to_its_own_terminal_and_its_mirror_shows_the_program() {
        // FAILS IF the window's own terminal (`--here`) does not start, the
        // attach is not answered with a snapshot the mirror decodes, the
        // bytes after it do not reach the mirror, or text the window sends
        // does not reach the program.
        let dir = scratch("attach");
        let program = dir.join("program");
        write_executable(
            &program,
            "#!/bin/sh\nprintf 'hello-from-the-program\\n'\nwhile IFS= read -r l; do printf 'got:%s\\n' \"$l\"; done\n",
        );
        let reaching = Reaching::New {
            program: program.to_string_lossy().into_owned(),
            args: Vec::new(),
            cwd: Some(dir),
            here: true,
        };
        let mirrored = Mirrored::new().unwrap();
        let (tell, rx, moved) = ears();
        let started = start(
            &reaching,
            &showing(),
            spawn::Born::default(),
            &mirrored,
            &tell,
            7,
        )
        .expect("started");
        assert!(started.reach.in_process() && !started.fell_back);
        until(&rx, |h| matches!(h, Heard::Ready));
        until_shown(&mirrored, &moved, "hello-from-the-program");
        started.link.send(ToHolder::Text("typed".to_owned()));
        started.link.send(ToHolder::Key(wire::KeyEvent {
            code: 28,
            key: "Enter".to_owned(),
            action: wire::KeyAction::Press,
            mods: wire::Mods::default(),
            consumed: wire::Mods::default(),
            unshifted: None,
            utf8: String::new(),
            composing: false,
        }));
        until_shown(&mirrored, &moved, "got:typed");
        // AND THE FRAME HAS IT: the render state read off the mirror, made
        // the cells the door draws.
        assert!(mirrored.update());
        let stroke = ronoctua_render::sprite::Stroke {
            top: 0,
            thickness: 1,
        };
        let geometry = ronoctua_render::cells::Geometry {
            cell_width: 10,
            cell_height: 20,
            baseline: 15,
            stroke: 1,
            underline: stroke,
            strike: stroke,
            overline: stroke,
            size: 14.0,
            scale: 1,
        };
        let palette = crate::terminal_sheet::Palette::of_the_page();
        let mut sheet = ronoctua_render::cells::Sheet::new(
            geometry,
            1,
            1,
            ronoctua_render::cells::Defaults {
                foreground: palette.foreground,
                background: palette.background,
                faint: 128,
            },
        );
        let mut frame = mirrored.frame();
        crate::terminal_sheet::fill(
            &mut sheet,
            &mut frame,
            &palette,
            crate::terminal_sheet::Showing {
                focused: true,
                blink_off: false,
                all: true,
                faint: 128,
            },
        );
        drop(frame);
        assert_eq!(
            (sheet.columns(), sheet.lines()),
            (60, 10),
            "the holder's size"
        );
        assert!(
            (0..sheet.lines())
                .any(|l| crate::terminal_sheet::line_text(&sheet, l).contains("got:typed")),
            "the frame does not show what the program wrote"
        );
        started.reach.window_gone();
        assert!(matches!(
            until(&rx, |h| matches!(h, Heard::Gone | Heard::Exited(_))),
            Heard::Gone | Heard::Exited(_)
        ));
    }

    #[test]
    fn an_arrival_carries_the_attachment_it_came_from() {
        // FAILS IF what a replaced attachment's reader says can be taken for
        // the new one's: every arrival names its generation.
        let dir = scratch("generation");
        let program = dir.join("program");
        write_executable(&program, "#!/bin/sh\nexit 3\n");
        let reaching = Reaching::New {
            program: program.to_string_lossy().into_owned(),
            args: Vec::new(),
            cwd: Some(dir),
            here: true,
        };
        let mirrored = Mirrored::new().unwrap();
        let (tell, rx, _moved) = ears();
        let started = start(
            &reaching,
            &showing(),
            spawn::Born::default(),
            &mirrored,
            &tell,
            41,
        )
        .expect("started");
        let a = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("an arrival");
        assert_eq!(a.generation, 41);
        assert_eq!(started.link.generation, 41);
        assert!(matches!(
            until(&rx, |h| matches!(h, Heard::Exited(_))),
            Heard::Exited(3)
        ));
    }
}
