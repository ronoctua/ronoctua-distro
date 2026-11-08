//! IPV6: a laptop configuring itself on a network — a link-local address
//! checked for duplicates, the router found by solicitation and
//! advertisement, a global address made from its prefix (SLAAC), a
//! neighbour's MAC found by Neighbor Discovery instead of ARP, and a packet
//! out through the router.
//!
//! THE LAPTOP SHOWS NO ADDRESS under its name: it has none when the story
//! begins and three as it goes (`::`, its link-local one, its global one),
//! which the capture lines and the headers name.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const SWITCH: usize = 1;
const PRINTER: usize = 2;
const ROUTER: usize = 3;
const INTERNET: usize = 4;
const SERVER: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 2),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::None,
    },
    Device {
        kind: Kind::Switch,
        at: at(-4, -1),
        name: k::PACKETS_DEVICE_SWITCH,
        address: Address::None,
    },
    Device {
        kind: Kind::Printer,
        at: at(-3, 5),
        name: k::PACKETS_DEVICE_PRINTER,
        address: Address::V6([0x2001, 0xdb8, 1, 1, 0, 0, 0, 0x20]),
    },
    Device {
        kind: Kind::Router,
        at: at(1, -2),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V6([0xfe80, 0, 0, 0, 0, 0, 0, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(5, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(9, -2),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V6([0x2001, 0xdb8, 2, 0, 0, 0, 0, 0x10]),
    },
];

/// The laptop's global address, made from the router's prefix.
const GLOBAL: &str = "2001:db8:1:1:3c91:b7e2:41f0:6a5d";

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_IPV6_1_TITLE,
        sub: k::PACKETS_IPV6_1_SUB,
        body: k::PACKETS_IPV6_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ADDRESS, "fe80::8d4f:2a1c:93e6:5b07"),
            fact(k::PACKETS_LABEL_SCOPE, k::PACKETS_IPV6_1_SCOPE),
            wire(k::PACKETS_LABEL_MULTICAST, "ff02::1:ffe6:5b07"),
        ],
        focus: &[LAPTOP, SWITCH],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SWITCH,
                    Class::Lookup,
                    "ICMPv6",
                    "NS (DAD)",
                    "NS who-has fe80::8d4f:2a1c:93e6:5b07",
                    78,
                )
                .says(k::PACKETS_IPV6_P1)
                .headers(&[
                    header(
                        &anatomy::IPV6,
                        &["6", "0", "0", "24", "58", "255", "::", "ff02::1:ffe6:5b07"],
                        &[6, 7],
                    ),
                    header(
                        &anatomy::NDP,
                        &["135", "0", "0x79e0", "0", "fe80::8d4f:2a1c:93e6:5b07"],
                        &[0, 4],
                    ),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Clock),
            },
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_IPV6_2_TITLE,
        sub: k::PACKETS_IPV6_2_SUB,
        body: k::PACKETS_IPV6_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PREFIX, "2001:db8:1:1::/64"),
            wire(k::PACKETS_LABEL_GATEWAY, "fe80::1"),
            wire(k::PACKETS_LABEL_FLAGS, "A=1 L=1 M=0 O=0"),
        ],
        focus: &[LAPTOP, SWITCH, ROUTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    ROUTER,
                    Class::Lookup,
                    "ICMPv6",
                    "RS",
                    "Router Solicitation → ff02::2",
                    70,
                )
                .says(k::PACKETS_IPV6_P2)
                .headers(&[header(
                    &anatomy::IPV6,
                    &[
                        "6",
                        "0",
                        "0",
                        "16",
                        "58",
                        "255",
                        "fe80::8d4f:2a1c:93e6:5b07",
                        "ff02::2",
                    ],
                    &[4, 7],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    ROUTER,
                    LAPTOP,
                    Class::Response,
                    "ICMPv6",
                    "RA 2001:db8:1:1::/64",
                    "Router Advertisement 2001:db8:1:1::/64 A=1 L=1",
                    118,
                )
                .says(k::PACKETS_IPV6_P3)
                .headers(&[header(
                    &anatomy::IPV6,
                    &["6", "0", "0", "64", "58", "255", "fe80::1", "ff02::1"],
                    &[5, 6, 7],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_IPV6_3_TITLE,
        sub: k::PACKETS_IPV6_3_SUB,
        body: k::PACKETS_IPV6_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ADDRESS, GLOBAL),
            fact(k::PACKETS_LABEL_SCOPE, k::PACKETS_IPV6_3_SCOPE),
        ],
        focus: &[LAPTOP],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
            Beat::Rest,
        ],
    },
    Step {
        title: k::PACKETS_IPV6_4_TITLE,
        sub: k::PACKETS_IPV6_4_SUB,
        body: k::PACKETS_IPV6_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_MULTICAST, "ff02::1:ff00:20"),
            wire(k::PACKETS_LABEL_MAC, "00:00:5e:00:53:14"),
            fact(k::PACKETS_LABEL_CACHE, k::PACKETS_IPV6_4_CACHE),
        ],
        focus: &[LAPTOP, SWITCH, PRINTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    PRINTER,
                    Class::Lookup,
                    "ICMPv6",
                    "NS 2001:db8:1:1::20",
                    "NS who-has 2001:db8:1:1::20",
                    86,
                )
                .says(k::PACKETS_IPV6_P4)
                .headers(&[
                    header(
                        &anatomy::IPV6,
                        &["6", "0", "0", "32", "58", "255", GLOBAL, "ff02::1:ff00:20"],
                        &[7],
                    ),
                    header(
                        &anatomy::NDP,
                        &[
                            "135",
                            "0",
                            "0xcc1c",
                            "0",
                            "2001:db8:1:1::20",
                            "SLLA 00:00:5e:00:53:0a",
                        ],
                        &[0, 4, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PRINTER,
                    LAPTOP,
                    Class::Response,
                    "ICMPv6",
                    "NA 00:00:5e:00:53:14",
                    "NA 2001:db8:1:1::20 is-at 00:00:5e:00:53:14",
                    86,
                )
                .says(k::PACKETS_IPV6_P5)
                .headers(&[
                    header(
                        &anatomy::IPV6,
                        &["6", "0", "0", "32", "58", "255", "2001:db8:1:1::20", GLOBAL],
                        &[],
                    ),
                    // SOLICITED AND OVERRIDE: the S and O flags, 0x40000000
                    // and 0x20000000.
                    header(
                        &anatomy::NDP,
                        &[
                            "136",
                            "0",
                            "0x3a5c",
                            "0x60000000",
                            "2001:db8:1:1::20",
                            "TLLA 00:00:5e:00:53:14",
                        ],
                        &[0, 3, 5],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_IPV6_5_TITLE,
        sub: k::PACKETS_IPV6_5_SUB,
        body: k::PACKETS_IPV6_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "2001:db8:2::10"),
            wire(k::PACKETS_LABEL_NEXT_HOP, "fe80::1"),
            wire(k::PACKETS_LABEL_TTL_HOPS, "64"),
        ],
        focus: &[],
        beats: &[Beat::Send(
            Packet::new(
                LAPTOP,
                SERVER,
                Class::Data,
                "IPv6",
                "hlim=64",
                "2001:db8:2::10 hlim=64 TCP 50412 → 443",
                94,
            )
            .says(k::PACKETS_IPV6_P6)
            .headers(&[
                header(
                    &anatomy::IPV6,
                    &[
                        "6",
                        "0",
                        "0x8b3c1",
                        "40",
                        "6",
                        "64",
                        GLOBAL,
                        "2001:db8:2::10",
                    ],
                    &[4, 5, 7],
                ),
                header(
                    &anatomy::TCP,
                    &[
                        "50412", "443", "0", "0", "10", "0", "0", "0", "0", "0", "0", "0", "1",
                        "0", "64800", "0x9d2e", "0",
                    ],
                    &[],
                ),
            ]),
        )],
    },
];

/// IPv6.
pub static IPV6: Protocol = Protocol {
    id: "ipv6",
    group: Group::Link,
    layer: Layer(3),
    name: k::PACKETS_IPV6_NAME,
    title: k::PACKETS_IPV6_TITLE,
    tagline: k::PACKETS_IPV6_TAGLINE,
    overview: k::PACKETS_IPV6_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_ADDRESSING, k::PACKETS_IPV6_ADDRESSING),
        wire(k::PACKETS_FACT_HEADER, "40 B"),
        fact(k::PACKETS_FACT_REPLACES, k::PACKETS_IPV6_REPLACES),
        wire(k::PACKETS_FACT_DEFINED, "RFC 8200, RFC 4861, RFC 4862"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, SWITCH, 1),
            link(SWITCH, PRINTER, 1),
            link(SWITCH, ROUTER, 1),
            link(ROUTER, INTERNET, 5),
            link(INTERNET, SERVER, 8),
        ],
    },
    steps: &STEPS,
};
