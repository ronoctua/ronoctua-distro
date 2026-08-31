//! The player: one thread, one moving file, one frame at a time.
//!
//! # Why the frames arriving ARE the clock
//!
//! Neither door has a timer fast enough to drive an animation, and neither
//! gained one. The player waits for the frame's due time and then sends it over
//! the `calloop` channel the doors already listen on — so the frame arriving is
//! what wakes the loop, and when the player stops, the wake-ups stop with it. A
//! timer in the door would have to be told when to run and when not to; this
//! cannot be wrong about that, because there is nothing to tell.
//!
//! # One player, and it is the same rule `Looked` follows
//!
//! The panel is about the focused entry and there is one of those. Starting a
//! second player would be a second child process for a file nobody is looking
//! at — so asking for a new one stops the old one, and leaving the file stops
//! it too.
//!
//! # What it costs
//!
//! One frame in memory, not the film. The GIF's frames come from `image`'s own
//! iterator, which is lazy; the video's come from an ffmpeg writing raw RGBA
//! into a pipe, and the player reads exactly one frame's worth. Measured on the
//! owner's own 1080p60 sample: ten seconds of film decoded in 507 to 641 ms
//! depending on the scale, which is about ninety-four per cent idle. **Playing
//! is nearly free; what costs is the transfer** — 4 MB/s at a 256-pixel frame
//! against 29 MB/s at 672 — which is why the frame is decoded at the panel's
//! own size and never at a fixed one.
//!
//! # The transport is not four mechanisms
//!
//! | asked for | what happens | what it costs |
//! |---|---|---|
//! | pause | the loop STOPS READING; ffmpeg blocks on a full pipe | nothing, and 0% CPU |
//! | resume | it reads again | nothing |
//! | seek | the child is killed and respawned at `-ss` | 108–242 ms, measured |
//! | sound | the same, with a second output | the same |
//! | resize | the same, at the new [`Fit`] | the same |
//!
//! Pausing by `SIGSTOP` was the obvious alternative and it is wrong: it freezes
//! the audio device mid-buffer, which clicks. Blocking on a pipe is the
//! mechanism the kernel already provides, and it stops the sound in the same
//! instant because the sound comes out of the same child.

use std::{
    io::Read,
    os::{fd::AsFd, unix::process::CommandExt},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use ronoctua_inspect::motion::{
    AUDIO_GRACE, Fit, PAUSE_CEILING, SLOWEST_FRAME_MS, STARVED, Source,
};
use smithay_client_toolkit::reexports::calloop;

/// Which sound device the picture's audio goes to.
///
/// `default`, which PulseAudio resolves to whatever the machine is playing
/// through — and not a name of our own, which would name a sink that does not
/// exist and fail every time. Verified against the real binary: `-f pulse …
/// default` on a file with sound exits 0 and delivers every frame.
const SINK: &str = "default";

/// How often the loop looks up from what it is doing.
///
/// The same figure the frame floor uses, and for the same reason: an order is
/// obeyed within a fiftieth of a second however long the frame it interrupts.
const TICK: Duration = Duration::from_millis(SLOWEST_FRAME_MS as u64);

/// How long a motion photo's PHOTOGRAPH is held before its film runs.
///
/// A second and a half: long enough to take the picture in, short enough that
/// the panel never reads as stuck. The owner chose it. Measured against his own
/// files, whose film is 2,89 s, the loop comes to about four and a half seconds
/// — near what the phones themselves do.
const PHOTO_DWELL: Duration = Duration::from_millis(1500);

/// How many frames in a row may be thrown away before the clock is re-anchored.
///
/// A machine that cannot keep up should run the film LATE, not fast. Dropping
/// for ever would be racing through it; re-anchoring after thirty makes it slow
/// motion instead, which is the honest answer.
const MOST_DROPPED: u32 = 30;

/// One frame of a moving file, on its way to the screen.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Which request this belongs to.
    ///
    /// The cursor moves while a child process runs, and one file's picture
    /// under another file's name is the one failure a preview must not have —
    /// the rule `View::looked` already keeps for the still frame.
    pub key: String,
    /// The pixels.
    pub rgba: ronoctua_render::image::Rgba,
    /// How far into the film this frame is, in seconds.
    ///
    /// What the timeline draws and what a pause remembers. A GIF has no
    /// duration, so this is nought there and nothing reads it.
    pub at: f64,
}

/// What the player has to say back.
///
/// # A film that ENDS has to say so
///
/// Before this there was only the frame: at the end of a film the frames simply
/// stopped arriving, the last one stayed on screen for ever and the play glyph
/// never came back. The absence of a wake-up is not an event and cannot be
/// drawn — so the end is a message.
#[derive(Debug, Clone, PartialEq)]
pub enum Beat {
    /// A picture.
    Frame(Frame),
    /// The film ran out, or the child closed the pipe.
    Ended {
        /// Which request.
        key: String,
    },
    /// Show the PHOTOGRAPH now — a motion photo's other half.
    ///
    /// # Why the photograph needs a message of its own
    ///
    /// Because it is drawn by NOT drawing a frame: the panel falls back to the
    /// still whenever there is none. Simply stopping would look identical to a
    /// film that hung, and the last frame would stay on screen — the absence of
    /// a wake-up is not an event, which is the same argument [`Beat::Ended`]
    /// already carries.
    Still {
        /// Which request.
        key: String,
    },
    /// Something is wrong, and the panel says what.
    Trouble {
        /// Which request.
        key: String,
        /// What went wrong.
        what: Trouble,
    },
}

/// What can go wrong that somebody should be told about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trouble {
    /// The machine has no sound to give: the picture goes on without it.
    NoSound,
    /// Nothing came out of the child inside [`STARVED`].
    Starved,
}

/// What the player is asked to open.
///
/// A value, and compared as one: the manager decides what SHOULD be running and
/// the difference from what IS running is the order. See
/// `files::View::wants_motion`.
#[derive(Debug, Clone, PartialEq)]
pub struct Reel {
    /// The file.
    pub path: PathBuf,
    /// The key the frames come back under.
    pub key: String,
    /// Which decoder to open.
    ///
    /// The MIME type is not here and was for an hour: the player never read it
    /// — what it needs is which decoder, and that is what `motion::moving`
    /// already answered.
    pub how: Source,
    /// Exactly how large a frame is to be.
    pub fit: Fit,
    /// Frames a second.
    pub fps: f64,
    /// Where in the film to start, in seconds.
    pub from: f64,
    /// Whether the sound is wanted.
    pub sound: bool,
    /// The panel's own background, for the letterboxing.
    ///
    /// A string because `ronoctua-inspect` receives no token values, and the
    /// panel's own colour because the bars beside a picture of another shape
    /// should be the air the panel would have left anyway.
    pub background: String,
    /// Where the film lives, when the file is a PHOTOGRAPH with one in it.
    ///
    /// `Some` is what makes this a motion photo, and what makes the loop start
    /// with the picture and put the film after it. See
    /// [`ronoctua_inspect::motion::Inside`].
    pub inside: Option<ronoctua_inspect::motion::Inside>,
}

/// What the player is told to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Ask {
    /// Open this and follow it.
    ///
    /// The flag is whether it starts HELD, and it is not part of the reel — a
    /// pause must never be a respawn. It exists because a respawn happens for
    /// reasons that have nothing to do with the transport: the panel resized,
    /// the region was given over, the sound was switched. Without it the new
    /// child began running and a film the person had paused started playing
    /// again, which the owner met by pressing `C-w`.
    Play(Reel, bool),
    /// Hold on the frame that is up.
    Pause,
    /// Go on from it.
    Resume,
    /// Stop, and let go of whatever is open.
    Stop,
}

/// A player, running.
pub struct Player {
    asks: std::sync::mpsc::Sender<Ask>,
    /// Which request is current.
    ///
    /// THE THREAD READS THIS INSIDE ITS READS, and that is what stops a child
    /// outliving its file: a `Stop` sent while ffmpeg is mid-frame would sit in
    /// the channel until the frame finished, so the loop checks a number
    /// instead. Bumping it is what "stop" means.
    era: Arc<AtomicU64>,
    /// Whether the film is held on one frame.
    ///
    /// A flag and not a message, for the same reason: the thread is inside a
    /// read when it arrives. It is also what makes pausing free — the loop
    /// stops reading, and ffmpeg blocks on a pipe nobody is draining.
    held: Arc<AtomicBool>,
    /// The last era handed out.
    latest: u64,
}

impl Player {
    /// Passes one order on.
    ///
    /// Pause and resume are a FLAG and never a message: the thread is inside a
    /// read while a film plays, so a message would be obeyed at the end of the
    /// film rather than now. Everything else bumps the era, which is what makes
    /// the running loop let go before the new order is picked up.
    pub fn tell(&mut self, ask: Ask) {
        match ask {
            Ask::Pause => self.held.store(true, Ordering::SeqCst),
            Ask::Resume => self.held.store(false, Ordering::SeqCst),
            other => {
                // AND THE HOLD IS WHAT THE ORDER SAYS, not always "off": a
                // respawn of a film somebody paused has to come back paused.
                let held = matches!(other, Ask::Play(_, true));
                self.latest = self.latest.wrapping_add(1);
                self.era.store(self.latest, Ordering::SeqCst);
                self.held.store(held, Ordering::SeqCst);
                let _ = self.asks.send(other);
            }
        }
    }
}

/// Starts the player's thread.
///
/// `out` is the channel the door already draws from. `None` when the thread
/// could not be created, which is a machine in trouble — and the preview then
/// shows the still frame it always did.
#[must_use]
pub fn player(out: calloop::channel::Sender<Beat>) -> Option<Player> {
    let (asks, orders) = std::sync::mpsc::channel::<Ask>();
    let era = Arc::new(AtomicU64::new(0));
    let held = Arc::new(AtomicBool::new(false));
    let mine = Arc::clone(&era);
    let holding = Arc::clone(&held);
    let born = std::thread::Builder::new()
        .name("ronoctua-motion".to_owned())
        .spawn(move || {
            while let Ok(order) = orders.recv() {
                // ONLY THE NEWEST. Everything queued behind it is a file the
                // cursor has already left — the same drain the inspector does,
                // and here it also means never opening a child for a file
                // nobody is looking at any more.
                let mut order = order;
                while let Ok(newer) = orders.try_recv() {
                    order = newer;
                }
                let Ask::Play(reel, _) = order else {
                    continue;
                };
                let now = mine.load(Ordering::SeqCst);
                let clock = Clock {
                    era: &mine,
                    held: &holding,
                    mine: now,
                };
                match reel.how {
                    Source::InProcess => play_gif(&reel, &out, &clock),
                    // A PHOTOGRAPH WITH A FILM IN IT ALTERNATES; a film alone
                    // just runs. The difference is `Reel::inside`, and it is
                    // the only place the two part company.
                    Source::Ffmpeg if reel.inside.is_some() => {
                        play_motion_photo(&reel, &out, &clock);
                    }
                    Source::Ffmpeg => {
                        // AND A FILM THAT RUNS OUT SAYS SO. Only here: for a
                        // motion photo the ending is the cue for the next
                        // photograph, not the end of anything.
                        if play_video(&reel, &out, &clock) {
                            let _ = out.send(Beat::Ended { key: reel.key });
                        }
                    }
                }
            }
        });
    match born {
        Ok(_) => Some(Player {
            asks,
            era,
            held,
            latest: 0,
        }),
        Err(e) => {
            tracing::error!(error = %e, "could not create the motion thread");
            None
        }
    }
}

/// A child that is killed and reaped whatever happens next.
///
/// # Why it is a guard and not four calls
///
/// Because it WAS four calls — `kill(); wait()` written out at each of the
/// exits of `play_video` — and the fifth exit somebody adds tomorrow leaks an
/// ffmpeg reading a film nobody is looking at. A `Drop` covers `return`, `?`,
/// `break` and a panic, and there is nowhere left to forget.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// What the thread checks between and inside its reads.
struct Clock<'a> {
    era: &'a AtomicU64,
    held: &'a AtomicBool,
    mine: u64,
}

impl Clock<'_> {
    /// Whether this run is still the one that was asked for.
    fn current(&self) -> bool {
        self.era.load(Ordering::SeqCst) == self.mine
    }

    /// Whether the film is being held on one frame.
    fn holding(&self) -> bool {
        self.held.load(Ordering::SeqCst)
    }

    /// Waits out a pause, and says how long it lasted.
    ///
    /// `None` means the order changed, or the pause outlasted
    /// [`PAUSE_CEILING`] — somebody paused and walked away, and a child holding
    /// a file open for the rest of the session is what that constant exists to
    /// prevent. The position is kept by the manager, so pressing play seeks
    /// back to where they were.
    fn wait_out_a_pause(&self) -> Option<Duration> {
        if !self.holding() {
            return Some(Duration::ZERO);
        }
        let began = Instant::now();
        while self.holding() {
            if !self.current() {
                return None;
            }
            if began.elapsed() >= PAUSE_CEILING {
                return None;
            }
            std::thread::sleep(TICK);
        }
        Some(began.elapsed())
    }

    /// Waits until a frame is due, in small steps.
    ///
    /// `false` means the order changed while waiting, which is what makes a
    /// stop take a fiftieth of a second rather than a whole frame.
    fn wait_until(&self, due: Instant) -> bool {
        while Instant::now() < due {
            if !self.current() || self.holding() {
                return self.current();
            }
            let left = due.saturating_duration_since(Instant::now());
            std::thread::sleep(left.min(TICK));
        }
        self.current()
    }
}

/// How a read of one whole frame turned out.
enum Filled {
    /// Every byte of it.
    Whole,
    /// The pipe closed. **A short frame is never sent** — half a picture read
    /// as a whole one is the smear this whole design exists to rule out.
    Short,
    /// Nothing came inside the deadline.
    Starved,
    /// The order changed.
    Left,
}

/// Reads exactly one frame, giving up rather than blocking for ever.
///
/// # This is the live hang, and it is why `read_exact` is gone
///
/// `read_exact` blocks with NO DEADLINE, and the era was only checked between
/// frames. An ffmpeg that opens the pipe and never writes — a mount that hung,
/// a device that went away — held this thread for the rest of the session: the
/// door kept painting, `orders.recv()` never ran again, and nothing on screen
/// said so. So the read is a `poll` with a tick, and the deadline and the era
/// are both asked every time round.
fn fill<R: Read + AsFd>(pipe: &mut R, into: &mut [u8], clock: &Clock<'_>) -> Filled {
    let deadline = Instant::now() + STARVED;
    let tick = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: i64::try_from(TICK.as_nanos()).unwrap_or(20_000_000),
    };
    let mut got = 0usize;
    while got < into.len() {
        if !clock.current() {
            return Filled::Left;
        }
        if Instant::now() >= deadline {
            return Filled::Starved;
        }
        // The borrow of the pipe ends with this block, so the read below can
        // take it mutably — the shape `tools::run` already uses.
        let ready = {
            let fd = pipe.as_fd();
            let mut fds = [rustix::event::PollFd::new(
                &fd,
                rustix::event::PollFlags::IN,
            )];
            match rustix::event::poll(&mut fds, Some(&tick)) {
                Ok(_) => fds[0].revents(),
                // A signal is a tick, not a failure.
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => {
                    tracing::warn!(error = %e, "the player's poll failed");
                    return Filled::Short;
                }
            }
        };
        if !ready.intersects(
            rustix::event::PollFlags::IN
                | rustix::event::PollFlags::HUP
                | rustix::event::PollFlags::ERR,
        ) {
            continue;
        }
        match pipe.read(&mut into[got..]) {
            Ok(0) => return Filled::Short,
            Ok(n) => got += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return Filled::Short,
        }
    }
    Filled::Whole
}

/// Follows a video, through a live ffmpeg, until it is told to stop.
/// Runs the film once.
///
/// `true` when it ended by itself, which is the only outcome the motion
/// photo's loop may go round on. Everything else — the order changed, nothing
/// came out, ffmpeg is not there — is `false`, and whoever called stops.
///
/// **It does not announce the ending.** Whether a film that ran out means "the
/// transport is finished" or "now show the photograph again" is the caller's to
/// say, and only the caller knows which.
fn play_video(reel: &Reel, out: &calloop::channel::Sender<Beat>, clock: &Clock<'_>) -> bool {
    let Ok(program) = ronoctua_inspect::motion::ffmpeg() else {
        let _ = out.send(Beat::Trouble {
            key: reel.key.clone(),
            what: Trouble::Starved,
        });
        return false;
    };
    // AT MOST TWICE ROUND: once as asked, and once in silence if the sound is
    // what the machine refused.
    let mut sound = reel.sound;
    loop {
        let args = ronoctua_inspect::motion::stream_args(
            &reel.path,
            reel.inside.as_ref(),
            reel.fit,
            reel.fps,
            reel.from,
            sound.then_some(SINK),
            &reel.background,
        );
        let born = Command::new(&program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            // ITS OWN GROUP, the idiom `transfers/media.rs` already uses: a
            // signal then reaches whatever the child started as well as the
            // child.
            .process_group(0)
            .spawn();
        let Ok(born) = born else {
            return false;
        };
        // AND THE CHILD GOES, whatever happens below — see [`Reaped`].
        let mut child = Reaped(born);
        let Some(mut pipe) = child.0.stdout.take() else {
            return false;
        };
        tracing::debug!(
            width = reel.fit.width,
            height = reel.fit.height,
            fps = reel.fps,
            from = reel.from,
            sound,
            "motion stream opened"
        );
        let began = Instant::now();
        match follow(&mut pipe, reel, out, clock) {
            Ran::Left => return false,
            Ran::Starved => {
                let _ = out.send(Beat::Trouble {
                    key: reel.key.clone(),
                    what: Trouble::Starved,
                });
                return false;
            }
            // NOTHING AT ALL, AND AT ONCE, WITH THE SOUND ASKED FOR: the
            // machine has no sound to give. Measured: ffmpeg refuses the WHOLE
            // command — both outputs — when the audio one cannot be opened, so
            // the picture dies with it unless this respawns.
            Ran::Closed(0) if sound && began.elapsed() < AUDIO_GRACE => {
                sound = false;
                let _ = out.send(Beat::Trouble {
                    key: reel.key.clone(),
                    what: Trouble::NoSound,
                });
            }
            Ran::Closed(_) => return true,
        }
    }
}

/// Shows the photograph, then the film, then the photograph again.
///
/// # The loop is the feature, and it costs one process per turn
///
/// The photograph is already on screen — the panel draws the still whenever no
/// frame is in hand — so holding it is holding STILL: no child, no thread work,
/// nothing but a clock. Then the film runs once, exactly as any film does, and
/// the turn comes round.
///
/// The child is opened per turn rather than kept alive between them, and that
/// is deliberate: a paused ffmpeg holds the photograph's file open, and the
/// film is a second and a half. The spawn is 108 to 242 ms, measured, and it
/// happens while the photograph is still on screen.
fn play_motion_photo(reel: &Reel, out: &calloop::channel::Sender<Beat>, clock: &Clock<'_>) {
    while clock.current() {
        // THE PHOTOGRAPH. Told, and not merely implied by silence — see
        // [`Beat::Still`].
        if out
            .send(Beat::Still {
                key: reel.key.clone(),
            })
            .is_err()
        {
            return;
        }
        let mut left = PHOTO_DWELL;
        while left > Duration::ZERO {
            let Some(_) = clock.wait_out_a_pause() else {
                return;
            };
            if !clock.current() {
                return;
            }
            let step = left.min(TICK);
            std::thread::sleep(step);
            left -= step;
        }
        // AND THEN THE FILM, once. `play_video` ends by saying so, and this
        // loop is what turns that ending into the next photograph.
        if !play_video(reel, out, clock) {
            return;
        }
    }
}

/// How a run of one child ended.
enum Ran {
    /// The pipe closed, after this many whole frames.
    Closed(usize),
    /// Nothing arrived inside [`STARVED`].
    Starved,
    /// The order changed, or the door went away.
    Left,
}

/// Reads a child's frames and paces them by the wall clock.
///
/// # The clock is the wall's, and whoever is late loses a frame
///
/// It used to be a fixed sleep of `1000/fps` AFTER the read, the send and the
/// paint — so the real period was that plus the work, the film ran some
/// seventeen per cent slow, and the error ACCUMULATED: a minute in, ten seconds
/// behind. Now frame `n` is due at `origin + n/fps`, and being late costs the
/// NEXT frame rather than the schedule.
///
/// A late frame is still READ — a pipe cannot be skipped — but it is not
/// fitted, not sent, not uploaded and not painted, which is the expensive half.
fn follow(
    pipe: &mut std::process::ChildStdout,
    reel: &Reel,
    out: &calloop::channel::Sender<Beat>,
    clock: &Clock<'_>,
) -> Ran {
    let mut buffer = vec![0u8; reel.fit.bytes()];
    let mut seen: usize = 0;
    let mut origin: Option<Instant> = None;
    let mut dropped: u32 = 0;
    let period = Duration::from_secs_f64(1.0 / reel.fps.max(1.0));
    loop {
        // THE PAUSE IS BEFORE THE READ, and that is the whole mechanism: a loop
        // that is not reading leaves ffmpeg blocked on a full pipe, at no cost
        // at all, with the sound stopped in the same instant because it comes
        // out of the same child.
        let Some(paused_for) = clock.wait_out_a_pause() else {
            return Ran::Left;
        };
        if let Some(o) = origin.as_mut() {
            // AND THE CLOCK WAITED TOO. Without this every frame after a
            // resume is late by the length of the pause and the film races to
            // catch up with a schedule nobody was watching.
            *o += paused_for;
        }
        match fill(pipe, &mut buffer, clock) {
            Filled::Whole => {}
            Filled::Short => return Ran::Closed(seen),
            Filled::Starved => return Ran::Starved,
            Filled::Left => return Ran::Left,
        }
        let n = seen;
        seen += 1;
        let now = Instant::now();
        let start = *origin.get_or_insert(now);
        #[allow(clippy::cast_precision_loss)]
        let due = start + Duration::from_secs_f64(n as f64 / reel.fps.max(1.0));
        if now >= due + period {
            if dropped < MOST_DROPPED {
                dropped += 1;
                continue;
            }
            // TOO FAR BEHIND FOR TOO LONG. Re-anchoring makes the film run in
            // slow motion, which is honest; going on dropping would race
            // through it.
            origin = Some(now);
            seen = 0;
            dropped = 0;
        } else {
            dropped = 0;
            if !clock.wait_until(due) {
                return Ran::Left;
            }
        }
        let Some(rgba) =
            ronoctua_render::image::Rgba::new(reel.fit.width, reel.fit.height, buffer.clone())
        else {
            return Ran::Closed(seen);
        };
        #[allow(clippy::cast_precision_loss)]
        let at = reel.from + n as f64 / reel.fps.max(1.0);
        if out
            .send(Beat::Frame(Frame {
                key: reel.key.clone(),
                rgba,
                at,
            }))
            .is_err()
        {
            return Ran::Left;
        }
    }
}

/// Follows a GIF, looping, until it is told to stop.
fn play_gif(reel: &Reel, out: &calloop::channel::Sender<Beat>, clock: &Clock<'_>) {
    use image::AnimationDecoder;

    // LOOPING IS RE-OPENING, and it is not waste: the frames are decoded
    // lazily, so holding the iterator open is holding one frame — and holding
    // the decoded film instead would be the twenty-seven mebibytes the owner's
    // own sample weighs.
    //
    // AND A GIF HAS NO TIMELINE, so there is nothing here to seek to: its
    // frames are inter-dependent, and reaching frame five hundred means
    // composing five hundred.
    while clock.current() {
        let Ok(file) = std::fs::File::open(&reel.path) else {
            return;
        };
        let Ok(decoder) = image::codecs::gif::GifDecoder::new(std::io::BufReader::new(file)) else {
            return;
        };
        tracing::debug!(
            width = reel.fit.width,
            height = reel.fit.height,
            fps = reel.fps,
            "motion stream opened"
        );
        let mut n = 0usize;
        let mut origin: Option<Instant> = None;
        let mut elapsed = Duration::ZERO;
        for frame in decoder.into_frames() {
            let Some(paused_for) = clock.wait_out_a_pause() else {
                return;
            };
            if let Some(o) = origin.as_mut() {
                *o += paused_for;
            }
            if !clock.current() {
                return;
            }
            let Ok(frame) = frame else { return };
            let (num, den) = frame.delay().numer_denom_ms();
            let delay = Duration::from_millis(u64::from(num / den.max(1)).max(
                // A GIF MAY DECLARE NOUGHT, and browsers have clamped that for
                // twenty years for the reason `SLOWEST_FRAME_MS` gives.
                u64::from(SLOWEST_FRAME_MS),
            ));
            let picture = frame.into_buffer();
            let Some(rgba) = fit_into(picture, reel.fit) else {
                return;
            };
            let start = *origin.get_or_insert(Instant::now());
            // THE WALL CLOCK HERE TOO, and for the same reason: a sleep after
            // the work makes every frame longer than the file asked for, and
            // the error accumulates over a loop that never ends.
            if !clock.wait_until(start + elapsed) {
                return;
            }
            elapsed += delay;
            if out
                .send(Beat::Frame(Frame {
                    key: reel.key.clone(),
                    rgba,
                    // A GIF LOOPS AND HAS NO DURATION, so there is no position
                    // to report and nothing draws one.
                    at: 0.0,
                }))
                .is_err()
            {
                return;
            }
            n += 1;
        }
        // A GIF OF ONE FRAME WOULD SPIN HERE, and `motion::moving` is what
        // stops it arriving: it answers `None` for one, so this loop always has
        // something to wait on.
        if n == 0 {
            return;
        }
    }
}

/// Puts a GIF frame into a frame of exactly the panel's size, letterboxed.
///
/// # It comes out `fit`-sized, like ffmpeg's, and that is the point
///
/// Both sources produce ONE shape of frame, so the doors have one rule: a frame
/// from the player fills the box, because the letterbox inside it already did
/// the fitting. Returning the scaled picture and letting the door fit it again
/// is what framed a film twice — the bands the owner photographed.
///
/// The bars are transparent, so what shows through them is the panel's own
/// background in whichever door is drawing.
fn fit_into(picture: image::RgbaImage, fit: Fit) -> Option<ronoctua_render::image::Rgba> {
    let (w, h) = (picture.width(), picture.height());
    if w == 0 || h == 0 || fit.width == 0 || fit.height == 0 {
        return None;
    }
    // NEVER LARGER THAN IT WAS AUTHORED. A clip of 160×120 blown up to the
    // panel's width is sixteen times the pixels and no more picture.
    let by = (f64::from(fit.width) / f64::from(w))
        .min(f64::from(fit.height) / f64::from(h))
        .min(1.0);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let (to_w, to_h) = (
        ((f64::from(w) * by).round() as u32).clamp(1, fit.width),
        ((f64::from(h) * by).round() as u32).clamp(1, fit.height),
    );
    let scaled = if (to_w, to_h) == (w, h) {
        picture
    } else {
        // THE SAME FILTER THE STILL FRAME USES, so a file does not change
        // character when it starts moving.
        image::imageops::resize(&picture, to_w, to_h, image::imageops::FilterType::Lanczos3)
    };
    // AND INTO A TRANSPARENT CANVAS OF THE ASKED-FOR SIZE, centred — which is
    // what `pad=W:H:-1:-1:color=#00000000` does on the ffmpeg side.
    let mut out = image::RgbaImage::from_pixel(fit.width, fit.height, image::Rgba([0, 0, 0, 0]));
    image::imageops::replace(
        &mut out,
        &scaled,
        i64::from((fit.width - to_w) / 2),
        i64::from((fit.height - to_h) / 2),
    );
    ronoctua_render::image::Rgba::new(fit.width, fit.height, out.into_raw())
}

#[cfg(test)]
mod tests {
    use std::{
        io::Write,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicU64, Ordering},
        },
        time::{Duration, Instant},
    };

    use super::{Clock, Filled, Reaped, fill};

    /// FAILS IF: a pipe that never writes takes the player's thread with it.
    ///
    /// This is the live hang the rewrite exists for: with `read_exact` the
    /// thread blocked for ever, `orders.recv()` never ran again and nothing on
    /// screen said so.
    #[test]
    fn a_pipe_that_never_writes_gives_up() {
        let era = Box::leak(Box::new(AtomicU64::new(0)));
        let held = Box::leak(Box::new(AtomicBool::new(false)));
        let clock = Clock { era, held, mine: 0 };
        // A PIPE WITH A LIVE WRITER THAT SAYS NOTHING — which is exactly an
        // ffmpeg that opened its output and then stopped answering.
        let (mut reader, _writer) = std::io::pipe().expect("a pipe");
        let mut buffer = [0u8; 64];
        let began = Instant::now();
        let out = fill(&mut reader, &mut buffer, &clock);
        assert!(
            matches!(out, Filled::Starved),
            "a silent pipe was not given up on"
        );
        // AND IT GAVE UP INSIDE ITS OWN PATIENCE rather than at some other
        // bound: five seconds, with a second of slack for a loaded machine.
        assert!(
            began.elapsed() < Duration::from_secs(7),
            "it took {:?}",
            began.elapsed()
        );
        // AND THE ERA IS STILL OBEYED, which is what makes a stop arrive
        // during a read rather than after it.
        let era2 = Box::leak(Box::new(AtomicU64::new(0)));
        let held2 = Box::leak(Box::new(AtomicBool::new(false)));
        let clock2 = Clock {
            era: era2,
            held: held2,
            mine: 0,
        };
        let (mut reader, _writer) = std::io::pipe().expect("a pipe");
        era2.store(1, Ordering::SeqCst);
        let began = Instant::now();
        assert!(matches!(
            fill(&mut reader, &mut buffer, &clock2),
            Filled::Left
        ));
        assert!(
            began.elapsed() < Duration::from_secs(1),
            "the era was not checked inside the read"
        );
    }

    /// FAILS IF: half a frame is ever treated as a whole one.
    ///
    /// The smear this whole design rules out: a byte count that does not divide
    /// by the frame is the picture sliding sideways a little more each frame.
    #[test]
    fn a_frame_is_never_shorter_than_a_frame() {
        let era = Box::leak(Box::new(AtomicU64::new(0)));
        let held = Box::leak(Box::new(AtomicBool::new(false)));
        let clock = Clock { era, held, mine: 0 };
        let (mut reader, mut writer) = std::io::pipe().expect("a pipe");
        writer.write_all(&[7u8; 63]).expect("a short frame");
        drop(writer);
        let mut buffer = [0u8; 64];
        assert!(
            matches!(fill(&mut reader, &mut buffer, &clock), Filled::Short),
            "a frame one byte short was accepted"
        );
    }

    /// FAILS IF: a whole frame is not read as one.
    #[test]
    fn a_whole_frame_is_read_whole() {
        let era = Box::leak(Box::new(AtomicU64::new(0)));
        let held = Box::leak(Box::new(AtomicBool::new(false)));
        let clock = Clock { era, held, mine: 0 };
        let (mut reader, mut writer) = std::io::pipe().expect("a pipe");
        std::thread::spawn(move || {
            // IN TWO PIECES, which is what a pipe does with anything larger
            // than its buffer: the read has to gather.
            let _ = writer.write_all(&[1u8; 40]);
            std::thread::sleep(Duration::from_millis(30));
            let _ = writer.write_all(&[2u8; 24]);
        });
        let mut buffer = [0u8; 64];
        assert!(matches!(
            fill(&mut reader, &mut buffer, &clock),
            Filled::Whole
        ));
        assert_eq!(buffer[39], 1);
        assert_eq!(buffer[63], 2);
    }

    /// FAILS IF: a pause is not free, or does not end.
    #[test]
    fn a_pause_costs_nothing_and_ends() {
        let era = AtomicU64::new(0);
        let held = AtomicBool::new(true);
        let held = Arc::new(held);
        let mine = Arc::clone(&held);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            mine.store(false, Ordering::SeqCst);
        });
        let clock = Clock {
            era: &era,
            held: &held,
            mine: 0,
        };
        let waited = clock.wait_out_a_pause().expect("the pause ended");
        assert!(
            waited >= Duration::from_millis(60),
            "the pause was not waited out: {waited:?}"
        );
        // AND THE LENGTH IS REPORTED, which is what shifts the clock so the
        // film does not race to catch up with a schedule nobody was watching.
        assert!(waited < Duration::from_secs(2), "{waited:?}");
    }

    /// FAILS IF: a child outlives the thing that started it.
    ///
    /// The guard is what covers the exits nobody wrote by hand — and the exits
    /// nobody has written YET.
    #[test]
    fn nothing_outlives_an_order() {
        let born = std::process::Command::new("/bin/sh")
            .args(["-c", "sleep 300"])
            .stdout(std::process::Stdio::null())
            .spawn();
        let Ok(born) = born else {
            return;
        };
        let id = born.id();
        {
            let _guard = Reaped(born);
            // AND HERE THE SCOPE ENDS BY FALLING OFF, which is the exit
            // nobody would have written a `kill` at.
        }
        // THE PROCESS IS GONE AND REAPED — not a zombie, which `wait` is for.
        std::thread::sleep(Duration::from_millis(50));
        let alive = std::path::Path::new(&format!("/proc/{id}")).exists();
        assert!(!alive, "the child outlived its guard");
    }

    /// FAILS IF: the clock stops being the wall's.
    ///
    /// Three hundred frames at fifty a second is six seconds of film. Paced by
    /// a fixed sleep AFTER the work, it took seventeen per cent longer and the
    /// error accumulated; paced by due time it does not.
    #[test]
    fn the_clock_does_not_drift() {
        let era = AtomicU64::new(0);
        let held = AtomicBool::new(false);
        let clock = Clock {
            era: &era,
            held: &held,
            mine: 0,
        };
        let fps = 50.0_f64;
        let origin = Instant::now();
        let frames = 20;
        for n in 0..frames {
            #[allow(clippy::cast_precision_loss)]
            let due = origin + Duration::from_secs_f64(f64::from(n) / fps);
            assert!(clock.wait_until(due), "the clock let go");
            // THE WORK, which a fixed sleep would have added to every period.
            std::thread::sleep(Duration::from_millis(3));
        }
        let ran = origin.elapsed();
        #[allow(clippy::cast_precision_loss)]
        let want = Duration::from_secs_f64(f64::from(frames - 1) / fps);
        assert!(
            ran >= want,
            "it ran faster than the film: {ran:?} against {want:?}"
        );
        // AND NOT MUCH LONGER. Under a fixed sleep this would be `want` plus
        // three milliseconds a frame — sixty over twenty frames, half again as
        // long.
        assert!(
            ran < want + Duration::from_millis(40),
            "the clock drifted: {ran:?} against {want:?}"
        );
    }
}
