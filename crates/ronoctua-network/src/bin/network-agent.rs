//! The daemon that answers NetworkManager when it asks for the Wi-Fi password.
//!
//! # Why a system binary, and not a mode of the footer
//!
//! The network has to come up **before** anyone logs in. An agent tied to the graphics
//! session would mean no Wi-Fi until login — and on a machine whose
//! first job is to download the system over the network, that is the opposite of what one
//! wants.
//!
//! # What happens if it is not running
//!
//! NetworkManager finds no agent at all and asks the password of whoever is
//! looking — the same thing it does when the agent answers `NoSecrets`. The network
//! is not tied to this process being alive; it is tied to **someone** being able to
//! answer. It is deliberate that the failure mode is that one.

use anyhow::{Context, Result};
use ronoctua_network::agent::{AGENT_PATH, Agent, IDENTIFIER, RONOCTUA_PATH, Ronoctua};
use ronoctua_network::nm;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_network=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    // The SYSTEM bus: NetworkManager belongs to the host, not to the session. On
    // the session bus the registration would return "name has no owner", which is the
    // same error as an absent service and would lead to the wrong diagnosis.
    let connection = zbus::blocking::connection::Builder::system()
        .context("sem barramento de sistema")?
        // THE WELL-KNOWN NAME IS MANDATORY, and not decoration.
        //
        // NetworkManager would find the agent by the connection's unique name, but the
        // footer calls `org.ronoctua.Network1` by fixed address — without claiming
        // the name, `Guardar` would fail with "name has no owner", which is the same
        // error as an absent daemon and would lead to the wrong diagnosis.
        .name(IDENTIFIER)
        .context("reivindicar o nome no barramento")?
        .serve_at(AGENT_PATH, Agent)?
        .serve_at(RONOCTUA_PATH, Ronoctua)?
        .build()
        .context("expor o agente no barramento")?;

    let manager = nm::AgentManagerProxyBlocking::new(&connection)
        .context("NetworkManager fora do barramento")?;
    manager
        .register(IDENTIFIER)
        .context("registrar como agente de segredo")?;
    tracing::info!(identificador = IDENTIFIER, "agente de rede registrado");

    // The connection has to LIVE: dropping it undoes the registration and NetworkManager
    // starts asking the password of nobody, with nothing here noticing. Same
    // design as the notification daemon.
    loop {
        std::thread::park();
        let _ = &connection;
    }
}
