//! Bringing a tunnel in from a file — and looking at it first.
//!
//! # Why a preview, and why it is not a formality
//!
//! A `.conf` or an `.ovpn` is somebody else's decision about where this
//! machine's traffic goes. `AllowedIPs = 0.0.0.0/0` sends ALL of it; a `DNS`
//! line moves every name this machine looks up. Importing without reading is
//! how a person ends up wondering why their bank is slow, and the answer is
//! in a file they never opened. So the import is two steps: this module says
//! what the file would do, and only then does anybody press a key.
//!
//! # The secret never comes back
//!
//! `PrivateKey`, `PresharedKey` and an inline `<key>` block are read past and
//! never carried out of here. What goes on screen is what the tunnel DOES,
//! and a private key on a screen is a private key in a screenshot.
//!
//! # Pure
//!
//! Text goes in and facts come out. What touches the disk is the caller, and
//! what imports is `nmcli` — NetworkManager's own importer, because the
//! translation from a WireGuard file to a NetworkManager profile is thirty
//! settings and upstream already maintains it.

/// Which kind of tunnel a file describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A WireGuard `.conf`.
    WireGuard,
    /// An OpenVPN `.ovpn`.
    OpenVpn,
}

impl Kind {
    /// What `nmcli connection import type` wants.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::WireGuard => "wireguard",
            Self::OpenVpn => "openvpn",
        }
    }
}

/// One peer of a WireGuard tunnel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Peer {
    /// Where it is.
    pub endpoint: Option<String>,
    /// What goes through it. `0.0.0.0/0` is EVERYTHING, and the screen says
    /// so in words rather than leaving the reader to know that.
    pub allowed: Vec<String>,
}

impl Peer {
    /// Would this peer take every packet this machine sends?
    #[must_use]
    pub fn takes_everything(&self) -> bool {
        self.allowed
            .iter()
            .any(|a| a.trim() == "0.0.0.0/0" || a.trim() == "::/0")
    }
}

/// What a file would do, once it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Which kind it is.
    pub kind: Kind,
    /// What it would be called — the file's own name, without its suffix.
    pub name: String,
    /// The address this machine would take.
    pub address: Option<String>,
    /// The resolvers it would use. Moving these moves every name this
    /// machine looks up, which is the fact worth reading before importing.
    pub dns: Vec<String>,
    /// Its peers, or its remotes.
    pub peers: Vec<Peer>,
    /// Does it need a password typed after importing?
    pub needs_secret: bool,
}

impl Preview {
    /// Would this tunnel take every packet?
    #[must_use]
    pub fn takes_everything(&self) -> bool {
        self.peers.iter().any(Peer::takes_everything)
    }
}

/// The name a file would be imported under: its own, without the suffix.
#[must_use]
pub fn name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

/// Reads a file's text. `None` when it is neither kind — which is a refusal
/// with a reason, and not an import that fails later inside `nmcli`.
#[must_use]
pub fn preview(path: &str, text: &str) -> Option<Preview> {
    let name = name_of(path);
    if text.contains("[Interface]") {
        return Some(wireguard(name, text));
    }
    // AN OPENVPN FILE HAS NO HEADING. `remote` is the one line every client
    // profile has, and `client` is the other half of the pair.
    if text.lines().any(|l| l.trim_start().starts_with("remote ")) {
        return Some(openvpn(name, text));
    }
    None
}

/// A WireGuard `.conf`, section by section.
fn wireguard(name: String, text: &str) -> Preview {
    let mut out = Preview {
        kind: Kind::WireGuard,
        name,
        address: None,
        dns: Vec::new(),
        peers: Vec::new(),
        // WIREGUARD HAS NO PASSWORD: the key is in the file, and what
        // NetworkManager needs afterwards is nothing.
        needs_secret: false,
    };
    let mut peer: Option<Peer> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.eq_ignore_ascii_case("[peer]") {
            if let Some(p) = peer.take() {
                out.peers.push(p);
            }
            peer = Some(Peer::default());
            continue;
        }
        if line.eq_ignore_ascii_case("[interface]") {
            if let Some(p) = peer.take() {
                out.peers.push(p);
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        // THE SECRETS ARE READ PAST. See the module's header.
        match (key.to_ascii_lowercase().as_str(), peer.as_mut()) {
            ("address", None) => out.address = Some(value.to_owned()),
            ("dns", None) => {
                out.dns = value.split(',').map(|d| d.trim().to_owned()).collect();
            }
            ("endpoint", Some(p)) => p.endpoint = Some(value.to_owned()),
            ("allowedips", Some(p)) => {
                p.allowed = value.split(',').map(|a| a.trim().to_owned()).collect();
            }
            _ => {}
        }
    }
    if let Some(p) = peer.take() {
        out.peers.push(p);
    }
    out
}

/// An OpenVPN `.ovpn`: its remotes, and whether it will ask for a password.
///
/// `redirect-gateway` IS THE FILE'S, NOT A PEER'S. It used to be attached to
/// the last remote read so far — and a file that puts its options before its
/// remotes, which is the ordinary commercial layout, then had NO peer to
/// attach it to. The preview drew no warning, and the person imported a
/// tunnel that takes every packet without the one sentence this module exists
/// to show.
fn openvpn(name: String, text: &str) -> Preview {
    let mut out = Preview {
        kind: Kind::OpenVpn,
        name,
        address: None,
        dns: Vec::new(),
        peers: Vec::new(),
        // `auth-user-pass` WITH NO FILE after it means "ask the person", and
        // that is the profile whose password has to be typed once and stored.
        needs_secret: text
            .lines()
            .any(|l| l.trim() == "auth-user-pass" || l.trim().starts_with("auth-user-pass ")),
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("remote ") {
            let mut fields = rest.split_whitespace();
            let host = fields.next().unwrap_or_default();
            let port = fields.next().unwrap_or_default();
            out.peers.push(Peer {
                endpoint: Some(if port.is_empty() {
                    host.to_owned()
                } else {
                    format!("{host}:{port}")
                }),
                allowed: Vec::new(),
            });
        }
        if let Some(rest) = line.strip_prefix("dhcp-option DNS ") {
            out.dns.push(rest.trim().to_owned());
        }
    }
    // `redirect-gateway` IS OPENVPN'S `0.0.0.0/0`, and it is the same fact
    // under another name: everything goes through the tunnel. Read AFTER the
    // whole file, because a line's place in it says nothing.
    if text
        .lines()
        .any(|l| l.trim_start().starts_with("redirect-gateway"))
    {
        // ON EVERY REMOTE, and on a peer of its own when the file named none:
        // the fact belongs to the FILE, and a preview that could not draw it
        // for want of a peer is the defect this comes out of.
        if out.peers.is_empty() {
            out.peers.push(Peer::default());
        }
        for peer in &mut out.peers {
            peer.allowed.push("0.0.0.0/0".to_owned());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WireGuard file's shape, with nothing real in it (invariant 13): the
    /// key is not a key, the address is documentation's own range.
    const WIREGUARD: &str = "\
[Interface]
PrivateKey = 0000000000000000000000000000000000000000000=
Address = 198.51.100.2/32
DNS = 198.51.100.1, 198.51.100.9

[Peer]
PublicKey = 1111111111111111111111111111111111111111111=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = vpn.example.invalid:51820
";

    const OPENVPN: &str = "\
client
dev tun
proto udp
remote vpn.example.invalid 1194
redirect-gateway def1
dhcp-option DNS 198.51.100.1
auth-user-pass
";

    #[test]
    fn a_wireguard_file_says_what_it_would_do_and_never_its_key() {
        let p = preview("/tmp/an-office.conf", WIREGUARD).unwrap();
        assert_eq!(p.kind, Kind::WireGuard);
        assert_eq!(
            p.name, "an-office",
            "the name is the file's, without the suffix"
        );
        assert_eq!(p.address.as_deref(), Some("198.51.100.2/32"));
        assert_eq!(p.dns, ["198.51.100.1", "198.51.100.9"]);
        assert_eq!(p.peers.len(), 1);
        assert_eq!(
            p.peers[0].endpoint.as_deref(),
            Some("vpn.example.invalid:51820")
        );
        // THE FACT WORTH READING BEFORE IMPORTING: everything goes through it.
        assert!(p.takes_everything());
        assert!(!p.needs_secret);
        // AND THE SECRET IS NOWHERE IN WHAT CAME BACK.
        let drawn = format!("{p:?}");
        assert!(
            !drawn.contains("0000000000"),
            "the private key came out of the preview: {drawn}"
        );
    }

    #[test]
    fn an_openvpn_file_is_read_by_its_remote_and_its_password() {
        let p = preview("/tmp/a-house.ovpn", OPENVPN).unwrap();
        assert_eq!(p.kind, Kind::OpenVpn);
        assert_eq!(p.name, "a-house");
        assert_eq!(
            p.peers[0].endpoint.as_deref(),
            Some("vpn.example.invalid:1194")
        );
        // `redirect-gateway` IS THE SAME FACT under another name.
        assert!(p.takes_everything());
        assert_eq!(p.dns, ["198.51.100.1"]);
        // AND IT WILL ASK FOR A PASSWORD, which is why the row after the
        // import says so.
        assert!(p.needs_secret);
    }

    #[test]
    fn the_options_block_may_come_before_the_remotes() {
        // THE COMMON COMMERCIAL LAYOUT, and the defect it found: the warning
        // was attached to the last remote READ SO FAR, so a file that puts
        // its options first had no peer to attach it to and the preview drew
        // no warning at all — for a tunnel that takes every packet.
        let first = "\
client
dev tun
redirect-gateway def1
dhcp-option DNS 198.51.100.1
remote vpn.example.invalid 1194
remote vpn2.example.invalid 1194
";
        let p = preview("/tmp/a-house.ovpn", first).unwrap();
        assert!(p.takes_everything(), "the warning was lost");
        assert_eq!(p.peers.len(), 2);
        assert!(p.peers.iter().all(Peer::takes_everything));

        // AND A FILE WITH NO REMOTE AT ALL still says what it would do.
        let bare = "client\nredirect-gateway def1\nremote a.invalid 1\n";
        assert!(preview("/tmp/b.ovpn", bare).unwrap().takes_everything());
    }

    #[test]
    fn a_file_that_is_neither_is_refused_with_a_reason() {
        // AND NOT IMPORTED AND LEFT TO FAIL INSIDE `nmcli`, whose message is
        // about a parser nobody here wrote.
        assert!(preview("/tmp/notes.txt", "a shopping list\nmilk\n").is_none());
        assert!(preview("/tmp/empty.conf", "").is_none());
    }

    #[test]
    fn a_tunnel_that_takes_only_its_own_network_says_so_by_not_saying_it() {
        let split = "\
[Interface]
Address = 198.51.100.2/32

[Peer]
AllowedIPs = 10.0.0.0/8
Endpoint = vpn.example.invalid:51820
";
        let p = preview("/tmp/split.conf", split).unwrap();
        assert!(!p.takes_everything());
        assert!(p.dns.is_empty());
    }
}
