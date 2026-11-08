//! RADIUS and 802.1X: a laptop let onto an enterprise Wi-Fi network — EAP
//! over EAPOL to the access point, relayed inside RADIUS to the server, an
//! EAP-TLS handshake that checks both certificates, the directory asked,
//! Access-Accept with a key and a VLAN, and the session recorded.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const LAPTOP: usize = 0;
const AP: usize = 1;
const SERVER: usize = 2;
const DIRECTORY: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_LAPTOP,
        address: Address::Mac([0x00, 0x00, 0x5e, 0x00, 0x53, 0x20]),
    },
    Device {
        kind: Kind::AccessPoint,
        at: at(-4, -1),
        name: k::PACKETS_DEVICE_ACCESS_POINT,
        address: Address::V4([10, 0, 0, 2]),
    },
    Device {
        kind: Kind::Tower(Emblem::Key),
        at: at(3, 2),
        name: k::PACKETS_DEVICE_RADIUS_SERVER,
        address: Address::V4([10, 0, 0, 10]),
    },
    Device {
        kind: Kind::Rack,
        at: at(8, -3),
        name: k::PACKETS_RADIUS_DEVICE_DIRECTORY,
        address: Address::V4([10, 0, 0, 20]),
    },
];

/// The access point's first 802.11 data frame to the laptop.
const WIFI_DOWN: &[&str] = &[
    "0x8802",
    "44",
    "00:00:5e:00:53:20",
    "00:00:5e:00:53:01",
    "00:00:5e:00:53:01",
    "SN=12, FN=0",
];
/// And the laptop's answer to it.
const WIFI_UP: &[&str] = &[
    "0x8801",
    "44",
    "00:00:5e:00:53:01",
    "00:00:5e:00:53:20",
    "00:00:5e:00:53:01",
    "SN=5, FN=0",
];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_RADIUS_1_TITLE,
        sub: k::PACKETS_RADIUS_1_SUB,
        body: k::PACKETS_RADIUS_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_IDENTITY, "user@example.org"),
            wire(k::PACKETS_LABEL_PORT, "UDP 1812"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(AP, LAPTOP, Class::Request, "EAP", "Request/Identity", "Request, Identity", 43)
                    .says(k::PACKETS_RADIUS_P1)
                    .headers(&[
                        header(&anatomy::WIFI, WIFI_DOWN, &[]),
                        header(&anatomy::EAPOL, &["2", "0", "5"], &[1]),
                    ]),
            ),
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    AP,
                    Class::Response,
                    "EAP",
                    "user@example.org",
                    "Response, Identity: user@example.org",
                    59,
                )
                .says(k::PACKETS_RADIUS_P2)
                .headers(&[
                    header(&anatomy::WIFI, WIFI_UP, &[]),
                    header(&anatomy::EAPOL, &["2", "0", "21"], &[1, 2]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    AP,
                    SERVER,
                    Class::Request,
                    "RADIUS",
                    "Access-Request",
                    "Access-Request id=0",
                    146,
                )
                .says(k::PACKETS_RADIUS_P3)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "132", "0x2f41", "000", "0", "64", "17", "0x3b5e", "10.0.0.2", "10.0.0.10",
                        ],
                        &[9],
                    ),
                    header(&anatomy::UDP, &["41644", "1812", "112", "0x6e12"], &[1]),
                    header(
                        &anatomy::RADIUS,
                        &[
                            "1",
                            "0",
                            "104",
                            "0x9f4c13d2…27e1",
                            "User-Name=user@example.org, NAS-IP-Address=10.0.0.2, \
                             Calling-Station-Id=00-00-5E-00-53-20, EAP-Message, Message-Authenticator",
                        ],
                        &[0, 1, 3, 4],
                    ),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_RADIUS_2_TITLE,
        sub: k::PACKETS_RADIUS_2_SUB,
        body: k::PACKETS_RADIUS_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PROTOCOL, "EAP-TLS (TLS 1.3)"),
            wire(k::PACKETS_LABEL_CERTIFICATE, "CN=radius.example.org"),
            wire(k::PACKETS_LABEL_TRUST, "CN=Example Root CA"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SERVER,
                    Class::Crypto,
                    "EAP-TLS",
                    "ClientHello",
                    "EAP-Response/TLS ClientHello",
                    303,
                )
                .says(k::PACKETS_RADIUS_P4)
                .headers(&[header(
                    &anatomy::TLS,
                    &[
                        "22",
                        "0x0301",
                        "254",
                        "1",
                        "250",
                        "TLS_AES_128_GCM_SHA256, key_share x25519",
                    ],
                    &[3],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    LAPTOP,
                    Class::Crypto,
                    "EAP-TLS",
                    "Certificate",
                    "EAP-Request/TLS ServerHello, Certificate, CertificateVerify, Finished",
                    2_800,
                )
                .says(k::PACKETS_RADIUS_P5)
                .headers(&[
                    header(
                        &anatomy::TLS,
                        &[
                            "22",
                            "0x0303",
                            "122",
                            "2",
                            "118",
                            "TLS_AES_128_GCM_SHA256, key_share x25519",
                        ],
                        &[3],
                    ),
                    header(
                        &anatomy::TLS,
                        &[
                            "23",
                            "0x0303",
                            "2650",
                            "",
                            "",
                            "EncryptedExtensions, CertificateRequest, Certificate, CertificateVerify, Finished",
                        ],
                        &[0, 5],
                    ),
                ]),
            ),
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Certificate),
            },
        ],
    },
    Step {
        title: k::PACKETS_RADIUS_3_TITLE,
        sub: k::PACKETS_RADIUS_3_SUB,
        body: k::PACKETS_RADIUS_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CERTIFICATE, "CN=user@example.org"),
            wire(k::PACKETS_LABEL_ACCOUNT, "memberOf: cn=staff"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_RADIUS_3_VERDICT),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    LAPTOP,
                    SERVER,
                    Class::Crypto,
                    "EAP-TLS",
                    "Certificate",
                    "EAP-Response/TLS Certificate, CertificateVerify, Finished",
                    1_900,
                )
                .says(k::PACKETS_RADIUS_P6)
                .headers(&[header(
                    &anatomy::TLS,
                    &[
                        "23",
                        "0x0303",
                        "1840",
                        "",
                        "",
                        "Certificate, CertificateVerify, Finished",
                    ],
                    &[0, 5],
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    DIRECTORY,
                    Class::Lookup,
                    "LDAP",
                    "searchRequest",
                    "searchRequest(2) \"dc=example,dc=org\" wholeSubtree",
                    160,
                )
                .says(k::PACKETS_RADIUS_P7),
            ),
            Beat::Send(
                Packet::new(
                    DIRECTORY,
                    SERVER,
                    Class::Response,
                    "LDAP",
                    "searchResEntry",
                    "searchResEntry(2) \"cn=user,ou=people,dc=example,dc=org\"",
                    420,
                )
                .says(k::PACKETS_RADIUS_P8),
            ),
            Beat::Light {
                at: SERVER,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_RADIUS_4_TITLE,
        sub: k::PACKETS_RADIUS_4_SUB,
        body: k::PACKETS_RADIUS_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_STATUS, "Access-Accept"),
            wire(k::PACKETS_LABEL_VLAN, "20"),
            fact(k::PACKETS_LABEL_ENCRYPTION, k::PACKETS_RADIUS_4_ENCRYPTION),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    SERVER,
                    AP,
                    Class::Response,
                    "RADIUS",
                    "Access-Accept",
                    "Access-Accept id=6",
                    219,
                )
                .says(k::PACKETS_RADIUS_P9)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "205", "0x0000", "000", "0", "64", "17", "0x6a1b", "10.0.0.10", "10.0.0.2",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["1812", "41644", "185", "0x1f07"], &[0]),
                    header(
                        &anatomy::RADIUS,
                        &[
                            "2",
                            "6",
                            "177",
                            "0x51a8e0c3…9d42",
                            "EAP-Message (EAP-Success), MS-MPPE-Recv-Key, MS-MPPE-Send-Key, Tunnel-Type=VLAN, \
                             Tunnel-Medium-Type=IEEE-802, Tunnel-Private-Group-Id=20, Message-Authenticator",
                        ],
                        &[0, 3, 4],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(AP, LAPTOP, Class::Response, "EAP", "EAP-Success", "Success", 42)
                    .says(k::PACKETS_RADIUS_P10)
                    .headers(&[
                        header(
                            &anatomy::WIFI,
                            &[
                                "0x8802",
                                "44",
                                "00:00:5e:00:53:20",
                                "00:00:5e:00:53:01",
                                "00:00:5e:00:53:01",
                                "SN=19, FN=0",
                            ],
                            &[],
                        ),
                        header(&anatomy::EAPOL, &["2", "0", "4"], &[2]),
                    ]),
            ),
            Beat::Light {
                at: AP,
                prop: Some(Prop::Key),
            },
            Beat::Light {
                at: LAPTOP,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_RADIUS_5_TITLE,
        sub: k::PACKETS_RADIUS_5_SUB,
        body: k::PACKETS_RADIUS_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "UDP 1813"),
            wire(k::PACKETS_LABEL_SESSION, "Acct-Session-Id=5F0C2A71"),
        ],
        focus: &[AP, SERVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    AP,
                    SERVER,
                    Class::Control,
                    "RADIUS",
                    "Acct Start",
                    "Accounting-Request id=0",
                    121,
                )
                .says(k::PACKETS_RADIUS_P11)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "107", "0x2f52", "000", "0", "64", "17", "0x3b4d", "10.0.0.2", "10.0.0.10",
                        ],
                        &[],
                    ),
                    header(&anatomy::UDP, &["41645", "1813", "87", "0x90c4"], &[1]),
                    header(
                        &anatomy::RADIUS,
                        &[
                            "4",
                            "0",
                            "79",
                            "0x7be2a05f…14ac",
                            "Acct-Status-Type=Start, Acct-Session-Id=5F0C2A71, User-Name=user@example.org, \
                             Calling-Station-Id=00-00-5E-00-53-20, NAS-IP-Address=10.0.0.2",
                        ],
                        &[0, 4],
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    AP,
                    Class::Control,
                    "RADIUS",
                    "Acct Response",
                    "Accounting-Response id=0",
                    62,
                )
                .says(k::PACKETS_RADIUS_P12),
            ),
        ],
    },
];

/// RADIUS and 802.1X.
pub static RADIUS: Protocol = Protocol {
    id: "radius",
    group: Group::Security,
    layer: Layer(7),
    name: k::PACKETS_RADIUS_NAME,
    title: k::PACKETS_RADIUS_TITLE,
    tagline: k::PACKETS_RADIUS_TAGLINE,
    overview: k::PACKETS_RADIUS_OVERVIEW,
    facts: &[
        fact(k::PACKETS_FACT_PORT, k::PACKETS_RADIUS_PORT),
        wire(k::PACKETS_FACT_TRANSPORT, "EAPOL, UDP"),
        wire(k::PACKETS_FACT_AUTHENTICATION, "EAP-TLS, PEAP, EAP-TTLS"),
        fact(k::PACKETS_FACT_MODEL, k::PACKETS_RADIUS_MODEL),
        wire(
            k::PACKETS_FACT_DEFINED,
            "IEEE 802.1X-2020, RFC 2865, RFC 2866, RFC 3579, RFC 9190",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(LAPTOP, AP, 1),
            link(AP, SERVER, 1),
            link(SERVER, DIRECTORY, 1),
        ],
    },
    steps: &STEPS,
};
