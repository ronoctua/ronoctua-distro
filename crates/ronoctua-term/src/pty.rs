//! The PTY: the terminal's other side — §7.10.
//!
//! # Division of labour
//!
//! The engine interprets bytes; this module produces and consumes them. The
//! engine never sees a file descriptor and this module never sees a cell —
//! that is what makes it possible to test each on its own.
//!
//! # Why `setsid --ctty`, and not `fork` + `TIOCSCTTY`
//!
//! Giving the child a CONTROLLING terminal requires two calls between the `fork` and
//! the `exec`. In Rust that is `Command::pre_exec`, which is `unsafe` — and this
//! crate denies `unsafe_code` everywhere but the engine's FFI, which is not
//! fussiness: a mistake there runs in the child, after the fork, in the one place in
//! the program where nothing is safe.
//!
//! util-linux's `setsid` makes exactly those two calls, in a binary that
//! exists on every installation and that is audited by far more people. The cost is
//! one extra `exec` per terminal opened.
//!
//! Without a controlling terminal the shell is born with no job control: `Ctrl-C` does not
//! interrupt, `fg`/`bg` do not exist, and none of that gives an error — it just does not
//! work.
//!
//! # The master is close-on-exec
//!
//! It was not, and every program the shell ran inherited the master — the
//! terminal's own end. A holder that died then could not hang its shell up:
//! the kernel sends SIGHUP when the LAST master closes, and the shell's
//! children were holding one. `O_CLOEXEC` keeps the master in this process.

use std::ffi::OsString;
use std::io::{Read as _, Write as _};
use std::os::fd::{AsFd as _, OwnedFd};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

/// `setsid`, by PATH and not by name.
///
/// A systemd scope comes up with a minimal `PATH`, and a `setsid` that does not resolve
/// fails inside the pty — the message shows up on the terminal's SCREEN, and not in the
/// shell's journal. It cost a VM test to find out.
///
/// `option_env!` with a fallback to the name: that way `cargo build` outside Nix
/// still works, and inside it the path is the store's.
const SETSID_PATH: &str = match option_env!("RONOCTUA_SETSID") {
    Some(p) => p,
    None => "setsid",
};

/// What the terminal tells the program it is.
///
/// `xterm-ghostty` because the engine IS Ghostty's: its terminfo describes
/// exactly what `libghostty-vt` understands (the kitty keyboard, the
/// underline styles, synchronized output…), which `xterm-256color` cannot say.
/// The `xterm-` prefix keeps the programs that test `TERM` for `xterm*`
/// working as they do under Ghostty itself.
pub const TERM_KIND: &str = "xterm-ghostty";

/// What the terminal says it is when `xterm-ghostty`'s entry is nowhere to be
/// found: claiming a name whose terminfo is absent makes ncurses refuse to
/// start (`cannot initialize terminal type`), which is worse than describing
/// the terminal a little short. The entry present on every machine is xterm's.
pub const FALLBACK_TERM: &str = "xterm-256color";

/// Where ncurses looks for the entry, by store path.
///
/// The session does define `TERMINFO_DIRS` (NixOS sets it for every profile),
/// and this is the floor for when it does not: the nested loop on a host that
/// is not NixOS, and any unit started outside a login session. Set, it wins;
/// absent, the variable stays as the environment left it.
const TERMINFO_DIRS: Option<&str> = option_env!("RONOCTUA_TERMINFO_DIRS");

/// The program that starts when nobody says which.
///
/// `$SHELL` is what the person chose; `/bin/sh` is what always exists.
#[must_use]
pub fn default_program() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
}

/// A terminal's size: cells, and the pixels of one cell.
///
/// The pixels reach the program as `ws_xpixel`/`ws_ypixel` (the whole text
/// area: cells times a cell), which is what image tools size by. Zero means
/// "not known", which is what the kernel says when nobody tells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Columns.
    pub columns: u16,
    /// Rows.
    pub rows: u16,
    /// One cell's width in physical pixels.
    pub cell_width_px: u16,
    /// One cell's height in physical pixels.
    pub cell_height_px: u16,
}

impl Size {
    /// A size in cells, with the pixels unknown.
    #[must_use]
    pub const fn cells(columns: u16, rows: u16) -> Self {
        Self {
            columns,
            rows,
            cell_width_px: 0,
            cell_height_px: 0,
        }
    }

    /// The kernel's form of it.
    const fn winsize(self) -> rustix::termios::Winsize {
        rustix::termios::Winsize {
            ws_row: self.rows,
            ws_col: self.columns,
            // Saturating: a text area wider than 65 535 pixels is not a size
            // the field can hold, and a wrapped one would be a lie.
            ws_xpixel: self.columns.saturating_mul(self.cell_width_px),
            ws_ypixel: self.rows.saturating_mul(self.cell_height_px),
        }
    }
}

/// What a program is started with.
#[derive(Debug, Clone, Copy)]
pub struct Spawn<'a> {
    /// The program.
    pub program: &'a str,
    /// Its arguments.
    pub args: &'a [String],
    /// The size it is born with.
    pub size: Size,
    /// Where it starts, or `None` for this process's directory.
    pub cwd: Option<&'a Path>,
    /// The terminal's id, exported as `RONOCTUA_TERMINAL` — only for a
    /// terminal a holder keeps, which the census and the agents' hook can
    /// find again. An in-process terminal passes `None`, and the variable is
    /// REMOVED, so one opened from inside a held terminal does not claim its
    /// parent's id.
    pub terminal: Option<&'a str>,
    /// The distro's version, as `TERM_PROGRAM_VERSION`.
    pub version: &'a str,
}

/// A pseudoterminal with a child process on the other side.
pub struct Pty {
    /// The host's side. It reads the child's output and writes what is typed.
    master: OwnedFd,
    child: std::process::Child,
}

impl Pty {
    /// Opens a PTY and starts the program inside it.
    ///
    /// # Errors
    ///
    /// The system refuses to open the pty, or the program does not exist.
    pub fn spawn(spawn: &Spawn<'_>) -> Result<Self> {
        use rustix::pty::OpenptFlags;

        // `NOCTTY`: the MASTER must not become this process's controlling
        // terminal. `CLOEXEC`: it must not reach the child either (see the
        // top of the module).
        let master =
            rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC)
                .context("could not open a pty")?;
        rustix::pty::grantpt(&master).context("grantpt")?;
        rustix::pty::unlockpt(&master).context("unlockpt")?;

        let name = rustix::pty::ptsname(&master, Vec::new()).context("ptsname")?;
        let name = name.to_str().context("the pty's name is not UTF-8")?;

        // The size goes in BEFORE the spawn: a shell born thinking the
        // screen is 80x24 and only afterwards receiving `SIGWINCH` has already printed the prompt
        // at the wrong width.
        rustix::termios::tcsetwinsize(master.as_fd(), spawn.size.winsize())
            .context("tell the pty the size")?;

        // `NOCTTY` on the SLAVE too, and this one is not a formality: a
        // holder is a session's leader with no controlling terminal (it was
        // started through `setsid --fork`, as a unit's process is by
        // systemd), and the kernel gives such a process the first terminal it
        // opens without the flag. The slave would become the HOLDER's, and
        // the child's `setsid --ctty` would be refused it (`EPERM`: it
        // belongs to another session) and exit before the program ran.
        let secondary = std::fs::File::from(
            rustix::fs::open(
                name,
                rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NOCTTY | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .with_context(|| format!("open {name}"))?,
        );

        let mut assembled = std::process::Command::new(SETSID_PATH);
        for (key, value) in
            environment(spawn, std::env::var_os("TERMINFO_DIRS"), &terminfo_search())
        {
            match value {
                Some(v) => assembled.env(key, v),
                None => assembled.env_remove(key),
            };
        }
        if let Some(dir) = spawn.cwd {
            // `PWD` too: it is this process's, inherited, and a program that
            // trusts it (a shell does when it names the directory it is in)
            // would report the wrong one.
            assembled.current_dir(dir).env("PWD", dir);
        }
        let child = assembled
            // `--ctty` makes stdin (the slave) the CONTROLLING terminal. See
            // the top of the module.
            .arg("--ctty")
            .arg(spawn.program)
            .args(spawn.args)
            .stdin(secondary.try_clone().context("clone the secondary")?)
            .stdout(secondary.try_clone().context("clone the secondary")?)
            .stderr(secondary)
            .spawn()
            .with_context(|| format!("could not run `{}` in the pty", spawn.program))?;

        // The slave goes out of scope here, and that is on purpose: as long as WE
        // keep it open, reading the master never returns end of file — the
        // terminal would stay alive forever after the child exits.
        Ok(Self { master, child })
    }

    /// Opens a PTY for an in-process terminal: no id to export, the pixels
    /// unknown, this process's directory and version.
    ///
    /// # Errors
    ///
    /// The system refuses to open the pty, or the program does not exist.
    pub fn open(program: &str, args: &[String], columns: u16, lines: u16) -> Result<Self> {
        Self::spawn(&Spawn {
            program,
            args,
            size: Size::cells(columns, lines),
            cwd: None,
            terminal: None,
            version: env!("CARGO_PKG_VERSION"),
        })
    }

    /// The program's output on a descriptor of its own, for a thread that
    /// does nothing but read it — the holder's.
    ///
    /// # Errors
    ///
    /// The master could not be duplicated.
    pub fn output(&self) -> Result<Output> {
        Ok(Output {
            file: std::fs::File::from(self.master.try_clone().context("clone the master")?),
        })
    }

    /// Typing on a descriptor of its own, for a thread that does nothing but
    /// write it — so a program that stops reading its input blocks that
    /// thread and never the one that reads its output (the two would
    /// otherwise wait on each other forever).
    ///
    /// # Errors
    ///
    /// The master could not be duplicated.
    pub fn input(&self) -> Result<Input> {
        Ok(Input {
            file: std::fs::File::from(self.master.try_clone().context("clone the master")?),
        })
    }

    /// The program's process id: the leader of its session, so the group
    /// a hang-up reaches.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// How the program ended, a shell's way — its exit status, or 128 plus
    /// the signal that killed it — or `None` while it runs. Reaps it.
    ///
    /// # Errors
    ///
    /// Could not query the child.
    pub fn exit_code(&mut self) -> Result<Option<i32>> {
        use std::os::unix::process::ExitStatusExt as _;
        Ok(self
            .child
            .try_wait()
            .context("ask after the child")?
            .map(|st| st.code().unwrap_or_else(|| 128 + st.signal().unwrap_or(0))))
    }

    /// Sends bytes to the program. It is what "typing" means.
    ///
    /// # Errors
    ///
    /// The child closed the pty.
    pub fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        let mut f = std::fs::File::from(self.master.try_clone().context("clone the master")?);
        f.write_all(bytes).context("write to the pty")?;
        f.flush().context("drain the pty")
    }

    /// Tells the program the new size; the kernel sends it `SIGWINCH`.
    ///
    /// # Errors
    ///
    /// The `ioctl` failed.
    pub fn resize(&self, size: Size) -> Result<()> {
        rustix::termios::tcsetwinsize(self.master.as_fd(), size.winsize())
            .context("tell the pty the size")
    }

    /// Reads the program's output in a loop, calling `ver` on each chunk.
    ///
    /// It blocks until the child closes the pty **or** until `ver` returns
    /// [`Stream::Para`]. Made to run on a thread of its own — and that is why
    /// [`Pty::write_bytes`] clones the descriptor instead of taking `&mut self`.
    ///
    /// # Why `ver` decides when to stop
    ///
    /// A loop that only ends when the process does cannot be interrupted
    /// from outside: `read` blocks, and there is nothing to signal. Whoever consumes the
    /// bytes is who knows they have seen enough — and without that any test
    /// against a program that does not exit (a shell, a `cat`) hangs.
    ///
    /// # Errors
    ///
    /// A read error that is not the normal end.
    pub fn pump(&self, mut ver: impl FnMut(&[u8]) -> Stream) -> Result<()> {
        let mut output = self.output()?;
        let mut buf = [0u8; 4096];
        while let Some(n) = output.read(&mut buf)? {
            if ver(&buf[..n]) == Stream::Para {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Is the program still running?
    ///
    /// # Errors
    ///
    /// Could not query the child.
    pub fn alive_flag(&mut self) -> Result<bool> {
        Ok(self
            .child
            .try_wait()
            .context("ask after the child")?
            .is_none())
    }

    /// Kills the program and waits. Idempotent.
    ///
    /// # Errors
    ///
    /// Could not wait for the child.
    pub fn finish(&mut self) -> Result<()> {
        let _ = self.child.kill();
        let _ = self.child.wait().context("wait for the child")?;
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // Without this, closing a terminal would leave the shell running until the end of the
        // session — one zombie per terminal opened, invisible on screen.
        let _ = self.finish();
    }
}

/// The program's output, on a descriptor of its own ([`Pty::output`]).
pub struct Output {
    file: std::fs::File,
}

impl Output {
    /// Reads what the program wrote: `Some(n)` bytes, or `None` at the end —
    /// every process on the other side closed it.
    ///
    /// # Errors
    ///
    /// A read error that is not the normal end.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        loop {
            match self.file.read(buf) {
                Ok(0) => return Ok(None),
                Ok(n) => return Ok(Some(n)),
                // EIO on the master is how Linux says "the last process on the
                // other side exited". It is not an error: it is the normal end
                // of every terminal that closes.
                Err(e) if e.raw_os_error() == Some(5) => return Ok(None),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e).context("read the pty"),
            }
        }
    }
}

/// Typing, on a descriptor of its own ([`Pty::input`]).
pub struct Input {
    file: std::fs::File,
}

impl Input {
    /// Writes all of `bytes` to the program — blocking while it does not read.
    ///
    /// # Errors
    ///
    /// The program's side is gone.
    pub fn write_all(&mut self, bytes: &[u8]) -> Result<()> {
        self.file.write_all(bytes).context("write to the pty")
    }
}

/// What [`Pty::pump`] does after delivering a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// Keep reading.
    Follow,
    /// Stop the loop. The process goes on running.
    Para,
}

/// What the program's environment gets: a value to set, or `None` to remove.
///
/// The terminal's own names are SET, not left as the environment had them:
/// the environment is this process's, inherited from whatever started it —
/// another terminal's `TERM` when the loop runs inside one — and it describes
/// that terminal, not this. A person who wants another `TERM` says so in
/// their shell's rc, which runs after this.
///
/// A pure function of its inputs, so the policy is tested as data.
fn environment(
    spawn: &Spawn<'_>,
    inherited_terminfo_dirs: Option<OsString>,
    search: &[PathBuf],
) -> Vec<(&'static str, Option<OsString>)> {
    let term = if has_terminfo(TERM_KIND, search) {
        TERM_KIND
    } else {
        tracing::warn!(
            wanted = TERM_KIND,
            using = FALLBACK_TERM,
            "no terminfo entry for the terminal's own name; describing it as xterm"
        );
        FALLBACK_TERM
    };
    let mut out = vec![
        ("TERM", Some(OsString::from(term))),
        ("COLORTERM", Some(OsString::from("truecolor"))),
        ("TERM_PROGRAM", Some(OsString::from("ronoctua"))),
        ("TERM_PROGRAM_VERSION", Some(OsString::from(spawn.version))),
        ("RONOCTUA_TERMINAL", spawn.terminal.map(OsString::from)),
        // A WINDOW'S activation token is the window's, once: a program in the
        // terminal that opened a window of its own with it would steal the
        // focus as if it were the terminal being opened.
        ("XDG_ACTIVATION_TOKEN", None),
        ("DESKTOP_STARTUP_ID", None),
    ];
    if let (None, Some(dirs)) = (inherited_terminfo_dirs, TERMINFO_DIRS) {
        out.push(("TERMINFO_DIRS", Some(OsString::from(dirs))));
    }
    out
}

/// The directories ncurses would search for an entry, in its order:
/// `$TERMINFO`, `~/.terminfo`, each of `$TERMINFO_DIRS` (or the floor when
/// that is unset), and the system's.
fn terminfo_search() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(t) = std::env::var_os("TERMINFO") {
        dirs.push(PathBuf::from(t));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".terminfo"));
    }
    let listed = std::env::var_os("TERMINFO_DIRS").or_else(|| TERMINFO_DIRS.map(OsString::from));
    if let Some(listed) = listed {
        dirs.extend(std::env::split_paths(&listed).filter(|p| !p.as_os_str().is_empty()));
    }
    dirs.push(PathBuf::from("/usr/share/terminfo"));
    dirs
}

/// Whether a terminfo entry exists in one of `dirs`, in either of the two
/// layouts ncurses writes: by first letter (`x/xterm-ghostty`) or by its hex
/// code (`78/xterm-ghostty`).
fn has_terminfo(name: &str, dirs: &[PathBuf]) -> bool {
    let Some(first) = name.bytes().next() else {
        return false;
    };
    let by_letter = char::from(first).to_string();
    let by_hex = format!("{first:02x}");
    dirs.iter()
        .any(|d| d.join(&by_letter).join(name).is_file() || d.join(&by_hex).join(name).is_file())
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::{Pty, Size, Spawn, Stream, environment, has_terminfo};

    /// Reads until it sees `needle`, or until the time ceiling.
    ///
    /// It stops the loop as soon as it finds it: the program may not finish on its own (a
    /// `cat`, a shell), and waiting for the end would hang the test.
    fn read_until(pty: &Pty, needle: &str) -> String {
        let limit = std::time::Duration::from_secs(20);
        let start = std::time::Instant::now();
        let mut everything = String::new();
        let r = pty.pump(|b| {
            everything.push_str(&String::from_utf8_lossy(b));
            if everything.contains(needle) || start.elapsed() > limit {
                Stream::Para
            } else {
                Stream::Follow
            }
        });
        assert!(r.is_ok(), "pumping failed: {r:?}");
        everything
    }

    fn spawn<'a>(program: &'a str, args: &'a [String]) -> Spawn<'a> {
        Spawn {
            program,
            args,
            size: Size::cells(40, 10),
            cwd: None,
            terminal: None,
            version: "9.9",
        }
    }

    #[test]
    fn the_program_runs_and_the_output_comes_back_through_the_pty() {
        let mut pty = Pty::open("sh", &["-c".into(), "echo ronoctua-pty-ok".into()], 40, 10)
            .expect("open the pty");
        let seen_at = read_until(&pty, "ronoctua-pty-ok");
        pty.finish().unwrap();
        assert!(
            seen_at.contains("ronoctua-pty-ok"),
            "the pty gave back {seen_at:?}"
        );
    }

    #[test]
    fn what_is_written_reaches_the_program() {
        // `cat` returns what it receives. It is the shortest test that proves the
        // INPUT path exists — without it the terminal would be just a viewer.
        let mut pty = Pty::open("cat", &[], 40, 10).expect("open the pty");
        pty.write_bytes(b"echo-back\n").unwrap();
        let seen_at = read_until(&pty, "echo-back");
        pty.finish().unwrap();
        assert!(
            seen_at.contains("echo-back"),
            "the pty gave back {seen_at:?}"
        );
    }

    #[test]
    fn the_size_reaches_the_program() {
        // The shell only knows the screen's width through the `ioctl`. If the size were
        // not applied BEFORE the spawn, `stty size` would say 24 80 — the default
        // — and the prompt would be born at the wrong width.
        let mut pty =
            Pty::open("sh", &["-c".into(), "stty size".into()], 37, 11).expect("open the pty");
        let seen_at = read_until(&pty, "11 37");
        pty.finish().unwrap();
        assert!(seen_at.contains("11 37"), "`stty size` said {seen_at:?}");
    }

    #[test]
    fn the_child_dies_with_the_pty() {
        // Without the `Drop`, every closed terminal would leave a shell running until
        // the end of the session — invisible, and only noticed through memory.
        let mut pty = Pty::open("sleep", &["300".into()], 20, 5).expect("open the pty");
        assert!(pty.alive_flag().unwrap());
        pty.finish().unwrap();
        assert!(
            !pty.alive_flag().unwrap(),
            "the child survived the shutdown"
        );
    }

    #[test]
    fn the_master_is_close_on_exec() {
        // FAILS IF the master leaks into the shell: then a holder that dies
        // cannot hang its shell up, because the shell's own children keep the
        // master open. Asked twice — of the descriptor, and of what the child
        // actually holds open.
        let args = [
            "-c".to_owned(),
            "for f in /proc/$$/fd/*; do readlink \"$f\"; done; echo fds-listed".to_owned(),
        ];
        let mut pty = Pty::spawn(&spawn("sh", &args)).expect("open the pty");
        let flags = rustix::io::fcntl_getfd(&pty.master).expect("the master's flags");
        assert!(flags.contains(rustix::io::FdFlags::CLOEXEC), "{flags:?}");
        let listed = read_until(&pty, "fds-listed");
        pty.finish().unwrap();
        assert!(
            !listed.contains("ptmx"),
            "the child holds the master: {listed:?}"
        );
    }

    #[test]
    fn pixels_reach_the_winsize() {
        // FAILS IF the cell's pixels are dropped again: `ws_xpixel` and
        // `ws_ypixel` were always 0, and image tools that size by them drew
        // nothing.
        let mut pty = Pty::spawn(&Spawn {
            size: Size {
                columns: 30,
                rows: 7,
                cell_width_px: 9,
                cell_height_px: 18,
            },
            ..spawn("sleep", &["300".to_owned()])
        })
        .expect("open the pty");
        let born = rustix::termios::tcgetwinsize(&pty.master).unwrap();
        assert_eq!(
            (born.ws_col, born.ws_row, born.ws_xpixel, born.ws_ypixel),
            (30, 7, 270, 126)
        );
        pty.resize(Size {
            columns: 10,
            rows: 4,
            cell_width_px: 20,
            cell_height_px: 40,
        })
        .unwrap();
        let resized = rustix::termios::tcgetwinsize(&pty.master).unwrap();
        assert_eq!(
            (
                resized.ws_col,
                resized.ws_row,
                resized.ws_xpixel,
                resized.ws_ypixel
            ),
            (10, 4, 200, 160)
        );
        pty.finish().unwrap();
    }

    #[test]
    fn the_program_starts_where_it_is_told() {
        // FAILS IF the working directory is ignored: `C-S-t` opens the new
        // terminal in the front one's folder, and a `Path=` entry names one.
        let args = ["-c".to_owned(), "pwd; echo pwd-said".to_owned()];
        let dir = std::env::temp_dir();
        let mut pty = Pty::spawn(&Spawn {
            cwd: Some(&dir),
            ..spawn("sh", &args)
        })
        .expect("open the pty");
        let said = read_until(&pty, "pwd-said");
        pty.finish().unwrap();
        let canonical = dir.canonicalize().unwrap_or_else(|_| dir.clone());
        assert!(
            said.contains(dir.to_str().unwrap()) || said.contains(canonical.to_str().unwrap()),
            "{said:?} is not in {dir:?}"
        );
    }

    #[test]
    fn the_program_is_told_which_terminal_it_runs_in() {
        // FAILS IF a program sees another terminal's TERM, no COLORTERM (no
        // truecolor), or a held terminal's id missing — the agents' hook and
        // the shell's marks key on `RONOCTUA_TERMINAL`.
        let args = [
            "-c".to_owned(),
            "echo \"[$COLORTERM|$TERM_PROGRAM|$TERM_PROGRAM_VERSION|$RONOCTUA_TERMINAL]\""
                .to_owned(),
        ];
        let mut pty = Pty::spawn(&Spawn {
            terminal: Some("0123456789abcdef"),
            ..spawn("sh", &args)
        })
        .expect("open the pty");
        let said = read_until(&pty, "]");
        pty.finish().unwrap();
        assert!(
            said.contains("[truecolor|ronoctua|9.9|0123456789abcdef]"),
            "{said:?}"
        );
    }

    #[test]
    fn a_programs_end_is_its_code_or_its_signal_a_shells_way() {
        // FAILS IF a program killed by a signal reads as having exited with
        // 0 — the view would say "ended normally" of a terminal that crashed.
        let mut pty = Pty::open("sh", &["-c".into(), "exit 7".into()], 20, 5).unwrap();
        let _ = read_until(&pty, "never said");
        let mut code = None;
        let start = std::time::Instant::now();
        while code.is_none() && start.elapsed() < std::time::Duration::from_secs(20) {
            code = pty.exit_code().unwrap();
            std::thread::yield_now();
        }
        assert_eq!(code, Some(7));
        let mut killed = Pty::open("sleep", &["300".into()], 20, 5).unwrap();
        killed.finish().unwrap();
        assert_eq!(killed.exit_code().unwrap(), Some(128 + 9));
    }

    #[test]
    fn a_windows_activation_token_is_not_the_programs() {
        // FAILS IF a program started in the terminal inherits the token the
        // window was opened with: it would take the focus as that window.
        let plan = environment(&spawn("sh", &[]), None, &[]);
        assert!(plan.contains(&("XDG_ACTIVATION_TOKEN", None)), "{plan:?}");
        assert!(plan.contains(&("DESKTOP_STARTUP_ID", None)), "{plan:?}");
    }

    #[test]
    fn an_in_process_terminal_claims_no_id() {
        // FAILS IF a terminal opened from INSIDE a held one inherits its id:
        // the hook would file this terminal's agent under its parent.
        let plan = environment(&spawn("sh", &[]), None, &[]);
        assert!(
            plan.contains(&("RONOCTUA_TERMINAL", None)),
            "the id is not removed: {plan:?}"
        );
    }

    #[test]
    fn the_terminal_names_itself_only_where_its_terminfo_exists() {
        // FAILS IF `TERM=xterm-ghostty` is exported where no entry exists —
        // ncurses programs then die at once — or if an existing entry is not
        // found under either of ncurses' two layouts.
        let none: Vec<PathBuf> = Vec::new();
        let plan = environment(&spawn("sh", &[]), Some(OsString::from("/x")), &none);
        assert!(plan.contains(&("TERM", Some(OsString::from(super::FALLBACK_TERM)))));
        assert!(
            !plan.iter().any(|(k, _)| *k == "TERMINFO_DIRS"),
            "an inherited TERMINFO_DIRS is overridden"
        );

        let root = std::env::temp_dir().join(format!("ronoctua-terminfo-{}", std::process::id()));
        for layout in ["x", "78"] {
            let dir = root.join(layout);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(super::TERM_KIND), b"entry").unwrap();
            assert!(
                has_terminfo(super::TERM_KIND, std::slice::from_ref(&root)),
                "{layout}"
            );
            let plan = environment(&spawn("sh", &[]), None, std::slice::from_ref(&root));
            assert!(plan.contains(&("TERM", Some(OsString::from(super::TERM_KIND)))));
            std::fs::remove_dir_all(&dir).unwrap();
        }
        let _ = std::fs::remove_dir_all(&root);
    }
}
