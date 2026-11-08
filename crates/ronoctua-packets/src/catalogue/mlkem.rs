//! POST-QUANTUM KEY EXCHANGE: TLS 1.3 with the hybrid group X25519MLKEM768 —
//! why a recording made today matters, the ClientHello that carries an
//! ML-KEM-768 encapsulation key beside an X25519 share (and grows past one
//! TCP segment), the server's encapsulation, the two secrets joined, and how
//! this system's own TLS and SSH offer it first.

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
const ATTACKER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_CLIENT,
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
        kind: Kind::Tower(Emblem::Globe),
        at: at(6, -2),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
    Device {
        kind: Kind::Storage,
        at: at(3, 6),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::None,
    },
];

/// A TCP header between the client's port 52344 and the server's 443: ports,
/// numbers, offset, the eight flags (CWR ECE URG ACK PSH RST SYN FIN), window.
macro_rules! tcp {
    ($src:literal, $dst:literal, $seq:literal, $ack:literal,
     [$cwr:literal, $ece:literal, $urg:literal, $ackf:literal, $psh:literal, $rst:literal, $syn:literal, $fin:literal],
     $win:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, $seq, $ack, "5", "0", $cwr, $ece, $urg, $ackf, $psh, $rst, $syn, $fin,
                $win, "0x91d3", "0",
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_MLKEM_1_TITLE,
        sub: k::PACKETS_MLKEM_1_SUB,
        body: k::PACKETS_MLKEM_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "x25519"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_MLKEM_1_VERDICT),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Lock),
            },
            Beat::Light {
                at: ATTACKER,
                prop: Some(Prop::Clock),
            },
            Beat::Rest,
        ],
    },
    Step {
        title: k::PACKETS_MLKEM_2_TITLE,
        sub: k::PACKETS_MLKEM_2_SUB,
        body: k::PACKETS_MLKEM_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "X25519MLKEM768 (0x11EC)"),
            wire(k::PACKETS_LABEL_SIZE, "1184 B + 32 B"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SERVER],
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
                    "TCP",
                    "ClientHello (1/2)",
                    "52344 → 443 [ACK] Seq=1 Ack=1 Len=1448",
                    1_502,
                )
                .says(k::PACKETS_MLKEM_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "1488", "0x1c47", "010", "0", "64", "6", "0x9a3e", "192.168.1.10",
                            "203.0.113.10",
                        ],
                        &[4],
                    ),
                    tcp!("52344", "443", "1", "1", ["0", "0", "0", "1", "0", "0", "0", "0"], "502", &[2]),
                    header(
                        &anatomy::TLS,
                        &[
                            "22",
                            "0x0301",
                            "1732",
                            "1",
                            "1728",
                            "cipher_suites: TLS_AES_128_GCM_SHA256, …; server_name: www.example.com; \
                             supported_groups: X25519MLKEM768, x25519, secp256r1; key_share: X25519MLKEM768, x25519",
                        ],
                        &[2, 5],
                    ),
                    header(
                        &anatomy::KEY_SHARE,
                        &["0x11EC", "1216", "7c0a…e93b (1184 B)", "4b7e…d013 (32 B)"],
                        &[0, 1, 2, 3],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "TLSv1.3",
                    "ClientHello (2/2)",
                    "Client Hello (SNI=www.example.com)",
                    343,
                )
                .says(k::PACKETS_MLKEM_P2)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "329", "0x1c48", "010", "0", "64", "6", "0xa0c1", "192.168.1.10",
                            "203.0.113.10",
                        ],
                        &[4],
                    ),
                    tcp!("52344", "443", "1449", "1", ["0", "0", "0", "1", "1", "0", "0", "0"], "502", &[2, 10]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MLKEM_3_TITLE,
        sub: k::PACKETS_MLKEM_3_SUB,
        body: k::PACKETS_MLKEM_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ALGORITHM, "ML-KEM-768 (FIPS 203)"),
            wire(k::PACKETS_LABEL_SIZE, "1088 B + 32 B"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SERVER],
        beats: &[
            Beat::Light {
                at: SERVER,
                prop: Some(Prop::Key),
            },
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "TLSv1.3",
                    "ServerHello",
                    "Server Hello",
                    1_269,
                )
                .says(k::PACKETS_MLKEM_P3)
                .headers(&[
                    tcp!("443", "52344", "1", "1738", ["0", "0", "0", "1", "1", "0", "0", "0"], "509", &[3]),
                    header(
                        &anatomy::TLS,
                        &[
                            "22",
                            "0x0303",
                            "1210",
                            "2",
                            "1206",
                            "cipher_suite: TLS_AES_128_GCM_SHA256; supported_versions: 0x0304; \
                             key_share: X25519MLKEM768",
                        ],
                        &[2, 4],
                    ),
                    header(
                        &anatomy::KEY_SHARE,
                        &["0x11EC", "1120", "3d81…a4c6 (1088 B)", "9fd7…c3a1 (32 B)"],
                        &[0, 1, 2, 3],
                    ),
                ]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_MLKEM_4_TITLE,
        sub: k::PACKETS_MLKEM_4_SUB,
        body: k::PACKETS_MLKEM_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "32 B ‖ 32 B → HKDF"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_MLKEM_4_VERDICT),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Crypto,
                    "TLSv1.3",
                    "Certificate, Finished",
                    "Encrypted Extensions, Certificate, Certificate Verify, Finished",
                    2_365,
                )
                .says(k::PACKETS_MLKEM_P4)
                .headers(&[header(
                    &anatomy::TLS,
                    &["23", "0x0303", "32", "8", "11", "alpn: h2"],
                    &[0],
                )]),
            ),
            Beat::Send(
                Packet::new(CLIENT, SERVER, Class::Crypto, "TLSv1.3", "Finished", "Finished", 112)
                    .says(k::PACKETS_MLKEM_P5)
                    .headers(&[header(
                        &anatomy::TLS,
                        &["23", "0x0303", "53", "20", "32", "verify_data: 0b6d…2e58"],
                        &[0, 3],
                    )]),
            ),
            Beat::Light {
                at: ATTACKER,
                prop: Some(Prop::Cross),
            },
        ],
    },
    Step {
        title: k::PACKETS_MLKEM_5_TITLE,
        sub: k::PACKETS_MLKEM_5_SUB,
        body: k::PACKETS_MLKEM_5_BODY,
        facts: &[
            wire(k::PACKETS_MLKEM_LABEL_TLS, "X25519MLKEM768, x25519"),
            wire(
                k::PACKETS_MLKEM_LABEL_SSH,
                "mlkem768x25519-sha256, sntrup761x25519-sha512, curve25519-sha256",
            ),
        ],
        focus: &[CLIENT],
        beats: &[
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Shield),
            },
            Beat::Rest,
        ],
    },
];

/// Post-quantum key exchange, the ML-KEM hybrid.
pub static MLKEM: Protocol = Protocol {
    id: "mlkem",
    group: Group::Security,
    layer: Layer(6),
    name: k::PACKETS_MLKEM_NAME,
    title: k::PACKETS_MLKEM_TITLE,
    tagline: k::PACKETS_MLKEM_TAGLINE,
    overview: k::PACKETS_MLKEM_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_STANDARD, "FIPS 203 (ML-KEM)"),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_MLKEM_SECURITY),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_MLKEM_USED_BY),
        wire(
            k::PACKETS_FACT_DEFINED,
            "draft-ietf-tls-ecdhe-mlkem, draft-ietf-sshm-mlkem-hybrid-kex",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, SERVER, 4),
            link(INTERNET, ATTACKER, 1),
        ],
    },
    steps: &STEPS,
};
