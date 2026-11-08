//! QUIC: a connection and its TLS 1.3 keys agreed in one round trip over
//! UDP, HTTP/3 requests on independent streams, a loss that holds up only
//! its own stream, the connection following a phone from Wi-Fi to mobile
//! data, and 0-RTT when it comes back.
//!
//! The phone is drawn twice — on the Wi-Fi and, later, on mobile data —
//! because a packet flies along one way between two devices, and the point
//! of the network change is that the way changes while the connection does
//! not.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const PHONE: usize = 0;
const WIFI: usize = 1;
const MOVED: usize = 2;
const MOBILE: usize = 3;
const INTERNET: usize = 4;
const SERVER: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Phone,
        at: at(-9, -1),
        name: k::PACKETS_QUIC_DEVICE_PHONE_WIFI,
        address: Address::V4([192, 168, 1, 23]),
    },
    Device {
        kind: Kind::AccessPoint,
        at: at(-4, -5),
        name: k::PACKETS_DEVICE_ACCESS_POINT,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Phone,
        at: at(-9, 5),
        name: k::PACKETS_QUIC_DEVICE_PHONE_MOBILE,
        address: Address::V4([10, 64, 12, 7]),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(-4, 3),
        name: k::PACKETS_DEVICE_ISP_ROUTER,
        address: Address::V4([10, 64, 0, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(1, -1),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(7, -3),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// The connection ID the phone picked for itself — the server writes it as
/// the destination of everything it sends on the Wi-Fi.
const PHONE_CID: &str = "0xc1a5e0f2d47b3a96";
/// The server's — the phone's destination once the server has answered.
const SERVER_CID: &str = "0xf0a2b4c6d8e01357";
/// The random destination of the very first packet, from which the Initial
/// keys are derived (RFC 9001's own example).
const FIRST_CID: &str = "0x8394c8f03e515708";

/// An IPv4 header carrying UDP, with "don't fragment" set as QUIC asks:
/// total length, identification, TTL, checksum, source, destination.
macro_rules! ip {
    ($len:literal, $id:literal, $ttl:literal, $sum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", $len, $id, "010", "0", $ttl, "17", $sum, $src, $dst,
            ],
            $lit,
        )
    };
}

/// A UDP header: ports, length, checksum.
macro_rules! udp {
    ($src:literal, $dst:literal, $len:literal, $sum:literal, $lit:expr) => {
        header(&anatomy::UDP, &[$src, $dst, $len, $sum], $lit)
    };
}

/// A QUIC v1 Initial packet: the long header of type 0, with 8-byte
/// connection IDs, a token length of 0 and a 4-byte packet number.
macro_rules! initial {
    ($dcid:expr, $scid:expr, $len:literal, $pn:literal, $payload:literal, $lit:expr) => {
        header(
            &anatomy::QUIC_INITIAL,
            &[
                "1",
                "1",
                "0",
                "0",
                "3",
                "0x00000001",
                "8",
                $dcid,
                "8",
                $scid,
                "0",
                $len,
                $pn,
                $payload,
            ],
            $lit,
        )
    };
}

/// A QUIC v1 long header — 0-RTT (1) or Handshake (2) — with 8-byte
/// connection IDs and a 4-byte packet number.
macro_rules! long {
    ($kind:literal, $dcid:expr, $scid:expr, $len:literal, $pn:literal, $payload:literal, $lit:expr) => {
        header(
            &anatomy::QUIC_LONG,
            &[
                "1",
                "1",
                $kind,
                "0",
                "3",
                "0x00000001",
                "8",
                $dcid,
                "8",
                $scid,
                $len,
                $pn,
                $payload,
            ],
            $lit,
        )
    };
}

/// A QUIC short (1-RTT) header: no version, no source ID — just where it
/// goes, its number and the encrypted frames.
macro_rules! short {
    ($dcid:expr, $pn:literal, $payload:literal, $lit:expr) => {
        header(
            &anatomy::QUIC_SHORT,
            &["0", "1", "0", "0", "0", "3", $dcid, $pn, $payload],
            $lit,
        )
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_QUIC_1_TITLE,
        sub: k::PACKETS_QUIC_1_SUB,
        body: k::PACKETS_QUIC_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TRANSPORT, "UDP 443"),
            wire(k::PACKETS_LABEL_VERSION, "0x00000001"),
            wire(k::PACKETS_LABEL_SIZE, "1200 B"),
        ],
        focus: &[PHONE, WIFI, INTERNET, SERVER],
        beats: &[
            Beat::Type {
                at: PHONE,
                wire: "https://www.example.com/",
            },
            Beat::Send(
                Packet::new(
                    PHONE,
                    SERVER,
                    Class::Handshake,
                    "QUIC",
                    "Initial",
                    "Initial, DCID=8394c8f03e515708, SCID=c1a5e0f2d47b3a96, PKN: 0, CRYPTO, PADDING",
                    1_242,
                )
                .says(k::PACKETS_QUIC_P1)
                .headers(&[
                    ip!("1228", "0x5a1f", "64", "0xde37", "192.168.1.23", "203.0.113.10", &[9]),
                    udp!("50544", "443", "1208", "0x8e7b", &[1]),
                    initial!(
                        FIRST_CID,
                        PHONE_CID,
                        "1174",
                        "0",
                        "CRYPTO [ClientHello: SNI=www.example.com, ALPN=h3, key_share] + PADDING",
                        &[2, 5, 7, 13]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_QUIC_2_TITLE,
        sub: k::PACKETS_QUIC_2_SUB,
        body: k::PACKETS_QUIC_2_BODY,
        facts: &[
            wire(k::PACKETS_QUIC_LABEL_CID, "f0a2b4c6d8e01357"),
            wire(k::PACKETS_LABEL_CERTIFICATE, "www.example.com"),
        ],
        focus: &[PHONE, WIFI, INTERNET, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    PHONE,
                    Class::Crypto,
                    "QUIC",
                    "Initial + Handshake",
                    "Initial, DCID=c1a5e0f2d47b3a96, SCID=f0a2b4c6d8e01357, PKN: 0, ACK, CRYPTO; Handshake, PKN: 0, CRYPTO",
                    1_242,
                )
                .says(k::PACKETS_QUIC_P2)
                .headers(&[
                    ip!("1228", "0x0000", "52", "0x4457", "203.0.113.10", "192.168.1.23", &[]),
                    udp!("443", "50544", "1208", "0x1d4c", &[0]),
                    initial!(
                        PHONE_CID,
                        SERVER_CID,
                        "123",
                        "0",
                        "ACK + CRYPTO [ServerHello: key_share]",
                        &[7, 9, 13]
                    ),
                    long!(
                        "2",
                        PHONE_CID,
                        SERVER_CID,
                        "1026",
                        "0",
                        "CRYPTO [EncryptedExtensions, Certificate…]",
                        &[2]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    PHONE,
                    Class::Crypto,
                    "QUIC",
                    "Handshake",
                    "Handshake, DCID=c1a5e0f2d47b3a96, SCID=f0a2b4c6d8e01357, PKN: 1, CRYPTO",
                    1_204,
                )
                .says(k::PACKETS_QUIC_P3)
                .headers(&[
                    ip!("1190", "0x0000", "52", "0x447d", "203.0.113.10", "192.168.1.23", &[]),
                    udp!("443", "50544", "1170", "0x5b02", &[]),
                    long!(
                        "2",
                        PHONE_CID,
                        SERVER_CID,
                        "1137",
                        "1",
                        "CRYPTO […Certificate: www.example.com, CertificateVerify, Finished]",
                        &[2, 12]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_QUIC_3_TITLE,
        sub: k::PACKETS_QUIC_3_SUB,
        body: k::PACKETS_QUIC_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ROUND_TRIPS, "1"),
            wire(k::PACKETS_LABEL_STREAM, "0, 4"),
            wire(k::PACKETS_LABEL_REQUEST, "GET /, GET /logo.png"),
        ],
        focus: &[PHONE, WIFI, INTERNET, SERVER],
        beats: &[
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Certificate),
            },
            Beat::Send(
                Packet::new(
                    PHONE,
                    SERVER,
                    Class::Crypto,
                    "QUIC",
                    "Finished",
                    "Handshake, DCID=f0a2b4c6d8e01357, SCID=c1a5e0f2d47b3a96, PKN: 0, ACK, CRYPTO",
                    131,
                )
                .says(k::PACKETS_QUIC_P4)
                .headers(&[
                    ip!("117", "0x5a20", "64", "0xe28d", "192.168.1.23", "203.0.113.10", &[]),
                    udp!("50544", "443", "97", "0x0c61", &[]),
                    long!("2", SERVER_CID, PHONE_CID, "64", "0", "ACK + CRYPTO [Finished]", &[7, 12]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    SERVER,
                    Class::Request,
                    "HTTP/3",
                    "GET / + /logo.png",
                    "1-RTT, DCID=f0a2b4c6d8e01357, PKN: 0, STREAM(0) HEADERS GET /, STREAM(4) HEADERS GET /logo.png",
                    148,
                )
                .says(k::PACKETS_QUIC_P5)
                .headers(&[
                    ip!("134", "0x5a21", "64", "0xe27b", "192.168.1.23", "203.0.113.10", &[]),
                    udp!("50544", "443", "114", "0x3fa9", &[]),
                    short!(
                        SERVER_CID,
                        "0",
                        "STREAM 0: HEADERS :method=GET :path=/ · STREAM 4: HEADERS :method=GET :path=/logo.png",
                        &[0, 6, 8]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_QUIC_4_TITLE,
        sub: k::PACKETS_QUIC_4_SUB,
        body: k::PACKETS_QUIC_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ACK, "1"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_QUIC_4_STATE),
        ],
        focus: &[PHONE, WIFI, INTERNET, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    INTERNET,
                    Class::Refusal,
                    "HTTP/3",
                    "STREAM 4",
                    "1-RTT, DCID=c1a5e0f2d47b3a96, PKN: 0, STREAM(4) HEADERS 200, DATA",
                    1_242,
                )
                .says(k::PACKETS_QUIC_P6)
                .headers(&[short!(PHONE_CID, "0", "STREAM 4: HEADERS :status=200, DATA", &[7, 8])]),
            ),
            Beat::Light {
                at: INTERNET,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    SERVER,
                    PHONE,
                    Class::Response,
                    "HTTP/3",
                    "STREAM 0: 200",
                    "1-RTT, DCID=c1a5e0f2d47b3a96, PKN: 1, HANDSHAKE_DONE, STREAM(0) HEADERS 200, DATA",
                    1_242,
                )
                .says(k::PACKETS_QUIC_P7)
                .headers(&[
                    ip!("1228", "0x0000", "52", "0x4457", "203.0.113.10", "192.168.1.23", &[]),
                    udp!("443", "50544", "1208", "0xa4e0", &[]),
                    short!(
                        PHONE_CID,
                        "1",
                        "HANDSHAKE_DONE · STREAM 0: HEADERS :status=200, DATA",
                        &[7, 8]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    SERVER,
                    Class::Control,
                    "QUIC",
                    "ACK 1",
                    "1-RTT, DCID=f0a2b4c6d8e01357, PKN: 1, ACK 1",
                    76,
                )
                .says(k::PACKETS_QUIC_P8)
                .headers(&[short!(SERVER_CID, "1", "ACK 1", &[8])]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    PHONE,
                    Class::Response,
                    "HTTP/3",
                    "STREAM 4",
                    "1-RTT, DCID=c1a5e0f2d47b3a96, PKN: 2, STREAM(4) HEADERS 200, DATA",
                    1_242,
                )
                .says(k::PACKETS_QUIC_P9)
                .headers(&[short!(PHONE_CID, "2", "STREAM 4: HEADERS :status=200, DATA", &[7, 8])]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_QUIC_5_TITLE,
        sub: k::PACKETS_QUIC_5_SUB,
        body: k::PACKETS_QUIC_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ADDRESS, "192.168.1.23 → 10.64.12.7"),
            wire(k::PACKETS_QUIC_LABEL_CID, "5c8e21d47ab0396f"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: WIFI,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    MOVED,
                    SERVER,
                    Class::Control,
                    "QUIC",
                    "ACK",
                    "1-RTT, DCID=5c8e21d47ab0396f, PKN: 2, ACK 1-2",
                    76,
                )
                .says(k::PACKETS_QUIC_P10)
                .headers(&[
                    ip!("62", "0x77c1", "64", "0x709c", "10.64.12.7", "203.0.113.10", &[11]),
                    udp!("61702", "443", "42", "0x27f3", &[0]),
                    short!("0x5c8e21d47ab0396f", "2", "ACK 1-2", &[6]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    MOVED,
                    Class::Control,
                    "QUIC",
                    "PATH_CHALLENGE",
                    "1-RTT, DCID=2e47d9a0b5c1f863, PKN: 3, PATH_CHALLENGE",
                    80,
                )
                .says(k::PACKETS_QUIC_P11)
                .headers(&[
                    ip!("66", "0x0000", "52", "0xf459", "203.0.113.10", "10.64.12.7", &[12]),
                    udp!("443", "61702", "46", "0x9c15", &[]),
                    short!("0x2e47d9a0b5c1f863", "3", "PATH_CHALLENGE 3b9f61d0e2a7c845", &[6, 8]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    MOVED,
                    SERVER,
                    Class::Control,
                    "QUIC",
                    "PATH_RESPONSE",
                    "1-RTT, DCID=5c8e21d47ab0396f, PKN: 3, PATH_RESPONSE, PADDING",
                    1_242,
                )
                .says(k::PACKETS_QUIC_P12)
                .headers(&[short!(
                    "0x5c8e21d47ab0396f",
                    "3",
                    "PATH_RESPONSE 3b9f61d0e2a7c845 + PADDING",
                    &[8]
                )]),
            ),
            Beat::Light {
                at: MOVED,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_QUIC_6_TITLE,
        sub: k::PACKETS_QUIC_6_SUB,
        body: k::PACKETS_QUIC_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ROUND_TRIPS, "0"),
            wire(k::PACKETS_LABEL_RESUMPTION, "NewSessionTicket → pre_shared_key"),
        ],
        focus: &[MOVED, MOBILE, INTERNET, SERVER],
        beats: &[
            Beat::Light {
                at: MOVED,
                prop: Some(Prop::Key),
            },
            Beat::Send(
                Packet::new(
                    MOVED,
                    SERVER,
                    Class::Request,
                    "QUIC",
                    "Initial + 0-RTT",
                    "Initial, DCID=6b2a91e0c37d54f8, SCID=0d93b7e1a24c58f6, PKN: 0, CRYPTO; 0-RTT, PKN: 0, STREAM(0)",
                    1_242,
                )
                .says(k::PACKETS_QUIC_P13)
                .headers(&[
                    ip!("1228", "0x1f04", "64", "0xc4cb", "10.64.12.7", "203.0.113.10", &[]),
                    udp!("55129", "443", "1208", "0x61be", &[]),
                    initial!(
                        "0x6b2a91e0c37d54f8",
                        "0x0d93b7e1a24c58f6",
                        "1080",
                        "0",
                        "CRYPTO [ClientHello: pre_shared_key, early_data, key_share]",
                        &[13]
                    ),
                    long!(
                        "1",
                        "0x6b2a91e0c37d54f8",
                        "0x0d93b7e1a24c58f6",
                        "69",
                        "0",
                        "STREAM 0: HEADERS :method=GET :path=/",
                        &[2, 12]
                    ),
                ]),
            ),
        ],
    },
];

/// QUIC, and HTTP/3 over it.
pub static QUIC: Protocol = Protocol {
    id: "quic",
    group: Group::Transport,
    layer: Layer(4),
    name: k::PACKETS_QUIC_NAME,
    title: k::PACKETS_QUIC_TITLE,
    tagline: k::PACKETS_QUIC_TAGLINE,
    overview: k::PACKETS_QUIC_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "443"),
        wire(k::PACKETS_FACT_TRANSPORT, "UDP"),
        fact(k::PACKETS_FACT_ENCRYPTION, k::PACKETS_QUIC_ENCRYPTION),
        fact(k::PACKETS_FACT_RELIABILITY, k::PACKETS_QUIC_RELIABILITY),
        wire(
            k::PACKETS_FACT_DEFINED,
            "RFC 9000, RFC 9001, RFC 9002, RFC 9114",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(PHONE, WIFI, 2),
            link(WIFI, INTERNET, 5),
            link(MOVED, MOBILE, 15),
            link(MOBILE, INTERNET, 4),
            link(INTERNET, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
