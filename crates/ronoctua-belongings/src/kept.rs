//! What is kept — the table `assets/belongings.nix` generates, and the one
//! arithmetic that reads it.
//!
//! The table is data (`RONOCTUA_BELONGINGS_RS`, copied in by `build.rs`); what
//! it MEANS is here, once, for the engine that walks the home, the door that
//! shows "What is kept", and the census test that compares every store's
//! declared verdict with what this arithmetic makes of it.
//!
//! # The rules
//!
//! A path is kept by default when it lies under one of the [`TREES`] — and no
//! [`NEVER`] name pattern matches a component below the tree's root — or is
//! one of the [`FILES`]. A [`NEVER`] path takes it (and what is under it)
//! out. On top of that come the person's own choices ([`kept_here`]): what
//! they added (links kept as links), what they took out — the most specific
//! of the two wins ([`excluded_here`]) — and what another machine added,
//! which travels in the index as a ROOT, so a machine keeps what any of the
//! person's machines chose to keep, new files in it included. A `hard` never
//! is never kept, whoever asks.
//!
//! Name patterns apply to the TREES only: a person who adds `home/.bashrc`
//! asked for a dotfile, and `.*` (the stores' drafts) is not about them.

use crate::places::covers;
use crate::prefs::Belongings;

/// A path kept, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// The logical path.
    pub path: &'static str,
    /// The reason's id (`belongings-reason-<id>`).
    pub reason: &'static str,
}

/// What a [`Never`] matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pattern {
    /// A component's name, with at most one `*` (`*.pid`, `.*`, `scroll-*.conf`).
    Name(&'static str),
    /// A logical path, and what is under it.
    Path(&'static str),
}

/// What is never kept, and why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Never {
    /// What it matches.
    pub pattern: Pattern,
    /// The reason's id.
    pub reason: &'static str,
    /// Not kept even when the person adds it.
    pub hard: bool,
}

/// One entry of the census of preference files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Census {
    /// The logical path of the file a `Store` writes.
    pub path: &'static str,
    /// Whether the rules keep it.
    pub kept: bool,
    /// Why.
    pub reason: &'static str,
}

mod table {
    use super::{Census, Never, Pattern, Place};
    include!(concat!(env!("OUT_DIR"), "/belongings.rs"));
}

pub use table::{FILES, LARGE_BYTES, NEVER, REASONS, STORES, TREES};

/// What the table makes of a path, before the person's choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Kept, and why.
    Kept(&'static str),
    /// Taken out by a [`NEVER`], and why.
    Never(&'static Never),
    /// Neither under a tree nor one of the files.
    Outside,
}

/// Whether a name matches a pattern with at most one `*`. Pure.
#[must_use]
pub fn name_matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((before, after)) => {
            name.len() >= before.len() + after.len()
                && name.starts_with(before)
                && name.ends_with(after)
        }
    }
}

/// The never that takes `path` out by its path, if one does.
fn never_by_path(path: &str) -> Option<&'static Never> {
    NEVER
        .iter()
        .find(|n| matches!(n.pattern, Pattern::Path(p) if covers(p, path)))
}

/// What the table makes of `path` (a logical path). Pure.
#[must_use]
pub fn by_default(path: &str) -> Verdict {
    if let Some(never) = never_by_path(path) {
        return Verdict::Never(never);
    }
    for tree in TREES {
        if !covers(tree.path, path) {
            continue;
        }
        let below = path[tree.path.len()..].trim_start_matches('/');
        for component in below.split('/').filter(|c| !c.is_empty()) {
            if let Some(never) = NEVER
                .iter()
                .find(|n| matches!(n.pattern, Pattern::Name(p) if name_matches(p, component)))
            {
                return Verdict::Never(never);
            }
        }
        return Verdict::Kept(tree.reason);
    }
    FILES
        .iter()
        .find(|f| f.path == path)
        .map_or(Verdict::Outside, |f| Verdict::Kept(f.reason))
}

/// Whether this machine keeps `path`: the table, the person's choices, and
/// `added_elsewhere` — a folder or file a person added on any of their
/// machines covers it (the index's roots say so). Pure.
#[must_use]
pub fn kept_here(path: &str, prefs: &Belongings, added_elsewhere: bool) -> bool {
    if never_by_path(path).is_some_and(|n| n.hard) {
        return false;
    }
    if excluded_here(path, prefs) {
        return false;
    }
    if added_here(path, prefs) || added_elsewhere {
        return true;
    }
    matches!(by_default(path), Verdict::Kept(_))
}

/// Whether the person took `path` out on this machine: of their choices that
/// cover it, the MOST SPECIFIC is a "taken out" — so `home/notes` added under
/// a `home` taken out is kept, and `config/studio` taken out under the
/// configuration kept is not. Pure.
#[must_use]
pub fn excluded_here(path: &str, prefs: &Belongings) -> bool {
    let deepest = |list: &[String]| {
        list.iter()
            .filter(|e| covers(e, path))
            .map(String::len)
            .max()
    };
    match (deepest(&prefs.excluded), deepest(&prefs.extra)) {
        (Some(out), Some(added)) => out >= added,
        (Some(_), None) => true,
        (None, _) => false,
    }
}

/// Whether the person added `path` on this machine (it or a folder above
/// it) — what the index marks as travelling to every machine. Pure.
#[must_use]
pub fn added_here(path: &str, prefs: &Belongings) -> bool {
    prefs.extra.iter().any(|e| covers(e, path))
}

/// Whether a person may add `path` at all: a logical path that no hard never
/// covers. Pure.
#[must_use]
pub fn may_add(path: &str) -> bool {
    crate::places::is_logical(path) && !never_by_path(path).is_some_and(|n| n.hard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_match_with_one_star() {
        assert!(name_matches("*.pid", "column-player.pid"));
        assert!(name_matches(".*", ".theme.json.123.tmp"));
        assert!(name_matches("scroll-*.conf", "scroll-theme.conf"));
        assert!(!name_matches("scroll-*.conf", "scroll.conf"));
        assert!(!name_matches("*.pid", "pid"));
        assert!(name_matches("exact", "exact"));
    }

    /// FAILS IF: a store's declared verdict in `assets/belongings.nix` is not
    /// what the rules make of its path — the census would say "kept" over a
    /// file the walk leaves out, or the other way round.
    #[test]
    fn every_store_is_where_the_census_says() {
        assert!(!STORES.is_empty());
        for store in STORES {
            let kept = matches!(by_default(store.path), Verdict::Kept(_));
            assert_eq!(
                kept,
                store.kept,
                "{} is declared {} and the rules make it {:?}",
                store.path,
                store.kept,
                by_default(store.path)
            );
            assert!(REASONS.contains(&store.reason));
        }
    }

    #[test]
    fn the_defaults_keep_the_preferences_and_never_the_access() {
        assert!(matches!(by_default("config/theme.json"), Verdict::Kept(_)));
        assert!(matches!(
            by_default("config/player-keys/tmdb"),
            Verdict::Kept(_)
        ));
        for never in [
            "config/column-player.pid",
            "config/.theme.json.42.tmp",
            "config/scroll-theme.conf",
            "config/belongings.json",
            "config/belongings-keys/deploy",
            "state/belongings/last.json",
            "home/.local/state/syncthing/cert.pem",
        ] {
            assert!(
                matches!(by_default(never), Verdict::Never(_)),
                "{never} is kept"
            );
        }
        assert_eq!(by_default("home/notes/a.md"), Verdict::Outside);
        assert!(matches!(
            by_default("state/software/history.jsonl"),
            Verdict::Kept("programs")
        ));
    }

    #[test]
    fn the_persons_choices_go_over_the_table_but_never_over_a_hard_never() {
        let mut prefs = Belongings::default();
        assert!(!kept_here("home/notes/a.md", &prefs, false));
        assert!(
            kept_here("home/notes/a.md", &prefs, true),
            "added elsewhere"
        );
        prefs.extra.push("home/notes".to_owned());
        assert!(kept_here("home/notes/a.md", &prefs, false));
        assert!(added_here("home/notes/a.md", &prefs));
        // A dotfile the person added is kept: name patterns are the trees'.
        prefs.extra.push("home/.bashrc".to_owned());
        assert!(kept_here("home/.bashrc", &prefs, false));
        // Syncthing's identity is out by default, and in when asked for.
        assert!(!kept_here(
            "home/.local/state/syncthing/cert.pem",
            &prefs,
            false
        ));
        prefs.extra.push("home/.local/state/syncthing".to_owned());
        assert!(kept_here(
            "home/.local/state/syncthing/cert.pem",
            &prefs,
            false
        ));
        // The deploy key never, whoever asks.
        prefs.extra.push("config/belongings-keys".to_owned());
        assert!(!kept_here("config/belongings-keys/deploy", &prefs, true));
        assert!(!may_add("config/belongings-keys"));
        // Taken out beats everything the person did not mark hard.
        prefs.excluded.push("config/theme.json".to_owned());
        assert!(!kept_here("config/theme.json", &prefs, true));
        // THE MOST SPECIFIC CHOICE WINS: a folder kept under one taken out is
        // kept, and one taken out under one kept is not.
        let mut prefs = Belongings::default();
        prefs.excluded.push("home".to_owned());
        prefs.extra.push("home/notes".to_owned());
        assert!(kept_here("home/notes/a.md", &prefs, false));
        assert!(!kept_here("home/music/b.ogg", &prefs, true));
        prefs.excluded.push("home/notes/private".to_owned());
        assert!(!kept_here("home/notes/private/c.md", &prefs, true));
        assert!(excluded_here("home/notes/private", &prefs));
        assert!(!excluded_here("home/notes", &prefs));
    }
}
