//! ARP: the gateway's MAC address found from its IPv4 address — a question
//! broadcast to the whole network, the answer sent back to the asker alone,
//! the packet that could then leave, and a gratuitous announcement.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const SWITCH: usize = 1;
const DESKTOP: usize = 2;
const ROUTER: usize = 3;
const INTERNET: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 2),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Switch,
        at: at(-2, -1),
        name: k::PACKETS_DEVICE_SWITCH,
        address: Address::None,
    },
    Device {
        kind: Kind::Desktop,
        at: at(2, 5),
        name: k::PACKETS_DEVICE_DESKTOP,
        address: Address::V4([192, 168, 1, 11]),
    },
    Device {
        kind: Kind::Router,
        at: at(5, -3),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(9, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
];

/// The laptop's question, broadcast.
const ETH_QUESTION: &[&str] = &["ff:ff:ff:ff:ff:ff", "00:00:5e:00:53:0a", "0x0806"];
/// Who has 192.168.1.1? Tell 192.168.1.10 — the target's MAC left at zero.
const ARP_QUESTION: &[&str] = &[
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
/// The router's announcement, broadcast.
const ETH_ANNOUNCEMENT: &[&str] = &["ff:ff:ff:ff:ff:ff", "00:00:5e:00:53:01", "0x0806"];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_ARP_1_TITLE,
        sub: k::PACKETS_ARP_1_SUB,
        body: k::PACKETS_ARP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "203.0.113.10"),
            wire(k::PACKETS_LABEL_GATEWAY, "192.168.1.1"),
            fact(k::PACKETS_LABEL_CACHE, k::PACKETS_ARP_1_CACHE),
        ],
        focus: &[LAPTOP],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "ping 203.0.113.10",
            },
            Beat::Rest,
        ],
    },
    Step {
        title: k::PACKETS_ARP_2_TITLE,
        sub: k::PACKETS_ARP_2_SUB,
        body: k::PACKETS_ARP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_QUESTION, "who-has 192.168.1.1"),
            wire(k::PACKETS_LABEL_DESTINATION, "ff:ff:ff:ff:ff:ff"),
            fact(k::PACKETS_LABEL_OPCODE, k::PACKETS_ARP_2_OPCODE),
        ],
        focus: &[LAPTOP, SWITCH, DESKTOP, ROUTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SWITCH,
                    Class::Lookup,
                    "ARP",
                    "who-has 192.168.1.1",
                    "who-has 192.168.1.1 tell 192.168.1.10",
                    42,
                )
                .says(k::PACKETS_ARP_P1)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_QUESTION, &[0, 2]),
                    header(&anatomy::ARP, ARP_QUESTION, &[4, 5, 6, 7, 8]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    DESKTOP,
                    Class::Lookup,
                    "ARP",
                    "who-has 192.168.1.1",
                    "who-has 192.168.1.1 tell 192.168.1.10",
                    42,
                )
                .says(k::PACKETS_ARP_P2)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_QUESTION, &[0]),
                    header(&anatomy::ARP, ARP_QUESTION, &[8]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SWITCH,
                    ROUTER,
                    Class::Lookup,
                    "ARP",
                    "who-has 192.168.1.1",
                    "who-has 192.168.1.1 tell 192.168.1.10",
                    42,
                )
                .says(k::PACKETS_ARP_P3)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_QUESTION, &[0]),
                    header(&anatomy::ARP, ARP_QUESTION, &[8]),
                ]),
            ),
            Beat::Light {
                at: DESKTOP,
                prop: Some(Prop::Cross),
            },
        ],
    },
    Step {
        title: k::PACKETS_ARP_3_TITLE,
        sub: k::PACKETS_ARP_3_SUB,
        body: k::PACKETS_ARP_3_BODY,
        facts: &[
            wire(
                k::PACKETS_LABEL_ANSWER,
                "192.168.1.1 is-at 00:00:5e:00:53:01",
            ),
            fact(k::PACKETS_LABEL_OPCODE, k::PACKETS_ARP_3_OPCODE),
            fact(k::PACKETS_LABEL_CACHE, k::PACKETS_ARP_3_CACHE),
        ],
        focus: &[LAPTOP, SWITCH, ROUTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    ROUTER,
                    LAPTOP,
                    Class::Response,
                    "ARP",
                    "is-at 00:00:5e:00:53:01",
                    "192.168.1.1 is-at 00:00:5e:00:53:01",
                    42,
                )
                .says(k::PACKETS_ARP_P4)
                .headers(&[
                    header(
                        &anatomy::ETHERNET,
                        &["00:00:5e:00:53:0a", "00:00:5e:00:53:01", "0x0806"],
                        &[0],
                    ),
                    header(
                        &anatomy::ARP,
                        &[
                            "1",
                            "0x0800",
                            "6",
                            "4",
                            "2",
                            "00:00:5e:00:53:01",
                            "192.168.1.1",
                            "00:00:5e:00:53:0a",
                            "192.168.1.10",
                        ],
                        &[4, 5, 6],
                    ),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_ARP_4_TITLE,
        sub: k::PACKETS_ARP_4_SUB,
        body: k::PACKETS_ARP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_MAC, "00:00:5e:00:53:01"),
            wire(k::PACKETS_LABEL_DESTINATION, "203.0.113.10"),
        ],
        focus: &[LAPTOP, SWITCH, ROUTER, INTERNET],
        beats: &[Beat::Send(
            Packet::new(
                LAPTOP,
                ROUTER,
                Class::Request,
                "ICMP",
                "echo request",
                "echo request id=0x1c2b seq=1 ttl=64",
                98,
            )
            .says(k::PACKETS_ARP_P5)
            .headers(&[
                header(
                    &anatomy::ETHERNET,
                    &["00:00:5e:00:53:01", "00:00:5e:00:53:0a", "0x0800"],
                    &[0],
                ),
                header(
                    &anatomy::IPV4,
                    &[
                        "4",
                        "5",
                        "0",
                        "0",
                        "84",
                        "0x5c1e",
                        "010",
                        "0",
                        "64",
                        "1",
                        "0xe0cd",
                        "192.168.1.10",
                        "203.0.113.10",
                    ],
                    &[12],
                ),
                header(&anatomy::ICMP, &["8", "0", "0x4d5a", "0x1c2b", "1"], &[]),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_ARP_5_TITLE,
        sub: k::PACKETS_ARP_5_SUB,
        body: k::PACKETS_ARP_5_BODY,
        facts: &[
            wire(
                k::PACKETS_LABEL_ANSWER,
                "192.168.1.1 is-at 00:00:5e:00:53:01",
            ),
            wire(k::PACKETS_LABEL_DESTINATION, "ff:ff:ff:ff:ff:ff"),
            fact(k::PACKETS_LABEL_TRUST, k::PACKETS_ARP_5_TRUST),
        ],
        focus: &[LAPTOP, SWITCH, DESKTOP, ROUTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    ROUTER,
                    SWITCH,
                    Class::Response,
                    "ARP",
                    "is-at 00:00:5e:00:53:01",
                    "192.168.1.1 is-at 00:00:5e:00:53:01",
                    42,
                )
                .says(k::PACKETS_ARP_P6)
                .headers(&[
                    header(&anatomy::ETHERNET, ETH_ANNOUNCEMENT, &[0]),
                    // GRATUITOUS: the sender's and the target's IP address
                    // are the same, and so are the MACs (RFC 5944, 4.6).
                    header(
                        &anatomy::ARP,
                        &[
                            "1",
                            "0x0800",
                            "6",
                            "4",
                            "2",
                            "00:00:5e:00:53:01",
                            "192.168.1.1",
                            "00:00:5e:00:53:01",
                            "192.168.1.1",
                        ],
                        &[5, 6, 8],
                    ),
                ]),
            ),
            Beat::Send(Packet::new(
                SWITCH,
                LAPTOP,
                Class::Response,
                "ARP",
                "is-at 00:00:5e:00:53:01",
                "192.168.1.1 is-at 00:00:5e:00:53:01",
                42,
            )),
            Beat::Send(Packet::new(
                SWITCH,
                DESKTOP,
                Class::Response,
                "ARP",
                "is-at 00:00:5e:00:53:01",
                "192.168.1.1 is-at 00:00:5e:00:53:01",
                42,
            )),
        ],
    },
];

/// ARP.
pub static ARP: Protocol = Protocol {
    id: "arp",
    group: Group::Link,
    layer: Layer(2),
    name: k::PACKETS_ARP_NAME,
    title: k::PACKETS_ARP_TITLE,
    tagline: k::PACKETS_ARP_TAGLINE,
    overview: k::PACKETS_ARP_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_TRANSPORT, "Ethernet, EtherType 0x0806"),
        wire(k::PACKETS_FACT_HEADER, "28 B"),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_ARP_USED_BY),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_ARP_SECURITY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 826, RFC 5227"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, SWITCH, 1),
            link(SWITCH, DESKTOP, 1),
            link(SWITCH, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
        ],
    },
    steps: &STEPS,
};
