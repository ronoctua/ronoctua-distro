//! The queue's half of the engine: the window of two in mpv, what plays next,
//! every edit of the tree, and the two files that remember it.
//!
//! # The window
//!
//! mpv holds the leaf that plays and the one after it, each loaded by us —
//! the one after only when one mpv plays it alone (a file or an address; a
//! stream never has one after it, so a failed relay can try the next). When
//! mpv moves on by itself, [`Engine::settle`] moves the queue's cursor with
//! it, drops what mpv left behind and appends the next one; when mpv runs out,
//! [`Engine::ended`] asks the queue. mpv is never the one that decides.
//!
//! Which entry of mpv's playlist is which leaf is kept by mpv's OWN entry id,
//! read back after every load, and every event is checked against mpv's
//! playlist as it is NOW: an event can be older than a load made since.
//!
//! # What is remembered, and when
//!
//! `state_dir/queue.rqueue` (our own format, [`own::queue_to_text`]) whenever
//! the tree's revision moves, and `state_dir/player.json` — where in the
//! tree, how far in, how loud, repeat and shuffle — whenever one of those
//! changes by an event: a leaf changing, a pause, a stop, closing. Never on a
//! clock. Version 1 of `player.json` held the whole queue, flat; it is read
//! once as files and written in the new form.
//!
//! # Saved, or not
//!
//! A queue opened from a `.rqueue` — or saved as one — keeps where it came
//! from ([`Origin`], in `player.json` too): the file, its name, what the
//! queue WAS then (a SHA-256 of the queue as a file keeps it, [`fingerprint`])
//! and what the file was then (its length, time and inode, [`FileStamp`]).
//! "Not saved" is the fingerprint of now against the one kept — not the
//! tree's revision, which does not move for a lane's volume — computed only
//! when something changed since it was last computed. Saving again writes
//! over the file unless it changed on the disk since, which is the person's
//! to say (`force`).

use std::path::Path;

use ronoctua_player::{
    own::{self, Item, LaneKind, QueueFile},
    queue::{self, Body, Entry, EntryId, Name, Place, Queue, Step},
    wire::{Repeat, Trouble},
};
use serde::{Deserialize, Serialize};

use super::{Engine, Refused, Resolved, Shared, load, location_of};
use crate::mpv::Mpv;

/// The file the queue is kept in, in the state directory.
const QUEUE_FILE: &str = "queue.rqueue";

/// How long a broadcast may stay silent before it counts as failed and its
/// next relay is tried, in seconds (mpv's `network-timeout`).
const STREAM_PATIENCE_SECONDS: u32 = 12;

/// Which address of `stream` is tried after address `tried` failed: the
/// next one while one is left; `None` when every one was tried — and the
/// stream is said to be down. Pure.
pub(super) fn after_failure(stream: &own::Stream, tried: usize) -> Option<usize> {
    let next = tried.checked_add(1)?;
    (next < stream.addresses().count()).then_some(next)
}

/// The file where the queue was is kept in — the name version 1 kept the
/// whole queue under.
const WHERE_FILE: &str = "player.json";

/// The version of [`WHERE_FILE`] written now.
const WHERE_VERSION: u32 = 2;

/// What joins the names of a set's sources where this daemon says a set's
/// name itself — MPRIS's title, and a playlist mpv expanded. A sign between
/// names, not a word in any language; a door joins with its own
/// (`player-set-joiner`).
pub(crate) const JOINER: &str = " + ";

/// How often a queue that changed only in a lane — a start, a volume — is
/// written at most: a meter dragged says dozens of changes a second, and
/// each would be the whole queue written under the engine's lock.
const LANES_KEPT_EVERY: std::time::Duration = std::time::Duration::from_secs(1);

/// When the queue is written to the disk ([`keep_due`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Keep {
    /// Nothing changed since the last write.
    Nothing,
    /// Now.
    Now,
    /// Once this much more time went by: only a lane changed since a write
    /// a moment ago ([`Engine::keep_after`]).
    After(std::time::Duration),
}

/// When the queue is written to the disk: `kept` is what was written last
/// (the tree's revision and the lanes changed in place), `now` what the
/// queue is, `last` when it was written. At once when the tree changed; when
/// only a lane did, once [`LANES_KEPT_EVERY`] went by since the last write —
/// [`Keep::After`] the rest of it, and the LAST change of a meter let go is
/// written then, even when nothing is said after it (a paused set). A save
/// writes it at once. Pure.
#[must_use]
pub(crate) fn keep_due(
    kept: Option<(u64, u64)>,
    now: (u64, u64),
    last: Option<std::time::Instant>,
    at: std::time::Instant,
) -> Keep {
    match (kept, last) {
        (Some(k), _) if k == now => Keep::Nothing,
        (Some((revision, _)), Some(l)) if revision == now.0 => {
            let since = at.saturating_duration_since(l);
            if since >= LANES_KEPT_EVERY {
                Keep::Now
            } else {
                Keep::After(LANES_KEPT_EVERY - since)
            }
        }
        _ => Keep::Now,
    }
}

/// A file as it was when it was read or written: what [`Origin`] compares
/// to tell a `.rqueue` that changed on the disk since.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileStamp {
    /// Its length, bytes.
    pub len: u64,
    /// When it was last written: seconds since the epoch, and nanoseconds.
    pub mtime_s: i64,
    /// The nanoseconds of it.
    pub mtime_ns: i64,
    /// Its inode: a file renamed over it is another file.
    pub ino: u64,
}

impl FileStamp {
    /// The file at `path` now; `None` when it is not there.
    #[must_use]
    pub(crate) fn of(path: &Path) -> Option<Self> {
        use std::os::unix::fs::MetadataExt as _;
        let m = std::fs::metadata(path).ok()?;
        Some(Self {
            len: m.len(),
            mtime_s: m.mtime(),
            mtime_ns: m.mtime_nsec(),
            ino: m.ino(),
        })
    }
}

/// Where the queue was opened from, or last saved as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Origin {
    /// The `.rqueue`.
    pub path: String,
    /// What it is called.
    pub name: String,
    /// The queue as it was then ([`fingerprint`]).
    pub fingerprint: String,
    /// The file as it was then; `None` when it could not be read.
    pub stamp: Option<FileStamp>,
}

/// The queue as a file keeps it, paths absolute, hashed: two queues with the
/// same things in the same order with the same settings — a lane's volume,
/// a set's name — are one fingerprint, and any change is another. SHA-256,
/// hex. Pure.
#[must_use]
pub(crate) fn fingerprint(queue: &Queue) -> String {
    let bytes = serde_json::to_vec(&queue.kept(None)).unwrap_or_default();
    aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, &bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// What the Queue's title says of where it is kept: nothing in it, never
/// kept, the file's, or changed since — the fingerprint of `now` against
/// the one `origin` kept. Pure.
#[must_use]
pub(crate) fn kept_as(
    queue: &Queue,
    origin: Option<&Origin>,
    now: impl FnOnce() -> String,
) -> queue::QueueKept {
    use queue::QueueKept;
    if queue.is_empty() {
        return QueueKept::Empty;
    }
    match origin {
        None => QueueKept::Never,
        Some(o) if now() == o.fingerprint => QueueKept::Same {
            path: o.path.clone(),
            name: o.name.clone(),
        },
        Some(o) => QueueKept::Changed {
            path: o.path.clone(),
            name: o.name.clone(),
        },
    }
}

/// Writes `queue` as a `.rqueue` at `to`, called `name` (else after the
/// file): where it is kept from now on — the file, its name, the queue's
/// fingerprint and the file as written. Writes nothing but `to`.
///
/// # Errors
///
/// [`Trouble::Unwritable`]: no folder or name in `to`, or the write failed.
pub(crate) fn written_queue(
    queue: &Queue,
    to: &str,
    name: Option<String>,
) -> Result<Origin, Trouble> {
    let unwritable = |why: &str| Trouble::Unwritable {
        path: to.to_owned(),
        why: why.to_owned(),
    };
    let path = Path::new(to);
    let folder = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| unwritable("no folder"))?;
    let file = path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| unwritable("no file name"))?;
    let name = name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
    let text = own::queue_to_text(&queue.kept(name.clone()), folder);
    ronoctua_xdg::write_atomically(folder, file, text.as_bytes())
        .map_err(|e| unwritable(&e.to_string()))?;
    let called = name.unwrap_or_else(|| {
        path.file_stem()
            .map_or_else(|| file.to_owned(), |s| s.to_string_lossy().into_owned())
    });
    Ok(Origin {
        path: to.to_owned(),
        name: called,
        fingerprint: fingerprint(queue),
        stamp: FileStamp::of(path),
    })
}

/// Where "save again" writes, and under what name: the file the queue came
/// from — refused with [`Trouble::NoOrigin`] when it came from none, and
/// with [`Trouble::ChangedOnDisk`] when that file changed on the disk since
/// it was read or written, unless `force` (the person's answer to the
/// question). Reads the file's stamp.
///
/// # Errors
///
/// As above.
pub(crate) fn again(origin: Option<&Origin>, force: bool) -> Result<(String, String), Trouble> {
    let origin = origin.ok_or(Trouble::NoOrigin)?;
    if !force && FileStamp::of(Path::new(&origin.path)) != origin.stamp {
        return Err(Trouble::ChangedOnDisk {
            path: origin.path.clone(),
        });
    }
    Ok((origin.path.clone(), origin.name.clone()))
}

/// Refused with [`Trouble::AlreadyQueued`] when any of `places` — each
/// worked out before the lock ([`queue::place_of`]) — is a leaf of `queue`
/// already: by the leaf's own text ([`Queue::holding`]), or by where a leaf
/// written otherwise IS (`aliases`, written → canonical, found off the
/// lock). Text only: this runs under the engine's lock.
///
/// # Errors
///
/// As above.
pub(crate) fn not_queued_in(
    queue: &Queue,
    aliases: &std::collections::HashMap<String, String>,
    places: &[String],
) -> Result<(), Refused> {
    let held = |place: &String| {
        queue.holding(place).or_else(|| {
            aliases
                .iter()
                .filter(|(_, canonical)| *canonical == place)
                .find_map(|(written, _)| queue.holding(written))
        })
    };
    match places.iter().find_map(held) {
        Some((entry, name)) => Err(Trouble::AlreadyQueued { entry, name }.into()),
        None => Ok(()),
    }
}

/// Where the queue was, as [`WHERE_FILE`] keeps it — both versions, so one
/// read tells them apart: version 1 had `queue` and `current` (an index),
/// version 2 has `version` and `at` (a path in the tree).
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct Remembered {
    version: u32,
    /// The current leaf's path in the tree ([`Queue::path_of`]).
    at: Option<Vec<usize>>,
    /// How far into it, in seconds.
    position: f64,
    volume: Option<u8>,
    repeat: Repeat,
    shuffle: bool,
    /// Where the queue was opened from or last saved as.
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<Origin>,
    /// Version 1's flat queue, imported once.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    queue: Vec<OldItem>,
    /// Version 1's current, an index into `queue`.
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<usize>,
}

/// An item of version 1's flat queue.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct OldItem {
    location: String,
    title: Option<String>,
}

fn remembered() -> ronoctua_store::Store<Remembered> {
    ronoctua_store::Store::at(ronoctua_store::state_dir(), WHERE_FILE)
}

/// One entry of mpv's playlist, as its `playlist` property says it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct MpvEntry {
    /// mpv's id for it, which no other entry of this mpv ever has.
    pub id: i64,
    /// What it opens.
    pub filename: String,
    /// The title a playlist gave it.
    pub title: Option<String>,
    /// It is the one playing (or loading).
    pub current: bool,
}

/// mpv's playlist, now.
pub(super) fn mpv_playlist(mpv: &Mpv) -> Vec<MpvEntry> {
    mpv.text("playlist")
        .map(|json| playlist_of(&json))
        .unwrap_or_default()
}

/// mpv's `playlist` property, read.
fn playlist_of(json: &str) -> Vec<MpvEntry> {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str(json) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|e| {
            Some(MpvEntry {
                id: e.get("id")?.as_i64()?,
                filename: e.get("filename")?.as_str()?.to_owned(),
                title: e.get("title").and_then(|t| t.as_str()).map(str::to_owned),
                current: e.get("current").and_then(serde_json::Value::as_bool) == Some(true),
            })
        })
        .collect()
}

/// A seed for a shuffle, from the system's random source.
fn seed() -> u64 {
    ronoctua_player::remote::random::<8>()
        .map(u64::from_le_bytes)
        .unwrap_or_else(|_| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| {
                    u64::try_from(d.as_nanos() & u128::from(u64::MAX)).unwrap_or(0)
                })
        })
}

/// The first leaf under an entry, in the tree's order; the entry itself
/// when it is one.
pub(super) fn first_leaf(entry: &Entry) -> Option<EntryId> {
    match &entry.body {
        Body::Playlist { children, .. } => children.iter().find_map(first_leaf),
        _ => Some(entry.id),
    }
}

/// Names the leaf `id` `title` when it is still what it arrived as: one
/// address, `url`, that nobody named — neither the person nor the thing put
/// (`Thing::Named`) nor a listing. `true` when it was named. Pure.
pub(crate) fn name_if_unnamed(queue: &mut Queue, id: EntryId, url: &str, title: &str) -> bool {
    let title = title.trim();
    let unnamed = matches!(
        queue.find(id).map(|e| &e.body),
        Some(Body::Single { location, title: None, .. }) if location == url
    );
    if !unnamed || title.is_empty() || title == queue::file_name(url) {
        return false;
    }
    queue.rename(id, title).is_ok()
}

/// A name as this daemon says it, where it must say one itself.
pub(crate) fn name_text(name: &Name) -> String {
    match name {
        Name::Titled(t) => t.clone(),
        Name::Location(l) => queue::file_name(l).to_owned(),
        Name::Together(parts) => queue::together(parts, JOINER, u16::MAX),
    }
}

/// The lane a set is played by until its lanes play together: its first
/// picture, else its first sound, else its first lane.
fn lead_lane(set: &own::Set) -> Option<&own::Lane> {
    let on = || set.lanes.iter().filter(|l| l.enabled);
    on().find(|l| l.kind == LaneKind::Video)
        .or_else(|| on().find(|l| l.kind == LaneKind::Audio))
        .or_else(|| set.lanes.first())
}

impl Engine {
    /// The queue, as the daemon keeps it.
    #[must_use]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// The shuffled order, while shuffle is on.
    #[must_use]
    pub fn order(&self) -> Option<&[EntryId]> {
        self.order.as_deref()
    }

    /// What was remembered: the queue, where it was, and the switches.
    pub(super) fn restore(&mut self) {
        let r = remembered().read();
        self.state.volume = r.volume.unwrap_or(100).min(130);
        self.state.repeat = r.repeat;
        let kept = ronoctua_store::state_dir()
            .map(|d| d.join(QUEUE_FILE))
            .filter(|p| p.exists());
        let (read, at, imported) = match &kept {
            Some(path) => match own::read(path) {
                Ok(own::Own::Queue(file)) => (Queue::from_file(file), r.at.clone(), false),
                other => {
                    tracing::error!(path = %path.display(), read = ?other.err(), "the kept queue will not read; starting from none");
                    (Ok(Queue::new()), None, false)
                }
            },
            // VERSION 1 kept the queue flat, beside where it was: its items
            // are files, and they come in once, as files.
            None if !r.queue.is_empty() => {
                let file = QueueFile {
                    name: None,
                    entries: r
                        .queue
                        .iter()
                        .map(|i| Item::Single {
                            location: i.location.clone(),
                            title: i.title.clone(),
                            seconds: None,
                        })
                        .collect(),
                };
                (Queue::from_file(file), r.current.map(|c| vec![c]), true)
            }
            None => (Ok(Queue::new()), None, false),
        };
        self.queue = read.unwrap_or_else(|refusal| {
            tracing::error!(
                ?refusal,
                "the kept queue passes a ceiling; starting from none"
            );
            Queue::new()
        });
        let current = at
            .and_then(|p| self.queue.at_path(&p))
            .filter(|e| e.body.is_leaf())
            .map(|e| e.id);
        self.state.current = current;
        if let Some(c) = current
            && r.position > 0.0
        {
            self.resume = Some((c, r.position));
        }
        if r.shuffle {
            self.shuffle(true);
        }
        // WHERE IT CAME FROM, and what it was then: "not saved" survives a
        // restart, as the queue does.
        self.origin = r.origin.filter(|_| !self.queue.is_empty());
        // WHAT WAS READ IS WHAT IS ON THE DISK — unless it came from version
        // 1, which the first broadcast writes in the new form.
        if !imported {
            self.kept_revision = Some((self.queue.revision(), self.lanes_moved));
        }
    }

    /// Writes the queue to the disk, and where it is in it (its path may
    /// have moved with the change).
    pub(super) fn keep_queue(&mut self) {
        if self.leaving {
            return;
        }
        let Some(dir) = ronoctua_store::state_dir() else {
            return;
        };
        let text = own::queue_to_text(&self.queue.kept(None), &dir);
        match ronoctua_xdg::write_atomically(&dir, QUEUE_FILE, text.as_bytes()) {
            Ok(()) => {
                self.kept_revision = Some((self.queue.revision(), self.lanes_moved));
                self.queue_kept_at = Some(std::time::Instant::now());
            }
            Err(e) => tracing::warn!(error = %e, "could not keep the queue"),
        }
        self.remember_where();
    }

    /// A lane's last change, written once [`Keep::After`] `wait` is up: a
    /// meter let go a moment after the last write would otherwise wait for
    /// the next thing the player says — on a paused set, maybe never before
    /// the session ends. ONE thread, sleeping once, and only while such a
    /// write waits (`keep_armed`); what it writes is the queue as it is then.
    pub(super) fn keep_after(&mut self, wait: std::time::Duration) {
        if self.keep_armed {
            return;
        }
        let Ok(shared) = self.shared() else { return };
        let spawned = std::thread::Builder::new()
            .name("player-keep".into())
            .spawn(move || {
                std::thread::sleep(wait);
                let (lock, _) = &*shared;
                if let Ok(mut e) = lock.lock() {
                    e.keep_armed = false;
                    if e.kept_revision != Some((e.queue.revision(), e.lanes_moved)) {
                        e.keep_queue();
                    }
                }
            });
        self.keep_armed = spawned.is_ok();
    }

    /// Writes where the queue is: this machine's own place while it follows
    /// another.
    pub(super) fn remember_where(&self) {
        if self.leaving {
            return;
        }
        let (current, position) = self
            .following
            .as_ref()
            .map_or((self.state.current, self.state.position), |f| f.resume);
        let r = Remembered {
            version: WHERE_VERSION,
            at: current.and_then(|c| self.queue.path_of(c)),
            position,
            volume: Some(self.state.volume),
            repeat: self.state.repeat,
            shuffle: self.state.shuffle,
            origin: self.origin.clone(),
            queue: Vec::new(),
            current: None,
        };
        if let Err(e) = remembered().write(&r) {
            tracing::warn!(error = %e, "could not remember where the queue was");
        }
    }

    /// The current leaf is `id` now — from its start, until mpv says where.
    pub(super) fn set_current(&mut self, id: Option<EntryId>) {
        if self.state.current != id {
            self.address = 0;
            self.state.position = 0.0;
            // ANOTHER ENTRY PLAYS: its start is a start (`directory.rs`).
            self.directories.forget_start();
        }
        self.state.current = id;
        self.dirty = true;
    }

    /// The name an entry was GIVEN — a title, a stream's name, a set's name,
    /// or a set's sources together — before what mpv reads in the file.
    pub(super) fn given_title(&self, id: EntryId) -> Option<String> {
        match &self.queue.find(id)?.body {
            Body::Single { title, .. } => title.clone(),
            Body::Stream(stream) => stream.title.clone(),
            // A SET IS NEVER NAMED AFTER THE ONE FILE mpv is playing of it.
            body @ Body::Set { .. } => Some(name_text(&body.name())),
            Body::Playlist { name, .. } => Some(name.clone()),
        }
    }

    /// Where a leaf plays from: its file, the address of its stream being
    /// tried, a set's first picture (else its first sound).
    pub(super) fn location_of_leaf(&self, id: EntryId) -> Option<String> {
        match &self.queue.find(id)?.body {
            Body::Single { location, .. } => Some(location.clone()),
            Body::Stream(stream) => {
                let n = if self.state.current == Some(id) {
                    self.address
                } else {
                    0
                };
                stream.addresses().nth(n).map(str::to_owned)
            }
            Body::Set { set, .. } => lead_lane(set).map(|l| l.source.clone()),
            Body::Playlist { .. } => None,
        }
    }

    /// What mpv is handed for a leaf, and the options for that file only;
    /// `None` for a playlist.
    fn target_of(&self, id: EntryId) -> Option<(String, Vec<(&'static str, String)>)> {
        let location = self.location_of_leaf(id)?;
        let mut options = Vec::new();
        match &self.queue.find(id)?.body {
            Body::Stream(stream) => {
                if let Some(r) = &stream.referrer {
                    options.push(("referrer", r.clone()));
                }
                if let Some(ua) = &stream.user_agent {
                    options.push(("user-agent", ua.clone()));
                }
                // A BROADCAST THAT DOES NOT ANSWER gives way to its next
                // relay in seconds: mpv's own patience is a minute, and a
                // station of a directory often has a dead address first.
                if stream.live {
                    options.push(("network-timeout", STREAM_PATIENCE_SECONDS.to_string()));
                }
            }
            // A SET IS NEVER ONE FILE in the window: its voices are the
            // session's (`session.rs`).
            Body::Set { .. } | Body::Single { .. } | Body::Playlist { .. } => {}
        }
        Some((location, options))
    }

    /// Plays a leaf: mpv's playlist becomes it and the one after — or, for a
    /// set, its voices play it together (`session.rs`).
    ///
    /// A leaf still being read from the network plays once it is read.
    pub(super) fn load_leaf(
        &mut self,
        mpv: &Mpv,
        id: EntryId,
        paused: bool,
    ) -> Result<(), Refused> {
        // ANOTHER MACHINE'S LANES play in the voices while following: this
        // machine's own leaf plays when it leaves.
        if let Some(f) = &self.following {
            return Err(Trouble::Following {
                host: f.host.clone(),
            }
            .into());
        }
        if self.resolving.contains(&id) {
            // WHAT PLAYED STOPS: "play this now" was asked, and this is not
            // ready yet.
            if !self.window.is_empty() || self.session.is_some() {
                self.stop(mpv)?;
            }
            self.play_when_resolved = Some(id);
            self.set_current(Some(id));
            return Ok(());
        }
        if matches!(self.queue.find(id).map(|e| &e.body), Some(Body::Set { .. })) {
            let at = match self.resume.take() {
                Some((leaf, at)) if leaf == id => at,
                _ => 0.0,
            };
            // ANOTHER SET, or the same one from its start: what played
            // together before is let go first, so no part of it is taken
            // for a part of this one at the wrong place.
            self.end_session();
            return self.start_set(mpv, id, at, paused);
        }
        // A FILE ALONE: whatever played together leaves, and mpv chooses its
        // tracks again — a track chosen for the last file is not this one's.
        self.end_session();
        for name in ["vid", "aid", "sid"] {
            let _ = mpv.set(name, "auto");
        }
        let Some((location, mut options)) = self.target_of(id) else {
            return Err(Trouble::Empty {
                path: self
                    .queue
                    .find(id)
                    .map(|e| name_text(&e.body.name()))
                    .unwrap_or_default(),
            }
            .into());
        };
        let mut start = 0.0;
        if let Some((leaf, at)) = self.resume.take()
            && leaf == id
        {
            start = at;
            options.push(("start", format!("{at:.3}")));
        }
        self.stopping = false;
        mpv.set("pause", if paused { "yes" } else { "no" })?;
        load(mpv, &location, "replace", &options)?;
        self.window = mpv_playlist(mpv)
            .last()
            .map(|e| vec![(e.id, id)])
            .unwrap_or_default();
        self.set_current(Some(id));
        self.state.position = start;
        self.refresh_lookahead(mpv);
        self.remember_where();
        Ok(())
    }

    /// Plays the current leaf, else the first in the order.
    pub(super) fn play(&mut self, mpv: &Mpv) -> Result<(), Refused> {
        // THE CURRENT — or its first leaf, when what was current became a
        // playlist (read from the network, or opened by mpv) — else the
        // first of the order.
        let leaf = self
            .state
            .current
            .and_then(|c| self.queue.find(c))
            .and_then(first_leaf)
            .or_else(|| self.queue.order(self.order.as_deref()).first().copied());
        match leaf {
            Some(l) => self.load_leaf(mpv, l, false),
            None => Ok(()),
        }
    }

    /// Stops: mpv lets everything go, and the current leaf stays current.
    pub(super) fn stop(&mut self, mpv: &Mpv) -> Result<(), Refused> {
        self.end_session();
        self.stopping = true;
        // PLAYED AGAIN after a stop, a stream starts again (`directory.rs`).
        self.directories.forget_start();
        self.window.clear();
        mpv.command(&["stop"])?;
        // IDLE NOW, not when its event comes: a play asked right after would
        // otherwise unpause a player with nothing in it.
        self.idle = true;
        self.state.position = 0.0;
        self.remember_where();
        Ok(())
    }

    /// Empties the queue — and it is no longer the file it came from: what
    /// is put next is a new queue.
    pub(super) fn clear(&mut self) {
        self.origin = None;
        self.aliases.clear();
        self.queue.clear();
        self.resolving.clear();
        self.play_when_resolved = None;
        self.resume = None;
        if self.state.shuffle {
            self.reshuffle();
        }
        self.set_current(None);
        self.remember_where();
    }

    /// The next leaf: mpv's own step when it has it open already.
    pub(super) fn next(&mut self, mpv: &Mpv) -> Result<(), Refused> {
        let Some(target) = self.queue.next(
            self.state.current,
            self.state.repeat,
            Step::Asked,
            self.order.as_deref(),
        ) else {
            return Ok(());
        };
        let paused = !self.idle && !self.state.playing;
        if !self.idle && self.window.get(1).map(|w| w.1) == Some(target) {
            mpv.command(&["playlist-next", "force"])?;
            return Ok(());
        }
        self.load_leaf(mpv, target, paused)
    }

    /// The leaf before; the start of this one when there is none.
    pub(super) fn previous(&mut self, mpv: &Mpv) -> Result<(), Refused> {
        let paused = !self.idle && !self.state.playing;
        match self
            .queue
            .previous(self.state.current, self.state.repeat, self.order.as_deref())
        {
            Some(target) => self.load_leaf(mpv, target, paused),
            None if self.session.is_some() => {
                self.session_seek(0.0);
                Ok(())
            }
            None if !self.idle => {
                mpv.command(&["seek", "0", "absolute", "exact"])?;
                Ok(())
            }
            None => Ok(()),
        }
    }

    /// Plays an entry: a leaf, or a playlist from its first leaf.
    pub(super) fn jump(&mut self, mpv: &Mpv, id: EntryId) -> Result<(), Refused> {
        let entry = self.queue.find(id).ok_or(Trouble::NoSuchEntry { id })?;
        let Some(leaf) = first_leaf(entry) else {
            return Err(Trouble::Empty {
                path: name_text(&entry.body.name()),
            }
            .into());
        };
        self.load_leaf(mpv, leaf, false)
    }

    /// Takes an entry out; what plays moves on when it was inside.
    pub(super) fn remove(&mut self, mpv: &Mpv, id: EntryId) -> Result<(), Refused> {
        let current = self.state.current;
        let after =
            self.queue
                .after_removing(current, id, self.state.repeat, self.order.as_deref());
        self.queue.remove(id)?;
        let queue = &self.queue;
        self.resolving.retain(|r| queue.find(*r).is_some());
        if self
            .play_when_resolved
            .is_some_and(|p| queue.find(p).is_none())
        {
            self.play_when_resolved = None;
        }
        if let Some(order) = &mut self.order {
            order.retain(|o| queue.find(*o).is_some());
        }
        if after == current {
            self.refresh_lookahead(mpv);
            return Ok(());
        }
        // WHAT PLAYED WAS INSIDE WHAT WENT.
        if self.window.is_empty() && self.session.is_none() {
            self.set_current(after);
            self.remember_where();
            return Ok(());
        }
        match after {
            Some(next) => self.load_leaf(mpv, next, !self.state.playing),
            None => {
                self.stop(mpv)?;
                self.set_current(None);
                Ok(())
            }
        }
    }

    /// Puts what was resolved into the queue (see `Ask::Put` for where).
    pub(super) fn put_resolved(
        &mut self,
        resolved: Vec<Result<Resolved, Trouble>>,
        after: Option<EntryId>,
        into: Option<EntryId>,
        play: bool,
        replace: bool,
        shared: &Shared,
    ) -> Result<Vec<EntryId>, Refused> {
        let place = match (after, into) {
            (Some(a), _) => Place::After(a),
            (None, Some(p)) => Place::End(Some(p)),
            // PLAYED NOW, from outside: right after what plays, and the
            // queue is kept.
            (None, None) if play => self
                .state
                .current
                .filter(|c| self.queue.find(*c).is_some())
                .map_or(Place::End(None), Place::After),
            (None, None) => Place::End(None),
        };
        self.put_at(resolved, place, play, replace, shared)
    }

    /// Puts what was resolved at `place` — or, with `replace`, starts the
    /// queue over with it — and plays the first when `play`; the ids given.
    pub(crate) fn put_at(
        &mut self,
        resolved: Vec<Result<Resolved, Trouble>>,
        place: Place,
        play: bool,
        replace: bool,
        shared: &Shared,
    ) -> Result<Vec<EntryId>, Refused> {
        let mut troubles = Vec::new();
        let mut items = Vec::new();
        let mut jobs = Vec::new();
        let mut titles = Vec::new();
        let mut subtitles = Vec::new();
        let mut whole: Option<(Vec<Item>, Option<Origin>)> = None;
        let mut aliases = Vec::new();
        let count = resolved.len();
        for r in resolved {
            match r {
                Ok(Resolved::Now(item)) => {
                    items.push(item);
                    jobs.push(None);
                    titles.push(None);
                }
                // PLAYABLE NOW, and named when its title comes.
                Ok(Resolved::Titling { item, url }) => {
                    items.push(item);
                    jobs.push(None);
                    titles.push(Some(url));
                }
                Ok(Resolved::Queue {
                    item,
                    entries,
                    stamp,
                    aliases: places,
                }) => {
                    aliases.extend(places);
                    // A QUEUE FILE OPENED TO START OVER IS the queue, not a
                    // playlist inside it — and the file it came from.
                    if replace && count == 1 {
                        let origin = match &item {
                            Item::Playlist {
                                name,
                                from: Some(path),
                                ..
                            } => Some(Origin {
                                path: path.clone(),
                                name: name.clone(),
                                fingerprint: String::new(),
                                stamp,
                            }),
                            _ => None,
                        };
                        whole = Some((entries, origin));
                    }
                    items.push(item);
                    jobs.push(None);
                    titles.push(None);
                }
                Ok(Resolved::Later { placeholder, job }) => {
                    items.push(placeholder);
                    jobs.push(Some(job));
                    titles.push(None);
                }
                // A SUBTITLE FILE goes over a film once the films are in.
                Ok(Resolved::Subtitle(path)) => subtitles.push(path),
                Err(t) => troubles.push(t),
            }
        }
        if items.is_empty() {
            // ONLY SUBTITLES: nothing is put, and each goes over a film —
            // the queue is never started over for one.
            if !subtitles.is_empty() {
                return self.put_subtitles(subtitles, troubles, place, play, shared);
            }
            return Err(troubles
                .into_iter()
                .next()
                .unwrap_or(Trouble::Empty {
                    path: String::new(),
                })
                .into());
        }
        let ids = if replace {
            let (ids, origin) = match whole {
                Some((entries, origin)) => {
                    jobs = vec![None; entries.len()];
                    titles = vec![None; entries.len()];
                    (self.queue.replace(entries)?, origin)
                }
                None => (self.queue.replace(items)?, None),
            };
            // STARTED OVER: the file it came from, and what it is now — or,
            // from anything else, a queue never kept.
            self.origin = origin.map(|o| Origin {
                fingerprint: fingerprint(&self.queue),
                ..o
            });
            self.resolving.clear();
            self.play_when_resolved = None;
            self.resume = None;
            self.set_current(None);
            self.aliases.clear();
            ids
        } else {
            self.queue.insert(place, items)?
        };
        // WHERE A `.rqueue`'S FILES ARE, beside how it writes them.
        self.aliases.extend(aliases);
        let named: Vec<(EntryId, String)> = ids
            .iter()
            .zip(titles)
            .filter_map(|(id, url)| url.map(|u| (*id, u)))
            .collect();
        super::resolve::spawn_titles(named, shared.clone());
        let waiting: Vec<(EntryId, super::resolve::Job)> = ids
            .iter()
            .zip(jobs)
            .filter_map(|(id, job)| job.map(|j| (*id, j)))
            .collect();
        self.resolving.extend(waiting.iter().map(|(id, _)| *id));
        super::resolve::spawn(waiting, shared.clone());
        if self.state.shuffle {
            self.reshuffle();
        }
        if !troubles.is_empty() {
            self.state.troubles = troubles;
        }
        // PUT WHILE FOLLOWING another machine: it waits in the queue, and
        // this machine plays it when it leaves.
        if (play || replace)
            && self.following.is_none()
            && let Some(first) = ids.first().copied()
        {
            let mpv = self.main(shared)?;
            self.jump(&mpv, first)?;
        } else if let Some(m) = self.first_mpv() {
            self.refresh_lookahead(&m);
        }
        // AND THE SUBTITLE FILES put with them, over the films they are
        // named after — which were put just now, and play already if asked.
        if !subtitles.is_empty() {
            let mut refused = Vec::new();
            for path in subtitles {
                if let Err(Refused(t)) = self.open_subtitle(&path, place, false, shared) {
                    refused.push(t);
                }
            }
            if !refused.is_empty() {
                self.state.troubles = refused;
            }
        }
        Ok(ids)
    }

    /// Subtitle files put with nothing else: each goes over a film
    /// (`subtitles.rs`, `open_subtitle`); refused when none could.
    fn put_subtitles(
        &mut self,
        subtitles: Vec<String>,
        mut troubles: Vec<Trouble>,
        place: Place,
        play: bool,
        shared: &Shared,
    ) -> Result<Vec<EntryId>, Refused> {
        let mut any = false;
        for path in subtitles {
            match self.open_subtitle(&path, place, play, shared) {
                Ok(()) => any = true,
                Err(Refused(t)) => troubles.push(t),
            }
        }
        if !any {
            return Err(troubles
                .into_iter()
                .next()
                .unwrap_or(Trouble::Empty {
                    path: String::new(),
                })
                .into());
        }
        if !troubles.is_empty() {
            self.state.troubles = troubles;
        }
        Ok(Vec::new())
    }

    /// A reading from the network is done: the placeholder becomes what it
    /// read, keeping its id; it plays now when that was asked.
    pub(crate) fn resolved(&mut self, id: EntryId, got: Result<Item, Trouble>, shared: &Shared) {
        // REMOVED OR CLEARED while it was read: nothing to put anywhere.
        if !self.resolving.contains(&id) {
            return;
        }
        self.resolving.retain(|r| *r != id);
        match got.and_then(|item| self.queue.set_body(id, item).map_err(Trouble::from)) {
            Ok(()) => {}
            // THE PLACEHOLDER STAYS, as the address it was: mpv gets its
            // own chance at it, and the trouble says why ours failed.
            Err(t) => self.state.troubles = vec![t],
        }
        if self.state.shuffle {
            self.reshuffle();
        }
        if self.play_when_resolved == Some(id) {
            self.play_when_resolved = None;
            match self.main(shared) {
                Ok(mpv) => {
                    if let Err(Refused(t)) = self.jump(&mpv, id) {
                        self.state.troubles = vec![t];
                    }
                }
                Err(e) => {
                    self.state.troubles = vec![Trouble::NoEngine {
                        why: format!("{e:#}"),
                    }];
                }
            }
        } else if let Some(m) = self.first_mpv() {
            self.refresh_lookahead(&m);
        }
        self.dirty = true;
    }

    /// Writes the queue as a `.rqueue` at `to` — and it is that file's now:
    /// what the Queue's title says it is kept as.
    pub(super) fn save_queue(&mut self, to: &str, name: Option<String>) -> Result<(), Refused> {
        self.origin = Some(written_queue(&self.queue, to, name)?);
        // AND THE COPY THIS PLAYER WAKES WITH is what was saved — a lane
        // changed a moment before would otherwise be missing from it, and
        // the queue would wake "not saved".
        if self.kept_revision == Some((self.queue.revision(), self.lanes_moved)) {
            self.remember_where();
        } else {
            self.keep_queue();
        }
        Ok(())
    }

    /// Writes the queue again into the file it came from
    /// ([`Ask::SaveQueueAgain`](ronoctua_player::wire::Ask::SaveQueueAgain)),
    /// refused as [`again`] says.
    pub(super) fn save_queue_again(&mut self, force: bool) -> Result<(), Refused> {
        let (path, name) = again(self.origin.as_ref(), force)?;
        self.save_queue(&path, Some(name))
    }

    /// Where the queue is kept, as the state says it — its fingerprint
    /// computed only when the queue changed since it last was.
    pub(super) fn queue_kept(&self) -> queue::QueueKept {
        kept_as(&self.queue, self.origin.as_ref(), || {
            let key = (self.queue.revision(), self.lanes_moved);
            let mut known = self.fingerprinted.borrow_mut();
            match &*known {
                Some((at, print)) if *at == key => print.clone(),
                _ => {
                    let print = fingerprint(&self.queue);
                    *known = Some((key, print.clone()));
                    print
                }
            }
        })
    }

    /// A leaf that arrived as an address and nobody named — `id`, still at
    /// `url` — called `title` now: the video's own title, from yt-dlp or
    /// from mpv. The name was learned, not changed by anybody ([`learned`]).
    ///
    /// [`learned`]: Engine::learned
    pub(crate) fn adopt_title(&mut self, id: EntryId, url: &str, title: &str) {
        if self.learned(|q| name_if_unnamed(q, id, url, title)) {
            self.dirty = true;
        }
    }

    /// A change to the queue that NOBODY made — a title learned, a list mpv
    /// opened into what it holds — done by `change` (`true` when it changed
    /// something): a queue that was its file's before stays its file's, its
    /// fingerprint taken again; one that was not stays not.
    pub(crate) fn learned(&mut self, change: impl FnOnce(&mut Queue) -> bool) -> bool {
        let was_kept = matches!(self.queue_kept(), queue::QueueKept::Same { .. });
        let changed = change(&mut self.queue);
        if changed
            && was_kept
            && let Some(o) = self.origin.as_mut()
        {
            o.fingerprint = fingerprint(&self.queue);
        }
        changed
    }

    /// Refused when any of `places` ([`queue::place_of`], worked out before
    /// the lock) is a leaf of the queue already — the `once` of a put
    /// ([`not_queued_in`]).
    pub(crate) fn not_queued(&self, places: &[String]) -> Result<(), Refused> {
        not_queued_in(&self.queue, &self.aliases, places)
    }

    /// Where the files of the queue this player woke with ARE, asked of the
    /// disk on a thread of its own (a queue of ten thousand files, a network
    /// mount that does not answer) and kept beside them once known: a queue
    /// from before files were put by their canonical paths, or opened from a
    /// `.rqueue`, still answers "is it in the queue already?".
    pub(super) fn find_aliases(&self, shared: &Shared) {
        let entries = self.queue.kept(None).entries;
        if entries.is_empty() {
            return;
        }
        let shared = shared.clone();
        let _ = std::thread::Builder::new()
            .name("player-places".into())
            .spawn(move || {
                let found = super::resolve::aliases_of(&entries);
                if found.is_empty() {
                    return;
                }
                if let Ok(mut e) = shared.0.lock() {
                    e.aliases.extend(found);
                }
            });
    }

    /// Shuffle on (a new order, the current first) or off.
    pub(super) fn shuffle(&mut self, on: bool) {
        self.state.shuffle = on;
        self.order = None;
        if on {
            self.reshuffle();
        }
    }

    /// A new order, with the current first — after the leaves changed.
    fn reshuffle(&mut self) {
        self.order = Some(queue::shuffled(
            &self.queue.leaves(),
            self.state.current,
            seed(),
        ));
    }

    /// Makes the entry after the current in mpv the queue's next one.
    pub(super) fn refresh_lookahead(&mut self, mpv: &Mpv) {
        if self.following.is_some() || self.window.is_empty() {
            return;
        }
        let Some(current) = self.state.current else {
            return;
        };
        if self.window[0].1 != current {
            return;
        }
        // A STREAM HAS NOTHING AFTER IT in mpv: when its address fails, the
        // next relay is tried (`ended`), not the next entry.
        let streaming = matches!(
            self.queue.find(current).map(|e| &e.body),
            Some(Body::Stream(_))
        );
        let wanted = if self.state.repeat == Repeat::One || streaming {
            None
        } else {
            self.queue.next(
                Some(current),
                self.state.repeat,
                Step::Ended,
                self.order.as_deref(),
            )
        }
        .filter(|n| !self.resolving.contains(n))
        // ONE mpv PLAYS IT ALONE, or it is not in the window: a set waits for
        // the end of what plays and is loaded then.
        .filter(|n| {
            matches!(
                self.queue.find(*n).map(|e| &e.body),
                Some(Body::Single { .. } | Body::Stream(_))
            )
        });
        if self.window.get(1).map(|w| w.1) == wanted {
            return;
        }
        // EVERYTHING AFTER THE CURRENT goes, in mpv's list as in ours.
        let list = mpv_playlist(mpv);
        if let Some(at) = list.iter().position(|e| e.id == self.window[0].0) {
            for index in (at + 1..list.len()).rev() {
                let _ = mpv.command(&["playlist-remove", &index.to_string()]);
            }
        }
        self.window.truncate(1);
        let Some(next) = wanted else {
            return;
        };
        let Some((location, options)) = self.target_of(next) else {
            return;
        };
        if load(mpv, &location, "append", &options).is_ok()
            && let Some(e) = mpv_playlist(mpv).last()
        {
            self.window.push((e.id, next));
        }
    }

    /// mpv moved in its window, or its playlist changed: the queue follows
    /// what mpv plays NOW.
    pub(super) fn settle(&mut self, mpv: &Mpv) {
        if self.following.is_some() || self.window.is_empty() {
            return;
        }
        let list = mpv_playlist(mpv);
        let Some(at) = list.iter().position(|e| e.current) else {
            return;
        };
        let Some(k) = self.window.iter().position(|(m, _)| *m == list[at].id) else {
            self.adopt_expansion(mpv, &list, at);
            return;
        };
        let leaf = self.window[k].1;
        // WHAT mpv LEFT BEHIND goes.
        for index in (0..at).rev() {
            let _ = mpv.command(&["playlist-remove", &index.to_string()]);
        }
        self.window.drain(..k);
        if self.state.current != Some(leaf) {
            self.set_current(Some(leaf));
            self.remember_where();
        }
        self.refresh_lookahead(mpv);
    }

    /// mpv plays entries nobody gave it: it opened one of ours as a list of
    /// its own (an archive, a cue sheet, a list inside a list). That entry
    /// becomes a playlist of what mpv found, with the same id, and the
    /// window maps what mpv holds again.
    fn adopt_expansion(&mut self, mpv: &Mpv, list: &[MpvEntry], at: usize) {
        let known: Vec<i64> = self.window.iter().map(|w| w.0).collect();
        let gone: Vec<EntryId> = self
            .window
            .iter()
            .filter(|(m, _)| !list.iter().any(|e| e.id == *m))
            .map(|w| w.1)
            .collect();
        let fresh: Vec<&MpvEntry> = list.iter().filter(|e| !known.contains(&e.id)).collect();
        // NOT ONE OF OURS, or not one we can tell: mpv is let go until the
        // next load, rather than guessed at.
        let ([opened], false) = (gone.as_slice(), fresh.is_empty()) else {
            self.window.clear();
            return;
        };
        let opened = *opened;
        let Some(entry) = self.queue.find(opened) else {
            self.window.clear();
            return;
        };
        let item = Item::Playlist {
            name: name_text(&entry.body.name()),
            from: self.location_of_leaf(opened),
            entries: fresh
                .iter()
                .map(|e| Item::Single {
                    location: location_of(&e.filename),
                    title: e.title.clone(),
                    seconds: None,
                })
                .collect(),
        };
        // WHAT mpv FOUND IN IT was learned, not changed by anybody: a queue
        // that was its file's stays its file's (`learned`).
        if !self.learned(|q| q.set_body(opened, item).is_ok()) {
            self.window.clear();
            return;
        }
        let children: Vec<EntryId> = match self.queue.find(opened).map(|e| &e.body) {
            Some(Body::Playlist { children, .. }) => children.iter().map(|c| c.id).collect(),
            _ => Vec::new(),
        };
        let mapped: Vec<(i64, EntryId)> = fresh
            .iter()
            .map(|e| e.id)
            .zip(children)
            .chain(self.window.iter().copied())
            .collect();
        for index in (0..at).rev() {
            let _ = mpv.command(&["playlist-remove", &index.to_string()]);
        }
        self.window = list[at..]
            .iter()
            .filter_map(|e| mapped.iter().find(|(m, _)| *m == e.id).copied())
            .collect();
        if let Some(&(_, leaf)) = self.window.first() {
            self.set_current(Some(leaf));
        }
        if self.state.shuffle {
            self.reshuffle();
        }
        self.refresh_lookahead(mpv);
    }

    /// mpv ran out of its window, and nobody stopped it: a stream tries its
    /// next address, and otherwise the queue says what comes next.
    pub(super) fn ended(&mut self, mpv: &Mpv) {
        self.window.clear();
        let failed = std::mem::take(&mut self.failed);
        if failed && self.next_address(mpv) {
            return;
        }
        if failed {
            self.failures += 1;
        }
        // EVERY LEAF FAILED ONCE, one after the other: the queue stops here
        // rather than going round for ever.
        if self.failures > self.queue.leaf_count() {
            self.failures = 0;
            self.set_current(None);
            self.remember_where();
            return;
        }
        // A FAILED LEAF IS SKIPPED, even when it was to repeat.
        let step = if failed { Step::Asked } else { Step::Ended };
        match self.queue.next(
            self.state.current,
            self.state.repeat,
            step,
            self.order.as_deref(),
        ) {
            Some(next) => {
                if let Err(Refused(t)) = self.load_leaf(mpv, next, false) {
                    self.state.troubles = vec![t];
                }
            }
            None => {
                self.set_current(None);
                self.remember_where();
            }
        }
    }

    /// The current stream's next address, when one is left; `true` when it
    /// is being tried. With none left, what was tried is said, once.
    fn next_address(&mut self, mpv: &Mpv) -> bool {
        let Some(current) = self.state.current else {
            return false;
        };
        let Some(Body::Stream(stream)) = self.queue.find(current).map(|e| &e.body) else {
            return false;
        };
        let count = stream.addresses().count();
        if let Some(next) = after_failure(stream, self.address) {
            self.address = next;
            tracing::info!(
                address = self.address,
                "a stream's address failed; the next is tried"
            );
            if let Err(Refused(t)) = self.load_leaf(mpv, current, false) {
                self.state.troubles = vec![t];
            }
            return true;
        }
        let title = stream.title.clone().unwrap_or_else(|| stream.url.clone());
        let url = stream.url.clone();
        self.state.troubles = vec![Trouble::StreamDown {
            title,
            tried: count,
        }];
        if !self.state.unanswered.contains(&url) {
            self.state.unanswered.push(url);
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_player::wire::{Say, Thing};

    /// FAILS IF: mpv's playlist stops being read by its entry ids and its
    /// current mark — the window would map the wrong leaf.
    #[test]
    fn mpvs_playlist_is_read_by_its_ids() {
        let json = r#"[{"filename":"/m/a.flac","current":true,"playing":true,"id":7},
                       {"filename":"ronoctua-decode:///m/b.sid","title":"B","id":8}]"#;
        let list = playlist_of(json);
        assert_eq!(list.len(), 2);
        assert_eq!((list[0].id, list[0].current), (7, true));
        assert_eq!((list[1].id, list[1].current), (8, false));
        assert_eq!(list[1].title.as_deref(), Some("B"));
        assert_eq!(location_of(&list[1].filename), "/m/b.sid");
        assert!(playlist_of("not json").is_empty());
    }

    /// FAILS IF: the first version's flat queue is no longer read, or the
    /// second version is written with the first's fields.
    #[test]
    fn where_the_queue_was_reads_both_versions() {
        let v1: Remembered = serde_json::from_str(
            r#"{"queue":[{"location":"/m/a.flac","title":null},{"location":"/m/b.flac"}],
                "current":1,"volume":80,"repeat":"queue"}"#,
        )
        .expect("version 1");
        assert_eq!(v1.version, 0);
        assert_eq!(v1.queue.len(), 2);
        assert_eq!(v1.current, Some(1));
        assert_eq!(v1.repeat, Repeat::Queue);
        let v2 = Remembered {
            version: WHERE_VERSION,
            at: Some(vec![1, 0]),
            position: 12.5,
            volume: Some(90),
            repeat: Repeat::One,
            shuffle: true,
            origin: None,
            queue: Vec::new(),
            current: None,
        };
        let text = serde_json::to_string(&v2).expect("written");
        assert!(
            !text.contains("\"queue\"") && !text.contains("\"current\""),
            "{text}"
        );
        let back: Remembered = serde_json::from_str(&text).expect("read");
        assert_eq!(back.at, Some(vec![1, 0]));
        assert!(back.shuffle);
    }

    /// FAILS IF: "not saved" follows the tree's revision instead of what
    /// the queue holds — a lane's volume does not move the revision — or a
    /// queue saved, or only reordered back, is said to be changed.
    #[test]
    fn a_queue_is_its_file_s_until_what_it_holds_changes() {
        use ronoctua_player::own::{Lane, LaneId, Set};
        use ronoctua_player::queue::QueueKept;
        let mut set = Set::default();
        set.lanes
            .push(Lane::new(LaneId(1), LaneKind::Video, "/m/film.mkv"));
        let mut q = Queue::from_file(QueueFile {
            name: None,
            entries: vec![
                Item::Single {
                    location: "/m/a.flac".into(),
                    title: None,
                    seconds: None,
                },
                Item::Set { set, from: None },
            ],
        })
        .expect("a queue");
        // OPENED: the fingerprint of what it is.
        let origin = Origin {
            path: "/q/Sunday.rqueue".into(),
            name: "Sunday".into(),
            fingerprint: fingerprint(&q),
            stamp: None,
        };
        let now = |q: &Queue| kept_as(q, Some(&origin), || fingerprint(q));
        assert!(matches!(now(&q), QueueKept::Same { .. }));
        // A LANE'S VOLUME: the revision stays, the queue changed.
        let revision = q.revision();
        let set_id = q.leaves()[1];
        q.set_mut(set_id).expect("a set").lanes[0].volume_percent = 40;
        assert_eq!(q.revision(), revision, "a volume is not the outline's news");
        assert!(matches!(now(&q), QueueKept::Changed { .. }));
        q.set_mut(set_id).expect("a set").lanes[0].volume_percent = 100;
        assert!(
            matches!(now(&q), QueueKept::Same { .. }),
            "put back, it is the file's again"
        );
        // AN ENTRY REMOVED is a change; SAVED, the new fingerprint is kept.
        let first = q.leaves()[0];
        q.remove(first).expect("removed");
        assert!(matches!(now(&q), QueueKept::Changed { .. }));
        let saved = Origin {
            fingerprint: fingerprint(&q),
            ..origin.clone()
        };
        assert!(matches!(
            kept_as(&q, Some(&saved), || fingerprint(&q)),
            QueueKept::Same { .. }
        ));
        // NEVER KEPT, and NOTHING IN IT.
        assert_eq!(kept_as(&q, None, || unreachable!()), QueueKept::Never);
        assert_eq!(
            kept_as(&Queue::new(), Some(&saved), || unreachable!()),
            QueueKept::Empty
        );
    }

    /// A folder of this test's own, empty.
    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-engine-queue-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        dir
    }

    /// FAILS IF: a queue saved and read back — the `.rqueue` opened again,
    /// the copy the player wakes with after a restart — is not the queue it
    /// was, to the last bit of a lane's start dragged along the timeline
    /// (a float the default JSON reader gives back one unit off); or "save
    /// again" writes over a file that changed on the disk without the
    /// person's `force`, or writes where the queue came from nowhere.
    #[test]
    fn a_saved_queue_reads_back_as_the_queue_it_was() {
        use ronoctua_player::own::{Lane, LaneId, Set};
        let dir = scratch("saved");
        let mut set = Set::default();
        let mut lane = Lane::new(LaneId(1), LaneKind::Video, "/m/film.mkv");
        // THE FLOATS the reader used to miss (measured with serde_json
        // 1.0.151: …731 came back …733, …921 came back …92).
        lane.start_seconds = 113.369_931_496_647_31;
        set.lanes.push(lane);
        let q = Queue::from_file(QueueFile {
            name: None,
            entries: vec![
                Item::Single {
                    location: "/m/a.flac".into(),
                    title: None,
                    seconds: Some(197.562_174_045_628_92),
                },
                Item::Set { set, from: None },
            ],
        })
        .expect("a queue");
        let file = dir.join("Sunday.rqueue");
        let to = file.to_string_lossy().into_owned();
        let origin = written_queue(&q, &to, None).expect("written");
        assert_eq!(origin.name, "Sunday");
        assert_eq!(origin.fingerprint, fingerprint(&q));
        // READ BACK: the same queue, the same fingerprint.
        let Ok(own::Own::Queue(back)) = own::read(&file) else {
            panic!("the file does not read back");
        };
        let back = Queue::from_file(back).expect("a queue");
        assert_eq!(
            fingerprint(&back),
            origin.fingerprint,
            "a queue kept whole reads back as another"
        );
        // SAVE AGAIN: the file as it was — then changed on the disk.
        assert_eq!(
            again(Some(&origin), false),
            Ok((to.clone(), "Sunday".to_owned()))
        );
        let mut text = std::fs::read(&file).expect("there");
        text.push(b'\n');
        std::fs::write(&file, text).expect("changed");
        assert_eq!(
            again(Some(&origin), false),
            Err(Trouble::ChangedOnDisk { path: to.clone() })
        );
        assert!(again(Some(&origin), true).is_ok(), "the person said so");
        let rewritten = written_queue(&q, &to, Some("Sunday".into())).expect("written");
        assert!(again(Some(&rewritten), false).is_ok(), "its own write");
        assert_eq!(again(None, true), Err(Trouble::NoOrigin));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: "is it in the queue already?" misses a file the queue holds
    /// under another spelling — a link, or a `.rqueue`'s path through a
    /// linked folder — or refuses a file it does not hold; or the refusal
    /// reaches every door's troubles instead of the one who asked.
    #[test]
    fn once_is_asked_by_where_a_file_is() {
        let dir = scratch("once");
        let real = dir.join("real");
        std::fs::create_dir_all(&real).expect("a folder");
        std::fs::write(real.join("a.flac"), b"a").expect("a file");
        std::fs::write(real.join("b.flac"), b"b").expect("a file");
        std::os::unix::fs::symlink(&real, dir.join("Music")).expect("a linked folder");
        std::os::unix::fs::symlink(real.join("a.flac"), dir.join("link.flac")).expect("a link");
        let canonical = |p: &std::path::Path| {
            std::fs::canonicalize(p)
                .expect("there")
                .to_string_lossy()
                .into_owned()
        };
        let a = canonical(&real.join("a.flac"));
        // PUT BY ITS CANONICAL PATH (a file put by a door): a link is it.
        let q = Queue::from_file(QueueFile {
            name: None,
            entries: vec![Item::Single {
                location: a.clone(),
                title: None,
                seconds: None,
            }],
        })
        .expect("a queue");
        let none = std::collections::HashMap::new();
        let link = super::super::resolve::places_of(&[Thing::Location(
            dir.join("link.flac").to_string_lossy().into_owned(),
        )]);
        assert!(matches!(
            not_queued_in(&q, &none, &link),
            Err(Refused(Trouble::AlreadyQueued { .. }))
        ));
        let b = super::super::resolve::places_of(&[Thing::Location(
            real.join("b.flac").to_string_lossy().into_owned(),
        )]);
        assert!(not_queued_in(&q, &none, &b).is_ok());
        // A `.rqueue`'S SPELLING, through the linked folder: kept as it is
        // written, and found by where it is.
        let written = dir.join("Music/a.flac").to_string_lossy().into_owned();
        let entries = vec![Item::Single {
            location: written.clone(),
            title: None,
            seconds: None,
        }];
        let aliases: std::collections::HashMap<String, String> =
            super::super::resolve::aliases_of(&entries)
                .into_iter()
                .collect();
        assert_eq!(aliases.get(&written), Some(&a));
        let spelled = Queue::from_file(QueueFile {
            name: None,
            entries,
        })
        .expect("a queue");
        assert!(
            not_queued_in(&spelled, &none, &link).is_ok(),
            "without where it is, the spelling hides it"
        );
        assert!(matches!(
            not_queued_in(&spelled, &aliases, &link),
            Err(Refused(Trouble::AlreadyQueued { .. }))
        ));
        assert!(not_queued_in(&spelled, &aliases, &b).is_ok());
        // THE ASKER'S QUESTION, and nobody else's trouble.
        let said = Trouble::NoOrigin;
        let mut troubles = vec![said.clone()];
        let refused = not_queued_in(&q, &none, &link);
        assert!(matches!(
            super::super::answer_of(refused, &mut troubles),
            Say::Refused(Trouble::AlreadyQueued { .. })
        ));
        assert_eq!(
            troubles,
            vec![said],
            "every door told of one door's question"
        );
        let refused = Err(Refused(Trouble::ChangedOnDisk { path: "x".into() }));
        assert!(matches!(
            super::super::answer_of(refused, &mut troubles),
            Say::Refused(Trouble::ChangedOnDisk { .. })
        ));
        assert_eq!(troubles, vec![Trouble::ChangedOnDisk { path: "x".into() }]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a change to the tree waits to be written, a lane dragged
    /// writes the whole queue at every motion, or a lane's last change is
    /// not given a time to be written at.
    #[test]
    fn a_lane_s_changes_are_kept_at_most_once_a_second() {
        let t0 = std::time::Instant::now();
        let later = t0 + LANES_KEPT_EVERY;
        let soon = t0 + LANES_KEPT_EVERY / 4;
        assert_eq!(
            keep_due(Some((3, 7)), (3, 7), Some(t0), later),
            Keep::Nothing,
            "nothing new"
        );
        assert_eq!(
            keep_due(Some((3, 7)), (4, 7), Some(t0), soon),
            Keep::Now,
            "the tree: at once"
        );
        assert_eq!(
            keep_due(Some((3, 7)), (3, 8), Some(t0), soon),
            Keep::After(LANES_KEPT_EVERY - LANES_KEPT_EVERY / 4),
            "a lane, a moment ago: the rest of its second"
        );
        assert_eq!(
            keep_due(Some((3, 7)), (3, 9), Some(t0), later),
            Keep::Now,
            "a lane, a second on"
        );
        assert_eq!(keep_due(None, (3, 7), None, t0), Keep::Now, "never written");
        assert_eq!(
            keep_due(Some((3, 7)), (3, 8), None, t0),
            Keep::Now,
            "never written this run"
        );
    }

    /// FAILS IF: a lane's last change — a meter let go a moment after the
    /// queue was written, on a set that then says nothing — is never written
    /// to the disk, or is written as more than one thread's write.
    #[test]
    fn a_lane_s_last_change_is_written_after_its_second() {
        crate::engine::alone::run(module_path!(), "a_lane_s_last_change_alone", &[]);
    }

    /// The half of [`a_lane_s_last_change_is_written_after_its_second`]
    /// that runs alone: an engine in a home of its own, a set kept, then its
    /// lane changed in place twice a moment after.
    #[test]
    fn a_lane_s_last_change_alone() {
        use ronoctua_player::own::{Lane, LaneId, Set};
        use std::sync::{Arc, Condvar, Mutex};
        if !crate::engine::alone::is_alone() {
            return;
        }
        let shared: Shared = Arc::new((Mutex::new(Engine::new()), Condvar::new()));
        let (lock, _) = &*shared;
        let mut set = Set::default();
        set.lanes
            .push(Lane::new(LaneId(1), LaneKind::Audio, "/m/song.flac"));
        let id = {
            let mut e = lock.lock().expect("the engine");
            e.set_shared(&shared);
            e.queue = Queue::from_file(QueueFile {
                name: None,
                entries: vec![Item::Set { set, from: None }],
            })
            .expect("a queue");
            // WRITTEN, as every change of the tree is.
            e.broadcast();
            assert_eq!(
                e.kept_revision,
                Some((e.queue.revision(), e.lanes_moved)),
                "the tree was not written at once"
            );
            let id = e.queue.leaves()[0];
            // A METER DRAGGED: two values a moment after the write, and the
            // set says nothing after the last one.
            for volume in [40, 55] {
                if let Some(set) = e.queue.set_mut(id) {
                    set.lanes[0].volume_percent = volume;
                }
                e.lanes_moved += 1;
                e.broadcast();
            }
            assert!(e.keep_armed, "nothing waits to write the last change");
            assert_ne!(
                e.kept_revision,
                Some((e.queue.revision(), e.lanes_moved)),
                "a lane's change written at every motion"
            );
            id
        };
        let kept = ronoctua_store::state_dir()
            .expect("a state folder")
            .join(QUEUE_FILE);
        let deadline = std::time::Instant::now() + 4 * LANES_KEPT_EVERY;
        loop {
            let written = {
                let e = lock.lock().expect("the engine");
                e.kept_revision == Some((e.queue.revision(), e.lanes_moved))
            };
            if written {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "the last change was never written"
            );
            std::thread::sleep(LANES_KEPT_EVERY / 10);
        }
        let Ok(own::Own::Queue(file)) = own::read(&kept) else {
            panic!("the kept queue does not read");
        };
        let back = Queue::from_file(file).expect("a queue");
        let Some(Body::Set { set, .. }) = back.find(back.leaves()[0]).map(|e| &e.body) else {
            panic!("no set kept");
        };
        assert_eq!(
            set.lanes[0].volume_percent, 55,
            "the last value is not the one kept"
        );
        let e = lock.lock().expect("the engine");
        assert!(!e.keep_armed, "the write is still waiting");
        assert!(e.queue.find(id).is_some());
    }

    /// FAILS IF: a file changed on the disk is not told from the one read —
    /// a rewrite of the same length, a file renamed over it — or a file that
    /// did not change is.
    #[test]
    fn a_file_is_its_length_time_and_inode() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-engine-stamp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let file = dir.join("q.rqueue");
        std::fs::write(&file, b"one").expect("written");
        let first = FileStamp::of(&file).expect("there");
        assert_eq!(FileStamp::of(&file), Some(first));
        let other = dir.join("new");
        std::fs::write(&other, b"one").expect("written");
        std::fs::rename(&other, &file).expect("renamed over it");
        assert_ne!(FileStamp::of(&file), Some(first), "another inode");
        assert_eq!(FileStamp::of(&dir.join("gone")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a title learned for an address names a leaf somebody named,
    /// one that is no longer that address, or names it after its own
    /// address — or a leaf that is still what it arrived as is not named.
    #[test]
    fn a_learned_title_names_only_an_unnamed_address() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        let mut q = Queue::from_file(QueueFile {
            name: None,
            entries: vec![
                Item::Single {
                    location: url.into(),
                    title: None,
                    seconds: None,
                },
                Item::Single {
                    location: url.into(),
                    title: Some("Named by the person".into()),
                    seconds: None,
                },
            ],
        })
        .expect("a queue");
        let (a, b) = (q.leaves()[0], q.leaves()[1]);
        assert!(!name_if_unnamed(&mut q, a, url, "  "));
        assert!(
            !name_if_unnamed(&mut q, a, url, "watch"),
            "its address's own last piece is no title"
        );
        assert!(!name_if_unnamed(
            &mut q,
            a,
            "https://other.example/x",
            "Other"
        ));
        assert!(name_if_unnamed(&mut q, a, url, " A song "));
        assert_eq!(
            q.find(a).map(|e| e.body.name()),
            Some(Name::Titled("A song".into()))
        );
        assert!(!name_if_unnamed(&mut q, a, url, "Again"), "named once");
        assert!(!name_if_unnamed(&mut q, b, url, "A song"));
    }

    /// FAILS IF: a set is played by one of its subtitles, or a lane switched
    /// off leads it.
    #[test]
    fn a_set_is_led_by_its_first_picture_then_its_first_sound() {
        use ronoctua_player::own::{Lane, LaneId, Set};
        let mut set = Set::default();
        set.lanes
            .push(Lane::new(LaneId(1), LaneKind::Subtitle, "/s.srt"));
        set.lanes
            .push(Lane::new(LaneId(2), LaneKind::Audio, "/a.opus"));
        set.lanes
            .push(Lane::new(LaneId(3), LaneKind::Video, "/v.mkv"));
        assert_eq!(lead_lane(&set).map(|l| l.id), Some(LaneId(3)));
        set.lanes[2].enabled = false;
        assert_eq!(lead_lane(&set).map(|l| l.id), Some(LaneId(2)));
    }

    /// FAILS IF: a set's name as this daemon says it is ever one file's when
    /// it plays several.
    #[test]
    fn a_name_is_said_whole() {
        assert_eq!(name_text(&Name::Location("/m/a b.flac".into())), "a b.flac");
        assert_eq!(
            name_text(&Name::Together(vec![
                "Film".into(),
                "commentary.opus".into()
            ])),
            "Film + commentary.opus"
        );
    }
}
