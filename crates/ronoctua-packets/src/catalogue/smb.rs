//! SMB: a file read from a Windows share — SMB 3.1.1 negotiated on port
//! 445, the user proved by a Kerberos ticket from the domain controller, the
//! share connected (and encrypted), a file opened, read and closed.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const DESKTOP: usize = 0;
const SWITCH: usize = 1;
const CONTROLLER: usize = 2;
const SERVER: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Desktop,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_DESKTOP,
        address: Address::V4([192, 168, 10, 21]),
    },
    Device {
        kind: Kind::Switch,
        at: at(-2, 0),
        name: k::PACKETS_DEVICE_SWITCH,
        address: Address::None,
    },
    Device {
        kind: Kind::Tower(Emblem::Key),
        at: at(3, -5),
        name: k::PACKETS_SMB_DEVICE_DC,
        address: Address::V4([192, 168, 10, 5]),
    },
    Device {
        kind: Kind::Storage,
        at: at(6, 2),
        name: k::PACKETS_DEVICE_FILE_SERVER,
        address: Address::V4([192, 168, 10, 20]),
    },
];

/// An SMB2 header: credit charge, status, command, credits, flags, message
/// ID, tree ID, session ID, signature — the protocol ID, the structure size,
/// the next command and the reserved word are the same in all of them.
macro_rules! smb2 {
    ($charge:expr, $status:expr, $command:expr, $credits:expr, $flags:expr, $message:expr,
     $tree:expr, $session:expr, $signature:expr, $lit:expr) => {
        header(
            &anatomy::SMB2,
            &[
                "0xFE534D42",
                "64",
                $charge,
                $status,
                $command,
                $credits,
                $flags,
                "0",
                $message,
                "0x0000FEFF",
                $tree,
                $session,
                $signature,
            ],
            $lit,
        )
    };
}

/// The session the server grants.
const SESSION: &str = "0x00001c0000000045";

/// The way between the desktop and the file server, for the steps' frames.
const WAY: &[usize] = &[DESKTOP, SWITCH, SERVER];

static STEPS: [Step; 5] = [
    Step {
        title: k::PACKETS_SMB_1_TITLE,
        sub: k::PACKETS_SMB_1_SUB,
        body: k::PACKETS_SMB_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SERVER_PORT, "445"),
            wire(k::PACKETS_LABEL_DIALECT, "3.1.1"),
            wire(k::PACKETS_LABEL_CIPHER, "AES-128-GCM"),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    SERVER,
                    Class::Handshake,
                    "SMB2",
                    "NEGOTIATE",
                    "NEGOTIATE Dialects=2.0.2, 2.1, 3.0, 3.0.2, 3.1.1",
                    318,
                )
                .says(k::PACKETS_SMB_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "304",
                            "0x51a2",
                            "010",
                            "0",
                            "128",
                            "6",
                            "0x13f6",
                            "192.168.10.21",
                            "192.168.10.20",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "49702", "445", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0",
                            "0", "1026", "0xa61c", "0",
                        ],
                        &[1],
                    ),
                    smb2!(
                        "0",
                        "0x00000000",
                        "0x0000",
                        "1",
                        "0x00000000",
                        "0",
                        "0",
                        "0",
                        "0",
                        &[0, 4]
                    ),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    DESKTOP,
                    Class::Handshake,
                    "SMB2",
                    "NEGOTIATE 3.1.1",
                    "NEGOTIATE STATUS_SUCCESS Dialect=3.1.1 Cipher=AES-128-GCM",
                    362,
                )
                .says(k::PACKETS_SMB_P2)
                .headers(&[smb2!(
                    "0",
                    "0x00000000",
                    "0x0000",
                    "1",
                    "0x00000001",
                    "0",
                    "0",
                    "0",
                    "0",
                    &[3, 6]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMB_2_TITLE,
        sub: k::PACKETS_SMB_2_SUB,
        body: k::PACKETS_SMB_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ACCOUNT, "alice@EXAMPLE.COM"),
            fact(k::PACKETS_LABEL_PROTOCOL, k::PACKETS_SMB_2_PROTOCOL),
            wire(k::PACKETS_LABEL_SESSION, SESSION),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    CONTROLLER,
                    Class::Crypto,
                    "KRB5",
                    "TGS-REQ",
                    "TGS-REQ cifs/fs1.example.com",
                    1_478,
                )
                .says(k::PACKETS_SMB_P3)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4",
                            "5",
                            "0",
                            "0",
                            "1464",
                            "0x51a4",
                            "010",
                            "0",
                            "128",
                            "6",
                            "0x0e2b",
                            "192.168.10.21",
                            "192.168.10.5",
                        ],
                        &[12],
                    ),
                    header(&anatomy::TCP, &["49703", "88"], &[1]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    CONTROLLER,
                    DESKTOP,
                    Class::Crypto,
                    "KRB5",
                    "TGS-REP",
                    "TGS-REP cifs/fs1.example.com",
                    1_612,
                )
                .says(k::PACKETS_SMB_P4),
            ),
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    SERVER,
                    Class::Crypto,
                    "SMB2",
                    "SESSION_SETUP",
                    "SESSION_SETUP Kerberos AP-REQ cifs/fs1.example.com",
                    1_696,
                )
                .says(k::PACKETS_SMB_P5)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0001",
                    "33",
                    "0x00000000",
                    "1",
                    "0",
                    "0",
                    "0",
                    &[4]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    DESKTOP,
                    Class::Crypto,
                    "SMB2",
                    "STATUS_SUCCESS",
                    "SESSION_SETUP STATUS_SUCCESS SessionId=0x00001c0000000045",
                    326,
                )
                .says(k::PACKETS_SMB_P6)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0001",
                    "33",
                    "0x00000009",
                    "1",
                    "0",
                    SESSION,
                    "0x3f9c…e21a",
                    &[3, 11, 12]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMB_3_TITLE,
        sub: k::PACKETS_SMB_3_SUB,
        body: k::PACKETS_SMB_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_SHARE, "\\\\fs1.example.com\\share"),
            wire(k::PACKETS_LABEL_FLAGS, "SMB2_SHAREFLAG_ENCRYPT_DATA"),
            wire(k::PACKETS_LABEL_ENCRYPTION, "AES-128-GCM"),
        ],
        focus: WAY,
        beats: &[
            Beat::Type {
                at: DESKTOP,
                wire: "\\\\fs1.example.com\\share",
            },
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    SERVER,
                    Class::Request,
                    "SMB2",
                    "TREE_CONNECT",
                    "TREE_CONNECT Path=\\\\fs1.example.com\\share",
                    176,
                )
                .says(k::PACKETS_SMB_P7)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0003",
                    "33",
                    "0x00000008",
                    "2",
                    "0",
                    SESSION,
                    "0x8a41…07bd",
                    &[4, 11]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    DESKTOP,
                    Class::Response,
                    "SMB2",
                    "TreeId=5",
                    "TREE_CONNECT STATUS_SUCCESS TreeId=5 ShareFlags=ENCRYPT_DATA",
                    138,
                )
                .says(k::PACKETS_SMB_P8)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0003",
                    "33",
                    "0x00000009",
                    "2",
                    "5",
                    SESSION,
                    "0x51e0…9c3f",
                    &[10]
                )]),
            ),
            Beat::Light {
                at: DESKTOP,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_SMB_4_TITLE,
        sub: k::PACKETS_SMB_4_SUB,
        body: k::PACKETS_SMB_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PATH, "example.docx"),
            wire(k::PACKETS_LABEL_COMMAND, "CREATE, READ"),
            wire(k::PACKETS_LABEL_SIZE, "48213"),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    SERVER,
                    Class::Request,
                    "SMB2",
                    "CREATE example.docx",
                    "CREATE example.docx Disposition=FILE_OPEN",
                    254,
                )
                .says(k::PACKETS_SMB_P9)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0005",
                    "33",
                    "0x00000000",
                    "3",
                    "5",
                    SESSION,
                    "0",
                    &[4]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    DESKTOP,
                    Class::Response,
                    "SMB2",
                    "FileId",
                    "CREATE STATUS_SUCCESS File=example.docx EndOfFile=48213",
                    262,
                )
                .says(k::PACKETS_SMB_P10)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0005",
                    "33",
                    "0x00000001",
                    "3",
                    "5",
                    SESSION,
                    "0",
                    &[3]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    SERVER,
                    Class::Request,
                    "SMB2",
                    "READ 65536",
                    "READ File=example.docx Length=65536 Offset=0",
                    223,
                )
                .says(k::PACKETS_SMB_P11)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0008",
                    "33",
                    "0x00000000",
                    "4",
                    "5",
                    SESSION,
                    "0",
                    &[2, 4]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    DESKTOP,
                    Class::Data,
                    "SMB2",
                    "READ 48213",
                    "READ STATUS_SUCCESS DataLength=48213",
                    50_185,
                )
                .says(k::PACKETS_SMB_P12)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0008",
                    "33",
                    "0x00000001",
                    "4",
                    "5",
                    SESSION,
                    "0",
                    &[4]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SMB_5_TITLE,
        sub: k::PACKETS_SMB_5_SUB,
        body: k::PACKETS_SMB_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_COMMAND, "CLOSE"),
            fact(k::PACKETS_LABEL_STATE, k::PACKETS_SMB_5_STATE),
        ],
        focus: WAY,
        beats: &[
            Beat::Send(
                Packet::new(
                    DESKTOP,
                    SERVER,
                    Class::Control,
                    "SMB2",
                    "CLOSE",
                    "CLOSE File=example.docx",
                    198,
                )
                .says(k::PACKETS_SMB_P13)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0006",
                    "33",
                    "0x00000000",
                    "5",
                    "5",
                    SESSION,
                    "0",
                    &[4]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    DESKTOP,
                    Class::Control,
                    "SMB2",
                    "STATUS_SUCCESS",
                    "CLOSE STATUS_SUCCESS",
                    234,
                )
                .says(k::PACKETS_SMB_P14)
                .headers(&[smb2!(
                    "1",
                    "0x00000000",
                    "0x0006",
                    "33",
                    "0x00000001",
                    "5",
                    "5",
                    SESSION,
                    "0",
                    &[3]
                )]),
            ),
            Beat::Light {
                at: DESKTOP,
                prop: Some(Prop::Check),
            },
        ],
    },
];

/// SMB.
pub static SMB: Protocol = Protocol {
    id: "smb",
    group: Group::Mail,
    layer: Layer(7),
    name: k::PACKETS_SMB_NAME,
    title: k::PACKETS_SMB_TITLE,
    tagline: k::PACKETS_SMB_TAGLINE,
    overview: k::PACKETS_SMB_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "445"),
        fact(k::PACKETS_FACT_TRANSPORT, k::PACKETS_SMB_TRANSPORT),
        wire(k::PACKETS_FACT_ENCRYPTION, "AES-128-GCM, AES-256-GCM"),
        wire(k::PACKETS_FACT_REPLACES, "SMB1 (CIFS)"),
        wire(k::PACKETS_FACT_DEFINED, "MS-SMB2"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(DESKTOP, SWITCH, 1),
            link(SWITCH, CONTROLLER, 1),
            link(SWITCH, SERVER, 1),
        ],
    },
    steps: &STEPS,
};
