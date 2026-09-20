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
//! # WHAT IT LOOKS AT: the entries in the root, each one whole
//!
//! Not every file, weighed on its own. The owner stopped that, and the reason
//! is worth having here too: a cache is rarely a heap of independent files. It
//! is an index and the things the index names, a manifest and its blobs, a
//! database and its write-ahead log. Take the ones nobody read lately and leave
//! the rest, and you have not freed a cache — you have left a program holding
//! half of something, which is a class of failure nobody can debug from a
//! directory listing.
//!
//! Nearly every program keeps its cache in a directory of its own under the
//! root, which is what makes the coarse rule the SAFE one: the unit of deletion
//! becomes the unit the program created. **A directory is as old as its
//! youngest file**, so a cache anything still touches is never taken, however
//! much of it is ancient — and the walk still has to descend to find that out.
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
use std::path::Path;
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
    /// Files in the ROOT taken, or that would have been.
    ///
    /// A cache kept as a single file rather than a directory — `.tdb`, a
    /// sqlite database, an index. Most of what goes is a directory.
    pub files: u64,
    /// What they weighed.
    pub bytes: u64,
    /// Entries in the root looked at and left, because something still uses
    /// them — or because they could not be dated at all.
    pub kept: u64,
    /// Directories in the root taken WHOLE, or that would have been.
    ///
    /// The ordinary case: nearly every program keeps its cache in one of these,
    /// and a directory goes entire or not at all. See the module header.
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

/// Take every entry in `root` that nothing has used for `keep` or longer.
///
/// An entry is a file or a DIRECTORY, and a directory goes whole or not at all
/// — see the module header for why that is the safe rule and not the coarse
/// one. Its age is its youngest file's, which is what "nothing in here has been
/// touched" means.
///
/// `now` is a PARAMETER and not a clock: a sweep whose age arithmetic reads the
/// system clock cannot be tested against a file planted two days in the future,
/// and that file is the case this most needs to get right.
///
/// The root itself is never removed.
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
    let Ok(entries) = std::fs::read_dir(root) else {
        out.refused += 1;
        return out;
    };

    for entry in entries {
        let Ok(entry) = entry else {
            out.refused += 1;
            continue;
        };
        let path = entry.path();
        // SYMLINK METADATA, ALWAYS. `metadata` here would follow a link out of
        // the cache root and make every rule below a rule about somebody else's
        // files.
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            out.refused += 1;
            continue;
        };
        // A LINK, A SOCKET, A FIFO, A DEVICE: left alone. The first is what this
        // must not follow and the rest belong to something running right now.
        if !meta.is_dir() && !meta.is_file() {
            continue;
        }
        // AND NEVER OFF THIS DISK. A mount under the cache root is another
        // filesystem with another owner's rules on it.
        if meta.dev() != disk {
            continue;
        }

        let (age, weight) = if meta.is_dir() {
            match measure(&path, now, disk) {
                Some(measured) => measured,
                None => {
                    // SOMETHING IN THERE COULD NOT BE READ OR COULD NOT BE
                    // DATED, so this directory has no age — and a directory
                    // with no age is one this does not delete. Refusing to
                    // answer is the only safe answer about a tree one cannot
                    // see all of.
                    out.refused += 1;
                    out.kept += 1;
                    continue;
                }
            }
        } else {
            match unused_for(&meta, now) {
                Some(age) => (age, meta.len()),
                None => {
                    out.kept += 1;
                    continue;
                }
            }
        };

        if age < keep {
            out.kept += 1;
            continue;
        }
        if act == Act::Execute {
            let gone = if meta.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            match gone {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(_) => {
                    out.refused += 1;
                    continue;
                }
            }
        }
        if meta.is_dir() {
            out.dirs += 1;
        } else {
            out.files += 1;
        }
        out.bytes += weight;
    }
    out
}

/// How long since ANYTHING under `at` was used, and what it all weighs.
///
/// The age of a directory is its YOUNGEST file's, which is what "nothing in
/// here has been touched" means — one file somebody opened yesterday keeps the
/// whole thing, however ancient the rest of it is. That is the property that
/// makes taking a directory whole safe: a cache in use is never taken.
///
/// `None` when any part of it could not be read or could not be dated. A
/// directory this cannot see all of is one it must not delete, and refusing to
/// answer is the only honest way to say so.
fn measure(at: &Path, now: SystemTime, disk: u64) -> Option<(Duration, u64)> {
    let mut youngest = Duration::MAX;
    let mut weight = 0u64;
    let mut empty = true;
    // AN EXPLICIT STACK AND NOT RECURSION. A cache directory's depth is not
    // ours to bound, and a deep one would be a stack overflow — which in this
    // process is a sweep that half ran.
    let mut stack = vec![at.to_path_buf()];
    while let Some(here) = stack.pop() {
        for entry in std::fs::read_dir(&here).ok()? {
            let entry = entry.ok()?;
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).ok()?;
            if meta.is_dir() {
                // A MOUNT INSIDE MAKES THE WHOLE THING UNANSWERABLE, because
                // what is under it is not this filesystem's to weigh — and a
                // `remove_dir_all` over it would be deleting across the
                // boundary this refuses to cross.
                if meta.dev() != disk {
                    return None;
                }
                stack.push(path);
                empty = false;
                continue;
            }
            empty = false;
            // A LINK OR A SOCKET INSIDE IS NOT WEIGHED AND NOT DATED. It is
            // still taken with the directory, which is right: it belongs to
            // whatever made the directory.
            if !meta.is_file() {
                continue;
            }
            youngest = youngest.min(unused_for(&meta, now)?);
            weight = weight.saturating_add(meta.len());
        }
    }
    // AN EMPTY DIRECTORY HAS NO YOUNGEST FILE, so it is dated by ITSELF — and
    // by its MODIFICATION time alone, which is the one place in this crate that
    // does not take the later of the two.
    //
    // Because a directory's access time is not evidence about the person: it
    // moves when ANYBODY lists it, and the thing that lists it most reliably is
    // this walk. `read_dir` a few lines above stamps `atime` with now, so a
    // rule taking the later of the two would find every empty directory freshly
    // used the moment it looked — for ever, and the abandoned one would never
    // go. Found by a test that planted one three months old and watched it
    // survive.
    //
    // A directory's mtime moves when its CONTENTS change, which is exactly the
    // question being asked of an empty one.
    if empty {
        let mine = std::fs::symlink_metadata(at).ok()?;
        return Some((now.duration_since(mine.modified().ok()?).ok()?, 0));
    }
    Some((youngest, weight))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

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
    fn a_directory_is_as_old_as_its_youngest_file_and_goes_whole() {
        // THE RULE THE OWNER ASKED FOR, and the reason: a cache is rarely a
        // heap of independent files. Take the ones nobody read lately and leave
        // the rest, and a program is left holding half of an index — a class of
        // failure nobody can debug from a directory listing.
        //
        // So the unit is the entry in the ROOT, and one file somebody opened
        // yesterday keeps the whole thing.
        let tree = Tree::new("pasta-inteira");
        tree.plant("alive/ancient.bin", days(400));
        tree.plant("alive/deep/older.bin", days(400));
        tree.plant("alive/deep/read-yesterday.bin", days(1));
        tree.plant("dead/one.bin", days(90));
        tree.plant("dead/deep/two.bin", days(90));

        let took = sweep(&tree.0, SystemTime::now(), days(40), Act::Execute);

        assert!(
            tree.at("alive/ancient.bin").exists(),
            "a four-hundred-day-old file was taken out from under a cache in use"
        );
        assert!(tree.at("alive/deep/read-yesterday.bin").exists());
        assert!(
            !tree.at("dead").exists(),
            "a directory nothing has touched in three months stayed"
        );
        assert_eq!(
            took.dirs, 1,
            "it took something other than the one directory"
        );
        assert_eq!(took.files, 0, "it went inside and took files one by one");
        assert_eq!(took.kept, 1, "it did not count the live one as kept");
        assert!(took.bytes > 0, "it weighed nothing");
    }

    #[test]
    fn a_directory_it_cannot_see_all_of_is_one_it_does_not_delete() {
        // REFUSING TO ANSWER IS THE ONLY SAFE ANSWER about a tree one cannot
        // read: an unreadable corner could hold the file that is in use, and
        // `remove_dir_all` would take it with everything else.
        let tree = Tree::new("ilegivel");
        tree.plant("blind/old.bin", days(400));
        let shut = tree.at("blind/inner");
        std::fs::create_dir_all(&shut).unwrap();
        std::fs::write(shut.join("x.bin"), b"x").unwrap();
        // NO PERMISSION TO LOOK INSIDE, which is what a cache of another
        // program's can genuinely be.
        let mut mode = std::fs::metadata(&shut).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o000);
        std::fs::set_permissions(&shut, mode).unwrap();

        let took = sweep(&tree.0, SystemTime::now(), days(40), Act::Execute);

        assert!(
            tree.at("blind").exists(),
            "a directory with an unreadable corner was deleted anyway"
        );
        assert_eq!(took.refused, 1, "it did not say it could not answer");
        assert_eq!(took.dirs, 0);

        let mut mode = std::fs::metadata(&shut).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
        let _ = std::fs::set_permissions(&shut, mode);
    }

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
        tree.plant("old.bin", days(60));
        tree.plant("new.bin", days(1));
        let took = sweep(&tree.0, SystemTime::now(), days(40), Act::Execute);
        assert_eq!(took.files, 1, "it did not take the old one");
        assert_eq!(took.kept, 1, "it did not leave the new one");
        assert!(!tree.at("old.bin").exists());
        assert!(tree.at("new.bin").exists(), "today's work went");
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
        tree.plant("dead/one.bin", days(90));
        tree.plant("dead/two.bin", days(90));
        tree.plant("alive/new.bin", days(2));

        let said = sweep(&tree.0, SystemTime::now(), days(40), Act::Rehearse);
        assert_eq!(said.dirs, 1);
        assert!(said.bytes > 0);
        assert!(tree.at("dead").exists(), "a rehearsal deleted a directory");

        let did = sweep(&tree.0, SystemTime::now(), days(40), Act::Execute);
        assert_eq!(
            (did.dirs, did.bytes),
            (said.dirs, said.bytes),
            "the rehearsal's number and the real one disagree"
        );
    }

    #[test]
    fn an_empty_directory_is_dated_by_itself() {
        // THE ONE CASE where a directory's own stamp is the honest answer: it
        // has no youngest file to be as old as. A program that made its cache
        // directory this morning and has not written into it yet must keep it;
        // one abandoned three months ago is rubbish like any other.
        let tree = Tree::new("vazia");
        let fresh = tree.at("just-made");
        std::fs::create_dir_all(&fresh).unwrap();
        let old = tree.at("abandoned");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::File::open(&old)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_accessed(SystemTime::now() - days(90))
                    .set_modified(SystemTime::now() - days(90)),
            )
            .unwrap();

        let took = sweep(&tree.0, SystemTime::now(), days(40), Act::Execute);

        assert!(fresh.exists(), "a directory made this morning was taken");
        assert!(
            !old.exists(),
            "an abandoned empty directory stayed for ever"
        );
        assert_eq!(took.dirs, 1);
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
