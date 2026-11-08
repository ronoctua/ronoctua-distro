//! SOCKS5: a greeting, a login in the clear, a CONNECT by name, the
//! server's own connection to the site, and the bytes it then relays both
//! ways unread.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const SOCKS: usize = 3;
const SERVER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-5, -1),
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
        kind: Kind::Tower(Emblem::Arrows),
        at: at(2, -5),
        name: k::PACKETS_DEVICE_SOCKS_SERVER,
        address: Address::V4([198, 51, 100, 80]),
    },
    Device {
        kind: Kind::Tower(Emblem::Globe),
        at: at(7, 1),
        name: k::PACKETS_DEVICE_WEB_SERVER,
        address: Address::V4([203, 0, 113, 10]),
    },
];

/// The browser's ClientHello, as TLS carries it on either connection.
const HELLO: &[&str] = &[
    "22",
    "0x0301",
    "1765",
    "1",
    "1761",
    "server_name: www.example.com",
];
/// The site's ServerHello.
const HELLO_BACK: &[&str] = &[
    "22",
    "0x0303",
    "1210",
    "2",
    "1206",
    "cipher_suite: TLS_AES_128_GCM_SHA256",
];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_SOCKS5_1_TITLE,
        sub: k::PACKETS_SOCKS5_1_SUB,
        body: k::PACKETS_SOCKS5_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "1080"),
            wire(k::PACKETS_SOCKS5_LABEL_METHODS, "0x00, 0x02"),
            wire(k::PACKETS_LABEL_REPLY, "0x02"),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SOCKS],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SOCKS,
                    Class::Handshake,
                    "SOCKS",
                    "05 02 00 02",
                    "VER=5 NMETHODS=2 METHODS=0x00,0x02",
                    58,
                )
                .says(k::PACKETS_SOCKS5_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "44",
                            "0x6611",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0xe884",
                            "192.168.1.10",
                            "198.51.100.80",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "1080", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x6d10", "0",
                        ],
                        &[1],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SOCKS,
                    CLIENT,
                    Class::Handshake,
                    "SOCKS",
                    "05 02",
                    "VER=5 METHOD=0x02",
                    56,
                )
                .says(k::PACKETS_SOCKS5_P2),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SOCKS5_2_TITLE,
        sub: k::PACKETS_SOCKS5_2_SUB,
        body: k::PACKETS_SOCKS5_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_USER, "user1"),
            wire(k::PACKETS_LABEL_STATUS, "0x00"),
            fact(k::PACKETS_LABEL_ENCRYPTION, k::PACKETS_SOCKS5_2_ENCRYPTION),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SOCKS],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SOCKS,
                    Class::Crypto,
                    "SOCKS",
                    "user1 / k7#qL2",
                    "VER=1 UNAME=user1 PASSWD=k7#qL2",
                    68,
                )
                .says(k::PACKETS_SOCKS5_P3)
                .headers(&[header(
                    &anatomy::TCP,
                    &[
                        "52344", "1080", "5", "3", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                        "0", "502", "0x0b72", "0",
                    ],
                    &[1],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SOCKS,
                    CLIENT,
                    Class::Crypto,
                    "SOCKS",
                    "01 00",
                    "VER=1 STATUS=0x00",
                    56,
                )
                .says(k::PACKETS_SOCKS5_P4),
            ),
            Beat::Light {
                at: SOCKS,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_SOCKS5_3_TITLE,
        sub: k::PACKETS_SOCKS5_3_SUB,
        body: k::PACKETS_SOCKS5_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "CONNECT (0x01)"),
            wire(k::PACKETS_LABEL_ADDRESS, "ATYP=0x03 www.example.com"),
            wire(k::PACKETS_LABEL_PORT, "443"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SOCKS,
                    Class::Request,
                    "SOCKS",
                    "CONNECT www.example.com:443",
                    "VER=5 CMD=CONNECT ATYP=DOMAINNAME www.example.com:443",
                    76,
                )
                .says(k::PACKETS_SOCKS5_P5)
                .headers(&[header(
                    &anatomy::SOCKS5,
                    &["5", "1", "0", "3", "len=15 www.example.com", "443"],
                    &[1, 3, 4, 5],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SOCKS,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "40312 → 443 [SYN] Seq=0 Win=64240 MSS=1460",
                    74,
                )
                .says(k::PACKETS_SOCKS5_P6)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "60",
                            "0xa1c0",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x326d",
                            "198.51.100.80",
                            "203.0.113.10",
                        ],
                        &[11, 12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "40312", "443", "0", "0", "10", "0", "0", "0", "0", "0", "0", "0", "1",
                            "0", "64240", "0x9a4e", "0",
                        ],
                        &[0, 1, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    SOCKS,
                    Class::Handshake,
                    "TCP",
                    "SYN-ACK",
                    "443 → 40312 [SYN, ACK] Seq=0 Ack=1 Win=65160",
                    74,
                )
                .says(k::PACKETS_SOCKS5_P7),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SOCKS5_4_TITLE,
        sub: k::PACKETS_SOCKS5_4_SUB,
        body: k::PACKETS_SOCKS5_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REPLY, "0x00"),
            wire(k::PACKETS_LABEL_ADDRESS, "198.51.100.80:40312"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_SOCKS5_4_STATE),
        ],
        focus: &[CLIENT, ROUTER, INTERNET, SOCKS],
        beats: &[
            Beat::Send(
                Packet::new(
                    SOCKS,
                    CLIENT,
                    Class::Response,
                    "SOCKS",
                    "REP=0x00",
                    "VER=5 REP=0x00 BND=198.51.100.80:40312",
                    64,
                )
                .says(k::PACKETS_SOCKS5_P8)
                .headers(&[header(
                    &anatomy::SOCKS5,
                    &["5", "0", "0", "1", "198.51.100.80", "40312"],
                    &[1, 3, 4, 5],
                )]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_SOCKS5_5_TITLE,
        sub: k::PACKETS_SOCKS5_5_SUB,
        body: k::PACKETS_SOCKS5_5_BODY,
        facts: &[
            wire(k::PACKETS_SOCKS5_LABEL_SEEN, "198.51.100.80:40312"),
            fact(k::PACKETS_LABEL_ENCRYPTION, k::PACKETS_SOCKS5_5_ENCRYPTION),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SOCKS,
                    Class::Data,
                    "TLS",
                    "Client Hello",
                    "Client Hello (SNI=www.example.com)",
                    1_878,
                )
                .says(k::PACKETS_SOCKS5_P9)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "1080", "41", "15", "5", "0", "0", "0", "0", "1", "1", "0",
                            "0", "0", "502", "0xc3f1", "0",
                        ],
                        &[1],
                    ),
                    header(&anatomy::TLS, HELLO, &[5]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SOCKS,
                    SERVER,
                    Class::Data,
                    "TLS",
                    "Client Hello",
                    "Client Hello (SNI=www.example.com)",
                    1_878,
                )
                .says(k::PACKETS_SOCKS5_P10)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "40312", "443", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x5e27", "0",
                        ],
                        &[0, 1],
                    ),
                    header(&anatomy::TLS, HELLO, &[5]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    SOCKS,
                    Class::Data,
                    "TLS",
                    "Server Hello",
                    "Server Hello, Change Cipher Spec, Application Data",
                    3_986,
                )
                .says(k::PACKETS_SOCKS5_P11)
                .headers(&[header(&anatomy::TLS, HELLO_BACK, &[])]),
            ),
            Beat::Send(
                Packet::new(
                    SOCKS,
                    CLIENT,
                    Class::Data,
                    "TLS",
                    "Server Hello",
                    "Server Hello, Change Cipher Spec, Application Data",
                    3_986,
                )
                .says(k::PACKETS_SOCKS5_P12)
                .headers(&[header(&anatomy::TLS, HELLO_BACK, &[])]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Lock),
            },
        ],
    },
];

/// SOCKS5.
pub static SOCKS5: Protocol = Protocol {
    id: "socks5",
    group: Group::Web,
    layer: Layer(5),
    name: k::PACKETS_SOCKS5_NAME,
    title: k::PACKETS_SOCKS5_TITLE,
    tagline: k::PACKETS_SOCKS5_TAGLINE,
    overview: k::PACKETS_SOCKS5_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "1080"),
        fact(k::PACKETS_FACT_TRANSPORT, k::PACKETS_SOCKS5_TRANSPORT),
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_SOCKS5_AUTHENTICATION,
        ),
        fact(k::PACKETS_FACT_USED_BY, k::PACKETS_SOCKS5_USED_BY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 1928, RFC 1929"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, SOCKS, 6),
            link(INTERNET, SERVER, 5),
        ],
    },
    steps: &STEPS,
};
