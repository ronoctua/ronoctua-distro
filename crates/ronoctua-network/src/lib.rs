//! Network — the only crate that talks to NetworkManager.
//!
//! Same boundary as `ronoctua-scroll`, and for the same reason: when two
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
pub mod privacy;
pub mod refusal;
pub mod secret;
mod stored;
pub mod vpn;

pub use stored::Stored;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

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

/// A refusal the screen says in a sentence of its own — a closed set, carried
/// inside the `anyhow::Error` the call returns, so a door can tell it from
/// the daemon's own words (`downcast_ref`) and say it in the person's
/// language. The `Display` is English, for the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// NetworkManager has no Bluetooth device for that phone: it is not
    /// paired, or it does not offer its connection (PAN).
    NoPhone,
    /// There is no VPN profile by that name — it went between the import
    /// and the answer.
    NoVpn,
    /// The profile by that name is not a VPN's (it has no `vpn` section).
    NotVpn,
}

impl Refusal {
    /// Every refusal, for the test that demands a sentence for each.
    pub const ALL: [Self; 3] = [Self::NoPhone, Self::NoVpn, Self::NotVpn];
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoPhone => {
                "NetworkManager has no Bluetooth device by that address: the phone is not paired, or does not share its connection"
            }
            Self::NoVpn => "there is no VPN profile by that name",
            Self::NotVpn => "the profile has no vpn section",
        })
    }
}

impl std::error::Error for Refusal {}

/// How the machine is on the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Medium {
    /// Cable.
    Wired,
    /// Radio.
    Radio,
    /// A phone's connection, shared over Bluetooth (PAN) — NetworkManager's
    /// `bnep` interface. See [`Manager::tether`] for who owns it.
    Phone,
}

impl Medium {
    /// Which link carries the traffic when several are up: the cable, then
    /// the radio, then a phone — the order of NetworkManager's own route
    /// metrics for the three.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Wired => 0,
            Self::Radio => 1,
            Self::Phone => 2,
        }
    }
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

/// NetworkManager's `NMMetered` as an answer: yes and guessed yes are
/// metered, no and guessed no are not, and unknown (or a number the daemon
/// does not define) is no answer at all.
#[must_use]
pub const fn metered_of(value: u32) -> Option<bool> {
    match value {
        1 | 3 => Some(true),
        2 | 4 => Some(false),
        _ => None,
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

/// `psk-flags` 0, `NONE`: NetworkManager keeps the password itself, inside
/// the profile. Only for a profile held in memory
/// ([`Manager::join_for_this_boot`]), which has no file on disk: on disk it
/// is the world's default, `psk=<password>` inside the file — what the TPM's
/// agent exists to avoid.
pub const KEPT_BY_THE_DAEMON: u32 = 0;

/// `psk-flags` 1, `AGENT_OWNED`: the profile does not hold the password; the
/// agent hands it over when NetworkManager asks, opening the TPM blob. What a
/// remembered network is on disk ([`Manager::remember`]).
///
/// # And `NOT_SAVED` (2) is not used
///
/// NetworkManager does not take a password marked 1 or 2 from the call that
/// adds the profile: it asks the agents for it when the activation needs it
/// (measured on NetworkManager 1.56 — `no secrets: No agents were available
/// for this request`, before the radio tried a key). A join with either one
/// hangs on an agent that has the password at that moment, and none has it:
/// the TPM's agent answers only with a blob, and a password just typed has
/// none yet. So a password typed is joined with [`KEPT_BY_THE_DAEMON`], in
/// memory, and the profile goes to disk as 1 only once it is up and the blob
/// is sealed.
pub const KEPT_BY_THE_AGENT: u32 = 1;

/// How long a join is tried. NetworkManager ends an activation on its own —
/// a refused key at its association timeout (25 s), no address at its DHCP
/// timeout (45 s) — and a REFUSED key is the verdict at once; an activation
/// that ended for any other reason is asked again until this passes
/// ([`Manager::joining`]). A right password is slow after a wrong one: the
/// supplicant holds the network back after a failure, and the first
/// activation ended in the association timeout before the hold did — the
/// right password read as refused, 30 s after it was typed (the installer's
/// Wi-Fi test, 2026-10-23). That ending clears the hold, so the activation
/// asked again is heard; a second one that never associated is the network
/// not letting the radio in ([`follow`]).
pub const JOIN_MOST_S: u64 = 90;

/// No access point: NetworkManager chooses one the profile fits
/// ([`Manager::radio_toward`] says why a join never names one).
const ANY_ACCESS_POINT: zbus::zvariant::ObjectPath<'static> =
    zbus::zvariant::ObjectPath::from_static_str_unchecked("/");

/// The settings of a Wi-Fi profile for `ssid`, with `password` under
/// `psk_flags`.
fn wifi_settings<'a>(
    ssid: &'a str,
    password: Option<&'a str>,
    sae: bool,
    psk_flags: u32,
) -> HashMap<&'static str, HashMap<&'static str, Value<'a>>> {
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
                ("key-mgmt", Value::from(if sae { "sae" } else { "wpa-psk" })),
                ("psk", Value::from(s)),
                ("psk-flags", Value::from(psk_flags)),
            ]),
        );
    }
    connection
}

/// A profile as [`Manager::remember`] writes it to disk: the same, with the
/// password left to the agent — `psk-flags` [`KEPT_BY_THE_AGENT`] and no
/// `psk`. `None` for a profile with no key (an open network): there is no
/// password to remember. Pure.
#[must_use]
pub fn left_to_the_agent(mut cfg: Configuration) -> Option<Configuration> {
    let security = cfg.get_mut("802-11-wireless-security")?;
    security.remove("psk");
    security.insert("psk-flags".to_owned(), OwnedValue::from(KEPT_BY_THE_AGENT));
    Some(cfg)
}

/// What [`Manager::connect`] started: the profile it brought up and the
/// activation under way — which a try again replaces ([`follow`]).
#[derive(Debug, Clone)]
pub struct Joining {
    profile: OwnedObjectPath,
    /// Whether the join MADE the profile — the only kind
    /// [`Manager::abandon`] deletes and [`Manager::remember`] keeps: a
    /// saved profile brought up is the person's.
    made: bool,
    /// The attempt, shared by every copy of the join: a door's copy follows
    /// the activation a try again put in the place of the first.
    attempt: Arc<Mutex<Attempt>>,
}

/// Where a join's attempt is.
#[derive(Debug)]
struct Attempt {
    /// The activation under way.
    active: OwnedObjectPath,
    /// Whether a follower watches the radio ([`follow`]): it judges an
    /// activation that ended. A join with none — a test's, or one whose
    /// follower could not start — takes the ending as its verdict.
    followed: bool,
    /// The verdict, once settled: by the follower, or by
    /// [`Manager::abandon`].
    settled: Option<Joined>,
}

impl Joining {
    /// A join at these paths — what a door's test holds, with no daemon.
    ///
    /// # Errors
    ///
    /// A path that is not an object path.
    pub fn at(profile: &str, active: &str, made: bool) -> Result<Self> {
        Ok(Self::of(
            OwnedObjectPath::try_from(profile)?,
            OwnedObjectPath::try_from(active)?,
            made,
        ))
    }

    fn of(profile: OwnedObjectPath, active: OwnedObjectPath, made: bool) -> Self {
        Self {
            profile,
            made,
            attempt: Arc::new(Mutex::new(Attempt {
                active,
                followed: false,
                settled: None,
            })),
        }
    }

    fn attempt(&self) -> MutexGuard<'_, Attempt> {
        self.attempt.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Settles the join, once: a later verdict does not replace the first.
    fn settle(&self, verdict: Joined) {
        self.attempt().settled.get_or_insert(verdict);
    }
}

/// Where a join stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Joined {
    /// On its way: associating, checking the key, getting an address — or
    /// trying again ([`JOIN_MOST_S`]).
    Coming,
    /// Up, with an address.
    Up,
    /// Over: the network refused the key — NetworkManager asked for a new
    /// password, and nobody had one ([`nm::REASON_NO_SECRETS`]); or, on a
    /// network in view that takes SAE, two activations in a row never
    /// associated ([`unanswered`]).
    Refused,
    /// Over without coming up for another reason: the network went away,
    /// or nothing came up within [`JOIN_MOST_S`].
    Failed,
}

/// What a join's follower does with one `StateChanged(new, old, reason)` of
/// its radio, `in_time` while [`JOIN_MOST_S`] has not passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Next {
    /// Nothing yet: the radio is on its way.
    Wait,
    /// The radio is up: the activation says the rest.
    Done,
    /// Settled: the network refused the key.
    Refused,
    /// The same profile again: the activation ended for another reason, in
    /// time.
    Again,
    /// Settled by the network: an activation never associated
    /// ([`nm::REASON_SSID_NOT_FOUND`]) after one that did not either —
    /// [`unanswered`] says what it was.
    Unanswered,
    /// Settled: nothing came up in time.
    Failed,
    /// Taken down without failing — another network, the radio off: over
    /// if the activation is gone.
    Gone,
}

/// Pure. `unheard` counts this join's activations that ended before
/// associating.
const fn next_of(new: u32, old: u32, reason: u32, in_time: bool, unheard: u32) -> Next {
    match new {
        nm::STATE_ACTIVATED => Next::Done,
        nm::STATE_FAILED if reason == nm::REASON_NO_SECRETS => Next::Refused,
        nm::STATE_FAILED if reason == nm::REASON_SSID_NOT_FOUND && unheard > 0 => Next::Unanswered,
        nm::STATE_FAILED if in_time => Next::Again,
        nm::STATE_FAILED => Next::Failed,
        nm::STATE_DISCONNECTED if old != nm::STATE_FAILED => Next::Gone,
        _ => Next::Wait,
    }
}

/// A join whose activations never associated, twice: REFUSED when the
/// network is still in view (`rsn`: every access point of its name on the
/// radio, together — [`in_view`]) and takes SAE — which a `wpa-psk` profile
/// tries too, where the radio can — SAE turns a wrong key down at
/// authentication, before
/// association, and NetworkManager says nothing else of it
/// ([`nm::REASON_SSID_NOT_FOUND`]). Not a hold: the first ending cleared
/// the supplicant's. Out of view, or a `wpa-psk`-only network — whose wrong
/// key ends in [`nm::REASON_NO_SECRETS`] —, it failed. Pure.
#[must_use]
pub const fn unanswered(rsn: Option<u32>) -> Joined {
    match rsn {
        Some(rsn) if rsn & nm::AP_SEC_KEY_MGMT_SAE != 0 => Joined::Refused,
        _ => Joined::Failed,
    }
}

/// Follows a join on its radio's `StateChanged`, heard from before the
/// activation was asked: a refused key settles it at once; an activation
/// that ended for any other reason is asked again, the same profile on the
/// same radio, while [`JOIN_MOST_S`] has not passed — but a second one that
/// never associated is the network's answer ([`unanswered`], read off the
/// access points of `ssid` the radio still lists); nothing up by then
/// settles it. Every way out but the radio coming up settles the join, so a
/// door never waits on a follower that stopped.
fn follow(
    connection: &zbus::blocking::Connection,
    signals: zbus::blocking::MessageIterator,
    device: &OwnedObjectPath,
    ssid: &str,
    join: &Joining,
    asked: Instant,
) {
    let mut unheard = 0;
    for message in signals {
        // ABANDONED: nobody waits for this join any more.
        if join.attempt().settled.is_some() {
            return;
        }
        let Ok(message) = message else {
            continue;
        };
        let Ok((new, old, reason)) = message.body().deserialize::<(u32, u32, u32)>() else {
            continue;
        };
        let in_time = asked.elapsed() < Duration::from_secs(JOIN_MOST_S);
        let next = next_of(new, old, reason, in_time, unheard);
        if new == nm::STATE_FAILED && reason == nm::REASON_SSID_NOT_FOUND {
            unheard += 1;
        }
        match next {
            Next::Wait => {}
            Next::Done => return,
            Next::Refused => return join.settle(Joined::Refused),
            Next::Unanswered => {
                return join.settle(unanswered(in_view(connection, device, ssid)));
            }
            Next::Failed => return join.settle(Joined::Failed),
            Next::Gone => {
                let active = join.attempt().active.clone();
                if state_of(connection, &active).is_none() {
                    return join.settle(Joined::Failed);
                }
            }
            Next::Again => {
                tracing::info!(reason, "the join ended without coming up; asking it again");
                let again = nm::ManagerProxyBlocking::new(connection).and_then(|g| {
                    g.activate_connection(
                        &join.profile.as_ref(),
                        &device.as_ref(),
                        &ANY_ACCESS_POINT,
                    )
                });
                match again {
                    Ok(active) => join.attempt().active = active,
                    Err(e) => {
                        tracing::info!(error = %e, "the join could not be asked again");
                        return join.settle(Joined::Failed);
                    }
                }
            }
        }
    }
    // THE STREAM ENDED: NetworkManager went away.
    join.settle(Joined::Failed);
}

/// `NM_ACTIVE_CONNECTION_STATE` of the activation at `active`, or `None`
/// once it no longer answers.
fn state_of(connection: &zbus::blocking::Connection, active: &OwnedObjectPath) -> Option<u32> {
    nm::ActiveProxyBlocking::builder(connection)
        .path(active.clone())
        .ok()
        .and_then(|b| b.build().ok())
        .and_then(|a| a.state().ok())
}

/// `NM_ACTIVE_CONNECTION_STATE` -> where a join stands. `None`: the
/// activation no longer answers, which is how NetworkManager ends one that
/// failed — it removes the object. Pure.
#[must_use]
pub const fn joined_of(state: Option<u32>) -> Joined {
    match state {
        // UNKNOWN and ACTIVATING.
        Some(0 | 1) => Joined::Coming,
        Some(2) => Joined::Up,
        // DEACTIVATING, DEACTIVATED, and an object gone.
        _ => Joined::Failed,
    }
}

/// Who seals a remembered password into the TPM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seal {
    /// The agent, over the bus — for the person's session, which has no TPM
    /// of its own; the bus policy lets the distro's user ask it, and nobody
    /// else (`nix/modules/network.nix`).
    ThroughTheAgent,
    /// This process — root, with the TPM and the blobs' folder: the boot's
    /// console. Root is no exception to the agent's bus policy.
    Here,
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

/// One access point of a network, by its key flags, as a join hears it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heard {
    /// `WpaFlags`.
    pub wpa: u32,
    /// `RsnFlags`.
    pub rsn: u32,
}

/// Whether a join's profile for a network is `sae`, from EVERY access point
/// of its name: only when none takes PSK and one is WPA3-only. Pure.
///
/// A network is more than one access point. A router on three bands is WPA2
/// and WPA3 in transition on 2.4 and 5 GHz, and WPA3 alone on 6 GHz, as the
/// standard requires there — and the `wpa-psk` such a network gets reaches
/// its 6 GHz band too: for `wpa-psk`, NetworkManager hands the supplicant
/// SAE as well when the radio can (SAE, PMF and BIP; `nm-supplicant-config.c`).
/// Read off one access point, a 6 GHz one would have put the whole network
/// on `sae`, which a radio without SAE cannot join on any band.
#[must_use]
pub fn sae_profile(heard: &[Heard]) -> bool {
    let takes_psk = heard
        .iter()
        .any(|h| (h.wpa | h.rsn) & nm::AP_SEC_KEY_MGMT_PSK != 0);
    !takes_psk && heard.iter().any(|h| wpa3_only(h.rsn))
}

/// Every access point of `ssid` the radio at `device` lists.
fn heard_of(
    connection: &zbus::blocking::Connection,
    device: &OwnedObjectPath,
    ssid: &str,
) -> Vec<Heard> {
    let Some(radio) = nm::RadioProxyBlocking::builder(connection)
        .path(device.clone())
        .ok()
        .and_then(|b| b.build().ok())
    else {
        return Vec::new();
    };
    radio
        .get_all_access_points()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| {
            let p = nm::AccessPointProxyBlocking::builder(connection)
                .path(path)
                .ok()?
                .build()
                .ok()?;
            p.ssid()
                .is_ok_and(|b| ssid_name(&b) == ssid)
                .then(|| Heard {
                    wpa: p.wpa_flags().unwrap_or(0),
                    rsn: p.rsn_flags().unwrap_or(0),
                })
        })
        .collect()
}

/// The `RsnFlags` of every access point of `ssid` the radio at `device`
/// lists, together; `None` when it lists none. IN VIEW is an access point
/// NetworkManager still lists: it removes the object of one no scan sees
/// any more.
fn in_view(
    connection: &zbus::blocking::Connection,
    device: &OwnedObjectPath,
    ssid: &str,
) -> Option<u32> {
    let heard = heard_of(connection, device, ssid);
    (!heard.is_empty()).then(|| heard.iter().fold(0, |all, h| all | h.rsn))
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

/// A NetworkManager profile's settings, section by section, as the bus
/// hands them.
pub type Configuration = HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>;

/// The SSID stored in a saved NetworkManager profile.
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

    /// The SSIDs that already have a saved profile — on disk: a join held in
    /// memory is gone at the next boot, and saying "saved" of it would skip
    /// the password the network then asks again.
    fn saved_ones(&self) -> Result<Vec<String>> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        let mut output = Vec::new();
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            // The saved SSID is `ay`, as on the access point.
            if let Some(name) = profile_ssid(&cfg)
                && !self.in_memory(&path)
            {
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
        // READ EVERY PROPERTY LIVE, NOT FROM THE CACHE. zbus reuses a proxy per
        // connection+object and caches its properties, kept current by the
        // `PropertiesChanged` signal — but the watch's own message stream
        // (`watch`) consumes those signals on this long-lived connection, so a
        // cached `ActiveConnections` froze at what it was when the shell
        // started and a tunnel raised afterwards never appeared (the modeline
        // drew no mark, the view stayed empty until it was reopened). A fresh
        // `Get` every reading is a few more calls on a path that already reads
        // the whole of the network, and it is the difference between the
        // snapshot telling the truth and telling the past.
        let Ok(manager) = nm::ManagerProxyBlocking::builder(&self.connection)
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
        else {
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
                .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
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

    /// Whether the way out is METERED — paid by the byte, a phone's
    /// hotspot — as the daemon says of the primary connection, its guesses
    /// included (a hotspot it recognises is a guessed yes). `None` when the
    /// daemon is not there or does not know: that is not a "no", and a
    /// caller that waits on a metered network must not take it for one
    /// without saying so.
    ///
    /// What waits on it: the brain's update, which never spends a metered
    /// connection on a scheduled run.
    #[must_use]
    pub fn metered(&self) -> Option<bool> {
        let manager = self.manager().ok()?;
        metered_of(manager.metered().ok()?)
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
                nm::BLUETOOTH_KIND => Medium::Phone,
                // Loopback, tunnel, bridge: they are not how the person is on the
                // network, and listing them would fill the screen with useless rows.
                _ => continue,
            };
            let state = State::from_nm(d.state().unwrap_or(0));
            // A PHONE'S LINK IS NAMED BY ITS IP SIDE, `bnep0`: its
            // `Interface` is the phone's hardware address, and the Bluetooth
            // view names the same interface — the two screens agree on it.
            //
            // A PHONE WITH NO LINK IS NO LINK: every paired phone that offers
            // its connection is a Bluetooth device to NetworkManager, up or
            // not, and listed here each would be a "disconnected" row named
            // by a hardware address. Only the one in use, with its `bnep0`,
            // is how this machine is on a network; the others are the
            // Bluetooth view's rows, with their own switch.
            let interface = if medium == Medium::Phone {
                match d.ip_interface() {
                    Ok(ip) if !ip.is_empty() => ip,
                    _ => continue,
                }
            } else {
                d.interface().unwrap_or_default()
            };

            let serving = medium == Medium::Radio
                && self
                    .radio(&path)
                    .is_ok_and(|r| r.mode().unwrap_or(0) == nm::RADIO_IS_AP);
            let (network, rate_kbps, signal, band) = if medium == Medium::Radio {
                self.radio_detail(&path)
            } else if medium == Medium::Phone {
                // THE PHONE'S NAME is the network this link is on.
                let name = nm::BluetoothDeviceProxyBlocking::builder(&self.connection)
                    .path(path.clone())
                    .and_then(zbus::blocking::proxy::Builder::build)
                    .ok()
                    .and_then(|b| b.name().ok())
                    .filter(|n| !n.is_empty());
                (name, 0, 0, Band::Other)
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

    /// Every profile the daemon has stored — but a Wi-Fi join held in memory
    /// ([`Manager::join_for_this_boot`]): it is gone at the next boot, and
    /// listing it among the remembered would say otherwise.
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
            if profile_ssid(&cfg).is_some() && self.in_memory(&path) {
                continue;
            }
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
        // NMCLI'S WORDS GO TO THE LOG: they are English, and the door says
        // the refusal by its name (`refusal::Refusal::ImportRefused`).
        let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        let said = if said.is_empty() {
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        } else {
            said
        };
        tracing::warn!(%said, "nmcli would not import the file");
        Err(refusal::Named(refusal::Refusal::ImportRefused).into())
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

    /// Brings a VPN location up THROUGH THE ROOT AGENT — the only way the
    /// sealed key is handed over for a tunnel the person raised.
    ///
    /// NetworkManager serves an agent-owned secret for a user-raised
    /// connection only from an agent whose uid matches the person's; our key's
    /// agent is root, so the activation must come from root too. This asks the
    /// root agent to do it (`crate::agent::Ronoctua::raise`), which also makes
    /// sure the tunnel actually answers and takes it down if it does not. It
    /// returns as soon as the bringing-up has started — the snapshot and the
    /// modeline tell how it went.
    ///
    /// # Errors
    ///
    /// The network agent is not on the bus.
    pub fn raise_tunnel(&self, at: &str) -> Result<()> {
        self.vault()?
            .raise(at)
            .context("the network agent would not bring the tunnel up")?;
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

    /// Brings `ssid` up, and answers the join to follow with
    /// [`Manager::joining`]: NetworkManager says later how it went, not here.
    ///
    /// - With a PASSWORD, a join for this boot
    ///   ([`Manager::join_for_this_boot`]); keeping it past the boot is
    ///   [`Manager::remember`], once it is up.
    /// - With none, the saved profile, whose password is the agent's — or,
    ///   for an open network, a new profile on disk: it holds no secret.
    ///
    /// # Errors
    ///
    /// There is no radio, the network vanished between the list and the
    /// keypress, or NetworkManager refused the profile.
    pub fn connect(&self, ssid: &str, password: Option<&str>) -> Result<Joining> {
        if password.is_some() {
            return self.join_for_this_boot(ssid, password);
        }
        let (device, sae) = self.radio_toward(ssid)?;
        let g = self.manager()?;
        let signals = self.signals_of(&device);
        if let Some(profile) = self.profile_of(ssid)? {
            let active =
                g.activate_connection(&profile.as_ref(), &device.as_ref(), &ANY_ACCESS_POINT)?;
            return Ok(self.followed(Joining::of(profile, active, false), device, ssid, signals));
        }
        let connection = wifi_settings(ssid, None, sae, KEPT_BY_THE_DAEMON);
        let (profile, active) =
            g.add_and_activate_connection(connection, &device.as_ref(), &ANY_ACCESS_POINT)?;
        Ok(self.followed(Joining::of(profile, active, true), device, ssid, signals))
    }

    /// The radio's `StateChanged`, heard from now on — asked BEFORE the
    /// activation, so the one that ends it is never missed. `None` when the
    /// bus would not listen: the join then goes unfollowed.
    fn signals_of(&self, device: &OwnedObjectPath) -> Option<zbus::blocking::MessageIterator> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(nm::SERVICE)
            .ok()?
            .path(device.clone())
            .ok()?
            .interface(nm::DEVICE_INTERFACE)
            .ok()?
            .member("StateChanged")
            .ok()?
            .build();
        zbus::blocking::MessageIterator::for_match_rule(rule, &self.connection, None).ok()
    }

    /// `join`, with a follower on its radio ([`follow`]) when `signals` were
    /// heard; `ssid` is the network it was asked toward.
    fn followed(
        &self,
        join: Joining,
        device: OwnedObjectPath,
        ssid: &str,
        signals: Option<zbus::blocking::MessageIterator>,
    ) -> Joining {
        let Some(signals) = signals else {
            return join;
        };
        join.attempt().followed = true;
        let (mine, connection, asked) = (join.clone(), self.connection.clone(), Instant::now());
        let ssid = ssid.to_owned();
        let started = std::thread::Builder::new()
            .name("net-join".to_owned())
            .spawn(move || follow(&connection, signals, &device, &ssid, &mine, asked));
        if started.is_err() {
            join.attempt().followed = false;
        }
        join
    }

    /// Joins `ssid` for THIS BOOT, with the password NetworkManager keeps
    /// itself ([`KEPT_BY_THE_DAEMON`]) — the only join a password just typed
    /// can make: no agent has it yet ([`KEPT_BY_THE_AGENT`] says why). Follow
    /// it with [`Manager::joining`]; drop one that did not come up with
    /// [`Manager::abandon`].
    ///
    /// # Why nothing stays
    ///
    /// The profile is `persist = memory`: NetworkManager keeps it, and its
    /// password, under `/run/NetworkManager` — RAM, root's alone —, and the
    /// machine stopping takes both. Not `volatile`, which deletes the profile
    /// when the link goes down: a link that drops has to come back on its
    /// own. The joins of this SSID made before it are deleted first, so a
    /// password typed again does not leave a second profile beside the first.
    ///
    /// # Errors
    ///
    /// There is no radio, the network is out of range, or NetworkManager
    /// refused the profile.
    pub fn join_for_this_boot(&self, ssid: &str, password: Option<&str>) -> Result<Joining> {
        let (device, sae) = self.radio_toward(ssid)?;
        for earlier in self.held_in_memory(ssid)? {
            let _ = self.profile(&earlier)?.delete();
        }
        let connection = wifi_settings(ssid, password, sae, KEPT_BY_THE_DAEMON);
        let signals = self.signals_of(&device);
        let (profile, active, _) = self.manager()?.add_and_activate_connection2(
            connection,
            &device.as_ref(),
            &ANY_ACCESS_POINT,
            HashMap::from([("persist", Value::from("memory"))]),
        )?;
        Ok(self.followed(Joining::of(profile, active, true), device, ssid, signals))
    }

    /// Where a join stands.
    ///
    /// AN ACTIVATION THAT ENDED is its follower's to judge, on the radio's
    /// own signal ([`follow`]): a refused key, or the same profile once
    /// more. Until it has, the join is on its way — a verdict read off the
    /// ending alone said "refused" of a right password the supplicant was
    /// still holding back.
    #[must_use]
    pub fn joining(&self, join: &Joining) -> Joined {
        let (active, followed, settled) = {
            let attempt = join.attempt();
            (attempt.active.clone(), attempt.followed, attempt.settled)
        };
        if let Some(verdict) = settled {
            return verdict;
        }
        match joined_of(state_of(&self.connection, &active)) {
            Joined::Failed if followed => Joined::Coming,
            other => other,
        }
    }

    /// Deletes the profile a join made — one that did not come up, so the
    /// daemon does not try its password again on its own — and settles the
    /// join, so its follower asks nothing again. A saved profile brought up
    /// is the person's, and stays.
    ///
    /// # Errors
    ///
    /// The bus did not answer, or the profile is gone already.
    pub fn abandon(&self, join: &Joining) -> Result<()> {
        join.settle(Joined::Failed);
        if join.made {
            self.profile(&join.profile)?.delete()?;
        }
        Ok(())
    }

    /// Keeps a network joined with a typed password past this boot: the
    /// password sealed in the TPM, and the join's profile written to disk
    /// with the password left to the agent ([`left_to_the_agent`]). The link
    /// that is up stays up.
    ///
    /// # Only what came up
    ///
    /// A join that is not up is refused ([`refusal::Refusal::Failed`]):
    /// sealing a password the network has not accepted stores rubbish that,
    /// at the next boot, the agent hands over and the radio refuses.
    ///
    /// # Sealed first
    ///
    /// The agent answers only with a blob that exists: a profile on disk that
    /// asked it before the blob was written would be a link that drops and
    /// does not come back. A profile the bus refuses to write takes back a
    /// blob that was not there before, so no secret is left that nothing asks
    /// for.
    ///
    /// # Errors
    ///
    /// The join is not up, the TPM refused, or NetworkManager refused the
    /// profile.
    pub fn remember(&self, join: &Joining, ssid: &str, password: &str, seal: Seal) -> Result<()> {
        if !join.made || self.joining(join) != Joined::Up {
            return Err(refusal::Named(refusal::Refusal::Failed).into());
        }
        let had = match seal {
            Seal::ThroughTheAgent => self.has_stored_password(ssid),
            Seal::Here => secret::ha(ssid),
        };
        match seal {
            Seal::ThroughTheAgent => self.vault()?.store(ssid, password)?,
            Seal::Here => secret::seal(ssid, password)?,
        }
        let profile = self.profile(&join.profile)?;
        let written = profile
            .get_settings()
            .map_err(anyhow::Error::from)
            .and_then(|cfg| {
                left_to_the_agent(cfg)
                    .ok_or_else(|| refusal::Named(refusal::Refusal::Invalid).into())
            })
            .and_then(|cfg| {
                profile
                    .update2(
                        cfg,
                        nm::UPDATE_TO_DISK | nm::UPDATE_NO_REAPPLY,
                        HashMap::new(),
                    )
                    .map_err(anyhow::Error::from)
            });
        if let Err(e) = written {
            if !had {
                match seal {
                    Seal::ThroughTheAgent => {
                        if let Ok(v) = self.vault() {
                            let _ = v.forget(ssid);
                        }
                    }
                    Seal::Here => {
                        let _ = secret::erase(ssid);
                    }
                }
            }
            return Err(e);
        }
        Ok(())
    }

    /// Whether a profile is held in memory, with no file on disk.
    fn in_memory(&self, path: &OwnedObjectPath) -> bool {
        self.profile(path)
            .and_then(|p| Ok(p.flags()?))
            .is_ok_and(|f| f & nm::PROFILE_UNSAVED != 0)
    }

    /// The Wi-Fi profiles of `ssid` held in memory — joins of this boot.
    fn held_in_memory(&self, ssid: &str) -> Result<Vec<OwnedObjectPath>> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        let mut out = Vec::new();
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            if profile_ssid(&cfg).as_deref() == Some(ssid) && self.in_memory(&path) {
                out.push(path);
            }
        }
        Ok(out)
    }

    /// The radio that sees `ssid`, and whether its profile is `sae`
    /// ([`sae_profile`], over every access point of the name on that radio).
    ///
    /// THE RADIO THAT SEES IT, and not the first one listed: a machine with
    /// a second radio (a USB one, or one serving as an access point) lists
    /// it first as often as not, and that radio is out of range of
    /// everything.
    ///
    /// # The radio, and never an access point
    ///
    /// A join names the radio and leaves the access point to NetworkManager
    /// ([`ANY_ACCESS_POINT`]), as `nmcli connection up` and its own
    /// autoconnect do: completing a new profile with none named, it takes the
    /// first access point its compatibility test lets through
    /// (`nm_wifi_aps_find_first_compatible`, which wants PSK for `wpa-psk`).
    /// A join used to name the first access point of the name in the radio's
    /// list, and a network is more than one: on a router on three bands, the
    /// supplicant had listed the 6 GHz one first, which takes WPA3 alone.
    /// NetworkManager refused that pair itself, before the radio tried a key
    /// ("Access point does not support PSK but setting requires it", its
    /// audit at `INFO`), and the installer said "check its password" of a
    /// right one (an MT7922, 2026-10-25).
    fn radio_toward(&self, ssid: &str) -> Result<(OwnedObjectPath, bool)> {
        let radios = self.devices_of_kind(nm::WIFI_KIND)?;
        if radios.is_empty() {
            return Err(refusal::Named(refusal::Refusal::NoRadio).into());
        }
        for device in radios {
            let heard = heard_of(&self.connection, &device, ssid);
            if !heard.is_empty() {
                return Ok((device, sae_profile(&heard)));
            }
        }
        Err(refusal::Named(refusal::Refusal::OutOfRange).into())
    }

    /// The saved profile of `ssid` — on disk (see [`Manager::saved_ones`]).
    fn profile_of(&self, ssid: &str) -> Result<Option<OwnedObjectPath>> {
        Ok(self
            .profiles_of(ssid)?
            .into_iter()
            .find(|p| !self.in_memory(p)))
    }

    /// Every profile of `ssid`, on disk and in memory.
    fn profiles_of(&self, ssid: &str) -> Result<Vec<OwnedObjectPath>> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        let mut out = Vec::new();
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            if profile_ssid(&cfg).as_deref() == Some(ssid) {
                out.push(path);
            }
        }
        Ok(out)
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
    /// Joining somebody's network keeps no password on disk (in memory until
    /// the machine stops, or sealed for the TPM's agent). This is the other
    /// direction: NetworkManager has to hand
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
            .ok_or(refusal::Named(refusal::Refusal::NoRadio))?;
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
            Medium::Phone => nm::BLUETOOTH_KIND,
        };
        let path = self
            .devices_of_kind(kind)?
            .into_iter()
            .next()
            .ok_or(refusal::Named(refusal::Refusal::NoDevice))?;
        self.device(&path)?.disconnect()?;
        Ok(())
    }

    /// Takes down whatever is active on ONE link: the device of that medium
    /// whose interface is the one named — `wlan0`, `enp1s0`, or a phone's
    /// `bnep0` (its IP side, the name [`Manager::links`] gives it).
    ///
    /// BY THE LINK, NOT THE MEDIUM: with two phones paired, the first
    /// Bluetooth device NetworkManager lists may be the one that is not in
    /// use, and `RET` on the link in use would take down nothing — or the
    /// other phone.
    ///
    /// # Errors
    ///
    /// No device of that medium has that interface, or the bus refused.
    pub fn disconnect_link(&self, medium: Medium, interface: &str) -> Result<()> {
        let kind = match medium {
            Medium::Wired => nm::ETHERNET_KIND,
            Medium::Radio => nm::WIFI_KIND,
            Medium::Phone => nm::BLUETOOTH_KIND,
        };
        let path = self
            .devices_of_kind(kind)?
            .into_iter()
            .find(|path| {
                self.device(path).is_ok_and(|d| {
                    d.interface().is_ok_and(|i| i == interface)
                        || d.ip_interface().is_ok_and(|i| i == interface)
                })
            })
            .context("no device has that interface")?;
        self.device(&path)?.disconnect()?;
        Ok(())
    }

    /// Uses a phone's connection over Bluetooth, or stops — PAN, through
    /// NetworkManager.
    ///
    /// # Who owns `bnep0`
    ///
    /// NETWORKMANAGER, and this is the decision both views keep. BlueZ's
    /// `Network1.Connect("nap")` alone makes the `bnep0` interface and stops
    /// there: nothing asks it for an address, and the phone's internet is a
    /// cable plugged into nothing. NetworkManager's Bluetooth device does the
    /// whole of it — it asks BlueZ for the link, runs DHCP on `bnep0` and
    /// puts the route in — so the Bluetooth view's switch comes HERE, and
    /// both views then read the one link from their own daemon: the network
    /// view NetworkManager's `bnep0` link, the Bluetooth view BlueZ's
    /// `Network1.Interface`, the same name.
    ///
    /// A profile already stored for the phone is brought up; with none, one
    /// is made — `panu`, never on its own (`autoconnect` off: a phone's
    /// data plan is not something a machine starts spending by itself).
    ///
    /// # Errors
    ///
    /// NetworkManager has no Bluetooth device for that address (not paired,
    /// or the phone does not offer its connection), or it refused.
    pub fn tether(&self, address: &str, on: bool) -> Result<()> {
        let device = self
            .devices_of_kind(nm::BLUETOOTH_KIND)?
            .into_iter()
            .find(|path| {
                nm::BluetoothDeviceProxyBlocking::builder(&self.connection)
                    .path(path.clone())
                    .and_then(zbus::blocking::proxy::Builder::build)
                    .and_then(|b| b.hw_address())
                    .is_ok_and(|a| a.eq_ignore_ascii_case(address))
            })
            .ok_or(Refusal::NoPhone)?;
        if !on {
            self.device(&device)?.disconnect()?;
            return Ok(());
        }
        let g = self.manager()?;
        let none = zbus::zvariant::ObjectPath::try_from("/")?;
        if let Some(profile) = self.tether_profile_of(address)? {
            g.activate_connection(&profile.as_ref(), &device.as_ref(), &none)?;
            return Ok(());
        }
        // An address BlueZ did not give is no phone NetworkManager has.
        let bytes = hardware_bytes(address).ok_or(Refusal::NoPhone)?;
        let mut connection: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
        connection.insert(
            "connection",
            HashMap::from([
                ("id", Value::from(address)),
                ("type", Value::from("bluetooth")),
                ("autoconnect", Value::from(false)),
            ]),
        );
        connection.insert(
            "bluetooth",
            HashMap::from([
                ("bdaddr", Value::from(bytes)),
                ("type", Value::from("panu")),
            ]),
        );
        g.add_and_activate_connection(connection, &device.as_ref(), &none)?;
        Ok(())
    }

    /// The stored Bluetooth profile for a phone, when there is one.
    fn tether_profile_of(&self, address: &str) -> Result<Option<OwnedObjectPath>> {
        let wanted = hardware_bytes(address);
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            let bdaddr = cfg
                .get("bluetooth")
                .and_then(|b| b.get("bdaddr"))
                .and_then(|v| Vec::<u8>::try_from(v.clone()).ok());
            if bdaddr.is_some() && bdaddr == wanted {
                return Ok(Some(path));
            }
        }
        Ok(None)
    }

    /// Stores a VPN profile's user and password, so it comes up without
    /// asking — what an imported `.ovpn` with `auth-user-pass` needs once.
    ///
    /// `GetSettings` + `GetSecrets` + `Update`: the profile as it is, the
    /// secrets it already had carried along (an `Update` replaces the WHOLE
    /// profile, and a secret it does not carry is deleted), the user name in
    /// `vpn.data`, and the password in `vpn.secrets` with `password-flags=0`
    /// — stored by NetworkManager, root-owned, in the profile. The password
    /// travels in this one call and nowhere else: never an `argv` (`nmcli`
    /// would put it in one), never a log.
    ///
    /// # Errors
    ///
    /// There is no VPN profile by that name, or NetworkManager refused.
    pub fn store_vpn_secret(&self, tunnel: &str, user: &str, password: &str) -> Result<()> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        let mut found = None;
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            let field = |section: &str, key: &str| {
                cfg.get(section)
                    .and_then(|s| s.get(key))
                    .and_then(|v| String::try_from(v.clone()).ok())
            };
            if field("connection", "id").as_deref() == Some(tunnel)
                && field("connection", "type").as_deref() == Some("vpn")
            {
                found = Some((path, cfg));
                break;
            }
        }
        let (path, cfg) = found.ok_or(Refusal::NoVpn)?;
        let profile = self.profile(&path)?;
        let kept: HashMap<String, String> = profile
            .get_secrets("vpn")
            .ok()
            .and_then(|s| s.get("vpn").cloned())
            .and_then(|v| v.get("secrets").cloned())
            .and_then(|v| HashMap::<String, String>::try_from(v).ok())
            .unwrap_or_default();
        let settings = vpn_secret_settings(&cfg, &kept, user, password)?;
        let borrowed: HashMap<&str, HashMap<&str, Value<'_>>> = settings
            .iter()
            .map(|(section, fields)| {
                (
                    section.as_str(),
                    fields
                        .iter()
                        .map(|(k, v)| (k.as_str(), Value::from(v.clone())))
                        .collect(),
                )
            })
            .collect();
        profile.update(borrowed)?;
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

    /// Deletes a network's profiles — the saved ones and a join held in
    /// memory: forgetting a network leaves none of it behind.
    ///
    /// # Errors
    ///
    /// There is no profile with that name, or polkit refused.
    pub fn forget(&self, ssid: &str) -> Result<()> {
        let paths = self.profiles_of(ssid)?;
        if paths.is_empty() {
            return Err(refusal::Named(refusal::Refusal::NoProfile).into());
        }
        for path in paths {
            self.profile(&path)?.delete()?;
        }
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

/// A hardware address's six bytes, `AA:BB:CC:DD:EE:FF` → `[0xaa, …]`.
#[must_use]
pub fn hardware_bytes(address: &str) -> Option<Vec<u8>> {
    let bytes: Vec<u8> = address
        .split(':')
        .map(|part| u8::from_str_radix(part, 16).ok())
        .collect::<Option<_>>()?;
    (bytes.len() == 6).then_some(bytes)
}

/// The profile `Update` takes to store a VPN's user and password: every
/// section as it was read, `vpn.data` with the user and `password-flags=0`
/// laid over, and `vpn.secrets` the ones kept plus the password.
///
/// PURE, so the merge is tested without a daemon — the part that would lose
/// a profile's other settings, or another secret, if it were wrong.
///
/// # Errors
///
/// The profile has no `vpn` section, so it is no VPN.
pub fn vpn_secret_settings(
    cfg: &HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>,
    kept: &HashMap<String, String>,
    user: &str,
    password: &str,
) -> Result<HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>> {
    let mut settings = cfg.clone();
    let vpn = settings.get_mut("vpn").ok_or(Refusal::NotVpn)?;
    let mut data: HashMap<String, String> = vpn
        .get("data")
        .and_then(|v| HashMap::<String, String>::try_from(v.clone()).ok())
        .unwrap_or_default();
    if !user.is_empty() {
        data.insert("username".to_owned(), user.to_owned());
    }
    data.insert("password-flags".to_owned(), "0".to_owned());
    let mut secrets = kept.clone();
    secrets.insert("password".to_owned(), password.to_owned());
    vpn.insert("data".to_owned(), owned_map(data)?);
    vpn.insert("secrets".to_owned(), owned_map(secrets)?);
    Ok(settings)
}

/// A string map as the `a{ss}` NetworkManager keeps `vpn.data` in.
fn owned_map(map: HashMap<String, String>) -> Result<zbus::zvariant::OwnedValue> {
    zbus::zvariant::OwnedValue::try_from(Value::from(map)).context("a map that is no D-Bus value")
}

/// Watches the radio until a scan FINISHES, then calls `on_scan` — once.
///
/// # Why a thread, and why not a timer
///
/// `RequestScan` returns at once and the radio answers seconds later. Blocking
/// the footer's loop would freeze keyboard, painting and compositor events for those
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
        .ok_or(refusal::Named(refusal::Refusal::NoRadio))?;

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

    /// A GUESS IS AN ANSWER, AND UNKNOWN IS NOT: the daemon guesses a phone's
    /// hotspot is metered, and a caller that waits on a metered network must
    /// wait on that too — while "it does not know" must not read as "no".
    #[test]
    fn metered_reads_the_guesses_and_not_the_unknown() {
        assert_eq!(metered_of(1), Some(true));
        assert_eq!(metered_of(3), Some(true), "a guessed yes is metered");
        assert_eq!(metered_of(2), Some(false));
        assert_eq!(metered_of(4), Some(false));
        assert_eq!(metered_of(0), None, "unknown is no answer");
        assert_eq!(metered_of(9), None, "a number the daemon does not define");
    }

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

    #[test]
    fn a_network_is_sae_only_when_every_access_point_is() {
        let (psk, sae) = (nm::AP_SEC_KEY_MGMT_PSK, nm::AP_SEC_KEY_MGMT_SAE);
        let rsn = |rsn| Heard { wpa: 0, rsn };
        // THE ROUTER ON THREE BANDS that the installer could not join, as its
        // radio listed it: the 6 GHz access point (WPA3 alone) first, then
        // 2.4 and 5 GHz in transition. `wpa-psk`, which reaches 6 GHz too.
        assert!(
            !sae_profile(&[rsn(sae), rsn(psk | sae), rsn(psk | sae)]),
            "a network that takes PSK on two bands went on `sae`"
        );
        // Listed in any order, the same answer.
        assert!(!sae_profile(&[rsn(psk | sae), rsn(sae)]));
        // ONLY WPA3, on every access point (a network on 6 GHz alone).
        assert!(sae_profile(&[rsn(sae), rsn(sae)]));
        assert!(sae_profile(&[rsn(sae)]));
        // WPA2, WPA1 (its flags in `WpaFlags`), open, and nothing heard.
        assert!(!sae_profile(&[rsn(psk)]));
        assert!(!sae_profile(&[Heard { wpa: psk, rsn: 0 }, rsn(sae)]));
        assert!(!sae_profile(&[rsn(0)]));
        assert!(!sae_profile(&[]));
    }
    use super::*;

    #[test]
    fn a_hardware_address_is_six_bytes_or_nothing() {
        assert_eq!(
            hardware_bytes("AA:bb:0C:00:11:22"),
            Some(vec![0xaa, 0xbb, 0x0c, 0x00, 0x11, 0x22])
        );
        assert_eq!(hardware_bytes("AA:BB"), None);
        assert_eq!(hardware_bytes("not:an:address:at:all:zz"), None);
    }

    #[test]
    fn storing_a_vpn_password_keeps_the_profile_and_its_other_secrets() {
        // AN `Update` REPLACES THE WHOLE PROFILE: a merge that dropped a
        // section, a `vpn.data` key or a secret it was not told about would
        // break the tunnel it is meant to finish.
        let owned = |v: Value<'_>| zbus::zvariant::OwnedValue::try_from(v).unwrap();
        let mut cfg: HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>> = HashMap::new();
        cfg.insert(
            "connection".into(),
            HashMap::from([("id".to_owned(), owned(Value::from("work")))]),
        );
        cfg.insert(
            "vpn".into(),
            HashMap::from([
                (
                    "data".to_owned(),
                    owned(Value::from(HashMap::from([
                        ("remote".to_owned(), "vpn.example.org".to_owned()),
                        ("connection-type".to_owned(), "password".to_owned()),
                    ]))),
                ),
                (
                    "service-type".to_owned(),
                    owned(Value::from("org.freedesktop.NetworkManager.openvpn")),
                ),
            ]),
        );
        let kept = HashMap::from([("cert-pass".to_owned(), "kept".to_owned())]);
        let out = vpn_secret_settings(&cfg, &kept, "ana", "s3cret").unwrap();
        assert!(out.contains_key("connection"));
        let vpn = &out["vpn"];
        assert!(vpn.contains_key("service-type"));
        let data = HashMap::<String, String>::try_from(vpn["data"].clone()).unwrap();
        assert_eq!(data["remote"], "vpn.example.org");
        assert_eq!(data["username"], "ana");
        assert_eq!(data["password-flags"], "0");
        let secrets = HashMap::<String, String>::try_from(vpn["secrets"].clone()).unwrap();
        assert_eq!(secrets["password"], "s3cret");
        assert_eq!(secrets["cert-pass"], "kept");
        // AND NO VPN SECTION IS NO VPN.
        cfg.remove("vpn");
        assert!(vpn_secret_settings(&cfg, &kept, "ana", "x").is_err());
    }

    #[test]
    fn the_cable_carries_before_the_radio_and_the_radio_before_a_phone() {
        assert!(Medium::Wired.rank() < Medium::Radio.rank());
        assert!(Medium::Radio.rank() < Medium::Phone.rank());
    }

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

    /// FAILS IF: the profile a remembered network writes to disk keeps the
    /// password (`psk=` in a file a backup of `/persist` carries), keeps the
    /// flag that leaves it to the daemon, or loses a setting of its own — or
    /// an open network is given a key to remember.
    #[test]
    fn a_remembered_profile_leaves_the_password_to_the_agent() {
        let value = |v: &str| OwnedValue::try_from(Value::from(v)).unwrap();
        let cfg: Configuration = HashMap::from([
            (
                "connection".to_owned(),
                HashMap::from([("id".to_owned(), value("home"))]),
            ),
            (
                "802-11-wireless-security".to_owned(),
                HashMap::from([
                    ("key-mgmt".to_owned(), value("wpa-psk")),
                    ("psk".to_owned(), value("Zq7-radio")),
                    ("psk-flags".to_owned(), OwnedValue::from(KEPT_BY_THE_DAEMON)),
                ]),
            ),
        ]);
        let kept = left_to_the_agent(cfg).expect("a secured profile");
        let security = &kept["802-11-wireless-security"];
        assert!(!security.contains_key("psk"), "the password went to disk");
        assert_eq!(security["psk-flags"], OwnedValue::from(KEPT_BY_THE_AGENT));
        assert_eq!(security["key-mgmt"], value("wpa-psk"));
        assert_eq!(kept["connection"]["id"], value("home"));
        let open: Configuration = HashMap::from([(
            "connection".to_owned(),
            HashMap::from([("id".to_owned(), value("café"))]),
        )]);
        assert!(
            left_to_the_agent(open).is_none(),
            "an open network has no key"
        );
        // AND THE TWO FLAGS ARE NetworkManager's: 0 kept by the daemon (in
        // memory), 1 the agent's.
        assert_eq!((KEPT_BY_THE_DAEMON, KEPT_BY_THE_AGENT), (0, 1));
    }

    #[test]
    fn a_join_is_up_only_when_activated() {
        assert_eq!(joined_of(Some(0)), Joined::Coming);
        assert_eq!(joined_of(Some(1)), Joined::Coming);
        assert_eq!(joined_of(Some(2)), Joined::Up);
        assert_eq!(joined_of(Some(3)), Joined::Failed);
        assert_eq!(joined_of(Some(4)), Joined::Failed);
        assert_eq!(joined_of(None), Joined::Failed);
    }

    /// FAILS IF: a join's follower takes a refused key from anything but
    /// NetworkManager's "no secrets" or a SECOND activation in a row that
    /// never associated — the first one, a right password the supplicant
    /// held back after a wrong one, read as "refused" — gives up on an
    /// activation that ended in time, tries one again past the deadline, or
    /// takes the radio leaving a FAILED state, or a step on its way, for the
    /// join taken down.
    #[test]
    fn a_key_is_refused_by_no_secrets_or_twice_unheard_and_the_rest_is_tried_again() {
        // `NM_DEVICE_STATE_REASON_SUPPLICANT_TIMEOUT`.
        let timeout = 11;
        let away = nm::REASON_SSID_NOT_FOUND;
        assert_eq!(
            next_of(120, 50, nm::REASON_NO_SECRETS, true, 0),
            Next::Refused
        );
        assert_eq!(
            next_of(120, 50, nm::REASON_NO_SECRETS, false, 0),
            Next::Refused
        );
        assert_eq!(next_of(120, 50, timeout, true, 0), Next::Again);
        assert_eq!(next_of(120, 50, timeout, true, 1), Next::Again);
        assert_eq!(
            next_of(120, 50, away, true, 0),
            Next::Again,
            "the first never-associated ending: maybe a hold"
        );
        assert_eq!(
            next_of(120, 50, away, true, 1),
            Next::Unanswered,
            "the second: the hold was cleared"
        );
        assert_eq!(next_of(120, 50, away, false, 1), Next::Unanswered);
        assert_eq!(next_of(120, 50, timeout, false, 0), Next::Failed);
        assert_eq!(next_of(120, 50, away, false, 0), Next::Failed);
        assert_eq!(next_of(100, 90, 0, true, 0), Next::Done);
        assert_eq!(next_of(100, 90, 0, false, 1), Next::Done);
        assert_eq!(
            next_of(30, 120, 0, true, 0),
            Next::Wait,
            "a failure's own aftermath"
        );
        assert_eq!(next_of(30, 110, 39, true, 0), Next::Gone, "taken down");
        for step in [40, 50, 60, 70, 80, 90] {
            assert_eq!(next_of(step, step - 10, 0, true, 1), Next::Wait, "{step}");
        }
        assert_eq!(
            (
                nm::STATE_DISCONNECTED,
                nm::STATE_ACTIVATED,
                nm::STATE_FAILED,
                nm::REASON_NO_SECRETS,
                nm::REASON_SSID_NOT_FOUND,
            ),
            (30, 100, 120, 7, 53),
            "NetworkManager's numbers"
        );
    }

    /// FAILS IF: a join that never associated twice is called refused
    /// anywhere but on a network in view that takes SAE (WPA3, alone or in
    /// transition) — a `wpa-psk`-only network says a wrong key with "no
    /// secrets", and one out of view refused nothing.
    #[test]
    fn twice_unheard_is_a_refused_key_only_where_sae_answers() {
        let (psk, sae) = (nm::AP_SEC_KEY_MGMT_PSK, nm::AP_SEC_KEY_MGMT_SAE);
        assert_eq!(unanswered(Some(sae)), Joined::Refused, "WPA3 only");
        assert_eq!(unanswered(Some(psk | sae)), Joined::Refused, "transition");
        assert_eq!(unanswered(Some(psk)), Joined::Failed, "WPA2");
        assert_eq!(unanswered(Some(0)), Joined::Failed, "open");
        assert_eq!(unanswered(None), Joined::Failed, "out of view");
    }

    /// FAILS IF: a join with no follower (a test's) waits for one, a settled
    /// join is settled again, or a copy of a join does not see the verdict
    /// the original was given.
    #[test]
    fn a_verdict_is_given_once_and_every_copy_sees_it() {
        let join = Joining::at(
            "/org/freedesktop/NetworkManager/Settings/9",
            "/org/freedesktop/NetworkManager/ActiveConnection/9",
            true,
        )
        .unwrap();
        let copy = join.clone();
        assert!(
            !join.attempt().followed,
            "a join with no daemon has a follower"
        );
        join.settle(Joined::Refused);
        join.settle(Joined::Failed);
        assert_eq!(copy.attempt().settled, Some(Joined::Refused));
    }

    /// FAILS IF: the profile a join hands NetworkManager loses the password,
    /// its flags or the key management the access point asks for.
    #[test]
    fn the_settings_carry_the_key_and_its_flags() {
        let s = wifi_settings("home", Some("Zq7-radio"), false, KEPT_BY_THE_DAEMON);
        let security = &s["802-11-wireless-security"];
        assert_eq!(security["psk"], Value::from("Zq7-radio"));
        assert_eq!(security["psk-flags"], Value::from(0u32));
        assert_eq!(security["key-mgmt"], Value::from("wpa-psk"));
        let sae = wifi_settings("home", Some("Zq7-radio"), true, KEPT_BY_THE_AGENT);
        assert_eq!(
            sae["802-11-wireless-security"]["key-mgmt"],
            Value::from("sae")
        );
        assert_eq!(
            sae["802-11-wireless-security"]["psk-flags"],
            Value::from(1u32)
        );
        assert!(
            !wifi_settings("open", None, false, KEPT_BY_THE_DAEMON)
                .contains_key("802-11-wireless-security"),
            "an open network carries no key"
        );
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

/// What only a radio and an access point prove — run by `wifi-joins-vm`
/// (`tests/wifi-joins-vm.nix`), in a VM whose second radio sees the network
/// named in `RONOCTUA_TEST_WIFI_SSID`, its password in
/// `RONOCTUA_TEST_WIFI_PASSWORD`. Ignored anywhere else: a machine's own
/// radio is not a test's to join.
#[cfg(test)]
mod on_the_air {
    use super::*;
    use std::time::{Duration, Instant};

    /// The daemon, the network — seen by the radio before a join names it —
    /// and its password.
    fn air() -> (Manager, String, String) {
        let variable = |name: &str| {
            std::env::var(name).unwrap_or_else(|_| panic!("{name} names the test's network"))
        };
        let manager = Manager::fresh().expect("no system bus");
        let ssid = variable("RONOCTUA_TEST_WIFI_SSID");
        let deadline = Instant::now() + Duration::from_secs(60);
        while !manager
            .networks()
            .unwrap_or_default()
            .iter()
            .any(|n| n.ssid == ssid)
        {
            assert!(Instant::now() < deadline, "the radio never saw `{ssid}`");
            let _ = manager.scan();
            std::thread::sleep(Duration::from_secs(2));
        }
        (manager, ssid, variable("RONOCTUA_TEST_WIFI_PASSWORD"))
    }

    /// NetworkManager's verdict, waited for as the doors and the console wait.
    fn verdict(manager: &Manager, join: &Joining) -> Joined {
        let deadline = Instant::now() + Duration::from_secs(JOIN_MOST_S);
        loop {
            let joined = manager.joining(join);
            if joined != Joined::Coming || Instant::now() >= deadline {
                return joined;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Whether `ssid` is among the remembered networks.
    fn remembered(manager: &Manager, ssid: &str) -> bool {
        manager
            .saved_profiles()
            .unwrap_or_default()
            .iter()
            .any(|s| s.ssid.as_deref() == Some(ssid))
    }

    /// FAILS IF: the footer's join — as the distro's user, with the TPM's
    /// agent the only one registered — is not taken; is listed as
    /// remembered before "yes"; or "yes" seals nothing, drops the link, or
    /// leaves the network out of the remembered.
    #[test]
    #[ignore = "needs a radio and an access point: wifi-joins-vm"]
    fn the_footer_joins_then_remembers() {
        let (m, ssid, password) = air();
        let join = m.connect(&ssid, Some(&password)).expect("the join");
        assert_eq!(verdict(&m, &join), Joined::Up, "the network refused it");
        assert!(!remembered(&m, &ssid), "a join for this boot is remembered");
        m.remember(&join, &ssid, &password, Seal::ThroughTheAgent)
            .expect("remembering it");
        assert!(m.has_stored_password(&ssid), "the agent holds no blob");
        assert_eq!(m.joining(&join), Joined::Up, "remembering dropped the link");
        assert!(
            remembered(&m, &ssid),
            "the remembered network is not listed"
        );
    }

    /// FAILS IF: a join nobody asks to keep does not come up, or is listed
    /// as remembered, or has its password sealed.
    #[test]
    #[ignore = "needs a radio and an access point: wifi-joins-vm"]
    fn a_join_is_kept_for_this_boot_only() {
        let (m, ssid, password) = air();
        let join = m.connect(&ssid, Some(&password)).expect("the join");
        assert_eq!(verdict(&m, &join), Joined::Up, "the network refused it");
        assert!(!remembered(&m, &ssid), "a join for this boot is remembered");
        assert!(!m.has_stored_password(&ssid), "a blob was sealed for it");
    }

    /// FAILS IF: a wrong password is not refused, can be remembered, or
    /// leaves its profile behind for the daemon to try again.
    #[test]
    #[ignore = "needs a radio and an access point: wifi-joins-vm"]
    fn a_wrong_password_is_refused_and_kept_nowhere() {
        let (m, ssid, password) = air();
        let wrong = format!("{password}-not");
        let join = m.connect(&ssid, Some(&wrong)).expect("the join");
        assert_eq!(verdict(&m, &join), Joined::Refused, "a wrong key was taken");
        assert!(
            m.remember(&join, &ssid, &wrong, Seal::ThroughTheAgent)
                .is_err(),
            "a refused password was remembered"
        );
        m.abandon(&join).expect("abandoning it");
        assert!(
            m.held_in_memory(&ssid).expect("the profiles").is_empty(),
            "the refused join's profile stayed"
        );
    }

    /// FAILS IF: the right password typed right after a wrong one does not
    /// come up — the supplicant holds the network back after the refusal,
    /// and NetworkManager's association timeout ended the first activation
    /// before the hold did: read as refused, it was the installer's first
    /// screen refusing a right password.
    #[test]
    #[ignore = "needs a radio and an access point: wifi-joins-vm"]
    fn a_right_password_after_a_wrong_one_comes_up() {
        let (m, ssid, password) = air();
        let wrong = format!("{password}-not");
        let join = m.connect(&ssid, Some(&wrong)).expect("the wrong join");
        assert_eq!(verdict(&m, &join), Joined::Refused, "a wrong key was taken");
        m.abandon(&join).expect("abandoning it");
        let began = Instant::now();
        let join = m.connect(&ssid, Some(&password)).expect("the right join");
        assert_eq!(
            verdict(&m, &join),
            Joined::Up,
            "the right password, after {:?}",
            began.elapsed()
        );
        println!("the right password came up in {:?}", began.elapsed());
    }

    /// FAILS IF: the boot's console — root, sealing itself, since the
    /// agent's bus policy answers only the distro's user — cannot join and
    /// remember.
    #[test]
    #[ignore = "needs a radio, an access point and root: wifi-joins-vm"]
    fn the_console_joins_then_remembers_as_root() {
        let (m, ssid, password) = air();
        let join = m.connect(&ssid, Some(&password)).expect("the join");
        assert_eq!(verdict(&m, &join), Joined::Up, "the network refused it");
        m.remember(&join, &ssid, &password, Seal::Here)
            .expect("remembering it");
        assert!(secret::ha(&ssid), "no blob was sealed");
        assert!(
            remembered(&m, &ssid),
            "the remembered network is not listed"
        );
    }
}
