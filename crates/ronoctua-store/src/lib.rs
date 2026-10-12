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
//!
//! # A file that does not read whole keeps the rest of it
//!
//! The person's files travel between their machines (the belongings) and
//! under their own hand, so a file this machine cannot take whole is ordinary:
//! a newer machine's answer, one typo. [`Store::read`] is LENIENT
//! ([`lenient`]): the value that will not read is repaired — the type's
//! default there, or the element dropped — and everything else is read. Each
//! repair goes into a ledger beside the state ([`repair`],
//! `state/repairs/`), where the belongings' page shows it, and a WRITE puts
//! every repair the writer did not override back in the file
//! ([`Store::write`]): nothing a person or a newer machine wrote is erased by
//! a machine that merely did not understand it. Keys a type does not know
//! survive a write the same way, and so does the file's key order
//! ([`json::Node`]).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub use ronoctua_xdg::{NAME, config_dir, state_dir, write_atomically};

pub mod json;
pub mod lenient;
pub mod repair;

pub use json::Node;
pub use lenient::Refused;
pub use repair::{Ledger, Repair};

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

/// The repair ledger's folder of this process: `$XDG_STATE_HOME/ronoctua/repairs`
/// — what the view that lists the repairs watches.
#[must_use]
pub fn repairs_dir() -> Option<PathBuf> {
    state_dir().map(|s| repair::dir_under(&s))
}

/// The roots a store's LOGICAL path is read against, for its ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Roots {
    /// This process's (`config_dir`, `state_dir`), found once.
    Env,
    /// Given — a test's, or the belongings' `Places`.
    Given {
        /// The configuration root.
        config: Option<PathBuf>,
        /// The state root, which holds the ledger.
        state: Option<PathBuf>,
    },
}

/// This process's two roots, found ONCE: a store's ledger is placed by
/// comparing the store's folder with them, so a test's scratch folder —
/// under neither — never writes a ledger into the home of whoever runs it.
fn env_roots() -> &'static (Option<PathBuf>, Option<PathBuf>) {
    static ROOTS: OnceLock<(Option<PathBuf>, Option<PathBuf>)> = OnceLock::new();
    ROOTS.get_or_init(|| (config_dir(), state_dir()))
}

/// `config/<rest>` or `state/<rest>` — the logical path of a file under one of
/// the two roots (the configuration first, as the belongings name it).
fn logical_of(path: &Path, config: Option<&Path>, state: &Path) -> Option<String> {
    for (root, word) in [(config, "config"), (Some(state), "state")] {
        let Some(root) = root else {
            continue;
        };
        if let Ok(rest) = path.strip_prefix(root) {
            let rest = rest.to_str()?;
            if !rest.is_empty() {
                return Some(format!("{word}/{rest}"));
            }
        }
    }
    None
}

/// Why a file that is there gave nothing of itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trouble {
    /// It could not be read (a permission, a disk): nothing is known of what
    /// it says.
    Unreadable(String),
    /// It is not JSON.
    NotJson(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreadable(e) | Self::NotJson(e) => f.write_str(e),
        }
    }
}

/// A read, and what it took to read.
#[derive(Debug, Clone)]
pub struct Reading<T> {
    /// What the file says — repaired, or the default.
    pub value: T,
    /// Whether there is a file at all.
    pub found: bool,
    /// What was repaired to read it (also in the ledger).
    pub repairs: Vec<Repair>,
    /// Why a file that is there gave nothing.
    pub trouble: Option<Trouble>,
}

impl<T> Reading<T> {
    /// Whether nothing of a file that is there could be kept: not JSON, or
    /// JSON that is nothing like the type — what a writer must not write over
    /// ([`Store::parsed`]).
    #[must_use]
    pub fn lost_everything(&self) -> bool {
        matches!(self.trouble, Some(Trouble::NotJson(_)))
            || self.repairs.iter().any(|r| r.pointer.is_empty())
    }

    /// Whether PART of the file did not read — a value repaired, the rest
    /// kept. What a view that shows the file owes a sentence about while no
    /// screen lists the ledger: what it shows in that place is not what the
    /// file says (the file keeps it, [`Store::write`]).
    #[must_use]
    pub fn part_not_read(&self) -> bool {
        !self.repairs.is_empty() && !self.lost_everything()
    }

    /// Whether the file, when there is one, read exactly as it is written:
    /// nothing repaired, and JSON. An unreadable file says nothing about
    /// its contents and counts as whole, as it always has.
    #[must_use]
    pub fn whole(&self) -> bool {
        self.repairs.is_empty() && !matches!(self.trouble, Some(Trouble::NotJson(_)))
    }
}

/// A domain's own check ([`Store::checked`]).
type Check<T> = Arc<dyn Fn(&T) -> Vec<Refused> + Send + Sync>;

/// One file, under one root.
///
/// `T` is what it holds. Reading a file that is not there gives
/// `T::default()`; reading one that does not read whole keeps what does and
/// repairs the rest ([`lenient`], [`repair`]) — a read-only home, a full disk,
/// a file somebody hand-edited, a newer machine's answer: none of those is a
/// reason for the desktop not to come up, nor for the rest of the file to be
/// lost.
pub struct Store<T> {
    dir: Option<PathBuf>,
    file: &'static str,
    roots: Roots,
    /// The domain's own check, carried so that EVERY read and write of this
    /// store judges the file the same way ([`Store::checked`]).
    check: Option<Check<T>>,
    what: std::marker::PhantomData<T>,
}

impl<T> Clone for Store<T> {
    fn clone(&self) -> Self {
        Self {
            dir: self.dir.clone(),
            file: self.file,
            roots: self.roots.clone(),
            check: self.check.clone(),
            what: std::marker::PhantomData,
        }
    }
}

impl<T> std::fmt::Debug for Store<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Store")
            .field("dir", &self.dir)
            .field("file", &self.file)
            .field("roots", &self.roots)
            .field("checked", &self.check.is_some())
            .finish()
    }
}

impl<T: serde::Serialize + serde::de::DeserializeOwned + Default> Store<T> {
    /// A store in `dir`, named `file`. `None` is a store that keeps nothing.
    #[must_use]
    pub const fn at(dir: Option<PathBuf>, file: &'static str) -> Self {
        Self {
            dir,
            file,
            roots: Roots::Env,
            check: None,
            what: std::marker::PhantomData,
        }
    }

    /// The same store, with a check of the domain's own: a value serde took
    /// and the domain refuses (an id that names nothing this machine knows)
    /// is repaired like a value serde refused.
    ///
    /// # Why the check travels with the store
    ///
    /// Because a write judges the OLD file to know what the read repaired
    /// ([`Store::write`]), and it has to judge it exactly as the read did. A
    /// check handed to the read and forgotten at the write let the write take
    /// the domain's repairs for the person's values — the substitute written
    /// over the file's own answer at the first unrelated save — and a second
    /// reader with another check rewrote the ledger on every read, so a "leave
    /// it" flapped. Carried here, every read, [`Store::parsed`] and every
    /// write of this store use the one check, and there is nothing to pass.
    #[must_use]
    pub fn checked(self, check: impl Fn(&T) -> Vec<Refused> + Send + Sync + 'static) -> Self {
        Self {
            check: Some(Arc::new(check)),
            ..self
        }
    }

    /// What the domain refuses of `value` — nothing, without a check.
    fn refusals(&self, value: &T) -> Vec<Refused> {
        self.check.as_ref().map_or_else(Vec::new, |c| c(value))
    }

    /// The same store, its logical path read against these roots instead of
    /// the process's — where its repair ledger goes (`<state>/repairs/`). For
    /// a test, and for a caller whose roots are not the process's (the
    /// belongings' `Places`).
    #[must_use]
    pub fn with_roots(self, config: Option<PathBuf>, state: Option<PathBuf>) -> Self {
        Self {
            roots: Roots::Given { config, state },
            ..self
        }
    }

    /// Where it is, when it is anywhere.
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join(self.file))
    }

    /// Its logical path (`config/theme.json`) and its ledger's folder, when it
    /// lies under the roots — `None` for a store elsewhere (a test's scratch
    /// folder, the repository, a folder of root's), which keeps no ledger.
    fn ledger_place(&self) -> Option<(PathBuf, String)> {
        let path = self.path()?;
        let (config, state) = match &self.roots {
            Roots::Env => {
                let (c, s) = env_roots();
                (c.clone(), s.clone())
            }
            Roots::Given { config, state } => (config.clone(), state.clone()),
        };
        let state = state?;
        let logical = logical_of(&path, config.as_deref(), &state)?;
        Some((repair::dir_under(&state), logical))
    }

    /// Its ledger, if it has one now.
    #[must_use]
    pub fn ledger(&self) -> Option<Ledger> {
        let (book, logical) = self.ledger_place()?;
        repair::read(&book, &logical)
    }

    /// The ledger follows what a read found in `bytes`: written when there are
    /// repairs (a "leave it" kept for the same bytes and repairs), removed
    /// when there are none.
    fn note(&self, bytes: &[u8], repairs: &[Repair]) {
        let Some((book, logical)) = self.ledger_place() else {
            return;
        };
        if repairs.is_empty() {
            repair::note(&book, &logical, None);
            return;
        }
        let file_sha256 = repair::sha256_hex(bytes);
        let dismissed = repair::read(&book, &logical).is_some_and(|l| {
            l.dismissed && l.file_sha256 == file_sha256 && l.repairs.as_slice() == repairs
        });
        let ledger = Ledger {
            path: logical.clone(),
            file_sha256,
            repairs: repairs.to_vec(),
            dismissed,
        };
        repair::note(&book, &logical, Some(&ledger));
    }

    /// What is in it, repaired where it must be — or the default, with the
    /// reason logged.
    #[must_use]
    pub fn read(&self) -> T {
        self.reading().value
    }

    /// The read, whole: the value, whether a file was there, what was
    /// repaired, and why a file gave nothing — for a domain that SAYS what it
    /// refused (the theme's complaints, the language's) or that a part of the
    /// file did not read ([`Reading::part_not_read`]).
    #[must_use]
    pub fn reading(&self) -> Reading<T> {
        let nothing = |found: bool, trouble: Option<Trouble>| Reading {
            value: T::default(),
            found,
            repairs: Vec::new(),
            trouble,
        };
        let Some(path) = self.path() else {
            return nothing(false, None);
        };
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            // NOT AN ERROR: a file that has never been written is the ordinary
            // state of a fresh machine, and every one of these degrades to the
            // default anyway.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return nothing(false, None),
            Err(e) => {
                tracing::warn!(path = %path.display(), error = %e, "could not read the store");
                return nothing(true, Some(Trouble::Unreadable(e.to_string())));
            }
        };
        let doc = match Node::parse(&bytes) {
            Ok(doc) => doc,
            Err(e) => {
                // LOUD, because the person loses what was in it: their filters,
                // their adjustments. Silently starting from nothing is how a
                // day's work disappears with nothing to read afterwards.
                tracing::error!(
                    path = %path.display(),
                    error = %e,
                    "the store is not JSON; starting from the default"
                );
                return nothing(true, Some(Trouble::NotJson(e.to_string())));
            }
        };
        let read = lenient::read::<T>(doc, &|v| self.refusals(v));
        if !read.repairs.is_empty() {
            // LOUD TOO, and named: what was repaired is in the ledger, and
            // the file itself is untouched until somebody writes the fix.
            tracing::warn!(
                path = %path.display(),
                repairs = read.repairs.len(),
                places = ?read.repairs.iter().map(|r| r.pointer.as_str()).collect::<Vec<_>>(),
                "the store did not read whole; the rest of it was kept"
            );
        }
        self.note(&bytes, &read.repairs);
        Reading {
            value: read.value,
            found: true,
            repairs: read.repairs,
            trouble: None,
        }
    }

    /// The same read, but saying whether the answer came from the FILE.
    ///
    /// `None` means exactly one thing: there is a file, and NOTHING of it can
    /// be read — it is not JSON, or it is JSON that is nothing like the type.
    /// Not "no file" and not "no directory" — both of those are ordinary, and
    /// both answer `Some(T::default())`, because a machine on its first
    /// morning has no file and is not in trouble. And not "a file with a value
    /// that did not read": that is the rest of the file, repaired, and a write
    /// keeps the repaired value's original ([`Store::write`]). A domain that
    /// must refuse that too has [`Store::whole`]; one that SHOWS the file
    /// owes the person a word about it ([`Reading::part_not_read`]).
    ///
    /// # Who has to ask this instead of [`Store::read`]
    ///
    /// Whoever is about to WRITE THE WHOLE VALUE BACK. This store's promise is
    /// that a file which will not parse costs ONE feature and says so; that
    /// holds while the fallback is only read. The moment a caller reads the
    /// fallback, lays its own change over it and writes, the promise inverts:
    /// one unparseable byte plus one keystroke and the file's contents are
    /// gone for good — and, where something else is derived from it, so is
    /// that. A reader degrades. A writer must refuse.
    #[must_use]
    pub fn parsed(&self) -> Option<T> {
        let reading = self.reading();
        match reading.trouble {
            // UNREADABLE IS NOT UNPARSEABLE. A permission error says nothing
            // about the contents, and writing is what will fail next anyway,
            // with its own message.
            Some(Trouble::Unreadable(_)) => Some(T::default()),
            _ if reading.lost_everything() => None,
            _ => Some(reading.value),
        }
    }

    /// The value only when the file reads WHOLE ([`Reading::whole`]) — `None`
    /// for a file that is there and did not: not JSON, or a value of it
    /// repaired. What [`Store::parsed`] answered before reads were lenient.
    ///
    /// For a domain whose CONSUMER reads the file strictly — so a value this
    /// store would repair is a file the consumer refuses whole (the
    /// belongings' engine and `belongings.json`) — or whose missing part
    /// would be a promise silently broken (a reminder that does not ring). It
    /// keeps refusing such a file, with the sentence it always had.
    #[must_use]
    pub fn whole(&self) -> Option<T> {
        let reading = self.reading();
        reading.whole().then_some(reading.value)
    }

    /// Writes it whole, atomically. A store that keeps nothing writes nothing.
    ///
    /// # What it keeps of the file it replaces
    ///
    /// The file is read again first — judged with the store's own check, as
    /// every read of it is ([`Store::checked`]) — and the value goes on disk
    /// with:
    ///
    /// - every key the type does not know put back where the value lacks it —
    ///   a newer machine's field survives an older machine's write;
    /// - every repair a read made put back where the writer left the value as
    ///   the read gave it — a repair nobody accepted is never written, and a
    ///   value the writer DID change is the writer's;
    /// - the file's key order.
    ///
    /// Then the ledger follows the file it wrote.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn write(&self, value: &T) -> std::io::Result<()> {
        let Some(dir) = self.dir.as_ref() else {
            return Ok(());
        };
        let check = |v: &T| self.refusals(v);
        let new = Node::of(value).map_err(std::io::Error::other)?;
        let old = std::fs::read(dir.join(self.file))
            .ok()
            .and_then(|b| Node::parse(&b).ok());
        let (out, judged) = match old {
            Some(old) => {
                let was = lenient::read::<T>(old.clone(), &check);
                let judged = !was.repairs.is_empty();
                (lenient::kept_over(new, &old, &was), judged)
            }
            None => (new, false),
        };
        let bytes = out.pretty();
        write_atomically(dir, self.file, &bytes)?;
        // THE LEDGER FOLLOWS THE FILE: a write over a file with repairs may
        // keep some of them (the untouched ones), so it is judged again; one
        // over a file that read whole leaves nothing to repair.
        if judged {
            let again = lenient::read::<T>(out, &check);
            self.note(&bytes, &again.repairs);
        } else {
            self.note(&bytes, &[]);
        }
        Ok(())
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
    fn a_writer_can_tell_a_broken_file_from_a_missing_one() {
        // THE DISTINCTION A WRITER NEEDS, and the one `read` cannot make: it
        // answers the same empty value for "no file yet" and for "a file I
        // could not understand". A reader may treat those alike and degrade. A
        // caller that reads, changes and writes back must not: laying its own
        // change over the fallback and writing turns one unparseable byte into
        // the permanent loss of everything the file held.
        let dir = scratch("parsed");
        let s: Store<Vec<String>> = Store::at(Some(dir.clone()), "y.json");

        // NO FILE is not trouble — it is every machine's first morning.
        assert_eq!(s.parsed(), Some(Vec::new()));

        // A GOOD FILE comes back as itself.
        s.write(&vec!["one".to_owned()]).unwrap();
        assert_eq!(s.parsed(), Some(vec!["one".to_owned()]));

        // A BROKEN FILE is the one case that answers `None` — while `read`
        // still degrades, because the two have different jobs.
        std::fs::write(dir.join("y.json"), b"{ not json").unwrap();
        assert_eq!(s.parsed(), None);
        assert!(s.read().is_empty());

        // AND A STORE THAT KEEPS NOTHING is not "broken" either: there is no
        // file to lose, so a writer may proceed and write nowhere.
        let nowhere: Store<Vec<String>> = Store::at(None, "y.json");
        assert_eq!(nowhere.parsed(), Some(Vec::new()));

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

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Borders {
        #[default]
        Standard,
        Discreet,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Choice {
        theme: String,
        borders: Borders,
        size: u16,
    }

    /// A store under a configuration root, whose ledger goes under the state
    /// root beside it — both inside a scratch folder.
    fn rooted(tag: &str) -> (PathBuf, Store<Choice>) {
        rooted_as(tag, "theme.json")
    }

    /// [`rooted`], of any type and file.
    fn rooted_as<T>(tag: &str, file: &'static str) -> (PathBuf, Store<T>)
    where
        T: serde::Serialize + serde::de::DeserializeOwned + Default,
    {
        let root = scratch(tag);
        let config = root.join("config");
        let state = root.join("state");
        std::fs::create_dir_all(&config).unwrap();
        let store = Store::at(Some(config.clone()), file).with_roots(Some(config), Some(state));
        (root, store)
    }

    #[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Shelf {
        id: String,
        borders: Borders,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Shelves {
        theme: String,
        borders: Borders,
        size: u16,
        shelves: Vec<Shelf>,
    }

    /// FAILS IF: a file with one unknown variant, one value of the wrong
    /// type and one element of a list that will not read, read through a
    /// `Store` under the person's roots, loses anything else — or its LEDGER
    /// (the file under `state/repairs/`, what the belongings' page lists)
    /// does not list exactly those three, each with what the file had.
    #[test]
    fn a_store_keeps_the_rest_of_a_broken_file() {
        let (root, store) = rooted_as::<Shelves>("broken", "shelves.json");
        std::fs::write(
            root.join("config/shelves.json"),
            br#"{
              "theme": "owl",
              "borders": "Hairline",
              "size": "large",
              "shelves": [
                {"id": "a", "borders": "discreet"},
                {"id": "b", "borders": 7},
                {"id": "c", "borders": "standard"}
              ]
            }"#,
        )
        .unwrap();
        let read = store.read();
        assert_eq!(read.theme, "owl", "a good field was lost");
        assert_eq!(read.borders, Borders::Standard);
        assert_eq!(read.size, 0);
        let ids: Vec<&str> = read.shelves.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["a", "c"], "a good element was lost");
        assert_eq!(read.shelves[0].borders, Borders::Discreet);

        let ledger = store.ledger().expect("no ledger was written");
        assert_eq!(ledger.path, "config/shelves.json");
        let lines: Vec<(&str, repair::Why, String)> = ledger
            .repairs
            .iter()
            .map(|r| (r.pointer.as_str(), r.why, r.found.to_string()))
            .collect();
        assert_eq!(
            lines,
            [
                (
                    "/borders",
                    repair::Why::UnknownVariant,
                    "\"Hairline\"".to_owned()
                ),
                (
                    "/shelves/1",
                    repair::Why::WrongType,
                    r#"{"id":"b","borders":7}"#.to_owned()
                ),
                ("/size", repair::Why::WrongType, "\"large\"".to_owned()),
            ],
            "the ledger is not those three"
        );
        assert_eq!(
            ledger.file_sha256,
            repair::sha256_hex(&std::fs::read(root.join("config/shelves.json")).unwrap())
        );
        assert!(!ledger.dismissed);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(rename_all = "kebab-case")]
    enum Transform {
        #[default]
        Normal,
        Flipped,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Screen {
        transform: Transform,
        scale: u8,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
    #[serde(default)]
    struct Screens {
        screens: std::collections::BTreeMap<String, Screen>,
        text: u8,
    }

    /// The second half of [`an_untouched_repair_is_not_written`]: a newer
    /// machine's value INSIDE a map's value — the type's default has no
    /// screen, so the read takes the value out and the type writes its own
    /// default back — erased from the file by a write that changed another
    /// screen, or the ledger no longer listing it.
    fn a_repaired_screen_survives_a_write_of_another() {
        let (root, store) = rooted_as::<Screens>("screens", "display.json");
        let file = root.join("config/display.json");
        std::fs::write(
            &file,
            br#"{"screens": {"DEL U2720Q": {"transform": "flipped-90", "scale": 2}, "LG": {"scale": 1}}, "text": 11}"#,
        )
        .unwrap();
        let mut prefs = store.parsed().expect("the rest of the file reads");
        assert_eq!(prefs.screens["DEL U2720Q"].transform, Transform::Normal);
        prefs.screens.get_mut("LG").unwrap().scale = 3;
        store.write(&prefs).unwrap();
        let doc = Node::parse(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(
            doc.at("/screens/DEL U2720Q/transform"),
            Some(&Node::String("flipped-90".to_owned())),
            "the newer machine's transform was erased: {doc}"
        );
        assert_eq!(doc.at("/screens/LG/scale"), Some(&Node::Number(3.into())));
        let ledger = store.ledger().expect("the repair is still listed");
        assert_eq!(ledger.repairs.len(), 1);
        assert_eq!(ledger.repairs[0].pointer, "/screens/DEL U2720Q/transform");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the domain's check is not the one every read AND write of
    /// the store uses — a write that judged the old file without it took the
    /// domain's repair for the person's value and wrote the substitute over
    /// it — or reading twice rewrites the ledger (a "leave it" flapping).
    #[test]
    fn the_domains_check_travels_with_the_store() {
        let (root, store) = rooted("checked");
        let store = store.checked(|c: &Choice| {
            if ["owl", "lark"].contains(&c.theme.as_str()) {
                Vec::new()
            } else {
                vec![Refused {
                    pointer: "/theme".to_owned(),
                    why: repair::Why::UnknownVariant,
                    kept: Some(Node::String("owl".to_owned())),
                }]
            }
        });
        let file = root.join("config/theme.json");
        std::fs::write(&file, br#"{"theme": "heron", "size": 3}"#).unwrap();
        let mut c = store.read();
        assert_eq!(c.theme, "owl", "the domain's refusal was not repaired");
        let ledger = store.ledger().expect("a ledger");
        repair::dismiss(&root.join("state/repairs"), &file, &ledger).unwrap();
        // ANOTHER READER of the same bytes — a clone of the store, as a second
        // door holds one: the same judgement, so the answer stays.
        let other = store.clone();
        let _ = other.read();
        drop(other);
        assert!(store.ledger().unwrap().dismissed, "the ledger flapped");
        // A WRITE OF ANOTHER VALUE keeps the person's word.
        c.size = 9;
        store.write(&c).unwrap();
        let doc = Node::parse(&std::fs::read(&file).unwrap()).unwrap();
        assert_eq!(
            doc.at("/theme"),
            Some(&Node::String("heron".to_owned())),
            "the domain's substitute was written over the file's value"
        );
        assert_eq!(doc.at("/size"), Some(&Node::Number(9.into())));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: `whole` takes a file with a repaired value (what a strict
    /// consumer refuses), or refuses a file that reads as written.
    #[test]
    fn whole_is_only_a_file_read_as_written() {
        let (root, store) = rooted("whole");
        let file = root.join("config/theme.json");
        assert_eq!(
            store.whole(),
            Some(Choice::default()),
            "no file is not broken"
        );
        std::fs::write(&file, br#"{"theme": "owl", "borders": "Hairline"}"#).unwrap();
        assert_eq!(store.whole(), None);
        assert!(store.parsed().is_some());
        let reading = store.reading();
        assert!(reading.part_not_read() && !reading.whole());
        std::fs::write(&file, br#"{"theme": "owl", "future": 1}"#).unwrap();
        assert_eq!(store.whole().map(|c| c.theme), Some("owl".to_owned()));
        std::fs::write(&file, b"{ not json").unwrap();
        assert_eq!(store.whole(), None);
        assert!(!store.reading().part_not_read(), "nothing of it was kept");
        let _ = std::fs::remove_dir_all(&root);
    }

    fn on_disk(root: &Path) -> Node {
        Node::parse(&std::fs::read(root.join("config/theme.json")).unwrap()).unwrap()
    }

    /// FAILS IF: an older machine's write — a value whose type does not know
    /// a field a newer machine wrote — erases that field, or reorders the
    /// file the person wrote.
    #[test]
    fn a_newer_field_survives_an_older_write() {
        let (root, store) = rooted("newer");
        std::fs::write(
            root.join("config/theme.json"),
            br#"{"size": 3, "shade": {"warmth": 7}, "theme": "owl", "borders": "standard"}"#,
        )
        .unwrap();
        let mut c = store.read();
        assert_eq!(c.theme, "owl");
        assert!(store.ledger().is_none(), "an unknown field is not a repair");
        c.size = 4;
        store.write(&c).unwrap();
        let doc = on_disk(&root);
        assert_eq!(
            doc.at("/shade/warmth"),
            Some(&Node::Number(7.into())),
            "the newer field was erased"
        );
        assert_eq!(doc.at("/size"), Some(&Node::Number(4.into())));
        assert!(
            doc.to_string().starts_with(r#"{"size":4,"shade""#),
            "the person's order was not kept: {doc}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a write of a value the person did not change at a repaired
    /// place writes the repair (the default it substituted) over what the
    /// file said — at the top of the file or inside a map's value, where the
    /// type writes its own default — or the ledger does not follow: listed
    /// after the read, kept "left" for the same bytes, gone once a write
    /// overrides it.
    #[test]
    fn an_untouched_repair_is_not_written() {
        let (root, store) = rooted("untouched");
        std::fs::write(
            root.join("config/theme.json"),
            br#"{"theme": "owl", "borders": "Hairline", "size": 3}"#,
        )
        .unwrap();
        let mut c = store.read();
        assert_eq!(c.borders, Borders::Standard, "the read repairs");
        let ledger = store.ledger().expect("a ledger for a repaired read");
        assert_eq!(ledger.path, "config/theme.json");
        assert_eq!(ledger.repairs.len(), 1);
        assert_eq!(ledger.repairs[0].pointer, "/borders");
        assert!(
            root.join("state/repairs")
                .join(repair::file_name("config/theme.json"))
                .is_file()
        );

        // THE PERSON CHANGED THE SIZE, and nothing else: "Hairline" stays.
        c.size = 9;
        store.write(&c).unwrap();
        let doc = on_disk(&root);
        assert_eq!(
            doc.at("/borders"),
            Some(&Node::String("Hairline".to_owned())),
            "an untouched repair was written"
        );
        assert_eq!(doc.at("/size"), Some(&Node::Number(9.into())));
        // The ledger follows the file it wrote: still that one repair.
        let after = store.ledger().expect("the repair is still there");
        assert_eq!(after.repairs, ledger.repairs);

        // "LEAVE IT" is remembered for these bytes, across reads.
        let file = root.join("config/theme.json");
        repair::dismiss(&root.join("state/repairs"), &file, &after).unwrap();
        let _ = store.read();
        assert!(
            store.ledger().unwrap().dismissed,
            "a read forgot the person's answer"
        );

        // A WRITER THAT CHOOSES the repaired value is the person's choice:
        // written, and nothing is left to repair.
        c.borders = Borders::Discreet;
        store.write(&c).unwrap();
        assert_eq!(
            on_disk(&root).at("/borders"),
            Some(&Node::String("discreet".to_owned()))
        );
        assert!(store.ledger().is_none(), "the ledger outlived its repair");
        let _ = std::fs::remove_dir_all(&root);

        // AND WHERE THE TYPE'S DEFAULT HAS NOTHING — a value of a map.
        a_repaired_screen_survives_a_write_of_another();
    }

    #[test]
    fn a_store_outside_the_roots_keeps_no_ledger() {
        // A TEST'S SCRATCH FOLDER is under neither of the process's roots, so
        // a repaired read there writes nothing anywhere else.
        let dir = scratch("no-ledger");
        let s: Store<Choice> = Store::at(Some(dir.clone()), "x.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("x.json"), br#"{"borders": 7}"#).unwrap();
        let r = s.reading();
        assert_eq!(r.repairs.len(), 1);
        assert!(s.ledger().is_none());
        // JSON that is nothing like the type is not something to write over.
        std::fs::write(dir.join("x.json"), b"[1, 2]").unwrap();
        assert_eq!(s.parsed(), None);
        // A value that did not read is: the rest of the file is there.
        std::fs::write(dir.join("x.json"), br#"{"borders": 7, "size": 2}"#).unwrap();
        assert_eq!(s.parsed().map(|c| c.size), Some(2));
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
