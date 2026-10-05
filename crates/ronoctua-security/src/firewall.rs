//! The person's firewall table.
//!
//! # Two tables, marks between them
//!
//! In nftables a `drop` in any table is final and an `accept` is not: an
//! `accept` ends one table's say and the packet goes on to the next. So the
//! person's table `inet ronoctua` (priority `filter - 10`, before the
//! declared one) decides and MARKS packets, and the declared `nixos-fw`
//! obeys: it accepts [`MARK_ALLOWED`] first, and logs (rate-limited,
//! [`LOG_PREFIX`]) what it is about to refuse unless [`MARK_LOG_OFF`] is
//! set, more often under [`MARK_CHATTY`]. The bits (28–30) miss Tailscale's
//! `0xff0000` and netavark's `0x2000` (`firewall-marks-are-free`).
//!
//! # What the table says
//!
//! Loopback, the machine's own virtual networks ([`LOCAL`]) and established
//! traffic are not its business. The essentials
//! (DHCP replies, IPv6 neighbour discovery, the ICMP errors a connection
//! needs) pass everywhere, shields included. Shields drop every new
//! connection. Off marks everything allowed — the person turned the firewall
//! off. Otherwise an interface on a PUBLIC network jumps to a chain that
//! allows only the rules made for public networks, logs and drops the rest;
//! on a HOME network the person's rules allow or refuse, and what they do not
//! name is left to the declared firewall, which is today's behaviour.
//! `tailscale0` is never public.
//!
//! # From a wish to a table
//!
//! The person stages a [`Settings`] at [`STAGED`] ([`stage`]); the root
//! helper opens it with `O_NOFOLLOW`, caps it at [`WISH_MOST_BYTES`],
//! validates it strictly ([`Settings::check`]), resolves which interfaces are
//! on a public network (from the NetworkManager connection each is on,
//! [`LINKS`]), checks [`script`] with `nft -c`, writes [`FILE`], and applies
//! the script with `nft -f`. [`STATUS`] says what was applied and how.
//!
//! # What NetworkManager is asked
//!
//! Through `ronoctua-network`, by the helper, as [`Known`] — never read from
//! its files. The first time it answers, every connection it has stored
//! becomes a home network ([`MIGRATED`] marks that done): the networks of
//! before keep today's behaviour, and the ones made after are born public.
//! And a connection this machine SERVES (a hotspot, `method=shared`) is never
//! public: the machine is that network's router, its clients are given the
//! machine's DHCP and DNS ([`SERVED`]), and everything else on it is what a
//! home network gets.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tools::{self, Tools};
use crate::{Refusal, Why, refused};

/// The table's name (family `inet`).
pub const TABLE: &str = "ronoctua";
/// The mark bit of a packet the person allowed.
pub const MARK_ALLOWED: u32 = 0x1000_0000;
/// The mark bit that turns the refusal log off.
pub const MARK_LOG_OFF: u32 = 0x2000_0000;
/// The mark bit that makes the refusal log chatty.
pub const MARK_CHATTY: u32 = 0x4000_0000;
/// The kernel log prefix of a refused connection — what the brain's
/// `security` module matches.
pub const LOG_PREFIX: &str = "ronoctua-refused: ";
/// The person's settings, root `0644` (readable by the person).
pub const FILE: &str = "/var/lib/ronoctua-security/firewall.json";
/// Where the person stages a wish (a person-owned `0700` folder).
pub const STAGED: &str = "/run/ronoctua-security/staged/wish.json";
/// What the helper last applied, and how.
pub const STATUS: &str = "/run/ronoctua-security/firewall.status";
/// Each interface's NetworkManager connection, as the dispatcher told it.
pub const LINKS: &str = "/run/ronoctua-security/links.json";
/// The lock every change of the table takes.
pub const LOCK: &str = "/run/ronoctua-security/firewall.lock";
/// The machine's own virtual networks — the interfaces the declared firewall
/// trusts (the Android container's bridge, …), `tailscale0` left out — one
/// name a line, written by `nix/modules/security.nix`. The person's table has
/// no say on them: shields that cut a container off its DHCP and DNS are not
/// what "block all incoming" means.
pub const LOCAL: &str = "/etc/ronoctua-security/local-interfaces";
/// The most local interfaces read.
pub const LOCAL_MOST: usize = 64;
/// The mark that every connection NetworkManager had stored when it first
/// answered was made a home network — once, and never again (an empty file).
pub const MIGRATED: &str = "/var/lib/ronoctua-security/networks-migrated";
/// The interfaces on a network this machine serves (a hotspot), as the
/// helper learnt from NetworkManager when each came up.
pub const SERVED: &str = "/run/ronoctua-security/served.json";
/// The largest wish the helper reads.
pub const WISH_MOST_BYTES: u64 = 64 * 1024;
/// The settings' format.
pub const FORMAT: u32 = 1;
/// The most rules a wish may carry.
pub const RULES_MOST: usize = 256;
/// The most port ranges one rule may carry.
pub const RANGES_MOST: usize = 32;
/// The most networks a wish may name.
pub const NETWORKS_MOST: usize = 1024;
/// The longest note, in characters.
pub const NOTE_MOST: usize = 200;
/// The interface that is never public.
pub const NEVER_PUBLIC: &str = "tailscale0";

/// A network's profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Profile {
    /// A network the person trusts.
    Home,
    /// Any other — the default for a network never seen.
    Public,
}

/// How much the refusals are logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogLevel {
    /// Not at all.
    Off,
    /// Rate-limited (6 a minute).
    #[default]
    Quiet,
    /// More (60 a minute).
    Chatty,
}

impl LogLevel {
    /// The bits the table sets for the declared firewall's log.
    #[must_use]
    pub const fn mark(self) -> u32 {
        match self {
            Self::Off => MARK_LOG_OFF,
            Self::Quiet => 0,
            Self::Chatty => MARK_CHATTY,
        }
    }

    /// The table's own rate for it: `(per minute, burst)`; `None` when off.
    #[must_use]
    pub const fn rate(self) -> Option<(u32, u32)> {
        match self {
            Self::Off => None,
            Self::Quiet => Some((6, 12)),
            Self::Chatty => Some((60, 120)),
        }
    }
}

/// What a rule does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    /// Let it in (mark it allowed).
    Allow,
    /// Refuse it (logged, then dropped).
    Deny,
}

/// A rule's protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    /// TCP.
    Tcp,
    /// UDP.
    Udp,
}

impl Protocol {
    const fn word(self) -> &'static str {
        match self {
            Self::Tcp => "tcp",
            Self::Udp => "udp",
        }
    }
}

/// One of the person's rules.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Its id (`r-` and up to 40 of `a-z`, digits and `-`).
    pub id: String,
    /// What it does.
    pub action: Action,
    /// The protocol.
    #[serde(rename = "proto")]
    pub protocol: Protocol,
    /// Port ranges, inclusive (`[8080, 8080]` is one port); empty is every
    /// port of the protocol.
    pub ports: Vec<(u16, u16)>,
    /// Only from this source (an address, or CIDR with no host bits).
    pub source: Option<String>,
    /// Only on this interface.
    pub interface: Option<String>,
    /// The profiles it applies in (at least one).
    pub profiles: Vec<Profile>,
    /// The person's note — data, never in the table.
    pub note: String,
}

/// The person's settings: `firewall.json`, and what a wish stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// The format ([`FORMAT`]).
    pub format: u32,
    /// Whether the person's table decides at all (off: everything marked
    /// allowed — the firewall is off; shields still drop).
    pub enabled: bool,
    /// Shields: every new incoming connection dropped.
    pub shields: bool,
    /// The profile of a network with no entry.
    pub default_profile: Profile,
    /// Each NetworkManager connection's profile, by its UUID.
    pub networks: BTreeMap<String, Profile>,
    /// The refusal log.
    pub log: LogLevel,
    /// The rules.
    pub rules: Vec<Rule>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            format: FORMAT,
            enabled: true,
            shields: false,
            default_profile: Profile::Public,
            networks: BTreeMap::new(),
            log: LogLevel::Quiet,
            rules: Vec::new(),
        }
    }
}

/// Whether a name is one an interface may have here: 1–15 of ASCII letters,
/// digits, `_`, `.` and `-`, not `.`/`..`, not starting with `-`. (Linux
/// allows more; nothing this reads from NetworkManager needs more, and every
/// byte of it lands inside quotes in the table.)
#[must_use]
pub fn interface_is_plain(name: &str) -> bool {
    (1..=15).contains(&name.len())
        && name != "."
        && name != ".."
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// Whether a text is a connection UUID as NetworkManager writes it:
/// `8-4-4-4-12` lowercase hex digits.
#[must_use]
pub fn connection_is_plain(uuid: &str) -> bool {
    let groups: Vec<&str> = uuid.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
}

/// A source, checked: the canonical text and whether it is IPv6.
///
/// An address alone, or an address and a prefix length with no host bits
/// set. The text is rewritten canonically (`::ffff:0:0/96` rather than any
/// of its spellings), so the table never carries what the person typed.
#[must_use]
pub fn source_of(text: &str) -> Option<(String, bool)> {
    let (address, prefix) = match text.split_once('/') {
        Some((a, p)) => {
            if p.is_empty() || p.len() > 3 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            (a, Some(p.parse::<u8>().ok()?))
        }
        None => (text, None),
    };
    let address: IpAddr = address.parse().ok()?;
    let bits = if address.is_ipv4() { 32 } else { 128 };
    let Some(prefix) = prefix else {
        return Some((address.to_string(), address.is_ipv6()));
    };
    if u32::from(prefix) > bits {
        return None;
    }
    let host_bits_clear = match address {
        IpAddr::V4(v4) => {
            let n = u32::from(v4);
            prefix == 32 || n & (u32::MAX >> prefix) == 0
        }
        IpAddr::V6(v6) => {
            let n = u128::from(v6);
            prefix == 128 || n & (u128::MAX >> prefix) == 0
        }
    };
    host_bits_clear.then(|| (format!("{address}/{prefix}"), address.is_ipv6()))
}

fn rule_id_is_plain(id: &str) -> bool {
    id.strip_prefix("r-").is_some_and(|rest| {
        (1..=40).contains(&rest.len())
            && rest
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    })
}

impl Rule {
    fn check(&self) -> bool {
        let ports_fit =
            self.ports.len() <= RANGES_MOST && self.ports.iter().all(|&(a, b)| a >= 1 && a <= b);
        let profiles: BTreeSet<Profile> = self.profiles.iter().copied().collect();
        rule_id_is_plain(&self.id)
            && ports_fit
            && self
                .source
                .as_deref()
                .is_none_or(|s| source_of(s).is_some())
            && self.interface.as_deref().is_none_or(interface_is_plain)
            && !self.profiles.is_empty()
            && profiles.len() == self.profiles.len()
            && self.note.chars().count() <= NOTE_MOST
            && !self.note.chars().any(char::is_control)
    }
}

impl Settings {
    /// Whether every field is what it must be.
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`] for a format that is not [`FORMAT`], too many rules,
    /// ranges or networks, a network that is not a UUID, two rules with one
    /// id, or a rule whose id, ports, source, interface, profiles or note is
    /// not plain.
    pub fn check(&self) -> Result<(), Refusal> {
        let ids: BTreeSet<&str> = self.rules.iter().map(|r| r.id.as_str()).collect();
        let fine = self.format == FORMAT
            && self.rules.len() <= RULES_MOST
            && self.networks.len() <= NETWORKS_MOST
            && self.networks.keys().all(|uuid| connection_is_plain(uuid))
            && ids.len() == self.rules.len()
            && self.rules.iter().all(Rule::check);
        if fine {
            Ok(())
        } else {
            Err(refused(Why::Invalid))
        }
    }

    /// The settings a wish's bytes say, checked.
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`] for more than [`WISH_MOST_BYTES`], JSON that is not
    /// exactly [`Settings`] (an unknown field included), or what
    /// [`Settings::check`] refuses.
    pub fn of_bytes(bytes: &[u8]) -> Result<Self, Refusal> {
        if bytes.len() as u64 > WISH_MOST_BYTES {
            return Err(refused(Why::Invalid));
        }
        let settings: Self = serde_json::from_slice(bytes).map_err(|_| refused(Why::Invalid))?;
        settings.check()?;
        Ok(settings)
    }

    /// The profile of the network with this connection UUID.
    #[must_use]
    pub fn profile_of(&self, connection: &str) -> Profile {
        self.networks
            .get(connection)
            .copied()
            .unwrap_or(self.default_profile)
    }
}

/// Each interface's NetworkManager connection UUID.
pub type Links = BTreeMap<String, String>;

/// Everything the script is made of: the settings, and which interfaces are
/// on a public network now (resolved by the helper, never staged).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Wish {
    /// The settings.
    pub settings: Settings,
    /// The interfaces on a public network.
    pub public_interfaces: Vec<String>,
    /// The machine's own virtual networks ([`LOCAL`]), left to the declared
    /// firewall.
    pub local_interfaces: Vec<String>,
    /// The networks this machine serves ([`SERVED`]): never public, and
    /// their clients are given DHCP and DNS.
    pub served_interfaces: Vec<String>,
}

impl Wish {
    /// The wish of these settings on these links: an interface is public
    /// when its network's profile is — and never [`NEVER_PUBLIC`]. Pure.
    #[must_use]
    pub fn of(settings: Settings, links: &Links) -> Self {
        let public_interfaces = links
            .iter()
            .filter(|(interface, connection)| {
                interface.as_str() != NEVER_PUBLIC
                    && interface_is_plain(interface)
                    && settings.profile_of(connection) == Profile::Public
            })
            .map(|(interface, _)| interface.clone())
            .collect();
        Self {
            settings,
            public_interfaces,
            local_interfaces: Vec::new(),
            served_interfaces: Vec::new(),
        }
    }

    /// The same wish, with the machine's own virtual networks left alone —
    /// and never public.
    #[must_use]
    pub fn with_local(mut self, local: Vec<String>) -> Self {
        self.public_interfaces.retain(|i| !local.contains(i));
        self.local_interfaces = local;
        self
    }

    /// The same wish, with the networks this machine serves — never public.
    #[must_use]
    pub fn with_served(mut self, served: Vec<String>) -> Self {
        self.public_interfaces.retain(|i| !served.contains(i));
        self.served_interfaces = served;
        self
    }
}

/// The statement that marks a packet allowed and ends this table's say.
fn allowed() -> String {
    format!("meta mark set meta mark | {MARK_ALLOWED:#010x} accept")
}

/// The match of one rule (without its verdict).
fn matched(rule: &Rule) -> String {
    let mut out = String::new();
    if let Some(interface) = &rule.interface {
        let _ = write!(out, "iifname \"{interface}\" ");
    }
    if let Some((source, v6)) = rule.source.as_deref().and_then(source_of) {
        let _ = write!(out, "{} saddr {source} ", if v6 { "ip6" } else { "ip" });
    }
    let protocol = rule.protocol.word();
    let port = |&(a, b): &(u16, u16)| {
        if a == b {
            a.to_string()
        } else {
            format!("{a}-{b}")
        }
    };
    match rule.ports.as_slice() {
        [] => {
            let _ = write!(out, "meta l4proto {protocol}");
        }
        [one] => {
            let _ = write!(out, "{protocol} dport {}", port(one));
        }
        many => {
            let ports: Vec<String> = many.iter().map(port).collect();
            let _ = write!(out, "{protocol} dport {{ {} }}", ports.join(", "));
        }
    }
    out
}

/// The refusal log statement at this level; `None` when off.
fn logged(level: LogLevel) -> Option<String> {
    level.rate().map(|(per_minute, burst)| {
        format!(
            "limit rate {per_minute}/minute burst {burst} packets log prefix \"{LOG_PREFIX}\" level info"
        )
    })
}

/// A rule's lines, indented.
fn rule_lines(out: &mut String, rule: &Rule, level: LogLevel) {
    let matched = matched(rule);
    let id = &rule.id;
    match rule.action {
        Action::Allow => {
            let _ = writeln!(out, "    {matched} {} comment \"{id}\"", allowed());
        }
        Action::Deny => {
            if let Some(log) = logged(level) {
                let _ = writeln!(out, "    {matched} {log}");
            }
            let _ = writeln!(out, "    {matched} drop comment \"{id}\"");
        }
    }
}

/// What passes everywhere, shields included: the replies a DHCP client
/// waits for, IPv6 neighbour discovery and the ICMP errors a connection
/// needs. Marked allowed, so a public network's drop does not take the
/// address or the path MTU away.
const ESSENTIALS: [&str; 4] = [
    "udp sport 67 udp dport 68",
    "ip6 saddr fe80::/10 udp sport 547 udp dport 546",
    "icmpv6 type { destination-unreachable, packet-too-big, time-exceeded, parameter-problem, \
     mld-listener-query, nd-router-advert, nd-neighbor-solicit, nd-neighbor-advert }",
    "icmp type { destination-unreachable, time-exceeded, parameter-problem }",
];

/// What the clients of a network this machine serves need of it, on that
/// network only: an address (DHCP), names (DNS) and, over IPv6, the router's
/// answer to a solicitation. Marked allowed, shields included — the declared
/// firewall opens none of them, and a hotspot whose clients get no address
/// is not one.
const SERVING: [&str; 3] = [
    "udp dport { 53, 67 }",
    "tcp dport 53",
    "icmpv6 type nd-router-solicit",
];

/// The nft script that replaces the person's table with `wish`, as one
/// atomic transaction. Pure.
///
/// It trusts `wish` to have been [`Settings::check`]ed: a rule that is not
/// plain is left out rather than written, so even an unchecked wish cannot
/// put a byte the person chose outside a quoted, checked field.
#[must_use]
pub fn script(wish: &Wish) -> String {
    let settings = &wish.settings;
    let mut out = String::new();
    let _ = writeln!(out, "table inet {TABLE}");
    let _ = writeln!(out, "delete table inet {TABLE}");
    let _ = writeln!(out, "table inet {TABLE} {{");
    let locals: Vec<&String> = wish
        .local_interfaces
        .iter()
        .filter(|i| i.as_str() != NEVER_PUBLIC && interface_is_plain(i))
        .collect();
    let served: Vec<&String> = wish
        .served_interfaces
        .iter()
        .filter(|i| i.as_str() != NEVER_PUBLIC && interface_is_plain(i) && !locals.contains(i))
        .collect();
    let publics: Vec<&String> = wish
        .public_interfaces
        .iter()
        .filter(|i| {
            i.as_str() != NEVER_PUBLIC
                && interface_is_plain(i)
                && !locals.contains(i)
                && !served.contains(i)
        })
        .collect();
    if settings.enabled {
        out.push_str("  set public {\n    type ifname\n");
        if !publics.is_empty() {
            let names: Vec<String> = publics.iter().map(|i| format!("\"{i}\"")).collect();
            let _ = writeln!(out, "    elements = {{ {} }}", names.join(", "));
        }
        out.push_str("  }\n\n");
    }
    out.push_str("  chain input {\n");
    out.push_str("    type filter hook input priority filter - 10; policy accept;\n");
    out.push_str("    iif \"lo\" accept\n");
    if !locals.is_empty() {
        let names: Vec<String> = locals.iter().map(|i| format!("\"{i}\"")).collect();
        let _ = writeln!(out, "    iifname {{ {} }} accept", names.join(", "));
    }
    out.push_str("    ct state established,related accept\n");
    for essential in ESSENTIALS {
        let _ = writeln!(out, "    {essential} {}", allowed());
    }
    if !served.is_empty() {
        let names: Vec<String> = served.iter().map(|i| format!("\"{i}\"")).collect();
        for need in SERVING {
            let _ = writeln!(
                out,
                "    iifname {{ {} }} {need} {}",
                names.join(", "),
                allowed()
            );
        }
    }
    if settings.shields {
        out.push_str("    ct state new drop\n");
    }
    let log_mark = if settings.enabled {
        settings.log.mark()
    } else {
        0
    };
    if log_mark != 0 {
        let _ = writeln!(out, "    meta mark set meta mark | {log_mark:#010x}");
    }
    if !settings.enabled {
        let _ = writeln!(out, "    {}", allowed());
        out.push_str("  }\n}\n");
        return out;
    }
    out.push_str("    iifname @public jump public\n");
    let plain_rules = settings.rules.iter().filter(|r| r.check());
    for rule in plain_rules
        .clone()
        .filter(|r| r.profiles.contains(&Profile::Home))
    {
        rule_lines(&mut out, rule, settings.log);
    }
    out.push_str("  }\n\n");
    out.push_str("  chain public {\n");
    for rule in plain_rules.filter(|r| r.profiles.contains(&Profile::Public)) {
        rule_lines(&mut out, rule, settings.log);
    }
    if let Some(log) = logged(settings.log) {
        let _ = writeln!(out, "    tcp flags syn / fin,syn,rst,ack {log}");
    }
    out.push_str("    drop\n");
    out.push_str("  }\n}\n");
    out
}

/// What the helper last did to the table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// The format ([`FORMAT`]).
    pub format: u32,
    /// When, in seconds since the epoch.
    pub applied_s: i64,
    /// Whether the table in the kernel is the one [`FILE`] says.
    pub ok: bool,
    /// Why not, when it is not.
    pub why: Option<Why>,
    /// Each interface NetworkManager told of, and its profile.
    pub profiles: BTreeMap<String, Profile>,
    /// Connections that are up and the person never placed: they have the
    /// default profile, and the page offers "treat as home".
    pub unplaced: Vec<String>,
    /// The interfaces on a network this machine serves (a hotspot): neither
    /// home nor public, and in neither list above.
    #[serde(default)]
    pub served: Vec<String>,
}

impl Status {
    /// The status of a wish's settings on these links, applied or not.
    #[must_use]
    pub fn of(settings: &Settings, links: &Links, ok: Result<(), Refusal>) -> Self {
        let mut unplaced: Vec<String> = links
            .values()
            .filter(|c| !settings.networks.contains_key(*c))
            .cloned()
            .collect();
        unplaced.sort();
        unplaced.dedup();
        Self {
            format: FORMAT,
            applied_s: crate::now_unix_s(),
            ok: ok.is_ok(),
            why: ok.err().map(|r| r.why),
            profiles: links
                .iter()
                .map(|(interface, connection)| {
                    let profile = if interface == NEVER_PUBLIC {
                        Profile::Home
                    } else {
                        settings.profile_of(connection)
                    };
                    (interface.clone(), profile)
                })
                .collect(),
            unplaced,
            served: Vec::new(),
        }
    }

    /// The same status, with the interfaces this machine serves taken out of
    /// the profiles (and their connections out of the unplaced) and listed on
    /// their own.
    #[must_use]
    pub fn serving(mut self, served: &[String], links: &Links) -> Self {
        let connections: BTreeSet<&String> = served.iter().filter_map(|i| links.get(i)).collect();
        self.profiles
            .retain(|interface, _| !served.contains(interface));
        self.unplaced.retain(|c| !connections.contains(c));
        self.served = served.to_vec();
        self
    }
}

/// The connections that are up on a PUBLIC network and that the person never
/// placed — each with its interface: the networks "treat as home" is about
/// (the page's row, and the watch's event when one comes up). Sorted by
/// connection. Pure.
#[must_use]
pub fn public_unplaced(status: &Status, links: &Links) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = status
        .profiles
        .iter()
        .filter(|(_, p)| **p == Profile::Public)
        .filter_map(|(iface, _)| links.get(iface).map(|uuid| (uuid.clone(), iface.clone())))
        .filter(|(uuid, _)| status.unplaced.contains(uuid))
        .collect();
    out.sort();
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// What NetworkManager said of its stored connections — asked through
/// `ronoctua-network` by the helper, never read from its files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Known {
    /// Every stored connection's UUID.
    pub saved: BTreeSet<String>,
    /// The ones whose network this machine serves (`method=shared`).
    pub served: BTreeSet<String>,
}

impl Known {
    /// The facts of these `(uuid, serves)` pairs: UUIDs lowercased, and one
    /// that is not plain left out. Pure.
    #[must_use]
    pub fn of(stored: impl IntoIterator<Item = (String, bool)>) -> Self {
        let mut known = Self::default();
        for (uuid, serves) in stored {
            let uuid = uuid.to_ascii_lowercase();
            if !connection_is_plain(&uuid) || known.saved.len() >= NETWORKS_MOST {
                continue;
            }
            if serves {
                known.served.insert(uuid.clone());
            }
            known.saved.insert(uuid);
        }
        known
    }
}

/// Where the firewall's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// [`FILE`].
    pub file: PathBuf,
    /// The folder of [`STAGED`].
    pub staged: PathBuf,
    /// [`STATUS`].
    pub status: PathBuf,
    /// [`LINKS`].
    pub links: PathBuf,
    /// [`LOCK`].
    pub lock: PathBuf,
    /// [`LOCAL`].
    pub local: PathBuf,
    /// [`MIGRATED`].
    pub migrated: PathBuf,
    /// [`SERVED`].
    pub served: PathBuf,
}

impl Places {
    /// The machine's.
    #[must_use]
    pub fn system() -> Self {
        Self {
            file: PathBuf::from(FILE),
            staged: Path::new(STAGED)
                .parent()
                .map_or_else(PathBuf::new, Path::to_path_buf),
            status: PathBuf::from(STATUS),
            links: PathBuf::from(LINKS),
            lock: PathBuf::from(LOCK),
            local: PathBuf::from(LOCAL),
            migrated: PathBuf::from(MIGRATED),
            served: PathBuf::from(SERVED),
        }
    }

    fn wish(&self) -> PathBuf {
        self.staged.join("wish.json")
    }
}

/// The person's settings as the file holds them; `None` when there is no
/// file yet. Read as the person (`0644`).
///
/// # Errors
///
/// [`Why::Unreadable`] when it cannot be read, [`Why::Invalid`] when it is
/// not valid settings.
pub fn read_settings() -> Result<Option<Settings>, Refusal> {
    read_settings_at(Path::new(FILE))
}

fn read_settings_at(path: &Path) -> Result<Option<Settings>, Refusal> {
    match std::fs::read(path) {
        Ok(bytes) => Settings::of_bytes(&bytes).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(refused(Why::Unreadable)),
    }
}

/// What the helper last did; `None` before it ever ran. Read as the person.
#[must_use]
pub fn read_status() -> Option<Status> {
    serde_json::from_slice(&std::fs::read(STATUS).ok()?).ok()
}

/// Stages `settings` for the helper, as the person: checked first (so a bad
/// wish is refused before anybody types a password), then written whole into
/// the person's staging folder.
///
/// # Errors
///
/// What [`Settings::check`] refuses; [`Why::Failed`] when it cannot be
/// written.
pub fn stage(settings: &Settings) -> Result<(), Refusal> {
    stage_at(&Places::system(), settings)
}

fn stage_at(places: &Places, settings: &Settings) -> Result<(), Refusal> {
    settings.check()?;
    let bytes = serde_json::to_vec(settings).map_err(|_| refused(Why::Invalid))?;
    crate::write_whole(&places.wish(), &bytes, 0o600).map_err(|_| refused(Why::Failed))
}

/// The lock on the table, held while one change is made.
struct Held(#[allow(dead_code)] std::fs::File);

fn lock(places: &Places) -> Result<Held, Refusal> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&places.lock)
        .map_err(|e| {
            eprintln!("<3>{} could not be opened: {e}", places.lock.display());
            refused(Why::Failed)
        })?;
    rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)
        .map_err(|_| refused(Why::Failed))?;
    Ok(Held(file))
}

/// The machine's own virtual networks, as the module wrote them; none when
/// there is no file.
fn read_local(places: &Places) -> Vec<String> {
    std::fs::read_to_string(&places.local)
        .map(|text| {
            text.lines()
                .map(str::trim)
                .filter(|name| interface_is_plain(name) && *name != NEVER_PUBLIC)
                .take(LOCAL_MOST)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn read_links(places: &Places) -> Links {
    std::fs::read(&places.links)
        .ok()
        .and_then(|b| serde_json::from_slice::<Links>(&b).ok())
        .map(|links| {
            links
                .into_iter()
                .filter(|(i, c)| interface_is_plain(i) && connection_is_plain(c))
                .collect()
        })
        .unwrap_or_default()
}

/// The interfaces on a network this machine serves, as the helper wrote
/// them; none when there is no file.
fn read_served(places: &Places) -> Vec<String> {
    std::fs::read(&places.served)
        .ok()
        .and_then(|b| serde_json::from_slice::<BTreeSet<String>>(&b).ok())
        .map(|served| {
            served
                .into_iter()
                .filter(|i| interface_is_plain(i) && i != NEVER_PUBLIC)
                .collect()
        })
        .unwrap_or_default()
}

/// The settings to apply: the file's, or the defaults when there is none
/// yet.
fn load(places: &Places) -> Result<Settings, Refusal> {
    Ok(read_settings_at(&places.file)?.unwrap_or_default())
}

/// The first time NetworkManager answers, every connection it has stored and
/// the person never placed becomes a home network — the networks of before
/// keep today's behaviour; new ones are born public. Whether it did so now
/// (the caller marks it done once the table is applied); never again after
/// [`MIGRATED`], and not while NetworkManager has not answered.
fn migrate(places: &Places, settings: &mut Settings, known: Option<&Known>) -> bool {
    let Some(known) = known else {
        return false;
    };
    if places.migrated.exists() {
        return false;
    }
    for uuid in &known.saved {
        if settings.networks.len() >= NETWORKS_MOST {
            break;
        }
        settings
            .networks
            .entry(uuid.clone())
            .or_insert(Profile::Home);
    }
    true
}

/// Marks the first placement done.
fn migrated(places: &Places) {
    if let Err(e) = crate::write_whole(&places.migrated, b"", 0o644) {
        eprintln!("<4>{} could not be written: {e}", places.migrated.display());
    }
}

/// Checks the table with `nft -c`, keeps the settings, applies the table
/// with `nft -f`, and writes the status either way.
fn apply(
    places: &Places,
    tools: &Tools,
    settings: &Settings,
    links: &Links,
) -> Result<(), Refusal> {
    let served = read_served(places);
    let applied = (|| {
        settings.check()?;
        let nft = tools::given(tools.nft.as_ref())?;
        let script = script(
            &Wish::of(settings.clone(), links)
                .with_local(read_local(places))
                .with_served(served.clone()),
        );
        tools::run(nft, &["-c", "-f", "-"], Some(script.as_bytes()))?;
        let bytes = serde_json::to_vec_pretty(settings).map_err(|_| refused(Why::Invalid))?;
        crate::write_whole(&places.file, &bytes, 0o644).map_err(|e| {
            eprintln!("<3>{} could not be written: {e}", places.file.display());
            refused(Why::Failed)
        })?;
        tools::run(nft, &["-f", "-"], Some(script.as_bytes())).map(|_| ())
    })();
    let status = Status::of(settings, links, applied).serving(&served, links);
    if let Ok(bytes) = serde_json::to_vec(&status) {
        let _ = crate::write_whole(&places.status, &bytes, 0o644);
    }
    applied
}

/// Applies the settings the file holds (a unit: the boot, an nftables
/// restart). A file that is not valid leaves the kernel's table as it is.
/// NetworkManager is not asked here: at boot it may not be up yet, and the
/// first link it reports places the networks.
///
/// # Errors
///
/// [`Why::Invalid`], [`Why::Unreadable`], [`Why::Unstartable`],
/// [`Why::Failed`].
pub fn apply_stored(places: &Places, tools: &Tools) -> Result<(), Refusal> {
    let _held = lock(places)?;
    let links = read_links(places);
    match load(places) {
        Ok(settings) => apply(places, tools, &settings, &links),
        Err(refusal) => {
            // No profiles: nothing the file says could be read.
            let status = Status::of(&Settings::default(), &Links::new(), Err(refusal));
            if let Ok(bytes) = serde_json::to_vec(&status) {
                let _ = crate::write_whole(&places.status, &bytes, 0o644);
            }
            Err(refusal)
        }
    }
}

/// The wish `uid` staged, read the careful way and consumed: the folder is
/// theirs and private, the file is theirs, regular, not a link, at most
/// [`WISH_MOST_BYTES`].
fn take_wish(places: &Places, uid: u32) -> Result<Settings, Refusal> {
    use std::io::Read as _;
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let folder = std::fs::symlink_metadata(&places.staged).map_err(|_| refused(Why::Unreadable))?;
    if !folder.is_dir() || folder.uid() != uid || folder.mode() & 0o077 != 0 {
        eprintln!(
            "<3>{} is not the person's private folder",
            places.staged.display()
        );
        return Err(refused(Why::Invalid));
    }
    let path = places.wish();
    let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
    let opened = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(i32::try_from(flags.bits()).unwrap_or(0))
        .open(&path);
    // Consumed, whatever it is: a wish left behind would be applied again by
    // the next `firewall-apply` through the door, which turns one password
    // into a standing one. `unlink` takes a link away and never follows it.
    let _ = std::fs::remove_file(&path);
    let file = opened.map_err(|_| refused(Why::Unreadable))?;
    let meta = file.metadata().map_err(|_| refused(Why::Unreadable))?;
    if !meta.is_file() || meta.uid() != uid || meta.len() > WISH_MOST_BYTES {
        return Err(refused(Why::Invalid));
    }
    let mut bytes = Vec::new();
    file.take(WISH_MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| refused(Why::Unreadable))?;
    Settings::of_bytes(&bytes)
}

/// Applies the wish the person `uid` staged (through the door). A page that
/// staged before the first placement never saw the networks of before, so
/// they are placed into the wish as they would have been into the file.
///
/// # Errors
///
/// What [`apply_stored`] refuses, and a wish that is not the person's or not
/// valid ([`Why::Invalid`], [`Why::Unreadable`]).
pub fn apply_staged(
    places: &Places,
    tools: &Tools,
    uid: u32,
    known: Option<&Known>,
) -> Result<(), Refusal> {
    let _held = lock(places)?;
    let mut settings = take_wish(places, uid)?;
    let placed = migrate(places, &mut settings, known);
    apply(places, tools, &settings, &read_links(places))?;
    if placed {
        migrated(places);
    }
    Ok(())
}

/// A network came up on `interface`, or went away: the links follow, and
/// the table with them. `known` is what NetworkManager said of its stored
/// connections, `None` when it did not answer: then the network is not known
/// to be one this machine serves, and nothing is placed.
///
/// # Errors
///
/// What [`apply_stored`] refuses.
pub fn link(
    places: &Places,
    tools: &Tools,
    interface: &str,
    connection: &str,
    up: bool,
    known: Option<&Known>,
) -> Result<(), Refusal> {
    if !interface_is_plain(interface) || !connection_is_plain(connection) {
        return Err(refused(Why::Invalid));
    }
    let _held = lock(places)?;
    let mut links = read_links(places);
    let mut served: BTreeSet<String> = read_served(places).into_iter().collect();
    if up {
        links.insert(interface.to_owned(), connection.to_owned());
    } else {
        links.remove(interface);
    }
    if up && known.is_some_and(|k| k.served.contains(connection)) {
        served.insert(interface.to_owned());
    } else {
        served.remove(interface);
    }
    if let Ok(bytes) = serde_json::to_vec(&links) {
        let _ = crate::write_whole(&places.links, &bytes, 0o644);
    }
    if let Ok(bytes) = serde_json::to_vec(&served) {
        let _ = crate::write_whole(&places.served, &bytes, 0o644);
    }
    let mut settings = load(places)?;
    let placed = migrate(places, &mut settings, known);
    apply(places, tools, &settings, &links)?;
    if placed {
        migrated(places);
    }
    Ok(())
}

/// Raises the shields — the one change that needs no password, because it
/// only ever takes away.
///
/// # Errors
///
/// What [`apply_stored`] refuses.
pub fn shields_up(places: &Places, tools: &Tools) -> Result<(), Refusal> {
    let _held = lock(places)?;
    let mut settings = load(places)?;
    settings.shields = true;
    apply(places, tools, &settings, &read_links(places))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    const UUID_HOME: &str = "3f2a9c1e-0b7d-4e55-9a51-2c8d7e6f1a0b";
    const UUID_CAFE: &str = "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d";

    fn rule(id: &str, action: Action, ports: &[(u16, u16)], profiles: &[Profile]) -> Rule {
        Rule {
            id: id.to_owned(),
            action,
            protocol: Protocol::Tcp,
            ports: ports.to_vec(),
            source: None,
            interface: None,
            profiles: profiles.to_vec(),
            note: String::new(),
        }
    }

    #[test]
    fn the_settings_are_the_file_the_design_names() {
        let text = r#"{"format":1,"enabled":true,"shields":false,"default_profile":"public","networks":{"3f2a9c1e-0b7d-4e55-9a51-2c8d7e6f1a0b":"home"},"log":"quiet","rules":[{"id":"r-1","action":"allow","proto":"tcp","ports":[[8080,8080]],"source":"192.168.1.0/24","interface":null,"profiles":["home"],"note":"dev server"}]}"#;
        let settings = Settings::of_bytes(text.as_bytes()).unwrap();
        assert_eq!(settings.rules[0].ports, [(8080, 8080)]);
        assert_eq!(settings.networks[UUID_HOME], Profile::Home);
        assert_eq!(serde_json::to_string(&settings).unwrap(), text);
        for mark in [MARK_ALLOWED, MARK_LOG_OFF, MARK_CHATTY] {
            assert_eq!(mark.count_ones(), 1);
            assert_eq!(mark & 0x00ff_0000, 0, "clear of Tailscale's bits");
            assert_eq!(mark & 0x2000, 0, "clear of netavark's bit");
        }
        assert_eq!(MARK_ALLOWED & (MARK_LOG_OFF | MARK_CHATTY), 0);
        assert_eq!(MARK_LOG_OFF & MARK_CHATTY, 0);
    }

    /// FAILS IF: "treat as home" names a network the person placed, one on
    /// a home profile, or one that is not up.
    #[test]
    fn the_networks_to_treat_as_home_are_public_and_unplaced() {
        let mut settings = Settings::default();
        settings
            .networks
            .insert(UUID_HOME.to_owned(), Profile::Home);
        let links: Links = [
            ("enp3s0".to_owned(), UUID_HOME.to_owned()),
            ("wlan0".to_owned(), UUID_CAFE.to_owned()),
        ]
        .into();
        let status = Status::of(&settings, &links, Ok(()));
        assert_eq!(
            public_unplaced(&status, &links),
            [(UUID_CAFE.to_owned(), "wlan0".to_owned())]
        );
        settings
            .networks
            .insert(UUID_CAFE.to_owned(), Profile::Public);
        let placed = Status::of(&settings, &links, Ok(()));
        assert!(public_unplaced(&placed, &links).is_empty(), "placed public");
        settings.default_profile = Profile::Home;
        settings.networks.remove(UUID_CAFE);
        let home = Status::of(&settings, &links, Ok(()));
        assert!(public_unplaced(&home, &links).is_empty(), "home by default");
    }

    #[test]
    fn a_hostile_wish_is_refused() {
        let good = Settings {
            rules: vec![rule(
                "r-web",
                Action::Allow,
                &[(8080, 8080)],
                &[Profile::Home],
            )],
            ..Settings::default()
        };
        good.check().unwrap();
        let with = |f: &dyn Fn(&mut Settings)| {
            let mut s = good.clone();
            f(&mut s);
            s.check().map_err(|r| r.why)
        };
        let invalid = Err(Why::Invalid);
        assert_eq!(with(&|s| s.format = 2), invalid, "a format from elsewhere");
        assert_eq!(with(&|s| s.rules[0].id = "r-x\" accept".into()), invalid);
        assert_eq!(with(&|s| s.rules[0].id = "x-1".into()), invalid);
        assert_eq!(with(&|s| s.rules[0].id = "r-".into()), invalid);
        assert_eq!(
            with(&|s| s.rules[0].ports = vec![(0, 10)]),
            invalid,
            "port 0"
        );
        assert_eq!(
            with(&|s| s.rules[0].ports = vec![(20, 10)]),
            invalid,
            "backwards"
        );
        assert_eq!(
            with(&|s| s.rules[0].ports = vec![(1, 1); RANGES_MOST + 1]),
            invalid
        );
        for source in [
            "10.0.0.1/24",
            "10.0.0.0/33",
            "10.0.0.0/",
            "10.0.0.0/+8",
            "any",
            "10.0.0.0/8 accept",
            "fe80::1/64",
            "::/129",
            "1.2.3.4; drop",
        ] {
            assert_eq!(
                with(&|s| s.rules[0].source = Some(source.into())),
                invalid,
                "{source}"
            );
        }
        for interface in [
            "",
            "a b",
            "\"eth0\"",
            "-x",
            "..",
            "abcdefghijklmnop",
            "eth*",
            "é",
        ] {
            assert_eq!(
                with(&|s| s.rules[0].interface = Some(interface.into())),
                invalid,
                "{interface:?}"
            );
        }
        assert_eq!(with(&|s| s.rules[0].profiles = vec![]), invalid);
        assert_eq!(
            with(&|s| s.rules[0].profiles = vec![Profile::Home, Profile::Home]),
            invalid
        );
        assert_eq!(with(&|s| s.rules[0].note = "a\nb".into()), invalid);
        assert_eq!(
            with(&|s| s.rules[0].note = "n".repeat(NOTE_MOST + 1)),
            invalid
        );
        assert_eq!(
            with(&|s| s.rules.push(s.rules[0].clone())),
            invalid,
            "one id twice"
        );
        assert_eq!(
            with(&|s| {
                s.networks.insert("not-a-uuid".into(), Profile::Home);
            }),
            invalid
        );
        assert_eq!(
            with(&|s| s.rules = (0..=RULES_MOST)
                .map(|n| rule(&format!("r-{n}"), Action::Allow, &[], &[Profile::Home]))
                .collect()),
            invalid
        );
        // And what JSON alone can say.
        for text in [
            &b"{"[..],
            br#"{"format":1}"#,
            br#"{"format":1,"enabled":true,"shields":false,"default_profile":"public","networks":{},"log":"quiet","rules":[],"extra":1}"#,
            br#"{"format":1,"enabled":true,"shields":false,"default_profile":"office","networks":{},"log":"quiet","rules":[]}"#,
            br#"{"format":1,"enabled":true,"shields":false,"default_profile":"public","networks":{},"log":"quiet","rules":[{"id":"r-1","action":"allow","proto":"tcp","ports":[[1,70000]],"source":null,"interface":null,"profiles":["home"],"note":""}]}"#,
        ] {
            assert_eq!(Settings::of_bytes(text).unwrap_err().why, Why::Invalid);
        }
        let huge = vec![b' '; usize::try_from(WISH_MOST_BYTES).unwrap() + 1];
        assert_eq!(Settings::of_bytes(&huge).unwrap_err().why, Why::Invalid);
    }

    #[test]
    fn sources_are_rewritten_canonically() {
        assert_eq!(
            source_of("192.168.1.0/24"),
            Some(("192.168.1.0/24".into(), false))
        );
        assert_eq!(
            source_of("192.168.1.7"),
            Some(("192.168.1.7".into(), false))
        );
        assert_eq!(source_of("0.0.0.0/0"), Some(("0.0.0.0/0".into(), false)));
        assert_eq!(source_of("FD00:0::/8"), Some(("fd00::/8".into(), true)));
        assert_eq!(
            source_of("2001:db8::1/128"),
            Some(("2001:db8::1/128".into(), true))
        );
        assert_eq!(source_of("192.168.1.1/24"), None, "host bits");
    }

    #[test]
    fn the_default_table_decides_little() {
        let wish = Wish::default();
        assert_eq!(
            script(&wish),
            "table inet ronoctua\n\
             delete table inet ronoctua\n\
             table inet ronoctua {\n\
             \x20 set public {\n\
             \x20   type ifname\n\
             \x20 }\n\
             \n\
             \x20 chain input {\n\
             \x20   type filter hook input priority filter - 10; policy accept;\n\
             \x20   iif \"lo\" accept\n\
             \x20   ct state established,related accept\n\
             \x20   udp sport 67 udp dport 68 meta mark set meta mark | 0x10000000 accept\n\
             \x20   ip6 saddr fe80::/10 udp sport 547 udp dport 546 meta mark set meta mark | 0x10000000 accept\n\
             \x20   icmpv6 type { destination-unreachable, packet-too-big, time-exceeded, parameter-problem, mld-listener-query, nd-router-advert, nd-neighbor-solicit, nd-neighbor-advert } meta mark set meta mark | 0x10000000 accept\n\
             \x20   icmp type { destination-unreachable, time-exceeded, parameter-problem } meta mark set meta mark | 0x10000000 accept\n\
             \x20   iifname @public jump public\n\
             \x20 }\n\
             \n\
             \x20 chain public {\n\
             \x20   tcp flags syn / fin,syn,rst,ack limit rate 6/minute burst 12 packets log prefix \"ronoctua-refused: \" level info\n\
             \x20   drop\n\
             \x20 }\n\
             }\n"
        );
    }

    fn full_wish() -> Wish {
        let mut networks = BTreeMap::new();
        networks.insert(UUID_HOME.to_owned(), Profile::Home);
        let links: Links = [
            ("eth0".to_owned(), UUID_HOME.to_owned()),
            ("wlp2s0".to_owned(), UUID_CAFE.to_owned()),
            (NEVER_PUBLIC.to_owned(), UUID_CAFE.to_owned()),
        ]
        .into();
        let settings = Settings {
            networks,
            log: LogLevel::Chatty,
            rules: vec![
                Rule {
                    source: Some("192.168.1.0/24".into()),
                    ..rule("r-web", Action::Allow, &[(8080, 8080)], &[Profile::Home])
                },
                Rule {
                    protocol: Protocol::Udp,
                    interface: Some("wlp2s0".into()),
                    source: Some("fd00::/8".into()),
                    ..rule(
                        "r-game",
                        Action::Allow,
                        &[(27015, 27015), (27020, 27030)],
                        &[Profile::Home, Profile::Public],
                    )
                },
                rule("r-no-ssh", Action::Deny, &[(22, 22)], &[Profile::Home]),
                rule("r-all-tcp", Action::Deny, &[], &[Profile::Public]),
            ],
            ..Settings::default()
        };
        Wish::of(settings, &links)
    }

    #[test]
    fn a_full_table_marks_what_the_person_allowed() {
        let wish = full_wish();
        assert_eq!(
            wish.public_interfaces,
            ["wlp2s0"],
            "tailscale0 is never public"
        );
        assert_eq!(
            script(&wish),
            "table inet ronoctua\n\
             delete table inet ronoctua\n\
             table inet ronoctua {\n\
             \x20 set public {\n\
             \x20   type ifname\n\
             \x20   elements = { \"wlp2s0\" }\n\
             \x20 }\n\
             \n\
             \x20 chain input {\n\
             \x20   type filter hook input priority filter - 10; policy accept;\n\
             \x20   iif \"lo\" accept\n\
             \x20   ct state established,related accept\n\
             \x20   udp sport 67 udp dport 68 meta mark set meta mark | 0x10000000 accept\n\
             \x20   ip6 saddr fe80::/10 udp sport 547 udp dport 546 meta mark set meta mark | 0x10000000 accept\n\
             \x20   icmpv6 type { destination-unreachable, packet-too-big, time-exceeded, parameter-problem, mld-listener-query, nd-router-advert, nd-neighbor-solicit, nd-neighbor-advert } meta mark set meta mark | 0x10000000 accept\n\
             \x20   icmp type { destination-unreachable, time-exceeded, parameter-problem } meta mark set meta mark | 0x10000000 accept\n\
             \x20   meta mark set meta mark | 0x40000000\n\
             \x20   iifname @public jump public\n\
             \x20   ip saddr 192.168.1.0/24 tcp dport 8080 meta mark set meta mark | 0x10000000 accept comment \"r-web\"\n\
             \x20   iifname \"wlp2s0\" ip6 saddr fd00::/8 udp dport { 27015, 27020-27030 } meta mark set meta mark | 0x10000000 accept comment \"r-game\"\n\
             \x20   tcp dport 22 limit rate 60/minute burst 120 packets log prefix \"ronoctua-refused: \" level info\n\
             \x20   tcp dport 22 drop comment \"r-no-ssh\"\n\
             \x20 }\n\
             \n\
             \x20 chain public {\n\
             \x20   iifname \"wlp2s0\" ip6 saddr fd00::/8 udp dport { 27015, 27020-27030 } meta mark set meta mark | 0x10000000 accept comment \"r-game\"\n\
             \x20   meta l4proto tcp limit rate 60/minute burst 120 packets log prefix \"ronoctua-refused: \" level info\n\
             \x20   meta l4proto tcp drop comment \"r-all-tcp\"\n\
             \x20   tcp flags syn / fin,syn,rst,ack limit rate 60/minute burst 120 packets log prefix \"ronoctua-refused: \" level info\n\
             \x20   drop\n\
             \x20 }\n\
             }\n"
        );
    }

    #[test]
    fn shields_off_and_a_silent_log_say_so() {
        let mut wish = full_wish();
        wish.settings.shields = true;
        wish.settings.log = LogLevel::Off;
        let text = script(&wish);
        let shields = text.find("ct state new drop").expect("shields drop");
        let essentials = text.find("udp sport 67").unwrap();
        let rules = text.find("jump public").unwrap();
        assert!(essentials < shields && shields < rules, "{text}");
        assert!(
            text.contains("meta mark set meta mark | 0x20000000\n"),
            "{text}"
        );
        assert!(
            !text.contains("log prefix"),
            "a silent log logs nothing: {text}"
        );

        wish.settings.enabled = false;
        let off = script(&wish);
        assert!(
            off.contains("ct state new drop"),
            "shields hold with the firewall off"
        );
        assert!(
            off.ends_with("    meta mark set meta mark | 0x10000000 accept\n  }\n}\n"),
            "{off}"
        );
        assert!(
            !off.contains("jump public") && !off.contains("comment \"r-"),
            "{off}"
        );
    }

    #[test]
    fn an_unchecked_rule_never_reaches_the_table() {
        let mut wish = full_wish();
        wish.settings.rules.push(Rule {
            interface: Some("x\" accept; drop".into()),
            ..rule("r-evil", Action::Allow, &[(1, 1)], &[Profile::Home])
        });
        wish.public_interfaces.push("eth0\" }".into());
        let text = script(&wish);
        assert!(
            !text.contains("r-evil") && !text.contains("eth0\" }"),
            "{text}"
        );
    }

    #[test]
    fn the_machines_own_bridges_are_not_its_business() {
        let mut settings = full_wish().settings;
        settings.shields = true;
        let links: Links = [("waydroid0".to_owned(), UUID_CAFE.to_owned())].into();
        let wish = Wish::of(settings, &links).with_local(vec![
            "waydroid0".to_owned(),
            NEVER_PUBLIC.to_owned(),
            "bad name".to_owned(),
        ]);
        assert!(
            wish.public_interfaces.is_empty(),
            "a local bridge is never public"
        );
        let text = script(&wish);
        assert!(
            text.contains("    iif \"lo\" accept\n    iifname { \"waydroid0\" } accept\n"),
            "{text}"
        );
        let shields = text.find("ct state new drop").unwrap();
        assert!(
            text.find("\"waydroid0\"").unwrap() < shields,
            "before the shields"
        );
        assert!(
            !text.contains("iifname { \"waydroid0\", \"tailscale0\""),
            "{text}"
        );
        assert!(!text.contains("bad name"), "{text}");
    }

    #[test]
    fn a_served_network_keeps_its_addresses_under_the_shields() {
        let mut settings = full_wish().settings;
        settings.shields = true;
        let links: Links = [("wlp2s0".to_owned(), UUID_CAFE.to_owned())].into();
        let wish = Wish::of(settings, &links).with_served(vec![
            "wlp2s0".to_owned(),
            NEVER_PUBLIC.to_owned(),
            "bad name".to_owned(),
        ]);
        assert!(wish.public_interfaces.is_empty(), "served is never public");
        let text = script(&wish);
        let served = "    iifname { \"wlp2s0\" } udp dport { 53, 67 } meta mark set meta mark | 0x10000000 accept\n\
                      \x20   iifname { \"wlp2s0\" } tcp dport 53 meta mark set meta mark | 0x10000000 accept\n\
                      \x20   iifname { \"wlp2s0\" } icmpv6 type nd-router-solicit meta mark set meta mark | 0x10000000 accept\n";
        let at = text.find(served).unwrap_or_else(|| panic!("{text}"));
        assert!(
            text.find("icmp type {").unwrap() < at && at < text.find("ct state new drop").unwrap(),
            "after the essentials, before the shields: {text}"
        );
        assert!(
            !text.contains("tailscale0") && !text.contains("bad name"),
            "{text}"
        );
        // Nothing else of the network is opened by it: the person's rules
        // are what a home network gets.
        assert_eq!(text.matches("\"wlp2s0\"").count(), 5, "{text}");
    }

    /// The snapshots, put through the real `nft -c` when this build has it —
    /// in a user and network namespace of its own, where `nft` may talk to a
    /// kernel it cannot harm. A machine that allows no such namespace skips
    /// with a line saying so.
    #[test]
    fn the_snapshots_are_tables_nft_accepts() {
        let Some(nft) = option_env!("RONOCTUA_NFT") else {
            return;
        };
        let Some(unshare) = option_env!("RONOCTUA_UNSHARE") else {
            return;
        };
        let mut shielded = full_wish();
        shielded.settings.shields = true;
        let mut off = full_wish();
        off.settings.enabled = false;
        let probe = tools::run(
            Path::new(unshare),
            &["--user", "--map-root-user", "--net", nft, "--version"],
            None,
        );
        if probe.is_err() {
            eprintln!("no user namespace here: the snapshots were not given to nft");
            return;
        }
        let local = full_wish().with_local(vec!["waydroid0".to_owned()]);
        let mut served = full_wish().with_served(vec!["wlp2s0".to_owned()]);
        served.settings.shields = true;
        for wish in [Wish::default(), full_wish(), shielded, off, local, served] {
            let text = script(&wish);
            let out = tools::run(
                Path::new(unshare),
                &["--user", "--map-root-user", "--net", nft, "-c", "-f", "-"],
                Some(text.as_bytes()),
            );
            assert!(out.is_ok(), "nft refused:\n{text}");
        }
        eprintln!("nft accepted the six tables");
    }

    #[test]
    fn what_network_manager_said_is_kept_plain() {
        let known = Known::of([
            ("3F2A9C1E-0B7D-4E55-9A51-2C8D7E6F1A0B".to_owned(), false),
            (UUID_CAFE.to_owned(), true),
            ("../x".to_owned(), true),
            (String::new(), false),
        ]);
        assert_eq!(
            known.saved,
            [UUID_HOME.to_owned(), UUID_CAFE.to_owned()].into()
        );
        assert_eq!(known.served, [UUID_CAFE.to_owned()].into());
    }

    fn places(dir: &Scratch) -> Places {
        let staged = dir.0.join("staged");
        std::fs::create_dir(&staged).unwrap();
        std::fs::set_permissions(&staged, std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .unwrap();
        Places {
            file: dir.0.join("firewall.json"),
            staged,
            status: dir.0.join("firewall.status"),
            links: dir.0.join("links.json"),
            lock: dir.0.join("lock"),
            local: dir.0.join("local-interfaces"),
            migrated: dir.0.join("networks-migrated"),
            served: dir.0.join("served.json"),
        }
    }

    /// An `nft` that keeps what it was fed: `-c` into `checked`, `-f` into
    /// `applied`.
    fn fake_nft(dir: &Scratch) -> Tools {
        let nft = dir.program(
            "nft",
            &format!(
                "#!/bin/sh\nif [ \"$1\" = -c ]; then out={0}/checked; else out={0}/applied; fi\n\
                 : > \"$out\"\nwhile IFS= read -r line; do printf '%s\\n' \"$line\" >> \"$out\"; done\n",
                dir.0.display()
            ),
        );
        Tools {
            nft: Some(nft),
            ..Tools::default()
        }
    }

    fn uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    #[test]
    fn the_networks_of_before_are_placed_at_home_once() {
        let dir = Scratch::new("fw-first");
        let places = places(&dir);
        let tools = fake_nft(&dir);
        std::fs::write(&places.local, "waydroid0\ntailscale0\n").unwrap();

        // THE BOOT: NetworkManager is not asked, and nothing is placed yet.
        apply_stored(&places, &tools).unwrap();
        let kept = read_settings_at(&places.file).unwrap().unwrap();
        assert_eq!(kept, Settings::default());
        assert_eq!(
            kept.default_profile,
            Profile::Public,
            "new networks are born public"
        );
        assert!(!places.migrated.exists());
        let applied = std::fs::read_to_string(dir.0.join("applied")).unwrap();
        assert_eq!(
            applied,
            script(&Wish::of(kept, &Links::new()).with_local(vec!["waydroid0".to_owned()]))
        );

        // A link while NetworkManager does not answer places nothing.
        link(&places, &tools, "eth0", UUID_HOME, true, None).unwrap();
        assert!(!places.migrated.exists());
        assert!(
            read_settings_at(&places.file)
                .unwrap()
                .unwrap()
                .networks
                .is_empty()
        );

        // THE FIRST ANSWER: every connection stored then is at home, for good.
        let before = Known::of([(UUID_HOME.to_owned(), false)]);
        link(&places, &tools, "eth0", UUID_HOME, true, Some(&before)).unwrap();
        assert!(places.migrated.exists());
        let kept = read_settings_at(&places.file).unwrap().unwrap();
        assert_eq!(kept.networks[UUID_HOME], Profile::Home);

        // A network made after is born public, and stays unplaced.
        let after = Known::of([(UUID_HOME.to_owned(), false), (UUID_CAFE.to_owned(), false)]);
        link(&places, &tools, "wlp2s0", UUID_CAFE, true, Some(&after)).unwrap();
        let kept = read_settings_at(&places.file).unwrap().unwrap();
        assert!(
            !kept.networks.contains_key(UUID_CAFE),
            "placed once, never again"
        );
        let status: Status =
            serde_json::from_slice(&std::fs::read(&places.status).unwrap()).unwrap();
        assert!(status.ok);
        assert_eq!(status.profiles["wlp2s0"], Profile::Public);
        assert_eq!(status.profiles["eth0"], Profile::Home);
        assert_eq!(status.unplaced, [UUID_CAFE]);
        assert!(status.served.is_empty());
        let applied = std::fs::read_to_string(dir.0.join("applied")).unwrap();
        assert!(applied.contains("elements = { \"wlp2s0\" }"), "{applied}");
        link(&places, &tools, "wlp2s0", UUID_CAFE, false, Some(&after)).unwrap();
        let applied = std::fs::read_to_string(dir.0.join("applied")).unwrap();
        assert!(!applied.contains("elements"), "{applied}");

        shields_up(&places, &tools).unwrap();
        assert!(read_settings_at(&places.file).unwrap().unwrap().shields);
        assert!(
            std::fs::read_to_string(dir.0.join("applied"))
                .unwrap()
                .contains("ct state new drop")
        );
    }

    #[test]
    fn a_network_this_machine_serves_is_never_public() {
        let dir = Scratch::new("fw-served");
        let places = places(&dir);
        let tools = fake_nft(&dir);
        std::fs::write(&places.migrated, b"").unwrap();
        // A hotspot: a connection nobody placed, which the default would make
        // public — and NetworkManager says this machine serves it.
        let hotspot = Known::of([(UUID_CAFE.to_owned(), true)]);
        link(&places, &tools, "wlp2s0", UUID_CAFE, true, Some(&hotspot)).unwrap();
        let applied = std::fs::read_to_string(dir.0.join("applied")).unwrap();
        assert!(!applied.contains("elements"), "not public: {applied}");
        assert!(
            applied.contains(
                "    iifname { \"wlp2s0\" } udp dport { 53, 67 } meta mark set meta mark | 0x10000000 accept\n"
            ),
            "{applied}"
        );
        let status: Status =
            serde_json::from_slice(&std::fs::read(&places.status).unwrap()).unwrap();
        assert_eq!(status.served, ["wlp2s0"]);
        assert!(
            status.profiles.is_empty() && status.unplaced.is_empty(),
            "{status:?}"
        );

        // Down, it is served no more; and a link NetworkManager did not
        // describe is not taken for one it serves.
        link(&places, &tools, "wlp2s0", UUID_CAFE, false, Some(&hotspot)).unwrap();
        assert!(read_served(&places).is_empty());
        link(&places, &tools, "wlp2s0", UUID_CAFE, true, None).unwrap();
        assert!(read_served(&places).is_empty());
        let applied = std::fs::read_to_string(dir.0.join("applied")).unwrap();
        assert!(applied.contains("elements = { \"wlp2s0\" }"), "{applied}");
    }

    #[test]
    fn a_staged_wish_is_taken_once_and_only_from_its_owner() {
        let dir = Scratch::new("fw-staged");
        let places = places(&dir);
        let tools = fake_nft(&dir);
        let wish = Settings {
            rules: vec![rule(
                "r-web",
                Action::Allow,
                &[(8080, 8080)],
                &[Profile::Home],
            )],
            ..Settings::default()
        };
        stage_at(&places, &wish).unwrap();
        assert_eq!(
            apply_staged(&places, &tools, uid() + 1, None)
                .unwrap_err()
                .why,
            Why::Invalid,
            "somebody else's folder"
        );
        apply_staged(&places, &tools, uid(), None).unwrap();
        assert_eq!(read_settings_at(&places.file).unwrap(), Some(wish.clone()));
        assert!(!places.wish().exists(), "consumed");
        assert_eq!(
            apply_staged(&places, &tools, uid(), None).unwrap_err().why,
            Why::Unreadable,
            "not replayed"
        );

        // A wish staged before the first placement gets the networks of
        // before — and a network the person already placed keeps its place.
        let placed = Settings {
            networks: [(UUID_CAFE.to_owned(), Profile::Public)].into(),
            ..wish.clone()
        };
        stage_at(&places, &placed).unwrap();
        let known = Known::of([(UUID_HOME.to_owned(), false), (UUID_CAFE.to_owned(), false)]);
        apply_staged(&places, &tools, uid(), Some(&known)).unwrap();
        let kept = read_settings_at(&places.file).unwrap().unwrap();
        assert_eq!(kept.networks[UUID_HOME], Profile::Home);
        assert_eq!(kept.networks[UUID_CAFE], Profile::Public);
        assert!(places.migrated.exists());

        // A link where the wish should be is refused, and not followed.
        let elsewhere = dir.0.join("elsewhere.json");
        std::fs::write(&elsewhere, serde_json::to_vec(&wish).unwrap()).unwrap();
        std::os::unix::fs::symlink(&elsewhere, places.wish()).unwrap();
        assert_eq!(
            apply_staged(&places, &tools, uid(), None).unwrap_err().why,
            Why::Unreadable
        );
        // So is a wish larger than the cap, and one that is not valid.
        std::fs::write(places.wish(), vec![b' '; 70 * 1024]).unwrap();
        assert_eq!(
            apply_staged(&places, &tools, uid(), None).unwrap_err().why,
            Why::Invalid
        );
        std::fs::write(places.wish(), br#"{"format":1}"#).unwrap();
        assert_eq!(
            apply_staged(&places, &tools, uid(), None).unwrap_err().why,
            Why::Invalid
        );
        // A folder others may read is not private.
        std::fs::set_permissions(
            &places.staged,
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        stage_at(&places, &wish).unwrap();
        assert_eq!(
            apply_staged(&places, &tools, uid(), None).unwrap_err().why,
            Why::Invalid
        );
        assert_eq!(
            stage_at(&places, &Settings { format: 9, ..wish })
                .unwrap_err()
                .why,
            Why::Invalid,
            "checked before anybody types a password"
        );
    }

    #[test]
    fn a_table_nft_refuses_leaves_the_file_and_says_so() {
        let dir = Scratch::new("fw-refused");
        let places = places(&dir);
        let tools = Tools {
            nft: Some(dir.program("nft", "#!/bin/sh\nexit 1\n")),
            ..Tools::default()
        };
        assert_eq!(apply_stored(&places, &tools).unwrap_err().why, Why::Failed);
        assert!(!places.file.exists(), "nothing kept that was not applied");
        let status: Status =
            serde_json::from_slice(&std::fs::read(&places.status).unwrap()).unwrap();
        assert!(!status.ok);
        assert_eq!(status.why, Some(Why::Failed));
        std::fs::write(&places.file, b"{ not settings").unwrap();
        assert_eq!(
            apply_stored(&places, &fake_nft(&dir)).unwrap_err().why,
            Why::Invalid
        );
        assert!(
            !dir.0.join("applied").exists(),
            "the kernel's table was not touched"
        );
    }
}
