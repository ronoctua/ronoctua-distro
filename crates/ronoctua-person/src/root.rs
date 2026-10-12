//! The names root's half of the person's data answers to: two units, the
//! folders the person stages into, where the programs say what they did, and
//! the lock that keeps them — and the update — from interleaving.
//!
//! ```text
//! /run/ronoctua-person/            root's, 0755
//!   staged/                        the person's, 0700: a plain tree (`person/`, `brain-modules/`, `nonce`)
//!   approve/                       the person's, 0700: one `request.json` at a time
//!   verdict.json                   root's, 0644: what the last apply did
//!   approve-verdict.json           root's, 0644: what the last approval did
//!   signer-pin                     root's, 0600: the signing key the sudo door was given, until pinned
//!   machine.lock                   root's, 0600: who may touch the machine's person data now
//! ```
//!
//! `/run` because none of it outlives a boot, and none of it should: the
//! person stages again before every apply (the update's `person-stage`, the
//! software centre's declare). The folder above the two staging folders is
//! ROOT's, so the person can fill them but never swap one for a link.
//!
//! `nix/modules/person-apply.nix` declares the same names, and
//! `person-apply-rule-is-narrow` holds them together.
//!
//! # The lock, and why it is a file only root can open
//!
//! `apply` swaps the machine's `person/`, `verify` decides what the rebuild
//! skips, `approve` writes `local/approved.json` — and the update's rebuild
//! reads all three. They take [`LOCK`] exclusively, each for as long as it
//! reads or writes. A lock on a folder everybody may read (the machine
//! directory itself) could be HELD by anybody — `flock` needs only a read
//! descriptor — and one unprivileged process would stall root's paths for
//! ever; nobody but root can open a `0600` file of root's in a folder of
//! root's.
//!
//! The UPDATE holds it from `verify` through the build and the switch (the
//! contract in `docs/ARCHITECTURE.md`, L06): an apply in between would hand
//! the rebuild an entry verify never tried. A holder hands the lock down to
//! its children as their standard input ([`Lock::for_a_child`],
//! `ronoctua-person hold`), and a program started that way finds it there
//! instead of waiting on itself ([`Lock::take`]). Everybody else waits at
//! most [`LOCK_WAIT_S`] and then refuses with a reason — `busy` — rather than
//! hanging a unit whose start never times out.

/// The unit that validates the staged tree and swaps it into the machine
/// directory. Started by the person through ONE narrow polkit rule (verb
/// `start`, this unit, the one user), and by root's own update.
pub const APPLY_UNIT: &str = "ronoctua-person-apply.service";

/// The unit that approves a person brain module's root part. Started ONLY by
/// the sudo door (`RootVerb::PersonModuleApprove`); no polkit rule names it.
pub const APPROVE_UNIT: &str = "ronoctua-person-approve.service";

/// The unit that pins the person's signing key in the machine directory
/// (`local/signer.json`). Started ONLY by the sudo door
/// (`RootVerb::PersonSignerPin`), after the person's password, with the key
/// root's helper wrote in [`SIGNER_PIN`]; no polkit rule names it.
pub const PIN_UNIT: &str = "ronoctua-person-pin.service";

/// Root's folder of the person's staging.
pub const RUN: &str = "/run/ronoctua-person";

/// Where the person stages a plain tree for the applier.
pub const STAGED: &str = "/run/ronoctua-person/staged";

/// Where the person stages one approval request ([`crate::approve::REQUEST`]).
pub const APPROVE: &str = "/run/ronoctua-person/approve";

/// What the sudo door was asked to approve — the module and the hash the
/// person was shown, as the door's own arguments — written by root's helper
/// (`ronoctua-security-root`, `0600`) after the password and before it starts
/// [`APPROVE_UNIT`], which consumes it. Root's folder: no process of the
/// person's can write it, so what is approved is what the door was given,
/// never what sits in the person's folder by then.
pub const APPROVE_PIN: &str = "/run/ronoctua-person/approve-pin";

/// The signing key the sudo door was asked to pin — the public key, as the
/// door's own argument, which the person's passphrase opened from `key.age`
/// — written by root's helper (`ronoctua-security-root`, `0600`) before it
/// starts [`PIN_UNIT`], which consumes it. Root's folder, as
/// [`APPROVE_PIN`]: what is pinned is what the door was given.
pub const SIGNER_PIN: &str = "/run/ronoctua-person/signer-pin";

/// What the last apply did ([`crate::tree::Verdict`]), written by root.
pub const VERDICT: &str = "/run/ronoctua-person/verdict.json";

/// What the last approval did ([`crate::approve::Verdict`]), written by root.
pub const APPROVE_VERDICT: &str = "/run/ronoctua-person/approve-verdict.json";

/// The lock of the machine's person data: root's, `0600`, in root's folder.
pub const LOCK: &str = "/run/ronoctua-person/machine.lock";

/// How long a program waits for [`LOCK`] before refusing as busy. An apply
/// or an approval holds it for seconds; the update holds it through a whole
/// rebuild, and a declaration made meanwhile is refused with that reason
/// instead of hanging the software centre for an hour.
pub const LOCK_WAIT_S: u64 = 30;

#[cfg(feature = "apply")]
pub use lock::{Lock, LockRefused};

/// The lock itself — root's half only (feature `apply`): it opens a file and
/// calls `flock`, which the pure library never does.
#[cfg(feature = "apply")]
mod lock {
    use std::os::fd::{AsFd, OwnedFd};
    use std::path::Path;
    use std::time::{Duration, Instant};

    use rustix::fs::{CWD, FileType, FlockOperation, Mode, OFlags, Stat};
    use rustix::io::Errno;

    /// The lock, held for as long as this lives.
    #[derive(Debug)]
    pub struct Lock {
        /// The descriptor that holds it; `None` when it is held through the
        /// standard input a holder handed down — closing that is the
        /// holder's business, and unlocking it would release the holder's.
        own: Option<OwnedFd>,
    }

    /// Why the lock was not taken.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum LockRefused {
        /// Somebody held it longer than the wait.
        Busy,
        /// The lock, or its folder, is not ours alone: somebody else could
        /// open it, and hold it for ever.
        NotOurs(String),
        /// It could not be made or opened.
        Unopenable(String),
    }

    impl std::fmt::Display for LockRefused {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Busy => write!(f, "another of root's programs holds the lock"),
                Self::NotOurs(what) => write!(f, "the lock is not root's alone: {what}"),
                Self::Unopenable(what) => write!(f, "the lock could not be opened: {what}"),
            }
        }
    }

    impl std::error::Error for LockRefused {}

    /// A folder or file that is `me`'s and that nobody else may write (a
    /// folder) or even open (the lock).
    fn ours(st: &Stat, me: u32, closed: u32, what: &str) -> Result<(), LockRefused> {
        if st.st_uid != me {
            return Err(LockRefused::NotOurs(format!(
                "{what} belongs to uid {}",
                st.st_uid
            )));
        }
        if st.st_mode & closed != 0 {
            return Err(LockRefused::NotOurs(format!(
                "{what} has mode {:o}",
                st.st_mode & 0o7777
            )));
        }
        Ok(())
    }

    /// The lock file at `path`, made `0600` if it is not there, in a folder
    /// that is ours and writable by nobody else (made `0755` if missing).
    fn open(path: &Path, me: u32) -> Result<OwnedFd, LockRefused> {
        let unopenable = |e: Errno| LockRefused::Unopenable(format!("{}: {e}", path.display()));
        let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
            return Err(LockRefused::Unopenable(format!(
                "{} is not a file's path",
                path.display()
            )));
        };
        let folder = if folder.as_os_str().is_empty() {
            Path::new(".")
        } else {
            folder
        };
        match rustix::fs::mkdirat(CWD, folder, Mode::from_raw_mode(0o755)) {
            Ok(()) | Err(Errno::EXIST) => {}
            Err(e) => return Err(unopenable(e)),
        }
        let dir = rustix::fs::openat(
            CWD,
            folder,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(unopenable)?;
        ours(
            &rustix::fs::fstat(&dir).map_err(unopenable)?,
            me,
            0o022,
            "its folder",
        )?;
        let file = rustix::fs::openat(
            &dir,
            name,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(unopenable)?;
        let st = rustix::fs::fstat(&file).map_err(unopenable)?;
        if FileType::from_raw_mode(st.st_mode) != FileType::RegularFile {
            return Err(LockRefused::NotOurs("it is not a regular file".to_owned()));
        }
        ours(&st, me, 0o077, "it")?;
        Ok(file)
    }

    /// Whether the standard input IS this lock, and now holds it: a holder
    /// handed it down (the same open file, so `flock` is already ours).
    fn handed_down(lock: &OwnedFd) -> bool {
        let stdin = std::io::stdin();
        let (Ok(theirs), Ok(mine)) = (rustix::fs::fstat(stdin.as_fd()), rustix::fs::fstat(lock))
        else {
            return false;
        };
        theirs.st_dev == mine.st_dev
            && theirs.st_ino == mine.st_ino
            && rustix::fs::flock(stdin.as_fd(), FlockOperation::NonBlockingLockExclusive).is_ok()
    }

    impl Lock {
        /// Takes the lock at `path` (root's [`super::LOCK`]), waiting at most
        /// `wait` for whoever holds it — or finds it already held by the
        /// caller that handed it down as the standard input.
        ///
        /// The wait asks again every 200 ms, and only while waiting: `flock`
        /// has no deadline of its own, and a thread left blocked in it would
        /// take the lock after the refusal was given.
        ///
        /// # Errors
        ///
        /// [`LockRefused::Busy`] after `wait`; [`LockRefused::NotOurs`] for a
        /// lock somebody else could open; [`LockRefused::Unopenable`].
        pub fn take(path: &Path, wait: Duration) -> Result<Self, LockRefused> {
            let me = rustix::process::geteuid().as_raw();
            let file = open(path, me)?;
            if handed_down(&file) {
                return Ok(Self { own: None });
            }
            let deadline = Instant::now() + wait;
            loop {
                match rustix::fs::flock(&file, FlockOperation::NonBlockingLockExclusive) {
                    Ok(()) => return Ok(Self { own: Some(file) }),
                    Err(Errno::WOULDBLOCK | Errno::INTR) => {}
                    Err(e) => {
                        return Err(LockRefused::Unopenable(format!("{}: {e}", path.display())));
                    }
                }
                let now = Instant::now();
                if now >= deadline {
                    return Err(LockRefused::Busy);
                }
                std::thread::sleep((deadline - now).min(Duration::from_millis(200)));
            }
        }

        /// Takes the lock at `path`, waiting for it for as long as its holder
        /// keeps it — or finds it handed down, as [`Lock::take`] does.
        ///
        /// THE UPDATE'S ROOT HALF WAITS THIS WAY (`ronoctua-person hold
        /// --patient`): a unit that never times out, that nobody watches, and
        /// whose work is the same as the holder's — an apply building the
        /// system. Refused as busy after [`super::LOCK_WAIT_S`] it failed the
        /// whole update while the machine was merely applying; waiting, it
        /// runs once the apply is done. The wait is the kernel's, blocked in
        /// `flock` until the holder lets go — no clock — and stopping the
        /// unit ends it.
        ///
        /// # Errors
        ///
        /// [`LockRefused::NotOurs`] for a lock somebody else could open;
        /// [`LockRefused::Unopenable`].
        pub fn wait_for(path: &Path) -> Result<Self, LockRefused> {
            let me = rustix::process::geteuid().as_raw();
            let file = open(path, me)?;
            if handed_down(&file) {
                return Ok(Self { own: None });
            }
            loop {
                match rustix::fs::flock(&file, FlockOperation::LockExclusive) {
                    Ok(()) => return Ok(Self { own: Some(file) }),
                    Err(Errno::INTR) => {}
                    Err(e) => {
                        return Err(LockRefused::Unopenable(format!("{}: {e}", path.display())));
                    }
                }
            }
        }

        /// The lock as a child's standard input — the same open file, so the
        /// child holds it as this process does, and [`Lock::take`] in the
        /// child finds it instead of waiting on its own parent.
        ///
        /// # Errors
        ///
        /// When the descriptor cannot be duplicated.
        pub fn for_a_child(&self) -> std::io::Result<std::process::Stdio> {
            match &self.own {
                Some(fd) => Ok(std::process::Stdio::from(fd.try_clone()?)),
                None => Ok(std::process::Stdio::inherit()),
            }
        }

        /// Whether this process took the lock itself (`false`: a holder
        /// handed it down).
        #[must_use]
        pub const fn is_own(&self) -> bool {
            self.own.is_some()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt as _;

        fn scratch(what: &str) -> std::path::PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-person-lock-{what}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            dir
        }

        /// FAILS IF: a second taker does not wait and then refuse as busy, or
        /// the lock is not free again once its holder is gone.
        #[test]
        fn a_held_lock_is_waited_for_then_refused_as_busy() {
            let dir = scratch("busy");
            let path = dir.join("machine.lock");
            let first = Lock::take(&path, Duration::ZERO).unwrap();
            assert!(first.is_own());
            let started = Instant::now();
            assert_eq!(
                Lock::take(&path, Duration::from_millis(300)).unwrap_err(),
                LockRefused::Busy
            );
            assert!(started.elapsed() >= Duration::from_millis(300), "it waited");
            drop(first);
            assert!(Lock::take(&path, Duration::ZERO).is_ok(), "free again");
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "made for root's eyes only");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// FAILS IF: a patient taker refuses while the lock is held, or does
        /// not take it as soon as its holder lets go.
        #[test]
        fn a_patient_taker_waits_until_the_holder_lets_go() {
            let dir = scratch("patient");
            let path = dir.join("machine.lock");
            let first = Lock::take(&path, Duration::ZERO).unwrap();
            let started = Instant::now();
            let waiting = std::thread::spawn(move || Lock::wait_for(&path).map(|l| l.is_own()));
            std::thread::sleep(Duration::from_millis(400));
            assert!(!waiting.is_finished(), "it did not wait for the holder");
            drop(first);
            assert_eq!(
                waiting.join().unwrap(),
                Ok(true),
                "it did not take the lock"
            );
            assert!(started.elapsed() >= Duration::from_millis(400));
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// FAILS IF: a lock somebody else could open — and so hold for ever —
        /// is used, or a lock that is a link is followed.
        #[test]
        fn a_lock_others_could_open_is_refused() {
            let dir = scratch("open");
            let path = dir.join("machine.lock");
            std::fs::write(&path, b"").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(matches!(
                Lock::take(&path, Duration::ZERO),
                Err(LockRefused::NotOurs(_))
            ));
            std::fs::remove_file(&path).unwrap();
            std::fs::write(dir.join("elsewhere"), b"").unwrap();
            std::os::unix::fs::symlink(dir.join("elsewhere"), &path).unwrap();
            assert!(matches!(
                Lock::take(&path, Duration::ZERO),
                Err(LockRefused::Unopenable(_))
            ));
            std::fs::remove_file(&path).unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
            assert!(matches!(
                Lock::take(&path, Duration::ZERO),
                Err(LockRefused::NotOurs(_))
            ));
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_staging_folders_live_in_roots_folder() {
        for inside in [STAGED, APPROVE, APPROVE_PIN, VERDICT, APPROVE_VERDICT, LOCK] {
            let parent = std::path::Path::new(inside).parent().unwrap();
            assert_eq!(parent, std::path::Path::new(RUN), "{inside}");
        }
        assert!(APPLY_UNIT.ends_with(".service") && APPROVE_UNIT.ends_with(".service"));
        assert_ne!(APPLY_UNIT, APPROVE_UNIT);
    }
}
