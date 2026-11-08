//! WPA3: a phone joins a Wi-Fi network by its password — the beacon, SAE's
//! commit and confirm, the association and the 4-way handshake — while an
//! attacker records every frame, learns nothing it can use, and is ignored
//! when it forges a goodbye.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const PHONE: usize = 0;
const AP: usize = 1;
const ATTACKER: usize = 2;

static DEVICES: [Device; 3] = [
    Device {
        kind: Kind::Phone,
        at: at(-7, 2),
        name: k::PACKETS_DEVICE_PHONE,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x10]),
    },
    Device {
        kind: Kind::AccessPoint,
        at: at(5, -2),
        name: k::PACKETS_DEVICE_ACCESS_POINT,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x01]),
    },
    Device {
        kind: Kind::Laptop,
        at: at(0, 6),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x66]),
    },
];

/// An 802.11 header: frame control, duration, the three addresses (receiver,
/// transmitter, BSSID or the other end), sequence control.
macro_rules! wifi {
    ($fc:literal, $dur:literal, $a1:literal, $a2:literal, $a3:literal, $sn:literal, $lit:expr) => {
        header(&anatomy::WIFI, &[$fc, $dur, $a1, $a2, $a3, $sn], $lit)
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_WPA3_1_TITLE,
        sub: k::PACKETS_WPA3_1_SUB,
        body: k::PACKETS_WPA3_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "SAE (AKM 00-0F-AC:8)"),
            wire(k::PACKETS_LABEL_CIPHER, "CCMP-128 (AES)"),
            fact(k::PACKETS_LABEL_MODE, k::PACKETS_WPA3_1_MODE),
        ],
        focus: &[PHONE, AP],
        beats: &[Beat::Send(
            Packet::new(
                AP,
                PHONE,
                Class::Lookup,
                "802.11",
                "Beacon",
                "Beacon, SN=310, FN=0, BI=100, SSID=\"example\"",
                268,
            )
            .says(k::PACKETS_WPA3_P1)
            .headers(&[wifi!(
                "0x8000",
                "0",
                "ff:ff:ff:ff:ff:ff",
                "00:00:5e:00:53:01",
                "00:00:5e:00:53:01",
                "SN=310, FN=0",
                &[0, 2]
            )]),
        )],
    },
    Step {
        title: k::PACKETS_WPA3_2_TITLE,
        sub: k::PACKETS_WPA3_2_SUB,
        body: k::PACKETS_WPA3_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ALGORITHM, "SAE, Group=19 (P-256)"),
            wire(k::PACKETS_LABEL_SEQ, "1 (commit)"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    AP,
                    Class::Crypto,
                    "802.11",
                    "SAE Commit",
                    "Authentication, SN=1, FN=0, SAE Commit, Group=19",
                    128,
                )
                .says(k::PACKETS_WPA3_P2)
                .headers(&[
                    wifi!(
                        "0xb000",
                        "314",
                        "00:00:5e:00:53:01",
                        "00:00:5e:00:53:10",
                        "00:00:5e:00:53:01",
                        "SN=1, FN=0",
                        &[0]
                    ),
                    header(
                        &anatomy::SAE,
                        &[
                            "3",
                            "1",
                            "0",
                            "19",
                            "5c1e8a07…3fa2f0",
                            "x=9b04c27a…7d3e, y=41c8a9f3…e015",
                        ],
                        &[0, 1, 3, 4, 5],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    AP,
                    PHONE,
                    Class::Crypto,
                    "802.11",
                    "SAE Commit",
                    "Authentication, SN=1702, FN=0, SAE Commit, Group=19",
                    128,
                )
                .says(k::PACKETS_WPA3_P3)
                .headers(&[
                    wifi!(
                        "0xb000",
                        "314",
                        "00:00:5e:00:53:10",
                        "00:00:5e:00:53:01",
                        "00:00:5e:00:53:01",
                        "SN=1702, FN=0",
                        &[]
                    ),
                    header(
                        &anatomy::SAE,
                        &[
                            "3",
                            "1",
                            "0",
                            "19",
                            "e27d4c19…6b0b94",
                            "x=3fa1d850…c86b, y=07d2e4b1…5a19",
                        ],
                        &[4, 5],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WPA3_3_TITLE,
        sub: k::PACKETS_WPA3_3_SUB,
        body: k::PACKETS_WPA3_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SEQ, "2 (confirm)"),
            wire(k::PACKETS_LABEL_ALGORITHM, "HMAC-SHA-256 (KCK)"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_WPA3_3_VERDICT),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    AP,
                    Class::Crypto,
                    "802.11",
                    "SAE Confirm",
                    "Authentication, SN=2, FN=0, SAE Confirm, Send-Confirm=1",
                    64,
                )
                .says(k::PACKETS_WPA3_P4)
                .headers(&[wifi!(
                    "0xb000",
                    "314",
                    "00:00:5e:00:53:01",
                    "00:00:5e:00:53:10",
                    "00:00:5e:00:53:01",
                    "SN=2, FN=0",
                    &[]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    AP,
                    PHONE,
                    Class::Crypto,
                    "802.11",
                    "SAE Confirm",
                    "Authentication, SN=1703, FN=0, SAE Confirm, Send-Confirm=1",
                    64,
                )
                .says(k::PACKETS_WPA3_P5),
            ),
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: AP,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: ATTACKER,
                prop: Some(Prop::Cross),
            },
        ],
    },
    Step {
        title: k::PACKETS_WPA3_4_TITLE,
        sub: k::PACKETS_WPA3_4_SUB,
        body: k::PACKETS_WPA3_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_STATUS, "0, AID=1"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_WPA3_4_STATE),
        ],
        focus: &[PHONE, AP],
        beats: &[
            Beat::Send(
                Packet::new(
                    PHONE,
                    AP,
                    Class::Handshake,
                    "802.11",
                    "Assoc Req",
                    "Association Request, SN=3, FN=0, SSID=\"example\"",
                    196,
                )
                .says(k::PACKETS_WPA3_P6)
                .headers(&[wifi!(
                    "0x0000",
                    "314",
                    "00:00:5e:00:53:01",
                    "00:00:5e:00:53:10",
                    "00:00:5e:00:53:01",
                    "SN=3, FN=0",
                    &[0]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    AP,
                    PHONE,
                    Class::Handshake,
                    "802.11",
                    "Assoc Resp",
                    "Association Response, SN=1704, FN=0, Status=0, AID=1",
                    182,
                )
                .says(k::PACKETS_WPA3_P7),
            ),
        ],
    },
    Step {
        title: k::PACKETS_WPA3_5_TITLE,
        sub: k::PACKETS_WPA3_5_SUB,
        body: k::PACKETS_WPA3_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_NONCE, "ANonce, SNonce"),
            wire(k::PACKETS_LABEL_CIPHER, "CCMP-128"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_WPA3_5_STATE),
        ],
        focus: &[PHONE, AP],
        beats: &[
            Beat::Send(
                Packet::new(
                    AP,
                    PHONE,
                    Class::Crypto,
                    "EAPOL",
                    "M1 ANonce",
                    "EAPOL-Key 1/4, Info=0x0088, Replay=1",
                    133,
                )
                .says(k::PACKETS_WPA3_P8)
                .headers(&[
                    wifi!(
                        "0x8802",
                        "44",
                        "00:00:5e:00:53:10",
                        "00:00:5e:00:53:01",
                        "00:00:5e:00:53:01",
                        "SN=1705, FN=0",
                        &[]
                    ),
                    header(
                        &anatomy::EAPOL,
                        &[
                            "2",
                            "3",
                            "95",
                            "2",
                            "0x0088",
                            "16",
                            "1",
                            "8f3a51c0…e29b",
                            "0",
                        ],
                        &[1, 4, 6, 7],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    AP,
                    Class::Crypto,
                    "EAPOL",
                    "M2 SNonce",
                    "EAPOL-Key 2/4, Info=0x0108, Replay=1",
                    155,
                )
                .says(k::PACKETS_WPA3_P9)
                .headers(&[
                    wifi!(
                        "0x8801",
                        "44",
                        "00:00:5e:00:53:01",
                        "00:00:5e:00:53:10",
                        "00:00:5e:00:53:01",
                        "SN=4, FN=0",
                        &[]
                    ),
                    header(
                        &anatomy::EAPOL,
                        &[
                            "2",
                            "3",
                            "117",
                            "2",
                            "0x0108",
                            "0",
                            "1",
                            "c41d7e09…5a63",
                            "6b2e90d4…f018",
                            "RSNE: AKM 00-0F-AC:8, CCMP-128, MFPC=1, MFPR=1",
                        ],
                        &[4, 7, 8],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    AP,
                    PHONE,
                    Class::Crypto,
                    "EAPOL",
                    "M3 GTK",
                    "EAPOL-Key 3/4, Info=0x13c8, Replay=2",
                    221,
                )
                .says(k::PACKETS_WPA3_P10)
                .headers(&[header(
                    &anatomy::EAPOL,
                    &[
                        "2",
                        "3",
                        "183",
                        "2",
                        "0x13c8",
                        "16",
                        "2",
                        "8f3a51c0…e29b",
                        "91c03a7e…3d7a",
                        "AES-KW(KEK; RSNE, GTK, IGTK)",
                    ],
                    &[4, 8, 9],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    PHONE,
                    AP,
                    Class::Crypto,
                    "EAPOL",
                    "M4",
                    "EAPOL-Key 4/4, Info=0x0308, Replay=2",
                    133,
                )
                .says(k::PACKETS_WPA3_P11)
                .headers(&[header(
                    &anatomy::EAPOL,
                    &[
                        "2",
                        "3",
                        "95",
                        "2",
                        "0x0308",
                        "0",
                        "2",
                        "0",
                        "5e88b1f2…c412",
                    ],
                    &[4, 8],
                )]),
            ),
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Lock),
            },
            Beat::Light {
                at: AP,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_WPA3_6_TITLE,
        sub: k::PACKETS_WPA3_6_SUB,
        body: k::PACKETS_WPA3_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_FRAME, "Deauthentication, Reason=7"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_WPA3_6_VERDICT),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    ATTACKER,
                    PHONE,
                    Class::Refusal,
                    "802.11",
                    "Deauth",
                    "Deauthentication, SN=0, FN=0, Reason=7",
                    26,
                )
                .says(k::PACKETS_WPA3_P12)
                .headers(&[wifi!(
                    "0xc000",
                    "314",
                    "00:00:5e:00:53:10",
                    "00:00:5e:00:53:01",
                    "00:00:5e:00:53:01",
                    "SN=0, FN=0",
                    &[0, 3]
                )]),
            ),
            Beat::Light {
                at: PHONE,
                prop: Some(Prop::Shield),
            },
        ],
    },
];

/// WPA3.
pub static WPA3: Protocol = Protocol {
    id: "wpa3",
    group: Group::Security,
    layer: Layer(2),
    name: k::PACKETS_WPA3_NAME,
    title: k::PACKETS_WPA3_TITLE,
    tagline: k::PACKETS_WPA3_TAGLINE,
    overview: k::PACKETS_WPA3_OVERVIEW,
    facts: &[
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_WPA3_AUTHENTICATION,
        ),
        wire(k::PACKETS_FACT_ENCRYPTION, "CCMP-128 (AES)"),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_WPA3_SECURITY),
        wire(k::PACKETS_FACT_REPLACES, "WPA2-PSK"),
        wire(
            k::PACKETS_FACT_DEFINED,
            "Wi-Fi Alliance WPA3 Specification, IEEE 802.11-2020 (SAE, PMF), RFC 7664",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(PHONE, AP, 1),
            link(ATTACKER, PHONE, 1),
            link(ATTACKER, AP, 1),
        ],
    },
    steps: &STEPS,
};
