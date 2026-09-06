//! BlueZ's D-Bus proxies and the value helpers — the only part of this crate
//! that knows the daemon's bus shape.
//!
//! # Why the boundary is here
//!
//! The split `ronoctua_disks::udisks` and `ronoctua_power::upower` already
//! use: this file translates the bus, and `lib.rs` does not import it beyond
//! the helpers. What that buys is that `Device::from_faces` is fed the map
//! `GetManagedObjects` hands back, and the tests feed it an INVENTED one —
//! no adapter, no addresses, no root (invariant 13).
//!
//! # Everything is read through `GetManagedObjects`, and nothing through `Get`
//!
//! BlueZ declares its optional properties with an `exists` predicate: `RSSI`,
//! `TxPower`, `Modalias`, `Icon`, `Class`, `Appearance` and `Name` are simply
//! ABSENT from the map when the daemon has nothing to say, and asking for one
//! with `org.freedesktop.DBus.Properties.Get` answers `No such property` —
//! an error, not an empty value. One call gives the adapters, the devices,
//! their batteries and their transports together, and a missing key is the
//! answer rather than a log line.
//!
//! # The manager lives at `/`, and so do its signals
//!
//! `InterfacesAdded` and `InterfacesRemoved` are emitted from the root, not
//! from `/org/bluez` — so a match rule with `path_namespace=/org/bluez` would
//! see every property change and MISS every arrival and departure. The rules
//! here are by interface, which is what a device coming and going looks like.

// `#[zbus::proxy]` GENERATES a struct per trait, and the workspace's
// `missing_docs` flags them. The `allow` stays on the MODULE, not the crate.
#![allow(missing_docs)]

use std::collections::HashMap;

use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// The service, on the **system** bus — the radio belongs to the machine.
pub const SERVICE: &str = "org.bluez";

/// The object manager's own path.
pub const ROOT: &str = "/";

/// The daemon's own object, where the agent manager lives.
pub const MANAGER: &str = "/org/bluez";

/// The interface an adapter carries.
pub const ADAPTER: &str = "org.bluez.Adapter1";

/// The interface a device carries.
pub const DEVICE: &str = "org.bluez.Device1";

/// A device's battery, when something reports one.
pub const BATTERY: &str = "org.bluez.Battery1";

/// Classic HID — an LE keyboard does not carry it.
pub const INPUT: &str = "org.bluez.Input1";

/// An audio stream, on a child object of the device.
pub const TRANSPORT: &str = "org.bluez.MediaTransport1";

/// One object's properties, as the manager hands them back.
pub type Faces = HashMap<String, OwnedValue>;

/// Every object, with every interface it carries.
pub type Managed = HashMap<OwnedObjectPath, HashMap<String, Faces>>;

/// The object manager at `/`: one call for the whole radio.
#[zbus::proxy(
    interface = "org.freedesktop.DBus.ObjectManager",
    default_service = "org.bluez",
    default_path = "/"
)]
pub trait Objects {
    /// Adapters, devices, batteries and transports, in one answer.
    fn get_managed_objects(&self) -> zbus::Result<Managed>;
}

/// One radio.
#[zbus::proxy(interface = "org.bluez.Adapter1", default_service = "org.bluez")]
pub trait Adapter {
    /// Starts looking. The answer comes as devices appearing on the bus.
    fn start_discovery(&self) -> zbus::Result<()>;

    /// Stops looking. Discovery is reference-counted per client, so a second
    /// stop from a client that never started answers `Failed`.
    fn stop_discovery(&self) -> zbus::Result<()>;

    /// Forgets a device: its keys leave `/var/lib/bluetooth`.
    fn remove_device(&self, device: &zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;

    /// Narrows what discovery reports. `Transport` is `auto`, `bredr` or
    /// `le`; `DuplicateData = false` is what quiets the advertisement storm.
    fn set_discovery_filter(
        &self,
        filter: HashMap<&str, &zbus::zvariant::Value<'_>>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn powered(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn set_powered(&self, on: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_discoverable(&self, on: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_discoverable_timeout(&self, seconds: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_pairable(&self, on: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_pairable_timeout(&self, seconds: u32) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_alias(&self, alias: &str) -> zbus::Result<()>;
}

/// One device.
#[zbus::proxy(interface = "org.bluez.Device1", default_service = "org.bluez")]
pub trait Device {
    /// Connects every profile it has. BLOCKS for seconds — this is the whole
    /// reason the orders live on a thread of their own.
    fn connect(&self) -> zbus::Result<()>;

    /// Drops the link.
    fn disconnect(&self) -> zbus::Result<()>;

    /// Connects ONE profile, by its service id.
    fn connect_profile(&self, uuid: &str) -> zbus::Result<()>;

    /// Drops one profile.
    fn disconnect_profile(&self, uuid: &str) -> zbus::Result<()>;

    /// Pairs. The questions come back through the agent.
    fn pair(&self) -> zbus::Result<()>;

    /// Takes the question back — the only method here that does not block.
    fn cancel_pairing(&self) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_trusted(&self, on: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_blocked(&self, on: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_wake_allowed(&self, on: bool) -> zbus::Result<()>;

    #[zbus(property)]
    fn set_alias(&self, alias: &str) -> zbus::Result<()>;
}

/// Where an agent is registered.
#[zbus::proxy(
    interface = "org.bluez.AgentManager1",
    default_service = "org.bluez",
    default_path = "/org/bluez"
)]
pub trait AgentManager {
    /// Hands bluetoothd an object of ours to ask its questions of.
    fn register_agent(
        &self,
        agent: &zbus::zvariant::ObjectPath<'_>,
        capability: &str,
    ) -> zbus::Result<()>;

    /// Gives it back.
    fn unregister_agent(&self, agent: &zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;

    /// Asks to be the one asked when no other agent answers.
    fn request_default_agent(&self, agent: &zbus::zvariant::ObjectPath<'_>) -> zbus::Result<()>;
}

/// A property that may not be there, as a `bool`.
#[must_use]
pub fn flag(f: &Faces, name: &str) -> bool {
    f.get(name)
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
}

/// A property that may not be there, as a `String`.
#[must_use]
pub fn text(f: &Faces, name: &str) -> String {
    f.get(name)
        .and_then(|v| <&str>::try_from(v).ok())
        .unwrap_or_default()
        .to_owned()
}

/// A property that may not be there, as a `String`, absent when it is.
#[must_use]
pub fn maybe_text(f: &Faces, name: &str) -> Option<String> {
    let s = text(f, name);
    (!s.is_empty()).then_some(s)
}

/// A `u8` property — `Percentage`, `Version`.
#[must_use]
pub fn small(f: &Faces, name: &str) -> Option<u8> {
    f.get(name).and_then(|v| u8::try_from(v).ok())
}

/// A `u16` property — `Appearance`, `Manufacturer`.
#[must_use]
pub fn medium(f: &Faces, name: &str) -> Option<u16> {
    f.get(name).and_then(|v| u16::try_from(v).ok())
}

/// A `u32` property — `Class`, `DiscoverableTimeout`.
#[must_use]
pub fn number(f: &Faces, name: &str) -> Option<u32> {
    f.get(name).and_then(|v| u32::try_from(v).ok())
}

/// An `i16` property — `RSSI` and `TxPower`, both of which are absent until
/// the device has been heard from.
#[must_use]
pub fn signed(f: &Faces, name: &str) -> Option<i16> {
    f.get(name).and_then(|v| i16::try_from(v).ok())
}

/// A list of strings — `UUIDs`, `Roles`.
#[must_use]
pub fn words(f: &Faces, name: &str) -> Vec<String> {
    f.get(name)
        .and_then(|v| Vec::<String>::try_from(v.try_clone().ok()?).ok())
        .unwrap_or_default()
}

/// An object path property — `Adapter`, a transport's `Device`.
#[must_use]
pub fn path(f: &Faces, name: &str) -> Option<String> {
    f.get(name)
        .and_then(|v| OwnedObjectPath::try_from(v.try_clone().ok()?).ok())
        .map(|p| p.as_str().to_owned())
}
