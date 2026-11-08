//! IP (IPv4): a packet routed hop by hop from a laptop to a server — each
//! router reading the destination alone, choosing the longest prefix that
//! holds it and lowering the TTL — and the answer, routed on its own.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const ROUTER: usize = 1;
const PROVIDER: usize = 2;
const TRANSIT: usize = 3;
const SERVER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-5, -1),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(0, 2),
        name: k::PACKETS_DEVICE_ISP_ROUTER,
        address: Address::V4([198, 51, 100, 1]),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(4, -3),
        name: k::PACKETS_DEVICE_TRANSIT_ROUTER,
        address: Address::V4([192, 0, 2, 1]),
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(9, 1),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// The same packet's IPv4 header on each hop: the TTL one lower, so the
/// checksum recomputed, and — past the home router — the public source
/// address NAT put in. The checksums are the real ones.
const HOP_1: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "60",
    "0x6b2e",
    "010",
    "0",
    "64",
    "6",
    "0xd1d0",
    "192.168.1.10",
    "203.0.113.10",
];
const HOP_2: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "60",
    "0x6b2e",
    "010",
    "0",
    "63",
    "6",
    "0x6a38",
    "198.51.100.23",
    "203.0.113.10",
];
const HOP_3: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "60",
    "0x6b2e",
    "010",
    "0",
    "62",
    "6",
    "0x6b38",
    "198.51.100.23",
    "203.0.113.10",
];
const HOP_4: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "60",
    "0x6b2e",
    "010",
    "0",
    "61",
    "6",
    "0x6c38",
    "198.51.100.23",
    "203.0.113.10",
];
/// The server's answer, as it leaves.
const ANSWER: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "60",
    "0x0000",
    "010",
    "0",
    "64",
    "6",
    "0xd466",
    "203.0.113.10",
    "198.51.100.23",
];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_IP_1_TITLE,
        sub: k::PACKETS_IP_1_SUB,
        body: k::PACKETS_IP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PREFIX, "192.168.1.0/24"),
            wire(k::PACKETS_LABEL_GATEWAY, "192.168.1.1"),
            wire(k::PACKETS_LABEL_TTL_HOPS, "64"),
        ],
        focus: &[LAPTOP, ROUTER],
        beats: &[Beat::Send(
            Packet::new(
                LAPTOP,
                ROUTER,
                Class::Data,
                "IPv4",
                "TTL=64",
                "192.168.1.10 → 203.0.113.10 TTL=64 TCP",
                74,
            )
            .says(k::PACKETS_IP_P1)
            .headers(&[header(&anatomy::IPV4, HOP_1, &[8, 9, 11, 12])]),
        )],
    },
    Step {
        title: k::PACKETS_IP_2_TITLE,
        sub: k::PACKETS_IP_2_SUB,
        body: k::PACKETS_IP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ROUTE, "0.0.0.0/0 via 198.51.100.1"),
            wire(k::PACKETS_LABEL_TRANSLATION, "192.168.1.10 → 198.51.100.23"),
            wire(k::PACKETS_LABEL_TTL_HOPS, "63"),
        ],
        focus: &[ROUTER, PROVIDER],
        beats: &[Beat::Send(
            Packet::new(
                ROUTER,
                PROVIDER,
                Class::Data,
                "IPv4",
                "TTL=63",
                "198.51.100.23 → 203.0.113.10 TTL=63 TCP",
                74,
            )
            .says(k::PACKETS_IP_P2)
            .headers(&[header(&anatomy::IPV4, HOP_2, &[8, 10, 11])]),
        )],
    },
    Step {
        title: k::PACKETS_IP_3_TITLE,
        sub: k::PACKETS_IP_3_SUB,
        body: k::PACKETS_IP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PREFIX, "203.0.113.0/24"),
            wire(k::PACKETS_LABEL_NEXT_HOP, "192.0.2.1"),
            wire(k::PACKETS_LABEL_TTL_HOPS, "62"),
        ],
        focus: &[PROVIDER, TRANSIT],
        beats: &[Beat::Send(
            Packet::new(
                PROVIDER,
                TRANSIT,
                Class::Data,
                "IPv4",
                "TTL=62",
                "198.51.100.23 → 203.0.113.10 TTL=62 TCP",
                74,
            )
            .says(k::PACKETS_IP_P3)
            .headers(&[header(&anatomy::IPV4, HOP_3, &[8, 12])]),
        )],
    },
    Step {
        title: k::PACKETS_IP_4_TITLE,
        sub: k::PACKETS_IP_4_SUB,
        body: k::PACKETS_IP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_NEXT_HOP, "203.0.113.10"),
            wire(k::PACKETS_LABEL_PROTOCOL, "6 TCP"),
            wire(k::PACKETS_LABEL_TTL_HOPS, "61"),
        ],
        focus: &[TRANSIT, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    TRANSIT,
                    SERVER,
                    Class::Data,
                    "IPv4",
                    "TTL=61",
                    "198.51.100.23 → 203.0.113.10 TTL=61 TCP",
                    74,
                )
                .says(k::PACKETS_IP_P4)
                .headers(&[header(&anatomy::IPV4, HOP_4, &[6, 8, 9])]),
            ),
            Beat::Light {
                at: SERVER,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_IP_5_TITLE,
        sub: k::PACKETS_IP_5_SUB,
        body: k::PACKETS_IP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "203.0.113.10"),
            wire(k::PACKETS_LABEL_DESTINATION, "198.51.100.23"),
            wire(k::PACKETS_LABEL_TTL_HOPS, "64"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Response,
                    "IPv4",
                    "TTL=64",
                    "203.0.113.10 → 198.51.100.23 TTL=64 TCP",
                    74,
                )
                .says(k::PACKETS_IP_P5)
                .headers(&[header(&anatomy::IPV4, ANSWER, &[8, 11, 12])]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
];

/// IP, version 4.
pub static IP: Protocol = Protocol {
    id: "ip",
    group: Group::Link,
    layer: Layer(3),
    name: k::PACKETS_IP_NAME,
    title: k::PACKETS_IP_TITLE,
    tagline: k::PACKETS_IP_TAGLINE,
    overview: k::PACKETS_IP_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_ADDRESSING, k::PACKETS_IP_ADDRESSING),
        wire(k::PACKETS_FACT_HEADER, "20–60 B"),
        fact(k::PACKETS_FACT_RELIABILITY, k::PACKETS_IP_RELIABILITY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 791"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(ROUTER, PROVIDER, 6),
            link(PROVIDER, TRANSIT, 8),
            link(TRANSIT, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
