//! An ICMP sweep, with no privilege at all.
//!
//! # Why this does not need root, and why that is not luck
//!
//! Linux has a third kind of ICMP socket: `SOCK_DGRAM` with `IPPROTO_ICMP`,
//! open to any process whose group is inside `net.ipv4.ping_group_range`. On
//! NixOS that range is `0 2147483647` by default — every group — which is
//! what `ping` itself uses rather than a setuid binary.
//!
//! The kernel does two things for such a socket that a raw one would leave to
//! us: it fills in the echo IDENTIFIER, and it computes the CHECKSUM. So the
//! packet written here has zeros in both, and a reply's identifier is not
//! ours to match on — the SEQUENCE is, and that is what the sweep tracks.
//!
//! # Why not ARP
//!
//! An ARP probe needs a raw socket, which needs `CAP_NET_RAW`, which needs a
//! capability on the binary. What this does instead is send an echo to every
//! address in the range: the machines that answer are listed, and the ones
//! that do not answer but ANSWERED THE ARP are in the kernel's own table —
//! which `ronoctua_memory::net::neighbours` reads, for free, and which this
//! sweep fills as a side effect.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use rustix::net::{AddressFamily, SocketType, ipproto};

/// One machine that answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reply {
    /// Who answered.
    pub address: Ipv4Addr,
}

/// An echo request, as the kernel wants it from a datagram socket.
///
/// TYPE 8, CODE 0, and zeros where the kernel writes: the identifier is the
/// socket's and the checksum is computed on the way out. Writing our own
/// identifier is the mistake that makes a sweep see no replies at all.
#[must_use]
pub fn echo(sequence: u16) -> [u8; 8] {
    let [high, low] = sequence.to_be_bytes();
    [8, 0, 0, 0, 0, 0, high, low]
}

/// The sequence number in a reply, when it is one.
///
/// A datagram ICMP socket may hand back the IP header as well — it depends on
/// the kernel and on whether the reply came through an error — so a packet
/// beginning with an IPv4 header (`0x45`) is stepped over.
#[must_use]
pub fn sequence_of(packet: &[u8]) -> Option<u16> {
    let body = if packet.first().is_some_and(|b| b >> 4 == 4) {
        let length = usize::from(packet.first()? & 0x0f) * 4;
        packet.get(length..)?
    } else {
        packet
    };
    // TYPE 0 is `echo reply`; anything else is somebody else's business.
    if *body.first()? != 0 {
        return None;
    }
    Some(u16::from_be_bytes([*body.get(6)?, *body.get(7)?]))
}

/// Every address of a network, from one address in it and its prefix.
///
/// CAPPED AT 254, WHICH IS THE `/24` AROUND THE ADDRESS — not the last 254 of
/// whatever the prefix covers, which is what this used to do. A sweep of a
/// `/16` is sixty-five thousand packets and a minute of waiting, so it has to
/// be cut; cutting it at the END put `192.168.1.5/16` on `192.168.255.x` and
/// `10.0.4.7/22` on `10.0.7.x` — ranges that hold neither this machine nor
/// its neighbours, so `l` reported "0 of 254" on a network with five machines
/// on it. The 254 addresses that answer the question are the ones next to the
/// address we were given.
#[must_use]
pub fn hosts(address: Ipv4Addr, prefix: u8) -> Vec<Ipv4Addr> {
    let raw = u64::from(u32::from(address));
    // NARROWER THAN A `/24` IS SWEPT WHOLE; anything wider is swept as the
    // `/24` this address is in.
    let width = u32::from(prefix.clamp(0, 32).max(24));
    let mask = u64::from(u32::MAX << (32 - width));
    let network = raw & mask;
    let size = 1u64 << (32 - width);
    // THE HOSTS, and neither the network's own address nor its broadcast —
    // except on a `/31` and a `/32`, which have neither and where both
    // addresses are somebody.
    let (first, last) = if size <= 2 {
        (network, network + size - 1)
    } else {
        (network + 1, network + size - 2)
    };
    (first..=last)
        .filter_map(|n| u32::try_from(n).ok())
        .map(Ipv4Addr::from)
        .collect()
}

/// Sends one echo to every address and collects what comes back.
///
/// ON A THREAD, ALWAYS: it waits `patience` for the slowest answer, which is
/// a second the caller's loop must not spend — the `systemd-run --scope` trap
/// CLAUDE.md records.
///
/// # Errors
///
/// The kernel refused the socket, which on this system means
/// `net.ipv4.ping_group_range` was narrowed.
pub fn sweep(addresses: &[Ipv4Addr], patience: Duration) -> anyhow::Result<Vec<Reply>> {
    // NOTHING TO ASK IS NOT A SWEEP: without this the loop below waits the
    // whole `patience` for replies to packets nobody sent, and the socket is
    // opened on a machine whose `ping_group_range` might refuse it — which
    // would turn "there is no address to sweep" into an error on screen.
    if addresses.is_empty() {
        return Ok(Vec::new());
    }
    let socket = open()?;
    let mut sent: Vec<(u16, Ipv4Addr)> = Vec::with_capacity(addresses.len());
    let mut out: Vec<Reply> = Vec::new();
    let mut packet = [0u8; 256];
    for (i, address) in addresses.iter().enumerate() {
        let sequence = u16::try_from(i).unwrap_or(u16::MAX);
        let to = SocketAddr::V4(SocketAddrV4::new(*address, 0));
        // A REFUSAL PER ADDRESS IS NOT A FAILED SWEEP: an unreachable network
        // answers `EHOSTUNREACH` on the send, and the other 253 are still
        // worth asking.
        if rustix::net::sendto(
            &socket,
            &echo(sequence),
            rustix::net::SendFlags::empty(),
            &to,
        )
        .is_err()
        {
            continue;
        }
        sent.push((sequence, *address));
        // AND WHAT HAS ALREADY COME BACK IS READ NOW, without waiting, so a
        // fast network's replies do not pile up behind three seconds of
        // sending.
        while let Ok((read, _)) =
            rustix::net::recv(&socket, &mut packet, rustix::net::RecvFlags::DONTWAIT)
        {
            record(&packet[..read], &sent, &mut out);
        }
    }
    // AND THEN THE WAIT, which is what the slowest machine on a bad wifi link
    // needs. THE DEADLINE STARTS HERE and not before the sending: sending to
    // 254 addresses is not instant, because each one whose hardware address
    // the kernel does not know yet waits for ARP — measured on the owner's
    // own network, three seconds. Started earlier, the whole receive window
    // was gone before the first packet went out, and the sweep found nobody
    // on a network with five machines on it.
    let listening = Instant::now();
    while listening.elapsed() < patience {
        let Ok((read, _)) =
            rustix::net::recv(&socket, &mut packet, rustix::net::RecvFlags::empty())
        else {
            continue;
        };
        record(&packet[..read], &sent, &mut out);
    }
    out.sort();
    Ok(out)
}

/// One echo to one machine, and how long it took.
///
/// THIS is where a round trip is measured, and the sweep is not: a sweep's
/// first packet to a machine waits for ARP — three seconds on the owner's own
/// network — and the number would read as a terrible link for a machine in
/// the next room. One address at a time, already resolved by the sweep that
/// listed it, measures the thing it claims to.
///
/// `None` means it did not answer inside `patience`.
///
/// # Errors
///
/// The kernel refused the socket.
pub fn ping(address: Ipv4Addr, patience: Duration) -> anyhow::Result<Option<u32>> {
    let socket = open()?;
    let to = SocketAddr::V4(SocketAddrV4::new(address, 0));
    let at = Instant::now();
    rustix::net::sendto(&socket, &echo(1), rustix::net::SendFlags::empty(), &to)
        .map_err(|e| anyhow::anyhow!("{address}: {e}"))?;
    let mut packet = [0u8; 256];
    while at.elapsed() < patience {
        let Ok((read, _)) =
            rustix::net::recv(&socket, &mut packet, rustix::net::RecvFlags::empty())
        else {
            continue;
        };
        if sequence_of(&packet[..read]) == Some(1) {
            return Ok(Some(
                u32::try_from(at.elapsed().as_millis()).unwrap_or(u32::MAX),
            ));
        }
    }
    Ok(None)
}

/// The socket both of them use.
fn open() -> anyhow::Result<rustix::fd::OwnedFd> {
    let socket = rustix::net::socket(AddressFamily::INET, SocketType::DGRAM, Some(ipproto::ICMP))
        .map_err(|e| anyhow::anyhow!("no unprivileged ICMP socket: {e}"))?;
    // A DEADLINE ON THE READ, or the last `recv` waits for a packet that is
    // never coming and the thread never ends.
    rustix::net::sockopt::set_socket_timeout(
        &socket,
        rustix::net::sockopt::Timeout::Recv,
        Some(Duration::from_millis(100)),
    )?;
    Ok(socket)
}

/// Puts one reply against the request it answers.
///
/// A SECOND REPLY FROM THE SAME MACHINE does not add a row: some stacks send
/// several, and a list with a machine twice reads as two machines.
fn record(packet: &[u8], sent: &[(u16, Ipv4Addr)], out: &mut Vec<Reply>) {
    let Some(sequence) = sequence_of(packet) else {
        return;
    };
    let Some((_, address)) = sent.iter().find(|(s, _)| *s == sequence) else {
        return;
    };
    if out.iter().any(|r| r.address == *address) {
        return;
    }
    out.push(Reply { address: *address });
}

/// The addresses of the network an interface is on, from `192.168.1.5/24`.
#[must_use]
pub fn hosts_of(address_with_prefix: &str) -> Vec<Ipv4Addr> {
    let (address, prefix) = address_with_prefix
        .split_once('/')
        .unwrap_or((address_with_prefix, "24"));
    let (Ok(address), Ok(prefix)) = (address.parse::<Ipv4Addr>(), prefix.parse::<u8>()) else {
        return Vec::new();
    };
    hosts(address, prefix)
}

/// Which of two addresses is this machine's own, for the row that marks it.
#[must_use]
pub fn mine(address_with_prefix: &str) -> Option<IpAddr> {
    address_with_prefix
        .split('/')
        .next()?
        .parse::<IpAddr>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_echo_leaves_the_identifier_and_the_checksum_to_the_kernel() {
        // WRITING OUR OWN IDENTIFIER is the mistake that makes a datagram
        // sweep see no replies at all: the kernel rewrites the field, and a
        // matcher that expected its own number matches nothing.
        let packet = echo(7);
        assert_eq!(packet[0], 8, "type 8 is an echo request");
        assert_eq!(packet[1], 0);
        assert_eq!(
            &packet[2..6],
            &[0, 0, 0, 0],
            "checksum and id are the kernel's"
        );
        assert_eq!(&packet[6..8], &[0, 7]);
    }

    #[test]
    fn a_reply_is_read_with_or_without_the_ip_header() {
        // BARE, as a datagram socket usually hands it over.
        let bare = [0u8, 0, 0x12, 0x34, 0, 0, 0, 9];
        assert_eq!(sequence_of(&bare), Some(9));
        // AND WITH THE HEADER, which is what some kernels do: `0x45` is IPv4
        // with a five-word header, so the body starts twenty bytes in.
        let mut wrapped = vec![0x45u8; 20];
        wrapped.extend_from_slice(&bare);
        assert_eq!(sequence_of(&wrapped), Some(9));
        // AND SOMETHING THAT IS NOT AN ECHO REPLY is not ours to count.
        let unreachable = [3u8, 1, 0, 0, 0, 0, 0, 9];
        assert_eq!(sequence_of(&unreachable), None);
        assert_eq!(sequence_of(&[]), None);
    }

    #[test]
    fn a_network_is_swept_without_its_own_address_or_its_broadcast() {
        let whole = hosts("192.168.1.5".parse().unwrap(), 24);
        assert_eq!(whole.len(), 254);
        assert_eq!(whole[0], "192.168.1.1".parse::<Ipv4Addr>().unwrap());
        assert_eq!(whole[253], "192.168.1.254".parse::<Ipv4Addr>().unwrap());
        // A SMALLER NETWORK IS SMALLER, and not padded out to 254.
        let small = hosts("10.0.0.2".parse().unwrap(), 30);
        assert_eq!(small.len(), 2);
        assert_eq!(small[0], "10.0.0.1".parse::<Ipv4Addr>().unwrap());
        // A `/31` IS A LINK BETWEEN TWO MACHINES: no network address, no
        // broadcast, and both halves are somebody.
        assert_eq!(hosts("10.0.0.2".parse().unwrap(), 31).len(), 2);
        assert_eq!(hosts("10.0.0.2".parse().unwrap(), 32).len(), 1);
    }

    #[test]
    fn a_wider_prefix_is_swept_around_the_address_and_not_at_its_end() {
        // THE DEFECT THIS PREVENTS: the cap kept the LAST 254 addresses of
        // whatever the prefix covered, so `192.168.1.5/16` swept
        // `192.168.255.x` and a `/22` office network swept a range holding
        // neither this machine nor its neighbours — "0 of 254 answered" on a
        // network with five machines on it.
        let wide = hosts("192.168.1.5".parse().unwrap(), 16);
        assert_eq!(wide.len(), 254);
        assert_eq!(wide[0], "192.168.1.1".parse::<Ipv4Addr>().unwrap());
        assert_eq!(wide[253], "192.168.1.254".parse::<Ipv4Addr>().unwrap());
        // AND A `/22`, which is what an office or an ISP hands out.
        let office = hosts("10.0.4.7".parse().unwrap(), 22);
        assert_eq!(office[0], "10.0.4.1".parse::<Ipv4Addr>().unwrap());
        assert!(
            office.contains(&"10.0.4.7".parse::<Ipv4Addr>().unwrap()),
            "the sweep does not include the machine's own network"
        );
    }

    #[test]
    fn the_address_and_its_prefix_come_out_of_the_one_string() {
        assert_eq!(hosts_of("192.168.1.5/24").len(), 254);
        assert_eq!(hosts_of("192.168.1.5").len(), 254, "no prefix is a /24");
        assert!(hosts_of("not an address").is_empty());
        assert_eq!(
            mine("192.168.1.5/24"),
            Some("192.168.1.5".parse::<IpAddr>().unwrap())
        );
    }
}
