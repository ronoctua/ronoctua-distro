//! NetworkManager's D-Bus proxies — the only part of this crate that
//! knows D-Bus exists.
//!
//! # Why the boundary is here
//!
//! The same split as `ronoctua-notify`: `dbus.rs` translates the bus and the
//! rest of the crate does not import it. The gain is that the rules — how a frequency
//! becomes a band, how the networks are ordered, what counts as "secured" —
//! can be tested without bringing up any bus, and that is precisely where the bug
//! a test catches lives.
//!
//! # Blocking, on a thread
//!
//! zbus has both APIs. We use the blocking one, for the same reason as the
//! notification daemon: the shell is a `calloop` loop with one thread per
//! event source, and bringing an async executor inside it
//! would add a second concurrency model without solving anything.

// `#[zbus::proxy]` GENERATES a struct per trait (`ManagerProxy`,
// `ManagerProxyBlocking`), and the workspace's `missing_docs` flags them. There is
// nowhere to write their documentation: the macro creates them after expansion.
// What can be documented — the trait and each method — is documented, and that is
// where the reader reads it. The `allow` stays on the module, and not on the crate, so the
// rest goes on requiring docs.
#![allow(missing_docs)]

use std::collections::HashMap;

use zbus::zvariant::{ObjectPath, OwnedObjectPath, Value};

/// The service, on the **system** bus.
///
/// System and not session: NetworkManager belongs to the host, not to the logged-in person.
/// Asking for it on the session bus returns "name has no owner", which is the same error as an
/// absent service and leads to the wrong diagnosis.
pub const SERVICE: &str = "org.freedesktop.NetworkManager";

/// Device type, from the `NM_DEVICE_TYPE` enum.
pub const ETHERNET_KIND: u32 = 1;
/// Likewise, Wi-Fi radio.
pub const WIFI_KIND: u32 = 2;

/// `NM_DEVICE_STATE_ACTIVATED` — the only state that counts as connected.
pub const STATE_ACTIVATED: u32 = 100;
/// The range of transitional states: preparing, configuring, authenticating.
pub const STATE_TRANSITION: std::ops::Range<u32> = 30..100;

/// `NM_802_11_MODE_AP` — the radio is BEING an access point, not using one.
pub const RADIO_IS_AP: u32 = 3;

/// `NM_802_11_AP_FLAGS_PRIVACY` — the network asks for a key.
pub const AP_PRIVACY: u32 = 0x1;

/// `NM_802_11_AP_SEC_KEY_MGMT_PSK` — WPA/WPA2 with a pre-shared key.
///
/// Present in `RsnFlags` on every access point that still accepts `wpa-psk`,
/// including one in WPA3 TRANSITION mode.
pub const AP_SEC_KEY_MGMT_PSK: u32 = 0x100;

/// `NM_802_11_AP_SEC_KEY_MGMT_SAE` — WPA3.
///
/// A WPA3-ONLY access point advertises this and NOT `PSK`. It is the difference
/// that decides between `wpa-psk` and `sae`, and getting it wrong gives a
/// refusal from NetworkManager instead of a connection.
pub const AP_SEC_KEY_MGMT_SAE: u32 = 0x400;

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
pub trait Manager {
    /// Every network device, cable and radio.
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// Activates an ALREADY SAVED connection. It is the path for whoever has connected before —
    /// and the only one that does not need the password again.
    fn activate_connection(
        &self,
        connection: &ObjectPath<'_>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;

    /// Creates the connection and activates it in one go. It is the path for whoever is
    /// typing the password right now.
    fn add_and_activate_connection(
        &self,
        connection: HashMap<&str, HashMap<&str, Value<'_>>>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;

    /// Is the radio on?
    #[zbus(property)]
    fn wireless_enabled(&self) -> zbus::Result<bool>;

    /// Is networking on AT ALL? This is the aeroplane switch: it takes every
    /// device down, radio and cable alike, and it is what `Enable(false)`
    /// writes. `WirelessEnabled` above is only the radio.
    #[zbus(property)]
    fn networking_enabled(&self) -> zbus::Result<bool>;

    /// Turns every device off, or back on — the aeroplane.
    fn enable(&self, on: bool) -> zbus::Result<()>;

    /// What the daemon believes about the way OUT: 1 none, 2 portal,
    /// 3 limited, 4 full. Believes, and does not measure — with the
    /// connectivity check off it answers `full` for any route at all, which
    /// is why `nix/modules/network.nix` turns the check on.
    #[zbus(property)]
    fn connectivity(&self) -> zbus::Result<u32>;

    /// Every connection that is UP: a VPN is one of these, and so is the
    /// Wi-Fi carrying it.
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// Takes one down — a VPN, or a link.
    fn deactivate_connection(&self, active: &zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;

    /// Turns the radio on and off. It is `rfkill` in software.
    #[zbus(property)]
    fn set_wireless_enabled(&self, on: bool) -> zbus::Result<()>;
}

/// One connection that is UP.
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait Active {
    /// `802-11-wireless`, `802-3-ethernet`, `vpn`, `wireguard`, `tun`.
    #[zbus(property)]
    fn type_(&self) -> zbus::Result<String>;

    /// The profile's name, which is what a person called it.
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;

    /// Is it a VPN in NetworkManager's own sense? WireGuard is NOT: it is a
    /// device type, and it answers `false` here while being exactly the
    /// thing a person means by "the VPN is up". The rule that reads both is
    /// `vpn_active`.
    #[zbus(property)]
    fn vpn(&self) -> zbus::Result<bool>;

    /// Where it stands: 2 is activated.
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait Device {
    /// `NM_DEVICE_TYPE`: 1 cable, 2 radio.
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;

    /// `NM_DEVICE_STATE`.
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    /// `wlan0`, `enp1s0`.
    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;

    /// Where this device's IPv4 configuration lives — addresses, gateway and
    /// resolvers. `/` while the device has none, which is how NetworkManager
    /// says "not configured" and NOT an error.
    #[zbus(property)]
    fn ip4_config(&self) -> zbus::Result<OwnedObjectPath>;

    /// Takes down whatever is active on this device.
    fn disconnect(&self) -> zbus::Result<()>;
}

/// One device's IPv4 configuration.
///
/// THE ADDRESS THE MACHINE ACTUALLY HAS, and not the one a profile asks for:
/// a DHCP lease, a link-local fallback and a static address all arrive here
/// the same way, which is why the screen reads this and never the profile.
/// The MAC is deliberately NOT read — the interface name already says which
/// device a row is about, and one identifier fewer is one fewer to keep out
/// of a screenshot.
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.IP4Config",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait Ip4 {
    /// One entry per address, each with `address` and `prefix`.
    #[zbus(property)]
    fn address_data(&self) -> zbus::Result<Vec<HashMap<String, zbus::zvariant::OwnedValue>>>;

    /// The way out of this device, or empty when it has none.
    #[zbus(property)]
    fn gateway(&self) -> zbus::Result<String>;

    /// One entry per resolver, each with `address`.
    #[zbus(property)]
    fn nameserver_data(&self) -> zbus::Result<Vec<HashMap<String, zbus::zvariant::OwnedValue>>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait Radio {
    /// Asks for a scan. The result does NOT come back through here: it shows up in the
    /// access point list a few seconds later.
    fn request_scan(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;

    /// Everything the radio can see right now.
    fn get_all_access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// The access point in use, or `/` when there is none.
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<OwnedObjectPath>;

    /// What the radio is DOING: 2 is joined to a network, 3 is BEING one.
    /// It is the only way to tell a hotspot from a connection — the active
    /// connection's type is `802-11-wireless` either way.
    #[zbus(property)]
    fn mode(&self) -> zbus::Result<u32>;

    /// The negotiated rate, in kb/s.
    #[zbus(property)]
    fn bitrate(&self) -> zbus::Result<u32>;

    /// When the last scan ended, in `CLOCK_BOOTTIME` milliseconds.
    ///
    /// It is the ONLY honest signal that the radio has finished: `RequestScan`
    /// returns at once, and the access points appear in NetworkManager's list
    /// seconds later. Watching this property is what lets the view re-read
    /// exactly once, when there is something new — instead of polling on a timer
    /// and spending a frame on a list that has not moved.
    #[zbus(property)]
    fn last_scan(&self) -> zbus::Result<i64>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wired",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait Wired {
    /// Link speed, in Mb/s.
    #[zbus(property)]
    fn speed(&self) -> zbus::Result<u32>;

    /// Is there a cable plugged in?
    #[zbus(property)]
    fn carrier(&self) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait AccessPoint {
    /// The SSID, raw. **It is not text**: the standard allows any byte, and there is
    /// a router in the world with an SSID that is not valid UTF-8.
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;

    /// 0 to 100.
    #[zbus(property)]
    fn strength(&self) -> zbus::Result<u8>;

    /// In MHz. It is where the band comes from.
    #[zbus(property)]
    fn frequency(&self) -> zbus::Result<u32>;

    /// The access point's ceiling, in kb/s.
    #[zbus(property)]
    fn max_bitrate(&self) -> zbus::Result<u32>;

    /// `NM_802_11_AP_FLAGS`.
    #[zbus(property)]
    fn flags(&self) -> zbus::Result<u32>;

    /// WPA1. Zero when there is none.
    #[zbus(property)]
    fn wpa_flags(&self) -> zbus::Result<u32>;

    /// WPA2/WPA3.
    #[zbus(property)]
    fn rsn_flags(&self) -> zbus::Result<u32>;
}

#[zbus::proxy(
    interface = "org.ronoctua.Network1",
    default_service = "org.ronoctua.agent",
    default_path = "/org/ronoctua/Network"
)]
pub trait Vault {
    /// Asks the daemon to seal this password into the TPM.
    ///
    /// What seals is root: `/dev/tpmrm0` does not belong to the user, and through one
    /// restricted method root seals and nothing more — instead of giving the whole TPM to the
    /// user through the `tss` group, forever and for anything.
    fn store(&self, ssid: &str, password: &str) -> zbus::Result<()>;

    /// Deletes this network's blob.
    fn forget(&self, ssid: &str) -> zbus::Result<()>;

    /// Is there a stored password for this network?
    fn has(&self, ssid: &str) -> zbus::Result<bool>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.AgentManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/AgentManager"
)]
pub trait AgentManager {
    /// Announces this process as a secret agent.
    ///
    /// NetworkManager starts asking the password of whoever registered here,
    /// instead of demanding it in the profile file.
    fn register(&self, identifier: &str) -> zbus::Result<()>;

    /// Leaves the list. Calling it at shutdown keeps NM from waiting for an answer
    /// from a process that has already died.
    fn unregister(&self) -> zbus::Result<()>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
pub trait Profiles {
    /// Every saved profile, of any medium.
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
pub trait Profile {
    /// The whole profile. The password does NOT come here — it is a secret, and
    /// NetworkManager only hands it over through `GetSecrets`, which this crate does not
    /// call: there is no reason for a saved password to travel as far as the screen.
    #[allow(clippy::type_complexity)]
    fn get_settings(
        &self,
    ) -> zbus::Result<HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>>;

    /// Deletes the profile. It is "forget this network".
    fn delete(&self) -> zbus::Result<()>;
}
