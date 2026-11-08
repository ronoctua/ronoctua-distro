//! BitTorrent: a file shared by pieces — the info hash from a magnet link, the
//! tracker's list of peers, the handshake and the bitfield, blocks asked of
//! two peers at once, each piece checked against its hash and offered on.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const INTERNET: usize = 1;
const TRACKER: usize = 2;
const SEED: usize = 3;
const PEER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 2),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 0, 2, 10]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(-2, 0),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::List),
        at: at(4, -5),
        name: k::PACKETS_DEVICE_TRACKER,
        address: Address::V4([203, 0, 113, 30]),
    },
    Device {
        kind: Kind::Desktop,
        at: at(8, 0),
        name: k::PACKETS_BITTORRENT_DEVICE_SEED,
        address: Address::V4([198, 51, 100, 21]),
    },
    Device {
        kind: Kind::Laptop,
        at: at(4, 5),
        name: k::PACKETS_DEVICE_PEER,
        address: Address::V4([203, 0, 113, 77]),
    },
];

/// An IPv4 header carrying TCP, `$len` bytes in all, and its checksum.
macro_rules! ip {
    ($len:literal, $checksum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", $len, "0x7c3a", "010", "0", "64", "6", $checksum, $src, $dst,
            ],
            $lit,
        )
    };
}

/// A TCP header from `$src` to `$dst`, with data.
macro_rules! tcp {
    ($src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "1024",
                "0x3e8b", "0",
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_BITTORRENT_1_TITLE,
        sub: k::PACKETS_BITTORRENT_1_SUB,
        body: k::PACKETS_BITTORRENT_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_HASH, "btih:6a8f2d0c…5a79"),
            wire(k::PACKETS_LABEL_PIECE, "2800 × 256 KiB"),
            wire(k::PACKETS_LABEL_SIZE, "700 MiB"),
        ],
        focus: &[LAPTOP],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "magnet:?xt=urn:btih:6a8f2d0c4b9e1f3a5c7d9e0b2a4c6e8f1d3b5a79&tr=http%3A%2F%2Ftracker.example.org%2Fannounce",
            },
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Page),
            },
        ],
    },
    Step {
        title: k::PACKETS_BITTORRENT_2_TITLE,
        sub: k::PACKETS_BITTORRENT_2_SUB,
        body: k::PACKETS_BITTORRENT_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REQUEST, "GET /announce?…&event=started"),
            wire(k::PACKETS_LABEL_SWARM, "complete=1 incomplete=2"),
            wire(k::PACKETS_LABEL_INTERVAL, "1800 s"),
        ],
        focus: &[LAPTOP, INTERNET, TRACKER],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    TRACKER,
                    Class::Lookup,
                    "HTTP",
                    "GET /announce",
                    "GET /announce?info_hash=%6a%8f%2d…%5a%79&port=51413&left=734003200&event=started HTTP/1.1",
                    326,
                )
                .says(k::PACKETS_BITTORRENT_P1)
                .headers(&[
                    ip!("312", "0xbf5c", "192.0.2.10", "203.0.113.30", &[12]),
                    tcp!("50820", "80", &[1]),
                    header(
                        &anatomy::HTTP_REQUEST,
                        &[
                            "GET /announce?info_hash=%6a%8f%2d…%5a%79&peer_id=-TR4060-k8hj0wgej6ch&port=51413\
                             &uploaded=0&downloaded=0&left=734003200&event=started&compact=1 HTTP/1.1",
                            "Host: tracker.example.org",
                            "User-Agent: Transmission/4.0.6",
                            "Accept: */*",
                            "",
                        ],
                        &[0, 1],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    TRACKER,
                    LAPTOP,
                    Class::Lookup,
                    "HTTP",
                    "200 OK",
                    "HTTP/1.1 200 OK (text/plain)",
                    188,
                )
                .says(k::PACKETS_BITTORRENT_P2)
                .headers(&[header(
                    &anatomy::HTTP_RESPONSE,
                    &[
                        "HTTP/1.1 200 OK",
                        "Content-Type: text/plain",
                        "Content-Length: 69",
                        "",
                        "d8:completei1e10:incompletei2e8:intervali1800e5:peers12:…e",
                    ],
                    &[0, 4],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_BITTORRENT_3_TITLE,
        sub: k::PACKETS_BITTORRENT_3_SUB,
        body: k::PACKETS_BITTORRENT_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PROTOCOL, "BitTorrent protocol"),
            wire(k::PACKETS_LABEL_PEER, "-TR4060-k8hj0wgej6ch"),
            wire(k::PACKETS_LABEL_PORT, "51413"),
        ],
        focus: &[LAPTOP, INTERNET, SEED],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SEED,
                    Class::Handshake,
                    "BitTorrent",
                    "Handshake",
                    "Handshake info_hash=6a8f2d0c…5a79 peer_id=-TR4060-k8hj0wgej6ch",
                    122,
                )
                .says(k::PACKETS_BITTORRENT_P3)
                .headers(&[
                    ip!("108", "0xd1fe", "192.0.2.10", "198.51.100.21", &[]),
                    tcp!("50822", "51413", &[1]),
                    header(
                        &anatomy::BITTORRENT,
                        &[
                            "19",
                            "BitTorrent protocol",
                            "0x0000000000100005",
                            "6a8f2d0c4b9e1f3a5c7d9e0b2a4c6e8f1d3b5a79",
                            "-TR4060-k8hj0wgej6ch",
                        ],
                        &[0, 1, 2, 3, 4],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SEED,
                    LAPTOP,
                    Class::Handshake,
                    "BitTorrent",
                    "Handshake",
                    "Handshake info_hash=6a8f2d0c…5a79 peer_id=-qB5010-x7Hd3kPq9Lm2",
                    122,
                )
                .says(k::PACKETS_BITTORRENT_P4)
                .headers(&[header(
                    &anatomy::BITTORRENT,
                    &[
                        "19",
                        "BitTorrent protocol",
                        "0x0000000000100005",
                        "6a8f2d0c4b9e1f3a5c7d9e0b2a4c6e8f1d3b5a79",
                        "-qB5010-x7Hd3kPq9Lm2",
                    ],
                    &[3, 4],
                )]),
            ),
            Beat::Send(
                Packet::new(SEED, LAPTOP, Class::Control, "BitTorrent", "Bitfield", "Bitfield, Len:0x15f", 409)
                    .says(k::PACKETS_BITTORRENT_P5),
            ),
        ],
    },
    Step {
        title: k::PACKETS_BITTORRENT_4_TITLE,
        sub: k::PACKETS_BITTORRENT_4_SUB,
        body: k::PACKETS_BITTORRENT_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PIECE, "0, 1913"),
            wire(k::PACKETS_LABEL_SIZE, "16 KiB"),
            wire(k::PACKETS_LABEL_OFFSET, "begin=0"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(LAPTOP, SEED, Class::Control, "BitTorrent", "Interested", "Interested", 60)
                    .says(k::PACKETS_BITTORRENT_P6),
            ),
            Beat::Send(
                Packet::new(SEED, LAPTOP, Class::Control, "BitTorrent", "Unchoke", "Unchoke", 60)
                    .says(k::PACKETS_BITTORRENT_P7),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SEED,
                    Class::Request,
                    "BitTorrent",
                    "Request 0",
                    "Request, Piece (Idx:0x0,Begin:0x0,Len:0x4000)",
                    71,
                )
                .says(k::PACKETS_BITTORRENT_P8)
                .headers(&[
                    ip!("57", "0xd231", "192.0.2.10", "198.51.100.21", &[12]),
                    tcp!("50822", "51413", &[1]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    PEER,
                    Class::Request,
                    "BitTorrent",
                    "Request 1913",
                    "Request, Piece (Idx:0x779,Begin:0x0,Len:0x4000)",
                    71,
                )
                .says(k::PACKETS_BITTORRENT_P9)
                .headers(&[
                    ip!("57", "0xc02c", "192.0.2.10", "203.0.113.77", &[12]),
                    tcp!("50824", "6881", &[1]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SEED,
                    LAPTOP,
                    Class::Data,
                    "BitTorrent",
                    "Piece 0",
                    "Piece, Idx:0x0,Begin:0x0,Len:0x4000",
                    16_451,
                )
                .says(k::PACKETS_BITTORRENT_P10),
            ),
            Beat::Send(
                Packet::new(
                    PEER,
                    LAPTOP,
                    Class::Data,
                    "BitTorrent",
                    "Piece 1913",
                    "Piece, Idx:0x779,Begin:0x0,Len:0x4000",
                    16_451,
                )
                .says(k::PACKETS_BITTORRENT_P11),
            ),
        ],
    },
    Step {
        title: k::PACKETS_BITTORRENT_5_TITLE,
        sub: k::PACKETS_BITTORRENT_5_SUB,
        body: k::PACKETS_BITTORRENT_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_HASH, "SHA-1 (v1), SHA-256 (v2)"),
            wire(k::PACKETS_LABEL_COMMAND, "HAVE 0"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_BITTORRENT_5_VERDICT),
        ],
        focus: &[LAPTOP, INTERNET, PEER],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    PEER,
                    Class::Control,
                    "BitTorrent",
                    "Have 0",
                    "Have, Piece (Idx:0x0)",
                    63,
                )
                .says(k::PACKETS_BITTORRENT_P12)
                .headers(&[
                    ip!("49", "0xc034", "192.0.2.10", "203.0.113.77", &[]),
                    tcp!("50824", "6881", &[]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PEER,
                    LAPTOP,
                    Class::Request,
                    "BitTorrent",
                    "Request 0",
                    "Request, Piece (Idx:0x0,Begin:0x0,Len:0x4000)",
                    71,
                )
                .says(k::PACKETS_BITTORRENT_P13),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    PEER,
                    Class::Data,
                    "BitTorrent",
                    "Piece 0",
                    "Piece, Idx:0x0,Begin:0x0,Len:0x4000",
                    16_451,
                )
                .says(k::PACKETS_BITTORRENT_P14),
            ),
        ],
    },
];

/// BitTorrent.
pub static BITTORRENT: Protocol = Protocol {
    id: "bittorrent",
    group: Group::Things,
    layer: Layer(7),
    name: k::PACKETS_BITTORRENT_NAME,
    title: k::PACKETS_BITTORRENT_TITLE,
    tagline: k::PACKETS_BITTORRENT_TAGLINE,
    overview: k::PACKETS_BITTORRENT_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_BITTORRENT_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP, uTP (UDP)"),
        fact(k::PACKETS_FACT_MODEL, k::PACKETS_BITTORRENT_MODEL),
        wire(
            k::PACKETS_FACT_DEFINED,
            "BEP 3, BEP 5 (DHT), BEP 9, BEP 52 (v2)",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, INTERNET, 6),
            link(INTERNET, TRACKER, 8),
            link(INTERNET, SEED, 10),
            link(INTERNET, PEER, 12),
        ],
    },
    steps: &STEPS,
};
