//! The footer tells systemd it is alive — so a footer that HANGS is one
//! that dies.
//!
//! # Why
//!
//! `OnFailure=` sees a process that dies. One that stops answering — a tight
//! loop, a GPU that hung, a wait with no deadline, a `SIGSTOP` — stays
//! `active`, the lifeline is never raised, and the person has a frozen strip
//! and no way to be told why. With `WatchdogSec=` on the unit, systemd
//! expects a `WATCHDOG=1` at least that often; a footer whose loop stopped
//! turning sends none, the unit is failed (`Result=watchdog`), and
//! `OnFailure=` raises the line as it does for a crash.
//!
//! # From the LOOP, and on a tick that was already there
//!
//! The ping has to come from the thread that would hang, or it proves
//! nothing: a thread of its own would go on pinging over a dead loop. And it
//! costs no clock of its own: the footer already wakes once a second to let a
//! message go (`surface::run`'s one-second timer), and that tick pets the dog
//! — at half the interval systemd asked for, as `sd_watchdog_enabled(3)`
//! advises.
//!
//! # Only under a watchdog
//!
//! systemd says it wants pings by setting `WATCHDOG_USEC` (and
//! `WATCHDOG_PID`, which has to be this process) beside `NOTIFY_SOCKET`. With
//! any of them missing — the nested loop, a footer started by hand — this is
//! nothing at all.

use std::os::linux::net::SocketAddrExt as _;

/// The three variables systemd hands the footer for its watchdog.
pub const FOOTERS_OWN: [&str; 3] = ["NOTIFY_SOCKET", "WATCHDOG_USEC", "WATCHDOG_PID"];

/// A child of the footer without the footer's watchdog: what it launches —
/// a program, a door, a column — must not speak to systemd for the footer's
/// unit, which `NotifyAccess=main` refuses and logs (`sd_notify(3)` asks the
/// main process to unset them; `remove_var` is `unsafe`, and the crate denies
/// it, so each child is born without them instead).
pub fn unshared(cmd: &mut std::process::Command) -> &mut std::process::Command {
    for name in FOOTERS_OWN {
        cmd.env_remove(name);
    }
    cmd
}
use std::os::unix::net::{SocketAddr, UnixDatagram};
use std::time::{Duration, Instant};

/// What systemd asked for, and where to say it.
#[derive(Debug)]
pub struct Watchdog {
    /// The socket and the address systemd listens on.
    to: Option<(UnixDatagram, SocketAddr)>,
    /// How often to say it: half of what systemd waits.
    every: Duration,
    /// When it was last said.
    last: Option<Instant>,
}

/// The interval to ping at, from `WATCHDOG_USEC` and `WATCHDOG_PID` — `None`
/// when systemd asked for no pings from THIS process.
#[must_use]
pub fn interval(usec: Option<&str>, pid: Option<&str>, me: u32) -> Option<Duration> {
    let usec: u64 = usec?.trim().parse().ok()?;
    if usec == 0 {
        return None;
    }
    // A PID THAT IS NOT MINE is a variable inherited from a parent the
    // watchdog was meant for; answering for it would keep a hung parent alive.
    if let Some(p) = pid
        && p.trim().parse::<u32>().ok() != Some(me)
    {
        return None;
    }
    Some(Duration::from_micros(usec / 2))
}

/// The address `NOTIFY_SOCKET` names: a path, or an abstract name after `@`.
fn address_of(socket: &str) -> Option<SocketAddr> {
    match socket.strip_prefix('@') {
        Some(name) => SocketAddr::from_abstract_name(name.as_bytes()).ok(),
        None if socket.starts_with('/') => SocketAddr::from_pathname(socket).ok(),
        None => None,
    }
}

impl Watchdog {
    /// What the environment says. Nothing to do is a watchdog that never
    /// pings.
    #[must_use]
    pub fn from_env() -> Self {
        let every = interval(
            std::env::var("WATCHDOG_USEC").ok().as_deref(),
            std::env::var("WATCHDOG_PID").ok().as_deref(),
            std::process::id(),
        );
        let to = every
            .and(std::env::var("NOTIFY_SOCKET").ok())
            .and_then(|s| address_of(&s))
            .zip(UnixDatagram::unbound().ok())
            .map(|(addr, d)| (d, addr));
        if to.is_some() {
            tracing::info!(every = ?every, "the footer pets systemd's watchdog");
        }
        Self {
            to,
            every: every.unwrap_or(Duration::MAX),
            last: None,
        }
    }

    /// A tick of the loop: says `WATCHDOG=1` when half the interval has gone.
    pub fn pet(&mut self, now: Instant) {
        let Some((socket, addr)) = self.to.as_ref() else {
            return;
        };
        if self
            .last
            .is_some_and(|l| now.saturating_duration_since(l) < self.every)
        {
            return;
        }
        if let Err(e) = socket.send_to_addr(b"WATCHDOG=1", addr) {
            tracing::warn!(error = %e, "systemd's watchdog did not hear the footer");
        }
        self.last = Some(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a child the footer launches is born with the footer's
    /// watchdog in its environment.
    #[test]
    fn a_child_is_born_without_the_footers_watchdog() {
        // NEVER SPAWNED: only its environment is read.
        let mut cmd = std::process::Command::new("/bin/sh");
        cmd.env("NOTIFY_SOCKET", "/run/x").env("KEPT", "1");
        unshared(&mut cmd);
        let set: std::collections::BTreeMap<_, _> = cmd.get_envs().collect();
        for name in FOOTERS_OWN {
            assert_eq!(set.get(std::ffi::OsStr::new(name)), Some(&None), "{name}");
        }
        assert_eq!(
            set.get(std::ffi::OsStr::new("KEPT")),
            Some(&Some(std::ffi::OsStr::new("1")))
        );
    }

    /// FAILS IF: the footer pings for a watchdog that is not its own, or not
    /// at half the interval systemd waits.
    #[test]
    fn the_footer_pings_only_its_own_watchdog() {
        assert_eq!(
            interval(Some("30000000"), Some("42"), 42),
            Some(Duration::from_secs(15))
        );
        assert_eq!(
            interval(Some("30000000"), None, 42),
            Some(Duration::from_secs(15))
        );
        assert_eq!(interval(Some("30000000"), Some("7"), 42), None);
        assert_eq!(interval(None, Some("42"), 42), None);
        assert_eq!(interval(Some("0"), Some("42"), 42), None);
        assert_eq!(interval(Some("soon"), None, 42), None);
    }

    /// FAILS IF: a ping does not reach the socket systemd names, or a second
    /// one inside the interval does.
    #[test]
    fn a_ping_reaches_the_socket_and_waits_its_turn() {
        let dir = std::env::temp_dir().join(format!("ronoctua-watchdog-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("notify");
        let _ = std::fs::remove_file(&path);
        let listener = UnixDatagram::bind(&path).expect("bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let mut dog = Watchdog {
            to: Some((
                UnixDatagram::unbound().expect("a socket"),
                address_of(path.to_str().expect("utf-8")).expect("an address"),
            )),
            every: Duration::from_secs(15),
            last: None,
        };
        let t0 = Instant::now();
        dog.pet(t0);
        let mut buf = [0_u8; 32];
        let n = listener.recv(&mut buf).expect("the first ping");
        assert_eq!(&buf[..n], b"WATCHDOG=1");
        dog.pet(t0 + Duration::from_secs(1));
        assert!(
            listener.recv(&mut buf).is_err(),
            "a second ping inside the interval"
        );
        dog.pet(t0 + Duration::from_secs(16));
        assert!(
            listener.recv(&mut buf).is_ok(),
            "no ping after the interval"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
