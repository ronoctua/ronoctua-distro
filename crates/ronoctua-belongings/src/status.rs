//! What a run of the belongings leaves behind for others to read (feature
//! `prefs`): `status.json`, how the last run ended, and `last.json`, what the
//! last sync left — the base of the next merge.
//!
//! # The file is the broadcast
//!
//! The door ("Status", "Conflicts") and the brain module (the mark) never ask
//! the queue: they watch these two files and the session folder, and redraw
//! when one moves. The CLI writes each whole and atomically.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::Why;
use crate::plan::{Kind, Side};

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Outcome {
    /// Synced: the repository and this machine agree.
    Synced {
        /// Paths written here.
        pulled: u32,
        /// Paths sent to the repository.
        pushed: u32,
        /// Paths deleted here (kept in `replaced/`).
        deleted_here: u32,
        /// Paths deleted in the repository.
        deleted_there: u32,
        /// How many paths are kept now.
        kept: u32,
        /// How many bytes they weigh.
        kept_bytes: u64,
        /// The paths the person added that are past the "large" line, with
        /// their weight — said on the page, never refused.
        #[serde(default)]
        large: Vec<(String, u64)>,
        /// Paths that could not be read here, left as they were.
        #[serde(default)]
        unreadable: Vec<String>,
        /// Paths the repository had that could not be written here: the next
        /// sync tries again.
        #[serde(default)]
        unwritten: Vec<String>,
    },
    /// Prepared, and waiting for the person to decide these many conflicts.
    Waiting {
        /// How many.
        conflicts: u32,
    },
    /// Not done, and why.
    Refused {
        /// Why.
        why: Why,
    },
    /// The person dropped a prepared sync.
    Abandoned,
    /// A deploy key was made.
    Keyed,
    /// The passphrase was changed.
    Rekeyed,
}

/// `status.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// When it ended, seconds since the epoch.
    pub at_s: i64,
    /// How.
    pub outcome: Outcome,
}

impl Status {
    /// Reads `status.json`; `None` when there is none or it does not parse
    /// (a door then says nothing happened yet, which is true of this machine).
    #[must_use]
    pub fn read(path: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }

    /// Whether it says a sync failed.
    #[must_use]
    pub const fn failed(&self) -> bool {
        matches!(self.outcome, Outcome::Refused { .. })
    }
}

/// One path as the last sync left it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Known {
    /// Its SHA-256, hexadecimal.
    pub sha256: String,
    /// A file or a link.
    pub kind: Kind,
    /// Its permission bits.
    pub mode: u32,
    /// Its size, in bytes.
    pub size_bytes: u64,
    /// Whether a person added it (it travels to every machine).
    pub added: bool,
}

impl Known {
    /// The side it was.
    #[must_use]
    pub fn side(&self) -> Side {
        Side {
            size_bytes: self.size_bytes,
            modified_s: 0,
            sha256: self.sha256.clone(),
            kind: self.kind,
            mode: self.mode,
        }
    }
}

/// `last.json`: what the last sync left.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Last {
    /// The repository it was made with. A different one now is a first sync.
    pub repository: String,
    /// The repository's commit after it.
    pub commit: String,
    /// When, seconds since the epoch.
    pub at_s: i64,
    /// Every path kept then.
    pub entries: BTreeMap<String, Known>,
    /// The folders and files a person added (on any machine) that this
    /// machine FOLLOWED after that sync — what its "stop keeping" may take
    /// out of the repository (`crate::merge::roots_after`). A root another
    /// machine adds later, under a path this one took out, is not followed
    /// and so not taken out again.
    pub roots: std::collections::BTreeSet<String>,
}

impl Last {
    /// Reads `last.json`; `None` when there is none or it does not parse —
    /// which the engine treats as a first sync (nothing is deleted on a first
    /// sync: with no base, a path missing on one side is new on the other).
    #[must_use]
    pub fn read(path: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }
}

/// Whether a sync is due at `now_s`, given the last one and the period: never
/// with no period, at once with no sync yet. Pure.
#[must_use]
pub fn due(now_s: i64, last_s: Option<i64>, period_days: Option<u16>) -> bool {
    let Some(days) = period_days else {
        return false;
    };
    last_s.is_none_or(|at| now_s >= at + i64::from(days) * 86_400)
}

/// When the next sync comes due, seconds since the epoch: `None` with no
/// period or no sync yet (nothing to count from). Pure.
#[must_use]
pub fn due_at(last_s: Option<i64>, period_days: Option<u16>) -> Option<i64> {
    Some(last_s? + i64::from(period_days?) * 86_400)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_status_crosses_the_file_whole() {
        for outcome in [
            Outcome::Synced {
                pulled: 1,
                pushed: 2,
                deleted_here: 0,
                deleted_there: 3,
                kept: 40,
                kept_bytes: 9000,
                large: vec![("home/videos".to_owned(), 200 << 20)],
                unreadable: vec![],
                unwritten: vec!["config/x.json".to_owned()],
            },
            Outcome::Waiting { conflicts: 4 },
            Outcome::Refused {
                why: Why::WrongPassphrase,
            },
            Outcome::Abandoned,
            Outcome::Keyed,
            Outcome::Rekeyed,
        ] {
            let s = Status { at_s: 5, outcome };
            let text = serde_json::to_string(&s).unwrap();
            assert_eq!(serde_json::from_str::<Status>(&text).unwrap(), s);
        }
    }

    #[test]
    fn due_counts_the_period_from_the_last_sync() {
        let day = 86_400;
        assert!(due(0, None, Some(7)), "never synced is due");
        assert!(!due(0, None, None), "never is never due");
        assert!(!due(6 * day, Some(0), Some(7)));
        assert!(due(7 * day, Some(0), Some(7)));
        assert_eq!(due_at(Some(10), Some(1)), Some(10 + day));
        assert_eq!(due_at(None, Some(1)), None);
        assert_eq!(due_at(Some(10), None), None);
    }
}
