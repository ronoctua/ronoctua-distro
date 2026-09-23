//! Sound for a recording or a stream: capture, mix, and hand to the encoders.
//!
//! # Opened only while something records
//!
//! The microphone is opened when a recording or a stream starts with the
//! microphone on, and closed when the last of them ends — the sound is a
//! value ([`Sound`]) the engine holds exactly that long, and each `pw-cat`
//! inside it is reaped by its guard. The modeline's microphone mark lights for
//! it, as it does for any program: hiding our own use would be the one lie
//! that makes the mark useless.
//!
//! # The clock is the wall's, and silence is written
//!
//! The mixer writes one 10 ms block per 10 ms since it started, taking what
//! each input has buffered and padding the rest with silence. It does not
//! wait on an input: the desktop monitor gives nothing while nothing plays,
//! and a mix that waited for it would stall the microphone — and a file whose
//! sound stops while its pictures go on drifts out of step with them.

use std::{
    fs::File,
    io::{Read as _, Write as _},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::Duration,
};

use ronoctua_audio::pipewire::{CAPTURE_CHANNELS, CAPTURE_RATE, Capture};
use ronoctua_camera::frames::Reaped;

use crate::mix::{self, Knobs};

/// Samples per block: 10 ms, both channels.
const BLOCK: usize = (CAPTURE_RATE as usize / 100) * CAPTURE_CHANNELS as usize;

/// The levels and knobs the engine shows and turns.
#[derive(Debug, Default)]
pub struct Board {
    /// Microphone, desktop.
    pub knobs: [Knobs; 2],
    /// Microphone, desktop, 0–100.
    pub levels: [u8; 2],
}

/// Where the mix goes: every FIFO an encoder reads, opened when it is ready.
type Outs = Arc<Mutex<Vec<(PathBuf, File)>>>;

/// The running sound.
pub struct Sound {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    outs: Outs,
    /// Shared knobs and levels.
    pub board: Arc<Mutex<Board>>,
}

impl Sound {
    /// Adds an encoder's FIFO. Opening one for writing BLOCKS until the
    /// encoder opens it for reading, so it happens on a thread of its own —
    /// the engine never waits, and a recording already running keeps its
    /// sound while a stream joins.
    pub fn attach(&self, path: PathBuf) {
        use std::os::unix::fs::OpenOptionsExt as _;
        let outs = self.outs.clone();
        let stop = self.stop.clone();
        let _ = std::thread::Builder::new()
            .name("studio-fifo".into())
            .spawn(move || {
                // NON-BLOCKING open: it fails (`ENXIO`) until the encoder has the
                // other end, and is retried — so an encoder that died before
                // opening leaves no thread blocked for ever.
                for _ in 0..200 {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    match std::fs::OpenOptions::new()
                        .write(true)
                        .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
                        .open(&path)
                    {
                        Ok(f) => {
                            if let Ok(mut o) = outs.lock() {
                                o.push((path, f));
                            }
                            return;
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(50)),
                    }
                }
                tracing::warn!(fifo = %path.display(), "the encoder never opened its sound");
            });
    }

    /// Removes one.
    pub fn detach(&self, path: &std::path::Path) {
        if let Ok(mut o) = self.outs.lock() {
            o.retain(|(p, _)| p != path);
        }
    }
}

impl Drop for Sound {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// One input: the recorder child and a reader thread buffering its samples.
struct Input {
    _child: Reaped,
    buffer: Arc<Mutex<Vec<i16>>>,
}

fn open(what: Capture, stop: &Arc<AtomicBool>) -> Option<Input> {
    let mut child = ronoctua_audio::pipewire::capture(what).ok()?;
    let mut out = child.stdout.take()?;
    let buffer: Arc<Mutex<Vec<i16>>> = Arc::default();
    let (b, s) = (buffer.clone(), stop.clone());
    std::thread::Builder::new()
        .name("studio-sound-in".into())
        .spawn(move || {
            let mut bytes = vec![0u8; BLOCK * 2];
            let mut samples = Vec::new();
            while !s.load(Ordering::SeqCst) {
                let Ok(n) = out.read(&mut bytes) else { return };
                if n == 0 {
                    return;
                }
                mix::samples(&bytes[..n - n % 2], &mut samples);
                if let Ok(mut buf) = b.lock() {
                    buf.extend_from_slice(&samples);
                    // Never more than a second behind: a stalled writer must
                    // not grow this without end.
                    let most = CAPTURE_RATE as usize * CAPTURE_CHANNELS as usize;
                    if buf.len() > most {
                        let cut = buf.len() - most;
                        buf.drain(..cut);
                    }
                }
            }
        })
        .ok()?;
    Some(Input {
        _child: Reaped(child),
        buffer,
    })
}

/// Starts capturing the chosen inputs; the mix goes to every attached FIFO.
pub fn start(microphone: bool, desktop: bool, knobs: [Knobs; 2]) -> Sound {
    let stop = Arc::new(AtomicBool::new(false));
    let board = Arc::new(Mutex::new(Board {
        knobs,
        levels: [0, 0],
    }));
    let outs: Outs = Arc::default();
    let (s, b, o) = (stop.clone(), board.clone(), outs.clone());
    let thread = std::thread::Builder::new()
        .name("studio-sound".into())
        .spawn(move || {
            let inputs = [
                microphone.then(|| open(Capture::Microphone, &s)).flatten(),
                desktop.then(|| open(Capture::Desktop, &s)).flatten(),
            ];
            let mut mixed = vec![0i16; BLOCK];
            let mut bytes = Vec::new();
            let mut takes: [Vec<i16>; 2] = [Vec::new(), Vec::new()];
            let started = std::time::Instant::now();
            let mut sent: u64 = 0;
            while !s.load(Ordering::SeqCst) {
                // THE CLOCK: one block per 10 ms of wall time since the start,
                // so silence (an input with nothing buffered) still advances
                // the file and sound stays in step with the pictures.
                let due = started.elapsed().as_millis() as u64 / 10;
                if sent >= due {
                    std::thread::sleep(Duration::from_millis(2));
                    continue;
                }
                sent += 1;
                for (i, input) in inputs.iter().enumerate() {
                    takes[i].clear();
                    if let Some(input) = input
                        && let Ok(mut buf) = input.buffer.lock()
                    {
                        let n = buf.len().min(BLOCK);
                        takes[i].extend(buf.drain(..n));
                    }
                }
                let knobs = b.lock().map(|b| b.knobs).unwrap_or_default();
                mix::mix(&mut mixed, &[(&takes[0], knobs[0]), (&takes[1], knobs[1])]);
                if let Ok(mut board) = b.lock() {
                    board.levels = [mix::level(&takes[0]), mix::level(&takes[1])];
                }
                mix::bytes(&mixed, &mut bytes);
                // NEVER A BLOCKING WRITE: a FIFO the encoder is not reading
                // is full, and a block that does not fit is DROPPED — a
                // click in the file, instead of a studio that waits for ever
                // on a pipe while somebody holds its lock.
                if let Ok(mut outs) = o.lock() {
                    outs.retain_mut(|(_, f)| match f.write(&bytes) {
                        Ok(_) => true,
                        Err(e) => e.kind() == std::io::ErrorKind::WouldBlock,
                    });
                }
            }
        })
        .ok();
    Sound {
        stop,
        thread,
        outs,
        board,
    }
}
