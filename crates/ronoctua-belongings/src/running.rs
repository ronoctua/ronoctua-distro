//! One run at a time (feature `prefs`): what keeps two syncs of one machine
//! from running together, and what lets a door SEE that one runs.
//!
//! # Why
//!
//! Every run works in the same clone (`state/belongings/repo`), and the first
//! thing a run does there is make the work tree exactly the repository's head
//! (`checkout -f`, `clean -fdx`). Two runs at once each throw the other's new
//! blobs away before its `git add`, and the index pushed then names blobs
//! that are not there — while both runs say they synced. So `prepare`,
//! `apply`, `abandon` and a change of passphrase each HOLD this lock for the
//! whole run, and one that finds it held is refused ([`Why::Busy`]) and
//! touches nothing: not the secrets the other run is using, not the status
//! it is going to write.
//!
//! # The lock
//!
//! `flock` on `state/belongings/run.lock` (the standard library's
//! `File::try_lock`): the kernel lets it go when the process ends, however it
//! ends, so a run that was killed never leaves the machine locked. The file is
//! never deleted — deleting a lock file races whoever opened it a moment
//! before.
//!
//! # What a door sees
//!
//! [`busy`] asks without waiting (a shared lock, let go at once), and a door
//! refuses a second sync with the same sentence. A run marks the file when it
//! takes the lock (its time changes), so the door's watch of the state folder
//! hears a run begin; it hears the end in the status the run writes.

use std::fs::{File, TryLockError};
use std::os::unix::fs::OpenOptionsExt as _;
use std::time::{Duration, Instant, SystemTime};

use crate::places::Places;
use crate::secret::make_private_dir;
use crate::{Refusal, Why};

/// How long a run waits for the lock before it is refused. A door that only
/// LOOKS ([`busy`]) holds it for microseconds; a run holds it for seconds.
pub const PATIENCE: Duration = Duration::from_millis(600);

/// How often the lock is asked again while waiting.
const AGAIN: Duration = Duration::from_millis(20);

/// The lock, held until it is dropped.
#[derive(Debug)]
pub struct Held {
    _file: File,
}

/// Takes this machine's lock, waiting at most [`PATIENCE`].
///
/// # Errors
///
/// [`Why::Busy`] when another run holds it; [`Why::Failed`] when the state
/// folder or the file cannot be made.
pub fn hold(places: &Places) -> Result<Held, Refusal> {
    let failed = |_| Refusal::of(Why::Failed);
    make_private_dir(&places.own()).map_err(failed)?;
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(places.lock())
        .map_err(failed)?;
    let until = Instant::now() + PATIENCE;
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if Instant::now() < until => {
                std::thread::sleep(AGAIN);
            }
            Err(TryLockError::WouldBlock) => return Err(Refusal::of(Why::Busy)),
            Err(TryLockError::Error(_)) => return Err(Refusal::of(Why::Failed)),
        }
    }
    // SAID TO WHOEVER WATCHES THE FOLDER: a new time is an `ATTRIB`, which a
    // door's watch hears — and redraws "a sync is running".
    let _ = file.set_modified(SystemTime::now());
    Ok(Held { _file: file })
}

/// Whether a run holds the lock now. Never waits, and never makes the file.
#[must_use]
pub fn busy(places: &Places) -> bool {
    let Ok(file) = File::open(places.lock()) else {
        return false;
    };
    matches!(file.try_lock_shared(), Err(TryLockError::WouldBlock))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a second run gets the lock while the first holds it, a door
    /// cannot see that one runs, or the lock outlives the run that held it.
    #[test]
    fn one_run_at_a_time_and_a_door_sees_it() {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-belongings-running-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let places = Places::under(&root);
        assert!(!busy(&places), "busy with no lock file at all");
        let held = hold(&places).expect("the first run takes it");
        assert!(busy(&places), "a door does not see the run");
        let started = Instant::now();
        assert_eq!(hold(&places).unwrap_err().why, Why::Busy);
        assert!(
            started.elapsed() >= PATIENCE,
            "the second run did not wait before it was refused"
        );
        drop(held);
        assert!(!busy(&places), "the lock outlived its run");
        // A LOOK DOES NOT TAKE IT: a run that starts while a door looks waits
        // the look out, and gets it.
        let look = File::open(places.lock()).unwrap();
        look.try_lock_shared().unwrap();
        let waiting = std::thread::spawn(move || hold(&places).map(|_| ()));
        std::thread::sleep(AGAIN * 3);
        drop(look);
        assert_eq!(waiting.join().unwrap(), Ok(()));
        let _ = std::fs::remove_dir_all(root);
    }
}
