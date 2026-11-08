//! TCP: a connection opened with the three-way handshake, bytes sent and
//! acknowledged, and the connection closed from both sides.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const SERVER: usize = 3;

static DEVICES: [Device; 4] = [
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
        kind: Kind::Tower(Emblem::Gear),
        at: at(6, -2),
        name: k::PACKETS_DEVICE_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// An IPv4 header of the client's, `$len` bytes in all.
macro_rules! ip_out {
    ($len:literal, $checksum:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4",
                "5",
                "0",
                "0",
                $len,
                "0x1c46",
                "010",
                "0",
                "64",
                "6",
                $checksum,
                "192.168.1.10",
                "203.0.113.10",
            ],
            $lit,
        )
    };
}
/// The server's, `$len` bytes in all.
macro_rules! ip_back {
    ($len:literal, $checksum:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4",
                "5",
                "0",
                "0",
                $len,
                "0x0000",
                "010",
                "0",
                "56",
                "6",
                $checksum,
                "203.0.113.10",
                "192.168.1.10",
            ],
            $lit,
        )
    };
}

/// A TCP header: ports, numbers, offset, the eight flags (CWR ECE URG ACK PSH
/// RST SYN FIN), window.
macro_rules! tcp {
    ($src:literal, $dst:literal, $seq:literal, $ack:literal, $off:literal,
     [$cwr:literal, $ece:literal, $urg:literal, $ackf:literal, $psh:literal, $rst:literal, $syn:literal, $fin:literal],
     $win:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, $seq, $ack, $off, "0", $cwr, $ece, $urg, $ackf, $psh, $rst, $syn, $fin,
                $win, "0x4c2a", "0",
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_TCP_1_TITLE,
        sub: k::PACKETS_TCP_1_SUB,
        body: k::PACKETS_TCP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_FLAGS, "SYN"),
            wire(k::PACKETS_LABEL_SEQ, "0"),
            wire(k::PACKETS_LABEL_WINDOW, "64240"),
        ],
        focus: &[],
        beats: &[Beat::Send(
            Packet::new(
                CLIENT,
                SERVER,
                Class::Handshake,
                "TCP",
                "SYN",
                "52344 → 80 [SYN] Seq=0 Win=64240 MSS=1460",
                66,
            )
            .says(k::PACKETS_TCP_P1)
            .headers(&[
                ip_out!("52", "0x20c1", &[9]),
                tcp!(
                    "52344",
                    "80",
                    "0",
                    "0",
                    "8",
                    ["0", "0", "0", "0", "0", "0", "1", "0"],
                    "64240",
                    &[0, 1, 2, 12]
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_TCP_2_TITLE,
        sub: k::PACKETS_TCP_2_SUB,
        body: k::PACKETS_TCP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_FLAGS, "SYN, ACK"),
            wire(k::PACKETS_LABEL_SEQ, "0"),
            wire(k::PACKETS_LABEL_ACK, "1"),
        ],
        focus: &[],
        beats: &[Beat::Send(
            Packet::new(
                SERVER,
                CLIENT,
                Class::Handshake,
                "TCP",
                "SYN-ACK",
                "80 → 52344 [SYN, ACK] Seq=0 Ack=1 Win=65160",
                66,
            )
            .says(k::PACKETS_TCP_P2)
            .headers(&[
                ip_back!("52", "0x4507", &[]),
                tcp!(
                    "80",
                    "52344",
                    "0",
                    "1",
                    "8",
                    ["0", "0", "0", "1", "0", "0", "1", "0"],
                    "65160",
                    &[2, 3, 9, 12]
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_TCP_3_TITLE,
        sub: k::PACKETS_TCP_3_SUB,
        body: k::PACKETS_TCP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_FLAGS, "ACK"),
            wire(k::PACKETS_LABEL_STATE, "ESTABLISHED"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "ACK",
                    "52344 → 80 [ACK] Seq=1 Ack=1",
                    54,
                )
                .says(k::PACKETS_TCP_P3)
                .headers(&[
                    ip_out!("40", "0x20cd", &[]),
                    tcp!(
                        "52344",
                        "80",
                        "1",
                        "1",
                        "5",
                        ["0", "0", "0", "1", "0", "0", "0", "0"],
                        "502",
                        &[3, 9]
                    ),
                ]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Check),
            },
            Beat::Light {
                at: SERVER,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_TCP_4_TITLE,
        sub: k::PACKETS_TCP_4_SUB,
        body: k::PACKETS_TCP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SENT, "517 → Ack=518"),
            wire(k::PACKETS_LABEL_RECEIVED, "1448 → Ack=1449"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Data,
                    "TCP",
                    "Len=517",
                    "52344 → 80 [PSH, ACK] Seq=1 Ack=1 Len=517",
                    571,
                )
                .says(k::PACKETS_TCP_P4)
                .headers(&[
                    ip_out!("557", "0x1ec8", &[]),
                    tcp!(
                        "52344",
                        "80",
                        "1",
                        "1",
                        "5",
                        ["0", "0", "0", "1", "1", "0", "0", "0"],
                        "502",
                        &[2, 10]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Control,
                    "TCP",
                    "ACK 518",
                    "80 → 52344 [ACK] Seq=1 Ack=518",
                    54,
                )
                .says(k::PACKETS_TCP_P5)
                .headers(&[
                    ip_back!("40", "0x4513", &[]),
                    tcp!(
                        "80",
                        "52344",
                        "1",
                        "518",
                        "5",
                        ["0", "0", "0", "1", "0", "0", "0", "0"],
                        "509",
                        &[3, 9]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Data,
                    "TCP",
                    "Len=1448",
                    "80 → 52344 [PSH, ACK] Seq=1 Ack=518 Len=1448",
                    1_502,
                )
                .says(k::PACKETS_TCP_P6)
                .headers(&[
                    ip_back!("1488", "0x3f6b", &[]),
                    tcp!(
                        "80",
                        "52344",
                        "1",
                        "518",
                        "5",
                        ["0", "0", "0", "1", "1", "0", "0", "0"],
                        "509",
                        &[2, 10]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Control,
                    "TCP",
                    "ACK 1449",
                    "52344 → 80 [ACK] Seq=518 Ack=1449",
                    54,
                )
                .says(k::PACKETS_TCP_P7)
                .headers(&[
                    ip_out!("40", "0x20cd", &[]),
                    tcp!(
                        "52344",
                        "80",
                        "518",
                        "1449",
                        "5",
                        ["0", "0", "0", "1", "0", "0", "0", "0"],
                        "501",
                        &[3]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_TCP_5_TITLE,
        sub: k::PACKETS_TCP_5_SUB,
        body: k::PACKETS_TCP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_FLAGS, "FIN, ACK"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_TCP_5_STATE),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Control,
                    "TCP",
                    "FIN",
                    "52344 → 80 [FIN, ACK] Seq=518 Ack=1449",
                    54,
                )
                .says(k::PACKETS_TCP_P8)
                .headers(&[
                    ip_out!("40", "0x20cd", &[]),
                    tcp!(
                        "52344",
                        "80",
                        "518",
                        "1449",
                        "5",
                        ["0", "0", "0", "1", "0", "0", "0", "1"],
                        "501",
                        &[13]
                    ),
                ]),
            ),
            Beat::Send(Packet::new(
                SERVER,
                CLIENT,
                Class::Control,
                "TCP",
                "ACK 519",
                "80 → 52344 [ACK] Seq=1449 Ack=519",
                54,
            )),
            Beat::Send(Packet::new(
                SERVER,
                CLIENT,
                Class::Control,
                "TCP",
                "FIN",
                "80 → 52344 [FIN, ACK] Seq=1449 Ack=519",
                54,
            )),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Control,
                    "TCP",
                    "ACK 1450",
                    "52344 → 80 [ACK] Seq=519 Ack=1450",
                    54,
                )
                .says(k::PACKETS_TCP_P9),
            ),
        ],
    },
];

/// TCP.
pub static TCP: Protocol = Protocol {
    id: "tcp",
    group: Group::Transport,
    layer: Layer(4),
    name: k::PACKETS_TCP_NAME,
    title: k::PACKETS_TCP_TITLE,
    tagline: k::PACKETS_TCP_TAGLINE,
    overview: k::PACKETS_TCP_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_HEADER, "20–60 B"),
        wire(k::PACKETS_FACT_FLAGS, "SYN, ACK, PSH, FIN, RST, URG"),
        fact(k::PACKETS_FACT_RELIABILITY, k::PACKETS_TCP_RELIABILITY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 9293"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
