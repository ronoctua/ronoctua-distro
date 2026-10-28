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
//! The agent does **not** receive the password from anyone typing. A password
//! typed joins in memory, kept by NetworkManager itself
//! (`Manager::join_for_this_boot`), and only once that join is up is the
//! password sealed and the profile written to disk with `psk-flags=1`
//! (`Manager::remember`). So this daemon comes in when NetworkManager next
//! needs the password — a link that drops, and every boot — when there is
//! nobody left typing.
//!
//! It cannot be the other way round: NetworkManager does not take a password
//! marked agent-owned from the call that adds the profile — it asks the
//! agents, at activation (measured on NetworkManager 1.56) — and a password
//! just typed has no blob yet.
//!
//! # Failing here is not failing
//!
//! With no blob, no TPM or with the TPM refusing, the agent answers `NoSecrets`.
//! To NetworkManager that means "ask someone else, or ask the person"
//! — which is exactly the flow for whoever has never connected to that network. The network is
//! not tied to this daemon being alive; it is tied to **someone** being able to
//! answer.

use std::collections::HashMap;
use std::time::{Duration, Instant};

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

/// The setting NetworkManager asks a WireGuard private key under. The person
/// has one key for every location (`crate::secret::seal_wireguard`), carried
/// with `private-key-flags=1`, so this is the one other section this agent
/// answers.
const WIREGUARD_SECTION: &str = "wireguard";

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

/// Whether the connection leaves its Wi-Fi password to an agent
/// (`psk-flags` [`crate::KEPT_BY_THE_AGENT`]) — the only kind whose password
/// is this agent's blob. Pure.
fn left_to_this_agent(connection: &HashMap<String, HashMap<String, OwnedValue>>) -> bool {
    connection
        .get(SECURITY_SECTION)
        .and_then(|s| s.get("psk-flags"))
        .and_then(|v| u32::try_from(v).ok())
        .is_some_and(|f| f & crate::KEPT_BY_THE_AGENT != 0)
}

/// NetworkManager asking for the WireGuard private key: the one sealed blob,
/// returned under `private-key`.
///
/// `NoSecrets` sends the question on — to the person, or to nobody — exactly
/// as the Wi-Fi path does: a machine whose key is not sealed yet, or whose TPM
/// refuses, meets a location that will not come up, which the view reports as
/// the daemon having no key, not as this agent failing.
fn wireguard_secret(flags: u32) -> Result<HashMap<String, HashMap<String, OwnedValue>>, Failure> {
    // NM telling us the key we have is no good: nothing else to try (there is
    // one key), so send it on rather than hand the same one back in a loop.
    if flags & ASK_AGAIN != 0 {
        return Err(Failure::NoSecrets(
            "NetworkManager asked for a new WireGuard key; the stored one does not do".into(),
        ));
    }

    let key = secret::open_wireguard().map_err(|e| {
        // The reason (the TPM's path and state) goes to the log, not the bus.
        tracing::info!(error = %e, "no WireGuard key stored");
        Failure::NoSecrets("there is no WireGuard key stored".into())
    })?;

    let mut wireguard = HashMap::new();
    wireguard.insert(
        "private-key".to_owned(),
        OwnedValue::try_from(Value::from(key))
            .map_err(|e| Failure::NoSecrets(format!("pack the key: {e}")))?,
    );
    Ok(HashMap::from([(WIREGUARD_SECTION.to_owned(), wireguard)]))
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
        // THE WIREGUARD KEY — the one sealed blob, handed over by
        // `private-key`. A VPN location's profile carries `private-key-flags=1`
        // and no key of its own (`nix/modules/vpn.nix`), so NM asks here at
        // activation, the way it asks for a Wi-Fi password.
        if section == WIREGUARD_SECTION {
            return wireguard_secret(flags);
        }

        if section != SECURITY_SECTION {
            return Err(Failure::NotSupported(format!(
                "this agent knows `{SECURITY_SECTION}` and `{WIREGUARD_SECTION}`, and `{section}` was asked for"
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

        let ssid = ssid_of(&connection)
            .ok_or_else(|| Failure::NotSupported("a connection with no SSID".into()))?;

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

    /// The connection was deleted: the blob goes with it — when the blob was
    /// ITS password ([`left_to_this_agent`]).
    ///
    /// Without this, "forget this network" would leave the sealed password behind — an
    /// orphan secret nobody knows exists any more. And without the condition,
    /// NetworkManager telling of ANY profile of that SSID — it tells every
    /// deletion — would take the blob a remembered profile still asks for:
    /// a join held in memory deleted because the network refused its key, or
    /// typed again.
    fn delete_secrets(
        &self,
        connection: HashMap<String, HashMap<String, OwnedValue>>,
        _path: zbus::zvariant::OwnedObjectPath,
    ) {
        if left_to_this_agent(&connection)
            && let Some(ssid) = ssid_of(&connection)
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

    /// Brings a tunnel up, as ROOT, so NetworkManager asks THIS agent for the
    /// sealed key.
    ///
    /// # Why it has to be root who activates
    ///
    /// NetworkManager serves a secret the person RAISED only from an agent
    /// whose uid matches the person's (`USER_REQUESTED`); a root agent is
    /// ignored for it ("No agents were available"). Our key's agent is root —
    /// it holds the TPM — so the activation itself must come from root for the
    /// key to be handed over. A location that is `autoconnect=false` and raised
    /// from the shell would otherwise never get its key.
    ///
    /// # Why it returns before it is up
    ///
    /// The bringing-up asks US, over this same bus, for the key. Blocking here
    /// would stop this connection answering that and the key would never land.
    /// So the work goes to a thread, and the shell learns the outcome from the
    /// link itself (the snapshot, the modeline), not from this call.
    ///
    /// # The safety the thread carries
    ///
    /// A WireGuard device reaches `activated` the instant its routes are set —
    /// with `0.0.0.0/0;::/0` that is the default route — and with NO handshake.
    /// If the server never answers (the key is wrong, or it is unreachable)
    /// every packet is black-holed and the way out dies. So the thread VERIFIES
    /// a handshake and, failing one, takes the tunnel back DOWN.
    fn raise(&self, connection: String) {
        let Ok(path) = zbus::zvariant::OwnedObjectPath::try_from(connection) else {
            tracing::warn!("raise was given something that is not a connection path");
            return;
        };
        let spawned = std::thread::Builder::new()
            .name("ronoctua-vpn-raise".to_owned())
            .spawn(move || {
                if let Err(e) = raise_and_watch(&path) {
                    tracing::info!(error = %e, "the tunnel was not brought up");
                }
            });
        if let Err(e) = spawned {
            tracing::warn!(error = %e, "no thread to bring the tunnel up");
        }
    }
}

/// `NM_ACTIVE_CONNECTION_STATE_ACTIVATED`.
const ACTIVATED: u32 = 2;

/// Brings `path` up and makes sure it actually CONNECTED, taking it back down
/// if it did not — see [`Ronoctua::raise`]. Runs on a thread, off the agent's
/// own bus connection, so NetworkManager can still ask that connection for the
/// key while this waits.
fn raise_and_watch(path: &zbus::zvariant::OwnedObjectPath) -> anyhow::Result<()> {
    use anyhow::Context as _;
    let conn = zbus::blocking::Connection::system().context("the system bus is away")?;
    let nm =
        crate::nm::ManagerProxyBlocking::new(&conn).context("NetworkManager is off the bus")?;

    // ONE LOCATION AT A TIME: take down any other VPN of ours that is up, so
    // two tunnels never fight over the default route.
    for active in nm.active_connections().unwrap_or_default() {
        if active_is_ours(&conn, &active) {
            let _ = nm.deactivate_connection(&active.as_ref());
        }
    }

    let none = zbus::zvariant::ObjectPath::try_from("/")?;
    let active = nm
        .activate_connection(&path.as_ref(), &none, &none)
        .context("NetworkManager would not bring it up")?;

    // WAIT for the link to stand. A WireGuard device gets here at once.
    let standing = wait_until(Duration::from_secs(5), || {
        state_of(&conn, &active) == Some(ACTIVATED)
    });
    if !standing {
        let _ = nm.deactivate_connection(&active.as_ref());
        anyhow::bail!("the tunnel did not activate");
    }

    // VERIFY it answers. No handshake in a few seconds — the key is wrong or
    // the server is unreachable — and the default route it holds is a black
    // hole, so take it back down to keep the way out.
    let answered = iface_of(&conn, &active)
        .is_some_and(|iface| handshake_within(&iface, Duration::from_secs(8)));
    if !answered {
        tracing::info!("no WireGuard handshake; taking the tunnel back down");
        let _ = nm.deactivate_connection(&active.as_ref());
        anyhow::bail!("the server never answered");
    }
    Ok(())
}

/// Polls `ready` until it holds or the time runs out.
fn wait_until(within: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + within;
    loop {
        if ready() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// The activation's state, or `None` once it no longer answers.
fn state_of(
    conn: &zbus::blocking::Connection,
    active: &zbus::zvariant::OwnedObjectPath,
) -> Option<u32> {
    crate::nm::ActiveProxyBlocking::builder(conn)
        .path(active.clone())
        .ok()
        .and_then(|b| b.build().ok())
        .and_then(|a| a.state().ok())
}

/// Whether an active connection is one of our VPN locations (`ronoctua-vpn-*`).
fn active_is_ours(
    conn: &zbus::blocking::Connection,
    active: &zbus::zvariant::OwnedObjectPath,
) -> bool {
    crate::nm::ActiveProxyBlocking::builder(conn)
        .path(active.clone())
        .ok()
        .and_then(|b| b.build().ok())
        .and_then(|a| a.id().ok())
        .is_some_and(|id| id.starts_with(crate::vpn::LOCATION_PREFIX))
}

/// The `rvpn-*` interface an activation runs over, for reading its handshake.
fn iface_of(
    conn: &zbus::blocking::Connection,
    active: &zbus::zvariant::OwnedObjectPath,
) -> Option<String> {
    let ac = crate::nm::ActiveProxyBlocking::builder(conn)
        .path(active.clone())
        .ok()?
        .build()
        .ok()?;
    let device = ac.devices().ok()?.into_iter().next()?;
    let d = crate::nm::DeviceProxyBlocking::builder(conn)
        .path(device)
        .ok()?
        .build()
        .ok()?;
    d.ip_interface()
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| d.interface().ok())
        .filter(|s| !s.is_empty())
}

/// Whether the tunnel on `iface` has had a handshake within the time — the one
/// true sign a WireGuard link is carrying, and not just configured.
fn handshake_within(iface: &str, within: Duration) -> bool {
    // `wg` by its baked path (`RONOCTUA_WG`, set by `nix/modules/network.nix`);
    // a bare name is the fallback for a shell that has it on PATH.
    let wg = std::env::var("RONOCTUA_WG").unwrap_or_else(|_| "wg".to_owned());
    let deadline = Instant::now() + within;
    loop {
        if latest_handshake(&wg, iface) > 0 {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
}

/// The newest handshake time on `iface`, in seconds since the epoch, or `0`
/// for "never" — `wg show <iface> latest-handshakes` reads it (needs root,
/// which this daemon is).
fn latest_handshake(wg: &str, iface: &str) -> u64 {
    let Ok(out) = std::process::Command::new(wg)
        .args(["show", iface, "latest-handshakes"])
        .output()
    else {
        return 0;
    };
    // Each line is `<public-key>\t<unix-seconds>`; any nonzero time is a
    // handshake that landed.
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split_whitespace().next_back())
        .filter_map(|ts| ts.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
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

    /// FAILS IF: deleting a profile whose password NetworkManager kept
    /// itself — a join held in memory, refused or typed again — erases the
    /// blob a remembered profile of the same network still asks for; or
    /// deleting the remembered one leaves its blob behind.
    #[test]
    fn only_a_profile_left_to_the_agent_takes_its_blob() {
        let with_flags = |flags: Option<u32>| {
            let mut c = connection_with_ssid(b"home");
            if let Some(f) = flags {
                c.insert(
                    SECURITY_SECTION.to_owned(),
                    HashMap::from([("psk-flags".to_owned(), OwnedValue::from(f))]),
                );
            }
            c
        };
        assert!(left_to_this_agent(&with_flags(Some(
            crate::KEPT_BY_THE_AGENT
        ))));
        assert!(!left_to_this_agent(&with_flags(Some(
            crate::KEPT_BY_THE_DAEMON
        ))));
        assert!(!left_to_this_agent(&with_flags(None)), "an open network");
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
