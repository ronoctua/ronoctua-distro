//! Names of machines on the LAN — the one crate that asks systemd-resolved.
//!
//! The LAN view lists neighbours by address; [`names`] asks resolved
//! (`ResolveAddress` on `org.freedesktop.resolve1`, any interface) for the
//! name it knows — from LLMNR, mDNS or the network's DNS, which on a home
//! network is the router answering for the leases it handed out — without a
//! resolver of our own. It never asks on its own: the view asks for the rows
//! it shows, on a thread, and keeps the answers.
//!
//! No answer is not a failure: most addresses have no name, and that is
//! `Ok(None)`.
//!
//! # Why it is a crate
//!
//! `ronoctua-network` speaks NetworkManager and nothing else, and the AGENTS
//! table gives every external service one crate. resolved is a service: this
//! is its crate, and `one-resolve-boundary` refuses a second caller of
//! `resolve1` anywhere else.

#![forbid(unsafe_code)]

use std::net::IpAddr;

/// resolved's name on the system bus.
const SERVICE: &str = "org.freedesktop.resolve1";
/// Its manager object.
const PATH: &str = "/org/freedesktop/resolve1";
/// Its manager interface.
const MANAGER: &str = "org.freedesktop.resolve1.Manager";

/// `AF_INET`, as resolved takes a family.
const AF_INET: i32 = 2;
/// `AF_INET6`.
const AF_INET6: i32 = 10;

/// Why resolved answered nothing, as a closed set (`lan-resolve-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Refusal {
    /// The system bus could not be reached.
    NoBus,
    /// resolved is not running (or not on the bus).
    NoResolved,
    /// resolved answered with a failure other than "no such name".
    Failed,
}

impl Refusal {
    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoBus => "no-bus",
            Self::NoResolved => "no-resolved",
            Self::Failed => "failed",
        }
    }

    /// Every refusal — the shell's `every_resolve_refusal_is_said` demands a
    /// sentence for each, in both languages.
    pub const ALL: [Self; 3] = [Self::NoBus, Self::NoResolved, Self::Failed];
}

/// What one D-Bus error means for an address: no name (`Ok(None)`), or a
/// refusal with its reason.
///
/// PURE, so the classification is tested without a bus: resolved answers
/// "this address has no name" as an ERROR (`NXDOMAIN`, or no PTR record), and
/// reading that as a failure would paint every quiet phone on the network as
/// a fault.
pub fn meaning_of(error_name: &str) -> Result<Option<String>, Refusal> {
    match error_name {
        "org.freedesktop.resolve1.DnsError.NXDOMAIN"
        | "org.freedesktop.resolve1.NoSuchResourceRecord"
        | "org.freedesktop.resolve1.NoNameServers" => Ok(None),
        "org.freedesktop.DBus.Error.ServiceUnknown"
        | "org.freedesktop.DBus.Error.NameHasNoOwner"
        | "org.freedesktop.systemd1.NoSuchUnit" => Err(Refusal::NoResolved),
        _ => Err(Refusal::Failed),
    }
}

/// The first name of an answer — resolved hands a list of `(ifindex, name)`,
/// and a machine with several names is named by the first, which is the one
/// its own lease or announcement gave. A trailing dot is not part of a name
/// anybody reads.
#[must_use]
pub fn first_name(answer: &[(i32, String)]) -> Option<String> {
    answer
        .iter()
        .map(|(_, name)| name.trim_end_matches('.').to_owned())
        .find(|name| !name.is_empty())
}

/// The bytes and the family resolved takes for an address.
fn wire_of(address: IpAddr) -> (i32, Vec<u8>) {
    match address {
        IpAddr::V4(v4) => (AF_INET, v4.octets().to_vec()),
        IpAddr::V6(v6) => (AF_INET6, v6.octets().to_vec()),
    }
}

/// One conversation with resolved, for a batch of addresses.
pub struct Resolver {
    bus: zbus::blocking::Connection,
}

impl Resolver {
    /// Opens the system bus.
    ///
    /// # Errors
    ///
    /// [`Refusal::NoBus`].
    pub fn fresh() -> Result<Self, Refusal> {
        zbus::blocking::Connection::system()
            .map(|bus| Self { bus })
            .map_err(|_| Refusal::NoBus)
    }

    /// The name resolved knows for `address`, or `None` when it knows none.
    /// Blocks on D-Bus (and resolved on the network): call it on a thread.
    ///
    /// # Errors
    ///
    /// [`Refusal::NoResolved`] or [`Refusal::Failed`].
    pub fn name_of(&self, address: IpAddr) -> Result<Option<String>, Refusal> {
        let (family, bytes) = wire_of(address);
        // INTERFACE 0 IS ANY, and FLAGS 0 asks every protocol resolved has —
        // DNS, LLMNR and mDNS — which is the point: the router's DNS knows
        // the leases, LLMNR the Windows machines, mDNS the rest.
        let reply = self.bus.call_method(
            Some(SERVICE),
            PATH,
            Some(MANAGER),
            "ResolveAddress",
            &(0i32, family, bytes, 0u64),
        );
        match reply {
            Ok(message) => {
                let (answer, _flags): (Vec<(i32, String)>, u64) =
                    message.body().deserialize().map_err(|_| Refusal::Failed)?;
                Ok(first_name(&answer))
            }
            Err(zbus::Error::MethodError(name, _, _)) => meaning_of(name.as_str()),
            Err(zbus::Error::FDO(e)) => match *e {
                zbus::fdo::Error::ServiceUnknown(_) | zbus::fdo::Error::NameHasNoOwner(_) => {
                    Err(Refusal::NoResolved)
                }
                _ => Err(Refusal::Failed),
            },
            Err(_) => Err(Refusal::Failed),
        }
    }
}

/// The names of several addresses, on ONE connection — what a thread asks
/// for the rows a view shows. A bus that cannot be reached refuses every one
/// of them with the same reason, which is the truth.
#[must_use]
pub fn names(addresses: &[IpAddr]) -> Vec<(IpAddr, Result<Option<String>, Refusal>)> {
    match Resolver::fresh() {
        Ok(resolver) => addresses
            .iter()
            .map(|a| (*a, resolver.name_of(*a)))
            .collect(),
        Err(why) => addresses.iter().map(|a| (*a, Err(why))).collect(),
    }
}

/// The name resolved knows for one address.
///
/// # Errors
///
/// As [`Resolver::name_of`], and [`Refusal::NoBus`].
pub fn resolve_address(address: IpAddr) -> Result<Option<String>, Refusal> {
    Resolver::fresh()?.name_of(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_such_name_is_no_name_and_not_a_failure() {
        // MOST ADDRESSES HAVE NO NAME, and resolved says so as an error: read
        // as a failure, every phone on the network would be drawn as a fault.
        assert_eq!(
            meaning_of("org.freedesktop.resolve1.DnsError.NXDOMAIN"),
            Ok(None)
        );
        assert_eq!(
            meaning_of("org.freedesktop.resolve1.NoSuchResourceRecord"),
            Ok(None)
        );
        assert_eq!(
            meaning_of("org.freedesktop.DBus.Error.ServiceUnknown"),
            Err(Refusal::NoResolved)
        );
        assert_eq!(
            meaning_of("org.freedesktop.resolve1.DnsError.SERVFAIL"),
            Err(Refusal::Failed)
        );
    }

    #[test]
    fn the_first_name_is_taken_without_its_dot() {
        let answer = vec![(2, "printer.lan.".to_owned()), (2, "other.lan".to_owned())];
        assert_eq!(first_name(&answer), Some("printer.lan".to_owned()));
        assert_eq!(first_name(&[]), None);
        assert_eq!(first_name(&[(0, ".".to_owned())]), None);
    }

    /// The real bus: a machine running resolved names its own loopback.
    /// Ignored, because a build sandbox has no bus — run it by hand with
    /// `cargo test -p ronoctua-resolve -- --ignored`.
    #[test]
    #[ignore = "needs a system bus with resolved on it"]
    fn the_real_resolved_names_the_loopback() {
        let loopback: IpAddr = "127.0.0.1".parse().unwrap();
        assert_eq!(resolve_address(loopback), Ok(Some("localhost".to_owned())));
        let answers = names(&[loopback, "192.0.2.1".parse().unwrap()]);
        assert_eq!(answers.len(), 2);
        assert_eq!(
            answers[1].1,
            Ok(None),
            "a documentation address has no name"
        );
    }

    #[test]
    fn an_address_crosses_as_its_family_and_bytes() {
        let v4: IpAddr = "192.168.1.10".parse().unwrap();
        assert_eq!(wire_of(v4), (AF_INET, vec![192, 168, 1, 10]));
        let v6: IpAddr = "fe80::1".parse().unwrap();
        let (family, bytes) = wire_of(v6);
        assert_eq!(family, AF_INET6);
        assert_eq!(bytes.len(), 16);
        assert_eq!(Refusal::NoResolved.id(), "no-resolved");
        assert_eq!(Refusal::ALL.len(), 3);
    }
}
