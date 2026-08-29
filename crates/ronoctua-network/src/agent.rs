//! NetworkManager's secret agent.
//!
//! # The idea, in one sentence
//!
//! The keyfile stops containing the password (`psk-flags=1`), and when
//! NetworkManager needs to connect it **asks over D-Bus**. What answers
//! is this, opening the blob sealed in the TPM.
//!
//! It is the mechanism NM itself offers for this — the same one GNOME
//! Keyring uses — and not a workaround around it.
//!
//! # One job only
//!
//! The agent does **not** receive the password from anyone. `AddAndActivateConnection`
//! accepts the password in the call itself, and the `psk-flags` decide only what gets
//! **persisted**. So the shell still hands the password to NM at connection
//! time, and this daemon only comes in at the next boot, when there is
//! nobody left typing.
//!
//! # Failing here is not failing
//!
//! With no blob, no TPM or with the TPM refusing, the agent answers `NoSecrets`.
//! To NetworkManager that means "ask someone else, or ask the person"
//! — which is exactly the flow for whoever has never connected to that network. The network is
//! not tied to this daemon being alive; it is tied to **someone** being able to
//! answer.

use std::collections::HashMap;

use zbus::zvariant::{OwnedValue, Value};

use crate::secret;

/// Where NetworkManager expects to find an agent.
///
/// The path is fixed by its specification: an agent at another path is an
/// agent NM never finds.
pub const AGENT_PATH: &str = "/org/freedesktop/NetworkManager/SecretAgent";

/// Our own object, for the shell to request sealing.
pub const RONOCTUA_PATH: &str = "/org/ronoctua/Network";

/// How we identify ourselves to NetworkManager.
pub const IDENTIFIER: &str = "org.ronoctua.agent";

/// `NM_SECRET_AGENT_GET_SECRETS_FLAG_REQUEST_NEW` — "what you have is
/// wrong, ask for another".
const ASK_AGAIN: u32 = 0x2;

/// The keyfile section where the WPA password lives.
const SECURITY_SECTION: &str = "802-11-wireless-security";

/// The errors NetworkManager understands from an agent.
///
/// The error's name on the bus matters: `NoSecrets` makes NM move on to the
/// next agent or ask the person. A generic error would be treated as an
/// agent failure, and the diagnosis would point at the wrong place.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.NetworkManager.SecretAgent.Error")]
pub enum Failure {
    /// We do not have the secret — ask someone who does.
    NoSecrets(String),
    /// Not a connection this agent knows anything about.
    NotSupported(String),
}

/// The object exported to NetworkManager.
pub struct Agent;

/// The SSID out of the dictionary NM sends.
///
/// It arrives as `ay`, and not as text — for the usual reason: the standard
/// allows any byte at all in an SSID.
fn ssid_of(connection: &HashMap<String, HashMap<String, OwnedValue>>) -> Option<String> {
    let v = connection.get("802-11-wireless")?.get("ssid")?;
    Vec::<u8>::try_from(v.clone())
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
}

#[zbus::interface(name = "org.freedesktop.NetworkManager.SecretAgent")]
impl Agent {
    /// NetworkManager asking for the password.
    ///
    /// # Errors
    ///
    /// `NoSecrets` when there is no blob, the TPM refuses, or NM has warned that the
    /// stored secret is wrong.
    fn get_secrets(
        &self,
        connection: HashMap<String, HashMap<String, OwnedValue>>,
        _path: zbus::zvariant::OwnedObjectPath,
        section: String,
        _hints: Vec<String>,
        flags: u32,
    ) -> Result<HashMap<String, HashMap<String, OwnedValue>>, Failure> {
        if section != SECURITY_SECTION {
            return Err(Failure::NotSupported(format!(
                "this agent only knows about `{SECURITY_SECTION}`, and `{section}` was asked for"
            )));
        }

        // NM WARNING THAT WHAT WE HAVE IS NO GOOD.
        //
        // Without this branch, a password changed on the router would cause a loop:
        // NM asks, we hand over the old one, it fails, it asks again. Returning
        // `NoSecrets` here sends the question to whoever can type the new one.
        if flags & ASK_AGAIN != 0 {
            return Err(Failure::NoSecrets(
                "NetworkManager asked for a new password; the stored one does not do".into(),
            ));
        }

        let ssid =
            ssid_of(&connection).ok_or_else(|| Failure::NotSupported("conexão sem SSID".into()))?;

        let password = secret::open(&ssid).map_err(|e| {
            // The reason goes to the daemon's log, and NOT to the bus: the
            // `systemd-creds` message may mention the TPM's path and state,
            // and the one asking is a service, not a person.
            tracing::info!(error = %e, "no password stored for this network");
            Failure::NoSecrets("there is no password stored for this network".into())
        })?;

        let mut security = HashMap::new();
        security.insert(
            "psk".to_owned(),
            OwnedValue::try_from(Value::from(password))
                .map_err(|e| Failure::NoSecrets(format!("pack the password: {e}")))?,
        );
        Ok(HashMap::from([(SECURITY_SECTION.to_owned(), security)]))
    }

    /// Request cancelled. There is nothing to undo: the read is synchronous and short.
    fn cancel_get_secrets(&self, _path: zbus::zvariant::OwnedObjectPath, _section: String) {}

    /// NM asking us to STORE what it has.
    ///
    /// We do nothing here, and that is deliberate: who decides to store is the person,
    /// through a key in the footer, and the sealing comes in through [`Ronoctua::store`].
    /// Sealing everything NM sends would make "do not remember" store it anyway.
    fn save_secrets(
        &self,
        _connection: HashMap<String, HashMap<String, OwnedValue>>,
        _path: zbus::zvariant::OwnedObjectPath,
    ) {
    }

    /// The connection was deleted: the blob goes with it.
    ///
    /// Without this, "forget this network" would leave the sealed password behind — an
    /// orphan secret nobody knows exists any more.
    fn delete_secrets(
        &self,
        connection: HashMap<String, HashMap<String, OwnedValue>>,
        _path: zbus::zvariant::OwnedObjectPath,
    ) {
        if let Some(ssid) = ssid_of(&connection)
            && let Err(e) = secret::erase(&ssid)
        {
            tracing::warn!(error = %e, "could not erase the network's secret");
        }
    }
}

/// Our own interface, for the footer to request sealing.
///
/// # Why the shell does not seal on its own
///
/// `/dev/tpmrm0` belongs to root. Putting the user in the `tss` group would give them the whole
/// TPM, forever, for anything. Here root seals and nothing more —
/// and who may call is decided by the bus policy in
/// `nix/modules/network.nix`, not by this code.
pub struct Ronoctua;

#[zbus::interface(name = "org.ronoctua.Network1")]
impl Ronoctua {
    /// Seals this network's password into the TPM.
    ///
    /// # Errors
    ///
    /// The TPM refused, or the directory could not be created.
    fn store(&self, ssid: String, password: String) -> zbus::fdo::Result<()> {
        secret::seal(&ssid, &password).map_err(|e| {
            tracing::warn!(error = %e, "could not seal the password");
            // The TPM's message goes to the log; for the caller, enough
            // to decide what to do.
            zbus::fdo::Error::Failed("the TPM refused to store the password".into())
        })?;
        tracing::info!("password sealed in the TPM");
        Ok(())
    }

    /// Deletes this network's blob.
    ///
    /// # Errors
    ///
    /// The file exists and could not be removed.
    fn forget(&self, ssid: String) -> zbus::fdo::Result<()> {
        secret::erase(&ssid).map_err(|e| zbus::fdo::Error::Failed(format!("erase the secret: {e}")))
    }

    /// Is there a stored password for this network?
    ///
    /// The footer uses this to say, in the list, which networks reconnect
    /// on their own — and not to confuse "saved in NetworkManager" with "the password
    /// exists".
    fn has(&self, ssid: String) -> bool {
        secret::ha(&ssid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connection_with_ssid(ssid: &[u8]) -> HashMap<String, HashMap<String, OwnedValue>> {
        let v = OwnedValue::try_from(Value::from(ssid.to_vec())).unwrap();
        HashMap::from([(
            "802-11-wireless".to_owned(),
            HashMap::from([("ssid".to_owned(), v)]),
        )])
    }

    #[test]
    fn the_ssid_comes_from_the_nm_dictionary() {
        assert_eq!(
            ssid_of(&connection_with_ssid(b"home")).as_deref(),
            Some("home")
        );
    }

    #[test]
    fn a_connection_with_no_wireless_section_has_no_ssid() {
        assert!(ssid_of(&HashMap::new()).is_none());
    }

    #[test]
    fn asking_again_does_not_give_back_the_stored_password() {
        // The defect this branch avoids: a password changed on the router becomes a
        // loop — NM asks, we hand over the old one, it fails, it asks again.
        // Without reaching the disk: the flag decides first.
        let r = Agent.get_secrets(
            connection_with_ssid(b"home"),
            zbus::zvariant::OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Settings/1")
                .unwrap(),
            SECURITY_SECTION.to_owned(),
            Vec::new(),
            ASK_AGAIN,
        );
        assert!(matches!(r, Err(Failure::NoSecrets(_))));
    }

    #[test]
    fn a_section_that_is_not_wifi_is_refused_as_unsupported() {
        // `NotSupported` and not `NoSecrets`: saying "I do not have it" for a VPN
        // section would make NM keep asking this agent.
        let r = Agent.get_secrets(
            connection_with_ssid(b"home"),
            zbus::zvariant::OwnedObjectPath::try_from("/org/freedesktop/NetworkManager/Settings/1")
                .unwrap(),
            "vpn".to_owned(),
            Vec::new(),
            0,
        );
        assert!(matches!(r, Err(Failure::NotSupported(_))));
    }
}
