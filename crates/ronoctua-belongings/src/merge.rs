//! The repository's index, and the three-way merge per path (feature
//! `crypt`) — pure: it reads the three sides and writes a [`Plan`].
//!
//! # The three sides
//!
//! The BASE is `last.json` (what the last sync of this machine left), OURS
//! this machine's files ([`crate::walk`]), THEIRS the repository's index. Per
//! path:
//!
//! | base → here | base → there | step |
//! |---|---|---|
//! | same | same | [`Step::Same`] |
//! | same | changed | [`Step::Pull`] (deleted there: [`Step::DeleteLocal`]) |
//! | changed | same | [`Step::Push`] (deleted here: [`Step::DeleteRemote`]) |
//! | changed | changed, to the same | [`Step::Same`] |
//! | changed | changed, differently | a [`Conflict`] |
//!
//! "Changed" is content, kind and mode — never the time (a copy changes it).
//! A deletion on one side and a change on the other is a conflict too.
//!
//! # A first sync
//!
//! With no base, nothing is deleted: a path on one side only is NEW there and
//! goes to the other, and a path on both that differs is a conflict that
//! starts marked "take the repository's" — the restore of a new installation.
//!
//! # What the person added
//!
//! A folder or file the person added travels to every machine AS A ROOT
//! (the index's `roots`): every machine walks it, so a file created inside it
//! on any of them is kept too. One machine that followed it and then takes
//! it out ([`roots_after`]) removes the root and every path under it from
//! the repository ([`Step::Unkeep`]), and the others FORGET what they had of
//! it ([`Step::Forget`]) — the files stay where they are, because "stop
//! keeping" is not "delete". A machine takes out only what it followed: a
//! root added again elsewhere, under a path this machine took out long ago,
//! is left to the machines that keep it.
//!
//! # What cannot be read
//!
//! A path this machine cannot read — or that lies in a folder it cannot read
//! — is neither present nor missing: left alone on both sides.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::kept;
use crate::places::covers;
use crate::plan::{Conflict, Decision, Entry, Plan, Side, Step};
use crate::prefs::Belongings;
use crate::status::Last;
use crate::walk::Found;

/// The index's format.
pub const INDEX_VERSION: u32 = 1;

/// One path in the repository.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Blob {
    /// The logical path.
    pub path: String,
    /// The blob's random id: `blobs/<id>.age`.
    pub id: String,
    /// What it is.
    #[serde(flatten)]
    pub side: Side,
    /// The machine that wrote it last — data.
    pub machine: String,
    /// Whether a person added it (it travels to every machine).
    pub added: bool,
}

/// The index (`index.age`, decrypted).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    /// [`INDEX_VERSION`].
    pub version: u32,
    /// Every path, in order.
    pub entries: Vec<Blob>,
    /// Every machine that synced, and when it last did (seconds since the
    /// epoch) — its name exists only here, encrypted.
    pub machines: BTreeMap<String, i64>,
    /// The folders and files a person added on any machine: every machine
    /// walks them.
    pub roots: BTreeSet<String>,
}

impl Index {
    /// An empty index.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            version: INDEX_VERSION,
            ..Self::default()
        }
    }

    /// The entries by path.
    #[must_use]
    pub fn by_path(&self) -> BTreeMap<&str, &Blob> {
        self.entries.iter().map(|b| (b.path.as_str(), b)).collect()
    }

    /// The machine that synced last, for a path deleted there.
    #[must_use]
    pub fn last_machine(&self) -> String {
        self.machines
            .iter()
            .max_by_key(|(_, at)| **at)
            .map(|(m, _)| m.clone())
            .unwrap_or_default()
    }
}

/// Whether the base's side and another are the same.
fn same(a: Option<&Side>, b: Option<&Side>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => a.same_as(b),
        _ => false,
    }
}

/// The roots the index says after this machine's sync: the repository's,
/// less those this machine FOLLOWED (its last sync's `roots`) and has taken
/// out since, and this machine's own additions. Pure.
#[must_use]
pub fn roots_after(base: Option<&Last>, remote: &Index, prefs: &Belongings) -> BTreeSet<String> {
    let followed = base.map(|b| &b.roots);
    let mut out: BTreeSet<String> = remote
        .roots
        .iter()
        .filter(|r| !(kept::excluded_here(r, prefs) && followed.is_some_and(|f| f.contains(*r))))
        .cloned()
        .collect();
    out.extend(
        prefs
            .extra
            .iter()
            .filter(|e| kept::may_add(e) && !kept::excluded_here(e, prefs))
            .cloned(),
    );
    out
}

/// Of `roots` (what [`roots_after`] gave), the ones this machine walks: not
/// taken out here — what it follows, and what its "stop keeping" may take
/// out next time. Pure.
#[must_use]
pub fn followed(roots: &BTreeSet<String>, prefs: &Belongings) -> BTreeSet<String> {
    roots
        .iter()
        .filter(|r| !kept::excluded_here(r, prefs))
        .cloned()
        .collect()
}

/// Whether one of `roots` covers `path`. Pure.
#[must_use]
pub fn under_a_root(roots: &BTreeSet<String>, path: &str) -> bool {
    roots.iter().any(|r| covers(r, path))
}

/// The plan for these three sides. `remote_commit` and `made_s` are the
/// engine's to fill. Pure.
#[must_use]
pub fn plan(base: Option<&Last>, local: &Found, remote: &Index, prefs: &Belongings) -> Plan {
    let theirs = remote.by_path();
    let known = base.map(|b| &b.entries);
    let after = roots_after(base, remote, prefs);
    // THE ROOTS THIS MACHINE TAKES OUT NOW: in the repository, and not after.
    let removed: BTreeSet<String> = remote.roots.difference(&after).cloned().collect();
    let mut paths: BTreeSet<&str> = local.files.keys().map(String::as_str).collect();
    paths.extend(theirs.keys().copied());
    if let Some(known) = known {
        paths.extend(known.keys().map(String::as_str));
    }
    let mut out = Plan {
        first_run: base.is_none(),
        ..Plan::default()
    };
    for path in paths {
        // UNREADABLE HERE — the path, or a folder it is in — is neither
        // missing nor present: left alone on both sides.
        if local.unreadable.iter().any(|u| covers(u, path)) {
            continue;
        }
        let r = theirs.get(path).copied();
        let b = known.and_then(|k| k.get(path));
        let l = local.files.get(path);
        let added_now = under_a_root(&after, path);
        if !kept::kept_here(path, prefs, added_now) {
            if r.is_some() && under_a_root(&removed, path) {
                // TAKEN OUT HERE, an addition this machine followed: it
                // leaves the repository, for every machine.
                out.entries.push(entry(path, Step::Unkeep));
            } else if r.is_none() && b.is_some_and(|b| b.added) {
                // ANOTHER MACHINE STOPPED KEEPING what a person had added:
                // forgotten here, and the file stays where it is.
                out.entries.push(entry(path, Step::Forget));
            }
            // ANYTHING ELSE stays in the repository for the machines that
            // keep it.
            continue;
        }
        let rs = r.map(|r| &r.side);
        let bs = b.map(crate::status::Known::side);
        if same(l, rs) {
            if l.is_some() {
                out.entries.push(entry(path, Step::Same));
            }
            continue;
        }
        let added = added_now || b.is_some_and(|b| b.added);
        // WHETHER THE REPOSITORY STILL HAS A ROOT OVER IT: an added path gone
        // from under a root that is still there was DELETED; gone with its
        // root, it was taken out.
        let root_there = under_a_root(&remote.roots, path);
        let conflict = |suggested: Option<Decision>| Conflict {
            path: path.to_owned(),
            local: l.cloned(),
            remote: rs.cloned(),
            machine: r.map_or_else(|| remote.last_machine(), |r| r.machine.clone()),
            text: false,
            added_by_person: added,
            suggested,
            theirs: None,
        };
        let Some(_) = base else {
            // A FIRST SYNC: nothing is deleted, and what differs starts as
            // "take the repository's".
            match (l, rs) {
                (Some(_), None) => out.entries.push(entry(path, Step::Push)),
                (None, Some(_)) => out.entries.push(entry(path, Step::Pull)),
                _ => out.conflicts.push(conflict(Some(Decision::TakeRemote))),
            }
            continue;
        };
        let changed_here = !same(bs.as_ref(), l);
        let changed_there = !same(bs.as_ref(), rs);
        let step = match (changed_here, changed_there) {
            (false, true) => match rs {
                Some(_) => Step::Pull,
                // GONE THERE WITH ITS ROOT: another machine stopped keeping
                // it. This machine adds it again when it added it itself,
                // and forgets it otherwise.
                None if b.is_some_and(|b| b.added) && !root_there => {
                    if kept::added_here(path, prefs) {
                        Step::Push
                    } else {
                        Step::Forget
                    }
                }
                // GONE THERE ALONE: deleted, and the deletion travels.
                None => Step::DeleteLocal,
            },
            (true, false) => match l {
                Some(_) => Step::Push,
                None => Step::DeleteRemote,
            },
            _ => {
                out.conflicts.push(conflict(None));
                continue;
            }
        };
        out.entries.push(entry(path, step));
    }
    out
}

fn entry(path: &str, step: Step) -> Entry {
    Entry {
        path: path.to_owned(),
        step,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Kind;
    use crate::status::Known;

    fn side(sha: &str) -> Side {
        Side {
            size_bytes: 3,
            modified_s: 1,
            sha256: sha.to_owned(),
            kind: Kind::File,
            mode: 0o644,
        }
    }

    fn known(sha: &str) -> Known {
        Known {
            sha256: sha.to_owned(),
            mode: 0o644,
            size_bytes: 3,
            ..Known::default()
        }
    }

    fn blob(path: &str, sha: &str) -> Blob {
        Blob {
            path: path.to_owned(),
            id: format!("id-{sha}"),
            side: side(sha),
            machine: "laptop".to_owned(),
            added: false,
        }
    }

    fn step_of(plan: &Plan, path: &str) -> Option<Step> {
        plan.entries.iter().find(|e| e.path == path).map(|e| e.step)
    }

    /// FAILS IF: a change on one side is not taken, a change on both is not a
    /// conflict, a deletion is not followed, or a deletion against a change
    /// is taken without asking.
    #[test]
    fn one_side_changed_is_taken_and_both_changed_is_asked() {
        let base = Last {
            repository: "r".to_owned(),
            entries: [
                ("config/same.json", "a"),
                ("config/here.json", "a"),
                ("config/there.json", "a"),
                ("config/both.json", "a"),
                ("config/both-alike.json", "a"),
                ("config/gone-here.json", "a"),
                ("config/gone-there.json", "a"),
                ("config/gone-here-changed-there.json", "a"),
            ]
            .into_iter()
            .map(|(p, s)| (p.to_owned(), known(s)))
            .collect(),
            ..Last::default()
        };
        let mut local = Found::default();
        for (p, s) in [
            ("config/same.json", "a"),
            ("config/here.json", "b"),
            ("config/there.json", "a"),
            ("config/both.json", "b"),
            ("config/both-alike.json", "c"),
            ("config/gone-there.json", "a"),
            ("config/new-here.json", "n"),
        ] {
            local.files.insert(p.to_owned(), side(s));
        }
        let remote = Index {
            version: INDEX_VERSION,
            entries: vec![
                blob("config/same.json", "a"),
                blob("config/here.json", "a"),
                blob("config/there.json", "z"),
                blob("config/both.json", "z"),
                blob("config/both-alike.json", "c"),
                blob("config/gone-here.json", "a"),
                blob("config/gone-here-changed-there.json", "z"),
                blob("config/new-there.json", "m"),
            ],
            ..Index::fresh()
        };
        let plan = plan(Some(&base), &local, &remote, &Belongings::default());
        assert_eq!(step_of(&plan, "config/same.json"), Some(Step::Same));
        assert_eq!(step_of(&plan, "config/here.json"), Some(Step::Push));
        assert_eq!(step_of(&plan, "config/there.json"), Some(Step::Pull));
        assert_eq!(step_of(&plan, "config/both-alike.json"), Some(Step::Same));
        assert_eq!(
            step_of(&plan, "config/gone-here.json"),
            Some(Step::DeleteRemote)
        );
        assert_eq!(
            step_of(&plan, "config/gone-there.json"),
            Some(Step::DeleteLocal)
        );
        assert_eq!(step_of(&plan, "config/new-here.json"), Some(Step::Push));
        assert_eq!(step_of(&plan, "config/new-there.json"), Some(Step::Pull));
        let asked: Vec<&str> = plan.conflicts.iter().map(|c| c.path.as_str()).collect();
        assert_eq!(
            asked,
            ["config/both.json", "config/gone-here-changed-there.json"]
        );
        assert!(plan.conflicts.iter().all(|c| c.suggested.is_none()));
        assert_eq!(plan.conflicts[0].machine, "laptop");
        assert!(plan.conflicts[1].local.is_none());
        assert!(!plan.first_run);
    }

    /// FAILS IF: a first sync deletes anything, or does not start a
    /// difference as "take the repository's".
    #[test]
    fn a_first_sync_deletes_nothing_and_restores_what_differs() {
        let mut local = Found::default();
        local.files.insert("config/mine.json".to_owned(), side("m"));
        local
            .files
            .insert("config/theme.json".to_owned(), side("fresh"));
        let remote = Index {
            entries: vec![
                blob("config/theirs.json", "t"),
                blob("config/theme.json", "chosen"),
            ],
            ..Index::fresh()
        };
        let plan = plan(None, &local, &remote, &Belongings::default());
        assert!(plan.first_run);
        assert_eq!(step_of(&plan, "config/mine.json"), Some(Step::Push));
        assert_eq!(step_of(&plan, "config/theirs.json"), Some(Step::Pull));
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].suggested, Some(Decision::TakeRemote));
        assert!(plan.decided(&plan.suggested()));
    }

    /// FAILS IF: a path this machine does not keep is deleted from the
    /// repository, an unreadable file is taken for a deleted one, or a path a
    /// person stopped keeping is deleted anywhere instead of forgotten.
    #[test]
    fn what_is_not_kept_here_is_left_alone_and_added_paths_are_forgotten() {
        let mut prefs = Belongings::default();
        prefs.excluded.push("config/gestures.json".to_owned());
        let base = Last {
            entries: [
                ("config/gestures.json".to_owned(), known("g")),
                ("config/locked.json".to_owned(), known("l")),
                (
                    "home/notes/a.md".to_owned(),
                    Known {
                        added: true,
                        ..known("n")
                    },
                ),
            ]
            .into_iter()
            .collect(),
            ..Last::default()
        };
        let mut local = Found::default();
        local.unreadable.insert("config/locked.json".to_owned());
        local.files.insert("home/notes/a.md".to_owned(), side("n"));
        let remote = Index {
            entries: vec![
                blob("config/gestures.json", "g"),
                blob("config/locked.json", "l"),
            ],
            ..Index::fresh()
        };
        let plan = plan(Some(&base), &local, &remote, &prefs);
        assert_eq!(
            step_of(&plan, "config/gestures.json"),
            None,
            "excluded, left alone"
        );
        assert_eq!(
            step_of(&plan, "config/locked.json"),
            None,
            "unreadable, left alone"
        );
        // Another machine stopped keeping the notes: forgotten, not deleted.
        assert_eq!(step_of(&plan, "home/notes/a.md"), Some(Step::Forget));

        // AND THE MACHINE THAT STOPPED KEEPING IT — one that followed the root
        // — takes it out of the repository, root and all.
        let mut elsewhere = Belongings::default();
        assert!(elsewhere.stop_keeping("home/notes"));
        let followed = Last {
            roots: notes_root(),
            ..base.clone()
        };
        let remote = Index {
            entries: vec![Blob {
                added: true,
                ..blob("home/notes/a.md", "n")
            }],
            roots: notes_root(),
            ..Index::fresh()
        };
        let plan = super::plan(Some(&followed), &local, &remote, &elsewhere);
        assert_eq!(step_of(&plan, "home/notes/a.md"), Some(Step::Unkeep));
        assert!(roots_after(Some(&followed), &remote, &elsewhere).is_empty());

        // A MACHINE THAT NEVER FOLLOWED IT leaves it to the others: its old
        // "taken out" does not take out a root added again elsewhere.
        let plan = super::plan(Some(&base), &local, &remote, &elsewhere);
        assert_eq!(step_of(&plan, "home/notes/a.md"), None);
        assert_eq!(
            roots_after(Some(&base), &remote, &elsewhere),
            notes_root(),
            "a root this machine never followed was taken out"
        );
    }

    fn notes_root() -> BTreeSet<String> {
        ["home/notes".to_owned()].into_iter().collect()
    }

    /// FAILS IF: a file created in an added folder on another machine is not
    /// kept here, a file deleted under a root that is still there is
    /// forgotten instead of deleted, or one gone with its root is deleted.
    #[test]
    fn an_added_folder_travels_as_a_folder() {
        let prefs = Belongings::default();
        let added = |sha: &str| Known {
            added: true,
            ..known(sha)
        };
        let base = Last {
            entries: [
                ("home/notes/a.md".to_owned(), added("a")),
                ("home/notes/gone.md".to_owned(), added("g")),
            ]
            .into_iter()
            .collect(),
            roots: notes_root(),
            ..Last::default()
        };
        let mut local = Found::default();
        local.files.insert("home/notes/a.md".to_owned(), side("a"));
        local
            .files
            .insert("home/notes/gone.md".to_owned(), side("g"));
        local
            .files
            .insert("home/notes/new.md".to_owned(), side("n"));
        let remote = Index {
            entries: vec![Blob {
                added: true,
                ..blob("home/notes/a.md", "a")
            }],
            roots: notes_root(),
            ..Index::fresh()
        };
        let plan = plan(Some(&base), &local, &remote, &prefs);
        assert_eq!(
            step_of(&plan, "home/notes/new.md"),
            Some(Step::Push),
            "a file new in the folder, on a machine that did not add it"
        );
        assert_eq!(
            step_of(&plan, "home/notes/gone.md"),
            Some(Step::DeleteLocal),
            "deleted there, under a root still there"
        );
        // THE ROOT GONE: forgotten, all of it, the new file left alone.
        let remote = Index {
            entries: Vec::new(),
            ..Index::fresh()
        };
        let plan = super::plan(Some(&base), &local, &remote, &prefs);
        assert_eq!(step_of(&plan, "home/notes/a.md"), Some(Step::Forget));
        assert_eq!(step_of(&plan, "home/notes/new.md"), None);
        // UNLESS THIS MACHINE ADDED IT ITSELF: then it goes back.
        let mut mine = Belongings::default();
        assert!(mine.keep("home/notes"));
        let plan = super::plan(Some(&base), &local, &remote, &mine);
        assert_eq!(step_of(&plan, "home/notes/a.md"), Some(Step::Push));
        assert_eq!(roots_after(Some(&base), &remote, &mine), notes_root());
    }

    /// FAILS IF: the files of a FOLDER this machine cannot read are taken for
    /// deleted — which would delete them on every other machine.
    #[test]
    fn a_folder_that_cannot_be_read_is_left_alone() {
        let base = Last {
            entries: [
                ("config/studio/scenes.json".to_owned(), known("s")),
                ("config/studio/lights.json".to_owned(), known("l")),
            ]
            .into_iter()
            .collect(),
            ..Last::default()
        };
        let mut local = Found::default();
        local.unreadable.insert("config/studio".to_owned());
        let remote = Index {
            entries: vec![
                blob("config/studio/scenes.json", "s"),
                blob("config/studio/lights.json", "changed there"),
            ],
            ..Index::fresh()
        };
        let plan = plan(Some(&base), &local, &remote, &Belongings::default());
        assert!(plan.entries.is_empty(), "{:?}", plan.entries);
        assert!(plan.conflicts.is_empty(), "{:?}", plan.conflicts);
    }

    #[test]
    fn a_mode_change_is_a_change_and_a_time_is_not() {
        let base = Last {
            entries: [("config/k".to_owned(), known("a"))].into_iter().collect(),
            ..Last::default()
        };
        let mut local = Found::default();
        local.files.insert(
            "config/k".to_owned(),
            Side {
                mode: 0o600,
                modified_s: 999,
                ..side("a")
            },
        );
        let remote = Index {
            entries: vec![Blob {
                side: Side {
                    modified_s: 5,
                    ..side("a")
                },
                ..blob("config/k", "a")
            }],
            ..Index::fresh()
        };
        let plan = plan(Some(&base), &local, &remote, &Belongings::default());
        assert_eq!(step_of(&plan, "config/k"), Some(Step::Push));
    }
}
