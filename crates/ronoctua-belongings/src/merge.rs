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

use crate::chunks::ChunkRef;
use crate::fleet::Seen;
use crate::kept;
use crate::places::covers;
use crate::plan::{Conflict, Decision, Entry, Plan, Side, Step};
use crate::prefs::Belongings;
use crate::status::Last;
use crate::walk::Found;

/// The index's format (format 2's: files are chunks).
pub const INDEX_VERSION: u32 = 2;

/// One path in the repository.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Blob {
    /// The logical path.
    pub path: String,
    /// What it is.
    #[serde(flatten)]
    pub side: Side,
    /// The machine that wrote it last — data.
    pub machine: String,
    /// Whether a person added it (it travels to every machine).
    pub added: bool,
    /// Its content, in order: `chunks/<aa>/<id>.age` each (a link's: its
    /// target's text). Empty for an empty file.
    pub chunks: Vec<ChunkRef>,
}

/// The index (`index.age`, decrypted).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    /// [`INDEX_VERSION`].
    pub version: u32,
    /// Every path, in order.
    pub entries: Vec<Blob>,
    /// Every machine that synced: when it last did, and when the distro it
    /// runs was built — its name exists only here, encrypted.
    pub machines: BTreeMap<String, Seen>,
    /// The folders and files a person added on any machine: every machine
    /// walks them.
    pub roots: BTreeSet<String>,
    // ── anchor: L08 ──
    /// The predefined targets a person stopped keeping on EVERY machine, by
    /// id: who did and when (the belongings-targets-unlock package fills
    /// it; this one carries it whole through every sync).
    pub dropped: BTreeMap<String, Dropped>,
    /// Folder modes to restore, by logical path — a private target's
    /// folders come back `0700` (filled by the belongings-targets-unlock
    /// package; carried whole by every sync).
    pub dirs: BTreeMap<String, u32>,
    // ── anchor: L10 ──
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
            .max_by_key(|(_, seen)| seen.at_s)
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

// ── anchor: L08 ──
/// A target a person stopped keeping everywhere.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Dropped {
    /// The machine that dropped it — data.
    pub by: String,
    /// When, seconds since the epoch.
    pub at_s: i64,
}

/// `index.age`, opened: where the index itself is. The index is chunked
/// like any file — its body grows with the files kept, and only the chunks
/// a sync changed move — so the one small sealed file names its chunks.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexHead {
    /// [`INDEX_VERSION`]: a newer one is a newer format.
    pub version: u32,
    /// The index's size, in bytes.
    pub size_bytes: u64,
    /// The index's SHA-256, hexadecimal.
    pub sha256: String,
    /// The index's chunks, in order.
    pub chunks: Vec<ChunkRef>,
}

/// Every chunk an index and its head name. Pure.
#[must_use]
pub fn referenced(index: &Index, head: Option<&IndexHead>) -> BTreeSet<String> {
    index
        .entries
        .iter()
        .flat_map(|b| &b.chunks)
        .chain(head.into_iter().flat_map(|h| &h.chunks))
        .map(|c| c.id.clone())
        .collect()
}
// ── anchor: L09 ──
// ── anchor: L10 ──
/// What becomes of a path of a target NO MACHINE KEEPS any more (the index's
/// `dropped`), given whether the repository has a copy and whether this
/// machine's last sync kept it: a copy leaves the repository — whichever
/// machine sees it, the one that dropped the target first, or one whose
/// push raced the drop — and a path this machine kept is forgotten, the
/// file left where it is. "Not on any machine" is never "delete". Pure.
#[must_use]
pub const fn dropped_rule(in_repository: bool, kept_before: bool) -> Option<Step> {
    if in_repository {
        Some(Step::Unkeep)
    } else if kept_before {
        Some(Step::Forget)
    } else {
        None
    }
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
    // THE TARGETS NO MACHINE KEEPS any more (`dropped_rule`).
    let dropped: BTreeSet<String> = remote.dropped.keys().cloned().collect();
    for path in paths {
        // UNREADABLE HERE — the path, or a folder it is in — is neither
        // missing nor present: left alone on both sides.
        if local.unreadable.iter().any(|u| covers(u, path)) {
            continue;
        }
        // SET ASIDE FOR WHAT IT IS (`crate::sift`) — the path, or a link to
        // a folder it is under: nothing is pulled through it.
        if let Some((_, why)) = local.never.iter().find(|(n, _)| covers(n, path)) {
            // THE ACCESS KEY under another name, in the repository already
            // (pushed before this rule): it leaves it, for every machine —
            // the others then delete the copy they took. Left there, every
            // other machine would keep pulling this machine's key into its
            // own `~/.ssh`.
            if *why == crate::sift::ACCESS && theirs.contains_key(path) {
                out.entries.push(entry(path, Step::Unkeep));
            }
            // A LINK THE SYSTEM MADE is neither deleted from the repository
            // (another machine may keep a real file there) nor pulled over.
            continue;
        }
        let r = theirs.get(path).copied();
        let b = known.and_then(|k| k.get(path));
        let l = local.files.get(path);
        if kept::dropped_everywhere(path, &dropped) {
            if let Some(step) = dropped_rule(r.is_some(), b.is_some()) {
                out.entries.push(entry(path, step));
            }
            continue;
        }
        let added_now = under_a_root(&after, path);
        if !kept::kept_here(path, prefs, added_now) {
            // A TARGET'S FILE never leaves with an addition over it (one
            // above the target, the whole home, or one made by hand at its
            // path): the target is kept on the other machines whatever this
            // one took out ("not on this machine" leaves the repository's
            // copies to them), and a target leaves the repository only by
            // "not on any machine" (`dropped_rule`, above). Without it, a
            // machine that did not sync while the addition lasted — its base
            // says the file is the target's, not added — would read the
            // file's leaving as a deletion and delete its own copy.
            if r.is_some() && under_a_root(&removed, path) && kept::target_of(path).is_none() {
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
            held: None,
            merge: None,
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
            side: side(sha),
            machine: "laptop".to_owned(),
            ..Blob::default()
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

    /// FAILS IF: a path a predefined target of an older build kept — the
    /// `doom` target's `local.el`, replaced by `emacs` on 2026-10-19: in the
    /// repository, in this machine's base, its id maybe in `dropped` — is
    /// deleted from the repository, deleted or changed here, pulled to a new
    /// machine, or forgotten as if a person had stopped keeping it. No table
    /// names it any more, so nothing moves it on either side; and a request
    /// about the old id is refused instead of waiting for ever.
    #[test]
    fn a_path_no_target_names_any_more_stays_put() {
        let old = "home/.config/doom/local.el";
        assert!(kept::target_of(old).is_none() && kept::target_by_id("doom").is_none());
        let prefs = Belongings::default();
        let remote = Index {
            entries: vec![blob(old, "d")],
            dropped: [("doom".to_owned(), Dropped::default())]
                .into_iter()
                .collect(),
            ..Index::fresh()
        };
        let base = Last {
            repository: "r".to_owned(),
            entries: [(old.to_owned(), known("d"))].into_iter().collect(),
            ..Last::default()
        };
        // THE MACHINE THAT PUSHED IT, whose walk no longer meets it.
        assert_eq!(
            step_of(&plan(Some(&base), &Found::default(), &remote, &prefs), old),
            None
        );
        // EVEN MET, AND CHANGED HERE (a person who adds it back by hand takes
        // the other path: an addition).
        let mut local = Found::default();
        local.files.insert(old.to_owned(), side("e"));
        assert_eq!(
            step_of(&plan(Some(&base), &local, &remote, &prefs), old),
            None
        );
        // A NEW MACHINE: nothing pulled — the restore asks the same rule.
        assert_eq!(
            step_of(&plan(None, &Found::default(), &remote, &prefs), old),
            None
        );
        assert!(!kept::kept_here(old, &prefs, false));
        assert!(
            !crate::requests::Request::DropEverywhere {
                target: "doom".to_owned()
            }
            .is_known()
        );
    }

    /// FAILS IF: an addition made by hand at a target's path before the
    /// target existed (`home/.config/emacs`, before 2026-10-19) stops being
    /// what it was — a root of its own, whose files travel as the person's
    /// addition — or adds what the target skips; or the target's files leave
    /// the repository when it is taken out here ("not on this machine" on the
    /// target takes the addition with it, `Belongings::stop_keeping`), or
    /// another machine, or a new one, moves the package it once sent.
    #[test]
    fn an_old_addition_at_a_target_is_a_root_and_takes_no_file_out() {
        let added = "home/.config/emacs";
        let init = "home/.config/emacs/init.el";
        let package = "home/.config/emacs/elpa/magit-4.1/magit.el";
        let remote = Index {
            entries: vec![
                Blob {
                    added: true,
                    ..blob(init, "i")
                },
                Blob {
                    added: true,
                    ..blob(package, "p")
                },
            ],
            roots: [added.to_owned()].into_iter().collect(),
            ..Index::fresh()
        };
        let base = Last {
            repository: "r".to_owned(),
            entries: [init, package]
                .into_iter()
                .map(|p| {
                    (
                        p.to_owned(),
                        Known {
                            added: true,
                            ..known(if p == init { "i" } else { "p" })
                        },
                    )
                })
                .collect(),
            roots: [added.to_owned()].into_iter().collect(),
            ..Last::default()
        };
        let mut local = Found::default();
        local.files.insert(init.to_owned(), side("i"));
        // THE MACHINE THAT MADE IT: a root, as before the target; the
        // configuration syncs, and the package stays where it is.
        let prefs = Belongings {
            extra: vec![added.to_owned()],
            ..Belongings::default()
        };
        let planned = plan(Some(&base), &local, &remote, &prefs);
        assert_eq!(step_of(&planned, init), Some(Step::Same));
        assert_eq!(step_of(&planned, package), None, "the package moved");
        assert_eq!(
            roots_after(Some(&base), &remote, &prefs),
            [added.to_owned()].into_iter().collect()
        );
        // ANOTHER MACHINE TOOK THE ROOT OUT, its files with it (one on
        // a88e849, which has no guard): this one, which added it itself,
        // adds it again — as before the target.
        let gone = Index {
            entries: Vec::new(),
            ..Index::fresh()
        };
        let planned = plan(Some(&base), &local, &gone, &prefs);
        assert_eq!(step_of(&planned, init), Some(Step::Push));
        assert_eq!(step_of(&planned, package), Some(Step::Forget));
        // "NOT ON THIS MACHINE" on the target: the addition goes with it,
        // and no file leaves the repository — the others keep the target.
        let mut prefs = Belongings {
            extra: vec![added.to_owned(), format!("{added}/lisp")],
            ..Belongings::default()
        };
        for path in kept::target_by_id("emacs").unwrap().paths {
            prefs.stop_keeping(path);
        }
        assert_eq!(prefs.extra, [format!("{added}/lisp")]);
        let planned = plan(Some(&base), &local, &remote, &prefs);
        assert_eq!(step_of(&planned, init), None, "the others' copy left");
        assert_eq!(step_of(&planned, package), None, "the package moved");
        assert_eq!(
            roots_after(Some(&base), &remote, &prefs),
            [format!("{added}/lisp")].into_iter().collect()
        );
        // ANOTHER MACHINE, which follows the root: in step, the package
        // never pulled — and a new one takes the configuration alone.
        let planned = plan(Some(&base), &local, &remote, &Belongings::default());
        assert_eq!(step_of(&planned, init), Some(Step::Same));
        assert_eq!(step_of(&planned, package), None);
        let planned = plan(None, &Found::default(), &remote, &Belongings::default());
        assert_eq!(step_of(&planned, init), Some(Step::Pull));
        assert_eq!(step_of(&planned, package), None, "a package was pulled");
    }

    /// FAILS IF: a file the target skips — in the repository, in a base, on
    /// a disk, added or not, under no root, a root above the target (the
    /// whole home) or a root at its path (an addition from before it), the
    /// root in the repository, in this machine's base (followed), both or
    /// neither, and this machine adding, keeping or taking out the target
    /// or the root — is pushed, pulled, deleted on either side, taken out
    /// of the repository or made a question. What the table skips is never
    /// a reason to touch a file another machine has (the second review's
    /// M4), and a root taken out takes no file of a target with it; the
    /// most a sync does is forget a base's copy of what a person once added
    /// (the file stays where it is).
    #[test]
    fn what_a_target_skips_is_never_moved_nor_deleted() {
        let target = "home/.config/emacs";
        let skipped = "home/.config/emacs/eln-cache/30.2/x.eln";
        assert!(kept::target_of(skipped).is_some_and(|t| t.skips(skipped)));
        let mut seen = BTreeSet::new();
        for root in [None, Some("home"), Some(target)] {
            for (in_repo, in_base, on_disk, added, rooted, followed) in (0..64).map(|n| {
                (
                    n & 1 != 0,
                    n & 2 != 0,
                    n & 4 != 0,
                    n & 8 != 0,
                    n & 16 != 0,
                    n & 32 != 0,
                )
            }) {
                if root.is_none() && (rooted || followed) {
                    continue;
                }
                let roots = |on: bool| -> BTreeSet<String> {
                    root.filter(|_| on).map(str::to_owned).into_iter().collect()
                };
                let remote = Index {
                    entries: in_repo
                        .then(|| Blob {
                            added,
                            ..blob(skipped, "r")
                        })
                        .into_iter()
                        .collect(),
                    roots: roots(rooted),
                    ..Index::fresh()
                };
                let base = Last {
                    repository: "r".to_owned(),
                    entries: in_base
                        .then(|| {
                            (
                                skipped.to_owned(),
                                Known {
                                    added,
                                    ..known("b")
                                },
                            )
                        })
                        .into_iter()
                        .collect(),
                    roots: roots(followed),
                    ..Last::default()
                };
                let mut local = Found::default();
                if on_disk {
                    local.files.insert(skipped.to_owned(), side("l"));
                }
                let mut choices = vec![
                    Belongings::default(),
                    Belongings {
                        extra: vec![target.to_owned()],
                        ..Belongings::default()
                    },
                    Belongings {
                        excluded: vec![target.to_owned()],
                        ..Belongings::default()
                    },
                ];
                if let Some(root) = root {
                    choices.push(Belongings {
                        extra: vec![root.to_owned()],
                        ..Belongings::default()
                    });
                    choices.push(Belongings {
                        excluded: vec![root.to_owned()],
                        ..Belongings::default()
                    });
                }
                for prefs in &choices {
                    for base in [None, Some(&base)] {
                        let planned = plan(base, &local, &remote, prefs);
                        let step = step_of(&planned, skipped);
                        let forgets = base.is_some() && in_base && added && !in_repo;
                        assert_eq!(
                            step,
                            forgets.then_some(Step::Forget),
                            "root {root:?} in repo {rooted} followed {followed} | file: repo \
                             {in_repo} base {in_base} disk {on_disk} added {added} | {prefs:?}"
                        );
                        assert!(planned.conflicts.is_empty());
                        // WHAT THE LOOP REACHED: a root taken out now, with the
                        // file in the repository — the case a missing guard
                        // turns into `Unkeep`.
                        let after = roots_after(base, &remote, prefs);
                        if in_repo && remote.roots.difference(&after).next().is_some() {
                            seen.insert(root);
                        }
                    }
                }
            }
        }
        assert_eq!(
            seen,
            [Some("home"), Some(target)].into_iter().collect(),
            "a root taken out with the file in the repository was never planned"
        );
    }

    /// FAILS IF: a machine that takes out an addition ABOVE a target (the
    /// whole home, added on another machine) while the target is "not on
    /// this machine" here takes the target's files out of the repository —
    /// the others keep the target — or stops taking the addition's own
    /// files out.
    #[test]
    fn a_root_taken_out_above_a_target_leaves_the_targets_files() {
        let init = "home/.config/emacs/init.el";
        let notes = "home/notes/a.md";
        let added = |path: &str, sha: &str| Blob {
            added: true,
            ..blob(path, sha)
        };
        let remote = Index {
            entries: vec![added(init, "i"), added(notes, "n")],
            roots: ["home".to_owned()].into_iter().collect(),
            ..Index::fresh()
        };
        let base = Last {
            repository: "r".to_owned(),
            entries: [init, notes]
                .into_iter()
                .map(|p| {
                    (
                        p.to_owned(),
                        Known {
                            added: true,
                            ..known(if p == init { "i" } else { "n" })
                        },
                    )
                })
                .collect(),
            roots: ["home".to_owned()].into_iter().collect(),
            ..Last::default()
        };
        let prefs = Belongings {
            excluded: vec!["home".to_owned(), "home/.config/emacs".to_owned()],
            ..Belongings::default()
        };
        let mut local = Found::default();
        local.files.insert(init.to_owned(), side("i"));
        local.files.insert(notes.to_owned(), side("n"));
        let planned = plan(Some(&base), &local, &remote, &prefs);
        assert_eq!(step_of(&planned, init), None, "the target's file left");
        assert_eq!(step_of(&planned, notes), Some(Step::Unkeep));
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
