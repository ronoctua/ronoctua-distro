//! What the security centre's firewall asks of the stored profiles.
//!
//! The firewall gives each network a profile (home or public) by the UUID of
//! the connection it is on, and it has two questions for NetworkManager that
//! nobody else in the tree asks: which connections exist at all (the first
//! time it runs, every one of them becomes a home network, so today's
//! networks keep today's behaviour), and whether this machine SERVES the
//! network a connection makes — a hotspot, a shared cable — which is never a
//! public network: the machine is its router, and its clients need the
//! machine's DHCP and DNS.
//!
//! Asked here, of the daemon, and never read from its files: this crate is
//! the one that talks to NetworkManager, and a profile the daemon holds only
//! in memory is not in any file.

use anyhow::Result;

use crate::{Configuration, Manager, nm};

/// A stored profile, as the firewall needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// NetworkManager's UUID of the profile.
    pub uuid: String,
    /// Whether this machine serves the network the profile makes:
    /// `ipv4.method` or `ipv6.method` is `shared`.
    pub serves: bool,
}

/// NetworkManager's word for "this machine is the router here".
const SHARED: &str = "shared";

/// The facts of one profile's settings; `None` when it names no UUID.
fn stored_of(cfg: &Configuration) -> Option<Stored> {
    let text = |section: &str, field: &str| {
        cfg.get(section)
            .and_then(|s| s.get(field))
            .and_then(|v| String::try_from(v.clone()).ok())
    };
    let uuid = text("connection", "uuid").filter(|u| !u.is_empty())?;
    let serves = ["ipv4", "ipv6"]
        .iter()
        .any(|family| text(family, "method").as_deref() == Some(SHARED));
    Some(Stored { uuid, serves })
}

impl Manager {
    /// Every profile the daemon has stored, with its UUID and whether this
    /// machine serves its network.
    ///
    /// # Errors
    ///
    /// The bus did not answer. A profile that will not read is SKIPPED, as in
    /// [`Manager::saved_profiles`]: one unreadable recipe must not empty the
    /// list.
    pub fn stored(&self) -> Result<Vec<Stored>> {
        let profiles = nm::ProfilesProxyBlocking::new(&self.connection)?;
        let mut out = Vec::new();
        for path in profiles.list_connections()? {
            let Ok(cfg) = self.profile(&path)?.get_settings() else {
                continue;
            };
            out.extend(stored_of(&cfg));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use zbus::zvariant::{OwnedValue, Value};

    use super::*;

    fn settings(fields: &[(&str, &str, &str)]) -> Configuration {
        let mut cfg: Configuration = HashMap::new();
        for (section, field, value) in fields {
            cfg.entry((*section).to_owned()).or_default().insert(
                (*field).to_owned(),
                OwnedValue::try_from(Value::from(*value)).unwrap(),
            );
        }
        cfg
    }

    #[test]
    fn a_hotspot_is_a_network_this_machine_serves() {
        let uuid = "3f2a9c1e-0b7d-4e55-9a51-2c8d7e6f1a0b";
        assert_eq!(
            stored_of(&settings(&[
                ("connection", "uuid", uuid),
                ("ipv4", "method", "shared"),
                ("ipv6", "method", "ignore"),
            ])),
            Some(Stored {
                uuid: uuid.to_owned(),
                serves: true
            })
        );
        assert_eq!(
            stored_of(&settings(&[
                ("connection", "uuid", uuid),
                ("ipv6", "method", "shared"),
            ]))
            .map(|s| s.serves),
            Some(true),
            "IPv6 alone is enough"
        );
        assert_eq!(
            stored_of(&settings(&[
                ("connection", "uuid", uuid),
                ("ipv4", "method", "auto"),
            ]))
            .map(|s| s.serves),
            Some(false),
            "a network joined is not one served"
        );
        assert_eq!(
            stored_of(&settings(&[("ipv4", "method", "shared")])),
            None,
            "no UUID, nothing to place"
        );
        assert_eq!(stored_of(&settings(&[("connection", "uuid", "")])), None);
    }
}
