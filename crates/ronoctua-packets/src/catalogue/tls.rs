//! TLS 1.3: the handshake that makes a TCP connection private — the
//! ClientHello and ServerHello that agree a key, the server's flight already
//! encrypted (EncryptedExtensions, Certificate, CertificateVerify, Finished),
//! the client's Finished — then records sealed with AEAD, a session ticket
//! for the next time, and close_notify.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const SERVER: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-4, 0),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(1, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(6, -2),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// A TLS record — content type, legacy version, length — and, inside it, a
/// handshake message's type, length and body. After the ServerHello the
/// record says 23 (application data) outside, and what follows is shown
/// as a capture holding the session's keys shows it.
macro_rules! tls {
    ($ct:literal, $ver:literal, $len:literal, $hs:literal, $hslen:literal, $body:literal, $lit:expr) => {
        header(&anatomy::TLS, &[$ct, $ver, $len, $hs, $hslen, $body], $lit)
    };
}

/// An encrypted record whose inside is not a handshake message: application
/// data, an alert. Its type and length are all a capture can read.
macro_rules! sealed {
    ($len:literal, $lit:expr) => {
        header(&anatomy::TLS, &["23", "0x0303", $len], $lit)
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_TLS_1_TITLE,
        sub: k::PACKETS_TLS_1_SUB,
        body: k::PACKETS_TLS_1_BODY,
        facts: &[
            wire(k::PACKETS_TLS_LABEL_SNI, "www.example.com"),
            wire(k::PACKETS_LABEL_CIPHER, "TLS_AES_128_GCM_SHA256"),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "x25519"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Key),
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "TLSv1.3",
                    "ClientHello",
                    "Client Hello (SNI=www.example.com)",
                    571,
                )
                .says(k::PACKETS_TLS_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "557", "0x1c47", "010", "0", "64", "6", "0xb0a1", "192.168.1.10",
                            "203.0.113.10",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "443", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "502", "0x77c1",
                            "0",
                        ],
                        &[1],
                    ),
                    tls!(
                        "22",
                        "0x0301",
                        "512",
                        "1",
                        "508",
                        "random: 6f1c…a9e2; cipher_suites: TLS_AES_128_GCM_SHA256, TLS_AES_256_GCM_SHA384, \
                         TLS_CHACHA20_POLY1305_SHA256; server_name: www.example.com; supported_versions: 0x0304; \
                         supported_groups: x25519, secp256r1; key_share: x25519 4b7e…d013; alpn: h2, http/1.1",
                        &[0, 3, 5]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_TLS_2_TITLE,
        sub: k::PACKETS_TLS_2_SUB,
        body: k::PACKETS_TLS_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CIPHER, "TLS_AES_128_GCM_SHA256"),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "x25519"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_TLS_2_STATE),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "TLSv1.3",
                    "ServerHello",
                    "Server Hello",
                    181,
                )
                .says(k::PACKETS_TLS_P2)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "443", "52344", "1", "518", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "509", "0x3e0c",
                            "0",
                        ],
                        &[3],
                    ),
                    tls!(
                        "22",
                        "0x0303",
                        "122",
                        "2",
                        "118",
                        "random: d41a…0c7f; cipher_suite: TLS_AES_128_GCM_SHA256; supported_versions: 0x0304; \
                         key_share: x25519 9fd7…c3a1",
                        &[3, 5]
                    ),
                ]),
            ),
            Beat::Light {
                at: SERVER,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_TLS_3_TITLE,
        sub: k::PACKETS_TLS_3_SUB,
        body: k::PACKETS_TLS_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CERTIFICATE, "CN=www.example.com"),
            wire(k::PACKETS_LABEL_SIGNATURE, "ecdsa_secp256r1_sha256"),
            fact(k::PACKETS_LABEL_ENCRYPTION, k::PACKETS_TLS_3_ENCRYPTION),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "TLSv1.3",
                    "EncryptedExtensions",
                    "Encrypted Extensions",
                    91,
                )
                .says(k::PACKETS_TLS_P3)
                .headers(&[tls!("23", "0x0303", "32", "8", "11", "alpn: h2", &[0, 3])]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Crypto,
                    "TLSv1.3",
                    "Certificate",
                    "Certificate",
                    2_169,
                )
                .says(k::PACKETS_TLS_P4)
                .headers(&[tls!(
                    "23",
                    "0x0303",
                    "2056",
                    "11",
                    "2035",
                    "certificate_list: CN=www.example.com; CN=Example Issuing CA",
                    &[3, 5]
                )]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Certificate),
            },
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Crypto,
                    "TLSv1.3",
                    "CertificateVerify",
                    "Certificate Verify",
                    155,
                )
                .says(k::PACKETS_TLS_P5)
                .headers(&[tls!(
                    "23",
                    "0x0303",
                    "96",
                    "15",
                    "75",
                    "algorithm: ecdsa_secp256r1_sha256; signature: 3045…b1f2",
                    &[3, 5]
                )]),
            ),
            Beat::Send(
                Packet::new(SERVER, CLIENT, Class::Crypto, "TLSv1.3", "Finished", "Finished", 112)
                    .says(k::PACKETS_TLS_P6)
                    .headers(&[tls!("23", "0x0303", "53", "20", "32", "verify_data: 7a1e…c904", &[3, 5])]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_TLS_4_TITLE,
        sub: k::PACKETS_TLS_4_SUB,
        body: k::PACKETS_TLS_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ROUND_TRIPS, "1"),
            wire(k::PACKETS_LABEL_RECORD, "application_data (23)"),
            wire(k::PACKETS_LABEL_ENCRYPTION, "AES-128-GCM"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(CLIENT, SERVER, Class::Crypto, "TLSv1.3", "Finished", "Finished", 112)
                    .says(k::PACKETS_TLS_P7)
                    .headers(&[tls!("23", "0x0303", "53", "20", "32", "verify_data: 0b6d…2e58", &[3])]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Lock),
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Request,
                    "TLSv1.3",
                    "Application Data",
                    "Application Data",
                    209,
                )
                .says(k::PACKETS_TLS_P8)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "443", "576", "2385", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "501",
                            "0x6b0e", "0",
                        ],
                        &[2],
                    ),
                    sealed!("150", &[0, 2]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Response,
                    "TLSv1.3",
                    "Application Data",
                    "Application Data",
                    1_242,
                )
                .says(k::PACKETS_TLS_P9)
                .headers(&[sealed!("1183", &[0, 2])]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_TLS_5_TITLE,
        sub: k::PACKETS_TLS_5_SUB,
        body: k::PACKETS_TLS_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_RESUMPTION, "pre_shared_key, psk_dhe_ke"),
            wire(k::PACKETS_LABEL_TTL, "7200"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Crypto,
                    "TLSv1.3",
                    "NewSessionTicket",
                    "New Session Ticket",
                    293,
                )
                .says(k::PACKETS_TLS_P10)
                .headers(&[tls!(
                    "23",
                    "0x0303",
                    "234",
                    "4",
                    "213",
                    "ticket_lifetime: 7200; ticket_age_add: 0x5c2e91a7; ticket_nonce: 8 B; ticket: 192 B",
                    &[3, 5]
                )]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Clock),
            },
        ],
    },
    Step {
        title: k::PACKETS_TLS_6_TITLE,
        sub: k::PACKETS_TLS_6_SUB,
        body: k::PACKETS_TLS_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_RECORD, "alert: close_notify (0)"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_TLS_6_STATE),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Control,
                    "TLSv1.3",
                    "close_notify",
                    "Alert: warning (1), close_notify (0)",
                    78,
                )
                .says(k::PACKETS_TLS_P11)
                .headers(&[sealed!("19", &[0, 2])]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Control,
                    "TLSv1.3",
                    "close_notify",
                    "Alert: warning (1), close_notify (0)",
                    78,
                )
                .says(k::PACKETS_TLS_P12)
                .headers(&[sealed!("19", &[0])]),
            ),
        ],
    },
];

/// TLS 1.3.
pub static TLS: Protocol = Protocol {
    id: "tls",
    group: Group::Security,
    layer: Layer(6),
    name: k::PACKETS_TLS_NAME,
    title: k::PACKETS_TLS_TITLE,
    tagline: k::PACKETS_TLS_TAGLINE,
    overview: k::PACKETS_TLS_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_TLS_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        wire(
            k::PACKETS_FACT_ENCRYPTION,
            "AES-128-GCM, AES-256-GCM, ChaCha20-Poly1305",
        ),
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_TLS_AUTHENTICATION,
        ),
        wire(k::PACKETS_FACT_DEFINED, "RFC 8446"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
