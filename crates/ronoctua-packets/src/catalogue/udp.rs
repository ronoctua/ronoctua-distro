//! UDP: datagrams with no connection — a DNS question and its answer in one
//! round trip, a game's stream of updates, one of them lost on the way, and
//! the game deciding it does not matter.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const RESOLVER: usize = 3;
const GAME: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-4, 0),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(1, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Rack,
        at: at(2, -5),
        name: k::PACKETS_DEVICE_RESOLVER,
        address: Address::V4([8, 8, 8, 8]),
    },
    Device {
        kind: Kind::Tower(Emblem::Gear),
        at: at(7, -1),
        name: k::PACKETS_UDP_DEVICE_GAME_SERVER,
        address: Address::V4([203, 0, 113, 30]),
    },
];

/// The IPv4 header of the game's update: 68 bytes, UDP.
const IP_UPDATE: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "68",
    "0x7e12",
    "000",
    "0",
    "64",
    "17",
    "0xfec5",
    "192.168.1.10",
    "203.0.113.30",
];
/// The IPv4 header of the server's state of the world: 148 bytes.
const IP_STATE: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "148",
    "0x0000",
    "000",
    "0",
    "55",
    "17",
    "0x8588",
    "203.0.113.30",
    "192.168.1.10",
];
/// The UDP header of an update: 8 bytes of header and 40 of data.
const UDP_UPDATE: &[&str] = &["50000", "28000", "48", "0x6d1f"];
/// The UDP header of the state: 8 bytes of header and 120 of data.
const UDP_STATE: &[&str] = &["28000", "50000", "128", "0xc2a0"];

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_UDP_1_TITLE,
        sub: k::PACKETS_UDP_1_SUB,
        body: k::PACKETS_UDP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "192.168.1.10:51000"),
            wire(k::PACKETS_LABEL_DESTINATION, "8.8.8.8:53"),
            fact(k::PACKETS_LABEL_HANDSHAKE, k::PACKETS_UDP_1_HANDSHAKE),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, RESOLVER],
        beats: &[Beat::Send(
            Packet::new(
                CLIENT,
                RESOLVER,
                Class::Lookup,
                "UDP",
                "A? www.example.com",
                "51000 → 53 Len=33",
                75,
            )
            .says(k::PACKETS_UDP_P1)
            .headers(&[
                header(
                    &anatomy::IPV4,
                    &[
                        "4",
                        "5",
                        "0",
                        "0",
                        "61",
                        "0x8c11",
                        "000",
                        "0",
                        "64",
                        "17",
                        "0x1cdd",
                        "192.168.1.10",
                        "8.8.8.8",
                    ],
                    &[9],
                ),
                header(
                    &anatomy::UDP,
                    &["51000", "53", "41", "0x9c2b"],
                    &[0, 1, 2, 3],
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_UDP_2_TITLE,
        sub: k::PACKETS_UDP_2_SUB,
        body: k::PACKETS_UDP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "8.8.8.8:53"),
            wire(k::PACKETS_LABEL_DESTINATION, "192.168.1.10:51000"),
            wire(k::PACKETS_LABEL_ROUND_TRIPS, "1"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, RESOLVER],
        beats: &[Beat::Send(
            Packet::new(
                RESOLVER,
                CLIENT,
                Class::Response,
                "UDP",
                "A 203.0.113.10",
                "53 → 51000 Len=49",
                91,
            )
            .says(k::PACKETS_UDP_P2)
            .headers(&[
                header(
                    &anatomy::IPV4,
                    &[
                        "4",
                        "5",
                        "0",
                        "0",
                        "77",
                        "0x0000",
                        "000",
                        "0",
                        "121",
                        "17",
                        "0x6fde",
                        "8.8.8.8",
                        "192.168.1.10",
                    ],
                    &[],
                ),
                header(&anatomy::UDP, &["53", "51000", "57", "0x41ae"], &[0, 1]),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_UDP_3_TITLE,
        sub: k::PACKETS_UDP_3_SUB,
        body: k::PACKETS_UDP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "28000"),
            wire(k::PACKETS_LABEL_INTERVAL, "50 ms"),
            wire(k::PACKETS_LABEL_SIZE, "8 + 40 B"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, GAME],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    GAME,
                    Class::Data,
                    "UDP",
                    "Len=40",
                    "50000 → 28000 Len=40",
                    82,
                )
                .says(k::PACKETS_UDP_P3)
                .headers(&[
                    header(&anatomy::IPV4, IP_UPDATE, &[9]),
                    header(&anatomy::UDP, UDP_UPDATE, &[0, 1, 2]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GAME,
                    CLIENT,
                    Class::Data,
                    "UDP",
                    "Len=120",
                    "28000 → 50000 Len=120",
                    162,
                )
                .says(k::PACKETS_UDP_P4)
                .headers(&[
                    header(&anatomy::IPV4, IP_STATE, &[]),
                    header(&anatomy::UDP, UDP_STATE, &[2]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    GAME,
                    Class::Data,
                    "UDP",
                    "Len=40",
                    "50000 → 28000 Len=40",
                    82,
                )
                .says(k::PACKETS_UDP_P5)
                .headers(&[
                    header(&anatomy::IPV4, IP_UPDATE, &[]),
                    header(&anatomy::UDP, UDP_UPDATE, &[]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_UDP_4_TITLE,
        sub: k::PACKETS_UDP_4_SUB,
        body: k::PACKETS_UDP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "203.0.113.30:28000"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_UDP_4_STATE),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, GAME],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    INTERNET,
                    Class::Refusal,
                    "UDP",
                    "Len=40",
                    "50000 → 28000 Len=40",
                    82,
                )
                .says(k::PACKETS_UDP_P6)
                .headers(&[
                    header(&anatomy::IPV4, IP_UPDATE, &[]),
                    header(&anatomy::UDP, UDP_UPDATE, &[0, 1, 2, 3]),
                ]),
            ),
            Beat::Light {
                at: INTERNET,
                prop: Some(Prop::Cross),
            },
        ],
    },
    Step {
        title: k::PACKETS_UDP_5_TITLE,
        sub: k::PACKETS_UDP_5_SUB,
        body: k::PACKETS_UDP_5_BODY,
        facts: &[fact(k::PACKETS_LABEL_REASON, k::PACKETS_UDP_5_REASON)],
        focus: &[CLIENT, ROUTER, INTERNET, GAME],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    GAME,
                    Class::Data,
                    "UDP",
                    "Len=40",
                    "50000 → 28000 Len=40",
                    82,
                )
                .says(k::PACKETS_UDP_P7)
                .headers(&[
                    header(&anatomy::IPV4, IP_UPDATE, &[]),
                    header(&anatomy::UDP, UDP_UPDATE, &[]),
                ]),
            ),
            Beat::Light {
                at: GAME,
                prop: Some(Prop::Check),
            },
            Beat::Send(
                Packet::new(
                    GAME,
                    CLIENT,
                    Class::Data,
                    "UDP",
                    "Len=120",
                    "28000 → 50000 Len=120",
                    162,
                )
                .says(k::PACKETS_UDP_P8)
                .headers(&[
                    header(&anatomy::IPV4, IP_STATE, &[]),
                    header(&anatomy::UDP, UDP_STATE, &[]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_UDP_6_TITLE,
        sub: k::PACKETS_UDP_6_SUB,
        body: k::PACKETS_UDP_6_BODY,
        facts: &[
            fact(k::PACKETS_UDP_LABEL_PICK_UDP, k::PACKETS_UDP_6_UDP),
            fact(k::PACKETS_UDP_LABEL_PICK_TCP, k::PACKETS_UDP_6_TCP),
            wire(k::PACKETS_FACT_HEADER, "UDP 8 B · TCP 20–60 B"),
        ],
        focus: &[],
        beats: &[Beat::Rest],
    },
];

/// UDP.
pub static UDP: Protocol = Protocol {
    id: "udp",
    group: Group::Transport,
    layer: Layer(4),
    name: k::PACKETS_UDP_NAME,
    title: k::PACKETS_UDP_TITLE,
    tagline: k::PACKETS_UDP_TAGLINE,
    overview: k::PACKETS_UDP_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_HEADER, "8 B"),
        fact(k::PACKETS_FACT_RELIABILITY, k::PACKETS_UDP_RELIABILITY),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_UDP_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 768"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, RESOLVER, 1),
            link(INTERNET, GAME, 6),
        ],
    },
    steps: &STEPS,
};
