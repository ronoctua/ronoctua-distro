//! FTP: a file fetched over two connections — the control connection on
//! port 21, a login in plain text that anyone on the way can read, passive
//! mode, and a second TCP connection that carries the file.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const ATTACKER: usize = 1;
const ACCESS_POINT: usize = 2;
const INTERNET: usize = 3;
const SERVER: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Laptop,
        at: at(-9, -4),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::V4([192, 168, 1, 66]),
    },
    Device {
        kind: Kind::AccessPoint,
        at: at(-4, 0),
        name: k::PACKETS_DEVICE_ACCESS_POINT,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(1, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Storage,
        at: at(7, -2),
        name: k::PACKETS_DEVICE_FILE_SERVER,
        address: Address::V4([203, 0, 113, 21]),
    },
];

/// One line of the dialogue, as the anatomy opens it — lit, or not.
macro_rules! line_of {
    ($text:literal) => {
        header(&anatomy::LINE, &[$text], &[])
    };
    ($text:literal, lit) => {
        header(&anatomy::LINE, &[$text], &[0])
    };
}

/// The laptop's way to the server, for the steps' frames.
const WAY: &[usize] = &[LAPTOP, ACCESS_POINT, INTERNET, SERVER];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_FTP_1_TITLE,
        sub: k::PACKETS_FTP_1_SUB,
        body: k::PACKETS_FTP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "21"),
            wire(k::PACKETS_LABEL_REPLY, "220"),
            fact(k::PACKETS_LABEL_CHANNEL, k::PACKETS_FTP_1_CHANNEL),
        ],
        focus: WAY,
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "ftp ftp.example.com",
            },
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Response,
                    "FTP",
                    "220",
                    "S: 220 ftp.example.com",
                    75,
                )
                .says(k::PACKETS_FTP_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "61",
                            "0x0000",
                            "010",
                            "0",
                            "52",
                            "6",
                            "0x8a3c",
                            "203.0.113.21",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "21", "50230", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "509", "0x13d7", "0",
                        ],
                        &[0],
                    ),
                    line_of!("220 ftp.example.com", lit),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_FTP_2_TITLE,
        sub: k::PACKETS_FTP_2_SUB,
        body: k::PACKETS_FTP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_USER, "alice"),
            wire(k::PACKETS_LABEL_COMMAND, "PASS s3cret"),
            fact(k::PACKETS_LABEL_ENCRYPTION, k::PACKETS_FTP_2_ENCRYPTION),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SERVER,
                    Class::Request,
                    "FTP",
                    "USER alice",
                    "C: USER alice",
                    66,
                )
                .says(k::PACKETS_FTP_P2)
                .headers(&[line_of!("USER alice", lit)]),
            ),
            Beat::Send(
                Packet::new(SERVER, LAPTOP, Class::Response, "FTP", "331", "S: 331", 59)
                    .says(k::PACKETS_FTP_P3)
                    .headers(&[line_of!("331", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SERVER,
                    Class::Request,
                    "FTP",
                    "PASS s3cret",
                    "C: PASS s3cret",
                    67,
                )
                .says(k::PACKETS_FTP_P4)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "53",
                            "0x3b07",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x5e12",
                            "192.168.1.10",
                            "203.0.113.21",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "50230", "21", "13", "27", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "502", "0x7f40", "0",
                        ],
                        &[1],
                    ),
                    line_of!("PASS s3cret", lit),
                ]),
            ),
            Beat::Light {
                at: ATTACKER,
                prop: Some(Prop::Key),
            },
            Beat::Send(
                Packet::new(SERVER, LAPTOP, Class::Response, "FTP", "230", "S: 230", 59)
                    .says(k::PACKETS_FTP_P5)
                    .headers(&[line_of!("230", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_FTP_3_TITLE,
        sub: k::PACKETS_FTP_3_SUB,
        body: k::PACKETS_FTP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_REPLY, "227 (203,0,113,21,195,149)"),
            wire(k::PACKETS_LABEL_PORT, "195 × 256 + 149 = 50069"),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(LAPTOP, SERVER, Class::Request, "FTP", "PASV", "C: PASV", 60)
                    .says(k::PACKETS_FTP_P6)
                    .headers(&[line_of!("PASV", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Response,
                    "FTP",
                    "227",
                    "S: 227 (203,0,113,21,195,149)",
                    82,
                )
                .says(k::PACKETS_FTP_P7)
                .headers(&[line_of!("227 (203,0,113,21,195,149)", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_FTP_4_TITLE,
        sub: k::PACKETS_FTP_4_SUB,
        body: k::PACKETS_FTP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "RETR example.tar.gz"),
            fact(k::PACKETS_LABEL_CHANNEL, k::PACKETS_FTP_4_CHANNEL),
            wire(k::PACKETS_LABEL_REPLY, "150, 226"),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SERVER,
                    Class::Handshake,
                    "TCP",
                    "SYN",
                    "50236 → 50069 [SYN] Seq=0 Win=64240",
                    66,
                )
                .says(k::PACKETS_FTP_P8)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "52",
                            "0x3b0a",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x5e31",
                            "192.168.1.10",
                            "203.0.113.21",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "50236", "50069", "0", "0", "8", "0", "0", "0", "0", "0", "0", "0",
                            "1", "0", "64240", "0xc2a9", "0",
                        ],
                        &[1, 12],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SERVER,
                    Class::Request,
                    "FTP",
                    "RETR example.tar.gz",
                    "C: RETR example.tar.gz",
                    75,
                )
                .says(k::PACKETS_FTP_P9)
                .headers(&[line_of!("RETR example.tar.gz", lit)]),
            ),
            Beat::Send(
                Packet::new(SERVER, LAPTOP, Class::Response, "FTP", "150", "S: 150", 59)
                    .says(k::PACKETS_FTP_P10)
                    .headers(&[line_of!("150", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Data,
                    "FTP-DATA",
                    "example.tar.gz",
                    "50069 → 50236 Len=1048576",
                    1_087_402,
                )
                .says(k::PACKETS_FTP_P11)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "1500",
                            "0x0000",
                            "010",
                            "0",
                            "52",
                            "6",
                            "0x2fd8",
                            "203.0.113.21",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "50069", "50236", "1", "1", "5", "0", "0", "0", "0", "1", "0", "0",
                            "0", "0", "509", "0x9b14", "0",
                        ],
                        &[0],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(SERVER, LAPTOP, Class::Response, "FTP", "226", "S: 226", 59)
                    .says(k::PACKETS_FTP_P12)
                    .headers(&[line_of!("226", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_FTP_5_TITLE,
        sub: k::PACKETS_FTP_5_SUB,
        body: k::PACKETS_FTP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PROTOCOL, "SFTP, FTPS"),
            wire(k::PACKETS_LABEL_PORT, "22 (SFTP), 21 AUTH TLS / 990 (FTPS)"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Lock),
            },
            Beat::Light {
                at: SERVER,
                prop: Some(Prop::Lock),
            },
            Beat::Rest,
        ],
    },
];

/// FTP.
pub static FTP: Protocol = Protocol {
    id: "ftp",
    group: Group::Mail,
    layer: Layer(7),
    name: k::PACKETS_FTP_NAME,
    title: k::PACKETS_FTP_TITLE,
    tagline: k::PACKETS_FTP_TAGLINE,
    overview: k::PACKETS_FTP_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_FTP_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_FTP_SECURITY),
        wire(k::PACKETS_FTP_LABEL_REPLACED_BY, "SFTP, FTPS"),
        wire(k::PACKETS_FACT_DEFINED, "RFC 959, RFC 2428"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ACCESS_POINT, 2),
            link(ATTACKER, ACCESS_POINT, 2),
            link(ACCESS_POINT, INTERNET, 6),
            link(INTERNET, SERVER, 8),
        ],
    },
    steps: &STEPS,
};
