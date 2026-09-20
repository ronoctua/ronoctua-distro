//! What Emacs is doing on this machine, read off `/proc`.
//!
//! # Why it is here and not in the brain
//!
//! Because this crate is the ONLY boundary with `/proc` — the same rule that
//! keeps `niri-ipc` inside `ronoctua-niri` and NetworkManager inside
//! `ronoctua-network`. The brain asks; it does not open `/proc/<pid>/cmdline`
//! itself, any more than it opens `/proc/pressure/cpu`.
//!
//! # What the three answers are, and why they are three
//!
//! Emacs has a shape that catches people out: one process can be a DAEMON that
//! loads the configuration once and holds it, a CLIENT that attaches to it in
//! milliseconds, or a STANDALONE that loads everything again in its own
//! process. The third looks exactly like the second from the outside — same
//! window, same buffers, same name in a task list — and behaves nothing like
//! it: its configuration is a separate copy, its buffers are not the session's,
//! and closing it loses them.
//!
//! So this answers all three separately, and the screen says which.

use std::path::{Path, PathBuf};

/// What was found running.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Emacsen {
    /// A process started with `--daemon` or `--fg-daemon`.
    pub daemon: bool,
    /// At least one `emacsclient`.
    pub clients: u16,
    /// An Emacs that is neither: it loaded its own copy of the configuration.
    pub standalone: u16,
}

/// Where to look. A parameter, so a test drives a planted tree.
#[derive(Debug, Clone)]
pub struct Places {
    /// Usually `/proc`.
    proc: PathBuf,
}

impl Default for Places {
    fn default() -> Self {
        Self {
            proc: PathBuf::from("/proc"),
        }
    }
}

impl Places {
    /// Somewhere else — what a test hands in.
    #[must_use]
    pub fn under(proc: PathBuf) -> Self {
        Self { proc }
    }
}

/// Walks the process table and says what kind of Emacs is running.
///
/// # What it does NOT do
///
/// Decide anything. Whether a standalone Emacs is worth mentioning, and what
/// to say about it, is the brain's and the screen's — this reports.
#[must_use]
pub fn look(p: &Places) -> Emacsen {
    let mut out = Emacsen::default();
    let Ok(entries) = std::fs::read_dir(&p.proc) else {
        return out;
    };
    for e in entries.flatten() {
        // A PID IS A DIRECTORY WHOSE NAME IS A NUMBER. `/proc` has plenty that
        // are not — `self`, `net`, `sys` — and reading `cmdline` out of those
        // answers nothing.
        let name = e.file_name();
        let Some(name) = name.to_str() else { continue };
        if !name.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let Some(argv) = cmdline(&e.path()) else {
            continue;
        };
        match kind_of(&argv) {
            Some(Kind::Daemon) => out.daemon = true,
            Some(Kind::Client) => out.clients = out.clients.saturating_add(1),
            Some(Kind::Standalone) => out.standalone = out.standalone.saturating_add(1),
            None => {}
        }
    }
    out
}

/// The three shapes one Emacs process can have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Daemon,
    Client,
    Standalone,
}

/// What a command line makes this process.
///
/// # Why the program is taken from the last path segment
///
/// Because `argv[0]` is whatever started it: `/usr/bin/emacs`, a store path,
/// or the bare word. Matching the whole of it would answer differently on a
/// NixOS machine and on the one this was written on, which is the kind of
/// difference that makes a check pass and a screen lie.
fn kind_of(argv: &[String]) -> Option<Kind> {
    let program = argv.first()?.rsplit('/').next()?;
    // `emacsclient` FIRST, because it starts with `emacs` and a prefix test in
    // the other order calls every client a standalone.
    if program == "emacsclient" {
        return Some(Kind::Client);
    }
    if program != "emacs" {
        return None;
    }
    // THE DAEMON SAYS SO IN ITS ARGUMENTS, in one of two spellings, and either
    // may carry a name after an `=`: `--daemon=server`.
    let daemon = argv.iter().skip(1).any(|a| {
        a == "--daemon"
            || a == "--fg-daemon"
            || a.starts_with("--daemon=")
            || a.starts_with("--fg-daemon=")
    });
    Some(if daemon {
        Kind::Daemon
    } else {
        Kind::Standalone
    })
}

/// One process's arguments, split on the NUL that `/proc` writes between them.
fn cmdline(dir: &Path) -> Option<Vec<String>> {
    let raw = std::fs::read(dir.join("cmdline")).ok()?;
    if raw.is_empty() {
        // A KERNEL THREAD, which has no command line at all. Not a failure.
        return None;
    }
    Some(
        raw.split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plant(root: &Path, pid: u32, argv: &[&str]) {
        let dir = root.join(pid.to_string());
        std::fs::create_dir_all(&dir).expect("mkdir");
        let mut raw = Vec::new();
        for a in argv {
            raw.extend_from_slice(a.as_bytes());
            raw.push(0);
        }
        std::fs::write(dir.join("cmdline"), raw).expect("write");
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-emacs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        dir
    }

    /// A CLIENT IS NOT A STANDALONE, and that is the whole point of this file.
    ///
    /// `emacsclient` begins with `emacs`, so a prefix test in the wrong order
    /// reports every client as a second copy of the editor — which is exactly
    /// the message this is here to avoid putting on somebody's screen.
    #[test]
    fn the_three_shapes_are_told_apart() {
        let root = scratch("shapes");
        plant(&root, 100, &["/nix/store/x/bin/emacs", "--fg-daemon"]);
        plant(&root, 101, &["emacsclient", "-c", "notes.org"]);
        plant(&root, 102, &["/usr/bin/emacs", "notes.org"]);
        // Not Emacs at all, and a kernel thread with no command line.
        plant(&root, 103, &["/bin/bash"]);
        std::fs::create_dir_all(root.join("104")).expect("mkdir");
        std::fs::write(root.join("104").join("cmdline"), b"").expect("write");
        // And a directory that is not a pid.
        std::fs::create_dir_all(root.join("self")).expect("mkdir");

        let found = look(&Places::under(root.clone()));
        assert!(found.daemon, "the daemon was not seen");
        assert_eq!(found.clients, 1, "a client was miscounted");
        assert_eq!(found.standalone, 1, "a standalone was miscounted");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The daemon's name does not change what it is.
    #[test]
    fn a_named_daemon_is_still_a_daemon() {
        assert_eq!(
            kind_of(&["emacs".into(), "--daemon=work".into()]),
            Some(Kind::Daemon)
        );
        assert_eq!(
            kind_of(&["emacs".into(), "--fg-daemon=work".into()]),
            Some(Kind::Daemon)
        );
        assert_eq!(kind_of(&["emacs".into()]), Some(Kind::Standalone));
        assert_eq!(kind_of(&["vim".into()]), None);
    }
}
