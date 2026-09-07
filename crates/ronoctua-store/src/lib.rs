//! One way to keep a person's file — and one way to offer it to everybody.
//!
//! # Why this exists
//!
//! Because the pattern was written once per domain, and the copies had already
//! drifted. A census of the tree found THREE atomic writes (this crate's
//! ancestor, a private copy in `ronoctua_notify::store`, and a
//! truncate-in-place in the network's secret), TWO stores that were not atomic
//! at all (both of the brain's), FOUR resolutions of the XDG roots outside
//! [`ronoctua_xdg`], TWO identical `Where` structs, and the block that reads a
//! file, fails to parse it, logs and falls back to the default written SEVEN
//! times. About a thousand lines of load, save, watch and reconcile, per
//! domain.
//!
//! [`ronoctua_xdg`] stays what it is — sixteen lines of path arithmetic and an
//! atomic write, with ZERO dependencies, because three daemons link it for
//! that alone. This crate is the layer above: it knows what a `T` is, so it
//! carries serde, and it is what a domain with a file should reach for.
//!
//! # Each domain keeps its own FILE
//!
//! Not one database. The reasons are this system's own, and they were weighed
//! against a distro-wide SQLite before this crate was written:
//!
//! - **The file is the broadcast.** `theme.json` is written by one process and
//!   watched by eight; the notification history is written by two and
//!   reconciled by both. A database would still need a second channel to say
//!   "something changed", so it would replace a mechanism that works with one
//!   that needs another mechanism.
//! - **The person edits these by hand**, and `docs/ARCHITECTURE.md` leans on
//!   that when it argues a filter is `field + test` and not a regex.
//! - **The blast radius.** A file that will not parse costs ONE feature, which
//!   degrades and says so. One database costs all of them.
//! - **Nothing in this repository links a database**, and the ISO carries what
//!   the shell links.
//!
//! # The second root is the REPOSITORY
//!
//! A filter a person writes is theirs, on their machine, in their
//! `$XDG_CONFIG_HOME` — that is the default and it never changes on its own.
//! They may also decide one is worth everybody's while, and then it goes into
//! the repository this distro is built from, becomes part of the next build,
//! and reaches every machine that pulls it.
//!
//! **What lands there is PUBLIC.** The repository is public (invariant 2) and
//! no personal datum belongs in it (invariant 13), so nothing here promotes
//! anything on its own: [`Shared`] is only ever handed one item at a time, by
//! name, by a gesture that has already said so on screen.
//!
//! # And nothing asks the network
//!
//! "Has what I shared arrived?" is answered by comparing against the table this
//! machine COMPILED. The only way distro content reaches a machine is a build
//! from the flake, so the build is the answer — see [`Shared::landed`]. It is
//! the efficient way because it is not a check at all.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::PathBuf;

pub use ronoctua_xdg::{NAME, config_dir, state_dir, write_atomically};

/// Where a process keeps things — and `None` means it does not keep them.
///
/// Carried and never found: a `Where` built by a test has `None` everywhere,
/// which is what stops a unit test writing into the configuration of whoever
/// ran `cargo test`. The application resolves it once, at the top.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Where {
    /// `$XDG_CONFIG_HOME/ronoctua` — what the person chose.
    pub config: Option<PathBuf>,
    /// `$XDG_STATE_HOME/ronoctua` — what happened.
    pub state: Option<PathBuf>,
    /// The repository this machine's distro is built from, when it has one.
    pub repo: Option<PathBuf>,
}

impl Where {
    /// The three the APPLICATION uses, found once from the environment.
    #[must_use]
    pub fn of_env() -> Self {
        Self {
            config: config_dir(),
            state: state_dir(),
            repo: repo_dir(),
        }
    }
}

/// The repository this machine's distro is built from, if it is on it.
///
/// From `$RONOCTUA_REPO`, which the NixOS module sets from the `ronoctua.repo`
/// option — never searched for. Invariant 1 is about what an agent may go
/// looking for, and the same restraint is right here: a program that hunts for
/// a git checkout will find somebody else's.
///
/// `None` on a machine that installed the distro rather than building it, and
/// then sharing is simply not offered.
#[must_use]
pub fn repo_dir() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("RONOCTUA_REPO")?);
    path.join("flake.nix").is_file().then_some(path)
}

/// One file, under one root.
///
/// `T` is what it holds. Reading a file that is not there, or that will not
/// parse, gives `T::default()` and says why in the log — a read-only home, a
/// full disk, a file somebody hand-edited into invalid JSON: none of those is
/// a reason for the desktop not to come up.
#[derive(Debug, Clone)]
pub struct Store<T> {
    dir: Option<PathBuf>,
    file: &'static str,
    what: std::marker::PhantomData<T>,
}

impl<T: serde::Serialize + serde::de::DeserializeOwned + Default> Store<T> {
    /// A store in `dir`, named `file`. `None` is a store that keeps nothing.
    #[must_use]
    pub const fn at(dir: Option<PathBuf>, file: &'static str) -> Self {
        Self {
            dir,
            file,
            what: std::marker::PhantomData,
        }
    }

    /// Where it is, when it is anywhere.
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(self.file))
    }

    /// What is in it — or the default, with the reason logged.
    #[must_use]
    pub fn read(&self) -> T {
        let Some(path) = self.path() else {
            return T::default();
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            // NOT AN ERROR: a file that has never been written is the ordinary
            // state of a fresh machine, and every one of these degrades to the
            // default anyway.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return T::default(),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "could not read the store");
                return T::default();
            }
        };
        match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(e) => {
                // LOUD, because the person loses what was in it: their filters,
                // their adjustments. Silently starting from nothing is how a
                // day's work disappears with nothing to read afterwards.
                tracing::error!(
                    path = %path.display(),
                    error = %e,
                    "the store will not parse; starting from the default"
                );
                T::default()
            }
        }
    }

    /// Writes it whole, atomically. A store that keeps nothing writes nothing.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn write(&self, value: &T) -> std::io::Result<()> {
        let Some(dir) = self.dir.as_ref() else {
            return Ok(());
        };
        let bytes = serde_json::to_vec_pretty(value)?;
        write_atomically(dir, self.file, &bytes)
    }
}

/// The half of a store that goes to EVERYBODY.
///
/// It writes into the repository, under `assets/shared/`, in the JSON the
/// build reads. Two things it deliberately does not do: decide what goes, and
/// commit. What goes is named one item at a time by somebody who was shown
/// what it says; committing needs the owner in front of the machine anyway.
#[derive(Debug, Clone, Default)]
pub struct Shared {
    root: Option<PathBuf>,
}

/// Where inside the repository the shared files live.
pub const SHARED: &str = "assets/shared";

impl Shared {
    /// The one this repository has, or nothing.
    #[must_use]
    pub fn of(repo: Option<PathBuf>) -> Self {
        Self {
            root: repo.map(|r| r.join(SHARED)),
        }
    }

    /// Can anything be shared from this machine at all?
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.root.is_some()
    }

    /// What the repository holds under `file` today.
    #[must_use]
    pub fn read<T: serde::de::DeserializeOwned + Default + serde::Serialize>(
        &self,
        file: &'static str,
    ) -> T {
        Store::at(self.root.clone(), file).read()
    }

    /// Puts `value` in the repository under `file`.
    ///
    /// It changes SOURCE, not state: what makes it real is the next build, and
    /// what publishes it is the owner's own commit.
    ///
    /// # Errors
    ///
    /// There is no repository, or it cannot be written.
    pub fn write<T: serde::Serialize + serde::de::DeserializeOwned + Default>(
        &self,
        file: &'static str,
        value: &T,
    ) -> std::io::Result<()> {
        if self.root.is_none() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "this machine has no repository to share into",
            ));
        }
        Store::at(self.root.clone(), file).write(value)
    }

    /// Has something shared already come back as part of the distro?
    ///
    /// `compiled` is what this build has — the table generated from the
    /// repository at the time it was built. If the id is in it, the copy that
    /// was kept locally while it travelled has done its job and can go.
    ///
    /// **Nothing here asks the network, or git.** The only way content of the
    /// distro reaches a machine is a build from the flake, so the build is the
    /// answer: it is either in this binary's table or it is still on its way.
    pub fn landed<'a>(id: &str, compiled: impl IntoIterator<Item = &'a str>) -> bool {
        compiled.into_iter().any(|it| it == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-store-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_store_that_keeps_nothing_reads_the_default_and_writes_no_file() {
        // What every test builds, and the reason it can: a `None` root cannot
        // reach anybody's real configuration.
        let s: Store<Vec<String>> = Store::at(None, "x.json");
        assert!(s.read().is_empty());
        assert!(s.write(&vec!["a".to_owned()]).is_ok());
        assert_eq!(s.path(), None);
    }

    #[test]
    fn what_is_written_comes_back_and_a_broken_file_does_not_take_the_desktop_down() {
        let dir = scratch("round");
        let s: Store<Vec<String>> = Store::at(Some(dir.clone()), "x.json");
        // Nothing there yet is not an error; it is a fresh machine.
        assert!(s.read().is_empty());
        s.write(&vec!["one".to_owned(), "two".to_owned()]).unwrap();
        assert_eq!(s.read(), vec!["one".to_owned(), "two".to_owned()]);
        // AND NO SCRATCH LEFT BEHIND: the write goes through a `.new` and a
        // rename, so a watcher sees one finished file and never half of one.
        assert!(!dir.join("x.json.new").exists());
        // Hand-edited into nonsense: the default, and a loud line in the log.
        std::fs::write(dir.join("x.json"), b"{ not json").unwrap();
        assert!(s.read().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sharing_needs_a_repository_and_says_so_when_there_is_none() {
        let closed = Shared::of(None);
        assert!(!closed.is_open());
        assert!(
            closed
                .write("notifications.json", &Vec::<String>::new())
                .is_err()
        );

        let dir = scratch("shared");
        let open = Shared::of(Some(dir.clone()));
        assert!(open.is_open());
        open.write("notifications.json", &vec!["a".to_owned()])
            .unwrap();
        // UNDER `assets/shared`, which is where the build looks.
        assert!(dir.join(SHARED).join("notifications.json").is_file());
        assert_eq!(
            open.read::<Vec<String>>("notifications.json"),
            vec!["a".to_owned()]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_has_arrived_is_answered_by_the_build_and_nothing_else() {
        // The whole of the "is it there yet" question. No socket, no git, no
        // clock: the table this binary was built with either has it or does not.
        assert!(Shared::landed(
            "quiet-backups",
            ["small-things-are-quick", "quiet-backups"]
        ));
        assert!(!Shared::landed("quiet-backups", ["small-things-are-quick"]));
    }
}
