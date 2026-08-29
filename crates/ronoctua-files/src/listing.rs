//! Reading a directory, and the order it comes out in.
//!
//! # Hidden files are SHOWN, and that is the owner's rule
//!
//! Unix's only definition of hidden is "the name starts with a dot", and it
//! exists because an early `ls` filtered `.` and `..` by prefix. Everything
//! built on top of that accident is convention. Here the whole directory is
//! shown and `.` toggles it — a file manager that hides half of `$HOME` by
//! default is a file manager one cannot use to fix a dotfile.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::{Entry, Kind};

/// What the rows are sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    /// By name, accents folded. The default, and what a person navigates by.
    #[default]
    Name,
    /// By size, largest first when ascending is false.
    Size,
    /// By last modification.
    Modified,
    /// By what the thing is, then by name inside each kind.
    Kind,
}

impl Order {
    /// The orders, in the sequence the `s` key steps through them.
    pub const ALL: [Self; 4] = [Self::Name, Self::Size, Self::Modified, Self::Kind];

    /// The text key for this order's name.
    #[must_use]
    pub const fn label(self) -> &'static str {
        use ronoctua_i18n::key;
        match self {
            Self::Name => key::FILES_ORDER_NAME,
            Self::Size => key::FILES_ORDER_SIZE,
            Self::Modified => key::FILES_ORDER_MODIFIED,
            Self::Kind => key::FILES_ORDER_KIND,
        }
    }

    /// The next one, wrapping.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Name => Self::Size,
            Self::Size => Self::Modified,
            Self::Modified => Self::Kind,
            Self::Kind => Self::Name,
        }
    }
}

/// How a listing is arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort {
    /// The key.
    pub by: Order,
    /// Ascending — A to Z, small to large, old to new.
    pub ascending: bool,
    /// Directories before files, whatever the key.
    ///
    /// On by default, and it is not decoration: a directory is a PLACE and a
    /// file is a thing. Interleaving them turns "where do I go from here" into
    /// a hunt through a hundred rows, and going somewhere is the most common
    /// thing one does in a listing.
    pub directories_first: bool,
}

impl Default for Sort {
    fn default() -> Self {
        Self {
            by: Order::Name,
            ascending: true,
            directories_first: true,
        }
    }
}

/// Every entry of a directory, sorted.
///
/// # Errors
///
/// When the directory cannot be opened — it does not exist, or it is not
/// readable. A single entry that fails to `lstat` is DROPPED rather than
/// failing the whole listing: a directory with one unreadable inode is still a
/// directory one wants to look at, and the alternative is a screen that says
/// nothing at all.
pub fn read_dir_at(dir: &Path, sort: Sort) -> Result<Vec<Entry>> {
    let handle =
        std::fs::read_dir(dir).with_context(|| format!("could not list {}", dir.display()))?;
    let mut out: Vec<Entry> = handle
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            Entry::of_path(name, &e.path())
        })
        .collect();
    sort_entries(&mut out, sort);
    Ok(out)
}

/// The same order `read_dir_at` applies, on a list one already has.
///
/// Separate so that changing the sort key does not cost a second `readdir` —
/// and so the whole ordering can be tested without a filesystem.
pub fn sort_entries(entries: &mut [Entry], sort: Sort) {
    entries.sort_by(|a, b| {
        if sort.directories_first {
            let place = is_place(b).cmp(&is_place(a));
            if place != std::cmp::Ordering::Equal {
                return place;
            }
        }
        let ordering = match sort.by {
            Order::Name => folded(a).cmp(&folded(b)),
            Order::Size => a.bytes.cmp(&b.bytes),
            Order::Modified => a.modified.cmp(&b.modified),
            // Kind first, then name INSIDE the kind: a column of twelve
            // symlinks in random order answers "what is this" and not "which
            // one".
            Order::Kind => a.kind.cmp(&b.kind).then_with(|| folded(a).cmp(&folded(b))),
        };
        if sort.ascending {
            ordering
        } else {
            ordering.reverse()
        }
        // THE TIE IS BROKEN BY NAME, always. Without it, two files of the same
        // size swap places between two readings of the same directory — the
        // cursor lands on a different row than the one that was looked at.
        .then_with(|| folded(a).cmp(&folded(b)))
    });
}

/// A symlink that points at a directory counts as a place.
///
/// Following the link here and nowhere else is deliberate: this decides where
/// the row is DRAWN, and a link into a directory is used to go there.
fn is_place(e: &Entry) -> bool {
    matches!(e.opens_as(), Kind::Directory)
}

/// The name as sorting reads it: accents off, case down.
///
/// `ronoctua_apps::fold` and not `to_lowercase`, for the reason that crate
/// gives: without it `Ágil` sorts after `Zebra`, which is where nobody looks.
fn folded(e: &Entry) -> String {
    ronoctua_apps::fold::fold(&e.name)
}

/// The directory's own modification time, for "has this changed since I looked".
///
/// The same shape as `ronoctua_shell::apps::stamp`: one `stat` answers whether
/// a re-read is worth it, and a re-read that finds nothing new is a frame spent
/// for nothing.
#[must_use]
pub fn stamp(dir: &Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(dir).ok()?.modified().ok()
}

/// The path broken into the pieces a breadcrumb offers.
///
/// Returns `(label, path)` from the root to the directory itself. It is what
/// makes any ancestor ONE gesture away — with only a "go up" key, reaching the
/// home directory from six levels down is six presses, and the mouse has
/// nothing at all to aim at.
#[must_use]
pub fn ancestry(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    let mut here = PathBuf::new();
    for part in dir.components() {
        here.push(part.as_os_str());
        let label = part.as_os_str().to_string_lossy().into_owned();
        out.push((label, here.clone()));
    }
    out
}

/// Where the manager opens when nobody says where.
///
/// `preferred` is a directory NAME relative to home, not a path: the option
/// that carries it is a machine's preference, and a machine that does not have
/// that directory should land in its home rather than on an error. Falls back
/// to `/` only when there is no home at all, which is the state a rescue shell
/// boots into.
#[must_use]
pub fn start_at(home: Option<&Path>, preferred: Option<&str>) -> PathBuf {
    let Some(home) = home else {
        return PathBuf::from("/");
    };
    if let Some(name) = preferred {
        let wanted = home.join(name);
        if wanted.is_dir() {
            return wanted;
        }
    }
    home.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, kind: Kind, bytes: u64, modified: i64) -> Entry {
        Entry {
            name: name.to_owned(),
            kind,
            mode: if matches!(kind, Kind::Directory) {
                0o040_755
            } else {
                0o100_644
            },
            links: 1,
            uid: 1000,
            gid: 100,
            bytes,
            modified,
            target: None,
            aims_at: None,
        }
    }

    #[test]
    fn directories_come_first_and_names_fold() {
        // THE DEFECT THIS CATCHES: `Ágil` sorted after `Zebra`, because a plain
        // byte comparison puts every accent above every ASCII letter. It is the
        // same defect `ronoctua_apps` already paid for once.
        let mut list = vec![
            entry("Zebra", Kind::File, 1, 0),
            entry("Ágil", Kind::File, 1, 0),
            entry("beta", Kind::Directory, 1, 0),
            entry("alfa", Kind::File, 1, 0),
        ];
        sort_entries(&mut list, Sort::default());
        let names: Vec<_> = list.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["beta", "Ágil", "alfa", "Zebra"]);
    }

    #[test]
    fn a_link_into_a_directory_is_a_place() {
        // A symlink to a directory is what one clicks to GO somewhere. Drawn
        // among the files it is unreachable by the gesture it exists for.
        let mut link = entry("atalho", Kind::Symlink, 12, 0);
        link.aims_at = Some(Kind::Directory);
        let mut list = vec![entry("file", Kind::File, 1, 0), link];
        sort_entries(&mut list, Sort::default());
        assert_eq!(list[0].name, "atalho");
    }

    #[test]
    fn equal_keys_do_not_shuffle_between_readings() {
        // THE DEFECT THIS CATCHES: two files of the same size trading places
        // between two readings of the same directory. Nothing on screen would
        // say why, and the cursor would land on a row nobody chose.
        let mut a = vec![
            entry("b", Kind::File, 10, 0),
            entry("a", Kind::File, 10, 0),
            entry("c", Kind::File, 10, 0),
        ];
        let sort = Sort {
            by: Order::Size,
            ascending: true,
            directories_first: false,
        };
        sort_entries(&mut a, sort);
        let mut b = vec![
            entry("c", Kind::File, 10, 0),
            entry("b", Kind::File, 10, 0),
            entry("a", Kind::File, 10, 0),
        ];
        sort_entries(&mut b, sort);
        let names_a: Vec<_> = a.iter().map(|e| e.name.as_str()).collect();
        let names_b: Vec<_> = b.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names_a, names_b);
        assert_eq!(names_a, ["a", "b", "c"]);
    }

    #[test]
    fn descending_reverses_the_key_and_not_the_tie() {
        let mut list = vec![
            entry("a", Kind::File, 1, 0),
            entry("b", Kind::File, 9, 0),
            entry("c", Kind::File, 9, 0),
        ];
        sort_entries(
            &mut list,
            Sort {
                by: Order::Size,
                ascending: false,
                directories_first: false,
            },
        );
        let names: Vec<_> = list.iter().map(|e| e.name.as_str()).collect();
        // The big ones first, and among the big ones still `b` before `c`.
        assert_eq!(names, ["b", "c", "a"]);
    }

    #[test]
    fn the_breadcrumb_names_every_ancestor() {
        let parts = ancestry(Path::new("/home/pessoa/main"));
        let labels: Vec<_> = parts.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["/", "home", "pessoa", "main"]);
        assert_eq!(parts[2].1, PathBuf::from("/home/pessoa"));
    }

    #[test]
    fn the_start_falls_back_instead_of_failing() {
        let home = std::env::temp_dir();
        // A name that is not there lands in the home, never on an error.
        assert_eq!(start_at(Some(&home), Some("does-not-exist-anywhere")), home);
        assert_eq!(start_at(None, Some("main")), PathBuf::from("/"));
    }

    #[test]
    fn the_order_cycles_through_all_of_them() {
        // THE DEFECT THIS CATCHES: an order added to `ALL` and forgotten in
        // `next`, which makes the `s` key unable to reach it — a sort that
        // exists, is translated, and is unreachable.
        let mut seen = Vec::new();
        let mut o = Order::Name;
        for _ in 0..Order::ALL.len() {
            seen.push(o);
            o = o.next();
        }
        assert_eq!(o, Order::Name, "the cycle did not close");
        for wanted in Order::ALL {
            assert!(seen.contains(&wanted), "the key never reaches {wanted:?}");
        }
    }
}
