//! mDNS and DNS-SD: a laptop finds the printer on its network with no DNS
//! server — one multicast question, the printer's own answer, kept in the
//! cache — and a desktop that joins, claims its `.local` name and
//! announces itself.
//!
//! The multicast group is drawn as the local network every device hangs
//! from: what is sent to 224.0.0.251 stops there, as every device on the
//! network hears it.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const LAN: usize = 1;
const PRINTER: usize = 2;
const DESKTOP: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Switch,
        at: at(-2, -1),
        name: k::PACKETS_MDNS_DEVICE_LAN,
        address: Address::V4([224, 0, 0, 251]),
    },
    Device {
        kind: Kind::Printer,
        at: at(5, -4),
        name: k::PACKETS_DEVICE_PRINTER,
        address: Address::V4([192, 168, 1, 40]),
    },
    Device {
        kind: Kind::Desktop,
        at: at(5, 4),
        name: k::PACKETS_DEVICE_DESKTOP,
        address: Address::V4([192, 168, 1, 30]),
    },
];

/// An IPv4 header of an mDNS packet: to the group, with the TTL of 255
/// every mDNS packet is sent with — total length, checksum, source.
macro_rules! ip {
    ($len:literal, $sum:literal, $src:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4",
                "5",
                "0",
                "0",
                $len,
                "0x0000",
                "000",
                "0",
                "255",
                "17",
                $sum,
                $src,
                "224.0.0.251",
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_MDNS_1_TITLE,
        sub: k::PACKETS_MDNS_1_SUB,
        body: k::PACKETS_MDNS_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_QUESTION, "_ipp._tcp.local PTR"),
            wire(k::PACKETS_LABEL_MULTICAST, "224.0.0.251:5353"),
        ],
        focus: &[LAPTOP, LAN, PRINTER],
        beats: &[Beat::Send(
            Packet::new(
                LAPTOP,
                LAN,
                Class::Lookup,
                "MDNS",
                "PTR? _ipp._tcp.local",
                "0x0000 PTR? _ipp._tcp.local QM",
                75,
            )
            .says(k::PACKETS_MDNS_P1)
            .headers(&[
                header(
                    &anatomy::IPV4,
                    &[
                        "4", "5", "0", "0", "61", "0x3a01", "000", "0", "255", "17", "0xdf00", "192.168.1.10",
                        "224.0.0.251",
                    ],
                    &[8, 12],
                ),
                header(&anatomy::UDP, &["5353", "5353", "41", "0x4b1e"], &[0, 1]),
                header(
                    &anatomy::DNS,
                    &[
                        "0x0000",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "000",
                        "0",
                        "1",
                        "0",
                        "0",
                        "0",
                        "_ipp._tcp.local  PTR  IN  QM",
                    ],
                    &[0, 13],
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_MDNS_2_TITLE,
        sub: k::PACKETS_MDNS_2_SUB,
        body: k::PACKETS_MDNS_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ANSWER, "printer._ipp._tcp.local"),
            wire(k::PACKETS_LABEL_PORT, "631"),
            wire(k::PACKETS_LABEL_ADDRESS, "192.168.1.40"),
        ],
        focus: &[LAPTOP, LAN, PRINTER],
        beats: &[
            Beat::Send(
                Packet::new(
                    PRINTER,
                    LAN,
                    Class::Response,
                    "MDNS",
                    "PTR + SRV + TXT + A",
                    "0x0000 PTR printer._ipp._tcp.local SRV 0 0 631 printer.local TXT A 192.168.1.40",
                    190,
                )
                .says(k::PACKETS_MDNS_P2)
                .headers(&[
                    ip!("176", "0x1871", "192.168.1.40", &[11, 12]),
                    header(&anatomy::UDP, &["5353", "5353", "156", "0x8d40"], &[]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x0000",
                            "1",
                            "0",
                            "1",
                            "0",
                            "0",
                            "0",
                            "000",
                            "0",
                            "0",
                            "1",
                            "0",
                            "3",
                            "",
                            "_ipp._tcp.local  PTR  printer._ipp._tcp.local  TTL 4500  ·  printer._ipp._tcp.local  SRV 0 0 631 printer.local  ·  TXT txtvers=1 rp=ipp/print  ·  printer.local  A  192.168.1.40",
                        ],
                        &[3, 10, 12, 14],
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
        title: k::PACKETS_MDNS_3_TITLE,
        sub: k::PACKETS_MDNS_3_SUB,
        body: k::PACKETS_MDNS_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TTL, "PTR 4500, TXT 4500, SRV 120, A 120"),
            wire(k::PACKETS_LABEL_DESTINATION, "192.168.1.40:631"),
        ],
        focus: &[LAPTOP, LAN, PRINTER],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Clock),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    PRINTER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "52610 → 631 [SYN] Seq=0 Win=64240",
                    66,
                )
                .says(k::PACKETS_MDNS_P3)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "52", "0x3a02", "010", "0", "64", "6", "0x7d3f", "192.168.1.10",
                            "192.168.1.40",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "52610", "631", "0", "0", "8", "0", "0", "0", "0", "0", "0", "0", "1", "0", "64240", "0x6d21",
                            "0",
                        ],
                        &[1, 12],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MDNS_4_TITLE,
        sub: k::PACKETS_MDNS_4_SUB,
        body: k::PACKETS_MDNS_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_QUESTION, "desktop.local ANY"),
            wire(k::PACKETS_LABEL_INTERVAL, "250 ms"),
        ],
        focus: &[LAN, PRINTER, DESKTOP],
        beats: &[
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    LAN,
                    Class::Lookup,
                    "MDNS",
                    "ANY? desktop.local",
                    "0x0000 ANY? desktop.local QU, A 192.168.1.30",
                    89,
                )
                .says(k::PACKETS_MDNS_P4)
                .headers(&[
                    ip!("75", "0x18e0", "192.168.1.30", &[]),
                    header(&anatomy::UDP, &["5353", "5353", "55", "0x2f6a"], &[]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x0000",
                            "0",
                            "0",
                            "0",
                            "0",
                            "0",
                            "0",
                            "000",
                            "0",
                            "1",
                            "0",
                            "1",
                            "0",
                            "desktop.local  ANY  IN  QU",
                        ],
                        &[11, 13],
                    ),
                ]),
            ),
            Beat::Rest,
            Beat::Light {
                at: DESKTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_MDNS_5_TITLE,
        sub: k::PACKETS_MDNS_5_SUB,
        body: k::PACKETS_MDNS_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_RECORD, "desktop.local A 192.168.1.30"),
            wire(k::PACKETS_LABEL_PORT, "445"),
            wire(k::PACKETS_LABEL_INTERVAL, "1 s"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    LAN,
                    Class::Response,
                    "MDNS",
                    "A + PTR + SRV + TXT",
                    "0x0000 A 192.168.1.30 PTR desktop._smb._tcp.local SRV 0 0 445 desktop.local TXT",
                    153,
                )
                .says(k::PACKETS_MDNS_P5)
                .headers(&[
                    ip!("139", "0x18a0", "192.168.1.30", &[12]),
                    header(&anatomy::UDP, &["5353", "5353", "119", "0x51d7"], &[]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x0000",
                            "1",
                            "0",
                            "1",
                            "0",
                            "0",
                            "0",
                            "000",
                            "0",
                            "0",
                            "4",
                            "0",
                            "0",
                            "",
                            "desktop.local  A  192.168.1.30  TTL 120  ·  _smb._tcp.local  PTR  desktop._smb._tcp.local  TTL 4500  ·  desktop._smb._tcp.local  SRV 0 0 445 desktop.local  ·  TXT",
                        ],
                        &[3, 10, 14],
                    ),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
];

/// mDNS and DNS-SD.
pub static MDNS: Protocol = Protocol {
    id: "mdns",
    group: Group::Naming,
    layer: Layer(7),
    name: k::PACKETS_MDNS_NAME,
    title: k::PACKETS_MDNS_TITLE,
    tagline: k::PACKETS_MDNS_TAGLINE,
    overview: k::PACKETS_MDNS_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "5353"),
        wire(k::PACKETS_FACT_TRANSPORT, "UDP"),
        wire(k::PACKETS_FACT_ADDRESSING, "224.0.0.251, ff02::fb"),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_MDNS_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 6762, RFC 6763"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, LAN, 1),
            link(LAN, PRINTER, 1),
            link(LAN, DESKTOP, 1),
        ],
    },
    steps: &STEPS,
};
