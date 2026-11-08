//! A PACKET'S HEADERS, bit by bit — what the manual's anatomy opens.
//!
//! A [`Layout`] is a header's shape: its fields, in order, each so many bits
//! wide, as the standard draws them — in rows of 32 bits, the width every RFC
//! draws a header in. A [`Header`] is one packet's: a layout, the values the
//! packet gives its fields (wire, invariant 8's exception of 2026-11-08) and
//! the fields its step talks about, which the window lights. A text protocol
//! (HTTP's request) is a layout of LINES instead of bits ([`Layout::text`]).
//!
//! [`rows`] lays a layout out in rows of [`ROW_BITS`]: a field wider than
//! what is left of a row goes on in the next one, and a field of any length
//! (`bits == 0`) takes rows of its own.

use crate::key;
use ronoctua_i18n::BookKey;

/// How many bits a row of a header holds.
pub const ROW_BITS: u16 = 32;

/// A header's shape.
#[derive(Debug)]
pub struct Layout {
    /// Its name: `IPv4`.
    pub name: BookKey,
    /// Its fields, in order.
    pub fields: &'static [Field],
    /// Lines of text, not bits: each field is a line.
    pub text: bool,
}

/// A field of a header.
#[derive(Debug)]
pub struct Field {
    /// How wide, in bits; 0 for a field of any length.
    pub bits: u16,
    /// Its name.
    pub name: BookKey,
    /// What it is for, in a sentence.
    pub about: BookKey,
}

/// One packet's header.
#[derive(Debug, Clone, Copy)]
pub struct Header {
    /// Its shape.
    pub layout: &'static Layout,
    /// The values of its fields, in the layout's order — wire; a field past
    /// the end shows none.
    pub values: &'static [&'static str],
    /// The fields its step talks about, by their places in the layout.
    pub lit: &'static [usize],
}

/// A header, with its values and what is lit.
#[must_use]
pub const fn header(
    layout: &'static Layout,
    values: &'static [&'static str],
    lit: &'static [usize],
) -> Header {
    Header {
        layout,
        values,
        lit,
    }
}

/// A piece of a field in a row: which field, where in the row it starts and
/// how many of the row's bits it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// The field, by its place in the layout.
    pub field: usize,
    /// Where it starts in the row, in bits.
    pub from: u16,
    /// How many bits of the row it takes ([`ROW_BITS`] for a field of any
    /// length).
    pub bits: u16,
    /// The piece that names the field — its first; the others go on with it.
    pub first: bool,
}

/// A layout in rows of [`ROW_BITS`]: each row, its pieces left to right. A
/// text layout is a row a line.
#[must_use]
pub fn rows(layout: &Layout) -> Vec<Vec<Piece>> {
    let mut rows: Vec<Vec<Piece>> = Vec::new();
    let mut row: Vec<Piece> = Vec::new();
    let mut used: u16 = 0;
    for (field, f) in layout.fields.iter().enumerate() {
        if layout.text || f.bits == 0 {
            if !row.is_empty() {
                rows.push(std::mem::take(&mut row));
                used = 0;
            }
            rows.push(vec![Piece {
                field,
                from: 0,
                bits: ROW_BITS,
                first: true,
            }]);
            continue;
        }
        let mut left = f.bits;
        let mut first = true;
        while left > 0 {
            let take = left.min(ROW_BITS - used);
            row.push(Piece {
                field,
                from: used,
                bits: take,
                first,
            });
            first = false;
            used += take;
            left -= take;
            if used == ROW_BITS {
                rows.push(std::mem::take(&mut row));
                used = 0;
            }
        }
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

/// A field.
const fn field(bits: u16, name: BookKey, about: BookKey) -> Field {
    Field { bits, name, about }
}

/// ETHERNET II: the frame on the wire of a local network.
pub static ETHERNET: Layout = Layout {
    name: key::PACKETS_HDR_ETHERNET,
    text: false,
    fields: &[
        field(48, key::PACKETS_F_ETH_DST, key::PACKETS_F_ETH_DST_ABOUT),
        field(48, key::PACKETS_F_ETH_SRC, key::PACKETS_F_ETH_SRC_ABOUT),
        field(16, key::PACKETS_F_ETH_TYPE, key::PACKETS_F_ETH_TYPE_ABOUT),
    ],
};

/// IPv4, RFC 791.
pub static IPV4: Layout = Layout {
    name: key::PACKETS_HDR_IPV4,
    text: false,
    fields: &[
        field(
            4,
            key::PACKETS_F_IP4_VERSION,
            key::PACKETS_F_IP4_VERSION_ABOUT,
        ),
        field(4, key::PACKETS_F_IP4_IHL, key::PACKETS_F_IP4_IHL_ABOUT),
        field(6, key::PACKETS_F_IP4_DSCP, key::PACKETS_F_IP4_DSCP_ABOUT),
        field(2, key::PACKETS_F_IP4_ECN, key::PACKETS_F_IP4_ECN_ABOUT),
        field(
            16,
            key::PACKETS_F_IP4_LENGTH,
            key::PACKETS_F_IP4_LENGTH_ABOUT,
        ),
        field(16, key::PACKETS_F_IP4_ID, key::PACKETS_F_IP4_ID_ABOUT),
        field(3, key::PACKETS_F_IP4_FLAGS, key::PACKETS_F_IP4_FLAGS_ABOUT),
        field(
            13,
            key::PACKETS_F_IP4_FRAGMENT,
            key::PACKETS_F_IP4_FRAGMENT_ABOUT,
        ),
        field(8, key::PACKETS_F_IP4_TTL, key::PACKETS_F_IP4_TTL_ABOUT),
        field(
            8,
            key::PACKETS_F_IP4_PROTOCOL,
            key::PACKETS_F_IP4_PROTOCOL_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_IP4_CHECKSUM,
            key::PACKETS_F_IP4_CHECKSUM_ABOUT,
        ),
        field(32, key::PACKETS_F_IP4_SRC, key::PACKETS_F_IP4_SRC_ABOUT),
        field(32, key::PACKETS_F_IP4_DST, key::PACKETS_F_IP4_DST_ABOUT),
    ],
};

/// UDP, RFC 768.
pub static UDP: Layout = Layout {
    name: key::PACKETS_HDR_UDP,
    text: false,
    fields: &[
        field(16, key::PACKETS_F_SRC_PORT, key::PACKETS_F_SRC_PORT_ABOUT),
        field(16, key::PACKETS_F_DST_PORT, key::PACKETS_F_DST_PORT_ABOUT),
        field(
            16,
            key::PACKETS_F_UDP_LENGTH,
            key::PACKETS_F_UDP_LENGTH_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_UDP_CHECKSUM,
            key::PACKETS_F_UDP_CHECKSUM_ABOUT,
        ),
    ],
};

/// TCP, RFC 9293 — its eight flags each a bit of their own.
pub static TCP: Layout = Layout {
    name: key::PACKETS_HDR_TCP,
    text: false,
    fields: &[
        field(16, key::PACKETS_F_SRC_PORT, key::PACKETS_F_SRC_PORT_ABOUT),
        field(16, key::PACKETS_F_DST_PORT, key::PACKETS_F_DST_PORT_ABOUT),
        field(32, key::PACKETS_F_TCP_SEQ, key::PACKETS_F_TCP_SEQ_ABOUT),
        field(32, key::PACKETS_F_TCP_ACK, key::PACKETS_F_TCP_ACK_ABOUT),
        field(
            4,
            key::PACKETS_F_TCP_OFFSET,
            key::PACKETS_F_TCP_OFFSET_ABOUT,
        ),
        field(
            4,
            key::PACKETS_F_TCP_RESERVED,
            key::PACKETS_F_TCP_RESERVED_ABOUT,
        ),
        field(1, key::PACKETS_F_TCP_CWR, key::PACKETS_F_TCP_CWR_ABOUT),
        field(1, key::PACKETS_F_TCP_ECE, key::PACKETS_F_TCP_ECE_ABOUT),
        field(1, key::PACKETS_F_TCP_URG, key::PACKETS_F_TCP_URG_ABOUT),
        field(1, key::PACKETS_F_TCP_ACKF, key::PACKETS_F_TCP_ACKF_ABOUT),
        field(1, key::PACKETS_F_TCP_PSH, key::PACKETS_F_TCP_PSH_ABOUT),
        field(1, key::PACKETS_F_TCP_RST, key::PACKETS_F_TCP_RST_ABOUT),
        field(1, key::PACKETS_F_TCP_SYN, key::PACKETS_F_TCP_SYN_ABOUT),
        field(1, key::PACKETS_F_TCP_FIN, key::PACKETS_F_TCP_FIN_ABOUT),
        field(
            16,
            key::PACKETS_F_TCP_WINDOW,
            key::PACKETS_F_TCP_WINDOW_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_TCP_CHECKSUM,
            key::PACKETS_F_TCP_CHECKSUM_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_TCP_URGENT,
            key::PACKETS_F_TCP_URGENT_ABOUT,
        ),
    ],
};

/// DNS, RFC 1035: the header, and the question and answers after it.
pub static DNS: Layout = Layout {
    name: key::PACKETS_HDR_DNS,
    text: false,
    fields: &[
        field(16, key::PACKETS_F_DNS_ID, key::PACKETS_F_DNS_ID_ABOUT),
        field(1, key::PACKETS_F_DNS_QR, key::PACKETS_F_DNS_QR_ABOUT),
        field(
            4,
            key::PACKETS_F_DNS_OPCODE,
            key::PACKETS_F_DNS_OPCODE_ABOUT,
        ),
        field(1, key::PACKETS_F_DNS_AA, key::PACKETS_F_DNS_AA_ABOUT),
        field(1, key::PACKETS_F_DNS_TC, key::PACKETS_F_DNS_TC_ABOUT),
        field(1, key::PACKETS_F_DNS_RD, key::PACKETS_F_DNS_RD_ABOUT),
        field(1, key::PACKETS_F_DNS_RA, key::PACKETS_F_DNS_RA_ABOUT),
        field(3, key::PACKETS_F_DNS_Z, key::PACKETS_F_DNS_Z_ABOUT),
        field(4, key::PACKETS_F_DNS_RCODE, key::PACKETS_F_DNS_RCODE_ABOUT),
        field(
            16,
            key::PACKETS_F_DNS_QDCOUNT,
            key::PACKETS_F_DNS_QDCOUNT_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_DNS_ANCOUNT,
            key::PACKETS_F_DNS_ANCOUNT_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_DNS_NSCOUNT,
            key::PACKETS_F_DNS_NSCOUNT_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_DNS_ARCOUNT,
            key::PACKETS_F_DNS_ARCOUNT_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_DNS_QUESTION,
            key::PACKETS_F_DNS_QUESTION_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_DNS_ANSWER,
            key::PACKETS_F_DNS_ANSWER_ABOUT,
        ),
    ],
};

/// AN HTTP/1.1 REQUEST, RFC 9112: its lines.
pub static HTTP_REQUEST: Layout = Layout {
    name: key::PACKETS_HDR_HTTP_REQUEST,
    text: true,
    fields: &[
        field(
            0,
            key::PACKETS_F_HTTP_REQUEST_LINE,
            key::PACKETS_F_HTTP_REQUEST_LINE_ABOUT,
        ),
        field(0, key::PACKETS_F_HTTP_HOST, key::PACKETS_F_HTTP_HOST_ABOUT),
        field(
            0,
            key::PACKETS_F_HTTP_AGENT,
            key::PACKETS_F_HTTP_AGENT_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTP_ACCEPT,
            key::PACKETS_F_HTTP_ACCEPT_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTP_BLANK,
            key::PACKETS_F_HTTP_BLANK_ABOUT,
        ),
    ],
};

/// AN HTTP/1.1 RESPONSE: its status line, headers and body.
pub static HTTP_RESPONSE: Layout = Layout {
    name: key::PACKETS_HDR_HTTP_RESPONSE,
    text: true,
    fields: &[
        field(
            0,
            key::PACKETS_F_HTTP_STATUS_LINE,
            key::PACKETS_F_HTTP_STATUS_LINE_ABOUT,
        ),
        field(0, key::PACKETS_F_HTTP_TYPE, key::PACKETS_F_HTTP_TYPE_ABOUT),
        field(
            0,
            key::PACKETS_F_HTTP_LENGTH,
            key::PACKETS_F_HTTP_LENGTH_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTP_BLANK,
            key::PACKETS_F_HTTP_BLANK_ABOUT,
        ),
        field(0, key::PACKETS_F_HTTP_BODY, key::PACKETS_F_HTTP_BODY_ABOUT),
    ],
};

/// ANY HTTP/1.1 MESSAGE, RFC 9112: a start line, headers, a blank line, a body.
pub static HTTP_MESSAGE: Layout = Layout {
    name: key::PACKETS_HDR_HTTP_MESSAGE,
    text: true,
    fields: &[
        field(
            0,
            key::PACKETS_F_HTTPM_START,
            key::PACKETS_F_HTTPM_START_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTPM_HEADER,
            key::PACKETS_F_HTTPM_HEADER_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTPM_HEADER,
            key::PACKETS_F_HTTPM_HEADER_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTPM_HEADER,
            key::PACKETS_F_HTTPM_HEADER_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTP_BLANK,
            key::PACKETS_F_HTTP_BLANK_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_HTTPM_BODY,
            key::PACKETS_F_HTTPM_BODY_ABOUT,
        ),
    ],
};

/// ONE LINE of a text protocol — SMTP (RFC 5321), POP3 (RFC 1939), IMAP
/// (RFC 9051), FTP (RFC 959): a command or a reply.
pub static LINE: Layout = Layout {
    name: key::PACKETS_HDR_LINE,
    text: true,
    fields: &[field(
        0,
        key::PACKETS_F_LINE_LINE,
        key::PACKETS_F_LINE_LINE_ABOUT,
    )],
};

/// ARP, RFC 826, for IPv4 over Ethernet.
pub static ARP: Layout = Layout {
    name: key::PACKETS_HDR_ARP,
    text: false,
    fields: &[
        field(16, key::PACKETS_F_ARP_HTYPE, key::PACKETS_F_ARP_HTYPE_ABOUT),
        field(16, key::PACKETS_F_ARP_PTYPE, key::PACKETS_F_ARP_PTYPE_ABOUT),
        field(8, key::PACKETS_F_ARP_HLEN, key::PACKETS_F_ARP_HLEN_ABOUT),
        field(8, key::PACKETS_F_ARP_PLEN, key::PACKETS_F_ARP_PLEN_ABOUT),
        field(
            16,
            key::PACKETS_F_ARP_OPCODE,
            key::PACKETS_F_ARP_OPCODE_ABOUT,
        ),
        field(48, key::PACKETS_F_ARP_SHA, key::PACKETS_F_ARP_SHA_ABOUT),
        field(32, key::PACKETS_F_ARP_SPA, key::PACKETS_F_ARP_SPA_ABOUT),
        field(48, key::PACKETS_F_ARP_THA, key::PACKETS_F_ARP_THA_ABOUT),
        field(32, key::PACKETS_F_ARP_TPA, key::PACKETS_F_ARP_TPA_ABOUT),
    ],
};

/// IPv6, RFC 8200: the fixed 40-byte header.
pub static IPV6: Layout = Layout {
    name: key::PACKETS_HDR_IPV6,
    text: false,
    fields: &[
        field(
            4,
            key::PACKETS_F_IP6_VERSION,
            key::PACKETS_F_IP6_VERSION_ABOUT,
        ),
        field(8, key::PACKETS_F_IP6_CLASS, key::PACKETS_F_IP6_CLASS_ABOUT),
        field(20, key::PACKETS_F_IP6_FLOW, key::PACKETS_F_IP6_FLOW_ABOUT),
        field(
            16,
            key::PACKETS_F_IP6_LENGTH,
            key::PACKETS_F_IP6_LENGTH_ABOUT,
        ),
        field(8, key::PACKETS_F_IP6_NEXT, key::PACKETS_F_IP6_NEXT_ABOUT),
        field(8, key::PACKETS_F_IP6_HOPS, key::PACKETS_F_IP6_HOPS_ABOUT),
        field(128, key::PACKETS_F_IP6_SRC, key::PACKETS_F_IP6_SRC_ABOUT),
        field(128, key::PACKETS_F_IP6_DST, key::PACKETS_F_IP6_DST_ABOUT),
    ],
};

/// ICMP, RFC 792 (ICMPv6, RFC 4443): an echo's header and its data.
pub static ICMP: Layout = Layout {
    name: key::PACKETS_HDR_ICMP,
    text: false,
    fields: &[
        field(8, key::PACKETS_F_ICMP_TYPE, key::PACKETS_F_ICMP_TYPE_ABOUT),
        field(8, key::PACKETS_F_ICMP_CODE, key::PACKETS_F_ICMP_CODE_ABOUT),
        field(
            16,
            key::PACKETS_F_ICMP_CHECKSUM,
            key::PACKETS_F_ICMP_CHECKSUM_ABOUT,
        ),
        field(16, key::PACKETS_F_ICMP_ID, key::PACKETS_F_ICMP_ID_ABOUT),
        field(16, key::PACKETS_F_ICMP_SEQ, key::PACKETS_F_ICMP_SEQ_ABOUT),
        field(0, key::PACKETS_F_ICMP_DATA, key::PACKETS_F_ICMP_DATA_ABOUT),
    ],
};

/// NDP, RFC 4861: a neighbor solicitation or advertisement.
pub static NDP: Layout = Layout {
    name: key::PACKETS_HDR_NDP,
    text: false,
    fields: &[
        field(8, key::PACKETS_F_NDP_TYPE, key::PACKETS_F_NDP_TYPE_ABOUT),
        field(8, key::PACKETS_F_NDP_CODE, key::PACKETS_F_NDP_CODE_ABOUT),
        field(
            16,
            key::PACKETS_F_NDP_CHECKSUM,
            key::PACKETS_F_NDP_CHECKSUM_ABOUT,
        ),
        field(32, key::PACKETS_F_NDP_FLAGS, key::PACKETS_F_NDP_FLAGS_ABOUT),
        field(
            128,
            key::PACKETS_F_NDP_TARGET,
            key::PACKETS_F_NDP_TARGET_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_NDP_OPTIONS,
            key::PACKETS_F_NDP_OPTIONS_ABOUT,
        ),
    ],
};

/// A TLS 1.3 RECORD, RFC 8446, carrying a handshake message.
pub static TLS: Layout = Layout {
    name: key::PACKETS_HDR_TLS,
    text: false,
    fields: &[
        field(8, key::PACKETS_F_TLS_TYPE, key::PACKETS_F_TLS_TYPE_ABOUT),
        field(
            16,
            key::PACKETS_F_TLS_VERSION,
            key::PACKETS_F_TLS_VERSION_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_TLS_LENGTH,
            key::PACKETS_F_TLS_LENGTH_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_TLS_HS_TYPE,
            key::PACKETS_F_TLS_HS_TYPE_ABOUT,
        ),
        field(
            24,
            key::PACKETS_F_TLS_HS_LENGTH,
            key::PACKETS_F_TLS_HS_LENGTH_ABOUT,
        ),
        field(0, key::PACKETS_F_TLS_BODY, key::PACKETS_F_TLS_BODY_ABOUT),
    ],
};

/// A TLS 1.3 KEY SHARE, RFC 8446 §4.2.8, for the hybrid X25519MLKEM768
/// (draft-ietf-tls-ecdhe-mlkem): the ML-KEM half first.
pub static KEY_SHARE: Layout = Layout {
    name: key::PACKETS_HDR_KEY_SHARE,
    text: false,
    fields: &[
        field(16, key::PACKETS_F_KS_GROUP, key::PACKETS_F_KS_GROUP_ABOUT),
        field(16, key::PACKETS_F_KS_LENGTH, key::PACKETS_F_KS_LENGTH_ABOUT),
        field(0, key::PACKETS_F_KS_MLKEM, key::PACKETS_F_KS_MLKEM_ABOUT),
        field(
            256,
            key::PACKETS_F_KS_X25519,
            key::PACKETS_F_KS_X25519_ABOUT,
        ),
    ],
};

/// QUIC'S LONG HEADER, RFC 9000 §17.2: the packets that set a connection up.
pub static QUIC_LONG: Layout = Layout {
    name: key::PACKETS_HDR_QUIC_LONG,
    text: false,
    fields: &[
        field(1, key::PACKETS_F_QL_FORM, key::PACKETS_F_QL_FORM_ABOUT),
        field(1, key::PACKETS_F_QL_FIXED, key::PACKETS_F_QL_FIXED_ABOUT),
        field(2, key::PACKETS_F_QL_TYPE, key::PACKETS_F_QL_TYPE_ABOUT),
        field(
            2,
            key::PACKETS_F_QL_RESERVED,
            key::PACKETS_F_QL_RESERVED_ABOUT,
        ),
        field(2, key::PACKETS_F_QL_PNLEN, key::PACKETS_F_QL_PNLEN_ABOUT),
        field(
            32,
            key::PACKETS_F_QL_VERSION,
            key::PACKETS_F_QL_VERSION_ABOUT,
        ),
        field(8, key::PACKETS_F_QL_DCIL, key::PACKETS_F_QL_DCIL_ABOUT),
        field(64, key::PACKETS_F_QL_DCID, key::PACKETS_F_QL_DCID_ABOUT),
        field(8, key::PACKETS_F_QL_SCIL, key::PACKETS_F_QL_SCIL_ABOUT),
        field(64, key::PACKETS_F_QL_SCID, key::PACKETS_F_QL_SCID_ABOUT),
        field(16, key::PACKETS_F_QL_LENGTH, key::PACKETS_F_QL_LENGTH_ABOUT),
        field(32, key::PACKETS_F_QL_PN, key::PACKETS_F_QL_PN_ABOUT),
        field(
            0,
            key::PACKETS_F_QL_PAYLOAD,
            key::PACKETS_F_QL_PAYLOAD_ABOUT,
        ),
    ],
};

/// QUIC'S INITIAL PACKET, RFC 9000 §17.2.2: the long header with a token's
/// length after the source ID — a token of length 0 needs no token field.
pub static QUIC_INITIAL: Layout = Layout {
    name: key::PACKETS_HDR_QUIC_INITIAL,
    text: false,
    fields: &[
        field(1, key::PACKETS_F_QL_FORM, key::PACKETS_F_QL_FORM_ABOUT),
        field(1, key::PACKETS_F_QL_FIXED, key::PACKETS_F_QL_FIXED_ABOUT),
        field(2, key::PACKETS_F_QL_TYPE, key::PACKETS_F_QL_TYPE_ABOUT),
        field(
            2,
            key::PACKETS_F_QL_RESERVED,
            key::PACKETS_F_QL_RESERVED_ABOUT,
        ),
        field(2, key::PACKETS_F_QL_PNLEN, key::PACKETS_F_QL_PNLEN_ABOUT),
        field(
            32,
            key::PACKETS_F_QL_VERSION,
            key::PACKETS_F_QL_VERSION_ABOUT,
        ),
        field(8, key::PACKETS_F_QL_DCIL, key::PACKETS_F_QL_DCIL_ABOUT),
        field(64, key::PACKETS_F_QL_DCID, key::PACKETS_F_QL_DCID_ABOUT),
        field(8, key::PACKETS_F_QL_SCIL, key::PACKETS_F_QL_SCIL_ABOUT),
        field(64, key::PACKETS_F_QL_SCID, key::PACKETS_F_QL_SCID_ABOUT),
        field(
            8,
            key::PACKETS_F_QL_TOKEN_LENGTH,
            key::PACKETS_F_QL_TOKEN_LENGTH_ABOUT,
        ),
        field(16, key::PACKETS_F_QL_LENGTH, key::PACKETS_F_QL_LENGTH_ABOUT),
        field(32, key::PACKETS_F_QL_PN, key::PACKETS_F_QL_PN_ABOUT),
        field(
            0,
            key::PACKETS_F_QL_PAYLOAD,
            key::PACKETS_F_QL_PAYLOAD_ABOUT,
        ),
    ],
};

/// QUIC'S SHORT HEADER, RFC 9000 §17.3: every packet after the handshake.
pub static QUIC_SHORT: Layout = Layout {
    name: key::PACKETS_HDR_QUIC_SHORT,
    text: false,
    fields: &[
        field(1, key::PACKETS_F_QS_FORM, key::PACKETS_F_QS_FORM_ABOUT),
        field(1, key::PACKETS_F_QS_FIXED, key::PACKETS_F_QS_FIXED_ABOUT),
        field(1, key::PACKETS_F_QS_SPIN, key::PACKETS_F_QS_SPIN_ABOUT),
        field(
            2,
            key::PACKETS_F_QS_RESERVED,
            key::PACKETS_F_QS_RESERVED_ABOUT,
        ),
        field(1, key::PACKETS_F_QS_PHASE, key::PACKETS_F_QS_PHASE_ABOUT),
        field(2, key::PACKETS_F_QS_PNLEN, key::PACKETS_F_QS_PNLEN_ABOUT),
        field(64, key::PACKETS_F_QS_DCID, key::PACKETS_F_QS_DCID_ABOUT),
        field(32, key::PACKETS_F_QS_PN, key::PACKETS_F_QS_PN_ABOUT),
        field(
            0,
            key::PACKETS_F_QS_PAYLOAD,
            key::PACKETS_F_QS_PAYLOAD_ABOUT,
        ),
    ],
};

/// WIREGUARD'S HANDSHAKE INITIATION, from its whitepaper (Donenfeld, NDSS
/// 2017): 148 bytes.
pub static WIREGUARD_INIT: Layout = Layout {
    name: key::PACKETS_HDR_WIREGUARD_INIT,
    text: false,
    fields: &[
        field(8, key::PACKETS_F_WGI_TYPE, key::PACKETS_F_WGI_TYPE_ABOUT),
        field(
            24,
            key::PACKETS_F_WGI_RESERVED,
            key::PACKETS_F_WGI_RESERVED_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_WGI_SENDER,
            key::PACKETS_F_WGI_SENDER_ABOUT,
        ),
        field(
            256,
            key::PACKETS_F_WGI_EPHEMERAL,
            key::PACKETS_F_WGI_EPHEMERAL_ABOUT,
        ),
        field(
            384,
            key::PACKETS_F_WGI_STATIC,
            key::PACKETS_F_WGI_STATIC_ABOUT,
        ),
        field(
            224,
            key::PACKETS_F_WGI_TIMESTAMP,
            key::PACKETS_F_WGI_TIMESTAMP_ABOUT,
        ),
        field(128, key::PACKETS_F_WGI_MAC1, key::PACKETS_F_WGI_MAC1_ABOUT),
        field(128, key::PACKETS_F_WGI_MAC2, key::PACKETS_F_WGI_MAC2_ABOUT),
    ],
};

/// WIREGUARD'S TRANSPORT DATA, from the same whitepaper: a packet, encrypted.
pub static WIREGUARD_DATA: Layout = Layout {
    name: key::PACKETS_HDR_WIREGUARD_DATA,
    text: false,
    fields: &[
        field(8, key::PACKETS_F_WGD_TYPE, key::PACKETS_F_WGD_TYPE_ABOUT),
        field(
            24,
            key::PACKETS_F_WGD_RESERVED,
            key::PACKETS_F_WGD_RESERVED_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_WGD_RECEIVER,
            key::PACKETS_F_WGD_RECEIVER_ABOUT,
        ),
        field(
            64,
            key::PACKETS_F_WGD_COUNTER,
            key::PACKETS_F_WGD_COUNTER_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_WGD_PACKET,
            key::PACKETS_F_WGD_PACKET_ABOUT,
        ),
    ],
};

/// IPSEC'S ESP, RFC 4303: the header, the encrypted payload and its trailer.
pub static ESP: Layout = Layout {
    name: key::PACKETS_HDR_ESP,
    text: false,
    fields: &[
        field(32, key::PACKETS_F_ESP_SPI, key::PACKETS_F_ESP_SPI_ABOUT),
        field(32, key::PACKETS_F_ESP_SEQ, key::PACKETS_F_ESP_SEQ_ABOUT),
        field(
            0,
            key::PACKETS_F_ESP_PAYLOAD,
            key::PACKETS_F_ESP_PAYLOAD_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_ESP_PADLEN,
            key::PACKETS_F_ESP_PADLEN_ABOUT,
        ),
        field(8, key::PACKETS_F_ESP_NEXT, key::PACKETS_F_ESP_NEXT_ABOUT),
        field(128, key::PACKETS_F_ESP_ICV, key::PACKETS_F_ESP_ICV_ABOUT),
    ],
};

/// IKEv2, RFC 7296: the header and its chain of payloads.
pub static IKE: Layout = Layout {
    name: key::PACKETS_HDR_IKE,
    text: false,
    fields: &[
        field(64, key::PACKETS_F_IKE_ISPI, key::PACKETS_F_IKE_ISPI_ABOUT),
        field(64, key::PACKETS_F_IKE_RSPI, key::PACKETS_F_IKE_RSPI_ABOUT),
        field(8, key::PACKETS_F_IKE_NEXT, key::PACKETS_F_IKE_NEXT_ABOUT),
        field(
            8,
            key::PACKETS_F_IKE_VERSION,
            key::PACKETS_F_IKE_VERSION_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_IKE_EXCHANGE,
            key::PACKETS_F_IKE_EXCHANGE_ABOUT,
        ),
        field(8, key::PACKETS_F_IKE_FLAGS, key::PACKETS_F_IKE_FLAGS_ABOUT),
        field(32, key::PACKETS_F_IKE_MSGID, key::PACKETS_F_IKE_MSGID_ABOUT),
        field(
            32,
            key::PACKETS_F_IKE_LENGTH,
            key::PACKETS_F_IKE_LENGTH_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_IKE_PAYLOADS,
            key::PACKETS_F_IKE_PAYLOADS_ABOUT,
        ),
    ],
};

/// NTPv4, RFC 5905: the 48 bytes of a client's or a server's packet.
pub static NTP: Layout = Layout {
    name: key::PACKETS_HDR_NTP,
    text: false,
    fields: &[
        field(2, key::PACKETS_F_NTP_LI, key::PACKETS_F_NTP_LI_ABOUT),
        field(3, key::PACKETS_F_NTP_VN, key::PACKETS_F_NTP_VN_ABOUT),
        field(3, key::PACKETS_F_NTP_MODE, key::PACKETS_F_NTP_MODE_ABOUT),
        field(
            8,
            key::PACKETS_F_NTP_STRATUM,
            key::PACKETS_F_NTP_STRATUM_ABOUT,
        ),
        field(8, key::PACKETS_F_NTP_POLL, key::PACKETS_F_NTP_POLL_ABOUT),
        field(
            8,
            key::PACKETS_F_NTP_PRECISION,
            key::PACKETS_F_NTP_PRECISION_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_NTP_ROOTDELAY,
            key::PACKETS_F_NTP_ROOTDELAY_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_NTP_ROOTDISP,
            key::PACKETS_F_NTP_ROOTDISP_ABOUT,
        ),
        field(32, key::PACKETS_F_NTP_REFID, key::PACKETS_F_NTP_REFID_ABOUT),
        field(64, key::PACKETS_F_NTP_REFTS, key::PACKETS_F_NTP_REFTS_ABOUT),
        field(
            64,
            key::PACKETS_F_NTP_ORIGIN,
            key::PACKETS_F_NTP_ORIGIN_ABOUT,
        ),
        field(
            64,
            key::PACKETS_F_NTP_RECEIVE,
            key::PACKETS_F_NTP_RECEIVE_ABOUT,
        ),
        field(
            64,
            key::PACKETS_F_NTP_TRANSMIT,
            key::PACKETS_F_NTP_TRANSMIT_ABOUT,
        ),
    ],
};

/// DHCP, RFC 2131 — the server name and boot file fields left out.
pub static DHCP: Layout = Layout {
    name: key::PACKETS_HDR_DHCP,
    text: false,
    fields: &[
        field(8, key::PACKETS_F_DHCP_OP, key::PACKETS_F_DHCP_OP_ABOUT),
        field(
            8,
            key::PACKETS_F_DHCP_HTYPE,
            key::PACKETS_F_DHCP_HTYPE_ABOUT,
        ),
        field(8, key::PACKETS_F_DHCP_HLEN, key::PACKETS_F_DHCP_HLEN_ABOUT),
        field(8, key::PACKETS_F_DHCP_HOPS, key::PACKETS_F_DHCP_HOPS_ABOUT),
        field(32, key::PACKETS_F_DHCP_XID, key::PACKETS_F_DHCP_XID_ABOUT),
        field(16, key::PACKETS_F_DHCP_SECS, key::PACKETS_F_DHCP_SECS_ABOUT),
        field(
            16,
            key::PACKETS_F_DHCP_FLAGS,
            key::PACKETS_F_DHCP_FLAGS_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_DHCP_CIADDR,
            key::PACKETS_F_DHCP_CIADDR_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_DHCP_YIADDR,
            key::PACKETS_F_DHCP_YIADDR_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_DHCP_SIADDR,
            key::PACKETS_F_DHCP_SIADDR_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_DHCP_GIADDR,
            key::PACKETS_F_DHCP_GIADDR_ABOUT,
        ),
        field(
            128,
            key::PACKETS_F_DHCP_CHADDR,
            key::PACKETS_F_DHCP_CHADDR_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_DHCP_OPTIONS,
            key::PACKETS_F_DHCP_OPTIONS_ABOUT,
        ),
    ],
};

/// MQTT, OASIS MQTT Version 5.0: the fixed header and what follows it.
pub static MQTT: Layout = Layout {
    name: key::PACKETS_HDR_MQTT,
    text: false,
    fields: &[
        field(4, key::PACKETS_F_MQTT_TYPE, key::PACKETS_F_MQTT_TYPE_ABOUT),
        field(
            4,
            key::PACKETS_F_MQTT_FLAGS,
            key::PACKETS_F_MQTT_FLAGS_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_MQTT_REMAINING,
            key::PACKETS_F_MQTT_REMAINING_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_MQTT_VARIABLE,
            key::PACKETS_F_MQTT_VARIABLE_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_MQTT_PAYLOAD,
            key::PACKETS_F_MQTT_PAYLOAD_ABOUT,
        ),
    ],
};

/// MODBUS TCP's MBAP header and PDU, from the Modbus Organization's
/// "Modbus Messaging on TCP/IP Implementation Guide" V1.0b.
pub static MBAP: Layout = Layout {
    name: key::PACKETS_HDR_MBAP,
    text: false,
    fields: &[
        field(
            16,
            key::PACKETS_F_MBAP_TRANSACTION,
            key::PACKETS_F_MBAP_TRANSACTION_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_MBAP_PROTOCOL,
            key::PACKETS_F_MBAP_PROTOCOL_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_MBAP_LENGTH,
            key::PACKETS_F_MBAP_LENGTH_ABOUT,
        ),
        field(8, key::PACKETS_F_MBAP_UNIT, key::PACKETS_F_MBAP_UNIT_ABOUT),
        field(
            8,
            key::PACKETS_F_MBAP_FUNCTION,
            key::PACKETS_F_MBAP_FUNCTION_ABOUT,
        ),
        field(0, key::PACKETS_F_MBAP_DATA, key::PACKETS_F_MBAP_DATA_ABOUT),
    ],
};

/// A WEBSOCKET FRAME, RFC 6455, as a client sends it: masked.
pub static WEBSOCKET: Layout = Layout {
    name: key::PACKETS_HDR_WEBSOCKET,
    text: false,
    fields: &[
        field(1, key::PACKETS_F_WS_FIN, key::PACKETS_F_WS_FIN_ABOUT),
        field(3, key::PACKETS_F_WS_RSV, key::PACKETS_F_WS_RSV_ABOUT),
        field(4, key::PACKETS_F_WS_OPCODE, key::PACKETS_F_WS_OPCODE_ABOUT),
        field(1, key::PACKETS_F_WS_MASK, key::PACKETS_F_WS_MASK_ABOUT),
        field(7, key::PACKETS_F_WS_LENGTH, key::PACKETS_F_WS_LENGTH_ABOUT),
        field(32, key::PACKETS_F_WS_KEY, key::PACKETS_F_WS_KEY_ABOUT),
        field(
            0,
            key::PACKETS_F_WS_PAYLOAD,
            key::PACKETS_F_WS_PAYLOAD_ABOUT,
        ),
    ],
};

/// BITTORRENT'S HANDSHAKE, BEP 3: 68 bytes.
pub static BITTORRENT: Layout = Layout {
    name: key::PACKETS_HDR_BITTORRENT,
    text: false,
    fields: &[
        field(
            8,
            key::PACKETS_F_BT_PSTRLEN,
            key::PACKETS_F_BT_PSTRLEN_ABOUT,
        ),
        field(152, key::PACKETS_F_BT_PSTR, key::PACKETS_F_BT_PSTR_ABOUT),
        field(
            64,
            key::PACKETS_F_BT_RESERVED,
            key::PACKETS_F_BT_RESERVED_ABOUT,
        ),
        field(160, key::PACKETS_F_BT_HASH, key::PACKETS_F_BT_HASH_ABOUT),
        field(160, key::PACKETS_F_BT_PEER, key::PACKETS_F_BT_PEER_ABOUT),
    ],
};

/// AN 802.11 FRAME'S MAC HEADER, IEEE 802.11-2020: the Wi-Fi frame.
pub static WIFI: Layout = Layout {
    name: key::PACKETS_HDR_WIFI,
    text: false,
    fields: &[
        field(16, key::PACKETS_F_WIFI_FC, key::PACKETS_F_WIFI_FC_ABOUT),
        field(
            16,
            key::PACKETS_F_WIFI_DURATION,
            key::PACKETS_F_WIFI_DURATION_ABOUT,
        ),
        field(
            48,
            key::PACKETS_F_WIFI_ADDR1,
            key::PACKETS_F_WIFI_ADDR1_ABOUT,
        ),
        field(
            48,
            key::PACKETS_F_WIFI_ADDR2,
            key::PACKETS_F_WIFI_ADDR2_ABOUT,
        ),
        field(
            48,
            key::PACKETS_F_WIFI_ADDR3,
            key::PACKETS_F_WIFI_ADDR3_ABOUT,
        ),
        field(16, key::PACKETS_F_WIFI_SEQ, key::PACKETS_F_WIFI_SEQ_ABOUT),
    ],
};

/// AN EAPOL-KEY FRAME, IEEE 802.11-2020 §12.7.2: a message of the four-way
/// handshake (the key IV, RSC and data length left out).
pub static EAPOL: Layout = Layout {
    name: key::PACKETS_HDR_EAPOL,
    text: false,
    fields: &[
        field(
            8,
            key::PACKETS_F_EAPOL_VERSION,
            key::PACKETS_F_EAPOL_VERSION_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_EAPOL_TYPE,
            key::PACKETS_F_EAPOL_TYPE_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_EAPOL_LENGTH,
            key::PACKETS_F_EAPOL_LENGTH_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_EAPOL_DESCRIPTOR,
            key::PACKETS_F_EAPOL_DESCRIPTOR_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_EAPOL_INFO,
            key::PACKETS_F_EAPOL_INFO_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_EAPOL_KEYLEN,
            key::PACKETS_F_EAPOL_KEYLEN_ABOUT,
        ),
        field(
            64,
            key::PACKETS_F_EAPOL_REPLAY,
            key::PACKETS_F_EAPOL_REPLAY_ABOUT,
        ),
        field(
            256,
            key::PACKETS_F_EAPOL_NONCE,
            key::PACKETS_F_EAPOL_NONCE_ABOUT,
        ),
        field(
            128,
            key::PACKETS_F_EAPOL_MIC,
            key::PACKETS_F_EAPOL_MIC_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_EAPOL_DATA,
            key::PACKETS_F_EAPOL_DATA_ABOUT,
        ),
    ],
};

/// WPA3'S SAE COMMIT, IEEE 802.11-2020 §12.4 (Dragonfly, RFC 7664), on the
/// curve P-256.
pub static SAE: Layout = Layout {
    name: key::PACKETS_HDR_SAE,
    text: false,
    fields: &[
        field(
            16,
            key::PACKETS_F_SAE_ALGORITHM,
            key::PACKETS_F_SAE_ALGORITHM_ABOUT,
        ),
        field(16, key::PACKETS_F_SAE_SEQ, key::PACKETS_F_SAE_SEQ_ABOUT),
        field(
            16,
            key::PACKETS_F_SAE_STATUS,
            key::PACKETS_F_SAE_STATUS_ABOUT,
        ),
        field(16, key::PACKETS_F_SAE_GROUP, key::PACKETS_F_SAE_GROUP_ABOUT),
        field(
            256,
            key::PACKETS_F_SAE_SCALAR,
            key::PACKETS_F_SAE_SCALAR_ABOUT,
        ),
        field(
            512,
            key::PACKETS_F_SAE_ELEMENT,
            key::PACKETS_F_SAE_ELEMENT_ABOUT,
        ),
    ],
};

/// RADIUS, RFC 2865.
pub static RADIUS: Layout = Layout {
    name: key::PACKETS_HDR_RADIUS,
    text: false,
    fields: &[
        field(
            8,
            key::PACKETS_F_RADIUS_CODE,
            key::PACKETS_F_RADIUS_CODE_ABOUT,
        ),
        field(8, key::PACKETS_F_RADIUS_ID, key::PACKETS_F_RADIUS_ID_ABOUT),
        field(
            16,
            key::PACKETS_F_RADIUS_LENGTH,
            key::PACKETS_F_RADIUS_LENGTH_ABOUT,
        ),
        field(
            128,
            key::PACKETS_F_RADIUS_AUTH,
            key::PACKETS_F_RADIUS_AUTH_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_RADIUS_ATTRS,
            key::PACKETS_F_RADIUS_ATTRS_ABOUT,
        ),
    ],
};

/// A SOCKS5 REQUEST, RFC 1928 — and its reply, which has the same shape.
pub static SOCKS5: Layout = Layout {
    name: key::PACKETS_HDR_SOCKS5,
    text: false,
    fields: &[
        field(
            8,
            key::PACKETS_F_SOCKS_VERSION,
            key::PACKETS_F_SOCKS_VERSION_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_SOCKS_COMMAND,
            key::PACKETS_F_SOCKS_COMMAND_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_SOCKS_RESERVED,
            key::PACKETS_F_SOCKS_RESERVED_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_SOCKS_ATYP,
            key::PACKETS_F_SOCKS_ATYP_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_SOCKS_ADDRESS,
            key::PACKETS_F_SOCKS_ADDRESS_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_SOCKS_PORT,
            key::PACKETS_F_SOCKS_PORT_ABOUT,
        ),
    ],
};

/// BGP-4, RFC 4271: the header and the message after it.
pub static BGP: Layout = Layout {
    name: key::PACKETS_HDR_BGP,
    text: false,
    fields: &[
        field(
            128,
            key::PACKETS_F_BGP_MARKER,
            key::PACKETS_F_BGP_MARKER_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_BGP_LENGTH,
            key::PACKETS_F_BGP_LENGTH_ABOUT,
        ),
        field(8, key::PACKETS_F_BGP_TYPE, key::PACKETS_F_BGP_TYPE_ABOUT),
        field(
            0,
            key::PACKETS_F_BGP_MESSAGE,
            key::PACKETS_F_BGP_MESSAGE_ABOUT,
        ),
    ],
};

/// A TOR CELL, Tor's specification (tor-spec, link protocol 4 and later):
/// 514 bytes, always.
pub static TOR_CELL: Layout = Layout {
    name: key::PACKETS_HDR_TOR_CELL,
    text: false,
    fields: &[
        field(
            32,
            key::PACKETS_F_TOR_CIRCUIT,
            key::PACKETS_F_TOR_CIRCUIT_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_TOR_COMMAND,
            key::PACKETS_F_TOR_COMMAND_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_TOR_PAYLOAD,
            key::PACKETS_F_TOR_PAYLOAD_ABOUT,
        ),
    ],
};

/// SSH'S BINARY PACKET, RFC 4253 §6.
pub static SSH: Layout = Layout {
    name: key::PACKETS_HDR_SSH,
    text: false,
    fields: &[
        field(
            32,
            key::PACKETS_F_SSH_LENGTH,
            key::PACKETS_F_SSH_LENGTH_ABOUT,
        ),
        field(
            8,
            key::PACKETS_F_SSH_PADLEN,
            key::PACKETS_F_SSH_PADLEN_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_SSH_PAYLOAD,
            key::PACKETS_F_SSH_PAYLOAD_ABOUT,
        ),
        field(
            0,
            key::PACKETS_F_SSH_PADDING,
            key::PACKETS_F_SSH_PADDING_ABOUT,
        ),
        field(0, key::PACKETS_F_SSH_MAC, key::PACKETS_F_SSH_MAC_ABOUT),
    ],
};

/// THE SMB2 HEADER, Microsoft's MS-SMB2 §2.2.1.2: 64 bytes, for SMB 2 and 3.
pub static SMB2: Layout = Layout {
    name: key::PACKETS_HDR_SMB2,
    text: false,
    fields: &[
        field(
            32,
            key::PACKETS_F_SMB_PROTOCOL,
            key::PACKETS_F_SMB_PROTOCOL_ABOUT,
        ),
        field(16, key::PACKETS_F_SMB_SIZE, key::PACKETS_F_SMB_SIZE_ABOUT),
        field(
            16,
            key::PACKETS_F_SMB_CHARGE,
            key::PACKETS_F_SMB_CHARGE_ABOUT,
        ),
        field(
            32,
            key::PACKETS_F_SMB_STATUS,
            key::PACKETS_F_SMB_STATUS_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_SMB_COMMAND,
            key::PACKETS_F_SMB_COMMAND_ABOUT,
        ),
        field(
            16,
            key::PACKETS_F_SMB_CREDITS,
            key::PACKETS_F_SMB_CREDITS_ABOUT,
        ),
        field(32, key::PACKETS_F_SMB_FLAGS, key::PACKETS_F_SMB_FLAGS_ABOUT),
        field(32, key::PACKETS_F_SMB_NEXT, key::PACKETS_F_SMB_NEXT_ABOUT),
        field(64, key::PACKETS_F_SMB_MSGID, key::PACKETS_F_SMB_MSGID_ABOUT),
        field(
            32,
            key::PACKETS_F_SMB_RESERVED,
            key::PACKETS_F_SMB_RESERVED_ABOUT,
        ),
        field(32, key::PACKETS_F_SMB_TREE, key::PACKETS_F_SMB_TREE_ABOUT),
        field(
            64,
            key::PACKETS_F_SMB_SESSION,
            key::PACKETS_F_SMB_SESSION_ABOUT,
        ),
        field(
            128,
            key::PACKETS_F_SMB_SIGNATURE,
            key::PACKETS_F_SMB_SIGNATURE_ABOUT,
        ),
    ],
};

/// Every layout, for the tests and the window's sizes.
pub static ALL: &[&Layout] = &[
    &ETHERNET,
    &IPV4,
    &UDP,
    &TCP,
    &DNS,
    &HTTP_REQUEST,
    &HTTP_RESPONSE,
    &HTTP_MESSAGE,
    &LINE,
    &ARP,
    &IPV6,
    &ICMP,
    &NDP,
    &TLS,
    &KEY_SHARE,
    &QUIC_LONG,
    &QUIC_INITIAL,
    &QUIC_SHORT,
    &WIREGUARD_INIT,
    &WIREGUARD_DATA,
    &ESP,
    &IKE,
    &NTP,
    &DHCP,
    &MQTT,
    &MBAP,
    &WEBSOCKET,
    &BITTORRENT,
    &WIFI,
    &EAPOL,
    &SAE,
    &RADIUS,
    &SOCKS5,
    &BGP,
    &TOR_CELL,
    &SSH,
    &SMB2,
];
