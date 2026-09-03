//! What `systemd-oomd` killed, and why — section 4.2, the involuntary half.
//!
//! # Why this exists
//!
//! `nix/modules/memory.nix` makes the machine survive running out of memory by
//! having `systemd-oomd` close an application before the kernel starts
//! thrashing. That works, and on its own it is a window vanishing with no
//! explanation — which is the same class of defect as every silent failure this
//! project has spent its time removing. The application did not crash: the
//! system closed it, on purpose, and it can say so.
//!
//! # The mechanism, and what it costs
//!
//! `org.freedesktop.oom1.Manager.Killed` is a signal on the **system** bus,
//! carrying the cgroup path and a reason. systemd's own bus policy already lets
//! any local user RECEIVE it (`org.freedesktop.oom1.conf` denies
//! `send_destination` and allows `receive_sender`), so the cost is one extra
//! connection: no new package, no new unit, no D-Bus policy of ours, no polkit
//! rule, and no bridge from root into the session.
//!
//! The cgroup path that arrives is exactly what
//! `ronoctua_shell::foreign::scope_from_cgroup` already parses, so the
//! application's identity comes out of code that already exists and is already
//! tested.
//!
//! # What it does NOT report
//!
//! Kills by the kernel's own OOM killer, and by anything else. Only `oomd`
//! announces itself. That is accepted rather than worked around: the whole
//! design exists to make `oomd` act first, and the VM check proves it does.
//!
//! # Why a raw match and not a `Proxy`
//!
//! Building a `zbus::blocking::Proxy` performs a `GetNameOwner` when it is
//! created. The footer may well come up before `systemd-oomd` has taken the
//! name — on a slow boot it usually does — and the proxy would then fail to
//! build, leaving the footer permanently deaf to a name that appeared a second
//! later. A `MatchRule` on the sender is registered with the bus and starts
//! delivering whenever the sender shows up.

use std::sync::mpsc::Sender;

use crate::dbus::Event;

/// The bus name `systemd-oomd` owns.
///
/// The match is on the SENDER and not merely on the interface and member. It is
/// the difference between "the system killed something" and "any process on
/// this bus says the system killed something" — the second would let anything
/// local forge a notification the person is meant to trust.
const SENDER: &str = "org.freedesktop.oom1";
const INTERFACE: &str = "org.freedesktop.oom1.Manager";
const MEMBER: &str = "Killed";

/// Why a cgroup was killed.
///
/// An enum and not the raw string, so a typo in a match arm is a compile error.
/// It is the lesson this repository already wrote down about values that cross a
/// boundary as text — a tray table validated at build time that never reached
/// the Rust, and the same shape more than once since.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// The cgroup filled memory and swap.
    MemoryUsed,
    /// It kept the machine stalled on memory for long enough to matter. This is
    /// the one the distro's policy is tuned to produce.
    MemoryPressure,
    /// A reason a later systemd invented. Kept whole rather than dropped: an
    /// unknown reason is still a kill, and reporting it badly beats not
    /// reporting it.
    Other(String),
}

impl Reason {
    /// From the string `org.freedesktop.oom1(5)` documents.
    #[must_use]
    pub fn from_wire(s: &str) -> Self {
        match s {
            "memory-used" => Self::MemoryUsed,
            "memory-pressure" => Self::MemoryPressure,
            other => Self::Other(other.to_owned()),
        }
    }
}

/// Listens for kills, on a thread of its own.
///
/// Returns `false` when the system bus is unreachable or the match is refused.
/// **That is not fatal, by the same contract as [`crate::dbus::listen`]**: the
/// footer loses the ability to explain a kill, and goes on being the footer.
/// Taking the shell down over it would trade one feature for all of them.
pub fn listen(channel: Sender<Event>) -> bool {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    let born = std::thread::Builder::new()
        .name("oom".into())
        .spawn(move || {
            let rule = zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(SENDER)
                .and_then(|b| b.interface(INTERFACE))
                .and_then(|b| b.member(MEMBER))
                .map(zbus::match_rule::Builder::build);

            let started = rule.and_then(|rule| {
                zbus::blocking::Connection::system()
                    .and_then(|c| zbus::blocking::MessageIterator::for_match_rule(rule, &c, None))
            });

            match started {
                Ok(messages) => {
                    let _ = ready_tx.send(true);
                    tracing::info!(sender = SENDER, "listening for deaths from lack of memory");
                    for message in messages {
                        let Ok(message) = message else { continue };
                        // The signal carries `(s cgroup, s reason)`. A body of
                        // another shape means systemd changed the signature, and
                        // guessing at it would report the wrong application.
                        // The body is BOUND first: `deserialize` borrows from it,
                        // and a temporary would not live long enough.
                        let body = message.body();
                        let parsed: zbus::Result<(String, String)> = body.deserialize();
                        let Ok((cgroup, reason)) = parsed else {
                            tracing::warn!("a death signal with an unexpected body; ignored");
                            continue;
                        };
                        let reason = Reason::from_wire(&reason);
                        tracing::info!(%cgroup, ?reason, "oomd killed a cgroup");
                        if channel.send(Event::OomKilled { cgroup, reason }).is_err() {
                            // The loop is gone; so is the reason to listen.
                            return;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "no warning about running out of memory");
                    let _ = ready_tx.send(false);
                }
            }
        });

    if born.is_err() {
        tracing::warn!("could not start the out-of-memory thread");
        return false;
    }
    ready_rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::Reason;

    /// The two strings `org.freedesktop.oom1(5)` documents, plus the shape of
    /// what happens when a later systemd adds a third.
    #[test]
    fn the_reason_comes_off_the_wire_whole() {
        assert_eq!(Reason::from_wire("memory-pressure"), Reason::MemoryPressure);
        assert_eq!(Reason::from_wire("memory-used"), Reason::MemoryUsed);
        assert_eq!(
            Reason::from_wire("something-new"),
            Reason::Other("something-new".to_owned()),
            "an unknown reason is still a death, and has to arrive"
        );
    }
}
