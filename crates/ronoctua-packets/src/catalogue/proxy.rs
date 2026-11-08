//! A forward HTTP proxy: a request sent to the proxy with the whole URL,
//! fetched by the proxy, cached, a blocked site refused, and a CONNECT
//! tunnel that carries HTTPS through it unread.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const PROXY_HOST: usize = 1;
const INTERNET: usize = 2;
const SERVER: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Tower(Emblem::Arrows),
        at: at(-4, -2),
        name: k::PACKETS_DEVICE_PROXY,
        address: Address::V4([192, 168, 1, 3]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(1, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(7, -2),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// The laptop's request for the page, as it reaches the proxy.
const ASK: &[&str] = &[
    "GET http://www.example.com/ HTTP/1.1",
    "Host: www.example.com",
    "User-Agent: Mozilla/5.0",
    "Accept: text/html",
    "",
];

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_PROXY_1_TITLE,
        sub: k::PACKETS_PROXY_1_SUB,
        body: k::PACKETS_PROXY_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "proxy.example:3128"),
            wire(
                k::PACKETS_LABEL_REQUEST,
                "GET http://www.example.com/ HTTP/1.1",
            ),
        ],
        focus: &[CLIENT, PROXY_HOST],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "http://www.example.com/",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    PROXY_HOST,
                    Class::Request,
                    "HTTP",
                    "GET http://www.example.com/",
                    "GET http://www.example.com/ HTTP/1.1",
                    161,
                )
                .says(k::PACKETS_PROXY_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "147",
                            "0x5a10",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x5cf7",
                            "192.168.1.10",
                            "192.168.1.3",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "3128", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x8a31", "0",
                        ],
                        &[1],
                    ),
                    header(&anatomy::HTTP_MESSAGE, ASK, &[0]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_PROXY_2_TITLE,
        sub: k::PACKETS_PROXY_2_SUB,
        body: k::PACKETS_PROXY_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REQUEST, "GET / HTTP/1.1"),
            wire(k::PACKETS_LABEL_STATUS, "200 OK"),
            wire(k::PACKETS_LABEL_CACHE, "Cache-Control: max-age=600"),
        ],
        focus: &[PROXY_HOST, INTERNET, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    PROXY_HOST,
                    SERVER,
                    Class::Request,
                    "HTTP",
                    "GET /",
                    "GET / HTTP/1.1",
                    150,
                )
                .says(k::PACKETS_PROXY_P2)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "40112", "80", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x3e9c", "0",
                        ],
                        &[0, 1],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "GET / HTTP/1.1",
                            "Host: www.example.com",
                            "Via: 1.1 proxy.example",
                            "X-Forwarded-For: 192.168.1.10",
                            "",
                        ],
                        &[0, 2, 3],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    PROXY_HOST,
                    Class::Response,
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/html)",
                    13_149,
                )
                .says(k::PACKETS_PROXY_P3)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 200 OK",
                        "Content-Type: text/html; charset=UTF-8",
                        "Cache-Control: max-age=600",
                        "Content-Length: 12553",
                        "",
                        "<!doctype html><html><head><title>Example…",
                    ],
                    &[0, 2],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_PROXY_3_TITLE,
        sub: k::PACKETS_PROXY_3_SUB,
        body: k::PACKETS_PROXY_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CACHE, "max-age=600"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_PROXY_3_STATE),
        ],
        focus: &[CLIENT, PROXY_HOST],
        beats: &[
            Beat::Send(
                Packet::new(
                    PROXY_HOST,
                    CLIENT,
                    Class::Response,
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/html)",
                    13_173,
                )
                .says(k::PACKETS_PROXY_P4)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 200 OK",
                        "Via: 1.1 proxy.example",
                        "Cache-Control: max-age=600",
                        "Content-Length: 12553",
                        "",
                        "<!doctype html><html><head><title>Example…",
                    ],
                    &[1],
                )]),
            ),
            Beat::Light {
                at: PROXY_HOST,
                prop: Some(Prop::Page),
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    PROXY_HOST,
                    Class::Request,
                    "HTTP",
                    "GET http://www.example.com/",
                    "GET http://www.example.com/ HTTP/1.1",
                    161,
                )
                .says(k::PACKETS_PROXY_P5)
                .headers(&[header(&anatomy::HTTP_MESSAGE, ASK, &[])]),
            ),
            Beat::Send(
                Packet::new(
                    PROXY_HOST,
                    CLIENT,
                    Class::Response,
                    "HTTP",
                    "200 OK, Age: 120",
                    "HTTP/1.1 200 OK (text/html)",
                    13_183,
                )
                .says(k::PACKETS_PROXY_P6)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 200 OK",
                        "Age: 120",
                        "Via: 1.1 proxy.example",
                        "Content-Length: 12553",
                        "",
                        "<!doctype html><html><head><title>Example…",
                    ],
                    &[1],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_PROXY_4_TITLE,
        sub: k::PACKETS_PROXY_4_SUB,
        body: k::PACKETS_PROXY_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_URL, "http://games.example.net/"),
            wire(k::PACKETS_LABEL_STATUS, "403 Forbidden"),
            fact(k::PACKETS_LABEL_REASON, k::PACKETS_PROXY_4_REASON),
        ],
        focus: &[CLIENT, PROXY_HOST],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "http://games.example.net/",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    PROXY_HOST,
                    Class::Request,
                    "HTTP",
                    "GET http://games.example.net/",
                    "GET http://games.example.net/ HTTP/1.1",
                    165,
                )
                .says(k::PACKETS_PROXY_P7)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "GET http://games.example.net/ HTTP/1.1",
                        "Host: games.example.net",
                        "User-Agent: Mozilla/5.0",
                        "Accept: text/html",
                        "",
                    ],
                    &[0, 1],
                )]),
            ),
            Beat::Light {
                at: PROXY_HOST,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    PROXY_HOST,
                    CLIENT,
                    Class::Refusal,
                    "HTTP",
                    "403 Forbidden",
                    "HTTP/1.1 403 Forbidden (text/html)",
                    3_379,
                )
                .says(k::PACKETS_PROXY_P8)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 403 Forbidden",
                        "Content-Type: text/html; charset=UTF-8",
                        "Content-Length: 3104",
                        "Cache-Control: no-store",
                        "",
                        "<!doctype html><html><head><title>403…",
                    ],
                    &[0],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_PROXY_5_TITLE,
        sub: k::PACKETS_PROXY_5_SUB,
        body: k::PACKETS_PROXY_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_METHOD, "CONNECT"),
            wire(k::PACKETS_LABEL_DESTINATION, "www.example.com:443"),
            wire(k::PACKETS_LABEL_STATUS, "200 Connection established"),
        ],
        focus: &[CLIENT, PROXY_HOST],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "https://www.example.com/",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    PROXY_HOST,
                    Class::Request,
                    "HTTP",
                    "CONNECT www.example.com:443",
                    "CONNECT www.example.com:443 HTTP/1.1",
                    176,
                )
                .says(k::PACKETS_PROXY_P9)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "162",
                            "0x5a20",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x5cd8",
                            "192.168.1.10",
                            "192.168.1.3",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "52350", "3128", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x91d4", "0",
                        ],
                        &[1],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "CONNECT www.example.com:443 HTTP/1.1",
                            "Host: www.example.com:443",
                            "Proxy-Connection: keep-alive",
                            "User-Agent: Mozilla/5.0",
                            "",
                        ],
                        &[0],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PROXY_HOST,
                    CLIENT,
                    Class::Response,
                    "HTTP",
                    "200 Connection established",
                    "HTTP/1.1 200 Connection established",
                    93,
                )
                .says(k::PACKETS_PROXY_P10)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &["HTTP/1.1 200 Connection established"],
                    &[0],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_PROXY_6_TITLE,
        sub: k::PACKETS_PROXY_6_SUB,
        body: k::PACKETS_PROXY_6_BODY,
        facts: &[
            wire(
                k::PACKETS_LABEL_TUNNEL,
                "192.168.1.10 ⇄ 192.168.1.3 ⇄ 203.0.113.10:443",
            ),
            fact(k::PACKETS_PROXY_LABEL_SEES, k::PACKETS_PROXY_6_SEES),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Crypto,
                    "TLS",
                    "Client Hello",
                    "Client Hello (SNI=www.example.com)",
                    1_878,
                )
                .says(k::PACKETS_PROXY_P11)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "52350", "3128", "123", "40", "5", "0", "0", "0", "0", "1", "1", "0",
                            "0", "0", "502", "0x77e0", "0",
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
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Crypto,
                    "TLS",
                    "Server Hello",
                    "Server Hello, Change Cipher Spec, Application Data",
                    3_986,
                )
                .says(k::PACKETS_PROXY_P12)
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
];

/// A forward HTTP proxy.
pub static PROXY: Protocol = Protocol {
    id: "proxy",
    group: Group::Web,
    layer: Layer(7),
    name: k::PACKETS_PROXY_NAME,
    title: k::PACKETS_PROXY_TITLE,
    tagline: k::PACKETS_PROXY_TAGLINE,
    overview: k::PACKETS_PROXY_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_PROXY_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        wire(k::PACKETS_FACT_METHODS, "GET http://…, CONNECT host:443"),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_PROXY_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 9110, RFC 9112"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, PROXY_HOST, 1),
            link(PROXY_HOST, INTERNET, 5),
            link(INTERNET, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
