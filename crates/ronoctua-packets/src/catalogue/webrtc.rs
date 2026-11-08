//! WebRTC: a video call between two browsers — offer and answer through a
//! signalling server, a public address learnt from STUN, a TURN relay kept
//! in reserve, connectivity checks across both routers, DTLS, then SRTP
//! flowing peer to peer.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const ROUTER: usize = 1;
const SIGNALLING: usize = 2;
const THEIR_ROUTER: usize = 3;
const THEIR_LAPTOP: usize = 4;
const STUN: usize = 5;
const TURN: usize = 6;

static DEVICES: [Device; 7] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-5, -1),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([198, 51, 100, 7]),
    },
    Device {
        kind: Kind::Tower(Emblem::Hub),
        at: at(0, -6),
        name: k::PACKETS_DEVICE_SIGNALLING,
        address: Address::V4([203, 0, 113, 20]),
    },
    Device {
        kind: Kind::Router,
        at: at(5, -1),
        name: k::PACKETS_DEVICE_THEIR_ROUTER,
        address: Address::V4([192, 0, 2, 77]),
    },
    Device {
        kind: Kind::Laptop,
        at: at(9, 3),
        name: k::PACKETS_DEVICE_THEIR_LAPTOP,
        address: Address::V4([10, 0, 0, 23]),
    },
    Device {
        kind: Kind::Tower(Emblem::Mirror),
        at: at(-3, 5),
        name: k::PACKETS_DEVICE_STUN_SERVER,
        address: Address::V4([203, 0, 113, 40]),
    },
    Device {
        kind: Kind::Tower(Emblem::Arrows),
        at: at(3, 5),
        name: k::PACKETS_DEVICE_TURN_SERVER,
        address: Address::V4([203, 0, 113, 30]),
    },
];

/// A line of SDP, lit.
macro_rules! sdp {
    ($line:literal) => {
        header(&anatomy::LINE, &[$line], &[0])
    };
}

/// An IPv4 header carrying UDP: total length, identification, TTL,
/// checksum, source, destination.
macro_rules! ip_udp {
    ($len:literal, $id:literal, $ttl:literal, $sum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", $len, $id, "000", "0", $ttl, "17", $sum, $src, $dst,
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_WEBRTC_1_TITLE,
        sub: k::PACKETS_WEBRTC_1_SUB,
        body: k::PACKETS_WEBRTC_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CHANNEL, "wss://signal.example.com"),
            wire(k::PACKETS_LABEL_MEDIA, "m=audio, m=video"),
            wire(k::PACKETS_LABEL_CERTIFICATE, "a=fingerprint:sha-256"),
        ],
        focus: &[LAPTOP, ROUTER, SIGNALLING, THEIR_ROUTER, THEIR_LAPTOP],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SIGNALLING,
                    Class::Handshake,
                    "SDP",
                    "offer",
                    "type=offer m=audio m=video a=setup:actpass",
                    4_862,
                )
                .says(k::PACKETS_WEBRTC_P1)
                .headers(&[
                    sdp!("m=audio 9 UDP/TLS/RTP/SAVPF 111"),
                    sdp!("m=video 9 UDP/TLS/RTP/SAVPF 96 97"),
                    sdp!("a=ice-ufrag:F7gI"),
                    sdp!("a=fingerprint:sha-256 4A:AD:B9:B1:3F:82:18:3B:26:12:…"),
                    sdp!("a=setup:actpass"),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SIGNALLING,
                    THEIR_LAPTOP,
                    Class::Handshake,
                    "SDP",
                    "offer",
                    "type=offer m=audio m=video a=setup:actpass",
                    4_862,
                )
                .says(k::PACKETS_WEBRTC_P2),
            ),
            Beat::Send(
                Packet::new(
                    THEIR_LAPTOP,
                    SIGNALLING,
                    Class::Handshake,
                    "SDP",
                    "answer",
                    "type=answer m=audio m=video a=setup:active",
                    4_410,
                )
                .says(k::PACKETS_WEBRTC_P3)
                .headers(&[
                    sdp!("a=ice-ufrag:x9Kq"),
                    sdp!("a=fingerprint:sha-256 D1:07:5C:E2:9A:40:6B:F3:11:8E:…"),
                    sdp!("a=setup:active"),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SIGNALLING,
                    LAPTOP,
                    Class::Handshake,
                    "SDP",
                    "answer",
                    "type=answer m=audio m=video a=setup:active",
                    4_410,
                )
                .says(k::PACKETS_WEBRTC_P4),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WEBRTC_2_TITLE,
        sub: k::PACKETS_WEBRTC_2_SUB,
        body: k::PACKETS_WEBRTC_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PRIVATE, "192.168.1.10:50000"),
            wire(k::PACKETS_LABEL_PUBLIC, "198.51.100.7:54321"),
            wire(k::PACKETS_LABEL_CANDIDATE, "typ srflx"),
        ],
        focus: &[LAPTOP, ROUTER, STUN],
        beats: &[
            Beat::Send(
                Packet::new(LAPTOP, STUN, Class::Lookup, "STUN", "Binding Request", "Binding Request", 62)
                    .says(k::PACKETS_WEBRTC_P5)
                    .headers(&[
                        ip_udp!("48", "0x7d01", "64", "0xffe0", "192.168.1.10", "203.0.113.40", &[11, 12]),
                        header(&anatomy::UDP, &["50000", "3478", "28", "0x5e1a"], &[0, 1]),
                    ]),
            ),
            Beat::Send(
                Packet::new(
                    STUN,
                    LAPTOP,
                    Class::Lookup,
                    "STUN",
                    "198.51.100.7:54321",
                    "Binding Success Response XOR-MAPPED-ADDRESS: 198.51.100.7:54321",
                    74,
                )
                .says(k::PACKETS_WEBRTC_P6)
                .headers(&[
                    ip_udp!("60", "0x0000", "55", "0x85d6", "203.0.113.40", "192.168.1.10", &[]),
                    header(&anatomy::UDP, &["3478", "50000", "40", "0x93b1"], &[]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WEBRTC_3_TITLE,
        sub: k::PACKETS_WEBRTC_3_SUB,
        body: k::PACKETS_WEBRTC_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_RELAY, "203.0.113.30:49152"),
            wire(k::PACKETS_LABEL_CANDIDATE, "typ relay"),
            wire(k::PACKETS_LABEL_LEASE, "LIFETIME 600 s"),
        ],
        focus: &[LAPTOP, ROUTER, TURN],
        beats: &[
            Beat::Send(
                Packet::new(LAPTOP, TURN, Class::Handshake, "TURN", "Allocate", "Allocate Request UDP", 170)
                    .says(k::PACKETS_WEBRTC_P7)
                    .headers(&[
                        ip_udp!("156", "0x7d02", "64", "0xff7d", "192.168.1.10", "203.0.113.30", &[12]),
                        header(&anatomy::UDP, &["50000", "3478", "136", "0x2c47"], &[1]),
                    ]),
            ),
            Beat::Send(
                Packet::new(
                    TURN,
                    LAPTOP,
                    Class::Handshake,
                    "TURN",
                    "203.0.113.30:49152",
                    "Allocate Success Response XOR-RELAYED-ADDRESS: 203.0.113.30:49152 LIFETIME: 600",
                    126,
                )
                .says(k::PACKETS_WEBRTC_P8)
                .headers(&[
                    ip_udp!("112", "0x0000", "56", "0x84ac", "203.0.113.30", "192.168.1.10", &[11]),
                    header(&anatomy::UDP, &["3478", "50000", "92", "0x71e0"], &[0]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WEBRTC_4_TITLE,
        sub: k::PACKETS_WEBRTC_4_SUB,
        body: k::PACKETS_WEBRTC_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "192.168.1.10:50000"),
            wire(k::PACKETS_LABEL_DESTINATION, "192.0.2.77:61000"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_WEBRTC_4_VERDICT),
        ],
        focus: &[LAPTOP, ROUTER, THEIR_ROUTER, THEIR_LAPTOP],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    THEIR_LAPTOP,
                    Class::Handshake,
                    "STUN",
                    "Binding Request",
                    "Binding Request user: x9Kq:F7gI",
                    142,
                )
                .says(k::PACKETS_WEBRTC_P9)
                .headers(&[
                    ip_udp!("128", "0x7d03", "64", "0x796a", "192.168.1.10", "192.0.2.77", &[12]),
                    header(&anatomy::UDP, &["50000", "61000", "108", "0xa3c4"], &[0, 1]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    THEIR_LAPTOP,
                    LAPTOP,
                    Class::Handshake,
                    "STUN",
                    "Binding Success",
                    "Binding Success Response XOR-MAPPED-ADDRESS: 198.51.100.7:54321",
                    106,
                )
                .says(k::PACKETS_WEBRTC_P10)
                .headers(&[
                    ip_udp!("92", "0x41aa", "53", "0xbfe7", "192.0.2.77", "192.168.1.10", &[11]),
                    header(&anatomy::UDP, &["61000", "50000", "72", "0x0d52"], &[0]),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
            Beat::Light {
                at: THEIR_LAPTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_WEBRTC_5_TITLE,
        sub: k::PACKETS_WEBRTC_5_SUB,
        body: k::PACKETS_WEBRTC_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CERTIFICATE, "sha-256 D1:07:5C:E2:9A:40:…"),
            fact(k::PACKETS_LABEL_TRUST, k::PACKETS_WEBRTC_5_TRUST),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "DTLS-SRTP"),
        ],
        focus: &[LAPTOP, ROUTER, THEIR_ROUTER, THEIR_LAPTOP],
        beats: &[
            Beat::Send(
                Packet::new(THEIR_LAPTOP, LAPTOP, Class::Crypto, "DTLS", "Client Hello", "Client Hello", 202)
                    .says(k::PACKETS_WEBRTC_P11)
                    .headers(&[
                        ip_udp!("188", "0x41ab", "53", "0xbf86", "192.0.2.77", "192.168.1.10", &[]),
                        header(&anatomy::UDP, &["61000", "50000", "168", "0x5b19"], &[0, 1]),
                    ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Certificate),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    THEIR_LAPTOP,
                    Class::Crypto,
                    "DTLS",
                    "Server Hello, Certificate",
                    "Server Hello, Certificate, Server Key Exchange, Certificate Request, Server Hello Done",
                    912,
                )
                .says(k::PACKETS_WEBRTC_P12)
                .headers(&[
                    ip_udp!("898", "0x7d04", "64", "0x7667", "192.168.1.10", "192.0.2.77", &[]),
                    header(&anatomy::UDP, &["50000", "61000", "878", "0xe6a0"], &[2]),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Lock),
            },
            Beat::Light {
                at: THEIR_LAPTOP,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_WEBRTC_6_TITLE,
        sub: k::PACKETS_WEBRTC_6_SUB,
        body: k::PACKETS_WEBRTC_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_MEDIA, "Opus, VP8"),
            wire(k::PACKETS_LABEL_ENCRYPTION, "SRTP"),
            wire(k::PACKETS_LABEL_TRANSPORT, "UDP 50000 ⇄ 61000"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    THEIR_LAPTOP,
                    Class::Data,
                    "SRTP",
                    "Opus",
                    "PT=111 SSRC=0x5c3a91f2 Seq=24817 Time=3840",
                    154,
                )
                .says(k::PACKETS_WEBRTC_P13)
                .headers(&[
                    ip_udp!("140", "0x7d05", "64", "0x795c", "192.168.1.10", "192.0.2.77", &[12]),
                    header(&anatomy::UDP, &["50000", "61000", "120", "0x48d3"], &[2]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    THEIR_LAPTOP,
                    LAPTOP,
                    Class::Data,
                    "SRTP",
                    "VP8",
                    "PT=96 SSRC=0x1e4b77a0 Seq=9312 Time=270000",
                    1_222,
                )
                .says(k::PACKETS_WEBRTC_P14)
                .headers(&[
                    ip_udp!("1208", "0x41ac", "53", "0xbb89", "192.0.2.77", "192.168.1.10", &[11]),
                    header(&anatomy::UDP, &["61000", "50000", "1188", "0x9e07"], &[2]),
                ]),
            ),
        ],
    },
];

/// WebRTC.
pub static WEBRTC: Protocol = Protocol {
    id: "webrtc",
    group: Group::Web,
    layer: Layer(7),
    name: k::PACKETS_WEBRTC_NAME,
    title: k::PACKETS_WEBRTC_TITLE,
    tagline: k::PACKETS_WEBRTC_TAGLINE,
    overview: k::PACKETS_WEBRTC_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_WEBRTC_PORT),
        fact(k::PACKETS_FACT_TRANSPORT, k::PACKETS_WEBRTC_TRANSPORT),
        fact(k::PACKETS_FACT_ENCRYPTION, k::PACKETS_WEBRTC_ENCRYPTION),
        wire(k::PACKETS_FACT_STANDARD, "W3C WebRTC 1.0"),
        wire(
            k::PACKETS_FACT_DEFINED,
            "RFC 8825, RFC 8445, RFC 8489, RFC 8656",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(ROUTER, SIGNALLING, 12),
            link(SIGNALLING, THEIR_ROUTER, 12),
            link(ROUTER, STUN, 8),
            link(ROUTER, TURN, 10),
            link(TURN, THEIR_ROUTER, 10),
            link(ROUTER, THEIR_ROUTER, 15),
            link(THEIR_ROUTER, THEIR_LAPTOP, 1),
        ],
    },
    steps: &STEPS,
};
