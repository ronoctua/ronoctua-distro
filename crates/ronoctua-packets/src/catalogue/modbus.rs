//! Modbus/TCP: a SCADA station reads a PLC's registers, starts a pump by
//! writing a coil, and gets an exception for an address that does not exist
//! — and a laptop on the same switch stops the pump just as easily, since
//! nothing asks who sends a command.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const SCADA: usize = 0;
const SWITCH: usize = 1;
const PLC: usize = 2;
const ATTACKER: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Desktop,
        at: at(-8, 2),
        name: k::PACKETS_DEVICE_SCADA,
        address: Address::V4([10, 10, 1, 10]),
    },
    Device {
        kind: Kind::Switch,
        at: at(-2, -1),
        name: k::PACKETS_DEVICE_SWITCH,
        address: Address::None,
    },
    Device {
        kind: Kind::Controller,
        at: at(6, 2),
        name: k::PACKETS_DEVICE_PLC,
        address: Address::V4([10, 10, 1, 50]),
    },
    Device {
        kind: Kind::Laptop,
        at: at(-3, 6),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::V4([10, 10, 1, 66]),
    },
];

/// The station's TCP header towards the PLC's port 502.
const TCP_SCADA: &[&str] = &[
    "49812", "502", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "8192", "0x6b1d",
    "0",
];

/// An IPv4 header carrying TCP, `$len` bytes in all, and its checksum.
macro_rules! ip {
    ($len:literal, $checksum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", $len, "0x4a10", "010", "0", "64", "6", $checksum, $src, $dst,
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_MODBUS_1_TITLE,
        sub: k::PACKETS_MODBUS_1_SUB,
        body: k::PACKETS_MODBUS_1_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_FUNCTION, k::PACKETS_MODBUS_1_FUNCTION),
            wire(k::PACKETS_LABEL_REGISTER, "0–3 (40001–40004)"),
            wire(k::PACKETS_LABEL_PORT, "502"),
        ],
        focus: &[SCADA, SWITCH, PLC],
        beats: &[
            Beat::Send(
                Packet::new(
                    SCADA,
                    PLC,
                    Class::Request,
                    "Modbus/TCP",
                    "FC=03 Qty=4",
                    "Trans=1 Unit=1 FC=0x03 Addr=0 Qty=4",
                    66,
                )
                .says(k::PACKETS_MODBUS_P1)
                .headers(&[
                    ip!("52", "0xda64", "10.10.1.10", "10.10.1.50", &[]),
                    header(&anatomy::TCP, TCP_SCADA, &[1]),
                    header(
                        &anatomy::MBAP,
                        &["1", "0", "6", "1", "0x03", "Addr=0x0000 Qty=4"],
                        &[0, 3, 4, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PLC,
                    SCADA,
                    Class::Response,
                    "Modbus/TCP",
                    "482, 215, 0, 1",
                    "Trans=1 Unit=1 FC=0x03 Values=482,215,0,1",
                    71,
                )
                .says(k::PACKETS_MODBUS_P2)
                .headers(&[
                    ip!("57", "0xda5f", "10.10.1.50", "10.10.1.10", &[]),
                    header(
                        &anatomy::MBAP,
                        &[
                            "1",
                            "0",
                            "11",
                            "1",
                            "0x03",
                            "ByteCount=8 Values=482, 215, 0, 1",
                        ],
                        &[0, 2, 5],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MODBUS_2_TITLE,
        sub: k::PACKETS_MODBUS_2_SUB,
        body: k::PACKETS_MODBUS_2_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_FUNCTION, k::PACKETS_MODBUS_2_FUNCTION),
            wire(k::PACKETS_LABEL_ADDRESS, "10"),
            wire(k::PACKETS_LABEL_VALUE, "0xFF00"),
        ],
        focus: &[SCADA, SWITCH, PLC],
        beats: &[
            Beat::Send(
                Packet::new(
                    SCADA,
                    PLC,
                    Class::Request,
                    "Modbus/TCP",
                    "FC=05 0xFF00",
                    "Trans=2 Unit=1 FC=0x05 Addr=10 Value=0xFF00",
                    66,
                )
                .says(k::PACKETS_MODBUS_P3)
                .headers(&[
                    ip!("52", "0xda64", "10.10.1.10", "10.10.1.50", &[]),
                    header(&anatomy::TCP, TCP_SCADA, &[]),
                    header(
                        &anatomy::MBAP,
                        &["2", "0", "6", "1", "0x05", "Addr=0x000A Value=0xFF00"],
                        &[4, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PLC,
                    SCADA,
                    Class::Response,
                    "Modbus/TCP",
                    "FC=05 0xFF00",
                    "Trans=2 Unit=1 FC=0x05 Addr=10 Value=0xFF00",
                    66,
                )
                .says(k::PACKETS_MODBUS_P4)
                .headers(&[header(
                    &anatomy::MBAP,
                    &["2", "0", "6", "1", "0x05", "Addr=0x000A Value=0xFF00"],
                    &[5],
                )]),
            ),
            Beat::Light {
                at: PLC,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_MODBUS_3_TITLE,
        sub: k::PACKETS_MODBUS_3_SUB,
        body: k::PACKETS_MODBUS_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_FUNCTION, "0x83 = 0x03 + 0x80"),
            wire(k::PACKETS_LABEL_ERROR, "0x02"),
            fact(k::PACKETS_LABEL_REASON, k::PACKETS_MODBUS_3_REASON),
        ],
        focus: &[SCADA, SWITCH, PLC],
        beats: &[
            Beat::Send(
                Packet::new(
                    SCADA,
                    PLC,
                    Class::Request,
                    "Modbus/TCP",
                    "FC=03 Addr=500",
                    "Trans=3 Unit=1 FC=0x03 Addr=500 Qty=4",
                    66,
                )
                .says(k::PACKETS_MODBUS_P5)
                .headers(&[header(
                    &anatomy::MBAP,
                    &["3", "0", "6", "1", "0x03", "Addr=0x01F4 Qty=4"],
                    &[5],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    PLC,
                    SCADA,
                    Class::Refusal,
                    "Modbus/TCP",
                    "FC=0x83 Ex=02",
                    "Trans=3 Unit=1 FC=0x83 Exception=0x02",
                    63,
                )
                .says(k::PACKETS_MODBUS_P6)
                .headers(&[
                    ip!("49", "0xda67", "10.10.1.50", "10.10.1.10", &[]),
                    header(
                        &anatomy::MBAP,
                        &["3", "0", "3", "1", "0x83", "ExceptionCode=0x02"],
                        &[2, 4, 5],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MODBUS_4_TITLE,
        sub: k::PACKETS_MODBUS_4_SUB,
        body: k::PACKETS_MODBUS_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SOURCE, "10.10.1.66"),
            wire(k::PACKETS_LABEL_VALUE, "0x0000"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_MODBUS_4_VERDICT),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    ATTACKER,
                    PLC,
                    Class::Request,
                    "Modbus/TCP",
                    "FC=05 0x0000",
                    "Trans=1 Unit=1 FC=0x05 Addr=10 Value=0x0000",
                    66,
                )
                .says(k::PACKETS_MODBUS_P7)
                .headers(&[
                    ip!("52", "0xda2c", "10.10.1.66", "10.10.1.50", &[11]),
                    header(
                        &anatomy::TCP,
                        &[
                            "53117", "502", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "8192", "0x29c4", "0",
                        ],
                        &[1],
                    ),
                    header(
                        &anatomy::MBAP,
                        &["1", "0", "6", "1", "0x05", "Addr=0x000A Value=0x0000"],
                        &[4, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PLC,
                    ATTACKER,
                    Class::Response,
                    "Modbus/TCP",
                    "FC=05 0x0000",
                    "Trans=1 Unit=1 FC=0x05 Addr=10 Value=0x0000",
                    66,
                )
                .says(k::PACKETS_MODBUS_P8),
            ),
            Beat::Light {
                at: PLC,
                prop: None,
            },
        ],
    },
    Step {
        title: k::PACKETS_MODBUS_5_TITLE,
        sub: k::PACKETS_MODBUS_5_SUB,
        body: k::PACKETS_MODBUS_5_BODY,
        facts: &[
            fact(k::PACKETS_LABEL_SCOPE, k::PACKETS_MODBUS_5_SCOPE),
            wire(k::PACKETS_LABEL_PORT, "802 (TLS)"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: SWITCH,
                prop: Some(Prop::Shield),
            },
            Beat::Light {
                at: ATTACKER,
                prop: Some(Prop::Cross),
            },
            Beat::Rest,
        ],
    },
];

/// Modbus/TCP.
pub static MODBUS: Protocol = Protocol {
    id: "modbus",
    group: Group::Things,
    layer: Layer(7),
    name: k::PACKETS_MODBUS_NAME,
    title: k::PACKETS_MODBUS_TITLE,
    tagline: k::PACKETS_MODBUS_TAGLINE,
    overview: k::PACKETS_MODBUS_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "502, 802 (TLS)"),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP (Modbus RTU: RS-485)"),
        fact(k::PACKETS_FACT_MODEL, k::PACKETS_MODBUS_MODEL),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_MODBUS_SECURITY),
        wire(k::PACKETS_FACT_DEFINED, "Modbus V1.1b3, Modbus/TCP V1.0b"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(SCADA, SWITCH, 1),
            link(SWITCH, PLC, 1),
            link(ATTACKER, SWITCH, 1),
        ],
    },
    steps: &STEPS,
};
