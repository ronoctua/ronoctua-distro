//! What the security centre shows of a connection that is up: how the
//! machine's hardware address shows on it, and whether its DNS is encrypted.
//!
//! Read from the profile the active connection was made from — asked of the
//! daemon over D-Bus, like every other question this crate answers — and
//! SHOWN, never changed here: a setting a person wants is a change of the
//! profile, which is the network view's.

use anyhow::Result;

use crate::{Configuration, Manager, nm};

/// How the machine's hardware (MAC) address shows on a connection —
/// `cloned-mac-address` of its Wi-Fi or cable section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mac {
    /// The device's own, burnt-in address.
    Permanent,
    /// Whatever the device has now (NetworkManager does not touch it).
    Preserve,
    /// A new random address every time it connects.
    Random,
    /// A random address that stays the same for this network.
    Stable,
    /// An address the person wrote.
    Explicit,
    /// Nothing in the profile: the daemon's default decides.
    Default,
}

impl Mac {
    /// Every one, in the order the keys are written.
    pub const ALL: [Self; 6] = [
        Self::Permanent,
        Self::Preserve,
        Self::Random,
        Self::Stable,
        Self::Explicit,
        Self::Default,
    ];

    /// Its kebab-case name, the suffix of its text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Permanent => "permanent",
            Self::Preserve => "preserve",
            Self::Random => "random",
            Self::Stable => "stable",
            Self::Explicit => "explicit",
            Self::Default => "default",
        }
    }

    /// The mode a profile's value names. Pure.
    #[must_use]
    pub fn of(value: Option<&str>) -> Self {
        match value.map(str::trim) {
            None | Some("") => Self::Default,
            Some("permanent") => Self::Permanent,
            Some("preserve") => Self::Preserve,
            Some("random") => Self::Random,
            Some("stable" | "stable-ssid") => Self::Stable,
            Some(_) => Self::Explicit,
        }
    }
}

/// Whether a connection's DNS goes over TLS — `connection.dns-over-tls`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tls {
    /// Always encrypted, or no DNS.
    Yes,
    /// Encrypted when the server offers it.
    Opportunistic,
    /// Never.
    No,
    /// Nothing in the profile: the resolver's default decides.
    Default,
}

impl Tls {
    /// Every one, in the order the keys are written.
    pub const ALL: [Self; 4] = [Self::Yes, Self::Opportunistic, Self::No, Self::Default];

    /// Its kebab-case name, the suffix of its text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Yes => "yes",
            Self::Opportunistic => "opportunistic",
            Self::No => "no",
            Self::Default => "default",
        }
    }

    /// The mode NetworkManager's number names (`-1` default, `0` no, `1`
    /// opportunistic, `2` yes). Pure.
    #[must_use]
    pub const fn of(value: Option<i32>) -> Self {
        match value {
            Some(2) => Self::Yes,
            Some(1) => Self::Opportunistic,
            Some(0) => Self::No,
            _ => Self::Default,
        }
    }
}

/// One connection that is up, as the security centre shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Privacy {
    /// What the person called the profile.
    pub id: String,
    /// NetworkManager's UUID of the profile.
    pub uuid: String,
    /// `802-11-wireless`, `802-3-ethernet`, …
    pub kind: String,
    /// How the hardware address shows.
    pub mac: Mac,
    /// Whether its DNS is encrypted.
    pub dns: Tls,
}

/// The privacy of a profile's settings; `None` for one that names no UUID.
/// Pure over the settings dictionary.
fn privacy_of(cfg: &Configuration) -> Option<Privacy> {
    let text = |section: &str, field: &str| {
        cfg.get(section)
            .and_then(|s| s.get(field))
            .and_then(|v| String::try_from(v.clone()).ok())
    };
    let number = |section: &str, field: &str| {
        cfg.get(section)
            .and_then(|s| s.get(field))
            .and_then(|v| i32::try_from(v.clone()).ok())
    };
    let uuid = text("connection", "uuid").filter(|u| !u.is_empty())?;
    let kind = text("connection", "type").unwrap_or_default();
    let mac = if kind == "802-11-wireless" {
        text("802-11-wireless", "assigned-mac-address")
            .or_else(|| text("802-11-wireless", "cloned-mac-address"))
    } else {
        text("802-3-ethernet", "assigned-mac-address")
            .or_else(|| text("802-3-ethernet", "cloned-mac-address"))
    };
    Some(Privacy {
        id: text("connection", "id").unwrap_or_default(),
        uuid,
        kind,
        mac: Mac::of(mac.as_deref()),
        dns: Tls::of(number("connection", "dns-over-tls")),
    })
}

impl Manager {
    /// Every connection that is up and is not a tunnel, with how the
    /// hardware address shows on it and whether its DNS is encrypted.
    ///
    /// # Errors
    ///
    /// The bus did not answer. A connection whose profile will not read is
    /// SKIPPED, as in [`Manager::saved_profiles`].
    pub fn privacy(&self) -> Result<Vec<Privacy>> {
        let manager = self.manager()?;
        let mut out = Vec::new();
        for at in manager.active_connections().unwrap_or_default() {
            let Ok(active) = nm::ActiveProxyBlocking::builder(&self.connection)
                .path(at.clone())
                .and_then(zbus::blocking::proxy::Builder::build)
            else {
                continue;
            };
            if active.vpn().unwrap_or(false) {
                continue;
            }
            let Ok(profile) = active.connection() else {
                continue;
            };
            let Ok(cfg) = self.profile(&profile)?.get_settings() else {
                continue;
            };
            out.extend(
                privacy_of(&cfg)
                    .filter(|p| p.kind == "802-11-wireless" || p.kind == "802-3-ethernet"),
            );
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zbus::zvariant::{OwnedValue, Value};

    use super::*;

    fn settings(text: &[(&str, &str, &str)], numbers: &[(&str, &str, i32)]) -> Configuration {
        let mut cfg: Configuration = HashMap::new();
        for (section, field, value) in text {
            cfg.entry((*section).to_owned()).or_default().insert(
                (*field).to_owned(),
                OwnedValue::try_from(Value::from(*value)).unwrap(),
            );
        }
        for (section, field, value) in numbers {
            cfg.entry((*section).to_owned()).or_default().insert(
                (*field).to_owned(),
                OwnedValue::try_from(Value::from(*value)).unwrap(),
            );
        }
        cfg
    }

    /// FAILS IF: a profile's MAC mode or its DNS over TLS stops being read
    /// from the section of its own kind, or a missing value stops reading as
    /// "the default decides".
    #[test]
    fn a_profiles_privacy_is_read_from_its_own_sections() {
        let wifi = settings(
            &[
                ("connection", "uuid", "u-1"),
                ("connection", "id", "home"),
                ("connection", "type", "802-11-wireless"),
                ("802-11-wireless", "cloned-mac-address", "random"),
                ("802-3-ethernet", "cloned-mac-address", "permanent"),
            ],
            &[("connection", "dns-over-tls", 2)],
        );
        let p = privacy_of(&wifi).unwrap();
        assert_eq!((p.mac, p.dns), (Mac::Random, Tls::Yes));
        assert_eq!((p.id.as_str(), p.uuid.as_str()), ("home", "u-1"));
        let cable = settings(
            &[
                ("connection", "uuid", "u-2"),
                ("connection", "type", "802-3-ethernet"),
            ],
            &[],
        );
        let p = privacy_of(&cable).unwrap();
        assert_eq!((p.mac, p.dns), (Mac::Default, Tls::Default));
        assert!(privacy_of(&settings(&[("connection", "id", "x")], &[])).is_none());
        assert_eq!(Mac::of(Some("stable-ssid")), Mac::Stable);
        assert_eq!(Mac::of(Some("02:00:00:00:00:01")), Mac::Explicit);
        assert_eq!(Tls::of(Some(0)), Tls::No);
        assert_eq!(Tls::of(Some(-1)), Tls::Default);
    }
}
