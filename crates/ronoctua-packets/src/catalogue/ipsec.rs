//! IPsec: a laptop behind a home router joins an office network — IKEv2's
//! IKE_SA_INIT (proposals, Diffie-Hellman, nonces, NAT detection) and
//! IKE_AUTH (identities, the first CHILD_SA) moving to UDP 4500, ESP
//! carrying whole packets in tunnel mode both ways, and the CHILD_SA rekeyed.

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

static DEVICES: [Device; 5] = [
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
];

/// An IKEv2 header of this IKE SA — the laptop's SPI, then the gateway's,
/// next payload, version 2.0, exchange type, flags (0x08 the original
/// initiator, 0x20 a response), message ID, length — and its payloads.
macro_rules! ike {
    ($r:literal, $next:literal, $exchange:literal, $flags:literal, $mid:literal, $len:literal, $payloads:literal,
     $lit:expr) => {
        header(
            &anatomy::IKE,
            &[
                "0x3a9f1c2b7d4e8a10",
                $r,
                $next,
                "0x20",
                $exchange,
                $flags,
                $mid,
                $len,
                $payloads,
            ],
            $lit,
        )
    };
}

/// The gateway's SPI, once it has answered.
macro_rules! ike_sa {
    ($next:literal, $exchange:literal, $flags:literal, $mid:literal, $len:literal, $payloads:literal, $lit:expr) => {
        ike!(
            "0xc81d55e02f4a9b37",
            $next,
            $exchange,
            $flags,
            $mid,
            $len,
            $payloads,
            $lit
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_IPSEC_1_TITLE,
        sub: k::PACKETS_IPSEC_1_SUB,
        body: k::PACKETS_IPSEC_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "UDP 500"),
            wire(k::PACKETS_LABEL_ALGORITHM, "ENCR_AES_GCM_16 (256), PRF_HMAC_SHA2_256"),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "Curve25519 (group 31)"),
        ],
        focus: &[LAPTOP, ROUTER, INTERNET, GATEWAY],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "swanctl --initiate --child office",
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GATEWAY,
                    Class::Handshake,
                    "ISAKMP",
                    "IKE_SA_INIT",
                    "IKE_SA_INIT MID=00 Initiator Request",
                    264,
                )
                .says(k::PACKETS_IPSEC_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "250", "0x51c3", "000", "0", "64", "17", "0x2e7a", "192.168.1.10",
                            "203.0.113.1",
                        ],
                        &[9],
                    ),
                    header(&anatomy::UDP, &["500", "500", "230", "0x6a1f"], &[0, 1]),
                    ike!(
                        "0x0000000000000000",
                        "33",
                        "34",
                        "0x08",
                        "0",
                        "222",
                        "SA(ENCR_AES_GCM_16-256, PRF_HMAC_SHA2_256, 31) KE(31) Ni N(NAT_DETECTION_SOURCE_IP) \
                         N(NAT_DETECTION_DESTINATION_IP) N(IKEV2_FRAGMENTATION_SUPPORTED) N(SIGNATURE_HASH_ALGORITHMS)",
                        &[0, 4, 5, 8]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    LAPTOP,
                    Class::Handshake,
                    "ISAKMP",
                    "IKE_SA_INIT",
                    "IKE_SA_INIT MID=00 Responder Response",
                    289,
                )
                .says(k::PACKETS_IPSEC_P2)
                .headers(&[
                    header(&anatomy::UDP, &["500", "500", "255", "0x0b94"], &[]),
                    ike_sa!(
                        "33",
                        "34",
                        "0x20",
                        "0",
                        "247",
                        "SA(ENCR_AES_GCM_16-256, PRF_HMAC_SHA2_256, 31) KE(31) Nr N(NAT_DETECTION_SOURCE_IP) \
                         N(NAT_DETECTION_DESTINATION_IP) CERTREQ N(IKEV2_FRAGMENTATION_SUPPORTED) \
                         N(SIGNATURE_HASH_ALGORITHMS)",
                        &[1, 5, 8]
                    ),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: GATEWAY,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_IPSEC_2_TITLE,
        sub: k::PACKETS_IPSEC_2_SUB,
        body: k::PACKETS_IPSEC_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "UDP 4500"),
            wire(k::PACKETS_LABEL_ADDRESS, "10.8.0.2"),
            wire(k::PACKETS_IPSEC_LABEL_SELECTORS, "10.8.0.2/32 ↔ 10.0.0.0/24"),
        ],
        focus: &[LAPTOP, ROUTER, INTERNET, GATEWAY],
        beats: &[
            Beat::Light { at: ROUTER, prop: None },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GATEWAY,
                    Class::Crypto,
                    "ISAKMP",
                    "IKE_AUTH",
                    "IKE_AUTH MID=01 Initiator Request",
                    1_258,
                )
                .says(k::PACKETS_IPSEC_P3)
                .headers(&[
                    header(&anatomy::UDP, &["4500", "4500", "1224", "0x93d0"], &[0, 1]),
                    ike_sa!(
                        "46",
                        "35",
                        "0x08",
                        "1",
                        "1212",
                        "SK: IDi CERT CERTREQ AUTH CP(CFG_REQUEST: INTERNAL_IP4_ADDRESS) SA TSi TSr \
                         N(MOBIKE_SUPPORTED)",
                        &[2, 4, 8]
                    ),
                ]),
            ),
            Beat::Light {
                at: GATEWAY,
                prop: Some(Prop::Certificate),
            },
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    LAPTOP,
                    Class::Crypto,
                    "ISAKMP",
                    "IKE_AUTH",
                    "IKE_AUTH MID=01 Responder Response",
                    1_226,
                )
                .says(k::PACKETS_IPSEC_P4)
                .headers(&[
                    header(&anatomy::UDP, &["4500", "4500", "1192", "0x47be"], &[]),
                    ike_sa!(
                        "46",
                        "35",
                        "0x20",
                        "1",
                        "1180",
                        "SK: IDr CERT AUTH CP(CFG_REPLY: INTERNAL_IP4_ADDRESS=10.8.0.2) SA TSi(10.8.0.2/32) \
                         TSr(10.0.0.0/24) N(MOBIKE_SUPPORTED)",
                        &[5, 8]
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
        title: k::PACKETS_IPSEC_3_TITLE,
        sub: k::PACKETS_IPSEC_3_SUB,
        body: k::PACKETS_IPSEC_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SESSION, "SPI=0xcf3a8d21"),
            wire(k::PACKETS_LABEL_ENCRYPTION, "AES-GCM-256, ICV 16 B"),
            fact(k::PACKETS_LABEL_MODE, k::PACKETS_IPSEC_3_MODE),
        ],
        focus: &[],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "https://10.0.0.20/",
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GATEWAY,
                    Class::Data,
                    "ESP",
                    "ESP seq=1",
                    "ESP (SPI=0xcf3a8d21)",
                    138,
                )
                .says(k::PACKETS_IPSEC_P5)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "124", "0x51c9", "000", "0", "64", "17", "0x2e01", "192.168.1.10",
                            "203.0.113.1",
                        ],
                        &[9],
                    ),
                    header(&anatomy::UDP, &["4500", "4500", "104", "0x0000"], &[1]),
                    header(
                        &anatomy::ESP,
                        &[
                            "0xcf3a8d21",
                            "1",
                            "IV 5f0a…c812 (8 B) ‖ IPv4 10.8.0.2 → 10.0.0.20 TCP 50112 → 443 [SYN] (60 B) ‖ 01 02",
                            "2",
                            "4",
                            "3e9d…07a5 (16 B)",
                        ],
                        &[0, 1, 4, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    APP,
                    Class::Data,
                    "TCP",
                    "SYN",
                    "50112 → 443 [SYN] Seq=0 Win=64240 MSS=1460",
                    74,
                )
                .says(k::PACKETS_IPSEC_P6)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "60", "0x7d1e", "010", "0", "63", "6", "0x9c40", "10.8.0.2", "10.0.0.20",
                        ],
                        &[11, 12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "50112", "443", "0", "0", "10", "0", "0", "0", "0", "0", "0", "0", "1", "0", "64240",
                            "0x5a7c", "0",
                        ],
                        &[12],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_IPSEC_4_TITLE,
        sub: k::PACKETS_IPSEC_4_SUB,
        body: k::PACKETS_IPSEC_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SESSION, "SPI=0xc6e09b14"),
            wire(k::PACKETS_LABEL_SEQ, "1"),
            wire(k::PACKETS_LABEL_INTERVAL, "20 s"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    APP,
                    GATEWAY,
                    Class::Data,
                    "TCP",
                    "SYN-ACK",
                    "443 → 50112 [SYN, ACK] Seq=0 Ack=1 Win=65160 MSS=1460",
                    74,
                )
                .says(k::PACKETS_IPSEC_P7)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "60", "0x0000", "010", "0", "64", "6", "0x1a5d", "10.0.0.20", "10.8.0.2",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "443", "50112", "0", "1", "10", "0", "0", "0", "0", "1", "0", "0", "1", "0", "65160",
                            "0xe2b1", "0",
                        ],
                        &[9, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    LAPTOP,
                    Class::Data,
                    "ESP",
                    "ESP seq=1",
                    "ESP (SPI=0xc6e09b14)",
                    138,
                )
                .says(k::PACKETS_IPSEC_P8)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "124", "0x0000", "000", "0", "56", "17", "0x6b2c", "203.0.113.1",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["4500", "4500", "104", "0x0000"], &[]),
                    header(
                        &anatomy::ESP,
                        &[
                            "0xc6e09b14",
                            "1",
                            "IV 9a07…3e5b (8 B) ‖ IPv4 10.0.0.20 → 10.8.0.2 TCP 443 → 50112 [SYN, ACK] (60 B) ‖ 01 02",
                            "2",
                            "4",
                            "b71c…4e2d (16 B)",
                        ],
                        &[0, 1],
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
        title: k::PACKETS_IPSEC_5_TITLE,
        sub: k::PACKETS_IPSEC_5_SUB,
        body: k::PACKETS_IPSEC_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_INTERVAL, "3600 s"),
            wire(k::PACKETS_LABEL_SESSION, "SPI=0xc2b75e08, SPI=0xd417a3c9"),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "KE(31)"),
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
                    Class::Crypto,
                    "ISAKMP",
                    "CREATE_CHILD_SA",
                    "CREATE_CHILD_SA MID=02 Initiator Request",
                    282,
                )
                .says(k::PACKETS_IPSEC_P9)
                .headers(&[
                    header(&anatomy::UDP, &["4500", "4500", "248", "0x5c31"], &[]),
                    ike_sa!(
                        "46",
                        "36",
                        "0x08",
                        "2",
                        "236",
                        "SK: N(REKEY_SA, SPI=0xc6e09b14) SA Ni KEi(31) TSi TSr",
                        &[4, 8]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    LAPTOP,
                    Class::Crypto,
                    "ISAKMP",
                    "CREATE_CHILD_SA",
                    "CREATE_CHILD_SA MID=02 Responder Response",
                    274,
                )
                .says(k::PACKETS_IPSEC_P10)
                .headers(&[ike_sa!(
                    "46",
                    "36",
                    "0x20",
                    "2",
                    "228",
                    "SK: SA Nr KEr(31) TSi TSr",
                    &[8]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    GATEWAY,
                    Class::Control,
                    "ISAKMP",
                    "INFORMATIONAL",
                    "INFORMATIONAL MID=03 Initiator Request",
                    122,
                )
                .says(k::PACKETS_IPSEC_P11)
                .headers(&[ike_sa!(
                    "46",
                    "37",
                    "0x08",
                    "3",
                    "76",
                    "SK: D(ESP, SPI=0xc6e09b14)",
                    &[4, 8]
                )]),
            ),
        ],
    },
];

/// IPsec: IKEv2 and ESP.
pub static IPSEC: Protocol = Protocol {
    id: "ipsec",
    group: Group::Security,
    layer: Layer(3),
    name: k::PACKETS_IPSEC_NAME,
    title: k::PACKETS_IPSEC_TITLE,
    tagline: k::PACKETS_IPSEC_TAGLINE,
    overview: k::PACKETS_IPSEC_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "UDP 500, UDP 4500"),
        fact(k::PACKETS_FACT_TRANSPORT, k::PACKETS_IPSEC_TRANSPORT),
        wire(k::PACKETS_FACT_ENCRYPTION, "AES-GCM, ChaCha20-Poly1305"),
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_IPSEC_AUTHENTICATION,
        ),
        wire(k::PACKETS_FACT_DEFINED, "RFC 7296, RFC 4303, RFC 3948"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, GATEWAY, 6),
            link(GATEWAY, APP, 1),
        ],
    },
    steps: &STEPS,
};
