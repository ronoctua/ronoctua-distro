//! Who else is on this network.
//!
//! # What it can find, and what it cannot
//!
//! It sends an ICMP echo to every address of the local network and lists what
//! answers. A machine that is there and does not answer — a firewall that
//! drops pings, which is most Windows machines — is NOT here, and the screen
//! says so rather than implying the network is empty. The kernel's own ARP
//! table (`ronoctua_memory::net::neighbours`) is the other half: it holds
//! everything this machine has actually spoken to, answered pings or not, and
//! the sweep fills it as a side effect.
//!
//! # No privilege, and no `nmap` process
//!
//! The socket is `SOCK_DGRAM`/`IPPROTO_ICMP`, which needs nothing (see
//! [`icmp`]). `nmap` is in this system for its VENDOR TABLE only, which is a
//! text file — running it would be a process per host and a scan somebody
//! else's network would notice.

#![forbid(unsafe_code)]

pub mod icmp;
pub mod oui;

/// How long a sweep waits for the slowest answer.
///
/// One second: a local network answers in single milliseconds, and what a
/// longer wait buys is a machine over a bad wifi link — which is worth one
/// second and not five.
pub const PATIENCE: std::time::Duration = std::time::Duration::from_secs(1);

/// What one sweep found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Who answered, in order.
    pub replies: Vec<icmp::Reply>,
    /// How many addresses were asked.
    pub asked: usize,
    /// WHICH addresses — `192.168.1.1-254`. A sweep is capped at 254, so on
    /// anything wider than a `/24` it asked about a PART of the network, and
    /// a count with no range beside it reads as "this is everybody".
    pub range: String,
    /// What the kernel said when it would not give us a socket at all.
    ///
    /// A REFUSAL IS NOT AN EMPTY NETWORK. `net.ipv4.ping_group_range` can be
    /// narrowed, and a sweep that came back with nothing then read exactly
    /// like a network with nobody on it — the two need different words.
    pub refused: Option<String>,
}

/// One echo to one machine, and how long it took — the honest round trip.
///
/// # Errors
///
/// The kernel refused the socket.
pub fn ping(address: std::net::Ipv4Addr) -> anyhow::Result<Option<u32>> {
    icmp::ping(address, PATIENCE)
}

/// Sweeps the network of one address, on the calling thread.
///
/// # Errors
///
/// The kernel refused the socket — which on this system means
/// `net.ipv4.ping_group_range` was narrowed.
pub fn sweep(address_with_prefix: &str) -> anyhow::Result<Found> {
    let hosts = icmp::hosts_of(address_with_prefix);
    let range = match (hosts.first(), hosts.last()) {
        (Some(first), Some(last)) if first == last => first.to_string(),
        (Some(first), Some(last)) => format!("{first}-{last}"),
        _ => String::new(),
    };
    // THE REFUSAL IS A VALUE, not an error the caller has to word: both
    // doors draw this straight into a row.
    let (replies, refused) = match icmp::sweep(&hosts, PATIENCE) {
        Ok(replies) => (replies, None),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };
    Ok(Found {
        replies,
        asked: hosts.len(),
        range,
        refused,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_sweep_of_nothing_asks_nothing() {
        // AN ADDRESS THAT IS NOT ONE is no hosts, and the sweep is a socket
        // that is opened and closed — never an error the screen has to word.
        let found = super::sweep("not an address").unwrap();
        assert_eq!(found.asked, 0);
        assert!(found.replies.is_empty());
        assert!(found.range.is_empty());

        // AND A REAL NETWORK SAYS WHICH ADDRESSES IT ASKED — the sweep is
        // capped at 254, so on anything wider than a `/24` this is the only
        // thing that keeps the count from reading as "everybody".
        let whole = super::icmp::hosts_of("192.168.1.5/16");
        assert_eq!(whole.len(), 254);
    }
}
