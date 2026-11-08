//! WireGuard: a laptop joined to a private network behind a gateway — the
//! peers' keys and allowed IPs, the Noise IK handshake (initiation and
//! response), whole IP packets carried inside UDP 51820, the keepalive that
//! holds a NAT open, and the silence a stranger without a key gets.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const GATEWAY: usize = 3;
const APP: usize = 4;
const ATTACKER: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-5, 0),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(0, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Shield),
        at: at(5, -2),
        name: k::PACKETS_DEVICE_VPN_GATEWAY,
        address: Address::V4([203, 0, 113, 1]),
    },
    Device {
        kind: Kind::Tower(Emblem::Gear),
        at: at(9, 2),
        name: k::PACKETS_DEVICE_APP_SERVER,
        address: Address::V4([10, 0, 0, 20]),
    },
    Device {
        kind: Kind::Desktop,
        at: at(1, -6),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::V4([198, 51, 100, 66]),
    },
];

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_WIREGUARD_1_TITLE,
        sub: k::PACKETS_WIREGUARD_1_SUB,
        body: k::PACKETS_WIREGUARD_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TUNNEL, "wg0 10.8.0.2/32"),
            wire(k::PACKETS_LABEL_ROUTE, "AllowedIPs = 10.0.0.0/24"),
            wire(k::PACKETS_LABEL_PEER, "Endpoint = 203.0.113.1:51820"),
        ],
        focus: &[LAPTOP, ROUTER, INTERNET, GATEWAY],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "wg-quick up wg0",
            },
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: GATEWAY,
                prop: Some(Prop::Key),
            },
            Beat::Rest,
        ],
    },
    Step {
        title: k::PACKETS_WIREGUARD_2_TITLE,
        sub: k::PACKETS_WIREGUARD_2_SUB,
        body: k::PACKETS_WIREGUARD_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "UDP 51820"),
            wire(k::PACKETS_LABEL_SIZE, "148 B"),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "Noise IK, Curve25519"),
        ],
        focus: &[LAPTOP, ROUTER, INTERNET, GATEWAY],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "ping 10.0.0.20",
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GATEWAY,
                    Class::Handshake,
                    "WireGuard",
                    "Initiation",
                    "Handshake Initiation, sender=0x1c4f8a2e",
                    190,
                )
                .says(k::PACKETS_WIREGUARD_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "176",
                            "0x3b1a",
                            "000",
                            "0",
                            "64",
                            "17",
                            "0x7c55",
                            "192.168.1.10",
                            "203.0.113.1",
                        ],
                        &[9, 12],
                    ),
                    header(&anatomy::UDP, &["48712", "51820", "156", "0x5e1d"], &[1]),
                    header(
                        &anatomy::WIREGUARD_INIT,
                        &[
                            "1",
                            "0",
                            "0x1c4f8a2e",
                            "e3b0…9a41 (32 B)",
                            "5d27…c08e (48 B)",
                            "a9f1…3b62 (28 B)",
                            "0f3c…88d1 (16 B)",
                            "00…00 (16 B)",
                        ],
                        &[0, 3, 4, 5, 6],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WIREGUARD_3_TITLE,
        sub: k::PACKETS_WIREGUARD_3_SUB,
        body: k::PACKETS_WIREGUARD_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SIZE, "92 B"),
            wire(k::PACKETS_LABEL_SESSION, "0x1c4f8a2e ↔ 0x9d27e6b3"),
            wire(k::PACKETS_LABEL_ROUND_TRIPS, "1"),
        ],
        focus: &[LAPTOP, ROUTER, INTERNET, GATEWAY],
        beats: &[
            Beat::Light {
                at: GATEWAY,
                prop: Some(Prop::Check),
            },
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    LAPTOP,
                    Class::Handshake,
                    "WireGuard",
                    "Response",
                    "Handshake Response, sender=0x9d27e6b3, receiver=0x1c4f8a2e",
                    134,
                )
                .says(k::PACKETS_WIREGUARD_P2)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "120",
                            "0x0000",
                            "000",
                            "0",
                            "56",
                            "17",
                            "0x9d14",
                            "203.0.113.1",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["51820", "48712", "100", "0x7a02"], &[0, 2]),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_WIREGUARD_4_TITLE,
        sub: k::PACKETS_WIREGUARD_4_SUB,
        body: k::PACKETS_WIREGUARD_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ROUTE, "10.8.0.2 → 10.0.0.20"),
            wire(k::PACKETS_LABEL_ENCRYPTION, "ChaCha20-Poly1305"),
            wire(k::PACKETS_LABEL_SIZE, "84 B → 128 B"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GATEWAY,
                    Class::Data,
                    "WireGuard",
                    "Transport",
                    "Transport Data, receiver=0x9d27e6b3, counter=0",
                    170,
                )
                .says(k::PACKETS_WIREGUARD_P3)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "156",
                            "0x3b1b",
                            "000",
                            "0",
                            "64",
                            "17",
                            "0x7c3a",
                            "192.168.1.10",
                            "203.0.113.1",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["48712", "51820", "136", "0x2f90"], &[2]),
                    header(
                        &anatomy::WIREGUARD_DATA,
                        &[
                            "4",
                            "0",
                            "0x9d27e6b3",
                            "0",
                            "IPv4 10.8.0.2 → 10.0.0.20 ICMP 8 (84 B → 96 B) ‖ Poly1305 (16 B)",
                        ],
                        &[0, 2, 3, 4],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    APP,
                    Class::Data,
                    "ICMP",
                    "echo request",
                    "Echo (ping) request id=0x0003, seq=1/256, ttl=63",
                    98,
                )
                .says(k::PACKETS_WIREGUARD_P4)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "84",
                            "0x9e1f",
                            "010",
                            "0",
                            "63",
                            "1",
                            "0x8a4b",
                            "10.8.0.2",
                            "10.0.0.20",
                        ],
                        &[11, 12],
                    ),
                    header(
                        &anatomy::ICMP,
                        &["8", "0", "0x4d2a", "0x0003", "1", "56 B"],
                        &[0],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    APP,
                    GATEWAY,
                    Class::Data,
                    "ICMP",
                    "echo reply",
                    "Echo (ping) reply id=0x0003, seq=1/256, ttl=64",
                    98,
                )
                .says(k::PACKETS_WIREGUARD_P5)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "84",
                            "0x4c02",
                            "000",
                            "0",
                            "64",
                            "1",
                            "0x1b6e",
                            "10.0.0.20",
                            "10.8.0.2",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::ICMP,
                        &["0", "0", "0x552a", "0x0003", "1", "56 B"],
                        &[0],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    LAPTOP,
                    Class::Data,
                    "WireGuard",
                    "Transport",
                    "Transport Data, receiver=0x1c4f8a2e, counter=0",
                    170,
                )
                .says(k::PACKETS_WIREGUARD_P6)
                .headers(&[
                    header(&anatomy::UDP, &["51820", "48712", "136", "0xd4a3"], &[]),
                    header(
                        &anatomy::WIREGUARD_DATA,
                        &[
                            "4",
                            "0",
                            "0x1c4f8a2e",
                            "0",
                            "IPv4 10.0.0.20 → 10.8.0.2 ICMP 0 (84 B → 96 B) ‖ Poly1305 (16 B)",
                        ],
                        &[2, 4],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WIREGUARD_5_TITLE,
        sub: k::PACKETS_WIREGUARD_5_SUB,
        body: k::PACKETS_WIREGUARD_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_INTERVAL, "PersistentKeepalive = 25"),
            wire(k::PACKETS_WIREGUARD_LABEL_REKEY, "120 s"),
            wire(k::PACKETS_LABEL_SIZE, "32 B"),
        ],
        focus: &[LAPTOP, ROUTER, INTERNET, GATEWAY],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Clock),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GATEWAY,
                    Class::Control,
                    "WireGuard",
                    "Keepalive",
                    "Keepalive, receiver=0x9d27e6b3, counter=7",
                    74,
                )
                .says(k::PACKETS_WIREGUARD_P7)
                .headers(&[
                    header(&anatomy::UDP, &["48712", "51820", "40", "0x11c8"], &[2]),
                    header(
                        &anatomy::WIREGUARD_DATA,
                        &["4", "0", "0x9d27e6b3", "7", "Poly1305 (16 B)"],
                        &[3, 4],
                    ),
                ]),
            ),
            Beat::Light {
                at: ROUTER,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_WIREGUARD_6_TITLE,
        sub: k::PACKETS_WIREGUARD_6_SUB,
        body: k::PACKETS_WIREGUARD_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "198.51.100.66"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_WIREGUARD_6_VERDICT),
        ],
        focus: &[INTERNET, GATEWAY, ATTACKER],
        beats: &[
            Beat::Send(
                Packet::new(
                    ATTACKER,
                    GATEWAY,
                    Class::Refusal,
                    "WireGuard",
                    "Initiation",
                    "Handshake Initiation, sender=0x5e2b77a1",
                    190,
                )
                .says(k::PACKETS_WIREGUARD_P8)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "176",
                            "0x6d02",
                            "000",
                            "0",
                            "52",
                            "17",
                            "0x4e8b",
                            "198.51.100.66",
                            "203.0.113.1",
                        ],
                        &[11],
                    ),
                    header(&anatomy::UDP, &["40311", "51820", "156", "0x8b7e"], &[1]),
                    header(
                        &anatomy::WIREGUARD_INIT,
                        &[
                            "1",
                            "0",
                            "0x5e2b77a1",
                            "8c41…2fd0 (32 B)",
                            "b3e9…7a14 (48 B)",
                            "64d0…e1c9 (28 B)",
                            "1a7e…05bf (16 B)",
                            "00…00 (16 B)",
                        ],
                        &[6],
                    ),
                ]),
            ),
            Beat::Light {
                at: GATEWAY,
                prop: Some(Prop::Shield),
            },
            Beat::Rest,
        ],
    },
];

/// WireGuard.
pub static WIREGUARD: Protocol = Protocol {
    id: "wireguard",
    group: Group::Security,
    layer: Layer(3),
    name: k::PACKETS_WIREGUARD_NAME,
    title: k::PACKETS_WIREGUARD_TITLE,
    tagline: k::PACKETS_WIREGUARD_TAGLINE,
    overview: k::PACKETS_WIREGUARD_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "51820"),
        wire(k::PACKETS_FACT_TRANSPORT, "UDP"),
        wire(
            k::PACKETS_FACT_ENCRYPTION,
            "Noise_IKpsk2_25519_ChaChaPoly_BLAKE2s",
        ),
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_WIREGUARD_AUTHENTICATION,
        ),
        fact(k::PACKETS_FACT_DEFINED, k::PACKETS_WIREGUARD_DEFINED),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, GATEWAY, 6),
            link(GATEWAY, APP, 1),
            link(ATTACKER, INTERNET, 5),
        ],
    },
    steps: &STEPS,
};
