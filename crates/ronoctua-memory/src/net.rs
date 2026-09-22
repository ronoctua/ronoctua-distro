//! The machine's SOCKETS and its interface counters — the second thing this
//! crate reads out of `/proc`, and the whole of what the network view's
//! "ports" and "monitor" sections know.
//!
//! # Why here and not in `ronoctua-network`
//!
//! Because this crate is the border with `/proc`, the same rule that makes
//! `ronoctua-scroll` the only one that speaks to the compositor. What
//! `ronoctua-network` knows is what NetworkManager says; what a listening
//! port is, the daemon has no idea about — it is a row in `/proc/net/tcp`,
//! and a second crate opening `/proc` would be the second border this one
//! exists to prevent.
//!
//! # What can be known, and what honestly cannot
//!
//! A socket's line carries an INODE, not a process. Finding the process means
//! walking `/proc/<pid>/fd` and reading each link, and the kernel only lets
//! one read the fds of one's OWN processes. So `sshd` listening on 22 comes
//! back with no owner — and the screen says so, rather than leaving a blank
//! that reads like "nothing is listening there".
//!
//! # The parsers are pure
//!
//! A `&str` goes in and rows come out, so the whole grammar of
//! `/proc/net/tcp6` — little-endian words, hex ports, a state column — is
//! tested without a `/proc`. Only [`sockets`], [`owners`] and [`counters`]
//! open a file, and their root is a parameter.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::Path;

use crate::Places;

/// Which of the four files a socket came out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Protocol {
    /// TCP over IPv4.
    Tcp,
    /// TCP over IPv6.
    Tcp6,
    /// UDP over IPv4.
    Udp,
    /// UDP over IPv6.
    Udp6,
}

impl Protocol {
    /// The four, in the order the screen lists them.
    pub const ALL: [Self; 4] = [Self::Tcp, Self::Tcp6, Self::Udp, Self::Udp6];

    /// The file under `/proc/net`.
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Tcp6 => "tcp6",
            Self::Udp => "udp",
            Self::Udp6 => "udp6",
        }
    }

    /// The word on screen. NOT a text key: `TCP` is the same in every
    /// language, like `5 GHz` — invariant 8 is about sentences.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Tcp => "TCP",
            Self::Tcp6 => "TCP6",
            Self::Udp => "UDP",
            Self::Udp6 => "UDP6",
        }
    }

    /// Is it UDP? A UDP socket has no `LISTEN` state — the kernel writes `07`
    /// for one nobody has connected, and that is the one worth showing.
    #[must_use]
    pub const fn datagram(self) -> bool {
        matches!(self, Self::Udp | Self::Udp6)
    }

    const fn wide(self) -> bool {
        matches!(self, Self::Tcp6 | Self::Udp6)
    }
}

/// What owns a socket, when the kernel let us see.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Owner {
    /// The process.
    pub pid: u32,
    /// Its `comm` — the name the kernel keeps, at most fifteen bytes.
    pub program: String,
}

/// One socket the kernel is holding open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socket {
    /// Which file it came from.
    pub protocol: Protocol,
    /// The address it is bound to.
    pub address: IpAddr,
    /// The port it is bound to.
    pub port: u16,
    /// The other end, when there is one.
    pub peer: Option<(IpAddr, u16)>,
    /// Is it waiting for somebody — `LISTEN`, or a UDP socket nobody has
    /// connected?
    pub listening: bool,
    /// The inode, which is what ties it to a process's fd.
    pub inode: u64,
    /// Who holds it, when it is one of ours.
    pub owner: Option<Owner>,
}

impl Socket {
    /// `0.0.0.0:22`, `[::]:631` — the form a person searches the web for.
    #[must_use]
    pub fn named(&self) -> String {
        match self.address {
            IpAddr::V4(a) => format!("{a}:{}", self.port),
            IpAddr::V6(a) => format!("[{a}]:{}", self.port),
        }
    }
}

/// One interface's counters since boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counters {
    /// `wlan0`.
    pub interface: String,
    /// Bytes in.
    pub received: u64,
    /// Bytes out.
    pub sent: u64,
}

/// A little-endian hex word as an address: `0100007F` is `127.0.0.1`.
///
/// LITTLE-ENDIAN, and that is the whole trap: the kernel prints the address
/// in the machine's byte order, so reading it big-endian gives `1.0.0.127` —
/// a plausible-looking address that is not the one bound.
#[must_use]
fn ipv4_of(hex: &str) -> Option<IpAddr> {
    // EIGHT DIGITS, like the six-word one below asks for thirty-two: a
    // shorter field parses to a NUMBER perfectly well and comes out as a
    // different address, which is the failure that looks like data.
    if hex.len() != 8 {
        return None;
    }
    let raw = u32::from_str_radix(hex, 16).ok()?;
    Some(IpAddr::V4(Ipv4Addr::from(raw.swap_bytes())))
}

/// Four little-endian words as an IPv6 address.
#[must_use]
fn ipv6_of(hex: &str) -> Option<IpAddr> {
    if hex.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, chunk) in hex.as_bytes().chunks(8).enumerate() {
        let word = u32::from_str_radix(std::str::from_utf8(chunk).ok()?, 16).ok()?;
        bytes[i * 4..i * 4 + 4].copy_from_slice(&word.swap_bytes().to_be_bytes());
    }
    Some(IpAddr::V6(Ipv6Addr::from(bytes)))
}

/// `0100007F:0035` — an address and a port, in the file's own spelling.
fn endpoint_of(field: &str, wide: bool) -> Option<(IpAddr, u16)> {
    let (address, port) = field.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let address = if wide {
        ipv6_of(address)?
    } else {
        ipv4_of(address)?
    };
    Some((address, port))
}

/// `LISTEN`, in the state column.
const LISTEN: &str = "0A";
/// A UDP socket with no peer — the closest thing UDP has to listening.
const UNCONNECTED: &str = "07";

/// The whole of one `/proc/net/*` file.
///
/// PURE, and that is what makes the byte order testable: the addresses in the
/// fixtures are ones whose wrong reading is obviously wrong.
#[must_use]
pub fn parse_sockets(content: &str, protocol: Protocol) -> Vec<Socket> {
    let mut out = Vec::new();
    for line in content.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        // `sl local rem st tx:rx tr:when retrnsmt uid timeout inode`
        if f.len() < 10 {
            continue;
        }
        let Some((address, port)) = endpoint_of(f[1], protocol.wide()) else {
            continue;
        };
        let peer = endpoint_of(f[2], protocol.wide()).filter(|(_, p)| *p != 0);
        let Ok(inode) = f[9].parse::<u64>() else {
            continue;
        };
        let waiting = if protocol.datagram() {
            f[3] == UNCONNECTED
        } else {
            f[3] == LISTEN
        };
        out.push(Socket {
            protocol,
            address,
            port,
            peer,
            listening: waiting,
            inode,
            owner: None,
        });
    }
    out
}

/// Every socket the kernel is holding, with the owner filled in where the
/// kernel let us look.
#[must_use]
pub fn sockets(p: &Places) -> Vec<Socket> {
    let mut out = Vec::new();
    for protocol in Protocol::ALL {
        let path = p.procfs.join("net").join(protocol.file());
        if let Some(text) = crate::read(&path) {
            out.extend(parse_sockets(&text, protocol));
        }
    }
    let mine = owners(&p.procfs);
    for s in &mut out {
        s.owner = mine.get(&s.inode).cloned();
    }
    // LISTENING FIRST, then by port: what a person opens this section to see
    // is what is reachable from outside, and a thousand established sockets
    // would bury it.
    out.sort_by(|a, b| {
        b.listening
            .cmp(&a.listening)
            .then(a.port.cmp(&b.port))
            .then(a.protocol.cmp(&b.protocol))
    });
    out
}

/// Which of OUR processes holds which socket.
///
/// `/proc/<pid>/fd` is readable only for one's own processes, so what comes
/// back is a partial map by construction — see the module's header for why
/// that is said on screen rather than hidden.
#[must_use]
pub fn owners(procfs: &Path) -> HashMap<u64, Owner> {
    use std::os::unix::fs::MetadataExt as _;
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(procfs) else {
        return out;
    };
    let me = rustix::process::getuid().as_raw();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|n| n.parse::<u32>().ok()) else {
            continue;
        };
        // OURS OR NOTHING, and the `stat` is what makes this cheap enough to
        // ride a one-second clock: `/proc/<pid>/fd` of somebody else's
        // process cannot be read anyway, and finding that out by trying is a
        // `readdir` per process — measured at 15 ms a sample on the owner's
        // machine, which is 1,5% of a core spent all day to learn nothing.
        let Ok(about) = entry.metadata() else {
            continue;
        };
        if about.uid() != me {
            continue;
        }
        let Ok(fds) = std::fs::read_dir(entry.path().join("fd")) else {
            // NOT AN ERROR: a process that exited between the listing and
            // this line.
            continue;
        };
        // THE NAME IS READ ONCE A SOCKET IS FOUND, and not before: most
        // processes hold none, and a file read per process is the other half
        // of the cost.
        let mut program: Option<String> = None;
        for fd in fds.flatten() {
            let Ok(target) = std::fs::read_link(fd.path()) else {
                continue;
            };
            let Some(inode) = target
                .to_str()
                .and_then(|t| t.strip_prefix("socket:["))
                .and_then(|t| t.strip_suffix(']'))
                .and_then(|t| t.parse::<u64>().ok())
            else {
                continue;
            };
            let named = program.get_or_insert_with(|| {
                crate::read(&entry.path().join("comm"))
                    .map(|c| c.trim().to_owned())
                    .unwrap_or_default()
            });
            out.entry(inode).or_insert_with(|| Owner {
                pid,
                program: named.clone(),
            });
        }
    }
    out
}

/// One machine this one has spoken to on the local network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Neighbour {
    /// Its address.
    pub address: std::net::IpAddr,
    /// Its hardware address, as the kernel prints it.
    pub hardware: String,
    /// Which of our interfaces it was seen on.
    pub interface: String,
    /// Is the entry usable? The kernel keeps stale ones with `0x0` flags,
    /// and drawing them would list machines that left hours ago.
    pub fresh: bool,
}

/// `/proc/net/arp`, whole.
///
/// IPv4 ONLY, and the file is why: the kernel has no `arp` file for IPv6 —
/// the neighbours there live in netlink, which is a different conversation.
/// The heading says which this is, rather than the screen implying both.
#[must_use]
pub fn parse_neighbours(content: &str) -> Vec<Neighbour> {
    let mut out = Vec::new();
    for line in content.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        // `IP address  HW type  Flags  HW address  Mask  Device`
        if f.len() < 6 {
            continue;
        }
        let Ok(address) = f[0].parse::<std::net::IpAddr>() else {
            continue;
        };
        // `0x2` is COMPLETE; `0x0` is an entry the kernel has given up on.
        let fresh = u32::from_str_radix(f[2].trim_start_matches("0x"), 16).unwrap_or(0) != 0;
        out.push(Neighbour {
            address,
            hardware: f[3].to_owned(),
            interface: f[5].to_owned(),
            fresh,
        });
    }
    out.sort_by_key(|n| n.address);
    out
}

/// The neighbours this machine has spoken to.
#[must_use]
pub fn neighbours(p: &Places) -> Vec<Neighbour> {
    crate::read(&p.procfs.join("net/arp"))
        .as_deref()
        .map(parse_neighbours)
        .unwrap_or_default()
}

/// Every interface's counters since boot, `lo` apart.
///
/// The loopback is left out because this section answers "what is going in
/// and out of this machine", and the loopback goes nowhere: on a desktop it
/// is usually the largest number of the lot and it would be the first thing
/// read.
#[must_use]
pub fn counters(p: &Places) -> Vec<Counters> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(p.sysfs.join("class/net")) else {
        return out;
    };
    for entry in entries.flatten() {
        let interface = entry.file_name().to_string_lossy().into_owned();
        if interface == "lo" {
            continue;
        }
        let number = |which: &str| {
            crate::read(&entry.path().join("statistics").join(which))
                .and_then(|t| t.trim().parse::<u64>().ok())
        };
        let (Some(received), Some(sent)) = (number("rx_bytes"), number("tx_bytes")) else {
            continue;
        };
        out.push(Counters {
            interface,
            received,
            sent,
        });
    }
    out.sort_by(|a, b| a.interface.cmp(&b.interface));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real file's shape, with addresses whose wrong reading is
    /// obviously wrong: `0100007F` is `127.0.0.1` and never `1.0.0.127`.
    const TCP: &str = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 41231 1 0000 100 0 0 10 0
   1: 0100007F:8F31 0100007F:1F90 01 00000000:00000000 00:00000000 00000000  1000        0 41777 1 0000 20 0 0 10 0
   2: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 20114 1 0000 100 0 0 10 0
";

    const TCP6: &str = "\
  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000000000000000000000000000:0277 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 19993 1 0000 100 0 0 10 0
";

    const UDP: &str = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode ref pointer drops
  16: 0100007F:0035 00000000:0000 07 00000000:00000000 00:00000000 00000000   991        0 22334 2 0000 0
";

    #[test]
    fn the_address_is_read_in_the_machines_byte_order() {
        // THE TRAP: the kernel prints the address in the machine's byte
        // order. Read big-endian, `0100007F` is `1.0.0.127` — an address that
        // looks plausible, resolves to nothing, and would have sent somebody
        // hunting for a service that is on localhost all along.
        let rows = parse_sockets(TCP, Protocol::Tcp);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].address, "127.0.0.1".parse::<IpAddr>().unwrap());
        assert_eq!(rows[0].port, 8080, "the port is hex, and 1F90 is 8080");
        assert!(rows[0].listening);
        assert_eq!(rows[0].inode, 41231);
        assert_eq!(rows[0].named(), "127.0.0.1:8080");
        // A CONNECTION IS NOT A LISTENER, and its peer is filled in.
        assert!(!rows[1].listening);
        assert_eq!(
            rows[1].peer,
            Some(("127.0.0.1".parse::<IpAddr>().unwrap(), 8080))
        );
        // AND `0.0.0.0:22`, which is what "the whole world can reach this"
        // looks like.
        assert_eq!(rows[2].address, "0.0.0.0".parse::<IpAddr>().unwrap());
        assert_eq!(rows[2].port, 22);
        assert!(rows[2].listening);
    }

    #[test]
    fn the_six_words_are_little_endian_too() {
        let rows = parse_sockets(TCP6, Protocol::Tcp6);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].address, "::".parse::<IpAddr>().unwrap());
        assert_eq!(rows[0].port, 631);
        assert_eq!(rows[0].named(), "[::]:631");
        assert!(rows[0].listening);
    }

    #[test]
    fn a_udp_socket_with_no_peer_is_the_one_waiting() {
        // UDP HAS NO `LISTEN`. Reading `0A` here would list nothing at all,
        // and the resolver on 53 is exactly the row worth seeing.
        let rows = parse_sockets(UDP, Protocol::Udp);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].listening);
        assert_eq!(rows[0].port, 53);
        assert_eq!(rows[0].peer, None, "a zero port is no peer at all");
    }

    #[test]
    fn a_short_address_is_refused_and_not_read_as_another_one() {
        // `0100007F` IS EIGHT DIGITS. A truncated field parses to a number
        // perfectly well and comes back as a DIFFERENT address, which is a
        // failure that looks exactly like data.
        let short = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 41231 1 0000 100
";
        assert!(parse_sockets(short, Protocol::Tcp).is_empty());
    }

    #[test]
    fn a_short_line_is_skipped_and_not_guessed_at() {
        assert!(parse_sockets("header\n  0: 0100007F:1F90\n", Protocol::Tcp).is_empty());
        assert!(parse_sockets("", Protocol::Tcp).is_empty());
    }

    #[test]
    fn the_neighbours_come_out_of_the_arp_table() {
        const ARP: &str = "\
IP address       HW type     Flags       HW address            Mask     Device
192.168.1.1      0x1         0x2         aa:bb:cc:dd:ee:01     *        wlan0
192.168.1.9      0x1         0x0         00:00:00:00:00:00     *        wlan0
";
        let rows = parse_neighbours(ARP);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].address, "192.168.1.1".parse::<IpAddr>().unwrap());
        assert_eq!(rows[0].hardware, "aa:bb:cc:dd:ee:01");
        assert_eq!(rows[0].interface, "wlan0");
        assert!(rows[0].fresh);
        // AN ENTRY THE KERNEL GAVE UP ON is not a machine on the network:
        // `0x0` is what a neighbour that stopped answering leaves behind.
        assert!(!rows[1].fresh);
    }

    #[test]
    fn the_counters_come_out_of_a_tree_that_can_be_faked() {
        let root = std::env::temp_dir().join(format!("ronoctua-net-{}", std::process::id()));
        let make = |interface: &str, rx: &str, tx: &str| {
            let at = root.join("class/net").join(interface).join("statistics");
            std::fs::create_dir_all(&at).unwrap();
            std::fs::write(at.join("rx_bytes"), rx).unwrap();
            std::fs::write(at.join("tx_bytes"), tx).unwrap();
        };
        make("wlan0", "1024\n", "2048\n");
        make("lo", "999999\n", "999999\n");
        let places = Places {
            sysfs: root.clone(),
            ..Places::default()
        };
        let read = counters(&places);
        std::fs::remove_dir_all(&root).ok();
        // THE LOOPBACK IS LEFT OUT: it is usually the biggest number on the
        // machine and it goes nowhere.
        assert_eq!(
            read,
            vec![Counters {
                interface: "wlan0".to_owned(),
                received: 1024,
                sent: 2048,
            }]
        );
    }
}
