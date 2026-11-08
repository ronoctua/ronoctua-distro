//! HTTP: a page asked for and sent — with the DNS lookup and the TCP
//! handshake that come before it, as a browser does them.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const RESOLVER: usize = 3;
const SERVER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 4),
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
        at: at(1, 3),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Rack,
        at: at(2, -5),
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

/// The client's IPv4 header towards the server, `$len` bytes in all.
macro_rules! ip_out {
    ($len:literal, $checksum:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4",
                "5",
                "0",
                "0",
                $len,
                "0x1c46",
                "010",
                "0",
                "64",
                "6",
                $checksum,
                "192.168.1.10",
                "203.0.113.10",
            ],
            $lit,
        )
    };
}
/// The server's, back, `$len` bytes in all.
macro_rules! ip_back {
    ($len:literal, $checksum:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4",
                "5",
                "0",
                "0",
                $len,
                "0x0000",
                "010",
                "0",
                "56",
                "6",
                $checksum,
                "203.0.113.10",
                "192.168.1.10",
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_HTTP_1_TITLE,
        sub: k::PACKETS_HTTP_1_SUB,
        body: k::PACKETS_HTTP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_URL, "http://www.example.com/"),
            wire(k::PACKETS_LABEL_METHOD, "GET"),
            wire(k::PACKETS_LABEL_NEXT_HOP, "192.168.1.1"),
        ],
        focus: &[CLIENT],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "http://www.example.com/",
            },
            Beat::Rest,
        ],
    },
    Step {
        title: k::PACKETS_HTTP_2_TITLE,
        sub: k::PACKETS_HTTP_2_SUB,
        body: k::PACKETS_HTTP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_RESOLVER, "8.8.8.8"),
            wire(k::PACKETS_LABEL_TRANSPORT, "UDP 53"),
            wire(k::PACKETS_LABEL_ANSWER, "A 203.0.113.10"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, RESOLVER],
        beats: &[
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
                .says(k::PACKETS_HTTP_P1)
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
                        &[9, 12],
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
                            "0",
                            "0",
                            "1",
                            "0",
                            "0",
                            "0",
                            "www.example.com  A  IN",
                        ],
                        &[1, 5, 13],
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
                .says(k::PACKETS_HTTP_P2)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "77",
                            "0x0000",
                            "000",
                            "0",
                            "121",
                            "17",
                            "0x5d02",
                            "8.8.8.8",
                            "192.168.1.10",
                        ],
                        &[11],
                    ),
                    header(&anatomy::UDP, &["53", "51000", "57", "0x41ae"], &[0]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x3f2a",
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
                        &[1, 10, 14],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_HTTP_3_TITLE,
        sub: k::PACKETS_HTTP_3_SUB,
        body: k::PACKETS_HTTP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "80"),
            wire(k::PACKETS_LABEL_CLIENT_PORT, "52344"),
            wire(k::PACKETS_LABEL_HANDSHAKE, "SYN → SYN-ACK → ACK"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "52344 → 80 [SYN] Seq=0 Win=64240",
                    66,
                )
                .says(k::PACKETS_HTTP_P3)
                .headers(&[
                    ip_out!("52", "0x20c1", &[9]),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "80", "0", "0", "8", "0", "0", "0", "0", "0", "0", "0", "1",
                            "0", "64240", "0x3c1f", "0",
                        ],
                        &[2, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "TCP",
                    "SYN-ACK",
                    "80 → 52344 [SYN, ACK] Seq=0 Ack=1",
                    66,
                )
                .says(k::PACKETS_HTTP_P4)
                .headers(&[
                    ip_back!("52", "0x4507", &[]),
                    header(
                        &anatomy::TCP,
                        &[
                            "80", "52344", "0", "1", "8", "0", "0", "0", "0", "1", "0", "0", "1",
                            "0", "65160", "0x9e02", "0",
                        ],
                        &[3, 9, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "ACK",
                    "52344 → 80 [ACK] Seq=1 Ack=1",
                    54,
                )
                .says(k::PACKETS_HTTP_P5)
                .headers(&[
                    ip_out!("40", "0x20cd", &[]),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "80", "1", "1", "5", "0", "0", "0", "0", "1", "0", "0", "0",
                            "0", "502", "0x1b7e", "0",
                        ],
                        &[3, 9],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_HTTP_4_TITLE,
        sub: k::PACKETS_HTTP_4_SUB,
        body: k::PACKETS_HTTP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REQUEST, "GET / HTTP/1.1"),
            wire(k::PACKETS_LABEL_STATUS, "200 OK"),
            wire(k::PACKETS_LABEL_CONTENT, "text/html"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Request,
                    "HTTP",
                    "GET /",
                    "GET / HTTP/1.1",
                    484,
                )
                .says(k::PACKETS_HTTP_P6)
                .headers(&[
                    ip_out!("470", "0x1f1f", &[]),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "80", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x77c0", "0",
                        ],
                        &[10],
                    ),
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
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/html)",
                    12_800,
                )
                .says(k::PACKETS_HTTP_P7)
                .headers(&[
                    ip_back!("1500", "0x3f5f", &[]),
                    header(
                        &anatomy::TCP,
                        &[
                            "80", "52344", "1", "431", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "509", "0x5f18", "0",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::HTTP_RESPONSE,
                        &[
                            "HTTP/1.1 200 OK",
                            "Content-Type: text/html; charset=UTF-8",
                            "Content-Length: 12553",
                            "",
                            "<!doctype html><html><head><title>Example…",
                        ],
                        &[0, 1, 2],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_HTTP_5_TITLE,
        sub: k::PACKETS_HTTP_5_SUB,
        body: k::PACKETS_HTTP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CONTENT, "text/html"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_HTTP_5_STATE),
        ],
        focus: &[CLIENT],
        beats: &[
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Page),
            },
            Beat::Rest,
        ],
    },
];

/// HTTP.
pub static HTTP: Protocol = Protocol {
    id: "http",
    group: Group::Web,
    layer: Layer(7),
    name: k::PACKETS_HTTP_NAME,
    title: k::PACKETS_HTTP_TITLE,
    tagline: k::PACKETS_HTTP_TAGLINE,
    overview: k::PACKETS_HTTP_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_HTTP_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_FORMAT, k::PACKETS_HTTP_FORMAT),
        wire(k::PACKETS_FACT_METHODS, "GET, POST, PUT, DELETE, HEAD"),
        wire(k::PACKETS_FACT_DEFINED, "RFC 9110, RFC 9112"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, RESOLVER, 1),
            link(INTERNET, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
