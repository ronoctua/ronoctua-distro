//! Walks a cache root and deletes what nothing has used for a while.
//!
//! # Why this exists
//!
//! Because nothing swept anything. The owner asked how this distro's caches
//! worked and the honest answer was that neither of the two on disk was ever
//! cleaned — no timer, no tmpfiles rule, no walk. His rule came back in one
//! sentence: nothing cached may stay for ever.
//!
//! Measured on his machine the day this was written: `$XDG_CACHE_HOME` held
//! 254 GB of cargo artefacts, 36 GB of a package manager's downloads and 2 GB
//! of a browser's — against 4,6 MB of our own thumbnails. **The thumbnails were
//! never the problem**, and that is why the reach is the whole root and not a
//! directory of ours.
//!
//! # What it will not do, and why each one is here
//!
//! This binary deletes files for a living, so the refusals are the design:
//!
//! - **It never follows a symbolic link.** Every entry is looked at with
//!   [`std::fs::symlink_metadata`], never `metadata`. A `~/.cache/x -> /home`
//!   followed by a sweep is the worst thing this code could possibly do.
//! - **It never leaves the filesystem it started on.** A mount under the cache
//!   root is somebody else's disk with somebody else's rules.
//! - **It deletes regular files only.** A socket, a fifo or a device under
//!   there belongs to a program that is running now.
//! - **A timestamp in the FUTURE keeps the file.** The clock may have been
//!   wrong when it was written, and throwing away what one cannot date is worse
//!   than keeping it — the rule `ronoctua_notify`'s own prune already follows.
//! - **It rehearses unless told otherwise.** [`Act::Rehearse`] counts exactly
//!   what [`Act::Execute`] would take and removes nothing, which is the shape
//!   `nix run .#install` already has.
//!
//! # And the machine's load is NOT this crate's question
//!
//! It is answered before this is ever started, by `ronoctua_brain::runner::judge`
//! against a gate the person configured in one place. A second opinion here
//! would be a second policy that drifts from the first — the same refusal
//! `tasks/update.rs` writes out about itself.

use std::io;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Whether the walk is a rehearsal or the real thing.
///
/// The default is the harmless one on purpose: a caller that forgets to say
/// counts instead of deleting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Act {
    /// Count what would go, and take nothing.
    #[default]
    Rehearse,
    /// Take it.
    Execute,
}

/// What one walk found and did.
///
/// `bytes` is what the FILES weighed, not what the filesystem gave back:
/// on btrfs with compression or a reflink those two differ, and the honest
/// number to report is the one that was measured rather than one guessed at
/// from a `statvfs` before and after — which another process writing at the
/// same time would make a lie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Swept {
    /// Regular files taken, or that would have been.
    pub files: u64,
    /// What they weighed.
    pub bytes: u64,
    /// Regular files looked at and left, because they are still in use.
    pub kept: u64,
    /// Directories removed because THIS walk emptied them.
    ///
    /// Never one that was already empty when the walk arrived: that one belongs
    /// to a program that made it and has not written into it yet, and taking it
    /// would be this sweep doing something nobody asked for. Zero in a
    /// rehearsal, which counts files and touches nothing.
    pub dirs: u64,
    /// Entries the walk could not read or could not remove.
    ///
    /// A count and not a failure: a cache directory being written by the
    /// program that owns it while this walks is ordinary, and a sweep that gave
    /// up on the first `EACCES` would be a sweep that never finished once.
    pub refused: u64,
}

/// How long since a file was last USED, by the later of its two timestamps.
///
/// # Why the later of the two, when the rule says "last use"
///
/// Because `atime` alone is a trap on a filesystem mounted `noatime`, where it
/// is stamped once at creation and never moved again. There, a file WRITTEN
/// yesterday still carries an access time from the day it was made — so a rule
/// reading `atime` alone would delete this morning's work and call it a month
/// old. Taking the later of the two is what makes "last use" mean the same
/// thing on a machine that keeps access times and on one that does not.
///
/// Measured where this runs: btrfs with `relatime`, and access times move — an
/// entry written at 16:24 read back at 21:52 carries both.
///
/// `None` when the file's stamp is in the FUTURE, which keeps it.
#[must_use]
pub fn unused_for(meta: &std::fs::Metadata, now: SystemTime) -> Option<Duration> {
    let used = match (meta.accessed(), meta.modified()) {
        (Ok(a), Ok(m)) => a.max(m),
        (Ok(t), Err(_)) | (Err(_), Ok(t)) => t,
        // NEITHER STAMP READABLE IS NOT "INFINITELY OLD". A filesystem that
        // answers neither is one this has no business deciding about.
        (Err(_), Err(_)) => return None,
    };
    now.duration_since(used).ok()
}

/// Walk `root` and take every regular file unused for `keep` or longer.
///
/// `now` is a PARAMETER and not a clock: a sweep whose age arithmetic reads the
/// system clock cannot be tested against a file planted two days in the future,
/// and that file is the case this most needs to get right.
///
/// The root itself is never removed, and neither are its immediate children:
/// `fontconfig`, `starship` and a dozen others expect their directory to be
/// there, and recreating it is the login's business, not a sweep's. Everything
/// deeper that ends up empty goes.
pub fn sweep(root: &Path, now: SystemTime, keep: Duration, act: Act) -> Swept {
    let mut out = Swept::default();
    let Ok(top) = std::fs::symlink_metadata(root) else {
        // NO ROOT IS NOT A FAILURE: a machine that has never cached anything
        // has nothing to sweep, and saying so with an error would make the
        // first boot look broken.
        return out;
    };
    if !top.is_dir() {
        out.refused += 1;
        return out;
    }
    let disk = top.dev();

    // AN EXPLICIT STACK AND NOT RECURSION. A cache root is somebody else's
    // directory tree and its depth is not ours to bound; a deep one would be a
    // stack overflow, which in this process is a sweep that half ran.
    //
    // The `bool` is "have its children been walked yet": a directory has to be
    // visited twice — once to queue what is in it, and once afterwards to find
    // out whether anything is left. The depth rides along because the root's
    // own children are kept whatever happens.
    // AND WHICH DIRECTORIES THIS WALK TOOK SOMETHING OUT OF. A directory that
    // was ALREADY empty when the walk arrived is left where it is: it belongs
    // to a program that made it and has not written into it yet, and removing
    // it would be this sweep doing something nobody asked for.
    let mut emptied: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    let mut stack: Vec<(PathBuf, usize, bool)> = vec![(root.to_path_buf(), 0, false)];
    while let Some((at, depth, walked)) = stack.pop() {
        if walked {
            // COMING BACK UP. Everything below has been dealt with, so a
            // directory that is empty AND that this walk took something out of
            // is one this walk emptied.
            if depth > 1 && emptied.contains(&at) && is_empty(&at) {
                match std::fs::remove_dir(&at) {
                    Ok(()) => {
                        out.dirs += 1;
                        if let Some(up) = at.parent() {
                            emptied.insert(up.to_path_buf());
                        }
                    }
                    // A directory something wrote into between the two visits
                    // is not empty any more, and that is not a failure.
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(_) => out.refused += 1,
                }
            }
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&at) else {
            out.refused += 1;
            continue;
        };
        stack.push((at.clone(), depth, true));
        for entry in entries {
            let Ok(entry) = entry else {
                out.refused += 1;
                continue;
            };
            let path = entry.path();
            // SYMLINK METADATA, ALWAYS. `metadata` here would follow a link out
            // of the cache root and make every rule below a rule about
            // somebody else's files.
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                out.refused += 1;
                continue;
            };
            if meta.is_dir() {
                // AND NEVER OFF THIS DISK. A mount under the cache root is
                // another filesystem with another owner's rules on it.
                if meta.dev() == disk {
                    stack.push((path, depth + 1, false));
                }
                continue;
            }
            // A LINK, A SOCKET, A FIFO, A DEVICE: left alone. The first is what
            // this must not follow and the rest belong to something that is
            // running right now.
            if !meta.is_file() {
                continue;
            }
            match unused_for(&meta, now) {
                Some(age) if age >= keep => {
                    if act == Act::Execute {
                        match std::fs::remove_file(&path) {
                            Ok(()) => {
                                emptied.insert(at.clone());
                            }
                            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                            Err(_) => {
                                out.refused += 1;
                                continue;
                            }
                        }
                    }
                    out.files += 1;
                    out.bytes += meta.len();
                }
                // STILL IN USE, or stamped in the future — which keeps it.
                _ => out.kept += 1,
            }
        }
    }
    out
}

/// Whether a directory has nothing left in it.
///
/// An unreadable directory answers NO, which is what keeps a sweep from
/// removing something it could not look inside.
fn is_empty(at: &Path) -> bool {
    std::fs::read_dir(at).is_ok_and(|mut d| d.next().is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tree of our own, under `/tmp`, that goes away with the test.
    ///
    /// Written rather than pulled in, for the reason `ronoctua-memory`'s own
    /// `mod tempdir` gives: a crate whose whole argument is that it links
    /// almost nothing does not grow a dependency to make a directory.
    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let at = std::env::temp_dir().join(format!(
                "ronoctua-sweep-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = std::fs::remove_dir_all(&at);
            std::fs::create_dir_all(&at).expect("a scratch root");
            Self(at)
        }

        fn at(&self, rel: &str) -> PathBuf {
            self.0.join(rel)
        }

        /// A file with both stamps set to `age` ago.
        fn plant(&self, rel: &str, age: Duration) -> PathBuf {
            self.plant_with(rel, age, age)
        }

        /// A file whose two stamps DISAGREE — which is the whole of the
        /// `noatime` case.
        fn plant_with(&self, rel: &str, read: Duration, written: Duration) -> PathBuf {
            let at = self.at(rel);
            if let Some(up) = at.parent() {
                std::fs::create_dir_all(up).expect("the parent");
            }
            std::fs::write(&at, b"some cached bytes").expect("plant a file");
            let file = std::fs::File::options()
                .write(true)
                .open(&at)
                .expect("open to stamp");
            file.set_times(
                std::fs::FileTimes::new()
                    .set_accessed(SystemTime::now() - read)
                    .set_modified(SystemTime::now() - written),
            )
            .expect("stamp it");
            at
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn days(n: u64) -> Duration {
        Duration::from_secs(n * SECONDS_IN_A_DAY)
    }

    const SECONDS_IN_A_DAY: u64 = 24 * 60 * 60;

    #[test]
    fn a_sandbox_with_nothing_in_it_proves_nothing_so_this_plants() {
        // THE DEFECT THIS CATCHES IS IN THE TEST ITSELF, and this repository
        // has met it: `dev-exits-clean` ran in a sandbox whose `/tmp` was
        // empty, every loop of the prune returned 0 by not running, the rule
        // went in green and took the session down on the first real machine.
        //
        // So every case below plants, and every one of them asserts what was
        // NOT taken as well as what was.
        let tree = Tree::new("plantada");
        tree.plant("deep/old.bin", days(40));
        tree.plant("deep/new.bin", days(1));
        let took = sweep(&tree.0, SystemTime::now(), days(20), Act::Execute);
        assert_eq!(took.files, 1, "it did not take the old one");
        assert_eq!(took.kept, 1, "it did not leave the new one");
        assert!(!tree.at("deep/old.bin").exists());
        assert!(tree.at("deep/new.bin").exists(), "today's work went");
    }

    #[test]
    fn a_link_out_of_the_root_is_neither_followed_nor_taken() {
        // THE WORST THING THIS CODE COULD DO. A `~/.cache/x -> /home` walked by
        // a sweep is somebody's home directory deleted by age. Every entry is
        // looked at with `symlink_metadata`, so a link is never a directory and
        // never a file here — it is skipped, and what it points at is never
        // opened.
        let tree = Tree::new("link");
        let outside = Tree::new("fora");
        let precious = outside.plant("do-not-touch.txt", days(400));
        std::os::unix::fs::symlink(&outside.0, tree.at("escape")).expect("a link out");
        // AND A LINK TO A FILE, which is the other half: it is not a regular
        // file, so no rule below reaches it however old it looks.
        let old = tree.plant("real-old.bin", days(40));
        std::os::unix::fs::symlink(&old, tree.at("pointer")).expect("a link to a file");

        let took = sweep(&tree.0, SystemTime::now(), days(20), Act::Execute);

        assert!(
            precious.exists(),
            "the sweep followed a link out of its root"
        );
        assert!(outside.0.exists(), "it took the directory the link led to");
        assert!(
            tree.at("pointer").symlink_metadata().is_ok(),
            "it took a symbolic link, which is not a regular file"
        );
        assert_eq!(took.files, 1, "it took something other than the one file");
        assert!(
            !old.exists(),
            "it did not take the file it was there to take"
        );
    }

    #[test]
    fn a_stamp_in_the_future_keeps_the_file() {
        // The clock may have been wrong when it was written, and throwing away
        // what one cannot date is worse than keeping it — the rule
        // `ronoctua_notify::store::prune` already follows about a record from
        // the future.
        let tree = Tree::new("futuro");
        let at = tree.at("tomorrow.bin");
        std::fs::write(&at, b"x").unwrap();
        let file = std::fs::File::options().write(true).open(&at).unwrap();
        let ahead = SystemTime::now() + days(2);
        file.set_times(
            std::fs::FileTimes::new()
                .set_accessed(ahead)
                .set_modified(ahead),
        )
        .unwrap();

        let took = sweep(&tree.0, SystemTime::now(), days(20), Act::Execute);

        assert_eq!(took.files, 0, "a file from the future was taken");
        assert_eq!(took.kept, 1);
        assert!(at.exists());
    }

    #[test]
    fn a_file_written_today_survives_a_filesystem_with_no_access_times() {
        // THE DEFECT THIS CATCHES: reading `atime` alone. On a filesystem
        // mounted `noatime` it is stamped once at creation and never moves
        // again, so this morning's work still carries an access time from the
        // day the file was made — and a rule that read only that would delete
        // it and call it a month old.
        //
        // The later of the two is what makes "last use" mean the same thing on
        // a machine that keeps access times and on one that does not.
        let tree = Tree::new("noatime");
        let fresh = tree.plant_with("written-today.bin", days(40), days(0));
        let stale = tree.plant_with("really-old.bin", days(40), days(40));

        let took = sweep(&tree.0, SystemTime::now(), days(20), Act::Execute);

        assert!(
            fresh.exists(),
            "a file written today was taken for an access time that never moves"
        );
        assert!(!stale.exists(), "the one nothing has touched stayed");
        assert_eq!(took.files, 1);
        assert_eq!(took.kept, 1);
    }

    #[test]
    fn a_rehearsal_counts_exactly_what_it_would_take_and_takes_nothing() {
        // The shape `nix run .#install` already has, and the reason is that
        // this is the only thing in the tree that deletes a person's files: the
        // number has to be checkable before anybody believes it.
        let tree = Tree::new("ensaio");
        tree.plant("a/old-1.bin", days(30));
        tree.plant("a/old-2.bin", days(30));
        tree.plant("a/new.bin", days(2));

        let said = sweep(&tree.0, SystemTime::now(), days(20), Act::Rehearse);
        assert_eq!(said.files, 2);
        assert!(said.bytes > 0);
        assert!(
            tree.at("a/old-1.bin").exists(),
            "a rehearsal deleted a file"
        );

        let did = sweep(&tree.0, SystemTime::now(), days(20), Act::Execute);
        assert_eq!(
            (did.files, did.bytes),
            (said.files, said.bytes),
            "the rehearsal's number and the real one disagree"
        );
    }

    #[test]
    fn the_roots_own_children_stay_and_what_the_walk_empties_goes() {
        // `fontconfig`, `starship` and a dozen others expect their directory to
        // be there and recreating it is the login's business. Deeper than that,
        // a directory this walk emptied is cruft — but one that was ALREADY
        // empty belongs to a program that made it and has not written into it
        // yet.
        let tree = Tree::new("pastas");
        tree.plant("keeper/deep/gone.bin", days(30));
        std::fs::create_dir_all(tree.at("keeper/untouched")).unwrap();
        std::fs::create_dir_all(tree.at("empty-child")).unwrap();

        let took = sweep(&tree.0, SystemTime::now(), days(20), Act::Execute);

        assert!(
            !tree.at("keeper/deep").exists(),
            "the directory the walk emptied stayed"
        );
        assert!(
            tree.at("keeper").exists(),
            "a child of the root was removed"
        );
        assert!(
            tree.at("empty-child").exists(),
            "an empty child of the root was removed"
        );
        assert!(
            tree.at("keeper/untouched").exists(),
            "a directory that was already empty was removed"
        );
        assert_eq!(took.dirs, 1, "it counted a directory it did not empty");
    }

    #[test]
    fn a_root_that_is_not_there_is_not_a_failure() {
        // A machine that has never cached anything has nothing to sweep, and
        // saying so with an error would make a first boot look broken.
        let took = sweep(
            Path::new("/nonexistent/cache/root"),
            SystemTime::now(),
            days(20),
            Act::Execute,
        );
        assert_eq!(took, Swept::default());
    }

    #[test]
    fn the_age_is_the_later_of_the_two_stamps() {
        // The unit under the rule above, on its own — so a change to `sweep`
        // cannot quietly make this true by accident somewhere else.
        let tree = Tree::new("idade");
        let at = tree.plant_with("x.bin", days(9), days(3));
        let meta = std::fs::symlink_metadata(&at).unwrap();
        let age = unused_for(&meta, SystemTime::now()).expect("a readable stamp");
        assert!(
            age < days(4),
            "the earlier stamp won: the file reads as {} days old",
            age.as_secs() / SECONDS_IN_A_DAY
        );
    }
}
