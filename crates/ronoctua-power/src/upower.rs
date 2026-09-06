//! UPower's D-Bus proxies and the value helpers — the only part of this crate
//! that knows the battery daemon's bus shape.
//!
//! # Why the boundary is here
//!
//! The same split as `ronoctua_disks::udisks`: this file translates the bus,
//! and `lib.rs` does not import it beyond the helpers. What that buys is that
//! `Battery::from_props` is fed a property map — the same map `GetAll` hands
//! back — and is tested against one copied off a real machine with the serial
//! number left out.
//!
//! # One `GetAll` per device, not twenty-five property gets
//!
//! A device carries thirty-odd properties, and the footer wants most of them.
//! Reading them one call at a time is thirty round trips for the bytes of one;
//! `org.freedesktop.DBus.Properties.GetAll` answers them together, and it is
//! what the fixture test feeds — the parser and the bus see one shape.
//!
//! # `Serial` is not read
//!
//! A battery's serial number is on the bus and this repository is public
//! (invariant 13). Nothing here needs it; nothing here reads it.

// `#[zbus::proxy]` GENERATES a struct per trait, and the workspace's
// `missing_docs` flags them. The `allow` stays on the MODULE, not the crate.
#![allow(missing_docs)]

use std::collections::HashMap;

use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// The service, on the **system** bus — the battery belongs to the machine.
pub const SERVICE: &str = "org.freedesktop.UPower";

/// The daemon's own object.
pub const ROOT: &str = "/org/freedesktop/UPower";

/// The interface every device carries.
pub const DEVICE: &str = "org.freedesktop.UPower.Device";

/// One object's properties, as `GetAll` hands them back.
pub type Props = HashMap<String, OwnedValue>;

/// The daemon: which devices exist, which one stands for "the battery".
#[zbus::proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
pub trait Upower {
    /// Every device it knows, batteries and chargers and peripherals alike.
    fn enumerate_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// The composite device: what a screen should call "the battery" when a
    /// machine has two, and a device that says `IsPresent = false` when it
    /// has none.
    fn get_display_device(&self) -> zbus::Result<OwnedObjectPath>;

    /// What the daemon will DO at the action level — `HybridSleep`,
    /// `Hibernate`, `PowerOff` — after any fallback it applied on its own.
    fn get_critical_action(&self) -> zbus::Result<String>;

    #[zbus(property)]
    fn on_battery(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn lid_is_closed(&self) -> zbus::Result<bool>;

    #[zbus(property)]
    fn lid_is_present(&self) -> zbus::Result<bool>;
}

/// Everything one object says on one interface, in one round trip.
///
/// # Errors
///
/// The object is not there, or the daemon did not answer.
pub fn get_all(
    connection: &zbus::blocking::Connection,
    service: &str,
    path: &str,
    interface: &str,
) -> anyhow::Result<Props> {
    let proxy = zbus::blocking::fdo::PropertiesProxy::builder(connection)
        .destination(service)?
        .path(path)?
        .build()?;
    let name = zbus::names::InterfaceName::try_from(interface)?;
    Ok(proxy.get_all(name)?)
}

/// Writes one property — the way `ActiveProfile` is set.
///
/// # Errors
///
/// The daemon refused (polkit, or a value it does not offer), or is absent.
pub fn set_one(
    connection: &zbus::blocking::Connection,
    service: &str,
    path: &str,
    interface: &str,
    property: &str,
    value: zbus::zvariant::Value<'_>,
) -> anyhow::Result<()> {
    let proxy = zbus::blocking::fdo::PropertiesProxy::builder(connection)
        .destination(service)?
        .path(path)?
        .build()?;
    let name = zbus::names::InterfaceName::try_from(interface)?;
    proxy.set(name, property, value)?;
    Ok(())
}

/// A `s` property, or the empty string.
#[must_use]
pub fn text_of(v: Option<&OwnedValue>) -> String {
    v.and_then(|v| <String>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// A `b` property, or `false`.
#[must_use]
pub fn flag_of(v: Option<&OwnedValue>) -> bool {
    v.and_then(|v| <bool>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// A `b` property, or `None` when it is not there at all.
///
/// `None` and not `false`: `BatteryAware` only exists from
/// power-profiles-daemon 0.23 on, and a switch drawn as "off" for a daemon
/// that has no such switch would be a control that switches nothing.
#[must_use]
pub fn flag_if_there(v: Option<&OwnedValue>) -> Option<bool> {
    v.and_then(|v| <bool>::try_from(v.clone()).ok())
}

/// A `u` property, or zero.
#[must_use]
pub fn u32_of(v: Option<&OwnedValue>) -> u32 {
    v.and_then(|v| <u32>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// An `i` property, or zero.
#[must_use]
pub fn i32_of(v: Option<&OwnedValue>) -> i32 {
    v.and_then(|v| <i32>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// An `x` property, or zero.
#[must_use]
pub fn i64_of(v: Option<&OwnedValue>) -> i64 {
    v.and_then(|v| <i64>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// A `t` property, or zero.
#[must_use]
pub fn u64_of(v: Option<&OwnedValue>) -> u64 {
    v.and_then(|v| <u64>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// A `d` property, or zero.
#[must_use]
pub fn f64_of(v: Option<&OwnedValue>) -> f64 {
    v.and_then(|v| <f64>::try_from(v.clone()).ok())
        .unwrap_or_default()
}

/// An `aa{sv}` property — a list of dictionaries — or nothing.
#[must_use]
pub fn dicts_of(v: Option<&OwnedValue>) -> Vec<Props> {
    v.and_then(|v| <Vec<Props>>::try_from(v.clone()).ok())
        .unwrap_or_default()
}
