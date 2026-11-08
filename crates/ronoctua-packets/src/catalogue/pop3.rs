//! POP3: mail downloaded and taken off the server — TLS on port 995, a
//! login, what is waiting, one message fetched, marked and deleted at QUIT.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const MAIL: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
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
        at: at(1, 2),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Envelope),
        at: at(6, -2),
        name: k::PACKETS_DEVICE_MAIL_SERVER,
        address: Address::V4([198, 51, 100, 25]),
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

/// The way between the laptop and the server, for the steps' frames.
const WAY: &[usize] = &[LAPTOP, ROUTER, INTERNET, MAIL];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_POP3_1_TITLE,
        sub: k::PACKETS_POP3_1_SUB,
        body: k::PACKETS_POP3_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "995"),
            wire(k::PACKETS_LABEL_ENCRYPTION, "TLS 1.3"),
            wire(k::PACKETS_LABEL_REPLY, "+OK"),
        ],
        focus: WAY,
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Lock),
            },
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "POP",
                    "+OK",
                    "S: +OK POP3 mail.example.net",
                    103,
                )
                .says(k::PACKETS_POP3_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "89",
                            "0x0000",
                            "010",
                            "0",
                            "56",
                            "6",
                            "0x56a0",
                            "198.51.100.25",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(&anatomy::TCP, &["995", "50320"], &[0]),
                    header(&anatomy::TLS, &["23", "0x0303", "44"], &[0]),
                    line_of!("+OK POP3 mail.example.net", lit),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_POP3_2_TITLE,
        sub: k::PACKETS_POP3_2_SUB,
        body: k::PACKETS_POP3_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_USER, "alice"),
            wire(k::PACKETS_LABEL_COMMAND, "USER, PASS"),
            wire(k::PACKETS_LABEL_STATE, "TRANSACTION"),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "POP",
                    "USER alice",
                    "C: USER alice",
                    88,
                )
                .says(k::PACKETS_POP3_P2)
                .headers(&[line_of!("USER alice", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Crypto,
                    "POP",
                    "PASS",
                    "C: PASS s3cret",
                    89,
                )
                .says(k::PACKETS_POP3_P3)
                .headers(&[
                    header(&anatomy::TLS, &["23", "0x0303", "30"], &[0]),
                    line_of!("PASS s3cret", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(MAIL, LAPTOP, Class::Response, "POP", "+OK", "S: +OK", 81)
                    .says(k::PACKETS_POP3_P4)
                    .headers(&[line_of!("+OK", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_POP3_3_TITLE,
        sub: k::PACKETS_POP3_3_SUB,
        body: k::PACKETS_POP3_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "STAT, LIST"),
            wire(k::PACKETS_LABEL_REPLY, "+OK 2 3271"),
            wire(k::PACKETS_LABEL_SIZE, "1205, 2066"),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(LAPTOP, MAIL, Class::Request, "POP", "STAT", "C: STAT", 82)
                    .says(k::PACKETS_POP3_P5)
                    .headers(&[line_of!("STAT", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "POP",
                    "+OK 2 3271",
                    "S: +OK 2 3271",
                    88,
                )
                .says(k::PACKETS_POP3_P6)
                .headers(&[line_of!("+OK 2 3271", lit)]),
            ),
            Beat::Send(
                Packet::new(LAPTOP, MAIL, Class::Request, "POP", "LIST", "C: LIST", 82)
                    .says(k::PACKETS_POP3_P7)
                    .headers(&[line_of!("LIST", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "POP",
                    "1 1205, 2 2066",
                    "S: +OK 2 3271 | 1 1205 | 2 2066 | .",
                    107,
                )
                .says(k::PACKETS_POP3_P8)
                .headers(&[
                    line_of!("+OK 2 3271"),
                    line_of!("1 1205", lit),
                    line_of!("2 2066", lit),
                    line_of!(".", lit),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_POP3_4_TITLE,
        sub: k::PACKETS_POP3_4_SUB,
        body: k::PACKETS_POP3_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "RETR 1"),
            wire(k::PACKETS_LABEL_REPLY, "+OK 1205"),
            wire(k::PACKETS_LABEL_SIZE, "1205"),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "POP",
                    "RETR 1",
                    "C: RETR 1",
                    84,
                )
                .says(k::PACKETS_POP3_P9)
                .headers(&[line_of!("RETR 1", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Data,
                    "POP",
                    "+OK 1205",
                    "S: +OK 1205 | From: carol@example.org … | .",
                    1_294,
                )
                .says(k::PACKETS_POP3_P10)
                .headers(&[
                    line_of!("+OK 1205", lit),
                    line_of!("From: carol@example.org"),
                    line_of!("To: alice@example.net"),
                    line_of!("…"),
                    line_of!(".", lit),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Envelope),
            },
        ],
    },
    Step {
        title: k::PACKETS_POP3_5_TITLE,
        sub: k::PACKETS_POP3_5_SUB,
        body: k::PACKETS_POP3_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "DELE 1, QUIT"),
            wire(k::PACKETS_LABEL_STATE, "UPDATE"),
            fact(k::PACKETS_LABEL_MAILBOX, k::PACKETS_POP3_5_MAILBOX),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "POP",
                    "DELE 1",
                    "C: DELE 1",
                    84,
                )
                .says(k::PACKETS_POP3_P11)
                .headers(&[line_of!("DELE 1", lit)]),
            ),
            Beat::Send(
                Packet::new(MAIL, LAPTOP, Class::Response, "POP", "+OK", "S: +OK", 81)
                    .says(k::PACKETS_POP3_P12)
                    .headers(&[line_of!("+OK", lit)]),
            ),
            Beat::Send(
                Packet::new(LAPTOP, MAIL, Class::Control, "POP", "QUIT", "C: QUIT", 82)
                    .says(k::PACKETS_POP3_P13)
                    .headers(&[line_of!("QUIT", lit)]),
            ),
            Beat::Send(
                Packet::new(MAIL, LAPTOP, Class::Control, "POP", "+OK", "S: +OK", 81)
                    .says(k::PACKETS_POP3_P14)
                    .headers(&[line_of!("+OK", lit)]),
            ),
            Beat::Rest,
        ],
    },
];

/// POP3.
pub static POP3: Protocol = Protocol {
    id: "pop3",
    group: Group::Mail,
    layer: Layer(7),
    name: k::PACKETS_POP3_NAME,
    title: k::PACKETS_POP3_TITLE,
    tagline: k::PACKETS_POP3_TAGLINE,
    overview: k::PACKETS_POP3_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_POP3_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_FORMAT, k::PACKETS_POP3_FORMAT),
        fact(k::PACKETS_FACT_MODEL, k::PACKETS_POP3_MODEL),
        wire(k::PACKETS_FACT_DEFINED, "RFC 1939, RFC 8314"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, MAIL, 5),
        ],
    },
    steps: &STEPS,
};
