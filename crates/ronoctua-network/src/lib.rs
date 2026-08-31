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

    /// How it appears on screen. **It is not UI text** — it is a number with a
    /// unit, the same in both languages, and that is why it does not go through the `.ftl`
    /// (invariant 8 holds for sentences, not for "5 GHz").
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ghz24 => "2,4 GHz",
            Self::Ghz5 => "5 GHz",
            Self::Ghz6 => "6 GHz",
            Self::Other => "—",
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
}

impl Link {
    /// The rate, readable. `1921500` becomes `1,9 Gb/s`.
    ///
    /// One decimal place and not two: the negotiated rate wobbles every second,
    /// and the second place would be noise flickering on screen.
    #[must_use]
    pub fn rate(&self) -> String {
        readable_rate(self.rate_kbps)
    }
}

/// `kb/s` -> `340 Mb/s` or `1,9 Gb/s`.
#[must_use]
pub fn readable_rate(kbps: u32) -> String {
    match kbps {
        0 => "—".to_owned(),
        // Below 1 Mb/s the smaller unit is the only one that does not lie.
        1..=999 => format!("{kbps} kb/s"),
        1_000..=999_999 => format!("{} Mb/s", kbps / 1_000),
        _ => {
            let tenth = (kbps + 50_000) / 100_000;
            format!("{},{} Gb/s", tenth / 10, tenth % 10)
        }
    }
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
#[must_use]
pub fn group_by_ssid(mut networks: Vec<Network>) -> Vec<Network> {
    // Strongest first, so `dedup_by` keeps it.
    networks.sort_by(|a, b| {
        b.active_one
            .cmp(&a.active_one)
            .then(b.signal.cmp(&a.signal))
            .then(a.ssid.cmp(&b.ssid))
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

            output.push(Link {
                medium,
                interface,
                network,
                state,
                rate_kbps,
                signal,
                band,
            });
        }
        Ok(output)
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
/// seconds — the same `systemd-run --scope` trap CLAUDE.md records. And polling
/// on a timer would spend a frame on a list that has not moved.
///
/// `LastScan` changes exactly when there is something new to read. The thread
/// blocks on that property's change stream and says so once.
///
/// # Errors
///
/// There is no bus, or the machine has no radio.
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
                tracing::debug!("o rádio terminou uma varredura");
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
                ssid: "casa".into(),
                signal: 40,
                band: Band::Ghz24,
                secured: true,
                wpa3_only: false,
                saved: true,
                active_one: false,
                max_rate_kbps: 144_000,
            },
            Network {
                ssid: "casa".into(),
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
    fn the_active_comes_before_the_saved_and_the_saved_before_the_strong_neighbour() {
        let networks = vec![
            Network {
                ssid: "vizinho".into(),
                signal: 99,
                band: Band::Ghz24,
                secured: true,
                wpa3_only: false,
                saved: false,
                active_one: false,
                max_rate_kbps: 0,
            },
            Network {
                ssid: "salva".into(),
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
        assert_eq!(names, ["current", "salva", "vizinho"]);
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
        assert_eq!(readable_rate(0), "—");
        assert_eq!(readable_rate(540), "540 kb/s");
        assert_eq!(readable_rate(340_000), "340 Mb/s");
        assert_eq!(readable_rate(1_921_500), "1,9 Gb/s");
        assert_eq!(readable_rate(2_401_900), "2,4 Gb/s");
    }
}
