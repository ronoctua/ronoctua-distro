//! A voice: one mpv, the thread that draws its pictures and the thread that
//! hears its events — and what the voice was measured to hold.
//!
//! # Every voice is opened the same way
//!
//! With sound and with a render context, always ([`Voice::open`]): a voice
//! that plays only a picture is told `aid=no`, one that plays only a sound
//! `vid=no`. So any voice can take any part of any set, and the one that held
//! a file keeps holding it when the file is played together with another —
//! nothing is reloaded that is already loaded.
//!
//! # Each ends by its own flag
//!
//! Its two threads stop on the voice's own stop flag and nothing else: not
//! an index into a list that shifts when a voice leaves. A voice is ended OFF
//! the engine's lock ([`retire`]): both threads take the lock, and joining
//! them under it would wait for ever. The render thread holds the render
//! context and frees it on its way out, before the reaper lets the last
//! handle to the mpv go — the order libmpv requires.

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use ronoctua_player::{
    own::LaneId,
    session,
    wire::{Track, TrackKind},
};

use super::{Shared, id, spawn_fetch};
use crate::mpv::{self, Aligned, Event, Mpv, Render};

/// Every property a voice reports, under the ids of [`id`] — the same list
/// for every voice.
const OBSERVED: [(u64, &str, std::ffi::c_int); 15] = [
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
    (id::EOF, "eof-reached", mpv::FORMAT_FLAG),
];

/// One voice.
pub(crate) struct Voice {
    /// Its name for the daemon's life, never reused: its threads know their
    /// voice by it, and a voice that left is one no thread finds.
    pub serial: u64,
    /// Its mpv.
    pub mpv: Arc<Mpv>,
    /// Raised to end its threads.
    stop: Arc<AtomicBool>,
    /// Wakes its render thread.
    pub poke: mpv::Poke,
    /// Its render and events threads, joined by [`retire`].
    threads: Vec<JoinHandle<()>>,
    /// What is loaded in it, as a lane writes a location; `None` with
    /// nothing loaded.
    pub loaded: Option<String>,
    /// The part of a set it plays; `None` while it plays the queue's window
    /// (the first voice, with nothing played together) or nothing.
    pub part: Option<session::Voice>,
    /// Its source's length, once known.
    pub duration: Option<f64>,
    /// Its picture's size, while it shows one.
    pub size: Option<(u32, u32)>,
    /// Its source's tracks.
    pub tracks: Vec<Track>,
    /// It reached the end of its file (and, kept open, waits there).
    pub eof: bool,
    /// Its file could not be played, and mpv's words for why.
    pub failed: Option<String>,
    /// Until when it is SEEKED rather than nudged: a load or a seek just
    /// happened (`sync::SETTLING`).
    pub settling_until: Option<Instant>,
    /// How far from the set's clock it was last measured, milliseconds.
    pub drift_ms: i64,
    /// Properties as last set on it, so the same value is never set twice —
    /// setting a track again can reopen it.
    applied: HashMap<&'static str, String>,
    /// Subtitle files added to it, by location, and the id mpv gave each.
    pub added: Vec<(String, i64)>,
    /// What mpv's parts that load a file said of it since it started
    /// loading — the `ytdl_hook` (yt-dlp's words) and `cplayer`, errors
    /// only, the last [`HEARD_MOST`] ([`keeps`]).
    pub heard: std::collections::VecDeque<String>,
    /// What the program behind mpv said when this file would not play: the
    /// last `ERROR:` line [`said_of`] found in [`Voice::heard`].
    pub said: Option<String>,
    /// mpv said FILE_LOADED for what it holds: its tracks are what the
    /// file has, and a sound it lacks is known to be lacking.
    pub file_loaded: bool,
}

/// How many lines of mpv's log a voice keeps of the file it loads.
pub(super) const HEARD_MOST: usize = 8;

/// The least level of mpv's log a voice asks for: nothing below what
/// [`keeps`] keeps is sent at all.
const LOG_LEVEL: &str = "error";

/// The longest line of a program's words kept, in characters.
pub(super) const SAID_MOST_CHARS: usize = 400;

/// Whether a line of mpv's log is one a voice keeps: an error of the part
/// that asks yt-dlp (`ytdl_hook`) or of the player's core (`cplayer`), which
/// is where a file that will not load says why. Pure.
#[must_use]
pub(super) fn keeps(prefix: &str, level: &str) -> bool {
    matches!(prefix, "ytdl_hook" | "cplayer") && matches!(level, "error" | "fatal")
}

/// What yt-dlp said of a file that would not play: the last `ERROR:` line
/// among what was heard, from its words on, `ERROR:` itself left out, cut at
/// [`SAID_MOST_CHARS`]. `None` when it said none. Pure.
///
/// The hook says yt-dlp's whole standard error in ONE `msg.error`, and mpv's
/// client API hands it over a line per event (`mpv_event_log_message.text`
/// is one line since API 1.6): so each heard line is a line — and a message
/// that still holds several is read line by line all the same.
#[must_use]
pub(super) fn said_of<'a>(heard: impl DoubleEndedIterator<Item = &'a String>) -> Option<String> {
    heard.rev().find_map(|message| {
        message.lines().rev().find_map(|line| {
            let (_, words) = line.split_once("ERROR:")?;
            let words = words.trim();
            (!words.is_empty()).then(|| words.chars().take(SAID_MOST_CHARS).collect())
        })
    })
}

impl Voice {
    /// A voice with nothing loaded: its mpv, its render context and both
    /// its threads.
    ///
    /// THE RENDER CONTEXT FIRST, before anything is loaded: `vo=libmpv`
    /// looks for it when a video starts, and with none it drops the video for
    /// good — measured, a second voice that played and never showed a frame.
    pub(super) fn open(serial: u64, shared: &Shared) -> Result<Self> {
        let mpv = super::open_mpv()?;
        for (id, name, format) in OBSERVED {
            mpv.observe(id, name, format)?;
        }
        // WHY A FILE WOULD NOT PLAY is said in mpv's log and nowhere else —
        // yt-dlp's `ERROR:` line comes through the ytdl hook's log — so a
        // voice listens to it, from ERRORS up: what it keeps ([`keeps`]),
        // and not the warnings a broken stream says many times a second.
        mpv.request_log_messages(LOG_LEVEL)?;
        let stop = Arc::new(AtomicBool::new(false));
        let (render, poke) = spawn_render(mpv.clone(), serial, stop.clone(), shared.clone())?;
        let events = spawn_events(mpv.clone(), serial, stop.clone(), shared.clone())?;
        Ok(Self {
            serial,
            mpv,
            stop,
            poke,
            threads: vec![render, events],
            loaded: None,
            part: None,
            duration: None,
            size: None,
            tracks: Vec::new(),
            eof: false,
            failed: None,
            settling_until: None,
            drift_ms: 0,
            applied: HashMap::new(),
            added: Vec::new(),
            heard: std::collections::VecDeque::new(),
            said: None,
            file_loaded: false,
        })
    }

    /// A line of mpv's log: kept when it is one [`keeps`] names, the oldest
    /// going past [`HEARD_MOST`].
    pub(super) fn hear(&mut self, prefix: &str, level: &str, text: &str) {
        if !keeps(prefix, level) {
            return;
        }
        if self.heard.len() == HEARD_MOST {
            self.heard.pop_front();
        }
        self.heard.push_back(text.trim_end().to_owned());
    }

    /// A new file starts: nothing heard of the last one is about this one.
    pub(super) fn starting(&mut self) {
        self.heard.clear();
        self.said = None;
        self.file_loaded = false;
    }

    /// Sets `name` to `value` unless it already is.
    pub(super) fn set(&mut self, name: &'static str, value: &str) {
        if self.applied.get(name).map(String::as_str) == Some(value) {
            return;
        }
        match self.mpv.set(name, value) {
            Ok(()) => {
                self.applied.insert(name, value.to_owned());
            }
            Err(e) => tracing::debug!(error = %e, name, value, "a voice would not take a property"),
        }
    }

    /// Notes that `name` IS `value` — mpv set it itself, as `sub-add …
    /// select` sets the subtitle track — so a later [`Voice::set`] of the
    /// same value is not taken for news, and one of another value is.
    pub(super) fn assume(&mut self, name: &'static str, value: &str) {
        self.applied.insert(name, value.to_owned());
    }

    /// Forgets what was set, for a voice that goes back to playing the
    /// queue's window as mpv's own defaults.
    pub(super) fn forget_applied(&mut self) {
        self.applied.clear();
    }

    /// A new file is loading in it: nothing measured of the last one is
    /// true of this one.
    pub(super) fn loading(&mut self, location: &str) {
        self.loaded = Some(location.to_owned());
        self.duration = None;
        self.size = None;
        self.tracks.clear();
        self.eof = false;
        self.failed = None;
        self.file_loaded = false;
        self.added.clear();
        self.applied.remove("sid");
        self.applied.remove("secondary-sid");
        self.applied.remove("sub-delay");
        self.applied.remove("secondary-sub-delay");
        self.settle();
    }

    /// It was loaded or sought: seeks, not nudges, for a while.
    pub(super) fn settle(&mut self) {
        self.settling_until = Some(Instant::now() + ronoctua_player::sync::SETTLING);
    }

    /// Whether it is settling now.
    pub(super) fn settling(&self) -> bool {
        self.settling_until.is_some_and(|u| Instant::now() < u)
    }

    /// Whether its source has a cover among its tracks.
    pub(super) fn has_cover(&self) -> bool {
        self.tracks
            .iter()
            .any(|t| t.kind == TrackKind::Video && t.albumart)
    }

    /// Whether it plays `lane`'s picture or sound.
    pub(super) fn plays(&self, lane: LaneId) -> bool {
        self.part
            .as_ref()
            .is_some_and(|p| p.video == Some(lane) || p.audio == Some(lane))
    }
}

/// Ends a voice, off the engine's lock: its threads are told, its mpv is
/// told to quit (which wakes its events thread at once), and a reaper joins
/// both threads and lets the mpv go last. The render thread frees its render
/// context on its way out — before the mpv, as libmpv requires.
pub(super) fn retire(voice: Voice) {
    voice.stop.store(true, Ordering::SeqCst);
    voice.poke.ring();
    let _ = voice.mpv.command(&["quit"]);
    let Voice {
        mpv,
        threads,
        serial,
        ..
    } = voice;
    let _ = std::thread::Builder::new()
        .name("player-reaper".into())
        .spawn(move || {
            for thread in threads {
                let _ = thread.join();
            }
            // THE LAST HANDLE, here: `mpv_terminate_destroy` waits for the
            // core, and nobody waits for this thread.
            drop(mpv);
            tracing::debug!(serial, "a voice left");
        });
}

/// A voice's events, on a thread of their own, until its stop flag.
fn spawn_events(
    mpv: Arc<Mpv>,
    serial: u64,
    stop: Arc<AtomicBool>,
    shared: Shared,
) -> Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("player-events".into())
        .spawn(move || {
            loop {
                let event = mpv.wait(1.0);
                if event == Event::Shutdown || stop.load(Ordering::SeqCst) {
                    return;
                }
                // A LINE OF THE LOG NOBODY KEEPS — a decoder's error on a
                // broken stream, many a second — never takes the lock.
                if let Event::Log { prefix, level, .. } = &event
                    && !keeps(prefix, level)
                {
                    continue;
                }
                let (lock, bell) = &*shared;
                let Ok(mut e) = lock.lock() else { return };
                if event != Event::None {
                    e.on_voice_event(serial, &mpv, event);
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
        })
        .context("a voice's events thread")
}

/// A voice's render thread: when a frame is due, render it once per size
/// asked and publish it to every viewer of a picture this voice draws.
///
/// Gives back what wakes it, for the engine to keep: a voice is made under
/// the engine's lock, so taking it here would hang.
fn spawn_render(
    mpv: Arc<Mpv>,
    serial: u64,
    stop: Arc<AtomicBool>,
    shared: Shared,
) -> Result<(JoinHandle<()>, mpv::Poke)> {
    let render = Render::new(mpv)?;
    let poke = render.poke();
    let thread = std::thread::Builder::new()
        .name("player-render".into())
        .spawn(move || {
            let mut buffers: HashMap<(u32, u32), Aligned> = HashMap::new();
            loop {
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let due = render.wait(Duration::from_millis(500));
                if stop.load(Ordering::SeqCst) {
                    return;
                }
                let (owed, sizes) = {
                    let (lock, _) = &*shared;
                    let Ok(e) = lock.lock() else { return };
                    let mine = |v: &super::Viewer| e.draws(serial, v.lane);
                    let owed = e.viewers.iter().any(|v| v.owed && mine(v));
                    let mut sizes: Vec<(u32, u32)> = e
                        .viewers
                        .iter()
                        .filter(|v| mine(v))
                        .map(|v| v.ring.size())
                        .collect();
                    sizes.sort_unstable();
                    sizes.dedup();
                    (owed, sizes)
                };
                if !due && !owed {
                    continue;
                }
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
                let drawn = e.drawn.clone();
                e.viewers.retain_mut(|v| {
                    if !drawn.contains(&(v.lane, serial)) {
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
                    match std::io::Write::write(&mut v.stream, &[slot]) {
                        Ok(0) => false,
                        Ok(_) => true,
                        Err(err) => err.kind() == std::io::ErrorKind::WouldBlock,
                    }
                });
            }
        })
        .context("a voice's render thread")?;
    Ok((thread, poke))
}

/// The tracks mpv lists for what is loaded.
pub(super) fn tracks_of(mpv: &Mpv) -> Vec<Track> {
    mpv.text("track-list")
        .map(|json| tracks_in(&json))
        .unwrap_or_default()
}

/// mpv's `track-list`, read. A kind this does not know is left out, never
/// guessed at.
pub(super) fn tracks_in(json: &str) -> Vec<Track> {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    let text =
        |e: &serde_json::Value, k: &str| e.get(k).and_then(|v| v.as_str()).map(str::to_owned);
    let flag = |e: &serde_json::Value, k: &str| {
        e.get(k).and_then(serde_json::Value::as_bool) == Some(true)
    };
    entries
        .iter()
        .filter_map(|e| {
            let kind = match e.get("type")?.as_str()? {
                "video" => TrackKind::Video,
                "audio" => TrackKind::Audio,
                "sub" => TrackKind::Sub,
                _ => return None,
            };
            Some(Track {
                id: e.get("id")?.as_i64()?,
                kind,
                title: text(e, "title"),
                lang: text(e, "lang"),
                codec: text(e, "codec"),
                selected: flag(e, "selected"),
                albumart: flag(e, "albumart"),
                default: flag(e, "default"),
                forced: flag(e, "forced"),
                external: flag(e, "external"),
            })
        })
        .collect()
}

/// The id of the subtitle track mpv made of the file at `location`: the
/// newest external one naming it.
pub(super) fn added_track(json: &str, location: &str) -> Option<i64> {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str(json) else {
        return None;
    };
    entries
        .iter()
        .filter(|e| {
            e.get("type").and_then(|t| t.as_str()) == Some("sub")
                && e.get("external-filename").and_then(|f| f.as_str()) == Some(location)
        })
        .filter_map(|e| e.get("id")?.as_i64())
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: yt-dlp's `ERROR:` line is not what a refusal quotes — mpv
    /// hands the hook's message over a line per event, warnings and all, and
    /// then the hook's own summary — or a warning, another part of mpv, or
    /// an error line with nothing after it is taken for it; or a voice asks
    /// mpv for more of its log than it keeps.
    #[test]
    fn what_yt_dlp_said_is_its_last_error_line() {
        // THE FIXTURE: what mpv 0.41's `ytdl_hook.lua` logs when yt-dlp
        // fails — its standard error trimmed, said in one `msg.error` and
        // handed to a client a LINE PER EVENT (client.h: one line since API
        // 1.6) — then the hook's own line, and what the core says after.
        let sorry = "[youtube] dQw4w9WgXcQ: Sign in to confirm you\u{2019}re not a bot. \
                     Use --cookies-from-browser or --cookies for the authentication.";
        let error = format!("ERROR: {sorry}\n");
        let log = [
            (
                "ffmpeg/demuxer",
                "error",
                "hls: stream 0: segment unavailable\n",
            ),
            (
                "ytdl_hook",
                "error",
                "WARNING: [youtube] dQw4w9WgXcQ: nsig extraction failed\n",
            ),
            ("ytdl_hook", "error", error.as_str()),
            (
                "ytdl_hook",
                "error",
                "youtube-dl failed: unexpected error occurred\n",
            ),
            ("ytdl_hook", "warn", "ERROR: a warning level is not kept\n"),
            ("cplayer", "error", "Failed to recognize file format.\n"),
        ];
        let mut heard = std::collections::VecDeque::new();
        for (prefix, level, text) in log {
            if keeps(prefix, level) {
                heard.push_back(text.trim_end().to_owned());
            }
        }
        assert_eq!(heard.len(), 4, "{heard:?}");
        assert_eq!(said_of(heard.iter()).as_deref(), Some(sorry));
        // THE SAME, SAID AS ONE MESSAGE of several lines: read line by line.
        let whole = [
            format!("WARNING: [youtube] dQw4w9WgXcQ: nsig extraction failed\n{error}"),
            "youtube-dl failed: unexpected error occurred".to_owned(),
        ];
        assert_eq!(said_of(whole.iter()).as_deref(), Some(sorry));
        // WHAT IS ASKED OF mpv is what is kept: nothing below an error.
        assert!(keeps("ytdl_hook", LOG_LEVEL));
        assert!(!keeps("ytdl_hook", "warn"));
        // NOTHING SAID IS NONE, and an `ERROR:` with nothing after it is none.
        assert_eq!(said_of(std::iter::empty()), None);
        let bare = ["ERROR:  ".to_owned()];
        assert_eq!(said_of(bare.iter()), None);
        // A LONG ONE is cut, by characters.
        let long = [format!("ERROR: {}", "ç".repeat(SAID_MOST_CHARS * 2))];
        assert_eq!(
            said_of(long.iter()).map(|s| s.chars().count()),
            Some(SAID_MOST_CHARS)
        );
    }

    /// FAILS IF: mpv's list of tracks stops being read as the kinds the wire
    /// names, a cover stops being told from a film, or a kind nobody named
    /// is guessed into one.
    #[test]
    fn a_track_list_is_read_as_kinds_and_covers() {
        let json = r#"[
            {"id":1,"type":"video","albumart":true,"selected":true,"codec":"mjpeg"},
            {"id":1,"type":"audio","lang":"por","selected":true,"default":true},
            {"id":2,"type":"audio","lang":"eng","title":"Commentary"},
            {"id":1,"type":"sub","forced":true,"external":true,
             "external-filename":"/m/a.srt"},
            {"id":3,"type":"data"}
        ]"#;
        let tracks = tracks_in(json);
        assert_eq!(tracks.len(), 4, "{tracks:#?}");
        assert!(tracks[0].albumart && tracks[0].kind == TrackKind::Video);
        assert!(tracks[1].default && tracks[1].selected);
        assert_eq!(tracks[2].title.as_deref(), Some("Commentary"));
        assert!(tracks[3].forced && tracks[3].external && tracks[3].kind == TrackKind::Sub);
        assert_eq!(added_track(json, "/m/a.srt"), Some(1));
        assert_eq!(added_track(json, "/m/b.srt"), None);
        assert!(tracks_in("not json").is_empty());
    }
}
