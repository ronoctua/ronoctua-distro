//! The network gate: the installation does not start without a way out.
//!
//! # Why the network stopped being optional
//!
//! The person's repository is mandatory now (their declarations are
//! installed WITH the machine, their belongings restored into its home), the
//! installer asks the release which revision to install, and `nixos-install`
//! downloads what no cache on the ISO holds. None of that happens offline, so
//! the network question is a gate: leaving it requires a link — a cable, or
//! the Wi-Fi network chosen and joined — with no login page in front of it.
//! It answers with a sentence when there is none ([`Gate::key`]), on the
//! screen the person can fix it from.
//!
//! # NetworkManager's verdict on the internet is not the gate's
//!
//! What the gate does NOT judge is whether the link reaches the internet.
//! NetworkManager's answer to that is a plain-HTTP fetch of one page of one
//! host (`nix/modules/network.nix`): a network that blocks port 80, or that
//! host being down, reads as "limited" while GitHub, the binary cache and the
//! person's repository all answer. The hosts that matter are asked right
//! after this — the release, the access, the passphrase — and those answers
//! say for real whether the installation can go on, each on its own screen.
//!
//! [`gate`] is pure over `ronoctua-network`'s [`Snapshot`] (the one crate
//! that talks to NetworkManager); `crate::reach` connects and reads it.

use ronoctua_i18n::key;
use ronoctua_network::{Snapshot, Way};

/// What the gate says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// A link is up, with no login page in front of it: the hosts the
    /// installation needs are asked next.
    Open,
    /// NetworkManager does not answer — no system bus, no daemon.
    NoDaemon,
    /// Networking is switched off.
    Off,
    /// No link is connected: no cable, and no Wi-Fi chosen or joined.
    NoLink,
    /// The chosen Wi-Fi network refused the connection, or vanished.
    Unconnected,
    /// A login page stands between the link and the internet.
    Portal,
}

impl Gate {
    /// Whether the installation may go on.
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
        // HTTP. The release and the access are asked for real right after.
        Way::Full | Way::Unknown | Way::Limited | Way::None => Gate::Open,
    }
}

/// Whether a reading is worth waiting past: a link coming up, or a way out
/// not measured yet. A reading that is not is the one the gate judges.
/// Pure.
#[must_use]
pub fn unsettled(look: &Snapshot) -> bool {
    look.daemon && look.networking && (!look.linked() || look.way == Way::Unknown)
}

/// How long the gate waits, at most, for a link that is coming up — a
/// Wi-Fi network just joined takes seconds to get an address.
pub const SETTLE_MOST_S: u64 = 30;

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_network::{Band, Link, Medium, State};

    fn linked(way: Way) -> Snapshot {
        Snapshot {
            daemon: true,
            networking: true,
            way,
            links: vec![Link {
                medium: Medium::Wired,
                interface: "enp1s0".to_owned(),
                network: None,
                state: State::Connected,
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

    /// FAILS IF: the installation goes on with no daemon, with networking
    /// off, with no link or behind a login page — or stops a link because
    /// NetworkManager's own check (one page over plain HTTP) did not answer:
    /// "limited", "none" or not measured yet are for the release and the
    /// access to judge.
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
            Gate::Portal,
        ] {
            assert!(!g.open(), "{g:?}");
            assert_ne!(g.key(), Gate::Open.key(), "{g:?} says it is open");
        }
        assert!(Gate::Open.open());
        // What is worth waiting for: a link coming up, a way not measured.
        assert!(unsettled(&nothing));
        assert!(unsettled(&linked(Way::Unknown)));
        assert!(!unsettled(&linked(Way::Full)));
        assert!(!unsettled(&off));
        assert!(!unsettled(&Snapshot::default()));
    }
}
