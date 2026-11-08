//! `ronoctua-packets` — the network protocols' manual, as data.
//!
//! # What a protocol is here
//!
//! A [`Protocol`] is shown on a small network — its [`Scene`]: devices on an
//! isometric floor, joined by links — and told in [`Step`]s, each of which
//! frames some of the devices and plays [`Beat`]s on them: a [`Packet`] sent
//! from one device to another (it flies along the links between them), an
//! address typed, a device lit. The window (`packets_window.rs` in
//! `ronoctua-shell-manual`) draws the scene, flies the packets, lists them as
//! a capture would, explains each step, and opens a packet's headers bit by
//! bit ([`anatomy`]). Where everything is at a moment is [`timeline`]'s
//! answer, a pure function of the time; what the devices look like is
//! [`art`]'s.
//!
//! # Words and wire
//!
//! Everything a protocol SAYS ABOUT itself — its name, what it is for, what
//! each step does and why — is a key of this crate's book (`packets`, read
//! only by the window that opens it: `ronoctua_i18n::Book`), complete in
//! every declared language. What travels ON THE WIRE — `GET / HTTP/1.1`,
//! `[SYN, ACK] Seq=0 Ack=1`, an address, a port, a domain of the examples —
//! is the same in every language, as a capture shows it, and is DATA here
//! (invariant 8, the owner's exception of 2026-11-08): the `&'static str`s of
//! a [`Packet`] and of [`anatomy`]'s values. Addresses are typed numbers
//! ([`Address`]), formatted, never written as text.

#![deny(unsafe_code)]

pub mod anatomy;
pub mod art;
pub mod catalogue;
pub mod key;
pub mod timeline;

use ronoctua_i18n::BookKey;

/// The book this crate's sentences are (`ronoctua_i18n::Book::open`).
pub use key::BOOK;

/// A protocol of the manual.
#[derive(Debug)]
pub struct Protocol {
    /// The word that names it — the list's filter matches it, and a
    /// photograph asks for it (`--protocol http`).
    pub id: &'static str,
    /// Where the list puts it.
    pub group: Group,
    /// The layer of the OSI model it works at.
    pub layer: Layer,
    /// Its short name: `HTTP`.
    pub name: BookKey,
    /// What the letters stand for, or its long name.
    pub title: BookKey,
    /// What it does, in a line.
    pub tagline: BookKey,
    /// What it is for and how it works, in a paragraph.
    pub overview: BookKey,
    /// Its facts: layer, port, transport, format, where it is defined.
    pub facts: &'static [Fact],
    /// The network it is shown on.
    pub scene: Scene,
    /// What it does, step by step.
    pub steps: &'static [Step],
}

/// A fact of a protocol or a step: what it is called, and its value.
#[derive(Debug, Clone, Copy)]
pub struct Fact {
    /// What it is called.
    pub label: BookKey,
    /// What it is.
    pub value: Value,
}

/// A fact's value: words, or wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// Words, in the book.
    Said(BookKey),
    /// What the wire carries: a URL, a port, a record.
    Wire(&'static str),
}

/// Where the list puts a protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Group {
    /// The link and the network: frames, addresses, routes.
    Link,
    /// Transport: the conversations between programs.
    Transport,
    /// Names and discovery.
    Naming,
    /// The web.
    Web,
    /// Mail and files.
    Mail,
    /// Security and tunnels.
    Security,
    /// Time, machines and peers.
    Things,
}

impl Group {
    /// Every group, in the list's order.
    pub const ALL: [Self; 7] = [
        Self::Link,
        Self::Transport,
        Self::Naming,
        Self::Web,
        Self::Mail,
        Self::Security,
        Self::Things,
    ];

    /// Its name in the list.
    #[must_use]
    pub const fn name(self) -> BookKey {
        match self {
            Self::Link => key::PACKETS_GROUP_LINK,
            Self::Transport => key::PACKETS_GROUP_TRANSPORT,
            Self::Naming => key::PACKETS_GROUP_NAMING,
            Self::Web => key::PACKETS_GROUP_WEB,
            Self::Mail => key::PACKETS_GROUP_MAIL,
            Self::Security => key::PACKETS_GROUP_SECURITY,
            Self::Things => key::PACKETS_GROUP_THINGS,
        }
    }
}

/// A layer of the OSI model, 1 (physical) to 7 (application).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Layer(pub u8);

impl Layer {
    /// Every layer, from the wire up.
    pub const ALL: [Self; 7] = [
        Self(1),
        Self(2),
        Self(3),
        Self(4),
        Self(5),
        Self(6),
        Self(7),
    ];

    /// Its name: `Application`.
    #[must_use]
    pub const fn name(self) -> BookKey {
        match self.0 {
            1 => key::PACKETS_LAYER_1,
            2 => key::PACKETS_LAYER_2,
            3 => key::PACKETS_LAYER_3,
            4 => key::PACKETS_LAYER_4,
            5 => key::PACKETS_LAYER_5,
            6 => key::PACKETS_LAYER_6,
            _ => key::PACKETS_LAYER_7,
        }
    }
}

/// The network a protocol is shown on.
#[derive(Debug, Clone, Copy)]
pub struct Scene {
    /// Its devices.
    pub devices: &'static [Device],
    /// What joins them.
    pub links: &'static [Link],
}

/// A device of a scene.
#[derive(Debug, Clone, Copy)]
pub struct Device {
    /// What it is, and so how it is drawn.
    pub kind: art::Kind,
    /// Where it stands.
    pub at: Spot,
    /// What the scene calls it: `Client`, `DNS resolver`.
    pub name: BookKey,
    /// Its address, under its name.
    pub address: Address,
}

/// A place on the floor: a column and a row of the isometric lattice, a
/// column half a tile across and a row half a tile down. Where a device
/// stands is the floor point under its middle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spot {
    /// Across, in half tiles.
    pub col: i8,
    /// Down, in half tiles.
    pub row: i8,
}

/// A link between two devices of a scene, by their places in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    /// One end.
    pub a: usize,
    /// The other.
    pub b: usize,
    /// How long a packet takes across it, in milliseconds — the capture's
    /// clock, not the animation's.
    pub ms: u16,
}

/// An address, as a device of a scene has one. Typed numbers, formatted —
/// never text written by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Address {
    /// It shows none.
    None,
    /// IPv4.
    V4([u8; 4]),
    /// IPv6, in its eight groups.
    V6([u16; 8]),
    /// A link layer (MAC) address.
    Mac([u8; 6]),
    /// An autonomous system's number.
    As(u32),
    /// A name, as the wire carries it.
    Host(&'static str),
}

impl Address {
    /// How the address is written, or `None` when it shows none.
    #[must_use]
    pub fn text(self) -> Option<String> {
        match self {
            Self::None => None,
            Self::V4(o) => Some(std::net::Ipv4Addr::from(o).to_string()),
            Self::V6(g) => Some(std::net::Ipv6Addr::from(g).to_string()),
            Self::Mac(m) => Some(
                m.iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(":"),
            ),
            Self::As(n) => Some(format!("AS{n}")),
            Self::Host(h) => Some(h.to_owned()),
        }
    }
}

/// A step of a protocol: what it does, which devices it is about, and what
/// happens on them.
#[derive(Debug, Clone, Copy)]
pub struct Step {
    /// Its title: `DNS resolution`.
    pub title: BookKey,
    /// What it does, in a few words.
    pub sub: BookKey,
    /// What happens, and why, in a paragraph.
    pub body: BookKey,
    /// Its facts.
    pub facts: &'static [Fact],
    /// The devices the camera frames while it plays — none for the whole
    /// scene.
    pub focus: &'static [usize],
    /// What happens, in order.
    pub beats: &'static [Beat],
}

/// One thing that happens in a step.
#[derive(Debug, Clone, Copy)]
pub enum Beat {
    /// A packet is sent: it flies from one device to another along the
    /// links between them, and is listed as it leaves.
    Send(Packet),
    /// Something is typed beside a device: an address, a command.
    Type {
        /// The device.
        at: usize,
        /// What is typed — wire, as typed.
        wire: &'static str,
    },
    /// A device lights, and shows a sign of what it does now.
    Light {
        /// The device.
        at: usize,
        /// The sign, if any.
        prop: Option<art::Prop>,
    },
    /// A pause.
    Rest,
}

/// What a packet is, for its colour and the legend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Class {
    /// A question about where something is: DNS, ARP, DHCP.
    Lookup,
    /// Setting a connection up.
    Handshake,
    /// Asking for something.
    Request,
    /// The answer.
    Response,
    /// The data itself.
    Data,
    /// Keeping the conversation going: acknowledgements, keep-alives, the end.
    Control,
    /// Keys and proofs.
    Crypto,
    /// Something refused, lost or wrong.
    Refusal,
}

impl Class {
    /// Every class, in the legend's order.
    pub const ALL: [Self; 8] = [
        Self::Lookup,
        Self::Handshake,
        Self::Request,
        Self::Response,
        Self::Data,
        Self::Control,
        Self::Crypto,
        Self::Refusal,
    ];

    /// Its name in the legend.
    #[must_use]
    pub const fn name(self) -> BookKey {
        match self {
            Self::Lookup => key::PACKETS_CLASS_LOOKUP,
            Self::Handshake => key::PACKETS_CLASS_HANDSHAKE,
            Self::Request => key::PACKETS_CLASS_REQUEST,
            Self::Response => key::PACKETS_CLASS_RESPONSE,
            Self::Data => key::PACKETS_CLASS_DATA,
            Self::Control => key::PACKETS_CLASS_CONTROL,
            Self::Crypto => key::PACKETS_CLASS_CRYPTO,
            Self::Refusal => key::PACKETS_CLASS_REFUSAL,
        }
    }

    /// Its place in [`Class::ALL`].
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A packet: who sends it to whom, what it is, and what it carries.
#[derive(Debug, Clone, Copy)]
pub struct Packet {
    /// The device that sends it.
    pub from: usize,
    /// The device it is for.
    pub to: usize,
    /// What it is.
    pub class: Class,
    /// The protocol a capture lists it under: `TCP`.
    pub proto: &'static str,
    /// What its label says as it flies — wire: `SYN`, `GET /`.
    pub tag: &'static str,
    /// The capture's line — wire: `52344 → 80 [SYN] Seq=0`.
    pub info: &'static str,
    /// How big it is, in bytes, on the wire.
    pub bytes: u32,
    /// What it says, in words — for the card of the packet in flight.
    pub says: Option<BookKey>,
    /// Its headers, from the outermost in: what the anatomy opens.
    pub headers: &'static [anatomy::Header],
    /// The source and destination a capture shows, when they are not the
    /// addresses of the devices it flies between — wire: a probe a router
    /// answers, a packet translated on its way, one sent before its sender
    /// has an address.
    pub addresses: Option<(&'static str, &'static str)>,
}

impl Packet {
    /// A packet with nothing said of it and no headers.
    #[must_use]
    pub const fn new(
        from: usize,
        to: usize,
        class: Class,
        proto: &'static str,
        tag: &'static str,
        info: &'static str,
        bytes: u32,
    ) -> Self {
        Self {
            from,
            to,
            class,
            proto,
            tag,
            info,
            bytes,
            says: None,
            headers: &[],
            addresses: None,
        }
    }

    /// And the source and destination a capture shows — wire.
    #[must_use]
    pub const fn between(self, source: &'static str, destination: &'static str) -> Self {
        Self {
            addresses: Some((source, destination)),
            ..self
        }
    }

    /// And what it says, in words.
    #[must_use]
    pub const fn says(self, says: BookKey) -> Self {
        Self {
            says: Some(says),
            ..self
        }
    }

    /// And its headers.
    #[must_use]
    pub const fn headers(self, headers: &'static [anatomy::Header]) -> Self {
        Self { headers, ..self }
    }
}

/// A place on the floor, in half tiles.
#[must_use]
pub const fn at(col: i8, row: i8) -> Spot {
    Spot { col, row }
}

/// A link across which a packet takes `ms` milliseconds.
#[must_use]
pub const fn link(a: usize, b: usize, ms: u16) -> Link {
    Link { a, b, ms }
}

/// A fact in words.
#[must_use]
pub const fn fact(label: BookKey, value: BookKey) -> Fact {
    Fact {
        label,
        value: Value::Said(value),
    }
}

/// A fact the wire says.
#[must_use]
pub const fn wire(label: BookKey, value: &'static str) -> Fact {
    Fact {
        label,
        value: Value::Wire(value),
    }
}

impl Protocol {
    /// Every packet it sends, in order, with the step that sends it.
    pub fn packets(&self) -> impl Iterator<Item = (usize, &Packet)> + '_ {
        self.steps.iter().enumerate().flat_map(|(s, step)| {
            step.beats.iter().filter_map(move |b| match b {
                Beat::Send(p) => Some((s, p)),
                _ => None,
            })
        })
    }

    /// The devices between `from` and `to`, both included — the shortest
    /// way along the scene's links; `None` when no links join them.
    #[must_use]
    pub fn path(&self, from: usize, to: usize) -> Option<Vec<usize>> {
        let n = self.scene.devices.len();
        if from >= n || to >= n {
            return None;
        }
        let mut before = vec![usize::MAX; n];
        let mut queue = std::collections::VecDeque::from([from]);
        before[from] = from;
        while let Some(d) = queue.pop_front() {
            if d == to {
                let mut path = vec![to];
                let mut at = to;
                while at != from {
                    at = before[at];
                    path.push(at);
                }
                path.reverse();
                return Some(path);
            }
            for l in self.scene.links {
                let next = if l.a == d {
                    l.b
                } else if l.b == d {
                    l.a
                } else {
                    continue;
                };
                if before[next] == usize::MAX {
                    before[next] = d;
                    queue.push_back(next);
                }
            }
        }
        None
    }

    /// How long a packet from `from` to `to` takes on the wire, in
    /// milliseconds: the links' times along its way.
    #[must_use]
    pub fn latency(&self, from: usize, to: usize) -> u32 {
        self.path(from, to).map_or(0, |path| {
            path.windows(2)
                .map(|w| {
                    self.scene
                        .links
                        .iter()
                        .find(|l| (l.a == w[0] && l.b == w[1]) || (l.a == w[1] && l.b == w[0]))
                        .map_or(0, |l| u32::from(l.ms))
                })
                .sum()
        })
    }
}

#[cfg(test)]
mod tests;
