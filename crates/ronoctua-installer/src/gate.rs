//! The network's reading: whether the installer has a way out right now.
//!
//! # Why the network stopped being a gate
//!
//! It was one while the installation downloaded the system and asked the
//! release which installer to use. An installation never uses the network
//! now (the owner's plan): the ISO carries every path it needs, and the
//! person's belongings are restored on the machine's FIRST boot. So the
//! network question goes on with no network at all — the cable's row, or
//! no network — and what it reads is only whether the person's repository
//! can be asked NOW (`crate::model::Answers::online`): with a way out it is
//! proved before the disk is touched, as before; with none, its answers are
//! taken as typed and the first boot checks them.
//!
//! A Wi-Fi network the person CHOSE is still joined and judged: its
//! password is the one the installed machine keeps, and a wrong one is said
//! ([`Gate::key`]) on the screen it is typed on.
//!
//! # NetworkManager's verdict on the internet is not the gate's
//!
//! What the reading does NOT judge is whether the link reaches the
//! internet. NetworkManager's answer to that is a plain-HTTP fetch of one
//! page of one host (`nix/modules/network.nix`): a network that blocks port
//! 80, or that host being down, reads as "limited" while the person's
//! repository answers. The repository is asked right after this — the
//! access, the passphrase — and those answers say for real whether it can
//! be reached, each on its own screen.
//!
//! [`gate`] is pure over `ronoctua-network`'s [`Snapshot`] (the one crate
//! that talks to NetworkManager); `crate::reach` connects and reads it.

use crate::key;
use ronoctua_network::{Snapshot, State, Way};

/// What the reading says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// A link is up, with no login page in front of it: the person's
    /// repository is asked next.
    Open,
    /// NetworkManager does not answer — no system bus, no daemon.
    NoDaemon,
    /// Networking is switched off.
    Off,
    /// No link is connected: no cable, and no Wi-Fi chosen or joined.
    NoLink,
    /// The chosen Wi-Fi network refused its password.
    Unconnected,
    /// The chosen Wi-Fi network was not joined, and its password was not
    /// refused: NetworkManager turned the join down, the network went out
    /// of range, or nothing came up in time. Never said as a password to
    /// check — a right one was (2026-10-25).
    Unjoined,
    /// A login page stands between the link and the internet.
    Portal,
}

impl Gate {
    /// Whether there is a way out: the repository is asked now.
    #[must_use]
    pub const fn open(self) -> bool {
        matches!(self, Self::Open)
    }

    /// The sentence the person reads about it.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Open => key::INST_GATE_OPEN,
            Self::NoDaemon => key::INST_GATE_NO_DAEMON,
            Self::Off => key::INST_GATE_OFF,
            Self::NoLink => key::INST_GATE_NO_LINK,
            Self::Unconnected => key::INST_GATE_UNCONNECTED,
            Self::Unjoined => key::INST_GATE_UNJOINED,
            Self::Portal => key::INST_GATE_PORTAL,
        }
    }

    /// Its id, for the terminal-less mode's English line.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::NoDaemon => "no-daemon",
            Self::Off => "off",
            Self::NoLink => "no-link",
            Self::Unconnected => "unconnected",
            Self::Unjoined => "unjoined",
            Self::Portal => "portal",
        }
    }
}

/// What one reading of NetworkManager says of the way out. Pure.
#[must_use]
pub fn gate(look: &Snapshot) -> Gate {
    if !look.daemon {
        return Gate::NoDaemon;
    }
    if look.airplane() {
        return Gate::Off;
    }
    if !look.linked() {
        return Gate::NoLink;
    }
    match look.way {
        // A LOGIN PAGE is the one thing NetworkManager's check says that is
        // a reason to stop: the person can do something about it here.
        Way::Portal => Gate::Portal,
        // Everything else with a link is NOT judged here — `Unknown` (not
        // measured yet, or the check off), and `Limited` or `None`, which
        // only say that one page of one host did not answer over plain
        // HTTP. The access and the passphrase are asked for real right
        // after.
        Way::Full | Way::Unknown | Way::Limited | Way::None => Gate::Open,
    }
}

/// Whether a reading is worth waiting past: a link coming up, or one up
/// whose way out is not measured yet. A reading that is not is the one the
/// gate judges. Pure.
///
/// NO LINK AT ALL is not waited for: with no cable and no network chosen,
/// that is the answer, and the person who chose to go on without a network
/// would otherwise watch the screen for [`SETTLE_MOST_S`] before it did.
#[must_use]
pub fn unsettled(look: &Snapshot) -> bool {
    let coming = look.links.iter().any(|l| l.state == State::Connecting);
    look.daemon && look.networking && (coming || (look.linked() && look.way == Way::Unknown))
}

/// How long the gate waits, at most, for a link that is coming up — a
/// Wi-Fi network just joined takes seconds to get an address.
pub const SETTLE_MOST_S: u64 = 30;

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_network::{Band, Link, Medium};

    fn linked(way: Way) -> Snapshot {
        with_link(way, State::Connected)
    }

    fn with_link(way: Way, state: State) -> Snapshot {
        Snapshot {
            daemon: true,
            networking: true,
            way,
            links: vec![Link {
                medium: Medium::Wired,
                interface: "enp1s0".to_owned(),
                network: None,
                state,
                rate_kbps: 1_000_000,
                signal: 0,
                band: Band::Other,
                address: Some("192.168.1.4/24".to_owned()),
                gateway: Some("192.168.1.1".to_owned()),
                dns: Vec::new(),
                serving: false,
            }],
            ..Snapshot::default()
        }
    }

    /// FAILS IF: the reading says there is a way out with no daemon, with
    /// networking off, with no link or behind a login page — or says there
    /// is none because NetworkManager's own check (one page over plain
    /// HTTP) did not answer: "limited", "none" or not measured yet are for
    /// the access to judge; or it waits for a link when there is none
    /// coming.
    #[test]
    fn the_gate_opens_only_on_a_way_out() {
        assert_eq!(gate(&Snapshot::default()), Gate::NoDaemon);
        let off = Snapshot {
            daemon: true,
            networking: false,
            ..Snapshot::default()
        };
        assert_eq!(gate(&off), Gate::Off);
        let nothing = Snapshot {
            daemon: true,
            networking: true,
            ..Snapshot::default()
        };
        assert_eq!(gate(&nothing), Gate::NoLink);
        assert_eq!(gate(&linked(Way::Full)), Gate::Open);
        assert_eq!(gate(&linked(Way::Unknown)), Gate::Open);
        assert_eq!(gate(&linked(Way::Portal)), Gate::Portal);
        assert_eq!(gate(&linked(Way::Limited)), Gate::Open);
        assert_eq!(gate(&linked(Way::None)), Gate::Open);
        for g in [
            Gate::NoDaemon,
            Gate::Off,
            Gate::NoLink,
            Gate::Unconnected,
            Gate::Unjoined,
            Gate::Portal,
        ] {
            assert!(!g.open(), "{g:?}");
            assert_ne!(g.key(), Gate::Open.key(), "{g:?} says it is open");
        }
        assert!(Gate::Open.open());
        // What is worth waiting for: a link coming up, a way not measured —
        // and not a machine with no link coming at all, nor a cable that
        // is not plugged in.
        assert!(!unsettled(&nothing));
        assert!(!unsettled(&with_link(Way::None, State::Disconnected)));
        assert!(unsettled(&with_link(Way::None, State::Connecting)));
        assert!(unsettled(&linked(Way::Unknown)));
        assert!(!unsettled(&linked(Way::Full)));
        assert!(!unsettled(&off));
        assert!(!unsettled(&Snapshot::default()));
    }
}
