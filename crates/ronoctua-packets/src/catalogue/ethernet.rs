//! ETHERNET: frames through a switch on a local network — the switch
//! learning where each address is from the frames it reads, flooding a frame
//! whose destination it does not know yet, forwarding one whose destination
//! it does, and flooding a broadcast always.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const SWITCH: usize = 1;
const DESKTOP: usize = 2;
const PRINTER: usize = 3;
const ROUTER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 1),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x0a]),
    },
    Device {
        kind: Kind::Switch,
        at: at(-1, 0),
        name: k::PACKETS_DEVICE_SWITCH,
        address: Address::None,
    },
    Device {
        kind: Kind::Desktop,
        at: at(2, -6),
        name: k::PACKETS_DEVICE_DESKTOP,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x0b]),
    },
    Device {
        kind: Kind::Printer,
        at: at(8, 0),
        name: k::PACKETS_DEVICE_PRINTER,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x14]),
    },
    Device {
        kind: Kind::Router,
        at: at(3, 6),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x01]),
    },
];

/// The laptop's frame for the printer — the same on every port the switch
/// sends it out of: a switch copies a frame, it never rewrites it.
const ETH_TO_PRINTER: &[&str] = &["00:00:5e:00:53:14", "00:00:5e:00:53:0a", "0x0800"];
/// The printer's, back.
const ETH_TO_LAPTOP: &[&str] = &["00:00:5e:00:53:0a", "00:00:5e:00:53:14", "0x0800"];
/// The laptop's broadcast: an ARP question.
const ETH_BROADCAST: &[&str] = &["ff:ff:ff:ff:ff:ff", "00:00:5e:00:53:0a", "0x0806"];

/// The packet inside the first frame: the laptop's first to the printer.
const IP_FIRST: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "52",
    "0x4a7d",
    "010",
    "0",
    "64",
    "6",
    "0x6cd8",
    "192.168.1.10",
    "192.168.1.20",
];
/// The printer's answer.
const IP_ANSWER: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "52",
    "0x0000",
    "010",
    "0",
    "64",
    "6",
    "0xb755",
    "192.168.1.20",
    "192.168.1.10",
];
/// A full packet of the document.
const IP_DOCUMENT: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "1500",
    "0x4a7e",
    "010",
    "0",
    "64",
    "6",
    "0x672f",
    "192.168.1.10",
    "192.168.1.20",
];
/// The ARP question the broadcast carries.
const ARP_WHO_HAS: &[&str] = &[
    "1",
    "0x0800",
    "6",
    "4",
    "1",
    "00:00:5e:00:53:0a",
    "192.168.1.10",
    "00:00:00:00:00:00",
    "192.168.1.1",
];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_ETHERNET_1_TITLE,
        sub: k::PACKETS_ETHERNET_1_SUB,
        body: k::PACKETS_ETHERNET_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "00:00:5e:00:53:14"),
            wire(k::PACKETS_LABEL_SOURCE, "00:00:5e:00:53:0a"),
            wire(k::PACKETS_LABEL_PROTOCOL, "0x0800 IPv4"),
        ],
        focus: &[LAPTOP],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Envelope),
            },
            Beat::Rest,
        ],
    },
    Step {
        title: k::PACKETS_ETHERNET_2_TITLE,
        sub: k::PACKETS_ETHERNET_2_SUB,
        body: k::PACKETS_ETHERNET_2_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_TABLE, k::PACKETS_ETHERNET_2_TABLE),
            fact(
                k::PACKETS_LABEL_DESTINATION,
                k::PACKETS_ETHERNET_2_DESTINATION,
            ),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SWITCH,
                    Class::Request,
                    "Ethernet",
                    "→ 00:00:5e:00:53:14",
                    "00:00:5e:00:53:0a → 00:00:5e:00:53:14 IPv4",
                    66,
                )
                .says(k::PACKETS_ETHERNET_P1)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_TO_PRINTER, &[0, 1]),
                    header(&anatomy::IPV4, IP_FIRST, &[]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    DESKTOP,
                    Class::Request,
                    "Ethernet",
                    "→ 00:00:5e:00:53:14",
                    "00:00:5e:00:53:0a → 00:00:5e:00:53:14 IPv4",
                    66,
                )
                .says(k::PACKETS_ETHERNET_P2)
                .headers(&[header(&anatomy::ETHERNET, ETH_TO_PRINTER, &[0])]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    PRINTER,
                    Class::Request,
                    "Ethernet",
                    "→ 00:00:5e:00:53:14",
                    "00:00:5e:00:53:0a → 00:00:5e:00:53:14 IPv4",
                    66,
                )
                .says(k::PACKETS_ETHERNET_P3)
                .headers(&[header(&anatomy::ETHERNET, ETH_TO_PRINTER, &[0])]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    ROUTER,
                    Class::Request,
                    "Ethernet",
                    "→ 00:00:5e:00:53:14",
                    "00:00:5e:00:53:0a → 00:00:5e:00:53:14 IPv4",
                    66,
                )
                .says(k::PACKETS_ETHERNET_P4)
                .headers(&[header(&anatomy::ETHERNET, ETH_TO_PRINTER, &[0])]),
            ),
            Beat::Light {
                at: DESKTOP,
                prop: Some(Prop::Cross),
            },
            Beat::Light {
                at: ROUTER,
                prop: Some(Prop::Cross),
            },
            Beat::Light {
                at: PRINTER,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_ETHERNET_3_TITLE,
        sub: k::PACKETS_ETHERNET_3_SUB,
        body: k::PACKETS_ETHERNET_3_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_TABLE, k::PACKETS_ETHERNET_3_TABLE),
            wire(k::PACKETS_LABEL_DESTINATION, "00:00:5e:00:53:0a"),
        ],
        focus: &[LAPTOP, SWITCH, PRINTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    PRINTER,
                    LAPTOP,
                    Class::Response,
                    "Ethernet",
                    "→ 00:00:5e:00:53:0a",
                    "00:00:5e:00:53:14 → 00:00:5e:00:53:0a IPv4",
                    66,
                )
                .says(k::PACKETS_ETHERNET_P5)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_TO_LAPTOP, &[0, 1]),
                    header(&anatomy::IPV4, IP_ANSWER, &[]),
                ]),
            ),
            Beat::Light {
                at: SWITCH,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_ETHERNET_4_TITLE,
        sub: k::PACKETS_ETHERNET_4_SUB,
        body: k::PACKETS_ETHERNET_4_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_PORT, k::PACKETS_ETHERNET_4_PORT),
            wire(k::PACKETS_LABEL_SIZE, "1514 B"),
        ],
        focus: &[LAPTOP, SWITCH, PRINTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    PRINTER,
                    Class::Data,
                    "Ethernet",
                    "→ 00:00:5e:00:53:14",
                    "00:00:5e:00:53:0a → 00:00:5e:00:53:14 IPv4",
                    1_514,
                )
                .says(k::PACKETS_ETHERNET_P6)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_TO_PRINTER, &[0]),
                    header(&anatomy::IPV4, IP_DOCUMENT, &[4]),
                ]),
            ),
            Beat::Light {
                at: PRINTER,
                prop: Some(Prop::Page),
            },
        ],
    },
    Step {
        title: k::PACKETS_ETHERNET_5_TITLE,
        sub: k::PACKETS_ETHERNET_5_SUB,
        body: k::PACKETS_ETHERNET_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "ff:ff:ff:ff:ff:ff"),
            wire(k::PACKETS_LABEL_PROTOCOL, "0x0806 ARP"),
            fact(k::PACKETS_LABEL_SCOPE, k::PACKETS_ETHERNET_5_SCOPE),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SWITCH,
                    Class::Lookup,
                    "Ethernet",
                    "→ ff:ff:ff:ff:ff:ff",
                    "00:00:5e:00:53:0a → ff:ff:ff:ff:ff:ff ARP",
                    42,
                )
                .says(k::PACKETS_ETHERNET_P7)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_BROADCAST, &[0, 2]),
                    header(&anatomy::ARP, ARP_WHO_HAS, &[]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    DESKTOP,
                    Class::Lookup,
                    "Ethernet",
                    "→ ff:ff:ff:ff:ff:ff",
                    "00:00:5e:00:53:0a → ff:ff:ff:ff:ff:ff ARP",
                    42,
                )
                .headers(&[header(&anatomy::ETHERNET, ETH_BROADCAST, &[0])]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    PRINTER,
                    Class::Lookup,
                    "Ethernet",
                    "→ ff:ff:ff:ff:ff:ff",
                    "00:00:5e:00:53:0a → ff:ff:ff:ff:ff:ff ARP",
                    42,
                )
                .headers(&[header(&anatomy::ETHERNET, ETH_BROADCAST, &[0])]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    ROUTER,
                    Class::Lookup,
                    "Ethernet",
                    "→ ff:ff:ff:ff:ff:ff",
                    "00:00:5e:00:53:0a → ff:ff:ff:ff:ff:ff ARP",
                    42,
                )
                .headers(&[header(&anatomy::ETHERNET, ETH_BROADCAST, &[0])]),
            ),
        ],
    },
];

/// Ethernet.
pub static ETHERNET: Protocol = Protocol {
    id: "ethernet",
    group: Group::Link,
    layer: Layer(2),
    name: k::PACKETS_ETHERNET_NAME,
    title: k::PACKETS_ETHERNET_TITLE,
    tagline: k::PACKETS_ETHERNET_TAGLINE,
    overview: k::PACKETS_ETHERNET_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_ADDRESSING, k::PACKETS_ETHERNET_ADDRESSING),
        wire(k::PACKETS_FACT_HEADER, "14 B + 4 B FCS"),
        wire(k::PACKETS_LABEL_MTU, "1500 B"),
        wire(k::PACKETS_FACT_DEFINED, "IEEE 802.3, IEEE 802.1Q"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, SWITCH, 1),
            link(SWITCH, DESKTOP, 1),
            link(SWITCH, PRINTER, 1),
            link(SWITCH, ROUTER, 1),
        ],
    },
    steps: &STEPS,
};
