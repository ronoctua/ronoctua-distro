//! The MACHINE's clock — what time it thinks it is, what zone it is in, and
//! whether anything is keeping either of them right.
//!
//! # Why this is a crate and not part of the screen that draws it
//!
//! Because it is a boundary, and this tree has one crate per boundary:
//! `ronoctua-power` for logind and UPower, `ronoctua-disks` for udisks2,
//! `ronoctua-memory` for `/proc`. Two callers already want this one — the
//! `LEADER s l` view and the brain's watch — and a second reader of
//! `timedatectl` would be a second answer about one clock.
//!
//! # What it can change, and what it can only SAY
//!
//! This was the whole of the owner's complaint about that screen: it changed
//! what the INTERFACE draws and nothing about the computer. So:
//!
//!   - AUTOMATIC (`NTP`) is the machine's, and it is the default. On this
//!     distro the client behind it is `systemd-timesyncd`.
//!   - The DATE and the TIME are the machine's, and they can only be set while
//!     automatic is OFF — the daemon refuses otherwise, and so does the screen,
//!     with a sentence, before the key is pressed. Setting a clock that
//!     something corrects every half hour is a change that undoes itself.
//!   - The ZONE is the machine's when the flake leaves it to the machine
//!     (`ronoctua.time.zone = null`, which is the default). Pinned in the
//!     flake, NixOS starts timedated with `NIXOS_STATIC_TIMEZONE=1` and the
//!     row is drawn REFUSING — the shape the Syncthing and Tor views already
//!     have.
//!
//! # Why the rules are not in the proxy
//!
//! Because a rule that needs a bus to be read is a rule nobody tests. What is
//! editable given what the daemon says is [`Standing`]'s business, and
//! `Standing` is six fields and no zbus.

#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod timedate;

use anyhow::{Context as _, Result};

/// The second service this crate reads, and it is READ-ONLY.
///
/// `timesync1` is where the NTP client says who it is asking and how it is
/// going. It is the same subject — the machine's clock — so it lives behind
/// the same boundary rather than becoming a second crate; what keeps that
/// honest is that nothing here ever writes to it. Turning the client on and
/// off is `timedate1`'s `SetNTP`, which is the switch the person actually
/// means.
const TIMESYNC: (&str, &str, &str) = (
    "org.freedesktop.timesync1",
    "/org/freedesktop/timesync1",
    "org.freedesktop.timesync1.Manager",
);

/// What the machine says about its own clock.
///
/// Every field is what a property answered, and nothing here is inferred. A
/// machine with no timedated at all is [`Standing::unknown`], which is not the
/// same as a machine that answered `false` to everything — see
/// [`Standing::reachable`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Standing {
    /// Did the daemon answer at all?
    pub reachable: bool,
    /// The IANA zone the machine runs on.
    pub zone: String,
    /// Is a network time client running?
    pub automatic: bool,
    /// Can one be? `false` on a machine where none is installed, and then the
    /// switch is drawn refusing rather than flipping to nothing.
    pub can_be_automatic: bool,
    /// Has the clock actually been set by it? `automatic` says a client is
    /// running; this says it has managed to agree with somebody.
    pub synchronised: bool,
    /// Who it asks, when it is on and says so. Empty when there is nobody, or
    /// when `timesync1` is not the client in use.
    pub server: String,
    /// What the machine thinks the time is, in epoch seconds.
    pub now: i64,
}

impl Standing {
    /// A machine that did not answer.
    ///
    /// Distinct from `Default` by nothing but intent, and it is named because
    /// every caller has to tell "the clock is not automatic" from "nobody
    /// could be asked": the first is a switch to offer, the second is a row
    /// that says why there is none.
    #[must_use]
    pub fn unknown() -> Self {
        Self::default()
    }

    /// May the date and the time be set by hand right now?
    ///
    /// NO WHILE AUTOMATIC IS ON, and that is not this screen being cautious:
    /// the daemon itself refuses, because a client that corrects the clock
    /// every half hour would put back whatever was typed. Saying so before the
    /// key is pressed is the difference between a row that explains itself and
    /// a refusal that arrives without one.
    #[must_use]
    pub const fn can_set_by_hand(&self) -> bool {
        self.reachable && !self.automatic
    }

    /// May the zone be set?
    ///
    /// Only the daemon knows, because only it knows whether NixOS started it
    /// with `NIXOS_STATIC_TIMEZONE=1` — so this is the question this type
    /// cannot answer on its own, and the caller is told to ask and read the
    /// refusal. What IS answerable here is whether there is anybody to ask.
    #[must_use]
    pub const fn can_be_asked(&self) -> bool {
        self.reachable
    }
}

/// Reads the machine's clock over the system bus.
///
/// # Errors
///
/// There is no system bus, or timedated is not on it.
pub fn read() -> Result<Standing> {
    let bus =
        zbus::blocking::Connection::system().context("no system bus to ask about the clock")?;
    read_on(&bus)
}

/// The same, on a connection somebody already has.
///
/// # Errors
///
/// timedated is not on the bus, or answered something unreadable.
pub fn read_on(bus: &zbus::blocking::Connection) -> Result<Standing> {
    let props = get_all(bus, timedate::SERVICE, timedate::PATH, timedate::INTERFACE)
        .context("timedated did not answer")?;
    let boolean = |name: &str| -> bool {
        props
            .get(name)
            .and_then(|v| bool::try_from(v.clone()).ok())
            .unwrap_or(false)
    };
    let automatic = boolean("NTP");
    Ok(Standing {
        reachable: true,
        zone: props
            .get("Timezone")
            .and_then(|v| String::try_from(v.clone()).ok())
            .unwrap_or_default(),
        automatic,
        can_be_automatic: boolean("CanNTP"),
        synchronised: boolean("NTPSynchronized"),
        // ONLY WHEN IT IS ON. `timesync1` keeps the last server it used, and a
        // name beside a switch that is off reads as a machine still asking it.
        server: if automatic {
            server_on(bus).unwrap_or_default()
        } else {
            String::new()
        },
        now: props
            .get("TimeUSec")
            .and_then(|v| u64::try_from(v.clone()).ok())
            .and_then(|usec| i64::try_from(usec / 1_000_000).ok())
            .unwrap_or_default(),
    })
}

/// Who the NTP client is asking. Empty when `timesync1` is not the client.
fn server_on(bus: &zbus::blocking::Connection) -> Option<String> {
    let props = get_all(bus, TIMESYNC.0, TIMESYNC.1, TIMESYNC.2).ok()?;
    props
        .get("ServerName")
        .and_then(|v| String::try_from(v.clone()).ok())
        .filter(|s| !s.is_empty())
}

/// Every property of one interface, in one round trip.
fn get_all(
    bus: &zbus::blocking::Connection,
    service: &str,
    path: &str,
    interface: &str,
) -> Result<std::collections::HashMap<String, zbus::zvariant::OwnedValue>> {
    let proxy = zbus::blocking::fdo::PropertiesProxy::builder(bus)
        .destination(service)?
        .path(path)?
        .build()?;
    let name = zbus::names::InterfaceName::try_from(interface)?;
    Ok(proxy.get_all(name)?)
}

/// Turns the machine's network time client on, or off.
///
/// # Errors
///
/// There is no bus, timedated is absent, or polkit refused — which on a
/// machine without the narrow rule in `nix/modules/base.nix` is what happens,
/// and is why the refusal is returned rather than swallowed.
pub fn set_automatic(on: bool) -> Result<()> {
    let bus = zbus::blocking::Connection::system().context("no system bus")?;
    timedate::TimedateProxyBlocking::new(&bus)?
        .set_ntp(on, false)
        .context("timedated refused to change the network time client")
}

/// Sets the machine's clock, in epoch SECONDS.
///
/// # Errors
///
/// As [`set_automatic`], and additionally when the network time client is on:
/// the daemon refuses, because something that corrects the clock every half
/// hour would put this back.
pub fn set_now(epoch_seconds: i64) -> Result<()> {
    let bus = zbus::blocking::Connection::system().context("no system bus")?;
    let usec = epoch_seconds
        .checked_mul(1_000_000)
        .context("that time is too far from the epoch to be set")?;
    timedate::TimedateProxyBlocking::new(&bus)?
        .set_time(usec, false, false)
        .context("timedated refused to set the clock")
}

/// Sets the machine's time zone by its IANA name.
///
/// # Errors
///
/// As [`set_automatic`], and additionally when the flake pins the zone: NixOS
/// then runs timedated with `NIXOS_STATIC_TIMEZONE=1` and it refuses.
pub fn set_zone(zone: &str) -> Result<()> {
    let bus = zbus::blocking::Connection::system().context("no system bus")?;
    timedate::TimedateProxyBlocking::new(&bus)?
        .set_timezone(zone, false)
        .context("timedated refused to change the time zone")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A machine nobody could ask is not a machine whose clock is wrong.
    ///
    /// THE DEFECT THIS CATCHES: reading `automatic: false` off an unreachable
    /// daemon and offering to turn it on. The switch would do nothing, the
    /// refusal would arrive from the bus, and the screen would have said the
    /// machine's clock is unattended when it has no idea.
    #[test]
    fn an_unreachable_daemon_is_not_a_clock_that_is_off() {
        let nobody = Standing::unknown();
        assert!(!nobody.reachable);
        assert!(!nobody.can_be_asked());
        assert!(
            !nobody.can_set_by_hand(),
            "a machine with no timedated offered to set its clock"
        );
    }

    /// AUTOMATIC AND BY HAND ARE EXCLUSIVE, and the screen says so before the
    /// key rather than after the refusal.
    #[test]
    fn nothing_is_typed_into_a_clock_something_else_is_keeping() {
        let auto = Standing {
            reachable: true,
            automatic: true,
            can_be_automatic: true,
            synchronised: true,
            zone: "Europe/London".to_owned(),
            server: "3.pool.ntp.org".to_owned(),
            now: 1_788_000_000,
        };
        assert!(!auto.can_set_by_hand());
        assert!(auto.can_be_asked());

        let by_hand = Standing {
            automatic: false,
            server: String::new(),
            ..auto
        };
        assert!(by_hand.can_set_by_hand());
    }

    /// The three actions the polkit rule grants are the three this crate
    /// calls, and there is no fourth.
    ///
    /// THE DEFECT THIS CATCHES: a door held open for nobody. `set-local-rtc`
    /// is the obvious fourth and nothing here asks for it; the check
    /// `timedate-polkit-is-narrow` reads this list and the installed rule in
    /// both directions, and this is its other half.
    #[test]
    fn the_actions_are_the_three_that_are_called() {
        assert_eq!(timedate::ACTIONS.len(), 3);
        for verb in ["set-ntp", "set-time", "set-timezone"] {
            let want = format!("org.freedesktop.timedate1.{verb}");
            assert!(timedate::ACTIONS.contains(&want.as_str()), "{want}");
        }
        assert!(
            !timedate::ACTIONS
                .iter()
                .any(|a| a.ends_with("set-local-rtc")),
            "an action nothing calls is a door held open for nobody"
        );
    }
}
