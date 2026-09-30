//! Favourites and the recently played: a station, a channel, an archive film
//! or a live, remembered with the stream that plays it.
//!
//! # Two files, two writers
//!
//! - **Favourites** are the person's, in the config root
//!   (`player-favourites.json`), and every door writes them: a star pressed
//!   in the footer and one pressed in the window are two processes.
//! - **Recently played** is the daemon's, in the state root
//!   (`player-recent.json`), written when a stream starts.
//!
//! # A change is applied to the file as it is NOW
//!
//! A door's copy of the favourites is as old as its last read. Writing that
//! copy back would undo whatever another door did since — so a door never
//! writes its copy: it hands a [`Change`] (add this, remove that) to
//! [`Shelf::change`], which reads the file with `parsed` (a file that does not
//! parse is refused, never overwritten), applies the change by `(kind, id)`
//! and writes the result — under a lock beside the file (`.<file>.lock`), so
//! two changes at the same instant are applied one after the other.
//! [`merged`] reconciles a copy with the file for a door that holds one.

use std::fs::File;
use std::path::PathBuf;

use ronoctua_playlist::own::Stream;
use ronoctua_store::Store;
use serde::{Deserialize, Serialize};

use crate::country::Code;

/// The favourites' file, under the config root.
pub const FAVOURITES_FILE: &str = "player-favourites.json";

/// The recently played file, under the state root.
pub const RECENT_FILE: &str = "player-recent.json";

/// The layout of both files.
pub const VERSION: u32 = 1;

/// What a pick is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A radio station (by its radio-browser id).
    Radio,
    /// A TV channel (by its iptv-org id).
    Tv,
    /// An Internet Archive item (by its identifier).
    Archive,
    /// A YouTube live (by its video id).
    Live,
}

/// One thing remembered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pick {
    /// What it is.
    pub kind: Kind,
    /// Its id in its directory: with `kind`, what makes it the same pick.
    pub id: String,
    /// What it is called.
    pub title: String,
    /// Its country, when it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<Code>,
    /// What plays it.
    pub stream: Stream,
    /// When it was picked or played, in seconds since the epoch.
    pub at_unix_seconds: i64,
}

impl Pick {
    /// Whether `other` is the same thing, whatever its title or stream say.
    #[must_use]
    pub fn same(&self, other: &Self) -> bool {
        self.kind == other.kind && self.id == other.id
    }
}

/// A file of picks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Picks {
    /// Its layout, [`VERSION`].
    pub version: u32,
    /// The picks, newest first.
    pub picks: Vec<Pick>,
}

impl Default for Picks {
    fn default() -> Self {
        Self {
            version: VERSION,
            picks: Vec::new(),
        }
    }
}

/// One change to a list of picks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Keep this pick (replacing the same one), at the front.
    Add(Pick),
    /// Forget the pick of this kind and id.
    Remove(Kind, String),
}

/// `list` with `change` applied, at most `most` picks — the oldest go. Pure.
#[must_use]
pub fn applied(mut list: Vec<Pick>, change: &Change, most: usize) -> Vec<Pick> {
    match change {
        Change::Add(pick) => {
            list.retain(|p| !p.same(pick));
            list.insert(0, pick.clone());
        }
        Change::Remove(kind, id) => list.retain(|p| !(p.kind == *kind && p.id == *id)),
    }
    list.truncate(most);
    list
}

/// Two copies of one list, made one: every pick of either, once by
/// `(kind, id)`, the newer of two copies kept; newest first. Pure.
#[must_use]
pub fn merged(mine: &[Pick], theirs: &[Pick]) -> Vec<Pick> {
    let mut out: Vec<Pick> = Vec::with_capacity(mine.len() + theirs.len());
    for p in mine.iter().chain(theirs) {
        match out.iter_mut().find(|o| o.same(p)) {
            Some(o) if p.at_unix_seconds > o.at_unix_seconds => *o = p.clone(),
            Some(_) => {}
            None => out.push(p.clone()),
        }
    }
    out.sort_by(|a, b| {
        b.at_unix_seconds
            .cmp(&a.at_unix_seconds)
            .then(a.kind.cmp(&b.kind))
            .then_with(|| a.id.cmp(&b.id))
    });
    out
}

/// Why a list was not changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trouble {
    /// There is no home to keep the file in.
    NoHome,
    /// The file is there and does not parse — refused, never overwritten.
    Unreadable,
    /// The file was written by a newer layout.
    Newer,
    /// The file could not be written.
    Unwritable,
}

/// One of the two lists, where it is kept.
#[derive(Debug, Clone)]
pub struct Shelf {
    store: Store<Picks>,
    lock: Option<PathBuf>,
    most: usize,
}

impl Shelf {
    /// The favourites, at most `most` ([`crate::table::FAVOURITES_MOST`]).
    #[must_use]
    pub fn favourites(most: usize) -> Self {
        Self::at(ronoctua_xdg::config_dir(), FAVOURITES_FILE, most)
    }

    /// The recently played, at most `most` ([`crate::table::RECENT_MOST`]).
    #[must_use]
    pub fn recent(most: usize) -> Self {
        Self::at(ronoctua_xdg::state_dir(), RECENT_FILE, most)
    }

    /// A list in `dir`, under `file`.
    #[must_use]
    pub fn at(dir: Option<PathBuf>, file: &'static str, most: usize) -> Self {
        Self {
            lock: dir.as_ref().map(|d| d.join(format!(".{file}.lock"))),
            store: Store::at(dir, file),
            most,
        }
    }

    /// Where the file is, when it is anywhere.
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.store.path()
    }

    /// The list, newest first — empty when there is none, or it does not
    /// parse (said in the log by the store).
    #[must_use]
    pub fn read(&self) -> Vec<Pick> {
        let picks = self.store.read();
        if picks.version > VERSION {
            return Vec::new();
        }
        picks.picks
    }

    /// Applies `change` to the file as it is now, and answers the list
    /// written. Blocks while another process changes it.
    ///
    /// # Errors
    ///
    /// [`Trouble::NoHome`], [`Trouble::Unreadable`] for a file that does not
    /// parse, [`Trouble::Newer`], [`Trouble::Unwritable`].
    pub fn change(&self, change: &Change) -> Result<Vec<Pick>, Trouble> {
        let Some(lock) = &self.lock else {
            return Err(Trouble::NoHome);
        };
        if let Some(dir) = lock.parent() {
            std::fs::create_dir_all(dir).map_err(|_| Trouble::Unwritable)?;
        }
        let held = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock)
            .map_err(|_| Trouble::Unwritable)?;
        held.lock().map_err(|_| Trouble::Unwritable)?;
        let now = self.store.parsed().ok_or(Trouble::Unreadable)?;
        if now.version > VERSION {
            return Err(Trouble::Newer);
        }
        let picks = applied(now.picks, change, self.most);
        self.store
            .write(&Picks {
                version: VERSION,
                picks: picks.clone(),
            })
            .map_err(|_| Trouble::Unwritable)?;
        Ok(picks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(kind: Kind, id: &str, at: i64) -> Pick {
        Pick {
            kind,
            id: id.to_owned(),
            title: id.to_uppercase(),
            country: None,
            stream: Stream::typed(format!("https://x.example/{id}")),
            at_unix_seconds: at,
        }
    }

    #[test]
    fn a_pick_is_one_by_kind_and_id() {
        let list = applied(Vec::new(), &Change::Add(pick(Kind::Radio, "a", 1)), 10);
        let list = applied(list, &Change::Add(pick(Kind::Tv, "a", 2)), 10);
        let list = applied(list, &Change::Add(pick(Kind::Radio, "a", 3)), 10);
        let seen: Vec<(Kind, &str, i64)> = list
            .iter()
            .map(|p| (p.kind, p.id.as_str(), p.at_unix_seconds))
            .collect();
        assert_eq!(seen, [(Kind::Radio, "a", 3), (Kind::Tv, "a", 2)]);
        let list = applied(list, &Change::Remove(Kind::Tv, "a".to_owned()), 10);
        assert_eq!(list.len(), 1);
        let list = applied(list, &Change::Add(pick(Kind::Live, "b", 4)), 1);
        assert_eq!(list.len(), 1, "the oldest go past the ceiling");
        assert_eq!(list[0].id, "b");
    }

    #[test]
    fn two_copies_merge_by_kind_and_id_and_the_newer_wins() {
        let mine = [pick(Kind::Radio, "a", 5), pick(Kind::Tv, "b", 1)];
        let mut newer = pick(Kind::Tv, "b", 9);
        newer.title = "Renamed".to_owned();
        let theirs = [newer, pick(Kind::Archive, "c", 3)];
        let got = merged(&mine, &theirs);
        let seen: Vec<(&str, &str)> = got
            .iter()
            .map(|p| (p.id.as_str(), p.title.as_str()))
            .collect();
        assert_eq!(seen, [("b", "Renamed"), ("a", "A"), ("c", "C")]);
        assert_eq!(merged(&theirs, &mine), got, "whichever copy is mine");
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-broadcast-picks-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_change_is_applied_to_the_file_as_it_is_now() {
        let dir = scratch("now");
        let door_a = Shelf::at(Some(dir.clone()), FAVOURITES_FILE, 50);
        let door_b = Shelf::at(Some(dir.clone()), FAVOURITES_FILE, 50);
        door_a
            .change(&Change::Add(pick(Kind::Radio, "x", 1)))
            .unwrap();
        // DOOR B never read the file, and adds its own: `x` survives.
        let written = door_b.change(&Change::Add(pick(Kind::Tv, "y", 2))).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(door_a.read(), written);
        door_a
            .change(&Change::Remove(Kind::Radio, "x".to_owned()))
            .unwrap();
        assert_eq!(door_b.read().len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_file_that_does_not_parse_is_refused_and_kept() {
        let dir = scratch("broken");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(FAVOURITES_FILE), b"{ half").unwrap();
        let shelf = Shelf::at(Some(dir.clone()), FAVOURITES_FILE, 50);
        assert_eq!(
            shelf.change(&Change::Add(pick(Kind::Radio, "x", 1))),
            Err(Trouble::Unreadable)
        );
        assert_eq!(std::fs::read(dir.join(FAVOURITES_FILE)).unwrap(), b"{ half");
        std::fs::write(
            dir.join(FAVOURITES_FILE),
            br#"{"version": 99, "picks": []}"#,
        )
        .unwrap();
        assert_eq!(
            shelf.change(&Change::Add(pick(Kind::Radio, "x", 1))),
            Err(Trouble::Newer)
        );
        assert!(shelf.read().is_empty());
        let nowhere = Shelf::at(None, FAVOURITES_FILE, 50);
        assert_eq!(
            nowhere.change(&Change::Add(pick(Kind::Radio, "x", 1))),
            Err(Trouble::NoHome)
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
