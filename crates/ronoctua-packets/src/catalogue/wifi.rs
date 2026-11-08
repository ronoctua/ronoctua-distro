//! WI-FI (WPA2-PERSONAL): a phone joining a protected network — the access
//! point's beacon and a probe, the passphrase turned into a key that never
//! goes on the air, authentication and association, the four-way handshake
//! that proves the key and derives fresh ones, and the encrypted traffic
//! that follows.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const PHONE: usize = 0;
const ACCESS_POINT: usize = 1;
const ROUTER: usize = 2;

static DEVICES: [Device; 3] = [
    Device {
        kind: Kind::Phone,
        at: at(-8, 2),
        name: k::PACKETS_DEVICE_PHONE,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x21]),
    },
    Device {
        kind: Kind::AccessPoint,
        at: at(-1, -2),
        name: k::PACKETS_DEVICE_ACCESS_POINT,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0xa1]),
    },
    Device {
        kind: Kind::Router,
        at: at(6, 1),
        name: k::PACKETS_DEVICE_ROUTER,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x01]),
    },
];

/// An 802.11 header: frame control, duration, the three addresses and the
/// sequence control. The phone is `00:00:5e:00:53:21`, the access point
/// (and its BSSID) `00:00:5e:00:53:a1`.
macro_rules! dot11 {
    ($fc:literal, $duration:literal, $a1:literal, $a2:literal, $a3:literal, $seq:literal, $lit:expr) => {
        header(&anatomy::WIFI, &[$fc, $duration, $a1, $a2, $a3, $seq], $lit)
    };
}

/// The access point's random number, in messages 1 and 3.
const ANONCE: &str = "7c0e3a5f…91d4e8b2";

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_WIFI_1_TITLE,
        sub: k::PACKETS_WIFI_1_SUB,
        body: k::PACKETS_WIFI_1_BODY,
        facts: &[
            wire(k::PACKETS_WIFI_LABEL_SSID, "example"),
            wire(k::PACKETS_LABEL_CHANNEL, "6 (2437 MHz)"),
            wire(k::PACKETS_LABEL_INTERVAL, "100 TU"),
        ],
        focus: &[PHONE, ACCESS_POINT],
        beats: &[
            Beat::Send(
                Packet::new(
                    ACCESS_POINT,
                    PHONE,
                    Class::Lookup,
                    "802.11",
                    "Beacon",
                    "Beacon SN=1024 BI=100 SSID=example",
                    266,
                )
                .says(k::PACKETS_WIFI_P1)
                .headers(&[dot11!(
                    "0x8000",
                    "0",
                    "ff:ff:ff:ff:ff:ff",
                    "00:00:5e:00:53:a1",
                    "00:00:5e:00:53:a1",
                    "SN=1024 FN=0",
                    &[0, 2, 4]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    ACCESS_POINT,
                    Class::Lookup,
                    "802.11",
                    "Probe Request",
                    "Probe Request SN=310 SSID=example",
                    112,
                )
                .says(k::PACKETS_WIFI_P2)
                .headers(&[dot11!(
                    "0x4000",
                    "0",
                    "ff:ff:ff:ff:ff:ff",
                    "00:00:5e:00:53:21",
                    "ff:ff:ff:ff:ff:ff",
                    "SN=310 FN=0",
                    &[0, 3]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    ACCESS_POINT,
                    PHONE,
                    Class::Lookup,
                    "802.11",
                    "Probe Response",
                    "Probe Response SN=1025 BI=100 SSID=example",
                    290,
                )
                .says(k::PACKETS_WIFI_P3)
                .headers(&[dot11!(
                    "0x5000",
                    "314",
                    "00:00:5e:00:53:21",
                    "00:00:5e:00:53:a1",
                    "00:00:5e:00:53:a1",
                    "SN=1025 FN=0",
                    &[0, 2]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WIFI_2_TITLE,
        sub: k::PACKETS_WIFI_2_SUB,
        body: k::PACKETS_WIFI_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ALGORITHM, "PBKDF2-HMAC-SHA1 ×4096"),
            fact(k::PACKETS_LABEL_SIZE, k::PACKETS_WIFI_2_SIZE),
        ],
        focus: &[PHONE, ACCESS_POINT],
        beats: &[
            Beat::Type {
                at: PHONE,
                wire: "••••••••••••",
            },
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: ACCESS_POINT,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_WIFI_3_TITLE,
        sub: k::PACKETS_WIFI_3_SUB,
        body: k::PACKETS_WIFI_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ALGORITHM, "Open System"),
            wire(k::PACKETS_LABEL_CIPHER, "CCMP-128"),
            wire(k::PACKETS_WIFI_LABEL_AID, "1"),
        ],
        focus: &[PHONE, ACCESS_POINT],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    ACCESS_POINT,
                    Class::Handshake,
                    "802.11",
                    "Authentication Seq=1",
                    "Authentication SN=311 Alg=0 Seq=1",
                    30,
                )
                .says(k::PACKETS_WIFI_P4)
                .headers(&[dot11!(
                    "0xb000",
                    "314",
                    "00:00:5e:00:53:a1",
                    "00:00:5e:00:53:21",
                    "00:00:5e:00:53:a1",
                    "SN=311 FN=0",
                    &[0]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    ACCESS_POINT,
                    PHONE,
                    Class::Handshake,
                    "802.11",
                    "Authentication Seq=2",
                    "Authentication SN=1026 Alg=0 Seq=2 Status=0",
                    30,
                )
                .says(k::PACKETS_WIFI_P5)
                .headers(&[dot11!(
                    "0xb000",
                    "314",
                    "00:00:5e:00:53:21",
                    "00:00:5e:00:53:a1",
                    "00:00:5e:00:53:a1",
                    "SN=1026 FN=0",
                    &[]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    ACCESS_POINT,
                    Class::Handshake,
                    "802.11",
                    "Association Request",
                    "Association Request SN=312 SSID=example",
                    176,
                )
                .says(k::PACKETS_WIFI_P6)
                .headers(&[dot11!(
                    "0x0000",
                    "314",
                    "00:00:5e:00:53:a1",
                    "00:00:5e:00:53:21",
                    "00:00:5e:00:53:a1",
                    "SN=312 FN=0",
                    &[0]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    ACCESS_POINT,
                    PHONE,
                    Class::Handshake,
                    "802.11",
                    "Association Response",
                    "Association Response SN=1027 Status=0 AID=1",
                    158,
                )
                .says(k::PACKETS_WIFI_P7)
                .headers(&[dot11!(
                    "0x1000",
                    "314",
                    "00:00:5e:00:53:21",
                    "00:00:5e:00:53:a1",
                    "00:00:5e:00:53:a1",
                    "SN=1027 FN=0",
                    &[0]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WIFI_4_TITLE,
        sub: k::PACKETS_WIFI_4_SUB,
        body: k::PACKETS_WIFI_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_NONCE, "ANonce, SNonce"),
            wire(
                k::PACKETS_LABEL_KEY_EXCHANGE,
                "PTK = PRF(PMK, ANonce, SNonce, AA, SPA)",
            ),
        ],
        focus: &[PHONE, ACCESS_POINT],
        beats: &[
            Beat::Send(
                Packet::new(
                    ACCESS_POINT,
                    PHONE,
                    Class::Crypto,
                    "EAPOL",
                    "M1 ANonce",
                    "Key 1/4 ANonce Replay=1",
                    133,
                )
                .says(k::PACKETS_WIFI_P8)
                .headers(&[
                    dot11!(
                        "0x8802",
                        "44",
                        "00:00:5e:00:53:21",
                        "00:00:5e:00:53:a1",
                        "00:00:5e:00:53:a1",
                        "SN=0 FN=0",
                        &[]
                    ),
                    header(
                        &anatomy::EAPOL,
                        &["2", "3", "95", "2", "0x008a", "16", "1", ANONCE, "0"],
                        &[6, 7],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    ACCESS_POINT,
                    Class::Crypto,
                    "EAPOL",
                    "M2 SNonce, MIC",
                    "Key 2/4 SNonce MIC Replay=1",
                    155,
                )
                .says(k::PACKETS_WIFI_P9)
                .headers(&[
                    dot11!(
                        "0x8801",
                        "44",
                        "00:00:5e:00:53:a1",
                        "00:00:5e:00:53:21",
                        "00:00:5e:00:53:a1",
                        "SN=0 FN=0",
                        &[]
                    ),
                    header(
                        &anatomy::EAPOL,
                        &[
                            "1",
                            "3",
                            "117",
                            "2",
                            "0x010a",
                            "0",
                            "1",
                            "e41b07c2…5a3f9d60",
                            "9c1e47b0…7a02d5e3",
                            "RSNE",
                        ],
                        &[7, 8],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    ACCESS_POINT,
                    PHONE,
                    Class::Crypto,
                    "EAPOL",
                    "M3 GTK, MIC",
                    "Key 3/4 Install GTK MIC Replay=2",
                    189,
                )
                .says(k::PACKETS_WIFI_P10)
                .headers(&[
                    dot11!(
                        "0x8802",
                        "44",
                        "00:00:5e:00:53:21",
                        "00:00:5e:00:53:a1",
                        "00:00:5e:00:53:a1",
                        "SN=1 FN=0",
                        &[]
                    ),
                    header(
                        &anatomy::EAPOL,
                        &[
                            "2",
                            "3",
                            "151",
                            "2",
                            "0x13ca",
                            "16",
                            "2",
                            ANONCE,
                            "3f6a92d1…c80b4e17",
                            "AES-KW: RSNE, GTK KDE",
                        ],
                        &[4, 8, 9],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    ACCESS_POINT,
                    Class::Crypto,
                    "EAPOL",
                    "M4 MIC",
                    "Key 4/4 MIC Replay=2",
                    133,
                )
                .says(k::PACKETS_WIFI_P11)
                .headers(&[
                    dot11!(
                        "0x8801",
                        "44",
                        "00:00:5e:00:53:a1",
                        "00:00:5e:00:53:21",
                        "00:00:5e:00:53:a1",
                        "SN=1 FN=0",
                        &[]
                    ),
                    header(
                        &anatomy::EAPOL,
                        &[
                            "1",
                            "3",
                            "95",
                            "2",
                            "0x030a",
                            "0",
                            "2",
                            "0",
                            "b25e8f14…06c9a3d7",
                        ],
                        &[8],
                    ),
                ]),
            ),
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Lock),
            },
            Beat::Light {
                at: ACCESS_POINT,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_WIFI_5_TITLE,
        sub: k::PACKETS_WIFI_5_SUB,
        body: k::PACKETS_WIFI_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ENCRYPTION, "CCMP (AES-128-CCM)"),
            fact(k::PACKETS_LABEL_SCOPE, k::PACKETS_WIFI_5_SCOPE),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    ACCESS_POINT,
                    Class::Data,
                    "802.11",
                    "QoS Data CCMP",
                    "QoS Data SN=2 Protected PN=1",
                    110,
                )
                .says(k::PACKETS_WIFI_P12)
                .headers(&[dot11!(
                    "0x8841",
                    "44",
                    "00:00:5e:00:53:a1",
                    "00:00:5e:00:53:21",
                    "00:00:5e:00:53:01",
                    "SN=2 FN=0",
                    &[0, 4]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    ACCESS_POINT,
                    ROUTER,
                    Class::Data,
                    "Ethernet",
                    "→ 00:00:5e:00:53:01",
                    "00:00:5e:00:53:21 → 00:00:5e:00:53:01 IPv4",
                    74,
                )
                .says(k::PACKETS_WIFI_P13)
                .headers(&[
                    header(
                        &anatomy::ETHERNET,
                        &["00:00:5e:00:53:01", "00:00:5e:00:53:21", "0x0800"],
                        &[0, 1],
                    ),
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "60",
                            "0x3a1c",
                            "010",
                            "0",
                            "64",
                            "6",
                            "0x02cc",
                            "192.168.1.33",
                            "203.0.113.10",
                        ],
                        &[],
                    ),
                ]),
            ),
        ],
    },
];

/// Wi-Fi, WPA2-Personal.
pub static WIFI: Protocol = Protocol {
    id: "wifi",
    group: Group::Link,
    layer: Layer(2),
    name: k::PACKETS_WIFI_NAME,
    title: k::PACKETS_WIFI_TITLE,
    tagline: k::PACKETS_WIFI_TAGLINE,
    overview: k::PACKETS_WIFI_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_ENCRYPTION, "CCMP (AES-128)"),
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_WIFI_AUTHENTICATION,
        ),
        wire(k::PACKETS_FACT_REPLACES, "WEP, WPA"),
        wire(k::PACKETS_FACT_DEFINED, "IEEE 802.11i-2004 (IEEE 802.11)"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[link(PHONE, ACCESS_POINT, 2), link(ACCESS_POINT, ROUTER, 1)],
    },
    steps: &STEPS,
};
