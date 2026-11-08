//! DHCP: a laptop joining a network gets its address — DISCOVER, OFFER,
//! REQUEST and ACK, the first and third by broadcast — and renews the lease
//! halfway through it.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const SWITCH: usize = 1;
const SERVER: usize = 2;
const ROUTER: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x10]),
    },
    Device {
        kind: Kind::Switch,
        at: at(-3, 0),
        name: k::PACKETS_DEVICE_SWITCH,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::List),
        at: at(3, -4),
        name: k::PACKETS_DEVICE_DHCP_SERVER,
        address: Address::V4([192, 168, 1, 2]),
    },
    Device {
        kind: Kind::Router,
        at: at(4, 3),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([192, 168, 1, 1]),
    },
];

/// The laptop's hardware address, as `chaddr` carries it.
const MAC: &str = "00:00:5e:00:53:10";

/// An IPv4 header of a DHCP message: 328 bytes, UDP.
macro_rules! ip {
    ($id:literal, $sum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", "328", $id, "000", "0", "64", "17", $sum, $src, $dst,
            ],
            $lit,
        )
    };
}

/// A DHCP message: op, xid, ciaddr, yiaddr and options; hardware type 1
/// (Ethernet), 6-byte addresses, no relay.
macro_rules! dhcp {
    ($op:literal, $xid:literal, $ciaddr:literal, $yiaddr:literal, $options:literal, $lit:expr) => {
        header(
            &anatomy::DHCP,
            &[
                $op, "1", "6", "0", $xid, "0", "0x0000", $ciaddr, $yiaddr, "0.0.0.0", "0.0.0.0",
                MAC, $options,
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_DHCP_1_TITLE,
        sub: k::PACKETS_DHCP_1_SUB,
        body: k::PACKETS_DHCP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "0.0.0.0:68"),
            wire(k::PACKETS_LABEL_DESTINATION, "255.255.255.255:67"),
            wire(k::PACKETS_LABEL_MAC, MAC),
        ],
        focus: &[LAPTOP, SWITCH, SERVER],
        beats: &[Beat::Send(
            Packet::new(
                LAPTOP,
                SERVER,
                Class::Lookup,
                "DHCP",
                "DISCOVER",
                "0.0.0.0 → 255.255.255.255 DHCPDISCOVER xid=0x5a1c09e3",
                342,
            )
            .says(k::PACKETS_DHCP_P1)
            .headers(&[
                ip!("0x0000", "0x79a6", "0.0.0.0", "255.255.255.255", &[11, 12]),
                header(&anatomy::UDP, &["68", "67", "308", "0x5a3c"], &[0, 1]),
                dhcp!(
                    "1",
                    "0x5a1c09e3",
                    "0.0.0.0",
                    "0.0.0.0",
                    "53 Message type=DISCOVER · 55 Parameter request list=1,3,6,51",
                    &[0, 4, 11, 12]
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_DHCP_2_TITLE,
        sub: k::PACKETS_DHCP_2_SUB,
        body: k::PACKETS_DHCP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ADDRESS, "192.168.1.10"),
            wire(k::PACKETS_DHCP_LABEL_MASK, "255.255.255.0"),
            wire(k::PACKETS_LABEL_LEASE, "86400 s (24 h)"),
        ],
        focus: &[LAPTOP, SWITCH, SERVER],
        beats: &[
            Beat::Light {
                at: SERVER,
                prop: None,
            },
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Response,
                    "DHCP",
                    "OFFER",
                    "192.168.1.2 → 192.168.1.10 DHCPOFFER xid=0x5a1c09e3 yiaddr=192.168.1.10",
                    342,
                )
                .says(k::PACKETS_DHCP_P2)
                .headers(&[
                    ip!("0x6c1b", "0x8a2d", "192.168.1.2", "192.168.1.10", &[12]),
                    header(&anatomy::UDP, &["67", "68", "308", "0x8e1f"], &[]),
                    dhcp!(
                        "2",
                        "0x5a1c09e3",
                        "0.0.0.0",
                        "192.168.1.10",
                        "53 Message type=OFFER · 54 Server ID=192.168.1.2 · 51 Lease time=86400 · 1 Subnet mask=255.255.255.0 · 3 Router=192.168.1.1 · 6 DNS=192.168.1.1",
                        &[0, 4, 8, 12]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DHCP_3_TITLE,
        sub: k::PACKETS_DHCP_3_SUB,
        body: k::PACKETS_DHCP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ADDRESS, "192.168.1.10"),
            wire(k::PACKETS_DHCP_LABEL_CHOSEN, "192.168.1.2"),
            wire(k::PACKETS_LABEL_BROADCAST, "255.255.255.255"),
        ],
        focus: &[LAPTOP, SWITCH, SERVER],
        beats: &[Beat::Send(
            Packet::new(
                LAPTOP,
                SERVER,
                Class::Request,
                "DHCP",
                "REQUEST",
                "0.0.0.0 → 255.255.255.255 DHCPREQUEST xid=0x5a1c09e3",
                342,
            )
            .says(k::PACKETS_DHCP_P3)
            .headers(&[
                ip!("0x0000", "0x79a6", "0.0.0.0", "255.255.255.255", &[12]),
                header(&anatomy::UDP, &["68", "67", "308", "0x7b02"], &[]),
                dhcp!(
                    "1",
                    "0x5a1c09e3",
                    "0.0.0.0",
                    "0.0.0.0",
                    "53 Message type=REQUEST · 50 Requested IP=192.168.1.10 · 54 Server ID=192.168.1.2 · 55 Parameter request list=1,3,6,51",
                    &[7, 12]
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_DHCP_4_TITLE,
        sub: k::PACKETS_DHCP_4_SUB,
        body: k::PACKETS_DHCP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ADDRESS, "192.168.1.10/24"),
            wire(k::PACKETS_LABEL_GATEWAY, "192.168.1.1"),
            wire(k::PACKETS_DHCP_LABEL_DNS, "192.168.1.1"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Response,
                    "DHCP",
                    "ACK",
                    "192.168.1.2 → 192.168.1.10 DHCPACK xid=0x5a1c09e3 yiaddr=192.168.1.10",
                    342,
                )
                .says(k::PACKETS_DHCP_P4)
                .headers(&[
                    ip!("0x6c1c", "0x8a2c", "192.168.1.2", "192.168.1.10", &[]),
                    header(&anatomy::UDP, &["67", "68", "308", "0x2d6e"], &[]),
                    dhcp!(
                        "2",
                        "0x5a1c09e3",
                        "0.0.0.0",
                        "192.168.1.10",
                        "53 Message type=ACK · 54 Server ID=192.168.1.2 · 51 Lease time=86400 · 58 Renewal time=43200 · 59 Rebinding time=75600 · 1 Subnet mask=255.255.255.0 · 3 Router=192.168.1.1 · 6 DNS=192.168.1.1",
                        &[8, 12]
                    ),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
            Beat::Light {
                at: ROUTER,
                prop: None,
            },
        ],
    },
    Step {
        title: k::PACKETS_DHCP_5_TITLE,
        sub: k::PACKETS_DHCP_5_SUB,
        body: k::PACKETS_DHCP_5_BODY,
        facts: &[
            wire(k::PACKETS_DHCP_LABEL_RENEWAL, "43200 s (12 h)"),
            wire(k::PACKETS_LABEL_SOURCE, "192.168.1.10:68"),
            wire(k::PACKETS_LABEL_DESTINATION, "192.168.1.2:67"),
        ],
        focus: &[LAPTOP, SWITCH, SERVER],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Clock),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SERVER,
                    Class::Request,
                    "DHCP",
                    "REQUEST",
                    "192.168.1.10 → 192.168.1.2 DHCPREQUEST xid=0x1d7e44b0 ciaddr=192.168.1.10",
                    342,
                )
                .says(k::PACKETS_DHCP_P5)
                .headers(&[
                    ip!("0x8e3a", "0x680e", "192.168.1.10", "192.168.1.2", &[11, 12]),
                    header(&anatomy::UDP, &["68", "67", "308", "0x41c9"], &[]),
                    dhcp!(
                        "1",
                        "0x1d7e44b0",
                        "192.168.1.10",
                        "0.0.0.0",
                        "53 Message type=REQUEST · 55 Parameter request list=1,3,6,51",
                        &[7]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Response,
                    "DHCP",
                    "ACK",
                    "192.168.1.2 → 192.168.1.10 DHCPACK xid=0x1d7e44b0",
                    342,
                )
                .says(k::PACKETS_DHCP_P6)
                .headers(&[
                    ip!("0x6d02", "0x8946", "192.168.1.2", "192.168.1.10", &[]),
                    header(&anatomy::UDP, &["67", "68", "308", "0x9a04"], &[]),
                    dhcp!(
                        "2",
                        "0x1d7e44b0",
                        "192.168.1.10",
                        "192.168.1.10",
                        "53 Message type=ACK · 54 Server ID=192.168.1.2 · 51 Lease time=86400 · 58 Renewal time=43200 · 59 Rebinding time=75600",
                        &[12]
                    ),
                ]),
            ),
        ],
    },
];

/// DHCP.
pub static DHCP: Protocol = Protocol {
    id: "dhcp",
    group: Group::Link,
    layer: Layer(7),
    name: k::PACKETS_DHCP_NAME,
    title: k::PACKETS_DHCP_TITLE,
    tagline: k::PACKETS_DHCP_TAGLINE,
    overview: k::PACKETS_DHCP_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_DHCP_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "UDP"),
        wire(
            k::PACKETS_DHCP_LABEL_MESSAGES,
            "DISCOVER, OFFER, REQUEST, ACK, NAK, RELEASE",
        ),
        wire(
            k::PACKETS_FACT_DEFINED,
            "RFC 2131, RFC 2132 (DHCPv6: RFC 8415)",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, SWITCH, 1),
            link(SWITCH, SERVER, 1),
            link(SWITCH, ROUTER, 1),
        ],
    },
    steps: &STEPS,
};
