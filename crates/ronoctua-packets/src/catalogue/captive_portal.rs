//! A captive portal: a phone joins an open Wi-Fi, its probe is answered by
//! the gateway with a redirect, an HTTPS page fails, the portal's terms are
//! accepted, and the probe then reaches the internet.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const PHONE: usize = 0;
const ACCESS_POINT: usize = 1;
const GATEWAY: usize = 2;
const PORTAL: usize = 3;
const INTERNET: usize = 4;
const SERVER: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Phone,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_PHONE,
        address: Address::V4([10, 20, 0, 57]),
    },
    Device {
        kind: Kind::AccessPoint,
        at: at(-6, -1),
        name: k::PACKETS_DEVICE_ACCESS_POINT,
        address: Address::None,
    },
    Device {
        kind: Kind::Router,
        at: at(-2, 2),
        name: k::PACKETS_CAPTIVE_PORTAL_DEVICE_GATEWAY,
        address: Address::V4([10, 20, 0, 1]),
    },
    Device {
        kind: Kind::Tower(Emblem::Page),
        at: at(-1, -5),
        name: k::PACKETS_DEVICE_PORTAL,
        address: Address::V4([10, 20, 0, 2]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(3, 0),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(8, -3),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 20]),
    },
];

/// The system's probe, the same before and after.
const PROBE: &[&str] = &[
    "GET /generate_204 HTTP/1.1",
    "Host: check.example.com",
    "User-Agent: Mozilla/5.0",
    "Accept-Encoding: gzip",
    "",
];

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_CAPTIVE_PORTAL_1_TITLE,
        sub: k::PACKETS_CAPTIVE_PORTAL_1_SUB,
        body: k::PACKETS_CAPTIVE_PORTAL_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ADDRESS, "10.20.0.57"),
            wire(k::PACKETS_LABEL_GATEWAY, "10.20.0.1"),
            wire(k::PACKETS_CAPTIVE_PORTAL_LABEL_API, "https://portal.example.net/api"),
        ],
        focus: &[PHONE, ACCESS_POINT, GATEWAY],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    GATEWAY,
                    Class::Lookup,
                    "DHCP",
                    "DHCP Request",
                    "DHCP Request - Transaction ID 0x3903f326",
                    342,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "328", "0x0000", "000", "0", "64", "17", "0x79a6", "0.0.0.0",
                            "255.255.255.255",
                        ],
                        &[11, 12],
                    ),
                    header(&anatomy::UDP, &["68", "67", "308", "0x6b3d"], &[0, 1]),
                    header(
                        &anatomy::DHCP,
                        &[
                            "1",
                            "1",
                            "6",
                            "0",
                            "0x3903f326",
                            "0",
                            "0x0000",
                            "0.0.0.0",
                            "0.0.0.0",
                            "0.0.0.0",
                            "0.0.0.0",
                            "00:00:5e:00:53:2a",
                            "53=3 (Request), 50=10.20.0.57, 54=10.20.0.1, 55=1,3,6,114",
                        ],
                        &[11, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    PHONE,
                    Class::Lookup,
                    "DHCP",
                    "DHCP ACK",
                    "DHCP ACK - Transaction ID 0x3903f326",
                    342,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P2)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "328", "0x0000", "000", "0", "64", "17", "0x6544", "10.20.0.1",
                            "10.20.0.57",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["67", "68", "308", "0x1f02"], &[]),
                    header(
                        &anatomy::DHCP,
                        &[
                            "2",
                            "1",
                            "6",
                            "0",
                            "0x3903f326",
                            "0",
                            "0x0000",
                            "0.0.0.0",
                            "10.20.0.57",
                            "0.0.0.0",
                            "0.0.0.0",
                            "00:00:5e:00:53:2a",
                            "53=5 (ACK), 54=10.20.0.1, 1=255.255.255.0, 3=10.20.0.1, 6=10.20.0.1, 51=3600, 114=https://portal.example.net/api",
                        ],
                        &[8, 12],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_CAPTIVE_PORTAL_2_TITLE,
        sub: k::PACKETS_CAPTIVE_PORTAL_2_SUB,
        body: k::PACKETS_CAPTIVE_PORTAL_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_URL, "http://check.example.com/generate_204"),
            wire(k::PACKETS_CAPTIVE_PORTAL_LABEL_EXPECTED, "204 No Content"),
            wire(k::PACKETS_LABEL_STATUS, "302 Found"),
        ],
        focus: &[PHONE, ACCESS_POINT, GATEWAY],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    GATEWAY,
                    Class::Request,
                    "HTTP",
                    "GET /generate_204",
                    "GET /generate_204 HTTP/1.1",
                    157,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P3)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "143", "0x2b40", "010", "0", "64", "6", "0xc8c7", "10.20.0.57",
                            "203.0.113.20",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "41522", "80", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "502", "0x5d7e", "0",
                        ],
                        &[1],
                    ),
                    header(&anatomy::HTTP_MESSAGE, PROBE, &[0, 1]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    PHONE,
                    Class::Refusal,
                    "HTTP",
                    "302 Found",
                    "HTTP/1.1 302 Found",
                    164,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P4)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "150", "0x0000", "010", "0", "64", "6", "0xf400", "203.0.113.20",
                            "10.20.0.57",
                        ],
                        &[11],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "HTTP/1.1 302 Found",
                            "Location: https://portal.example.net/login",
                            "Cache-Control: no-store",
                            "Content-Length: 0",
                            "",
                        ],
                        &[0, 1],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_CAPTIVE_PORTAL_3_TITLE,
        sub: k::PACKETS_CAPTIVE_PORTAL_3_SUB,
        body: k::PACKETS_CAPTIVE_PORTAL_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_URL, "https://www.example.com/"),
            wire(k::PACKETS_LABEL_FLAGS, "RST, ACK"),
            fact(k::PACKETS_LABEL_REASON, k::PACKETS_CAPTIVE_PORTAL_3_REASON),
        ],
        focus: &[PHONE, ACCESS_POINT, GATEWAY],
        beats: &[
            Beat::Type {
                at: PHONE,
                wire: "https://www.example.com/",
            },
            Beat::Send(
                Packet::new(
                    PHONE,
                    GATEWAY,
                    Class::Handshake,
                    "TCP",
                    "SYN :443",
                    "41530 → 443 [SYN] Seq=0 Win=65535 MSS=1460",
                    74,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P5)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "60", "0x2b41", "010", "0", "64", "6", "0xc923", "10.20.0.57",
                            "203.0.113.10",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "41530", "443", "0", "0", "10", "0", "0", "0", "0", "0", "0", "0", "1", "0", "65535", "0x2e61",
                            "0",
                        ],
                        &[1, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    GATEWAY,
                    PHONE,
                    Class::Refusal,
                    "TCP",
                    "RST",
                    "443 → 41530 [RST, ACK] Seq=1 Ack=1 Win=0",
                    54,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P6)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "40", "0x0000", "010", "0", "64", "6", "0xf478", "203.0.113.10",
                            "10.20.0.57",
                        ],
                        &[11],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "443", "41530", "1", "1", "5", "0", "0", "0", "0", "1", "0", "1", "0", "0", "0", "0x8c90", "0",
                        ],
                        &[9, 11],
                    ),
                ]),
            ),
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Cross),
            },
        ],
    },
    Step {
        title: k::PACKETS_CAPTIVE_PORTAL_4_TITLE,
        sub: k::PACKETS_CAPTIVE_PORTAL_4_SUB,
        body: k::PACKETS_CAPTIVE_PORTAL_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_URL, "https://portal.example.net/login"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_CAPTIVE_PORTAL_4_STATE),
        ],
        focus: &[PHONE, ACCESS_POINT, GATEWAY, PORTAL],
        beats: &[
            Beat::Send(
                Packet::new(PHONE, PORTAL, Class::Request, "HTTP", "GET /login", "GET /login HTTP/1.1", 169)
                    .says(k::PACKETS_CAPTIVE_PORTAL_P7)
                    .headers(&[header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "GET /login HTTP/1.1",
                            "Host: portal.example.net",
                            "User-Agent: Mozilla/5.0",
                            "Accept: text/html",
                            "",
                        ],
                        &[0, 1],
                    )]),
            ),
            Beat::Send(
                Packet::new(
                    PORTAL,
                    PHONE,
                    Class::Response,
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/html)",
                    5_502,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P8)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 200 OK",
                        "Content-Type: text/html; charset=UTF-8",
                        "Cache-Control: no-store",
                        "Content-Length: 5320",
                        "",
                        "<!doctype html>…<form method=\"post\" action=\"/login\">…",
                    ],
                    &[0, 5],
                )]),
            ),
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Page),
            },
        ],
    },
    Step {
        title: k::PACKETS_CAPTIVE_PORTAL_5_TITLE,
        sub: k::PACKETS_CAPTIVE_PORTAL_5_SUB,
        body: k::PACKETS_CAPTIVE_PORTAL_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_MAC, "00:00:5e:00:53:2a"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_CAPTIVE_PORTAL_5_VERDICT),
        ],
        focus: &[PHONE, ACCESS_POINT, GATEWAY, PORTAL],
        beats: &[
            Beat::Send(
                Packet::new(PHONE, PORTAL, Class::Request, "HTTP", "POST /login", "POST /login HTTP/1.1", 216)
                    .says(k::PACKETS_CAPTIVE_PORTAL_P9)
                    .headers(&[header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "POST /login HTTP/1.1",
                            "Host: portal.example.net",
                            "Content-Type: application/x-www-form-urlencoded",
                            "Content-Length: 21",
                            "",
                            "terms=accept&room=214",
                        ],
                        &[0, 5],
                    )]),
            ),
            Beat::Light {
                at: PORTAL,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: GATEWAY,
                prop: Some(Prop::Check),
            },
            Beat::Send(
                Packet::new(
                    PORTAL,
                    PHONE,
                    Class::Response,
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/html)",
                    2_392,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P10),
            ),
        ],
    },
    Step {
        title: k::PACKETS_CAPTIVE_PORTAL_6_TITLE,
        sub: k::PACKETS_CAPTIVE_PORTAL_6_SUB,
        body: k::PACKETS_CAPTIVE_PORTAL_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_STATUS, "204 No Content"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_CAPTIVE_PORTAL_6_STATE),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    SERVER,
                    Class::Request,
                    "HTTP",
                    "GET /generate_204",
                    "GET /generate_204 HTTP/1.1",
                    157,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P11)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "143", "0x2b52", "010", "0", "64", "6", "0xc8b5", "10.20.0.57",
                            "203.0.113.20",
                        ],
                        &[12],
                    ),
                    header(&anatomy::HTTP_MESSAGE, PROBE, &[0]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    PHONE,
                    Class::Response,
                    "HTTP",
                    "204 No Content",
                    "HTTP/1.1 204 No Content",
                    118,
                )
                .says(k::PACKETS_CAPTIVE_PORTAL_P12)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "104", "0x0000", "010", "0", "57", "6", "0xfb2e", "203.0.113.20",
                            "10.20.0.57",
                        ],
                        &[8, 11],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &["HTTP/1.1 204 No Content", "Date: Sun, 08 Nov 2026 10:00:00 GMT", "", "", ""],
                        &[0],
                    ),
                ]),
            ),
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Check),
            },
        ],
    },
];

/// A captive portal.
pub static CAPTIVE_PORTAL: Protocol = Protocol {
    id: "captive-portal",
    group: Group::Web,
    layer: Layer(7),
    name: k::PACKETS_CAPTIVE_PORTAL_NAME,
    title: k::PACKETS_CAPTIVE_PORTAL_TITLE,
    tagline: k::PACKETS_CAPTIVE_PORTAL_TAGLINE,
    overview: k::PACKETS_CAPTIVE_PORTAL_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_CAPTIVE_PORTAL_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_CAPTIVE_PORTAL_USED_BY),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_CAPTIVE_PORTAL_SECURITY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 8952, RFC 8910, RFC 8908"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(PHONE, ACCESS_POINT, 3),
            link(ACCESS_POINT, GATEWAY, 1),
            link(GATEWAY, PORTAL, 1),
            link(GATEWAY, INTERNET, 5),
            link(INTERNET, SERVER, 6),
        ],
    },
    steps: &STEPS,
};
