//! MQTT: a sensor and a dashboard meet at a broker — CONNECT with a last
//! will, SUBSCRIBE, a reading published at QoS 1 and handed on, a retained
//! value for a newcomer, and the will announced when the sensor falls silent.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const SENSOR: usize = 0;
const BROKER: usize = 1;
const DASHBOARD: usize = 2;
const PHONE: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Controller,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_SENSOR,
        address: Address::V4([192, 168, 1, 40]),
    },
    Device {
        kind: Kind::Tower(Emblem::Hub),
        at: at(-1, -1),
        name: k::PACKETS_DEVICE_BROKER,
        address: Address::V4([192, 168, 1, 5]),
    },
    Device {
        kind: Kind::Desktop,
        at: at(7, 3),
        name: k::PACKETS_DEVICE_DASHBOARD,
        address: Address::V4([192, 168, 1, 20]),
    },
    Device {
        kind: Kind::Phone,
        at: at(6, -5),
        name: k::PACKETS_DEVICE_PHONE,
        address: Address::V4([192, 168, 1, 30]),
    },
];

/// A TCP header carrying MQTT, from `$src` to `$dst` — the broker listens on
/// 1883.
macro_rules! tcp {
    ($src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::TCP,
            &[
                $src, $dst, "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "502",
                "0x5e19", "0",
            ],
            $lit,
        )
    };
}

/// An IPv4 header carrying TCP, `$len` bytes in all, and its checksum.
macro_rules! ip {
    ($len:literal, $checksum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", $len, "0x1d2e", "010", "0", "64", "6", $checksum, $src, $dst,
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_MQTT_1_TITLE,
        sub: k::PACKETS_MQTT_1_SUB,
        body: k::PACKETS_MQTT_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "1883, 8883 (TLS)"),
            wire(k::PACKETS_LABEL_IDENTITY, "ClientId=kitchen-sensor"),
            wire(k::PACKETS_LABEL_INTERVAL, "KeepAlive=60 s"),
        ],
        focus: &[SENSOR, BROKER],
        beats: &[
            Beat::Send(
                Packet::new(
                    SENSOR,
                    BROKER,
                    Class::Handshake,
                    "MQTT",
                    "CONNECT",
                    "CONNECT ClientId=kitchen-sensor KeepAlive=60 Will=home/kitchen/status",
                    114,
                )
                .says(k::PACKETS_MQTT_P1)
                .headers(&[
                    ip!("100", "0x99e8", "192.168.1.40", "192.168.1.5", &[]),
                    tcp!("51522", "1883", &[1]),
                    header(
                        &anatomy::MQTT,
                        &[
                            "1",
                            "0000",
                            "58",
                            "ProtocolName=MQTT Level=5 Flags=0x2e KeepAlive=60",
                            "ClientId=kitchen-sensor WillTopic=home/kitchen/status WillPayload=offline",
                        ],
                        &[0, 3, 4],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    BROKER,
                    SENSOR,
                    Class::Handshake,
                    "MQTT",
                    "CONNACK",
                    "CONNACK ReasonCode=0x00",
                    60,
                )
                .says(k::PACKETS_MQTT_P2)
                .headers(&[header(
                    &anatomy::MQTT,
                    &["2", "0000", "3", "SessionPresent=0 ReasonCode=0x00"],
                    &[0, 3],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MQTT_2_TITLE,
        sub: k::PACKETS_MQTT_2_SUB,
        body: k::PACKETS_MQTT_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TOPIC, "home/kitchen/temperature"),
            wire(k::PACKETS_LABEL_QOS, "1"),
        ],
        focus: &[BROKER, DASHBOARD],
        beats: &[
            Beat::Send(
                Packet::new(
                    DASHBOARD,
                    BROKER,
                    Class::Request,
                    "MQTT",
                    "SUBSCRIBE",
                    "SUBSCRIBE PacketId=1 home/kitchen/temperature QoS=1",
                    86,
                )
                .says(k::PACKETS_MQTT_P3)
                .headers(&[
                    ip!("72", "0x9a18", "192.168.1.20", "192.168.1.5", &[]),
                    tcp!("50310", "1883", &[]),
                    header(
                        &anatomy::MQTT,
                        &["8", "0010", "30", "PacketId=1", "home/kitchen/temperature QoS=1"],
                        &[0, 4],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    BROKER,
                    DASHBOARD,
                    Class::Response,
                    "MQTT",
                    "SUBACK",
                    "SUBACK PacketId=1 ReasonCode=0x01",
                    60,
                )
                .says(k::PACKETS_MQTT_P4)
                .headers(&[header(
                    &anatomy::MQTT,
                    &["9", "0000", "4", "PacketId=1", "ReasonCode=0x01"],
                    &[3, 4],
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MQTT_3_TITLE,
        sub: k::PACKETS_MQTT_3_SUB,
        body: k::PACKETS_MQTT_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TOPIC, "home/kitchen/temperature"),
            wire(k::PACKETS_LABEL_VALUE, "21.5"),
            wire(k::PACKETS_LABEL_QOS, "1 → PUBACK"),
        ],
        focus: &[SENSOR, BROKER, DASHBOARD],
        beats: &[
            Beat::Send(
                Packet::new(
                    SENSOR,
                    BROKER,
                    Class::Data,
                    "MQTT",
                    "PUBLISH 21.5",
                    "PUBLISH PacketId=7 QoS=1 RETAIN home/kitchen/temperature 21.5",
                    89,
                )
                .says(k::PACKETS_MQTT_P5)
                .headers(&[
                    ip!("75", "0x9a01", "192.168.1.40", "192.168.1.5", &[]),
                    tcp!("51522", "1883", &[]),
                    header(
                        &anatomy::MQTT,
                        &["3", "0011", "33", "Topic=home/kitchen/temperature PacketId=7", "21.5"],
                        &[0, 1, 3, 4],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(BROKER, SENSOR, Class::Control, "MQTT", "PUBACK", "PUBACK PacketId=7", 60)
                    .says(k::PACKETS_MQTT_P6)
                    .headers(&[header(&anatomy::MQTT, &["4", "0000", "2", "PacketId=7"], &[0, 3])]),
            ),
            Beat::Send(
                Packet::new(
                    BROKER,
                    DASHBOARD,
                    Class::Data,
                    "MQTT",
                    "PUBLISH 21.5",
                    "PUBLISH PacketId=3 QoS=1 home/kitchen/temperature 21.5",
                    89,
                )
                .says(k::PACKETS_MQTT_P7)
                .headers(&[
                    ip!("75", "0x9a15", "192.168.1.5", "192.168.1.20", &[]),
                    tcp!("1883", "50310", &[]),
                    header(
                        &anatomy::MQTT,
                        &["3", "0010", "33", "Topic=home/kitchen/temperature PacketId=3", "21.5"],
                        &[1, 3],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(DASHBOARD, BROKER, Class::Control, "MQTT", "PUBACK", "PUBACK PacketId=3", 60)
                    .says(k::PACKETS_MQTT_P8),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MQTT_4_TITLE,
        sub: k::PACKETS_MQTT_4_SUB,
        body: k::PACKETS_MQTT_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TOPIC, "home/kitchen/#"),
            wire(k::PACKETS_LABEL_VALUE, "21.5 RETAIN=1"),
        ],
        focus: &[BROKER, PHONE],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    BROKER,
                    Class::Request,
                    "MQTT",
                    "SUBSCRIBE",
                    "SUBSCRIBE PacketId=1 home/kitchen/# QoS=0",
                    76,
                )
                .says(k::PACKETS_MQTT_P9)
                .headers(&[header(
                    &anatomy::MQTT,
                    &["8", "0010", "20", "PacketId=1", "home/kitchen/# QoS=0"],
                    &[4],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    BROKER,
                    PHONE,
                    Class::Response,
                    "MQTT",
                    "SUBACK",
                    "SUBACK PacketId=1 ReasonCode=0x00",
                    60,
                )
                .says(k::PACKETS_MQTT_P10),
            ),
            Beat::Send(
                Packet::new(
                    BROKER,
                    PHONE,
                    Class::Data,
                    "MQTT",
                    "PUBLISH 21.5",
                    "PUBLISH QoS=0 RETAIN home/kitchen/temperature 21.5",
                    87,
                )
                .says(k::PACKETS_MQTT_P11)
                .headers(&[
                    ip!("73", "0x9a0d", "192.168.1.5", "192.168.1.30", &[]),
                    tcp!("1883", "49207", &[]),
                    header(
                        &anatomy::MQTT,
                        &["3", "0001", "31", "Topic=home/kitchen/temperature", "21.5"],
                        &[1, 4],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_MQTT_5_TITLE,
        sub: k::PACKETS_MQTT_5_SUB,
        body: k::PACKETS_MQTT_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_TOPIC, "home/kitchen/status"),
            wire(k::PACKETS_LABEL_VALUE, "offline"),
            wire(k::PACKETS_LABEL_DELAY, "1.5 × 60 s = 90 s"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: SENSOR,
                prop: Some(Prop::Cross),
            },
            Beat::Light {
                at: BROKER,
                prop: Some(Prop::Clock),
            },
            Beat::Send(
                Packet::new(
                    BROKER,
                    PHONE,
                    Class::Data,
                    "MQTT",
                    "PUBLISH offline",
                    "PUBLISH QoS=0 home/kitchen/status offline",
                    85,
                )
                .says(k::PACKETS_MQTT_P12)
                .headers(&[
                    ip!("71", "0x9a0f", "192.168.1.5", "192.168.1.30", &[]),
                    tcp!("1883", "49207", &[]),
                    header(
                        &anatomy::MQTT,
                        &["3", "0000", "29", "Topic=home/kitchen/status", "offline"],
                        &[3, 4],
                    ),
                ]),
            ),
        ],
    },
];

/// MQTT.
pub static MQTT: Protocol = Protocol {
    id: "mqtt",
    group: Group::Things,
    layer: Layer(7),
    name: k::PACKETS_MQTT_NAME,
    title: k::PACKETS_MQTT_TITLE,
    tagline: k::PACKETS_MQTT_TAGLINE,
    overview: k::PACKETS_MQTT_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "1883, 8883 (TLS)"),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP, WebSocket"),
        fact(k::PACKETS_FACT_MODEL, k::PACKETS_MQTT_MODEL),
        wire(k::PACKETS_FACT_HEADER, "2–5 B"),
        wire(
            k::PACKETS_FACT_DEFINED,
            "MQTT 5.0 (OASIS), MQTT 3.1.1 (ISO/IEC 20922)",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(SENSOR, BROKER, 2),
            link(BROKER, DASHBOARD, 1),
            link(BROKER, PHONE, 3),
        ],
    },
    steps: &STEPS,
};
