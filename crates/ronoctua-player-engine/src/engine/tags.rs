//! What a file says about itself: its sheet read, a person's changes
//! written, its cover kept for MPRIS — all off the engine's lock.
//!
//! # Two threads, and neither is the lock's
//!
//! - `player-sheet` reads sheets (`ronoctua_tags::read`): lofty in the
//!   process, then ffprobe, exiftool and Matroska's own tools, each within
//!   its own deadline — seconds, at worst. A file that starts playing is read
//!   there too, so a door that opens the Metadata section finds it read, and
//!   so its cover can be handed to MPRIS. The cover of every sheet read is
//!   kept in the cache; what was read is kept in memory, the last
//!   [`SHEETS_KEPT`], under the file's stamp: a sheet is read again only
//!   when the file changed.
//! - `player-retag` writes (`ronoctua_tags::write`), ONE file at a time: the
//!   draft beside a file is named after the file and this process, so two
//!   writes of one file at once would share it. A write goes into that draft
//!   and one `rename` puts it where the file was — mpv, which may be playing
//!   the file, holds the old inode and plays on; the next time it loads the
//!   file it reads the new one. How the write goes is `State.retag`.
//!
//! # What follows a written file
//!
//! When a newer sheet of a file replaces the one known — written here, read
//! again on `refresh`, or found changed when it plays again — what the player
//! says of it follows: an entry of the queue that was named by the file's
//! OLD title takes the new one (a name a person or a list gave is theirs and
//! stays), the state's title, artist and album when it plays, MPRIS with
//! them, and `State.sheet_generation`, so every door showing the sheet asks
//! for it again.
//!
//! # The cover MPRIS is handed
//!
//! A picture inside a file is no address a listener can open, so the cover
//! of what plays is taken out into the player's cache
//! (`$XDG_CACHE_HOME/ronoctua/player`, `ronoctua_broadcast::cache`): one
//! directory for everything the player keeps, one ceiling
//! (`assets/cache.nix`, `playerMib`), the file read longest ago going first,
//! and written by this daemon alone. Its name is the file's, its stamp's and
//! the picture's place, hashed: a file written anew is a new name, and the
//! old one ages out.
//!
//! # An address
//!
//! Nothing here has its bytes. Its sheet is what it says while it plays —
//! a radio's ICY words, the tags of a file behind an address as mpv read
//! them (`ronoctua_tags::stream`) — read only, and new every time the
//! stream says something (`State.sheet_generation` moves then too).

use std::{
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime},
};

use ronoctua_broadcast::cache::Cache;
use ronoctua_player::{
    limit::{Limit, TooMany},
    queue::Body,
    wire::{self, DraftId, RetagStatus, Retagging, Say, Trouble},
};
use ronoctua_tags::{
    Cover, Edit, Fact, Field, Kind, Sheet, Stamp, TechKey, Tools, Why, read, stream, write,
};

use super::{Engine, Refused, Shared};
use crate::mpv::Mpv;

mod policy {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/cache.rs"));
}

/// How many sheets are kept, newest last: what plays, what played just
/// before, and a door looking through a set's sources — each a few
/// kilobytes.
pub(super) const SHEETS_KEPT: usize = 8;

/// How often a write's progress is said: as often as a position.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// One mebibyte.
const MIB: u64 = 1024 * 1024;

/// A sheet to read, and whoever waits for it.
struct Reading {
    /// The file, canonical.
    location: PathBuf,
    /// The connections waiting for the answer.
    replies: Vec<mpsc::Sender<Result<Sheet, Why>>>,
    /// Read it again even when the sheet kept has its stamp (`refresh`).
    fresh: bool,
}

/// A change to write, on the writing thread.
enum Writing {
    /// A person's changes — and, for a suggestion taken with its cover, the
    /// fetched picture the cover edit reads, removed once the write ends,
    /// written or not.
    Retag {
        location: String,
        stamp: Stamp,
        edits: Vec<Edit>,
        scratch: Option<PathBuf>,
    },
    /// A cover taken out beside the file.
    SaveCover {
        location: String,
        at: Option<ronoctua_tags::CoverAt>,
    },
    /// A saved draft, the person having said yes: written when the field in
    /// the file is still what the draft started from.
    Draft {
        id: DraftId,
        location: String,
        field: Field,
        original: String,
        value: String,
    },
}

impl Writing {
    fn location(&self) -> &str {
        match self {
            Self::Retag { location, .. }
            | Self::SaveCover { location, .. }
            | Self::Draft { location, .. } => location,
        }
    }

    /// A file of the daemon's own the write reads and nobody needs after it.
    fn scratch(&self) -> Option<PathBuf> {
        match self {
            Self::Retag { scratch, .. } => scratch.clone(),
            Self::SaveCover { .. } | Self::Draft { .. } => None,
        }
    }
}

/// How a write ended.
enum Written {
    /// The file is the new one: its sheet before (when it was known), after
    /// (read again), and its cover in the cache.
    Tags {
        before: Option<Box<Sheet>>,
        after: Result<Box<Sheet>, Why>,
        art: Option<PathBuf>,
        draft: Option<DraftId>,
    },
    /// A cover taken out into this file.
    Saved(PathBuf),
    /// Nothing written, and why — and the draft it was for, which is asked
    /// about no more until it is saved again.
    Refused {
        trouble: Trouble,
        draft: Option<DraftId>,
    },
}

impl Written {
    /// A refusal of a write that was not a draft's.
    fn no(location: &str, why: Why) -> Self {
        Self::Refused {
            trouble: trouble(location, why),
            draft: None,
        }
    }
}

/// The sheets, the writes and the cover of what plays.
pub(super) struct Tags {
    /// The programs, as the daemon was started with them.
    tools: Tools,
    /// The last sheets read, newest last.
    kept: Vec<Sheet>,
    /// The reading thread's mailbox, once it runs.
    reader: Option<mpsc::Sender<Reading>>,
    /// The writing thread's mailbox, once it runs.
    writer: Option<mpsc::Sender<Writing>>,
    /// Writes asked and not done yet, the one being written included.
    writes: usize,
    /// Raised when the player closes: a write stops, the file as it was.
    stop: Arc<AtomicBool>,
    /// The covers last kept in the cache, newest last: the file each came
    /// from, and where it is — what plays is among them.
    art: Vec<(PathBuf, PathBuf)>,
    /// When a write's progress was last said.
    said: Instant,
}

impl Tags {
    /// No sheet yet, the programs this daemon was started with.
    pub(super) fn open() -> Self {
        Self {
            tools: Tools::from_env(),
            kept: Vec::new(),
            reader: None,
            writer: None,
            writes: 0,
            stop: Arc::new(AtomicBool::new(false)),
            art: Vec::new(),
            said: Instant::now(),
        }
    }

    /// The player is closing: a write under way stops, leaving the file as
    /// it was and no draft beside it.
    pub(super) fn end(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    /// The sheet kept of `location`, while the file is still as it was read.
    fn still(&self, location: &Path, stamp: Stamp) -> Option<&Sheet> {
        self.kept
            .iter()
            .find(|s| s.location == location && s.stamp == stamp)
    }

    /// The sheet kept of `location`, whatever its stamp.
    fn known(&self, location: &Path) -> Option<&Sheet> {
        self.kept.iter().find(|s| s.location == location)
    }

    /// Remembers where a file's cover is kept, the oldest forgotten past
    /// [`SHEETS_KEPT`].
    fn kept_cover(&mut self, from: PathBuf, at: PathBuf) {
        self.art.retain(|(f, _)| *f != from);
        self.art.push((from, at));
        let past = self.art.len().saturating_sub(SHEETS_KEPT);
        self.art.drain(..past);
    }

    /// `sheet` as a door is handed it: with where its cover was KEPT, when it
    /// was — the door draws the picture from that file, and never reads a
    /// tag itself.
    fn handed(&self, mut sheet: Sheet) -> Sheet {
        sheet.cover_file = with_cover(&self.art, &sheet.location);
        sheet
    }

    /// Keeps a sheet, newest last, the oldest leaving past the ceiling.
    fn keep(&mut self, sheet: Sheet) {
        self.kept.retain(|s| s.location != sheet.location);
        self.kept.push(sheet);
        let past = self.kept.len().saturating_sub(SHEETS_KEPT);
        self.kept.drain(..past);
    }

    fn read(&mut self, reading: Reading, shared: &Shared) {
        let tools = self.tools.clone();
        let reader = self
            .reader
            .get_or_insert_with(|| spawn_reader(tools.clone(), shared.clone()));
        if let Err(mpsc::SendError(reading)) = reader.send(reading) {
            // A THREAD THAT DIED is started again, once.
            let reader = spawn_reader(tools, shared.clone());
            let _ = reader.send(reading);
            self.reader = Some(reader);
        }
    }

    fn write(&mut self, writing: Writing, shared: &Shared) {
        let tools = self.tools.clone();
        let stop = self.stop.clone();
        let writer = self
            .writer
            .get_or_insert_with(|| spawn_writer(tools.clone(), stop.clone(), shared.clone()));
        if let Err(mpsc::SendError(writing)) = writer.send(writing) {
            let writer = spawn_writer(tools, stop, shared.clone());
            let _ = writer.send(writing);
            self.writer = Some(writer);
        }
    }
}

/// Where a location given by a door is, as the sheets name it.
/// What a write is asked with, checked before it is queued: a field's value
/// its field refuses, a tag's name its format refuses (`tag_name`) — the
/// writer asks again, and these say so at once.
fn checked_edits(location: &str, edits: &[Edit]) -> Result<(), Refused> {
    for edit in edits {
        let checked = match edit {
            Edit::Set { field, value } => field.check(value).map(|_| ()),
            Edit::SetTag { origin, name, .. } | Edit::RemoveTag { origin, name } => {
                ronoctua_tags::tag_name(origin, name).map(|_| ())
            }
            _ => Ok(()),
        };
        checked.map_err(|why| trouble(location, why))?;
    }
    Ok(())
}

pub(super) fn canonical(location: &str) -> Result<(PathBuf, Stamp), Why> {
    let path = std::fs::canonicalize(location).map_err(|_| Why::Gone)?;
    let meta = std::fs::metadata(&path).map_err(|_| Why::Gone)?;
    if !meta.is_file() {
        return Err(Why::NotAFile);
    }
    Ok((path, Stamp::of(&meta)))
}

/// Whether a location is an address — or a file only one of our decoders
/// opens, which is a file all the same (`ronoctua-decode://` never reaches
/// here: the queue writes the path).
fn is_address(location: &str) -> bool {
    location.contains("://")
}

/// The trouble for a file's refusal.
pub(super) fn trouble(path: &str, why: Why) -> Trouble {
    Trouble::Tags {
        path: path.to_owned(),
        why,
    }
}

/// The sheet of `location` for the connection that asked
/// ([`wire::Ask::Sheet`]): an address from what it says while it plays; a
/// file from the sheet kept of it when it has not changed since, else read
/// on the reading thread — this connection waits for it, the lock does not.
pub(super) fn sheet_for(location: &str, shared: &Shared) -> Say {
    match sheet_of(location, shared) {
        Ok(sheet) => Say::Sheet(Box::new(sheet)),
        Err(t) => Say::Refused(t),
    }
}

/// [`sheet_for`], as a result.
pub(super) fn sheet_of(location: &str, shared: &Shared) -> Result<Sheet, Trouble> {
    let (lock, _) = &**shared;
    if is_address(location) {
        let e = lock
            .lock()
            .map_err(|_| trouble(location, Why::Unreadable))?;
        return e.stream_sheet(location);
    }
    let (path, stamp) = canonical(location).map_err(|why| trouble(location, why))?;
    let (reply, answer) = mpsc::channel();
    {
        let mut e = lock
            .lock()
            .map_err(|_| trouble(location, Why::Unreadable))?;
        if let Some(sheet) = e.tags.still(&path, stamp) {
            return Ok(e.tags.handed(sheet.clone()));
        }
        e.tags.read(
            Reading {
                location: path,
                replies: vec![reply],
                fresh: false,
            },
            shared,
        );
    }
    // THE READER ALWAYS ANSWERS — every program it runs has a deadline — or
    // it is gone, and its end is the answer.
    match answer.recv() {
        // THE COVER IT WAS READ WITH, kept by the reader before it answered.
        Ok(Ok(sheet)) => Ok(match lock.lock() {
            Ok(e) => e.tags.handed(sheet),
            Err(_) => sheet,
        }),
        Ok(Err(why)) => Err(trouble(location, why)),
        Err(_) => Err(trouble(location, Why::Unreadable)),
    }
}

/// Where the cover of the file at `location` was kept, by the covers
/// remembered newest last (`Tags::art`).
fn with_cover(art: &[(PathBuf, PathBuf)], location: &Path) -> Option<PathBuf> {
    art.iter()
        .rev()
        .find(|(from, _)| from == location)
        .map(|(_, at)| at.clone())
}

/// The reading thread: every sheet asked of one file is read once, and each
/// asker answered.
fn spawn_reader(tools: Tools, shared: Shared) -> mpsc::Sender<Reading> {
    let (tx, rx) = mpsc::channel::<Reading>();
    let _ = std::thread::Builder::new()
        .name("player-sheet".into())
        .spawn(move || {
            while let Ok(first) = rx.recv() {
                let mut all = vec![first];
                all.extend(rx.try_iter());
                let mut merged: Vec<Reading> = Vec::new();
                for r in all {
                    match merged.iter_mut().find(|m| m.location == r.location) {
                        Some(m) => {
                            m.replies.extend(r.replies);
                            m.fresh |= r.fresh;
                        }
                        None => merged.push(r),
                    }
                }
                for r in merged {
                    read_one(r, &tools, &shared);
                }
            }
        });
    tx
}

/// One file read, its cover kept in the cache, and the engine told.
fn read_one(r: Reading, tools: &Tools, shared: &Shared) {
    let (lock, bell) = &**shared;
    let stamp = std::fs::metadata(&r.location)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|m| Stamp::of(&m));
    let (hit, before) = {
        let Ok(e) = lock.lock() else { return };
        (
            stamp
                .filter(|_| !r.fresh)
                .and_then(|s| e.tags.still(&r.location, s).cloned()),
            e.tags.known(&r.location).cloned(),
        )
    };
    let read_now = hit.is_none();
    let got = hit.map_or_else(|| read::sheet(&r.location, tools), Ok);
    let art = got.as_ref().ok().and_then(|sheet| keep_cover(sheet, tools));
    {
        let Ok(mut e) = lock.lock() else { return };
        e.sheet_read(&r.location, &got, before, art, read_now);
        if e.dirty {
            e.broadcast();
        }
    }
    bell.notify_all();
    for reply in r.replies {
        let _ = reply.send(got.clone());
    }
}

/// The cover of a sheet in the player's cache, taken out when it is not
/// there yet; `None` when it has none, or it would not come out.
fn keep_cover(sheet: &Sheet, tools: &Tools) -> Option<PathBuf> {
    let cover = sheet.front_cover()?;
    let cache = Cache::player(u64::from(policy::PLAYER_MIB) * MIB).ok()?;
    let name = cover_name(sheet, cover);
    if let Ok(Some(_)) = cache.read(&name, Duration::MAX, SystemTime::now()) {
        return Some(cache.dir().join(name));
    }
    let picture = match read::cover(&sheet.location, &cover.at, tools) {
        Ok(p) => p,
        Err(why) => {
            tracing::info!(file = %sheet.location.display(), ?why, "its cover would not come out");
            return None;
        }
    };
    match cache.write(&name, &picture.bytes, None) {
        Ok(()) => Some(cache.dir().join(name)),
        Err(t) => {
            tracing::info!(?t, "a cover could not be kept");
            None
        }
    }
}

/// A cover's name in the cache: `cover-<hash>.<ext>`, the hash of the file,
/// its stamp and the picture's place — a file written anew is a new name.
///
/// FNV-1a, spelled out: the name must be the same in the next daemon, and
/// `std`'s hasher promises nothing across versions.
pub(super) fn cover_name(sheet: &Sheet, cover: &Cover) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    eat(sheet.location.as_os_str().as_encoded_bytes());
    eat(&sheet.stamp.len.to_le_bytes());
    eat(&sheet.stamp.mtime_ns.to_le_bytes());
    eat(&sheet.stamp.ino.to_le_bytes());
    eat(serde_json::to_string(&cover.at)
        .unwrap_or_default()
        .as_bytes());
    let extension = match cover.mime.as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        _ => "img",
    };
    let mut name = String::with_capacity(32);
    let _ = write!(name, "cover-{h:016x}.{extension}");
    name
}

/// The writing thread: one write at a time, each told to the engine as it
/// starts, as it goes, and as it ends.
fn spawn_writer(tools: Tools, stop: Arc<AtomicBool>, shared: Shared) -> mpsc::Sender<Writing> {
    let (tx, rx) = mpsc::channel::<Writing>();
    let _ = std::thread::Builder::new()
        .name("player-retag".into())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                let (lock, bell) = &*shared;
                let location = job.location().to_owned();
                let scratch = job.scratch();
                {
                    let Ok(mut e) = lock.lock() else { return };
                    e.writing_starts(&location);
                    e.broadcast();
                }
                bell.notify_all();
                let progress = |permille: u16| {
                    if let Ok(mut e) = lock.lock() {
                        e.writing_goes(permille);
                    }
                };
                let written = if stop.load(Ordering::SeqCst) {
                    Written::no(&location, Why::Cancelled)
                } else {
                    carry_out(job, &tools, &stop, &progress, &shared)
                };
                // WRITTEN OR NOT, a fetched cover has done its part.
                if let Some(scratch) = scratch {
                    let _ = std::fs::remove_file(scratch);
                }
                {
                    let Ok(mut e) = lock.lock() else { return };
                    e.writing_ends(&location, written);
                    e.broadcast();
                }
                bell.notify_all();
            }
        });
    tx
}

/// The sheet kept of a file as it is now, else read.
fn sheet_now(location: &Path, tools: &Tools, shared: &Shared) -> Result<Sheet, Why> {
    if let Some((path, stamp)) = location.to_str().and_then(|l| canonical(l).ok())
        && let Ok(e) = shared.0.lock()
        && let Some(sheet) = e.tags.still(&path, stamp)
    {
        return Ok(sheet.clone());
    }
    read::sheet(location, tools)
}

/// One write, carried out off the lock.
fn carry_out(
    job: Writing,
    tools: &Tools,
    stop: &AtomicBool,
    progress: &dyn Fn(u16),
    shared: &Shared,
) -> Written {
    let watch = write::Watch {
        stop: Some(stop),
        progress: Some(progress),
    };
    match job {
        Writing::Retag {
            location,
            stamp,
            edits,
            scratch: _,
        } => {
            let path = PathBuf::from(&location);
            let before = std::fs::canonicalize(&path)
                .ok()
                .and_then(|p| shared.0.lock().ok()?.tags.still(&p, stamp).cloned());
            match write::apply_watched(&path, stamp, &edits, tools, &watch) {
                Ok(_) => after_writing(&path, before, None, tools),
                Err(why) => Written::no(&location, why),
            }
        }
        Writing::Draft {
            id,
            location,
            field,
            original,
            value,
        } => {
            let path = PathBuf::from(&location);
            let refused = |why| Written::Refused {
                trouble: trouble(&location, why),
                draft: Some(id),
            };
            // THE FILE AS IT IS NOW, and the field as the draft found it: a
            // change made meanwhile to ANOTHER field is no reason to refuse,
            // one made to this field is.
            let now = match read::sheet(&path, tools) {
                Ok(s) => s,
                Err(why) => return refused(why),
            };
            if checked(field, now.value(field)) != original {
                return refused(Why::ChangedSince);
            }
            let edits = [Edit::Set { field, value }];
            match write::apply_watched(&path, now.stamp, &edits, tools, &watch) {
                Ok(_) => after_writing(&path, Some(now), Some(id), tools),
                Err(why) => refused(why),
            }
        }
        Writing::SaveCover { location, at } => {
            let path = PathBuf::from(&location);
            let at = match at {
                Some(at) => at,
                None => match sheet_now(&path, tools, shared) {
                    Ok(sheet) => match sheet.front_cover() {
                        Some(c) => c.at.clone(),
                        None => return Written::no(&location, Why::NoSuchThing),
                    },
                    Err(why) => return Written::no(&location, why),
                },
            };
            match write::save_cover(&path, &at, tools) {
                Ok(saved) => Written::Saved(saved),
                Err(why) => Written::no(&location, why),
            }
        }
    }
}

/// A field's value as a write would put it — the one comparison a draft is
/// held to.
pub(super) fn checked(field: Field, value: Option<&str>) -> String {
    let raw = value.unwrap_or_default();
    field.check(raw).unwrap_or_else(|_| raw.to_owned())
}

/// What is known of a file once it is written: its sheet read again, and
/// its cover — perhaps a new one — kept.
fn after_writing(
    path: &Path,
    before: Option<Sheet>,
    draft: Option<DraftId>,
    tools: &Tools,
) -> Written {
    let after = read::sheet(path, tools);
    let art = after
        .as_ref()
        .ok()
        .and_then(|sheet| keep_cover(sheet, tools));
    Written::Tags {
        before: before.map(Box::new),
        after: after.map(Box::new),
        art,
        draft,
    }
}

impl Engine {
    /// The file that plays as what plays — the current leaf's, a set's lead
    /// lane's — as the sheets name it.
    pub(super) fn lead_file(&self) -> Option<PathBuf> {
        let location = self.state.current.and_then(|c| self.location_of_leaf(c))?;
        if is_address(&location) {
            return None;
        }
        Some(std::fs::canonicalize(&location).unwrap_or_else(|_| PathBuf::from(location)))
    }

    /// Whether `file` is part of what plays: the lead, or any lane's source.
    fn plays_file(&self, file: &Path) -> bool {
        if self.lead_file().as_deref() == Some(file) {
            return true;
        }
        let (lanes, _) = self.lanes_now();
        lanes.iter().any(|l| {
            !is_address(&l.source)
                && (Path::new(&l.source) == file
                    || std::fs::canonicalize(&l.source).is_ok_and(|c| c == file))
        })
    }

    /// The cover of what plays, for MPRIS's `artUrl` — when it was kept
    /// and is still in the cache.
    #[must_use]
    pub fn art_url(&self) -> Option<String> {
        let lead = self.lead_file()?;
        let (_, at) = self.tags.art.iter().rev().find(|(from, _)| *from == lead)?;
        at.exists().then(|| format!("file://{}", at.display()))
    }

    /// A file that plays was loaded: its sheet is read, and its cover kept,
    /// before any door asks — MPRIS is handed the cover of the one that
    /// leads ([`Engine::art_url`]).
    pub(super) fn read_tags_of(&mut self, location: &str) {
        if is_address(location) {
            // AN ADDRESS SAYS NEW THINGS as it plays; a door asks again.
            self.state.sheet_generation += 1;
            return;
        }
        let Ok(shared) = self.shared() else { return };
        let Ok(path) = std::fs::canonicalize(location) else {
            return;
        };
        self.tags.read(
            Reading {
                location: path,
                replies: Vec::new(),
                fresh: false,
            },
            &shared,
        );
    }

    /// A sheet was read ([`read_one`]): kept; and when it replaced an older
    /// one of the same file, what the player says of the file follows it.
    fn sheet_read(
        &mut self,
        location: &Path,
        got: &Result<Sheet, Why>,
        before: Option<Sheet>,
        art: Option<PathBuf>,
        read_now: bool,
    ) {
        let Ok(sheet) = got else { return };
        if let Some(at) = art {
            self.tags.kept_cover(sheet.location.clone(), at);
            // MPRIS HEARS IT with the next broadcast.
            self.dirty = true;
        }
        if !read_now {
            return;
        }
        self.tags.keep(sheet.clone());
        if !self.plays_file(location) {
            return;
        }
        // NEWLY KNOWN of what plays: every door showing a sheet asks again.
        self.state.sheet_generation += 1;
        self.dirty = true;
        // CHANGED BY SOMEBODY ELSE since it was last read — another tagger,
        // or a refresh asked for: what the player says follows the file.
        if let Some(before) = before.filter(|b| b.stamp != sheet.stamp) {
            self.follow_tags(Some(&before), sheet);
        }
    }

    /// What the player says of a file follows its new sheet: the queue's
    /// names that came from its old title, and — while it plays — the
    /// state's title, artist, album and lyrics from its tags.
    fn follow_tags(&mut self, before: Option<&Sheet>, after: &Sheet) {
        let file = after.location.as_path();
        if let Some(before) = before {
            let old = before.value(Field::Title).map(str::to_owned);
            let new = after.value(Field::Title).map(str::to_owned);
            if let Some(old) = old.filter(|o| Some(o) != new.as_ref()) {
                self.retitle(file, &old, new.as_deref());
            }
        }
        let first = self
            .voices
            .first()
            .and_then(|v| v.loaded.clone())
            .or_else(|| self.loaded.clone());
        let tagged_here = first.as_deref().is_some_and(|l| {
            !is_address(l)
                && (Path::new(l) == file || std::fs::canonicalize(l).is_ok_and(|c| c == file))
        });
        if tagged_here {
            // WHAT mpv READ is the old inode's, and mpv reads no more of it:
            // the new tags are said in its place, as mpv would say them — a
            // title, else the file's name.
            self.mpv_title = Some(after.value(Field::Title).map_or_else(
                || ronoctua_player::queue::file_name(&file.to_string_lossy()).to_owned(),
                str::to_owned,
            ));
            self.state.artist = after
                .value(Field::Artist)
                .or_else(|| after.value(Field::AlbumArtist))
                .map(str::to_owned);
            self.state.album = after.value(Field::Album).map(str::to_owned);
            let from_the_tag = self.state.lyrics_from.as_deref().is_none_or(|f| f == "tag");
            let words_changed =
                before.is_none_or(|b| b.value(Field::Lyrics) != after.value(Field::Lyrics));
            if from_the_tag && words_changed {
                match after.value(Field::Lyrics) {
                    Some(text) => {
                        self.state.lyrics = ronoctua_player::lyrics::parse(text);
                        self.state.lyrics_from = Some("tag".to_owned());
                    }
                    None => {
                        self.state.lyrics.clear();
                        self.state.lyrics_from = None;
                    }
                }
            }
        }
        self.dirty = true;
    }

    /// Every entry named by `file`'s old title takes the new one: a file of
    /// the queue whose title was `old`, and a lane of a set titled `old`.
    /// A name that is not the old title was given by a person or a list, and
    /// stays theirs.
    fn retitle(&mut self, file: &Path, old: &str, new: Option<&str>) {
        let same = |location: &str| {
            !is_address(location)
                && (Path::new(location) == file
                    || std::fs::canonicalize(location).is_ok_and(|c| c == file))
        };
        let mut singles = Vec::new();
        let mut sets = Vec::new();
        for id in self.queue.leaves() {
            match self.queue.find(id).map(|e| &e.body) {
                Some(Body::Single {
                    location,
                    title: Some(t),
                    ..
                }) if t.trim() == old && same(location) => singles.push(id),
                Some(Body::Set { set, .. })
                    if set.lanes.iter().any(|l| {
                        l.title.as_deref().map(str::trim) == Some(old) && same(&l.source)
                    }) =>
                {
                    sets.push(id);
                }
                _ => {}
            }
        }
        for id in singles {
            let _ = self.queue.rename(id, new.unwrap_or_default());
        }
        for id in &sets {
            if let Some(set) = self.queue.set_mut(*id) {
                for lane in &mut set.lanes {
                    if lane.title.as_deref().map(str::trim) == Some(old) && same(&lane.source) {
                        lane.title = new.map(str::to_owned);
                    }
                }
            }
        }
        if !sets.is_empty() {
            // A SET'S NAME is its lanes' titles: the outline says it again.
            self.queue.touch();
        }
    }

    /// The read-only sheet of an address that plays, out of what its voice's
    /// mpv read.
    fn stream_sheet(&self, location: &str) -> Result<Sheet, Trouble> {
        let voice = self
            .voices
            .iter()
            .find(|v| v.loaded.as_deref() == Some(location))
            .or_else(|| {
                self.voices
                    .first()
                    .filter(|_| self.loaded.as_deref() == Some(location))
            })
            .ok_or_else(|| trouble(location, Why::Gone))?;
        let kind = if voice
            .tracks
            .iter()
            .any(|t| t.kind == wire::TrackKind::Video && !t.albumart)
        {
            Kind::Video
        } else {
            Kind::Music
        };
        let tags = super::metadata_of(&voice.mpv);
        Ok(stream::sheet(location, kind, &tags, measured(&voice.mpv)))
    }

    /// A write to a file's tags, asked ([`wire::Ask::Retag`]): checked here
    /// — an address, a value no field takes, a queue of writes too long —
    /// and written on the writing thread.
    pub(super) fn retag(
        &mut self,
        location: String,
        stamp: Stamp,
        edits: Vec<Edit>,
    ) -> Result<(), Refused> {
        if is_address(&location) {
            return Err(trouble(&location, Why::NotAFile).into());
        }
        if edits.is_empty() {
            return Ok(());
        }
        checked_edits(&location, &edits)?;
        self.queue_write(Writing::Retag {
            location,
            stamp,
            edits,
            scratch: None,
        })
    }

    /// A write a lookup's take asks for (`desk.rs`): checked like a
    /// person's ([`Engine::retag`]), with the fetched cover it reads, which
    /// goes once the write ends. A refusal leaves `scratch` to the caller.
    pub(super) fn retag_taken(
        &mut self,
        location: String,
        stamp: Stamp,
        edits: Vec<Edit>,
        scratch: Option<PathBuf>,
    ) -> Result<(), Refused> {
        if is_address(&location) {
            return Err(trouble(&location, Why::NotAFile).into());
        }
        checked_edits(&location, &edits)?;
        self.queue_write(Writing::Retag {
            location,
            stamp,
            edits,
            scratch,
        })
    }

    /// A cover taken out beside its file ([`wire::Ask::SaveCover`]).
    pub(super) fn save_cover(
        &mut self,
        location: String,
        at: Option<ronoctua_tags::CoverAt>,
    ) -> Result<(), Refused> {
        if is_address(&location) {
            return Err(trouble(&location, Why::NotAFile).into());
        }
        self.queue_write(Writing::SaveCover { location, at })
    }

    /// A saved draft to write ([`wire::Ask::WriteDraft`]).
    pub(super) fn write_draft(&mut self, id: DraftId) -> Result<(), Refused> {
        let (location, field, original, value) = self.drafts.to_write(id)?;
        self.queue_write(Writing::Draft {
            id,
            location,
            field,
            original,
            value,
        })
    }

    /// A write for the writing thread, behind the ones waiting.
    fn queue_write(&mut self, writing: Writing) -> Result<(), Refused> {
        if self.tags.writes > wire::WRITES_WAITING_MOST {
            return Err(Trouble::TooMany(TooMany {
                what: Limit::Writes,
                most: wire::WRITES_WAITING_MOST,
            })
            .into());
        }
        let shared = self.shared()?;
        self.tags.writes += 1;
        match self.state.retag.as_mut() {
            // BEHIND ANOTHER: the one being written stays what the state says.
            Some(r) if self.tags.writes > 1 => r.waiting = self.tags.writes - 1,
            // UNDER WAY AT ONCE, as the state says it — never the last
            // write's outcome, which a door would read as this one's.
            _ => {
                self.state.retag = Some(Retagging {
                    location: writing.location().to_owned(),
                    status: RetagStatus::Writing { permille: 0 },
                    waiting: 0,
                });
            }
        }
        self.tags.write(writing, &shared);
        Ok(())
    }

    /// A file's tags to be read again ([`wire::Ask::Refresh`]): an address
    /// says what it says now; a file is read on the reading thread, and what
    /// the player says of it follows.
    pub(super) fn refresh(&mut self, location: &str) -> Result<(), Refused> {
        if is_address(location) {
            self.state.sheet_generation += 1;
            return Ok(());
        }
        let (path, _) = canonical(location).map_err(|why| trouble(location, why))?;
        let shared = self.shared()?;
        self.tags.read(
            Reading {
                location: path,
                replies: Vec::new(),
                fresh: true,
            },
            &shared,
        );
        Ok(())
    }

    /// The writing thread took a write.
    fn writing_starts(&mut self, location: &str) {
        self.state.retag = Some(Retagging {
            location: location.to_owned(),
            status: RetagStatus::Writing { permille: 0 },
            waiting: self.tags.writes.saturating_sub(1),
        });
        self.tags.said = Instant::now();
        self.dirty = true;
    }

    /// How far it is, said as often as a position is.
    fn writing_goes(&mut self, permille: u16) {
        if let Some(r) = self.state.retag.as_mut()
            && let RetagStatus::Writing { permille: p } = &mut r.status
        {
            *p = permille.min(1000);
            self.dirty = true;
        }
        if self.tags.said.elapsed() >= PROGRESS_EVERY {
            self.tags.said = Instant::now();
            self.broadcast();
        }
    }

    /// The writing thread is done with a write.
    fn writing_ends(&mut self, location: &str, written: Written) {
        self.tags.writes = self.tags.writes.saturating_sub(1);
        let status = match written {
            Written::Tags {
                before,
                after,
                art,
                draft,
            } => {
                if let Some(id) = draft {
                    self.drafts.written(id);
                }
                match after {
                    Ok(after) => {
                        if let Some(at) = art {
                            self.tags.kept_cover(after.location.clone(), at);
                        }
                        self.follow_tags(before.as_deref(), &after);
                        self.tags.keep(*after);
                    }
                    Err(why) => {
                        // WRITTEN, and not read back: what is kept of it is
                        // old, and goes.
                        tracing::warn!(file = location, ?why, "a file written would not read back");
                        if let Ok(p) = std::fs::canonicalize(location) {
                            self.tags.kept.retain(|s| s.location != p);
                        }
                    }
                }
                // THE FILE CHANGED: every door showing it asks again, whether
                // it plays or not.
                self.state.sheet_generation += 1;
                RetagStatus::Written
            }
            Written::Saved(path) => RetagStatus::Saved {
                path: path.to_string_lossy().into_owned(),
            },
            Written::Refused { trouble: t, draft } => {
                if let Some(id) = draft {
                    self.drafts.refused(id);
                }
                // A REFUSAL NOBODY HEARS is the silence the protocol promises
                // not to leave: said with the others too.
                self.state.troubles = vec![t.clone()];
                RetagStatus::Refused(t)
            }
        };
        self.state.retag = Some(Retagging {
            location: location.to_owned(),
            status,
            waiting: self.tags.writes,
        });
        self.dirty = true;
    }
}

/// What mpv measured of a stream, as the technical rows a file's sheet has:
/// each bare, its unit in its key.
fn measured(mpv: &Mpv) -> Vec<Fact> {
    let mut out = Vec::new();
    let mut fact = |key: TechKey, value: Option<String>| {
        if let Some(value) = value.filter(|v| !v.is_empty()) {
            out.push(Fact { key, value });
        }
    };
    let whole = |v: f64| {
        #[allow(clippy::cast_possible_truncation)]
        let n = v.round() as i64;
        (n > 0).then(|| n.to_string())
    };
    fact(TechKey::Container, mpv.text("file-format"));
    fact(
        TechKey::DurationMs,
        mpv.float("duration").and_then(|d| whole(d * 1000.0)),
    );
    fact(TechKey::VideoCodec, mpv.text("current-tracks/video/codec"));
    fact(TechKey::WidthPx, mpv.text("video-params/w"));
    fact(TechKey::HeightPx, mpv.text("video-params/h"));
    fact(
        TechKey::FramesPerSecond,
        mpv.float("container-fps")
            .filter(|f| *f > 0.0)
            .map(three_decimals),
    );
    fact(TechKey::AudioCodec, mpv.text("current-tracks/audio/codec"));
    fact(TechKey::SampleRateHz, mpv.text("audio-params/samplerate"));
    fact(TechKey::Channels, mpv.text("audio-params/channel-count"));
    fact(TechKey::ChannelLayout, mpv.text("audio-params/hr-channels"));
    fact(
        TechKey::BitRateBitsPerSecond,
        mpv.float("audio-bitrate").and_then(whole),
    );
    out
}

/// A number with at most three decimals and none it does not need:
/// `23.976`, `25`.
fn three_decimals(value: f64) -> String {
    let text = format!("{value:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_tags::{Can, Container, CoverAt, CoverRole, TagFormat};

    fn sheet(location: &str, stamp: Stamp) -> Sheet {
        Sheet {
            location: PathBuf::from(location),
            stamp,
            kind: Kind::Music,
            container: Container::Flac,
            fields: Vec::new(),
            covers: vec![Cover {
                role: CoverRole::Front,
                mime: "image/jpeg".into(),
                width: None,
                height: None,
                bytes: None,
                at: CoverAt::Tag {
                    format: TagFormat::Vorbis,
                    index: 0,
                },
                can: Can::No(Why::NotAFile),
            }],
            cover_can: Can::No(Why::NotAFile),
            technical: Vec::new(),
            streams: Vec::new(),
            chapters: Vec::new(),
            other: Vec::new(),
            unread: Vec::new(),
            cover_file: None,
            adding: None,
        }
    }

    const ONE: Stamp = Stamp {
        len: 1,
        mtime_ns: 2,
        ino: 3,
    };

    /// FAILS IF: a cover's name in the cache stops being a name the cache
    /// keeps, stays the same when the file is written anew — MPRIS would be
    /// handed the old picture — or changes when nothing did.
    /// FAILS IF: a door is handed a sheet without where its cover was kept
    /// — the door never reads a tag, and draws the picture from that file —
    /// or with another file's cover, or an older one of its own.
    #[test]
    fn a_door_is_handed_where_the_cover_was_kept() {
        let art = vec![
            (PathBuf::from("/m/a.flac"), PathBuf::from("/c/cover-1.jpg")),
            (PathBuf::from("/m/b.flac"), PathBuf::from("/c/cover-2.png")),
            (PathBuf::from("/m/a.flac"), PathBuf::from("/c/cover-3.jpg")),
        ];
        assert_eq!(
            with_cover(&art, Path::new("/m/a.flac")),
            Some(PathBuf::from("/c/cover-3.jpg")),
            "the newest cover of the file"
        );
        assert_eq!(
            with_cover(&art, Path::new("/m/b.flac")),
            Some(PathBuf::from("/c/cover-2.png"))
        );
        assert_eq!(with_cover(&art, Path::new("/m/c.flac")), None);
        let mut tags = Tags::open();
        tags.kept_cover(PathBuf::from("/m/a.flac"), PathBuf::from("/c/cover-9.jpg"));
        let handed = tags.handed(sheet("/m/a.flac", ONE));
        assert_eq!(handed.cover_file, Some(PathBuf::from("/c/cover-9.jpg")));
        let wire = serde_json::to_string(&handed).expect("a sheet serialises");
        assert!(wire.contains("cover_file"), "{wire}");
    }

    #[test]
    fn a_cover_is_named_by_its_file_its_stamp_and_its_place() {
        let a = sheet("/m/a.flac", ONE);
        let name = cover_name(&a, &a.covers[0]);
        assert!(ronoctua_broadcast::cache::is_name(&name), "{name}");
        assert!(
            name.starts_with("cover-") && name.ends_with(".jpg"),
            "{name}"
        );
        assert_eq!(
            name,
            cover_name(&a, &a.covers[0]),
            "the same file, the same name"
        );
        let written = sheet("/m/a.flac", Stamp { ino: 4, ..ONE });
        assert_ne!(name, cover_name(&written, &written.covers[0]));
        let other = sheet("/m/b.flac", ONE);
        assert_ne!(name, cover_name(&other, &other.covers[0]));
    }

    /// FAILS IF: the sheets kept grow past their ceiling, a file is kept
    /// twice, or a file's sheet is found under a stamp it no longer has.
    #[test]
    fn the_sheets_kept_are_the_newest_and_each_under_its_stamp() {
        let mut t = Tags::open();
        for n in 0..(SHEETS_KEPT + 3) {
            t.keep(sheet(&format!("/m/{n}.flac"), ONE));
        }
        assert_eq!(t.kept.len(), SHEETS_KEPT);
        assert!(t.known(Path::new("/m/0.flac")).is_none(), "the oldest went");
        let last = format!("/m/{}.flac", SHEETS_KEPT + 2);
        assert!(t.still(Path::new(&last), ONE).is_some());
        t.keep(sheet(&last, Stamp { ino: 9, ..ONE }));
        assert_eq!(t.kept.len(), SHEETS_KEPT, "a file is kept once");
        assert!(
            t.still(Path::new(&last), ONE).is_none(),
            "not under its old stamp"
        );
    }

    /// FAILS IF: a draft is compared with the file by anything but what a
    /// write would put — a trailing newline the editor added would read as
    /// a change somebody else made.
    #[test]
    fn a_draft_is_compared_as_it_would_be_written() {
        assert_eq!(
            checked(Field::Lyrics, Some("one\ntwo\n\n")),
            checked(Field::Lyrics, Some("one\ntwo"))
        );
        assert_eq!(checked(Field::Comment, None), "");
        assert_ne!(
            checked(Field::Lyrics, Some("one")),
            checked(Field::Lyrics, Some("two"))
        );
    }

    /// FAILS IF: a tag's name its format refuses, or a row nothing here
    /// writes, is queued instead of refused when it is asked — the door
    /// would hear of it only when the write ran.
    #[test]
    fn a_tag_s_name_is_checked_when_it_is_asked() {
        let vorbis = ronoctua_tags::Origin::Lofty {
            format: TagFormat::Vorbis,
        };
        let set = |origin: ronoctua_tags::Origin, name: &str| {
            [Edit::SetTag {
                origin,
                name: name.to_owned(),
                value: "x".to_owned(),
            }]
        };
        assert!(checked_edits("/m/a.flac", &set(vorbis.clone(), "MOOD")).is_ok());
        assert!(checked_edits("/m/a.flac", &set(vorbis, "A=B")).is_err());
        let exif = ronoctua_tags::Origin::Exiftool {
            group: "Vorbis".to_owned(),
        };
        assert!(checked_edits("/m/a.flac", &set(exif, "Vendor")).is_err());
        let removed = [Edit::RemoveTag {
            origin: ronoctua_tags::Origin::Matroska {
                target: ronoctua_tags::Target::Track,
            },
            name: "BPS".to_owned(),
        }];
        assert!(checked_edits("/m/a.mkv", &removed).is_err());
    }

    #[test]
    fn a_rate_keeps_the_decimals_it_needs() {
        assert_eq!(three_decimals(23.976_023_976), "23.976");
        assert_eq!(three_decimals(25.0), "25");
        assert_eq!(three_decimals(29.97), "29.97");
    }
}
