//! SSH: a remote shell — the version lines, the algorithms negotiated
//! (post-quantum key exchange first), the key exchange signed with the
//! server's host key and checked against `known_hosts`, NEWKEYS, the user
//! authenticated by a public key, and a session channel with its shell.

use crate::anatomy::{self, header};
use crate::art::{Emblem, Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const ROUTER: usize = 1;
const INTERNET: usize = 2;
const SERVER: usize = 3;

static DEVICES: [Device; 4] = [
    Device {
        kind: Kind::Laptop,
        at: at(-8, 3),
        name: k::PACKETS_DEVICE_CLIENT,
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
        kind: Kind::Tower(Emblem::Key),
        at: at(6, -2),
        name: k::PACKETS_DEVICE_SSH_SERVER,
        address: Address::V4([203, 0, 113, 22]),
    },
];

/// An SSH packet before the keys: packet length, padding length, payload,
/// padding — and no MAC yet. The length plus its own four bytes is a
/// multiple of 8.
macro_rules! clear {
    ($len:literal, $pad:literal, $payload:literal, $padding:literal, $lit:expr) => {
        header(&anatomy::SSH, &[$len, $pad, $payload, $padding], $lit)
    };
}

/// An SSH packet after NEWKEYS, as a capture holding the session's keys shows
/// it: chacha20-poly1305@openssh.com encrypts the length on its own, aligns
/// the rest to 8 bytes and adds a 16-byte Poly1305 tag as its MAC.
macro_rules! sealed {
    ($len:literal, $pad:literal, $payload:literal, $padding:literal, $mac:literal, $lit:expr) => {
        header(&anatomy::SSH, &[$len, $pad, $payload, $padding, $mac], $lit)
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_SSH_1_TITLE,
        sub: k::PACKETS_SSH_1_SUB,
        body: k::PACKETS_SSH_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_PORT, "22"),
            wire(k::PACKETS_LABEL_VERSION, "SSH-2.0-OpenSSH_10.0"),
        ],
        focus: &[],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "ssh admin@ssh.example.com",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "SSHv2",
                    "SSH-2.0",
                    "SSH-2.0-OpenSSH_10.0",
                    76,
                )
                .says(k::PACKETS_SSH_P1)
                .headers(&[
                    header(
                        &anatomy::IPV4,
                        &[
                            "4", "5", "0", "0", "62", "0x2d10", "010", "0", "64", "6", "0x6f4b", "192.168.1.10",
                            "203.0.113.22",
                        ],
                        &[],
                    ),
                    header(
                        &anatomy::TCP,
                        &[
                            "52344", "22", "1", "1", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "502", "0x1f6a",
                            "0",
                        ],
                        &[1],
                    ),
                    header(&anatomy::LINE, &["SSH-2.0-OpenSSH_10.0"], &[0]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "SSHv2",
                    "SSH-2.0",
                    "SSH-2.0-OpenSSH_10.0",
                    76,
                )
                .says(k::PACKETS_SSH_P2)
                .headers(&[
                    header(
                        &anatomy::TCP,
                        &[
                            "22", "52344", "1", "23", "5", "0", "0", "0", "0", "1", "1", "0", "0", "0", "509", "0x8c02",
                            "0",
                        ],
                        &[0],
                    ),
                    header(&anatomy::LINE, &["SSH-2.0-OpenSSH_10.0"], &[0]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SSH_2_TITLE,
        sub: k::PACKETS_SSH_2_SUB,
        body: k::PACKETS_SSH_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "mlkem768x25519-sha256"),
            wire(k::PACKETS_SSH_LABEL_HOST_KEY, "ssh-ed25519"),
            wire(k::PACKETS_LABEL_CIPHER, "chacha20-poly1305@openssh.com"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Handshake,
                    "SSHv2",
                    "KEXINIT",
                    "SSH_MSG_KEXINIT (20)",
                    1_462,
                )
                .says(k::PACKETS_SSH_P3)
                .headers(&[clear!(
                    "1404",
                    "6",
                    "20 KEXINIT; kex_algorithms: mlkem768x25519-sha256, sntrup761x25519-sha512, curve25519-sha256, …; \
                     server_host_key_algorithms: ssh-ed25519, ecdsa-sha2-nistp256, …; \
                     encryption_algorithms: chacha20-poly1305@openssh.com, aes256-gcm@openssh.com, …",
                    "a3 1f 07 c9 5e 22",
                    &[0, 2]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Handshake,
                    "SSHv2",
                    "KEXINIT",
                    "SSH_MSG_KEXINIT (20)",
                    1_142,
                )
                .says(k::PACKETS_SSH_P4)
                .headers(&[clear!(
                    "1084",
                    "7",
                    "20 KEXINIT; kex_algorithms: mlkem768x25519-sha256, sntrup761x25519-sha512, curve25519-sha256, …; \
                     server_host_key_algorithms: ssh-ed25519; \
                     encryption_algorithms: chacha20-poly1305@openssh.com, aes256-gcm@openssh.com, …",
                    "5c e0 91 4d 2b 7a f3",
                    &[2]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SSH_3_TITLE,
        sub: k::PACKETS_SSH_3_SUB,
        body: k::PACKETS_SSH_3_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_KEY_EXCHANGE, "mlkem768x25519-sha256"),
            wire(k::PACKETS_LABEL_SIZE, "1216 B → 1120 B"),
            wire(k::PACKETS_LABEL_SIGNATURE, "ssh-ed25519"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Crypto,
                    "SSHv2",
                    "KEX_HYBRID_INIT",
                    "SSH_MSG_KEX_HYBRID_INIT (30)",
                    1_286,
                )
                .says(k::PACKETS_SSH_P5)
                .headers(&[clear!(
                    "1228",
                    "6",
                    "30 KEX_HYBRID_INIT; C_INIT: ML-KEM-768 7c0a…e93b (1184 B) ‖ X25519 4b7e…d013 (32 B)",
                    "e1 4b 90 3c 7d 08",
                    &[2]
                )]),
            ),
            Beat::Light {
                at: SERVER,
                prop: Some(Prop::Key),
            },
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Crypto,
                    "SSHv2",
                    "KEX_HYBRID_REPLY",
                    "SSH_MSG_KEX_HYBRID_REPLY (31), SSH_MSG_NEWKEYS (21)",
                    1_350,
                )
                .says(k::PACKETS_SSH_P6)
                .headers(&[clear!(
                    "1276",
                    "8",
                    "31 KEX_HYBRID_REPLY; K_S: ssh-ed25519 AAAAC3NzaC1lZDI1NTE5…; \
                     S_REPLY: ML-KEM-768 3d81…a4c6 (1088 B) ‖ X25519 9fd7…c3a1 (32 B); signature(H): ssh-ed25519",
                    "d2 6a 0e 91 7c 35 b8 4f",
                    &[2]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SSH_4_TITLE,
        sub: k::PACKETS_SSH_4_SUB,
        body: k::PACKETS_SSH_4_BODY,
        facts: &[
            wire(
                k::PACKETS_SSH_LABEL_FINGERPRINT,
                "SHA256:3q2+7wzK9fYv0bQ1mXc4tR8eLh5uJ6aN2pD9sGkWfZo",
            ),
            wire(k::PACKETS_LABEL_TRUST, "~/.ssh/known_hosts"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Key),
            },
            Beat::Type {
                at: CLIENT,
                wire: "yes",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Control,
                    "SSHv2",
                    "NEWKEYS",
                    "SSH_MSG_NEWKEYS (21)",
                    70,
                )
                .says(k::PACKETS_SSH_P7)
                .headers(&[clear!("12", "10", "21 NEWKEYS", "4f a2 19 c0 7d 3e 85 b1 06 e9", &[2])]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Lock),
            },
        ],
    },
    Step {
        title: k::PACKETS_SSH_5_TITLE,
        sub: k::PACKETS_SSH_5_SUB,
        body: k::PACKETS_SSH_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_USER, "admin"),
            wire(k::PACKETS_LABEL_METHOD, "publickey"),
            wire(k::PACKETS_LABEL_SIGNATURE, "ssh-ed25519, sk-ssh-ed25519@openssh.com"),
        ],
        focus: &[],
        beats: &[
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Key),
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Crypto,
                    "SSHv2",
                    "USERAUTH_REQUEST",
                    "SSH_MSG_USERAUTH_REQUEST (50) publickey",
                    282,
                )
                .says(k::PACKETS_SSH_P8)
                .headers(&[sealed!(
                    "208",
                    "8",
                    "50 USERAUTH_REQUEST; user: admin; service: ssh-connection; method: publickey; \
                     algorithm: ssh-ed25519; public_key: AAAAC3NzaC1lZDI1NTE5…; signature: ssh-ed25519",
                    "9b 03 e7 51 c4 2a 6f d8",
                    "6c1f…d2a0 (16 B)",
                    &[2, 4]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Response,
                    "SSHv2",
                    "USERAUTH_SUCCESS",
                    "SSH_MSG_USERAUTH_SUCCESS (52)",
                    82,
                )
                .says(k::PACKETS_SSH_P9)
                .headers(&[sealed!(
                    "8",
                    "6",
                    "52 USERAUTH_SUCCESS",
                    "e0 4a 71 9c 2b d5",
                    "a19e…7b19 (16 B)",
                    &[2]
                )]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_SSH_6_TITLE,
        sub: k::PACKETS_SSH_6_SUB,
        body: k::PACKETS_SSH_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_CHANNEL, "session"),
            wire(k::PACKETS_LABEL_REQUEST, "pty-req, shell"),
        ],
        focus: &[],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Request,
                    "SSHv2",
                    "CHANNEL_OPEN",
                    "SSH_MSG_CHANNEL_OPEN (90) session",
                    106,
                )
                .says(k::PACKETS_SSH_P10)
                .headers(&[sealed!(
                    "32",
                    "7",
                    "90 CHANNEL_OPEN; channel_type: session; sender_channel: 0; initial_window_size: 2097152; \
                     maximum_packet_size: 32768",
                    "3b 8e 52 f0 19 a6 c7",
                    "58d3…0e4f (16 B)",
                    &[2]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Response,
                    "SSHv2",
                    "CHANNEL_OPEN_CONFIRMATION",
                    "SSH_MSG_CHANNEL_OPEN_CONFIRMATION (91)",
                    98,
                )
                .says(k::PACKETS_SSH_P11)
                .headers(&[sealed!(
                    "24",
                    "6",
                    "91 CHANNEL_OPEN_CONFIRMATION; recipient_channel: 0; sender_channel: 0; \
                     initial_window_size: 2097152; maximum_packet_size: 32768",
                    "71 c2 0d 9e 46 b3",
                    "c04b…91e7 (16 B)",
                    &[2]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    CLIENT,
                    SERVER,
                    Class::Request,
                    "SSHv2",
                    "CHANNEL_REQUEST",
                    "SSH_MSG_CHANNEL_REQUEST (98) shell",
                    98,
                )
                .says(k::PACKETS_SSH_P12)
                .headers(&[sealed!(
                    "24",
                    "8",
                    "98 CHANNEL_REQUEST; recipient_channel: 0; request: shell; want_reply: 1",
                    "e8 25 7a 03 bc 61 9f 14",
                    "2a6e…f3c8 (16 B)",
                    &[2]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    SERVER,
                    CLIENT,
                    Class::Data,
                    "SSHv2",
                    "CHANNEL_DATA",
                    "SSH_MSG_CHANNEL_DATA (94) len=13",
                    106,
                )
                .says(k::PACKETS_SSH_P13)
                .headers(&[sealed!(
                    "32",
                    "9",
                    "94 CHANNEL_DATA; recipient_channel: 0; data: admin@ssh:~$ ",
                    "06 d9 4f 82 e1 3c 57 a0 bb",
                    "97f0…1c2d (16 B)",
                    &[2]
                )]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Check),
            },
        ],
    },
];

/// SSH.
pub static SSH: Protocol = Protocol {
    id: "ssh",
    group: Group::Security,
    layer: Layer(7),
    name: k::PACKETS_SSH_NAME,
    title: k::PACKETS_SSH_TITLE,
    tagline: k::PACKETS_SSH_TAGLINE,
    overview: k::PACKETS_SSH_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "22"),
        wire(k::PACKETS_FACT_TRANSPORT, "TCP"),
        wire(
            k::PACKETS_FACT_ENCRYPTION,
            "chacha20-poly1305@openssh.com, aes256-gcm@openssh.com",
        ),
        fact(
            k::PACKETS_FACT_AUTHENTICATION,
            k::PACKETS_SSH_AUTHENTICATION,
        ),
        wire(
            k::PACKETS_FACT_DEFINED,
            "RFC 4251, RFC 4252, RFC 4253, RFC 4254",
        ),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, ROUTER, 1),
            link(ROUTER, INTERNET, 4),
            link(INTERNET, SERVER, 4),
        ],
    },
    steps: &STEPS,
};
