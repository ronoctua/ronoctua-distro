//! Reading a directory, and the order it comes out in.
//!
//! # Hidden files are SHOWN, always, and that is the owner's rule
//!
//! Unix's only definition of hidden is "the name starts with a dot", and it
//! exists because an early `ls` filtered `.` and `..` by prefix. Everything
//! built on top of that accident is convention. Here the whole directory is
//! shown and there is no key that hides part of it — a file manager that hides
//! half of `$HOME` is a file manager one cannot use to fix a dotfile, and a
//! toggle is a way to end up in that state without knowing it.

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
        use crate::key;
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
    // THE /private VAULT, READ ONLY WHILE IT IS OPEN, and then off the
    // caller's thread (`guarded`). Refused, the error IS a [`Withheld`].
    guarded(dir, move |at| read_plainly(&at, sort))?
}

/// [`read_dir_at`] itself, for a folder that may be read where the caller
/// stands.
fn read_plainly(dir: &Path, sort: Sort) -> Result<Vec<Entry>> {
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
    // AND THE WAY BACK IS THE FIRST ROW, whatever the sort says.
    //
    // `read_dir` never yields `.` or `..` — that is a property of the syscall,
    // not an accident — so it is put here, and AFTER the sort: it is not one
    // of the names being ordered. It is the way out of the directory, and a
    // way out that moved to the middle of the list when somebody sorted by
    // size would be a way out one has to look for.
    //
    // At the root there is nowhere to go, so there is no row. A row that did
    // nothing would be the one thing in this listing that lies.
    if let Some(up) = dir
        .parent()
        .and_then(|p| Entry::of_path(Entry::PARENT.to_owned(), p))
    {
        out.insert(0, up);
    }
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
    // WHAT IS DRAWN, which is what a person sorts by eye: a trashed file from a
    // stick sorts among the others by its name, not by where the stick is.
    ronoctua_apps::fold::fold(e.shown())
}

/// A directory's entries WITHOUT the way back — for a listing that gathers
/// several directories into one (the merged trash), where each one's `..`
/// would be a way out of somewhere the person is not.
///
/// Each name is the entry's WHOLE PATH (see [`Entry::shown`]).
///
/// # Errors
///
/// When the directory cannot be opened.
pub fn gathered_from(dir: &Path) -> Result<Vec<Entry>> {
    // THE OPEN VAULT'S OWN BIN TOO, by the same guard (`guarded`).
    guarded(dir, |at| {
        let handle =
            std::fs::read_dir(&at).with_context(|| format!("could not list {}", at.display()))?;
        Ok(handle
            .flatten()
            .filter_map(|e| {
                let path = e.path();
                Entry::of_path(path.to_string_lossy().into_owned(), &path)
            })
            .collect())
    })?
}

/// The directory's own modification time, for "has this changed since I looked".
///
/// The same shape as `ronoctua_shell_door::launch::stamp`: one `stat` answers whether
/// a re-read is worth it, and a re-read that finds nothing new is a frame spent
/// for nothing.
#[must_use]
pub fn stamp(dir: &Path) -> Option<std::time::SystemTime> {
    // NOR STAT'ED but by the same guard: see `read_dir_at`.
    guarded(dir, |at| std::fs::metadata(&at).ok()?.modified().ok())
        .ok()
        .flatten()
}

// ─── the /private vault ─────────────────────────────────────────────────────

/// Why a folder inside the `/private` vault was not read. A door says it in
/// its own words ([`Withheld::sentence`]); this text is for the journal
/// only. It travels inside the `anyhow::Error` a listing returns:
/// [`withheld`] takes it out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Withheld {
    /// The vault is not open — locked, or suspended at sleep while in use:
    /// it is not touched at all.
    Shut,
    /// It is open, but did not answer within [`VAULT_PATIENCE`]: suspended
    /// an instant after its status was read. The read waits on its own
    /// thread until the passphrase resumes the vault; the door goes on.
    Silent,
}

impl Withheld {
    /// The sentence a door says instead of an empty listing.
    #[must_use]
    pub const fn sentence(self) -> &'static str {
        match self {
            Self::Shut => crate::key::FILES_PRIVATE_WITHHELD_SHUT,
            Self::Silent => crate::key::FILES_PRIVATE_WITHHELD_SILENT,
        }
    }
}

impl std::fmt::Display for Withheld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Shut => "the /private vault is not open",
            Self::Silent => "the /private vault did not answer",
        })
    }
}

impl std::error::Error for Withheld {}

/// The [`Withheld`] inside a listing's error, when that is why it failed.
#[must_use]
pub fn withheld(error: &anyhow::Error) -> Option<Withheld> {
    error.downcast_ref::<Withheld>().copied()
}

/// How long a door waits for a read inside the open vault before it says
/// the vault is not answering. A folder of the vault reads in microseconds
/// from the page cache and in milliseconds from the disk; what takes longer
/// is a vault frozen under the read.
pub const VAULT_PATIENCE: std::time::Duration = std::time::Duration::from_millis(400);

/// Reads inside the vault that outlived their wait and have not come back.
/// While one is out, the vault is taken for frozen: a further read is
/// refused at once instead of stranding a second thread beside it.
static STRANDED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Runs `read` on `dir` — the vault's guard every listing goes through.
///
/// Outside `/private` (once links are followed: `ronoctua_private::inside`,
/// which never looks inside the vault), `read` runs right here. Inside, the
/// vault must be OPEN (one read of its status file, which root rewrites
/// before it suspends), and then `read` runs on a thread of its own that the
/// caller waits for [`VAULT_PATIENCE`] at most: the status said "open" an
/// instant before the sleep hook suspended it, and a door's event loop that
/// read it itself would wait in the kernel until the passphrase — asked on
/// that same loop — resumed it.
fn guarded<T: Send + 'static>(
    dir: &Path,
    read: impl FnOnce(PathBuf) -> T + Send + 'static,
) -> std::result::Result<T, Withheld> {
    guarded_by(
        dir,
        Path::new(ronoctua_private::MOUNT_POINT),
        Path::new(ronoctua_private::STATUS_FILE),
        VAULT_PATIENCE,
        read,
    )
}

/// [`guarded`], with the vault's two places and the patience handed in.
fn guarded_by<T: Send + 'static>(
    dir: &Path,
    mount_point: &Path,
    status_file: &Path,
    patience: std::time::Duration,
    read: impl FnOnce(PathBuf) -> T + Send + 'static,
) -> std::result::Result<T, Withheld> {
    if !ronoctua_private::inside_with(dir, mount_point) {
        return Ok(read(dir.to_path_buf()));
    }
    if !ronoctua_private::is_open(status_file) {
        return Err(Withheld::Shut);
    }
    off_the_caller(dir.to_path_buf(), patience, read)
}

/// `read(dir)` on a thread, waited for `patience` at most. One that does not
/// come back in time is left to finish when it can, and counted
/// ([`STRANDED`]) until it does.
fn off_the_caller<T: Send + 'static>(
    dir: PathBuf,
    patience: std::time::Duration,
    read: impl FnOnce(PathBuf) -> T + Send + 'static,
) -> std::result::Result<T, Withheld> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU8, Ordering};
    const RUNNING: u8 = 0;
    const DONE: u8 = 1;
    const LEFT: u8 = 2;

    /// Marks the read done when the thread ends — returned or unwound — and
    /// gives its place back when the caller had left it behind.
    struct Ending(Arc<AtomicU8>);
    impl Drop for Ending {
        fn drop(&mut self) {
            if self.0.swap(DONE, Ordering::AcqRel) == LEFT {
                STRANDED.fetch_sub(1, Ordering::AcqRel);
            }
        }
    }

    if STRANDED.load(Ordering::Acquire) > 0 {
        return Err(Withheld::Silent);
    }
    let state = Arc::new(AtomicU8::new(RUNNING));
    let (send, came) = std::sync::mpsc::sync_channel(1);
    let ending = Ending(Arc::clone(&state));
    let spawned = std::thread::Builder::new()
        .name("ronoctua-vault-read".to_owned())
        .spawn(move || {
            let _ending = ending;
            let _ = send.send(read(dir));
        });
    if spawned.is_err() {
        return Err(Withheld::Silent);
    }
    match came.recv_timeout(patience) {
        Ok(value) => Ok(value),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Err(Withheld::Silent),
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            // COUNTED BEFORE IT IS LEFT, so a thread that ends in between
            // finds the count to give back.
            STRANDED.fetch_add(1, Ordering::AcqRel);
            if state
                .compare_exchange(RUNNING, LEFT, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                Err(Withheld::Silent)
            } else {
                // IT ENDED in that instant: its answer is already sent.
                STRANDED.fetch_sub(1, Ordering::AcqRel);
                came.recv().map_err(|_| Withheld::Silent)
            }
        }
    }
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
    fn a_gathered_entry_is_its_whole_path_and_sorts_by_its_name() {
        let dir = std::env::temp_dir().join(format!("ronoctua-gathered-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("zeta.txt"), b"z").unwrap();
        let found = gathered_from(&dir).unwrap();
        // NO WAY BACK: a gathered listing has no one folder to go up from.
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, dir.join("zeta.txt").to_string_lossy());
        assert_eq!(found[0].shown(), "zeta.txt");
        // And `join` with it is the file, which is what every door computes.
        assert_eq!(
            Path::new("/elsewhere").join(&found[0].name),
            dir.join("zeta.txt")
        );
        // SORTED BY WHAT IS DRAWN: `/tmp/...zeta` does not come before `alpha`.
        let mut both = vec![found[0].clone(), entry("alpha.txt", Kind::File, 1, 0)];
        sort_entries(&mut both, Sort::default());
        assert_eq!(both[0].shown(), "alpha.txt");
        let _ = std::fs::remove_dir_all(&dir);
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
        let parts = ancestry(Path::new("/home/somebody/main"));
        let labels: Vec<_> = parts.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, ["/", "home", "somebody", "main"]);
        assert_eq!(parts[2].1, PathBuf::from("/home/somebody"));
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

    /// FAILS IF: a folder of a vault that is not open is read (through a
    /// link too), an open one's is not, or a refusal stops being a
    /// [`Withheld`] the door can word.
    /// The two tests below share [`STRANDED`]: one at a time.
    static VAULT_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn the_vault_is_read_only_while_it_is_open() {
        let _one = VAULT_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let root = std::env::temp_dir().join(format!("ronoctua-withheld-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let mount = root.join("private");
        let home = root.join("home");
        std::fs::create_dir_all(mount.join("notes")).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(mount.join("notes/kept.txt"), b"k").unwrap();
        std::os::unix::fs::symlink(&mount, home.join("Private")).unwrap();
        let status = root.join("status.json");
        let state = |s: ronoctua_private::State| {
            let written = ronoctua_private::Status {
                state: s,
                ..ronoctua_private::Status::absent()
            };
            std::fs::write(&status, serde_json::to_vec(&written).unwrap()).unwrap();
        };
        let list = |dir: &Path| {
            guarded_by(
                dir,
                &mount,
                &status,
                std::time::Duration::from_secs(5),
                |at| read_plainly(&at, Sort::default()),
            )
        };
        for shut in [
            ronoctua_private::State::Locked,
            ronoctua_private::State::SuspendedInUse,
        ] {
            state(shut);
            assert_eq!(list(&mount.join("notes")).err(), Some(Withheld::Shut));
            assert_eq!(
                list(&home.join("Private/notes")).err(),
                Some(Withheld::Shut)
            );
            assert!(list(&home).is_ok(), "outside is read");
        }
        state(ronoctua_private::State::Open);
        let read = list(&home.join("Private/notes")).unwrap().unwrap();
        assert!(read.iter().any(|e| e.name == "kept.txt"), "{read:?}");
        // AND IT TRAVELS AS ITSELF inside the listing's error.
        let e: anyhow::Error = Withheld::Silent.into();
        assert_eq!(withheld(&e), Some(Withheld::Silent));
        assert_eq!(withheld(&anyhow::anyhow!("no")), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a read inside the vault that does not come back holds the
    /// caller past its patience, a second one is started beside it, or the
    /// vault stays refused once the first came back.
    #[test]
    fn a_frozen_read_is_left_behind_and_the_next_refused_at_once() {
        use std::time::{Duration, Instant};
        let _one = VAULT_TESTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (go, held) = std::sync::mpsc::channel::<()>();
        let started = Instant::now();
        let first = off_the_caller(PathBuf::from("/x"), Duration::from_millis(50), move |_| {
            let _ = held.recv();
            1
        });
        assert_eq!(first, Err(Withheld::Silent));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the caller waited"
        );
        // WHILE IT IS OUT: refused at once, nothing started.
        let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let r = std::sync::Arc::clone(&ran);
        let second = off_the_caller(PathBuf::from("/x"), Duration::from_secs(5), move |_| {
            r.store(true, std::sync::atomic::Ordering::SeqCst);
            2
        });
        assert_eq!(second, Err(Withheld::Silent));
        assert!(
            !ran.load(std::sync::atomic::Ordering::SeqCst),
            "a second read was started"
        );
        // THE VAULT RESUMED: the first read ends, and the next one is read.
        go.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while STRANDED.load(std::sync::atomic::Ordering::Acquire) > 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            off_the_caller(PathBuf::from("/x"), Duration::from_secs(5), |_| 3),
            Ok(3)
        );
    }
}
