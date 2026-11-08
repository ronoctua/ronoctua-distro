//! Every protocol of the manual, in the list's order within each group.

use crate::Protocol;

mod arp;
mod bgp;
mod bittorrent;
mod captive_portal;
mod dhcp;
mod dns;
mod dnssec;
mod doh;
mod ethernet;
mod ftp;
mod http;
mod https;
mod icmp;
mod imap;
mod ip;
mod ipsec;
mod ipv6;
mod mdns;
mod mlkem;
mod modbus;
mod mqtt;
mod nat;
mod ntp;
mod pop3;
mod proxy;
mod quic;
mod radius;
mod reverse_proxy;
mod smb;
mod smtp;
mod socks5;
mod ssh;
mod tcp;
mod tls;
mod tor;
mod udp;
mod webrtc;
mod websocket;
mod wifi;
mod wireguard;
mod wpa3;

/// Every protocol, in the list's order within each group.
pub static ALL: &[&Protocol] = &[
    &ethernet::ETHERNET,
    &arp::ARP,
    &wifi::WIFI,
    &ip::IP,
    &ipv6::IPV6,
    &icmp::ICMP,
    &nat::NAT,
    &dhcp::DHCP,
    &bgp::BGP,
    &tcp::TCP,
    &udp::UDP,
    &quic::QUIC,
    &dns::DNS,
    &dnssec::DNSSEC,
    &doh::DOH,
    &mdns::MDNS,
    &http::HTTP,
    &https::HTTPS,
    &websocket::WEBSOCKET,
    &webrtc::WEBRTC,
    &proxy::PROXY,
    &reverse_proxy::REVERSE_PROXY,
    &socks5::SOCKS5,
    &captive_portal::CAPTIVE_PORTAL,
    &smtp::SMTP,
    &pop3::POP3,
    &imap::IMAP,
    &ftp::FTP,
    &smb::SMB,
    &tls::TLS,
    &mlkem::MLKEM,
    &ssh::SSH,
    &wireguard::WIREGUARD,
    &ipsec::IPSEC,
    &wpa3::WPA3,
    &radius::RADIUS,
    &tor::TOR,
    &ntp::NTP,
    &mqtt::MQTT,
    &modbus::MODBUS,
    &bittorrent::BITTORRENT,
];

/// The protocol a word names.
#[must_use]
pub fn by_id(id: &str) -> Option<&'static Protocol> {
    ALL.iter().copied().find(|p| p.id == id)
}
