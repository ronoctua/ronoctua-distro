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

/// The SOCKS address the browser is pointed at.
pub const SOCKS: &str = "127.0.0.1:9050";

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
        stream.set_read_timeout(Some(crate::http::PATIENCE))?;
        stream.set_write_timeout(Some(crate::http::PATIENCE))?;
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
}
