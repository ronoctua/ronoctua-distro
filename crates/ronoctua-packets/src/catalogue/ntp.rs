//! NTP: a clock set right — the laptop's request (T1), the time server's
//! stamps (T2, T3), the arithmetic at T4, the stratum above the server, and
//! a later poll once the clock has been steered.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const TIME: usize = 3;
const STRATUM_ONE: usize = 4;

static DEVICES: [Device; 5] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Router,
        at: at(-5, 0),
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
        kind: Kind::Tower(Emblem::Clock),
        at: at(5, -2),
        name: k::PACKETS_DEVICE_TIME_SERVER,
        address: Address::V4([198, 51, 100, 123]),
    },
    Device {
        kind: Kind::Rack,
        at: at(9, -6),
        name: k::PACKETS_NTP_DEVICE_STRATUM_ONE,
        address: Address::V4([203, 0, 113, 1]),
    },
];

/// An NTP header: LI, version 4, mode, stratum, poll, precision, root delay,
/// root dispersion, reference ID, and the four timestamps — reference,
/// origin, receive, transmit.
macro_rules! ntp {
    ($li:literal, $mode:literal, $stratum:literal, $poll:literal, $precision:literal, $delay:literal,
     $dispersion:literal, $reference:literal, $updated:literal, $origin:literal, $receive:literal,
     $transmit:literal, $lit:expr) => {
        header(
            &anatomy::NTP,
            &[
                $li,
                "4",
                $mode,
                $stratum,
                $poll,
                $precision,
                $delay,
                $dispersion,
                $reference,
                $updated,
                $origin,
                $receive,
                $transmit,
            ],
            $lit,
        )
    };
}

/// The laptop's way to the time server, for the steps' frames.
const WAY: &[usize] = &[LAPTOP, ROUTER, INTERNET, TIME];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_NTP_1_TITLE,
        sub: k::PACKETS_NTP_1_SUB,
        body: k::PACKETS_NTP_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "123"),
            wire(k::PACKETS_LABEL_MODE, "3"),
            wire(k::PACKETS_LABEL_TIME, "T1 = 10:00:00.000"),
        ],
        focus: WAY,
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Clock),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    TIME,
                    Class::Request,
                    "NTP",
                    "T1",
                    "NTPv4 mode=3 xmt=10:00:00.000",
                    90,
                )
                .says(k::PACKETS_NTP_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "76", "0x8e3b", "010", "0", "64", "17", "0xc004", "192.168.1.10",
                            "198.51.100.123",
                        ],
                        &[9],
                    ),
                    header(&anatomy::UDP, &["39713", "123", "56", "0x6c1f"], &[1]),
                    ntp!(
                        "0",
                        "3",
                        "0",
                        "6",
                        "-20",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "0",
                        "10:00:00.000",
                        &[2, 12]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_NTP_2_TITLE,
        sub: k::PACKETS_NTP_2_SUB,
        body: k::PACKETS_NTP_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TIME, "T2 = 10:00:00.062, T3 = 10:00:00.063"),
            wire(k::PACKETS_LABEL_MODE, "4"),
            wire(k::PACKETS_LABEL_STRATUM, "2"),
        ],
        focus: WAY,
        beats: &[
            Beat::Light {
                at: TIME,
                prop: Some(Prop::Clock),
            },
            Beat::Send(
                Packet::new(
                    TIME,
                    LAPTOP,
                    Class::Response,
                    "NTP",
                    "T1 T2 T3",
                    "NTPv4 mode=4 stratum=2 org=10:00:00.000 rec=10:00:00.062 xmt=10:00:00.063",
                    90,
                )
                .says(k::PACKETS_NTP_P2)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "76", "0x0000", "010", "0", "57", "17", "0x5540", "198.51.100.123",
                            "192.168.1.10",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["123", "39713", "56", "0xb3e4"], &[0]),
                    ntp!(
                        "0",
                        "4",
                        "2",
                        "6",
                        "-23",
                        "0.016 s",
                        "0.002 s",
                        "203.0.113.1",
                        "09:57:56.210",
                        "10:00:00.000",
                        "10:00:00.062",
                        "10:00:00.063",
                        &[2, 3, 10, 11, 12]
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_NTP_3_TITLE,
        sub: k::PACKETS_NTP_3_SUB,
        body: k::PACKETS_NTP_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TIME, "T4 = 10:00:00.025"),
            wire(k::PACKETS_LABEL_DELAY, "(25 − 0) − (63 − 62) = 24 ms"),
            wire(k::PACKETS_LABEL_OFFSET, "((62 − 0) + (63 − 25)) / 2 = +50 ms"),
        ],
        focus: &[LAPTOP],
        beats: &[
            Beat::Type {
                at: LAPTOP,
                wire: "T4 = 10:00:00.025",
            },
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Clock),
            },
            Beat::Rest,
        ],
    },
    Step {
        title: k::PACKETS_NTP_4_TITLE,
        sub: k::PACKETS_NTP_4_SUB,
        body: k::PACKETS_NTP_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_STRATUM, "0 → 1 → 2 → 3"),
            wire(k::PACKETS_LABEL_SOURCE, "GPS"),
            wire(k::PACKETS_LABEL_INTERVAL, "128 s"),
        ],
        focus: &[TIME, STRATUM_ONE],
        beats: &[
            Beat::Send(
                Packet::new(
                    TIME,
                    STRATUM_ONE,
                    Class::Request,
                    "NTP",
                    "T1",
                    "NTPv4 mode=3 xmt=10:00:04.210",
                    90,
                )
                .says(k::PACKETS_NTP_P3)
                .headers(&[ntp!(
                    "0",
                    "3",
                    "2",
                    "7",
                    "-23",
                    "0.016 s",
                    "0.002 s",
                    "203.0.113.1",
                    "09:57:56.210",
                    "0",
                    "0",
                    "10:00:04.210",
                    &[2, 12]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    STRATUM_ONE,
                    TIME,
                    Class::Response,
                    "NTP",
                    "stratum=1 refid=GPS",
                    "NTPv4 mode=4 stratum=1 refid=GPS org=10:00:04.210 rec=10:00:04.218 xmt=10:00:04.218",
                    90,
                )
                .says(k::PACKETS_NTP_P4)
                .headers(&[ntp!(
                    "0",
                    "4",
                    "1",
                    "7",
                    "-22",
                    "0 s",
                    "0.0002 s",
                    "GPS",
                    "10:00:04.000",
                    "10:00:04.210",
                    "10:00:04.218",
                    "10:00:04.218",
                    &[3, 8]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_NTP_5_TITLE,
        sub: k::PACKETS_NTP_5_SUB,
        body: k::PACKETS_NTP_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_OFFSET, "((12 − 0) + (13 − 25)) / 2 = 0 ms"),
            wire(k::PACKETS_LABEL_INTERVAL, "64 s → 1024 s"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_NTP_5_STATE),
        ],
        focus: WAY,
        beats: &[
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Clock),
            },
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    TIME,
                    Class::Request,
                    "NTP",
                    "T1",
                    "NTPv4 mode=3 xmt=10:04:16.000",
                    90,
                )
                .says(k::PACKETS_NTP_P5)
                .headers(&[ntp!(
                    "0",
                    "3",
                    "3",
                    "8",
                    "-20",
                    "0.040 s",
                    "0.003 s",
                    "198.51.100.123",
                    "10:00:00.025",
                    "0",
                    "0",
                    "10:04:16.000",
                    &[3, 4, 8]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    TIME,
                    LAPTOP,
                    Class::Response,
                    "NTP",
                    "T1 T2 T3",
                    "NTPv4 mode=4 stratum=2 org=10:04:16.000 rec=10:04:16.012 xmt=10:04:16.013",
                    90,
                )
                .says(k::PACKETS_NTP_P6)
                .headers(&[ntp!(
                    "0",
                    "4",
                    "2",
                    "8",
                    "-23",
                    "0.016 s",
                    "0.002 s",
                    "203.0.113.1",
                    "10:00:04.218",
                    "10:04:16.000",
                    "10:04:16.012",
                    "10:04:16.013",
                    &[10, 11, 12]
                )]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
];

/// NTP.
pub static NTP: Protocol = Protocol {
    id: "ntp",
    group: Group::Things,
    layer: Layer(7),
    name: k::PACKETS_NTP_NAME,
    title: k::PACKETS_NTP_TITLE,
    tagline: k::PACKETS_NTP_TAGLINE,
    overview: k::PACKETS_NTP_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "123"),
        wire(k::PACKETS_FACT_TRANSPORT, "UDP"),
        fact(k::PACKETS_FACT_FORMAT, k::PACKETS_NTP_FORMAT),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_NTP_SECURITY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 5905, RFC 8915"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, ROUTER, 1),
            link(ROUTER, INTERNET, 5),
            link(INTERNET, TIME, 6),
            link(TIME, STRATUM_ONE, 8),
        ],
    },
    steps: &STEPS,
};
