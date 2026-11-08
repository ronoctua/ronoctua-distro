//! A reverse proxy: TLS ended in front of the servers, requests passed as
//! plain HTTP to two app servers in turn, a static file answered from the
//! cache, and an app server taken out of the rotation by a health check.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const FRONT: usize = 3;
const APP_1: usize = 4;
const APP_2: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 4),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-6, 0),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([198, 51, 100, 7]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(-2, 3),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Arrows),
        at: at(2, -2),
        name: k::PACKETS_DEVICE_REVERSE_PROXY,
        address: Address::V4([203, 0, 113, 10]),
    },
    Device {
        kind: Kind::Tower(Emblem::Gear),
        at: at(8, -5),
        name: k::PACKETS_DEVICE_APP_SERVER,
        address: Address::V4([10, 0, 0, 11]),
    },
    Device {
        kind: Kind::Tower(Emblem::Gear),
        at: at(8, 2),
        name: k::PACKETS_DEVICE_APP_SERVER,
        address: Address::V4([10, 0, 0, 12]),
    },
];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_REVERSE_PROXY_1_TITLE,
        sub: k::PACKETS_REVERSE_PROXY_1_SUB,
        body: k::PACKETS_REVERSE_PROXY_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CERTIFICATE, "SAN DNS:www.example.com"),
            wire(k::PACKETS_LABEL_PORT, "443"),
            fact(
                k::PACKETS_LABEL_ENCRYPTION,
                k::PACKETS_REVERSE_PROXY_1_ENCRYPTION,
            ),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, FRONT],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    FRONT,
                    Class::Crypto,
                    "TLS",
                    "Client Hello",
                    "Client Hello (SNI=www.example.com)",
                    1_878,
                )
                .says(k::PACKETS_REVERSE_PROXY_P1)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "443", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x5b2e", "0",
                        ],
                        &[1],
                    ),
                    header(
                        &anatomy::TLS,
                        &[
                            "22",
                            "0x0301",
                            "1765",
                            "1",
                            "1761",
                            "server_name: www.example.com",
                        ],
                        &[3, 5],
                    ),
                ]),
            ),
            Beat::Light {
                at: FRONT,
                prop: Some(Prop::Certificate),
            },
            Beat::Send(
                Packet::new(
                    FRONT,
                    CLIENT,
                    Class::Crypto,
                    "TLS",
                    "Server Hello",
                    "Server Hello, Change Cipher Spec, Application Data",
                    3_986,
                )
                .says(k::PACKETS_REVERSE_PROXY_P2)
                .headers(&[header(
                    &anatomy::TLS,
                    &[
                        "22",
                        "0x0303",
                        "1210",
                        "2",
                        "1206",
                        "cipher_suite: TLS_AES_128_GCM_SHA256",
                    ],
                    &[3],
                )]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_REVERSE_PROXY_2_TITLE,
        sub: k::PACKETS_REVERSE_PROXY_2_SUB,
        body: k::PACKETS_REVERSE_PROXY_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REQUEST, "GET /cart HTTP/1.1"),
            wire(k::PACKETS_LABEL_DESTINATION, "10.0.0.11:8080"),
            wire(k::PACKETS_LABEL_SOURCE, "X-Forwarded-For: 198.51.100.7"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    FRONT,
                    Class::Request,
                    "HTTP",
                    "GET /cart",
                    "GET /cart HTTP/1.1",
                    185,
                )
                .says(k::PACKETS_REVERSE_PROXY_P3)
                .headers(&[
                    header(
                        &anatomy::TLS,
                        &["23", "0x0303", "126", "", "", "c4 91 0e 7b 2a f3 58 d6 …"],
                        &[0],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "GET /cart HTTP/1.1",
                            "Host: www.example.com",
                            "Cookie: sid=7f3a9c",
                            "Accept: text/html",
                            "",
                        ],
                        &[0],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    FRONT,
                    APP_1,
                    Class::Request,
                    "HTTP",
                    "GET /cart",
                    "GET /cart HTTP/1.1",
                    220,
                )
                .says(k::PACKETS_REVERSE_PROXY_P4)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "206",
                            "0x3e01",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0xe81d",
                            "10.0.0.1",
                            "10.0.0.11",
                        ],
                        &[11, 12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "41822", "8080", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x1f6a", "0",
                        ],
                        &[1],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "GET /cart HTTP/1.1",
                            "Host: www.example.com",
                            "X-Forwarded-For: 198.51.100.7",
                            "X-Forwarded-Proto: https",
                            "",
                        ],
                        &[2, 3],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    APP_1,
                    FRONT,
                    Class::Response,
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/html)",
                    8_851,
                )
                .says(k::PACKETS_REVERSE_PROXY_P5)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 200 OK",
                        "Content-Type: text/html; charset=UTF-8",
                        "Cache-Control: private, no-store",
                        "Content-Length: 8412",
                        "",
                        "<!doctype html><html><head><title>Cart…",
                    ],
                    &[2],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    FRONT,
                    CLIENT,
                    Class::Response,
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/html)",
                    8_873,
                )
                .says(k::PACKETS_REVERSE_PROXY_P6)
                .headers(&[header(
                    &anatomy::TLS,
                    &["23", "0x0303", "8544", "", "", "5e 02 b7 c1 9d 44 0f 6a …"],
                    &[0],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_REVERSE_PROXY_3_TITLE,
        sub: k::PACKETS_REVERSE_PROXY_3_SUB,
        body: k::PACKETS_REVERSE_PROXY_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "10.0.0.12:8080"),
            fact(
                k::PACKETS_LABEL_ALGORITHM,
                k::PACKETS_REVERSE_PROXY_3_ALGORITHM,
            ),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    FRONT,
                    Class::Request,
                    "HTTP",
                    "GET /products/42",
                    "GET /products/42 HTTP/1.1",
                    192,
                )
                .says(k::PACKETS_REVERSE_PROXY_P7),
            ),
            Beat::Send(
                Packet::new(
                    FRONT,
                    APP_2,
                    Class::Request,
                    "HTTP",
                    "GET /products/42",
                    "GET /products/42 HTTP/1.1",
                    227,
                )
                .says(k::PACKETS_REVERSE_PROXY_P8)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "213",
                            "0x3e02",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0xe814",
                            "10.0.0.1",
                            "10.0.0.12",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "GET /products/42 HTTP/1.1",
                            "Host: www.example.com",
                            "X-Forwarded-For: 198.51.100.7",
                            "X-Forwarded-Proto: https",
                            "",
                        ],
                        &[0],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_REVERSE_PROXY_4_TITLE,
        sub: k::PACKETS_REVERSE_PROXY_4_SUB,
        body: k::PACKETS_REVERSE_PROXY_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CACHE, "public, max-age=86400"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_REVERSE_PROXY_4_STATE),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, FRONT],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    FRONT,
                    Class::Request,
                    "HTTP",
                    "GET /static/logo.png",
                    "GET /static/logo.png HTTP/1.1",
                    174,
                )
                .says(k::PACKETS_REVERSE_PROXY_P9),
            ),
            Beat::Send(
                Packet::new(
                    FRONT,
                    CLIENT,
                    Class::Response,
                    "HTTP",
                    "200 OK, Age: 3605",
                    "HTTP/1.1 200 OK (image/png)",
                    6_637,
                )
                .says(k::PACKETS_REVERSE_PROXY_P10)
                .headers(&[
                    header(
                        &anatomy::TLS,
                        &["23", "0x0303", "6362", "", "", "a7 1c 4e 90 d2 33 f8 0b …"],
                        &[],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "HTTP/1.1 200 OK",
                            "Content-Type: image/png",
                            "Cache-Control: public, max-age=86400",
                            "Age: 3605",
                            "",
                            "89 50 4e 47 0d 0a 1a 0a …",
                        ],
                        &[2, 3],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_REVERSE_PROXY_5_TITLE,
        sub: k::PACKETS_REVERSE_PROXY_5_SUB,
        body: k::PACKETS_REVERSE_PROXY_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REQUEST, "GET /healthz"),
            wire(k::PACKETS_LABEL_STATUS, "503 Service Unavailable"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_REVERSE_PROXY_5_VERDICT),
        ],
        focus: &[FRONT, APP_1, APP_2],
        beats: &[
            Beat::Send(
                Packet::new(
                    FRONT,
                    APP_2,
                    Class::Control,
                    "HTTP",
                    "GET /healthz",
                    "GET /healthz HTTP/1.1",
                    101,
                )
                .says(k::PACKETS_REVERSE_PROXY_P11)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &["GET /healthz HTTP/1.1", "Host: 10.0.0.12:8080", "", "", ""],
                    &[0],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    APP_2,
                    FRONT,
                    Class::Refusal,
                    "HTTP",
                    "503 Service Unavailable",
                    "HTTP/1.1 503 Service Unavailable",
                    126,
                )
                .says(k::PACKETS_REVERSE_PROXY_P12)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 503 Service Unavailable",
                        "Retry-After: 30",
                        "Content-Length: 0",
                        "",
                        "",
                    ],
                    &[0, 1],
                )]),
            ),
            Beat::Light {
                at: APP_2,
                prop: Some(Prop::Cross),
            },
        ],
    },
];

/// A reverse proxy.
pub static REVERSE_PROXY: Protocol = Protocol {
    id: "reverse-proxy",
    group: Group::Web,
    layer: Layer(7),
    name: k::PACKETS_REVERSE_PROXY_NAME,
    title: k::PACKETS_REVERSE_PROXY_TITLE,
    tagline: k::PACKETS_REVERSE_PROXY_TAGLINE,
    overview: k::PACKETS_REVERSE_PROXY_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_REVERSE_PROXY_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_REVERSE_PROXY_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 9110, RFC 7239"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 5),
            link(INTERNET, FRONT, 5),
            link(FRONT, APP_1, 1),
            link(FRONT, APP_2, 1),
        ],
    },
    steps: &STEPS,
};
