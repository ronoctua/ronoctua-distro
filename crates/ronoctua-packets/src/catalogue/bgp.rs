//! BGP: routes between autonomous systems — a session opened between two
//! neighbours, a prefix announced with its AS path growing at each hand-over,
//! the provider choosing between two paths, a false origin refused by RPKI,
//! and the route withdrawn.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const ISP: usize = 0;
const TRANSIT: usize = 1;
const SECOND: usize = 2;
const HOSTING: usize = 3;
const ATTACKER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::CoreRouter,
        at: at(-8, 0),
        name: k::PACKETS_DEVICE_ISP_ROUTER,
        address: Address::As(64500),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(0, -4),
        name: k::PACKETS_DEVICE_TRANSIT_ROUTER,
        address: Address::As(64510),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(1, 3),
        name: k::PACKETS_BGP_DEVICE_SECOND,
        address: Address::As(64505),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(8, -1),
        name: k::PACKETS_DEVICE_HOSTING_ROUTER,
        address: Address::As(64511),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(-4, 7),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::As(64496),
    },
];

/// Every BGP message starts with sixteen bytes of ones.
const MARKER: &str = "0xffffffffffffffffffffffffffffffff";

/// A TCP segment of the session between the hosting router (port 51234)
/// and the transit router (port 179), carrying BGP: PSH and ACK set.
macro_rules! tcp {
    ($src:literal, $dst:literal, $seq:literal, $ack:literal, $sum:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, $seq, $ack, "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "16384",
                $sum, "0",
            ],
            $lit,
        )
    };
}

/// A BGP message: its length, type and body.
macro_rules! bgp {
    ($len:literal, $kind:literal, $body:literal, $lit:expr) => {
        header(&anatomy::BGP, &[MARKER, $len, $kind, $body], $lit)
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_BGP_1_TITLE,
        sub: k::PACKETS_BGP_1_SUB,
        body: k::PACKETS_BGP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "TCP 179"),
            wire(k::PACKETS_LABEL_VERSION, "4"),
            wire(k::PACKETS_BGP_LABEL_HOLD, "90 s"),
        ],
        focus: &[TRANSIT, HOSTING],
        beats: &[
            Beat::Send(
                Packet::new(
                    HOSTING,
                    TRANSIT,
                    Class::Handshake,
                    "BGP",
                    "OPEN",
                    "OPEN My AS=64511 Hold time=90 BGP ID=203.0.113.1",
                    99,
                )
                .says(k::PACKETS_BGP_P1)
                .headers(&[
                    tcp!("51234", "179", "1", "1", "0x6a0e", &[1]),
                    bgp!(
                        "45",
                        "1",
                        "Version=4 My AS=64511 Hold time=90 BGP ID=203.0.113.1 Capabilities=IPv4 unicast, route refresh, 4-octet AS",
                        &[2, 3]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    TRANSIT,
                    HOSTING,
                    Class::Handshake,
                    "BGP",
                    "OPEN",
                    "OPEN My AS=64510 Hold time=90 BGP ID=192.0.2.1",
                    99,
                )
                .says(k::PACKETS_BGP_P2)
                .headers(&[
                    tcp!("179", "51234", "1", "46", "0x1f93", &[0]),
                    bgp!(
                        "45",
                        "1",
                        "Version=4 My AS=64510 Hold time=90 BGP ID=192.0.2.1 Capabilities=IPv4 unicast, route refresh, 4-octet AS",
                        &[3]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_BGP_2_TITLE,
        sub: k::PACKETS_BGP_2_SUB,
        body: k::PACKETS_BGP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_STATE, "Established"),
            wire(k::PACKETS_LABEL_INTERVAL, "30 s"),
        ],
        focus: &[TRANSIT, HOSTING],
        beats: &[
            Beat::Send(
                Packet::new(HOSTING, TRANSIT, Class::Control, "BGP", "KEEPALIVE", "KEEPALIVE", 73)
                    .says(k::PACKETS_BGP_P3)
                    .headers(&[
                        tcp!("51234", "179", "46", "46", "0x5c21", &[]),
                        header(&anatomy::BGP, &[MARKER, "19", "4"], &[1, 2]),
                    ]),
            ),
            Beat::Send(
                Packet::new(TRANSIT, HOSTING, Class::Control, "BGP", "KEEPALIVE", "KEEPALIVE", 73)
                    .says(k::PACKETS_BGP_P4)
                    .headers(&[
                        tcp!("179", "51234", "46", "65", "0x0b7d", &[]),
                        header(&anatomy::BGP, &[MARKER, "19", "4"], &[2]),
                    ]),
            ),
            Beat::Light {
                at: HOSTING,
                prop: Some(Prop::Check),
            },
            Beat::Light {
                at: TRANSIT,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_BGP_3_TITLE,
        sub: k::PACKETS_BGP_3_SUB,
        body: k::PACKETS_BGP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PREFIX, "203.0.113.0/24"),
            wire(k::PACKETS_LABEL_AS_PATH, "64511 → 64510 64511"),
        ],
        focus: &[ISP, TRANSIT, SECOND, HOSTING],
        beats: &[
            Beat::Send(
                Packet::new(
                    HOSTING,
                    TRANSIT,
                    Class::Data,
                    "BGP",
                    "UPDATE",
                    "UPDATE NLRI=203.0.113.0/24 AS_PATH=64511",
                    101,
                )
                .says(k::PACKETS_BGP_P5)
                .headers(&[
                    tcp!("51234", "179", "65", "65", "0x77c4", &[]),
                    bgp!(
                        "47",
                        "2",
                        "ORIGIN=IGP AS_PATH=64511 NEXT_HOP=192.0.2.2 NLRI=203.0.113.0/24",
                        &[2, 3]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    TRANSIT,
                    ISP,
                    Class::Data,
                    "BGP",
                    "UPDATE",
                    "UPDATE NLRI=203.0.113.0/24 AS_PATH=64510 64511",
                    105,
                )
                .says(k::PACKETS_BGP_P6)
                .headers(&[bgp!(
                    "51",
                    "2",
                    "ORIGIN=IGP AS_PATH=64510 64511 NEXT_HOP=198.51.100.1 NLRI=203.0.113.0/24",
                    &[3]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    TRANSIT,
                    SECOND,
                    Class::Data,
                    "BGP",
                    "UPDATE",
                    "UPDATE NLRI=203.0.113.0/24 AS_PATH=64510 64511",
                    105,
                )
                .says(k::PACKETS_BGP_P7)
                .headers(&[bgp!(
                    "51",
                    "2",
                    "ORIGIN=IGP AS_PATH=64510 64511 NEXT_HOP=192.0.2.5 NLRI=203.0.113.0/24",
                    &[3]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_BGP_4_TITLE,
        sub: k::PACKETS_BGP_4_SUB,
        body: k::PACKETS_BGP_4_BODY,
        facts: &[
            wire(k::PACKETS_BGP_LABEL_BEST, "64510 64511"),
            wire(k::PACKETS_BGP_LABEL_BACKUP, "64505 64510 64511"),
        ],
        focus: &[ISP, TRANSIT, SECOND],
        beats: &[
            Beat::Send(
                Packet::new(
                    SECOND,
                    ISP,
                    Class::Data,
                    "BGP",
                    "UPDATE",
                    "UPDATE NLRI=203.0.113.0/24 AS_PATH=64505 64510 64511",
                    109,
                )
                .says(k::PACKETS_BGP_P8)
                .headers(&[bgp!(
                    "55",
                    "2",
                    "ORIGIN=IGP AS_PATH=64505 64510 64511 NEXT_HOP=198.51.100.5 NLRI=203.0.113.0/24",
                    &[3]
                )]),
            ),
            Beat::Light {
                at: ISP,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_BGP_5_TITLE,
        sub: k::PACKETS_BGP_5_SUB,
        body: k::PACKETS_BGP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_AS_PATH, "64496"),
            wire(k::PACKETS_BGP_LABEL_ROA, "203.0.113.0/24 max /24 → AS64511"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_BGP_5_VERDICT),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: HOSTING,
                prop: Some(Prop::Certificate),
            },
            Beat::Send(
                Packet::new(
                    ATTACKER,
                    ISP,
                    Class::Refusal,
                    "BGP",
                    "UPDATE",
                    "UPDATE NLRI=203.0.113.0/24 AS_PATH=64496",
                    101,
                )
                .says(k::PACKETS_BGP_P9)
                .headers(&[bgp!(
                    "47",
                    "2",
                    "ORIGIN=IGP AS_PATH=64496 NEXT_HOP=198.51.100.9 NLRI=203.0.113.0/24",
                    &[3]
                )]),
            ),
            Beat::Light {
                at: ISP,
                prop: Some(Prop::Shield),
            },
        ],
    },
    Step {
        title: k::PACKETS_BGP_6_TITLE,
        sub: k::PACKETS_BGP_6_SUB,
        body: k::PACKETS_BGP_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PREFIX, "203.0.113.0/24"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_BGP_6_STATE),
        ],
        focus: &[ISP, TRANSIT, SECOND, HOSTING],
        beats: &[
            Beat::Send(
                Packet::new(
                    HOSTING,
                    TRANSIT,
                    Class::Control,
                    "BGP",
                    "WITHDRAW",
                    "UPDATE Withdrawn routes=203.0.113.0/24",
                    81,
                )
                .says(k::PACKETS_BGP_P10)
                .headers(&[
                    tcp!("51234", "179", "112", "65", "0x2e58", &[]),
                    bgp!("27", "2", "Withdrawn routes=203.0.113.0/24", &[1, 3]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    TRANSIT,
                    ISP,
                    Class::Control,
                    "BGP",
                    "WITHDRAW",
                    "UPDATE Withdrawn routes=203.0.113.0/24",
                    81,
                )
                .says(k::PACKETS_BGP_P11)
                .headers(&[bgp!("27", "2", "Withdrawn routes=203.0.113.0/24", &[3])]),
            ),
            Beat::Send(
                Packet::new(
                    TRANSIT,
                    SECOND,
                    Class::Control,
                    "BGP",
                    "WITHDRAW",
                    "UPDATE Withdrawn routes=203.0.113.0/24",
                    81,
                )
                .says(k::PACKETS_BGP_P12),
            ),
            Beat::Send(
                Packet::new(
                    SECOND,
                    ISP,
                    Class::Control,
                    "BGP",
                    "WITHDRAW",
                    "UPDATE Withdrawn routes=203.0.113.0/24",
                    81,
                )
                .says(k::PACKETS_BGP_P13)
                .headers(&[bgp!("27", "2", "Withdrawn routes=203.0.113.0/24", &[3])]),
            ),
        ],
    },
];

/// BGP.
pub static BGP: Protocol = Protocol {
    id: "bgp",
    group: Group::Link,
    layer: Layer(7),
    name: k::PACKETS_BGP_NAME,
    title: k::PACKETS_BGP_TITLE,
    tagline: k::PACKETS_BGP_TAGLINE,
    overview: k::PACKETS_BGP_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "179"),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        wire(
            k::PACKETS_BGP_LABEL_MESSAGES,
            "OPEN, UPDATE, KEEPALIVE, NOTIFICATION",
        ),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_BGP_SECURITY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 4271, RFC 6811 (RPKI)"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(ISP, TRANSIT, 8),
            link(ISP, SECOND, 10),
            link(TRANSIT, SECOND, 6),
            link(TRANSIT, HOSTING, 5),
            link(ISP, ATTACKER, 9),
        ],
    },
    steps: &STEPS,
};
