//! WebSocket: an HTTP connection upgraded to a lasting two-way channel —
//! a masked frame from the page, messages the server pushes unasked, a ping
//! answered, and a clean close.

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
        kind: Kind::Tower(Emblem::Hub),
        at: at(7, -1),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_WEBSOCKET_1_TITLE,
        sub: k::PACKETS_WEBSOCKET_1_SUB,
        body: k::PACKETS_WEBSOCKET_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_URL, "ws://www.example.com/chat"),
            wire(k::PACKETS_LABEL_UPGRADE, "websocket"),
            wire(k::PACKETS_LABEL_VERSION, "13"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SERVER],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "ws://www.example.com/chat",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "HTTP",
                    "Upgrade: websocket",
                    "GET /chat HTTP/1.1",
                    244,
                )
                .says(k::PACKETS_WEBSOCKET_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "230",
                            "0x1c46",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x200f",
                            "192.168.1.10",
                            "203.0.113.10",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "80", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x6b1d", "0",
                        ],
                        &[1],
                    ),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "GET /chat HTTP/1.1",
                            "Upgrade: websocket",
                            "Connection: Upgrade",
                            "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==",
                            "",
                        ],
                        &[0, 1, 2, 3],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WEBSOCKET_2_TITLE,
        sub: k::PACKETS_WEBSOCKET_2_SUB,
        body: k::PACKETS_WEBSOCKET_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_STATUS, "101 Switching Protocols"),
            wire(k::PACKETS_LABEL_HASH, "SHA-1, base64"),
            wire(k::PACKETS_LABEL_STATE, "OPEN"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "HTTP",
                    "101 Switching Protocols",
                    "HTTP/1.1 101 Switching Protocols",
                    183,
                )
                .says(k::PACKETS_WEBSOCKET_P2)
                .headers(&[header(
                    &anatomy::HTTP_MESSAGE,
                    &[
                        "HTTP/1.1 101 Switching Protocols",
                        "Upgrade: websocket",
                        "Connection: Upgrade",
                        "Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=",
                        "",
                    ],
                    &[0, 3],
                )]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_WEBSOCKET_3_TITLE,
        sub: k::PACKETS_WEBSOCKET_3_SUB,
        body: k::PACKETS_WEBSOCKET_3_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_OPCODE, k::PACKETS_WEBSOCKET_3_OPCODE),
            wire(k::PACKETS_WEBSOCKET_LABEL_MASK, "0x37fa213d"),
            wire(k::PACKETS_LABEL_SIZE, "2 + 4 + 19 B"),
        ],
        focus: &[],
        beats: &[Beat::Send(
            Packet::new(
                CLIENT,
                SERVER,
                Class::Data,
                "WebSocket",
                "Text [MASKED]",
                "Text [FIN] [MASKED] Len=19",
                79,
            )
            .says(k::PACKETS_WEBSOCKET_P3)
            .headers(&[
                header(
                    &anatomy::IPV4,
                    &[
                        "4",
                        "5",
                        "0",
                        "0",
                        "65",
                        "0x1c47",
                        "010",
                        "0",
                        "64",
                        "6",
                        "0x20b3",
                        "192.168.1.10",
                        "203.0.113.10",
                    ],
                    &[],
                ),
                header(
                    &anatomy::TCP,
                    &[
                        "52344", "80", "191", "130", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                        "0", "501", "0x2f0c", "0",
                    ],
                    &[2],
                ),
                header(
                    &anatomy::WEBSOCKET,
                    &[
                        "1",
                        "000",
                        "0x1",
                        "1",
                        "19",
                        "0x37fa213d",
                        "4c d8 52 48 55 89 42 4f 5e 98 44 1f 0d d8 64 65 76 d8 5c",
                    ],
                    &[0, 2, 3, 5, 6],
                ),
            ]),
        )],
    },
    Step {
        title: k::PACKETS_WEBSOCKET_4_TITLE,
        sub: k::PACKETS_WEBSOCKET_4_SUB,
        body: k::PACKETS_WEBSOCKET_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_FRAME, "FIN=1 opcode=0x1 MASK=0"),
            wire(k::PACKETS_LABEL_SIZE, "2 + 30 B"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Data,
                    "WebSocket",
                    "Text",
                    "Text [FIN] Len=30",
                    86,
                )
                .says(k::PACKETS_WEBSOCKET_P4)
                .headers(&[header(
                    &anatomy::WEBSOCKET,
                    &[
                        "1",
                        "000",
                        "0x1",
                        "0",
                        "30",
                        "",
                        r#"{"symbol":"EXA","price":42.17}"#,
                    ],
                    &[3, 4, 6],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Data,
                    "WebSocket",
                    "Text",
                    "Text [FIN] Len=30",
                    86,
                )
                .says(k::PACKETS_WEBSOCKET_P5)
                .headers(&[header(
                    &anatomy::WEBSOCKET,
                    &[
                        "1",
                        "000",
                        "0x1",
                        "0",
                        "30",
                        "",
                        r#"{"symbol":"EXA","price":42.19}"#,
                    ],
                    &[6],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WEBSOCKET_5_TITLE,
        sub: k::PACKETS_WEBSOCKET_5_SUB,
        body: k::PACKETS_WEBSOCKET_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_OPCODE, "0x9 → 0xA"),
            fact(k::PACKETS_LABEL_INTERVAL, k::PACKETS_WEBSOCKET_5_INTERVAL),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Control,
                    "WebSocket",
                    "Ping",
                    "Ping [FIN]",
                    56,
                )
                .says(k::PACKETS_WEBSOCKET_P6)
                .headers(&[header(
                    &anatomy::WEBSOCKET,
                    &["1", "000", "0x9", "0", "0", "", ""],
                    &[2],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Control,
                    "WebSocket",
                    "Pong",
                    "Pong [FIN] [MASKED]",
                    60,
                )
                .says(k::PACKETS_WEBSOCKET_P7)
                .headers(&[header(
                    &anatomy::WEBSOCKET,
                    &["1", "000", "0xA", "1", "0", "0x5c1e8a02", ""],
                    &[2, 3, 5],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WEBSOCKET_6_TITLE,
        sub: k::PACKETS_WEBSOCKET_6_SUB,
        body: k::PACKETS_WEBSOCKET_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_OPCODE, "0x8"),
            wire(k::PACKETS_LABEL_STATUS, "1000"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_WEBSOCKET_6_STATE),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Control,
                    "WebSocket",
                    "Close 1000",
                    "Connection Close [FIN] [MASKED] Code=1000",
                    62,
                )
                .says(k::PACKETS_WEBSOCKET_P8)
                .headers(&[header(
                    &anatomy::WEBSOCKET,
                    &["1", "000", "0x8", "1", "2", "0x9a4b77e1", "99 a3"],
                    &[2, 4, 6],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Control,
                    "WebSocket",
                    "Close 1000",
                    "Connection Close [FIN] Code=1000",
                    58,
                )
                .says(k::PACKETS_WEBSOCKET_P9)
                .headers(&[header(
                    &anatomy::WEBSOCKET,
                    &["1", "000", "0x8", "0", "2", "", "03 e8"],
                    &[2, 6],
                )]),
            ),
        ],
    },
];

/// WebSocket.
pub static WEBSOCKET: Protocol = Protocol {
    id: "websocket",
    group: Group::Web,
    layer: Layer(7),
    name: k::PACKETS_WEBSOCKET_NAME,
    title: k::PACKETS_WEBSOCKET_TITLE,
    tagline: k::PACKETS_WEBSOCKET_TAGLINE,
    overview: k::PACKETS_WEBSOCKET_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_WEBSOCKET_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_FORMAT, k::PACKETS_WEBSOCKET_FORMAT),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_WEBSOCKET_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 6455, RFC 8441, RFC 9220"),
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
