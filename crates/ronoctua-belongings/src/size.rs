//! The size budget (feature `crypt`): what the repository weighs, measured
//! after a sync, for `Synced.budget` and the compaction the view offers ("no"
//! drawn first, never automatic).
//!
//! # Two weights
//!
//! The TREE is what the head holds — every chunk of every file kept now, the
//! index and the plain files; it is what a new machine downloads at least.
//! The HISTORY is what the clone holds after a fetch (its `objects` folder):
//! every version every machine ever pushed, which only a compaction takes
//! out. What they ask of the person is `kept::weight`'s arithmetic, over the
//! table's `budget`.
//!
//! # Lengths, never blocks
//!
//! Both are the LENGTHS of the files, what was written. `git count-objects`
//! weighs a loose object by the blocks it holds (`st_blocks`), and a
//! filesystem that allocates late answers a few blocks for an object written
//! a moment ago: ZFS says 1 KiB for 2 MiB until its transaction group closes,
//! seconds later, and no `fsync`, `syncfs` or `sync` hurries it. A fetch
//! weighed a quarter of itself on one machine and all of it on another — and
//! the compaction's tests failed on the first. A length is the same on every
//! filesystem, at once.

use std::path::{Path, PathBuf};

use ronoctua_git::Repo;

use crate::status::Budget;

/// The clone's `objects` folder, as git names it (`rev-parse --git-path`),
/// or `None` when git could not say.
#[must_use]
pub fn objects_of(repo: &Repo) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    let said = repo
        .output(
            None,
            "rev-parse",
            &["--path-format=absolute", "--git-path", "objects"],
            ronoctua_git::LISTING_MOST_BYTES,
        )
        .ok()?;
    let path = said.strip_suffix(b"\n").unwrap_or(&said);
    (!path.is_empty()).then(|| PathBuf::from(std::ffi::OsStr::from_bytes(path)))
}

/// What the history in the `objects` folder weighs, in bytes — the packs,
/// their indexes and the loose objects, by their lengths.
#[must_use]
pub fn history_bytes(objects: &Path) -> u64 {
    bytes_under(objects, |_| false)
}

/// What the work tree at `root` holds, in bytes — every regular file but
/// git's own folder.
#[must_use]
pub fn tree_bytes(root: &Path) -> u64 {
    bytes_under(root, |name| name == ".git")
}

/// The lengths of every regular file below `root`, never following a link,
/// less the entries `skipped` names.
fn bytes_under(root: &Path, skipped: impl Fn(&std::ffi::OsStr) -> bool + Copy) -> u64 {
    fn walk(dir: &Path, depth: usize, skipped: impl Fn(&std::ffi::OsStr) -> bool + Copy) -> u64 {
        if depth > 16 {
            return 0;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut sum = 0u64;
        for entry in entries.flatten() {
            if skipped(&entry.file_name()) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                sum = sum.saturating_add(walk(&entry.path(), depth + 1, skipped));
            } else if meta.is_file() {
                sum = sum.saturating_add(meta.len());
            }
        }
        sum
    }
    walk(root, 0, skipped)
}

/// The budget of a measured repository: its two weights, and whether a
/// compaction would fold any of its history.
#[must_use]
pub const fn budget(tree_bytes: u64, history_bytes: u64, foldable: bool) -> Budget {
    Budget {
        tree_bytes,
        history_bytes,
        recommend_bytes: crate::kept::RECOMMEND_BYTES,
        foldable,
    }
}

// ── anchor: L08 ──

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kept::{Weight, weight};

    fn room(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-belongings-size-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// FAILS IF: the history is weighed by anything but the lengths of what
    /// the `objects` folder holds — a pack, its index and the loose objects —
    /// or follows a link out of it.
    #[test]
    fn the_history_is_the_packs_and_the_loose_objects() {
        let root = room("history");
        let objects = root.join("objects");
        std::fs::create_dir_all(objects.join("pack")).unwrap();
        std::fs::create_dir_all(objects.join("ab")).unwrap();
        std::fs::write(objects.join("pack/pack-1.pack"), vec![0u8; 2000]).unwrap();
        std::fs::write(objects.join("pack/pack-1.idx"), vec![0u8; 100]).unwrap();
        std::fs::write(objects.join("ab/cdef"), vec![0u8; 48]).unwrap();
        std::fs::write(root.join("elsewhere"), vec![0u8; 7000]).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), objects.join("ab/link")).unwrap();
        assert_eq!(history_bytes(&objects), 2148);
        assert_eq!(history_bytes(&root.join("absent")), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: the view would recommend compacting a repository whose
    /// history is light, close to its tree, or that a compaction could not
    /// fold — a small repository synced every day passes twice its tree
    /// within days, all of it younger than `keepDays` — or keep quiet about
    /// one past the line.
    #[test]
    fn the_weight_follows_the_table() {
        let mb = 1_000_000;
        let over = crate::kept::HISTORY_OVER_TREE;
        assert_eq!(weight(100 * mb, 120 * mb, true), Weight::Light);
        assert_eq!(
            weight(100 * mb, over * 100 * mb, true),
            Weight::Light,
            "too many times a small tree, but under the warning line"
        );
        assert_eq!(
            weight(300_000, mb, true),
            Weight::Light,
            "a few hundred kilobytes of configuration, synced daily"
        );
        assert_eq!(
            weight(400 * mb, crate::kept::WARN_BYTES.max(over * 400 * mb), true),
            Weight::Compact,
            "past the warning line and too many times the tree"
        );
        assert_eq!(
            weight(600 * mb, crate::kept::WARN_BYTES, true),
            Weight::Heavy
        );
        assert_eq!(
            weight(900 * mb, crate::kept::RECOMMEND_BYTES, true),
            Weight::Compact
        );
        assert_eq!(
            weight(900 * mb, crate::kept::RECOMMEND_BYTES, false),
            Weight::Heavy,
            "nothing a compaction could fold: said, never recommended"
        );
        assert_eq!(weight(0, 0, false), Weight::Light);
    }

    #[test]
    fn the_tree_is_every_file_but_gits_own() {
        let root = room("tree");
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::create_dir_all(root.join("chunks/ab")).unwrap();
        std::fs::write(root.join(".git/objects/x"), vec![0u8; 5000]).unwrap();
        std::fs::write(root.join("chunks/ab/c.age"), vec![0u8; 300]).unwrap();
        std::fs::write(root.join("index.age"), vec![0u8; 20]).unwrap();
        assert_eq!(tree_bytes(&root), 320);
        let _ = std::fs::remove_dir_all(root);
    }
}
