//! Asks for the Wi-Fi password ON THE CONSOLE when nobody is logged in.
//!
//! # Why this is not a mode of the agent
//!
//! Because NetworkManager's specification forbids it. When NM asks an agent for
//! a secret it passes `ALLOW_INTERACTION`, and an agent may only interact when
//! that flag is set. At boot-time autoconnect — which is exactly the case this
//! exists for — the flag is **not** set: NM is connecting on its own, with
//! nobody having asked for anything. An agent that prompted anyway would be
//! breaking the contract that makes it an agent, and the price would be paid by
//! whoever is watching a login screen wondering why it is asking about Wi-Fi.
//!
//! So this is not an agent. It is a service that behaves like a **person**: it
//! notices the network did not come up, asks on the console, and hands the
//! password to NM through the same call the footer uses.
//!
//! # When it stays quiet, and why that is most of the time
//!
//! - **The Wi-Fi is already up.** The sealed blob answered, which is the normal
//!   path, and there is nothing to ask.
//! - **Somebody is logged in graphically.** Then the question has a better place
//!   to be asked — `LEADER r r`, on a screen that can draw — and a prompt on a
//!   TTY nobody is looking at would be a password typed into the void.
//! - **There is no known network in range.** Asking for the password of a
//!   network that is not there teaches nothing and cannot succeed.
//!
//! # What it costs when it is wrong
//!
//! It prompts on a console for a network that would have connected a second
//! later. `systemd-ask-password` times out, the service exits, and nothing is
//! broken. That is the reason the timeout is short and the wait before asking is
//! not: it is cheaper to wait than to ask twice.

use std::process::Command;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use ronoctua_network::Manager;

/// How long to give NetworkManager before deciding the network is not coming.
///
/// The radio takes seconds to associate, DHCP takes more, and the sealed blob
/// path goes through the TPM. Asking before all of that has had its chance is
/// how one ends up with a prompt on screen and a connection behind it.
const SETTLE: Duration = Duration::from_secs(45);

/// How long the question stays on the console.
///
/// Short on purpose. Whoever is not there is not going to arrive, and a prompt
/// that never expires holds a TTY forever — including the one somebody would
/// use to fix the problem another way.
const ASK_TIMEOUT: Duration = Duration::from_secs(120);

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ronoctua_network=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();

    if graphical_session()? {
        tracing::info!("there is a graphical session; the question has a better place");
        return Ok(());
    }

    let manager = Manager::fresh().context("no NetworkManager on the bus")?;

    // WAIT FIRST, ASK LATER — and poll instead of sleeping the whole time, so a
    // machine that connects in three seconds does not hold a boot for
    // forty-five.
    let deadline = Instant::now() + SETTLE;
    while Instant::now() < deadline {
        if connected(&manager) {
            tracing::info!("the network came up on its own");
            return Ok(());
        }
        std::thread::sleep(Duration::from_secs(3));
    }

    let Some(ssid) = target(&manager) else {
        tracing::info!("no known network in range; nothing to ask");
        return Ok(());
    };

    let Some(password) = ask(&ssid)? else {
        tracing::info!(%ssid, "nobody answered");
        return Ok(());
    };

    // `remember = true`: whoever typed a password on a console at boot does not
    // want to type it again at the next one. It goes down the same path the
    // footer uses, so it is sealed in the TPM exactly like any other.
    manager
        .connect(&ssid, Some(&password), true)
        .with_context(|| format!("connect to `{ssid}`"))?;
    tracing::info!(%ssid, "conectado pelo console");
    Ok(())
}

/// Whether any login session is graphical.
///
/// Read from logind's own directory rather than asked over D-Bus, and the reason
/// is the moment: this runs while the system is still coming up, and a D-Bus
/// round trip to a logind that is not ready yet would answer "no sessions" —
/// which is the answer that makes this prompt. Being wrong in that direction is
/// the expensive one.
fn graphical_session() -> Result<bool> {
    let dir = std::path::Path::new("/run/systemd/sessions");
    if !dir.is_dir() {
        return Ok(false);
    }
    for entry in std::fs::read_dir(dir).context("ler as sessões do logind")? {
        let path = entry.context("session entry")?.path();
        // The directory also holds `*.ref` files, which are not session state.
        if path.extension().is_some() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            if let Some(kind) = line.strip_prefix("TYPE=")
                && matches!(kind.trim(), "wayland" | "x11" | "mir")
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Whether there is a live link of any medium.
///
/// Any medium, and not just Wi-Fi: a machine on a cable has the network this
/// exists to restore, and asking for a Wi-Fi password there would be asking for
/// something nobody needs.
fn connected(manager: &Manager) -> bool {
    manager
        .links()
        .map(|links| !links.is_empty())
        .unwrap_or(false)
}

/// Which network to ask about.
///
/// The strongest in range that this machine ALREADY has a profile for. A profile
/// means someone connected to it before, so the password is a thing that exists
/// and the question is answerable. Offering a network nobody ever used would be
/// asking a person at a console to invent a secret.
fn target(manager: &Manager) -> Option<String> {
    let mut known: Vec<_> = manager
        .networks()
        .ok()?
        .into_iter()
        .filter(|n| manager.has_stored_password(&n.ssid))
        .collect();
    known.sort_by_key(|n| std::cmp::Reverse(n.signal));
    known.into_iter().next().map(|n| n.ssid)
}

/// Asks on every active console, through systemd.
///
/// `systemd-ask-password` and not a `read` on stdin: this is a service, and its
/// stdin is the journal. The tool broadcasts the question to every console and
/// to plymouth, which is the same mechanism that asks for the disk's passphrase
/// at boot — so the person has already seen this exact prompt, in this exact
/// place, minutes earlier.
fn ask(ssid: &str) -> Result<Option<String>> {
    // BY PATH, NEVER BY NAME. A systemd unit's `PATH` is minimal, and a bare
    // name fails far from the cause: the service exits non-zero, the journal
    // says "No such file or directory", and nothing connects it to the Wi-Fi
    // question that never appeared. The store path comes from `nix/rust.nix`.
    let tool = option_env!("RONOCTUA_ASK_PASSWORD").unwrap_or("systemd-ask-password");
    let out = Command::new(tool)
        .arg(format!("--timeout={}", ASK_TIMEOUT.as_secs()))
        // No echo, and no keyring: the password's home is the TPM blob that
        // `connect` seals, and a copy in the kernel keyring would be a second
        // place holding it with a different lifetime.
        .arg("--echo=no")
        .arg("--no-tty")
        .arg(format!("Wi-Fi password for `{ssid}`:"))
        .output()
        .context("systemd-ask-password did not run")?;

    if !out.status.success() {
        // A timeout is not a failure: it is nobody there, which is the expected
        // outcome of a machine that booted alone.
        tracing::info!(status = ?out.status, "the question on the console was not answered");
        return Ok(None);
    }
    let password = String::from_utf8_lossy(&out.stdout).trim_end().to_owned();
    Ok(if password.is_empty() {
        None
    } else {
        Some(password)
    })
}
