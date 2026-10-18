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

pub mod options;
pub mod prefs;
pub mod tailscale;
pub mod tor;

use std::path::PathBuf;

/// How often the thread asks, at rest.
pub const BEAT: std::time::Duration = std::time::Duration::from_secs(10);

/// Where everything this crate touches is.
///
/// `systemctl` and the fragment directory joined the three sockets when Tor
/// grew a way to be STARTED: both used to be parameters threaded through
/// `watch`, and both are answers to "where is it", which is what this struct
/// is. A test can still point them somewhere harmless, which is the only
/// reason they are not bare constants.
#[derive(Debug, Clone)]
pub struct Places {
    /// Tailscale's local API.
    pub tailscale: PathBuf,
    /// Tor's control socket.
    pub tor: PathBuf,
    /// Tor's cookie.
    pub cookie: PathBuf,
    /// `systemctl`, by store path.
    pub systemctl: PathBuf,
    /// The directory whose torrc fragment this system owns.
    pub fragment: PathBuf,
}

impl Default for Places {
    fn default() -> Self {
        Self {
            tailscale: PathBuf::from(tailscale::SOCKET),
            tor: PathBuf::from(tor::SOCKET),
            cookie: PathBuf::from(tor::COOKIE),
            systemctl: PathBuf::from(tor::SYSTEMCTL),
            fragment: PathBuf::from(tor::FRAGMENT_DIR),
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
    /// What systemd says about `tor.service`.
    ///
    /// A SECOND FACT, not the same one. The control socket answers only when
    /// Tor is running, so `tor: None` used to be the only thing the screen
    /// knew — and it conflated "stopped", "never installed" and "you are not
    /// in the `tor` group" into one silence with no row under it. The comment
    /// in `network.rs` already said the unit and the circuit are different
    /// facts; this is the other one, finally read.
    pub standing: tor::Standing,
    /// Whether Tor is to come up with the session.
    ///
    /// OURS, not systemd's — see [`prefs`] for why `systemctl enable` is not
    /// an option here. It rides on the `Scene` rather than on the view's
    /// `Facts` so that no door, no click and no test grows a field for it: the
    /// thread already reads three things per beat and this is a fourth small
    /// file.
    pub autostart: bool,
    /// The torrc fragment this system owns, as it is on disk.
    ///
    /// Read rather than remembered: the file is what survives a reboot and
    /// what a second door also writes, so a copy kept in memory would be a
    /// second truth that drifts. It is a few hundred bytes.
    pub settings: tor::fragment::Lines,
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

/// Reads everything, once.
#[must_use]
pub fn look(places: &Places) -> Scene {
    Scene {
        tailscale: tailscale::look(&places.tailscale),
        tor: tor::look(&places.tor, &places.cookie),
        standing: tor::standing(&places.systemctl),
        autostart: prefs::read().autostart,
        settings: tor::fragment::read(&places.fragment),
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
    /// Write the torrc fragment, and make a running Tor re-read it.
    ///
    /// ONE ORDER FOR BOTH HALVES, and deliberately: a write without a reload
    /// is a setting that takes effect at the next restart and looks broken
    /// until then, and a reload without a write is a setting that vanishes at
    /// the next one. They are not two things a door should be able to get
    /// wrong separately.
    ///
    /// With Tor stopped the write still happens and the reload is skipped —
    /// there is nothing running to tell. That is not an error: it is how a
    /// person configures bridges BEFORE turning Tor on for the first time,
    /// which on a censored network is the only order that works.
    Configure(Box<tor::fragment::Lines>),
    /// Remember, or stop remembering, that Tor comes up with the session.
    Autostart(bool),
}

/// Why an order could not be carried out — by NAME.
///
/// The daemons' own words are English (Tor's answers, `systemctl`'s stderr,
/// the local API's bodies) and the message line is in the person's language
/// (invariant 8), so the door picks its sentence by this and the words go to
/// the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The daemon's socket is not there: it is not running.
    NoDaemon,
    /// The socket or the cookie is not this person's to use.
    NotAuthorized,
    /// `systemctl` could not start or stop the unit.
    UnitFailed,
    /// A file of ours could not be written.
    NotWritten,
    /// Tor would not take its configuration — a line it could not read.
    ConfigRefused,
    /// The daemon answered no.
    DaemonRefused,
    /// Anything else.
    Other,
}

/// A refusal, its name and what came with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    /// Which refusal it is.
    pub why: Refusal,
    /// THE PERSON'S OWN TEXT that Tor named as the line it could not read —
    /// data they typed, not the daemon's English, and the one thing that
    /// says which bridge was pasted wrong.
    pub line: Option<String>,
    /// The words that came with it, for the log.
    pub words: String,
}

/// A refusal with a name, carried inside an `anyhow::Error` from where it
/// happened to [`refused_of`].
#[derive(Debug)]
pub(crate) struct Said {
    /// Which refusal.
    pub why: Refusal,
    /// The daemon's answer.
    pub words: String,
}

impl std::fmt::Display for Said {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.words)
    }
}

impl std::error::Error for Said {}

/// The first quoted stretch of an answer — `'Brige'` in `553 Unrecognized
/// option 'Brige'` — which is the person's own text, given back.
#[must_use]
pub fn quoted(answer: &str) -> Option<String> {
    for q in ['\'', '"'] {
        let mut parts = answer.splitn(3, q);
        let _ = parts.next();
        if let (Some(inside), Some(_)) = (parts.next(), parts.next())
            && !inside.trim().is_empty()
        {
            return Some(inside.trim().to_owned());
        }
    }
    None
}

/// The refusal an error is, read off its chain.
#[must_use]
pub fn refused_of(e: &anyhow::Error, otherwise: Refusal) -> Refused {
    let words = format!("{e:#}");
    for cause in e.chain() {
        if let Some(s) = cause.downcast_ref::<Said>() {
            let line = (s.why == Refusal::ConfigRefused)
                .then(|| quoted(&s.words))
                .flatten();
            return Refused {
                why: s.why,
                line,
                words,
            };
        }
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            let why = match io.kind() {
                std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused => {
                    Refusal::NoDaemon
                }
                std::io::ErrorKind::PermissionDenied => Refusal::NotAuthorized,
                _ => continue,
            };
            return Refused {
                why,
                line: None,
                words,
            };
        }
    }
    Refused {
        why: otherwise,
        line: None,
        words,
    }
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
        refused: Option<Refused>,
    },
    /// A login wants this address opened.
    Login(String),
}

/// The handle a door keeps: it sends orders, and the thread answers on the
/// channel the door gave to [`watch`].
///
/// `Clone`, because a door hands it to the view it hosts: the footer's
/// network view orders the footer's own thread — the one the modeline's two
/// marks are read from — and a second thread of its own would poll the two
/// daemons twice and leave the marks a reading behind every order.
#[derive(Clone)]
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
pub fn watch<F>(places: Places, on_event: F) -> anyhow::Result<Hand>
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
                        let refused = carry_out(&places, &order, &on_event);
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
fn carry_out<F>(places: &Places, order: &Order, on_event: &F) -> Option<Refused>
where
    F: Fn(Event) + Send + 'static,
{
    let said = |r: anyhow::Result<()>| r.err().map(|e| refused_of(&e, Refusal::DaemonRefused));
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
            Err(e) => Some(refused_of(&e, Refusal::DaemonRefused)),
        },
        Order::NewIdentity => {
            said(tor::Control::open(&places.tor, &places.cookie).and_then(|mut c| c.new_identity()))
        }
        // THE UNIT, THROUGH SYSTEMD, and by STORE PATH: a bare name fails at
        // EXEC inside a user unit, where `PATH` is minimal — the trap
        // AGENTS.md records, and it shows up far from its cause.
        Order::Tor(on) => {
            let verb = if *on { "start" } else { "stop" };
            match std::process::Command::new(&places.systemctl)
                .args([verb, tor::UNIT])
                .output()
            {
                Ok(out) if out.status.success() => None,
                Ok(out) => Some(Refused {
                    why: Refusal::UnitFailed,
                    line: None,
                    words: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
                }),
                Err(e) => Some(Refused {
                    why: Refusal::UnitFailed,
                    line: None,
                    words: e.to_string(),
                }),
            }
        }
        Order::Configure(lines) => said(configure(places, lines)),
        Order::Autostart(on) => prefs::write(&prefs::Preferences { autostart: *on })
            .err()
            .map(|e| Refused {
                why: Refusal::NotWritten,
                line: None,
                words: e.to_string(),
            }),
    }
}

/// Writes the fragment, and tells a running Tor to re-read it.
///
/// The reload is skipped when nothing is running, and that is not a failure —
/// see [`Order::Configure`]. It is also skipped when the control socket will
/// not open, which is the same situation from one step further away.
fn configure(places: &Places, lines: &tor::fragment::Lines) -> anyhow::Result<()> {
    tor::fragment::write(&places.fragment, lines).map_err(|e| {
        anyhow::Error::from(Said {
            why: Refusal::NotWritten,
            words: format!("{e:#}"),
        })
    })?;
    if !matches!(tor::standing(&places.systemctl), tor::Standing::Running) {
        return Ok(());
    }
    // THE REFUSAL IS THE DAEMON'S OWN WORDS. A torrc Tor cannot parse comes
    // back here as a sentence naming the line, and that sentence is the only
    // thing on screen that could tell a person which bridge they pasted wrong.
    tor::Control::open(&places.tor, &places.cookie)?.reload()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Places that point at nothing, for the tests that must not touch a real
    /// daemon, a real `systemctl` or a real `/var/lib`.
    fn nowhere() -> Places {
        Places {
            tailscale: PathBuf::from("/nonexistent/tailscaled.sock"),
            tor: PathBuf::from("/nonexistent/tor"),
            cookie: PathBuf::from("/nonexistent/cookie"),
            systemctl: PathBuf::from("/nonexistent/systemctl"),
            fragment: PathBuf::from("/nonexistent/fragment"),
        }
    }

    #[test]
    fn a_machine_with_neither_daemon_draws_nothing() {
        // THE ORDINARY CASE, and the one the modeline's rule is about: two
        // sockets that are not there, and no mark of either.
        let scene = look(&nowhere());
        assert_eq!(scene, Scene::default());
        assert!(!scene.on_tailnet());
        assert!(!scene.through_tor());
        // AND `systemctl` THAT CANNOT BE RUN IS `Unknown`, NOT `Stopped`.
        // A build sandbox and a laptop whose Tor is off must not be one word:
        // the sandbox has nothing to say, and the laptop has a switch.
        assert_eq!(scene.standing, tor::Standing::Unknown);
        assert!(!scene.standing.actionable());
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
            ..Scene::default()
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
            ..Scene::default()
        };
        assert!(up.on_tailnet());
        assert!(up.through_tor());
    }

    #[test]
    fn a_stopped_tor_is_the_state_that_can_be_started() {
        // THE DEFECT THIS CLOSES. The switch's boolean was the literal `false`
        // at its one construction site, so `Order::Tor(true)` existed in this
        // crate and was reachable from nowhere in the shell — the owner
        // reported that he could not find a way to turn Tor on, and he was
        // right. The unit's standing is what the row asks now.
        assert!(tor::Standing::Stopped.starts());
        assert!(!tor::Standing::Running.starts());
        // AND THE TWO THAT ARE NOT A SWITCH AT ALL. `Absent` has no unit to
        // act on and `Unknown` has nothing to say; drawing a switch for either
        // would be a key that does nothing and says nothing.
        assert!(!tor::Standing::Absent.actionable());
        assert!(!tor::Standing::Unknown.actionable());
        assert!(tor::Standing::Stopped.actionable());
        assert!(tor::Standing::Moving.actionable());
    }

    #[test]
    fn configuring_writes_the_fragment_even_with_tor_stopped() {
        // THIS IS THE ORDER THAT HAS TO WORK ON A CENSORED NETWORK: bridges
        // are configured BEFORE Tor is turned on for the first time, because
        // without them it will not bootstrap at all. A `Configure` that needed
        // a running daemon would make that impossible and would look like a
        // key that does nothing.
        let dir = std::env::temp_dir().join(format!("ronoctua-tor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let places = Places {
            fragment: dir.clone(),
            ..nowhere()
        };
        let mut lines = tor::fragment::Lines::new();
        lines.insert("UseBridges".to_owned(), vec!["1".to_owned()]);
        assert_eq!(configure(&places, &lines).ok(), Some(()));
        // AND IT READS BACK AS WHAT WENT IN.
        assert_eq!(tor::fragment::read(&dir), lines);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// FAILS IF: a refusal that has a name is read as another, or the line
    /// Tor quoted — the person's own text — is lost.
    #[test]
    fn a_refusal_is_read_by_its_name() {
        let tor = anyhow::Error::from(Said {
            why: Refusal::ConfigRefused,
            words: "553 Unrecognized option 'Brige obfs4 1.2.3.4'".to_owned(),
        })
        .context("configuring");
        let r = refused_of(&tor, Refusal::Other);
        assert_eq!(r.why, Refusal::ConfigRefused);
        assert_eq!(r.line.as_deref(), Some("Brige obfs4 1.2.3.4"));
        let gone = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::NotFound))
            .context("/run/tailscale.sock");
        assert_eq!(refused_of(&gone, Refusal::Other).why, Refusal::NoDaemon);
        let denied =
            anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert_eq!(
            refused_of(&denied, Refusal::Other).why,
            Refusal::NotAuthorized
        );
        assert_eq!(
            refused_of(&anyhow::anyhow!("403"), Refusal::DaemonRefused).why,
            Refusal::DaemonRefused
        );
        assert_eq!(quoted("no quotes here"), None);
        assert_eq!(quoted("say \"this\" back").as_deref(), Some("this"));
    }
}
