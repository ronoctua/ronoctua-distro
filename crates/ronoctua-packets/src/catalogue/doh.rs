//! DNS over HTTPS: a lookup the old way, in the clear, for comparison —
//! then the same question inside an HTTPS connection to the resolver, as a
//! POST and as a GET, and what the provider's router is left to see.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const PROVIDER: usize = 1;
const INTERNET: usize = 2;
const RESOLVER: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::CoreRouter,
        at: at(-4, -1),
        name: k::PACKETS_DEVICE_ISP_ROUTER,
        address: Address::V4([203, 0, 113, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(1, 3),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Rack,
        at: at(6, -2),
        name: k::PACKETS_DEVICE_RESOLVER,
        address: Address::V4([198, 51, 100, 1]),
    },
];

/// The question about `www.example.com`.
const WWW: &str = "www.example.com  A  IN";
/// Its answer.
const ANSWER: &str = "www.example.com  A  203.0.113.10  TTL 3600";
/// The DNS question as DoH carries it — ID 0, RD set — in hexadecimal.
const QUERY_BYTES: &str = "00 00 01 00 00 01 00 00 00 00 00 00 03 77 77 77 07 65 78 61 6d 70 6c 65 03 63 6f 6d 00 00 01 00 01";
/// The answer, the same way.
const ANSWER_BYTES: &str = "00 00 81 80 00 01 00 01 00 00 00 00 03 77 77 77 07 65 78 61 6d 70 6c 65 03 63 6f 6d 00 00 01 00 01 c0 0c 00 01 00 01 00 00 0e 10 00 04 cb 00 71 0a";

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

/// A TCP segment carrying data — PSH and ACK set: ports, numbers, window.
macro_rules! tcp {
    ($src:literal, $dst:literal, $seq:literal, $ack:literal, $win:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, $seq, $ack, "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", $win,
                "0x5a3c", "0",
            ],
            $lit,
        )
    };
}

/// The DNS message inside: a question, or a question and its answer.
macro_rules! dns {
    (question, $lit:expr) => {
        header(
            &anatomy::DNS,
            &[
                "0x0000", "0", "0", "0", "0", "1", "0", "000", "0", "1", "0", "0", "0", WWW,
            ],
            $lit,
        )
    };
    (answer, $lit:expr) => {
        header(
            &anatomy::DNS,
            &[
                "0x0000", "1", "0", "0", "0", "1", "1", "000", "0", "1", "1", "0", "0", WWW, ANSWER,
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_DOH_1_TITLE,
        sub: k::PACKETS_DOH_1_SUB,
        body: k::PACKETS_DOH_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TRANSPORT, "UDP 53"),
            wire(k::PACKETS_LABEL_QUESTION, "www.example.com A"),
        ],
        focus: &[],
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
                .says(k::PACKETS_DOH_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "61", "0x8c11", "000", "0", "64", "17", "0x02b8", "192.168.1.10",
                            "198.51.100.1",
                        ],
                        &[12],
                    ),
                    header(&anatomy::UDP, &["51000", "53", "41", "0x9c2b"], &[1]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x3f2a", "0", "0", "0", "0", "1", "0", "000", "0", "1", "0", "0", "0", WWW,
                        ],
                        &[13],
                    ),
                ]),
            ),
            Beat::Light {
                at: PROVIDER,
                prop: Some(Prop::Page),
            },
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
                .says(k::PACKETS_DOH_P2)
                .headers(&[header(
                    &anatomy::DNS,
                    &[
                        "0x3f2a", "1", "0", "0", "0", "1", "1", "000", "0", "1", "1", "0", "0", WWW, ANSWER,
                    ],
                    &[14],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DOH_2_TITLE,
        sub: k::PACKETS_DOH_2_SUB,
        body: k::PACKETS_DOH_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_URL, "https://dns.example.net/dns-query"),
            wire(k::PACKETS_LABEL_SERVER_PORT, "443"),
            wire(k::PACKETS_LABEL_CERTIFICATE, "dns.example.net"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    RESOLVER,
                    Class::Handshake,
                    "TLSv1.3",
                    "ClientHello",
                    "Client Hello (SNI=dns.example.net)",
                    1_908,
                )
                .says(k::PACKETS_DOH_P3)
                .headers(&[
                    ip!("1500", "0x2b40", "64", "0x1df5", "192.168.1.10", "198.51.100.1", &[12]),
                    tcp!("49822", "443", "1", "1", "502", &[1]),
                    header(
                        &anatomy::TLS,
                        &[
                            "22",
                            "0x0301",
                            "1795",
                            "1",
                            "1791",
                            "random · SNI=dns.example.net · ALPN=h2 · supported_versions=0x0304 · key_share",
                        ],
                        &[0, 3, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    CLIENT,
                    Class::Crypto,
                    "TLSv1.3",
                    "ServerHello + Certificate",
                    "Server Hello, Change Cipher Spec, Encrypted Extensions, Certificate, Certificate Verify, Finished",
                    4_486,
                )
                .says(k::PACKETS_DOH_P4)
                .headers(&[
                    tcp!("443", "49822", "1", "1801", "509", &[]),
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
                        &[3],
                    ),
                ]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_DOH_3_TITLE,
        sub: k::PACKETS_DOH_3_SUB,
        body: k::PACKETS_DOH_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_METHOD, "POST /dns-query"),
            wire(k::PACKETS_LABEL_CONTENT, "application/dns-message"),
            wire(k::PACKETS_LABEL_ANSWER, "A 203.0.113.10"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    RESOLVER,
                    Class::Request,
                    "DoH",
                    "POST /dns-query",
                    "HEADERS[1]: POST /dns-query, DATA[1]: 0x0000 A? www.example.com",
                    158,
                )
                .says(k::PACKETS_DOH_P5)
                .headers(&[
                    ip!("144", "0x2b43", "64", "0x233e", "192.168.1.10", "198.51.100.1", &[12]),
                    tcp!("49822", "443", "1865", "4325", "502", &[1]),
                    header(&anatomy::TLS, &["23", "0x0303", "99"], &[0]),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "POST /dns-query HTTP/2",
                            ":authority: dns.example.net",
                            "content-type: application/dns-message",
                            "content-length: 33",
                            "",
                            QUERY_BYTES,
                        ],
                        &[0, 2],
                    ),
                    dns!(question, &[0, 13]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    CLIENT,
                    Class::Response,
                    "DoH",
                    "200",
                    "HEADERS[1]: 200 OK, DATA[1]: 0x0000 A www.example.com → 203.0.113.10",
                    164,
                )
                .says(k::PACKETS_DOH_P6)
                .headers(&[
                    tcp!("443", "49822", "4325", "1969", "509", &[]),
                    header(&anatomy::TLS, &["23", "0x0303", "105"], &[0]),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "HTTP/2 200",
                            "content-type: application/dns-message",
                            "cache-control: max-age=3600",
                            "content-length: 49",
                            "",
                            ANSWER_BYTES,
                        ],
                        &[0, 1],
                    ),
                    dns!(answer, &[14]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DOH_4_TITLE,
        sub: k::PACKETS_DOH_4_SUB,
        body: k::PACKETS_DOH_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_METHOD, "GET"),
            wire(k::PACKETS_LABEL_PATH, "/dns-query?dns=AAABAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    RESOLVER,
                    Class::Request,
                    "DoH",
                    "GET ?dns=AAAB…",
                    "HEADERS[3]: GET /dns-query?dns=AAABAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB",
                    146,
                )
                .says(k::PACKETS_DOH_P7)
                .headers(&[
                    ip!("132", "0x2b44", "64", "0x2349", "192.168.1.10", "198.51.100.1", &[]),
                    tcp!("49822", "443", "1969", "4435", "502", &[]),
                    header(&anatomy::TLS, &["23", "0x0303", "87"], &[0]),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "GET /dns-query?dns=AAABAAABAAAAAAAAA3d3dwdleGFtcGxlA2NvbQAAAQAB HTTP/2",
                            ":authority: dns.example.net",
                            ":scheme: https",
                            "accept: application/dns-message",
                            "",
                        ],
                        &[0],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    CLIENT,
                    Class::Response,
                    "DoH",
                    "200",
                    "HEADERS[3]: 200 OK, DATA[3]: 0x0000 A www.example.com → 203.0.113.10",
                    164,
                )
                .says(k::PACKETS_DOH_P8)
                .headers(&[
                    header(&anatomy::TLS, &["23", "0x0303", "105"], &[]),
                    header(
                        &anatomy::HTTP_MESSAGE,
                        &[
                            "HTTP/2 200",
                            "content-type: application/dns-message",
                            "cache-control: max-age=3600",
                            "content-length: 49",
                            "",
                            ANSWER_BYTES,
                        ],
                        &[2],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DOH_5_TITLE,
        sub: k::PACKETS_DOH_5_SUB,
        body: k::PACKETS_DOH_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_DESTINATION, "198.51.100.1:443"),
            fact(k::PACKETS_LABEL_TRUST, k::PACKETS_DOH_5_TRUST),
        ],
        focus: &[CLIENT, PROVIDER, RESOLVER],
        beats: &[
            Beat::Light {
                at: PROVIDER,
                prop: Some(Prop::Lock),
            },
            Beat::Light {
                at: RESOLVER,
                prop: Some(Prop::Page),
            },
            Beat::Rest,
        ],
    },
];

/// DNS over HTTPS.
pub static DOH: Protocol = Protocol {
    id: "doh",
    group: Group::Naming,
    layer: Layer(7),
    name: k::PACKETS_DOH_NAME,
    title: k::PACKETS_DOH_TITLE,
    tagline: k::PACKETS_DOH_TAGLINE,
    overview: k::PACKETS_DOH_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "443"),
        wire(k::PACKETS_FACT_TRANSPORT, "HTTP/2, HTTP/3"),
        fact(k::PACKETS_FACT_FORMAT, k::PACKETS_DOH_FORMAT),
        wire(k::PACKETS_FACT_ENCRYPTION, "TLS 1.3"),
        wire(k::PACKETS_FACT_DEFINED, "RFC 8484"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, PROVIDER, 3),
            link(PROVIDER, INTERNET, 4),
            link(INTERNET, RESOLVER, 6),
        ],
    },
    steps: &STEPS,
};
