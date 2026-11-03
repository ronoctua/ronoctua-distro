//! The Claude Code sessions this machine runs, as the managed hook tells
//! them — the one folder of them, the record, and the one reader.
//!
//! # Where they come from
//!
//! The distro declares a managed drop-in for Claude Code
//! (`/etc/claude-code/managed-settings.d/ronoctua.json`, `nix/modules/llm.nix`)
//! that runs `ronoctua-llm hook` on a session's events, for every account at
//! once ([`crate::hook`]). Each run writes the WHOLE record of its session —
//! never a delta — as `<session>.json` in [`folder`]: `$XDG_RUNTIME_DIR/
//! <name>-agents/` (`ronoctua-agents`, or `ronoctua-dev-agents` for the
//! development loop's sessions), a folder `0700` of files `0600`, gone with
//! the login like the sessions are.
//!
//! # What a record says
//!
//! Who the session is (its id, the account folder it runs in, our terminal
//! when it runs in one), whether it is still the process it was (the pid AND
//! the moment the kernel started it: a pid is handed on once its process is
//! reaped, and `kill(pid, 0)` then answers yes for a stranger — [`running`]),
//! where it works, and its [`Status`] as the hook last heard it. A session
//! that ended stays a TOMBSTONE (`ended`) until a later session's start
//! sweeps it away ([`sweep`]) — and not before no word older than its end can
//! still come: a late event of the session that ended then finds a record
//! that says so. A `/clear` ends one session and starts the next in the same
//! process at once, with the old session's last runs maybe still waiting for
//! the lock; swept then, the tombstone would let one of them write the ended
//! session back as a live agent of a process that lives on
//! ([`TOMBSTONE_KEPT_NS`]).
//!
//! # Newer wins, under one lock
//!
//! Claude Code runs its hooks in the background, so two runs for one session
//! can finish in either order. Each is stamped when it starts
//! ([`Session::said_at_ns`]) and writes only over an older word, holding the
//! folder's lock while it reads and writes ([`Lock`]); a reader never takes
//! it, since every write is a rename.
//!
//! # Read bounded
//!
//! At most [`MOST_FILES`] records of at most [`MOST_BYTES`] each, every one
//! checked whole (`deny_unknown_fields`, its format, its name): a record that
//! does not read is not an agent, and nothing is guessed of it.

use std::fs::File;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The runtime folder's leaf: `$XDG_RUNTIME_DIR/<name>-agents`.
const LEAF: &str = "agents";

/// The file the writers lock, in the folder — a leading dot, so no reader
/// takes it for a record.
const LOCK_FILE: &str = ".lock";

/// What every record's file name ends with.
const SUFFIX: &str = ".json";

/// The format written, and the only one read.
pub const FORMAT: u32 = 1;

/// The most records [`read`] reads: far more sessions than one person runs,
/// and a bound on a folder something else filled.
pub const MOST_FILES: usize = 256;

/// The most one record may weigh: three paths and a few numbers.
pub const MOST_BYTES: u64 = 16 * 1024;

/// The most entries a [`sweep`] looks at.
const MOST_SWEPT: usize = 4 * MOST_FILES;

/// How long a tombstone outlives its session's end while the session's
/// process still runs: far past the drop-in's 5 s timeout
/// (`nix/modules/llm.nix`), at which Claude Code kills a run that is still
/// going — so by then every run that started before the end has written or
/// is gone. A process that is gone writes nothing more ([`crate::hook`] finds
/// no Claude Code above it), and its tombstone goes at once.
pub(crate) const TOMBSTONE_KEPT_NS: u64 = 60 * 1_000_000_000;

/// The longest a session's id may be.
const MOST_SESSION_CHARS: usize = 64;

/// How many characters our terminals' ids have: sixteen lowercase hex, the
/// one form the terminals' census writes and the hook takes.
pub const TERMINAL_ID_CHARS: usize = 16;

/// A session's status, as the hook last heard it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// It is working on what it was asked.
    Working,
    /// It waits for the person: a permission, a question.
    Waiting,
    /// It is ready for the next prompt — and so is a session that has just
    /// started and was asked nothing yet.
    Done,
    /// Its last answer failed (an API error ended the turn).
    Failed,
}

impl Status {
    /// Every one.
    pub const ALL: [Self; 4] = [Self::Working, Self::Waiting, Self::Done, Self::Failed];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Waiting => "waiting",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }
}

/// One session's record, `<folder>/<session>.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Session {
    /// Always [`FORMAT`].
    pub format: u32,
    /// Claude Code's id of the session ([`fine_session`]), also the file's
    /// name.
    pub session: String,
    /// The account's folder it runs in: its `CLAUDE_CONFIG_DIR`, or
    /// `~/.claude` when that is not set — what maps it to an account.
    pub account_dir: PathBuf,
    /// Claude Code's process.
    pub pid: u32,
    /// When the kernel started that process, in clock ticks since boot: what
    /// tells it from a later process handed the same pid ([`running`]).
    pub pid_start_ticks: u64,
    /// Our terminal it runs in (`RONOCTUA_TERMINAL`, [`fine_terminal`]);
    /// `None` in any other.
    pub terminal: Option<String>,
    /// The folder it works in.
    pub folder: PathBuf,
    /// Its transcript, as Claude Code names it.
    pub transcript: Option<PathBuf>,
    /// What it is doing, as the hook last heard.
    pub status: Status,
    /// When the run that wrote this started, in nanoseconds since the epoch:
    /// a word older than the record's never replaces it.
    pub said_at_ns: u64,
    /// When this process of the session was first heard, in nanoseconds
    /// since the epoch — what keeps an agent's place among the others.
    pub started_at_ns: u64,
    /// Whether the session ended (a tombstone).
    pub ended: bool,
}

/// This instance's folder: `$XDG_RUNTIME_DIR/<name>-agents`, or `None`
/// without a runtime folder — never one in `/tmp`, which anybody can read.
#[must_use]
pub fn folder() -> Option<PathBuf> {
    ronoctua_xdg::runtime_file(LEAF)
}

/// Makes `folder` private (`0700`), or makes sure it is: a folder that is
/// there with a looser mode is tightened, and anything else in its place — a
/// file, a link — is refused, never written through.
///
/// # Errors
///
/// When it cannot be made (no runtime folder above it), or something that
/// is not a folder is in its place.
pub fn make_folder(folder: &Path) -> std::io::Result<()> {
    match std::fs::DirBuilder::new().mode(0o700).create(folder) {
        Ok(()) => return Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    let meta = std::fs::symlink_metadata(folder)?;
    if !meta.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "something that is not a folder is where the sessions' folder goes",
        ));
    }
    if meta.permissions().mode() & 0o777 != 0o700 {
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Whether `id` may name a session — and a file: lowercase hex and `-`, as
/// Claude Code's ids are (UUIDs), at most 64. Anything else is not written,
/// and not read: it would be a path.
#[must_use]
pub fn fine_session(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MOST_SESSION_CHARS
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')
}

/// Whether `id` is one of our terminals' ids: [`TERMINAL_ID_CHARS`]
/// lowercase hex.
#[must_use]
pub fn fine_terminal(id: &str) -> bool {
    id.len() == TERMINAL_ID_CHARS
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether `pid` is still the process that started at `start_ticks` — the
/// machine's answer, from `/proc` (`ronoctua_memory::lineage`).
#[must_use]
pub fn running(pid: u32, start_ticks: u64) -> bool {
    ronoctua_memory::lineage::is_running(pid, start_ticks)
}

/// Whether two folders are one, as written: the same components, whatever
/// the doubled or trailing slashes and `.` in between. Lexical, never the
/// disk's — an account's folder is compared as the person set it, and a
/// reader asking the disk would be no longer pure.
#[must_use]
pub fn same_folder(a: &Path, b: &Path) -> bool {
    a.components().eq(b.components())
}

/// Every record in `folder` that reads, by session — at most [`MOST_FILES`]
/// of them, each at most [`MOST_BYTES`]. A folder that is not there has
/// none.
#[must_use]
pub fn read(folder: &Path) -> Vec<Session> {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut out: Vec<Session> = entries
        .flatten()
        .filter_map(|e| session_of_name(&e.file_name()))
        .take(MOST_FILES)
        .filter_map(|session| read_one(folder, &session))
        .collect();
    out.sort_by(|a, b| a.session.cmp(&b.session));
    out
}

/// The session a file's name says, when it is a record's: `<session>.json`,
/// the session fine.
fn session_of_name(name: &std::ffi::OsStr) -> Option<String> {
    let session = name.to_str()?.strip_suffix(SUFFIX)?;
    fine_session(session).then(|| session.to_owned())
}

/// `<folder>/<session>.json`, for a session that may name a file.
fn record_path(folder: &Path, session: &str) -> Option<PathBuf> {
    fine_session(session).then(|| folder.join(format!("{session}{SUFFIX}")))
}

/// One session's record, when it is there and reads.
#[must_use]
pub(crate) fn read_one(folder: &Path, session: &str) -> Option<Session> {
    let bytes = crate::files::read_bounded(&record_path(folder, session)?, MOST_BYTES).ok()?;
    parse(&bytes, session)
}

/// A record, read whole: this format, the session its file is named for,
/// and a terminal that is one of ours.
fn parse(bytes: &[u8], session: &str) -> Option<Session> {
    serde_json::from_slice::<Session>(bytes).ok().filter(|s| {
        s.format == FORMAT
            && s.session == session
            && s.terminal.as_deref().is_none_or(fine_terminal)
    })
}

/// Writes `record` whole, `0600`, by a rename.
///
/// # Errors
///
/// For a session that may not name a file, a record past [`MOST_BYTES`]
/// (the reader would never read it), or a write that fails.
pub(crate) fn write(folder: &Path, record: &Session) -> std::io::Result<()> {
    let refused = |why: &str| std::io::Error::new(std::io::ErrorKind::InvalidInput, why.to_owned());
    let path = record_path(folder, &record.session).ok_or_else(|| refused("not a session"))?;
    let bytes = serde_json::to_vec_pretty(record).map_err(|e| refused(&e.to_string()))?;
    if bytes.len() as u64 > MOST_BYTES {
        return Err(refused("a record larger than the reader reads"));
    }
    crate::files::write_private(&path, &bytes)
}

/// The writers' lock on a folder, held until it is dropped: one writer at a
/// time reads a record and writes it back — and tells an API key's credits
/// ([`crate::hook`]) — so no word is lost between a read and a write.
///
/// `flock`, which the kernel lets go when the process ends however it ends;
/// the file is never deleted (deleting a lock file races whoever opened it a
/// moment before).
#[derive(Debug)]
pub struct Lock {
    file: File,
}

impl Lock {
    /// Takes `folder`'s lock, waiting for whoever holds it — a hook's run,
    /// which holds it for a read and a write. No clock: Claude Code's own
    /// timeout on the hook is the bound.
    ///
    /// # Errors
    ///
    /// When the lock file cannot be opened or locked.
    pub fn take(folder: &Path) -> std::io::Result<Self> {
        let file = Self::open(folder)?;
        file.lock()?;
        Ok(Self { file })
    }

    /// Takes it only if nobody holds it — `None` when somebody does.
    ///
    /// # Errors
    ///
    /// As [`Lock::take`].
    pub fn try_take(folder: &Path) -> std::io::Result<Option<Self>> {
        let file = Self::open(folder)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { file })),
            Err(std::fs::TryLockError::WouldBlock) => Ok(None),
            Err(std::fs::TryLockError::Error(e)) => Err(e),
        }
    }

    fn open(folder: &Path) -> std::io::Result<File> {
        File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(folder.join(LOCK_FILE))
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // LET GO EXPLICITLY, not by closing: a `flock` belongs to the open
        // file, and a child another thread forks holds a copy of every
        // descriptor until its `exec` (`ronoctua_belongings::running`).
        let _ = self.file.unlock();
    }
}

/// Removes, under the lock, every record whose process is not running any
/// more and every tombstone older than [`TOMBSTONE_KEPT_NS`] at `now_ns` —
/// but `keep`'s, the session that is starting. A record that does not read
/// is left alone: it may be a newer format, which is not this reader's to
/// judge. Answers how many went.
pub(crate) fn sweep(
    folder: &Path,
    _held: &Lock,
    keep: &str,
    now_ns: u64,
    alive: &dyn Fn(u32, u64) -> bool,
) -> usize {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    let mut gone = 0;
    for session in entries
        .flatten()
        .take(MOST_SWEPT)
        .filter_map(|e| session_of_name(&e.file_name()))
        .filter(|s| s != keep)
    {
        let Some(record) = read_one(folder, &session) else {
            continue;
        };
        let settled = record.said_at_ns.saturating_add(TOMBSTONE_KEPT_NS) < now_ns;
        if ((record.ended && settled) || !alive(record.pid, record.pid_start_ticks))
            && let Some(path) = record_path(folder, &session)
            && std::fs::remove_file(path).is_ok()
        {
            gone += 1;
        }
    }
    gone
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-llm-sessions-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub(crate) fn record(session: &str) -> Session {
        Session {
            format: FORMAT,
            session: session.to_owned(),
            account_dir: "/home/p/.claude".into(),
            pid: 4242,
            pid_start_ticks: 1_000,
            terminal: None,
            folder: "/home/p/work".into(),
            transcript: Some("/home/p/.claude/projects/-home-p-work/x.jsonl".into()),
            status: Status::Done,
            said_at_ns: 10,
            started_at_ns: 10,
            ended: false,
        }
    }

    /// FAILS IF: a record does not cross the file and back, or one the
    /// reader cannot vouch for is read — a field it does not know, another
    /// format, a file named for another session, a terminal that is not one
    /// of ours, or one past the bound.
    #[test]
    fn a_record_crosses_and_a_strange_one_is_refused() {
        let dir = scratch("crosses");
        let mine = Session {
            terminal: Some("0123456789abcdef".to_owned()),
            ..record("5f0c1e2a-0b1c-4d2e-8f3a-9b8c7d6e5f40")
        };
        write(&dir, &mine).unwrap();
        assert_eq!(read_one(&dir, &mine.session), Some(mine.clone()));
        let text = std::fs::read_to_string(dir.join(format!("{}.json", mine.session))).unwrap();
        for field in [
            "\"account-dir\"",
            "\"pid-start-ticks\"",
            "\"said-at-ns\"",
            "\"started-at-ns\"",
            "\"status\": \"done\"",
        ] {
            assert!(text.contains(field), "{field} in {text}");
        }
        for (what, strange) in [
            (
                "a field it does not know",
                text.replacen('{', "{\"extra\": 1,", 1),
            ),
            (
                "another format",
                text.replace("\"format\": 1", "\"format\": 2"),
            ),
            (
                "a terminal not of ours",
                text.replace("0123456789abcdef", "../../etc/passwd"),
            ),
            ("not JSON", "{ broken".to_owned()),
        ] {
            std::fs::write(dir.join(format!("{}.json", mine.session)), &strange).unwrap();
            assert_eq!(read_one(&dir, &mine.session), None, "{what}");
        }
        // NAMED FOR ANOTHER SESSION: the name is the id.
        std::fs::write(dir.join("abc.json"), &text).unwrap();
        assert_eq!(read_one(&dir, "abc"), None);
        // PAST THE BOUND, never written: the reader would not read it.
        let huge = Session {
            folder: PathBuf::from("/").join("x".repeat(MOST_BYTES as usize)),
            ..record("abc")
        };
        assert!(write(&dir, &huge).is_err());
        assert!(write(&dir, &record("../escape")).is_err(), "a path");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a session id or a terminal id that is a path, or not the
    /// form Claude Code and the terminals write, is taken.
    #[test]
    fn ids_are_the_forms_their_writers_write() {
        for fine in ["5f0c1e2a-0b1c-4d2e-8f3a-9b8c7d6e5f40", "a", "0-1"] {
            assert!(fine_session(fine), "{fine}");
        }
        for wrong in ["", "../x", "ABC", "a/b", "a.json", &"a".repeat(65)] {
            assert!(!fine_session(wrong), "{wrong}");
        }
        assert!(fine_terminal("0123456789abcdef"));
        for wrong in [
            "0123456789abcde",
            "0123456789ABCDEF",
            "0123456789abcdeg",
            "",
        ] {
            assert!(!fine_terminal(wrong), "{wrong}");
        }
    }

    /// FAILS IF: the reader reads past its bounds — more records than
    /// [`MOST_FILES`], one heavier than [`MOST_BYTES`] — or takes a draft, the
    /// lock or another file for a record.
    #[test]
    fn the_reader_is_bounded_and_reads_records_only() {
        let dir = scratch("bounded");
        for n in 0..MOST_FILES + 20 {
            write(&dir, &record(&format!("{n:x}"))).unwrap();
        }
        assert_eq!(read(&dir).len(), MOST_FILES);
        let dir = scratch("only");
        write(&dir, &record("aa")).unwrap();
        std::fs::write(dir.join(".aa.json.77.new"), b"{}").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.join("bb.json"), vec![b' '; MOST_BYTES as usize + 1]).unwrap();
        drop(Lock::take(&dir).unwrap());
        let read = read(&dir);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].session, "aa");
        assert_eq!(super::read(&dir.join("not-there")), Vec::new());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the folder is not made `0700` (or a looser one is left so),
    /// something in its place is written through, or a record is readable by
    /// anybody but its owner.
    #[test]
    fn the_folder_is_private_and_so_is_every_record() {
        let root = scratch("private");
        let folder = root.join("ronoctua-agents");
        make_folder(&folder).unwrap();
        let mode = |p: &Path| std::fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&folder), 0o700);
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
        make_folder(&folder).unwrap();
        assert_eq!(mode(&folder), 0o700, "tightened");
        write(&folder, &record("aa")).unwrap();
        assert_eq!(mode(&folder.join("aa.json")), 0o600);
        // A LINK where the folder goes is not followed.
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let link = root.join("ronoctua-dev-agents");
        std::os::unix::fs::symlink(&elsewhere, &link).unwrap();
        assert!(make_folder(&link).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a second writer takes the lock while the first holds it, or
    /// the lock outlives the writer that held it.
    #[test]
    fn one_writer_at_a_time() {
        let dir = scratch("lock");
        let first = Lock::take(&dir).unwrap();
        assert!(Lock::try_take(&dir).unwrap().is_none(), "held");
        drop(first);
        assert!(Lock::try_take(&dir).unwrap().is_some(), "let go");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a start does not sweep away the settled tombstones and the
    /// records of processes that are gone, sweeps away a living one or its
    /// own, sweeps a tombstone a late word of its session could still meet
    /// (its process alive, its end within [`TOMBSTONE_KEPT_NS`]), or deletes
    /// a record it cannot read (a newer format's).
    #[test]
    fn a_sweep_takes_the_ended_and_the_dead_and_nothing_else() {
        let dir = scratch("sweep");
        let alive = |pid: u32, ticks: u64| pid == 1 && ticks == 1;
        let now = 10 + TOMBSTONE_KEPT_NS + 1;
        let living = Session {
            pid: 1,
            pid_start_ticks: 1,
            ..record("a1")
        };
        let ended = Session {
            ended: true,
            ..living.clone()
        };
        let just_ended = Session {
            said_at_ns: now - 1,
            ..ended.clone()
        };
        for (session, r) in [
            ("a1", living),
            ("a2", ended),
            ("a3", record("a3")),
            ("a4", record("a4")),
            ("a6", just_ended),
        ] {
            write(
                &dir,
                &Session {
                    session: session.to_owned(),
                    ..r
                },
            )
            .unwrap();
        }
        std::fs::write(dir.join("a5.json"), b"{\"format\": 2}").unwrap();
        let held = Lock::take(&dir).unwrap();
        assert_eq!(sweep(&dir, &held, "a4", now, &alive), 2);
        let left: Vec<String> = read(&dir).into_iter().map(|s| s.session).collect();
        assert_eq!(left, ["a1", "a4", "a6"]);
        assert!(dir.join("a5.json").exists(), "not this reader's to judge");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: two spellings of one folder are taken for two accounts, or
    /// two folders for one.
    #[test]
    fn a_folder_is_compared_as_written() {
        assert!(same_folder(
            Path::new("/home/p/.claude-2/"),
            Path::new("/home/p//./.claude-2")
        ));
        assert!(!same_folder(
            Path::new("/home/p/.claude"),
            Path::new("/home/p/.claude-2")
        ));
    }
}
