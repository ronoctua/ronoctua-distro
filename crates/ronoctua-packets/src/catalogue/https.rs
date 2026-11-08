//! HTTPS: HTTP inside TLS — the address and the TCP connection, briefly,
//! then the TLS 1.3 handshake with a post-quantum hybrid key share and the
//! server's certificate, the request and the page sealed in TLS records,
//! and what an attacker on the same Wi-Fi is left to see.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const WIFI: usize = 1;
const ATTACKER: usize = 2;
const INTERNET: usize = 3;
const RESOLVER: usize = 4;
const SERVER: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 2),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::AccessPoint,
        at: at(-4, -2),
        name: k::PACKETS_DEVICE_ACCESS_POINT,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Laptop,
        at: at(-5, 6),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::V4([192, 168, 1, 66]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(1, 1),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Rack,
        at: at(3, -6),
        name: k::PACKETS_DEVICE_RESOLVER,
        address: Address::V4([8, 8, 8, 8]),
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(7, -1),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// An IPv4 header carrying TCP, "don't fragment" set: total length,
/// identification, TTL, checksum, source, destination.
macro_rules! ip {
    ($len:literal, $id:literal, $ttl:literal, $sum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", $len, $id, "010", "0", $ttl, "6", $sum, $src, $dst,
            ],
            $lit,
        )
    };
}

/// A TCP header: ports, numbers, offset, the eight flags (CWR ECE URG ACK
/// PSH RST SYN FIN), window.
macro_rules! tcp {
    ($src:literal, $dst:literal, $seq:literal, $ack:literal, $off:literal,
     [$cwr:literal, $ece:literal, $urg:literal, $ackf:literal, $psh:literal, $rst:literal, $syn:literal, $fin:literal],
     $win:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, $seq, $ack, $off, "0", $cwr, $ece, $urg, $ackf, $psh, $rst, $syn, $fin,
                $win, "0x91d4", "0",
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_HTTPS_1_TITLE,
        sub: k::PACKETS_HTTPS_1_SUB,
        body: k::PACKETS_HTTPS_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ANSWER, "A 203.0.113.10"),
            wire(k::PACKETS_LABEL_SERVER_PORT, "443"),
            wire(k::PACKETS_LABEL_HANDSHAKE, "SYN → SYN-ACK → ACK"),
        ],
        focus: &[],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "https://www.example.com/",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    RESOLVER,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com",
                    "0x3f2a A? www.example.com",
                    75,
                )
                .says(k::PACKETS_HTTPS_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "61", "0x8c11", "000", "0", "64", "17", "0x1cdd", "192.168.1.10",
                            "8.8.8.8",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["51000", "53", "41", "0x9c2b"], &[1]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x3f2a",
                            "0",
                            "0",
                            "0",
                            "0",
                            "1",
                            "0",
                            "000",
                            "0",
                            "1",
                            "0",
                            "0",
                            "0",
                            "www.example.com  A  IN",
                        ],
                        &[13],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    CLIENT,
                    Class::Lookup,
                    "DNS",
                    "A 203.0.113.10",
                    "0x3f2a A www.example.com → 203.0.113.10",
                    91,
                )
                .says(k::PACKETS_HTTPS_P2),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "52344 → 443 [SYN] Seq=0 Win=64240",
                    66,
                )
                .says(k::PACKETS_HTTPS_P3)
                .headers(&[
                    ip!("52", "0x1c46", "64", "0x20c1", "192.168.1.10", "203.0.113.10", &[12]),
                    tcp!("52344", "443", "0", "0", "8", ["0", "0", "0", "0", "0", "0", "1", "0"], "64240", &[1, 12]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "TCP",
                    "SYN-ACK",
                    "443 → 52344 [SYN, ACK] Seq=0 Ack=1",
                    66,
                )
                .says(k::PACKETS_HTTPS_P4),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "ACK",
                    "52344 → 443 [ACK] Seq=1 Ack=1",
                    54,
                )
                .says(k::PACKETS_HTTPS_P5),
            ),
        ],
    },
    Step {
        title: k::PACKETS_HTTPS_2_TITLE,
        sub: k::PACKETS_HTTPS_2_SUB,
        body: k::PACKETS_HTTPS_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_VERSION, "TLS 1.3"),
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "X25519MLKEM768"),
        ],
        focus: &[CLIENT, WIFI, ATTACKER, INTERNET, SERVER],
        beats: &[Beat::Send(
            Packet::new(
                CLIENT,
                SERVER,
                Class::Handshake,
                "TLSv1.3",
                "ClientHello",
                "Client Hello (SNI=www.example.com)",
                1_908,
            )
            .says(k::PACKETS_HTTPS_P6)
            .headers(&[
                ip!("1500", "0x1c48", "64", "0x1b17", "192.168.1.10", "203.0.113.10", &[]),
                tcp!("52344", "443", "1", "1", "5", ["0", "0", "0", "1", "1", "0", "0", "0"], "502", &[]),
                header(
                    &anatomy::TLS,
                    &[
                        "22",
                        "0x0301",
                        "1795",
                        "1",
                        "1791",
                        "random · SNI=www.example.com · supported_versions=0x0304 · cipher_suites=0x1301,0x1302,0x1303 · key_share",
                    ],
                    &[0, 3, 5],
                ),
                header(
                    &anatomy::KEY_SHARE,
                    &["0x11ec", "1216", "0x8c3a51d7… (1184 B)", "0x2f9c48b1…7a41"],
                    &[0, 2, 3],
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_HTTPS_3_TITLE,
        sub: k::PACKETS_HTTPS_3_SUB,
        body: k::PACKETS_HTTPS_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CIPHER, "TLS_AES_128_GCM_SHA256"),
            wire(k::PACKETS_LABEL_CERTIFICATE, "www.example.com"),
        ],
        focus: &[CLIENT, WIFI, INTERNET, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Crypto,
                    "TLSv1.3",
                    "ServerHello + Certificate",
                    "Server Hello, Change Cipher Spec, Encrypted Extensions, Certificate, Certificate Verify, Finished",
                    4_486,
                )
                .says(k::PACKETS_HTTPS_P7)
                .headers(&[
                    ip!("1500", "0x0000", "56", "0x3f5f", "203.0.113.10", "192.168.1.10", &[]),
                    tcp!("443", "52344", "1", "1801", "5", ["0", "0", "0", "1", "1", "0", "0", "0"], "509", &[3]),
                    header(
                        &anatomy::TLS,
                        &[
                            "22",
                            "0x0303",
                            "1210",
                            "2",
                            "1206",
                            "random · cipher_suite=0x1301 · supported_versions=0x0304 · key_share",
                        ],
                        &[3, 5],
                    ),
                    header(
                        &anatomy::KEY_SHARE,
                        &["0x11ec", "1120", "0x5e07c2a9… (1088 B)", "0x9b41e6f3…c25d"],
                        &[2, 3],
                    ),
                ]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Certificate),
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Crypto,
                    "TLSv1.3",
                    "Finished",
                    "Change Cipher Spec, Finished",
                    118,
                )
                .says(k::PACKETS_HTTPS_P8)
                .headers(&[
                    ip!("104", "0x1c4a", "64", "0x2089", "192.168.1.10", "203.0.113.10", &[]),
                    tcp!("52344", "443", "1801", "4325", "5", ["0", "0", "0", "1", "1", "0", "0", "0"], "501", &[]),
                    header(&anatomy::TLS, &["23", "0x0303", "53"], &[0]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_HTTPS_4_TITLE,
        sub: k::PACKETS_HTTPS_4_SUB,
        body: k::PACKETS_HTTPS_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REQUEST, "GET / HTTP/1.1"),
            wire(k::PACKETS_LABEL_STATUS, "200 OK"),
            wire(k::PACKETS_LABEL_RECORD, "application_data (23)"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Request,
                    "TLSv1.3",
                    "GET /",
                    "Application Data",
                    161,
                )
                .says(k::PACKETS_HTTPS_P9)
                .headers(&[
                    ip!("147", "0x1c4b", "64", "0x205d", "192.168.1.10", "203.0.113.10", &[12]),
                    tcp!("52344", "443", "1865", "4325", "5", ["0", "0", "0", "1", "1", "0", "0", "0"], "501", &[1]),
                    header(&anatomy::TLS, &["23", "0x0303", "102"], &[0, 2]),
                    header(
                        &anatomy::HTTP_REQUEST,
                        &[
                            "GET / HTTP/1.1",
                            "Host: www.example.com",
                            "User-Agent: Mozilla/5.0",
                            "Accept: text/html",
                            "",
                        ],
                        &[0, 1],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Response,
                    "TLSv1.3",
                    "200 OK",
                    "Application Data",
                    13_143,
                )
                .says(k::PACKETS_HTTPS_P10)
                .headers(&[
                    ip!("1500", "0x0000", "56", "0x3f5f", "203.0.113.10", "192.168.1.10", &[]),
                    tcp!("443", "52344", "4325", "1972", "5", ["0", "0", "0", "1", "1", "0", "0", "0"], "509", &[]),
                    header(&anatomy::TLS, &["23", "0x0303", "12652"], &[0, 2]),
                    header(
                        &anatomy::HTTP_RESPONSE,
                        &[
                            "HTTP/1.1 200 OK",
                            "Content-Type: text/html; charset=UTF-8",
                            "Content-Length: 12553",
                            "",
                            "<!doctype html><html><head><title>Example…",
                        ],
                        &[0, 4],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_HTTPS_5_TITLE,
        sub: k::PACKETS_HTTPS_5_SUB,
        body: k::PACKETS_HTTPS_5_BODY,
        facts: &[fact(k::PACKETS_LABEL_STATE, k::PACKETS_HTTPS_5_STATE)],
        focus: &[CLIENT, WIFI, ATTACKER],
        beats: &[
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Lock),
            },
            Beat::Light {
                at: ATTACKER,
                prop: Some(Prop::Cross),
            },
            Beat::Rest,
        ],
    },
];

/// HTTPS.
pub static HTTPS: Protocol = Protocol {
    id: "https",
    group: Group::Web,
    layer: Layer(7),
    name: k::PACKETS_HTTPS_NAME,
    title: k::PACKETS_HTTPS_TITLE,
    tagline: k::PACKETS_HTTPS_TAGLINE,
    overview: k::PACKETS_HTTPS_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "443"),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        wire(k::PACKETS_FACT_ENCRYPTION, "TLS 1.3"),
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_HTTPS_AUTHENTICATION,
        ),
        wire(k::PACKETS_FACT_DEFINED, "RFC 9110, RFC 8446"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, WIFI, 2),
            link(ATTACKER, WIFI, 2),
            link(WIFI, INTERNET, 5),
            link(INTERNET, RESOLVER, 1),
            link(INTERNET, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
