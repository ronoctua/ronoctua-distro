//! Bringing a tunnel in from a file — and looking at it first.
//!
//! # Why a preview, and why it is not a formality
//!
//! A `.conf` or an `.ovpn` is somebody else's decision about where this
//! machine's traffic goes. `AllowedIPs = 0.0.0.0/0` sends ALL of it; a `DNS`
//! line moves every name this machine looks up. Importing without reading is
//! how a person ends up wondering why their bank is slow, and the answer is
//! in a file they never opened. So the import is two steps: this module says
//! what the file would do, and only then does anybody press a key.
//!
//! # The secret never comes back
//!
//! `PrivateKey`, `PresharedKey` and an inline `<key>` block are read past and
//! never carried out of here. What goes on screen is what the tunnel DOES,
//! and a private key on a screen is a private key in a screenshot.
//!
//! # Pure
//!
//! Text goes in and facts come out. What touches the disk is the caller, and
//! what imports is `nmcli` — NetworkManager's own importer, because the
//! translation from a WireGuard file to a NetworkManager profile is thirty
//! settings and upstream already maintains it.

/// Which kind of tunnel a file describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A WireGuard `.conf`.
    WireGuard,
    /// An OpenVPN `.ovpn`.
    OpenVpn,
}

impl Kind {
    /// What `nmcli connection import type` wants.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::WireGuard => "wireguard",
            Self::OpenVpn => "openvpn",
        }
    }
}

/// One peer of a WireGuard tunnel.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Peer {
    /// Where it is.
    pub endpoint: Option<String>,
    /// What goes through it. `0.0.0.0/0` is EVERYTHING, and the screen says
    /// so in words rather than leaving the reader to know that.
    pub allowed: Vec<String>,
}

impl Peer {
    /// Would this peer take every packet this machine sends?
    #[must_use]
    pub fn takes_everything(&self) -> bool {
        self.allowed
            .iter()
            .any(|a| a.trim() == "0.0.0.0/0" || a.trim() == "::/0")
    }
}

/// What a file would do, once it is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    /// Which kind it is.
    pub kind: Kind,
    /// What it would be called — the file's own name, without its suffix.
    pub name: String,
    /// The address this machine would take.
    pub address: Option<String>,
    /// The resolvers it would use. Moving these moves every name this
    /// machine looks up, which is the fact worth reading before importing.
    pub dns: Vec<String>,
    /// Its peers, or its remotes.
    pub peers: Vec<Peer>,
    /// Does it need a password typed after importing?
    pub needs_secret: bool,
}

impl Preview {
    /// Would this tunnel take every packet?
    #[must_use]
    pub fn takes_everything(&self) -> bool {
        self.peers.iter().any(Peer::takes_everything)
    }
}

/// The name a file would be imported under: its own, without the suffix.
#[must_use]
pub fn name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_owned())
}

/// Reads a file's text. `None` when it is neither kind — which is a refusal
/// with a reason, and not an import that fails later inside `nmcli`.
#[must_use]
pub fn preview(path: &str, text: &str) -> Option<Preview> {
    let name = name_of(path);
    if text.contains("[Interface]") {
        return Some(wireguard(name, text));
    }
    // AN OPENVPN FILE HAS NO HEADING. `remote` is the one line every client
    // profile has, and `client` is the other half of the pair.
    if text.lines().any(|l| l.trim_start().starts_with("remote ")) {
        return Some(openvpn(name, text));
    }
    None
}

/// A WireGuard `.conf`, section by section.
fn wireguard(name: String, text: &str) -> Preview {
    let mut out = Preview {
        kind: Kind::WireGuard,
        name,
        address: None,
        dns: Vec::new(),
        peers: Vec::new(),
        // WIREGUARD HAS NO PASSWORD: the key is in the file, and what
        // NetworkManager needs afterwards is nothing.
        needs_secret: false,
    };
    let mut peer: Option<Peer> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.eq_ignore_ascii_case("[peer]") {
            if let Some(p) = peer.take() {
                out.peers.push(p);
            }
            peer = Some(Peer::default());
            continue;
        }
        if line.eq_ignore_ascii_case("[interface]") {
            if let Some(p) = peer.take() {
                out.peers.push(p);
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        // THE SECRETS ARE READ PAST. See the module's header.
        match (key.to_ascii_lowercase().as_str(), peer.as_mut()) {
            ("address", None) => out.address = Some(value.to_owned()),
            ("dns", None) => {
                out.dns = value.split(',').map(|d| d.trim().to_owned()).collect();
            }
            ("endpoint", Some(p)) => p.endpoint = Some(value.to_owned()),
            ("allowedips", Some(p)) => {
                p.allowed = value.split(',').map(|a| a.trim().to_owned()).collect();
            }
            _ => {}
        }
    }
    if let Some(p) = peer.take() {
        out.peers.push(p);
    }
    out
}

/// An OpenVPN `.ovpn`: its remotes, and whether it will ask for a password.
///
/// `redirect-gateway` IS THE FILE'S, NOT A PEER'S. It used to be attached to
/// the last remote read so far — and a file that puts its options before its
/// remotes, which is the ordinary commercial layout, then had NO peer to
/// attach it to. The preview drew no warning, and the person imported a
/// tunnel that takes every packet without the one sentence this module exists
/// to show.
fn openvpn(name: String, text: &str) -> Preview {
    let mut out = Preview {
        kind: Kind::OpenVpn,
        name,
        address: None,
        dns: Vec::new(),
        peers: Vec::new(),
        // `auth-user-pass` WITH NO FILE after it means "ask the person", and
        // that is the profile whose password has to be typed once and stored.
        needs_secret: text
            .lines()
            .any(|l| l.trim() == "auth-user-pass" || l.trim().starts_with("auth-user-pass ")),
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("remote ") {
            let mut fields = rest.split_whitespace();
            let host = fields.next().unwrap_or_default();
            let port = fields.next().unwrap_or_default();
            out.peers.push(Peer {
                endpoint: Some(if port.is_empty() {
                    host.to_owned()
                } else {
                    format!("{host}:{port}")
                }),
                allowed: Vec::new(),
            });
        }
        if let Some(rest) = line.strip_prefix("dhcp-option DNS ") {
            out.dns.push(rest.trim().to_owned());
        }
    }
    // `redirect-gateway` IS OPENVPN'S `0.0.0.0/0`, and it is the same fact
    // under another name: everything goes through the tunnel. Read AFTER the
    // whole file, because a line's place in it says nothing.
    if text
        .lines()
        .any(|l| l.trim_start().starts_with("redirect-gateway"))
    {
        // ON EVERY REMOTE, and on a peer of its own when the file named none:
        // the fact belongs to the FILE, and a preview that could not draw it
        // for want of a peer is the defect this comes out of.
        if out.peers.is_empty() {
            out.peers.push(Peer::default());
        }
        for peer in &mut out.peers {
            peer.allowed.push("0.0.0.0/0".to_owned());
        }
    }
    out
}

/// The prefix every profile the VPN register writes carries
/// (`nix/modules/vpn.nix`, `nix/scripts/vpn-sync.py`). A connection whose id
/// begins with it is one of the person's registered locations, which the view
/// lists apart — on a shelf of its own, with a readable label and a filter —
/// instead of a hundred flat rows under the tunnels heading.
pub const LOCATION_PREFIX: &str = "ronoctua-vpn-";

/// A registered location, as its connection id names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The country code, lower-case (`us`); or the whole rest of the id when it
    /// is not the `<country>-<city>` shape.
    pub country: String,
    /// The city code, lower-case (`nyc`); empty when the id had none.
    pub city: String,
}

impl Location {
    /// The label a row shows: the codes upper-cased (`US · NYC`), or the
    /// country alone when there is no city.
    #[must_use]
    pub fn label(&self) -> String {
        if self.city.is_empty() {
            self.country.to_uppercase()
        } else {
            format!(
                "{} · {}",
                self.country.to_uppercase(),
                self.city.to_uppercase()
            )
        }
    }

    /// What a typed filter is matched against, lower-case: the codes joined, so
    /// `us`, `nyc` and `us-nyc` all find `ronoctua-vpn-us-nyc`.
    #[must_use]
    pub fn haystack(&self) -> String {
        if self.city.is_empty() {
            self.country.clone()
        } else {
            format!("{}-{}", self.country, self.city)
        }
    }
}

/// The registered location a connection id names, or `None` when the id is not
/// one of the register's ([`LOCATION_PREFIX`]). Pure.
#[must_use]
pub fn location_of(id: &str) -> Option<Location> {
    let rest = id.strip_prefix(LOCATION_PREFIX)?;
    if rest.is_empty() {
        return None;
    }
    // `<country>-<city>` is the ordinary shape; anything else is kept whole as
    // the country, so a generically-named profile still shows and filters.
    Some(match rest.split_once('-') {
        Some((cc, city)) if !cc.is_empty() && !city.is_empty() => Location {
            country: cc.to_owned(),
            city: city.to_owned(),
        },
        _ => Location {
            country: rest.to_owned(),
            city: String::new(),
        },
    })
}

// ---------------------------------------------------------------------------
// REGISTERING A FOLDER, from the shell — the same work `nix/scripts/vpn-import.py`
// does, in Rust, so the person never opens a terminal. The view reads a folder
// of `.conf` files, asks the key once, and stages the two files the install
// unit takes (`ronoctua_security::vpn`): the key sealed in the TPM, the public
// locations turned into NetworkManager profiles by the sync. The key leaves here
// as ONE `0600` file and nothing else.
// ---------------------------------------------------------------------------

/// The staging the install unit takes from — the SAME three names the Rust root
/// half (`ronoctua_security::vpn`) and `nix/scripts/vpn-import.py` use. Kept in
/// step by the comment: they are in another crate, so no test ties them.
pub const STAGING_DIR: &str = "/run/ronoctua-vpn";
/// The staged key's file, `0600`, the bare base64.
pub const KEY_FILE: &str = "private.key";
/// The staged locations' file.
pub const LOCATIONS_FILE: &str = "locations.json";

/// The interface IPv6 address that anchors the `::/0` route into the tunnel —
/// the same literal `nix/scripts/vpn-import.py` uses. A provider that does not
/// egress IPv6 black-holes it in the tunnel, which is no leak.
const ULA6: &str = "fdce:1a2b:3c4d::2/64";

/// Whether `text` is a WireGuard private key: 32 bytes as 44 base64 characters
/// ending in `=`. The same check the root half makes
/// (`ronoctua_security::vpn::is_wireguard_key`). Pure.
#[must_use]
pub fn is_wireguard_key(text: &str) -> bool {
    let b = text.as_bytes();
    b.len() == 44
        && b.ends_with(b"=")
        && b[..43]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/'))
}

/// The public facts a `.conf` carries that a registration needs — including the
/// PUBLIC key, which the preview drops (it keeps only what a tunnel DOES). The
/// private key is read past.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfFacts {
    /// The interface's IPv4 address, or `None`.
    pub address4: Option<String>,
    /// Its IPv6 address, or `None` (the provider's simplified files have none).
    pub address6: Option<String>,
    /// The resolvers it names.
    pub dns: Vec<String>,
    /// The peer's public key.
    pub public_key: String,
    /// What the peer takes.
    pub allowed: Vec<String>,
    /// Where the peer is.
    pub endpoint: String,
}

/// Parses a WireGuard `.conf` for registration. `None` when it is not a
/// WireGuard file, or names no peer public key and endpoint. The `PrivateKey`
/// line is read past. Pure.
#[must_use]
pub fn parse_conf(text: &str) -> Option<ConfFacts> {
    if !text.contains("[Interface]") {
        return None;
    }
    let mut section = "";
    let mut address4 = None;
    let mut address6 = None;
    let mut dns = Vec::new();
    let mut public_key = None;
    let mut allowed = Vec::new();
    let mut endpoint = None;
    for line in text.lines() {
        let line = line.trim();
        if line.eq_ignore_ascii_case("[interface]") {
            section = "interface";
            continue;
        }
        if line.eq_ignore_ascii_case("[peer]") {
            section = "peer";
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
        match (section, key.as_str()) {
            ("interface", "address") => {
                for part in value.split(',').map(str::trim) {
                    if part.contains(':') {
                        address6 = Some(part.to_owned());
                    } else if !part.is_empty() {
                        address4 = Some(part.to_owned());
                    }
                }
            }
            ("interface", "dns") => {
                dns = value
                    .split(',')
                    .map(|d| d.trim().to_owned())
                    .filter(|d| !d.is_empty())
                    .collect();
            }
            ("peer", "publickey") => public_key = Some(value.to_owned()),
            ("peer", "allowedips") => {
                allowed = value
                    .split(',')
                    .map(|a| a.trim().to_owned())
                    .filter(|a| !a.is_empty())
                    .collect();
            }
            ("peer", "endpoint") => endpoint = Some(value.to_owned()),
            _ => {}
        }
    }
    Some(ConfFacts {
        address4,
        address6,
        dns,
        public_key: public_key?,
        allowed,
        endpoint: endpoint?,
    })
}

/// JSON-escapes `s` into `out`, quotes included. Correct for any string; the
/// values a `.conf` carries are ASCII, so it rarely does more than wrap them.
fn json_str(out: &mut String, s: &str) {
    use std::fmt::Write as _;
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A JSON array of strings.
fn json_array(out: &mut String, items: &[String]) {
    out.push('[');
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_str(out, it);
    }
    out.push(']');
}

/// The `allowed-ips` a location registers with: what the file named, plus the
/// two defaults so the IP cannot leak (the sync still gates IPv6 on the module's
/// option). IPv4 first.
fn allowed_with_defaults(allowed: &[String]) -> Vec<String> {
    let mut out: Vec<String> = allowed.to_vec();
    for net in ["0.0.0.0/0", "::/0"] {
        if !out.iter().any(|a| a == net) {
            out.push(net.to_owned());
        }
    }
    out
}

/// The locations document (`format` 1, the shape the sync and the install unit
/// read) for a set of `.conf` files, given as `(file-name, contents)`. Returns
/// the JSON text and the file names it SKIPPED (not a WireGuard file, or no
/// peer). Pure.
#[must_use]
pub fn locations_json(confs: &[(String, String)]) -> (String, Vec<String>) {
    let mut out =
        String::from("{\n  \"format\": 1,\n  \"provider\": \"imported\",\n  \"locations\": [");
    let mut skipped = Vec::new();
    let mut first = true;
    for (name, contents) in confs {
        let id = name.strip_suffix(".conf").unwrap_or(name).to_owned();
        let Some(facts) = parse_conf(contents) else {
            skipped.push(name.clone());
            continue;
        };
        let label = location_of(&format!("{LOCATION_PREFIX}{id}"))
            .map_or_else(|| id.clone(), |l| l.label());
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str("\n    {\"id\": ");
        json_str(&mut out, &id);
        out.push_str(", \"name\": ");
        json_str(&mut out, &label);
        out.push_str(", \"endpoint\": ");
        json_str(&mut out, &facts.endpoint);
        out.push_str(", \"publicKey\": ");
        json_str(&mut out, &facts.public_key);
        out.push_str(", \"dns\": ");
        json_array(&mut out, &facts.dns);
        out.push_str(", \"address4\": ");
        json_str(
            &mut out,
            facts.address4.as_deref().unwrap_or("10.14.0.2/16"),
        );
        out.push_str(", \"address6\": ");
        json_str(&mut out, facts.address6.as_deref().unwrap_or(ULA6));
        out.push_str(", \"allowedIPs\": ");
        json_array(&mut out, &allowed_with_defaults(&facts.allowed));
        out.push('}');
    }
    out.push_str("\n  ]\n}\n");
    (out, skipped)
}

/// What a registration staged — for the message the view shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    /// How many servers were registered.
    pub count: usize,
    /// The file names SKIPPED (not a WireGuard file).
    pub skipped: Vec<String>,
}

/// Why a folder could not be registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterWhy {
    /// The key is not a WireGuard key.
    NotAKey,
    /// The folder has no WireGuard server to register.
    NoServers,
    /// The folder could not be read, or a staged file not written.
    Unreadable,
}

/// Reads every `.conf` in `folder`, builds the public locations, and stages the
/// key (`0600`) and the locations (`0600`) in `staging_dir` for the install unit
/// (`ronoctua_security::vpn`), which seals the key in the TPM; the sync then
/// writes the profiles. The key leaves here as ONE `0600` file and nothing else.
///
/// # Errors
///
/// [`RegisterWhy::NotAKey`] when the key is not one; [`RegisterWhy::NoServers`]
/// when the folder has no WireGuard server; [`RegisterWhy::Unreadable`] when the
/// folder cannot be read or a staged file not written.
pub fn stage_registration(
    folder: &std::path::Path,
    key: &str,
    staging_dir: &std::path::Path,
) -> Result<Registered, RegisterWhy> {
    let key = key.trim();
    if !is_wireguard_key(key) {
        return Err(RegisterWhy::NotAKey);
    }
    let mut confs: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(folder).map_err(|_| RegisterWhy::Unreadable)? {
        let path = entry.map_err(|_| RegisterWhy::Unreadable)?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("conf") {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if let Ok(contents) = std::fs::read_to_string(&path) {
            confs.push((name, contents));
        }
    }
    confs.sort();
    let (json, skipped) = locations_json(&confs);
    let count = confs.len() - skipped.len();
    if count == 0 {
        return Err(RegisterWhy::NoServers);
    }
    write_private(&staging_dir.join(KEY_FILE), format!("{key}\n").as_bytes())
        .map_err(|_| RegisterWhy::Unreadable)?;
    write_private(&staging_dir.join(LOCATIONS_FILE), json.as_bytes())
        .map_err(|_| RegisterWhy::Unreadable)?;
    Ok(Registered { count, skipped })
}

/// Writes `0600` from creation (the key must never exist readable).
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(bytes)
}

// ---------------------------------------------------------------------------
// THE RUNTIME SETTINGS — IPv6, the kill switch, the DNS — changed from the
// shell WITHOUT a rebuild. A `key=value` file, not JSON: the shell stages a
// copy, the sync persists it and applies it (regenerating the profiles, adding
// or removing the kill-switch table). None are secret, so the live file is
// world-readable and the view reads it to draw the toggles
// (`nix/modules/vpn.nix`, `nix/scripts/vpn-sync.py`).
// ---------------------------------------------------------------------------

/// The live settings the sync reads and applies.
pub const SETTINGS: &str = "/var/lib/ronoctua-vpn/settings.conf";
/// Its file name, in the state folder.
pub const SETTINGS_FILE: &str = "settings.conf";
/// Where the shell stages a settings change for the sync to take.
pub const SETTINGS_STAGING: &str = "/run/ronoctua-vpn/settings.conf";

/// The VPN's runtime settings. None are secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// Route IPv6 into the tunnel so the real address cannot leak.
    pub ipv6: bool,
    /// Block ALL traffic that is not through a tunnel (the hard kill switch).
    pub kill_switch: bool,
    /// Resolvers for every location, or empty for each location's own.
    pub dns: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ipv6: true,
            kill_switch: false,
            dns: Vec::new(),
        }
    }
}

impl Settings {
    /// The `key=value` text a settings file holds — what the shell stages and
    /// the sync persists.
    #[must_use]
    pub fn text(&self) -> String {
        format!(
            "ipv6={}\nkill-switch={}\ndns={}\n",
            u8::from(self.ipv6),
            u8::from(self.kill_switch),
            self.dns.join(","),
        )
    }
}

/// The settings a `key=value` text holds; a missing or malformed key keeps its
/// default, so a short or absent file reads as the defaults. Pure.
#[must_use]
pub fn settings_of(text: &str) -> Settings {
    let mut s = Settings::default();
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        match key.trim() {
            "ipv6" => s.ipv6 = value.trim() == "1",
            "kill-switch" => s.kill_switch = value.trim() == "1",
            "dns" => {
                s.dns = value
                    .split(',')
                    .map(|d| d.trim().to_owned())
                    .filter(|d| !d.is_empty())
                    .collect();
            }
            _ => {}
        }
    }
    s
}

/// The settings a file holds, or the defaults when it is absent or unreadable.
#[must_use]
pub fn read_settings(path: &std::path::Path) -> Settings {
    std::fs::read_to_string(path).map_or_else(|_| Settings::default(), |t| settings_of(&t))
}

/// Stages a settings change for the sync to take — `0600` in the person's own
/// runtime folder. The sync persists it to [`SETTINGS`] and applies it.
///
/// # Errors
///
/// The staging folder is not there, or the file could not be written.
pub fn stage_settings(settings: &Settings, staging: &std::path::Path) -> std::io::Result<()> {
    write_private(staging, settings.text().as_bytes())
}

/// Where the shell stages the ids of locations to forget — one per line, for
/// the sync to drop from the locations and prune.
pub const FORGET_STAGING: &str = "/run/ronoctua-vpn/forget";

/// Stages a location to be forgotten: APPENDS its id (one per line) to the
/// forget file, so forgetting two before the sync runs drops both. The id is
/// the short one the locations carry (the `ronoctua-vpn-` prefix taken off).
///
/// The key is NOT touched: it is the whole set's, not one location's. The sync
/// removes the id from the locations and NetworkManager loses the profile on
/// the prune.
///
/// # Errors
///
/// The staging folder is not there, or the line could not be appended.
pub fn stage_forget(id: &str, staging: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let short = id.strip_prefix(LOCATION_PREFIX).unwrap_or(id);
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .mode(0o600)
        .open(staging)?;
    writeln!(f, "{short}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A WireGuard file's shape, with nothing real in it (invariant 13): the
    /// key is not a key, the address is documentation's own range.
    const WIREGUARD: &str = "\
[Interface]
PrivateKey = 0000000000000000000000000000000000000000000=
Address = 198.51.100.2/32
DNS = 198.51.100.1, 198.51.100.9

[Peer]
PublicKey = 1111111111111111111111111111111111111111111=
AllowedIPs = 0.0.0.0/0, ::/0
Endpoint = vpn.example.invalid:51820
";

    const OPENVPN: &str = "\
client
dev tun
proto udp
remote vpn.example.invalid 1194
redirect-gateway def1
dhcp-option DNS 198.51.100.1
auth-user-pass
";

    #[test]
    fn a_registered_location_is_read_from_its_connection_id() {
        let us = location_of("ronoctua-vpn-us-nyc").unwrap();
        assert_eq!(us.country, "us");
        assert_eq!(us.city, "nyc");
        assert_eq!(us.label(), "US · NYC");
        assert_eq!(us.haystack(), "us-nyc");
        // A generic name with no city still shows and filters.
        let office = location_of("ronoctua-vpn-office").unwrap();
        assert_eq!(office.label(), "OFFICE");
        assert_eq!(office.haystack(), "office");
        // Not the register's: left to the ordinary tunnel list.
        assert_eq!(location_of("my-work-vpn"), None);
        assert_eq!(location_of("ronoctua-vpn-"), None);
    }

    /// A key of the shape, with nothing real in it (invariant 13).
    const A_KEY: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA0=";

    #[test]
    fn a_key_is_44_base64_characters() {
        assert!(is_wireguard_key(A_KEY));
        assert!(!is_wireguard_key("short="));
        assert!(!is_wireguard_key(&"A".repeat(44)), "no trailing =");
        assert!(!is_wireguard_key(&format!("{A_KEY}\n")), "a line break");
    }

    #[test]
    fn a_conf_parses_its_public_facts_and_reads_the_key_past() {
        let f = parse_conf(WIREGUARD).unwrap();
        assert_eq!(f.public_key, "1111111111111111111111111111111111111111111=");
        assert_eq!(f.endpoint, "vpn.example.invalid:51820");
        assert_eq!(f.address4.as_deref(), Some("198.51.100.2/32"));
        assert_eq!(f.dns, ["198.51.100.1", "198.51.100.9"]);
        assert!(f.allowed.iter().any(|a| a == "0.0.0.0/0"));
        // AND THE PRIVATE KEY IS NOWHERE IN THE FACTS.
        assert!(!format!("{f:?}").contains("0000000000"));
        // Not a WireGuard file, or an interface with no peer endpoint.
        assert_eq!(parse_conf("just text"), None);
        assert_eq!(parse_conf("[Interface]\nAddress = 10.0.0.2/32\n"), None);
    }

    #[test]
    fn the_locations_json_carries_every_field_and_adds_ipv6() {
        let (json, skipped) = locations_json(&[("us-nyc.conf".to_owned(), WIREGUARD.to_owned())]);
        assert!(skipped.is_empty());
        assert!(json.contains("\"format\": 1"));
        assert!(json.contains("\"id\": \"us-nyc\""));
        assert!(json.contains("\"name\": \"US · NYC\""));
        assert!(json.contains("\"publicKey\": \"1111"));
        assert!(json.contains("\"endpoint\": \"vpn.example.invalid:51820\""));
        assert!(json.contains("::/0"), "IPv6 is added so the IP cannot leak");
        // A file that is not a WireGuard conf is skipped, and named.
        let (_, skipped) = locations_json(&[("bad.conf".to_owned(), "nope".to_owned())]);
        assert_eq!(skipped, ["bad.conf"]);
    }

    #[test]
    fn registering_a_folder_stages_the_key_0600_and_the_locations() {
        use std::os::unix::fs::PermissionsExt as _;
        let root = std::env::temp_dir().join(format!("ronoctua-vpn-reg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let folder = root.join("confs");
        let staging = root.join("staging");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(folder.join("us-nyc.conf"), WIREGUARD).unwrap();
        std::fs::write(folder.join("notes.txt"), "ignored").unwrap();

        let reg = stage_registration(&folder, A_KEY, &staging).unwrap();
        assert_eq!(reg.count, 1);
        assert!(reg.skipped.is_empty());
        // The key is staged `0600`, the bare base64.
        let key_path = staging.join(KEY_FILE);
        assert_eq!(
            std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_to_string(&key_path).unwrap().trim(), A_KEY);
        let loc = std::fs::read_to_string(staging.join(LOCATIONS_FILE)).unwrap();
        assert!(loc.contains("us-nyc") && loc.contains("\"format\": 1"));
        // A bad key, or a folder with no server, stages nothing.
        assert_eq!(
            stage_registration(&folder, "nope", &staging),
            Err(RegisterWhy::NotAKey)
        );
        assert_eq!(
            stage_registration(&staging, A_KEY, &staging),
            Err(RegisterWhy::NoServers)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_runtime_settings_round_trip_and_default() {
        let d = Settings::default();
        assert!(d.ipv6 && !d.kill_switch && d.dns.is_empty());
        let s = Settings {
            ipv6: false,
            kill_switch: true,
            dns: vec!["1.1.1.1".to_owned(), "1.0.0.1".to_owned()],
        };
        assert_eq!(settings_of(&s.text()), s);
        // A short, absent or malformed file keeps the defaults.
        assert_eq!(settings_of(""), Settings::default());
        assert!(!settings_of("ipv6=0\n").ipv6);
        assert_eq!(
            settings_of("garbage\nkill-switch=1"),
            Settings {
                ipv6: true,
                kill_switch: true,
                dns: vec![],
            }
        );
    }

    #[test]
    fn a_wireguard_file_says_what_it_would_do_and_never_its_key() {
        let p = preview("/tmp/an-office.conf", WIREGUARD).unwrap();
        assert_eq!(p.kind, Kind::WireGuard);
        assert_eq!(
            p.name, "an-office",
            "the name is the file's, without the suffix"
        );
        assert_eq!(p.address.as_deref(), Some("198.51.100.2/32"));
        assert_eq!(p.dns, ["198.51.100.1", "198.51.100.9"]);
        assert_eq!(p.peers.len(), 1);
        assert_eq!(
            p.peers[0].endpoint.as_deref(),
            Some("vpn.example.invalid:51820")
        );
        // THE FACT WORTH READING BEFORE IMPORTING: everything goes through it.
        assert!(p.takes_everything());
        assert!(!p.needs_secret);
        // AND THE SECRET IS NOWHERE IN WHAT CAME BACK.
        let drawn = format!("{p:?}");
        assert!(
            !drawn.contains("0000000000"),
            "the private key came out of the preview: {drawn}"
        );
    }

    #[test]
    fn an_openvpn_file_is_read_by_its_remote_and_its_password() {
        let p = preview("/tmp/a-house.ovpn", OPENVPN).unwrap();
        assert_eq!(p.kind, Kind::OpenVpn);
        assert_eq!(p.name, "a-house");
        assert_eq!(
            p.peers[0].endpoint.as_deref(),
            Some("vpn.example.invalid:1194")
        );
        // `redirect-gateway` IS THE SAME FACT under another name.
        assert!(p.takes_everything());
        assert_eq!(p.dns, ["198.51.100.1"]);
        // AND IT WILL ASK FOR A PASSWORD, which is why the row after the
        // import says so.
        assert!(p.needs_secret);
    }

    #[test]
    fn the_options_block_may_come_before_the_remotes() {
        // THE COMMON COMMERCIAL LAYOUT, and the defect it found: the warning
        // was attached to the last remote READ SO FAR, so a file that puts
        // its options first had no peer to attach it to and the preview drew
        // no warning at all — for a tunnel that takes every packet.
        let first = "\
client
dev tun
redirect-gateway def1
dhcp-option DNS 198.51.100.1
remote vpn.example.invalid 1194
remote vpn2.example.invalid 1194
";
        let p = preview("/tmp/a-house.ovpn", first).unwrap();
        assert!(p.takes_everything(), "the warning was lost");
        assert_eq!(p.peers.len(), 2);
        assert!(p.peers.iter().all(Peer::takes_everything));

        // AND A FILE WITH NO REMOTE AT ALL still says what it would do.
        let bare = "client\nredirect-gateway def1\nremote a.invalid 1\n";
        assert!(preview("/tmp/b.ovpn", bare).unwrap().takes_everything());
    }

    #[test]
    fn a_file_that_is_neither_is_refused_with_a_reason() {
        // AND NOT IMPORTED AND LEFT TO FAIL INSIDE `nmcli`, whose message is
        // about a parser nobody here wrote.
        assert!(preview("/tmp/notes.txt", "a shopping list\nmilk\n").is_none());
        assert!(preview("/tmp/empty.conf", "").is_none());
    }

    #[test]
    fn a_tunnel_that_takes_only_its_own_network_says_so_by_not_saying_it() {
        let split = "\
[Interface]
Address = 198.51.100.2/32

[Peer]
AllowedIPs = 10.0.0.0/8
Endpoint = vpn.example.invalid:51820
";
        let p = preview("/tmp/split.conf", split).unwrap();
        assert!(!p.takes_everything());
        assert!(p.dns.is_empty());
    }
}
