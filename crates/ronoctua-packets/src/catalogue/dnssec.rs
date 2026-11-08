//! DNSSEC: a signed answer, the zone's keys, the chain of trust up to the
//! root's key, the validated answer — and a forged one refused.
//!
//! The referrals that lead the resolver to `example.com`'s server are the
//! DNS protocol's story; here the resolver already knows where to ask.

use crate::anatomy::{self, header};
use crate::art::{Kind, Prop};
use crate::key as k;
use crate::{
    Address, Beat, Class, Device, Group, Layer, Packet, Protocol, Scene, Step, at, fact, link, wire,
};

const CLIENT: usize = 0;
const RESOLVER: usize = 1;
const ROOT: usize = 2;
const TLD: usize = 3;
const AUTHORITATIVE: usize = 4;
const ATTACKER: usize = 5;

static DEVICES: [Device; 6] = [
    Device {
        kind: Kind::Laptop,
        at: at(-9, 3),
        name: k::PACKETS_DEVICE_CLIENT,
        address: Address::V4([192, 168, 1, 10]),
    },
    Device {
        kind: Kind::Rack,
        at: at(-3, 0),
        name: k::PACKETS_DEVICE_RESOLVER,
        address: Address::V4([8, 8, 8, 8]),
    },
    Device {
        kind: Kind::Rack,
        at: at(2, -6),
        name: k::PACKETS_DEVICE_ROOT_SERVER,
        address: Address::V4([198, 41, 0, 4]),
    },
    Device {
        kind: Kind::Rack,
        at: at(7, -1),
        name: k::PACKETS_DEVICE_TLD_SERVER,
        address: Address::V4([192, 5, 6, 30]),
    },
    Device {
        kind: Kind::Rack,
        at: at(4, 5),
        name: k::PACKETS_DEVICE_AUTHORITATIVE,
        address: Address::V4([198, 51, 100, 53]),
    },
    Device {
        kind: Kind::Laptop,
        at: at(-4, 6),
        name: k::PACKETS_DEVICE_ATTACKER,
        address: Address::V4([203, 0, 113, 66]),
    },
];

/// The question about `www.example.com`.
const WWW: &str = "www.example.com  A  IN";

/// A DNS header: ID, the flags (QR, opcode, AA, TC, RD, RA), the three Z
/// bits (Z, AD, CD), RCODE, the four counts — the additional one counting
/// the EDNS OPT record that carries DO — then the question and the answer.
macro_rules! dns {
    ($id:literal, $qr:literal, $aa:literal, $rd:literal, $ra:literal, $z:literal, $rcode:literal,
     [$qd:literal, $an:literal, $ns:literal, $ar:literal], $question:expr, $answer:literal, $lit:expr) => {
        header(
            &anatomy::DNS,
            &[
                $id, $qr, "0", $aa, "0", $rd, $ra, $z, $rcode, $qd, $an, $ns, $ar, $question,
                $answer,
            ],
            $lit,
        )
    };
    ($id:literal, $qr:literal, $aa:literal, $rd:literal, $ra:literal, $z:literal, $rcode:literal,
     [$qd:literal, $an:literal, $ns:literal, $ar:literal], $question:expr, $lit:expr) => {
        header(
            &anatomy::DNS,
            &[
                $id, $qr, "0", $aa, "0", $rd, $ra, $z, $rcode, $qd, $an, $ns, $ar, $question,
            ],
            $lit,
        )
    };
}

/// An IPv4 header carrying UDP: total length, identification, TTL,
/// checksum, source, destination.
macro_rules! ip {
    ($len:literal, $id:literal, $ttl:literal, $sum:literal, $src:literal, $dst:literal, $lit:expr) => {
        header(
            &anatomy::IPV4,
            &[
                "4", "5", "0", "0", $len, $id, "000", "0", $ttl, "17", $sum, $src, $dst,
            ],
            $lit,
        )
    };
}

static STEPS: [Step; 6] = [
    Step {
        title: k::PACKETS_DNSSEC_1_TITLE,
        sub: k::PACKETS_DNSSEC_1_SUB,
        body: k::PACKETS_DNSSEC_1_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_QUESTION, "www.example.com A"),
            wire(k::PACKETS_LABEL_FLAGS, "EDNS0 DO=1"),
            wire(k::PACKETS_LABEL_ASKS, "198.51.100.53"),
        ],
        focus: &[CLIENT, RESOLVER, AUTHORITATIVE],
        beats: &[
            Beat::Type {
                at: CLIENT,
                wire: "www.example.com",
            },
            Beat::Send(
                Packet::new(
                    CLIENT,
                    RESOLVER,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com",
                    "0x8a1c A? www.example.com OPT",
                    86,
                )
                .says(k::PACKETS_DNSSEC_P1)
                .headers(&[
                    ip!("72", "0x8c11", "64", "0x1cd2", "192.168.1.10", "8.8.8.8", &[]),
                    header(&anatomy::UDP, &["51000", "53", "52", "0x9c2b"], &[1]),
                    dns!("0x8a1c", "0", "0", "1", "0", "010", "0", ["1", "0", "0", "1"], WWW, &[5, 7]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    AUTHORITATIVE,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com DO",
                    "0x2b9e A? www.example.com OPT DO",
                    86,
                )
                .says(k::PACKETS_DNSSEC_P2)
                .headers(&[
                    ip!("72", "0x4e21", "64", "0xf20b", "8.8.8.8", "198.51.100.53", &[12]),
                    header(&anatomy::UDP, &["40512", "53", "52", "0x11f0"], &[]),
                    dns!("0x2b9e", "0", "0", "0", "0", "000", "0", ["1", "0", "0", "1"], WWW, &[12, 13]),
                ]),
            ),
        ],
    },
    Step {
        title: k::PACKETS_DNSSEC_2_TITLE,
        sub: k::PACKETS_DNSSEC_2_SUB,
        body: k::PACKETS_DNSSEC_2_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ANSWER, "A 203.0.113.10"),
            wire(k::PACKETS_LABEL_SIGNATURE, "RRSIG A 13 3 3600"),
            wire(k::PACKETS_LABEL_ALGORITHM, "13 ECDSAP256SHA256"),
        ],
        focus: &[RESOLVER, AUTHORITATIVE],
        beats: &[Beat::Send(
            Packet::new(
                AUTHORITATIVE,
                RESOLVER,
                Class::Response,
                "DNS",
                "A + RRSIG",
                "0x2b9e A www.example.com → 203.0.113.10 RRSIG OPT",
                209,
            )
            .says(k::PACKETS_DNSSEC_P3)
            .headers(&[dns!(
                "0x2b9e",
                "1",
                "1",
                "0",
                "0",
                "000",
                "0",
                ["1", "2", "0", "1"],
                WWW,
                "www.example.com  A  203.0.113.10  TTL 3600  ·  RRSIG A 13 3 3600 20261208000000 20261108000000 45230 example.com. kq3J…",
                &[3, 10, 14]
            )]),
        )],
    },
    Step {
        title: k::PACKETS_DNSSEC_3_TITLE,
        sub: k::PACKETS_DNSSEC_3_SUB,
        body: k::PACKETS_DNSSEC_3_BODY,
        facts: &[
            wire(k::PACKETS_DNSSEC_LABEL_KSK, "DNSKEY 257 3 13"),
            wire(k::PACKETS_DNSSEC_LABEL_ZSK, "DNSKEY 256 3 13"),
            wire(k::PACKETS_LABEL_SIGNATURE, "RRSIG DNSKEY 13 2 3600"),
        ],
        focus: &[RESOLVER, AUTHORITATIVE],
        beats: &[
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    AUTHORITATIVE,
                    Class::Lookup,
                    "DNS",
                    "DNSKEY? example.com",
                    "0x4c17 DNSKEY? example.com OPT DO",
                    82,
                )
                .says(k::PACKETS_DNSSEC_P4)
                .headers(&[dns!(
                    "0x4c17",
                    "0",
                    "0",
                    "0",
                    "0",
                    "000",
                    "0",
                    ["1", "0", "0", "1"],
                    "example.com  DNSKEY  IN",
                    &[13]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    AUTHORITATIVE,
                    RESOLVER,
                    Class::Crypto,
                    "DNS",
                    "DNSKEY ×2 + RRSIG",
                    "0x4c17 DNSKEY 257, DNSKEY 256, RRSIG OPT",
                    349,
                )
                .says(k::PACKETS_DNSSEC_P5)
                .headers(&[dns!(
                    "0x4c17",
                    "1",
                    "1",
                    "0",
                    "0",
                    "000",
                    "0",
                    ["1", "3", "0", "1"],
                    "example.com  DNSKEY  IN",
                    "example.com  DNSKEY 257 3 13 mdsswUyr…  ·  DNSKEY 256 3 13 oJMRESz5…  ·  RRSIG DNSKEY 13 2 3600 … 2371 example.com. …",
                    &[10, 14]
                )]),
            ),
            Beat::Light {
                at: RESOLVER,
                prop: Some(Prop::Key),
            },
        ],
    },
    Step {
        title: k::PACKETS_DNSSEC_4_TITLE,
        sub: k::PACKETS_DNSSEC_4_SUB,
        body: k::PACKETS_DNSSEC_4_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_RECORD, "example.com. DS 2371 13 2"),
            wire(k::PACKETS_LABEL_HASH, "SHA-256"),
            fact(k::PACKETS_LABEL_TRUST, k::PACKETS_DNSSEC_4_TRUST),
        ],
        focus: &[RESOLVER, ROOT, TLD],
        beats: &[
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    TLD,
                    Class::Lookup,
                    "DNS",
                    "DS? example.com",
                    "0x9d02 DS? example.com OPT DO",
                    82,
                )
                .says(k::PACKETS_DNSSEC_P6)
                .headers(&[dns!(
                    "0x9d02",
                    "0",
                    "0",
                    "0",
                    "0",
                    "000",
                    "0",
                    ["1", "0", "0", "1"],
                    "example.com  DS  IN",
                    &[13]
                )]),
            ),
            Beat::Send(
                Packet::new(
                    TLD,
                    RESOLVER,
                    Class::Crypto,
                    "DNS",
                    "DS + RRSIG",
                    "0x9d02 DS 2371 13 2, RRSIG OPT",
                    229,
                )
                .says(k::PACKETS_DNSSEC_P7)
                .headers(&[dns!(
                    "0x9d02",
                    "1",
                    "1",
                    "0",
                    "0",
                    "000",
                    "0",
                    ["1", "2", "0", "1"],
                    "example.com  DS  IN",
                    "example.com  DS  2371 13 2 5f1c…a7e3  TTL 86400  ·  RRSIG DS 13 2 86400 … 52842 com. …",
                    &[14]
                )]),
            ),
            Beat::Send(
                Packet::new(RESOLVER, ROOT, Class::Lookup, "DNS", "DS? com", "0x13f5 DS? com OPT DO", 74)
                    .says(k::PACKETS_DNSSEC_P8)
                    .headers(&[dns!(
                        "0x13f5",
                        "0",
                        "0",
                        "0",
                        "0",
                        "000",
                        "0",
                        ["1", "0", "0", "1"],
                        "com  DS  IN",
                        &[13]
                    )]),
            ),
            Beat::Send(
                Packet::new(
                    ROOT,
                    RESOLVER,
                    Class::Crypto,
                    "DNS",
                    "DS + RRSIG",
                    "0x13f5 DS 19718 13 2, RRSIG OPT",
                    409,
                )
                .says(k::PACKETS_DNSSEC_P9)
                .headers(&[dns!(
                    "0x13f5",
                    "1",
                    "1",
                    "0",
                    "0",
                    "000",
                    "0",
                    ["1", "2", "0", "1"],
                    "com  DS  IN",
                    "com.  DS  19718 13 2 8acbb0cd…  TTL 86400  ·  RRSIG DS 8 1 86400 … 61050 . …",
                    &[14]
                )]),
            ),
            Beat::Light {
                at: RESOLVER,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_DNSSEC_5_TITLE,
        sub: k::PACKETS_DNSSEC_5_SUB,
        body: k::PACKETS_DNSSEC_5_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ANSWER, "A 203.0.113.10"),
            wire(k::PACKETS_LABEL_FLAGS, "AD=1"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_DNSSEC_5_VERDICT),
        ],
        focus: &[CLIENT, RESOLVER],
        beats: &[
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    CLIENT,
                    Class::Response,
                    "DNS",
                    "A 203.0.113.10 AD",
                    "0x8a1c A www.example.com → 203.0.113.10 OPT",
                    102,
                )
                .says(k::PACKETS_DNSSEC_P10)
                .headers(&[
                    ip!("88", "0x31c5", "121", "0x3e0e", "8.8.8.8", "192.168.1.10", &[]),
                    header(&anatomy::UDP, &["53", "51000", "68", "0x5d17"], &[]),
                    dns!(
                        "0x8a1c",
                        "1",
                        "0",
                        "1",
                        "1",
                        "010",
                        "0",
                        ["1", "1", "0", "1"],
                        WWW,
                        "www.example.com  A  203.0.113.10  TTL 3600",
                        &[7, 14]
                    ),
                ]),
            ),
            Beat::Light {
                at: CLIENT,
                prop: Some(Prop::Check),
            },
        ],
    },
    Step {
        title: k::PACKETS_DNSSEC_6_TITLE,
        sub: k::PACKETS_DNSSEC_6_SUB,
        body: k::PACKETS_DNSSEC_6_BODY,
        facts: &[
            wire(k::PACKETS_LABEL_ANSWER, "A 192.0.2.66"),
            fact(k::PACKETS_LABEL_VERDICT, k::PACKETS_DNSSEC_6_VERDICT),
            wire(k::PACKETS_LABEL_ERROR, "SERVFAIL, EDE 6"),
        ],
        focus: &[CLIENT, RESOLVER, AUTHORITATIVE, ATTACKER],
        beats: &[
            Beat::Send(
                Packet::new(
                    CLIENT,
                    RESOLVER,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com",
                    "0x5d3a A? www.example.com OPT",
                    86,
                )
                .says(k::PACKETS_DNSSEC_P11)
                .headers(&[dns!("0x5d3a", "0", "0", "1", "0", "010", "0", ["1", "0", "0", "1"], WWW, &[])]),
            ),
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    AUTHORITATIVE,
                    Class::Lookup,
                    "DNS",
                    "A? www.example.com DO",
                    "0x6e41 A? www.example.com OPT DO",
                    86,
                )
                .says(k::PACKETS_DNSSEC_P12)
                .headers(&[
                    ip!("72", "0x4f90", "64", "0xf09c", "8.8.8.8", "198.51.100.53", &[]),
                    header(&anatomy::UDP, &["40517", "53", "52", "0x5e8a"], &[0]),
                    dns!("0x6e41", "0", "0", "0", "0", "000", "0", ["1", "0", "0", "1"], WWW, &[0]),
                ]),
            ),
            Beat::Send(
                Packet::new(
                    ATTACKER,
                    RESOLVER,
                    Class::Refusal,
                    "DNS",
                    "A 192.0.2.66",
                    "0x6e41 A www.example.com → 192.0.2.66",
                    91,
                )
                .says(k::PACKETS_DNSSEC_P13)
                .headers(&[
                    ip!("77", "0x0d6b", "62", "0x34bd", "198.51.100.53", "8.8.8.8", &[11]),
                    header(&anatomy::UDP, &["53", "40517", "57", "0x7c03"], &[1]),
                    dns!(
                        "0x6e41",
                        "1",
                        "1",
                        "0",
                        "0",
                        "000",
                        "0",
                        ["1", "1", "0", "0"],
                        WWW,
                        "www.example.com  A  192.0.2.66  TTL 86400",
                        &[0, 10, 14]
                    ),
                ]),
            ),
            Beat::Light {
                at: RESOLVER,
                prop: Some(Prop::Cross),
            },
            Beat::Send(
                Packet::new(
                    RESOLVER,
                    CLIENT,
                    Class::Refusal,
                    "DNS",
                    "SERVFAIL",
                    "0x5d3a SERVFAIL A? www.example.com OPT",
                    92,
                )
                .says(k::PACKETS_DNSSEC_P14)
                .headers(&[
                    ip!("78", "0x3302", "121", "0x3cdb", "8.8.8.8", "192.168.1.10", &[]),
                    header(&anatomy::UDP, &["53", "51000", "58", "0xe31a"], &[]),
                    dns!("0x5d3a", "1", "0", "1", "1", "000", "2", ["1", "0", "0", "1"], WWW, &[8, 10]),
                ]),
            ),
        ],
    },
];

/// DNSSEC.
pub static DNSSEC: Protocol = Protocol {
    id: "dnssec",
    group: Group::Naming,
    layer: Layer(7),
    name: k::PACKETS_DNSSEC_NAME,
    title: k::PACKETS_DNSSEC_TITLE,
    tagline: k::PACKETS_DNSSEC_TAGLINE,
    overview: k::PACKETS_DNSSEC_OVERVIEW,
    facts: &[
        wire(k::PACKETS_FACT_PORT, "53"),
        wire(k::PACKETS_FACT_RECORDS, "RRSIG, DNSKEY, DS, NSEC, NSEC3"),
        fact(k::PACKETS_FACT_SECURITY, k::PACKETS_DNSSEC_SECURITY),
        wire(k::PACKETS_FACT_DEFINED, "RFC 4033, RFC 4034, RFC 4035"),
    ],
    scene: Scene {
        devices: &DEVICES,
        links: &[
            link(CLIENT, RESOLVER, 8),
            link(RESOLVER, ROOT, 12),
            link(RESOLVER, TLD, 10),
            link(RESOLVER, AUTHORITATIVE, 14),
            link(ATTACKER, RESOLVER, 3),
        ],
    },
    steps: &STEPS,
};
