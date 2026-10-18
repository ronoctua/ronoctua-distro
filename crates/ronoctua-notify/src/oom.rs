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
//! `ronoctua_shell_kit::foreign::scope_from_cgroup` already parses, so the
//! application's identity comes out of code that already exists and is already
//! tested.
//!
//! # What it does NOT report
//!
//! Kills by the kernel's own OOM killer, and by anything else: only `oomd`
//! announces itself on the bus. Those are the brain's — its watch on the
//! journal reads the kernel's kill line, systemd's word about the unit and
//! oomd's own line, and tells each kill once, with what died, where, how much
//! memory it held and how stalled the machine was.
//!
//! # One kill, one notification
//!
//! oomd's kill therefore arrives TWICE: here, at once, and from the brain a
//! few seconds later, knowing more. [`Kills`] is the footer's memory of the
//! kills it heard from either side, by cgroup and moment, and it says what the
//! second arrival does: add its words to the notification the first raised,
//! or nothing at all.
//!
//! # Why a raw match and not a `Proxy`
//!
//! Building a `zbus::blocking::Proxy` performs a `GetNameOwner` when it is
//! created. The footer may well come up before `systemd-oomd` has taken the
//! name — on a slow boot it usually does — and the proxy would then fail to
//! build, leaving the footer permanently deaf to a name that appeared a second
//! later. A `MatchRule` on the sender is registered with the bus and starts
//! delivering whenever the sender shows up.

use std::collections::VecDeque;
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime};

use crate::dbus::Event;
use crate::queue::RecordKey;

/// The name of the value of a brain's kill event that carries the killed
/// cgroup, as the bus names it — empty for a kill the bus never announces
/// (the kernel's).
pub const CGROUP_ARG: &str = "cgroup";

/// The name of the value of a brain's kill event that carries the second the
/// kill happened, by the journal's clock, since the epoch.
pub const EPOCH_ARG: &str = "epoch";

/// How far apart the bus's arrival and the journal's second may be and still
/// be one kill. The bus is at once; the journal's line is written within a
/// second of it — a minute is a margin, and two kills of one cgroup a minute
/// apart are not a thing a policy that waits fifteen seconds of pressure
/// produces.
pub const SAME_KILL: Duration = Duration::from_secs(60);

/// How many kills are remembered.
const KEPT: usize = 32;

/// What the footer does with a kill the brain told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Told {
    /// The bus announced it and its notification is up: put these words on
    /// that one.
    Amend(RecordKey),
    /// Nobody announced it: raise it.
    Raise,
    /// It was told already.
    Already,
}

/// One kill heard, from either side or from both.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Heard {
    cgroup: String,
    at: SystemTime,
    /// The notification the bus's arrival raised.
    raised: Option<RecordKey>,
    /// Whether oomd's bus signal announced it.
    by_bus: bool,
    /// Whether the brain told it.
    by_brain: bool,
}

/// The footer's memory of the kills it heard — see this module's header.
///
/// ONE TO ONE: an arrival from one side joins only a kill the OTHER side
/// alone has told, and of those the nearest in time. A service oomd kills
/// twice in a minute is two kills of one cgroup; joining by "same cgroup,
/// near enough" would take the second for the first and drop it.
#[derive(Debug, Default)]
pub struct Kills {
    heard: VecDeque<Heard>,
}

/// A cgroup as both sides spell it: the path under the cgroup root, one
/// slash first and none last.
fn spelled(cgroup: &str) -> String {
    format!("/{}", cgroup.trim().trim_matches('/'))
}

/// How far apart two moments are, whichever is first.
fn apart(a: SystemTime, b: SystemTime) -> Duration {
    a.duration_since(b)
        .or_else(|_| b.duration_since(a))
        .unwrap_or(Duration::ZERO)
}

impl Kills {
    /// None heard yet.
    #[must_use]
    pub fn fresh_one() -> Self {
        Self::default()
    }

    /// The kill of this cgroup nearest this moment, within [`SAME_KILL`],
    /// among those `wanted` takes.
    fn nearest(
        &mut self,
        cgroup: &str,
        at: SystemTime,
        wanted: impl Fn(&Heard) -> bool,
    ) -> Option<&mut Heard> {
        self.heard
            .iter_mut()
            .filter(|h| h.cgroup == cgroup && apart(h.at, at) <= SAME_KILL && wanted(h))
            .min_by_key(|h| apart(h.at, at))
    }

    fn keep(&mut self, heard: Heard) {
        self.heard.push_back(heard);
        while self.heard.len() > KEPT {
            self.heard.pop_front();
        }
    }

    /// oomd's bus signal arrived: `true` when it is to be raised — `false`
    /// when the brain already told this kill, with more to say.
    pub fn from_the_bus(&mut self, cgroup: &str, at: SystemTime) -> bool {
        let cgroup = spelled(cgroup);
        if let Some(h) = self.nearest(&cgroup, at, |h| h.by_brain && !h.by_bus) {
            h.by_bus = true;
            return false;
        }
        self.keep(Heard {
            cgroup,
            at,
            raised: None,
            by_bus: true,
            by_brain: false,
        });
        true
    }

    /// The bus's notification of this kill was raised under this key.
    pub fn raised(&mut self, cgroup: &str, at: SystemTime, key: RecordKey) {
        let cgroup = spelled(cgroup);
        if let Some(h) = self.nearest(&cgroup, at, |h| h.by_bus && h.raised.is_none()) {
            h.raised = Some(key);
        }
    }

    /// The brain told a kill of this cgroup at this moment. A kill with no
    /// cgroup is one the bus never announces — the kernel's — and is raised.
    /// The same kill told again (the same cgroup and second, the brain's mind
    /// read twice) is [`Told::Already`].
    pub fn from_the_brain(&mut self, cgroup: &str, at: SystemTime) -> Told {
        if cgroup.trim().is_empty() {
            return Told::Raise;
        }
        let cgroup = spelled(cgroup);
        if self
            .heard
            .iter()
            .any(|h| h.by_brain && h.cgroup == cgroup && h.at == at)
        {
            return Told::Already;
        }
        if let Some(h) = self.nearest(&cgroup, at, |h| h.by_bus && !h.by_brain) {
            h.by_brain = true;
            // ITS SECOND IS THE JOURNAL'S from now on: a repeat of this event
            // is known by it.
            h.at = at;
            return h.raised.map_or(Told::Raise, Told::Amend);
        }
        self.keep(Heard {
            cgroup,
            at,
            raised: None,
            by_bus: false,
            by_brain: true,
        });
        Told::Raise
    }
}

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
    use super::{Kills, Reason, SAME_KILL, Told};
    use crate::queue::RecordKey;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const HOG: &str = "/user.slice/user-1000.slice/user@1000.service/app.slice/hog.service";

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    #[test]
    fn a_kill_the_bus_raised_is_amended_by_the_brain_and_never_raised_twice() {
        let mut k = Kills::fresh_one();
        assert!(
            k.from_the_bus(HOG, at(1_000)),
            "the bus's own kill was not raised"
        );
        let key = RecordKey {
            id: 7,
            at: at(1_000),
        };
        k.raised(HOG, at(1_000), key);
        // THE BRAIN, a few seconds later, by the journal's second: the same
        // kill, even spelled without the first slash.
        assert_eq!(
            k.from_the_brain(HOG.trim_start_matches('/'), at(999)),
            Told::Amend(key)
        );
        // THE SAME EVENT AGAIN is said once; another second from the brain is
        // another kill (the brain tells each kill once, by its own second).
        assert_eq!(k.from_the_brain(HOG, at(999)), Told::Already);
        assert_eq!(k.from_the_brain(HOG, at(1_001)), Told::Raise);
    }

    #[test]
    fn a_kill_the_brain_told_first_is_not_raised_again_by_the_bus() {
        let mut k = Kills::fresh_one();
        assert_eq!(k.from_the_brain(HOG, at(50)), Told::Raise);
        assert!(!k.from_the_bus(HOG, at(52)), "one kill, two notifications");
    }

    #[test]
    fn two_kills_of_one_cgroup_within_the_window_are_two_in_either_order() {
        let key = |id, secs| RecordKey { id, at: at(secs) };
        // THE BUS AND THE BRAIN TAKE TURNS: each kill amends its own.
        let mut k = Kills::fresh_one();
        assert!(k.from_the_bus(HOG, at(1_000)));
        k.raised(HOG, at(1_000), key(1, 1_000));
        assert_eq!(k.from_the_brain(HOG, at(1_000)), Told::Amend(key(1, 1_000)));
        assert!(
            k.from_the_bus(HOG, at(1_020)),
            "the second kill's bus notice taken for the first's"
        );
        k.raised(HOG, at(1_020), key(2, 1_020));
        assert_eq!(k.from_the_brain(HOG, at(1_020)), Told::Amend(key(2, 1_020)));
        // BOTH BUS SIGNALS FIRST: the first kill's words go on the first's.
        let mut k = Kills::fresh_one();
        assert!(k.from_the_bus(HOG, at(1_000)));
        k.raised(HOG, at(1_000), key(1, 1_000));
        assert!(k.from_the_bus(HOG, at(1_020)));
        k.raised(HOG, at(1_020), key(2, 1_020));
        assert_eq!(k.from_the_brain(HOG, at(1_000)), Told::Amend(key(1, 1_000)));
        assert_eq!(k.from_the_brain(HOG, at(1_020)), Told::Amend(key(2, 1_020)));
        // THE BRAIN FIRST, twice: each bus signal is one of them, and a third
        // is a kill of its own.
        let mut k = Kills::fresh_one();
        assert_eq!(k.from_the_brain(HOG, at(1_000)), Told::Raise);
        assert_eq!(k.from_the_brain(HOG, at(1_020)), Told::Raise);
        assert!(!k.from_the_bus(HOG, at(1_001)));
        assert!(!k.from_the_bus(HOG, at(1_021)));
        assert!(
            k.from_the_bus(HOG, at(1_022)),
            "a third kill was never raised"
        );
        // AND A BRAIN'S KILL TOLD AGAIN is said once.
        assert_eq!(k.from_the_brain(HOG, at(1_020)), Told::Already);
    }

    #[test]
    fn two_kills_are_two_and_a_kernel_kill_is_always_raised() {
        let mut k = Kills::fresh_one();
        assert!(k.from_the_bus(HOG, at(0)));
        // THE SAME CGROUP, LONG AFTER: another kill.
        let later = at(SAME_KILL.as_secs() + 1);
        assert_eq!(k.from_the_brain(HOG, later), Told::Raise);
        // ANOTHER CGROUP, the same second: another kill.
        assert_eq!(
            k.from_the_brain("/app.slice/other.service", at(0)),
            Told::Raise
        );
        // NO CGROUP: the kernel's, which the bus never announces.
        assert_eq!(k.from_the_brain("", at(0)), Told::Raise);
        assert_eq!(k.from_the_brain("", at(0)), Told::Raise);
    }

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
