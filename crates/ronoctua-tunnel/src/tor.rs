//! Tor, through the control socket it already exposes.
//!
//! # What authenticates, and what it costs
//!
//! Cookie authentication: the daemon writes 32 bytes to
//! `/run/tor/control_auth_cookie`, and whoever can read them may send
//! commands. `nix/modules/tor.nix` makes the file group-readable and puts the
//! owner in `tor` — there is no polkit action for this, and no password.
//!
//! Starting and stopping the SERVICE is a different question and does have a
//! polkit rule, the only one this whole plan adds: `manage-units` on
//! `tor.service`, for the owner, verbs `start` and `stop`. See the module.
//!
//! # The protocol, in the four lines that matter
//!
//! Control-port replies are `NNN-` for a line with more coming, `NNN+` for a
//! block ending in a lone `.`, and `NNN ` for the last one. Everything here
//! parses that shape and nothing else, and the parser is pure.

use std::io::{BufRead as _, BufReader, Write as _};
use std::os::unix::net::UnixStream;
use std::path::Path;

/// The unit this crate starts and stops. Read by the check that keeps the
/// polkit rule narrow — one place, so the two cannot drift apart.
pub const UNIT: &str = "tor.service";

/// Where the control socket is, unless the environment says otherwise.
pub const SOCKET: &str = match option_env!("RONOCTUA_TOR_CONTROL") {
    Some(s) => s,
    None => "/run/tor/control",
};

/// Where the cookie is.
pub const COOKIE: &str = match option_env!("RONOCTUA_TOR_COOKIE") {
    Some(s) => s,
    None => "/run/tor/control_auth_cookie",
};

/// Where systemd is, baked at build time.
///
/// By its STORE PATH and not by name: in a unit the `PATH` is minimal, and the
/// failure would show up far from the cause (AGENTS.md).
pub const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(p) => p,
    None => "systemctl",
};

/// The SOCKS address Tor listens on.
///
/// IT COMES FROM THE FLAKE NOW, and that is a fix rather than a tidy-up. It
/// used to be this literal alone, while `nix/modules/tor.nix` declared no
/// `SOCKSPort` at all — so the two agreed because the nixpkgs default happened
/// to be the same number. Nothing would have said otherwise the day either
/// moved: the row on screen would have named a port nothing listened on, and a
/// browser pointed at it would have failed with a connection refused that
/// blames the network.
///
/// The module now declares it, passes it here, and `tor-socks-is-declared-once`
/// compares the two in BOTH directions.
pub const SOCKS: &str = match option_env!("RONOCTUA_TOR_SOCKS") {
    Some(s) => s,
    None => "127.0.0.1:9050",
};

/// The directory whose torrc fragment this system owns.
///
/// `%include`d by the flake's torrc, group-writable by `tor`, created by
/// tmpfiles — the shape `/var/lib/ronoctua/keyd` already has for the keyboards.
/// See [`fragment`] for why the configuration lives in a file rather than in
/// `SETCONF`.
pub const FRAGMENT_DIR: &str = match option_env!("RONOCTUA_TOR_FRAGMENT") {
    Some(s) => s,
    None => "/var/lib/ronoctua/tor",
};

/// The file inside it. One file, rewritten whole.
pub const FRAGMENT: &str = "ronoctua.conf";

/// Whether the unit is up.
///
/// THE SECTION EXISTS BECAUSE OF THIS TYPE. Before it, the whole Tor section
/// of the network view lived behind `if let Some(scene)` — and the control
/// socket only answers when Tor is RUNNING. So a stopped Tor drew no heading,
/// no row and no key: there was no way to turn it on, and the screen did not
/// say the program existed. The owner reported exactly that.
///
/// `Absent` is not `Stopped`, and the difference reaches the screen: a machine
/// that never installed Tor has nothing to offer, and a row about it would be
/// a row about a program that is not there. Only `LoadState` tells them apart —
/// `is-active` answers `inactive` for both.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Standing {
    /// systemd says it is active.
    Running,
    /// systemd says it is not. THIS is the state that had no screen.
    Stopped,
    /// Coming up or going down.
    Moving,
    /// There is no such unit on this machine.
    Absent,
    /// `systemctl` could not be asked — a build sandbox, a container with no
    /// systemd. NOT the same as stopped, and the screen says a different thing.
    #[default]
    Unknown,
}

impl Standing {
    /// The id, which is the stem of the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Moving => "moving",
            Self::Absent => "absent",
            Self::Unknown => "unknown",
        }
    }

    /// Is there a unit here at all to act on?
    ///
    /// What the view asks before drawing the switch: `Absent` and `Unknown`
    /// both mean there is nothing to start, and for different reasons.
    #[must_use]
    pub const fn actionable(self) -> bool {
        matches!(self, Self::Running | Self::Stopped | Self::Moving)
    }

    /// Which way the switch points — `true` means "pressing it starts Tor".
    ///
    /// This replaces a hard-coded `false` at the one place the row was built.
    /// The comment beside that literal explained the defect it was fixing —
    /// reading the CIRCUIT as the switch — and fixed it by making the row
    /// stop-only, which left `Order::Tor(true)` unreachable from the whole
    /// shell. The unit's own standing is the answer that was missing.
    #[must_use]
    pub const fn starts(self) -> bool {
        matches!(self, Self::Stopped)
    }

    /// What `systemctl show -p LoadState -p ActiveState --value` said.
    ///
    /// Two lines, in that order.
    #[must_use]
    pub fn of(said: &str) -> Self {
        let mut lines = said.trim().lines();
        let loaded = lines.next().unwrap_or("").trim();
        let active = lines.next().unwrap_or("").trim();
        if loaded == "not-found" {
            return Self::Absent;
        }
        if loaded.is_empty() {
            return Self::Unknown;
        }
        match active {
            "active" => Self::Running,
            "inactive" | "failed" => Self::Stopped,
            "activating" | "deactivating" | "reloading" => Self::Moving,
            _ => Self::Unknown,
        }
    }
}

/// Asks systemd how the unit is.
///
/// TWO QUESTIONS IN ONE CALL. `is-active` answers `inactive` both for a unit
/// that is stopped and for one that DOES NOT EXIST; only `LoadState` tells
/// them apart. The exit code is not the answer either: `is-active` exits
/// non-zero for an inactive unit, so reading it would make "stopped" and
/// "systemctl is not here" the same thing.
#[must_use]
pub fn standing(systemctl: &Path) -> Standing {
    match std::process::Command::new(systemctl)
        .args([
            "show",
            "-p",
            "LoadState",
            "-p",
            "ActiveState",
            "--value",
            UNIT,
        ])
        .output()
    {
        Ok(out) => Standing::of(&String::from_utf8_lossy(&out.stdout)),
        Err(e) => {
            tracing::debug!(error = %e, "systemctl could not be asked about tor");
            Standing::Unknown
        }
    }
}

/// What Tor says, as of one reading.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scene {
    /// Is there a circuit? It is the one question that decides whether
    /// anything can go through, and it is the mark's rule.
    pub established: bool,
    /// How far the bootstrap got, 0 to 100.
    pub bootstrap: u8,
    /// The daemon's own summary of that step.
    pub summary: String,
    /// `0.4.8.12`.
    pub version: String,
    /// Bytes read and written since it started.
    pub received: u64,
    /// Likewise written.
    pub sent: u64,
    /// How many circuits are open.
    pub circuits: usize,
}

impl Scene {
    /// Is there anything worth a mark on the modeline?
    #[must_use]
    pub const fn marked(&self) -> bool {
        self.established
    }
}

/// Does this line END the reply? `inside` is the reader's own state.
///
/// # Why this needs state at all
///
/// Three digits and a space is the last line — EXCEPT inside a `NNN+` block,
/// where a row of the block can look exactly the same: a circuit whose id has
/// reached three digits reads `123 BUILT $aaa~one`. Ending there hands the
/// rest of the block back as the answer to the NEXT question, so a version
/// and two byte counts arrive as circuit rows — every ten seconds, and only
/// once a session's ids pass a hundred, which is what made it look
/// intermittent. A block ends at a line that is one full stop, and that is
/// the only thing that can end it.
#[must_use]
pub fn ends_reply(line: &str, inside: &mut bool) -> bool {
    let line = line.trim_end_matches(['\r', '\n']);
    if *inside {
        if line == "." {
            *inside = false;
        }
        return false;
    }
    let bytes = line.as_bytes();
    if bytes.len() <= 3 || !bytes[..3].iter().all(u8::is_ascii_digit) {
        return false;
    }
    match bytes[3] {
        // `NNN+` opens a block; `NNN-` is a line with more coming.
        b'+' => {
            *inside = true;
            false
        }
        b' ' => true,
        _ => false,
    }
}

/// How many circuits a `circuit-status` reply says are BUILT.
///
/// BY ROW AND NOT BY REPLY LINE: the whole block is one entry of
/// [`lines_of`], with its rows inside separated by newlines.
#[must_use]
pub fn built_in(reply: &str) -> usize {
    lines_of(reply)
        .into_iter()
        .map(|(_, line)| {
            line.split('\n')
                .filter(|row| row.split_whitespace().nth(1) == Some("BUILT"))
                .count()
        })
        .sum()
}

/// One reply, split into its lines with the codes taken off.
///
/// PURE: everything this module knows about the protocol is here, and it is
/// tested against replies copied from the specification rather than from a
/// running daemon.
#[must_use]
pub fn lines_of(reply: &str) -> Vec<(u16, String)> {
    let mut out: Vec<(u16, String)> = Vec::new();
    // INSIDE A BLOCK, EVERY LINE IS THE BLOCK'S — even one that begins with
    // three digits and a space. A circuit whose id reached three digits reads
    // `123 BUILT $aaa~one`, and treating that as a fresh reply line split one
    // answer into several: the count of built circuits came back short, and
    // `value_of` could find a key inside somebody else's block.
    let mut inside = false;
    let append = |out: &mut Vec<(u16, String)>, line: &str| {
        if let Some((_, last)) = out.last_mut() {
            last.push('\n');
            last.push_str(line);
        }
    };
    for line in reply.lines() {
        let line = line.trim_end_matches('\r');
        if inside {
            if line == "." {
                inside = false;
            } else {
                append(&mut out, line);
            }
            continue;
        }
        if line.len() < 4 {
            continue;
        }
        let (code, rest) = line.split_at(3);
        let Ok(code) = code.parse::<u16>() else {
            // A CONTINUATION with no code of its own.
            append(&mut out, line);
            continue;
        };
        // `NNN+` OPENS A BLOCK: what follows belongs to this entry until the
        // lone full stop.
        if rest.starts_with('+') {
            inside = true;
        }
        out.push((code, rest[1..].to_owned()));
    }
    out
}

/// The value of one `GETINFO` key in a reply.
#[must_use]
pub fn value_of(reply: &str, key: &str) -> Option<String> {
    let wanted = format!("{key}=");
    lines_of(reply)
        .into_iter()
        .find_map(|(_, line)| line.strip_prefix(&wanted).map(str::to_owned))
}

/// The percentage out of `NOTICE BOOTSTRAP PROGRESS=100 TAG=done SUMMARY="Done"`.
#[must_use]
pub fn bootstrap_of(line: &str) -> (u8, String) {
    let progress = line
        .split_whitespace()
        .find_map(|f| f.strip_prefix("PROGRESS="))
        .and_then(|n| n.parse::<u8>().ok())
        .unwrap_or(0);
    let summary = line
        .split_once("SUMMARY=\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(s, _)| s.to_owned())
        .unwrap_or_default();
    (progress, summary)
}

/// A conversation with the control socket, authenticated.
pub struct Control {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Control {
    /// Opens the socket and authenticates with the cookie.
    ///
    /// # Errors
    ///
    /// There is no socket, the cookie is not readable — which is what an
    /// owner outside the `tor` group meets — or the daemon refused.
    pub fn open(socket: &Path, cookie: &Path) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(socket)
            .map_err(|e| anyhow::anyhow!("{}: {e}", socket.display()))?;
        stream.set_read_timeout(Some(ronoctua_http::PATIENCE))?;
        stream.set_write_timeout(Some(ronoctua_http::PATIENCE))?;
        let reader = BufReader::new(stream.try_clone()?);
        let mut me = Self { stream, reader };
        let bytes =
            std::fs::read(cookie).map_err(|e| anyhow::anyhow!("{}: {e}", cookie.display()))?;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let answer = me.ask(&format!("AUTHENTICATE {hex}"))?;
        anyhow::ensure!(
            answer.starts_with("250"),
            "the control socket refused the cookie: {answer}"
        );
        Ok(me)
    }

    /// Sends one command and reads its whole reply.
    ///
    /// # Errors
    ///
    /// The socket closed, or the daemon did not answer inside the deadline.
    pub fn ask(&mut self, command: &str) -> anyhow::Result<String> {
        self.stream.write_all(command.as_bytes())?;
        self.stream.write_all(b"\r\n")?;
        self.stream.flush()?;
        let mut out = String::new();
        // WHETHER THE READER IS INSIDE A BLOCK — see `ends_reply`, which is
        // where the whole of this protocol's framing lives.
        let mut inside = false;
        loop {
            let mut line = String::new();
            if self.reader.read_line(&mut line)? == 0 {
                break;
            }
            let done = ends_reply(&line, &mut inside);
            out.push_str(&line);
            if done {
                break;
            }
        }
        Ok(out)
    }

    /// One `GETINFO`, already unwrapped.
    ///
    /// # Errors
    ///
    /// The socket closed, or the daemon did not answer.
    pub fn info(&mut self, key: &str) -> anyhow::Result<Option<String>> {
        let reply = self.ask(&format!("GETINFO {key}"))?;
        Ok(value_of(&reply, key))
    }

    /// Makes the running daemon re-read its torrc.
    ///
    /// This is how the fragment reaches a Tor that is already up, and it goes
    /// through the CONTROL SOCKET rather than through systemd on purpose:
    /// `systemctl reload` would need a third verb in the polkit rule, and
    /// `tor-polkit-is-narrow` refuses anything but `start` and `stop` — with a
    /// reason, since a wider rule is a wider door for ever. A signal on a
    /// socket the owner is already allowed to use costs nothing.
    ///
    /// # Errors
    ///
    /// The socket closed, or the daemon refused — which is what a torrc it
    /// cannot parse looks like, and the message carries its words.
    pub fn reload(&mut self) -> anyhow::Result<()> {
        let answer = self.ask("SIGNAL RELOAD")?;
        anyhow::ensure!(
            answer.starts_with("250"),
            "Tor would not reload its configuration: {}",
            answer.trim()
        );
        Ok(())
    }

    /// What the daemon says an option is set to, right now.
    ///
    /// The proof that a fragment was actually read: writing a file and
    /// signalling a reload are two things that can both succeed while the
    /// value never changes — a typo in the option name is silently ignored by
    /// Tor, and the screen would go on showing what the person typed.
    ///
    /// # Errors
    ///
    /// The socket closed. An option Tor does not know is `Ok(None)`.
    pub fn conf(&mut self, key: &str) -> anyhow::Result<Option<String>> {
        let reply = self.ask(&format!("GETCONF {key}"))?;
        if !reply.starts_with("250") {
            return Ok(None);
        }
        Ok(value_of(&reply, key))
    }

    /// A new identity — the daemon builds fresh circuits.
    ///
    /// # Errors
    ///
    /// The socket closed, or the daemon refused.
    pub fn new_identity(&mut self) -> anyhow::Result<()> {
        let answer = self.ask("SIGNAL NEWNYM")?;
        anyhow::ensure!(answer.starts_with("250"), "the daemon refused: {answer}");
        Ok(())
    }

    /// Everything the view draws, in one round of questions.
    ///
    /// # Errors
    ///
    /// The socket closed mid-conversation.
    pub fn look(&mut self) -> anyhow::Result<Scene> {
        let established = self.info("status/circuit-established")?.as_deref() == Some("1");
        let (bootstrap, summary) = self
            .info("status/bootstrap-phase")?
            .map(|l| bootstrap_of(&l))
            .unwrap_or_default();
        let number = |text: Option<String>| text.and_then(|t| t.trim().parse::<u64>().ok());
        // ONE LINE, MANY CIRCUITS: `lines_of` collapses a `NNN+` block into a
        // single entry with its rows inside — which is what a reader wants
        // and what a COUNT must not use. Counting the entries answered "is
        // any circuit open" while the field says "how many are".
        let circuits = self
            .ask("GETINFO circuit-status")
            .map(|r| built_in(&r))
            .unwrap_or(0);
        Ok(Scene {
            established,
            bootstrap,
            summary,
            version: self.info("version")?.unwrap_or_default(),
            received: number(self.info("traffic/read")?).unwrap_or(0),
            sent: number(self.info("traffic/written")?).unwrap_or(0),
            circuits,
        })
    }
}

/// Reads the daemon, when it is there. `None` is the ordinary case: this
/// distro does not start Tor at boot.
#[must_use]
pub fn look(socket: &Path, cookie: &Path) -> Option<Scene> {
    Control::open(socket, cookie).ok()?.look().ok()
}

/// The configuration this system writes, and how it reaches Tor.
///
/// # Why a file and not `SETCONF`
///
/// The control socket can change a running Tor's configuration, and that was
/// the obvious route. It is the wrong one twice over.
///
/// `SETCONF` does not survive a restart, so every setting would quietly revert
/// the next time the unit came up — which is the silent-undo failure this
/// project spends its checks hunting. `SAVECONF` does survive, and writes into
/// the torrc, which here is a file in `/nix/store` and read-only by
/// construction (invariant 6).
///
/// So the module `%include`s a directory this system owns, the view writes one
/// file into it, and `SIGNAL RELOAD` makes the running daemon re-read the lot.
/// The file is what survives the reboot; the signal is what makes the running
/// daemon agree with it — the same division `ronoctua-display` uses for the
/// compositor.
///
/// # And the flake still wins
///
/// `%include` comes out FIRST in the generated torrc, because Nix sorts
/// attribute names and `%` sorts before letters. Tor takes the LAST value it
/// sees for an option, so anything `nix/modules/tor.nix` declares overrides
/// what is written here. That is not an accident to work around — it is
/// invariant 6 on the screen: the flake keeps the cookie authentication, the
/// control socket and the SOCKS port, those rows are drawn REFUSING, and
/// everything else is the person's.
pub mod fragment {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    /// One option's lines. A `Vec` because torrc has LINELISTS: `Bridge`
    /// legitimately appears many times, and a single `String` would have made
    /// "one bridge" the only expressible number.
    pub type Lines = BTreeMap<String, Vec<String>>;

    /// Where the file goes.
    #[must_use]
    pub fn path(dir: &Path) -> PathBuf {
        dir.join(super::FRAGMENT)
    }

    /// Is this a name torrc could have?
    ///
    /// Letters and digits only, starting with a letter. It is not politeness:
    /// a key is written straight into the file, so anything that can hold a
    /// newline or a space is a way to write a directive nobody asked for.
    #[must_use]
    pub fn a_name(key: &str) -> bool {
        !key.is_empty()
            && key.starts_with(|c: char| c.is_ascii_alphabetic())
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    }

    /// Is this a value torrc could hold on one line?
    ///
    /// THE ONE THAT MATTERS. A bridge line is PASTED — the person copies it
    /// out of a browser or an e-mail — and a pasted value that carries a
    /// newline would end the `Bridge` directive and start whatever the rest of
    /// the paste says. `ExitPolicy accept *:*` on the end of a bridge address
    /// is a working torrc and a very different Tor.
    ///
    /// A carriage return counts, and so does a NUL: both end a line for some
    /// reader somewhere, and neither is visible when it is pasted.
    #[must_use]
    pub fn a_value(value: &str) -> bool {
        !value.contains(['\n', '\r', '\0'])
    }

    /// The fragment's text.
    ///
    /// Pure, so every rule above is testable with no Tor and no filesystem —
    /// which a build sandbox is.
    ///
    /// # Errors
    ///
    /// A key or a value that could not go in a torrc safely. It names the one
    /// it refused: a person who pasted something odd is owed the reason.
    pub fn render(lines: &Lines) -> anyhow::Result<String> {
        let mut out = String::from(
            "# Written by ronoctua. Edited by hand, this file is overwritten.\n\
             # What the flake declares wins over what is here: see\n\
             # `ronoctua_tunnel::tor::fragment`.\n",
        );
        for (key, values) in lines {
            anyhow::ensure!(a_name(key), "`{key}` is not a name torrc could have");
            for value in values {
                anyhow::ensure!(
                    a_value(value),
                    "`{key}` was given a value with a line break in it"
                );
                let value = value.trim();
                if value.is_empty() {
                    continue;
                }
                out.push_str(key);
                out.push(' ');
                out.push_str(value);
                out.push('\n');
            }
        }
        Ok(out)
    }

    /// Writes the fragment, atomically.
    ///
    /// Through a neighbouring temporary file and a rename, and the temporary
    /// carries the PID: two doors are two PROCESSES, and two opens of one name
    /// with truncate interleave into a file that is neither. The lesson is
    /// `ronoctua-store`'s, and it cost a `display.json` that would not parse.
    ///
    /// # Errors
    ///
    /// The directory is not writable — which is what an owner outside the
    /// `tor` group meets — or a value was refused by [`render`].
    pub fn write(dir: &Path, lines: &Lines) -> anyhow::Result<()> {
        let text = render(lines)?;
        let final_path = path(dir);
        let draft = dir.join(format!("{}.{}.new", super::FRAGMENT, std::process::id()));
        std::fs::write(&draft, text.as_bytes())
            .map_err(|e| anyhow::anyhow!("{}: {e}", draft.display()))?;
        // GROUP-READABLE, because it is TOR that reads it and this runs as the
        // owner. `0640` and not `0644`: nothing outside the group has business
        // reading which bridges this machine uses.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = std::fs::set_permissions(&draft, std::fs::Permissions::from_mode(0o640));
        }
        std::fs::rename(&draft, &final_path).map_err(|e| {
            let _ = std::fs::remove_file(&draft);
            anyhow::anyhow!("{}: {e}", final_path.display())
        })
    }

    /// Reads back what was written, as lines.
    ///
    /// Used to fill the view: the file is the truth about what this system
    /// asked for, and keeping a second copy in a preferences file would be two
    /// truths. Comments and blank lines are skipped; anything else is
    /// `key value`.
    #[must_use]
    pub fn read(dir: &Path) -> Lines {
        let mut out = Lines::new();
        let Ok(text) = std::fs::read_to_string(path(dir)) else {
            return out;
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once(char::is_whitespace) else {
                continue;
            };
            out.entry(key.to_owned())
                .or_default()
                .push(value.trim().to_owned());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_is_read_by_its_codes() {
        let reply = "250-status/bootstrap-phase=NOTICE BOOTSTRAP PROGRESS=100 TAG=done SUMMARY=\"Done\"\r\n250 OK\r\n";
        let lines = lines_of(reply);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0, 250);
        assert!(lines[0].1.starts_with("status/bootstrap-phase="));
        assert_eq!(lines[1].1, "OK");
    }

    #[test]
    fn the_value_and_the_progress_come_out_of_the_line() {
        let reply = "250-status/bootstrap-phase=NOTICE BOOTSTRAP PROGRESS=45 TAG=requesting_descriptors SUMMARY=\"Asking for relay descriptors\"\r\n250 OK\r\n";
        let line = value_of(reply, "status/bootstrap-phase").unwrap();
        let (progress, summary) = bootstrap_of(&line);
        assert_eq!(progress, 45);
        assert_eq!(summary, "Asking for relay descriptors");
        // AND A KEY THAT IS NOT THERE is None, not an empty string: the two
        // are different facts and only one of them is drawn.
        assert_eq!(value_of(reply, "version"), None);
    }

    #[test]
    fn a_circuit_that_is_not_established_is_the_honest_answer_offline() {
        // A MACHINE WITH NO WAY OUT answers `0`, and that is not a failure to
        // read — it is Tor saying it has no circuit, which is what the row
        // has to show rather than "unknown".
        let reply = "250-status/circuit-established=0\r\n250 OK\r\n";
        assert_eq!(
            value_of(reply, "status/circuit-established").as_deref(),
            Some("0")
        );
    }

    /// The framing, as the reader runs it: hand it the lines and say where it
    /// stopped. It is `Control::ask`'s loop with the socket taken out.
    fn framed(reply: &str) -> String {
        let mut inside = false;
        let mut out = String::new();
        for line in reply.split_inclusive('\n') {
            let done = ends_reply(line, &mut inside);
            out.push_str(line);
            if done {
                break;
            }
        }
        out
    }

    #[test]
    fn only_a_code_ends_a_reply() {
        // THE DEFECT THIS PREVENTS: a row inside a `250+` block whose circuit
        // id has three digits — `123 BUILT …` — has a space at byte 3 too.
        // Read as the end of the reply, the rest of the block was then
        // handed back as the answer to the NEXT question: a version and two
        // byte counts arriving as circuit rows, every ten seconds, once a
        // session's ids passed a hundred.
        let mut inside = false;
        assert!(ends_reply("250 OK\r\n", &mut inside));
        assert!(!ends_reply(
            "250-status/version=recommended\r\n",
            &mut inside
        ));
        assert!(!ends_reply("250\r\n", &mut inside));

        // AND THE WHOLE BLOCK IS READ, three-digit ids included.
        let reply = "250+circuit-status=\r\n\
1 BUILT $a~one\r\n\
123 BUILT $b~two\r\n\
.\r\n250 OK\r\n";
        let read = framed(reply);
        assert_eq!(read, reply, "the reader stopped inside the block");
        // AND THE NEXT ANSWER IS NOT SWALLOWED WITH IT.
        let two = format!("{reply}250-version=0.4.9.11\r\n250 OK\r\n");
        assert_eq!(framed(&two), reply, "it read past the end of the reply");
    }

    #[test]
    fn the_circuits_are_counted_by_row_and_not_by_reply() {
        // `lines_of` COLLAPSES A BLOCK into one entry on purpose — that is
        // what a reader wants — so counting entries answered "is any circuit
        // open" while the field says "how many are". Eight built circuits
        // came back as one.
        let reply = "250+circuit-status=\r\n\
1 BUILT $a~one,$b~two\r\n\
2 BUILT $c~three\r\n\
3 LAUNCHED $d~four\r\n\
123 BUILT $e~five\r\n\
.\r\n250 OK\r\n";
        assert_eq!(built_in(reply), 3, "a launched circuit is not a built one");
        assert_eq!(built_in("250-circuit-status=\r\n250 OK\r\n"), 0);
    }

    #[test]
    fn a_block_reply_keeps_its_lines_together() {
        // `NNN+` ends in a lone `.`, and the lines between belong to the key.
        let reply = "250+circuit-status=\r\n1 BUILT $aaa~one,$bbb~two\r\n2 BUILT $ccc~three\r\n.\r\n250 OK\r\n";
        let built = lines_of(reply)
            .into_iter()
            .filter(|(_, l)| l.contains("BUILT"))
            .count();
        assert_eq!(built, 1, "the block is one line with its rows inside");
        let whole = lines_of(reply)[0].1.clone();
        assert!(whole.contains("$ccc~three"), "a row was dropped: {whole}");
    }

    #[test]
    fn absent_stopped_and_unaskable_are_three_different_words() {
        // THE ONE THAT DECIDES WHAT THE SCREEN SAYS. `is-active` answers
        // `inactive` both for a unit that is stopped and for one that does not
        // exist, so a machine that never installed Tor would carry a switch
        // for a program it does not have. Only `LoadState` tells them apart.
        assert_eq!(Standing::of("loaded\nactive"), Standing::Running);
        assert_eq!(Standing::of("loaded\ninactive\n"), Standing::Stopped);
        assert_eq!(Standing::of("loaded\nfailed"), Standing::Stopped);
        assert_eq!(Standing::of("loaded\nactivating"), Standing::Moving);
        assert_eq!(Standing::of("loaded\ndeactivating"), Standing::Moving);
        assert_eq!(Standing::of("not-found\ninactive"), Standing::Absent);
        // AND A BUILD SANDBOX, where there is no systemd to ask at all.
        assert_eq!(Standing::of(""), Standing::Unknown);
        assert_eq!(Standing::of("loaded\nsomething new"), Standing::Unknown);
        assert_eq!(Standing::default(), Standing::Unknown);
        // EVERY ONE HAS ITS OWN WORD, because the screen says a different
        // sentence for each and a shared id would make two of them one.
        let ids = [
            Standing::Running,
            Standing::Stopped,
            Standing::Moving,
            Standing::Absent,
            Standing::Unknown,
        ]
        .map(Standing::id);
        let unique: std::collections::BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "two standings share one word");
    }

    #[test]
    fn a_pasted_bridge_cannot_smuggle_a_second_directive() {
        // THE ONE THAT MATTERS IN THIS FILE. A bridge line is PASTED — out of
        // a browser, out of an e-mail — and a pasted value carrying a newline
        // would end the `Bridge` directive and start whatever came after it.
        // `ExitPolicy accept *:*` on the end of a bridge address is a working
        // torrc and a very different Tor.
        let mut lines = fragment::Lines::new();
        lines.insert(
            "Bridge".to_owned(),
            vec!["obfs4 10.0.0.1:443 CERT=x\nExitPolicy accept *:*".to_owned()],
        );
        let said = fragment::render(&lines).unwrap_err().to_string();
        assert!(said.contains("Bridge"), "{said}");
        assert!(said.contains("line break"), "{said}");
        // A CARRIAGE RETURN AND A NUL COUNT TOO: both end a line for some
        // reader somewhere, and neither is visible when it is pasted.
        for bad in ["a\rb", "a\0b"] {
            assert!(!fragment::a_value(bad), "`{bad:?}` was accepted");
        }
        // AND AN ORDINARY BRIDGE IS FINE.
        assert!(fragment::a_value("obfs4 10.0.0.1:443 CERT=abc iat-mode=0"));
    }

    #[test]
    fn a_key_that_is_not_a_torrc_name_is_refused() {
        // A key goes straight into the file, so anything that can hold a space
        // or a newline is another way to write a directive nobody asked for.
        for bad in ["", "1Bridge", "Use Bridges", "Bridge\nExitPolicy", "-x"] {
            assert!(!fragment::a_name(bad), "`{bad:?}` was accepted as a name");
        }
        for good in [
            "UseBridges",
            "ExitNodes",
            "ClientTransportPlugin",
            "Socks5Proxy",
        ] {
            assert!(fragment::a_name(good), "`{good}` was refused as a name");
        }
    }

    #[test]
    fn a_linelist_comes_out_as_one_line_each() {
        // `Bridge` legitimately appears many times, and a single `String`
        // would have made "one bridge" the only expressible number.
        let mut lines = fragment::Lines::new();
        lines.insert("UseBridges".to_owned(), vec!["1".to_owned()]);
        lines.insert(
            "Bridge".to_owned(),
            vec!["obfs4 a:1 CERT=x".to_owned(), "obfs4 b:2 CERT=y".to_owned()],
        );
        // An empty value is DROPPED rather than written as a bare key: a
        // `ExitNodes` with nothing after it is a torrc parse error, and the
        // view produces one whenever a field is cleared.
        lines.insert("ExitNodes".to_owned(), vec!["  ".to_owned()]);
        let text = fragment::render(&lines).unwrap();
        assert!(text.contains("\nBridge obfs4 a:1 CERT=x\n"), "{text}");
        assert!(text.contains("\nBridge obfs4 b:2 CERT=y\n"), "{text}");
        assert!(text.contains("\nUseBridges 1\n"), "{text}");
        assert!(!text.contains("ExitNodes"), "{text}");
        // AND IT SAYS WHOSE FILE IT IS, because somebody will find it and
        // wonder whether editing it does anything.
        assert!(text.starts_with("# Written by ronoctua."), "{text}");
    }

    #[test]
    fn what_is_written_reads_back_as_what_went_in() {
        let dir = std::env::temp_dir().join(format!("ronoctua-torfrag-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut lines = fragment::Lines::new();
        lines.insert("UseBridges".to_owned(), vec!["1".to_owned()]);
        lines.insert(
            "Bridge".to_owned(),
            vec!["obfs4 a:1 CERT=x".to_owned(), "obfs4 b:2 CERT=y".to_owned()],
        );
        fragment::write(&dir, &lines).unwrap();
        assert_eq!(fragment::read(&dir), lines);
        // THE DRAFT IS GONE: a rename that left its temporary behind would
        // fill this directory with one file per write, for ever.
        let left: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect();
        assert_eq!(left, vec![FRAGMENT.to_owned()], "a draft was left behind");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_fragment_that_is_not_there_is_no_settings_and_not_a_failure() {
        // The ordinary case on a machine where nobody has configured Tor.
        assert!(fragment::read(std::path::Path::new("/nonexistent/x")).is_empty());
    }
}
