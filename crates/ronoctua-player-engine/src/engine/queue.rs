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
        // WHAT WAS READ IS WHAT IS ON THE DISK — unless it came from version
        // 1, which the first broadcast writes in the new form.
        if !imported {
            self.kept_revision = Some(self.queue.revision());
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
            Ok(()) => self.kept_revision = Some(self.queue.revision()),
            Err(e) => tracing::warn!(error = %e, "could not keep the queue"),
        }
        self.remember_where();
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

    /// Empties the queue.
    pub(super) fn clear(&mut self) {
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
        let mut subtitles = Vec::new();
        let mut whole: Option<Vec<Item>> = None;
        let count = resolved.len();
        for r in resolved {
            match r {
                Ok(Resolved::Now(item)) => {
                    items.push(item);
                    jobs.push(None);
                }
                Ok(Resolved::Queue { item, entries }) => {
                    // A QUEUE FILE OPENED TO START OVER IS the queue, not a
                    // playlist inside it.
                    if replace && count == 1 {
                        whole = Some(entries);
                    }
                    items.push(item);
                    jobs.push(None);
                }
                Ok(Resolved::Later { placeholder, job }) => {
                    items.push(placeholder);
                    jobs.push(Some(job));
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
            let ids = match whole {
                Some(entries) => {
                    jobs = vec![None; entries.len()];
                    self.queue.replace(entries)?
                }
                None => self.queue.replace(items)?,
            };
            self.resolving.clear();
            self.play_when_resolved = None;
            self.resume = None;
            self.set_current(None);
            ids
        } else {
            self.queue.insert(place, items)?
        };
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

    /// Writes the queue as a `.rqueue` at `to`.
    pub(super) fn save_queue(&self, to: &str, name: Option<String>) -> Result<(), Refused> {
        let unwritable = |why: &str| Trouble::Unwritable {
            path: to.to_owned(),
            why: why.to_owned(),
        };
        let path = Path::new(to);
        let folder = path
            .parent()
            .filter(|p| p.is_absolute())
            .ok_or_else(|| unwritable("not an absolute path"))?;
        let file = path
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| unwritable("no file name"))?;
        let text = own::queue_to_text(&self.queue.kept(name), folder);
        ronoctua_xdg::write_atomically(folder, file, text.as_bytes())
            .map_err(|e| unwritable(&e.to_string()))?;
        Ok(())
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
        if self.queue.set_body(opened, item).is_err() {
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
