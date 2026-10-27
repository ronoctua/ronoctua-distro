//! `ronoctua-ssh-askpass` / `ronoctua-sudo-askpass` — what ssh and sudo run as
//! their askpass to ask a secret in the footer's OWN bar
//! (`ronoctua_shell_footer::sudo_bar`'s ask), never a floating window.
//!
//! ONE PROGRAM, TWO NAMES, MANY KINDS. Its own name says sudo (the system
//! password) from ssh; and WHICH ssh ask it is — a key's passphrase, a
//! security key's touch or PIN, a host's authenticity, a login password — is
//! read from OpenSSH's `SSH_ASKPASS_PROMPT` and the prompt's words
//! (`ssh_kind`), so the bar titles each as what it is and ONLY a passphrase as
//! a passphrase. It writes one line to the footer's control socket —
//! `askpass <kind> <context> <prompt>`
//! (`ronoctua_shell_footer::leader::ASKPASS`, spelled again here: built on its
//! own, with no crate to share the word from) — and reads one line back: the
//! secret, which it prints and nothing else. A closed connection — the person
//! cancelled, or there is no footer — prints nothing and ends in failure, so
//! ssh/sudo gives up cleanly.
//!
//! THE CONTEXT, FOR SUDO ONLY: what command sudo is about to run and where it
//! was asked, so the bar can show for WHAT and from WHERE root is being asked
//! — an agent's `sudo` in a terminal reaches the person at the bar, and they
//! see the command before they type. It is read from `/proc` (the parent is
//! sudo, its parent the shell), base64-encoded so it is one space-free token,
//! and `-` when there is none (every ssh ask). Dependency-free, best-effort:
//! anything `/proc` will not give drops that part.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

/// The footer's control socket (`ronoctua_shell_kit::ipc::socket_path`,
/// spelled again — no `/tmp` fallback, by section 9).
fn socket_path() -> Option<PathBuf> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")?;
    let display = std::env::var("WAYLAND_DISPLAY").unwrap_or_else(|_| "wayland-0".to_owned());
    Some(PathBuf::from(dir).join(format!("ronoctua-{display}.sock")))
}

/// Standard base64 (no padding), so the context is one token the socket's
/// line-split keeps whole. The footer decodes it (`crate::leader`).
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6 & 63) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[(n & 63) as usize] as char);
        }
    }
    out
}

/// A process's parent pid, from `/proc/<pid>/status` (`PPid:`).
fn ppid_of(pid: &str) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("PPid:"))
        .map(|v| v.trim().to_owned())
}

/// A process's command name, from `/proc/<pid>/comm`.
fn comm_of(pid: &str) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// The post-`comm` fields of `/proc/<pid>/stat`, split after the LAST `)` so a
/// comm with a space or a paren does not shift them. Field 1 is the parent
/// pid, field 4 the controlling terminal (`tty_nr`).
fn stat_fields(pid: &str) -> Option<Vec<String>> {
    let s = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let after = s.rsplit_once(')')?.1;
    Some(after.split_whitespace().map(str::to_owned).collect())
}

/// The controlling terminal's name from a `tty_nr`, Linux's device encoding
/// (`gnu_dev_major`/`gnu_dev_minor`): `pts/N` for the Unix98 pty majors, `ttyN`
/// for the console major, none for no terminal or a major this does not name.
/// The minor's high bits are MASKED out of `tty_nr >> 12`, not shifted up — a
/// shift would fold the major's own bits into the minor (a pts, major `0x88`,
/// came out `pts/2051` for `pts/3`).
fn tty_name(tty_nr: u64) -> Option<String> {
    if tty_nr == 0 {
        return None;
    }
    let major = (tty_nr >> 8) & 0xfff;
    let minor = (tty_nr & 0xff) | ((tty_nr >> 12) & !0xff);
    match major {
        136..=143 => Some(format!("pts/{}", (major - 136) * 256 + minor)),
        4 => Some(format!("tty{minor}")),
        _ => None,
    }
}

/// The command sudo will run, from its `/proc/<pid>/cmdline` — `argv[0]` (sudo
/// itself) dropped, then its leading options and the value of the few that
/// take one, so what is left is the command and its arguments.
fn command_of(pid: &str) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let args: Vec<String> = raw
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    // The sudo options that take a separate value — the flag AND the token
    // after it are dropped; every other `-x` is the flag alone.
    const TAKES_VALUE: &[&str] = &[
        "-u", "-g", "-p", "-h", "-C", "-r", "-t", "-T", "-R", "-D", "-U", "--user", "--group",
        "--prompt", "--host", "--close-from", "--role", "--type", "--chdir", "--other-user",
    ];
    let mut it = args.iter();
    it.next()?; // sudo itself
    let mut skip_value = false;
    while let Some(a) = it.next() {
        if skip_value {
            skip_value = false;
            continue;
        }
        if a.starts_with('-') {
            skip_value = TAKES_VALUE.contains(&a.as_str());
            continue;
        }
        let mut cmd = vec![a.clone()];
        cmd.extend(it.cloned());
        return Some(cmd.join(" "));
    }
    None
}

/// What sudo is about to run and where it was asked — this helper's parent is
/// sudo, and sudo's parent the shell that asked. `None` when `/proc` gives
/// neither.
fn sudo_context() -> Option<String> {
    let sudo = ppid_of("self")?;
    let command = command_of(&sudo);
    let fields = stat_fields(&sudo);
    // THE SHELL (sudo's parent) and the terminal it is on.
    let shell = fields
        .as_ref()
        .and_then(|f| f.get(1))
        .and_then(|gp| comm_of(gp));
    let tty = fields
        .as_ref()
        .and_then(|f| f.get(4))
        .and_then(|t| t.parse::<u64>().ok())
        .and_then(tty_name);
    let origin = match (shell, tty) {
        (Some(s), Some(t)) => Some(format!("{s} · {t}")),
        (Some(s), None) => Some(s),
        (None, Some(t)) => Some(t),
        (None, None) => None,
    };
    let joined = match (command, origin) {
        (Some(c), Some(o)) => format!("{c}  —  {o}"),
        (Some(c), None) => c,
        (None, Some(o)) => o,
        (None, None) => return None,
    };
    // One line: the bar draws the context in one row.
    Some(joined.replace(['\n', '\r', '\t'], " "))
}

/// The kind token the bar is titled by (`sudo_bar::Kind::of`). Only a request
/// that IS an SSH key's passphrase is called one; a security key's touch, its
/// PIN, a host's authenticity and a login password each say what they are, and
/// an ask none of these words name stays the neutral `ssh`.
///
/// OpenSSH's own signal leads: `SSH_ASKPASS_PROMPT=confirm` marks a yes/no (a
/// touch, a host's authenticity) rather than a secret; the prompt's words tell
/// the rest apart.
fn ssh_kind(prompt: &str, confirm: bool) -> &'static str {
    let p = prompt.to_ascii_lowercase();
    if confirm {
        if p.contains("presence") {
            "ssh-touch"
        } else {
            "ssh-host"
        }
    } else if p.contains("passphrase") {
        "ssh-passphrase"
    } else if p.contains("pin") {
        "ssh-pin"
    } else if p.contains("password") {
        "ssh-password"
    } else {
        "ssh"
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    // ssh/sudo pass the prompt as the one argument; keep it to a single line,
    // as the socket is line-based.
    let prompt = args
        .get(1)
        .map(|p| p.replace(['\n', '\r'], " "))
        .unwrap_or_default();
    // THE KIND. `…-sudo-askpass` asks the system password; otherwise it is an
    // ssh ask, and WHICH ssh ask is read from OpenSSH's `SSH_ASKPASS_PROMPT`
    // (`confirm` for a yes/no) and the prompt's words (`ssh_kind`), so the bar
    // titles a passphrase a passphrase and a touch a touch — never one as the
    // other. The footer titles the bar by this token (`sudo_bar::Kind`).
    let is_sudo = args
        .first()
        .and_then(|a0| a0.rsplit('/').next())
        .is_some_and(|name| name.contains("sudo"));
    let kind = if is_sudo {
        "sudo"
    } else {
        ssh_kind(&prompt, std::env::var("SSH_ASKPASS_PROMPT").as_deref() == Ok("confirm"))
    };
    // THE CONTEXT, base64, for sudo alone — `-` otherwise (no context, and the
    // sentinel a `/proc` that gave nothing falls back to).
    let context = if is_sudo {
        sudo_context().map(|c| base64(c.as_bytes()))
    } else {
        None
    };
    let context = context.unwrap_or_else(|| "-".to_owned());
    let Some(path) = socket_path() else {
        return ExitCode::FAILURE;
    };
    let Ok(mut stream) = UnixStream::connect(&path) else {
        return ExitCode::FAILURE;
    };
    if writeln!(stream, "askpass {kind} {context} {prompt}")
        .and_then(|()| stream.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    let mut line = String::new();
    match BufReader::new(&stream).read_line(&mut line) {
        // A secret came back on its own line (maybe empty, as some keys have
        // none): print just it.
        Ok(read) if read > 0 => {
            let secret = line.strip_suffix('\n').unwrap_or(&line);
            let mut out = std::io::stdout().lock();
            if writeln!(out, "{secret}").and_then(|()| out.flush()).is_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        // Cancelled, or no footer answered: nothing is printed, and ssh/sudo
        // gives up its prompt.
        _ => ExitCode::FAILURE,
    }
}
