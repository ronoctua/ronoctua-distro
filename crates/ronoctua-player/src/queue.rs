//! The queue: our own tree, and the one answer to "what plays next".
//!
//! # Why the queue is not mpv's playlist any more
//!
//! mpv's playlist is flat. A playlist inside the queue lost its name, an XSPF
//! was flattened, and a set of media played together had nowhere to live. So
//! the tree is ours: files, sets, named playlists that fold, and streams, each
//! an [`Entry`] with an [`EntryId`] that is never handed out twice in one
//! daemon's life — MPRIS's track id, a door's cursor and "play this one" all
//! survive a move.
//!
//! mpv is still what PLAYS, through a window of two entries the engine keeps
//! (the current leaf and the one after), because gapless audio only works
//! across mpv's own playlist. This module is pure: it answers questions about
//! the tree and changes it, and never touches a player.
//!
//! # Ceilings are refusals
//!
//! [`LEAVES_MOST`] things to play and [`DEPTH_MOST`] levels of nesting, and a
//! set's own ceilings ([`crate::session::refusal`]) for every set that comes
//! in. A change that would pass one is refused whole, as a value with the
//! number in it; the queue is left exactly as it was.

use std::collections::HashSet;
use std::path::Path;

use ronoctua_playlist::own::{Item, QueueFile, Set, Stream};
use serde::{Deserialize, Serialize};

use crate::limit::{Limit, TooMany};
use crate::wire::Repeat;

/// The most things to play one queue holds.
///
/// A person's whole music folder is a few thousand files; past ten thousand
/// the outline sent to every door on each change stops being small, and the
/// shuffle order stops being cheap to redo.
pub const LEAVES_MOST: usize = 10_000;

/// The deepest an entry sits: a playlist inside a playlist inside a playlist,
/// and the file in it.
///
/// Bounded because every walk of the tree is recursive, and because a door
/// indents a level a step: past four the names are pushed off a column.
pub const DEPTH_MOST: usize = 4;

/// An entry's name for the daemon's life.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntryId(pub u64);

/// One entry of the queue.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// Its name, never reused.
    pub id: EntryId,
    /// What it is.
    pub body: Body,
}

/// What an entry is.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// One file or address.
    Single {
        /// The path or URL.
        location: String,
        /// Its title, once known.
        title: Option<String>,
        /// How long it runs, when known.
        seconds: Option<f64>,
    },
    /// Media played together.
    Set {
        /// The set.
        set: Set,
        /// The `.rset` it came from.
        from: Option<String>,
    },
    /// A named list of more entries.
    Playlist {
        /// Its name.
        name: String,
        /// The file or address it was read from.
        from: Option<String>,
        /// What it holds.
        children: Vec<Entry>,
    },
    /// Something from the network.
    Stream(Stream),
}

impl Body {
    /// Is it something to play, rather than a list of them?
    #[must_use]
    pub const fn is_leaf(&self) -> bool {
        !matches!(self, Self::Playlist { .. })
    }

    /// What kind of entry the outline says it is.
    #[must_use]
    pub const fn kind(&self) -> NodeKind {
        match self {
            Self::Single { .. } => NodeKind::Single,
            Self::Set { .. } => NodeKind::Set,
            Self::Playlist { .. } => NodeKind::Playlist,
            Self::Stream(_) => NodeKind::Stream,
        }
    }

    /// What it is called — the facts a door words.
    ///
    /// # The rule
    ///
    /// A file is its title, else its file name. A playlist is its name. A
    /// stream is its title, else its address. A SET is the name it was saved
    /// with, else the name of the `.rset` it came from, else EVERY distinct
    /// source it plays, together — never the name of one file, because a set
    /// named after its film hides that a commentary and a dubbing play with
    /// it (the owner's rule: *"vários juntos: nunca o nome de um arquivo
    /// só"*). A set of one source is that source's name, because then it IS
    /// one medium.
    #[must_use]
    pub fn name(&self) -> Name {
        let given = |text: Option<&str>| {
            text.map(str::trim)
                .filter(|t| !t.is_empty())
                .map(ToOwned::to_owned)
        };
        match self {
            Self::Single {
                location, title, ..
            } => given(title.as_deref())
                .map_or_else(|| Name::Location(location.clone()), Name::Titled),
            Self::Playlist { name, .. } => Name::Titled(name.clone()),
            Self::Stream(stream) => given(stream.title.as_deref())
                .map_or_else(|| Name::Location(stream.url.clone()), Name::Titled),
            Self::Set { set, from } => given(set.name.as_deref())
                .or_else(|| {
                    from.as_deref()
                        .and_then(|from| Path::new(from).file_stem())
                        .and_then(|stem| stem.to_str())
                        .filter(|stem| !stem.trim().is_empty())
                        .map(ToOwned::to_owned)
                })
                .map_or_else(|| Name::Together(sources(set)), Name::Titled),
        }
    }

    /// As a file keeps it: without ids.
    #[must_use]
    pub fn kept(&self) -> Item {
        match self {
            Self::Single {
                location,
                title,
                seconds,
            } => Item::Single {
                location: location.clone(),
                title: title.clone(),
                seconds: *seconds,
            },
            Self::Set { set, from } => Item::Set {
                set: set.clone(),
                from: from.clone(),
            },
            Self::Playlist {
                name,
                from,
                children,
            } => Item::Playlist {
                name: name.clone(),
                from: from.clone(),
                entries: children.iter().map(|child| child.body.kept()).collect(),
            },
            Self::Stream(stream) => Item::Stream(stream.clone()),
        }
    }
}

/// Every distinct source of a set, in lane order, each by the title one of
/// its lanes gave, else its file name.
fn sources(set: &Set) -> Vec<String> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for lane in &set.lanes {
        if seen.contains(&lane.source.as_str()) {
            continue;
        }
        seen.push(&lane.source);
        let title = set
            .lanes
            .iter()
            .filter(|other| other.source == lane.source)
            .find_map(|other| {
                other
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
            });
        out.push(title.unwrap_or_else(|| file_name(&lane.source)).to_owned());
    }
    out
}

/// A location's last piece: a path's file name, an address's last segment
/// (else its host).
///
/// What a door shows for something with no title, before it knows more.
#[must_use]
pub fn file_name(location: &str) -> &str {
    if let Some((_, rest)) = location.split_once("://") {
        let rest = rest.split(['?', '#']).next().unwrap_or(rest);
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        return path
            .rsplit('/')
            .find(|piece| !piece.is_empty())
            .unwrap_or(host);
    }
    Path::new(location)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(location)
}

/// Several names as one, cut to `width_cells` — a set's name when nobody
/// gave it one.
///
/// `joiner` is the caller's, because it is text on a screen and comes from a
/// text key (invariant 8): the shell passes its `player-set-joiner`. The cut
/// is by grapheme clusters and measured in cells — the one ruler,
/// `ronoctua_protocol::cells` — so an ideograph or an emoji is never split
/// into a different drawing, and ends with the renderer's own `…`, which is
/// one cell.
#[must_use]
pub fn together(parts: &[String], joiner: &str, width_cells: u16) -> String {
    let joined = parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(joiner);
    if ronoctua_protocol::cells(&joined) <= width_cells {
        return joined;
    }
    if width_cells == 0 {
        return String::new();
    }
    let room = width_cells - 1;
    let mut used = 0u16;
    let mut out = String::new();
    for cluster in ronoctua_protocol::cells_of(&joined) {
        let one = ronoctua_protocol::cells_of_one(cluster);
        if used.saturating_add(one) > room {
            break;
        }
        used += one;
        out.push_str(cluster);
    }
    // NO SPACE BEFORE THE MARK: `Film +…`, never `Film + …`.
    out.truncate(out.trim_end().len());
    out.push('…');
    out
}

/// What an entry is called, as facts; the door words them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Name {
    /// A title, a playlist's name or a set's saved name.
    Titled(String),
    /// Nothing named it: the door shows [`file_name`] of this.
    Location(String),
    /// An unnamed set: every distinct source, joined by the door with
    /// [`together`].
    Together(Vec<String>),
}

/// The kind of an outline's node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    /// One file or address.
    Single,
    /// Media played together.
    Set,
    /// A named list.
    Playlist,
    /// Something from the network.
    Stream,
}

/// One entry of the outline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Node {
    /// Its id.
    pub id: EntryId,
    /// What it is called.
    pub name: Name,
    /// What it is.
    pub kind: NodeKind,
    /// A playlist's entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    /// How many lanes a set has; zero for anything else.
    #[serde(default)]
    pub lanes: u16,
}

/// The queue as the doors see it: its shape and names, nothing a lane does.
///
/// Sent whole, and only when it changes — a volume slid on a lane changes the
/// state, not this.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Outline {
    /// The queue's revision it was taken at.
    pub revision: u64,
    /// The top level.
    pub entries: Vec<Node>,
}

/// Where new or moved entries go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// First in a playlist, or first in the queue when `None`.
    Start(Option<EntryId>),
    /// Last in a playlist, or last in the queue when `None`.
    End(Option<EntryId>),
    /// Right after this entry, beside it.
    After(EntryId),
}

/// Why the queue moves on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// What played reached its end by itself.
    Ended,
    /// A person asked for the next one.
    Asked,
}

/// Why a change to the queue was not made — each a sentence on screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "refusal")]
pub enum Refusal {
    /// It would pass a ceiling.
    TooMany(TooMany),
    /// No entry has this id (any more).
    NoSuchEntry {
        /// The id asked about.
        id: EntryId,
    },
    /// Only a playlist holds entries.
    NotAPlaylist {
        /// The entry that is not one.
        id: EntryId,
    },
    /// A playlist cannot go inside itself.
    IntoItself,
    /// A playlist always has a name.
    Nameless,
}

impl From<TooMany> for Refusal {
    fn from(too_many: TooMany) -> Self {
        Self::TooMany(too_many)
    }
}

/// The queue.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Queue {
    /// Moves on every change to the tree.
    revision: u64,
    /// The top level.
    entries: Vec<Entry>,
    /// The last id handed out.
    issued: u64,
}

impl Queue {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A queue of what a file kept, with fresh ids.
    ///
    /// # Errors
    ///
    /// What [`Queue::insert`] refuses.
    pub fn from_file(file: QueueFile) -> Result<Self, Refusal> {
        let mut queue = Self::new();
        queue.insert(Place::End(None), file.entries)?;
        Ok(queue)
    }

    /// The queue as a file keeps it.
    #[must_use]
    pub fn kept(&self, name: Option<String>) -> QueueFile {
        QueueFile {
            name,
            entries: self.entries.iter().map(|entry| entry.body.kept()).collect(),
        }
    }

    /// Its revision: it moves whenever the tree changes.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// The top level.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Nothing in it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Say the tree changed in a way the outline shows, after a change made
    /// through [`Queue::set_mut`] (a lane added or removed).
    pub const fn touch(&mut self) {
        self.revision += 1;
    }

    /// A fresh id.
    const fn issue(&mut self) -> EntryId {
        self.issued += 1;
        EntryId(self.issued)
    }

    /// An item with ids, parents before their children.
    fn adopt(&mut self, item: Item) -> Entry {
        let id = self.issue();
        let body = match item {
            Item::Single {
                location,
                title,
                seconds,
            } => Body::Single {
                location,
                title,
                seconds,
            },
            Item::Set { set, from } => Body::Set { set, from },
            Item::Playlist {
                name,
                from,
                entries,
            } => Body::Playlist {
                name,
                from,
                children: entries.into_iter().map(|item| self.adopt(item)).collect(),
            },
            Item::Stream(stream) => Body::Stream(stream),
        };
        Entry { id, body }
    }

    /// Put `items` at `place`, and give back their ids.
    ///
    /// # Errors
    ///
    /// [`Refusal::NoSuchEntry`] or [`Refusal::NotAPlaylist`] for a place that
    /// is not one, [`Refusal::Nameless`] for a playlist with no name, and
    /// [`Refusal::TooMany`] when the queue would pass [`LEAVES_MOST`] or
    /// [`DEPTH_MOST`], or a set its own ceilings. Nothing is changed then.
    pub fn insert(&mut self, place: Place, items: Vec<Item>) -> Result<Vec<EntryId>, Refusal> {
        let depth = self.depth_at(place)?;
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let adding: usize = items.iter().map(leaves_in).sum();
        if self.leaf_count().saturating_add(adding) > LEAVES_MOST {
            return Err(too_many(Limit::Leaves, LEAVES_MOST));
        }
        for item in &items {
            admit(item, depth)?;
        }
        let adopted: Vec<Entry> = items.into_iter().map(|item| self.adopt(item)).collect();
        let ids = adopted.iter().map(|entry| entry.id).collect();
        let (list, at) = self.slot_mut(place)?;
        list.splice(at..at, adopted);
        self.touch();
        Ok(ids)
    }

    /// Empty it, and put `items` in.
    ///
    /// # Errors
    ///
    /// What [`Queue::insert`] refuses; the queue is kept as it was then.
    pub fn replace(&mut self, items: Vec<Item>) -> Result<Vec<EntryId>, Refusal> {
        let before = std::mem::take(&mut self.entries);
        match self.insert(Place::End(None), items) {
            Ok(ids) => {
                if ids.is_empty() && !before.is_empty() {
                    self.touch();
                }
                Ok(ids)
            }
            Err(refusal) => {
                self.entries = before;
                Err(refusal)
            }
        }
    }

    /// Empty it.
    pub fn clear(&mut self) {
        if !self.entries.is_empty() {
            self.entries.clear();
            self.touch();
        }
    }

    /// Take an entry out, and everything in it.
    ///
    /// # Errors
    ///
    /// [`Refusal::NoSuchEntry`].
    pub fn remove(&mut self, id: EntryId) -> Result<Entry, Refusal> {
        let path = self.path_of(id).ok_or(Refusal::NoSuchEntry { id })?;
        let (last, above) = path.split_last().ok_or(Refusal::NoSuchEntry { id })?;
        let list = list_at_mut(&mut self.entries, above).ok_or(Refusal::NoSuchEntry { id })?;
        let entry = list.remove(*last);
        self.touch();
        Ok(entry)
    }

    /// Move an entry, keeping its id, to `place`.
    ///
    /// # Errors
    ///
    /// [`Refusal::IntoItself`] for a playlist moved inside itself, and what
    /// [`Queue::insert`] refuses for the place and the depth.
    pub fn move_to(&mut self, id: EntryId, place: Place) -> Result<(), Refusal> {
        let moving = self.find(id).ok_or(Refusal::NoSuchEntry { id })?;
        let target = match place {
            Place::After(beside) if beside == id => return Ok(()),
            Place::After(beside) => Some(beside),
            Place::Start(into) | Place::End(into) => into,
        };
        if let Some(target) = target
            && (target == id || holds(moving, target))
        {
            return Err(Refusal::IntoItself);
        }
        let height = entry_height(moving);
        let depth = self.depth_at(place)?;
        if depth + height - 1 > DEPTH_MOST {
            return Err(too_many(Limit::Depth, DEPTH_MOST));
        }
        let path = self.path_of(id).ok_or(Refusal::NoSuchEntry { id })?;
        let (last, above) = path.split_last().ok_or(Refusal::NoSuchEntry { id })?;
        let list = list_at_mut(&mut self.entries, above).ok_or(Refusal::NoSuchEntry { id })?;
        let entry = list.remove(*last);
        match self.slot_mut(place) {
            Ok((list, at)) => list.insert(at, entry),
            Err(refusal) => {
                // PUT BACK where it was: its parent is not inside it, so the
                // path above it still leads there.
                if let Some(list) = list_at_mut(&mut self.entries, above) {
                    list.insert(*last, entry);
                }
                return Err(refusal);
            }
        }
        self.touch();
        Ok(())
    }

    /// Call an entry something else. An empty name takes a file's, a set's
    /// or a stream's own name away (it goes back to what [`Body::name`]
    /// finds); a playlist always has one.
    ///
    /// # Errors
    ///
    /// [`Refusal::NoSuchEntry`], and [`Refusal::Nameless`] for a playlist.
    pub fn rename(&mut self, id: EntryId, name: &str) -> Result<(), Refusal> {
        let name = name.trim();
        let given = (!name.is_empty()).then(|| name.to_owned());
        let path = self.path_of(id).ok_or(Refusal::NoSuchEntry { id })?;
        let entry = entry_at_mut(&mut self.entries, &path).ok_or(Refusal::NoSuchEntry { id })?;
        match &mut entry.body {
            Body::Single { title, .. } => *title = given,
            Body::Set { set, .. } => set.name = given,
            Body::Stream(stream) => stream.title = given,
            Body::Playlist { name: kept, .. } => *kept = given.ok_or(Refusal::Nameless)?,
        }
        self.touch();
        Ok(())
    }

    /// Make an entry something else, keeping its id: a placeholder once what
    /// it stood for is resolved, or a file that became a set.
    ///
    /// # Errors
    ///
    /// What [`Queue::insert`] refuses.
    pub fn set_body(&mut self, id: EntryId, item: Item) -> Result<(), Refusal> {
        let path = self.path_of(id).ok_or(Refusal::NoSuchEntry { id })?;
        let old = self.find(id).map_or(0, entry_leaves);
        let count = self.leaf_count() - old + leaves_in(&item);
        if count > LEAVES_MOST {
            return Err(too_many(Limit::Leaves, LEAVES_MOST));
        }
        admit(&item, path.len())?;
        let mut entry = self.adopt(item);
        entry.id = id;
        let slot = entry_at_mut(&mut self.entries, &path).ok_or(Refusal::NoSuchEntry { id })?;
        *slot = entry;
        self.touch();
        Ok(())
    }

    /// A set's lanes and arrangement, to change what they do.
    ///
    /// This does NOT move the revision: a lane's volume is the state's news,
    /// not the outline's. When the number of lanes changes, call
    /// [`Queue::touch`] — the outline carries it.
    pub fn set_mut(&mut self, id: EntryId) -> Option<&mut Set> {
        let path = self.path_of(id)?;
        match &mut entry_at_mut(&mut self.entries, &path)?.body {
            Body::Set { set, .. } => Some(set),
            _ => None,
        }
    }

    /// The entry named `id`.
    #[must_use]
    pub fn find(&self, id: EntryId) -> Option<&Entry> {
        self.at_path(&self.path_of(id)?)
    }

    /// Where `id` is: its index at each level, from the top.
    ///
    /// What the daemon remembers across a restart — ids are not kept, and a
    /// path into the same saved tree is the same entry.
    #[must_use]
    pub fn path_of(&self, id: EntryId) -> Option<Vec<usize>> {
        let mut path = Vec::new();
        path_in(&self.entries, id, &mut path).then_some(path)
    }

    /// The entry at a path from [`Queue::path_of`].
    #[must_use]
    pub fn at_path(&self, path: &[usize]) -> Option<&Entry> {
        let (last, above) = path.split_last()?;
        let mut list = &self.entries;
        for &n in above {
            match &list.get(n)?.body {
                Body::Playlist { children, .. } => list = children,
                _ => return None,
            }
        }
        list.get(*last)
    }

    /// Every thing to play, in the tree's order.
    #[must_use]
    pub fn leaves(&self) -> Vec<EntryId> {
        let mut out = Vec::new();
        leaves_of(&self.entries, &mut out);
        out
    }

    /// How many things to play there are.
    #[must_use]
    pub fn leaf_count(&self) -> usize {
        self.entries.iter().map(entry_leaves).sum()
    }

    /// The order things play in: the tree's, or `shuffle`'s.
    ///
    /// A shuffle order made before the queue changed still works: an id that
    /// is gone is skipped, and a leaf it does not know plays after it, in the
    /// tree's order — nothing is lost and nothing plays twice.
    #[must_use]
    pub fn order(&self, shuffle: Option<&[EntryId]>) -> Vec<EntryId> {
        let leaves = self.leaves();
        let Some(shuffle) = shuffle else {
            return leaves;
        };
        let known: HashSet<EntryId> = leaves.iter().copied().collect();
        let mut seen = HashSet::with_capacity(leaves.len());
        let mut out: Vec<EntryId> = shuffle
            .iter()
            .copied()
            .filter(|id| known.contains(id) && seen.insert(*id))
            .collect();
        out.extend(leaves.into_iter().filter(|id| !seen.contains(id)));
        out
    }

    /// What plays after `current`.
    ///
    /// `None` for `current` is "nothing is playing", and the answer is the
    /// first. With [`Repeat::One`] a leaf that ENDED plays again, and one a
    /// person skipped moves on; [`Repeat::Queue`] goes round.
    #[must_use]
    pub fn next(
        &self,
        current: Option<EntryId>,
        repeat: Repeat,
        step: Step,
        shuffle: Option<&[EntryId]>,
    ) -> Option<EntryId> {
        let order = self.order(shuffle);
        let Some(current) = current else {
            return order.first().copied();
        };
        let at = order.iter().position(|id| *id == current)?;
        if step == Step::Ended && repeat == Repeat::One {
            return Some(current);
        }
        match order.get(at + 1) {
            Some(next) => Some(*next),
            None if repeat == Repeat::Queue => order.first().copied(),
            None => None,
        }
    }

    /// What played before `current`; [`Repeat::Queue`] goes round.
    #[must_use]
    pub fn previous(
        &self,
        current: Option<EntryId>,
        repeat: Repeat,
        shuffle: Option<&[EntryId]>,
    ) -> Option<EntryId> {
        let order = self.order(shuffle);
        let at = order.iter().position(|id| Some(*id) == current)?;
        match at.checked_sub(1) {
            Some(before) => order.get(before).copied(),
            None if repeat == Repeat::Queue => order.last().copied(),
            None => None,
        }
    }

    /// What `current` becomes once `removed` is taken out — asked BEFORE
    /// removing it.
    ///
    /// `current` itself when it is not inside what goes; otherwise the next
    /// leaf in the order that stays (round, with [`Repeat::Queue`]); `None`
    /// when nothing after it stays.
    #[must_use]
    pub fn after_removing(
        &self,
        current: Option<EntryId>,
        removed: EntryId,
        repeat: Repeat,
        shuffle: Option<&[EntryId]>,
    ) -> Option<EntryId> {
        let current = current?;
        let mut gone = Vec::new();
        if let Some(entry) = self.find(removed) {
            leaves_of(std::slice::from_ref(entry), &mut gone);
        }
        if !gone.contains(&current) {
            return Some(current);
        }
        let order = self.order(shuffle);
        let at = order.iter().position(|id| *id == current)?;
        let stays = |id: &&EntryId| !gone.contains(id);
        match order[at + 1..].iter().find(stays) {
            Some(next) => Some(*next),
            None if repeat == Repeat::Queue => order[..at].iter().find(stays).copied(),
            None => None,
        }
    }

    /// The queue's shape and names, for the doors.
    #[must_use]
    pub fn outline(&self) -> Outline {
        Outline {
            revision: self.revision,
            entries: self.entries.iter().map(node).collect(),
        }
    }

    /// How deep a new entry at `place` sits, or why `place` is not one.
    fn depth_at(&self, place: Place) -> Result<usize, Refusal> {
        match place {
            Place::Start(None) | Place::End(None) => Ok(1),
            Place::Start(Some(into)) | Place::End(Some(into)) => {
                let path = self
                    .path_of(into)
                    .ok_or(Refusal::NoSuchEntry { id: into })?;
                match self.at_path(&path).map(|entry| &entry.body) {
                    Some(Body::Playlist { .. }) => Ok(path.len() + 1),
                    _ => Err(Refusal::NotAPlaylist { id: into }),
                }
            }
            Place::After(beside) => self
                .path_of(beside)
                .map(|path| path.len())
                .ok_or(Refusal::NoSuchEntry { id: beside }),
        }
    }

    /// The list `place` is in, and the index in it.
    fn slot_mut(&mut self, place: Place) -> Result<(&mut Vec<Entry>, usize), Refusal> {
        match place {
            Place::Start(into) | Place::End(into) => {
                let list = match into {
                    None => &mut self.entries,
                    Some(into) => {
                        let path = self
                            .path_of(into)
                            .ok_or(Refusal::NoSuchEntry { id: into })?;
                        match entry_at_mut(&mut self.entries, &path).map(|entry| &mut entry.body) {
                            Some(Body::Playlist { children, .. }) => children,
                            _ => return Err(Refusal::NotAPlaylist { id: into }),
                        }
                    }
                };
                let at = if matches!(place, Place::Start(_)) {
                    0
                } else {
                    list.len()
                };
                Ok((list, at))
            }
            Place::After(beside) => {
                let path = self
                    .path_of(beside)
                    .ok_or(Refusal::NoSuchEntry { id: beside })?;
                let (last, above) = path
                    .split_last()
                    .ok_or(Refusal::NoSuchEntry { id: beside })?;
                let list = list_at_mut(&mut self.entries, above)
                    .ok_or(Refusal::NoSuchEntry { id: beside })?;
                Ok((list, last + 1))
            }
        }
    }
}

/// A shuffle order: every leaf once, `first` first, the rest in an order
/// `seed` decides.
///
/// The seed is the caller's — the daemon draws it from the system's random
/// source — so this stays pure and a test can name the order it expects.
/// `first` that is not a leaf is ignored.
#[must_use]
pub fn shuffled(leaves: &[EntryId], first: Option<EntryId>, seed: u64) -> Vec<EntryId> {
    let first = first.filter(|first| leaves.contains(first));
    let mut rest: Vec<EntryId> = leaves
        .iter()
        .copied()
        .filter(|id| Some(*id) != first)
        .collect();
    // FISHER–YATES, from the end.
    let mut state = seed;
    for i in (1..rest.len()).rev() {
        let bound = u64::try_from(i + 1).unwrap_or(u64::MAX);
        let j = usize::try_from(splitmix(&mut state) % bound).unwrap_or(0);
        rest.swap(i, j);
    }
    first.into_iter().chain(rest).collect()
}

/// SplitMix64: a small generator whose whole state is the seed. Uniform
/// enough to shuffle a playlist, and not a secret of anybody's.
const fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The ceiling `what` refused.
const fn too_many(what: Limit, most: usize) -> Refusal {
    Refusal::TooMany(TooMany { what, most })
}

/// Whether `item` may go in at `depth`: its depth, its playlists' names and
/// its sets' own ceilings.
fn admit(item: &Item, depth: usize) -> Result<(), Refusal> {
    if depth + item_height(item) - 1 > DEPTH_MOST {
        return Err(too_many(Limit::Depth, DEPTH_MOST));
    }
    match item {
        Item::Single { .. } | Item::Stream(_) => Ok(()),
        Item::Set { set, .. } => crate::session::refusal(set).map_or(Ok(()), |t| Err(t.into())),
        Item::Playlist { name, entries, .. } => {
            if name.trim().is_empty() {
                return Err(Refusal::Nameless);
            }
            entries.iter().try_for_each(|item| admit(item, depth + 1))
        }
    }
}

/// How many levels an item spans: one for a leaf.
fn item_height(item: &Item) -> usize {
    match item {
        Item::Playlist { entries, .. } => 1 + entries.iter().map(item_height).max().unwrap_or(0),
        _ => 1,
    }
}

/// How many levels an entry spans.
fn entry_height(entry: &Entry) -> usize {
    match &entry.body {
        Body::Playlist { children, .. } => 1 + children.iter().map(entry_height).max().unwrap_or(0),
        _ => 1,
    }
}

/// How many things to play an item holds.
fn leaves_in(item: &Item) -> usize {
    match item {
        Item::Playlist { entries, .. } => entries.iter().map(leaves_in).sum(),
        _ => 1,
    }
}

/// How many things to play an entry holds.
fn entry_leaves(entry: &Entry) -> usize {
    match &entry.body {
        Body::Playlist { children, .. } => children.iter().map(entry_leaves).sum(),
        _ => 1,
    }
}

/// Whether `id` is somewhere inside `entry`.
fn holds(entry: &Entry, id: EntryId) -> bool {
    match &entry.body {
        Body::Playlist { children, .. } => children
            .iter()
            .any(|child| child.id == id || holds(child, id)),
        _ => false,
    }
}

/// The leaves under `list`, in order.
fn leaves_of(list: &[Entry], out: &mut Vec<EntryId>) {
    for entry in list {
        match &entry.body {
            Body::Playlist { children, .. } => leaves_of(children, out),
            _ => out.push(entry.id),
        }
    }
}

/// Fills `path` with where `id` is under `list`.
fn path_in(list: &[Entry], id: EntryId, path: &mut Vec<usize>) -> bool {
    for (n, entry) in list.iter().enumerate() {
        path.push(n);
        if entry.id == id {
            return true;
        }
        if let Body::Playlist { children, .. } = &entry.body
            && path_in(children, id, path)
        {
            return true;
        }
        path.pop();
    }
    false
}

/// The list of a playlist at `path` (the top level for an empty one).
fn list_at_mut<'a>(mut list: &'a mut Vec<Entry>, path: &[usize]) -> Option<&'a mut Vec<Entry>> {
    for &n in path {
        match &mut list.get_mut(n)?.body {
            Body::Playlist { children, .. } => list = children,
            _ => return None,
        }
    }
    Some(list)
}

/// The entry at `path`.
fn entry_at_mut<'a>(list: &'a mut Vec<Entry>, path: &[usize]) -> Option<&'a mut Entry> {
    let (last, above) = path.split_last()?;
    list_at_mut(list, above)?.get_mut(*last)
}

/// An entry as the outline carries it.
fn node(entry: &Entry) -> Node {
    let (children, lanes) = match &entry.body {
        Body::Playlist { children, .. } => (children.iter().map(node).collect(), 0),
        Body::Set { set, .. } => (
            Vec::new(),
            u16::try_from(set.lanes.len()).unwrap_or(u16::MAX),
        ),
        Body::Single { .. } | Body::Stream(_) => (Vec::new(), 0),
    };
    Node {
        id: entry.id,
        name: entry.body.name(),
        kind: entry.body.kind(),
        children,
        lanes,
    }
}

#[cfg(test)]
mod tests {
    use ronoctua_playlist::own::{Lane, LaneId, LaneKind};

    use super::*;

    /// A file.
    fn single(location: &str) -> Item {
        Item::Single {
            location: location.to_owned(),
            title: None,
            seconds: None,
        }
    }

    /// A playlist of `entries`.
    fn playlist(name: &str, entries: Vec<Item>) -> Item {
        Item::Playlist {
            name: name.to_owned(),
            from: None,
            entries,
        }
    }

    /// A set of these lanes, `(kind, source, title)`.
    fn set_of(lanes: &[(LaneKind, &str, Option<&str>)]) -> Set {
        Set {
            name: None,
            lanes: lanes
                .iter()
                .zip(1..)
                .map(|((kind, source, title), n)| {
                    let mut lane = Lane::new(LaneId(n), *kind, *source);
                    lane.title = title.map(ToOwned::to_owned);
                    lane
                })
                .collect(),
            arrangement: ronoctua_playlist::own::Arrangement::default(),
        }
    }

    /// `a`, then a playlist `P` of `b` and a playlist `Q` of `c`, then `d` —
    /// and the ids of `a`, `b`, `c`, `d`, `P`, `Q`.
    fn a_tree() -> (Queue, [EntryId; 6]) {
        let mut queue = Queue::new();
        queue
            .insert(
                Place::End(None),
                vec![
                    single("/a.flac"),
                    playlist(
                        "P",
                        vec![single("/b.flac"), playlist("Q", vec![single("/c.flac")])],
                    ),
                    single("/d.flac"),
                ],
            )
            .expect("inserted");
        let leaves = queue.leaves();
        let p = queue.entries()[1].id;
        let Body::Playlist { children, .. } = &queue.entries()[1].body else {
            panic!("a playlist");
        };
        let q = children[1].id;
        (queue, [leaves[0], leaves[1], leaves[2], leaves[3], p, q])
    }

    /// FAILS IF: an id is ever handed out twice, or an entry loses its id
    /// when it is resolved or moved — MPRIS and a door's cursor hold them.
    #[test]
    fn an_id_is_never_handed_out_twice() {
        let (mut queue, [a, b, _, d, p, _]) = a_tree();
        let before: Vec<EntryId> = queue.leaves();
        queue.remove(p).expect("removed");
        let again = queue
            .insert(Place::End(None), vec![single("/e.flac")])
            .expect("inserted");
        assert!(again.iter().all(|id| !before.contains(id) && *id != p));
        // RESOLVED IN PLACE: same id, new body.
        queue
            .set_body(a, playlist("Resolved", vec![single("/x.flac")]))
            .expect("resolved");
        assert_eq!(
            queue.find(a).map(|e| e.body.kind()),
            Some(NodeKind::Playlist)
        );
        // MOVED: same id, new place.
        queue.move_to(d, Place::Start(None)).expect("moved");
        assert_eq!(queue.entries()[0].id, d);
        assert!(queue.find(b).is_none());
    }

    /// FAILS IF: the next thing to play stops walking INTO playlists in the
    /// tree's order, or a stale id is answered with something.
    #[test]
    fn the_next_one_walks_through_the_playlists() {
        let (queue, [a, b, c, d, ..]) = a_tree();
        let next = |current| queue.next(current, Repeat::Nothing, Step::Ended, None);
        assert_eq!(next(None), Some(a));
        assert_eq!(next(Some(a)), Some(b));
        assert_eq!(next(Some(b)), Some(c));
        assert_eq!(next(Some(c)), Some(d));
        assert_eq!(next(Some(d)), None);
        assert_eq!(next(Some(EntryId(999))), None);
        let previous = |current| queue.previous(current, Repeat::Nothing, None);
        assert_eq!(previous(Some(c)), Some(b));
        assert_eq!(previous(Some(a)), None);
    }

    /// FAILS IF: repeating one plays it again when a person skipped it, or
    /// does not when it ended; or repeating the queue stops going round.
    #[test]
    fn repeat_decides_what_the_end_leads_to() {
        let (queue, [a, b, _, d, ..]) = a_tree();
        assert_eq!(queue.next(Some(b), Repeat::One, Step::Ended, None), Some(b));
        assert_eq!(
            queue.next(Some(b), Repeat::One, Step::Asked, None),
            queue.next(Some(b), Repeat::Nothing, Step::Asked, None)
        );
        assert_eq!(
            queue.next(Some(d), Repeat::Queue, Step::Ended, None),
            Some(a)
        );
        assert_eq!(
            queue.next(Some(d), Repeat::Queue, Step::Asked, None),
            Some(a)
        );
        assert_eq!(queue.previous(Some(a), Repeat::Queue, None), Some(d));
    }

    /// FAILS IF: a shuffle loses or repeats a leaf, does not start with the
    /// one playing, or is not the same order for the same seed.
    #[test]
    fn a_shuffle_is_every_leaf_once_with_the_current_first() {
        for n in 0..40u64 {
            let leaves: Vec<EntryId> = (1..=n).map(EntryId).collect();
            for seed in [0, 1, 7, 0xDEAD_BEEF] {
                let first = leaves.get(usize::try_from(n / 2).unwrap_or(0)).copied();
                let order = shuffled(&leaves, first, seed);
                assert_eq!(order.first().copied(), first.or(order.first().copied()));
                let mut sorted = order.clone();
                sorted.sort();
                assert_eq!(sorted, leaves, "n={n} seed={seed}");
                assert_eq!(order, shuffled(&leaves, first, seed));
            }
        }
        let leaves: Vec<EntryId> = (1..=20).map(EntryId).collect();
        assert_ne!(
            shuffled(&leaves, None, 1),
            shuffled(&leaves, None, 2),
            "two seeds, one order"
        );
        // A FIRST THAT IS NOT A LEAF is not invented into the order.
        assert_eq!(shuffled(&leaves, Some(EntryId(99)), 3).len(), 20);
    }

    /// FAILS IF: a shuffle made before the queue changed plays something
    /// twice, plays a removed thing, or never reaches what was added.
    #[test]
    fn a_stale_shuffle_still_reaches_every_leaf_once() {
        let (mut queue, [a, b, c, d, ..]) = a_tree();
        let stale = vec![d, EntryId(999), b, b, a];
        queue.remove(a).expect("removed");
        let e = queue
            .insert(Place::End(None), vec![single("/e.flac")])
            .expect("inserted")[0];
        assert_eq!(queue.order(Some(&stale)), vec![d, b, c, e]);
        assert_eq!(
            queue.next(Some(d), Repeat::Nothing, Step::Ended, Some(&stale)),
            Some(b)
        );
    }

    /// FAILS IF: removing a playlist leaves its leaves behind, or the one
    /// playing inside it has nowhere to go.
    #[test]
    fn removing_a_playlist_takes_its_leaves_and_the_current_moves_on() {
        let (mut queue, [a, b, c, d, p, q]) = a_tree();
        assert_eq!(
            queue.after_removing(Some(c), p, Repeat::Nothing, None),
            Some(d)
        );
        assert_eq!(
            queue.after_removing(Some(a), p, Repeat::Nothing, None),
            Some(a)
        );
        assert_eq!(
            queue.after_removing(Some(d), d, Repeat::Nothing, None),
            None
        );
        assert_eq!(
            queue.after_removing(Some(d), d, Repeat::Queue, None),
            Some(a)
        );
        assert_eq!(
            queue.after_removing(Some(c), q, Repeat::Nothing, None),
            Some(d)
        );
        queue.remove(p).expect("removed");
        assert_eq!(queue.leaves(), vec![a, d]);
        assert!(queue.find(b).is_none() && queue.find(q).is_none());
        assert_eq!(queue.remove(p), Err(Refusal::NoSuchEntry { id: p }));
    }

    /// FAILS IF: a move loses the entry, lands in the wrong place, or puts a
    /// playlist inside itself.
    #[test]
    fn a_move_keeps_the_entry_and_never_puts_a_playlist_in_itself() {
        let (mut queue, [a, b, c, d, p, q]) = a_tree();
        assert_eq!(
            queue.move_to(p, Place::End(Some(q))),
            Err(Refusal::IntoItself)
        );
        assert_eq!(queue.move_to(p, Place::After(c)), Err(Refusal::IntoItself));
        assert_eq!(
            queue.move_to(p, Place::Start(Some(p))),
            Err(Refusal::IntoItself)
        );
        assert_eq!(
            queue.move_to(a, Place::Start(Some(d))),
            Err(Refusal::NotAPlaylist { id: d })
        );
        let revision = queue.revision();
        queue.move_to(a, Place::After(a)).expect("nothing to do");
        assert_eq!(
            queue.revision(),
            revision,
            "a move to where it is moved the revision"
        );

        queue.move_to(d, Place::Start(Some(q))).expect("moved in");
        assert_eq!(queue.leaves(), vec![a, b, d, c]);
        queue.move_to(a, Place::After(c)).expect("moved beside");
        assert_eq!(queue.leaves(), vec![b, d, c, a]);
        assert_eq!(queue.path_of(a), Some(vec![0, 1, 2]));
        assert_eq!(queue.at_path(&[0, 1, 2]).map(|e| e.id), Some(a));
    }

    /// FAILS IF: a ceiling is passed, or a refused change leaves anything
    /// changed.
    #[test]
    fn the_ceilings_are_refusals_and_a_refusal_changes_nothing() {
        let mut queue = Queue::new();
        let many: Vec<Item> = (0..LEAVES_MOST)
            .map(|n| single(&format!("/{n}.flac")))
            .collect();
        queue
            .insert(Place::End(None), many)
            .expect("at the ceiling");
        let before = queue.clone();
        assert_eq!(
            queue.insert(Place::End(None), vec![single("/one-more.flac")]),
            Err(Refusal::TooMany(TooMany {
                what: Limit::Leaves,
                most: LEAVES_MOST
            }))
        );
        assert_eq!(queue, before);

        let mut queue = Queue::new();
        let deep = playlist(
            "1",
            vec![playlist("2", vec![playlist("3", vec![single("/4.flac")])])],
        );
        let ids = queue
            .insert(Place::End(None), vec![deep])
            .expect("four deep");
        let too_deep = queue.insert(
            Place::End(None),
            vec![playlist(
                "1",
                vec![playlist(
                    "2",
                    vec![playlist("3", vec![playlist("4", vec![single("/5.flac")])])],
                )],
            )],
        );
        assert_eq!(too_deep, Err(too_many(Limit::Depth, DEPTH_MOST)));
        // MOVING a two-level playlist into the deepest one is the same refusal.
        let two = queue
            .insert(
                Place::End(None),
                vec![playlist("x", vec![single("/y.flac")])],
            )
            .expect("two deep")[0];
        let path = queue.path_of(queue.leaves()[0]).expect("a leaf");
        let third = queue.at_path(&path[..3]).expect("the third level").id;
        assert_eq!(
            queue.move_to(two, Place::End(Some(third))),
            Err(too_many(Limit::Depth, DEPTH_MOST))
        );
        assert_eq!(
            queue.find(two).map(|e| e.body.kind()),
            Some(NodeKind::Playlist)
        );
        assert_eq!(queue.path_of(ids[0]), Some(vec![0]));

        // A SET PAST ITS OWN CEILING is refused at the door of the queue.
        let lanes: Vec<(LaneKind, &str, Option<&str>)> = (0..=crate::session::LANES_MOST)
            .map(|_| (LaneKind::Subtitle, "/s.srt", None))
            .collect();
        assert_eq!(
            Queue::new().insert(
                Place::End(None),
                vec![Item::Set {
                    set: set_of(&lanes),
                    from: None
                }]
            ),
            Err(too_many(Limit::Lanes, crate::session::LANES_MOST))
        );
        // AND A REPLACE THAT IS REFUSED KEEPS WHAT WAS THERE.
        let (mut queue, _) = a_tree();
        let kept = queue.clone();
        assert!(queue.replace(vec![playlist(" ", vec![])]).is_err());
        assert_eq!(queue, kept);
    }

    /// FAILS IF: a name is taken from a playlist, or an empty name does not
    /// give a file back its own.
    #[test]
    fn a_rename_follows_the_naming_rule() {
        let (mut queue, [a, _, _, _, p, _]) = a_tree();
        assert_eq!(queue.rename(p, "  "), Err(Refusal::Nameless));
        queue.rename(p, " Road trip ").expect("renamed");
        assert_eq!(
            queue.find(p).map(|e| e.body.name()),
            Some(Name::Titled("Road trip".to_owned()))
        );
        queue.rename(a, "Opening").expect("renamed");
        queue.rename(a, "").expect("unnamed");
        assert_eq!(
            queue.find(a).map(|e| e.body.name()),
            Some(Name::Location("/a.flac".to_owned()))
        );
    }

    /// FAILS IF: a set is ever named after one of its files when it plays
    /// several, or a set of one source is not that source's name.
    #[test]
    fn a_set_is_never_named_after_one_file() {
        let film_and_commentary = set_of(&[
            (LaneKind::Video, "/films/film.mkv", None),
            (LaneKind::Audio, "/films/film.mkv", Some("Film")),
            (LaneKind::Audio, "/films/commentary.opus", None),
            (LaneKind::Subtitle, "/films/film.en.srt", None),
        ]);
        let body = Body::Set {
            set: film_and_commentary.clone(),
            from: None,
        };
        assert_eq!(
            body.name(),
            Name::Together(vec![
                "Film".to_owned(),
                "commentary.opus".to_owned(),
                "film.en.srt".to_owned()
            ])
        );
        // A SET OF ONE SOURCE is one medium, and is its name.
        let one = Body::Set {
            set: set_of(&[
                (LaneKind::Video, "/films/film.mkv", None),
                (LaneKind::Audio, "/films/film.mkv", None),
            ]),
            from: None,
        };
        assert_eq!(one.name(), Name::Together(vec!["film.mkv".to_owned()]));
        // A NAME IT WAS SAVED WITH wins, then the file it came from.
        let from = Body::Set {
            set: film_and_commentary.clone(),
            from: Some("/films/Class 3.rset".to_owned()),
        };
        assert_eq!(from.name(), Name::Titled("Class 3".to_owned()));
        let mut named = film_and_commentary;
        named.name = Some("Evening".to_owned());
        assert_eq!(
            Body::Set {
                set: named,
                from: None
            }
            .name(),
            Name::Titled("Evening".to_owned())
        );
    }

    /// FAILS IF: the joined name is cut through a cluster, measured in
    /// clusters rather than cells, or passes its width.
    #[test]
    fn a_joined_name_is_cut_by_cluster_to_its_cells() {
        let parts = vec!["日本語".to_owned(), "Film".to_owned(), " ".to_owned()];
        assert_eq!(together(&parts, " + ", 40), "日本語 + Film");
        let cut = together(&parts, " + ", 8);
        assert_eq!(cut, "日本語…");
        assert!(ronoctua_protocol::cells(&cut) <= 8);
        // SIX CELLS OF IDEOGRAPHS do not fit in five: two whole ones and the
        // mark, never half of one.
        assert_eq!(together(&["日本語".to_owned()], "+", 5), "日本…");
        let couple = "👩\u{200d}❤\u{fe0f}\u{200d}👨";
        assert_eq!(
            together(&[couple.to_owned(), "x".repeat(9)], "|", 3),
            format!("{couple}…")
        );
        assert_eq!(together(&parts, " + ", 0), "");
        assert_eq!(together(&[], " + ", 10), "");
    }

    /// FAILS IF: a location with no title shows the wrong piece of itself.
    #[test]
    fn a_location_is_shown_by_its_last_piece() {
        assert_eq!(file_name("/music/album/01 Intro.flac"), "01 Intro.flac");
        assert_eq!(
            file_name("https://radio.example/live/stream.mp3?x=1"),
            "stream.mp3"
        );
        assert_eq!(file_name("https://radio.example/"), "radio.example");
        assert_eq!(file_name("https://radio.example"), "radio.example");
        assert_eq!(file_name("relative.ogg"), "relative.ogg");
    }

    /// FAILS IF: the outline stops carrying the shape and names doors draw,
    /// or its revision moves for a lane's volume.
    #[test]
    fn the_outline_carries_the_shape_and_moves_only_with_it() {
        let mut queue = Queue::new();
        let ids = queue
            .insert(
                Place::End(None),
                vec![
                    playlist("P", vec![single("/b.flac")]),
                    Item::Set {
                        set: set_of(&[
                            (LaneKind::Video, "/v.mkv", None),
                            (LaneKind::Audio, "/a.opus", None),
                        ]),
                        from: None,
                    },
                    Item::Stream(Stream::typed("https://radio.example/live")),
                ],
            )
            .expect("inserted");
        let outline = queue.outline();
        assert_eq!(outline.revision, queue.revision());
        assert_eq!(outline.entries.len(), 3);
        assert_eq!(outline.entries[0].kind, NodeKind::Playlist);
        assert_eq!(outline.entries[0].children.len(), 1);
        assert_eq!(outline.entries[1].lanes, 2);
        assert_eq!(
            outline.entries[2].name,
            Name::Location("https://radio.example/live".to_owned())
        );
        let revision = queue.revision();
        if let Some(set) = queue.set_mut(ids[1]) {
            set.lanes[0].volume_percent = 40;
        }
        assert_eq!(queue.revision(), revision);
        assert!(queue.set_mut(ids[0]).is_none());

        let text = serde_json::to_string(&outline).expect("serialises");
        assert!(text.contains(r#""kind":"playlist""#), "{text}");
        assert!(text.contains(r#""together":["v.mkv","a.opus"]"#), "{text}");
        assert_eq!(
            serde_json::from_str::<Outline>(&text).expect("parses"),
            outline
        );
    }

    /// FAILS IF: a queue kept in a file does not come back as the same
    /// shape.
    #[test]
    fn a_kept_queue_comes_back_as_the_same_shape() {
        let (queue, _) = a_tree();
        let file = queue.kept(Some("Sunday".to_owned()));
        let back = Queue::from_file(file.clone()).expect("read");
        assert_eq!(back.kept(Some("Sunday".to_owned())), file);
        let names = |q: &Queue| {
            q.outline()
                .entries
                .into_iter()
                .map(|n| (n.name, n.kind))
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&back), names(&queue));
    }
}
