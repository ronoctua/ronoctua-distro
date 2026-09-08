//! Network — the only crate that talks to NetworkManager.
//!
//! Same boundary as `ronoctua-niri`, and for the same reason: when two
//! parts of the system ask "am I connected?" of different places,
//! the day comes when they disagree and nobody knows which to believe.
//!
//! # What it does NOT do
//!
//! It does not draw, does not translate and does not decide policy. It returns [`Network`] and
//! [`Link`]; what chooses colour, text and on-screen order is the footer, which does not
//! know D-Bus exists.
//!
//! # The password
//!
//! It crosses from here to NetworkManager and **stops**. It is not written to a
//! log, does not come back in any structure, and the saved profile is never read
//! back with `GetSecrets`. A password that does not move around does not leak in the
//! wrong place.

#![forbid(unsafe_code)]

pub mod agent;
pub mod nm;
pub mod secret;
pub mod vpn;

use std::collections::HashMap;

use anyhow::{Context, Result};
use zbus::zvariant::{OwnedObjectPath, Value};

/// Which band the radio is on.
///
/// It is not decoration: it is the difference between 2.4 GHz on a channel contested by seven
/// neighbours and 5 GHz with 160 MHz free. In a actual measurement in this project, the
/// same transfer between the same machines gave **2.9 MB/s on the
/// first and 42 MB/s on the second** — and nothing on screen said which of them the
/// machine was on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Band {
    /// 2.4 GHz.
    Ghz24,
    /// 5 GHz.
    Ghz5,
    /// 6 GHz.
    Ghz6,
    /// A frequency outside the known bands.
    Other,
}

impl Band {
    /// The band of a frequency in MHz.
    ///
    /// The boundaries are those of the actual bands, and not roundings: channel
    /// 165 sits at 5825 MHz and 6 GHz starts at 5925 — two neighbouring numbers
    /// in different bands. Cutting at "5800" would misclassify the top
    /// of 5 GHz.
    #[must_use]
    pub const fn from_frequency(mhz: u32) -> Self {
        match mhz {
            2400..=2500 => Self::Ghz24,
            5150..=5895 => Self::Ghz5,
            5925..=7125 => Self::Ghz6,
            _ => Self::Other,
        }
    }

    /// How many gigahertz, for whoever draws it. `None` for a frequency
    /// outside the known bands.
    ///
    /// A NUMBER AND NOT A STRING, because 2,4 GHz is a DECIMAL and a decimal
    /// belongs to a language: this used to hand back `"2,4 GHz"` with a
    /// Portuguese comma in it, and an otherwise entirely English screen drew
    /// `band  2,4 GHz`. The crate has no language (the same rule that keeps a
    /// buffer from translating); `ronoctua_i18n::Arg::Fraction` is what turns
    /// this into text, in `shell::network`.
    #[must_use]
    pub const fn gigahertz(self) -> Option<f64> {
        match self {
            Self::Ghz24 => Some(2.4),
            Self::Ghz5 => Some(5.0),
            Self::Ghz6 => Some(6.0),
            Self::Other => None,
        }
    }
}

/// How the machine is on the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    /// Cable.
    Wired,
    /// Radio.
    Radio,
}

/// Where a device stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Node connection.
    Disconnected,
    /// Halfway there: authenticating, asking for an IP.
    Connecting,
    /// Active.
    Connected,
}

impl State {
    /// Translates the `NM_DEVICE_STATE` into the three cases the screen tells apart.
    ///
    /// NetworkManager has twelve states; the screen has nothing to do with
    /// ten of them. What matters to whoever is looking is: can I use it yet, is it
    /// trying, or not.
    #[must_use]
    pub const fn from_nm(state: u32) -> Self {
        if state == nm::STATE_ACTIVATED {
            Self::Connected
        } else if state >= nm::STATE_TRANSITION.start && state < nm::STATE_TRANSITION.end {
            Self::Connecting
        } else {
            Self::Disconnected
        }
    }
}

/// A wireless network within range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Network {
    /// The name.
    pub ssid: String,
    /// 0 to 100.
    pub signal: u8,
    /// The band this access point advertises on.
    pub band: Band,
    /// Does it ask for a password?
    pub secured: bool,
    /// WPA3-ONLY: the access point advertises `SAE` and not `PSK`.
    ///
    /// `wpa-psk` covers WPA2 and WPA3 in transition mode, which is what exists
    /// almost everywhere — and it is why this went unnoticed. An access point in
    /// WPA3-only refuses `wpa-psk`, and the failure comes back as
    /// NetworkManager's own message, which says nothing about the mode.
    pub wpa3_only: bool,
    /// There is already a saved profile — connecting will not ask for a password.
    pub saved: bool,
    /// This is the one in use.
    pub active_one: bool,
    /// The ceiling advertised by the access point, in kb/s.
    pub max_rate_kbps: u32,
}

/// A device's link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Cable or radio.
    pub medium: Medium,
    /// `wlan0`, `enp1s0`.
    pub interface: String,
    /// The network in use, when there is one.
    pub network: Option<String>,
    /// Where it stands.
    pub state: State,
    /// The negotiated rate, in kb/s. Zero when there is no link.
    pub rate_kbps: u32,
    /// Signal, 0 to 100. It only makes sense on the radio.
    pub signal: u8,
    /// The band, radio only.
    pub band: Band,
    /// The address this device HAS, with its prefix — `192.168.1.4/24`.
    /// `None` while it has none, which is what a device that is up without a
    /// lease looks like.
    pub address: Option<String>,
    /// The way out through this device.
    pub gateway: Option<String>,
    /// The resolvers this device was given.
    pub dns: Vec<String>,
    /// Is this radio BEING an access point rather than using one? It is the
    /// only way to tell a hotspot from a connection: the active connection's
    /// type is `802-11-wireless` either way.
    pub serving: bool,
}

impl Link {
    /// The rate, in the unit that does not lie about it. `None` with no link.
    ///
    /// One decimal place and not two: the negotiated rate wobbles every
    /// second, and the second place would be noise flickering on screen.
    #[must_use]
    pub fn rate(&self) -> Option<Rate> {
        readable_rate(self.rate_kbps)
    }
}

/// What the daemon believes about the way OUT.
///
/// BELIEVES, and does not measure — with `connectivity.uri` unset the daemon
/// answers [`Way::Full`] for any route at all, so a dead uplink and a captive
/// portal both look like the internet. `nix/modules/network.nix` turns the
/// check on, and the two middle answers are the reason it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Way {
    /// The daemon has not decided yet.
    #[default]
    Unknown,
    /// No way out at all.
    None,
    /// Something answered, and it was not what was asked for: a captive
    /// portal, the hotel's login page.
    Portal,
    /// A route exists and the check did not complete: the uplink is up and
    /// the internet is not reachable through it.
    Limited,
    /// The check went out and came back.
    Full,
}

impl Way {
    /// NetworkManager's own numbers.
    #[must_use]
    pub const fn of(state: u32) -> Self {
        match state {
            1 => Self::None,
            2 => Self::Portal,
            3 => Self::Limited,
            4 => Self::Full,
            _ => Self::Unknown,
        }
    }

    /// The kebab-case word behind `network-way-<id>`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::None => "none",
            Self::Portal => "portal",
            Self::Limited => "limited",
            Self::Full => "full",
        }
    }

    /// Is this a way out that WORKS? Only one of the five is.
    #[must_use]
    pub const fn out(self) -> bool {
        matches!(self, Self::Full)
    }
}

/// A connection that is up — the Wi-Fi, the cable, a VPN.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Active {
    /// Its object path, for the order that takes it down.
    pub at: String,
    /// What the person called the profile.
    pub id: String,
    /// `802-11-wireless`, `vpn`, `wireguard`, …
    pub kind: String,
    /// NetworkManager's own `Vpn` flag — true for the plugin kinds, and
    /// FALSE for WireGuard, which is a device.
    pub vpn: bool,
    /// Is it fully up?
    pub activated: bool,
}

impl Active {
    /// Is this a tunnel, in the sense a person means?
    ///
    /// Two things have to be read, and reading one is the bug: `Vpn` is true
    /// for NetworkManager's plugins (OpenVPN, OpenConnect) and false for
    /// WireGuard, which NM models as a DEVICE — and WireGuard is what most
    /// people mean today.
    #[must_use]
    pub fn tunnel(&self) -> bool {
        self.activated && (self.vpn || self.kind == "wireguard")
    }
}

/// A connection profile the daemon has STORED — a Wi-Fi network connected to
/// before, a VPN imported once.
///
/// It is not a connection: it is the recipe for one, and the difference is
/// what lets the view offer "bring this up" beside "take that down".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Saved {
    /// The profile's object path — what activating it needs.
    pub at: String,
    /// The name the person gave it.
    pub id: String,
    /// `802-11-wireless`, `vpn`, `wireguard`.
    pub kind: String,
    /// The NETWORK this profile joins, when it is a Wi-Fi one.
    ///
    /// # Why the id is not enough
    ///
    /// Because `id` is a NAME somebody typed, and NetworkManager lets it be
    /// anything: a profile renamed to `home` still joins `NET_2G4A1`, and a
    /// second profile for one network is called `MyWifi 1` by the daemon
    /// itself. Matching a saved profile against what the radio can see by its
    /// `id` therefore lists the same network twice — once as a network in
    /// range and once as one out of range, which is a lie about the machine.
    ///
    /// It costs nothing to carry: `saved_profiles` already asks each profile
    /// for its whole settings dictionary, and the SSID is a field of it.
    pub ssid: Option<String>,
}

impl Saved {
    /// Is this profile a tunnel?
    ///
    /// The same two words [`Active::tunnel`] reads, and for its reason:
    /// WireGuard is a `wireguard` connection and never a `vpn` one, so a test
    /// on the one word alone misses half of them.
    #[must_use]
    pub fn tunnel(&self) -> bool {
        self.kind == "vpn" || self.kind == "wireguard"
    }
}

/// The whole of what NetworkManager knows, as of one reading.
///
/// A SNAPSHOT and never a delta, the shape `ronoctua-disks`, `-power` and
/// `-bluetooth` all use: the modeline draws from it with no view open, and a
/// delta would have to be replayed in order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// What the daemon believes about the way out.
    pub way: Way,
    /// Is networking on at all? False is the aeroplane.
    pub networking: bool,
    /// Is the radio on?
    pub radio: bool,
    /// Every link, cable and radio.
    pub links: Vec<Link>,
    /// Every connection that is up.
    pub active: Vec<Active>,
    /// Did the daemon answer at all?
    pub daemon: bool,
    /// The profiles it has stored — every medium, so the view can say which
    /// tunnels exist as well as which are up.
    pub saved: Vec<Saved>,
}

impl Snapshot {
    /// The aeroplane: networking off, by somebody's decision.
    #[must_use]
    pub const fn airplane(&self) -> bool {
        self.daemon && !self.networking
    }

    /// A tunnel that is up, and what it is called.
    #[must_use]
    pub fn tunnel(&self) -> Option<&Active> {
        self.active.iter().find(|a| a.tunnel())
    }

    /// Is there a link that believes it is connected?
    #[must_use]
    pub fn linked(&self) -> bool {
        self.links.iter().any(|l| l.state == State::Connected)
    }

    /// Is something WRONG with the way out — a mark the modeline draws?
    ///
    /// Not "there is no internet": a laptop with the cable out and the radio
    /// off has no way out and nothing is wrong, which is why this asks for a
    /// link that thinks it is connected. What it catches is the case that
    /// looks fine and is not — the hotel's portal, the router with no
    /// uplink — and that is the one worth a glyph.
    #[must_use]
    pub fn adrift(&self) -> bool {
        // `Unknown` IS NOT A FAULT — it is the daemon saying it has not
        // measured yet, which is every machine for the first seconds after
        // boot and any machine whose `Connectivity` read failed. Counting it
        // as adrift put the "no way out" mark on a healthy laptop at every
        // login, which is the one thing this mark must never do.
        self.daemon
            && self.networking
            && self.linked()
            && self.way != Way::Unknown
            && !self.way.out()
    }
}

/// `kb/s` -> `340 Mb/s` or `1,9 Gb/s`.
#[must_use]
pub fn readable_rate(kbps: u32) -> Option<Rate> {
    match kbps {
        // NO LINK IS NO NUMBER, and the door draws the sentence for it: this
        // used to answer an em dash, which is a visual value written into a
        // crate that has no tokens (invariant 9).
        0 => None,
        // Below 1 Mb/s the smaller unit is the only one that does not lie.
        1..=999 => Some(Rate::Kilobits(kbps)),
        1_000..=999_999 => Some(Rate::Megabits(kbps / 1_000)),
        // A DECIMAL, AND THEREFORE A NUMBER: `1,9` and `1.9` are the same
        // rate in two languages, and only the door knows which.
        _ => Some(Rate::Gigabits(f64::from(kbps) / 1_000_000.0)),
    }
}

/// A link's speed, in the unit that does not lie about it.
///
/// THE UNIT IS THE CRATE'S AND THE FORMATTING IS THE DOOR'S: `Mb/s` is the
/// same in every language, a decimal separator is not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rate {
    /// Under a megabit.
    Kilobits(u32),
    /// Whole megabits.
    Megabits(u32),
    /// Gigabits, with one decimal place.
    Gigabits(f64),
}

/// NetworkManager's `psk-flags`, according to whether the person wants to remember.
///
/// - **1, `AGENT_OWNED`** — the profile does not store the password; what hands it over is the
///   agent, opening the TPM blob.
/// - **2, `NOT_SAVED`** — nothing is persisted anywhere. Next time
///   NetworkManager asks again.
///
/// What is NOT used is `0` (`NONE`), which is the world's default and the one that puts
/// `psk=<password>` inside the file.
#[must_use]
pub const fn flags(remember: bool) -> u32 {
    if remember { 1 } else { 2 }
}

/// Is a network secured?
///
/// All three flags, and not just the first: `PRIVACY` says "it has a cipher", but a
/// WPA3-only access point may advertise `PRIVACY` cleared and still require a
/// key, through `RSN`. Looking at only one of them would classify the network as open and
/// the connection would fail without asking for a password — which is the worse of the two errors, because
/// it looks like a radio fault.
#[must_use]
pub const fn secured(flags: u32, wpa: u32, rsn: u32) -> bool {
    flags & nm::AP_PRIVACY != 0 || wpa != 0 || rsn != 0
}

/// Does this access point require `sae` — that is, is it WPA3-ONLY?
///
/// The rule is the ABSENCE of `PSK`, not the presence of `SAE`: an access point
/// in TRANSITION mode advertises both, and it accepts `wpa-psk`. Keying on
/// `SAE` alone would push every transition-mode router onto `sae` and break
/// what works today.
#[must_use]
pub const fn wpa3_only(rsn: u32) -> bool {
    rsn & nm::AP_SEC_KEY_MGMT_SAE != 0 && rsn & nm::AP_SEC_KEY_MGMT_PSK == 0
}

/// `ay` from the bus -> a name.
///
/// The standard allows any byte in an SSID, so this is `from_utf8_lossy` and
/// not `from_utf8`: a router with a malformed name would become `None` and the network
/// would vanish from the list with no explanation.
#[must_use]
fn ssid_name(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The SSID stored in a saved NetworkManager profile.
type Configuration = HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>;

fn profile_ssid(cfg: &Configuration) -> Option<String> {
    let v = cfg.get("802-11-wireless")?.get("ssid")?;
    Vec::<u8>::try_from(v.clone()).ok().map(|b| ssid_name(&b))
}

/// Merges the access points of the same SSID, keeping the strongest.
///
/// A dual-band router advertises the same name on 2.4 and on 5 GHz, and a
/// mesh advertises from each node. Without this the list shows the same network four
/// times, and the person picks the wrong row — which is the weakest, because the
/// order came from the bus.
///
/// # `dedup_by` only removes NEIGHBOURS
///
/// Which is why the first sort is BY NAME. It used to be by signal, with the
/// active one first, and that reads like "strongest first, so `dedup` keeps
/// it" — but two access points of one name are only adjacent under that order
/// when nothing else sorts between them, and something usually does. On a
/// screen with six networks in range, THREE of them were listed twice: the
/// one in use sorts to the top and its own second band stays down among the
/// neighbours, where the deduplication never sees it.
/// Grouping by name first makes the duplicates neighbours by construction;
/// the tie-breaks inside a name are what decides WHICH of them survives, and
/// the display order is the second sort's job.
#[must_use]
pub fn group_by_ssid(mut networks: Vec<Network>) -> Vec<Network> {
    // BY NAME FIRST, then the best of that name: `dedup_by` keeps the first
    // of each run, so the run has to be the name and its head the best.
    networks.sort_by(|a, b| {
        a.ssid
            .cmp(&b.ssid)
            .then(b.active_one.cmp(&a.active_one))
            .then(b.saved.cmp(&a.saved))
            .then(b.signal.cmp(&a.signal))
    });
    networks.dedup_by(|a, b| a.ssid == b.ssid);
    // The final order is the viewer's: the active one on top, then the saved ones,
    // then by signal. Ordering by signal alone would hide the home network
    // below a neighbour's that happens to be closer.
    networks.sort_by(|a, b| {
        b.active_one
            .cmp(&a.active_one)
            .then(b.saved.cmp(&a.saved))
            .then(b.signal.cmp(&a.signal))
            .then(a.ssid.cmp(&b.ssid))
    });
    networks
}

/// A password for a hotspot: twelve characters somebody can read off a
/// screen and type on a phone.
///
/// FROM `/dev/urandom`, and out of an alphabet with no `0`/`O` and no
/// `1`/`l`/`I` in it: the person reading it is typing it on a small keyboard
/// from across a room, and a character nobody can tell apart is a password
/// that does not work for a reason nobody can see.
///
/// # Errors
///
/// The kernel's own randomness was not readable, which is a machine with
/// bigger problems than a hotspot.
pub fn hotspot_password() -> Result<String> {
    use std::io::Read as _;
    const ALPHABET: &[u8] = b"abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    const LENGTH: usize = 12;
    // A BOUNDED READ, and the bound is the whole point.
    //
    // `fs::read` on `/dev/urandom` NEVER RETURNS: it is a character device
    // with no end, so `read_to_end` grows a `Vec` until the kernel kills the
    // process — and this is called on the door's own event-loop thread, so
    // what dies is the footer, and `systemd-oomd` takes the whole cgroup with
    // it (the trap AGENTS.md records). Eight bytes per character is room for
    // the rejection sampling below to throw some away.
    let mut bytes = vec![0u8; LENGTH * 8];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .context("no randomness to make a password out of")?;
    // REJECTION SAMPLING, so every letter is as likely as every other: `%`
    // over a byte favours the front of the alphabet, which is a smaller
    // password than it looks.
    let ceiling = 256 - (256 % ALPHABET.len());
    let out: String = bytes
        .iter()
        .filter(|b| usize::from(**b) < ceiling)
        .take(LENGTH)
        .map(|b| char::from(ALPHABET[usize::from(*b) % ALPHABET.len()]))
        .collect();
    anyhow::ensure!(out.len() == LENGTH, "not enough randomness");
    Ok(out)
}

/// `nmcli`, by PATH and never by name: inside a systemd unit the `PATH` is
/// minimal, and the failure would show up far from its cause.
const NMCLI: &str = match option_env!("RONOCTUA_NMCLI") {
    Some(p) => p,
    None => "nmcli",
};

/// The conversation with NetworkManager.
pub struct Manager {
    connection: zbus::blocking::Connection,
}

impl Manager {
    /// Opens the connection to the system bus.
    ///
    /// # Errors
    ///
    /// Node bus, or no NetworkManager on it.
    pub fn fresh() -> Result<Self> {
        let connection = zbus::blocking::Connection::system()
            .context("no system bus to talk to NetworkManager")?;
        Ok(Self { connection })
    }

    /// The connection itself, for a match rule that is not a proxy's.
    pub const fn bus(&self) -> &zbus::blocking::Connection {
        &self.connection
    }

    fn manager(&self) -> Result<nm::ManagerProxyBlocking<'_>> {
        nm::ManagerProxyBlocking::new(&self.connection).context("NetworkManager is off the bus")
    }

    /// Is the Wi-Fi radio on?
    ///
    /// # Errors
    ///
    /// The bus did not answer.
    pub fn radio_on(&self) -> Result<bool> {
        Ok(self.manager()?.wireless_enabled()?)
    }

    /// Flips the radio's state and returns the new one.
    ///
    /// # Errors
    ///
    /// The bus did not answer, or polkit refused.
    pub fn toggle_radio(&self) -> Result<bool> {
        let g = self.manager()?;
        let fresh = !g.wireless_enabled()?;
        g.set_wireless_enabled(fresh)?;
        Ok(fresh)
    }

    /// Turns the radio on, or off.
    ///
    /// The state WANTED, and not a flip: two doors read the same snapshot and
    /// a flip sent from a stale one turns the radio back to where it already
    /// was — the switch that does nothing when pressed twice by two screens.
    ///
    /// # Errors
    ///
    /// The bus did not answer, or polkit refused.
    pub fn set_radio(&self, on: bool) -> Result<()> {
        self.manager()?.set_wireless_enabled(on)?;
        Ok(())
    }

    /// Asks every radio on the machine to scan.
    ///
    /// It returns at once: the result shows up in the access point list
    /// a few seconds later. The caller has to re-read — and that is why the
    /// view re-reads on its own instead of waiting here.
    ///
    /// # Errors
    ///
    /// The bus did not answer.
    pub fn scan(&self) -> Result<()> {
        for path in self.devices_of_kind(nm::WIFI_KIND)? {
            // A refusal is common and is not an error: NetworkManager rate-limits
            // scanning, and insisting speeds nothing up.
            let _ = self.radio(&path)?.request_scan(HashMap::new());
        }
        Ok(())
    }

    /// A device proxy from the path.
    ///
    /// The path comes in CLONED, and not borrowed. Borrowing would make the proxy
    /// hold the `OwnedObjectPath` for the whole scope, and every loop that
    /// returns the path after querying the proxy would stop compiling —
    /// which is exactly what happened in the first four versions of this
    /// file. A clone of an object path is a short string.
    fn device(&self, path: &OwnedObjectPath) -> Result<nm::DeviceProxyBlocking<'_>> {
        Ok(nm::DeviceProxyBlocking::builder(&self.connection)
            .path(path.clone())?
            .build()?)
    }

    fn radio(&self, path: &OwnedObjectPath) -> Result<nm::RadioProxyBlocking<'_>> {
        Ok(nm::RadioProxyBlocking::builder(&self.connection)
            .path(path.clone())?
            .build()?)
    }

    fn access_point_of(&self, path: &OwnedObjectPath) -> Result<nm::AccessPointProxyBlocking<'_>> {
        Ok(nm::AccessPointProxyBlocking::builder(&self.connection)
            .path(path.clone())?
            .build()?)
    }

    fn profile(&self, path: &OwnedObjectPath) -> Result<nm::ProfileProxyBlocking<'_>> {
        Ok(nm::ProfileProxyBlocking::builder(&self.connection)
            .path(path.clone())?
            .build()?)
    }

    fn devices_of_kind(&self, kind: u32) -> Result<Vec<OwnedObjectPath>> {
        let mut output = Vec::new();
        for path in self.manager()?.get_devices()? {
            if self.device(&path)?.device_type().unwrap_or(0) == kind {
                output.push(path);
            }
        }
        Ok(output)
    }

    /// The SSIDs that already have a saved profile.
    fn saved_ones(&self) -> Result<Vec<String>> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        let mut output = Vec::new();
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            // The saved SSID is `ay`, as on the access point.
            if let Some(name) = profile_ssid(&cfg) {
                output.push(name);
            }
        }
        Ok(output)
    }

    /// The networks within range, grouped by name.
    ///
    /// # Errors
    ///
    /// The bus did not answer.
    pub fn networks(&self) -> Result<Vec<Network>> {
        let saved_ones = self.saved_ones().unwrap_or_default();
        let mut output = Vec::new();

        for path in self.devices_of_kind(nm::WIFI_KIND)? {
            let radio = self.radio(&path)?;
            let active = radio.active_access_point().ok();

            for access_point in radio.get_all_access_points().unwrap_or_default() {
                let p = self.access_point_of(&access_point)?;
                let Ok(bytes) = p.ssid() else { continue };
                // A hidden SSID comes back empty. Showing a blank row would be
                // offering something that cannot be clicked.
                if bytes.is_empty() {
                    continue;
                }
                let ssid = ssid_name(&bytes);
                output.push(Network {
                    signal: p.strength().unwrap_or(0),
                    band: Band::from_frequency(p.frequency().unwrap_or(0)),
                    secured: secured(
                        p.flags().unwrap_or(0),
                        p.wpa_flags().unwrap_or(0),
                        p.rsn_flags().unwrap_or(0),
                    ),
                    wpa3_only: wpa3_only(p.rsn_flags().unwrap_or(0)),
                    saved: saved_ones.contains(&ssid),
                    active_one: active
                        .as_ref()
                        .is_some_and(|a| a.as_str() == access_point.as_str()),
                    max_rate_kbps: p.max_bitrate().unwrap_or(0),
                    ssid,
                });
            }
        }
        Ok(group_by_ssid(output))
    }

    /// The whole of it, in one reading. INFALLIBLE: a daemon that is not
    /// there answers an empty snapshot, which is what the screen draws.
    #[must_use]
    pub fn look(&self) -> Snapshot {
        let Ok(manager) = self.manager() else {
            return Snapshot::default();
        };
        let mut out = Snapshot {
            daemon: true,
            way: Way::of(manager.connectivity().unwrap_or(0)),
            networking: manager.networking_enabled().unwrap_or(true),
            radio: manager.wireless_enabled().unwrap_or(false),
            links: self.links().unwrap_or_default(),
            active: Vec::new(),
            saved: self.saved_profiles().unwrap_or_default(),
        };
        for at in manager.active_connections().unwrap_or_default() {
            let Ok(active) = nm::ActiveProxyBlocking::builder(&self.connection)
                .path(at.clone())
                .and_then(zbus::blocking::proxy::Builder::build)
            else {
                continue;
            };
            out.active.push(Active {
                at: at.as_str().to_owned(),
                id: active.id().unwrap_or_default(),
                kind: active.type_().unwrap_or_default(),
                vpn: active.vpn().unwrap_or(false),
                // 2 is `ACTIVATED`; anything else is on its way somewhere.
                activated: active.state().unwrap_or(0) == 2,
            });
        }
        out
    }

    /// Turns every device off, or back on — the aeroplane.
    ///
    /// # Errors
    ///
    /// The daemon is not there, or it refused. It does NOT refuse for want
    /// of permission: the person is in `networkmanager`, and NM's own polkit
    /// rules answer `yes` to that group for every action this crate uses.
    pub fn set_networking(&self, on: bool) -> Result<()> {
        self.manager()?.enable(on)?;
        Ok(())
    }

    /// Takes one connection down — a VPN, a link.
    ///
    /// # Errors
    ///
    /// The daemon is not there, the path is not one of its own, or it
    /// refused.
    pub fn deactivate(&self, at: &str) -> Result<()> {
        let path = zbus::zvariant::ObjectPath::try_from(at)?;
        self.manager()?.deactivate_connection(&path)?;
        Ok(())
    }

    /// The state of each device, cable and radio.
    ///
    /// # Errors
    ///
    /// The bus did not answer.
    pub fn links(&self) -> Result<Vec<Link>> {
        let mut output = Vec::new();
        for path in self.manager()?.get_devices()? {
            let d = self.device(&path)?;
            let kind = d.device_type().unwrap_or(0);
            let medium = match kind {
                nm::ETHERNET_KIND => Medium::Wired,
                nm::WIFI_KIND => Medium::Radio,
                // Loopback, tunnel, bridge: they are not how the person is on the
                // network, and listing them would fill the screen with useless rows.
                _ => continue,
            };
            let state = State::from_nm(d.state().unwrap_or(0));
            let interface = d.interface().unwrap_or_default();

            let serving = medium == Medium::Radio
                && self
                    .radio(&path)
                    .is_ok_and(|r| r.mode().unwrap_or(0) == nm::RADIO_IS_AP);
            let (network, rate_kbps, signal, band) = if medium == Medium::Radio {
                self.radio_detail(&path)
            } else {
                let c = nm::WiredProxyBlocking::builder(&self.connection)
                    .path(path.clone())?
                    .build()?;
                // `Speed` comes in Mb/s; the rest of the crate speaks kb/s.
                (
                    None,
                    c.speed().unwrap_or(0).saturating_mul(1_000),
                    0,
                    Band::Other,
                )
            };

            let (address, gateway, dns) = self.ip_facts(&path);
            output.push(Link {
                medium,
                interface,
                network,
                state,
                rate_kbps,
                signal,
                band,
                address,
                gateway,
                dns,
                serving,
            });
        }
        Ok(output)
    }

    /// The address, the gateway and the resolvers of one device.
    ///
    /// INFALLIBLE, like every other reading here: a device with no
    /// configuration is the ordinary case (a cable with no carrier, a radio
    /// that has not associated), and it answers three empties rather than an
    /// error the screen would have to word.
    fn ip_facts(&self, device: &OwnedObjectPath) -> (Option<String>, Option<String>, Vec<String>) {
        let empty = (None, None, Vec::new());
        let Ok(d) = self.device(device) else {
            return empty;
        };
        let Ok(at) = d.ip4_config() else {
            return empty;
        };
        // `/` IS HOW NETWORKMANAGER SAYS "NONE", and building a proxy on it
        // fails with a path error — which would be diagnosed as a dead bus.
        // The same trap `radio_detail` carries, for the same reason.
        if at.as_str() == "/" {
            return empty;
        }
        let Ok(ip) = nm::Ip4ProxyBlocking::builder(&self.connection)
            .path(at)
            .and_then(zbus::blocking::proxy::Builder::build)
        else {
            return empty;
        };
        let address = ip
            .address_data()
            .unwrap_or_default()
            .into_iter()
            .find_map(|e| {
                let a = e
                    .get("address")
                    .and_then(|v| String::try_from(v.clone()).ok())?;
                match e.get("prefix").and_then(|v| u32::try_from(v.clone()).ok()) {
                    Some(prefix) => Some(format!("{a}/{prefix}")),
                    None => Some(a),
                }
            });
        let gateway = ip.gateway().ok().filter(|g| !g.is_empty());
        let dns = ip
            .nameserver_data()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|e| {
                e.get("address")
                    .and_then(|v| String::try_from(v.clone()).ok())
            })
            .collect();
        (address, gateway, dns)
    }

    /// Every profile the daemon has stored.
    ///
    /// # Errors
    ///
    /// The bus did not answer. A profile that will not read is SKIPPED and
    /// not an error: one unreadable recipe must not empty the list.
    pub fn saved_profiles(&self) -> Result<Vec<Saved>> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        let mut out = Vec::new();
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            let Some(section) = cfg.get("connection") else {
                continue;
            };
            let text = |field: &str| {
                section
                    .get(field)
                    .and_then(|v| String::try_from(v.clone()).ok())
                    .unwrap_or_default()
            };
            out.push(Saved {
                at: path.as_str().to_owned(),
                id: text("id"),
                kind: text("type"),
                ssid: profile_ssid(&cfg),
            });
        }
        // BY NAME, and not in the daemon's order: that one is the order the
        // profiles were created in, which nobody remembers.
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Brings a file in as a profile, through NetworkManager's OWN importer.
    ///
    /// `nmcli` AND NOT A TRANSLATION OF OUR OWN: a WireGuard file becomes
    /// thirty NetworkManager settings, upstream maintains that mapping, and
    /// a second one here would be a second thing to keep in step with a
    /// format somebody else defines.
    ///
    /// THE PATH GOES IN `argv` AND THE SECRET DOES NOT: the key stays in the
    /// file, which `nmcli` reads itself, and nothing secret crosses a command
    /// line where every process on the machine could read it.
    ///
    /// # Errors
    ///
    /// There is no `nmcli`, or it refused — its own words come back.
    pub fn import(&self, path: &str, kind: vpn::Kind) -> Result<()> {
        let out = std::process::Command::new(NMCLI)
            .args(["connection", "import", "type", kind.id(), "file", path])
            .output()
            .with_context(|| format!("could not run {NMCLI}"))?;
        if out.status.success() {
            return Ok(());
        }
        let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        anyhow::bail!(if said.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        } else {
            said
        })
    }

    /// Brings a stored profile up — a tunnel, a network connected to before.
    ///
    /// The device is left to the daemon (`/`): a profile carries the medium
    /// it belongs to, and naming a device here would be this crate deciding
    /// something NetworkManager already knows.
    ///
    /// # Errors
    ///
    /// The daemon is not there, the path is not one of its profiles, or it
    /// refused.
    pub fn activate(&self, at: &str) -> Result<()> {
        let path = zbus::zvariant::ObjectPath::try_from(at)?;
        let none = zbus::zvariant::ObjectPath::try_from("/")?;
        self.manager()?.activate_connection(&path, &none, &none)?;
        Ok(())
    }

    /// Deletes a stored profile.
    ///
    /// # Errors
    ///
    /// The daemon is not there, the path is not one of its profiles, or it
    /// refused.
    pub fn delete(&self, at: &str) -> Result<()> {
        let path = OwnedObjectPath::try_from(at)?;
        self.profile(&path)?.delete()?;
        Ok(())
    }

    fn radio_detail(&self, path: &OwnedObjectPath) -> (Option<String>, u32, u8, Band) {
        const NO_LINK: (Option<String>, u32, u8, Band) = (None, 0, 0, Band::Other);

        let Ok(radio) = self.radio(path) else {
            return NO_LINK;
        };
        let rate = radio.bitrate().unwrap_or(0);
        let Ok(access_point) = radio.active_access_point() else {
            return (None, rate, 0, Band::Other);
        };
        // `/` is how NetworkManager says "none". Building a proxy on it
        // returns a path error, which would be diagnosed as a bus
        // failure — a "no network" turning into "no D-Bus".
        if access_point.as_str() == "/" {
            return (None, rate, 0, Band::Other);
        }
        let Ok(p) = self.access_point_of(&access_point) else {
            return (None, rate, 0, Band::Other);
        };
        let name = p.ssid().ok().map(|b| ssid_name(&b));
        (
            name,
            rate,
            p.strength().unwrap_or(0),
            Band::from_frequency(p.frequency().unwrap_or(0)),
        )
    }

    /// Connects to a network.
    ///
    /// When `password` is `None`, it tries through the saved profile. When there is a password,
    /// it creates the profile and activates — and the password comes back in nothing.
    ///
    /// # Errors
    ///
    /// There is no radio, the network vanished between the list and the keypress, or
    /// NetworkManager refused (a wrong password lands here).
    pub fn connect(&self, ssid: &str, password: Option<&str>, remember: bool) -> Result<()> {
        // Which key management this SSID needs, from the scan we already have.
        // Unknown network -> `wpa-psk`, which is what worked before this existed.
        let key_management_sae = self
            .networks()
            .unwrap_or_default()
            .iter()
            .find(|n| n.ssid == ssid)
            .is_some_and(|n| n.wpa3_only);
        let g = self.manager()?;
        let device = self
            .devices_of_kind(nm::WIFI_KIND)?
            .into_iter()
            .next()
            .context("this machine has no Wi-Fi radio")?;

        let access_point = self
            .access_point_at(&device, ssid)?
            .context("that network is no longer in range")?;

        if password.is_none()
            && let Some(profile) = self.profile_of(ssid)?
        {
            g.activate_connection(&profile.as_ref(), &device.as_ref(), &access_point.as_ref())?;
            return Ok(());
        }

        let mut connection: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
        connection.insert(
            "connection",
            HashMap::from([
                ("id", Value::from(ssid)),
                ("type", Value::from("802-11-wireless")),
            ]),
        );
        connection.insert(
            "802-11-wireless",
            HashMap::from([("ssid", Value::from(ssid.as_bytes()))]),
        );
        if let Some(s) = password {
            connection.insert(
                "802-11-wireless-security",
                HashMap::from([
                    // WPA3-ONLY ASKS FOR `sae`; everything else takes `wpa-psk`.
                    //
                    // `wpa-psk` covers WPA2 and WPA3 in TRANSITION mode, which is
                    // what exists almost everywhere — and that is exactly why a
                    // WPA3-only access point went unnoticed: it is the rare case,
                    // and the failure comes back as NetworkManager's own message,
                    // which says nothing about the mode.
                    //
                    // The choice reads the access point's `RsnFlags` — see
                    // `wpa3_only`, whose rule is the ABSENCE of `PSK` and not the
                    // presence of `SAE`.
                    (
                        "key-mgmt",
                        Value::from(if key_management_sae { "sae" } else { "wpa-psk" }),
                    ),
                    // THE PASSWORD GOES IN THE CALL ANYWAY — and this is what makes
                    // the whole design fit.
                    //
                    // The flags below govern only what is PERSISTED. NetworkManager
                    // uses what it receives here to connect
                    // now, and at the next boot it asks the agent. Without that,
                    // the agent would have to receive the password from the footer through some
                    // channel — one more surface for it to pass through.
                    ("psk", Value::from(s)),
                    ("psk-flags", Value::from(flags(remember))),
                ]),
            );
        }

        g.add_and_activate_connection(connection, &device.as_ref(), &access_point.as_ref())?;

        // SEAL AFTER CONNECTING, and not before.
        //
        // Storing a password that does not work is storing rubbish that, at the next
        // boot, the agent hands over and the radio refuses. Here the connection has already
        // been accepted by NetworkManager.
        if remember && let Some(s) = password {
            self.vault()?.store(ssid, s)?;
        }
        Ok(())
    }

    fn access_point_at(
        &self,
        device: &OwnedObjectPath,
        ssid: &str,
    ) -> Result<Option<OwnedObjectPath>> {
        for access_point in self
            .radio(device)?
            .get_all_access_points()
            .unwrap_or_default()
        {
            if self
                .access_point_of(&access_point)?
                .ssid()
                .is_ok_and(|b| ssid_name(&b) == ssid)
            {
                return Ok(Some(access_point));
            }
        }
        Ok(None)
    }

    fn profile_of(&self, ssid: &str) -> Result<Option<OwnedObjectPath>> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            if profile_ssid(&cfg).as_deref() == Some(ssid) {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    /// Turns the radio into an access point the other things in the room can
    /// join.
    ///
    /// # What it costs, and why the row says so
    ///
    /// A LAPTOP WITH ONE RADIO CANNOT DO BOTH. The card leaves the network it
    /// was on and starts answering as an access point, so the machine sharing
    /// its connection is sharing the CABLE, and a machine with no cable is
    /// sharing nothing. That is a sentence on the row, not a surprise
    /// afterwards.
    ///
    /// # The password IS stored, and that is not the Wi-Fi rule
    ///
    /// Joining somebody's network keeps nothing on disk (`psk-flags=2`, the
    /// TPM agent). This is the other direction: NetworkManager has to hand
    /// the key to `wpa_supplicant` on every activation, so it lives in the
    /// profile — root-owned, `0600`, in `/etc/NetworkManager`. A hotspot
    /// whose password nobody could give out is not one.
    ///
    /// # Errors
    ///
    /// There is no radio, or NetworkManager refused.
    pub fn hotspot(&self, ssid: &str, password: &str) -> Result<()> {
        let device = self
            .devices_of_kind(nm::WIFI_KIND)?
            .into_iter()
            .next()
            .context("this machine has no Wi-Fi radio")?;
        let mut connection: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
        connection.insert(
            "connection",
            HashMap::from([
                ("id", Value::from(ssid)),
                ("type", Value::from("802-11-wireless")),
                // NOT ON ITS OWN, EVER: a hotspot that came back at every
                // boot would be a machine quietly answering to the room.
                ("autoconnect", Value::from(false)),
            ]),
        );
        connection.insert(
            "802-11-wireless",
            HashMap::from([
                ("ssid", Value::from(ssid.as_bytes())),
                ("mode", Value::from("ap")),
                // 2.4 GHz: it is what every phone and every old thing in the
                // room can see, and range beats speed for something one turns
                // on to hand somebody a connection.
                ("band", Value::from("bg")),
            ]),
        );
        connection.insert(
            "802-11-wireless-security",
            HashMap::from([
                ("key-mgmt", Value::from("wpa-psk")),
                ("psk", Value::from(password)),
            ]),
        );
        connection.insert("ipv4", HashMap::from([("method", Value::from("shared"))]));
        connection.insert("ipv6", HashMap::from([("method", Value::from("ignore"))]));
        let none = zbus::zvariant::ObjectPath::try_from("/")?;
        self.manager()?
            .add_and_activate_connection(connection, &device.as_ref(), &none)?;
        Ok(())
    }

    /// Takes down whatever is active on the first device of the given medium.
    ///
    /// # Errors
    ///
    /// There is no device of that medium, or the bus refused.
    pub fn disconnect(&self, medium: Medium) -> Result<()> {
        let kind = match medium {
            Medium::Wired => nm::ETHERNET_KIND,
            Medium::Radio => nm::WIFI_KIND,
        };
        let path = self
            .devices_of_kind(kind)?
            .into_iter()
            .next()
            .context("no device of that kind")?;
        self.device(&path)?.disconnect()?;
        Ok(())
    }

    fn vault(&self) -> Result<nm::VaultProxyBlocking<'_>> {
        nm::VaultProxyBlocking::new(&self.connection).context("the network agent is not on the bus")
    }

    /// Is there a password sealed in the TPM for this network?
    ///
    /// Different from "saved in NetworkManager": a profile can exist with no
    /// password stored at all, and in that case connecting still asks for typing.
    #[must_use]
    pub fn has_stored_password(&self, ssid: &str) -> bool {
        self.vault().is_ok_and(|c| c.has(ssid).unwrap_or(false))
    }

    /// Deletes a network's saved profile.
    ///
    /// # Errors
    ///
    /// There is no profile with that name, or polkit refused.
    pub fn forget(&self, ssid: &str) -> Result<()> {
        let path = self
            .profile_of(ssid)?
            .context("there is no saved profile by that name")?;
        self.profile(&path)?.delete()?;
        // THE BLOB GOES WITH IT. The agent deletes it too on `DeleteSecrets`, and having
        // both paths is on purpose: the agent's covers whoever deleted the
        // profile from outside (`nmcli`), and this one covers the case of the agent being
        // down. Deleting what no longer exists is not an error.
        if let Ok(c) = self.vault() {
            let _ = c.forget(ssid);
        }
        Ok(())
    }
}

/// Watches the radio until a scan FINISHES, then calls `on_scan` — once.
///
/// # Why a thread, and why not a timer
///
/// `RequestScan` returns at once and the radio answers seconds later. Blocking
/// the footer's loop would freeze keyboard, painting and niri events for those
/// seconds — the same `systemd-run --scope` trap AGENTS.md records. And polling
/// on a timer would spend a frame on a list that has not moved.
///
/// `LastScan` changes exactly when there is something new to read. The thread
/// blocks on that property's change stream and says so once.
///
/// # Errors
///
/// There is no bus, or the machine has no radio.
pub fn watch<F>(on_change: F) -> Result<()>
where
    F: Fn(Snapshot) + Send + 'static,
{
    let manager = Manager::fresh()?;
    std::thread::Builder::new()
        .name("net-watch".to_owned())
        .spawn(move || {
            // THE MATCH RULE BEFORE THE FIRST READING, the lesson the power
            // and the disks crates both carry: a cable plugged during the
            // reading would announce itself before the rule landed.
            let rule = (|| {
                Ok::<_, zbus::Error>(
                    zbus::MatchRule::builder()
                        .msg_type(zbus::message::Type::Signal)
                        .sender(nm::SERVICE)?
                        .interface("org.freedesktop.DBus.Properties")?
                        .build(),
                )
            })();
            let Ok(rule) = rule else {
                tracing::warn!("no NetworkManager rule; the marks will not reread");
                return;
            };
            let Ok(messages) =
                zbus::blocking::MessageIterator::for_match_rule(rule, manager.bus(), None)
            else {
                tracing::warn!("no NetworkManager stream; the marks will not reread");
                return;
            };
            on_change(manager.look());
            let mut last = std::time::Instant::now();
            for _ in messages {
                // A FLOOR BETWEEN READINGS. NetworkManager talks constantly
                // while a link comes up — every property of every device —
                // and the modeline has five marks to draw.
                if last.elapsed() < std::time::Duration::from_millis(250) {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                }
                last = std::time::Instant::now();
                on_change(manager.look());
            }
            tracing::info!("the NetworkManager stream ended; no automatic reread");
        })
        .context("could not start the network thread")?;
    Ok(())
}

/// The scan's own watch — `LastScan` changes when there is something new.
pub fn watch_scan<F>(on_scan: F) -> Result<()>
where
    F: Fn() + Send + 'static,
{
    let manager = Manager::fresh()?;
    let device = manager
        .devices_of_kind(nm::WIFI_KIND)?
        .into_iter()
        .next()
        .context("this machine has no Wi-Fi radio")?;

    std::thread::Builder::new()
        .name("ronoctua-scan".to_owned())
        .spawn(move || {
            let Ok(radio) = manager.radio(&device) else {
                tracing::warn!("no radio proxy; the list will not reread on its own");
                return;
            };
            // A BLOCKING stream: the thread exists precisely to wait here.
            let mut changes = radio.receive_last_scan_changed();
            while changes.next().is_some() {
                tracing::debug!("the radio finished a scan");
                on_scan();
            }
            // The stream ending means NetworkManager went away. Saying so beats
            // a thread that quietly stops working.
            tracing::info!("the scan stream ended; no automatic reread");
        })
        .context("could not start the scan thread")?;
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn the_aeroplane_is_networking_off_and_nothing_else() {
        let mut s = Snapshot {
            daemon: true,
            networking: true,
            ..Snapshot::default()
        };
        assert!(!s.airplane());
        s.networking = false;
        assert!(s.airplane());
        // A DAEMON THAT IS NOT THERE is not an aeroplane: it is a machine
        // with nothing to say, and the mark would be a lie.
        s.daemon = false;
        assert!(!s.airplane());
    }

    #[test]
    fn a_hotspot_password_comes_back_at_all() {
        // THE DEFECT THIS EXISTS FOR: `fs::read` on `/dev/urandom` never
        // returns — a character device has no end — so this function grew a
        // `Vec` until the kernel killed the process, ON THE DOOR'S OWN EVENT
        // LOOP. The test that would have found it is this one: call it.
        let word = hotspot_password().expect("no randomness");
        assert_eq!(word.chars().count(), 12);
        // AND IT IS TYPEABLE FROM ACROSS A ROOM: no `0`/`O`, no `1`/`l`/`I`.
        for c in word.chars() {
            assert!(c.is_ascii_alphanumeric(), "{word}");
            assert!(!"0O1lI".contains(c), "an ambiguous character: {word}");
        }
        // AND IT IS NOT THE SAME PASSWORD EVERY TIME.
        assert_ne!(word, hotspot_password().unwrap());
    }

    #[test]
    fn a_stored_tunnel_is_either_word() {
        // WIREGUARD IS NEVER A `vpn` CONNECTION. NetworkManager gives it a
        // type of its own, and a test on the one word would have listed half
        // the tunnels this machine can have.
        let of = |kind: &str| Saved {
            at: "/s/1".to_owned(),
            id: "a tunnel".to_owned(),
            kind: kind.to_owned(),
            ssid: None,
        };
        assert!(of("vpn").tunnel());
        assert!(of("wireguard").tunnel());
        assert!(!of("802-11-wireless").tunnel());
        assert!(!of("802-3-ethernet").tunnel());
    }

    #[test]
    fn not_yet_measured_is_not_a_fault() {
        // THE MARK THIS PREVENTS: `Unknown` is what NetworkManager answers
        // before its first connectivity check and what a failed read falls
        // back to (`Way::of` takes `unwrap_or(0)`), so counting it as adrift
        // drew "no way out" on a healthy machine at every login.
        let waiting = Snapshot {
            daemon: true,
            networking: true,
            way: Way::Unknown,
            links: vec![Link {
                medium: Medium::Radio,
                interface: "wlan0".to_owned(),
                network: Some("a network".to_owned()),
                state: State::Connected,
                rate_kbps: 0,
                signal: 60,
                band: Band::Other,
                address: None,
                gateway: None,
                dns: Vec::new(),
                serving: false,
            }],
            active: Vec::new(),
            radio: true,
            saved: Vec::new(),
        };
        assert!(!waiting.adrift(), "a mark before the first measurement");
    }

    #[test]
    fn adrift_is_a_link_that_leads_nowhere() {
        let linked = |way: Way| Snapshot {
            daemon: true,
            networking: true,
            way,
            links: vec![Link {
                medium: Medium::Radio,
                interface: "wlan0".to_owned(),
                network: Some("a network".to_owned()),
                state: State::Connected,
                rate_kbps: 0,
                signal: 60,
                band: Band::Other,
                address: None,
                gateway: None,
                dns: Vec::new(),
                serving: false,
            }],
            active: Vec::new(),
            radio: true,
            saved: Vec::new(),
        };
        // The two that look fine and are not.
        assert!(linked(Way::Portal).adrift());
        assert!(linked(Way::Limited).adrift());
        assert!(linked(Way::None).adrift());
        // The one that works.
        assert!(!linked(Way::Full).adrift());
        // AND NOTHING CONNECTED IS NOT ADRIFT: a laptop with the cable out
        // and the radio off has no way out and nothing is wrong.
        let mut alone = linked(Way::None);
        alone.links.clear();
        assert!(!alone.adrift());
        // Nor is the aeroplane, which is somebody's decision.
        let mut off = linked(Way::None);
        off.networking = false;
        assert!(!off.adrift());
    }

    #[test]
    fn a_tunnel_is_the_plugin_flag_or_wireguard() {
        let up = |kind: &str, vpn: bool| Active {
            at: "/a/1".to_owned(),
            id: "a tunnel".to_owned(),
            kind: kind.to_owned(),
            vpn,
            activated: true,
        };
        // NetworkManager's own plugins say so.
        assert!(up("vpn", true).tunnel());
        // WireGuard does NOT — it is a device, and it is what most people
        // mean today. Reading only `Vpn` was the bug this pins.
        assert!(up("wireguard", false).tunnel());
        // The Wi-Fi carrying it is not a tunnel.
        assert!(!up("802-11-wireless", false).tunnel());
        // And one that is still coming up is not up.
        let mut coming = up("wireguard", false);
        coming.activated = false;
        assert!(!coming.tunnel());
    }

    #[test]
    fn the_ways_out_are_the_daemons_own_numbers() {
        for (n, way) in [
            (0, Way::Unknown),
            (1, Way::None),
            (2, Way::Portal),
            (3, Way::Limited),
            (4, Way::Full),
            (99, Way::Unknown),
        ] {
            assert_eq!(Way::of(n), way, "{n}");
        }
        assert!(Way::Full.out());
        for way in [Way::Unknown, Way::None, Way::Portal, Way::Limited] {
            assert!(!way.out(), "{way:?} is not a way out");
        }
        let mut seen = std::collections::BTreeSet::new();
        for way in [
            Way::Unknown,
            Way::None,
            Way::Portal,
            Way::Limited,
            Way::Full,
        ] {
            assert!(seen.insert(way.id()), "{way:?} shares an id");
        }
    }

    #[test]
    fn transition_mode_stays_on_wpa_psk() {
        // The case that exists almost everywhere: the access point advertises
        // BOTH, and accepts `wpa-psk`. Keying on the presence of `SAE` would
        // push every one of these onto `sae` and break what works today.
        let both = nm::AP_SEC_KEY_MGMT_PSK | nm::AP_SEC_KEY_MGMT_SAE;
        assert!(!wpa3_only(both), "the transition has to stay on wpa-psk");
    }

    #[test]
    fn wpa3_only_is_the_absence_of_psk() {
        assert!(wpa3_only(nm::AP_SEC_KEY_MGMT_SAE));
        // WPA2: no SAE at all.
        assert!(!wpa3_only(nm::AP_SEC_KEY_MGMT_PSK));
        // An open network has no RSN flags.
        assert!(!wpa3_only(0));
    }
    use super::*;

    #[test]
    fn the_band_comes_from_the_frequency() {
        assert_eq!(Band::from_frequency(2412), Band::Ghz24);
        assert_eq!(Band::from_frequency(5180), Band::Ghz5);
        assert_eq!(Band::from_frequency(6115), Band::Ghz6);
    }

    #[test]
    fn the_top_of_5ghz_does_not_become_6ghz() {
        // Channel 165 sits at 5825 MHz and 6 GHz starts at 5925: two
        // neighbouring numbers, different bands. A rounded cut at
        // "5800" would classify the top of 5 GHz as 6 GHz — and the screen
        // would say the wrong band precisely where the difference matters.
        assert_eq!(Band::from_frequency(5825), Band::Ghz5);
        assert_eq!(Band::from_frequency(5955), Band::Ghz6);
    }

    #[test]
    fn wpa3_with_no_privacy_flag_is_still_secured() {
        // The case that motivated looking at all three: a WPA3-only access point may
        // advertise `PRIVACY` cleared. Looking only at the first flag, the
        // network would be classified as open, the connection would go without a password and
        // would fail looking like a radio fault.
        assert!(secured(0, 0, 0x1)); // RSN only
        assert!(secured(0, 0x1, 0)); // WPA only
        assert!(secured(nm::AP_PRIVACY, 0, 0));
        assert!(!secured(0, 0, 0), "a genuinely open network");
    }

    #[test]
    fn the_nm_state_becomes_the_three_the_screen_tells_apart() {
        assert_eq!(State::from_nm(100), State::Connected);
        assert_eq!(State::from_nm(50), State::Connecting);
        assert_eq!(State::from_nm(30), State::Connecting);
        assert_eq!(State::from_nm(20), State::Disconnected);
        // 110 is `DEACTIVATING`: no longer usable.
        assert_eq!(State::from_nm(110), State::Disconnected);
    }

    #[test]
    fn the_same_network_on_two_bands_becomes_one_row() {
        // The actual case: a dual-band router advertises the same name on 2.4
        // and on 5 GHz. Without grouping, the list shows the network twice and the
        // person picks the top row — which came in the bus's order,
        // and not in the signal's.
        let networks = vec![
            Network {
                ssid: "a house".into(),
                signal: 40,
                band: Band::Ghz24,
                secured: true,
                wpa3_only: false,
                saved: true,
                active_one: false,
                max_rate_kbps: 144_000,
            },
            Network {
                ssid: "a house".into(),
                signal: 85,
                band: Band::Ghz5,
                secured: true,
                wpa3_only: false,
                saved: true,
                active_one: false,
                max_rate_kbps: 1_200_000,
            },
        ];
        let output = group_by_ssid(networks);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].signal, 85, "it kept the weaker point");
        assert_eq!(output[0].band, Band::Ghz5);
    }

    #[test]
    fn one_name_survives_however_the_others_sort_between_its_two_points() {
        // THE DEFECT THIS PREVENTS, and it was on the owner's screen: the
        // active point of a name sorts to the top and its twin stays down
        // among the neighbours, so `dedup_by` — which only removes
        // NEIGHBOURS — never sees the pair. Three names, six points, and one
        // of each has to come back.
        let point = |ssid: &str, signal: u8, active: bool| Network {
            ssid: ssid.into(),
            signal,
            band: if signal > 50 { Band::Ghz5 } else { Band::Ghz24 },
            secured: true,
            wpa3_only: false,
            saved: active,
            active_one: active,
            max_rate_kbps: 0,
        };
        let networks = vec![
            point("a house", 90, true),
            point("a neighbour", 70, false),
            point("a street", 60, false),
            point("a house", 40, false),
            point("a neighbour", 30, false),
            point("a street", 20, false),
        ];
        let names: Vec<_> = group_by_ssid(networks)
            .into_iter()
            .map(|r| (r.ssid, r.signal))
            .collect();
        assert_eq!(
            names,
            [
                ("a house".to_owned(), 90),
                ("a neighbour".to_owned(), 70),
                ("a street".to_owned(), 60)
            ]
        );
    }

    #[test]
    fn the_active_comes_before_the_saved_and_the_saved_before_the_strong_neighbour() {
        let networks = vec![
            Network {
                ssid: "a neighbour".into(),
                signal: 99,
                band: Band::Ghz24,
                secured: true,
                wpa3_only: false,
                saved: false,
                active_one: false,
                max_rate_kbps: 0,
            },
            Network {
                ssid: "a saved one".into(),
                signal: 30,
                band: Band::Ghz5,
                secured: true,
                wpa3_only: false,
                saved: true,
                active_one: false,
                max_rate_kbps: 0,
            },
            Network {
                ssid: "current".into(),
                signal: 10,
                band: Band::Ghz5,
                secured: true,
                wpa3_only: false,
                saved: true,
                active_one: true,
                max_rate_kbps: 0,
            },
        ];
        let names: Vec<_> = group_by_ssid(networks)
            .into_iter()
            .map(|r| r.ssid)
            .collect();
        assert_eq!(names, ["current", "a saved one", "a neighbour"]);
    }

    #[test]
    fn the_flags_never_give_back_zero() {
        // ZERO is the world's default, and it is the one that puts `psk=<password>` inside the
        // profile file — exactly what this work exists to
        // eliminate. Neither of the two paths may fall into it.
        assert_eq!(flags(true), 1, "remembering has to be AGENT_OWNED");
        assert_eq!(flags(false), 2, "not remembering has to be NOT_SAVED");
        for remember in [true, false] {
            assert_ne!(
                flags(remember),
                0,
                "it fell back to the default that writes to disk"
            );
        }
    }

    #[test]
    fn the_rate_changes_unit_without_losing_the_magnitude() {
        // NUMBERS AND UNITS, never a rendered decimal: `1,9` and `1.9` are
        // the same rate in two languages, and this crate has no language.
        assert_eq!(readable_rate(0), None, "no link is no number");
        assert_eq!(readable_rate(540), Some(Rate::Kilobits(540)));
        assert_eq!(readable_rate(340_000), Some(Rate::Megabits(340)));
        let Some(Rate::Gigabits(fast)) = readable_rate(1_921_500) else {
            panic!("a gigabit link came back in the wrong unit");
        };
        assert!((fast - 1.9215).abs() < 0.001, "{fast}");
        // AND ONE DECIMAL PLACE IS THE DOOR'S ROUNDING, not a truncation
        // here: 2 401 900 is 2,4 and not 2,401900.
        let Some(Rate::Gigabits(other)) = readable_rate(2_401_900) else {
            panic!("wrong unit");
        };
        assert!((other - 2.4019).abs() < 0.001, "{other}");
    }

    #[test]
    fn a_band_is_a_number_and_not_a_language() {
        // IT USED TO ANSWER `"2,4 GHz"` — with a Portuguese comma — and an
        // otherwise entirely English screen drew it that way.
        assert_eq!(Band::Ghz24.gigahertz(), Some(2.4));
        assert_eq!(Band::Ghz5.gigahertz(), Some(5.0));
        assert_eq!(Band::Other.gigahertz(), None);
    }
}
