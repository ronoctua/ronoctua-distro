//! The holder: one terminal's own process — the PTY, the program and the
//! authoritative engine, outliving every window that shows them.
//!
//! # Threads, and who owns what
//!
//! - **engine** owns the [`Vt`], the inbound filter, the status records, the
//!   state and every viewer's queue: everything that decides happens on it,
//!   in the order events arrive, so the stream a window follows is one order.
//! - **output** reads the PTY into the engine's channel, with a bounded
//!   amount in flight: a program writing faster than the engine reads waits
//!   in its own `write`, as it would on any terminal.
//! - **input** writes to the PTY. A program that stops reading its input
//!   blocks this thread, never the engine (which would then stop reading its
//!   output, and the two would wait on each other forever) — and what waits
//!   for it is bounded (`Typing`).
//! - **child** waits for the program to end — once, and without reaping it.
//! - **accept**, and per connection a **reader** and (for a window or a
//!   watcher) a **writer**: a window that stops reading fills its own queue
//!   and is let go ([`ToViewer::Lagging`]); the program never waits for one.
//! - **signals** (the served program only): `SIGTERM` and `SIGHUP` hang up.
//!
//! # A window follows; it never leads
//!
//! On attach the holder sends a snapshot, then every byte its engine is fed
//! (after the filter), in order. A resize is the holder's: the window asks,
//! the holder resizes the engine and the PTY TOGETHER and says
//! [`ToViewer::Resize`] at that byte — so the window's mirror reflows at the
//! same byte the engine did. The same for the colours, and for the end of a
//! synchronized-output hold the program never ended: the library has no
//! clock, so the holder ends it after [`SYNC_HOLD_LIMIT`] and says
//! [`ToViewer::Mode`] there. All input is encoded HERE, against the modes the
//! program set, from what the window says it saw (a key, a click, a paste).
//!
//! One window at a time: the newest attach takes the terminal and the older
//! one is told [`ToViewer::Taken`]. A window that hangs can therefore never
//! hold a terminal hostage.
//!
//! # The clipboard is the person's
//!
//! A program may WRITE a clipboard only while a window is attached to put it
//! there. A READ is asked of the person through the window, with
//! [`CONSENT_DEADLINE`] to answer, and refused while no window is attached —
//! the library's read is synchronous, so the terminal waits for the answer.
//!
//! # The state, whole, beside the socket
//!
//! Every change of the terminal's state (not of its screen) rewrites
//! `<id>.json` whole — at most once per [`STATE_SPACING`], so a program that
//! sets its title in a loop costs the footer ten reads a second, not ten
//! thousand — and goes to the window as [`ToViewer::State`].
//!
//! # The end
//!
//! When the program ends, the state says so ([`TerminalState::ended`]) and
//! the window is told [`ToViewer::Exited`]. With no window attached, the
//! holder STAYS until one has shown it once (the person sees how it ended),
//! then leaves, taking its socket and state with it. `SIGTERM`, `SIGHUP` and
//! [`Ask::End`] hang up instead: the served holder leaves without killing the
//! program, its master closes with the process, and the kernel hangs the
//! program up — a shell saves its history on `SIGHUP`, never on `SIGKILL`.

use std::collections::VecDeque;
use std::io::{self, BufReader, Write as _};
use std::net::Shutdown;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context as _, Result};
use ronoctua_terminals::registry::Registry;
use ronoctua_terminals::spawn::Serve;
use ronoctua_terminals::state::{
    self, Attention, CommandMark, Ended, Notice, Progress, ProgressState, TerminalState,
};
use ronoctua_terminals::wire::{
    self, Ask, Attach, AttachMode, Attached, ClipboardContent, ClipboardKind, ClipboardWrite,
    Consent as ConsentAsk, Exit, ModeChange, Size, ToHolder, ToViewer,
};
use ronoctua_terminals::{GroupId, PROTOCOL, TerminalId};

use crate::ghostty::{
    self, Clipboard, ClipboardAsk, Colors, Config, Effect, Format, Geometry, KeyInput, Mode, Mods,
    MouseInput, PasteRefused, PasteSource, PromptStep, Rgb, Scheme, Vt,
};
use crate::inbound::{Inbound, Piece};
use crate::pty::{self, Pty};
use crate::status::Statuses;

/// How long a program may hold the screen (synchronized output, mode 2026)
/// before the holder lets it go — the specification's own suggestion, and
/// Ghostty's.
pub const SYNC_HOLD_LIMIT: Duration = Duration::from_secs(1);

/// The least time between two writes of the state file.
pub const STATE_SPACING: Duration = Duration::from_millis(100);

/// How long the holder waits, after the program ended, for the rest of what
/// it wrote: a process it left behind may hold the PTY open for ever, and
/// the end is not kept waiting on it.
pub const DRAIN_GRACE: Duration = Duration::from_secs(1);

/// How long the person has to answer a program's clipboard read.
pub const CONSENT_DEADLINE: Duration = Duration::from_secs(60);

/// How long a terminal's history stays as the program left it before the
/// holder compresses it: the engine says when anything compression cares
/// about moved ([`Vt::compression_activity`]), and the wait starts over each
/// time — a program writing does not pay for compressing what it is about to
/// write past, and a terminal left alone gives back the memory its history
/// held (`terminal.h`, "Scrollback Compression": the caller drives it, on an
/// idle timer of its own).
pub const COMPRESS_IDLE: Duration = Duration::from_secs(5);

/// How long one write to a window may take before the window is given up
/// as hung.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// How much live output a window may have queued before it is let go.
const LAG_LIMIT_BYTES: usize = if cfg!(test) { 256 << 10 } else { 32 << 20 };

/// How much of the program's output may wait for the engine.
const OUTPUT_IN_FLIGHT_BYTES: usize = 1 << 20;

/// How much input may wait for a program that does not read it ([`Typing`]).
const INPUT_QUEUE_BYTES: usize = 8 << 20;

/// One read of the PTY.
const READ_CHUNK_BYTES: usize = 64 << 10;

/// One piece of a snapshot.
const SNAPSHOT_CHUNK_BYTES: usize = 1 << 20;

/// What a terminal is started with.
#[derive(Debug, Clone)]
pub struct Setup {
    /// Which terminal.
    pub id: TerminalId,
    /// Its group, until a window says another.
    pub group: GroupId,
    /// Its place there.
    pub order: u32,
    /// Its size at birth.
    pub size: Size,
    /// Where the program starts; `None` for this process's folder.
    pub cwd: Option<PathBuf>,
    /// The program.
    pub program: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// The distro's version, for XTVERSION and `TERM_PROGRAM_VERSION`.
    pub version: String,
    /// Where its socket and state go — and, given, the program is told its
    /// id (`RONOCTUA_TERMINAL`). `None` for a terminal in its window's own
    /// process, which nothing else can find.
    pub registry: Option<Registry>,
    /// What the person chose for it (`terminal.json`): its history's limit
    /// and its cursor, which its engine takes before the program writes.
    pub born: ronoctua_terminals::spawn::Born,
}

impl Setup {
    /// The setup of a served terminal, from the holder's arguments.
    #[must_use]
    pub fn served(serve: Serve, version: &str, registry: Registry) -> Self {
        Self {
            id: serve.id,
            group: serve.group,
            order: serve.order,
            size: serve.size,
            cwd: serve.cwd,
            program: serve.program,
            args: serve.args,
            version: version.to_owned(),
            registry: Some(registry),
            born: serve.born,
        }
    }
}

/// The engine's setup for a terminal of `version`, with what the person
/// chose for it.
fn config_of(version: &str, born: ronoctua_terminals::spawn::Born) -> Config {
    use ronoctua_terminals::spawn::CursorShape as Shape;
    Config {
        scrollback_bytes: born
            .scrollback_bytes
            .map(|b| usize::try_from(b).unwrap_or(usize::MAX)),
        cursor: born.cursor.map(|(shape, blinks)| ghostty::CursorDefault {
            shape: match shape {
                Shape::Block => ghostty::CursorShape::Block,
                Shape::Bar => ghostty::CursorShape::Bar,
                Shape::Underline => ghostty::CursorShape::Underline,
            },
            blinks,
        }),
        ..Config::for_version(version)
    }
}

/// How a holder ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The program ended, and a window has seen it.
    Ended {
        /// Its exit code, a shell's way.
        code: i32,
    },
    /// It was hung up: a signal, or [`Ask::End`].
    HungUp,
}

/// What becomes of a program the holder lets go of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// In-process: killed and reaped with the holder, as a window's own
    /// terminal always was.
    Kill,
    /// Served: left to the kernel's hang-up when this process exits.
    HangUp,
}

/// What reaches the engine thread.
enum Event {
    Output(Vec<u8>),
    OutputEnd,
    ChildExited,
    Connection(UnixStream),
    Asked {
        conn: u64,
        ask: Ask,
        stream: UnixStream,
    },
    Frame {
        conn: u64,
        frame: ToHolder,
    },
    Gone {
        conn: u64,
    },
    HangUp,
}

/// A running terminal, in this process.
pub struct Holder {
    events: Sender<Event>,
    engine: JoinHandle<Outcome>,
}

impl Holder {
    /// Starts a terminal in THIS process — the window's own, when no scope
    /// can be had, or a test's. With a registry it also listens on its
    /// socket and keeps its state file, as a served one does.
    ///
    /// # Errors
    ///
    /// The socket, the PTY, the program or the engine could not be had.
    pub fn start(setup: Setup) -> Result<Self> {
        Self::begin(setup, Ending::Kill)
    }

    /// A new connection, as if it had come to the socket — the in-process
    /// window's way in.
    ///
    /// # Errors
    ///
    /// No socket pair, or the holder has ended.
    pub fn connect(&self) -> io::Result<UnixStream> {
        let (ours, theirs) = UnixStream::pair()?;
        self.events
            .send(Event::Connection(theirs))
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        Ok(ours)
    }

    /// Hangs the terminal up, as `SIGTERM` would.
    pub fn hang_up(&self) {
        let _ = self.events.send(Event::HangUp);
    }

    /// Waits for the holder to end.
    #[must_use]
    pub fn wait(self) -> Outcome {
        self.engine.join().unwrap_or(Outcome::HungUp)
    }

    fn begin(setup: Setup, ending: Ending) -> Result<Self> {
        let (id, registry) = (setup.id.clone(), setup.registry.clone());
        Self::assemble(setup, ending).inspect_err(|_| {
            // A start that failed after its socket was bound leaves nothing
            // for the census to find and clean.
            if let Some(r) = &registry {
                r.forget(&id);
            }
        })
    }

    fn assemble(setup: Setup, ending: Ending) -> Result<Self> {
        let Setup {
            id,
            group,
            order,
            size,
            cwd,
            program,
            args,
            version,
            registry,
            born,
        } = setup;
        let size = Size {
            columns: size.columns.max(1),
            rows: size.rows.max(1),
            ..size
        };
        // THE SOCKET BEFORE THE STATE, so no reader ever finds a state file
        // whose socket is not there yet (the census would call it dead).
        let listener = match &registry {
            Some(r) => {
                r.ensure().context("the terminals' folder")?;
                let path = r.socket(&id);
                Some(
                    UnixListener::bind(&path)
                        .with_context(|| format!("bind {}", path.display()))?,
                )
            }
            None => None,
        };
        let cwd = cwd.or_else(|| std::env::current_dir().ok());
        let pty = Pty::spawn(&pty::Spawn {
            program: &program,
            args: &args,
            size: pty_size(size),
            cwd: cwd.as_deref(),
            terminal: registry.is_some().then_some(id.as_str()),
            version: &version,
        })?;
        let mut vt = Vt::new(size.columns, size.rows, &config_of(&version, born))
            .context("the terminal's engine")?;
        vt.resize(
            size.columns,
            size.rows,
            u32::from(size.cell_width_px),
            u32::from(size.cell_height_px),
        )
        .context("size the engine")?;
        let consent = Arc::new(Consent::default());
        let asker = Arc::clone(&consent);
        vt.set_clipboard_reader(Some(Box::new(move |ask| asker.ask(ask))))
            .context("take clipboard reads")?;

        let (events, inbox) = mpsc::channel();
        let credit = Arc::new(Credit::default());
        spawn_output(&pty, events.clone(), Arc::clone(&credit))?;
        let typing = spawn_input(&pty)?;
        spawn_child_wait(pty.pid(), events.clone())?;
        let stop_accept = match listener {
            Some(listener) => Some(spawn_accept(listener, events.clone())?),
            None => None,
        };

        let state = TerminalState {
            protocol: PROTOCOL,
            id: id.clone(),
            group,
            order,
            viewer: None,
            front: false,
            seen: false,
            title: String::new(),
            program: Path::new(&program)
                .file_name()
                .map_or_else(|| program.clone(), |n| n.to_string_lossy().into_owned()),
            cwd,
            started_s: now_s(),
            alone_since_s: Some(now_s()),
            attention: Attention::default(),
            notice: None,
            command: None,
            front_asked: 0,
            ended: None,
            statuses: Vec::new(),
            progress: None,
        };
        if let Some(r) = &registry {
            r.write(&state).context("the terminal's state file")?;
        }
        let mut inbound = Inbound::new();
        inbound.set_size(
            size.columns,
            size.rows,
            size.cell_width_px,
            size.cell_height_px,
        );
        let engine = Engine {
            id,
            vt,
            pty: Some(pty),
            inbound,
            statuses: Statuses::default(),
            state,
            size,
            colors: None,
            typing,
            credit,
            consent,
            registry,
            events: events.clone(),
            stop_accept,
            ending,
            viewer: None,
            watchers: Vec::new(),
            next_conn: 0,
            hold_until: None,
            write_due: None,
            last_write: Some(Instant::now()),
            dirty: false,
            drain_until: None,
            exit_code: None,
            output_ended: false,
            ended: false,
            leaving: false,
            was_blocked: false,
            command_began: None,
            squeeze: Squeeze::default(),
        };
        let engine = std::thread::Builder::new()
            .name("term-engine".to_owned())
            .spawn(move || engine.run(&inbox))
            .context("the engine's thread")?;
        Ok(Self { events, engine })
    }
}

/// Serves a terminal from its own process: [`Holder`] plus the signals that
/// hang it up, and `ready` once it can be attached to (the holder program
/// says [`ronoctua_terminals::spawn::READY`] there).
///
/// # Errors
///
/// The terminal could not be started, or the signals not taken.
pub fn serve(setup: Setup, ready: impl FnOnce()) -> Result<Outcome> {
    use signal_hook::consts::{SIGHUP, SIGTERM};
    // Taken BEFORE anything starts: a stop that arrived between the start
    // and the handler would leave the socket and state behind.
    let mut signals = signal_hook::iterator::Signals::new([SIGTERM, SIGHUP])
        .context("take SIGTERM and SIGHUP")?;
    let holder = Holder::begin(setup, Ending::HangUp)?;
    let events = holder.events.clone();
    std::thread::Builder::new()
        .name("term-signals".to_owned())
        .spawn(move || {
            if signals.forever().next().is_some() {
                let _ = events.send(Event::HangUp);
            }
        })
        .context("the signals' thread")?;
    ready();
    Ok(holder.wait())
}

/// The kernel's size for the wire's.
const fn pty_size(size: Size) -> pty::Size {
    pty::Size {
        columns: size.columns,
        rows: size.rows,
        cell_width_px: size.cell_width_px,
        cell_height_px: size.cell_height_px,
    }
}

fn now_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Locks, through a poisoned lock: a thread that panicked holding one of
/// these left nothing half-written that the others could not read.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// How much of the program's output is in the engine's channel — the
/// output thread waits while it is past [`OUTPUT_IN_FLIGHT_BYTES`].
#[derive(Default)]
struct Credit {
    in_flight: Mutex<(usize, bool)>,
    back: Condvar,
}

impl Credit {
    /// Waits for room, then counts `n` in; `false` once the engine has gone.
    fn take(&self, n: usize) -> bool {
        let mut g = lock(&self.in_flight);
        while g.0 >= OUTPUT_IN_FLIGHT_BYTES && !g.1 {
            g = self.back.wait(g).unwrap_or_else(PoisonError::into_inner);
        }
        g.0 += n;
        !g.1
    }

    fn give(&self, n: usize) {
        let mut g = lock(&self.in_flight);
        g.0 = g.0.saturating_sub(n);
        self.back.notify_all();
    }

    fn close(&self) {
        lock(&self.in_flight).1 = true;
        self.back.notify_all();
    }
}

fn spawn_output(pty: &Pty, events: Sender<Event>, credit: Arc<Credit>) -> Result<()> {
    let mut output = pty.output()?;
    std::thread::Builder::new()
        .name("term-output".to_owned())
        .spawn(move || {
            let mut buf = vec![0u8; READ_CHUNK_BYTES];
            loop {
                match output.read(&mut buf) {
                    Ok(Some(n)) => {
                        if !credit.take(n) || events.send(Event::Output(buf[..n].to_vec())).is_err()
                        {
                            return;
                        }
                    }
                    Ok(None) | Err(_) => {
                        let _ = events.send(Event::OutputEnd);
                        return;
                    }
                }
            }
        })
        .context("the output's thread")?;
    Ok(())
}

fn spawn_input(pty: &Pty) -> Result<Typing> {
    let mut input = pty.input()?;
    let (to_pty, writes) = mpsc::channel::<Vec<u8>>();
    let typing = Typing {
        to_pty,
        queued: Arc::new(AtomicUsize::new(0)),
    };
    let queued = Arc::clone(&typing.queued);
    std::thread::Builder::new()
        .name("term-input".to_owned())
        .spawn(move || {
            for bytes in writes {
                if input.write_all(&bytes).is_err() {
                    return;
                }
                queued.fetch_sub(bytes.len(), Ordering::AcqRel);
            }
        })
        .context("the input's thread")?;
    Ok(typing)
}

/// What waits to be written to the program, bounded.
///
/// Its input is not only what the person types: every question a program
/// asks the terminal (where is the cursor, what are you) is answered on it.
/// A program that writes questions and never reads — `cat` of a file that
/// happens to hold them — would otherwise queue answers for as long as it
/// writes, and the holder would grow until it was killed. Past
/// [`INPUT_QUEUE_BYTES`] what comes is dropped: answers nobody reads, keys a
/// program that reads nothing would never have seen. A write is refused only
/// when something already waits past the bound, so one paste of any size
/// still goes whole.
struct Typing {
    to_pty: Sender<Vec<u8>>,
    queued: Arc<AtomicUsize>,
}

impl Typing {
    /// Queues `bytes` for the program; `false` if they were dropped.
    fn send(&self, bytes: Vec<u8>) -> bool {
        if self.queued.load(Ordering::Acquire) >= INPUT_QUEUE_BYTES {
            return false;
        }
        self.queued.fetch_add(bytes.len(), Ordering::AcqRel);
        self.to_pty.send(bytes).is_ok()
    }
}

fn spawn_child_wait(pid: u32, events: Sender<Event>) -> Result<()> {
    use rustix::process::{Pid, WaitId, WaitIdOptions, waitid};
    let pid = i32::try_from(pid)
        .ok()
        .and_then(Pid::from_raw)
        .context("the program's process id")?;
    std::thread::Builder::new()
        .name("term-child".to_owned())
        .spawn(move || {
            // NOWAIT: told it ended, and left for the engine to reap through
            // the `Child` that owns it — reaping here would leave that
            // `Child` asking after a pid that may already be another's.
            loop {
                match waitid(
                    WaitId::Pid(pid),
                    WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
                ) {
                    Err(e) if e == rustix::io::Errno::INTR => {}
                    _ => break,
                }
            }
            let _ = events.send(Event::ChildExited);
        })
        .context("the child's thread")?;
    Ok(())
}

fn spawn_accept(listener: UnixListener, events: Sender<Event>) -> Result<Arc<AtomicBool>> {
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = Arc::clone(&stop);
    std::thread::Builder::new()
        .name("term-accept".to_owned())
        .spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::Acquire) {
                    return;
                }
                if let Ok(stream) = stream
                    && events.send(Event::Connection(stream)).is_err()
                {
                    return;
                }
            }
        })
        .context("the socket's thread")?;
    Ok(stop)
}

/// What waits to be written to one window (or watcher), in order.
#[derive(Default)]
struct Outbox {
    queue: Mutex<Queue>,
    wake: Condvar,
}

#[derive(Default)]
struct Queue {
    /// Frames, and whether each counts as live output.
    frames: VecDeque<(Vec<u8>, bool)>,
    live_bytes: usize,
    closing: bool,
    dead: bool,
}

impl Outbox {
    /// Queues a frame; `false` when it is live output past the window's
    /// limit — the window is too far behind to catch up frame by frame.
    fn push(&self, frame: Vec<u8>, live: bool) -> bool {
        let mut q = lock(&self.queue);
        if q.dead || q.closing {
            return true;
        }
        if live {
            if q.live_bytes + frame.len() > LAG_LIMIT_BYTES {
                return false;
            }
            q.live_bytes += frame.len();
        }
        q.frames.push_back((frame, live));
        self.wake.notify_one();
        true
    }

    /// Queues a last frame after what is queued, and closes.
    fn end_with(&self, last: Vec<u8>) {
        let mut q = lock(&self.queue);
        if !q.closing {
            q.frames.push_back((last, false));
            q.closing = true;
        }
        self.wake.notify_one();
    }

    /// Drops what is queued, writes `last` alone, and closes — for a window
    /// that fell behind, whose backlog is worth nothing to it.
    fn cut_with(&self, last: Vec<u8>) {
        let mut q = lock(&self.queue);
        q.frames.clear();
        q.live_bytes = 0;
        q.frames.push_back((last, false));
        q.closing = true;
        self.wake.notify_one();
    }

    /// Closes once what is queued is written.
    fn close(&self) {
        lock(&self.queue).closing = true;
        self.wake.notify_one();
    }

    /// Nothing more is written — the writer failed, or the terminal is hung
    /// up and owes nothing. A writer waiting for a frame is woken to see it:
    /// left asleep, it would keep its stream open, and the window would wait
    /// for the end of a terminal that has ended.
    fn kill(&self) {
        let mut q = lock(&self.queue);
        q.dead = true;
        q.frames.clear();
        q.live_bytes = 0;
        self.wake.notify_one();
    }

    /// The next frame to write, waiting for one; `None` when closed.
    fn next(&self) -> Option<Vec<u8>> {
        let mut q = lock(&self.queue);
        loop {
            if q.dead {
                return None;
            }
            if let Some((frame, live)) = q.frames.pop_front() {
                if live {
                    q.live_bytes = q.live_bytes.saturating_sub(frame.len());
                }
                return Some(frame);
            }
            if q.closing {
                return None;
            }
            q = self.wake.wait(q).unwrap_or_else(PoisonError::into_inner);
        }
    }
}

/// Writes an outbox to its stream, each frame with one `write_all`, and
/// shuts the stream's writing half when the outbox closes.
///
/// A write that FAILS — the window hung for [`WRITE_TIMEOUT`], or closed
/// its reading half — shuts BOTH halves: the connection's reader then reads
/// its end and the window goes as one that left goes ([`Event::Gone`]). A
/// window nothing can be written to must not go on counting as the one that
/// shows the terminal — it would hide it from every census, and keep
/// clipboard reads waiting on a question nobody can see.
fn spawn_writer(stream: UnixStream, outbox: Arc<Outbox>) -> Option<JoinHandle<()>> {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    std::thread::Builder::new()
        .name("term-viewer".to_owned())
        .spawn(move || {
            let mut stream = stream;
            while let Some(frame) = outbox.next() {
                if stream.write_all(&frame).is_err() {
                    outbox.kill();
                    let _ = stream.shutdown(Shutdown::Both);
                    return;
                }
            }
            let _ = stream.shutdown(Shutdown::Write);
        })
        .ok()
}

/// Reads one connection: its first line, then — for a window — its frames.
/// A person's answer to a clipboard read goes straight to the question
/// (the engine is waiting inside it, and could not take it).
fn read_client(conn: u64, stream: UnixStream, events: &Sender<Event>, consent: &Consent) {
    let Ok(writer) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(stream);
    let ask = match wire::read_line(&mut reader) {
        Ok(Some(line)) => match Ask::parse(&line) {
            Ok(ask) => ask,
            Err(e) => {
                tracing::debug!(error = %e, "a first line that is no ask; let go");
                return;
            }
        },
        // The census's knock: connected, and gone.
        _ => return,
    };
    let lasts = matches!(ask, Ask::Attach(_) | Ask::Watch);
    if events
        .send(Event::Asked {
            conn,
            ask,
            stream: writer,
        })
        .is_err()
        || !lasts
    {
        return;
    }
    while let Ok(Some((kind, payload))) = wire::read_frame(&mut reader) {
        match ToHolder::decode(kind, payload) {
            Ok(ToHolder::Answer(answer)) => consent.answer(conn, answer),
            Ok(frame) => {
                if events.send(Event::Frame { conn, frame }).is_err() {
                    break;
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "a window sent what is no frame; let go");
                break;
            }
        }
    }
    consent.leave(conn);
    let _ = events.send(Event::Gone { conn });
}

/// The question of a program's clipboard read, between the engine (waiting
/// in it) and the window's reader (bringing the answer).
#[derive(Default)]
struct Consent {
    inner: Mutex<Asking>,
}

#[derive(Default)]
struct Asking {
    window: Option<(u64, Arc<Outbox>)>,
    next: u64,
    pending: Option<(u64, mpsc::SyncSender<Option<String>>)>,
}

impl Consent {
    fn attach(&self, conn: u64, outbox: Arc<Outbox>) {
        let mut a = lock(&self.inner);
        a.window = Some((conn, outbox));
        a.pending = None;
    }

    /// A window left: a question it was asked is refused.
    fn leave(&self, conn: u64) {
        let mut a = lock(&self.inner);
        if a.window.as_ref().is_some_and(|(c, _)| *c == conn) {
            a.window = None;
            a.pending = None;
        }
    }

    fn answer(&self, conn: u64, answer: wire::Answer) {
        let mut a = lock(&self.inner);
        let ours = a.window.as_ref().is_some_and(|(c, _)| *c == conn);
        if ours
            && a.pending.as_ref().is_some_and(|(id, _)| *id == answer.id)
            && let Some((_, reply)) = a.pending.take()
        {
            let _ = reply.send(answer.text);
        }
    }

    /// Asks the attached window, and waits for its answer — refused at once
    /// with no window, and after [`CONSENT_DEADLINE`] with no answer.
    fn ask(&self, ask: &ClipboardAsk) -> Option<String> {
        let (id, heard) = {
            let mut a = lock(&self.inner);
            let (_, outbox) = a.window.clone()?;
            let id = a.next;
            a.next += 1;
            let (reply, heard) = mpsc::sync_channel(1);
            a.pending = Some((id, reply));
            outbox.push(
                ToViewer::Consent(ConsentAsk {
                    id,
                    clipboard: clipboard_kind(ask.clipboard),
                    mimes: ask.mimes.clone(),
                    program: state::plain(&ask.program, state::TITLE_MAX_CHARS),
                })
                .encode(),
                false,
            );
            (id, heard)
        };
        let answer = heard.recv_timeout(CONSENT_DEADLINE).ok().flatten();
        let mut a = lock(&self.inner);
        if a.pending.as_ref().is_some_and(|(p, _)| *p == id) {
            a.pending = None;
        }
        answer
    }
}

const fn clipboard_kind(c: Clipboard) -> ClipboardKind {
    match c {
        Clipboard::Standard => ClipboardKind::Standard,
        Clipboard::Selection => ClipboardKind::Selection,
        Clipboard::Primary => ClipboardKind::Primary,
    }
}

/// THE HISTORY'S COMPRESSION, on a clock armed only while there is work
/// for it: the idle wait after the engine's activity token moved, then one
/// bounded step a turn of the engine's loop — so a key or a resize that comes
/// meanwhile waits for one step, never for the whole pass — until the engine
/// says nothing remains. A terminal at rest, its history compressed, wakes
/// nothing.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Squeeze {
    /// The engine's activity token as last seen.
    seen: Option<u64>,
    /// When the next step is due; `None` while nothing is owed.
    due: Option<Instant>,
}

impl Squeeze {
    /// The engine's token `now`: when it moved, the idle wait starts over.
    fn heard(&mut self, activity: u64, now: Instant) {
        if self.seen != Some(activity) {
            self.seen = Some(activity);
            self.due = Some(now + COMPRESS_IDLE);
        }
    }

    /// One step ran: another is due at once while `more` remains, and none
    /// after. The token as it stands AFTER the step is the one seen — the
    /// step may move it itself, and a step that restarted its own wait would
    /// never end.
    fn stepped(&mut self, more: bool, activity: u64, now: Instant) {
        self.seen = Some(activity);
        self.due = more.then_some(now);
    }

    /// Runs the step that is due by `now`, if one is.
    fn run(&mut self, vt: &mut Vt, now: Instant) {
        if self.due.is_some_and(|at| at <= now) {
            let more = vt.compress_step();
            self.stepped(more, vt.compression_activity(), now);
        }
    }
}

/// The attached window.
struct Viewer {
    conn: u64,
    outbox: Arc<Outbox>,
    writer: Option<JoinHandle<()>>,
}

/// A connection that follows the state.
struct Watcher {
    conn: u64,
    outbox: Arc<Outbox>,
}

/// Everything the engine thread owns.
struct Engine {
    id: TerminalId,
    vt: Vt,
    pty: Option<Pty>,
    inbound: Inbound,
    statuses: Statuses,
    state: TerminalState,
    size: Size,
    colors: Option<wire::Colors>,
    typing: Typing,
    credit: Arc<Credit>,
    consent: Arc<Consent>,
    registry: Option<Registry>,
    events: Sender<Event>,
    stop_accept: Option<Arc<AtomicBool>>,
    ending: Ending,
    viewer: Option<Viewer>,
    watchers: Vec<Watcher>,
    next_conn: u64,
    hold_until: Option<Instant>,
    write_due: Option<Instant>,
    last_write: Option<Instant>,
    dirty: bool,
    drain_until: Option<Instant>,
    exit_code: Option<i32>,
    output_ended: bool,
    ended: bool,
    leaving: bool,
    was_blocked: bool,
    command_began: Option<Instant>,
    squeeze: Squeeze,
}

impl Engine {
    fn run(mut self, inbox: &Receiver<Event>) -> Outcome {
        loop {
            let deadline = [
                self.hold_until,
                self.write_due,
                self.drain_until,
                self.squeeze.due,
            ]
            .into_iter()
            .flatten()
            .min();
            let event = match deadline {
                Some(at) => {
                    match inbox.recv_timeout(at.saturating_duration_since(Instant::now())) {
                        Ok(e) => Some(e),
                        Err(RecvTimeoutError::Timeout) => None,
                        Err(RecvTimeoutError::Disconnected) => return self.finish(Outcome::HungUp),
                    }
                }
                None => match inbox.recv() {
                    Ok(e) => Some(e),
                    Err(_) => return self.finish(Outcome::HungUp),
                },
            };
            if let Some(event) = event
                && let Some(outcome) = self.on(event)
            {
                return self.finish(outcome);
            }
            // WHAT THE EVENT DID TO THE HISTORY: output, a resize, a scroll —
            // the engine's token says whether compression has work again.
            self.squeeze
                .heard(self.vt.compression_activity(), Instant::now());
            self.clocks();
            if self.leaving {
                let code = self.exit_code.unwrap_or(0);
                return self.finish(Outcome::Ended { code });
            }
        }
    }

    fn on(&mut self, event: Event) -> Option<Outcome> {
        match event {
            Event::Output(bytes) => {
                self.credit.give(bytes.len());
                self.output(&bytes);
            }
            Event::OutputEnd => {
                self.output_ended = true;
                if self.exit_code.is_some() {
                    self.program_ended();
                }
            }
            Event::ChildExited => {
                self.exit_code = Some(self.reap());
                if self.output_ended {
                    self.program_ended();
                } else {
                    self.drain_until = Some(Instant::now() + DRAIN_GRACE);
                }
            }
            Event::Connection(stream) => {
                let conn = self.next_conn;
                self.next_conn += 1;
                let (events, consent) = (self.events.clone(), Arc::clone(&self.consent));
                let _ = std::thread::Builder::new()
                    .name("term-client".to_owned())
                    .spawn(move || read_client(conn, stream, &events, &consent));
            }
            Event::Asked { conn, ask, stream } => return self.asked(conn, ask, stream),
            Event::Frame { conn, frame } => {
                if self.viewer.as_ref().is_some_and(|v| v.conn == conn) {
                    self.input(frame);
                }
            }
            Event::Gone { conn } => {
                if self.viewer.as_ref().is_some_and(|v| v.conn == conn) {
                    self.let_viewer_go(None);
                }
                self.watchers.retain(|w| w.conn != conn);
            }
            Event::HangUp => return Some(Outcome::HungUp),
        }
        None
    }

    /// The program's exit code, reaping it.
    fn reap(&mut self) -> i32 {
        let Some(pty) = &mut self.pty else { return 0 };
        match pty.exit_code() {
            Ok(Some(code)) => code,
            // Told it ended and still not waitable: it is made to end, and
            // says so in its code (killed, 128 + 9).
            _ => {
                let _ = pty.finish();
                pty.exit_code().ok().flatten().unwrap_or(128 + 9)
            }
        }
    }

    /// The deadlines that came.
    fn clocks(&mut self) {
        let now = Instant::now();
        if self.hold_until.is_some_and(|at| at <= now) {
            self.hold_until = None;
            if self.vt.held() {
                // The program never ended its hold: end it, here, for the
                // engine and — at this byte — for the window.
                if let Err(e) = self.vt.set_mode(Mode::SYNC_OUTPUT, false) {
                    tracing::warn!(error = %e, "could not end a synchronized-output hold");
                }
                self.tell_viewer(
                    ToViewer::Mode(ModeChange {
                        number: 2026,
                        ansi: false,
                        on: false,
                    }),
                    false,
                );
                self.effects();
            }
        }
        if self.drain_until.is_some_and(|at| at <= now) {
            self.program_ended();
        }
        if self.write_due.is_some_and(|at| at <= now) {
            self.write_state();
        }
        if self.dirty && self.write_due.is_none() {
            match self.last_write {
                Some(last) if last.elapsed() < STATE_SPACING => {
                    self.write_due = Some(last + STATE_SPACING);
                }
                _ => self.write_state(),
            }
        }
        self.squeeze.run(&mut self.vt, now);
    }

    fn write_pty(&self, bytes: Vec<u8>) {
        if !bytes.is_empty() && !self.typing.send(bytes) {
            tracing::debug!(id = %self.id, "the program reads none of its input; dropped some");
        }
    }

    /// Sends the window a frame; one that fell too far behind is let go.
    fn tell_viewer(&mut self, frame: ToViewer, live: bool) {
        let Some(v) = &self.viewer else { return };
        if !v.outbox.push(frame.encode(), live) {
            tracing::info!(id = %self.id, "a window fell behind its terminal; let go");
            self.let_viewer_go(Some(ToViewer::Lagging.encode()));
        }
    }

    fn output(&mut self, bytes: &[u8]) {
        for piece in self.inbound.filter(bytes) {
            match piece {
                Piece::Feed(bytes) => {
                    if let Some(v) = &self.viewer {
                        // The frame FIRST: whatever the bytes make the engine
                        // say (a clipboard question, a write) follows them.
                        let fits = v.outbox.push(wire::bytes_frame(&bytes), true);
                        self.vt.feed(&bytes);
                        if !fits {
                            tracing::info!(id = %self.id, "a window fell behind its terminal; let go");
                            self.let_viewer_go(Some(ToViewer::Lagging.encode()));
                        }
                    } else {
                        self.vt.feed(&bytes);
                    }
                    self.effects();
                }
                Piece::Reply(reply) => self.write_pty(reply),
            }
        }
    }

    /// What the engine said since it was last asked.
    fn effects(&mut self) {
        let effects = self.vt.take_effects();
        self.write_pty(effects.to_pty);
        for effect in effects.events {
            self.effect(effect);
        }
    }

    fn effect(&mut self, effect: Effect) {
        match effect {
            Effect::Bell => {
                if !self.state.seen {
                    self.state.attention.bell += 1;
                    self.dirty = true;
                }
            }
            Effect::Title(title) => {
                let title = state::plain(&title, state::TITLE_MAX_CHARS);
                if title != self.state.title {
                    self.state.title = title;
                    self.dirty = true;
                }
            }
            Effect::Pwd(said) => {
                if let Some(dir) = folder_of(&said)
                    && self.state.cwd.as_ref() != Some(&dir)
                {
                    self.state.cwd = Some(dir);
                    self.dirty = true;
                }
            }
            Effect::Notification { title, body } => {
                let generation = self.state.notice.as_ref().map_or(1, |n| n.generation + 1);
                self.state.notice = Some(Notice {
                    title: state::plain(&title, state::NOTICE_MAX_CHARS),
                    body: state::plain(&body, state::NOTICE_MAX_CHARS),
                    generation,
                });
                if !self.state.seen {
                    self.state.attention.notice += 1;
                }
                self.dirty = true;
            }
            Effect::Progress { state, percent } => {
                let progress = match state {
                    ghostty::ProgressState::Remove => None,
                    ghostty::ProgressState::Set => Some(ProgressState::Set),
                    ghostty::ProgressState::Error => Some(ProgressState::Error),
                    ghostty::ProgressState::Indeterminate => Some(ProgressState::Indeterminate),
                    ghostty::ProgressState::Pause => Some(ProgressState::Pause),
                }
                .map(|state| Progress { state, percent });
                if progress != self.state.progress {
                    self.state.progress = progress;
                    self.dirty = true;
                }
            }
            Effect::ProgramStatus(status) => {
                if self.statuses.report(&status, now_ms()) {
                    self.statuses_changed();
                }
            }
            Effect::Prompt(prompt) => self.prompt(prompt.step, prompt.exit_code),
            Effect::ClipboardWrite {
                clipboard,
                contents,
            } => self.tell_viewer(
                ToViewer::Clipboard(ClipboardWrite {
                    clipboard: clipboard_kind(clipboard),
                    contents: contents
                        .into_iter()
                        .map(|(mime, data)| ClipboardContent { mime, data })
                        .collect(),
                }),
                false,
            ),
            Effect::RenderHold(on) => {
                self.hold_until = on.then(|| Instant::now() + SYNC_HOLD_LIMIT);
            }
            Effect::Reset => {
                if self.statuses.reset() {
                    self.statuses_changed();
                }
                if !self.state.title.is_empty() {
                    self.state.title.clear();
                    self.dirty = true;
                }
            }
            // The window's size is the window's: a program does not resize
            // it (and an option to let one would be the person's).
            Effect::ResizeRequest { .. } => {}
        }
    }

    fn prompt(&mut self, step: PromptStep, exit_code: Option<i32>) {
        match step {
            PromptStep::PromptStart => {
                if self.statuses.prompt() {
                    self.statuses_changed();
                }
            }
            PromptStep::InputStart => {}
            PromptStep::OutputStart => {
                self.command_began = Some(Instant::now());
                self.state.command = Some(CommandMark {
                    since_s: now_s(),
                    running: true,
                    last_exit: None,
                    duration_ms: None,
                });
                self.dirty = true;
            }
            PromptStep::CommandEnd => {
                if self.statuses.prompt() {
                    self.statuses_changed();
                }
                let took = self
                    .command_began
                    .take()
                    .map(|t| u64::try_from(t.elapsed().as_millis()).unwrap_or(u64::MAX));
                if let Some(c) = &mut self.state.command {
                    c.running = false;
                    c.last_exit = exit_code;
                    c.duration_ms = took;
                    self.dirty = true;
                }
            }
        }
    }

    fn statuses_changed(&mut self) {
        self.state.statuses = self.statuses.records().to_vec();
        let blocked = self.statuses.blocked();
        if blocked && !self.was_blocked && !self.state.seen {
            self.state.attention.waiting = true;
        }
        if !blocked {
            self.state.attention.waiting = false;
        }
        self.was_blocked = blocked;
        self.dirty = true;
    }

    /// The window said something.
    fn input(&mut self, frame: ToHolder) {
        match frame {
            ToHolder::Key(key) => {
                let bytes = self.vt.encode_key(&key_input(&key));
                self.write_pty(bytes);
            }
            ToHolder::Text(text) => {
                // An input method commits text, never a control: one that did
                // would type a command the person never saw.
                let text: String = text.chars().filter(|c| !c.is_control()).collect();
                self.write_pty(text.into_bytes());
            }
            ToHolder::Mouse(mouse) => {
                let bytes = self
                    .vt
                    .encode_mouse(&mouse_input(&mouse), &geometry(self.size));
                self.write_pty(bytes);
            }
            ToHolder::Focus(focus) => {
                let bytes = self.vt.encode_focus(focus.gained);
                self.write_pty(bytes);
            }
            ToHolder::Paste(paste) => {
                let source = match paste.source {
                    wire::PasteSource::Clipboard => PasteSource::Clipboard,
                    wire::PasteSource::Text => PasteSource::Text,
                };
                // A PASTE NOT WRITTEN IS SAID TO THE WINDOW that sent it,
                // which says it on its line: the window judged it safe by
                // its mirror's modes, and the program may have changed them
                // a frame before this one judged it by the engine's.
                let refused = match self.vt.paste(&paste.text, source, paste.allow_unsafe) {
                    Ok(_) => {
                        self.effects();
                        None
                    }
                    Err(PasteRefused::Unsafe) => {
                        tracing::info!(
                            id = %self.id,
                            "a paste that could run a command arrived without the person's yes; not written"
                        );
                        Some(wire::PasteRefusal::Unsafe)
                    }
                    Err(PasteRefused::Library(e)) => {
                        tracing::warn!(error = %e, "a paste was refused");
                        Some(wire::PasteRefusal::Failed)
                    }
                };
                if let Some(why) = refused {
                    self.tell_viewer(ToViewer::PasteRefused(why), false);
                }
            }
            ToHolder::Resize(size) => self.resize(size),
            ToHolder::Colors(colors) => self.set_colors(colors, true),
            // Answers go straight to the question (`read_client`).
            ToHolder::Answer(_) => {}
            ToHolder::Place(place) => self.place(place),
            ToHolder::Detach => self.let_viewer_go(None),
        }
    }

    /// Resizes the engine and the PTY together, and says so at this byte.
    fn resize(&mut self, size: Size) {
        let size = Size {
            columns: size.columns.max(1),
            rows: size.rows.max(1),
            ..size
        };
        if size == self.size {
            return;
        }
        if let Err(e) = self.vt.resize(
            size.columns,
            size.rows,
            u32::from(size.cell_width_px),
            u32::from(size.cell_height_px),
        ) {
            tracing::warn!(error = %e, "the engine refused a size");
            return;
        }
        if let Some(pty) = &self.pty
            && let Err(e) = pty.resize(pty_size(size))
        {
            tracing::warn!(error = %e, "the PTY refused a size");
        }
        self.inbound.set_size(
            size.columns,
            size.rows,
            size.cell_width_px,
            size.cell_height_px,
        );
        self.size = size;
        self.tell_viewer(ToViewer::Resize(size), false);
        self.effects();
    }

    fn set_colors(&mut self, colors: wire::Colors, announce: bool) {
        let Some(engine) = engine_colors(&colors) else {
            tracing::warn!("a window sent a palette that is not 256 colours; kept the last");
            return;
        };
        if let Err(e) = self.vt.set_colors(&engine) {
            tracing::warn!(error = %e, "the engine refused the colours");
            return;
        }
        if let Some(palette) = colors.palette_of_256() {
            self.inbound
                .set_colors(&palette.map(|c| c.0), colors.background.0);
        }
        if announce {
            self.tell_viewer(ToViewer::Colors(colors.clone()), false);
        }
        self.colors = Some(colors);
        self.effects();
    }

    fn place(&mut self, place: wire::Place) {
        self.state.front = place.front;
        self.state.seen = place.seen;
        // A RENUMBERING, heard as it happens: the window ran out of room
        // between two places, and a window opened on this group later reads
        // the order from the holders.
        if let Some(order) = place.order {
            self.state.order = order;
        }
        if place.seen {
            self.state.attention = Attention::default();
        }
        self.dirty = true;
    }

    /// A first line was read.
    fn asked(&mut self, conn: u64, ask: Ask, stream: UnixStream) -> Option<Outcome> {
        match ask {
            Ask::Watch => {
                let outbox = Arc::new(Outbox::default());
                if let Ok(line) = self.state.line() {
                    outbox.push(line, false);
                }
                let _ = spawn_writer(stream, Arc::clone(&outbox));
                self.watchers.push(Watcher { conn, outbox });
            }
            Ask::State => {
                if let Ok(line) = self.state.line() {
                    // On a thread: an asker that does not read must not hold
                    // the engine.
                    let _ = std::thread::Builder::new()
                        .name("term-answer".to_owned())
                        .spawn(move || {
                            let mut stream = stream;
                            let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
                            let _ = stream.write_all(&line);
                        });
                }
            }
            Ask::Attach(attach) => self.attach(conn, stream, attach),
            Ask::Place(place) => {
                self.place(place);
                done(&stream);
            }
            Ask::Front => {
                self.state.front_asked += 1;
                self.write_state();
                done(&stream);
            }
            Ask::End => {
                done(&stream);
                return Some(match (self.ended, self.exit_code) {
                    (true, Some(code)) => Outcome::Ended { code },
                    _ => Outcome::HungUp,
                });
            }
            Ask::Detach => {
                self.let_viewer_go(Some(ToViewer::Taken.encode()));
                done(&stream);
            }
        }
        None
    }

    /// A window attaches: the newest one takes the terminal.
    fn attach(&mut self, conn: u64, stream: UnixStream, attach: Attach) {
        let taken = self.viewer.take();
        if let Some(old) = &taken {
            // AFTER what it was owed: a window taken mid-stream still has a
            // whole picture of what it showed.
            old.outbox.end_with(ToViewer::Taken.encode());
            self.consent.leave(old.conn);
        }
        let exact = attach.protocol == PROTOCOL && attach.engine == ghostty::ENGINE;
        self.set_colors(attach.colors.clone(), false);
        self.resize(attach.size);
        let body = if exact {
            self.vt.snapshot()
        } else {
            self.vt.format(Format::Vt)
        };
        let body = match body {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(error = %e, "no snapshot for a window; it is let go");
                if taken.is_some() {
                    // The window it took from is gone too: no window shows
                    // it now, and the state must not say one does.
                    self.alone();
                }
                return;
            }
        };
        let outbox = Arc::new(Outbox::default());
        let attached = Attached {
            protocol: PROTOCOL,
            mode: if exact {
                AttachMode::Exact
            } else {
                AttachMode::Portable
            },
            id: self.id.clone(),
            size: self.size,
        };
        outbox.push(ToViewer::Attached(attached).encode(), false);
        for chunk in body.chunks(SNAPSHOT_CHUNK_BYTES) {
            outbox.push(ToViewer::Snapshot(chunk.to_vec()).encode(), false);
        }
        outbox.push(ToViewer::Ready.encode(), false);
        if let Some(colors) = &self.colors {
            outbox.push(ToViewer::Colors(colors.clone()).encode(), false);
        }
        let writer = spawn_writer(stream, Arc::clone(&outbox));
        self.consent.attach(conn, Arc::clone(&outbox));
        self.viewer = Some(Viewer {
            conn,
            outbox,
            writer,
        });
        self.vt.accept_clipboard_writes(true);
        self.state.viewer = Some(attach.pid);
        self.state.group = attach.group;
        self.state.order = attach.order;
        self.state.alone_since_s = None;
        self.state.front = false;
        self.state.seen = false;
        self.write_state();
        if self.ended {
            // It has been shown how it ended: that was what it waited for.
            let code = self.exit_code.unwrap_or(0);
            if let Some(v) = &self.viewer {
                v.outbox.end_with(ToViewer::Exited(Exit { code }).encode());
            }
            self.leaving = true;
        }
    }

    /// The window goes — with `last` as its last frame, when it is told why.
    fn let_viewer_go(&mut self, last: Option<Vec<u8>>) {
        let Some(v) = self.viewer.take() else { return };
        match last {
            Some(frame) if frame.get(4) == Some(&(wire::Kind::Lagging as u8)) => {
                v.outbox.cut_with(frame);
            }
            Some(frame) => v.outbox.end_with(frame),
            None => v.outbox.close(),
        }
        self.consent.leave(v.conn);
        self.alone();
    }

    /// No window shows it any more: the clipboard closes to the program, and
    /// the state says it is in the background, since now.
    fn alone(&mut self) {
        self.vt.accept_clipboard_writes(false);
        self.state.viewer = None;
        self.state.front = false;
        self.state.seen = false;
        self.state.alone_since_s = Some(now_s());
        self.write_state();
    }

    fn program_ended(&mut self) {
        if self.ended {
            return;
        }
        self.ended = true;
        self.drain_until = None;
        let code = self.exit_code.unwrap_or(0);
        if self.statuses.ended() {
            self.statuses_changed();
        }
        if let Some(c) = &mut self.state.command {
            c.running = false;
        }
        self.state.ended = Some(Ended {
            code,
            at_s: now_s(),
        });
        self.write_state();
        if let Some(v) = &self.viewer {
            v.outbox.end_with(ToViewer::Exited(Exit { code }).encode());
            self.leaving = true;
        }
    }

    /// Writes the whole state now: the file, the window, the watchers.
    fn write_state(&mut self) {
        self.dirty = false;
        self.write_due = None;
        self.last_write = Some(Instant::now());
        let Ok(line) = self.state.line() else { return };
        if let Some(r) = &self.registry
            && let Err(e) = r.write(&self.state)
        {
            tracing::warn!(error = %e, id = %self.id, "could not write the terminal's state");
        }
        if let Some(v) = &self.viewer {
            v.outbox.push(
                ToViewer::State(Box::new(self.state.clone())).encode(),
                false,
            );
        }
        for w in &self.watchers {
            w.outbox.push(line.clone(), false);
        }
    }

    fn finish(mut self, outcome: Outcome) -> Outcome {
        if let Some(stop) = &self.stop_accept {
            stop.store(true, Ordering::Release);
            // The accept thread is in `accept`: a knock lets it see the flag.
            if let Some(r) = &self.registry {
                let _ = UnixStream::connect(r.socket(&self.id));
            }
        }
        if let Some(r) = &self.registry {
            r.forget(&self.id);
        }
        if let Some(v) = self.viewer.take() {
            if outcome == Outcome::HungUp {
                // Hung up: nothing is owed, and a window that does not read
                // must not keep a stopping terminal for its write's timeout.
                v.outbox.kill();
            } else {
                // Ended: its `Exited` is written before this process goes; a
                // window that does not read is given up by the write's own
                // timeout.
                v.outbox.close();
                if let Some(writer) = v.writer {
                    let _ = writer.join();
                }
            }
        }
        for w in &self.watchers {
            w.outbox.close();
        }
        self.credit.close();
        match (outcome, self.ending) {
            // The master closes when this process exits, and the kernel hangs
            // the program up then; dropping the `Pty` would `SIGKILL` it.
            (Outcome::HungUp, Ending::HangUp) => std::mem::forget(self.pty.take()),
            _ => drop(self.pty.take()),
        }
        outcome
    }
}

/// Answers an order done — a few bytes into an empty socket, which does not
/// block.
fn done(stream: &UnixStream) {
    let mut stream = stream;
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let _ = stream.write_all(wire::DONE_LINE);
}

/// The engine's key event for the window's.
fn key_input(key: &wire::KeyEvent) -> KeyInput {
    KeyInput {
        key: ghostty::Key::from_w3c(&key.key).unwrap_or(ghostty::Key::Unidentified),
        action: match key.action {
            wire::KeyAction::Press => ghostty::KeyAction::Press,
            wire::KeyAction::Repeat => ghostty::KeyAction::Repeat,
            wire::KeyAction::Release => ghostty::KeyAction::Release,
        },
        mods: mods(key.mods),
        consumed: mods(key.consumed),
        text: key.utf8.clone(),
        unshifted: key.unshifted,
        composing: key.composing,
    }
}

fn mods(m: wire::Mods) -> Mods {
    [
        (m.shift, Mods::SHIFT),
        (m.ctrl, Mods::CTRL),
        (m.alt, Mods::ALT),
        (m.super_key, Mods::SUPER),
        (m.caps_lock, Mods::CAPS_LOCK),
        (m.num_lock, Mods::NUM_LOCK),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .fold(Mods::NONE, |all, (_, bit)| all | bit)
}

fn mouse_input(m: &wire::MouseEvent) -> MouseInput {
    MouseInput {
        action: match m.action {
            wire::MouseAction::Press => ghostty::MouseAction::Press,
            wire::MouseAction::Release => ghostty::MouseAction::Release,
            wire::MouseAction::Motion => ghostty::MouseAction::Motion,
        },
        button: m.button.map(|b| match b {
            wire::Button::Left => ghostty::Button::Left,
            wire::Button::Middle => ghostty::Button::Middle,
            wire::Button::Right => ghostty::Button::Right,
            wire::Button::WheelUp => ghostty::Button::WheelUp,
            wire::Button::WheelDown => ghostty::Button::WheelDown,
            wire::Button::WheelLeft => ghostty::Button::WheelLeft,
            wire::Button::WheelRight => ghostty::Button::WheelRight,
            wire::Button::Back => ghostty::Button::Back,
            wire::Button::Forward => ghostty::Button::Forward,
        }),
        mods: mods(m.mods),
        x_px: m.x_px,
        y_px: m.y_px,
        any_button_held: m.any_button_held,
    }
}

/// The grid as the mouse encoder sees it: the window sends positions from
/// the grid's corner, so there is no padding here.
fn geometry(size: Size) -> Geometry {
    let (w, h) = (
        u32::from(size.cell_width_px.max(1)),
        u32::from(size.cell_height_px.max(1)),
    );
    Geometry {
        width_px: u32::from(size.columns) * w,
        height_px: u32::from(size.rows) * h,
        cell_width_px: w,
        cell_height_px: h,
        padding_top_px: 0,
        padding_bottom_px: 0,
        padding_left_px: 0,
        padding_right_px: 0,
    }
}

/// The engine's colours for the window's, or `None` for a palette that is
/// not 256.
fn engine_colors(c: &wire::Colors) -> Option<Colors> {
    let palette = c.palette_of_256()?;
    let rgb = |w: wire::Rgb| Rgb {
        r: w.0[0],
        g: w.0[1],
        b: w.0[2],
    };
    Some(Colors {
        foreground: rgb(c.foreground),
        background: rgb(c.background),
        cursor: c.cursor.map(rgb),
        palette: Box::new(palette.map(rgb)),
        scheme: match c.scheme {
            wire::Scheme::Light => Scheme::Light,
            wire::Scheme::Dark => Scheme::Dark,
        },
    })
}

/// The longest folder a shell's report is taken for: the kernel's own
/// limit on a path (`PATH_MAX`), its escapes each three bytes. Longer is no
/// folder a program could be started in, and the state the footer reads on
/// every change does not carry it.
const FOLDER_MAX_BYTES: usize = 3 * 4096;

/// The folder a shell reported: a plain path, or an OSC 7 `file://` URI of
/// THIS machine (one from across an ssh is the other machine's folder, and
/// opening a terminal there here would be wrong).
fn folder_of(said: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt as _;
    if said.len() > FOLDER_MAX_BYTES {
        return None;
    }
    if said.starts_with('/') {
        return Some(PathBuf::from(said));
    }
    let rest = said.strip_prefix("file://")?;
    let (host, path) = rest.split_at(rest.find('/')?);
    if !(host.is_empty() || host == "localhost" || host == this_host()) {
        return None;
    }
    let mut bytes = Vec::with_capacity(path.len());
    let mut raw = path.bytes();
    while let Some(b) = raw.next() {
        if b == b'%' {
            let hex = [raw.next()?, raw.next()?];
            bytes.push(u8::from_str_radix(std::str::from_utf8(&hex).ok()?, 16).ok()?);
        } else {
            bytes.push(b);
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
}

/// This machine's name, as the kernel has it.
fn this_host() -> &'static str {
    static HOST: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    HOST.get_or_init(|| {
        std::fs::read_to_string("/proc/sys/kernel/hostname")
            .map(|h| h.trim().to_owned())
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ghostty::{Decoder, Mirror};
    use ronoctua_terminals::wire::{KeyAction, KeyEvent, Paste};

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
            std::env::temp_dir().join(format!("ronoctua-holder-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The program the tests run: a prompt, and a few words it obeys.
    const PROGRAM: &str = r#"#!/bin/sh
printf 'ready>\n'
while IFS= read -r line; do
  case "$line" in
    exit*) exit "${line#exit }" ;;
    size) stty size ;;
    lines)
      i=0
      while [ "$i" -lt 60 ]; do
        printf 'line-%02d-abcdefghijklmnopqrstuvwxyz-abcdefghijklmnopqrstuvwxyz\n' "$i"
        i=$((i + 1))
      done
      printf 'lines-done\n' ;;
    edge) stty -echo; printf 'quiet\n'; IFS= read -r x; printf '\033[999C@\n'; stty echo ;;
    hold) printf '\033[?2026h' ;;
    clip) stty -echo; printf '\033]52;c;?\007'; IFS= read -r x; printf '%s\n' "$x" | tr -d '\033\007'; stty echo ;;
    *) printf 'got:%s\n' "$line" ;;
  esac
done
"#;

    const BORN: Size = Size {
        columns: 70,
        rows: 12,
        cell_width_px: 10,
        cell_height_px: 20,
    };

    fn setup(tag: &str, program: &str, registry: Option<Registry>) -> Setup {
        let dir = scratch(tag);
        let path = dir.join("program");
        write_executable(&path, program);
        Setup {
            id: TerminalId::fresh().unwrap(),
            group: GroupId::fresh().unwrap(),
            order: 0,
            size: BORN,
            cwd: Some(dir),
            program: path.to_string_lossy().into_owned(),
            args: Vec::new(),
            version: "9.9".to_owned(),
            registry,
            born: ronoctua_terminals::spawn::Born::default(),
        }
    }

    fn colors() -> wire::Colors {
        let c = Colors::of_a_test();
        let rgb = |c: Rgb| wire::Rgb([c.r, c.g, c.b]);
        wire::Colors {
            foreground: rgb(c.foreground),
            background: rgb(c.background),
            cursor: c.cursor.map(rgb),
            palette: c.palette.iter().copied().map(rgb).collect(),
            scheme: wire::Scheme::Dark,
        }
    }

    /// A window, as the tests drive one: it applies every frame as a real
    /// one must — resize when told, at that point — and keeps what it saw.
    struct Window {
        stream: UnixStream,
        reader: BufReader<UnixStream>,
        mirror: Option<Mirror>,
        snapshot: Vec<u8>,
        state: Option<TerminalState>,
        exited: Option<i32>,
        taken: bool,
        lagging: bool,
        resized: Vec<Size>,
        modes: Vec<ModeChange>,
        consent: Option<ConsentAsk>,
        colored: bool,
        ended: bool,
        refused: Vec<wire::PasteRefusal>,
    }

    impl Window {
        fn attach(stream: UnixStream, size: Size) -> Self {
            let mut stream = stream;
            stream
                .set_read_timeout(Some(Duration::from_secs(20)))
                .unwrap();
            let ask = Ask::Attach(Attach {
                protocol: PROTOCOL,
                engine: ghostty::ENGINE.to_owned(),
                pid: std::process::id(),
                size,
                colors: colors(),
                group: GroupId::fresh().unwrap(),
                order: 0,
            });
            stream.write_all(&ask.line().unwrap()).unwrap();
            let reader = BufReader::new(stream.try_clone().unwrap());
            let mut w = Self {
                stream,
                reader,
                mirror: None,
                snapshot: Vec::new(),
                state: None,
                exited: None,
                taken: false,
                lagging: false,
                resized: Vec::new(),
                modes: Vec::new(),
                consent: None,
                colored: false,
                ended: false,
                refused: Vec::new(),
            };
            // The snapshot, and the colours the holder gives it at once: a
            // mirror is compared whole, palette included.
            w.until(|w| w.mirror.is_some() && w.colored);
            w
        }

        /// Reads one frame and applies it; `false` at the end of the stream.
        fn read(&mut self) -> bool {
            let Some((kind, payload)) =
                wire::read_frame(&mut self.reader).expect("a frame in time")
            else {
                self.ended = true;
                return false;
            };
            match ToViewer::decode(kind, payload).unwrap() {
                ToViewer::Attached(a) => assert_eq!(a.mode, AttachMode::Exact),
                ToViewer::Snapshot(bytes) => self.snapshot.extend(bytes),
                ToViewer::Ready => {
                    self.mirror =
                        Some(Decoder::decode(std::mem::take(&mut self.snapshot)).unwrap());
                }
                ToViewer::Bytes(bytes) => self.mirror.as_mut().unwrap().feed(&bytes),
                ToViewer::Resize(s) => {
                    self.mirror
                        .as_mut()
                        .unwrap()
                        .resize(
                            s.columns,
                            s.rows,
                            u32::from(s.cell_width_px),
                            u32::from(s.cell_height_px),
                        )
                        .unwrap();
                    self.resized.push(s);
                }
                ToViewer::Mode(m) => {
                    self.mirror
                        .as_mut()
                        .unwrap()
                        .set_mode(Mode::dec(m.number), m.on)
                        .unwrap();
                    self.modes.push(m);
                }
                ToViewer::Colors(c) => {
                    self.mirror
                        .as_mut()
                        .unwrap()
                        .set_colors(&engine_colors(&c).unwrap())
                        .unwrap();
                    self.colored = true;
                }
                ToViewer::State(s) => self.state = Some(*s),
                ToViewer::Consent(c) => self.consent = Some(c),
                ToViewer::Clipboard(_) => {}
                ToViewer::Taken => self.taken = true,
                ToViewer::Lagging => self.lagging = true,
                ToViewer::Exited(e) => self.exited = Some(e.code),
                ToViewer::PasteRefused(why) => self.refused.push(why),
            }
            true
        }

        /// Reads frames until `done` holds (a read waits at most 20 s).
        fn until(&mut self, done: impl Fn(&Self) -> bool) {
            while !done(self) {
                assert!(self.read(), "the stream ended first");
            }
        }

        fn text(&self) -> String {
            String::from_utf8_lossy(&self.mirror.as_ref().unwrap().format(Format::Plain).unwrap())
                .into_owned()
        }

        fn shows(&self, what: &str) -> bool {
            self.text().contains(what)
        }

        fn send(&mut self, frame: &ToHolder) {
            self.stream.write_all(&frame.encode()).unwrap();
        }

        /// Types a line: text, as an input method would commit it, and Enter
        /// as a key.
        fn line(&mut self, text: &str) {
            self.send(&ToHolder::Text(text.to_owned()));
            self.send(&ToHolder::Key(KeyEvent {
                code: 28,
                key: "Enter".to_owned(),
                action: KeyAction::Press,
                mods: wire::Mods::default(),
                consumed: wire::Mods::default(),
                unshifted: None,
                utf8: String::new(),
                composing: false,
            }));
        }

        /// The VT the mirror writes out, its cursor and title.
        fn picture(&self) -> (Vec<u8>, ghostty::Cursor, String) {
            let m = self.mirror.as_ref().unwrap();
            (m.format(Format::Vt).unwrap(), m.cursor(), m.title())
        }
    }

    #[test]
    fn a_terminal_is_born_with_the_persons_history_and_cursor() {
        // FAILS IF what the person chose does not reach the engine before
        // the program writes: the history's limit (Ghostty's
        // `scrollback-limit`) left at the engine's own, or the cursor the
        // window first draws — the snapshot's — the engine's block instead of
        // the person's bar.
        use ronoctua_terminals::spawn::{Born, CursorShape};
        let born = Born {
            scrollback_bytes: Some(2_000_000),
            cursor: Some((CursorShape::Bar, true)),
        };
        let config = config_of("9.9", born);
        assert_eq!(config.scrollback_bytes, Some(2_000_000));
        assert_eq!(
            config.cursor,
            Some(ghostty::CursorDefault {
                shape: ghostty::CursorShape::Bar,
                blinks: true,
            })
        );
        assert_eq!(
            config_of("9.9", Born::default()),
            Config::for_version("9.9")
        );
        let holder = Holder::start(Setup {
            born,
            ..setup("born", PROGRAM, None)
        })
        .unwrap();
        let w = Window::attach(holder.connect().unwrap(), BORN);
        let state = ghostty::RenderState::new().unwrap();
        state.update(w.mirror.as_ref().unwrap()).unwrap();
        assert_eq!(state.frame().cursor().shape, ghostty::CursorShape::Bar);
        holder.hang_up();
        assert_eq!(holder.wait(), Outcome::HungUp);
    }

    #[test]
    fn a_paste_the_engine_refuses_is_said_to_its_window() {
        // FAILS IF a paste that could run a command, sent without the
        // person's yes, is written to the program — or is dropped with only
        // a line in the holder's log, and the window that sent it never
        // learns why nothing landed. One sent with the yes lands, and says
        // nothing.
        let holder = Holder::start(setup("refused", PROGRAM, None)).unwrap();
        let mut w = Window::attach(holder.connect().unwrap(), BORN);
        w.until(|w| w.shows("ready>"));
        w.send(&ToHolder::Paste(Paste {
            text: "unasked\n".to_owned(),
            source: wire::PasteSource::Clipboard,
            allow_unsafe: false,
        }));
        w.until(|w| !w.refused.is_empty());
        assert_eq!(w.refused, [wire::PasteRefusal::Unsafe]);
        w.send(&ToHolder::Paste(Paste {
            text: "asked\n".to_owned(),
            source: wire::PasteSource::Clipboard,
            allow_unsafe: true,
        }));
        w.until(|w| w.shows("got:asked"));
        assert!(!w.shows("unasked"), "the refused paste reached the program");
        assert_eq!(w.refused.len(), 1, "a paste that landed was said refused");
        holder.hang_up();
        assert_eq!(holder.wait(), Outcome::HungUp);
    }

    #[test]
    fn a_window_that_moves_a_place_renumbers_its_holder() {
        // FAILS IF a `Place` with an order leaves the holder's state at the
        // place its attach gave it — a window that renumbered its group
        // would leave the holders saying the old numbers, and the window
        // opened on them after it is gone would lay them out wrong — or one
        // with no order (a window of the first protocol) moves it.
        let registry = Registry::at(scratch("renumbered").join("terms"));
        let holder = Holder::start(setup("renumber", PROGRAM, Some(registry))).unwrap();
        let mut w = Window::attach(holder.connect().unwrap(), BORN);
        w.until(|w| w.shows("ready>"));
        w.send(&ToHolder::Place(wire::Place {
            front: true,
            seen: false,
            order: Some(5 << 16),
        }));
        w.until(|w| w.state.as_ref().is_some_and(|s| s.order == 5 << 16));
        w.send(&ToHolder::Place(wire::Place {
            front: false,
            seen: false,
            order: None,
        }));
        w.until(|w| w.state.as_ref().is_some_and(|s| !s.front));
        assert_eq!(w.state.as_ref().map(|s| s.order), Some(5 << 16));
        holder.hang_up();
        assert_eq!(holder.wait(), Outcome::HungUp);
    }

    #[test]
    fn the_history_is_compressed_once_quiet_and_the_clock_stops_when_done() {
        // FAILS IF the holder never compresses a terminal's history (it
        // holds it uncompressed for as long as the terminal lives), starts
        // before the history has been quiet for its wait — every burst of
        // output would pay for compressing what it is about to write past —
        // keeps its clock armed once the engine says nothing remains (a
        // terminal at rest would wake for ever), or arms it again with
        // nothing moved. Counted on planted instants: nothing here waits.
        let mut vt = Vt::new(80, 24, &Config::for_version("9.9")).unwrap();
        for i in 0..20_000 {
            vt.feed(format!("line {i} of a history worth compressing\r\n").as_bytes());
        }
        let t0 = Instant::now();
        let mut squeeze = Squeeze::default();
        squeeze.heard(vt.compression_activity(), t0);
        assert_eq!(squeeze.due, Some(t0 + COMPRESS_IDLE), "the wait starts");
        let before = squeeze;
        squeeze.run(&mut vt, t0 + COMPRESS_IDLE / 2);
        assert_eq!(squeeze, before, "a step ran before the history was quiet");
        squeeze.heard(vt.compression_activity(), t0 + COMPRESS_IDLE / 2);
        assert_eq!(squeeze, before, "the wait restarted with nothing moved");
        vt.feed(b"one more line\r\n");
        let t1 = t0 + COMPRESS_IDLE / 2;
        squeeze.heard(vt.compression_activity(), t1);
        assert_eq!(squeeze.due, Some(t1 + COMPRESS_IDLE), "output restarts it");
        let mut at = t1 + COMPRESS_IDLE;
        let mut steps = 0u32;
        while squeeze.due.is_some() {
            squeeze.run(&mut vt, at);
            steps += 1;
            assert!(steps < 1_000_000, "the compression never said it was done");
            // THE LOOP'S OWN TURN after a step: nothing moved but the step.
            squeeze.heard(vt.compression_activity(), at);
            at += Duration::from_millis(1);
        }
        assert!(steps > 1, "the whole history in one step is not idle work");
        squeeze.heard(vt.compression_activity(), at + COMPRESS_IDLE);
        assert_eq!(
            squeeze.due, None,
            "a terminal at rest armed its clock again"
        );
    }

    #[test]
    fn a_window_attaches_and_its_mirror_is_the_holder() {
        // FAILS IF a window's mirror differs from the holder's engine after
        // the snapshot or after the bytes that follow it, input does not
        // reach the program (text, a key, a paste), or a second window does
        // not take the terminal from the first.
        let holder = Holder::start(setup("mirror", PROGRAM, None)).unwrap();
        let mut a = Window::attach(holder.connect().unwrap(), BORN);
        a.until(|w| w.shows("ready>"));
        a.line("hi");
        a.send(&ToHolder::Paste(Paste {
            text: "pasted\n".to_owned(),
            source: wire::PasteSource::Clipboard,
            allow_unsafe: true,
        }));
        a.until(|w| w.shows("got:hi") && w.shows("got:pasted"));

        let mut b = Window::attach(holder.connect().unwrap(), BORN);
        a.until(|w| w.taken);
        assert_eq!(a.picture(), b.picture(), "after the snapshot");

        // The newest window drives now, and is followed by the same bytes.
        b.until(|w| w.state.as_ref().is_some_and(|s| s.viewer.is_some()));
        b.line("more");
        b.until(|w| w.shows("got:more"));
        let c = Window::attach(holder.connect().unwrap(), BORN);
        b.until(|w| w.taken);
        assert_eq!(b.picture(), c.picture(), "after more output");
        holder.hang_up();
        assert_eq!(holder.wait(), Outcome::HungUp);
    }

    #[test]
    fn a_resize_lands_at_the_same_byte() {
        // FAILS IF the holder resizes its engine at one byte and tells the
        // window at another, or the PTY is not told the size with the engine.
        // What the program prints right after the resize is written at the
        // right EDGE — where it lands depends on the width it is printed at,
        // and no reflow afterwards makes the two agree.
        let holder = Holder::start(setup("resize", PROGRAM, None)).unwrap();
        let mut a = Window::attach(holder.connect().unwrap(), BORN);
        a.until(|w| w.shows("ready>"));
        a.line("lines");
        a.until(|w| w.shows("lines-done"));
        // The program waits, saying nothing; the resize is asked; then the
        // line it waits for, and it prints at the edge.
        a.line("edge");
        a.until(|w| w.shows("quiet"));
        let smaller = Size {
            columns: 33,
            rows: 9,
            ..BORN
        };
        a.send(&ToHolder::Resize(smaller));
        a.line("");
        a.line("size");
        a.until(|w| w.shows("@") && w.shows("9 33"));
        assert_eq!(a.resized, [smaller]);
        assert_eq!(a.mirror.as_ref().unwrap().size(), (33, 9));
        let b = Window::attach(holder.connect().unwrap(), smaller);
        a.until(|w| w.taken);
        assert_eq!(a.picture(), b.picture());
        holder.hang_up();
        let _ = holder.wait();
    }

    #[test]
    fn an_exit_is_said_to_the_window_and_written_in_the_state() {
        // FAILS IF the program's end is not told to the window that shows
        // it, its code is lost, or the holder outlives a window having seen
        // it end (its files would stay in the registry for ever).
        let registry = Registry::at(scratch("exit-registry").join("terms"));
        let s = setup("exit", PROGRAM, Some(registry.clone()));
        let id = s.id.clone();
        let holder = Holder::start(s).unwrap();
        assert!(registry.state_file(&id).exists() && registry.socket(&id).exists());
        let mut a = Window::attach(UnixStream::connect(registry.socket(&id)).unwrap(), BORN);
        a.until(|w| w.shows("ready>"));
        a.line("exit 7");
        a.until(|w| w.exited.is_some());
        assert_eq!(a.exited, Some(7));
        let state = a.state.clone().unwrap();
        assert_eq!(state.ended.map(|e| e.code), Some(7));
        assert_eq!(holder.wait(), Outcome::Ended { code: 7 });
        assert!(!registry.state_file(&id).exists() && !registry.socket(&id).exists());
        assert!(!a.read(), "and nothing follows it");
    }

    #[test]
    fn an_ending_with_no_window_waits_to_be_seen_once() {
        // FAILS IF a terminal whose program ended in the background vanishes
        // before anyone saw how it ended, or stays after it was seen.
        let registry = Registry::at(scratch("linger-registry").join("terms"));
        let s = setup(
            "linger",
            "#!/bin/sh\necho bye-from-the-program\nexit 3\n",
            Some(registry.clone()),
        );
        let id = s.id.clone();
        let holder = Holder::start(s).unwrap();
        let start = Instant::now();
        while registry.read(&id).and_then(|s| s.ended).is_none() {
            assert!(start.elapsed() < Duration::from_secs(20), "it never ended");
            std::thread::sleep(Duration::from_millis(10));
        }
        let census = registry.census();
        assert_eq!(census.ended().count(), 1, "listed as ended");
        assert_eq!(census.away(), 0, "and not as running");
        let mut a = Window::attach(UnixStream::connect(registry.socket(&id)).unwrap(), BORN);
        a.until(|w| w.exited.is_some());
        assert!(a.shows("bye-from-the-program"), "{}", a.text());
        assert_eq!(a.exited, Some(3));
        assert_eq!(holder.wait(), Outcome::Ended { code: 3 });
        assert!(!registry.state_file(&id).exists());
    }

    /// The terminal's state, asked of the holder.
    fn state_of(holder: &Holder) -> TerminalState {
        let mut s = holder.connect().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        s.write_all(&Ask::State.line().unwrap()).unwrap();
        let line = wire::read_line(&mut BufReader::new(s)).unwrap().unwrap();
        serde_json::from_slice(&line).unwrap()
    }

    #[test]
    fn a_window_that_does_not_read_is_let_go_and_the_program_goes_on() {
        // FAILS IF a window that stops reading makes the program wait (it
        // would never finish its output), keeps the terminal, or is not told
        // why it was let go.
        let program = "#!/bin/sh\nyes 0123456789abcdefghijklmnopqrstuvwxyz | head -c 1500000\nprintf '\\033]2;all-written\\007'\nexec sleep 300\n";
        let holder = Holder::start(setup("lag", program, None)).unwrap();
        // Attached, and then never read while the program writes.
        let mut a = Window::attach(holder.connect().unwrap(), BORN);
        let start = Instant::now();
        let state = loop {
            let state = state_of(&holder);
            if state.title == "all-written" {
                break state;
            }
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "the program waited for a window"
            );
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(
            state.viewer, None,
            "the window that did not read was let go"
        );
        a.until(|w| w.lagging || w.taken || w.ended);
        assert!(a.lagging, "and told it fell behind");
        holder.hang_up();
        let _ = holder.wait();
    }

    #[test]
    fn a_hung_up_terminal_ends_its_windows_stream() {
        // FAILS IF the writer of a window is left asleep when the terminal is
        // hung up: its stream stays open, and the window waits for frames
        // from a terminal that no longer exists.
        let holder = Holder::start(setup("hang-up", PROGRAM, None)).unwrap();
        let mut a = Window::attach(holder.connect().unwrap(), BORN);
        a.until(|w| w.shows("ready>"));
        holder.hang_up();
        assert_eq!(holder.wait(), Outcome::HungUp);
        while a.read() {}
        assert!(a.ended, "the stream ended");
    }

    #[test]
    fn a_window_that_cannot_be_written_to_no_longer_shows_the_terminal() {
        // FAILS IF a window whose writes fail goes on counting as the one
        // that shows the terminal: no census would offer it, and the program
        // would keep a window nobody can reach.
        let holder = Holder::start(setup("unwritable", PROGRAM, None)).unwrap();
        let mut a = Window::attach(holder.connect().unwrap(), BORN);
        a.until(|w| w.shows("ready>"));
        a.until(|w| w.state.as_ref().is_some_and(|s| s.viewer.is_some()));
        // It reads nothing more: what the holder writes to it now fails.
        a.stream.shutdown(Shutdown::Read).unwrap();
        a.line("hi");
        let start = Instant::now();
        while state_of(&holder).viewer.is_some() {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "still shown by a window nothing reaches"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(state_of(&holder).alone_since_s.is_some());
        holder.hang_up();
        let _ = holder.wait();
    }

    #[test]
    fn input_a_program_never_reads_is_bounded() {
        // FAILS IF what waits for a program that reads nothing grows without
        // end — the answers to the questions it keeps asking are its input —
        // or a single large paste is refused while nothing waits.
        // Nothing is ever written: the receiving end is kept, and not read.
        let (to_pty, _unread) = mpsc::channel();
        let typing = || Typing {
            to_pty: to_pty.clone(),
            queued: Arc::new(AtomicUsize::new(0)),
        };
        let one = typing();
        assert!(
            one.send(vec![b'x'; INPUT_QUEUE_BYTES * 2]),
            "one paste goes whole"
        );
        assert!(
            !one.send(b"\x1b[?62;22c".to_vec()),
            "and nothing more while it waits"
        );
        let many = typing();
        let sent: usize = (0..100_000)
            .filter(|_| many.send(vec![0u8; 1024]))
            .map(|_| 1024)
            .sum();
        assert!(sent <= INPUT_QUEUE_BYTES + 1024, "{sent} bytes queued");
    }

    #[test]
    fn a_hold_the_program_never_ends_is_ended_and_framed() {
        // FAILS IF a program that begins synchronized output and never ends
        // it freezes the screen for ever, or the holder ends the hold without
        // telling the window where.
        let holder = Holder::start(setup("hold", PROGRAM, None)).unwrap();
        let mut a = Window::attach(holder.connect().unwrap(), BORN);
        a.until(|w| w.shows("ready>"));
        a.line("hold");
        a.until(|w| !w.modes.is_empty());
        assert_eq!(
            a.modes,
            [ModeChange {
                number: 2026,
                ansi: false,
                on: false
            }]
        );
        assert_eq!(
            a.mirror.as_ref().unwrap().mode(Mode::SYNC_OUTPUT),
            Some(false)
        );
        holder.hang_up();
        let _ = holder.wait();
    }

    #[test]
    fn a_clipboard_read_is_asked_of_the_window() {
        // FAILS IF a program reads the clipboard without the window asking
        // the person, or the person's answer does not reach it.
        let holder = Holder::start(setup("clip", PROGRAM, None)).unwrap();
        let mut a = Window::attach(holder.connect().unwrap(), BORN);
        a.until(|w| w.shows("ready>"));
        a.line("clip");
        a.until(|w| w.consent.is_some());
        let ask = a.consent.clone().unwrap();
        assert_eq!(ask.clipboard, ClipboardKind::Standard);
        a.send(&ToHolder::Answer(wire::Answer {
            id: ask.id,
            text: Some("secret".to_owned()),
        }));
        a.line("");
        // "secret" in base64.
        a.until(|w| w.shows("c2VjcmV0"));
        holder.hang_up();
        let _ = holder.wait();
    }

    #[test]
    fn a_key_by_its_w3c_name_is_the_engines_key() {
        // FAILS IF a key name the window sends maps to another key, or the
        // table misses one of the library's keys.
        assert_eq!(ghostty::Key::from_w3c("KeyA"), Some(ghostty::Key::A));
        assert_eq!(
            ghostty::Key::from_w3c("ArrowUp"),
            Some(ghostty::Key::ArrowUp)
        );
        assert_eq!(ghostty::Key::from_w3c("NotAKey"), None);
        assert_eq!(ghostty::Key::Enter.w3c(), "Enter");
        let mods = mods(wire::Mods {
            ctrl: true,
            shift: true,
            ..wire::Mods::default()
        });
        assert!(
            mods.contains(Mods::CTRL) && mods.contains(Mods::SHIFT) && !mods.contains(Mods::ALT)
        );
    }

    #[test]
    fn a_folder_is_taken_from_this_machine_only() {
        // FAILS IF the folder a shell reports over ssh — another machine's —
        // becomes where a new terminal here starts, or an escaped path is
        // not unescaped.
        assert_eq!(
            folder_of("file:///home/a%20b"),
            Some(PathBuf::from("/home/a b"))
        );
        assert_eq!(
            folder_of("file://localhost/tmp"),
            Some(PathBuf::from("/tmp"))
        );
        assert_eq!(folder_of("/srv"), Some(PathBuf::from("/srv")));
        assert_eq!(folder_of("file://elsewhere.example/home"), None);
        assert_eq!(folder_of("file:///bad%zz"), None);
        // A report no folder could be is not carried into the state.
        assert_eq!(
            folder_of(&format!("/{}", "a".repeat(FOLDER_MAX_BYTES))),
            None
        );
    }
}
