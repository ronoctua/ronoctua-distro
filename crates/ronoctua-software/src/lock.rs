//! One lane per source: two orders on the same source never run at once.
//!
//! The screen's orders run in the queue and the update run calls the same CLI
//! from a unit, so the lanes have to be something both see — a lock FILE per
//! source, taken with `flock`. The kernel releases it when the process dies,
//! so a killed job never leaves a lane shut.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use rustix::fs::{FlockOperation, flock};

use crate::Source;

/// A held lane. Dropping it (or dying) opens the lane again.
#[derive(Debug)]
pub struct Lane {
    _file: File,
}

/// Where the lock files live: the runtime directory when there is one (it
/// is private to the person), otherwise our state directory.
#[must_use]
pub fn directory(roots: &crate::Roots) -> PathBuf {
    roots.runtime.as_ref().map_or_else(
        || roots.ours_state(),
        |r| r.join(ronoctua_xdg::name()).join("software-lanes"),
    )
}

/// Take the lane of `source`, waiting for it. `waiting` is called once, before
/// blocking, only when the lane is actually taken by someone else — so the
/// job can say it is waiting instead of looking frozen.
///
/// # Errors
///
/// When the lock file cannot be created or locked.
pub fn take(dir: &Path, source: Source, waiting: impl FnOnce()) -> std::io::Result<Lane> {
    take_file(dir, source.word(), waiting)
}

/// The lane of the person's DECLARATION: whoever reads, changes and stages
/// `declared.json` and `declined.json` holds it, so two orders never write
/// one over the other — a declaration of a Flatpak and one of an AppImage run
/// in two source lanes at once, and both rewrite the same file.
///
/// # Errors
///
/// As [`take`].
pub fn take_declaration(dir: &Path, waiting: impl FnOnce()) -> std::io::Result<Lane> {
    take_file(dir, DECLARATION, waiting)
}

/// The declaration's lock file's stem; no source has this word.
pub const DECLARATION: &str = "declaration";

fn take_file(dir: &Path, stem: &str, waiting: impl FnOnce()) -> std::io::Result<Lane> {
    std::fs::create_dir_all(dir)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(format!("{stem}.lock")))?;
    if flock(&file, FlockOperation::NonBlockingLockExclusive).is_err() {
        waiting();
        flock(&file, FlockOperation::LockExclusive)?;
    }
    Ok(Lane { _file: file })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lane_is_taken_once_and_opens_again_when_dropped() {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-software-lane-{}", std::process::id()));
        let first = take(&dir, Source::Flatpak, || panic!("the lane was free")).unwrap();
        let other = OpenOptions::new()
            .write(true)
            .open(dir.join("flatpak.lock"))
            .unwrap();
        assert!(
            flock(&other, FlockOperation::NonBlockingLockExclusive).is_err(),
            "held"
        );
        drop(first);
        // A FEW TRIES, not one: the test harness runs other tests on other
        // threads, and a child one of them forks holds a copy of every open
        // descriptor — this lock's included — for the instant between its
        // fork and its exec. The lock is released; that copy is what answers.
        let released = (0..200).any(|_| {
            let ok = flock(&other, FlockOperation::NonBlockingLockExclusive).is_ok();
            if !ok {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            ok
        });
        assert!(released, "released");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_declarations_lane_is_no_sources() {
        assert!(Source::ALL.iter().all(|s| s.word() != DECLARATION));
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-declaration-lane-{}",
            std::process::id()
        ));
        let held = take_declaration(&dir, || panic!("free")).unwrap();
        // A SOURCE'S LANE is another file: holding the declaration's never
        // makes an install wait.
        let other = take(&dir, Source::Nix, || panic!("the nix lane was free")).unwrap();
        drop((held, other));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
