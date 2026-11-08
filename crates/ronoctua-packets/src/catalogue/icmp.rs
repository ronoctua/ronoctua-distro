//! ICMP: IP's own messages — a ping echoed back, a path traced hop by hop
//! with growing TTLs, and an address nobody answers at.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const ISP: usize = 2;
const HOSTING: usize = 3;
const SERVER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_CLIENT,
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
        at: at(-1, 2),
        name: k::PACKETS_DEVICE_ISP_ROUTER,
        address: Address::V4([198, 51, 100, 1]),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(3, -2),
        name: k::PACKETS_DEVICE_HOSTING_ROUTER,
        address: Address::V4([192, 0, 2, 1]),
    },
    Device {
        kind: Kind::Tower(Emblem::Gear),
        at: at(8, 1),
        name: k::PACKETS_DEVICE_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// What ping's echo carries: the moment it left (a `timeval`), then the
/// bytes 0x10 to 0x37 — 56 bytes in all.
const PING_DATA: &str = "6c 2e 0f 69 00 00 00 00 a4 9d 03 00 00 00 00 00 10 11 12 13 … 34 35 36 37";

/// The IPv4 header of a traceroute probe, its TTL `$ttl`.
macro_rules! probe_ip {
    ($ttl:literal, $id:literal, $sum:literal) => {
        header(
            &anatomy::IPV4,
            &[
                "4",
                "5",
                "0",
                "0",
                "60",
                $id,
                "000",
                "0",
                $ttl,
                "1",
                $sum,
                "192.168.1.10",
                "203.0.113.10",
            ],
            &[8, 12],
        )
    };
}

/// The IPv4 header of an ICMP error from `$from`, as it reaches the client.
macro_rules! error_ip {
    ($from:literal, $ttl:literal, $sum:literal) => {
        header(
            &anatomy::IPV4,
            &[
                "4",
                "5",
                "0",
                "0",
                "56",
                "0x0000",
                "000",
                "0",
                $ttl,
                "1",
                $sum,
                $from,
                "192.168.1.10",
            ],
            &[11],
        )
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_ICMP_1_TITLE,
        sub: k::PACKETS_ICMP_1_SUB,
        body: k::PACKETS_ICMP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "ping 203.0.113.10"),
            wire(k::PACKETS_ICMP_LABEL_MESSAGE, "type=8 code=0"),
            wire(k::PACKETS_LABEL_SEQ, "1"),
        ],
        focus: &[],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "ping 203.0.113.10",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Request,
                    "ICMP",
                    "Echo request",
                    "Echo (ping) request id=0x1a2b seq=1 ttl=64",
                    98,
                )
                .says(k::PACKETS_ICMP_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "84",
                            "0x3f1c",
                            "010",
                            "0",
                            "64",
                            "1",
                            "0xfdcf",
                            "192.168.1.10",
                            "203.0.113.10",
                        ],
                        &[9],
                    ),
                    header(
                        &anatomy::ICMP,
                        &["8", "0", "0xfbcb", "0x1a2b", "1", PING_DATA],
                        &[0, 1, 3, 4],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_ICMP_2_TITLE,
        sub: k::PACKETS_ICMP_2_SUB,
        body: k::PACKETS_ICMP_2_BODY,
        facts: &[
            wire(k::PACKETS_ICMP_LABEL_MESSAGE, "type=0 code=0"),
            wire(k::PACKETS_LABEL_SEQ, "1"),
            wire(k::PACKETS_ICMP_LABEL_RTT, "30 ms"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Response,
                    "ICMP",
                    "Echo reply",
                    "Echo (ping) reply id=0x1a2b seq=1 ttl=61",
                    98,
                )
                .says(k::PACKETS_ICMP_P2)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "84",
                            "0x9d40",
                            "000",
                            "0",
                            "61",
                            "1",
                            "0xe2ab",
                            "203.0.113.10",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::ICMP,
                        &["0", "0", "0x03cc", "0x1a2b", "1", PING_DATA],
                        &[0, 3, 4, 5],
                    ),
                ]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Clock),
            },
        ],
    },
    Step {
        title: k::PACKETS_ICMP_3_TITLE,
        sub: k::PACKETS_ICMP_3_SUB,
        body: k::PACKETS_ICMP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "traceroute -I 203.0.113.10"),
            wire(k::PACKETS_LABEL_TTL_HOPS, "1"),
            wire(k::PACKETS_LABEL_HOP, "1  192.168.1.1  2 ms"),
        ],
        focus: &[CLIENT, ROUTER],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "traceroute -I 203.0.113.10",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    ROUTER,
                    Class::Request,
                    "ICMP",
                    "TTL=1",
                    "Echo (ping) request dst=203.0.113.10 id=0x0c47 seq=1 ttl=1",
                    74,
                )
                .says(k::PACKETS_ICMP_P3)
                .headers(&[
                    probe_ip!("1", "0x5e01", "0x5e03"),
                    header(&anatomy::ICMP, &["8", "0", "0x9c51", "0x0c47", "1"], &[]),
                ]),
            ),
            Beat::Light {
                at: ROUTER,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    ROUTER,
                    CLIENT,
                    Class::Refusal,
                    "ICMP",
                    "Time exceeded",
                    "Time-to-live exceeded type=11 code=0",
                    70,
                )
                .says(k::PACKETS_ICMP_P4)
                .headers(&[
                    error_ip!("192.168.1.1", "64", "0xf769"),
                    header(
                        &anatomy::ICMP,
                        &[
                            "11",
                            "0",
                            "0xf4ff",
                            "0",
                            "0",
                            "IPv4 192.168.1.10 → 203.0.113.10 · ICMP type=8 id=0x0c47 seq=1",
                        ],
                        &[0, 1, 5],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_ICMP_4_TITLE,
        sub: k::PACKETS_ICMP_4_SUB,
        body: k::PACKETS_ICMP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TTL_HOPS, "2, 3"),
            wire(
                k::PACKETS_LABEL_PATH,
                "192.168.1.1 → 198.51.100.1 → 192.0.2.1",
            ),
        ],
        focus: &[CLIENT, ROUTER, ISP, HOSTING],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    ISP,
                    Class::Request,
                    "ICMP",
                    "TTL=2",
                    "Echo (ping) request dst=203.0.113.10 id=0x0c47 seq=4 ttl=2",
                    74,
                )
                .says(k::PACKETS_ICMP_P5)
                .headers(&[
                    probe_ip!("2", "0x5e04", "0x5d00"),
                    header(&anatomy::ICMP, &["8", "0", "0x9c4e", "0x0c47", "4"], &[4]),
                ]),
            ),
            Beat::Light {
                at: ISP,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    ISP,
                    CLIENT,
                    Class::Refusal,
                    "ICMP",
                    "Time exceeded",
                    "Time-to-live exceeded type=11 code=0",
                    70,
                )
                .says(k::PACKETS_ICMP_P6)
                .headers(&[
                    error_ip!("198.51.100.1", "63", "0x8fde"),
                    header(
                        &anatomy::ICMP,
                        &[
                            "11",
                            "0",
                            "0xf4fc",
                            "0",
                            "0",
                            "IPv4 192.168.1.10 → 203.0.113.10 · ICMP type=8 id=0x0c47 seq=4",
                        ],
                        &[0, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    HOSTING,
                    Class::Request,
                    "ICMP",
                    "TTL=3",
                    "Echo (ping) request dst=203.0.113.10 id=0x0c47 seq=7 ttl=3",
                    74,
                )
                .says(k::PACKETS_ICMP_P7)
                .headers(&[
                    probe_ip!("3", "0x5e07", "0x5bfd"),
                    header(&anatomy::ICMP, &["8", "0", "0x9c4b", "0x0c47", "7"], &[4]),
                ]),
            ),
            Beat::Light {
                at: HOSTING,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    HOSTING,
                    CLIENT,
                    Class::Refusal,
                    "ICMP",
                    "Time exceeded",
                    "Time-to-live exceeded type=11 code=0",
                    70,
                )
                .says(k::PACKETS_ICMP_P8)
                .headers(&[
                    error_ip!("192.0.2.1", "62", "0xf911"),
                    header(
                        &anatomy::ICMP,
                        &[
                            "11",
                            "0",
                            "0xf4f9",
                            "0",
                            "0",
                            "IPv4 192.168.1.10 → 203.0.113.10 · ICMP type=8 id=0x0c47 seq=7",
                        ],
                        &[0, 5],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_ICMP_5_TITLE,
        sub: k::PACKETS_ICMP_5_SUB,
        body: k::PACKETS_ICMP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TTL_HOPS, "4"),
            wire(k::PACKETS_LABEL_HOP, "4  203.0.113.10  30 ms"),
            wire(k::PACKETS_ICMP_LABEL_MESSAGE, "type=0 code=0"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Request,
                    "ICMP",
                    "TTL=4",
                    "Echo (ping) request dst=203.0.113.10 id=0x0c47 seq=10 ttl=4",
                    74,
                )
                .says(k::PACKETS_ICMP_P9)
                .headers(&[
                    probe_ip!("4", "0x5e0a", "0x5afa"),
                    header(&anatomy::ICMP, &["8", "0", "0x9c48", "0x0c47", "10"], &[4]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Response,
                    "ICMP",
                    "Echo reply",
                    "Echo (ping) reply id=0x0c47 seq=10 ttl=61",
                    74,
                )
                .says(k::PACKETS_ICMP_P10)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "60",
                            "0x2b77",
                            "000",
                            "0",
                            "61",
                            "1",
                            "0x548d",
                            "203.0.113.10",
                            "192.168.1.10",
                        ],
                        &[11],
                    ),
                    header(
                        &anatomy::ICMP,
                        &["0", "0", "0xa448", "0x0c47", "10"],
                        &[0, 4],
                    ),
                ]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_ICMP_6_TITLE,
        sub: k::PACKETS_ICMP_6_SUB,
        body: k::PACKETS_ICMP_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "ping 203.0.113.99"),
            wire(k::PACKETS_ICMP_LABEL_MESSAGE, "type=3 code=1"),
            fact(k::PACKETS_LABEL_REASON, k::PACKETS_ICMP_6_REASON),
        ],
        focus: &[CLIENT, ROUTER, ISP, HOSTING],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "ping 203.0.113.99",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    HOSTING,
                    Class::Request,
                    "ICMP",
                    "Echo request",
                    "Echo (ping) request dst=203.0.113.99 id=0x1a2c seq=1 ttl=64",
                    98,
                )
                .says(k::PACKETS_ICMP_P11)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "84",
                            "0x40a2",
                            "010",
                            "0",
                            "64",
                            "1",
                            "0xfbf0",
                            "192.168.1.10",
                            "203.0.113.99",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::ICMP,
                        &["8", "0", "0xfbca", "0x1a2c", "1", PING_DATA],
                        &[0],
                    ),
                ]),
            ),
            Beat::Light {
                at: HOSTING,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    HOSTING,
                    CLIENT,
                    Class::Refusal,
                    "ICMP",
                    "Unreachable",
                    "Destination unreachable type=3 code=1 (host unreachable)",
                    70,
                )
                .says(k::PACKETS_ICMP_P12)
                .headers(&[
                    error_ip!("192.0.2.1", "62", "0xf911"),
                    header(
                        &anatomy::ICMP,
                        &[
                            "3",
                            "1",
                            "0x8f1d",
                            "0",
                            "0",
                            "IPv4 192.168.1.10 → 203.0.113.99 · ICMP type=8 id=0x1a2c seq=1",
                        ],
                        &[0, 1, 5],
                    ),
                ]),
            ),
        ],
    },
];

/// ICMP.
pub static ICMP: Protocol = Protocol {
    id: "icmp",
    group: Group::Link,
    layer: Layer(3),
    name: k::PACKETS_ICMP_NAME,
    title: k::PACKETS_ICMP_TITLE,
    tagline: k::PACKETS_ICMP_TAGLINE,
    overview: k::PACKETS_ICMP_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_TRANSPORT, k::PACKETS_ICMP_TRANSPORT),
        wire(k::PACKETS_FACT_HEADER, "8 B"),
        fact(k::PACKETS_ICMP_LABEL_TYPES, k::PACKETS_ICMP_TYPES),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_ICMP_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 792, RFC 4443 (ICMPv6)"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, ISP, 5),
            link(ISP, HOSTING, 8),
            link(HOSTING, SERVER, 1),
        ],
    },
    steps: &STEPS,
};
