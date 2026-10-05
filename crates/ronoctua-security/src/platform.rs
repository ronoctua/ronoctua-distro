//! The device's facts, read only: Secure Boot, the TPM, and fwupd.
//!
//! Secure Boot from the EFI variable (`SecureBoot-8be4df61-…`, four bytes of
//! attributes and one of value), the TPM from `/sys/class/tpm/tpm0`, firmware
//! from fwupd over D-Bus (the HSI level and the updates available). Firmware
//! is never INSTALLED from here: that would be a new kind of imperative act,
//! and it stays out (the `fwupd-refresh` timer is off too).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::{Refusal, Why, refused};

/// Where the EFI variables are.
pub const EFIVARS: &str = "/sys/firmware/efi/efivars";
/// The Secure Boot variable, under [`EFIVARS`].
pub const SECURE_BOOT_VARIABLE: &str = "SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
/// The Setup Mode variable, under [`EFIVARS`].
pub const SETUP_MODE_VARIABLE: &str = "SetupMode-8be4df61-93ca-11d2-aa0d-00e098032b8c";
/// The TPM's major version file.
pub const TPM_VERSION: &str = "/sys/class/tpm/tpm0/tpm_version_major";

/// Secure Boot's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecureBoot {
    /// On.
    Enabled,
    /// Off.
    Disabled,
    /// The firmware is in setup mode (keys can be enrolled).
    SetupMode,
    /// Not an EFI boot: there is no Secure Boot.
    Unsupported,
}

impl SecureBoot {
    /// The state the two variables say. Pure: `efi` is whether the machine
    /// booted by EFI, the others the variables' bytes when present.
    #[must_use]
    pub fn of(efi: bool, secure_boot: Option<&[u8]>, setup_mode: Option<&[u8]>) -> Self {
        let on = |bytes: Option<&[u8]>| bytes.and_then(|b| b.get(4)).is_some_and(|&v| v == 1);
        if !efi {
            Self::Unsupported
        } else if on(setup_mode) {
            Self::SetupMode
        } else if on(secure_boot) {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

/// Secure Boot on this machine.
#[must_use]
pub fn secure_boot() -> SecureBoot {
    let vars = Path::new(EFIVARS);
    SecureBoot::of(
        vars.is_dir(),
        std::fs::read(vars.join(SECURE_BOOT_VARIABLE))
            .ok()
            .as_deref(),
        std::fs::read(vars.join(SETUP_MODE_VARIABLE))
            .ok()
            .as_deref(),
    )
}

/// The TPM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Tpm {
    /// No TPM.
    Absent,
    /// A TPM of this major version.
    Present {
        /// 1 or 2.
        version: u8,
    },
}

impl Tpm {
    /// The TPM its version file says (`None`: no file). Pure.
    #[must_use]
    pub fn of(version_file: Option<&str>) -> Self {
        version_file
            .and_then(|v| v.trim().parse().ok())
            .map_or(Self::Absent, |version| Self::Present { version })
    }
}

/// The TPM of this machine.
#[must_use]
pub fn tpm() -> Tpm {
    Tpm::of(std::fs::read_to_string(TPM_VERSION).ok().as_deref())
}

/// fwupd's bus name, object and interface.
pub const FWUPD: &str = "org.freedesktop.fwupd";
/// A device fwupd can update (`FWUPD_DEVICE_FLAG_UPDATABLE`).
pub const DEVICE_UPDATABLE: u64 = 1 << 1;
/// An attribute that passed (`FWUPD_SECURITY_ATTR_FLAG_SUCCESS`).
pub const ATTRIBUTE_SUCCESS: u64 = 1 << 0;
/// An attribute a newer one replaced (`FWUPD_SECURITY_ATTR_FLAG_OBSOLETED`).
pub const ATTRIBUTE_OBSOLETED: u64 = 1 << 1;

/// One of fwupd's host security attributes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attribute {
    /// Its id (`org.fwupd.hsi.Uefi.SecureBoot`) — data.
    pub id: String,
    /// Its name as fwupd says it (English) — data.
    pub name: String,
    /// The HSI level it counts for; `None` for a runtime attribute.
    pub level: Option<u32>,
    /// Whether it passed.
    pub passed: bool,
}

/// What fwupd says of the firmware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Firmware {
    /// The Host Security ID level (`HSI:2`, `HSI:3!` with a runtime issue),
    /// as fwupd computed it — data.
    pub hsi: Option<String>,
    /// How many devices have an update available (from the metadata fwupd
    /// already has: this machine never refreshes it on its own).
    pub updates: u32,
    /// Its host security attributes (`GetHostSecurityAttrs`), the ones a
    /// newer attribute replaced left out.
    pub attributes: Vec<Attribute>,
}

/// The HSI level of fwupd's `HostSecurityId` (`HSI:3! (v2.1.7)` is
/// `HSI:3!`); `None` when fwupd has computed none. Pure.
#[must_use]
pub fn hsi_of(host_security_id: &str) -> Option<String> {
    host_security_id
        .split_whitespace()
        .next()
        .filter(|level| level.starts_with("HSI:"))
        .map(str::to_owned)
}

type Dict = std::collections::HashMap<String, zbus::zvariant::OwnedValue>;

fn text_of(dict: &Dict, key: &str) -> Option<String> {
    dict.get(key)
        .and_then(|v| <&str>::try_from(v).ok())
        .map(str::to_owned)
}

fn u64_of(dict: &Dict, key: &str) -> Option<u64> {
    dict.get(key).and_then(|v| u64::try_from(v).ok())
}

fn u32_of(dict: &Dict, key: &str) -> Option<u32> {
    dict.get(key).and_then(|v| u32::try_from(v).ok())
}

/// An attribute of fwupd's dictionary; `None` for one with no id, and for
/// one a newer attribute replaced.
fn attribute_of(dict: &Dict) -> Option<Attribute> {
    let flags = u64_of(dict, "Flags").unwrap_or(0);
    if flags & ATTRIBUTE_OBSOLETED != 0 {
        return None;
    }
    Some(Attribute {
        id: text_of(dict, "AppstreamId")?,
        name: text_of(dict, "Name").unwrap_or_default(),
        level: u32_of(dict, "HsiLevel").filter(|&l| l > 0),
        passed: flags & ATTRIBUTE_SUCCESS != 0,
    })
}

/// fwupd's facts, read only. Blocks on D-Bus: a thread.
///
/// # Errors
///
/// [`Why::Unreadable`] when fwupd cannot be reached.
pub fn firmware() -> Result<Firmware, Refusal> {
    let unreadable = |e: zbus::Error| {
        eprintln!("fwupd: {e}");
        refused(Why::Unreadable)
    };
    let connection = zbus::blocking::Connection::system().map_err(unreadable)?;
    let daemon = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy<'_>>::new(&connection)
        .destination(FWUPD)
        .and_then(|b| b.path("/"))
        .and_then(|b| b.interface(FWUPD))
        .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
        .and_then(zbus::blocking::proxy::Builder::build)
        .map_err(unreadable)?;
    let attributes: Vec<Dict> = daemon
        .call("GetHostSecurityAttrs", &())
        .map_err(unreadable)?;
    let hsi = daemon
        .get_property::<String>("HostSecurityId")
        .ok()
        .and_then(|id| hsi_of(&id));
    let devices: Vec<Dict> = daemon.call("GetDevices", &()).map_err(unreadable)?;
    let mut updates = 0;
    for device in &devices {
        let updatable = u64_of(device, "Flags").is_some_and(|f| f & DEVICE_UPDATABLE != 0);
        let Some(id) = text_of(device, "DeviceId").filter(|_| updatable) else {
            continue;
        };
        // "Nothing to do" is fwupd's way of saying none; any answer that is
        // not a list is none too.
        let upgrades: Result<Vec<Dict>, _> = daemon.call("GetUpgrades", &(id.as_str(),));
        if upgrades.is_ok_and(|u| !u.is_empty()) {
            updates += 1;
        }
    }
    Ok(Firmware {
        hsi,
        updates,
        attributes: attributes.iter().filter_map(attribute_of).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facts_are_read_from_their_bytes() {
        let on = [6, 0, 0, 0, 1];
        let off = [6, 0, 0, 0, 0];
        assert_eq!(
            SecureBoot::of(true, Some(&on), Some(&off)),
            SecureBoot::Enabled
        );
        assert_eq!(SecureBoot::of(true, Some(&off), None), SecureBoot::Disabled);
        assert_eq!(
            SecureBoot::of(true, Some(&off), Some(&on)),
            SecureBoot::SetupMode
        );
        assert_eq!(SecureBoot::of(false, None, None), SecureBoot::Unsupported);
        assert_eq!(Tpm::of(Some("2\n")), Tpm::Present { version: 2 });
        assert_eq!(Tpm::of(None), Tpm::Absent);
        assert_eq!(Tpm::of(Some("")), Tpm::Absent);
    }

    /// fwupd on the machine that runs the test, read only — by hand
    /// (`cargo test -- --ignored`): a build has no system bus.
    #[test]
    #[ignore = "needs this machine's fwupd"]
    fn the_machines_firmware_is_read() {
        let firmware = firmware().unwrap();
        eprintln!(
            "{:?}, {} update(s), {} attribute(s), {} failed",
            firmware.hsi,
            firmware.updates,
            firmware.attributes.len(),
            firmware.attributes.iter().filter(|a| !a.passed).count()
        );
        assert!(
            firmware
                .hsi
                .as_deref()
                .is_some_and(|h| h.starts_with("HSI:"))
        );
        assert!(!firmware.attributes.is_empty());
    }

    #[test]
    fn fwupd_is_read_as_it_says_it() {
        use zbus::zvariant::{OwnedValue, Value};
        assert_eq!(hsi_of("HSI:3! (v2.1.7)").as_deref(), Some("HSI:3!"));
        assert_eq!(hsi_of("HSI:0").as_deref(), Some("HSI:0"));
        assert_eq!(hsi_of(""), None);
        assert_eq!(hsi_of("unknown"), None);
        let owned = |v: Value<'_>| OwnedValue::try_from(v).unwrap();
        let dict = |id: &str, level: Option<u32>, flags: u64| {
            let mut d = Dict::new();
            d.insert("AppstreamId".into(), owned(Value::from(id)));
            d.insert("Name".into(), owned(Value::from("Some check")));
            d.insert("Flags".into(), owned(Value::from(flags)));
            if let Some(level) = level {
                d.insert("HsiLevel".into(), owned(Value::from(level)));
            }
            d
        };
        assert_eq!(
            attribute_of(&dict("org.fwupd.hsi.Tpm.Version20", Some(1), 1)),
            Some(Attribute {
                id: "org.fwupd.hsi.Tpm.Version20".into(),
                name: "Some check".into(),
                level: Some(1),
                passed: true,
            })
        );
        let secure_boot = attribute_of(&dict("org.fwupd.hsi.Uefi.SecureBoot", None, 5120)).unwrap();
        assert!(!secure_boot.passed);
        assert_eq!(secure_boot.level, None, "a runtime attribute");
        assert_eq!(attribute_of(&dict("old", Some(2), 3)), None, "obsoleted");
        assert_eq!(attribute_of(&Dict::new()), None);
    }
}
