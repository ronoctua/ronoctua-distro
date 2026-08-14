//! The PTY: the terminal's other side — §7.10.
//!
//! # Division of labour
//!
//! [`crate::terminal::Terminal`] interprets bytes; this module produces and
//! consumes them. The engine never sees a file descriptor and this module never
//! sees a [`crate::Grid`] — that is what makes it possible to test each on its own, and what
//! leaves the VT thread free to swap engines without touching here.
//!
//! # Why `setsid --ctty`, and not `fork` + `TIOCSCTTY`
//!
//! Giving the child a CONTROLLING terminal requires two calls between the `fork` and
//! the `exec`. In Rust that is `Command::pre_exec`, which is `unsafe` — and this
//! crate is `forbid(unsafe_code)`, which is not fussiness: a mistake there runs in
//! the child, after the fork, in the one place in the program where nothing is safe.
//!
//! util-linux's `setsid` makes exactly those two calls, in a binary that
//! exists on every installation and that is audited by far more people. The cost is
//! one extra `exec` per terminal opened.
//!
//! Without a controlling terminal the shell is born with no job control: `Ctrl-C` does not
//! interrupt, `fg`/`bg` do not exist, and none of that gives an error — it just does not
//! work.

use std::io::{Read as _, Write as _};
use std::os::fd::{AsFd as _, OwnedFd};

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

/// The program that starts when nobody says which.
///
/// `$SHELL` is what the person chose; `/bin/sh` is what always exists.
#[must_use]
pub fn default_program() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned())
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
    pub fn open(program: &str, args: &[String], columns: u16, lines: u16) -> Result<Self> {
        use rustix::pty::OpenptFlags;

        // `NOCTTY`: the MASTER must not become the shell's controlling terminal —
        // if it did, what controlled the terminal would be the shell itself, and the
        // child would be left without one.
        let master = rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)
            .context("nao consegui abrir um pty")?;
        rustix::pty::grantpt(&master).context("grantpt")?;
        rustix::pty::unlockpt(&master).context("unlockpt")?;

        let name = rustix::pty::ptsname(&master, Vec::new()).context("ptsname")?;
        let name = name.to_str().context("o nome do pty nao e UTF-8")?;

        // The size goes in BEFORE the spawn: a shell born thinking the
        // screen is 80x24 and only afterwards receiving `SIGWINCH` has already printed the prompt
        // at the wrong width.
        resize_fd(master.as_fd(), columns, lines)?;

        let escravo = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(name)
            .with_context(|| format!("abrir {name}"))?;

        let child = std::process::Command::new(SETSID_PATH)
            // `--ctty` makes stdin (the slave) the CONTROLLING terminal. See
            // the top of the module.
            .arg("--ctty")
            .arg(program)
            .args(args)
            .stdin(escravo.try_clone().context("clonar o escravo")?)
            .stdout(escravo.try_clone().context("clonar o escravo")?)
            .stderr(escravo)
            .spawn()
            .with_context(|| format!("nao consegui rodar `{program}` no pty"))?;

        // The slave goes out of scope here, and that is on purpose: as long as WE
        // keep it open, reading the master never returns end of file — the
        // terminal would stay alive forever after the child exits.
        Ok(Self { master, child })
    }

    /// Sends bytes to the program. It is what "typing" means.
    ///
    /// # Errors
    ///
    /// The child closed the pty.
    pub fn write_bytes(&self, bytes: &[u8]) -> Result<()> {
        let mut f = std::fs::File::from(self.master.try_clone().context("clonar o mestre")?);
        f.write_all(bytes).context("escrever no pty")?;
        f.flush().context("esvaziar o pty")
    }

    /// Tells the program the new size.
    ///
    /// # Errors
    ///
    /// The `ioctl` failed.
    pub fn resize(&self, columns: u16, lines: u16) -> Result<()> {
        resize_fd(self.master.as_fd(), columns, lines)
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
        let mut f = std::fs::File::from(self.master.try_clone().context("clonar o mestre")?);
        let mut buf = [0u8; 4096];
        loop {
            match f.read(&mut buf) {
                Ok(0) => return Ok(()),
                Ok(n) => {
                    if ver(&buf[..n]) == Stream::Para {
                        return Ok(());
                    }
                }
                // EIO on the master is how Linux says "the last process on the
                // other side exited". It is not an error: it is the normal end of every
                // terminal that closes.
                Err(e) if e.raw_os_error() == Some(5) => return Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e).context("ler o pty"),
            }
        }
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
            .context("consultar o filho")?
            .is_none())
    }

    /// Kills the program and waits. Idempotent.
    ///
    /// # Errors
    ///
    /// Could not wait for the child.
    pub fn finish(&mut self) -> Result<()> {
        let _ = self.child.kill();
        let _ = self.child.wait().context("esperar o filho")?;
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

/// What [`Pty::pump`] does after delivering a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    /// Keep reading.
    Follow,
    /// Stop the loop. The process goes on running.
    Para,
}

fn resize_fd(fd: std::os::fd::BorrowedFd<'_>, columns: u16, lines: u16) -> Result<()> {
    let size = rustix::termios::Winsize {
        ws_row: lines,
        ws_col: columns,
        // Pixels: `SIGWINCH` carries all four fields, and no program
        // on our side uses the two pixel ones. Zero is what the kernel puts
        // when nobody says otherwise.
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    rustix::termios::tcsetwinsize(fd, size).context("informar o tamanho ao pty")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Pty, Stream};

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
        assert!(r.is_ok(), "bombear falhou: {r:?}");
        everything
    }

    #[test]
    fn the_program_runs_and_the_output_comes_back_through_the_pty() {
        let mut pty = Pty::open("sh", &["-c".into(), "echo ronoctua-pty-ok".into()], 40, 10)
            .expect("abrir o pty");
        let seen_at = read_until(&pty, "ronoctua-pty-ok");
        pty.finish().unwrap();
        assert!(
            seen_at.contains("ronoctua-pty-ok"),
            "o pty devolveu {seen_at:?}"
        );
    }

    #[test]
    fn what_is_written_reaches_the_program() {
        // `cat` returns what it receives. It is the shortest test that proves the
        // INPUT path exists — without it the terminal would be just a viewer.
        let mut pty = Pty::open("cat", &[], 40, 10).expect("abrir o pty");
        pty.write_bytes(b"eco-de-volta\n").unwrap();
        let seen_at = read_until(&pty, "eco-de-volta");
        pty.finish().unwrap();
        assert!(
            seen_at.contains("eco-de-volta"),
            "o pty devolveu {seen_at:?}"
        );
    }

    #[test]
    fn the_size_reaches_the_program() {
        // The shell only knows the screen's width through the `ioctl`. If the size were
        // not applied BEFORE the spawn, `stty size` would say 24 80 — the default
        // — and the prompt would be born at the wrong width.
        let mut pty =
            Pty::open("sh", &["-c".into(), "stty size".into()], 37, 11).expect("abrir o pty");
        let seen_at = read_until(&pty, "11 37");
        pty.finish().unwrap();
        assert!(seen_at.contains("11 37"), "`stty size` disse {seen_at:?}");
    }

    #[test]
    fn the_child_dies_with_the_pty() {
        // Without the `Drop`, every closed terminal would leave a shell running until
        // the end of the session — invisible, and only noticed through memory.
        let mut pty = Pty::open("sleep", &["300".into()], 20, 5).expect("abrir o pty");
        assert!(pty.alive_flag().unwrap());
        pty.finish().unwrap();
        assert!(!pty.alive_flag().unwrap(), "o filho sobreviveu ao encerrar");
    }
}
