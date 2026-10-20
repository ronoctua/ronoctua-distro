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
        /// The paths the person added, and the targets' paths, that are
        /// past the "large" line, with their weight — said on the page,
        /// never refused.
        #[serde(default)]
        large: Vec<(String, u64)>,
        /// Paths that could not be read here, left as they were.
        #[serde(default)]
        unreadable: Vec<String>,
        /// Paths the repository had that could not be written here: the next
        /// sync tries again.
        #[serde(default)]
        unwritten: Vec<String>,
        /// What a restore offers to turn on, "no" first (the
        /// belongings-targets-unlock package).
        #[serde(default)]
        offers: Vec<Offer>,
        /// The person's files a lenient read repaired, by logical path (the
        /// store-resilience package's ledger).
        #[serde(default)]
        repaired: Vec<String>,
        /// What the repository weighs against its budget (the
        /// belongings-sealed package); `None` before it is measured.
        #[serde(default)]
        budget: Option<Budget>,
        // ── anchor: L07 ──
        // ── anchor: L08 ──
        // ── anchor: L10 ──
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
    /// The history was compacted (the belongings-sealed package).
    Compacted {
        /// What the clone weighed before.
        before_bytes: u64,
        /// And after.
        after_bytes: u64,
    },
    /// The plain part alone was synced (the belongings-plain-merge package).
    PlainSynced {
        /// Plain files written here.
        pulled: u32,
        /// Plain files sent.
        pushed: u32,
    },
    // ── anchor: L08 ──
    /// A compaction found nothing to fold: every version is younger than
    /// `keep_days` days and of this format, and no chunk is orphaned — the
    /// history was left as it was (the belongings-sealed package).
    NothingToCompact {
        /// The days the compaction kept.
        keep_days: u64,
    },
    // ── anchor: L09 ──
    // ── anchor: L10 ──
}

/// Something a restore offers to turn on — "no" drawn first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "offer", rename_all = "kebab-case")]
pub enum Offer {
    /// Sign commits with this SSH key (`~/.gitconfig` through `ronoctua-git`).
    SshSigning {
        /// The public key's logical path.
        key: String,
    },
    /// Let a security key answer for the password (`pam_u2f`, sufficient).
    PamU2f,
    /// Install the VS Code extensions of the person's list that are missing
    /// here — through the queue (`order::Op::VscodeExtensions`).
    VscodeExtensions {
        /// How many were missing when the sync offered it.
        count: u32,
    },
}

/// What the repository weighs, against its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Budget {
    /// The tree, in bytes.
    pub tree_bytes: u64,
    /// The history (`git count-objects` size-pack after a fetch), in bytes.
    pub history_bytes: u64,
    /// What the table recommends, in bytes.
    pub recommend_bytes: u64,
    /// Whether a compaction of the table's `keepDays` would drop any commit
    /// — what a recommendation to compact needs (`kept::weight`): a history
    /// all younger than the cutoff weighs what it weighs.
    #[serde(default)]
    pub foldable: bool,
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
    // ── anchor: L09 ──
    /// The plain part as the last run of it left it — a sync's, or a plain
    /// run's alone (`sync-plain`, `plain pull`/`push`): the base of its next
    /// merge, its conflicts and what it left out for its size. A plain run
    /// writes only this, so the rest stays the last SEALED sync's.
    pub plain: crate::plain::PlainBase,
    // ── anchor: L10 ──
    /// The predefined targets NO machine keeps any more, by id, as the
    /// index said at that sync (`crate::merge::Index::dropped`) — what a
    /// door reads to say "not on any machine" without the passphrase.
    pub dropped: std::collections::BTreeSet<String>,
    /// When each file of [`Last::entries`] and of the plain part last
    /// changed here, in seconds since the epoch, as the run left it alike
    /// with its base — what a door compares a file's `stat` with to say
    /// "changed" without reading it (`crate::inventory::stamp_sealed`). A
    /// path the run could not vouch for (changed while it ran) has none.
    pub modified_s: BTreeMap<String, i64>,
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
                offers: vec![Offer::PamU2f],
                repaired: vec![],
                budget: Some(Budget::default()),
            },
            Outcome::Waiting { conflicts: 4 },
            Outcome::Refused {
                why: Why::WrongPassphrase,
            },
            Outcome::Abandoned,
            Outcome::Keyed,
            Outcome::Rekeyed,
            Outcome::Compacted {
                before_bytes: 9,
                after_bytes: 4,
            },
            Outcome::PlainSynced {
                pulled: 1,
                pushed: 2,
            },
            Outcome::NothingToCompact { keep_days: 30 },
        ] {
            let s = Status { at_s: 5, outcome };
            let text = serde_json::to_string(&s).unwrap();
            assert_eq!(serde_json::from_str::<Status>(&text).unwrap(), s);
        }
    }

    /// FAILS IF: a `status.json` written before the 2026-10-05 wave (no
    /// `offers`, `repaired`, `budget`) stops reading.
    #[test]
    fn an_older_status_still_reads() {
        let old = r#"{"at_s":5,"outcome":{"outcome":"synced","pulled":1,"pushed":0,
            "deleted_here":0,"deleted_there":0,"kept":3,"kept_bytes":10}}"#;
        let s: Status = serde_json::from_str(old).unwrap();
        assert!(matches!(
            s.outcome,
            Outcome::Synced { ref offers, budget: None, .. } if offers.is_empty()
        ));
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
