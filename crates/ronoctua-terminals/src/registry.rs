//! Where the held terminals are, and the census over them.
//!
//! # One folder, two files per terminal
//!
//! `$XDG_RUNTIME_DIR/<name>-terms/` (`ronoctua-dev-terms/` for the nested
//! loop, so the two never count each other's), mode `0700`: what is in it is
//! a socket that types into a shell. Each holder keeps `<id>.sock`, which it
//! listens on, and `<id>.json`, its whole state, written atomically on every
//! change. The socket is bound BEFORE the state is first written and the
//! state removed BEFORE the socket, so a state file always has its socket.
//!
//! # A holder that died says nothing, and the census cleans up after it
//!
//! A holder killed with `SIGKILL` (an OOM kill) leaves both files: a scope
//! runs no command after its process. So the census asks each socket: a
//! refused connection is a dead holder, and both its files go. Ids are
//! random, so there is no race with a new holder of the same name.

use std::io::{self, BufReader, Write as _};
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::TerminalId;
use crate::state::TerminalState;
use crate::wire::{self, Ask, Attach};

/// How long a one-shot question waits for its holder. A holder answers from
/// its engine thread at once; one that does not for this long is stopped or
/// lost, and the asker is told so instead of hanging with it.
pub const ASK_TIMEOUT: Duration = Duration::from_secs(5);

/// The largest state file read. A holder bounds every field it writes
/// (`state.rs`, `status.rs`): its worst case is under a megabyte. A file past
/// this is no holder's, and the footer — which reads these on every change —
/// does not take it in.
pub const STATE_MAX_BYTES: u64 = 4 << 20;

/// The most state files [`Registry::states`] reads: far more terminals than
/// one person holds, and a bound on a folder something else filled.
pub const STATES_MOST: usize = 256;

/// The registry's folder and what is in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registry {
    dir: PathBuf,
}

impl Registry {
    /// This session's: `$XDG_RUNTIME_DIR/<name>-terms/`, per instance. `None`
    /// without a runtime directory — there is no fallback to `/tmp`, because
    /// a socket into a shell in a shared folder is an open door.
    #[must_use]
    pub fn of_session() -> Option<Self> {
        ronoctua_xdg::runtime_file("terms").map(Self::at)
    }

    /// A registry in `dir` — a test's own folder.
    #[must_use]
    pub fn at(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The folder.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// A terminal's socket.
    #[must_use]
    pub fn socket(&self, id: &TerminalId) -> PathBuf {
        self.dir.join(format!("{id}.sock"))
    }

    /// A terminal's state file.
    #[must_use]
    pub fn state_file(&self, id: &TerminalId) -> PathBuf {
        self.dir.join(state_name(id))
    }

    /// Makes the folder, `0700` — and makes an existing one `0700` again.
    ///
    /// # Errors
    ///
    /// It could not be created or its mode set.
    pub fn ensure(&self) -> io::Result<()> {
        use std::os::unix::fs::DirBuilderExt as _;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&self.dir)?;
        let meta = std::fs::symlink_metadata(&self.dir)?;
        if !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} is not a folder", self.dir.display()),
            ));
        }
        if meta.permissions().mode() & 0o777 != 0o700 {
            std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    /// Writes a terminal's whole state, atomically.
    ///
    /// # Errors
    ///
    /// The file could not be written.
    pub fn write(&self, state: &TerminalState) -> io::Result<()> {
        let line = state.line().map_err(io::Error::other)?;
        ronoctua_xdg::write_atomically(&self.dir, &state_name(&state.id), &line)
    }

    /// A terminal's state as its file has it, or `None` if there is none (or
    /// it is not one).
    #[must_use]
    pub fn read(&self, id: &TerminalId) -> Option<TerminalState> {
        use std::io::Read as _;
        let mut bytes = Vec::new();
        std::fs::File::open(self.state_file(id))
            .ok()?
            .take(STATE_MAX_BYTES + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() as u64 > STATE_MAX_BYTES {
            return None;
        }
        serde_json::from_slice::<TerminalState>(&bytes)
            .ok()
            .filter(|s| &s.id == id)
    }

    /// Every terminal's state as its file has it, by id — at most
    /// [`STATES_MOST`] of them — asking NO holder and removing nothing.
    ///
    /// What a reader that may not wait on a socket reads: the brain, on its
    /// watch thread, which never blocks — and a census's `connect` to a
    /// frozen holder whose backlog is full would. A holder killed with
    /// `SIGKILL` left its file, which the next [`Registry::census`] clears;
    /// until then it is read here as if it were held, and a reader that
    /// cares whether its program still runs asks the process, not this.
    #[must_use]
    pub fn states(&self) -> Vec<TerminalState> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut out: Vec<TerminalState> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter_map(|name| name.strip_suffix(".json")?.parse::<TerminalId>().ok())
            .take(STATES_MOST)
            .filter_map(|id| self.read(&id))
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    /// Removes a terminal's files: the state first, then the socket, so no
    /// reader ever finds a state without its socket.
    pub fn forget(&self, id: &TerminalId) {
        let _ = std::fs::remove_file(self.state_file(id));
        let _ = std::fs::remove_file(self.socket(id));
    }

    /// Every terminal whose holder answers, by group and place in it; the
    /// files of every holder that does not are removed.
    ///
    /// It connects to each socket once, which is cheap — a local `connect`
    /// that is accepted and closed — but it IS one per terminal: call it
    /// where reading a folder is fine, not per frame.
    #[must_use]
    pub fn census(&self) -> Census {
        let mut terminals = Vec::new();
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Census { terminals };
        };
        // The names FIRST: what a directory lists while its entries are
        // being removed is unspecified, and a dead holder's files are.
        let names: Vec<String> = entries
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .collect();
        for name in &names {
            if let Some(id) = name.strip_suffix(".json").and_then(|s| s.parse().ok()) {
                match self.answers(&id) {
                    Liveness::Alive => {
                        if let Some(state) = self.read(&id) {
                            terminals.push(state);
                        }
                    }
                    Liveness::Dead => self.forget(&id),
                    Liveness::Unknown => {}
                }
            } else if let Some(id) = name
                .strip_suffix(".sock")
                .and_then(|s| s.parse::<TerminalId>().ok())
            {
                // A socket with no state: a holder between its bind and its
                // first write is alive and is left alone; a dead one is not.
                // A YOUNG one is left alone even when it refuses: between a
                // bind and its `listen` a living holder refuses too, and its
                // socket, once gone, could never be found again. One older
                // than any start may take is a holder that died starting.
                if !self.state_file(&id).exists()
                    && !self.young(&id)
                    && self.answers(&id) == Liveness::Dead
                {
                    let _ = std::fs::remove_file(self.socket(&id));
                }
            }
        }
        terminals.sort_by(|a, b| {
            (&a.group, a.order, a.started_s, &a.id).cmp(&(&b.group, b.order, b.started_s, &b.id))
        });
        Census { terminals }
    }

    /// Whether a terminal's socket was made less than a start's deadline ago
    /// (or its age cannot be told, which is not proof of death either).
    fn young(&self, id: &TerminalId) -> bool {
        std::fs::symlink_metadata(self.socket(id))
            .and_then(|m| m.modified())
            .ok()
            .and_then(|made| made.elapsed().ok())
            .is_none_or(|age| age < crate::spawn::START_DEADLINE)
    }

    /// Whether a terminal's holder answers on its socket.
    fn answers(&self, id: &TerminalId) -> Liveness {
        match UnixStream::connect(self.socket(id)) {
            Ok(_) => Liveness::Alive,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                ) =>
            {
                Liveness::Dead
            }
            // Busy, or a permission this process does not have: not ours to
            // judge, and certainly not ours to delete.
            Err(_) => Liveness::Unknown,
        }
    }

    /// A connection to a terminal's holder, its first line sent.
    fn open(&self, id: &TerminalId, ask: &Ask) -> io::Result<UnixStream> {
        let mut stream = UnixStream::connect(self.socket(id))?;
        stream.set_read_timeout(Some(ASK_TIMEOUT))?;
        stream.set_write_timeout(Some(ASK_TIMEOUT))?;
        stream.write_all(&ask.line().map_err(io::Error::other)?)?;
        Ok(stream)
    }

    /// Asks and reads the one-line answer.
    fn ask(&self, id: &TerminalId, ask: &Ask) -> io::Result<Vec<u8>> {
        let stream = self.open(id, ask)?;
        wire::read_line(&mut BufReader::new(stream))?
            .ok_or_else(|| io::ErrorKind::UnexpectedEof.into())
    }

    /// Asks an order and expects it done.
    fn order(&self, id: &TerminalId, ask: &Ask) -> io::Result<()> {
        let line = self.ask(id, ask)?;
        if line == wire::DONE_LINE[..wire::DONE_LINE.len() - 1] {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "the holder did not answer done",
            ))
        }
    }

    /// The terminal's whole state, from its holder (fresher than its file by
    /// the file's spacing). Blocks up to [`ASK_TIMEOUT`]: call it off a loop.
    ///
    /// # Errors
    ///
    /// The holder is not there, or answered something else.
    pub fn state_of(&self, id: &TerminalId) -> io::Result<TerminalState> {
        let line = self.ask(id, &Ask::State)?;
        serde_json::from_slice(&line).map_err(io::Error::other)
    }

    /// Asks the terminal's window to put it in front. Blocks up to
    /// [`ASK_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// The holder is not there.
    pub fn front(&self, id: &TerminalId) -> io::Result<()> {
        self.order(id, &Ask::Front)
    }

    /// Ends the terminal: its program is hung up. Blocks up to
    /// [`ASK_TIMEOUT`].
    ///
    /// # Errors
    ///
    /// The holder is not there.
    pub fn end(&self, id: &TerminalId) -> io::Result<()> {
        self.order(id, &Ask::End)
    }

    /// Attaches to the terminal: the stream, its attach sent, from which
    /// [`wire::ToViewer`] frames follow. The timeouts are cleared — a window
    /// waits for its terminal as long as it shows it.
    ///
    /// # Errors
    ///
    /// The holder is not there.
    pub fn attach(&self, id: &TerminalId, attach: &Attach) -> io::Result<UnixStream> {
        let stream = self.open(id, &Ask::Attach(attach.clone()))?;
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        Ok(stream)
    }
}

/// A state file's name.
fn state_name(id: &TerminalId) -> String {
    format!("{id}.json")
}

/// What a socket said when asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Liveness {
    Alive,
    Dead,
    Unknown,
}

/// The held terminals, by group and place.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    /// Every terminal whose holder answered.
    pub terminals: Vec<TerminalState>,
}

impl Census {
    /// How many are in the background: no window, still running.
    #[must_use]
    pub fn away(&self) -> usize {
        self.terminals.iter().filter(|t| t.away()).count()
    }

    /// The ones whose program ended and that no window has shown since.
    pub fn ended(&self) -> impl Iterator<Item = &TerminalState> {
        self.terminals
            .iter()
            .filter(|t| t.ended.is_some() && t.viewer.is_none())
    }

    /// Whether any terminal in the background asks for the person.
    #[must_use]
    pub fn attention(&self) -> bool {
        self.terminals
            .iter()
            .any(|t| t.viewer.is_none() && t.asks_attention())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tests::full;
    use std::os::unix::net::UnixListener;

    fn scratch(tag: &str) -> Registry {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-terminals-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Registry::at(dir)
    }

    /// A socket file nobody ever listened on. Not one bound and closed: a
    /// test thread that forks meanwhile holds the listener in its child until
    /// that child execs, and the socket answers for that moment.
    fn dead_socket(path: &Path) {
        use rustix::fs::{CWD, FileType, Mode, mknodat};
        mknodat(CWD, path, FileType::Socket, Mode::from_raw_mode(0o600), 0).unwrap();
    }

    /// Dates a file a minute back.
    fn made_long_ago(path: &Path) {
        use rustix::fs::{AtFlags, CWD, Timespec, Timestamps, utimensat};
        let then = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 60;
        let at = Timespec {
            tv_sec: i64::try_from(then).unwrap(),
            tv_nsec: 0,
        };
        utimensat(
            CWD,
            path,
            &Timestamps {
                last_access: at,
                last_modification: at,
            },
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .unwrap();
    }

    fn state(id: &str, viewer: Option<u32>) -> TerminalState {
        TerminalState {
            id: id.parse().unwrap(),
            viewer,
            ended: None,
            ..full()
        }
    }

    #[test]
    fn the_folder_is_private() {
        // FAILS IF the registry is readable by anyone else: a socket in it
        // types into a shell.
        let r = scratch("private");
        std::fs::create_dir_all(r.dir()).unwrap();
        std::fs::set_permissions(r.dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
        r.ensure().unwrap();
        let mode = std::fs::metadata(r.dir()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        let _ = std::fs::remove_dir_all(r.dir());
    }

    #[test]
    fn a_state_file_reads_back_whole() {
        // FAILS IF the file written is not the state read, a file whose id
        // is another terminal's is taken for this one, or a file of any size
        // is read whole.
        let r = scratch("roundtrip");
        r.ensure().unwrap();
        let s = full();
        r.write(&s).unwrap();
        assert_eq!(r.read(&s.id), Some(s.clone()));
        let other: TerminalId = "1111111111111111".parse().unwrap();
        std::fs::copy(r.state_file(&s.id), r.state_file(&other)).unwrap();
        assert_eq!(r.read(&other), None, "the file names another terminal");
        // A file past the bound is not taken in, however well it parses.
        let huge = TerminalState {
            title: "t".repeat(usize::try_from(STATE_MAX_BYTES).unwrap()),
            ..full()
        };
        r.write(&huge).unwrap();
        assert_eq!(r.read(&huge.id), None, "a state no holder writes");
        let _ = std::fs::remove_dir_all(r.dir());
    }

    #[test]
    fn the_census_counts_the_terminals_with_no_window_and_clears_a_dead_one() {
        // FAILS IF the background count includes a terminal a window shows,
        // a holder that died (its socket refuses) is counted forever, or the
        // socket of one still starting is taken for a dead one's.
        let r = scratch("census");
        r.ensure().unwrap();
        let alive = state("aaaaaaaaaaaaaaaa", None);
        let shown = state("bbbbbbbbbbbbbbbb", Some(1234));
        let dead = state("cccccccccccccccc", None);
        let _a = UnixListener::bind(r.socket(&alive.id)).unwrap();
        let _b = UnixListener::bind(r.socket(&shown.id)).unwrap();
        // A socket nobody listens on — what a `SIGKILL`ed holder leaves.
        dead_socket(&r.socket(&dead.id));
        for s in [&alive, &shown, &dead] {
            r.write(s).unwrap();
        }
        // A stray socket of a dead holder that had not written its state,
        // made longer ago than any start takes — and one made just now,
        // which may be a holder between its bind and its `listen`.
        let stray: TerminalId = "dddddddddddddddd".parse().unwrap();
        dead_socket(&r.socket(&stray));
        made_long_ago(&r.socket(&stray));
        let starting: TerminalId = "9999999999999999".parse().unwrap();
        dead_socket(&r.socket(&starting));

        let census = r.census();
        let ids: Vec<&str> = census.terminals.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb"]);
        assert_eq!(census.away(), 1);
        assert!(!r.state_file(&dead.id).exists() && !r.socket(&dead.id).exists());
        assert!(!r.socket(&stray).exists());
        assert!(r.socket(&starting).exists(), "a young one is left alone");
        assert!(r.socket(&alive.id).exists(), "a live one is left alone");
        let _ = std::fs::remove_dir_all(r.dir());
    }

    #[test]
    fn the_states_are_read_without_asking_a_holder_or_removing_anything() {
        // FAILS IF the read-only reader asks a socket (a holder nobody
        // listens for would then be judged, and its files removed under a
        // reader that may not wait), reads a file that is not a state's, or
        // reads past its bound.
        let r = scratch("states");
        r.ensure().unwrap();
        let listening = state("aaaaaaaaaaaaaaaa", None);
        let dead = state("cccccccccccccccc", Some(7));
        let _a = UnixListener::bind(r.socket(&listening.id)).unwrap();
        dead_socket(&r.socket(&dead.id));
        for s in [&listening, &dead] {
            r.write(s).unwrap();
        }
        std::fs::write(r.dir().join("notes.json"), b"{}").unwrap();
        std::fs::write(r.dir().join("bbbbbbbbbbbbbbbb.json"), b"{ not a state").unwrap();
        let ids: Vec<String> = r.states().into_iter().map(|s| s.id.to_string()).collect();
        assert_eq!(ids, ["aaaaaaaaaaaaaaaa", "cccccccccccccccc"]);
        assert!(
            r.state_file(&dead.id).exists() && r.socket(&dead.id).exists(),
            "the reader removed a dead holder's files"
        );
        // THE BOUND: a folder filled past it is read up to it — files, not
        // states: one that does not read counts against it too.
        std::fs::remove_file(r.dir().join("bbbbbbbbbbbbbbbb.json")).unwrap();
        for n in 0..STATES_MOST + 4 {
            let id = format!("{n:016x}");
            r.write(&state(&id, None)).unwrap();
        }
        assert_eq!(r.states().len(), STATES_MOST);
        assert!(Registry::at(r.dir().join("not-there")).states().is_empty());
        let _ = std::fs::remove_dir_all(r.dir());
    }

    #[test]
    fn an_ended_terminal_is_listed_apart_and_is_not_away() {
        // FAILS IF a terminal whose program ended in the background is
        // counted as running there — or not listed at all, which would leave
        // its holder waiting for a window that nothing offers to open.
        let mut ended = state("eeeeeeeeeeeeeeee", None);
        ended.ended = Some(crate::state::Ended { code: 0, at_s: 1 });
        let census = Census {
            terminals: vec![ended, state("ffffffffffffffff", None)],
        };
        assert_eq!(census.away(), 1);
        assert_eq!(census.ended().count(), 1);
    }
}
