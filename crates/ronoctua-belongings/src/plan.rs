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
    /// Changed on both sides and merged cleanly (JSON by field, text by
    /// hunk): the merged file is written here and pushed (the
    /// belongings-plain-merge package).
    Merge,
    // ── anchor: L09 ──
    // ── anchor: L10 ──
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
    // ── anchor: L09 ──
    /// Why the repository's side of a signed plain file was not taken by
    /// itself (`crate::plain::HeldWhy`): the person's signature did not
    /// vouch for it. Such a conflict has no suggestion and no merge.
    pub held: Option<crate::plain::HeldWhy>,
    /// What a merge of the two sides reached, when one did: the JSON fields
    /// that clash (each decided on its own, or all one way), or the hunks of
    /// a text that clash — and then the two merged decisions are offered.
    /// `None`: no merge (binary, no base kept, a target that says choose).
    pub merge: Option<Merging>,
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
    /// A text whose hunks conflict: merge it, and take this machine's side
    /// of each conflicting hunk (the belongings-plain-merge package).
    TakeMergedOurs,
    /// The same, taking the repository's side of each conflicting hunk.
    TakeMergedTheirs,
    // ── anchor: L09 ──
}

impl Decision {
    /// Every one, in the order a row offers them.
    pub const ALL: [Self; 5] = [
        Self::KeepLocal,
        Self::TakeRemote,
        Self::KeepBoth,
        Self::TakeMergedOurs,
        Self::TakeMergedTheirs,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::KeepLocal => "keep-local",
            Self::TakeRemote => "take-remote",
            Self::KeepBoth => "keep-both",
            Self::TakeMergedOurs => "take-merged-ours",
            Self::TakeMergedTheirs => "take-merged-theirs",
        }
    }

    /// Whether a conflict may take it: keeping both only for what the person
    /// added, and never under the configuration root — two copies of a
    /// setting side by side are one setting the system reads and one it does
    /// not, named after another machine.
    ///
    /// The two merged ones only on a conflict a merge reached
    /// ([`Conflict::merge`]).
    #[must_use]
    pub fn allowed(self, conflict: &Conflict) -> bool {
        match self {
            Self::KeepLocal | Self::TakeRemote => true,
            Self::KeepBoth => {
                conflict.added_by_person
                    && !crate::places::covers(crate::places::Root::Config.word(), &conflict.path)
            }
            // ── anchor: L09 ──
            // A MERGE REACHED IT: the joined file, with one side at every
            // place both changed — a text's hunks, or a document's fields.
            Self::TakeMergedOurs | Self::TakeMergedTheirs => conflict.merge.is_some(),
        }
    }
}

/// Every decision, by path.
pub type Decisions = BTreeMap<String, Decision>;

/// What a merge of a conflict's two sides reached (the belongings-plain-merge
/// package): it did not join them, and says where they clash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Merging {
    /// A JSON document joined field by field; these fields changed
    /// differently on both sides. Each is decided on its own
    /// ([`DecisionsFile::fields`]), or all one way (the merged decisions).
    Json {
        /// The fields that clash, in the document's order.
        fields: Vec<FieldConflict>,
    },
    /// A text joined hunk by hunk (`git merge-file`); this many hunks clash.
    Text {
        /// How many.
        hunks: u32,
    },
}

/// One field of a JSON document both sides changed differently.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct FieldConflict {
    /// Where, as a JSON pointer (`/screens/0/scale`) — but an element of a
    /// list whose elements all carry an `id` is named by its id
    /// (`/apps/nix:firefox`), so the pointer stays when the list moves.
    pub pointer: String,
    /// The value both sides came from, as compact JSON; `None`: there was
    /// none.
    pub base: Option<String>,
    /// This machine's value; `None`: deleted here.
    pub ours: Option<String>,
    /// The repository's value; `None`: deleted there.
    pub theirs: Option<String>,
}

/// The person's decisions of single fields: by path, by pointer — only
/// [`Decision::KeepLocal`] (this machine's value) and
/// [`Decision::TakeRemote`] (the repository's) mean anything there.
pub type FieldDecisions = BTreeMap<String, BTreeMap<String, Decision>>;

/// `decisions.json` as a door writes it: a decision per path, and per field
/// of a JSON conflict.
///
/// It also READS the shape before fields existed — a flat map of paths — so
/// a door of an older build is still understood.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct DecisionsFile {
    /// Every decision of a whole path.
    pub paths: Decisions,
    /// Every decision of a single field.
    pub fields: FieldDecisions,
}

impl<'de> Deserialize<'de> for DecisionsFile {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Shape {
            #[serde(default)]
            paths: Decisions,
            #[serde(default)]
            fields: FieldDecisions,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Either {
            Fields(Shape),
            Flat(Decisions),
        }
        Ok(match Either::deserialize(deserializer)? {
            Either::Fields(s) => Self {
                paths: s.paths,
                fields: s.fields,
            },
            Either::Flat(paths) => Self {
                paths,
                fields: FieldDecisions::new(),
            },
        })
    }
}

impl DecisionsFile {
    /// Decisions of whole paths only.
    #[must_use]
    pub fn of_paths(paths: Decisions) -> Self {
        Self {
            paths,
            fields: FieldDecisions::new(),
        }
    }

    /// The file at `path`; `None` when it is missing or broken.
    #[must_use]
    pub fn read(path: &std::path::Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }

    /// The decision of one field: this machine's value or the repository's.
    #[must_use]
    pub fn field(&self, path: &str, pointer: &str) -> Option<Decision> {
        self.fields
            .get(path)?
            .get(pointer)
            .copied()
            .filter(|d| matches!(d, Decision::KeepLocal | Decision::TakeRemote))
    }

    /// Whether every field of `conflict`'s JSON merge is decided — which
    /// decides the path, when no decision of the whole path does.
    #[must_use]
    pub fn fields_settle(&self, conflict: &Conflict) -> bool {
        match &conflict.merge {
            Some(Merging::Json { fields }) if !fields.is_empty() => fields
                .iter()
                .all(|f| self.field(&conflict.path, &f.pointer).is_some()),
            _ => false,
        }
    }
}

impl Plan {
    /// Whether every conflict has an allowed decision in `decisions`.
    #[must_use]
    pub fn decided(&self, decisions: &Decisions) -> bool {
        self.conflicts
            .iter()
            .all(|c| decisions.get(&c.path).is_some_and(|d| d.allowed(c)))
    }

    /// Whether every conflict is decided by `file`: a decision of its whole
    /// path that it may take, or — a JSON conflict — one of every field.
    #[must_use]
    pub fn settled(&self, file: &DecisionsFile) -> bool {
        self.conflicts
            .iter()
            .all(|c| file.paths.get(&c.path).is_some_and(|d| d.allowed(c)) || file.fields_settle(c))
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

    /// FAILS IF: a merged decision is offered where no merge reached the
    /// conflict, a JSON conflict is not settled by deciding every field, or
    /// the decisions file of an older door stops reading.
    #[test]
    fn merged_decisions_and_fields_settle_what_a_merge_reached() {
        let json = Conflict {
            merge: Some(Merging::Json {
                fields: vec![
                    FieldConflict {
                        pointer: "/theme".to_owned(),
                        ..FieldConflict::default()
                    },
                    FieldConflict {
                        pointer: "/apps/nix:x".to_owned(),
                        ..FieldConflict::default()
                    },
                ],
            }),
            ..conflict("config/theme.json", false)
        };
        let text = Conflict {
            merge: Some(Merging::Text { hunks: 2 }),
            ..conflict("home/notes.md", true)
        };
        let binary = conflict("home/photo.png", true);
        for d in [Decision::TakeMergedOurs, Decision::TakeMergedTheirs] {
            assert!(d.allowed(&json) && d.allowed(&text));
            assert!(!d.allowed(&binary), "a merge nobody made is offered");
        }
        let plan = Plan {
            conflicts: vec![json, text],
            ..Plan::default()
        };
        let mut file = DecisionsFile::default();
        file.paths
            .insert("home/notes.md".to_owned(), Decision::TakeMergedTheirs);
        file.fields
            .entry("config/theme.json".to_owned())
            .or_default()
            .insert("/theme".to_owned(), Decision::TakeRemote);
        assert!(!plan.settled(&file), "one field of two settled the path");
        file.fields
            .entry("config/theme.json".to_owned())
            .or_default()
            .insert("/apps/nix:x".to_owned(), Decision::KeepBoth);
        assert!(!plan.settled(&file), "keep both is no field's decision");
        file.fields
            .entry("config/theme.json".to_owned())
            .or_default()
            .insert("/apps/nix:x".to_owned(), Decision::KeepLocal);
        assert!(plan.settled(&file));
        // THE FILE CROSSES WHOLE, and the flat shape of before still reads.
        let text = serde_json::to_string(&file).unwrap();
        assert_eq!(serde_json::from_str::<DecisionsFile>(&text).unwrap(), file);
        let flat: DecisionsFile =
            serde_json::from_str(r#"{"home/notes.md":"take-remote"}"#).unwrap();
        assert_eq!(flat.paths["home/notes.md"], Decision::TakeRemote);
        assert!(flat.fields.is_empty());
        assert_eq!(
            serde_json::from_str::<DecisionsFile>("{}").unwrap(),
            DecisionsFile::default()
        );
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
