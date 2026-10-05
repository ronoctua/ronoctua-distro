//! The IPv6 neighbours — the kernel's own table, through netlink.
//!
//! # Why netlink, and why here
//!
//! `/proc/net/arp` is IPv4 and the kernel has no file for the other half: the
//! IPv6 neighbour cache lives behind `RTM_GETNEIGH` on a `NETLINK_ROUTE`
//! socket, which is the conversation `ip -6 neigh` has. One dump, one socket,
//! no privilege — reading the table is open to any process — and the answer is
//! what this machine has spoken to over IPv6, the same promise the ARP table
//! makes over IPv4 (`ronoctua_memory::net::neighbours`).
//!
//! `rustix` and not `libc`, for the reason this crate already gives for its
//! ICMP socket: the API is safe, and the crate is `forbid(unsafe_code)`. The
//! messages are BYTES parsed here, by [`parse`], which is what lets a test
//! hand it a dump made by hand.

use std::net::Ipv6Addr;

use rustix::net::{AddressFamily, SocketFlags, SocketType, netlink};

/// One IPv6 neighbour.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Neighbour6 {
    /// Its address.
    pub address: Ipv6Addr,
    /// Its hardware address, written the way the kernel's ARP file writes
    /// one — two hex digits a byte, colons between — so the maker table reads
    /// both families alike.
    pub hardware: String,
    /// The interface it was seen on, by index.
    pub interface: u32,
    /// Is the entry usable — a hardware address the kernel still stands by?
    /// A FAILED or INCOMPLETE entry is a machine that did not answer.
    pub fresh: bool,
}

/// `RTM_NEWNEIGH`: one entry of the table.
const RTM_NEWNEIGH: u16 = 28;
/// `RTM_GETNEIGH`: the request.
const RTM_GETNEIGH: u16 = 30;
/// `NLMSG_DONE`: the end of a dump.
const NLMSG_DONE: u16 = 3;
/// `NLMSG_ERROR`.
const NLMSG_ERROR: u16 = 2;
/// `NLM_F_REQUEST | NLM_F_DUMP`.
const DUMP: u16 = 0x0001 | 0x0300;
/// `AF_INET6`, as a neighbour message carries it.
const AF_INET6: u8 = 10;
/// `NDA_DST` and `NDA_LLADDR`.
const NDA_DST: u16 = 1;
const NDA_LLADDR: u16 = 2;
/// The states that mean "no hardware address to stand by": `NUD_INCOMPLETE`,
/// `NUD_FAILED`, `NUD_NOARP` (multicast and the like).
const NOT_FRESH: u16 = 0x01 | 0x20 | 0x40;

/// The request: a header and an `ndmsg` asking for the IPv6 family.
#[must_use]
pub fn request(sequence: u32) -> [u8; 28] {
    let mut out = [0u8; 28];
    out[0..4].copy_from_slice(&28u32.to_ne_bytes());
    out[4..6].copy_from_slice(&RTM_GETNEIGH.to_ne_bytes());
    out[6..8].copy_from_slice(&DUMP.to_ne_bytes());
    out[8..12].copy_from_slice(&sequence.to_ne_bytes());
    // `nlmsg_pid` 0: the kernel. Then the `ndmsg`: the family, and zeros —
    // every interface, every state.
    out[16] = AF_INET6;
    out
}

/// What one buffer of a dump says: its entries, and whether the dump is over.
///
/// PURE, and ROBUST to what a kernel may add: an attribute this does not know
/// is stepped over by its length, a message too short to be one ends the
/// buffer, and nothing indexes past the end.
#[must_use]
pub fn parse(buffer: &[u8]) -> (Vec<Neighbour6>, bool) {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 16 <= buffer.len() {
        let length = u32_at(buffer, at) as usize;
        if length < 16 || at + length > buffer.len() {
            break;
        }
        let kind = u16_at(buffer, at + 4);
        if kind == NLMSG_DONE || kind == NLMSG_ERROR {
            return (out, true);
        }
        if kind == RTM_NEWNEIGH
            && let Some(one) = entry(&buffer[at + 16..at + length])
        {
            out.push(one);
        }
        at += align(length);
    }
    (out, false)
}

/// One `ndmsg` and its attributes.
fn entry(body: &[u8]) -> Option<Neighbour6> {
    if body.len() < 12 || body[0] != AF_INET6 {
        return None;
    }
    let interface = u32::from_ne_bytes(body[4..8].try_into().ok()?);
    let state = u16_at(body, 8);
    let mut address = None;
    let mut hardware = None;
    let mut at = 12usize;
    while at + 4 <= body.len() {
        let length = usize::from(u16_at(body, at));
        let kind = u16_at(body, at + 2);
        if length < 4 || at + length > body.len() {
            break;
        }
        let value = &body[at + 4..at + length];
        match kind {
            NDA_DST => {
                let bytes: [u8; 16] = value.try_into().ok()?;
                address = Some(Ipv6Addr::from(bytes));
            }
            NDA_LLADDR if !value.is_empty() => {
                hardware = Some(
                    value
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<Vec<_>>()
                        .join(":"),
                );
            }
            _ => {}
        }
        at += align(length);
    }
    let address = address?;
    Some(Neighbour6 {
        address,
        fresh: hardware.is_some() && state & NOT_FRESH == 0,
        hardware: hardware.unwrap_or_default(),
        interface,
    })
}

/// Netlink's four-byte alignment.
const fn align(length: usize) -> usize {
    (length + 3) & !3
}

fn u16_at(bytes: &[u8], at: usize) -> u16 {
    bytes
        .get(at..at + 2)
        .and_then(|b| b.try_into().ok())
        .map_or(0, u16::from_ne_bytes)
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    bytes
        .get(at..at + 4)
        .and_then(|b| b.try_into().ok())
        .map_or(0, u32::from_ne_bytes)
}

/// The kernel's IPv6 neighbour table, with each entry's interface named.
///
/// One dump on one socket, on the calling thread; it does not touch the
/// network — the table is what the kernel already has.
///
/// # Errors
///
/// The kernel refused the socket or the dump.
pub fn neighbours6() -> anyhow::Result<Vec<(Neighbour6, String)>> {
    // `NETLINK_ROUTE` IS PROTOCOL ZERO, which `rustix` spells as no
    // protocol at all: its `Protocol` cannot hold a zero.
    let socket = rustix::net::socket_with(
        AddressFamily::NETLINK,
        SocketType::RAW,
        SocketFlags::CLOEXEC,
        None,
    )?;
    rustix::net::sendto(
        &socket,
        &request(1),
        rustix::net::SendFlags::empty(),
        &netlink::SocketAddrNetlink::new(0, 0),
    )?;
    let mut found = Vec::new();
    let mut buffer = vec![0u8; 32 * 1024];
    // A CEILING ON THE LOOP: a dump of a few hundred entries is a handful of
    // buffers, and a kernel that never said DONE must not hold a thread.
    for _ in 0..256 {
        let (read, _) = rustix::net::recv(&socket, &mut buffer, rustix::net::RecvFlags::empty())?;
        if read == 0 {
            break;
        }
        let (some, done) = parse(&buffer[..read]);
        found.extend(some);
        if done {
            break;
        }
    }
    // THE INTERFACE BY NAME, the way the ARP file gives it, through an
    // ordinary datagram socket's `SIOCGIFNAME`.
    let probe = rustix::net::socket(AddressFamily::INET6, SocketType::DGRAM, None)
        .or_else(|_| rustix::net::socket(AddressFamily::INET, SocketType::DGRAM, None))?;
    found.sort();
    found.dedup();
    Ok(found
        .into_iter()
        .map(|n| {
            let name = rustix::net::netdevice::index_to_name(&probe, n.interface)
                .unwrap_or_else(|_| n.interface.to_string());
            (n, name)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One `RTM_NEWNEIGH` message, as the kernel writes it.
    fn message(address: [u8; 16], lladdr: Option<[u8; 6]>, state: u16, ifindex: u32) -> Vec<u8> {
        let mut body = Vec::new();
        body.push(AF_INET6);
        body.extend([0u8; 3]);
        body.extend(ifindex.to_ne_bytes());
        body.extend(state.to_ne_bytes());
        body.extend([0u8, 0u8]);
        body.extend(20u16.to_ne_bytes());
        body.extend(NDA_DST.to_ne_bytes());
        body.extend(address);
        if let Some(mac) = lladdr {
            body.extend(10u16.to_ne_bytes());
            body.extend(NDA_LLADDR.to_ne_bytes());
            body.extend(mac);
            body.extend([0u8, 0u8]);
        }
        // AN ATTRIBUTE THIS DOES NOT KNOW, stepped over by its length.
        body.extend(8u16.to_ne_bytes());
        body.extend(9u16.to_ne_bytes());
        body.extend([1u8, 2, 3, 4]);
        let mut out = Vec::new();
        out.extend(u32::try_from(16 + body.len()).unwrap().to_ne_bytes());
        out.extend(RTM_NEWNEIGH.to_ne_bytes());
        out.extend([0u8; 10]);
        out.extend(body);
        out
    }

    fn done() -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(20u32.to_ne_bytes());
        out.extend(NLMSG_DONE.to_ne_bytes());
        out.extend([0u8; 14]);
        out
    }

    #[test]
    fn a_dump_reads_its_entries_and_its_end() {
        let router: Ipv6Addr = "fe80::1".parse().unwrap();
        let phone: Ipv6Addr = "2001:db8::22".parse().unwrap();
        let mut buffer = message(
            router.octets(),
            Some([0xaa, 0xbb, 0xcc, 0x00, 0x11, 0x22]),
            0x02,
            3,
        );
        buffer.extend(message(phone.octets(), None, 0x20, 3));
        buffer.extend(done());
        let (found, over) = parse(&buffer);
        assert!(over, "the dump's end was not read");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].address, router);
        assert_eq!(found[0].hardware, "aa:bb:cc:00:11:22");
        assert_eq!(found[0].interface, 3);
        assert!(found[0].fresh);
        // A FAILED ENTRY, with no hardware address, is a machine that did not
        // answer — listed by the kernel, not by the view.
        assert!(!found[1].fresh);
    }

    #[test]
    fn a_short_or_lying_buffer_reads_nothing_and_does_not_panic() {
        assert_eq!(parse(&[]), (Vec::new(), false));
        assert_eq!(parse(&[1, 2, 3]), (Vec::new(), false));
        // A LENGTH PAST THE END is the end of the buffer.
        let mut lying = message([0; 16], None, 0, 1);
        lying[0..4].copy_from_slice(&9999u32.to_ne_bytes());
        assert_eq!(parse(&lying), (Vec::new(), false));
        // AND AN IPv4 ENTRY is not this table's.
        let mut four = message([0; 16], None, 0, 1);
        four[16] = 2;
        assert_eq!(parse(&four).0, Vec::new());
    }

    /// The real kernel: the dump comes back whole and every interface has
    /// a name. Ignored, because a build sandbox has no network namespace
    /// worth reading — run by hand with `-- --ignored`.
    #[test]
    #[ignore = "reads this machine's own neighbour table"]
    fn the_real_table_reads_and_names_its_interfaces() {
        let found = neighbours6().expect("the kernel answers a dump");
        for (n, name) in &found {
            assert!(!name.is_empty(), "{n:?}");
            assert_ne!(name, &n.interface.to_string(), "{n:?} has no name");
        }
        eprintln!("{} IPv6 neighbours", found.len());
    }

    #[test]
    fn the_request_asks_for_a_dump_of_the_ipv6_family() {
        let r = request(7);
        assert_eq!(u32_at(&r, 0), 28);
        assert_eq!(u16_at(&r, 4), RTM_GETNEIGH);
        assert_eq!(u16_at(&r, 6), DUMP);
        assert_eq!(u32_at(&r, 8), 7);
        assert_eq!(r[16], AF_INET6);
    }
}
