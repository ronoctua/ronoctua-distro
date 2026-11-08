//! Tor: a circuit built through a guard, a middle and an exit relay — a key
//! agreed with each — a request wrapped in three layers that the relays peel
//! one by one, the exit connecting to the site from its own address, and the
//! answer wrapped again on the way back.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const GUARD: usize = 1;
const MIDDLE: usize = 2;
const EXIT: usize = 3;
const WEB: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 0, 2, 10]),
    },
    Device {
        kind: Kind::Tower(Emblem::Onion),
        at: at(-4, -3),
        name: k::PACKETS_DEVICE_GUARD,
        address: Address::V4([198, 51, 100, 7]),
    },
    Device {
        kind: Kind::Tower(Emblem::Onion),
        at: at(1, 2),
        name: k::PACKETS_DEVICE_MIDDLE,
        address: Address::V4([203, 0, 113, 42]),
    },
    Device {
        kind: Kind::Tower(Emblem::Onion),
        at: at(5, -4),
        name: k::PACKETS_DEVICE_EXIT,
        address: Address::V4([203, 0, 113, 99]),
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(9, 2),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([198, 51, 100, 80]),
    },
];

/// The laptop's IPv4 header towards the guard — a cell is 514 bytes, in a TLS
/// record of 536.
const IP_TO_GUARD: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "576",
    "0x3a1c",
    "010",
    "0",
    "64",
    "6",
    "0x9e21",
    "192.0.2.10",
    "198.51.100.7",
];
/// Its TCP header, to the guard's ORPort.
const TCP_TO_GUARD: &[&str] = &[
    "50432", "9001", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "501", "0x4c2a",
    "0",
];
/// The guard's IPv4 header towards the middle: the middle sees the guard, never
/// the laptop.
const IP_GUARD_TO_MIDDLE: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "576",
    "0x51e0",
    "010",
    "0",
    "64",
    "6",
    "0x1b9c",
    "198.51.100.7",
    "203.0.113.42",
];
/// The middle's towards the exit.
const IP_MIDDLE_TO_EXIT: &[&str] = &[
    "4",
    "5",
    "0",
    "0",
    "576",
    "0x6a07",
    "010",
    "0",
    "64",
    "6",
    "0x0e53",
    "203.0.113.42",
    "203.0.113.99",
];
/// The TLS record a cell travels in between two relays: application data,
/// unreadable on the wire.
const TLS_RECORD: &[&str] = &["23", "0x0303", "531"];

/// A Tor cell: the circuit ID on this link, the command, the payload.
macro_rules! cell {
    ($circ:literal, $cmd:literal, $payload:literal, $lit:expr) => {
        header(&anatomy::TOR_CELL, &[$circ, $cmd, $payload], $lit)
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_TOR_1_TITLE,
        sub: k::PACKETS_TOR_1_SUB,
        body: k::PACKETS_TOR_1_BODY,
        facts: &[
            wire(
                k::PACKETS_LABEL_PATH,
                "198.51.100.7 → 203.0.113.42 → 203.0.113.99",
            ),
            wire(k::PACKETS_LABEL_DESTINATION, "www.example.com:443"),
        ],
        focus: &[LAPTOP],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "https://www.example.com/",
            },
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Page),
            },
        ],
    },
    Step {
        title: k::PACKETS_TOR_2_TITLE,
        sub: k::PACKETS_TOR_2_SUB,
        body: k::PACKETS_TOR_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "CREATE2 / CREATED2"),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "ntor (Curve25519)"),
            wire(k::PACKETS_LABEL_CIRCUIT, "0x8000a1b2"),
        ],
        focus: &[LAPTOP, GUARD],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GUARD,
                    Class::Crypto,
                    "Tor",
                    "CREATE2",
                    "CREATE2 CircID=0x8000a1b2 HTYPE=ntor",
                    590,
                )
                .says(k::PACKETS_TOR_P1)
                .headers(&[
                    header(&anatomy::IPV4, IP_TO_GUARD, &[12]),
                    header(&anatomy::TCP, TCP_TO_GUARD, &[1]),
                    header(&anatomy::TLS, TLS_RECORD, &[0]),
                    cell!(
                        "0x8000a1b2",
                        "10 (CREATE2)",
                        "HTYPE=2 (ntor) HLEN=84 HDATA=ID|B|X",
                        &[0, 1, 2]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GUARD,
                    LAPTOP,
                    Class::Crypto,
                    "Tor",
                    "CREATED2",
                    "CREATED2 CircID=0x8000a1b2",
                    590,
                )
                .says(k::PACKETS_TOR_P2)
                .headers(&[cell!(
                    "0x8000a1b2",
                    "11 (CREATED2)",
                    "HLEN=64 HDATA=Y|AUTH",
                    &[1, 2]
                )]),
            ),
            Beat::Light {
                at: GUARD,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_TOR_3_TITLE,
        sub: k::PACKETS_TOR_3_SUB,
        body: k::PACKETS_TOR_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "RELAY_EARLY EXTEND2 / EXTENDED2"),
            wire(
                k::PACKETS_LABEL_CIRCUIT,
                "0x8000a1b2, 0x80003c4d, 0x80005e6f",
            ),
        ],
        focus: &[LAPTOP, GUARD, MIDDLE, EXIT],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GUARD,
                    Class::Crypto,
                    "Tor",
                    "EXTEND2",
                    "RELAY_EARLY CircID=0x8000a1b2 [EXTEND2]",
                    590,
                )
                .says(k::PACKETS_TOR_P3)
                .headers(&[
                    header(&anatomy::IPV4, IP_TO_GUARD, &[]),
                    header(&anatomy::TCP, TCP_TO_GUARD, &[]),
                    header(&anatomy::TLS, TLS_RECORD, &[]),
                    cell!(
                        "0x8000a1b2",
                        "9 (RELAY_EARLY)",
                        "K1(EXTEND2 203.0.113.42:9001 HTYPE=2 HDATA=ID|B|X)",
                        &[1, 2]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GUARD,
                    MIDDLE,
                    Class::Crypto,
                    "Tor",
                    "CREATE2",
                    "CREATE2 CircID=0x80003c4d HTYPE=ntor",
                    590,
                )
                .says(k::PACKETS_TOR_P4)
                .headers(&[
                    header(&anatomy::IPV4, IP_GUARD_TO_MIDDLE, &[11, 12]),
                    header(
                        &anatomy::TCP,
                        &[
                            "41890", "9001", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "501", "0x6f3e", "0",
                        ],
                        &[],
                    ),
                    header(&anatomy::TLS, TLS_RECORD, &[]),
                    cell!(
                        "0x80003c4d",
                        "10 (CREATE2)",
                        "HTYPE=2 (ntor) HLEN=84 HDATA=ID|B|X",
                        &[0]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    MIDDLE,
                    GUARD,
                    Class::Crypto,
                    "Tor",
                    "CREATED2",
                    "CREATED2 CircID=0x80003c4d",
                    590,
                )
                .says(k::PACKETS_TOR_P5)
                .headers(&[cell!(
                    "0x80003c4d",
                    "11 (CREATED2)",
                    "HLEN=64 HDATA=Y|AUTH",
                    &[2]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    GUARD,
                    LAPTOP,
                    Class::Crypto,
                    "Tor",
                    "EXTENDED2",
                    "RELAY CircID=0x8000a1b2 [EXTENDED2]",
                    590,
                )
                .says(k::PACKETS_TOR_P6)
                .headers(&[cell!(
                    "0x8000a1b2",
                    "3 (RELAY)",
                    "K1(EXTENDED2 HDATA=Y|AUTH)",
                    &[2]
                )]),
            ),
            Beat::Light {
                at: MIDDLE,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: EXIT,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_TOR_4_TITLE,
        sub: k::PACKETS_TOR_4_SUB,
        body: k::PACKETS_TOR_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "BEGIN www.example.com:443"),
            wire(k::PACKETS_LABEL_ENCRYPTION, "K1(K2(K3(…)))"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Onion),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GUARD,
                    Class::Request,
                    "Tor",
                    "K1(K2(K3(BEGIN)))",
                    "RELAY CircID=0x8000a1b2",
                    590,
                )
                .says(k::PACKETS_TOR_P7)
                .headers(&[
                    header(&anatomy::IPV4, IP_TO_GUARD, &[]),
                    header(&anatomy::TCP, TCP_TO_GUARD, &[]),
                    header(&anatomy::TLS, TLS_RECORD, &[]),
                    cell!(
                        "0x8000a1b2",
                        "3 (RELAY)",
                        "K1(K2(K3(BEGIN www.example.com:443)))",
                        &[2]
                    ),
                ]),
            ),
            Beat::Light {
                at: GUARD,
                prop: Some(Prop::Onion),
            },
            Beat::Send(
                Packet::new(
                    GUARD,
                    MIDDLE,
                    Class::Request,
                    "Tor",
                    "K2(K3(BEGIN))",
                    "RELAY CircID=0x80003c4d",
                    590,
                )
                .says(k::PACKETS_TOR_P8)
                .headers(&[
                    header(&anatomy::IPV4, IP_GUARD_TO_MIDDLE, &[11]),
                    cell!(
                        "0x80003c4d",
                        "3 (RELAY)",
                        "K2(K3(BEGIN www.example.com:443))",
                        &[0, 2]
                    ),
                ]),
            ),
            Beat::Light {
                at: MIDDLE,
                prop: Some(Prop::Onion),
            },
            Beat::Send(
                Packet::new(
                    MIDDLE,
                    EXIT,
                    Class::Request,
                    "Tor",
                    "K3(BEGIN)",
                    "RELAY CircID=0x80005e6f [BEGIN www.example.com:443]",
                    590,
                )
                .says(k::PACKETS_TOR_P9)
                .headers(&[
                    header(&anatomy::IPV4, IP_MIDDLE_TO_EXIT, &[11]),
                    cell!(
                        "0x80005e6f",
                        "3 (RELAY)",
                        "K3(BEGIN www.example.com:443)",
                        &[0, 2]
                    ),
                ]),
            ),
            Beat::Light {
                at: EXIT,
                prop: Some(Prop::Onion),
            },
        ],
    },
    Step {
        title: k::PACKETS_TOR_5_TITLE,
        sub: k::PACKETS_TOR_5_SUB,
        body: k::PACKETS_TOR_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "203.0.113.99"),
            wire(k::PACKETS_LABEL_COMMAND, "CONNECTED"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    EXIT,
                    WEB,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "41022 → 443 [SYN] Seq=0 Win=64240",
                    66,
                )
                .says(k::PACKETS_TOR_P10)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "52",
                            "0x5b2e",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x7c41",
                            "203.0.113.99",
                            "198.51.100.80",
                        ],
                        &[11],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "41022", "443", "0", "0", "8", "0", "0", "0", "0", "0", "0", "0", "1",
                            "0", "64240", "0x2d9a", "0",
                        ],
                        &[1, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    WEB,
                    EXIT,
                    Class::Handshake,
                    "TCP",
                    "SYN-ACK",
                    "443 → 41022 [SYN, ACK] Seq=0 Ack=1",
                    66,
                )
                .says(k::PACKETS_TOR_P11),
            ),
            Beat::Send(
                Packet::new(
                    EXIT,
                    LAPTOP,
                    Class::Response,
                    "Tor",
                    "CONNECTED",
                    "RELAY CircID=0x8000a1b2",
                    590,
                )
                .says(k::PACKETS_TOR_P12)
                .headers(&[cell!(
                    "0x8000a1b2",
                    "3 (RELAY)",
                    "K1(K2(K3(CONNECTED 198.51.100.80 TTL=300)))",
                    &[2]
                )]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Onion),
            },
        ],
    },
    Step {
        title: k::PACKETS_TOR_6_TITLE,
        sub: k::PACKETS_TOR_6_SUB,
        body: k::PACKETS_TOR_6_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_IDENTITY, k::PACKETS_TOR_6_IDENTITY),
            fact(k::PACKETS_LABEL_ADDRESS, k::PACKETS_TOR_6_ADDRESS),
        ],
        focus: &[LAPTOP],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Shield),
            },
            Beat::Rest,
        ],
    },
];

/// Tor.
pub static TOR: Protocol = Protocol {
    id: "tor",
    group: Group::Security,
    layer: Layer(7),
    name: k::PACKETS_TOR_NAME,
    title: k::PACKETS_TOR_TITLE,
    tagline: k::PACKETS_TOR_TAGLINE,
    overview: k::PACKETS_TOR_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_TOR_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP + TLS"),
        fact(k::PACKETS_FACT_ENCRYPTION, k::PACKETS_TOR_ENCRYPTION),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_TOR_SECURITY),
        wire(k::PACKETS_FACT_DEFINED, "tor-spec (spec.torproject.org)"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, GUARD, 12),
            link(GUARD, MIDDLE, 15),
            link(MIDDLE, EXIT, 14),
            link(EXIT, WEB, 8),
        ],
    },
    steps: &STEPS,
};
