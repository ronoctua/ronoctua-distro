//! SMTP: an email sent — submitted to your provider's server on port 587,
//! encrypted with STARTTLS, logged in, handed over with its envelope; then
//! the MX lookup and the relay to the recipient's server on port 25.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const INTERNET: usize = 1;
const MAIL: usize = 2;
const RESOLVER: usize = 3;
const RECIPIENT: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Cloud,
        at: at(-1, 3),
        name: k::PACKETS_DEVICE_INTERNET,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Envelope),
        at: at(-4, -3),
        name: k::PACKETS_DEVICE_MAIL_SERVER,
        address: Address::V4([198, 51, 100, 25]),
    },
    Device {
        kind: Kind::Rack,
        at: at(1, -6),
        name: k::PACKETS_DEVICE_RESOLVER,
        address: Address::V4([198, 51, 100, 53]),
    },
    Device {
        kind: Kind::Tower(Emblem::Envelope),
        at: at(7, 0),
        name: k::PACKETS_DEVICE_RECIPIENT_SERVER,
        address: Address::V4([203, 0, 113, 25]),
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

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_SMTP_1_TITLE,
        sub: k::PACKETS_SMTP_1_SUB,
        body: k::PACKETS_SMTP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "587"),
            wire(k::PACKETS_LABEL_COMMAND, "EHLO [192.168.1.10]"),
            wire(k::PACKETS_LABEL_REPLY, "250 STARTTLS"),
        ],
        focus: &[LAPTOP, INTERNET, MAIL],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "bob@example.com",
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "SMTP",
                    "EHLO",
                    "C: EHLO [192.168.1.10]",
                    75,
                )
                .says(k::PACKETS_SMTP_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "61", "0x2f10", "010", "0", "64", "6", "0x1fac", "192.168.1.10",
                            "198.51.100.25",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "50112", "587", "1", "29", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "502", "0x6e1b",
                            "0",
                        ],
                        &[1],
                    ),
                    line_of!("EHLO [192.168.1.10]", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "SMTP",
                    "250 STARTTLS",
                    "S: 250-mail.example.net … 250 STARTTLS",
                    150,
                )
                .says(k::PACKETS_SMTP_P2)
                .headers(&[
                    line_of!("250-mail.example.net"),
                    line_of!("250-PIPELINING"),
                    line_of!("250-SIZE 52428800"),
                    line_of!("250-ENHANCEDSTATUSCODES"),
                    line_of!("250 STARTTLS", lit),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMTP_2_TITLE,
        sub: k::PACKETS_SMTP_2_SUB,
        body: k::PACKETS_SMTP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ENCRYPTION, "STARTTLS → TLS 1.3"),
            wire(k::PACKETS_LABEL_COMMAND, "AUTH PLAIN"),
            wire(k::PACKETS_LABEL_REPLY, "235 2.7.0"),
        ],
        focus: &[LAPTOP, INTERNET, MAIL],
        beats: &[
            Beat::Send(
                Packet::new(LAPTOP, MAIL, Class::Crypto, "SMTP", "STARTTLS", "C: STARTTLS", 64)
                    .says(k::PACKETS_SMTP_P3)
                    .headers(&[line_of!("STARTTLS", lit)]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Lock),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Crypto,
                    "SMTP",
                    "AUTH PLAIN",
                    "C: AUTH PLAIN AGFsaWNlQGV4YW1wbGUubmV0AHMzY3JldA==",
                    125,
                )
                .says(k::PACKETS_SMTP_P4)
                .headers(&[
                    header(&anatomy::TLS, &["23", "0x0303", "66"], &[0]),
                    line_of!("AUTH PLAIN AGFsaWNlQGV4YW1wbGUubmV0AHMzY3JldA==", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(MAIL, LAPTOP, Class::Response, "SMTP", "235", "S: 235 2.7.0", 87)
                    .says(k::PACKETS_SMTP_P5)
                    .headers(&[line_of!("235 2.7.0", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMTP_3_TITLE,
        sub: k::PACKETS_SMTP_3_SUB,
        body: k::PACKETS_SMTP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "MAIL FROM:<alice@example.net>"),
            wire(k::PACKETS_LABEL_DESTINATION, "RCPT TO:<bob@example.com>"),
            wire(k::PACKETS_LABEL_REPLY, "250 2.0.0 OK"),
        ],
        focus: &[LAPTOP, INTERNET, MAIL],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Request,
                    "SMTP",
                    "MAIL FROM, RCPT TO, DATA",
                    "C: MAIL FROM:<alice@example.net> | RCPT TO:<bob@example.com> | DATA",
                    140,
                )
                .says(k::PACKETS_SMTP_P6)
                .headers(&[
                    header(&anatomy::TLS, &["23", "0x0303", "81"], &[]),
                    line_of!("MAIL FROM:<alice@example.net>", lit),
                    line_of!("RCPT TO:<bob@example.com>", lit),
                    line_of!("DATA"),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "SMTP",
                    "250, 250, 354",
                    "S: 250 2.1.0 | 250 2.1.5 | 354",
                    103,
                )
                .says(k::PACKETS_SMTP_P7)
                .headers(&[
                    line_of!("250 2.1.0"),
                    line_of!("250 2.1.5"),
                    line_of!("354", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    MAIL,
                    Class::Data,
                    "IMF",
                    "From: alice@example.net",
                    "from: alice@example.net, to: bob@example.com, (text/plain)",
                    1_164,
                )
                .says(k::PACKETS_SMTP_P8)
                .headers(&[
                    line_of!("From: alice@example.net", lit),
                    line_of!("To: bob@example.com", lit),
                    line_of!(""),
                    line_of!("…"),
                    line_of!(".", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    LAPTOP,
                    Class::Response,
                    "SMTP",
                    "250 OK",
                    "S: 250 2.0.0 OK id=4B7F21C0A3",
                    104,
                )
                .says(k::PACKETS_SMTP_P9)
                .headers(&[line_of!("250 2.0.0 OK id=4B7F21C0A3", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMTP_4_TITLE,
        sub: k::PACKETS_SMTP_4_SUB,
        body: k::PACKETS_SMTP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_QUESTION, "example.com MX"),
            wire(k::PACKETS_LABEL_ANSWER, "MX 10 mx.example.com"),
            wire(k::PACKETS_LABEL_ADDRESS, "203.0.113.25"),
        ],
        focus: &[MAIL, RESOLVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    MAIL,
                    RESOLVER,
                    Class::Lookup,
                    "DNS",
                    "MX? example.com",
                    "0x6d2e MX? example.com",
                    71,
                )
                .says(k::PACKETS_SMTP_P10)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "57", "0x7d21", "000", "0", "64", "17", "0xa8dd", "198.51.100.25",
                            "198.51.100.53",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["38211", "53", "37", "0x5a90"], &[1]),
                    header(
                        &anatomy::DNS,
                        &[
                            "0x6d2e", "0", "0", "0", "0", "1", "0", "0", "0", "1", "0", "0", "0", "example.com  MX  IN",
                        ],
                        &[13],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    MAIL,
                    Class::Lookup,
                    "DNS",
                    "MX 10 mx.example.com",
                    "0x6d2e MX example.com → 10 mx.example.com",
                    106,
                )
                .says(k::PACKETS_SMTP_P11)
                .headers(&[header(
                    &anatomy::DNS,
                    &[
                        "0x6d2e",
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
                        "1",
                        "example.com  MX  IN",
                        "example.com  MX  10 mx.example.com  TTL 3600  (+ mx.example.com  A  203.0.113.25)",
                    ],
                    &[10, 12, 14],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMTP_5_TITLE,
        sub: k::PACKETS_SMTP_5_SUB,
        body: k::PACKETS_SMTP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "25"),
            wire(k::PACKETS_LABEL_COMMAND, "EHLO mail.example.net"),
            wire(k::PACKETS_LABEL_SIGNATURE, "DKIM-Signature d=example.net"),
        ],
        focus: &[MAIL, INTERNET, RECIPIENT],
        beats: &[
            Beat::Send(
                Packet::new(
                    MAIL,
                    RECIPIENT,
                    Class::Request,
                    "SMTP",
                    "EHLO",
                    "C: EHLO mail.example.net",
                    77,
                )
                .says(k::PACKETS_SMTP_P12)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "63", "0x41c2", "010", "0", "64", "6", "0x9290", "198.51.100.25",
                            "203.0.113.25",
                        ],
                        &[12],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "41870", "25", "1", "27", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "502", "0x2d84",
                            "0",
                        ],
                        &[1],
                    ),
                    line_of!("EHLO mail.example.net", lit),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    MAIL,
                    RECIPIENT,
                    Class::Data,
                    "IMF",
                    "DKIM-Signature: d=example.net",
                    "from: alice@example.net, to: bob@example.com, (text/plain)",
                    1_842,
                )
                .says(k::PACKETS_SMTP_P13)
                .headers(&[
                    line_of!(
                        "DKIM-Signature: v=1; a=rsa-sha256; d=example.net; s=mail; h=from:to; bh=…; b=…",
                        lit
                    ),
                    line_of!("From: alice@example.net"),
                    line_of!("To: bob@example.com"),
                    line_of!("…"),
                    line_of!("."),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    RECIPIENT,
                    MAIL,
                    Class::Response,
                    "SMTP",
                    "250 OK",
                    "S: 250 2.0.0 OK id=9E3A60D2B4",
                    104,
                )
                .says(k::PACKETS_SMTP_P14)
                .headers(&[line_of!("250 2.0.0 OK id=9E3A60D2B4", lit)]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMTP_6_TITLE,
        sub: k::PACKETS_SMTP_6_SUB,
        body: k::PACKETS_SMTP_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_RECORD, "v=spf1 ip4:198.51.100.25 -all"),
            wire(k::PACKETS_LABEL_VERDICT, "spf=pass dkim=pass"),
            wire(k::PACKETS_LABEL_MAILBOX, "bob@example.com"),
        ],
        focus: &[RECIPIENT],
        beats: &[
            Beat::Light {
                at: RECIPIENT,
                prop: Some(Prop::Check),
            },
            Beat::Light {
                at: RECIPIENT,
                prop: Some(Prop::Envelope),
            },
            Beat::Rest,
        ],
    },
];

/// SMTP.
pub static SMTP: Protocol = Protocol {
    id: "smtp",
    group: Group::Mail,
    layer: Layer(7),
    name: k::PACKETS_SMTP_NAME,
    title: k::PACKETS_SMTP_TITLE,
    tagline: k::PACKETS_SMTP_TAGLINE,
    overview: k::PACKETS_SMTP_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_SMTP_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        fact(k::PACKETS_FACT_FORMAT, k::PACKETS_SMTP_FORMAT),
        wire(k::PACKETS_FACT_ENCRYPTION, "STARTTLS, TLS"),
        wire(k::PACKETS_FACT_DEFINED, "RFC 5321, RFC 6409, RFC 5322"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, INTERNET, 6),
            link(INTERNET, MAIL, 4),
            link(MAIL, RESOLVER, 1),
            link(INTERNET, RECIPIENT, 10),
        ],
    },
    steps: &STEPS,
};
