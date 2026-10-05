//! What a sync will do, and what the person decided (feature `prefs`).
//!
//! `prepare` writes a [`Plan`] into the session folder (`plan.json`); with no
//! conflict it is applied at once. With conflicts the view lists EVERY one
//! (path, size and time on both sides, "changed on {machine}"), and the
//! person decides each — [`Decisions`], written back as `decisions.json` —
//! before `apply` runs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The plan's file, in the session folder.
pub const PLAN_FILE: &str = "plan.json";
/// The decisions' file, in the session folder.
pub const DECISIONS_FILE: &str = "decisions.json";

/// What a path is on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A regular file.
    #[default]
    File,
    /// A symbolic link, kept as a link (its target is what is stored).
    Link,
}

/// One side of a path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Side {
    /// Its size, in bytes (a link's: its target's length).
    pub size_bytes: u64,
    /// When it last changed, seconds since the epoch.
    pub modified_s: i64,
    /// Its SHA-256, hexadecimal (a link's: of its target).
    pub sha256: String,
    /// A file or a link.
    pub kind: Kind,
    /// Its permission bits (`0o600` stays `0o600`).
    pub mode: u32,
}

impl Side {
    /// Whether two sides are the same content, kind and mode — what "changed"
    /// means for the merge. The time is not: copying a file changes it.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        self.sha256 == other.sha256 && self.kind == other.kind && self.mode == other.mode
    }
}

/// What happens to a path without a question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// Unchanged on both sides.
    Same,
    /// Changed here only: the repository takes it.
    Push,
    /// Changed there only: this machine takes it (the old one kept in
    /// `replaced/`).
    Pull,
    /// Deleted here only: deleted in the repository.
    DeleteRemote,
    /// Deleted there only: deleted here (kept in `replaced/`).
    DeleteLocal,
    /// Another machine stopped keeping a path the person had added: this one
    /// forgets it, and the file stays where it is.
    Forget,
    /// This machine stopped keeping a path the person had added: it leaves
    /// the repository, and every other machine forgets it.
    Unkeep,
}

impl Step {
    /// Whether it moves anything.
    #[must_use]
    pub const fn moves(self) -> bool {
        !matches!(self, Self::Same)
    }
}

/// A path the plan moves without asking.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Entry {
    /// The logical path.
    pub path: String,
    /// What happens to it.
    pub step: Step,
}

/// A path changed on both sides (or deleted on one and changed on the
/// other).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Conflict {
    /// The logical path.
    pub path: String,
    /// This machine's side; `None` when deleted here.
    pub local: Option<Side>,
    /// The repository's side; `None` when deleted there.
    pub remote: Option<Side>,
    /// The machine that changed the repository's side — data.
    pub machine: String,
    /// Whether both sides are text (`RET` shows the diff).
    pub text: bool,
    /// Whether the person added this path (only those may keep both).
    pub added_by_person: bool,
    /// The decision it starts with, when there is one: on the first sync of
    /// a machine, what differs is marked "take the repository's".
    pub suggested: Option<Decision>,
    /// The file, in the session's `theirs/`, holding the repository's side of
    /// a text — for the diff.
    pub theirs: Option<String>,
}

/// Everything a sync will do.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Plan {
    /// The repository's commit the plan was made against; `None` for an empty
    /// repository.
    pub remote_commit: Option<String>,
    /// The first sync of this machine (no `last.json`).
    pub first_run: bool,
    /// When it was made, seconds since the epoch.
    pub made_s: i64,
    /// The paths moved without a question.
    pub entries: Vec<Entry>,
    /// The paths the person decides.
    pub conflicts: Vec<Conflict>,
}

/// What the person decided for a conflict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    /// Keep this machine's (overwrites the repository's).
    KeepLocal,
    /// Take the repository's (overwrites this machine's; the old one goes to
    /// `replaced/`).
    TakeRemote,
    /// Keep both (only for paths the person added): the repository's lands
    /// beside this machine's ([`beside`]).
    KeepBoth,
}

impl Decision {
    /// The three, in the order a row offers them.
    pub const ALL: [Self; 3] = [Self::KeepLocal, Self::TakeRemote, Self::KeepBoth];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::KeepLocal => "keep-local",
            Self::TakeRemote => "take-remote",
            Self::KeepBoth => "keep-both",
        }
    }

    /// Whether a conflict may take it: keeping both only for what the person
    /// added, and never under the configuration root — two copies of a
    /// setting side by side are one setting the system reads and one it does
    /// not, named after another machine.
    #[must_use]
    pub fn allowed(self, conflict: &Conflict) -> bool {
        !matches!(self, Self::KeepBoth)
            || (conflict.added_by_person
                && !crate::places::covers(crate::places::Root::Config.word(), &conflict.path))
    }
}

/// Every decision, by path.
pub type Decisions = BTreeMap<String, Decision>;

impl Plan {
    /// Whether every conflict has an allowed decision in `decisions`.
    #[must_use]
    pub fn decided(&self, decisions: &Decisions) -> bool {
        self.conflicts
            .iter()
            .all(|c| decisions.get(&c.path).is_some_and(|d| d.allowed(c)))
    }

    /// The decisions it starts with — every conflict's suggestion.
    #[must_use]
    pub fn suggested(&self) -> Decisions {
        self.conflicts
            .iter()
            .filter_map(|c| c.suggested.map(|d| (c.path.clone(), d)))
            .collect()
    }

    /// How many paths take each step.
    #[must_use]
    pub fn count(&self, step: Step) -> usize {
        self.entries.iter().filter(|e| e.step == step).count()
    }

    /// Every conflict decided alike, where it may be: the bulk "all: keep
    /// this machine's" and "all: take the repository's". `KeepBoth` is not a
    /// bulk decision: it is only for some.
    #[must_use]
    pub fn all(&self, decision: Decision) -> Decisions {
        self.conflicts
            .iter()
            .filter(|c| decision.allowed(c))
            .map(|c| (c.path.clone(), decision))
            .collect()
    }
}

/// Where the repository's side of a path lands when both are kept: beside
/// it, with the other machine's name before the extension —
/// `home/notes/todo.md` from `laptop` is `home/notes/todo.laptop.md`. Pure.
#[must_use]
pub fn beside(path: &str, machine: &str) -> String {
    let machine: String = machine
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let machine = if machine.is_empty() {
        "other".to_owned()
    } else {
        machine
    };
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    let named = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => format!("{stem}.{machine}.{ext}"),
        _ => format!("{name}.{machine}"),
    };
    if dir.is_empty() {
        named
    } else {
        format!("{dir}/{named}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conflict(path: &str, added_by_person: bool) -> Conflict {
        Conflict {
            path: path.to_owned(),
            machine: "laptop".to_owned(),
            text: true,
            added_by_person,
            ..Conflict::default()
        }
    }

    #[test]
    fn keeping_both_is_only_for_what_the_person_added() {
        let plan = Plan {
            conflicts: vec![
                conflict("config/theme.json", false),
                conflict("home/notes.md", true),
            ],
            ..Plan::default()
        };
        let mut decisions = Decisions::new();
        decisions.insert("config/theme.json".to_owned(), Decision::KeepBoth);
        decisions.insert("home/notes.md".to_owned(), Decision::KeepBoth);
        assert!(!plan.decided(&decisions));
        decisions.insert("config/theme.json".to_owned(), Decision::TakeRemote);
        assert!(plan.decided(&decisions));
        // IN BULK, keeping both is never offered; the other two cover all.
        assert_eq!(plan.all(Decision::KeepBoth).len(), 1);
        // AND NEVER UNDER THE CONFIGURATION ROOT, added or not.
        assert!(!Decision::KeepBoth.allowed(&conflict("config/notes.json", true)));
        assert!(Decision::KeepBoth.allowed(&conflict("state/x/notes.md", true)));
        assert!(plan.decided(&plan.all(Decision::KeepLocal)));
        assert!(plan.decided(&plan.all(Decision::TakeRemote)));
    }

    #[test]
    fn both_kept_land_beside_each_other() {
        assert_eq!(
            beside("home/notes/todo.md", "laptop"),
            "home/notes/todo.laptop.md"
        );
        assert_eq!(beside("home/.bashrc", "noctua-x"), "home/.bashrc.noctua-x");
        assert_eq!(beside("home/Makefile", "a b"), "home/Makefile.a-b");
        assert_eq!(beside("home/x/y", ""), "home/x/y.other");
    }

    #[test]
    fn a_plan_crosses_the_file_whole() {
        let plan = Plan {
            remote_commit: Some("ab12".to_owned()),
            first_run: true,
            made_s: 7,
            entries: vec![Entry {
                path: "config/a.json".to_owned(),
                step: Step::Pull,
            }],
            conflicts: vec![Conflict {
                suggested: Some(Decision::TakeRemote),
                ..conflict("config/b.json", false)
            }],
        };
        let text = serde_json::to_string(&plan).unwrap();
        assert_eq!(serde_json::from_str::<Plan>(&text).unwrap(), plan);
        assert_eq!(
            plan.suggested().get("config/b.json"),
            Some(&Decision::TakeRemote)
        );
        assert_eq!(plan.count(Step::Pull), 1);
    }
}
