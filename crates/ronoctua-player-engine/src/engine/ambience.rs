//! The ambient sounds' mpvs: one for each sound that is on, looping its
//! file by itself, apart from the voices and from the queue.
//!
//! The rules — which sound is on, how loud, whether they are held, the
//! ceiling — are `ronoctua_player::ambience`'s mixer; this carries out what it
//! decides ([`Effect`]) and tells it what the mpvs did.
//!
//! # Apart from everything the queue is told
//!
//! No voice plays a sound and no sound is a voice: stop, next, the media keys
//! and the player's own volume reach the voices and never these. A sound
//! ends when it is turned off, when every sound is silenced, or when the
//! player closes ([`Ambience::end`]).
//!
//! # Round by itself
//!
//! mpv loops the file (`loop-file=inf`): `nix/ambience.nix` played each loop
//! three times round that way with the pinned mpv and got it back sample for
//! sample, so nothing here has to follow the seam. Each time round is a seek
//! mpv makes on its own — nothing else ever seeks a sound — and is counted
//! ([`Sounding::rounds`]).
//!
//! [`Sounding::rounds`]: ronoctua_player::wire::Sounding::rounds

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

use anyhow::{Context as _, Result};
use ronoctua_player::{
    ambience::{self, Effect, Mixer},
    wire::{self, Trouble},
};
use serde::{Deserialize, Serialize};

use super::{Engine, Refused, Shared};
use crate::mpv::{Event, Mpv};

/// The file the sounds' volumes are kept in, in the state directory — only
/// the volumes: a sound that was on is never on again by itself.
const REMEMBERED: &str = "player-ambience.json";

/// The version of [`REMEMBERED`] written now.
const REMEMBERED_VERSION: u32 = 1;

/// What is remembered of the sounds.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Remembered {
    version: u32,
    /// Each sound's volume, by id.
    volumes: BTreeMap<String, u8>,
}

fn remembered() -> ronoctua_store::Store<Remembered> {
    ronoctua_store::Store::at(ronoctua_store::state_dir(), REMEMBERED)
}

/// A sound that is on: its mpv and the thread that hears it.
struct Player {
    id: String,
    /// Its name for the daemon's life, never reused: a thread that hears a
    /// player that left finds no player by it.
    serial: u64,
    mpv: Arc<Mpv>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

/// The ambient sounds: the mixer, and an mpv for each sound that is on.
pub(crate) struct Ambience {
    mixer: Mixer,
    players: Vec<Player>,
    serials: u64,
}

impl Ambience {
    /// The sounds this machine has, all off, each at the volume it was left
    /// at. Nothing plays until somebody chooses a sound.
    pub(crate) fn open() -> Self {
        let volumes = remembered().read().volumes;
        Self {
            mixer: Mixer::new(ambience::from_env(), &volumes),
            players: Vec::new(),
            serials: 0,
        }
    }

    /// What the state says of them.
    pub(crate) fn state(&self) -> wire::Ambience {
        self.mixer.state()
    }

    /// Every sound ends: the player is closing.
    pub(crate) fn end(&mut self) {
        for p in self.players.drain(..) {
            retire(p);
        }
    }

    fn remember(&self) {
        let r = Remembered {
            version: REMEMBERED_VERSION,
            volumes: self.mixer.volumes(),
        };
        if let Err(e) = remembered().write(&r) {
            tracing::warn!(error = %e, "could not remember the ambient sounds' volumes");
        }
    }
}

/// What every ambient sound's mpv is started with: sound and nothing else,
/// looping its one file for ever.
fn options(volume_percent: u8) -> Vec<(&'static str, String)> {
    let mut o: Vec<(&'static str, String)> = [
        ("vo", "null"),
        ("vid", "no"),
        ("audio-display", "no"),
        ("terminal", "no"),
        ("config", "no"),
        ("input-default-bindings", "no"),
        ("input-vo-keyboard", "no"),
        ("idle", "yes"),
        // NONE OF mpv'S OWN SCRIPTS: each is a Lua state of its own, and a
        // loop of rain needs no controller, console or menu. Nor a media
        // control of its own: the media keys are the queue's.
        ("osc", "no"),
        ("ytdl", "no"),
        ("load-scripts", "no"),
        ("load-stats-overlay", "no"),
        ("load-console", "no"),
        ("load-auto-profiles", "no"),
        ("load-select", "no"),
        ("load-positioning", "no"),
        ("load-commands", "no"),
        ("load-context-menu", "no"),
        ("media-controls", "no"),
        ("sub-auto", "no"),
        ("audio-file-auto", "no"),
        ("cover-art-auto", "no"),
        // ITS OWN NAME for the sound server, so its volume is its own there
        // too — apart from the queue's `ronoctua`.
        ("audio-client-name", "ronoctua-ambience"),
        ("audio-fallback-to-null", "yes"),
        // ROUND BY ITSELF: measured exact across the seam
        // (`ambience::ROUTE`).
        ("loop-file", "inf"),
    ]
    .into_iter()
    .map(|(k, v)| (k, v.to_owned()))
    .collect();
    o.push(("volume", volume_percent.to_string()));
    // ONE FILE, ROUND AND ROUND: its outputs are asked once, when it opens
    // (`super::outputs` says why).
    if let Some(list) = super::outputs() {
        o.push(("ao", list.to_owned()));
    }
    o
}

/// An mpv playing `path` round and round.
fn open_player(path: &Path, volume_percent: u8) -> Result<Arc<Mpv>> {
    let owned = options(volume_percent);
    let pairs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let mpv = Arc::new(Mpv::new(&pairs)?);
    let location = path.to_string_lossy();
    mpv.command(&["loadfile", &location, "replace"])?;
    Ok(mpv)
}

/// A sound's mpv ends, off the engine's lock: its thread is told, its mpv is
/// told to quit (which wakes the thread at once), and a reaper joins the
/// thread and lets the mpv go last — `mpv_terminate_destroy` waits for the
/// core, and nobody waits for the reaper.
fn retire(mut p: Player) {
    p.stop.store(true, Ordering::SeqCst);
    let _ = p.mpv.command(&["quit"]);
    let thread = p.thread.take();
    let Player { mpv, serial, .. } = p;
    let _ = std::thread::Builder::new()
        .name("player-ambience-reaper".into())
        .spawn(move || {
            if let Some(t) = thread {
                let _ = t.join();
            }
            drop(mpv);
            tracing::debug!(serial, "an ambient sound left");
        });
}

/// What a sound's mpv does, on a thread of its own until its stop flag: each
/// seek is a time round, and a file that will not play turns the sound off.
fn spawn_events(
    mpv: Arc<Mpv>,
    serial: u64,
    stop: Arc<AtomicBool>,
    shared: Shared,
) -> Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("player-ambience".into())
        .spawn(move || {
            loop {
                let event = mpv.wait(1.0);
                if event == Event::Shutdown || stop.load(Ordering::SeqCst) {
                    return;
                }
                let (lock, bell) = &*shared;
                match event {
                    Event::Seek => {
                        let Ok(mut e) = lock.lock() else { return };
                        e.ambient_went_round(serial);
                        if e.dirty {
                            e.broadcast();
                        }
                    }
                    Event::EndFile {
                        error: Some(why), ..
                    } => {
                        let Ok(mut e) = lock.lock() else { return };
                        e.ambient_failed(serial, why);
                        e.broadcast();
                    }
                    _ => continue,
                }
                bell.notify_all();
            }
        })
        .context("an ambient sound's events thread")
}

impl Engine {
    /// Turns an ambient sound on or off ([`wire::Ask::Ambient`]).
    pub(super) fn ambient(&mut self, id: &str, on: bool, shared: &Shared) -> Result<(), Refused> {
        let effects = self.ambience.mixer.turn(id, on)?;
        self.carry_ambience(effects, shared)
    }

    /// A sound's own volume ([`wire::Ask::AmbientVolume`]), kept.
    pub(super) fn ambient_volume(
        &mut self,
        id: &str,
        percent: u8,
        shared: &Shared,
    ) -> Result<(), Refused> {
        let effects = self.ambience.mixer.volume(id, percent)?;
        self.ambience.remember();
        self.carry_ambience(effects, shared)
    }

    /// Every sound off ([`wire::Ask::AmbientSilence`]).
    pub(super) fn ambient_silence(&mut self, shared: &Shared) -> Result<(), Refused> {
        let effects = self.ambience.mixer.silence();
        self.carry_ambience(effects, shared)
    }

    /// Holds the sounds that are on, or lets them go on
    /// ([`wire::Ask::AmbientToggle`]).
    pub(super) fn ambient_toggle(&mut self, shared: &Shared) -> Result<(), Refused> {
        let effects = self.ambience.mixer.toggle()?;
        self.carry_ambience(effects, shared)
    }

    /// What the mixer decided, done to the mpvs.
    fn carry_ambience(&mut self, effects: Vec<Effect>, shared: &Shared) -> Result<(), Refused> {
        self.dirty = true;
        for effect in effects {
            match effect {
                Effect::Start {
                    id,
                    path,
                    volume_percent,
                } => {
                    let started = open_player(&path, volume_percent).and_then(|mpv| {
                        self.ambience.serials += 1;
                        let serial = self.ambience.serials;
                        let stop = Arc::new(AtomicBool::new(false));
                        match spawn_events(mpv.clone(), serial, stop.clone(), shared.clone()) {
                            Ok(thread) => Ok(Player {
                                id: id.clone(),
                                serial,
                                mpv,
                                stop,
                                thread: Some(thread),
                            }),
                            Err(e) => {
                                let _ = mpv.command(&["quit"]);
                                Err(e)
                            }
                        }
                    });
                    match started {
                        Ok(p) => self.ambience.players.push(p),
                        Err(e) => {
                            self.ambience.mixer.failed(&id);
                            return Err(Trouble::SoundUnplayable {
                                id,
                                why: format!("{e:#}"),
                            }
                            .into());
                        }
                    }
                }
                Effect::Stop { id } => {
                    if let Some(k) = self.ambience.players.iter().position(|p| p.id == id) {
                        retire(self.ambience.players.remove(k));
                    }
                }
                Effect::Volume { id, percent } => {
                    if let Some(p) = self.ambience.players.iter().find(|p| p.id == id) {
                        p.mpv.set("volume", &percent.to_string())?;
                    }
                }
                Effect::Hold(held) => {
                    for p in &self.ambience.players {
                        p.mpv.set("pause", if held { "yes" } else { "no" })?;
                    }
                }
            }
        }
        Ok(())
    }

    /// The sound `serial` went round once more — when it is still the one
    /// playing under that id.
    fn ambient_went_round(&mut self, serial: u64) {
        if let Some(p) = self.ambience.players.iter().find(|p| p.serial == serial) {
            let id = p.id.clone();
            self.ambience.mixer.went_round(&id);
            self.dirty = true;
        }
    }

    /// The sound `serial` would not play: it is off, and why is said.
    fn ambient_failed(&mut self, serial: u64, why: String) {
        let Some(k) = self
            .ambience
            .players
            .iter()
            .position(|p| p.serial == serial)
        else {
            return;
        };
        let p = self.ambience.players.remove(k);
        self.ambience.mixer.failed(&p.id);
        tracing::warn!(id = p.id, why, "an ambient sound would not play");
        self.state.troubles = vec![Trouble::SoundUnplayable {
            id: p.id.clone(),
            why,
        }];
        retire(p);
        self.dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: an ambient sound's mpv is started with a picture, a script
    /// of mpv's own, a media control of its own, the queue's name for the
    /// sound server, or without going round by itself.
    #[test]
    fn a_sound_s_mpv_is_sound_alone_and_goes_round_by_itself() {
        let o = options(70);
        let get = |k: &str| {
            o.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.as_str())
                .unwrap_or_else(|| panic!("{k} is not set"))
        };
        assert_eq!(get("vid"), "no");
        assert_eq!(get("loop-file"), "inf");
        assert_eq!(get("audio-client-name"), "ronoctua-ambience");
        assert_eq!(get("media-controls"), "no");
        assert_eq!(get("load-scripts"), "no");
        assert_eq!(get("ytdl"), "no");
        assert_eq!(get("volume"), "70");
        assert_eq!(ambience::ROUTE, "loop-file", "the route the options take");
    }

    /// FAILS IF: what is remembered of the sounds stops being read back, or
    /// starts carrying whether a sound was on — a player that starts must
    /// never play a sound by itself.
    #[test]
    fn only_the_volumes_are_remembered() {
        let mut volumes = BTreeMap::new();
        volumes.insert("rain".to_owned(), 35);
        let text = serde_json::to_string(&Remembered {
            version: REMEMBERED_VERSION,
            volumes,
        })
        .expect("written");
        assert_eq!(text, r#"{"version":1,"volumes":{"rain":35}}"#);
        let back: Remembered = serde_json::from_str(&text).expect("read");
        assert_eq!(back.volumes.get("rain"), Some(&35));
    }
}
