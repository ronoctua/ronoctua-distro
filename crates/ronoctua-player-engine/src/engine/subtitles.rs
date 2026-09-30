//! The subtitle lanes over each picture: read, converted, merged into ONE
//! ASS script per picture voice, and handed to that voice's mpv.
//!
//! # What mpv is given
//!
//! What a picture shows is `ronoctua_player::subtitles::show`'s decision —
//! the lanes of words merged on the primary track, one lane of pictures on
//! the secondary, an ASS or a network's track alone shown whole by mpv. This
//! file carries it out:
//!
//! - the script is written to `$XDG_RUNTIME_DIR/ronoctua-player-subs/
//!   <voice>.ass` (memory, and the dev instance's own directory), added with
//!   `sub-add <p> select`, and only THEN is the script it replaces removed
//!   (`sub-remove`): a change of lanes never leaves the picture bare;
//! - `sub-add` reads the whole file at once, so the next script may replace
//!   this one on disk while mpv shows it.
//!
//! # Off the lock, on a thread of its own
//!
//! Reading a file, converting it and merging six lanes of a film's dialogue
//! take milliseconds to seconds; a track INSIDE a film is found by ffmpeg
//! reading the whole film. None of it happens under the engine's lock: the
//! lock hands a plan to the `player-subtitles` thread, which answers with
//! what to show — and a plan that changed meanwhile makes the old answer one
//! nobody applies. Meanwhile the lanes say `preparing`, and the picture keeps
//! what it showed.
//!
//! # What ffmpeg converts, and its ceilings
//!
//! The formats only ffmpeg reads (`subtitles::Tail`) and the tracks inside a
//! file (`-map 0:s:<n>`) come back as ASS on its standard output, at most
//! [`FFMPEG_OUTPUT_MOST`] bytes and within [`FFMPEG_PATIENCE`]; past either
//! the lane is refused with the number. What was read is kept under its
//! stamp — the file, its size, when it last changed, the track — within the
//! ceiling `assets/cache.nix` gives (`playerSubtitlesMib`), so a lane change
//! merges again without reading anything.
//!
//! # Opened from outside
//!
//! A subtitle file opened with the player — the file manager,
//! `ronoctua-shell play`, MPRIS — goes over the film in the queue that has
//! its name, else over what plays, else over the film beside it that has its
//! name, which then plays; else it is refused, and the sentence says why
//! ([`Engine::open_subtitle`]).

use std::{
    collections::HashMap,
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, mpsc},
    time::{Duration, SystemTime},
};

use ronoctua_player::{
    own::{LaneId, LaneKind, Set},
    queue::{Body, EntryId, Place},
    session,
    subtitles::{
        self, Format, FrameRate, Look, Opened, Picture, Refusal, Rgb,
        show::{self, Native, Primary, Read, Show, Source, Wanted},
    },
    wire::{LaneStatus, Trouble},
};
use ronoctua_tokens::{Color, color, font, space};

use super::{Engine, Refused, Shared, resolve::Resolved, voice};

mod policy {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// The most of ASS ffmpeg may write for one lane: a film's dialogue is a
/// hundred kilobytes, and a converted file past this is not dialogue.
pub(super) const FFMPEG_OUTPUT_MOST: usize = 8 * 1024 * 1024;

/// How long ffmpeg may take over one lane.
pub(super) const FFMPEG_PATIENCE: Duration = Duration::from_secs(30);

/// How much of a file is read to tell pictures from words: a PGS stream is
/// told by its first bytes, and is often larger than any file of words.
const HEAD_BYTES: u64 = 64 * 1024;

/// The runtime directory's leaf the scripts are written under.
const DIRECTORY: &str = "player-subs";

/// Where the scripts of this instance are written.
pub(super) fn directory() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file(DIRECTORY)
}

/// The script a picture voice shows.
fn script_of(voice: u64) -> Option<PathBuf> {
    directory().map(|d| d.join(format!("{voice}.ass")))
}

/// What one picture voice was asked to show and shows.
#[derive(Debug, Default)]
struct Dressed {
    /// The plan last handed to the worker, and its number.
    asked: Option<(Plan, u64)>,
    /// What the worker answered for it.
    answer: Option<Show>,
    /// The answer is on the voice now: a file loaded again drops what was
    /// added, and the answer is put back.
    shown: bool,
    /// mpv's id for the merged script this voice shows.
    merged: Option<i64>,
}

/// What a picture needs of its lanes, and how its script is drawn.
#[derive(Debug, Clone, PartialEq)]
struct Plan {
    /// The lanes over it, in the order they stack.
    wanted: Vec<Wanted>,
    /// Its size, which gives the script its resolution.
    size: (u32, u32),
    /// Its frame rate, for a MicroDVD file that does not say its own.
    rate: Option<FrameRate>,
}

/// One plan for the worker.
#[derive(Debug)]
struct Job {
    voice: u64,
    generation: u64,
    plan: Plan,
}

/// The subtitle lanes of every picture voice.
#[derive(Debug, Default)]
pub(super) struct Subtitles {
    /// The worker's mailbox, once it runs.
    worker: Option<mpsc::Sender<Job>>,
    /// The last plan's number.
    generations: u64,
    /// Each picture voice, by its serial.
    voices: HashMap<u64, Dressed>,
    /// Subtitle files opened from outside, each waiting for the leaf it goes
    /// over to be loaded — a file's lanes are read off its tracks.
    pub attach: Vec<(EntryId, String)>,
}

impl Subtitles {
    /// A fresh start: what an earlier daemon left in the runtime directory
    /// goes (its voices' numbers are not this one's).
    pub(super) fn open() -> Self {
        if let Some(dir) = directory() {
            let _ = std::fs::remove_dir_all(dir);
        }
        Self::default()
    }

    /// What `lane` is doing, when the subtitles say more than its clock: its
    /// words being prepared, or refused with the reason.
    pub(super) fn status_of(&self, lane: LaneId) -> Option<LaneStatus> {
        for d in self.voices.values() {
            let Some((plan, _)) = &d.asked else { continue };
            if !plan.wanted.iter().any(|w| w.lane == lane) {
                continue;
            }
            return match &d.answer {
                None => Some(LaneStatus::Preparing),
                Some(show) => show
                    .refused
                    .iter()
                    .find(|(l, _)| *l == lane)
                    .map(|(_, t)| LaneStatus::Refused(t.clone())),
            };
        }
        None
    }

    /// mpv's id for the merged script the voice `serial` shows.
    pub(super) fn merged_of(&self, serial: u64) -> Option<i64> {
        self.voices.get(&serial).and_then(|d| d.merged)
    }

    /// A voice loaded its file again: what was added to it went with the
    /// last one.
    pub(super) fn reloaded(&mut self, voice: u64) {
        if let Some(d) = self.voices.get_mut(&voice) {
            d.shown = false;
            d.merged = None;
        }
    }

    /// The plan goes to the worker, which is started on first need.
    fn send(&mut self, job: Job, shared: &Shared) {
        let worker = self
            .worker
            .get_or_insert_with(|| spawn_worker(shared.clone()));
        if let Err(mpsc::SendError(job)) = worker.send(job) {
            // A WORKER THAT DIED is started again, once.
            let worker = spawn_worker(shared.clone());
            let _ = worker.send(job);
            self.worker = Some(worker);
        }
    }

    /// Every script goes: the player is leaving.
    pub(super) fn end(&mut self) {
        self.voices.clear();
        if let Some(dir) = directory() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

impl Engine {
    /// Every picture voice asked for what its lanes should show now — only
    /// when that changed: the same plan was answered already.
    pub(super) fn dress_subtitles(&mut self) {
        let Ok(shared) = self.shared() else { return };
        let (lanes, _) = self.lanes_now();
        let mut jobs = Vec::new();
        let Self {
            voices, subtitles, ..
        } = self;
        for v in voices.iter_mut() {
            let d = subtitles.voices.entry(v.serial).or_default();
            let picture = v
                .part
                .as_ref()
                .and_then(|p| p.video)
                .and_then(|id| lanes.iter().find(|l| l.id == id));
            let wanted = match picture {
                // NO PICTURE: nothing to go over.
                None => Vec::new(),
                Some(picture) => match show::wanted(picture, &lanes, &v.tracks) {
                    // ITS OWN TRACKS NOT READ YET: asked again when they are.
                    None => continue,
                    Some(w) => w,
                },
            };
            if wanted.is_empty() {
                undress(v, d);
                d.asked = None;
                d.answer = None;
                continue;
            }
            // ITS SIZE NOT KNOWN YET: the script's resolution is the
            // picture's, and it is asked again when the size comes.
            let Some(size) = v.size else { continue };
            let rate = v
                .mpv
                .float("container-fps")
                .and_then(|f| FrameRate::decimal(&format!("{f:.3}")));
            let plan = Plan { wanted, size, rate };
            if d.asked.as_ref().is_some_and(|(p, _)| *p == plan) {
                // THE SAME PLAN: its answer is on the voice — or goes back on
                // it, when the voice loaded its file again.
                if !d.shown
                    && let Some(answer) = d.answer.clone()
                {
                    show_on(v, d, &answer);
                }
                continue;
            }
            subtitles.generations += 1;
            let generation = subtitles.generations;
            d.asked = Some((plan.clone(), generation));
            d.answer = None;
            jobs.push(Job {
                voice: v.serial,
                generation,
                plan,
            });
        }
        // VOICES THAT LEFT take their scripts with them.
        let alive: Vec<u64> = self.voices.iter().map(|v| v.serial).collect();
        self.subtitles.voices.retain(|serial, _| {
            let keep = alive.contains(serial);
            if !keep && let Some(p) = script_of(*serial) {
                let _ = std::fs::remove_file(p);
            }
            keep
        });
        for job in jobs {
            self.subtitles.send(job, &shared);
        }
    }

    /// The worker's answer for a voice's plan: shown, unless the plan has
    /// changed since — then a newer answer is on its way.
    fn subtitles_ready(&mut self, voice: u64, generation: u64, answer: Show) {
        let Some(d) = self.subtitles.voices.get_mut(&voice) else {
            return;
        };
        if d.asked.as_ref().map(|(_, g)| *g) != Some(generation) {
            return;
        }
        if let Some(v) = self.voices.iter_mut().find(|v| v.serial == voice) {
            show_on(v, d, &answer);
        }
        d.answer = Some(answer);
        self.dirty = true;
    }

    /// Every voice lets go of the subtitles it was given: what plays
    /// together ends, and the first voice goes back to mpv's own choices.
    pub(super) fn undress_subtitles(&mut self) {
        let Self {
            voices, subtitles, ..
        } = self;
        for v in voices.iter_mut() {
            if let Some(d) = subtitles.voices.get_mut(&v.serial) {
                undress(v, d);
            }
        }
        for serial in subtitles.voices.keys() {
            if let Some(p) = script_of(*serial) {
                let _ = std::fs::remove_file(p);
            }
        }
        subtitles.voices.clear();
    }

    /// A subtitle file opened from outside the player: over the film in the
    /// queue that has its name, else over what plays, else over the film
    /// beside it that has its name — which is put where `place` says, and
    /// plays when `play` — else refused with the reason.
    pub(super) fn open_subtitle(
        &mut self,
        path: &str,
        place: Place,
        play: bool,
        shared: &Shared,
    ) -> Result<(), Refused> {
        // THE FILM IN THE QUEUE THAT HAS ITS NAME, the one that plays first.
        if let Some((id, over)) = self.named_after(path) {
            if Some(id) == self.state.current {
                match self.add_lane(path, LaneKind::Subtitle, None, 0.0, over) {
                    // STILL LOADING: its lanes are read off its tracks.
                    Err(Refused(Trouble::NothingPlaying)) => {
                        self.subtitles.attach.push((id, path.to_owned()));
                    }
                    other => other?,
                }
                return Ok(());
            }
            if let Some(set) = self.queue.set_mut(id) {
                let mut changed: Set = set.clone();
                let lane = changed.next_lane_id();
                let mut words = ronoctua_player::own::Lane::new(lane, LaneKind::Subtitle, path);
                words.over = over;
                changed.lanes.push(words);
                if let Some(t) = session::refusal(&changed) {
                    return Err(Trouble::TooMany(t).into());
                }
                *set = changed;
                self.queue.touch();
            } else {
                self.subtitles.attach.push((id, path.to_owned()));
            }
            if play {
                let mpv = self.main(shared)?;
                self.jump(&mpv, id)?;
            }
            return Ok(());
        }
        // WHAT PLAYS, when it has a picture.
        if self.plays_a_picture() {
            return self.add_lane(path, LaneKind::Subtitle, None, 0.0, None);
        }
        // THE FILM BESIDE IT that has its name.
        if let Some(film) = film_beside(Path::new(path)) {
            let item = ronoctua_player::own::Item::Single {
                location: film,
                title: None,
                seconds: None,
            };
            let ids = self.put_at(vec![Ok(Resolved::Now(item))], place, play, false, shared)?;
            if let Some(id) = ids.first() {
                self.subtitles.attach.push((*id, path.to_owned()));
            }
            return Ok(());
        }
        Err(Trouble::NoPicture {
            path: path.to_owned(),
        }
        .into())
    }

    /// The leaf of the queue named like the subtitle file at `path` — a film
    /// alone, or a set with a picture of that name — and the picture lane it
    /// goes over; the one that plays first, then one in the same folder.
    pub(super) fn named_after(&self, path: &str) -> Option<(EntryId, Option<LaneId>)> {
        let mut found: Vec<(EntryId, Option<LaneId>, bool)> = Vec::new();
        for id in self.queue.leaves() {
            match self.queue.find(id).map(|e| &e.body) {
                Some(Body::Single { location, .. })
                    if is_film(location) && subtitles::named_after(path, location) =>
                {
                    found.push((id, None, same_folder(path, location)));
                }
                Some(Body::Set { set, .. }) => {
                    if let Some(l) = set.lanes.iter().find(|l| {
                        l.kind == LaneKind::Video && subtitles::named_after(path, &l.source)
                    }) {
                        found.push((id, Some(l.id), same_folder(path, &l.source)));
                    }
                }
                _ => {}
            }
        }
        let current = self.state.current;
        found
            .iter()
            .find(|(id, _, _)| Some(*id) == current)
            .or_else(|| found.iter().find(|(_, _, near)| *near))
            .or_else(|| found.first())
            .map(|(id, over, _)| (*id, *over))
    }

    /// Whether what plays now has a picture for words to go over.
    fn plays_a_picture(&self) -> bool {
        if self.state.current.is_none() || self.following.is_some() {
            return false;
        }
        let (lanes, _) = self.lanes_now();
        lanes.iter().any(|l| l.kind == LaneKind::Video)
    }

    /// The subtitle files waiting for the leaf that plays, now that it is
    /// read: each becomes a lane over it — or, a file with no picture, is
    /// refused with the reason.
    pub(super) fn attach_waiting(&mut self) {
        let queue = &self.queue;
        self.subtitles
            .attach
            .retain(|(id, _)| queue.find(*id).is_some());
        let Some(current) = self.state.current else {
            return;
        };
        if !self.subtitles.attach.iter().any(|(id, _)| *id == current) {
            return;
        }
        let Some(plain) = self.plain_set() else {
            return;
        };
        // ITS TRACKS NOT READ YET: a file's lanes are its tracks.
        if self.voices.first().is_none_or(|v| v.tracks.is_empty()) {
            return;
        }
        let (waiting, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.subtitles.attach)
            .into_iter()
            .partition(|(id, _)| *id == current);
        self.subtitles.attach = rest;
        let picture = plain.lanes.iter().any(|l| l.kind == LaneKind::Video);
        let mut troubles = Vec::new();
        for (_, path) in waiting {
            if !picture {
                troubles.push(Trouble::NoPicture { path });
                continue;
            }
            if let Err(Refused(t)) = self.add_lane(&path, LaneKind::Subtitle, None, 0.0, None) {
                troubles.push(t);
            }
        }
        if !troubles.is_empty() {
            self.state.troubles = troubles;
        }
        self.dirty = true;
    }
}

/// Puts `answer` on the voice: the merged script added and selected BEFORE
/// the one it replaces is removed, a lane mpv shows by itself selected, the
/// lane of pictures on the secondary track, and every file no lane shows any
/// more removed.
fn show_on(v: &mut voice::Voice, d: &mut Dressed, answer: &Show) {
    let mut kept: Vec<String> = Vec::new();
    match &answer.primary {
        Primary::Merged(_) => {
            if let Some(p) = script_of(v.serial) {
                let p = p.to_string_lossy().into_owned();
                if v.mpv.command(&["sub-add", &p, "select"]).is_ok()
                    && let Some(id) = v
                        .mpv
                        .text("track-list")
                        .and_then(|json| voice::added_track(&json, &p))
                {
                    v.assume("sid", &id.to_string());
                    if let Some(old) = d.merged.replace(id)
                        && old != id
                    {
                        let _ = v.mpv.command(&["sub-remove", &old.to_string()]);
                    }
                }
            }
            v.set("sub-delay", "0");
        }
        Primary::Native(shown) => {
            drop_merged(v, d);
            match &shown.native {
                Native::File(f) => {
                    kept.push(f.clone());
                    if let Some(id) = added(v, f) {
                        v.set("sid", &id.to_string());
                    }
                }
                Native::Track(t) => v.set("sid", &t.to_string()),
            }
            v.set("sub-delay", &format!("{:.3}", shown.delay_seconds));
        }
        Primary::Nothing => {
            drop_merged(v, d);
            v.set("sid", "no");
        }
    }
    match &answer.secondary {
        None => v.set("secondary-sid", "no"),
        Some(shown) => {
            match &shown.native {
                Native::File(f) => {
                    kept.push(f.clone());
                    if let Some(id) = added(v, f) {
                        v.set("secondary-sid", &id.to_string());
                    }
                }
                Native::Track(t) => v.set("secondary-sid", &t.to_string()),
            }
            v.set(
                "secondary-sub-delay",
                &format!("{:.3}", shown.delay_seconds),
            );
        }
    }
    let gone: Vec<i64> = v
        .added
        .iter()
        .filter(|(f, _)| !kept.contains(f))
        .map(|(_, id)| *id)
        .collect();
    for id in gone {
        let _ = v.mpv.command(&["sub-remove", &id.to_string()]);
        v.added.retain(|(_, a)| *a != id);
    }
    d.shown = true;
}

/// The voice shows no subtitle lane of ours any more.
fn undress(v: &mut voice::Voice, d: &mut Dressed) {
    if d.merged.is_none() && v.added.is_empty() && d.answer.is_none() {
        return;
    }
    show_on(
        v,
        d,
        &Show {
            primary: Primary::Nothing,
            secondary: None,
            refused: Vec::new(),
        },
    );
}

/// The merged script leaves the voice.
fn drop_merged(v: &voice::Voice, d: &mut Dressed) {
    if let Some(id) = d.merged.take() {
        let _ = v.mpv.command(&["sub-remove", &id.to_string()]);
    }
}

/// The id mpv gave the subtitle file `file` in `v`, adding it first when it
/// was not.
fn added(v: &mut voice::Voice, file: &str) -> Option<i64> {
    if let Some((_, id)) = v.added.iter().find(|(f, _)| f == file) {
        return Some(*id);
    }
    v.mpv.command(&["sub-add", file, "auto"]).ok()?;
    let id = voice::added_track(&v.mpv.text("track-list")?, file)?;
    v.added.push((file.to_owned(), id));
    Some(id)
}

/// The worker: plans in, answers to the engine. Only the newest plan of each
/// voice is worked on — a person moving a lane's start sends a plan a step.
fn spawn_worker(shared: Shared) -> mpsc::Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    let _ = std::thread::Builder::new()
        .name("player-subtitles".into())
        .spawn(move || {
            let mut cache = Cache::new(policy::PLAYER_SUBTITLES_MIB as usize * 1024 * 1024);
            while let Ok(first) = rx.recv() {
                let mut jobs = vec![first];
                jobs.extend(rx.try_iter());
                let mut newest: Vec<Job> = Vec::new();
                for job in jobs {
                    newest.retain(|j| j.voice != job.voice);
                    newest.push(job);
                }
                for job in newest {
                    let answer = prepare(&job.plan, &mut cache);
                    let answer = match (&answer.primary, script_of(job.voice)) {
                        (Primary::Merged(script), Some(path)) => match write(&path, script) {
                            Ok(()) => answer,
                            Err(why) => Show {
                                primary: Primary::Nothing,
                                refused: job
                                    .plan
                                    .wanted
                                    .iter()
                                    .map(|w| {
                                        (
                                            w.lane,
                                            Trouble::Unwritable {
                                                path: path.to_string_lossy().into_owned(),
                                                why: why.clone(),
                                            },
                                        )
                                    })
                                    .collect(),
                                ..answer
                            },
                        },
                        _ => answer,
                    };
                    let (lock, bell) = &*shared;
                    let Ok(mut e) = lock.lock() else { return };
                    e.subtitles_ready(job.voice, job.generation, answer);
                    if e.dirty {
                        e.broadcast();
                    }
                    drop(e);
                    bell.notify_all();
                }
            }
        });
    tx
}

/// Writes a script whole, so mpv never reads half of one.
fn write(path: &Path, script: &str) -> Result<(), String> {
    let dir = path.parent().ok_or("no folder")?;
    let file = path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or("no file name")?;
    ronoctua_xdg::write_atomically(dir, file, script.as_bytes()).map_err(|e| e.to_string())
}

/// A plan, carried out: every lane read (or converted), and the decision.
fn prepare(plan: &Plan, cache: &mut Cache) -> Show {
    let mut reads: Vec<Option<Read>> = plan
        .wanted
        .iter()
        .map(|w| match &w.source {
            Source::File(path) => Some(read_file(path, plan.rate, cache)),
            Source::Address(url) => Some(Read::OnlyNative(Native::File(url.clone()))),
            Source::Streamed { track, .. } if w.is_image_track() => {
                Some(Read::Image(Native::Track(*track)))
            }
            Source::Streamed { track, .. } => Some(Read::OnlyNative(Native::Track(*track))),
            Source::Nowhere(p) => Some(Read::Refused(Trouble::Subtitle {
                path: p.clone(),
                why: Refusal::Streamed,
            })),
            Source::Empty(p) => Some(Read::Refused(Trouble::Subtitle {
                path: p.clone(),
                why: Refusal::Empty,
            })),
            Source::Inside { own, file, .. } if w.is_image_track() => Some(match own {
                Some(id) => Read::Image(Native::Track(*id)),
                // PICTURES INSIDE ANOTHER FILE: mpv shows only the tracks of
                // the file it has open, and ffmpeg does not make words of
                // them.
                None => Read::Refused(Trouble::Subtitle {
                    path: file.clone(),
                    why: Refusal::Unknown,
                }),
            }),
            Source::Inside { .. } => None,
        })
        .collect();
    // AN ASS TRACK OF THE PICTURE'S OWN FILE, alone and with no pictures, is
    // shown whole by mpv — and the film is not read for it.
    let words = reads
        .iter()
        .filter(|r| matches!(r, None | Some(Read::Words { .. } | Read::OnlyNative(_))))
        .count();
    let images = reads
        .iter()
        .filter(|r| matches!(r, Some(Read::Image(_))))
        .count();
    for (w, r) in plan.wanted.iter().zip(reads.iter_mut()) {
        if r.is_some() {
            continue;
        }
        let Source::Inside { file, n, own } = &w.source else {
            continue;
        };
        *r = Some(match own {
            Some(id) if words == 1 && images == 0 && w.is_ass_track() => {
                Read::OnlyNative(Native::Track(*id))
            }
            _ => read_inside(file, *n, own.filter(|_| w.is_ass_track()), cache),
        });
    }
    let reads: Vec<Read> = reads
        .into_iter()
        .map(|r| r.unwrap_or(Read::Refused(Trouble::NothingPlaying)))
        .collect();
    show::compose(&plan.wanted, &reads, &lane_colours(), &look_of(plan.size))
}

/// A colour of the tokens as the merge writes it — the same in every theme
/// and on every page (`nix/scripts/gen-theme.py` refuses otherwise), so the
/// first palette's is every palette's: the daemon knows no theme, and draws
/// with no door open.
fn rgb(c: Color) -> Rgb {
    let [r, g, b, _] = c.to_rgba8_in(0);
    Rgb::new(r, g, b)
}

/// The six lanes' colours, by slot.
fn lane_colours() -> [Rgb; 6] {
    [
        color::SUBTITLE_LANE_ONE,
        color::SUBTITLE_LANE_TWO,
        color::SUBTITLE_LANE_THREE,
        color::SUBTITLE_LANE_FOUR,
        color::SUBTITLE_LANE_FIVE,
        color::SUBTITLE_LANE_SIX,
    ]
    .map(rgb)
}

/// How a picture of `size` draws its lanes: the tokens' sizes, the
/// interface's family, and the font's own cell as the width lines break by.
fn look_of((width, height): (u32, u32)) -> Look {
    let (play_res_x, play_res_y) = Look::play_res_for(width, height);
    Look {
        play_res_x,
        play_res_y,
        font: font::FAMILY_UI.to_owned(),
        size_permille: u32::from(space::SUBTITLE_SIZEPERMILLE),
        outline_permille: u32::from(space::SUBTITLE_OUTLINEPERMILLE),
        margin_permille: u32::from(space::SUBTITLE_MARGINPERMILLE),
        gap_permille: u32::from(space::SUBTITLE_GAPPERMILLE),
        advance_permille: advance_permille(),
        edge_colour: rgb(color::SUBTITLE_EDGE),
    }
}

/// The font's cell, in thousandths of its size.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn advance_permille() -> u32 {
    (font::CELL_WIDTH_EM * 1000.0).round() as u32
}

/// A subtitle file on this disk, read.
fn read_file(path: &str, rate: Option<FrameRate>, cache: &mut Cache) -> Read {
    let unreadable = || {
        Read::Refused(Trouble::Unreadable {
            path: path.to_owned(),
        })
    };
    let Ok(meta) = std::fs::metadata(path) else {
        return unreadable();
    };
    let key = Key {
        path: path.to_owned(),
        bytes: meta.len(),
        modified: meta.modified().ok(),
        track: None,
    };
    if let Some(hit) = cache.get(&key) {
        return hit;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return unreadable();
    };
    let mut bytes = Vec::new();
    // THE HEAD FIRST: pictures are told by their first bytes, and a stream
    // of them is often larger than any file of words may be.
    if (&mut file)
        .take(HEAD_BYTES)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return unreadable();
    }
    if !matches!(
        subtitles::sniff::binary(&bytes),
        Some(subtitles::Kind::Image(_))
    ) && file
        .take(subtitles::BYTES_MOST as u64 + 1 - bytes.len() as u64)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return unreadable();
    }
    let extension = Path::new(path).extension().and_then(|e| e.to_str());
    match subtitles_of(&bytes, extension, rate) {
        Ok(Opened::Ours(track)) => {
            let ass = track.format == Format::Ass;
            let read = Read::Words {
                words: Arc::new(track),
                native: ass.then(|| Native::File(path.to_owned())),
            };
            cache.put(key, read.clone(), bytes.len());
            read
        }
        Ok(Opened::Ffmpeg(tail)) => {
            let text = subtitles::charset::decode(&bytes).text;
            let args = [
                "-v",
                "error",
                "-nostdin",
                "-f",
                tail.demuxer(),
                "-sub_charenc",
                "UTF-8",
                "-i",
                "pipe:0",
                "-c:s",
                "ass",
                "-f",
                "ass",
                "pipe:1",
            ];
            match ffmpeg(&args, Some(text.into_bytes())) {
                Ok(ass) => match subtitles::read(Format::Ass, &ass, None) {
                    Ok(track) => {
                        let read = Read::Words {
                            words: Arc::new(track),
                            native: None,
                        };
                        cache.put(key, read.clone(), ass.len());
                        read
                    }
                    Err(why) => Read::Refused(Trouble::Subtitle {
                        path: path.to_owned(),
                        why,
                    }),
                },
                Err(failed) => failed.said_of(path),
            }
        }
        Ok(Opened::Image(Picture::Pgs)) => Read::Image(Native::File(path.to_owned())),
        Ok(Opened::Image(Picture::VobSub)) => match vobsub_index(Path::new(path)) {
            Some(index) => Read::Image(Native::File(index)),
            None => Read::Refused(Trouble::Subtitle {
                path: path.to_owned(),
                why: Refusal::NoIndex,
            }),
        },
        Err(why) => Read::Refused(Trouble::Subtitle {
            path: path.to_owned(),
            why,
        }),
    }
}

/// What `bytes` are: pictures by their head alone, anything else read whole.
fn subtitles_of(
    bytes: &[u8],
    extension: Option<&str>,
    rate: Option<FrameRate>,
) -> Result<Opened, Refusal> {
    if let Some(subtitles::Kind::Image(picture)) = subtitles::sniff::binary(bytes) {
        return Ok(Opened::Image(picture));
    }
    subtitles::open(bytes, extension, rate)
}

/// The index mpv opens a VobSub by: the `.idx` itself, or the one beside a
/// `.sub`.
fn vobsub_index(path: &Path) -> Option<String> {
    let is_idx = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("idx"));
    if is_idx {
        return Some(path.to_string_lossy().into_owned());
    }
    ["idx", "IDX"]
        .iter()
        .map(|e| path.with_extension(e))
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

/// A track inside a file on this disk, converted to words by ffmpeg — the
/// `n`th subtitle stream; `ass` names the picture's own track when it is an
/// ASS script, which mpv could show whole alone.
fn read_inside(file: &str, n: usize, ass: Option<i64>, cache: &mut Cache) -> Read {
    let Ok(meta) = std::fs::metadata(file) else {
        return Read::Refused(Trouble::Unreadable {
            path: file.to_owned(),
        });
    };
    let key = Key {
        path: file.to_owned(),
        bytes: meta.len(),
        modified: meta.modified().ok(),
        track: Some(n),
    };
    let native = ass.map(Native::Track);
    if let Some(Read::Words { words, .. }) = cache.get(&key) {
        return Read::Words { words, native };
    }
    let map = format!("0:s:{n}");
    let args = [
        "-v", "error", "-nostdin", "-i", file, "-map", &map, "-c:s", "ass", "-f", "ass", "pipe:1",
    ];
    match ffmpeg(&args, None) {
        Ok(ass) => match subtitles::read(Format::Ass, &ass, None) {
            Ok(track) => {
                let words = Arc::new(track);
                cache.put(
                    key,
                    Read::Words {
                        words: words.clone(),
                        native: None,
                    },
                    ass.len(),
                );
                Read::Words { words, native }
            }
            Err(why) => Read::Refused(Trouble::Subtitle {
                path: file.to_owned(),
                why,
            }),
        },
        Err(failed) => failed.said_of(file),
    }
}

/// Why ffmpeg gave no words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Failed {
    /// It is not on this machine (`RONOCTUA_FFMPEG`).
    Missing,
    /// Past [`FFMPEG_PATIENCE`].
    Slow,
    /// Past [`FFMPEG_OUTPUT_MOST`].
    Large,
    /// It refused the input, or said nothing.
    Refused,
}

impl Failed {
    /// The lane's refusal, about `path`.
    fn said_of(self, path: &str) -> Read {
        let subtitle = |why| {
            Read::Refused(Trouble::Subtitle {
                path: path.to_owned(),
                why,
            })
        };
        match self {
            Self::Missing => Read::Refused(Trouble::Missing {
                program: "ffmpeg".to_owned(),
            }),
            Self::Slow => subtitle(Refusal::TooSlow {
                most_seconds: FFMPEG_PATIENCE.as_secs(),
            }),
            Self::Large => subtitle(Refusal::TooLarge {
                most_bytes: FFMPEG_OUTPUT_MOST,
            }),
            Self::Refused => subtitle(Refusal::Damaged),
        }
    }
}

/// Runs ffmpeg — BY ITS PATH (`RONOCTUA_FFMPEG`, set by the player's unit):
/// a unit's `PATH` does not have it — with `input` on its standard input,
/// and gives back what it wrote on its standard output, within the ceilings.
fn ffmpeg(args: &[&str], input: Option<Vec<u8>>) -> Result<Vec<u8>, Failed> {
    let program = std::env::var_os("RONOCTUA_FFMPEG").ok_or(Failed::Missing)?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| Failed::Missing)?;
    // WRITTEN AND READ AT ONCE, each on a thread: a converter that writes
    // before it has read everything would otherwise wait on us for ever.
    if let (Some(bytes), Some(mut stdin)) = (input, child.stdin.take()) {
        let _ = std::thread::Builder::new()
            .name("player-subtitles-in".into())
            .spawn(move || {
                let _ = stdin.write_all(&bytes);
            });
    }
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Failed::Refused);
    };
    let (said, heard) = mpsc::channel();
    let _ = std::thread::Builder::new()
        .name("player-subtitles-out".into())
        .spawn(move || {
            let mut out = Vec::new();
            let _ = stdout
                .take(FFMPEG_OUTPUT_MOST as u64 + 1)
                .read_to_end(&mut out);
            let _ = said.send(out);
        });
    let Ok(out) = heard.recv_timeout(FFMPEG_PATIENCE) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Failed::Slow);
    };
    if out.len() > FFMPEG_OUTPUT_MOST {
        let _ = child.kill();
        let _ = child.wait();
        return Err(Failed::Large);
    }
    match child.wait() {
        Ok(status) if status.success() && !out.is_empty() => Ok(out),
        _ => Err(Failed::Refused),
    }
}

/// A file's stamp: the same stamp is the same words.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Key {
    path: String,
    bytes: u64,
    modified: Option<SystemTime>,
    /// The stream inside it, for a track; `None` for the file itself.
    track: Option<usize>,
}

/// What was read, under its stamp, within a ceiling in bytes — the least
/// recently used goes first.
#[derive(Debug)]
struct Cache {
    most_bytes: usize,
    used_bytes: usize,
    uses: u64,
    entries: Vec<(Key, Read, usize, u64)>,
}

impl Cache {
    fn new(most_bytes: usize) -> Self {
        Self {
            most_bytes,
            used_bytes: 0,
            uses: 0,
            entries: Vec::new(),
        }
    }

    fn get(&mut self, key: &Key) -> Option<Read> {
        self.uses += 1;
        let uses = self.uses;
        let entry = self.entries.iter_mut().find(|(k, ..)| k == key)?;
        entry.3 = uses;
        Some(entry.1.clone())
    }

    fn put(&mut self, key: Key, read: Read, bytes: usize) {
        self.entries.retain(|(k, ..)| *k != key);
        self.used_bytes = self.entries.iter().map(|(_, _, b, _)| b).sum();
        // LARGER THAN THE WHOLE CEILING: read again when asked, never kept.
        if bytes > self.most_bytes {
            return;
        }
        while self.used_bytes + bytes > self.most_bytes {
            let Some(oldest) = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, (.., used))| *used)
                .map(|(i, _)| i)
            else {
                break;
            };
            let (_, _, freed, _) = self.entries.remove(oldest);
            self.used_bytes -= freed;
        }
        self.uses += 1;
        self.entries.push((key, read, bytes, self.uses));
        self.used_bytes += bytes;
    }
}

/// The names of films, which is what a subtitle file goes over — the video
/// half of a folder's media.
const FILMS: &[&str] = &[
    "3g2", "3gp", "amv", "asf", "avi", "divx", "f4v", "flv", "m2t", "m2ts", "m4v", "mkv", "mov",
    "mp4", "mpeg", "mpg", "mts", "mxf", "ogv", "rm", "rmvb", "ts", "vob", "webm", "wmv",
];

/// Whether `location` is named like a film.
fn is_film(location: &str) -> bool {
    Path::new(location.split(['?', '#']).next().unwrap_or(location))
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| FILMS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Whether two paths are in the same folder.
fn same_folder(a: &str, b: &str) -> bool {
    Path::new(a).parent() == Path::new(b).parent()
}

/// The film beside `subtitle` that has its name, if one is there.
fn film_beside(subtitle: &Path) -> Option<String> {
    let folder = subtitle.parent()?;
    let shown = subtitle.to_string_lossy();
    let mut films: Vec<String> = std::fs::read_dir(folder)
        .ok()?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file() || t.is_symlink()))
        .map(|e| e.path().to_string_lossy().into_owned())
        .filter(|p| is_film(p) && subtitles::named_after(&shown, p))
        .collect();
    // THE LONGEST NAME FIRST: `film.pt.srt` beside `film.mkv` and
    // `film.pt.mkv` is the second's.
    films.sort_by_key(|f| std::cmp::Reverse(f.len()));
    films.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: what a subtitle goes over is not told by its name — a film
    /// by its extension, whatever its case or the query after it.
    #[test]
    fn a_film_is_named_like_one() {
        assert!(is_film("/m/a.MKV") && is_film("http://h/m/a.mp4?k=1"));
        assert!(!is_film("/m/a.flac") && !is_film("/m/a.srt"));
    }

    /// FAILS IF: the film beside a subtitle file is not found, or another
    /// film is taken for it.
    #[test]
    fn the_film_beside_it_is_the_one_with_its_name() {
        let dir = std::env::temp_dir().join(format!("ronoctua-subs-beside-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        for name in ["film.mkv", "film.pt.mkv", "other.mp4", "film.nfo"] {
            std::fs::write(dir.join(name), b"").expect("a file");
        }
        let at = |n: &str| dir.join(n).to_string_lossy().into_owned();
        assert_eq!(film_beside(&dir.join("film.srt")), Some(at("film.mkv")));
        assert_eq!(
            film_beside(&dir.join("film.pt.srt")),
            Some(at("film.pt.mkv"))
        );
        assert_eq!(film_beside(&dir.join("nothing.srt")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the cache keeps more than its ceiling, drops what was used
    /// last instead of what was used least recently, or keeps what is larger
    /// than the whole of it.
    #[test]
    fn the_cache_keeps_its_ceiling_and_the_recently_read() {
        let key = |p: &str| Key {
            path: p.into(),
            bytes: 1,
            modified: None,
            track: None,
        };
        let read = || Read::Refused(Trouble::NothingPlaying);
        let mut cache = Cache::new(100);
        cache.put(key("a"), read(), 40);
        cache.put(key("b"), read(), 40);
        assert!(cache.get(&key("a")).is_some());
        cache.put(key("c"), read(), 40);
        assert!(
            cache.get(&key("b")).is_none(),
            "b was the least recently read"
        );
        assert!(cache.get(&key("a")).is_some() && cache.get(&key("c")).is_some());
        assert!(cache.used_bytes <= 100);
        cache.put(key("huge"), read(), 101);
        assert!(cache.get(&key("huge")).is_none());
        assert!(
            cache.get(&key("a")).is_some(),
            "a refused giant evicts nothing"
        );
    }

    /// FAILS IF: a lane's colour is not the same in every theme and on every
    /// page — the daemon knows no theme and reads the first palette, so a
    /// lane would differ between the film and a door's chip.
    #[test]
    fn a_lane_s_colour_is_the_same_in_every_palette() {
        use ronoctua_tokens::counts;
        for c in [
            color::SUBTITLE_LANE_ONE,
            color::SUBTITLE_LANE_TWO,
            color::SUBTITLE_LANE_THREE,
            color::SUBTITLE_LANE_FOUR,
            color::SUBTITLE_LANE_FIVE,
            color::SUBTITLE_LANE_SIX,
            color::SUBTITLE_EDGE,
        ] {
            let first = c.to_rgba8_in(0);
            for palette in 0..counts::THEMES * counts::PAGES {
                assert_eq!(c.to_rgba8_in(palette), first, "{c:?} in palette {palette}");
            }
        }
    }

    /// FAILS IF: the lanes' colours stop being the tokens', or two lanes are
    /// given one colour.
    #[test]
    fn every_lane_has_a_colour_of_its_own() {
        let colours = lane_colours();
        for (i, a) in colours.iter().enumerate() {
            for b in &colours[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let edge = look_of((1920, 1080)).edge_colour;
        assert!(!colours.contains(&edge));
        assert_eq!(look_of((1440, 1080)).play_res_x, 1440);
    }

    /// FAILS IF: a track inside a film is not turned into words by ffmpeg, a
    /// format only ffmpeg reads is not read through it — or, on a machine
    /// without ffmpeg, the lane is not said to miss it.
    #[test]
    fn ffmpeg_turns_a_film_s_track_and_the_long_tail_into_words() {
        let dir = std::env::temp_dir().join(format!("ronoctua-subs-ffmpeg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let at = |n: &str| dir.join(n).to_string_lossy().into_owned();
        let jss = dir.join("a.jss");
        std::fs::write(&jss, "#TIMERES 100\n0:00:01.00 0:00:02.00 D hello there\n").expect("jss");
        let mut cache = Cache::new(1 << 20);
        let Some(program) = std::env::var_os("RONOCTUA_FFMPEG") else {
            // WITHOUT FFMPEG the lane names the program, and nothing else
            // is pretended.
            let film = dir.join("film.mkv");
            std::fs::write(&film, b"not read").expect("film");
            for read in [
                read_file(&at("a.jss"), None, &mut cache),
                read_inside(&at("film.mkv"), 0, None, &mut cache),
            ] {
                match read {
                    Read::Refused(Trouble::Missing { program }) => assert_eq!(program, "ffmpeg"),
                    other => panic!("{other:?}"),
                }
            }
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        match read_file(&at("a.jss"), None, &mut cache) {
            Read::Words { words, native } => {
                assert_eq!(words.cues[0].text(), "hello there");
                assert_eq!(words.cues[0].start_ms, 1000);
                assert_eq!(native, None);
            }
            other => panic!("{other:?}"),
        }
        // A FILM WITH A SUBTITLE TRACK, made here: its only stream of words
        // is `0:s:0`.
        std::fs::write(
            dir.join("words.srt"),
            "1\n00:00:00,500 --> 00:00:01,500\nfrom inside\n",
        )
        .expect("srt");
        let made = Command::new(&program)
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=64x36:d=2",
                "-i",
            ])
            .arg(dir.join("words.srt"))
            .args(["-map", "0", "-map", "1", "-c:v", "ffv1", "-c:s", "srt"])
            .arg(dir.join("film.mkv"))
            .status()
            .expect("ffmpeg ran");
        assert!(made.success());
        match read_inside(&at("film.mkv"), 0, None, &mut cache) {
            Read::Words { words, .. } => {
                assert_eq!(words.cues[0].text(), "from inside");
                assert_eq!(words.cues[0].start_ms, 500);
            }
            other => panic!("{other:?}"),
        }
        // A STREAM THE FILM DOES NOT HAVE is refused, not empty words.
        match read_inside(&at("film.mkv"), 3, None, &mut cache) {
            Read::Refused(Trouble::Subtitle { why, .. }) => assert_eq!(why, Refusal::Damaged),
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a file of words is not read, pictures are not told by their
    /// head, a VobSub without its index is shown, or a file that is not
    /// there is not said to be unreadable.
    #[test]
    fn a_file_is_read_as_words_pictures_or_a_refusal() {
        let dir = std::env::temp_dir().join(format!("ronoctua-subs-read-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let srt = dir.join("a.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nhello\n").expect("srt");
        let ass = dir.join("a.ass");
        std::fs::write(
            &ass,
            "[Script Info]\n\n[Events]\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,x\n",
        )
        .expect("ass");
        let sup = dir.join("a.sup");
        // A PGS SEGMENT: `PG`, two timestamps, and its type (a palette).
        let mut pgs = b"PG".to_vec();
        pgs.extend_from_slice(&[0; 8]);
        pgs.push(0x14);
        pgs.extend_from_slice(&[0; 4]);
        std::fs::write(&sup, &pgs).expect("sup");
        let sub = dir.join("b.sub");
        std::fs::write(
            &sub,
            [
                0x00, 0x00, 0x01, 0xBA, 0x44, 0, 4, 0, 4, 1, 1, 0x89, 0xC3, 0xF8,
            ],
        )
        .expect("sub");
        let mut cache = Cache::new(1 << 20);
        let at = |p: &Path| p.to_string_lossy().into_owned();
        match read_file(&at(&srt), None, &mut cache) {
            Read::Words { words, native } => {
                assert_eq!(words.cues[0].text(), "hello");
                assert_eq!(native, None, "an SRT is merged even alone");
            }
            other => panic!("{other:?}"),
        }
        assert!(
            cache
                .get(&Key {
                    path: at(&srt),
                    bytes: std::fs::metadata(&srt).expect("meta").len(),
                    modified: std::fs::metadata(&srt).expect("meta").modified().ok(),
                    track: None,
                })
                .is_some()
        );
        match read_file(&at(&ass), None, &mut cache) {
            Read::Words { native, .. } => assert_eq!(native, Some(Native::File(at(&ass)))),
            other => panic!("{other:?}"),
        }
        match read_file(&at(&sup), None, &mut cache) {
            Read::Image(Native::File(f)) => assert_eq!(f, at(&sup)),
            other => panic!("{other:?}"),
        }
        match read_file(&at(&sub), None, &mut cache) {
            Read::Refused(Trouble::Subtitle { why, .. }) => assert_eq!(why, Refusal::NoIndex),
            other => panic!("{other:?}"),
        }
        std::fs::write(
            dir.join("b.idx"),
            "# VobSub index file, v7 (do not modify this line!)\n",
        )
        .expect("idx");
        match read_file(&at(&sub), None, &mut cache) {
            Read::Image(Native::File(f)) => assert!(f.ends_with("b.idx"), "{f}"),
            other => panic!("{other:?}"),
        }
        match read_file(&at(&dir.join("gone.srt")), None, &mut cache) {
            Read::Refused(Trouble::Unreadable { .. }) => {}
            other => panic!("{other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
