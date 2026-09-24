//! The engine: the mpv that plays, the ones that follow it, and whoever is
//! watching.
//!
//! # One decode, many pictures
//!
//! The item playing is decoded ONCE, by one mpv, and rendered by the software
//! renderer at every size a viewer asked for — the column, the footer and a
//! window each get their own ring, and each ring gets the same frame. Being in
//! step is therefore not something that is achieved: nothing could be out of
//! it. Sound comes out of that mpv, once.
//!
//! # The participants
//!
//! A DIFFERENT file shown alongside (two videos to one soundtrack) is a second
//! mpv with no sound, measured against the first every `sync::TICK` and nudged
//! by `sync::correct` — a speed a few percent off until it catches up, or a
//! seek when it is too far.
//!
//! # At rest
//!
//! No mpv exists until something is asked to play: the daemon at rest is a
//! socket and a sleeping thread. The queue it remembers is only a file.

use std::{
    collections::HashMap,
    io::{BufReader, Read, Write as _},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};

use ronoctua_player::{
    decoders, lrclib,
    prefs::{self, Preferences},
    sponsorblock::{self, Segment},
    sync::{self, Correction},
    wire::{self, Ask, Item, Participant, Repeat, Say, State, Track, Trouble},
};

use crate::mpv::{self, Aligned, Event, Mpv, Render, Value};

/// The shared engine: a lock and the bell the render and sync threads sleep on.
pub type Shared = Arc<(Mutex<Engine>, Condvar)>;

/// A viewer: a connection that asked for frames, and its ring.
struct Viewer {
    stream: UnixStream,
    ring: ronoctua_frames::Ring,
    /// Whose picture: `None` the main one, `Some(i)` a participant.
    of: Option<usize>,
    /// Owed the frame on screen NOW. A paused video and an mp3's cover send
    /// no new frame, so a view opened on one waited for something to change
    /// — the owner's cover stayed black until a click paused the song.
    owed: bool,
}

/// A participant: another file, no sound, kept in step.
struct Follower {
    item: String,
    mpv: Arc<Mpv>,
    drift_ms: i64,
}

/// Property ids for `observe`.
mod id {
    pub const PAUSE: u64 = 1;
    pub const TIME: u64 = 2;
    pub const DURATION: u64 = 3;
    pub const VOLUME: u64 = 4;
    pub const MUTE: u64 = 5;
    pub const SPEED: u64 = 6;
    pub const POS: u64 = 7;
    pub const PLAYLIST: u64 = 8;
    pub const TRACKS: u64 = 9;
    pub const METADATA: u64 = 10;
    pub const TITLE: u64 = 11;
    pub const WIDTH: u64 = 12;
    pub const HEIGHT: u64 = 13;
    pub const IDLE: u64 = 14;
}

/// The engine's state behind the lock.
pub struct Engine {
    mpv: Option<Arc<Mpv>>,
    state: State,
    watchers: Vec<UnixStream>,
    viewers: Vec<Viewer>,
    /// How to wake each render thread, by whose picture it draws.
    pokes: Vec<(Option<usize>, mpv::Poke)>,
    followers: Vec<Follower>,
    /// Something changed that watchers have not been told.
    dirty: bool,
    /// When watchers were last told of a mere POSITION change: those come
    /// dozens of times a second and are said four times.
    last_tick: Instant,
    idle: bool,
    /// What the person chose (`player.json`), reread on `Ask::Reload`.
    prefs: Preferences,
    /// The sponsored stretches of the video playing, and the ones already
    /// jumped.
    segments: Vec<Segment>,
    jumped: Vec<usize>,
    /// Requests to make, taken by the event thread and made OUTSIDE the lock:
    /// a host on the internet is given seconds, and the engine is not.
    fetches: Vec<Fetch>,
    /// The item lyrics were last asked for, so a tag arriving twice is not
    /// two requests.
    lyrics_asked: Option<String>,
    /// The pairing code on show, and when it was made.
    invite: Option<(String, Instant)>,
    /// The tailnet, as last read.
    tailnet: Option<crate::remote::Tailnet>,
    /// The machine this one follows, when it does.
    following: Option<crate::remote::Following>,
    /// Poked on every broadcast, so MPRIS announces what changed.
    mpris: Option<std::sync::mpsc::Sender<()>>,
    /// This machine's own queue, where it was, while it follows another —
    /// put back when it leaves. Following plays in the main player, and a
    /// queue that was simply replaced would be a person's playlist lost to
    /// pressing "join".
    kept: Option<(Vec<Item>, Option<usize>, f64)>,
    /// A start position set only for the queue being put back, cleared once
    /// that file has loaded — `start` is an option, and it would otherwise
    /// apply to every file after it.
    start_once: bool,
}

/// A request to a host on the internet, and the item it is for.
#[derive(Debug, Clone)]
enum Fetch {
    Lyrics {
        item: String,
        path: String,
    },
    Segments {
        item: String,
        id: String,
        path: String,
    },
}

/// The file the queue is remembered in.
const QUEUE_FILE: &str = "player.json";

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Remembered {
    queue: Vec<Item>,
    current: Option<usize>,
    volume: Option<u8>,
    repeat: Repeat,
}

fn remembered() -> ronoctua_store::Store<Remembered> {
    ronoctua_store::Store::at(ronoctua_store::state_dir(), QUEUE_FILE)
}

impl Engine {
    /// An engine with no mpv, and the queue it last had.
    #[must_use]
    pub fn new() -> Self {
        let r = remembered().read();
        let state = State {
            queue: r.queue,
            current: r.current,
            volume: r.volume.unwrap_or(100),
            repeat: r.repeat,
            speed: 1.0,
            ..State::default()
        };
        Self {
            mpv: None,
            state,
            watchers: Vec::new(),
            viewers: Vec::new(),
            pokes: Vec::new(),
            followers: Vec::new(),
            dirty: false,
            last_tick: Instant::now(),
            idle: true,
            prefs: prefs::store().read(),
            segments: Vec::new(),
            jumped: Vec::new(),
            fetches: Vec::new(),
            lyrics_asked: None,
            invite: None,
            tailnet: None,
            following: None,
            mpris: None,
            kept: None,
            start_once: false,
        }
    }

    /// The state, for an answer.
    #[must_use]
    pub fn state(&self) -> State {
        let mut s = self.state.clone();
        s.participants = self
            .followers
            .iter()
            .map(|f| Participant {
                item: f.item.clone(),
                drift_ms: f.drift_ms,
            })
            .collect();
        s.invite = self
            .invite
            .as_ref()
            .filter(|(_, at)| at.elapsed() < ronoctua_player::remote::CODE_LIFE)
            .map(|(c, _)| c.clone());
        s.tailnet = self.tailnet.as_ref().map(|t| t.name.clone());
        s.nearby = self
            .tailnet
            .as_ref()
            .map(|t| t.peers.iter().map(|(n, _)| n.clone()).collect())
            .unwrap_or_default();
        let peers = ronoctua_player::remote::peers().read();
        s.hosts = peers.hosts.iter().map(|p| p.name.clone()).collect();
        s.guests = peers.guests.iter().map(|p| p.name.clone()).collect();
        s.joined = self.following.as_ref().map(|f| wire::Joined {
            host: f.host.clone(),
            role: f.role,
            drift_ms: f.drift_ms,
        });
        s
    }

    /// The tailnet, as the listener last read it.
    pub fn set_tailnet(&mut self, t: Option<crate::remote::Tailnet>) {
        self.tailnet = t;
    }

    /// The tailnet, when this machine is on one.
    #[must_use]
    pub const fn tailnet(&self) -> Option<&crate::remote::Tailnet> {
        self.tailnet.as_ref()
    }

    /// What this machine calls itself to another: its tailnet name, or its
    /// host name.
    #[must_use]
    pub fn machine_name(&self) -> String {
        self.tailnet
            .as_ref()
            .map(|t| t.name.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| {
                std::fs::read_to_string("/etc/hostname")
                    .ok()
                    .map(|h| h.trim().to_owned())
            })
            .unwrap_or_default()
    }

    /// The key for a guest that read the right code, while it is good; the
    /// code is used up.
    pub fn take_invite(&mut self, code: &str) -> Option<String> {
        let (shown, at) = self.invite.as_ref()?;
        if at.elapsed() >= ronoctua_player::remote::CODE_LIFE
            || !ronoctua_player::remote::same(shown, code)
        {
            return None;
        }
        self.invite = None;
        let bytes: [u8; 32] = ronoctua_player::remote::random().ok()?;
        Some(ronoctua_player::remote::key_of(&bytes))
    }

    /// Where this machine is, for a machine that follows it.
    #[must_use]
    pub fn moment(&self) -> ronoctua_player::remote::Moment {
        let item = self.state.current;
        let address = item
            .and_then(|i| self.state.queue.get(i))
            .map(|i| i.location.clone())
            .filter(|l| l.contains("://"));
        let speed = if self.state.speed > 0.0 {
            self.state.speed
        } else {
            1.0
        };
        ronoctua_player::remote::Moment {
            clock_ms: ronoctua_player::remote::clock_ms(),
            address,
            item,
            position: self.state.position,
            playing: self.state.playing,
            speed,
            title: self.state.title.clone(),
            name: item
                .and_then(|i| self.state.queue.get(i))
                .filter(|i| !i.location.contains("://"))
                .and_then(|i| {
                    Path::new(&i.location)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                }),
        }
    }

    /// The location of queue item `i`.
    #[must_use]
    pub fn location_of(&self, i: usize) -> Option<String> {
        self.state.queue.get(i).map(|i| i.location.clone())
    }

    /// Puts back what following borrowed: the tracks and the speed as they
    /// were, and this machine's own queue, paused where it was.
    fn put_back(&mut self) {
        let Some(mpv) = self.mpv.clone() else {
            self.kept = None;
            return;
        };
        let _ = mpv.set("http-header-fields", "");
        let _ = mpv.set("aid", "auto");
        let _ = mpv.set("vid", "auto");
        let _ = mpv.set("speed", "1");
        let Some((queue, current, position)) = self.kept.take() else {
            return;
        };
        let _ = mpv.set("pause", "yes");
        let _ = mpv.command(&["playlist-clear"]);
        let _ = mpv.command(&["stop"]);
        for item in &queue {
            let _ = load(&mpv, &item.location, "append");
        }
        if let Some(at) = current {
            let _ = mpv.set("start", &format!("{position:.3}"));
            self.start_once = true;
            let _ = mpv.set("playlist-pos", &at.to_string());
        }
        self.dirty = true;
    }

    /// Following ended on its own: why, in the troubles.
    pub fn left(&mut self, why: &str) {
        self.following = None;
        self.put_back();
        self.state.troubles = vec![Trouble::Remote {
            why: why.to_owned(),
        }];
    }

    /// How far this machine is from the one it follows.
    pub fn drifted(&mut self, ms: i64) {
        if let Some(f) = self.following.as_mut() {
            f.drift_ms = ms;
        }
        self.dirty = true;
    }

    /// Tells every watcher, dropping the ones gone.
    pub fn broadcast(&mut self) {
        let say = Say::State(Box::new(self.state()));
        self.watchers
            .retain_mut(|w| wire::write_line(w, &say).is_ok());
        self.dirty = false;
        self.last_tick = Instant::now();
        if let Some(m) = &self.mpris {
            let _ = m.send(());
        }
    }

    /// Where to poke MPRIS.
    pub fn set_mpris(&mut self, poke: Option<std::sync::mpsc::Sender<()>>) {
        self.mpris = poke;
    }

    fn remember(&self) {
        // ANOTHER MACHINE'S QUEUE IS NOT THIS ONE'S: while following, what
        // the player holds is borrowed, and remembering it would make it the
        // queue this machine wakes up with.
        if self.following.is_some() {
            return;
        }
        let r = Remembered {
            queue: self.state.queue.clone(),
            current: self.state.current,
            volume: Some(self.state.volume),
            repeat: self.state.repeat,
        };
        if let Err(e) = remembered().write(&r) {
            tracing::warn!(error = %e, "could not remember the queue");
        }
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

/// What every mpv here is started with.
fn options(sound: bool) -> Vec<(&'static str, String)> {
    let mut o = vec![
        ("vo", "libmpv".to_owned()),
        // NO TERMINAL: measured, mpv touching a terminal it does not own
        // (a process group in the background) is what a daemon must never do.
        ("terminal", "no".to_owned()),
        ("input-default-bindings", "no".to_owned()),
        ("input-vo-keyboard", "no".to_owned()),
        // NOT THE PERSON'S mpv.conf: this is a system component, and an
        // option set for the standalone mpv (a vo, a profile) must not change
        // what the column shows.
        ("config", "no".to_owned()),
        ("idle", "yes".to_owned()),
        ("osc", "no".to_owned()),
        // NO OSD: the software renderer draws it INTO the frame, and every
        // door draws its own controls.
        ("osd-level", "0".to_owned()),
        // COVER ART IS THE PICTURE of a song: it arrives as a video track and
        // every door shows it like one.
        ("audio-display", "embedded-first".to_owned()),
        ("sub-auto", "fuzzy".to_owned()),
        ("audio-file-auto", "fuzzy".to_owned()),
        ("hwdec", "auto-copy".to_owned()),
        ("audio-client-name", "ronoctua".to_owned()),
        ("keep-open", "no".to_owned()),
        // A MACHINE WITH NO SOUND still plays: the output falls to `null`
        // instead of refusing the file. Without it every file on a machine
        // whose sound server is down is "audio output initialization failed"
        // — measured — and a video would not move either.
        ("audio-fallback-to-null", "yes".to_owned()),
        ("ytdl", "yes".to_owned()),
    ];
    if !sound {
        o.push(("ao", "null".to_owned()));
        o.push(("audio", "no".to_owned()));
    }
    // YT-DLP BY PATH, and the flags the queue already uses for it (cookies,
    // the PO token): a unit's PATH does not have it.
    if let Ok(path) = std::env::var("RONOCTUA_YT_DLP") {
        o.push(("script-opts", format!("ytdl_hook-ytdl_path={path}")));
    }
    // A LIST IS A QUEUE: an address with `list=` opens every video of it,
    // which is what a person who pasted a playlist meant.
    let mut raw = vec!["yes-playlist=".to_owned()];
    // AND THE PO TOKEN, the queue's own flags (`ronoctua_ytdlp`): what makes
    // an anonymous request work is the same for playing as for downloading.
    let pot = ronoctua_ytdlp::pot_flags();
    for pair in pot.as_chunks::<2>().0 {
        raw.push(format!("{}={}", pair[0].trim_start_matches("--"), pair[1]));
    }
    o.push(("ytdl-raw-options", raw.join(",")));
    o
}

fn open_mpv(sound: bool) -> Result<Arc<Mpv>> {
    let owned = options(sound);
    let pairs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let mpv = Arc::new(Mpv::new(&pairs)?);
    mpv.streams(decoders::PROTOCOL, Box::new(open_decoded))?;
    Ok(mpv)
}

/// What mpv opens for a `ronoctua-decode://` URI: the decoder's output.
fn open_decoded(uri: &str) -> Option<Box<dyn mpv::Stream>> {
    let path = decoders::path_of(uri)?;
    let decoder = decoders::for_path(Path::new(path))?;
    match crate::engine::decode(decoder, Path::new(path)) {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::warn!(error = %e, decoder = decoder.id, "a decoder would not start");
            None
        }
    }
}

/// Starts `decoder` on `path` and answers what it writes, WAV-shaped.
pub fn decode(decoder: &'static decoders::Decoder, path: &Path) -> Result<Box<dyn mpv::Stream>> {
    let (reader, child) = start(decoder, path)?;
    Ok(Box::new(Decoded {
        decoder,
        path: path.to_path_buf(),
        reader,
        child,
        at: 0,
        beyond: false,
    }))
}

/// Starts `decoder` on `path`: the WAV it writes, header mended, and the
/// process when the reader does not own it already.
fn start(
    decoder: &decoders::Decoder,
    path: &Path,
) -> Result<(Box<dyn Read + Send>, Option<std::process::Child>)> {
    let program =
        std::env::var(decoder.env).with_context(|| format!("{} is not set", decoder.env))?;
    let soundfont = std::env::var("RONOCTUA_SOUNDFONT").unwrap_or_default();
    // A FIFO for the decoders that only write to a file. It lives in the
    // runtime directory, named after this process and a counter.
    let fifo = match decoder.output {
        decoders::Output::WavToFile => {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let at = ronoctua_xdg::runtime_file(&format!("player-{}-{n}.wav", std::process::id()))
                .context("no runtime directory")?;
            let _ = std::fs::remove_file(&at);
            rustix::fs::mknodat(
                rustix::fs::CWD,
                &at,
                rustix::fs::FileType::Fifo,
                rustix::fs::Mode::from_raw_mode(0o600),
                0,
            )?;
            Some(at)
        }
        // A REGULAR FILE for the decoders that will not write into a FIFO —
        // zxtune checks what it creates and refuses one. In the CACHE and not
        // the runtime directory: a song is tens of megabytes as WAV, and the
        // runtime directory is memory. The decoder is given the name without
        // `.wav`, which it adds itself.
        decoders::Output::WavToGrowingFile => {
            static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let dir = ronoctua_xdg::cache_dir().context("no cache directory")?;
            std::fs::create_dir_all(&dir)?;
            let stem = dir.join(format!("player-{}-{n}", std::process::id()));
            let _ = std::fs::remove_file(stem.with_extension("wav"));
            Some(stem)
        }
        _ => None,
    };
    let args: Vec<String> = decoder
        .args
        .iter()
        .map(|a| {
            a.replace("{in}", &path.to_string_lossy())
                .replace(
                    "{out}",
                    &fifo
                        .as_ref()
                        .map(|f| f.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )
                .replace("{soundfont}", &soundfont)
        })
        .collect();
    let child = std::process::Command::new(&program)
        .args(&args)
        .stdin(std::process::Stdio::null())
        .stdout(if fifo.is_some() {
            std::process::Stdio::null()
        } else {
            std::process::Stdio::piped()
        })
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("running {program}"))?;
    let mut child = Some(child);
    let mut wav: Box<dyn Read + Send> = match (decoder.output, fifo) {
        (decoders::Output::WavToGrowingFile, Some(stem)) => Box::new(Growing::open(
            stem.with_extension("wav"),
            child.take().context("no decoder")?,
        )?),
        (decoders::Output::WavToFile, Some(at)) => {
            let file = std::fs::File::open(&at)?;
            let _ = std::fs::remove_file(&at);
            Box::new(file)
        }
        (decoders::Output::RawOnStdout, _) => {
            let out = child
                .as_mut()
                .and_then(|c| c.stdout.take())
                .context("no stdout")?;
            Box::new(std::io::Cursor::new(decoders::wav_header().to_vec()).chain(out))
        }
        _ => Box::new(
            child
                .as_mut()
                .and_then(|c| c.stdout.take())
                .context("no stdout")?,
        ),
    };
    // Every one is a WAV by now, and a streamed one says its data is empty.
    let mut header = [0u8; 44];
    wav.read_exact(&mut header)
        .context("the decoder wrote no header")?;
    decoders::endless(&mut header);
    let reader: Box<dyn Read + Send> = Box::new(std::io::Cursor::new(header.to_vec()).chain(wav));
    Ok((reader, child))
}

/// Removes what an earlier daemon's decoders left in the cache. A daemon that
/// is killed never runs the `Drop` that removes its file, and a song as WAV is
/// tens of megabytes; this runs once, before anything plays, when no file of
/// this process exists yet.
pub fn forget_old_decodes() {
    let Some(dir) = ronoctua_xdg::cache_dir() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name();
        let name = name.to_string_lossy();
        if name.starts_with("player-") {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// A file a decoder is still writing, read as it grows: at its end the reader
/// waits for more while the decoder lives, and answers the end only once it
/// has exited and nothing more came. It owns the decoder and the file, and
/// both go with it.
struct Growing {
    file: std::fs::File,
    path: PathBuf,
    child: std::process::Child,
}

/// How long a reader waits at the end of a growing file before looking again.
const GROWING_WAIT: Duration = Duration::from_millis(20);

/// How long the decoder is given to create its file at all.
const GROWING_BIRTH: Duration = Duration::from_secs(10);

impl Growing {
    fn open(path: PathBuf, mut child: std::process::Child) -> Result<Self> {
        let born = Instant::now();
        loop {
            if let Ok(file) = std::fs::File::open(&path) {
                return Ok(Self { file, path, child });
            }
            if child.try_wait().ok().flatten().is_some() || born.elapsed() > GROWING_BIRTH {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("the decoder wrote no file");
            }
            std::thread::sleep(GROWING_WAIT);
        }
    }
}

impl Read for Growing {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        loop {
            let n = self.file.read(buf)?;
            if n > 0 {
                return Ok(n);
            }
            if self.child.try_wait()?.is_some() {
                // EXITED: whatever it wrote after the last read is still owed.
                return self.file.read(buf);
            }
            std::thread::sleep(GROWING_WAIT);
        }
    }
}

impl Drop for Growing {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// A decoder's output, which reaps the decoder when mpv closes it.
///
/// # It SEEKS, and that is what makes a position row and a follower work
///
/// A pipe cannot go back, and a stream with no seek is one mpv can neither
/// jump in nor START at: a machine following another played a SID six
/// seconds behind for ever. Forward, the bytes up to the place are decoded
/// and thrown away — every decoder here is many times faster than playing;
/// backward, the decoder starts again from the beginning and does the same.
/// Its output is the same bytes every time, so a byte offset means one moment.
struct Decoded {
    decoder: &'static decoders::Decoder,
    path: PathBuf,
    reader: Box<dyn Read + Send>,
    child: Option<std::process::Child>,
    /// The byte the next read gives.
    at: u64,
    /// Sent past the claimed end by a probe: reads answer nothing.
    beyond: bool,
}

impl Read for Decoded {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.beyond {
            return Ok(0);
        }
        let n = self.reader.read(buf)?;
        self.at += n as u64;
        Ok(n)
    }
}

/// Where the endless WAV header's claimed data ends (`decoders::endless`).
const PAST_THE_CLAIM: u64 = u32::MAX as u64 - 36;

impl mpv::Stream for Decoded {
    fn seek_to(&mut self, to: u64) -> std::io::Result<u64> {
        // PAST THE END THE HEADER CLAIMS is a probe, not a place: the WAV
        // says "until the stream ends" with the largest size it can hold, and
        // the demuxer looks for a chunk after it — measured, a seek to byte
        // 4 294 967 304 on open. Decoding up to there played nothing for the
        // length of the whole song; REFUSING it made mpv take the stream for
        // ended. So the stream goes there without decoding anything, answers
        // nothing from there, and the seek back that follows starts the
        // decoder again — one restart per file, at open.
        if to >= PAST_THE_CLAIM {
            self.beyond = true;
            self.at = to;
            return Ok(to);
        }
        self.beyond = false;
        if to < self.at {
            let (reader, child) = start(self.decoder, &self.path).map_err(std::io::Error::other)?;
            if let Some(mut old) = std::mem::replace(&mut self.child, child) {
                let _ = old.kill();
                let _ = old.wait();
            }
            self.reader = reader;
            self.at = 0;
        }
        let skipped = std::io::copy(
            &mut (&mut self.reader).take(to - self.at),
            &mut std::io::sink(),
        )?;
        self.at += skipped;
        Ok(self.at)
    }
}

impl Drop for Decoded {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// What mpv is given for a queue item: a decoder's URI for the formats it
/// cannot read, the item itself otherwise.
/// What is asked for, with the playlists libmpv cannot read opened here.
///
/// # Only XSPF and ASX
///
/// libmpv reads `.m3u`, `.m3u8` and `.pls` itself, and reads them LAZILY — an
/// entry becomes an item when the queue reaches it, which is also how an HLS
/// stream is played. XSPF and ASX it does not know at all: handed one, it
/// ends the file at once and the queue moves on as if nothing had been asked.
/// Those two are read by the same parser the file manager's preview uses, so
/// the queue holds exactly the entries that panel listed.
///
/// A list that cannot be read, or names nothing, is passed through as it was:
/// libmpv then refuses it the way it refuses any other file, and the queue
/// shows that, instead of the ask vanishing here.
fn expanded(items: Vec<String>) -> Vec<String> {
    use ronoctua_playlist::{Kind, Where};
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let path = std::path::Path::new(item.strip_prefix("file://").unwrap_or(&item));
        let list = match ronoctua_playlist::kind_by_name(path) {
            Some(kind @ (Kind::Xspf | Kind::Asx)) => ronoctua_playlist::read(path, kind).ok(),
            _ => None,
        };
        let Some(list) = list else {
            out.push(item);
            continue;
        };
        for entry in list.entries {
            match entry.at {
                Where::Local(at) => out.push(at.to_string_lossy().into_owned()),
                Where::Remote(url) => out.push(url),
                Where::Refused => {}
            }
        }
    }
    out
}

/// The names libgme reads: the music of consoles and 8-bit computers, one
/// chip's register writes.
///
/// # Why they are named at all
///
/// Because ffmpeg recognizes them at a probe SCORE of 25 and libmpv asks for
/// 26 (`demuxer-lavf-probescore`), so every one of them was answered
/// "unrecognized file format" — measured on a VGM that `ffprobe` read
/// without a word. The plan counted them as free with `ffmpeg-full`; nothing
/// had played one. Lowering the threshold for every file would let a weak
/// guess win elsewhere, so the demuxer is named for these names only.
const GAME_MUSIC: [&str; 11] = [
    "vgm", "vgz", "nsf", "nsfe", "spc", "gbs", "hes", "kss", "ay", "sap", "gym",
];

/// The options a file is loaded with: libgme named for the names it reads.
fn load_options(location: &str) -> Option<&'static str> {
    let path = location.split(['?', '#']).next().unwrap_or(location);
    let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
    GAME_MUSIC
        .contains(&extension.as_str())
        .then_some("demuxer-lavf-format=libgme")
}

/// Loads one item — through its decoder when one of ours reads it, and with
/// the demuxer named when libmpv would not find it on its own.
pub(crate) fn load(mpv: &Mpv, location: &str, how: &str) -> Result<()> {
    let target = loadable(location);
    match load_options(location) {
        // `loadfile <url> <flags> <index> <options>`: -1 is "no index".
        Some(options) => mpv.command(&["loadfile", &target, how, "-1", options]),
        None => mpv.command(&["loadfile", &target, how]),
    }
}

pub(crate) fn loadable(location: &str) -> String {
    let p = Path::new(location);
    if !location.contains("://") && decoders::for_path(p).is_some() {
        decoders::uri_of(p)
    } else {
        location.to_owned()
    }
}

/// The queue's location back from what mpv holds.
fn location_of(loaded: &str) -> String {
    decoders::path_of(loaded).map_or_else(|| loaded.to_owned(), str::to_owned)
}

impl Engine {
    /// The main mpv, made on first need and handed its queue.
    pub(crate) fn main(&mut self, shared: &Shared) -> Result<Arc<Mpv>> {
        if let Some(m) = &self.mpv {
            return Ok(m.clone());
        }
        let mpv = open_mpv(true)?;
        for (id, name, format) in [
            (id::PAUSE, "pause", mpv::FORMAT_FLAG),
            (id::TIME, "time-pos", mpv::FORMAT_DOUBLE),
            (id::DURATION, "duration", mpv::FORMAT_DOUBLE),
            (id::VOLUME, "volume", mpv::FORMAT_DOUBLE),
            (id::MUTE, "mute", mpv::FORMAT_FLAG),
            (id::SPEED, "speed", mpv::FORMAT_DOUBLE),
            (id::POS, "playlist-pos", mpv::FORMAT_INT64),
            (id::PLAYLIST, "playlist", mpv::FORMAT_NONE),
            (id::TRACKS, "track-list", mpv::FORMAT_NONE),
            (id::METADATA, "metadata", mpv::FORMAT_NONE),
            (id::TITLE, "media-title", mpv::FORMAT_STRING),
            (id::WIDTH, "dwidth", mpv::FORMAT_INT64),
            (id::HEIGHT, "dheight", mpv::FORMAT_INT64),
            (id::IDLE, "idle-active", mpv::FORMAT_FLAG),
        ] {
            mpv.observe(id, name, format)?;
        }
        mpv.set("volume", &self.state.volume.to_string())?;
        apply_repeat(&mpv, self.state.repeat);
        // THE QUEUE IT REMEMBERED, loaded without playing.
        for item in &self.state.queue {
            load(&mpv, &item.location, "append")?;
        }
        self.mpv = Some(mpv.clone());
        spawn_events(mpv.clone(), shared.clone());
        let poke = spawn_render(mpv.clone(), None, shared.clone())?;
        self.keep_poke(None, poke);
        Ok(mpv)
    }

    /// Carries out an ask; the answer for the one who asked.
    pub fn ask(&mut self, ask: Ask, shared: &Shared) -> Say {
        self.dirty = true;
        match self.carry(ask, shared) {
            Ok(()) => Say::Done,
            Err(e) => {
                let t = Trouble::NoEngine {
                    why: format!("{e:#}"),
                };
                self.state.troubles = vec![t.clone()];
                Say::Refused(t)
            }
        }
    }

    fn carry(&mut self, ask: Ask, shared: &Shared) -> Result<()> {
        match ask {
            Ask::Watch | Ask::Look | Ask::Preview { .. } => {}
            Ask::Open { items, replace } => {
                let mpv = self.main(shared)?;
                for (n, item) in expanded(items).iter().enumerate() {
                    let how = if replace && n == 0 {
                        "replace"
                    } else if replace {
                        "append"
                    } else {
                        "append-play"
                    };
                    load(&mpv, item, how)?;
                }
                if replace {
                    mpv.set("pause", "no")?;
                }
            }
            Ask::Toggle => {
                let mpv = self.main(shared)?;
                if self.idle {
                    self.play_current(&mpv)?;
                } else {
                    mpv.command(&["cycle", "pause"])?;
                }
            }
            Ask::Play => {
                let mpv = self.main(shared)?;
                if self.idle {
                    self.play_current(&mpv)?;
                } else {
                    mpv.set("pause", "no")?;
                }
            }
            Ask::Pause => {
                if let Some(m) = &self.mpv {
                    m.set("pause", "yes")?;
                }
            }
            Ask::Stop => {
                if let Some(m) = &self.mpv {
                    m.command(&["stop", "keep-playlist"])?;
                }
            }
            Ask::Next => self.main(shared)?.command(&["playlist-next", "force"])?,
            Ask::Previous => self.main(shared)?.command(&["playlist-prev", "force"])?,
            Ask::Jump { index } => {
                let mpv = self.main(shared)?;
                mpv.command(&["playlist-play-index", &index.to_string()])?;
                mpv.set("pause", "no")?;
            }
            Ask::Remove { index } => {
                self.main(shared)?
                    .command(&["playlist-remove", &index.to_string()])?;
            }
            Ask::Move { from, to } => {
                // mpv's `playlist-move` puts the item BEFORE `to`; moving one
                // down therefore names the slot after.
                let target = if to > from { to + 1 } else { to };
                self.main(shared)?.command(&[
                    "playlist-move",
                    &from.to_string(),
                    &target.to_string(),
                ])?;
            }
            Ask::Clear => {
                let mpv = self.main(shared)?;
                mpv.command(&["stop"])?;
                mpv.command(&["playlist-clear"])?;
            }
            Ask::Seek { seconds, relative } => {
                if let Some(m) = &self.mpv {
                    m.command(&[
                        "seek",
                        &seconds.to_string(),
                        if relative { "relative" } else { "absolute" },
                        "exact",
                    ])?;
                }
            }
            Ask::Volume { percent } => {
                let p = percent.min(130);
                self.state.volume = p;
                if let Some(m) = &self.mpv {
                    m.set("volume", &p.to_string())?;
                }
                self.remember();
            }
            Ask::Mute => {
                if let Some(m) = &self.mpv {
                    m.command(&["cycle", "mute"])?;
                }
            }
            Ask::Speed { factor } => {
                if let Some(m) = &self.mpv {
                    m.set("speed", &factor.clamp(0.25, 4.0).to_string())?;
                }
            }
            Ask::Repeat => {
                self.state.repeat = self.state.repeat.next();
                if let Some(m) = &self.mpv {
                    apply_repeat(m, self.state.repeat);
                }
                self.remember();
            }
            Ask::Shuffle => {
                let mpv = self.main(shared)?;
                self.state.shuffle = !self.state.shuffle;
                mpv.command(&[if self.state.shuffle {
                    "playlist-shuffle"
                } else {
                    "playlist-unshuffle"
                }])?;
            }
            Ask::AudioTrack { id } => {
                if let Some(m) = &self.mpv {
                    m.set(
                        "aid",
                        &id.map_or_else(|| "no".to_owned(), |i| i.to_string()),
                    )?;
                }
            }
            Ask::SubtitleTrack { id } => {
                if let Some(m) = &self.mpv {
                    m.set(
                        "sid",
                        &id.map_or_else(|| "no".to_owned(), |i| i.to_string()),
                    )?;
                }
            }
            Ask::Reload => self.prefs = prefs::store().read(),
            Ask::Follow { item } => {
                let main = self.main(shared)?;
                let mpv = open_mpv(false)?;
                // THE RENDER CONTEXT BEFORE THE FILE, and the start before
                // the file too: `vo=libmpv` looks for its context when the
                // video starts, and with none it drops the video for good —
                // measured, a follower that played and never showed a frame.
                let index = self.followers.len();
                let poke = spawn_render(mpv.clone(), Some(index), shared.clone())?;
                self.keep_poke(Some(index), poke);
                if let Some(t) = main.float("time-pos") {
                    let _ = mpv.set("start", &format!("{t:.3}"));
                }
                load(&mpv, &item, "replace")?;
                spawn_quiet_events(mpv.clone());
                self.followers.push(Follower {
                    item,
                    mpv,
                    drift_ms: 0,
                });
                if self.followers.len() == 1 {
                    spawn_sync(shared.clone());
                }
            }
            Ask::Unfollow { index } => {
                if index < self.followers.len() {
                    let f = self.followers.remove(index);
                    let _ = f.mpv.command(&["quit"]);
                    // Viewers of the ones after it now point one lower.
                    self.viewers.retain(|v| v.of != Some(index));
                    for v in &mut self.viewers {
                        if let Some(i) = v.of
                            && i > index
                        {
                            v.of = Some(i - 1);
                        }
                    }
                }
            }
            Ask::Invite => {
                let code = ronoctua_player::remote::code_of(ronoctua_player::remote::random()?);
                self.invite = Some((code, Instant::now()));
            }
            // PAIRING TALKS TO ANOTHER MACHINE, so it is `serve_one`'s, outside
            // the lock; it never reaches here.
            Ask::PairWith { .. } => {}
            Ask::Join { host, role } => {
                let peers = ronoctua_player::remote::peers().read();
                let Some(peer) = peers.hosts.into_iter().find(|h| h.name == host) else {
                    anyhow::bail!("{host} is not paired with this machine");
                };
                if let Some(f) = self.following.take() {
                    f.stop.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                // THE QUEUE ASIDE, once: joining a second machine while
                // following a first keeps the one this machine had.
                if self.kept.is_none() {
                    self.kept = Some((
                        self.state.queue.clone(),
                        self.state.current,
                        self.state.position,
                    ));
                }
                let stop = crate::remote::follow(peer, role, shared.clone());
                self.following = Some(crate::remote::Following {
                    host,
                    role,
                    drift_ms: 0,
                    stop,
                });
            }
            Ask::Leave => {
                if let Some(f) = self.following.take() {
                    f.stop.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                self.put_back();
            }
            Ask::Forget { name } => {
                let mut p = ronoctua_player::remote::peers().read();
                p.hosts.retain(|h| h.name != name);
                p.guests.retain(|g| g.name != name);
                ronoctua_player::remote::keep(&p)?;
            }
        }
        Ok(())
    }

    fn play_current(&mut self, mpv: &Mpv) -> Result<()> {
        let at = self.state.current.unwrap_or(0);
        if self.state.queue.is_empty() {
            return Ok(());
        }
        mpv.command(&["playlist-play-index", &at.to_string()])?;
        mpv.set("pause", "no")
    }

    /// Keeps what wakes the render thread of `of`'s picture.
    fn keep_poke(&mut self, of: Option<usize>, poke: mpv::Poke) {
        self.pokes.retain(|(o, _)| *o != of);
        self.pokes.push((of, poke));
    }

    /// A new viewer on `stream`, at `width × height` of `of`.
    pub fn add_viewer(
        &mut self,
        stream: UnixStream,
        width: u32,
        height: u32,
        of: Option<usize>,
    ) -> Result<()> {
        if of.is_some_and(|i| i >= self.followers.len()) {
            anyhow::bail!("no such participant");
        }
        let width = (width / 16).max(1) * 16;
        let height = height.max(1);
        let ring = ronoctua_frames::Ring::new("ronoctua-player-view", width, height)?;
        let mut line = serde_json::to_vec(&Say::Preview {
            width,
            height,
            slots: ronoctua_frames::SLOTS,
        })?;
        line.push(b'\n');
        ring.hand_over(&stream, &line)?;
        stream.set_nonblocking(true)?;
        self.viewers.push(Viewer {
            stream,
            ring,
            of,
            owed: true,
        });
        if let Some((_, poke)) = self.pokes.iter().find(|(o, _)| *o == of) {
            poke.ring();
        }
        Ok(())
    }

    fn on_event(&mut self, mpv: &Mpv, event: Event) {
        match event {
            Event::Property { id, value } => {
                let tick = id == id::TIME;
                match (id, value) {
                    (id::PAUSE, Value::Flag(p)) => self.state.playing = !p && !self.idle,
                    (id::IDLE, Value::Flag(i)) => {
                        self.idle = i;
                        if i {
                            self.state.playing = false;
                            self.state.position = 0.0;
                        } else {
                            self.state.playing = mpv.text("pause").as_deref() == Some("no");
                        }
                    }
                    (id::TIME, Value::Float(t)) => {
                        self.state.position = t;
                        // A SPONSORED STRETCH IS JUMPED at its first frame.
                        if let Some(to) = sponsorblock::skip(t, &self.segments, &mut self.jumped) {
                            let _ = mpv.command(&["seek", &format!("{to:.3}"), "absolute+exact"]);
                        }
                    }
                    (id::TIME, _) => self.state.position = 0.0,
                    (id::DURATION, Value::Float(d)) => self.state.duration = Some(d),
                    (id::DURATION, _) => self.state.duration = None,
                    (id::VOLUME, Value::Float(v)) => {
                        self.state.volume = v.round().clamp(0.0, 130.0) as u8;
                    }
                    (id::MUTE, Value::Flag(m)) => self.state.muted = m,
                    (id::SPEED, Value::Float(s)) => self.state.speed = s,
                    (id::POS, Value::Int(p)) => {
                        self.state.current = usize::try_from(p).ok();
                        self.remember();
                    }
                    (id::PLAYLIST, _) => {
                        self.state.queue = playlist_of(mpv);
                        self.remember();
                    }
                    (id::TRACKS, _) => self.state.tracks = tracks_of(mpv),
                    (id::METADATA, _) => {
                        let tags = metadata_of(mpv);
                        let pick = |keys: &[&str]| {
                            keys.iter().find_map(|k| {
                                tags.iter()
                                    .find(|(t, _)| t.eq_ignore_ascii_case(k))
                                    .map(|(_, v)| v.clone())
                            })
                        };
                        self.state.artist = pick(&["artist", "album_artist"]);
                        self.state.album = pick(&["album"]);
                        self.find_lyrics(mpv, &tags);
                        self.ask_for_lyrics(&tags);
                    }
                    (id::TITLE, Value::Text(t)) => self.state.title = Some(t),
                    (id::TITLE, _) => self.state.title = None,
                    (id::WIDTH | id::HEIGHT, _) => {
                        let w = mpv.text("dwidth").and_then(|v| v.parse().ok());
                        let h = mpv.text("dheight").and_then(|v| v.parse().ok());
                        self.state.video = w.zip(h);
                    }
                    _ => {}
                }
                // A POSITION alone is said four times a second; anything else
                // at once.
                if !tick || self.last_tick.elapsed() >= Duration::from_millis(250) {
                    self.dirty = true;
                }
            }
            Event::EndFile {
                error: Some(why), ..
            } => {
                self.state.troubles = vec![Trouble::Unplayable { why }];
                self.dirty = true;
            }
            Event::FileLoaded => {
                if std::mem::take(&mut self.start_once) {
                    let _ = mpv.set("start", "none");
                }
                self.state.troubles.clear();
                self.state.lyrics.clear();
                self.state.lyrics_from = None;
                self.lyrics_asked = None;
                self.segments.clear();
                self.jumped.clear();
                if self.prefs.sponsorblock
                    && let Some(item) = mpv.text("path").map(|p| location_of(&p))
                    && let Some(id) = sponsorblock::video_id(&item)
                {
                    let path = sponsorblock::path(&id, &self.prefs.skipped);
                    self.fetches.push(Fetch::Segments { item, id, path });
                }
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// The location of the item playing.
    fn playing(&self) -> Option<String> {
        self.state
            .current
            .and_then(|i| self.state.queue.get(i))
            .map(|i| i.location.clone())
    }

    /// The fourth source of lyrics, when the other three had none and the
    /// person allowed it: `lrclib.net`, asked once per item.
    fn ask_for_lyrics(&mut self, tags: &[(String, String)]) {
        if !self.prefs.lyrics_online || !self.state.lyrics.is_empty() {
            return;
        }
        let Some(item) = self.playing() else { return };
        if self.lyrics_asked.as_deref() == Some(item.as_str()) {
            return;
        }
        let tag = |k: &str| {
            tags.iter()
                .find(|(t, _)| t.eq_ignore_ascii_case(k))
                .map(|(_, v)| v.clone())
        };
        // AN ARTIST AND A TITLE, or nothing is asked: a film's file name is
        // not a song, and a request made of it is a guess sent to a stranger.
        let (Some(artist), Some(title)) = (self.state.artist.clone(), tag("title")) else {
            return;
        };
        self.lyrics_asked = Some(item.clone());
        let path = lrclib::path(
            &artist,
            &title,
            self.state.album.as_deref(),
            self.state.duration,
        );
        self.fetches.push(Fetch::Lyrics { item, path });
    }

    /// The lyrics of what just loaded: a `.lrc` beside it, or its own tag.
    fn find_lyrics(&mut self, mpv: &Mpv, tags: &[(String, String)]) {
        let path = mpv
            .text("path")
            .map(|p| location_of(&p))
            .filter(|p| !p.contains("://"));
        if let Some(lrc) = path.as_deref().map(|p| Path::new(p).with_extension("lrc"))
            && let Ok(text) = std::fs::read_to_string(&lrc)
        {
            self.state.lyrics = ronoctua_player::lyrics::parse(&text);
            self.state.lyrics_from = Some("file".to_owned());
            return;
        }
        if let Some((_, text)) = tags.iter().find(|(k, _)| {
            ["lyrics", "unsyncedlyrics", "lyrics-eng", "uslt"]
                .iter()
                .any(|w| k.eq_ignore_ascii_case(w) || k.to_ascii_lowercase().starts_with("lyrics"))
        }) {
            self.state.lyrics = ronoctua_player::lyrics::parse(text);
            self.state.lyrics_from = Some("tag".to_owned());
        }
    }
}

fn apply_repeat(mpv: &Mpv, repeat: Repeat) {
    let (playlist, file) = match repeat {
        Repeat::Nothing => ("no", "no"),
        Repeat::Queue => ("inf", "no"),
        Repeat::One => ("no", "inf"),
    };
    let _ = mpv.set("loop-playlist", playlist);
    let _ = mpv.set("loop-file", file);
}

fn playlist_of(mpv: &Mpv) -> Vec<Item> {
    let Some(json) = mpv.text("playlist") else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str(&json) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|e| {
            let file = e.get("filename")?.as_str()?;
            Some(Item {
                location: location_of(file),
                title: e.get("title").and_then(|t| t.as_str()).map(str::to_owned),
            })
        })
        .collect()
}

fn tracks_of(mpv: &Mpv) -> Vec<Track> {
    let Some(json) = mpv.text("track-list") else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str(&json) else {
        return Vec::new();
    };
    let text =
        |e: &serde_json::Value, k: &str| e.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    entries
        .iter()
        .filter_map(|e| {
            Some(Track {
                id: e.get("id")?.as_i64()?,
                kind: text(e, "type")?,
                title: text(e, "title"),
                lang: text(e, "lang"),
                codec: text(e, "codec"),
                selected: e.get("selected").and_then(serde_json::Value::as_bool) == Some(true),
            })
        })
        .collect()
}

fn metadata_of(mpv: &Mpv) -> Vec<(String, String)> {
    let Some(json) = mpv.text("metadata") else {
        return Vec::new();
    };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str(&json) else {
        return Vec::new();
    };
    map.into_iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k, s.to_owned())))
        .collect()
}

/// The main mpv's events, on a thread of their own.
fn spawn_events(mpv: Arc<Mpv>, shared: Shared) {
    let _ = std::thread::Builder::new()
        .name("player-events".into())
        .spawn(move || {
            loop {
                let event = mpv.wait(1.0);
                if event == Event::Shutdown {
                    return;
                }
                let (lock, bell) = &*shared;
                let Ok(mut e) = lock.lock() else { return };
                if event != Event::None {
                    e.on_event(&mpv, event);
                }
                if e.dirty {
                    e.broadcast();
                }
                let fetches = std::mem::take(&mut e.fetches);
                drop(e);
                bell.notify_all();
                for f in fetches {
                    spawn_fetch(f, shared.clone());
                }
            }
        });
}

/// Makes one request on a thread of its own, and hands the answer to the
/// engine if the item it was for is still the one playing.
fn spawn_fetch(fetch: Fetch, shared: Shared) {
    let _ = std::thread::Builder::new()
        .name("player-fetch".into())
        .spawn(move || {
            let (host, path) = match &fetch {
                Fetch::Lyrics { path, .. } => (prefs::LRCLIB_HOST, path.clone()),
                Fetch::Segments { path, .. } => (prefs::SPONSORBLOCK_HOST, path.clone()),
            };
            let body = match get(host, &path) {
                Ok(b) => b,
                Err(e) => {
                    tracing::info!(host, error = %e, "nothing came back");
                    return;
                }
            };
            let (lock, bell) = &*shared;
            let Ok(mut e) = lock.lock() else { return };
            match fetch {
                Fetch::Lyrics { item, .. } => {
                    if e.playing().as_deref() != Some(item.as_str()) || !e.state.lyrics.is_empty() {
                        return;
                    }
                    if let Some(lines) = lrclib::parse(&body) {
                        e.state.lyrics = lines;
                        e.state.lyrics_from = Some(prefs::LRCLIB_HOST.to_owned());
                        e.broadcast();
                    }
                }
                Fetch::Segments { item, id, .. } => {
                    if e.playing().as_deref() != Some(item.as_str()) {
                        return;
                    }
                    e.segments = sponsorblock::parse(&body, &id);
                    e.jumped.clear();
                    tracing::info!(segments = e.segments.len(), "sponsored segments to skip");
                }
            }
            drop(e);
            bell.notify_all();
        });
}

/// `GET https://<host><path>`: a 200, or nothing. The 404 both services
/// answer for "nothing known about this" is an ordinary answer, not a fault.
fn get(host: &str, path: &str) -> Result<String> {
    let call = ronoctua_http::Call::new("GET", path, host)
        .with_headers(&[
            // LRCLIB asks every client to name itself.
            ("User-Agent", "ronoctua-player"),
            ("Accept", "application/json"),
        ])
        .waiting(ronoctua_http::tls::PATIENCE);
    let answer = ronoctua_http::tls::over_tls(host, 443, &call)?;
    if answer.status != 200 {
        anyhow::bail!("{host} answered {}", answer.status);
    }
    Ok(answer.body)
}

/// A participant's events: drained, so its queue never fills.
fn spawn_quiet_events(mpv: Arc<Mpv>) {
    let _ = std::thread::Builder::new()
        .name("player-follower".into())
        .spawn(move || while mpv.wait(1.0) != Event::Shutdown {});
}

/// The render thread of one mpv: when a frame is due, render it once per
/// size asked and publish it to each viewer of that mpv.
///
/// Gives back what wakes it, for the ENGINE to keep: every caller is an
/// `Engine` method, so the engine's lock is already held — taking it here
/// hung the player on the first thing it played, which the VM caught.
fn spawn_render(mpv: Arc<Mpv>, of: Option<usize>, shared: Shared) -> Result<mpv::Poke> {
    let render = Render::new(mpv)?;
    let poke = render.poke();
    std::thread::Builder::new()
        .name("player-render".into())
        .spawn(move || {
            let mut buffers: HashMap<(u32, u32), Aligned> = HashMap::new();
            loop {
                let due = render.wait(Duration::from_millis(500));
                let owed = {
                    let (lock, _) = &*shared;
                    lock.lock()
                        .is_ok_and(|e| e.viewers.iter().any(|v| v.of == of && v.owed))
                };
                if !due && !owed {
                    // A participant that left takes its render thread with it.
                    let (lock, _) = &*shared;
                    if let Some(i) = of
                        && lock.lock().is_ok_and(|e| i >= e.followers.len())
                    {
                        return;
                    }
                    continue;
                }
                let sizes: Vec<(u32, u32)> = {
                    let (lock, _) = &*shared;
                    let Ok(e) = lock.lock() else { return };
                    let mut s: Vec<(u32, u32)> = e
                        .viewers
                        .iter()
                        .filter(|v| v.of == of)
                        .map(|v| v.ring.size())
                        .collect();
                    s.sort_unstable();
                    s.dedup();
                    s
                };
                if sizes.is_empty() {
                    // Nobody is looking, and the frame is still taken — the
                    // renderer would otherwise hold the decoder back.
                    let one = buffers.entry((16, 16)).or_default();
                    let _ = render.render(16, 16, one);
                    continue;
                }
                for &(w, h) in &sizes {
                    let buf = buffers.entry((w, h)).or_default();
                    if let Err(e) = render.render(w, h, buf) {
                        tracing::debug!(error = %e, "a frame was not rendered");
                    }
                }
                let (lock, _) = &*shared;
                let Ok(mut e) = lock.lock() else { return };
                e.viewers.retain_mut(|v| {
                    if v.of != of {
                        return true;
                    }
                    let (w, h) = v.ring.size();
                    let Some(buf) = buffers.get(&(w, h)) else {
                        return true;
                    };
                    let len = w as usize * h as usize * 4;
                    let Ok(slot) = v.ring.publish(buf.get(len)) else {
                        return false;
                    };
                    v.owed = false;
                    // ONE BYTE, so a write is whole or nothing; nothing
                    // written is a viewer that went away.
                    match v.stream.write(&[slot]) {
                        Ok(0) => false,
                        Ok(_) => true,
                        Err(err) => err.kind() == std::io::ErrorKind::WouldBlock,
                    }
                });
            }
        })
        .context("the render thread")?;
    Ok(poke)
}

/// Keeps every participant in step with the main mpv.
fn spawn_sync(shared: Shared) {
    let _ = std::thread::Builder::new()
        .name("player-sync".into())
        .spawn(move || {
            loop {
                std::thread::sleep(sync::TICK);
                let (lock, _) = &*shared;
                let Ok(mut e) = lock.lock() else { return };
                if e.followers.is_empty() {
                    return;
                }
                let Some(main) = e.mpv.clone() else { continue };
                let Some(master) = main.float("time-pos") else {
                    continue;
                };
                let speed = main.float("speed").unwrap_or(1.0);
                let paused = main.text("pause").as_deref() == Some("yes");
                for f in &mut e.followers {
                    let _ = f.mpv.set("pause", if paused { "yes" } else { "no" });
                    let Some(at) = f.mpv.float("time-pos") else {
                        continue;
                    };
                    f.drift_ms = ((at - master) * 1000.0).round() as i64;
                    match sync::correct(master, at, speed) {
                        Correction::Keep => {
                            let _ = f.mpv.set("speed", &speed.to_string());
                        }
                        Correction::Speed(s) => {
                            let _ = f.mpv.set("speed", &s.to_string());
                        }
                        Correction::Seek(to) => {
                            let _ = f
                                .mpv
                                .command(&["seek", &to.to_string(), "absolute", "exact"]);
                        }
                    }
                }
                e.dirty = true;
            }
        });
}

/// One connection.
pub fn serve_one(stream: UnixStream, shared: &Shared) {
    let Ok(read_half) = stream.try_clone() else {
        return;
    };
    let mut input = BufReader::new(read_half);
    let ask: Ask = match wire::read_line(&mut input) {
        Ok(Some(a)) => a,
        _ => return,
    };
    // PAIRING TALKS TO ANOTHER MACHINE, and a host on the tailnet is given
    // seconds: the engine's lock is not held across it.
    if let Ask::PairWith { to, code } = &ask {
        let say = match crate::remote::pair_with(to, code, shared) {
            Ok(_) => Say::Done,
            Err(err) => Say::Refused(Trouble::Remote {
                why: format!("{err:#}"),
            }),
        };
        let mut out = stream;
        let _ = wire::write_line(&mut out, &say);
        let (lock, bell) = &**shared;
        if let Ok(mut e) = lock.lock() {
            e.broadcast();
        }
        bell.notify_all();
        return;
    }
    let (lock, bell) = &**shared;
    let Ok(mut e) = lock.lock() else { return };
    match ask {
        Ask::Watch => {
            let mut s = stream;
            if wire::write_line(&mut s, &Say::State(Box::new(e.state()))).is_ok() {
                e.watchers.push(s);
            }
        }
        Ask::Look => {
            let mut out = stream;
            let _ = wire::write_line(&mut out, &Say::State(Box::new(e.state())));
        }
        Ask::Preview { width, height, of } => {
            // A PREVIEW WANTS A PICTURE, so it wants the engine up.
            if of.is_none()
                && let Err(err) = e.main(shared)
            {
                let mut out = stream;
                let _ = wire::write_line(
                    &mut out,
                    &Say::Refused(Trouble::NoEngine {
                        why: format!("{err:#}"),
                    }),
                );
                return;
            }
            if let Err(err) = e.add_viewer(stream, width, height, of) {
                tracing::warn!(error = %err, "a preview was refused");
            }
        }
        other => {
            let say = e.ask(other, shared);
            e.broadcast();
            let mut out = stream;
            let _ = wire::write_line(&mut out, &say);
        }
    }
    drop(e);
    bell.notify_all();
}

/// Where a path the person gave is played from: absolute, so the queue means
/// the same file whoever reads it.
#[must_use]
pub fn absolute(item: &str) -> String {
    if item.contains("://") {
        return item.to_owned();
    }
    let p = PathBuf::from(item);
    std::fs::canonicalize(&p)
        .unwrap_or(p)
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::expanded;

    /// FAILS IF: a list libmpv cannot read reaches it whole, or one it CAN read
    /// is opened here and loses its laziness.
    /// FAILS IF: a name only libgme reads reaches libmpv without the demuxer
    /// named, and is refused as an unrecognized format — or the option
    /// leaks onto a file that libmpv finds on its own.
    #[test]
    fn the_music_of_consoles_names_its_demuxer() {
        let gme = Some("demuxer-lavf-format=libgme");
        assert_eq!(super::load_options("/m/tune.vgm"), gme);
        assert_eq!(super::load_options("/m/Zelda.SPC"), gme);
        assert_eq!(
            super::load_options("http://100.1.2.3:45517/m/a.nsf?k=1"),
            gme
        );
        assert_eq!(super::load_options("/m/song.flac"), None);
        assert_eq!(super::load_options("/m/tune.sid"), None);
    }

    #[test]
    fn the_lists_libmpv_does_not_know_are_opened_here() {
        let dir = std::env::temp_dir().join(format!("ronoctua-expand-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let xspf = dir.join("l.xspf");
        std::fs::write(
            &xspf,
            "<playlist version=\"1\" xmlns=\"http://xspf.org/ns/0/\"><trackList>\
             <track><location>one.flac</location></track>\
             <track><location>http://192.0.2.1/two.ogg</location></track>\
             </trackList></playlist>",
        )
        .unwrap();
        let m3u = dir.join("l.m3u").to_string_lossy().into_owned();
        let broken = dir.join("missing.asx").to_string_lossy().into_owned();
        let out = expanded(vec![
            xspf.to_string_lossy().into_owned(),
            m3u.clone(),
            broken.clone(),
        ]);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(
            out,
            vec![
                dir.join("one.flac").to_string_lossy().into_owned(),
                "http://192.0.2.1/two.ogg".to_owned(),
                m3u,
                broken,
            ]
        );
    }
}
