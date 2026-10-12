//! What the belongings keep, read once (feature `prefs`).
//!
//! # One reader of the tables
//!
//! `assets/belongings.nix` generates the tables ([`crate::kept`]): the trees
//! and the files kept by default, the predefined targets, the roots of the
//! plain part and what is never kept. The person adds folders and files and
//! takes some out (`belongings.json`), and what a person added on any of
//! their machines travels in the index as a root this machine follows
//! (`last.json`'s `roots`). A [`Scope`] is all of that, read in ONE place —
//! here, and in `kept.rs`, whose arithmetic it asks — and nothing else in the
//! workspace reads those tables (`belongings-list-is-the-walk`).
//!
//! # Why
//!
//! Because two things must agree for ever: the WALK, which decides what a
//! sync sends (`crate::walk`, the engine's), and the LISTING, which tells the
//! person what goes to their repository (`crate::inventory`, a door's). Both
//! start at [`Scope::roots`] and meet every path through [`Scope::meet`], so a
//! root added to the tables, a target, a plain root or a person's addition
//! is walked and listed alike — the page cannot show one set of files while
//! the sync sends another.

use std::collections::BTreeSet;
use std::path::Path;

use crate::kept::{self, Never, Place, PlainRoot, Target};
use crate::places::{Places, covers, is_logical};
use crate::prefs::Belongings;
use crate::requests::{Request, Requests};
use crate::status::Last;

/// The rules kept by default, each with whether it is a folder kept whole
/// (a tree) or a file. The table's.
pub fn rules() -> impl Iterator<Item = (&'static Place, bool)> {
    kept::TREES
        .iter()
        .map(|p| (p, true))
        .chain(kept::FILES.iter().map(|p| (p, false)))
}

/// The predefined encrypted targets. The table's.
#[must_use]
pub fn targets() -> &'static [Target] {
    kept::TARGETS
}

/// The roots of the plain part. The table's.
#[must_use]
pub fn plain_roots() -> &'static [PlainRoot] {
    kept::PLAIN
}

/// What is never kept, and why. The table's.
#[must_use]
pub fn never_rules() -> &'static [Never] {
    kept::NEVER
}

/// Where a path of the listing comes from — the group the page draws it in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Origin {
    /// A rule of the table: a tree or a file kept by default (its logical
    /// path).
    Rule(&'static str),
    /// A predefined target (its id).
    Target(&'static str),
    /// A folder or file the person added on this machine (the addition).
    Added(String),
    /// A folder or file the person added on another machine, which this one
    /// follows (the root).
    AddedElsewhere(String),
    /// A root of the plain part (its logical path).
    Plain(&'static str),
}

/// One path the walk meets.
#[derive(Debug, Clone, Copy)]
pub enum Met<'a> {
    /// A folder that could not be listed: what is in it is hidden — the
    /// walk takes it for every path under it.
    Hidden,
    /// Anything else, as `lstat` read it: a file, a link, a socket, a pipe.
    Entry(&'a std::fs::Metadata),
}

/// How deep a walk goes: a loop of bind mounts must end somewhere.
const DEEPEST: usize = 64;

/// What the belongings keep on this machine: the tables, the person's
/// choices, what this machine follows, and what no machine keeps any more.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Scope {
    /// What the person chose here.
    prefs: Belongings,
    /// The folders and files a person added on any machine that this one
    /// walks (the index's roots after this sync, or what the last sync
    /// followed).
    elsewhere: BTreeSet<String>,
    /// The paths the walk also looks at: what the base names, so that a
    /// deletion here is seen.
    looked: BTreeSet<String>,
    /// The targets no machine keeps, by id — the index's as the last sync
    /// left it, with the person's requests laid over.
    dropped: BTreeSet<String>,
    /// What a door asked of the next sync.
    asked: Requests,
    /// What the last sync left, when it is of the repository set now — what
    /// the listing compares each file with.
    base: Option<Last>,
    /// The paths a prepared sync waits on the person for.
    waiting: BTreeSet<String>,
}

impl Scope {
    /// What a DOOR reads: the settings, what the last sync left (its roots,
    /// what no machine keeps, what it kept) and what the person asked of the
    /// next sync. A `last` of another repository is no base.
    #[must_use]
    pub fn of(prefs: &Belongings, last: Option<&Last>, requests: &Requests) -> Self {
        let base = last.filter(|l| !l.repository.is_empty() && l.repository == prefs.repository);
        let followed = base.map(|b| b.roots.clone()).unwrap_or_default();
        let mut elsewhere: BTreeSet<String> = followed
            .into_iter()
            .filter(|r| !kept::excluded_here(r, prefs))
            .collect();
        elsewhere.extend(
            prefs
                .extra
                .iter()
                .filter(|e| kept::may_add(e) && !kept::excluded_here(e, prefs))
                .cloned(),
        );
        let looked = base
            .map(|b| b.entries.keys().cloned().collect())
            .unwrap_or_default();
        let mut dropped: BTreeSet<String> = base.map(|b| b.dropped.clone()).unwrap_or_default();
        for request in &requests.requests {
            match request {
                Request::DropEverywhere { target } => {
                    dropped.insert(target.clone());
                }
                Request::KeepEverywhere { target } => {
                    dropped.remove(target);
                }
                Request::Compact { .. } => {}
            }
        }
        Self {
            prefs: prefs.clone(),
            elsewhere,
            looked,
            dropped,
            asked: requests.clone(),
            base: base.cloned(),
            waiting: BTreeSet::new(),
        }
    }

    /// What a SYNC walks: the settings, the roots it follows after reading
    /// the index (`crate::merge::roots_after`) and the paths its base names.
    #[must_use]
    pub fn for_sync(
        prefs: &Belongings,
        elsewhere: &BTreeSet<String>,
        looked: &BTreeSet<String>,
    ) -> Self {
        Self {
            prefs: prefs.clone(),
            elsewhere: elsewhere.clone(),
            looked: looked.clone(),
            ..Self::default()
        }
    }

    /// The same, with the paths a prepared sync waits on the person for.
    #[must_use]
    pub fn waiting_on(mut self, paths: impl IntoIterator<Item = String>) -> Self {
        self.waiting = paths.into_iter().collect();
        self
    }

    /// The settings.
    #[must_use]
    pub const fn prefs(&self) -> &Belongings {
        &self.prefs
    }

    /// What the last sync left, when it is a base.
    #[must_use]
    pub const fn base(&self) -> Option<&Last> {
        self.base.as_ref()
    }

    /// Whether a prepared sync waits on the person for `path`.
    #[must_use]
    pub fn waits_on(&self, path: &str) -> bool {
        self.waiting.contains(path)
    }

    /// The roots this machine follows that a person added (here or on any
    /// machine).
    #[must_use]
    pub const fn elsewhere(&self) -> &BTreeSet<String> {
        &self.elsewhere
    }

    /// Whether no machine keeps the target `id` any more — or the person
    /// asked that of the next sync.
    #[must_use]
    pub fn dropped(&self, id: &str) -> bool {
        self.dropped.contains(id)
    }

    /// What the person asked of the next sync about the target `id`, when
    /// it is still waiting.
    #[must_use]
    pub fn asked(&self, id: &str) -> Option<&Request> {
        self.asked
            .requests
            .iter()
            .rev()
            .find(|r| r.target() == Some(id))
    }

    /// Every root the walk starts at: the trees and the files, every
    /// target's paths, the plain roots, the person's additions, what this
    /// machine follows and what the base names — none of them inside
    /// another, which walks it.
    #[must_use]
    pub fn roots(&self) -> BTreeSet<String> {
        let mut roots: BTreeSet<String> = rules()
            .map(|(p, _)| p.path.to_owned())
            .chain(
                targets()
                    .iter()
                    .flat_map(|t| t.paths.iter().map(|p| (*p).to_owned())),
            )
            .chain(plain_roots().iter().map(|r| r.path.to_owned()))
            .chain(self.prefs.extra.iter().cloned())
            .chain(self.elsewhere.iter().cloned())
            .chain(self.looked.iter().cloned())
            .filter(|r| is_logical(r))
            .collect();
        let all: Vec<String> = roots.iter().cloned().collect();
        roots.retain(|r| !all.iter().any(|o| o != r && covers(o, r)));
        roots
    }

    /// Meets every path under [`Scope::roots`] on this machine, in order:
    /// a folder is walked (links are never followed), a folder that cannot
    /// be listed is [`Met::Hidden`], everything else is handed over as
    /// `lstat` read it. A path inside another root is that root's
    /// (`home/.config/ronoctua` is `config`), and is met there once.
    pub fn meet(&self, places: &Places, f: &mut dyn FnMut(&str, &Path, Met<'_>)) {
        for root in self.roots() {
            let Some(at) = places.resolve(&root) else {
                continue;
            };
            visit(places, &root, &at, f, 0);
        }
    }

    /// Whether this machine keeps `path` sealed: the table, the person's
    /// choices and what a person added on any machine (`kept::kept_here`).
    #[must_use]
    pub fn kept_here(&self, path: &str) -> bool {
        kept::kept_here(
            path,
            &self.prefs,
            self.elsewhere.iter().any(|a| covers(a, path)),
        )
    }

    /// Where `path` comes from: the plain part, a target, else the deepest of
    /// the person's additions, what this machine follows and the rules that
    /// covers it (the person's first, at a tie). `None` for a path nothing
    /// names any more (a root of the base no rule covers).
    #[must_use]
    pub fn origin_of(&self, path: &str) -> Option<Origin> {
        if let Some(root) = kept::plain_root(path) {
            return Some(Origin::Plain(root.path));
        }
        if let Some(target) = kept::target_of(path) {
            return Some(Origin::Target(target.id));
        }
        let deepest = |list: &mut dyn Iterator<Item = &str>| {
            list.filter(|root| covers(root, path))
                .max_by_key(|root| root.len())
                .map(str::to_owned)
        };
        let added = deepest(&mut self.prefs.extra.iter().map(String::as_str));
        let elsewhere = deepest(&mut self.elsewhere.iter().map(String::as_str))
            .filter(|r| !self.prefs.extra.contains(r));
        let rule = rules()
            .map(|(p, _)| p.path)
            .filter(|p| covers(p, path))
            .max_by_key(|p| p.len());
        let candidates = [
            added.map(|a| (a.len(), 2, Origin::Added(a))),
            elsewhere.map(|e| (e.len(), 1, Origin::AddedElsewhere(e))),
            rule.map(|r| (r.len(), 0, Origin::Rule(r))),
        ];
        candidates
            .into_iter()
            .flatten()
            .max_by_key(|(len, rank, _)| (*len, *rank))
            .map(|(_, _, origin)| origin)
    }
}

/// One path of a walk: a folder walked, anything else met.
fn visit(
    places: &Places,
    logical: &str,
    at: &Path,
    f: &mut dyn FnMut(&str, &Path, Met<'_>),
    depth: usize,
) {
    // A PATH INSIDE ANOTHER ROOT is that root's: `home/.config/ronoctua` is
    // `config`, walked as itself.
    if places.logical_of(at).as_deref() != Some(logical) {
        return;
    }
    let Ok(meta) = std::fs::symlink_metadata(at) else {
        return;
    };
    if !meta.is_dir() {
        f(logical, at, Met::Entry(&meta));
        return;
    }
    if depth >= DEEPEST {
        return;
    }
    let Ok(entries) = std::fs::read_dir(at) else {
        // A FOLDER THAT CANNOT BE READ hides what is in it.
        f(logical, at, Met::Hidden);
        return;
    };
    let mut names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
    names.sort();
    for name in names {
        let Some(name) = name.to_str() else {
            continue;
        };
        let child = format!("{logical}/{name}");
        if !is_logical(&child) {
            continue;
        }
        visit(places, &child, &at.join(name), f, depth + 1);
    }
}

// ── anchor: L21 ──

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "ronoctua-belongings-scope-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// FAILS IF: a table's root, a target's path, a plain root, a person's
    /// addition or what this machine follows is not a root of the walk — or
    /// a root inside another is walked twice.
    #[test]
    fn every_kind_of_root_is_walked_once() {
        let prefs = Belongings {
            extra: vec!["home/notes".into(), "home/notes/deep".into()],
            ..Belongings::default()
        };
        let last = Last {
            repository: prefs.repository.clone(),
            roots: ["home/music".to_owned()].into_iter().collect(),
            ..Last::default()
        };
        // AN EMPTY ADDRESS IS NO BASE: what the last sync followed is not.
        let none = Scope::of(&prefs, Some(&last), &Requests::default());
        assert!(!none.roots().contains("home/music"));
        let prefs = Belongings {
            repository: "git@example.org:a/b.git".into(),
            ..prefs
        };
        let last = Last {
            repository: prefs.repository.clone(),
            ..last
        };
        let scope = Scope::of(&prefs, Some(&last), &Requests::default());
        let roots = scope.roots();
        for (place, _) in rules() {
            assert!(roots.contains(place.path), "{}", place.path);
        }
        for t in targets() {
            for p in t.paths {
                assert!(roots.iter().any(|r| covers(r, p)), "{p}");
            }
        }
        for r in plain_roots() {
            assert!(roots.iter().any(|o| covers(o, r.path)), "{}", r.path);
        }
        assert!(roots.contains("home/notes"));
        assert!(!roots.contains("home/notes/deep"), "a root inside another");
        assert!(roots.contains("home/music"), "what another machine added");
    }

    /// FAILS IF: a path's origin is not the most specific thing that names
    /// it — the plain part over a tree, a target over the home a person
    /// added, an addition under a tree over the tree.
    #[test]
    fn a_path_comes_from_the_most_specific_thing_that_names_it() {
        let prefs = Belongings {
            repository: "r".into(),
            extra: vec!["home".into(), "config/studio".into()],
            ..Belongings::default()
        };
        let last = Last {
            repository: "r".into(),
            roots: ["home/music".to_owned()].into_iter().collect(),
            ..Last::default()
        };
        let scope = Scope::of(&prefs, Some(&last), &Requests::default());
        assert_eq!(
            scope.origin_of("config/brain-modules/my-tea/module.rs"),
            Some(Origin::Plain("config/brain-modules"))
        );
        assert_eq!(
            scope.origin_of("home/.ssh/id_ed25519"),
            Some(Origin::Target("ssh"))
        );
        assert_eq!(
            scope.origin_of("config/studio/scenes.json"),
            Some(Origin::Added("config/studio".into()))
        );
        assert_eq!(
            scope.origin_of("config/theme.json"),
            Some(Origin::Rule("config"))
        );
        assert_eq!(
            scope.origin_of("home/music/a.ogg"),
            Some(Origin::AddedElsewhere("home/music".into()))
        );
        assert_eq!(
            scope.origin_of("home/notes/a.md"),
            Some(Origin::Added("home".into()))
        );
        let bare = Scope::of(&Belongings::default(), None, &Requests::default());
        assert_eq!(bare.origin_of("home/notes/a.md"), None);
    }

    /// FAILS IF: a request of "not on any machine" does not count as dropped
    /// until the sync settles it, or "on every machine again" does not undo a
    /// drop the last sync recorded.
    #[test]
    fn a_request_is_laid_over_what_the_index_dropped() {
        let prefs = Belongings::with_repository("git@example.org:a/b.git");
        let last = Last {
            repository: prefs.repository.clone(),
            dropped: ["gnupg".to_owned()].into_iter().collect(),
            ..Last::default()
        };
        let mut asked = Requests::default();
        assert!(asked.ask(Request::DropEverywhere {
            target: "ssh".into()
        }));
        assert!(asked.ask(Request::KeepEverywhere {
            target: "gnupg".into()
        }));
        let scope = Scope::of(&prefs, Some(&last), &asked);
        assert!(scope.dropped("ssh"));
        assert!(!scope.dropped("gnupg"));
        assert!(matches!(
            scope.asked("gnupg"),
            Some(Request::KeepEverywhere { .. })
        ));
        assert!(scope.asked("git").is_none());
    }

    /// FAILS IF: the walk follows a link, misses a file of a root, meets a
    /// path twice, or does not say a folder it could not list.
    #[test]
    fn the_walk_meets_every_path_once_and_follows_no_link() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = scratch("meet");
        let p = Places::under(&root);
        std::fs::create_dir_all(p.config.join("studio")).unwrap();
        std::fs::create_dir_all(p.home.join("elsewhere")).unwrap();
        std::fs::write(p.config.join("theme.json"), b"{}").unwrap();
        std::fs::write(p.home.join("elsewhere/x"), b"x").unwrap();
        std::os::unix::fs::symlink(p.home.join("elsewhere"), p.config.join("out")).unwrap();
        let hidden = p.config.join("locked");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o000)).unwrap();
        let scope = Scope::of(&Belongings::default(), None, &Requests::default());
        let mut met: Vec<(String, bool)> = Vec::new();
        scope.meet(&p, &mut |logical, _, m| {
            met.push((logical.to_owned(), matches!(m, Met::Hidden)));
        });
        let names: Vec<&str> = met.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"config/theme.json"), "{names:?}");
        assert!(names.contains(&"config/out"), "the link is met as itself");
        assert!(!names.contains(&"config/out/x"), "a link was followed");
        let unique: BTreeSet<&&str> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "a path met twice: {names:?}");
        if std::fs::read_dir(&hidden).is_err() {
            assert!(met.contains(&("config/locked".to_owned(), true)), "{met:?}");
        }
        std::fs::set_permissions(&hidden, std::fs::Permissions::from_mode(0o700)).unwrap();
        let _ = std::fs::remove_dir_all(root);
    }
}
