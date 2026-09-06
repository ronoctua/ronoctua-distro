//! The two tunnels the network view knows: Tailscale and Tor.
//!
//! # Why one crate for two daemons
//!
//! Because they are one idea on screen — "am I going out through something
//! else, and is it working?" — and because neither is a bus. Tailscale
//! answers HTTP on a Unix socket and Tor answers a line protocol on another;
//! putting them behind the same [`Scene`] is what lets the view draw two
//! sections and the modeline two marks without either knowing a protocol.
//!
//! # Neither is on at rest
//!
//! Tor does not start at boot (`ronoctua.tor.autostart`, false by default) and
//! Tailscale needs a login before it is anything. A machine that has never
//! used either draws NOTHING — the owner's rule for the whole modeline — and
//! the reading that says so is a socket that is not there, which costs one
//! `connect` that fails.
//!
//! # The clock is a thread, and it is slow on purpose
//!
//! Neither daemon signals: there is no bus to subscribe to. So [`watch`]
//! polls, every [`BEAT`], on a thread of its own, and sends the whole scene
//! down a channel — the shape every other frontier here has, minus the
//! subscription. Ten seconds is what a tunnel's state is worth: a person who
//! has just pressed a key gets an immediate reading, and the rest is a
//! background fact.

#![forbid(unsafe_code)]

pub mod http;
pub mod tailscale;
pub mod tor;

use std::path::PathBuf;

/// How often the thread asks, at rest.
pub const BEAT: std::time::Duration = std::time::Duration::from_secs(10);

/// Where the two daemons are.
#[derive(Debug, Clone)]
pub struct Places {
    /// Tailscale's local API.
    pub tailscale: PathBuf,
    /// Tor's control socket.
    pub tor: PathBuf,
    /// Tor's cookie.
    pub cookie: PathBuf,
}

impl Default for Places {
    fn default() -> Self {
        Self {
            tailscale: PathBuf::from(tailscale::SOCKET),
            tor: PathBuf::from(tor::SOCKET),
            cookie: PathBuf::from(tor::COOKIE),
        }
    }
}

/// What both daemons say, as of one reading.
///
/// `None` means "not there", which is the ordinary case for each of them and
/// is drawn as nothing at all — never as an error.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scene {
    /// Tailscale, when its socket answered.
    pub tailscale: Option<tailscale::Scene>,
    /// Tor, when its control socket answered.
    pub tor: Option<tor::Scene>,
}

impl Scene {
    /// Is Tailscale up and this machine on it?
    #[must_use]
    pub fn on_tailnet(&self) -> bool {
        self.tailscale
            .as_ref()
            .is_some_and(tailscale::Scene::marked)
    }

    /// Is there a Tor circuit?
    #[must_use]
    pub fn through_tor(&self) -> bool {
        self.tor.as_ref().is_some_and(tor::Scene::marked)
    }
}

/// Reads both, once.
#[must_use]
pub fn look(places: &Places) -> Scene {
    Scene {
        tailscale: tailscale::look(&places.tailscale),
        tor: tor::look(&places.tor, &places.cookie),
    }
}

/// What a door can ask for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Bring the tailnet up, or take it down.
    Tailscale(bool),
    /// Start a login, and open the address it gives back.
    Login,
    /// Leave the tailnet.
    Logout,
    /// Send everything through this node, or through none.
    ExitNode(Option<String>),
    /// Accept the tailnet's routes, or stop.
    AcceptRoutes(bool),
    /// Refuse incoming connections, or stop.
    ShieldsUp(bool),
    /// Start Tor's unit, or stop it.
    Tor(bool),
    /// Ask Tor for fresh circuits.
    NewIdentity,
}

/// What a door hears back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// A whole fresh reading.
    Scene(Box<Scene>),
    /// An order finished. `Err` carries what to put on the message line.
    Done {
        /// What was asked for.
        order: Order,
        /// What went wrong, when something did.
        refused: Option<String>,
    },
    /// A login wants this address opened.
    Login(String),
}

/// The handle a door keeps: it sends orders, and the thread answers on the
/// channel the door gave to [`watch`].
pub struct Hand {
    orders: std::sync::mpsc::Sender<Order>,
}

impl Hand {
    /// Asks for something. A closed channel means the thread is gone, which
    /// happens only while the process is shutting down.
    pub fn send(&self, order: Order) {
        if self.orders.send(order).is_err() {
            tracing::debug!("the tunnel thread is gone; the order was dropped");
        }
    }
}

/// Raises the thread that reads both daemons and carries out orders.
///
/// ONE THREAD, not two: the readings are sequential and cheap (a connect that
/// fails is microseconds), and an order has to be followed by a reading the
/// screen can trust. Two threads would need a lock between them for nothing.
///
/// # Errors
///
/// The thread could not be started.
pub fn watch<F>(places: Places, systemctl: PathBuf, on_event: F) -> anyhow::Result<Hand>
where
    F: Fn(Event) + Send + 'static,
{
    let (orders, inbox) = std::sync::mpsc::channel::<Order>();
    std::thread::Builder::new()
        // FIFTEEN BYTES is Linux's whole room for a thread's name, and a
        // longer one is silently cut — the lesson the power threads carry.
        .name("tunnels".to_owned())
        .spawn(move || {
            let mut last = Scene::default();
            on_event(Event::Scene(Box::new(look(&places))));
            loop {
                // THE ORDER FIRST, and the reading right after it: a person
                // who pressed a key is owed an answer before the next beat.
                match inbox.recv_timeout(BEAT) {
                    Ok(order) => {
                        let refused = carry_out(&places, &systemctl, &order, &on_event);
                        on_event(Event::Done { order, refused });
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
                let fresh = look(&places);
                if fresh != last {
                    last = fresh.clone();
                    on_event(Event::Scene(Box::new(fresh)));
                }
            }
            tracing::debug!("the tunnel thread ended with its last sender");
        })
        .map_err(|e| anyhow::anyhow!("could not start the tunnel thread: {e}"))?;
    Ok(Hand { orders })
}

/// Does one order, and says what went wrong when something did.
fn carry_out<F>(
    places: &Places,
    systemctl: &std::path::Path,
    order: &Order,
    on_event: &F,
) -> Option<String>
where
    F: Fn(Event) + Send + 'static,
{
    let said = |r: anyhow::Result<()>| r.err().map(|e| e.to_string());
    match order {
        Order::Tailscale(on) => said(tailscale::set_running(&places.tailscale, *on)),
        Order::AcceptRoutes(on) => said(tailscale::set_accept_routes(&places.tailscale, *on)),
        Order::ShieldsUp(on) => said(tailscale::set_shields_up(&places.tailscale, *on)),
        Order::ExitNode(id) => said(tailscale::set_exit_node(&places.tailscale, id.as_deref())),
        Order::Logout => said(tailscale::logout(&places.tailscale)),
        Order::Login => match tailscale::login(&places.tailscale) {
            Ok(Some(url)) => {
                on_event(Event::Login(url));
                None
            }
            Ok(None) => None,
            Err(e) => Some(e.to_string()),
        },
        Order::NewIdentity => {
            said(tor::Control::open(&places.tor, &places.cookie).and_then(|mut c| c.new_identity()))
        }
        // THE UNIT, THROUGH SYSTEMD, and by STORE PATH: a bare name fails at
        // EXEC inside a user unit, where `PATH` is minimal — the trap
        // CLAUDE.md records, and it shows up far from its cause.
        Order::Tor(on) => {
            let verb = if *on { "start" } else { "stop" };
            match std::process::Command::new(systemctl)
                .args([verb, tor::UNIT])
                .output()
            {
                Ok(out) if out.status.success() => None,
                Ok(out) => Some(String::from_utf8_lossy(&out.stderr).trim().to_owned()),
                Err(e) => Some(e.to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_with_neither_daemon_draws_nothing() {
        // THE ORDINARY CASE, and the one the modeline's rule is about: two
        // sockets that are not there, and no mark of either.
        let places = Places {
            tailscale: PathBuf::from("/nonexistent/tailscaled.sock"),
            tor: PathBuf::from("/nonexistent/tor"),
            cookie: PathBuf::from("/nonexistent/cookie"),
        };
        let scene = look(&places);
        assert_eq!(scene, Scene::default());
        assert!(!scene.on_tailnet());
        assert!(!scene.through_tor());
    }

    #[test]
    fn the_marks_are_the_running_ones_and_not_the_installed_ones() {
        let scene = Scene {
            tailscale: Some(tailscale::Scene {
                state: "NeedsLogin".to_owned(),
                ..tailscale::Scene::default()
            }),
            tor: Some(tor::Scene {
                bootstrap: 45,
                ..tor::Scene::default()
            }),
        };
        // INSTALLED IS NOT RUNNING: a daemon that has not finished starting
        // draws no mark, and the view is where the 45% is read.
        assert!(!scene.on_tailnet());
        assert!(!scene.through_tor());
        let up = Scene {
            tailscale: Some(tailscale::Scene {
                running: true,
                ..tailscale::Scene::default()
            }),
            tor: Some(tor::Scene {
                established: true,
                ..tor::Scene::default()
            }),
        };
        assert!(up.on_tailnet());
        assert!(up.through_tor());
    }
}
