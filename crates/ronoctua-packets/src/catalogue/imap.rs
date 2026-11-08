//! IMAP: a mailbox that stays on the server — the laptop logs in, selects
//! INBOX, lists the messages and reads one; the phone, idling on its own
//! connection, hears at once of the flag the laptop set and of new mail.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const ROUTER: usize = 1;
const PHONE: usize = 2;
const INTERNET: usize = 3;
const MAIL: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 4),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-5, 1),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::V4([192, 168, 1, 1]),
    },
    Device {
        kind: Kind::Phone,
        at: at(-8, -4),
        name: k::PACKETS_DEVICE_PHONE,
        address: Address::V4([10, 64, 12, 7]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(0, 0),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Envelope),
        at: at(7, -2),
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

/// The laptop's way to the server, for the steps' frames.
const LAPTOP_WAY: &[usize] = &[LAPTOP, ROUTER, INTERNET, MAIL];

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_IMAP_1_TITLE,
        sub: k::PACKETS_IMAP_1_SUB,
        body: k::PACKETS_IMAP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "993"),
            wire(k::PACKETS_LABEL_COMMAND, "a1 LOGIN alice"),
            wire(k::PACKETS_LABEL_REPLY, "a1 OK"),
        ],
        focus: LAPTOP_WAY,
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
                    "IMAP",
                    "* OK",
                    "S: * OK [CAPABILITY IMAP4rev2 IDLE AUTH=PLAIN] mail.example.net",
                    138,
                )
                .says(k::PACKETS_IMAP_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "124", "0x0000", "010", "0", "56", "6", "0x2e71", "198.51.100.25",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(&anatomy::TCP, &["993", "50440"], &[0]),
                    header(&anatomy::TLS, &["23", "0x0303", "79"], &[0]),
                    line_of!("* OK [CAPABILITY IMAP4rev2 IDLE AUTH=PLAIN] mail.example.net", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Crypto,
                    "IMAP",
                    "a1 LOGIN",
                    "C: a1 LOGIN alice s3cret",
                    99,
                )
                .says(k::PACKETS_IMAP_P2)
                .headers(&[
                    header(&anatomy::TLS, &["23", "0x0303", "40"], &[0]),
                    line_of!("a1 LOGIN alice s3cret", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(MAIL, LAPTOP, Class::Response, "IMAP", "a1 OK", "S: a1 OK LOGIN", 89)
                    .says(k::PACKETS_IMAP_P3)
                    .headers(&[line_of!("a1 OK LOGIN", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_IMAP_2_TITLE,
        sub: k::PACKETS_IMAP_2_SUB,
        body: k::PACKETS_IMAP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_MAILBOX, "INBOX"),
            wire(k::PACKETS_LABEL_REPLY, "* 5 EXISTS"),
            wire(k::PACKETS_LABEL_MODE, "READ-WRITE"),
        ],
        focus: LAPTOP_WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "IMAP",
                    "a2 SELECT INBOX",
                    "C: a2 SELECT INBOX",
                    93,
                )
                .says(k::PACKETS_IMAP_P4)
                .headers(&[line_of!("a2 SELECT INBOX", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "IMAP",
                    "* 5 EXISTS",
                    "S: * 5 EXISTS | * FLAGS (…) | * OK [UIDVALIDITY 1762560000] | a2 OK [READ-WRITE] SELECT",
                    198,
                )
                .says(k::PACKETS_IMAP_P5)
                .headers(&[
                    line_of!("* 5 EXISTS", lit),
                    line_of!("* FLAGS (\\Answered \\Flagged \\Deleted \\Seen \\Draft)"),
                    line_of!("* OK [UIDVALIDITY 1762560000]"),
                    line_of!("a2 OK [READ-WRITE] SELECT", lit),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_IMAP_3_TITLE,
        sub: k::PACKETS_IMAP_3_SUB,
        body: k::PACKETS_IMAP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "a3 FETCH 1:5 (FLAGS ENVELOPE)"),
            wire(k::PACKETS_LABEL_FLAGS, "1, 2 \\Seen — 3, 4, 5 ()"),
        ],
        focus: LAPTOP_WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "IMAP",
                    "a3 FETCH 1:5",
                    "C: a3 FETCH 1:5 (FLAGS ENVELOPE)",
                    107,
                )
                .says(k::PACKETS_IMAP_P6)
                .headers(&[line_of!("a3 FETCH 1:5 (FLAGS ENVELOPE)", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "IMAP",
                    "* FETCH (FLAGS ENVELOPE)",
                    "S: * 1 FETCH (FLAGS (\\Seen) ENVELOPE (…)) … * 5 FETCH (FLAGS () ENVELOPE (…)) | a3 OK FETCH",
                    1_589,
                )
                .says(k::PACKETS_IMAP_P7)
                .headers(&[
                    line_of!("* 1 FETCH (FLAGS (\\Seen) ENVELOPE (…))", lit),
                    line_of!("* 2 FETCH (FLAGS (\\Seen \\Answered) ENVELOPE (…))"),
                    line_of!("* 3 FETCH (FLAGS () ENVELOPE (…))", lit),
                    line_of!("…"),
                    line_of!("a3 OK FETCH"),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_IMAP_4_TITLE,
        sub: k::PACKETS_IMAP_4_SUB,
        body: k::PACKETS_IMAP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "a4 FETCH 3 BODY[]"),
            wire(k::PACKETS_LABEL_SIZE, "2066"),
            wire(k::PACKETS_LABEL_FLAGS, "3 \\Seen"),
        ],
        focus: LAPTOP_WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "IMAP",
                    "a4 FETCH 3 BODY[]",
                    "C: a4 FETCH 3 BODY[]",
                    95,
                )
                .says(k::PACKETS_IMAP_P8)
                .headers(&[line_of!("a4 FETCH 3 BODY[]", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Data,
                    "IMAP",
                    "* 3 FETCH BODY[]",
                    "S: * 3 FETCH (FLAGS (\\Seen) BODY[] {2066} …) | a4 OK FETCH",
                    2_198,
                )
                .says(k::PACKETS_IMAP_P9)
                .headers(&[
                    line_of!("* 3 FETCH (FLAGS (\\Seen) BODY[] {2066}", lit),
                    line_of!("…"),
                    line_of!(")"),
                    line_of!("a4 OK FETCH"),
                ]),
            ),
            Beat::Light {
                at: MAIL,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_IMAP_5_TITLE,
        sub: k::PACKETS_IMAP_5_SUB,
        body: k::PACKETS_IMAP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "b7 IDLE"),
            wire(k::PACKETS_LABEL_FLAGS, "* 3 FETCH (FLAGS (\\Seen))"),
            wire(k::PACKETS_LABEL_REPLY, "* 6 EXISTS"),
        ],
        focus: &[PHONE, INTERNET, MAIL],
        beats: &[
            Beat::Send(
                Packet::new(PHONE, MAIL, Class::Request, "IMAP", "b7 IDLE", "C: b7 IDLE", 85)
                    .says(k::PACKETS_IMAP_P10)
                    .headers(&[
                        header(
                            &anatomy::IPV4,
                            &[
                                "4", "5", "0", "0", "71", "0x6c03", "010", "0", "64", "6", "0x0b7d", "10.64.12.7",
                                "198.51.100.25",
                            ],
                            &[11],
                        ),
                        header(&anatomy::TCP, &["41022", "993"], &[1]),
                        line_of!("b7 IDLE", lit),
                    ]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    PHONE,
                    Class::Response,
                    "IMAP",
                    "* 3 FETCH (\\Seen)",
                    "S: * 3 FETCH (FLAGS (\\Seen))",
                    103,
                )
                .says(k::PACKETS_IMAP_P11)
                .headers(&[line_of!("* 3 FETCH (FLAGS (\\Seen))", lit)]),
            ),
            Beat::Light {
                at: MAIL,
                prop: Some(Prop::Envelope),
            },
            Beat::Send(
                Packet::new(MAIL, PHONE, Class::Response, "IMAP", "* 6 EXISTS", "S: * 6 EXISTS", 88)
                    .says(k::PACKETS_IMAP_P12)
                    .headers(&[line_of!("* 6 EXISTS", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_IMAP_6_TITLE,
        sub: k::PACKETS_IMAP_6_SUB,
        body: k::PACKETS_IMAP_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "a5 LOGOUT"),
            wire(k::PACKETS_LABEL_REPLY, "* BYE"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_IMAP_6_STATE),
        ],
        focus: LAPTOP_WAY,
        beats: &[
            Beat::Send(
                Packet::new(LAPTOP, MAIL, Class::Control, "IMAP", "a5 LOGOUT", "C: a5 LOGOUT", 87)
                    .says(k::PACKETS_IMAP_P13)
                    .headers(&[line_of!("a5 LOGOUT", lit)]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Control,
                    "IMAP",
                    "* BYE",
                    "S: * BYE IMAP4rev2 | a5 OK LOGOUT",
                    107,
                )
                .says(k::PACKETS_IMAP_P14)
                .headers(&[line_of!("* BYE IMAP4rev2", lit), line_of!("a5 OK LOGOUT")]),
            ),
            Beat::Rest,
        ],
    },
];

/// IMAP.
pub static IMAP: Protocol = Protocol {
    id: "imap",
    group: Group::Mail,
    layer: Layer(7),
    name: k::PACKETS_IMAP_NAME,
    title: k::PACKETS_IMAP_TITLE,
    tagline: k::PACKETS_IMAP_TAGLINE,
    overview: k::PACKETS_IMAP_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_IMAP_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_FORMAT, k::PACKETS_IMAP_FORMAT),
        fact(k::PACKETS_FACT_MODEL, k::PACKETS_IMAP_MODEL),
        wire(k::PACKETS_FACT_DEFINED, "RFC 9051, RFC 2177"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(ROUTER, INTERNET, 5),
            link(PHONE, INTERNET, 12),
            link(INTERNET, MAIL, 4),
        ],
    },
    steps: &STEPS,
};
