//! DNS: a name resolved from the root down — the resolver asking the root,
//! the `.com` servers and `example.com`'s own, and keeping the answer.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const RESOLVER: usize = 1;
const ROOT: usize = 2;
const TLD: usize = 3;
const AUTHORITATIVE: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Rack,
        at: at(-3, 0),
        name: k::PACKETS_DEVICE_RESOLVER,
        address: Address::V4([8, 8, 8, 8]),
    },
    Device {
        kind: Kind::Rack,
        at: at(2, -6),
        name: k::PACKETS_DEVICE_ROOT_SERVER,
        address: Address::V4([198, 41, 0, 4]),
    },
    Device {
        kind: Kind::Rack,
        at: at(7, -1),
        name: k::PACKETS_DEVICE_TLD_SERVER,
        address: Address::V4([192, 5, 6, 30]),
    },
    Device {
        kind: Kind::Rack,
        at: at(4, 5),
        name: k::PACKETS_DEVICE_AUTHORITATIVE,
        address: Address::V4([198, 51, 100, 53]),
    },
];

/// The resolver's question, the same at every level but for its ID.
macro_rules! question {
    ($id:literal, $lit:expr) => {
        header(
            &anatomy::DNS,
            &[
                $id,
                "0",
                "0",
                "0",
                "0",
                "0",
                "0",
                "0",
                "0",
                "1",
                "0",
                "0",
                "1",
                "www.example.com  A  IN",
            ],
            $lit,
        )
    };
}

/// An IPv4 header of the resolver's, towards `to`.
macro_rules! ip_from_resolver {
    ($to:literal) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", "72", "0x4e21", "000", "0", "64", "17", "0x93c0", "8.8.8.8",
                $to,
            ],
            &[12],
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_DNS_1_TITLE,
        sub: k::PACKETS_DNS_1_SUB,
        body: k::PACKETS_DNS_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_QUESTION, "www.example.com A"),
            wire(k::PACKETS_LABEL_RESOLVER, "8.8.8.8"),
            wire(k::PACKETS_LABEL_TRANSPORT, "UDP 53"),
        ],
        focus: &[CLIENT, RESOLVER],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "www.example.com",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    RESOLVER,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com",
                    "0x8a1c A? www.example.com",
                    75,
                )
                .says(k::PACKETS_DNS_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "61",
                            "0x8c11",
                            "000",
                            "0",
                            "64",
                            "17",
                            "0x2a91",
                            "192.168.1.10",
                            "8.8.8.8",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["51000", "53", "41", "0x9c2b"], &[1]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x8a1c",
                            "0",
                            "0",
                            "0",
                            "0",
                            "1",
                            "0",
                            "0",
                            "0",
                            "1",
                            "0",
                            "0",
                            "0",
                            "www.example.com  A  IN",
                        ],
                        &[0, 1, 5, 13],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DNS_2_TITLE,
        sub: k::PACKETS_DNS_2_SUB,
        body: k::PACKETS_DNS_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ASKS, "198.41.0.4"),
            wire(k::PACKETS_LABEL_REFERRAL, "com. NS a.gtld-servers.net"),
        ],
        focus: &[RESOLVER, ROOT],
        beats: &[
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    ROOT,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com",
                    "0x51d0 A? www.example.com",
                    86,
                )
                .says(k::PACKETS_DNS_P2)
                .headers(&[
                    ip_from_resolver!("198.41.0.4"),
                    header(&anatomy::UDP, &["40512", "53", "52", "0x11f0"], &[]),
                    question!("0x51d0", &[5]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    ROOT,
                    RESOLVER,
                    Class::Response,
                    "DNS",
                    "NS a.gtld-servers.net",
                    "0x51d0 com. NS a.gtld-servers.net",
                    882,
                )
                .says(k::PACKETS_DNS_P3)
                .headers(&[header(
                    &anatomy::DNS,
                    &[
                        "0x51d0",
                        "1",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "1",
                        "0",
                        "13",
                        "27",
                        "www.example.com  A  IN",
                        "com.  NS  a.gtld-servers.net … m.gtld-servers.net  (13 NS, 26 A/AAAA)",
                    ],
                    &[3, 10, 11, 14],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DNS_3_TITLE,
        sub: k::PACKETS_DNS_3_SUB,
        body: k::PACKETS_DNS_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ASKS, "192.5.6.30"),
            wire(k::PACKETS_LABEL_REFERRAL, "example.com. NS ns1.example.com"),
        ],
        focus: &[RESOLVER, TLD],
        beats: &[
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    TLD,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com",
                    "0x7c33 A? www.example.com",
                    86,
                )
                .says(k::PACKETS_DNS_P4)
                .headers(&[
                    ip_from_resolver!("192.5.6.30"),
                    header(&anatomy::UDP, &["40513", "53", "52", "0x0ae7"], &[]),
                    question!("0x7c33", &[]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    TLD,
                    RESOLVER,
                    Class::Response,
                    "DNS",
                    "NS ns1.example.com",
                    "0x7c33 example.com. NS ns1.example.com",
                    154,
                )
                .says(k::PACKETS_DNS_P5)
                .headers(&[header(
                    &anatomy::DNS,
                    &[
                        "0x7c33",
                        "1",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "1",
                        "0",
                        "2",
                        "3",
                        "www.example.com  A  IN",
                        "example.com.  NS  ns1.example.com, ns2.example.com  ns1.example.com.  A  198.51.100.53",
                    ],
                    &[11, 12, 14],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DNS_4_TITLE,
        sub: k::PACKETS_DNS_4_SUB,
        body: k::PACKETS_DNS_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ASKS, "198.51.100.53"),
            wire(k::PACKETS_LABEL_ANSWER, "A 203.0.113.10"),
            wire(k::PACKETS_LABEL_TTL, "3600"),
        ],
        focus: &[RESOLVER, AUTHORITATIVE],
        beats: &[
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    AUTHORITATIVE,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com",
                    "0x2b9e A? www.example.com",
                    86,
                )
                .says(k::PACKETS_DNS_P6)
                .headers(&[
                    ip_from_resolver!("198.51.100.53"),
                    header(&anatomy::UDP, &["40514", "53", "52", "0x5d31"], &[]),
                    question!("0x2b9e", &[]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    AUTHORITATIVE,
                    RESOLVER,
                    Class::Response,
                    "DNS",
                    "A 203.0.113.10",
                    "0x2b9e A www.example.com → 203.0.113.10",
                    102,
                )
                .says(k::PACKETS_DNS_P7)
                .headers(&[header(
                    &anatomy::DNS,
                    &[
                        "0x2b9e",
                        "1",
                        "0",
                        "1",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "1",
                        "1",
                        "0",
                        "1",
                        "www.example.com  A  IN",
                        "www.example.com  A  203.0.113.10  TTL 3600",
                    ],
                    &[3, 10, 14],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DNS_5_TITLE,
        sub: k::PACKETS_DNS_5_SUB,
        body: k::PACKETS_DNS_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ANSWER, "A 203.0.113.10"),
            fact(k::PACKETS_LABEL_CACHE, k::PACKETS_DNS_5_CACHE),
        ],
        focus: &[CLIENT, RESOLVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    CLIENT,
                    Class::Response,
                    "DNS",
                    "A 203.0.113.10",
                    "0x8a1c A www.example.com → 203.0.113.10",
                    91,
                )
                .says(k::PACKETS_DNS_P8)
                .headers(&[header(
                    &anatomy::DNS,
                    &[
                        "0x8a1c",
                        "1",
                        "0",
                        "0",
                        "0",
                        "1",
                        "1",
                        "0",
                        "0",
                        "1",
                        "1",
                        "0",
                        "0",
                        "www.example.com  A  IN",
                        "www.example.com  A  203.0.113.10  TTL 3600",
                    ],
                    &[6, 14],
                )]),
            ),
            Beat::Light {
                at: RESOLVER,
                prop: Some(Prop::Check),
            },
        ],
    },
];

/// DNS.
pub static DNS: Protocol = Protocol {
    id: "dns",
    group: Group::Naming,
    layer: Layer(7),
    name: k::PACKETS_DNS_NAME,
    title: k::PACKETS_DNS_TITLE,
    tagline: k::PACKETS_DNS_TAGLINE,
    overview: k::PACKETS_DNS_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "53"),
        fact(k::PACKETS_FACT_TRANSPORT, k::PACKETS_DNS_TRANSPORT),
        wire(k::PACKETS_FACT_RECORDS, "A, AAAA, CNAME, MX, NS, TXT"),
        wire(k::PACKETS_FACT_DEFINED, "RFC 1034, RFC 1035"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, RESOLVER, 8),
            link(RESOLVER, ROOT, 12),
            link(RESOLVER, TLD, 10),
            link(RESOLVER, AUTHORITATIVE, 14),
        ],
    },
    steps: &STEPS,
};
