//! What the journal's lines mean, and what is "off" — pure, so every rule is
//! a test over lines and values.

use std::collections::VecDeque;

use crate::moment::Moment;

/// How long a refused connection keeps the mark up and counts on it.
pub const RECENT_S: i64 = 15 * 60;

/// The most refused connections kept, newest first.
pub const REFUSED_MOST: usize = 256;

/// The most sudo events kept, newest first.
pub const SUDO_MOST: usize = 200;

/// How far back the sudo events are read at the first start: a week.
pub const SUDO_BACK_S: u64 = 7 * 24 * 3600;

/// How far back the refused connections are read at the first start.
pub const REFUSED_BACK_S: u64 = 3600;

/// One connection the firewall refused, from the kernel's line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// When, in seconds since the epoch.
    pub at: Moment,
    /// The interface it arrived on (`IN=`).
    pub iface: String,
    /// Who asked (`SRC=`).
    pub source: String,
    /// `tcp` or `udp` (lower case), or what the kernel said.
    pub proto: String,
    /// The port asked for (`DPT=`); `None` for a protocol with none.
    pub port: Option<u16>,
}

impl Refused {
    /// The row id the page reads back: `blocked:<proto>:<port>:<iface>:<source>`
    /// (`-` for no port). Data, parsed by the security centre to offer "allow".
    #[must_use]
    pub fn row_id(&self) -> String {
        format!(
            "blocked:{}:{}:{}:{}",
            self.proto,
            self.port.map_or_else(|| "-".to_owned(), |p| p.to_string()),
            self.iface,
            self.source
        )
    }
}

/// A plain word of the kernel's line: letters, digits and `.:_-`, at most 64 —
/// anything else is not taken (a line is data from the network's peer).
fn plain(value: &str) -> Option<String> {
    (!value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b':' | b'_' | b'-')))
    .then(|| value.to_owned())
}

/// A refused connection out of a kernel line carrying `prefix`; `None` for a
/// line that is not one, or lacks `IN=` or `SRC=`. Pure.
#[must_use]
pub fn refused_of(message: &str, prefix: &str, at: Moment) -> Option<Refused> {
    let rest = &message[message.find(prefix)? + prefix.len()..];
    let field = |name: &str| {
        rest.split_whitespace()
            .find_map(|w| w.strip_prefix(name)?.strip_prefix('='))
    };
    Some(Refused {
        at,
        iface: plain(field("IN")?)?,
        source: plain(field("SRC")?)?,
        proto: plain(field("PROTO").unwrap_or("?"))
            .unwrap_or_else(|| "?".to_owned())
            .to_ascii_lowercase(),
        port: field("DPT").and_then(|p| p.parse().ok()),
    })
}

/// What sudo said, as the Administrator section counts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sudo {
    /// A command ran as another user.
    Used,
    /// A password was wrong.
    Failed,
    /// The person may not use sudo at all.
    Refused,
}

/// One sudo event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SudoEvent {
    /// When.
    pub at: Moment,
    /// What.
    pub kind: Sudo,
    /// Who asked.
    pub user: String,
    /// The program, by its file name — data.
    pub command: String,
}

/// A sudo line, read; `None` for the lines that are no event of the person's
/// (a session opened, a password REQUIRED — the door's own `sudo -n` says
/// that before every question). Pure.
#[must_use]
pub fn sudo_of(message: &str, at: Moment) -> Option<SudoEvent> {
    let (user, rest) = message.split_once(" : ")?;
    let user = plain(user.trim())?;
    let command = rest
        .split(" ; ")
        .find_map(|part| part.trim().strip_prefix("COMMAND="))
        .map(|c| {
            let program = c.split_whitespace().next().unwrap_or_default();
            program.rsplit('/').next().unwrap_or(program).to_owned()
        })
        .unwrap_or_default();
    let head = rest.split(" ; ").next().unwrap_or_default();
    let kind = if head.contains("incorrect password attempt") {
        Sudo::Failed
    } else if head.contains("NOT in sudoers") || head.contains("command not allowed") {
        Sudo::Refused
    } else if head.starts_with("TTY=") || head.starts_with("PWD=") {
        Sudo::Used
    } else {
        return None;
    };
    Some(SudoEvent {
        at,
        kind,
        user,
        command,
    })
}

/// Pushes `item` first and keeps at most `most`.
pub fn ring<T>(list: &mut VecDeque<T>, item: T, most: usize) {
    list.push_front(item);
    list.truncate(most);
}

/// What is off, loudest first — at most one mark is drawn, the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Off {
    /// The table in the kernel is not the one asked for.
    Failed,
    /// The person's firewall decides nothing.
    Disabled,
    /// sudo asks for no password, until a time or with no limit.
    SudoOpen,
    /// Every new incoming connection is dropped.
    Shields,
    /// Connections were refused lately.
    Refused,
}

/// The facts the conditions are read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Facts {
    /// The helper's last apply failed, or the firewall's units logged an
    /// error after it.
    pub failed: bool,
    /// `firewall.json` says off.
    pub off: bool,
    /// The passwordless window's end, when one is open with an end.
    pub sudo_until: Option<Moment>,
    /// A passwordless window with no limit is open.
    pub sudo_no_limit: bool,
    /// `firewall.json` says shields.
    pub shields: bool,
    /// Refused connections in the last [`RECENT_S`].
    pub recent_refused: u32,
}

/// The interfaces of the public, unplaced networks in `now` that were never
/// seen — and `seen` grows by all of `now`. A network is news once. Pure.
pub fn fresh_networks(
    seen: &mut std::collections::BTreeSet<String>,
    now: &[(String, String)],
) -> Vec<String> {
    now.iter()
        .filter(|(connection, _)| seen.insert(connection.clone()))
        .map(|(_, iface)| iface.clone())
        .collect()
}

/// Everything off at `now`, loudest first. Pure.
#[must_use]
pub fn off_at(facts: &Facts, now: Moment) -> Vec<Off> {
    let mut out = Vec::new();
    if facts.failed {
        out.push(Off::Failed);
    }
    if facts.off {
        out.push(Off::Disabled);
    }
    if facts.sudo_no_limit || facts.sudo_until.is_some_and(|u| u > now) {
        out.push(Off::SudoOpen);
    }
    if facts.shields {
        out.push(Off::Shields);
    }
    if facts.recent_refused > 0 {
        out.push(Off::Refused);
    }
    out
}

/// How many refused connections are recent at `now`.
#[must_use]
pub fn recent(refused: &VecDeque<Refused>, now: Moment) -> u32 {
    let n = refused.iter().filter(|r| r.at > now - RECENT_S).count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// When the recent ones stop being recent — the one clock this module arms,
/// and only while one is recent.
#[must_use]
pub fn recent_ends(refused: &VecDeque<Refused>, now: Moment) -> Option<Moment> {
    refused
        .iter()
        .filter(|r| r.at > now - RECENT_S)
        .map(|r| r.at + RECENT_S)
        .min()
}

/// The blocked connections, one row each per (proto, port, iface, source):
/// how many, and the newest — newest first. Pure.
#[must_use]
pub fn grouped(refused: &VecDeque<Refused>) -> Vec<(Refused, u64)> {
    let mut out: Vec<(Refused, u64)> = Vec::new();
    for r in refused {
        match out.iter_mut().find(|(g, _)| g.row_id() == r.row_id()) {
            Some((_, n)) => *n += 1,
            None => out.push((r.clone(), 1)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFIX: &str = "ronoctua-refused: ";

    #[test]
    fn a_kernel_line_is_read_into_a_refused_connection() {
        let line = "ronoctua-refused: IN=wlan0 OUT= MAC=aa:bb SRC=203.0.113.9 DST=192.168.1.5 \
                    LEN=60 TOS=0x00 TTL=52 ID=1 DF PROTO=TCP SPT=51234 DPT=22 WINDOW=64240 SYN URGP=0";
        let r = refused_of(line, PREFIX, 100).expect("a refused line");
        assert_eq!(r.iface, "wlan0");
        assert_eq!(r.source, "203.0.113.9");
        assert_eq!(r.proto, "tcp");
        assert_eq!(r.port, Some(22));
        assert_eq!(r.row_id(), "blocked:tcp:22:wlan0:203.0.113.9");
        let v6 = "ronoctua-refused: IN=eth0 OUT= SRC=2001:db8::1 DST=2001:db8::2 PROTO=UDP SPT=5 DPT=5353";
        assert_eq!(refused_of(v6, PREFIX, 1).unwrap().source, "2001:db8::1");
        // NOT OURS, OR WITHOUT WHERE IT CAME FROM: nothing.
        assert_eq!(refused_of("IN=wlan0 SRC=1.2.3.4", PREFIX, 1), None);
        assert_eq!(
            refused_of("ronoctua-refused: OUT= SRC=1.2.3.4", PREFIX, 1),
            None
        );
        // A PEER'S BYTES ARE NOT TAKEN AS THEY COME.
        assert_eq!(
            refused_of("ronoctua-refused: IN=wlan0 SRC=1.2.3.4/../x", PREFIX, 1),
            None
        );
    }

    #[test]
    fn sudo_lines_are_uses_failures_refusals_or_nothing() {
        let used = "noctua : TTY=pts/1 ; PWD=/home/noctua ; USER=root ; COMMAND=/run/current-system/sw/bin/true";
        let e = sudo_of(used, 5).unwrap();
        assert_eq!(
            (e.kind, e.user.as_str(), e.command.as_str()),
            (Sudo::Used, "noctua", "true")
        );
        let door = "noctua : PWD=/home/noctua ; USER=root ; COMMAND=/nix/store/x-ronoctua-security-root/bin/ronoctua-security-root relax 15";
        assert_eq!(sudo_of(door, 5).unwrap().command, "ronoctua-security-root");
        let failed = "noctua : 1 incorrect password attempt ; PWD=/home/noctua ; USER=root ; COMMAND=/bin/ls";
        assert_eq!(sudo_of(failed, 5).unwrap().kind, Sudo::Failed);
        let refused =
            "guest : user NOT in sudoers ; TTY=pts/0 ; PWD=/ ; USER=root ; COMMAND=/bin/sh";
        assert_eq!(sudo_of(refused, 5).unwrap().kind, Sudo::Refused);
        // THE DOOR'S OWN `sudo -n` BEFORE A QUESTION is no event.
        let required =
            "noctua : a password is required ; PWD=/home/noctua ; USER=root ; COMMAND=/x";
        assert_eq!(sudo_of(required, 5), None);
        assert_eq!(
            sudo_of("pam_unix(sudo:session): session opened for user root", 5),
            None
        );
    }

    #[test]
    fn what_is_off_comes_loudest_first() {
        let all = Facts {
            failed: true,
            off: true,
            sudo_until: Some(200),
            sudo_no_limit: false,
            shields: true,
            recent_refused: 3,
        };
        assert_eq!(
            off_at(&all, 100),
            [
                Off::Failed,
                Off::Disabled,
                Off::SudoOpen,
                Off::Shields,
                Off::Refused
            ]
        );
        assert_eq!(off_at(&Facts::default(), 100), []);
        // A WINDOW THAT ENDED is not open, whatever the file still says.
        let ended = Facts {
            sudo_until: Some(100),
            ..Facts::default()
        };
        assert_eq!(off_at(&ended, 100), []);
        // ONE WITH NO LIMIT never ends by itself.
        let forever = Facts {
            sudo_no_limit: true,
            ..Facts::default()
        };
        assert_eq!(off_at(&forever, i64::MAX), [Off::SudoOpen]);
    }

    #[test]
    fn recent_refusals_count_and_say_when_they_stop() {
        let at = |s| Refused {
            at: s,
            iface: "wlan0".into(),
            source: "203.0.113.9".into(),
            proto: "tcp".into(),
            port: Some(22),
        };
        let mut list = VecDeque::new();
        ring(&mut list, at(1000), REFUSED_MOST);
        ring(&mut list, at(1500), REFUSED_MOST);
        assert_eq!(recent(&list, 1600), 2);
        assert_eq!(recent_ends(&list, 1600), Some(1000 + RECENT_S));
        assert_eq!(recent(&list, 1000 + RECENT_S), 1);
        assert_eq!(recent(&list, 1500 + RECENT_S), 0);
        assert_eq!(
            recent_ends(&list, 1500 + RECENT_S),
            None,
            "no clock at rest"
        );
        let g = grouped(&list);
        assert_eq!(g.len(), 1);
        assert_eq!((g[0].0.at, g[0].1), (1500, 2), "the newest, and how many");
        for n in 0..(REFUSED_MOST + 3) {
            ring(&mut list, at(i64::try_from(n).unwrap()), REFUSED_MOST);
        }
        assert_eq!(list.len(), REFUSED_MOST);
    }
}
