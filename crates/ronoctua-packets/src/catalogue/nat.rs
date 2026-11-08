//! NAT: two devices of a home behind the router's one public address — a
//! connection's source rewritten on the way out and its replies rewritten on
//! the way in, a second device given another port, and a packet nobody asked
//! for dropped at the door.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const PHONE: usize = 1;
const ROUTER: usize = 2;
const INTERNET: usize = 3;
const SERVER: usize = 4;
const ATTACKER: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, -2),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Phone,
        at: at(-8, 4),
        name: k::PACKETS_DEVICE_PHONE,
        address: Address::V4([192, 168, 1, 11]),
    },
    Device {
        kind: Kind::Router,
        at: at(-3, 1),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([203, 0, 113, 7]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(2, 1),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(8, -3),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([198, 51, 100, 20]),
    },
    Device {
        kind: Kind::Desktop,
        at: at(7, 5),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::V4([192, 0, 2, 66]),
    },
];

/// A TCP header opening a connection: ports, numbers, offset, the eight
/// flags (CWR ECE URG ACK PSH RST SYN FIN), window and checksum.
macro_rules! tcp {
    ($src:literal, $dst:literal, $ack:literal, $off:literal,
     [$cwr:literal, $ece:literal, $urg:literal, $ackf:literal, $psh:literal, $rst:literal, $syn:literal, $fin:literal],
     $win:literal, $sum:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, "0", $ack, $off, "0", $cwr, $ece, $urg, $ackf, $psh, $rst, $syn, $fin,
                $win, $sum, "0",
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_NAT_1_TITLE,
        sub: k::PACKETS_NAT_1_SUB,
        body: k::PACKETS_NAT_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PRIVATE, "192.168.1.10"),
            wire(k::PACKETS_LABEL_GATEWAY, "192.168.1.1"),
            wire(k::PACKETS_LABEL_DESTINATION, "198.51.100.20:443"),
        ],
        focus: &[LAPTOP, PHONE, ROUTER],
        beats: &[Beat::Send(
            Packet::new(
                LAPTOP,
                ROUTER,
                Class::Handshake,
                "TCP",
                "SYN",
                "192.168.1.10:52344 → 198.51.100.20:443 [SYN]",
                66,
            )
            .says(k::PACKETS_NAT_P1)
            .headers(&[
                header(
                    &anatomy::IPV4,
                    &[
                        "4",
                        "5",
                        "0",
                        "0",
                        "52",
                        "0x5b21",
                        "010",
                        "0",
                        "64",
                        "6",
                        "0xf3a8",
                        "192.168.1.10",
                        "198.51.100.20",
                    ],
                    &[11, 12],
                ),
                tcp!(
                    "52344",
                    "443",
                    "0",
                    "8",
                    ["0", "0", "0", "0", "0", "0", "1", "0"],
                    "64240",
                    "0x7c3a",
                    &[0]
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_NAT_2_TITLE,
        sub: k::PACKETS_NAT_2_SUB,
        body: k::PACKETS_NAT_2_BODY,
        facts: &[
            wire(
                k::PACKETS_LABEL_TRANSLATION,
                "192.168.1.10:52344 → 203.0.113.7:40001",
            ),
            wire(k::PACKETS_LABEL_PUBLIC, "203.0.113.7"),
        ],
        focus: &[ROUTER, INTERNET, SERVER],
        beats: &[
            Beat::Light {
                at: ROUTER,
                prop: None,
            },
            Beat::Send(
                Packet::new(
                    ROUTER,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "203.0.113.7:40001 → 198.51.100.20:443 [SYN]",
                    66,
                )
                .says(k::PACKETS_NAT_P2)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "52",
                            "0x5b21",
                            "010",
                            "0",
                            "63",
                            "6",
                            "0x7a53",
                            "203.0.113.7",
                            "198.51.100.20",
                        ],
                        &[10, 11],
                    ),
                    tcp!(
                        "40001",
                        "443",
                        "0",
                        "8",
                        ["0", "0", "0", "0", "0", "0", "1", "0"],
                        "64240",
                        "0xa95e",
                        &[0, 15]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_NAT_3_TITLE,
        sub: k::PACKETS_NAT_3_SUB,
        body: k::PACKETS_NAT_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "203.0.113.7:40001"),
            wire(k::PACKETS_LABEL_TABLE, "40001 → 192.168.1.10:52344"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    ROUTER,
                    Class::Handshake,
                    "TCP",
                    "SYN-ACK",
                    "198.51.100.20:443 → 203.0.113.7:40001 [SYN, ACK]",
                    66,
                )
                .says(k::PACKETS_NAT_P3)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "52",
                            "0x0000",
                            "010",
                            "0",
                            "57",
                            "6",
                            "0xdb74",
                            "198.51.100.20",
                            "203.0.113.7",
                        ],
                        &[12],
                    ),
                    tcp!(
                        "443",
                        "40001",
                        "1",
                        "8",
                        ["0", "0", "0", "1", "0", "0", "1", "0"],
                        "65160",
                        "0x3d84",
                        &[1]
                    ),
                ]),
            ),
            Beat::Light {
                at: ROUTER,
                prop: None,
            },
            Beat::Send(
                Packet::new(
                    ROUTER,
                    LAPTOP,
                    Class::Handshake,
                    "TCP",
                    "SYN-ACK",
                    "198.51.100.20:443 → 192.168.1.10:52344 [SYN, ACK]",
                    66,
                )
                .says(k::PACKETS_NAT_P4)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "52",
                            "0x0000",
                            "010",
                            "0",
                            "56",
                            "6",
                            "0x56ca",
                            "198.51.100.20",
                            "192.168.1.10",
                        ],
                        &[10, 12],
                    ),
                    tcp!(
                        "443",
                        "52344",
                        "1",
                        "8",
                        ["0", "0", "0", "1", "0", "0", "1", "0"],
                        "65160",
                        "0x1060",
                        &[1, 15]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_NAT_4_TITLE,
        sub: k::PACKETS_NAT_4_SUB,
        body: k::PACKETS_NAT_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PRIVATE, "192.168.1.11:52344"),
            wire(
                k::PACKETS_LABEL_TABLE,
                "40001 → 192.168.1.10:52344 · 40002 → 192.168.1.11:52344",
            ),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    ROUTER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "192.168.1.11:52344 → 198.51.100.20:443 [SYN]",
                    66,
                )
                .says(k::PACKETS_NAT_P5)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "52",
                            "0x2e90",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x2039",
                            "192.168.1.11",
                            "198.51.100.20",
                        ],
                        &[11],
                    ),
                    tcp!(
                        "52344",
                        "443",
                        "0",
                        "8",
                        ["0", "0", "0", "0", "0", "0", "1", "0"],
                        "64240",
                        "0x5d17",
                        &[0]
                    ),
                ]),
            ),
            Beat::Light {
                at: ROUTER,
                prop: None,
            },
            Beat::Send(
                Packet::new(
                    ROUTER,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "203.0.113.7:40002 → 198.51.100.20:443 [SYN]",
                    66,
                )
                .says(k::PACKETS_NAT_P6)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "52",
                            "0x2e90",
                            "010",
                            "0",
                            "63",
                            "6",
                            "0xa6e4",
                            "203.0.113.7",
                            "198.51.100.20",
                        ],
                        &[10, 11],
                    ),
                    tcp!(
                        "40002",
                        "443",
                        "0",
                        "8",
                        ["0", "0", "0", "0", "0", "0", "1", "0"],
                        "64240",
                        "0x8a3a",
                        &[0, 15]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_NAT_5_TITLE,
        sub: k::PACKETS_NAT_5_SUB,
        body: k::PACKETS_NAT_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "203.0.113.7:22"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_NAT_5_VERDICT),
        ],
        focus: &[ROUTER, INTERNET, ATTACKER],
        beats: &[
            Beat::Send(
                Packet::new(
                    ATTACKER,
                    ROUTER,
                    Class::Refusal,
                    "TCP",
                    "SYN",
                    "192.0.2.66:61000 → 203.0.113.7:22 [SYN]",
                    58,
                )
                .says(k::PACKETS_NAT_P7)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "44",
                            "0x9a3f",
                            "000",
                            "0",
                            "49",
                            "6",
                            "0xf142",
                            "192.0.2.66",
                            "203.0.113.7",
                        ],
                        &[12],
                    ),
                    tcp!(
                        "61000",
                        "22",
                        "0",
                        "6",
                        ["0", "0", "0", "0", "0", "0", "1", "0"],
                        "1024",
                        "0xe3b0",
                        &[1, 12]
                    ),
                ]),
            ),
            Beat::Light {
                at: ROUTER,
                prop: Some(Prop::Cross),
            },
        ],
    },
];

/// NAT.
pub static NAT: Protocol = Protocol {
    id: "nat",
    group: Group::Link,
    layer: Layer(3),
    name: k::PACKETS_NAT_NAME,
    title: k::PACKETS_NAT_TITLE,
    tagline: k::PACKETS_NAT_TAGLINE,
    overview: k::PACKETS_NAT_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_ADDRESSING, k::PACKETS_NAT_ADDRESSING),
        wire(
            k::PACKETS_NAT_LABEL_RANGES,
            "10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16",
        ),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_NAT_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 3022, RFC 1918, RFC 4787"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(PHONE, ROUTER, 2),
            link(ROUTER, INTERNET, 5),
            link(INTERNET, SERVER, 6),
            link(INTERNET, ATTACKER, 9),
        ],
    },
    steps: &STEPS,
};
