//! Running one of the three programs: the password on stdin under `setsid`,
//! both outputs read as one stream, and the refusal read from what it said.
//!
//! # Why `setsid`
//!
//! `7zz` and `unrar` ask for a password on the controlling terminal when a
//! process has one, and on stdin when it has none. A child started by a
//! footer has none today, and a child started by `cargo test` in a terminal
//! does — so the session is taken away explicitly (`setsid --wait`, the
//! store path `RONOCTUA_SETSID` names) and the question can only ever be
//! answered by what this module writes. Without it a password question on a
//! terminal would wait for a person who is not looking at one.
//!
//! # Why a watcher, and why the whole process group
//!
//! A caller that may hold or give up a run hands a [`Switch`]; a thread asks
//! it every [`POLL`] while the program runs and signals the program's
//! PROCESS GROUP — `setsid` made the program a session and group leader, and
//! 7zz's own threads and anything it started are in that group. Without
//! `setsid` (bsdtar, in a test) only the program itself is signalled: its
//! group is the caller's, and signalling that would stop the caller.
//!
//! # Why the outputs are merged
//!
//! Both programs print the password prompt, the progress and the errors on
//! whichever stream they like; one pipe for both keeps the order and needs
//! one reader, not a thread per stream.

use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};

use zeroize::Zeroize as _;

use crate::{Hold, Refusal, Switch, Tool, Why};

/// How often a running program's switch is asked: a tenth of a second is
/// under what a hand notices after pressing "hold", and costs nothing.
const POLL: std::time::Duration = std::time::Duration::from_millis(100);

/// The longest password file read: a password somebody typed, not a file.
pub(crate) const SECRET_MOST_BYTES: u64 = 4096;

/// The last bytes of output kept to read a refusal from.
const TAIL_BYTES: usize = 16 * 1024;

/// The longest piece handed on at once: a line without an end is cut here,
/// so output that never ends a line cannot grow without bound.
const PIECE_MOST_BYTES: usize = 64 * 1024;

/// A password read from its `0600` file, wiped when dropped.
pub(crate) struct Password(Vec<u8>);

impl Password {
    /// The password in `file`: its bytes, one trailing line end taken off.
    ///
    /// # Errors
    ///
    /// [`Why::NeedsPassword`] for a missing or empty file, [`Why::Unsupported`]
    /// for one too large or holding a line end, a NUL or anything but UTF-8 —
    /// a password the programs could not be given on one line.
    pub(crate) fn read(file: &Path, tool: Tool) -> Result<Self, Refusal> {
        let refused = |why| Refusal {
            why,
            tool: Some(tool),
        };
        let meta = std::fs::metadata(file).map_err(|_| refused(Why::NeedsPassword))?;
        if !meta.is_file() || meta.len() > SECRET_MOST_BYTES {
            return Err(refused(Why::Unsupported));
        }
        let mut bytes = std::fs::read(file).map_err(|_| refused(Why::NeedsPassword))?;
        if bytes.last() == Some(&b'\n') {
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
        }
        let bad = bytes.is_empty()
            || bytes.iter().any(|b| matches!(b, b'\n' | b'\r' | 0))
            || std::str::from_utf8(&bytes).is_err();
        if bad {
            let why = if bytes.is_empty() {
                Why::NeedsPassword
            } else {
                Why::Unsupported
            };
            bytes.zeroize();
            return Err(refused(why));
        }
        Ok(Self(bytes))
    }

    /// What goes to stdin: the password on `times` lines. A question that is
    /// asked fewer times leaves the rest unread; one asked more often (a
    /// wrong password asked again) meets the end of stdin and stops.
    fn lines(&self, times: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity((self.0.len() + 1) * times);
        for _ in 0..times {
            out.extend_from_slice(&self.0);
            out.push(b'\n');
        }
        out
    }
}

impl Drop for Password {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// One run of a program.
pub(crate) struct Call<'a> {
    /// Which.
    pub tool: Tool,
    /// Its arguments.
    pub args: Vec<OsString>,
    /// Where it runs (paths it stores are relative to this).
    pub cwd: Option<&'a Path>,
    /// The password, and how many lines of it stdin holds.
    pub password: Option<(&'a Password, usize)>,
    /// What else stdin holds: bsdtar's list of names (`--null -T -`), so a
    /// name is never an argument it reads as `@archive` or `-C folder`.
    /// Never with a password.
    pub listing: Option<&'a [u8]>,
}

/// How a run ended.
#[derive(Debug)]
pub(crate) struct Ended {
    /// The exit code; `None` when a signal ended it.
    pub code: Option<i32>,
    /// The last of what it said.
    pub tail: String,
}

/// Runs `call` to its end with `program` (and `setsid`), handing every piece
/// of output — split at line ends and at the backspaces a progress line is
/// redrawn with — to `each`; `switch`, when there is one, is asked while it
/// runs ([`watch`]).
pub(crate) fn run(
    program: &Path,
    setsid: Option<&Path>,
    call: &Call<'_>,
    switch: Option<&Switch>,
    each: &mut dyn FnMut(&str),
) -> Result<Ended, Refusal> {
    let refused = |why| Refusal {
        why,
        tool: Some(call.tool),
    };
    let (reader, writer) = std::io::pipe().map_err(|_| refused(Why::Unstartable))?;
    let writer_err = writer.try_clone().map_err(|_| refused(Why::Unstartable))?;
    let mut command = match setsid {
        Some(setsid) => {
            let mut c = Command::new(setsid);
            c.arg("--wait").arg(program);
            c
        }
        None => Command::new(program),
    };
    command
        .args(&call.args)
        // Times in the listings are read as UTC; the messages matched below
        // are the programs' own English; names are UTF-8 whatever the
        // person's locale.
        .env("TZ", "UTC0")
        .env("LC_ALL", "C.UTF-8")
        .env_remove("LANGUAGE")
        .stdin(if call.password.is_some() || call.listing.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(writer)
        .stderr(writer_err);
    if let Some(cwd) = call.cwd {
        command.current_dir(cwd);
    }
    let spawned = command.spawn();
    // The command holds the pipe's writing ends: dropped now, or the read
    // below would never see the end.
    drop(command);
    let mut child = spawned.map_err(|_| refused(Why::Unstartable))?;
    let over = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watcher = switch.map(|s| watch(child.id(), setsid.is_some(), s.clone(), over.clone()));
    let mut feeding = None;
    match (call.password, call.listing, child.stdin.take()) {
        (Some((password, times)), _, Some(mut stdin)) => {
            let mut lines = password.lines(times);
            // A child that stops before reading is not an error here: its
            // exit says what happened.
            let _ = stdin.write_all(&lines);
            lines.zeroize();
        }
        (None, Some(listing), Some(mut stdin)) => {
            // ON A THREAD: bsdtar writes a line per name while it reads
            // them, and a long list written from here would wait on a child
            // that waits on the output nobody is reading yet.
            let listing = listing.to_vec();
            feeding = Some(std::thread::spawn(move || {
                let _ = stdin.write_all(&listing);
            }));
        }
        _ => {}
    }
    let mut tail = Vec::new();
    let mut piece = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut reader = reader;
    loop {
        let n = match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        for &b in &buffer[..n] {
            if matches!(b, b'\n' | b'\r' | 0x08) {
                if !piece.is_empty() {
                    each(&String::from_utf8_lossy(&piece));
                    piece.clear();
                }
            } else {
                piece.push(b);
                if piece.len() >= PIECE_MOST_BYTES {
                    each(&String::from_utf8_lossy(&piece));
                    piece.clear();
                }
            }
        }
        tail.extend_from_slice(&buffer[..n]);
        if tail.len() > 2 * TAIL_BYTES {
            tail.drain(..tail.len() - TAIL_BYTES);
        }
    }
    if !piece.is_empty() {
        each(&String::from_utf8_lossy(&piece));
    }
    let status = child.wait().map_err(|_| refused(Why::Failed))?;
    over.store(true, std::sync::atomic::Ordering::Relaxed);
    if let Some(watcher) = watcher {
        let _ = watcher.join();
    }
    if let Some(feeding) = feeding {
        let _ = feeding.join();
    }
    Ok(Ended {
        code: status.code(),
        tail: String::from_utf8_lossy(&tail).into_owned(),
    })
}

/// Starts `program` for `call` with its archive on standard output, which
/// the caller reads (`child.stdout`) and waits on — what
/// [`crate::Tools::tar_stream`] hands encrypt-anything. The same clock and
/// locale as [`run`]; `call.listing` is written to stdin whole, and stdin
/// closed, before this returns — a few bytes the pipe holds, so this cannot
/// wait on output nobody reads yet. Its complaints go to this process's
/// standard error; a member it could not read makes it exit with a failure
/// at the end, which the caller's `wait` sees. No `setsid` and no password:
/// the child is in the caller's group, so whatever stops the caller's group
/// stops it too.
///
/// # Errors
///
/// [`Why::Unstartable`] when it does not start, [`Why::Failed`] when its
/// list of names could not be written (the child is gone by then).
pub(crate) fn stream(program: &Path, call: &Call<'_>) -> Result<std::process::Child, Refusal> {
    let refused = |why| Refusal {
        why,
        tool: Some(call.tool),
    };
    let mut command = Command::new(program);
    command
        .args(&call.args)
        .env("TZ", "UTC0")
        .env("LC_ALL", "C.UTF-8")
        .env_remove("LANGUAGE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    if let Some(cwd) = call.cwd {
        command.current_dir(cwd);
    }
    let mut child = command.spawn().map_err(|_| refused(Why::Unstartable))?;
    let written = child.stdin.take().map_or(Ok(()), |mut stdin| {
        stdin.write_all(call.listing.unwrap_or_default())
    });
    if written.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(refused(Why::Failed));
    }
    Ok(child)
}

/// Asks `switch` every [`POLL`] until `over`, and signals the program `pid`:
/// its whole process group when it leads one (`grouped`, under `setsid`),
/// itself otherwise. A hold is sent once and undone once; a stop is `SIGKILL`,
/// which also ends a held program. The thread ends with the run.
fn watch(
    pid: u32,
    grouped: bool,
    switch: Switch,
    over: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<()> {
    use rustix::process::{Pid, Signal, kill_process, kill_process_group};
    let send = move |signal: Signal| {
        let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
            return;
        };
        // THE GROUP FIRST, and the program itself when there is none yet:
        // `setsid` makes the group a moment after the spawn, and a signal
        // sent in that moment would otherwise reach nothing.
        if !grouped || kill_process_group(pid, signal).is_err() {
            let _ = kill_process(pid, signal);
        }
    };
    std::thread::spawn(move || {
        let mut held = false;
        while !over.load(std::sync::atomic::Ordering::Relaxed) {
            match switch.now() {
                Hold::Stop => {
                    send(Signal::KILL);
                    return;
                }
                Hold::Pause if !held => {
                    send(Signal::STOP);
                    held = true;
                }
                Hold::Go if held => {
                    send(Signal::CONT);
                    held = false;
                }
                Hold::Go | Hold::Pause => {}
            }
            std::thread::sleep(POLL);
        }
    })
}

/// What a run that ended with `code` and said `text` means: `None` for
/// success. `asked` is whether a password was given. Pure.
#[must_use]
pub(crate) fn verdict(tool: Tool, code: Option<i32>, text: &str, asked: bool) -> Option<Why> {
    let says = |words: &[&str]| words.iter().any(|w| text.contains(w));
    // MEASURED: `unrar lt` on something that is not a RAR says so and exits
    // with 0.
    if tool == Tool::Unrar && says(&["is not RAR archive"]) {
        return Some(Why::Corrupt);
    }
    if code == Some(0) {
        return None;
    }
    let password = |wrong| {
        if asked { wrong } else { Why::NeedsPassword }
    };
    Some(match tool {
        Tool::SevenZz => {
            if says(&["Wrong password"]) {
                password(Why::WrongPassword)
            } else if !asked && (says(&["Enter password", "Break signaled"]) || code == Some(255)) {
                // stdin is empty when no password is given, and the only
                // question left to ask on it (overwriting is answered by
                // `-aou -y`) is the password's: `-bso0` hides the prompt,
                // and what is left is the break at the end of stdin.
                Why::NeedsPassword
            } else if says(&[
                "Cannot open the file as archive",
                "Can not open the file as archive",
                "Cannot open the file as",
                "Is not archive",
                "Headers Error",
                "Data Error",
                "CRC Failed",
                "Unexpected end of archive",
                "Unconfirmed start of archive",
            ]) {
                Why::Corrupt
            } else {
                Why::Failed
            }
        }
        Tool::Unrar => {
            if code == Some(11) || says(&["password is incorrect", "wrong password"]) {
                password(Why::WrongPassword)
            } else if !asked && says(&["Enter password"]) {
                Why::NeedsPassword
            } else if code == Some(3)
                || says(&[
                    "Bad archive",
                    "is not RAR archive",
                    "Unexpected end of archive",
                    "checksum error",
                    "Corrupt header",
                    "CRC failed",
                ])
            {
                Why::Corrupt
            } else {
                Why::Failed
            }
        }
        Tool::Bsdtar => {
            if says(&[
                "Unrecognized archive format",
                "Truncated",
                "truncated",
                "Damaged",
                "Missing type keyword",
                "corrupt",
                "Premature end",
            ]) {
                Why::Corrupt
            } else {
                Why::Failed
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_a_program_said_is_one_refusal() {
        let seven = |code, text, asked| verdict(Tool::SevenZz, Some(code), text, asked);
        assert_eq!(seven(0, "Everything is Ok", false), None);
        assert_eq!(
            seven(2, "ERROR: Wrong password : a.txt", true),
            Some(Why::WrongPassword)
        );
        assert_eq!(
            seven(
                2,
                "Cannot open encrypted archive. Wrong password?\nHeaders Error",
                false
            ),
            Some(Why::NeedsPassword),
            "no password given: the archive needs one, it was not wrong"
        );
        assert_eq!(
            seven(255, "Enter password:\nBreak signaled", false),
            Some(Why::NeedsPassword)
        );
        assert_eq!(
            seven(2, "ERROR: x.7z\nCannot open the file as archive", false),
            Some(Why::Corrupt)
        );
        assert_eq!(seven(2, "No more files", false), Some(Why::Failed));

        let rar = |code, text, asked| verdict(Tool::Unrar, Some(code), text, asked);
        assert_eq!(
            rar(12, "The specified password is incorrect.\nRead error", true),
            Some(Why::WrongPassword)
        );
        assert_eq!(
            rar(12, "Enter password (will not be echoed) for a.rar:", false),
            Some(Why::NeedsPassword)
        );
        assert_eq!(rar(3, "", false), Some(Why::Corrupt));
        assert_eq!(
            rar(13, "ERROR: Bad archive x.rar", false),
            Some(Why::Corrupt)
        );
        assert_eq!(
            rar(0, "junk.rar is not RAR archive", false),
            Some(Why::Corrupt),
            "measured: exit 0"
        );
        assert_eq!(
            seven(255, "Break signaled", false),
            Some(Why::NeedsPassword)
        );
        assert_eq!(
            seven(255, "Break signaled", true),
            Some(Why::Failed),
            "a password given and still asked for more"
        );

        let tar = |code, text| verdict(Tool::Bsdtar, Some(code), text, false);
        assert_eq!(
            tar(
                1,
                "bsdtar: Error opening archive: Unrecognized archive format"
            ),
            Some(Why::Corrupt)
        );
        assert_eq!(
            tar(1, "bsdtar: Error opening archive: Failed to open 'x'"),
            Some(Why::Failed)
        );
        assert_eq!(
            verdict(Tool::Bsdtar, None, "", false),
            Some(Why::Failed),
            "a signal is a failure"
        );
    }

    #[test]
    fn a_password_file_holds_one_line() {
        let dir = std::env::temp_dir().join(format!("ronoctua-archive-pw-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("secret");
        std::fs::write(&file, "right horse\r\n").unwrap();
        let read = Password::read(&file, Tool::SevenZz).unwrap();
        assert_eq!(read.lines(2), b"right horse\nright horse\n");
        std::fs::write(&file, "two\nlines").unwrap();
        assert_eq!(
            Password::read(&file, Tool::SevenZz).err().unwrap().why,
            Why::Unsupported
        );
        std::fs::write(&file, "\n").unwrap();
        assert_eq!(
            Password::read(&file, Tool::Unrar).err().unwrap().why,
            Why::NeedsPassword
        );
        assert_eq!(
            Password::read(&dir.join("absent"), Tool::Unrar)
                .err()
                .unwrap()
                .why,
            Why::NeedsPassword
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
