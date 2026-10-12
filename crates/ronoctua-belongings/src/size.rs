//! The size budget (feature `crypt`): what the repository weighs, measured
//! after a sync, for `Synced.budget` and the compaction the view offers ("no"
//! drawn first, never automatic).
//!
//! # Two weights
//!
//! The TREE is what the head holds — every chunk of every file kept now, the
//! index and the plain files; it is what a new machine downloads at least.
//! The HISTORY is what the clone holds after a fetch (`git count-objects`):
//! every version every machine ever pushed, which only a compaction takes
//! out. What they ask of the person is `kept::weight`'s arithmetic, over the
//! table's `budget`.

use std::path::Path;

use crate::status::Budget;

/// The bytes `git count-objects -v` says the objects weigh: the packs and the
/// loose objects (`size-pack` and `size`, in KiB). `None` when it says
/// neither. Pure.
#[must_use]
pub fn history_bytes(count_objects: &str) -> Option<u64> {
    let mut kib = None::<u64>;
    for line in count_objects.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        if matches!(key.trim(), "size" | "size-pack") {
            let n: u64 = value.trim().parse().ok()?;
            kib = Some(kib.unwrap_or(0) + n);
        }
    }
    kib.map(|k| k.saturating_mul(1024))
}

/// What the work tree at `root` holds, in bytes — every regular file but
/// git's own folder.
#[must_use]
pub fn tree_bytes(root: &Path) -> u64 {
    fn walk(dir: &Path, depth: usize) -> u64 {
        if depth > 16 {
            return 0;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut sum = 0u64;
        for entry in entries.flatten() {
            if entry.file_name() == ".git" {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                sum = sum.saturating_add(walk(&entry.path(), depth + 1));
            } else if meta.is_file() {
                sum = sum.saturating_add(meta.len());
            }
        }
        sum
    }
    walk(root, 0)
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

    #[test]
    fn the_history_is_the_packs_and_the_loose_objects() {
        let said = "count: 12\nsize: 48\nin-pack: 300\npacks: 1\nsize-pack: 2000\n\
                    prune-packable: 0\ngarbage: 0\nsize-garbage: 0\n";
        assert_eq!(history_bytes(said), Some(2048 * 1024));
        assert_eq!(history_bytes("nothing here"), None);
        assert_eq!(history_bytes("size: lots"), None);
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
        let root =
            std::env::temp_dir().join(format!("ronoctua-belongings-size-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".git/objects")).unwrap();
        std::fs::create_dir_all(root.join("chunks/ab")).unwrap();
        std::fs::write(root.join(".git/objects/x"), vec![0u8; 5000]).unwrap();
        std::fs::write(root.join("chunks/ab/c.age"), vec![0u8; 300]).unwrap();
        std::fs::write(root.join("index.age"), vec![0u8; 20]).unwrap();
        assert_eq!(tree_bytes(&root), 320);
        let _ = std::fs::remove_dir_all(root);
    }
}
