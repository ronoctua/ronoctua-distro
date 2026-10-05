//! `LEADER s e` — the security centre, in one view with two doors: the
//! footer's region (`s e e`) and a window (`s e w`,
//! `crate::security_window`).
//!
//! # What is here, in order
//!
//! 1. **Now** — only what is off, in the danger colour, or one quiet line
//!    saying nothing is.
//! 2. **Firewall** — on or off, each network's profile (home, public, or
//!    "this machine serves it"), the shields, the person's rules, the
//!    connections refused lately with "allow" on each, the log's level, and
//!    the ports this machine answers on (listening sockets and the published
//!    ports of containers) with what the firewall does to each.
//! 3. **Administrator** — whether sudo asks for the password, the window
//!    without it (5, 15, 30 or 60 minutes) and its end, and a week of sudo.
//! 4. **Fingerprint** — only with a reader: the fingers enrolled, enrolling
//!    one (touch by touch), removing one, and fingers for sudo in terminals.
//! 5. **Device** — Secure Boot, the TPM, disk encryption, the firmware's
//!    security level and updates (read only), automatic updates, and two
//!    honest lines: there is no screen lock, and with autologin the boot
//!    passphrase is the only gate.
//! 6. **Network privacy** — per connection that is up: how the hardware
//!    address shows, whether DNS is encrypted; and the SSH server.
//! 7. **Online checks** — the VirusTotal key, a file, an e-mail address, a
//!    password; each row says what leaves the machine and where to.
//! 8. **Suggestions**, at the end — a VPN, Tor, the tailnet, Have I Been
//!    Pwned's own page.
//!
//! # Tightening is free, loosening asks
//!
//! Raising the shields is the person's with no password (one narrow polkit
//! rule, `ronoctua_security::shields`); every other change of the firewall,
//! the passwordless window, and the fingers go through the sudo door
//! (`ronoctua_security::authority`): `sudo -n` first, and the password
//! question in the door that asked only when that is refused. A change of the
//! firewall is a WHOLE `Settings` staged right before the door runs, never a
//! patch — the helper validates the whole of it and writes it whole.
//!
//! # Both doors, one view
//!
//! What a key or a click does is decided here ([`View::press`]) and handed
//! back as an [`Errand`]; each door carries it out (a thread, a question on
//! its own line, the queue) and hands what came back to [`View::came`]. The
//! footer and the window therefore ask the same questions, type the same
//! answers and say the same sentences.
//!
//! # Where the data comes from
//!
//! [`Scene::read`], on a thread of each door: the files the root helper
//! writes (`/run/ronoctua-security`, `/var/lib/ronoctua-security`), fprintd
//! and fwupd (`ronoctua-security`), udisks (`ronoctua-disks`),
//! NetworkManager (`ronoctua-network`), the listening sockets
//! (`ronoctua-memory`), podman (`ronoctua-containers`), the kept key and the
//! checks (`ronoctua-lookup`) — and the brain's `security-watch` module's
//! page, which keeps the connections refused lately and a week of sudo. A
//! door reads it again when those folders change, never on a clock.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ronoctua_i18n::{Arg, key};
use ronoctua_lookup::keys::Secret;
use ronoctua_security::Why;
use ronoctua_security::authority::Proof;
use ronoctua_security::firewall::{Action, LogLevel, Profile, Protocol, Rule, Settings, Status};
use ronoctua_security::fprint::{Finger, Reader};
use ronoctua_security::platform::{SecureBoot, Tpm};
use ronoctua_security::verb::{Fingers, RelaxMinutes, RootVerb};
use ronoctua_tokens as tokens;
use serde::{Deserialize, Serialize};

use crate::files::{Field, Hit as FilesHit, Ink};

/// The brain's module this page reads the refused connections and sudo's
/// uses from.
pub const WATCH: &str = "security-watch";

/// The keys that act only with a fingerprint reader, on the enrol row.
const FINGER_KEYS: [&str; 3] = ["security-left", "security-right", "security-enrol"];

/// The keys this view answers to. The hint line is GENERATED from it.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "security-down"),
    ("<up>", "security-up"),
    ("RET", "security-enter"),
    ("<left>", "security-left"),
    ("<right>", "security-right"),
    ("C-r", "security-relax"),
    ("C-b", "security-shields"),
    ("C-a", "security-rule-add"),
    ("C-d", "security-rule-delete"),
    ("C-e", "security-enrol"),
];

/// The question the sudo door's password is typed into (secret).
pub const PASSWORD: &str = "security-password";
/// The question a rule is typed into.
pub const RULE: &str = "security-rule";
/// The question VirusTotal's key is pasted into (secret).
pub const VT_KEY: &str = "security-vt-key";
/// The question a password to check is typed into (secret) — also the
/// command `LEADER s e p` runs.
pub const CHECK_PASSWORD: &str = "security-check-password";
/// The question an address to check is typed into — `LEADER s e m`.
pub const CHECK_EMAIL: &str = "security-check-email";
/// The question a file to check is typed into — `LEADER s e v`, and the
/// file manager's "Check with VirusTotal" with the path as its argument.
pub const CHECK_FILE: &str = "security-check-file";

/// "Treat as home" for the network that just came up — what the watch's
/// event offers on the minibuffer's line ([`home_errand`]).
pub const HOME: &str = "security-home";

/// The minutes a window opens for when nobody said (`LEADER s e r`).
pub const RELAX_DEFAULT: RelaxMinutes = RelaxMinutes::Fifteen;

/// Every command this view's keys, rows and questions reach — for
/// `leader::reachable`.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW.iter().map(|(_, id)| *id).chain([
        "security-open",
        "security-window",
        "security-firewall",
        PASSWORD,
        RULE,
        VT_KEY,
        CHECK_PASSWORD,
        CHECK_EMAIL,
        CHECK_FILE,
        "security-vt-forget",
        HOME,
    ])
}

// ─── the sections ───────────────────────────────────────────────────────────

/// A section of the page — also what `security-open <word>` opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    /// What is off.
    Now,
    /// The firewall.
    Firewall,
    /// sudo.
    Admin,
    /// Fingerprints (only with a reader).
    Finger,
    /// The device's facts.
    Device,
    /// Network privacy.
    Privacy,
    /// Online checks.
    Online,
    /// Suggestions.
    Suggest,
}

impl Section {
    /// All, in the page's order.
    pub const ALL: [Self; 8] = [
        Self::Now,
        Self::Firewall,
        Self::Admin,
        Self::Finger,
        Self::Device,
        Self::Privacy,
        Self::Online,
        Self::Suggest,
    ];

    /// Its word, as `security-open` takes it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Now => "now",
            Self::Firewall => "firewall",
            Self::Admin => "admin",
            Self::Finger => "finger",
            Self::Device => "device",
            Self::Privacy => "privacy",
            Self::Online => "online",
            Self::Suggest => "suggest",
        }
    }

    /// The section a word names.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.word() == word.trim())
    }

    /// Its title.
    const fn heading(self) -> &'static str {
        match self {
            Self::Now => key::SECURITY_HEADING_NOW,
            Self::Firewall => key::SECURITY_HEADING_FIREWALL,
            Self::Admin => key::SECURITY_HEADING_ADMIN,
            Self::Finger => key::SECURITY_HEADING_FINGER,
            Self::Device => key::SECURITY_HEADING_DEVICE,
            Self::Privacy => key::SECURITY_HEADING_PRIVACY,
            Self::Online => key::SECURITY_HEADING_ONLINE,
            Self::Suggest => key::SECURITY_HEADING_SUGGEST,
        }
    }
}

// ─── the scene ──────────────────────────────────────────────────────────────

/// A listening socket that answers from outside this machine.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Listener {
    /// `tcp` or `udp`.
    pub proto: String,
    /// The port.
    pub port: u16,
    /// The program holding it, when the kernel let us see (data).
    pub program: Option<String>,
}

/// A port a container publishes.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Published {
    /// The container's name (data).
    pub name: String,
    /// `tcp` or `udp`.
    pub proto: String,
    /// The port on this machine.
    pub port: u16,
}

/// A connection refused lately, from the watch.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Blocked {
    /// `tcp`, `udp`.
    pub proto: String,
    /// The port asked for.
    pub port: Option<u16>,
    /// The interface.
    pub iface: String,
    /// Who asked.
    pub source: String,
    /// The newest, in seconds since the epoch.
    pub at: i64,
    /// How many.
    pub count: u64,
}

/// One of sudo's events, from the watch: its words are the watch's own key.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Admin {
    /// When.
    pub at: i64,
    /// The watch's key for what happened.
    pub said: String,
    /// Who and what (data).
    pub who: String,
    /// It was a failure or a refusal.
    pub warning: bool,
}

/// A connection's privacy, by ids (`ronoctua_network::privacy`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Link {
    /// What the person called the profile (data).
    pub name: String,
    /// `Mac::id`.
    pub mac: String,
    /// `Tls::id`.
    pub dns: String,
}

/// fwupd's two facts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Firmware {
    /// The HSI level (`HSI:2`), when fwupd said one.
    pub hsi: Option<String>,
    /// Devices with an update available.
    pub updates: u32,
}

/// Everything the page draws — read by [`Scene::read`], or handed in whole
/// by a photograph (`--state`).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scene {
    /// It has been read at least once.
    pub read: bool,
    /// The person's settings; `None` before the helper ever wrote them.
    pub settings: Option<Settings>,
    /// Why they could not be read, when they could not.
    pub settings_refused: Option<Why>,
    /// What the helper last did.
    pub status: Option<Status>,
    /// Each interface's connection (`links.json`).
    pub links: BTreeMap<String, String>,
    /// The passwordless window's end, in seconds since the epoch.
    pub until: Option<i64>,
    /// The session was given the root helper.
    pub helper: bool,
    /// The fingerprint reader; `None` hides the section.
    pub reader: Option<Reader>,
    /// The enrolled fingers.
    pub fingers: Vec<Finger>,
    /// Fingers answer sudo in terminals.
    pub finger_sudo: bool,
    /// Secure Boot.
    pub secure_boot: Option<SecureBoot>,
    /// The TPM.
    pub tpm: Option<Tpm>,
    /// The system's disk is encrypted; `None` when it could not be read.
    pub encrypted: Option<bool>,
    /// fwupd; `None` when it did not answer.
    pub firmware: Option<Firmware>,
    /// Each connection that is up.
    pub privacy: Vec<Link>,
    /// The listening sockets that answer from outside.
    pub listening: Vec<Listener>,
    /// The containers' published ports.
    pub published: Vec<Published>,
    /// Refused lately.
    pub blocked: Vec<Blocked>,
    /// sudo, this week.
    pub admin: Vec<Admin>,
    /// The watch did not answer (the brain is down, or the module is off).
    pub watch_silent: bool,
    /// The person kept a VirusTotal key.
    pub vt_key: bool,
    /// The files checked.
    pub scans: Vec<ronoctua_lookup::virustotal::Scan>,
}

impl Scene {
    /// The settings in force: the file's, or what the helper assumes before
    /// it ever wrote one.
    #[must_use]
    pub fn settings(&self) -> Settings {
        self.settings.clone().unwrap_or_default()
    }

    /// Reads everything. BLOCKS (D-Bus, podman, the brain): a door's thread —
    /// once, when the page opens.
    #[must_use]
    pub fn read() -> Self {
        let mut scene = Self::default();
        scene.read_slow();
        scene.read_fast();
        scene
    }

    /// The same scene with what MOVES read again — what a folder that
    /// changed, a work the door ran or the watch's page can have changed —
    /// and the facts that do not move kept: the reader, Secure Boot, the TPM,
    /// the disk, fwupd and podman are asked once, when the page opens, never
    /// on every file the helper writes. BLOCKS: a door's thread.
    #[must_use]
    pub fn refreshed(mut self) -> Self {
        self.read_fast();
        self
    }

    /// The facts that do not move while the page is open, and are slow to
    /// ask (D-Bus activations, a podman process).
    fn read_slow(&mut self) {
        use ronoctua_security::{fprint, platform};
        self.reader = fprint::reader().ok().flatten();
        self.secure_boot = Some(platform::secure_boot());
        self.tpm = Some(platform::tpm());
        self.encrypted = encrypted();
        self.firmware = platform::firmware().ok().map(|f| Firmware {
            hsi: f.hsi,
            updates: f.updates,
        });
        self.published = published();
    }

    /// What moves: the helper's files, the fingers, the connections, the
    /// listening sockets, the watch's page, the key and the checks.
    fn read_fast(&mut self) {
        use ronoctua_security::{firewall, fprint};
        // AN Err IS NOT "NO FILE": `Ok(None)` is a machine whose helper never
        // wrote one (the defaults ARE the settings), an Err is a file that is
        // there and cannot be read — and nothing may be written over it from
        // here ([`settings_now`]).
        (self.settings, self.settings_refused) = match firewall::read_settings() {
            Ok(s) => (s, None),
            Err(r) => (None, Some(r.why)),
        };
        self.links = std::fs::read(firewall::LINKS)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let user = std::env::var("USER").unwrap_or_default();
        self.fingers = if self.reader.is_some() {
            fprint::enrolled(&user).unwrap_or_default()
        } else {
            Vec::new()
        };
        (self.blocked, self.admin, self.watch_silent) = watched();
        self.scans = ronoctua_lookup::virustotal::scans()
            .parsed()
            .unwrap_or_default()
            .scans;
        self.status = firewall::read_status();
        self.until = ronoctua_security::relax::open_until();
        self.helper = ronoctua_security::authority::has_helper();
        self.finger_sudo = fprint::sudo_allows(&user);
        self.privacy = links_up();
        self.listening = listening();
        self.vt_key = ronoctua_lookup::keys::has(ronoctua_lookup::keys::Key::VirusTotal);
        self.read = true;
    }

    /// The profiles of the networks the machine is on now (`firewall.status`),
    /// each once; empty before the helper ever said.
    #[must_use]
    pub fn profiles_in_use(&self) -> Vec<Profile> {
        let mut out: Vec<Profile> = self
            .status
            .iter()
            .flat_map(|s| s.profiles.values().copied())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The profile a refused connection's interface has now: the helper's
    /// word for it, or — for an interface it has not placed — its connection's
    /// in the settings, or the default.
    #[must_use]
    pub fn profile_of_interface(&self, iface: &str) -> Profile {
        if let Some(p) = self.status.as_ref().and_then(|s| s.profiles.get(iface)) {
            return *p;
        }
        let settings = self.settings();
        self.links
            .get(iface)
            .map_or(settings.default_profile, |uuid| settings.profile_of(uuid))
    }

    /// Whether anything answers on port 22 from outside: the SSH server.
    #[must_use]
    pub fn ssh_listening(&self) -> bool {
        self.listening
            .iter()
            .any(|l| l.proto == "tcp" && l.port == 22)
    }
}

/// Whether the system's disk is encrypted: the filesystem under the store, the
/// home or the root comes out of a LUKS container.
fn encrypted() -> Option<bool> {
    let rack = ronoctua_disks::Manager::fresh().ok()?.read().ok()?;
    ["/nix", "/home", "/"].iter().find_map(|at| {
        rack.volumes
            .iter()
            .find(|v| v.mounted_at.iter().any(|m| m == at))
            .map(|v| v.behind.is_some())
    })
}

/// Each connection that is up, as ids.
fn links_up() -> Vec<Link> {
    let Ok(manager) = ronoctua_network::Manager::fresh() else {
        return Vec::new();
    };
    manager
        .privacy()
        .unwrap_or_default()
        .into_iter()
        .map(|p| Link {
            name: p.id,
            mac: p.mac.id().to_owned(),
            dns: p.dns.id().to_owned(),
        })
        .collect()
}

/// The listening sockets not bound to the loopback, one per protocol and
/// port.
fn listening() -> Vec<Listener> {
    let places = ronoctua_memory::Places::default();
    let mut out: Vec<Listener> = Vec::new();
    for s in ronoctua_memory::net::sockets(&places) {
        if !s.listening || s.address.is_loopback() {
            continue;
        }
        let proto = s.protocol.file().trim_end_matches('6').to_owned();
        if out.iter().any(|l| l.proto == proto && l.port == s.port) {
            continue;
        }
        out.push(Listener {
            proto,
            port: s.port,
            program: s.owner.map(|o| o.program),
        });
    }
    out
}

/// The ports the running containers publish.
fn published() -> Vec<Published> {
    let Some(podman) = ronoctua_containers::Podman::from_env() else {
        return Vec::new();
    };
    let Ok(listing) = podman.list() else {
        return Vec::new();
    };
    listing
        .containers
        .iter()
        .filter(|c| c.state == ronoctua_containers::State::Running)
        .flat_map(|c| {
            let name = c.names.first().cloned().unwrap_or_else(|| c.id.clone());
            c.ports.iter().map(move |p| Published {
                name: name.clone(),
                proto: p.protocol.clone(),
                port: p.host_port,
            })
        })
        .collect()
}

/// The watch's page, as the two lists this page draws: refused lately, and
/// sudo's events — and whether it answered at all.
fn watched() -> (Vec<Blocked>, Vec<Admin>, bool) {
    let asked = ronoctua_brain::client::ask(&ronoctua_brain::Ask::Screen {
        module: WATCH.to_owned(),
    });
    match asked {
        Ok(ronoctua_brain::Say::Screen { screen, .. }) => {
            let (blocked, admin) = lists_of(&screen);
            (blocked, admin, false)
        }
        _ => (Vec::new(), Vec::new(), true),
    }
}

/// The two lists, out of the watch's page, by the row ids it writes
/// (`blocked:<proto>:<port>:<iface>:<source>`, `sudo:<n>`). Pure.
#[must_use]
pub fn lists_of(screen: &ronoctua_brain::module::Screen) -> (Vec<Blocked>, Vec<Admin>) {
    use ronoctua_brain::module::{Cell, Tone};
    let mut blocked = Vec::new();
    let mut admin = Vec::new();
    for row in screen.sections.iter().flat_map(|s| s.rows.iter()) {
        if let Some(rest) = row.id.strip_prefix("blocked:") {
            let parts: Vec<&str> = rest.splitn(4, ':').collect();
            let [proto, port, iface, source] = parts.as_slice() else {
                continue;
            };
            blocked.push(Blocked {
                proto: (*proto).to_owned(),
                port: port.parse().ok(),
                iface: (*iface).to_owned(),
                source: (*source).to_owned(),
                at: match row.value {
                    Cell::Moment(m) => m,
                    _ => 0,
                },
                count: match row.note {
                    Cell::Count(n) => n,
                    _ => 1,
                },
            });
        } else if row.id.starts_with("sudo:") {
            let Cell::Key(said) = &row.label else {
                continue;
            };
            admin.push(Admin {
                at: match row.value {
                    Cell::Moment(m) => m,
                    _ => 0,
                },
                said: said.to_string(),
                who: match &row.note {
                    Cell::Data(d) => d.clone(),
                    _ => String::new(),
                },
                warning: row.tone != Tone::Quiet,
            });
        }
    }
    (blocked, admin)
}

/// The section a mark of the watch opens on: its segment id is
/// `brain:<module>/<mark>`, and the passwordless window's mark is about the
/// administrator, every other one about the firewall. `None` for any other
/// module's mark, which opens its own page. Pure.
#[must_use]
pub fn section_of_mark(module: &str, segment: &str) -> Option<Section> {
    if module != WATCH {
        return None;
    }
    let mark = segment.rsplit_once('/').map(|(_, m)| m)?;
    Some(if mark == "sudo-open" {
        Section::Admin
    } else {
        Section::Firewall
    })
}

// ─── what the doors do ──────────────────────────────────────────────────────

/// What the sudo door is asked to do — each one a [`RootVerb`], and the
/// firewall's a whole [`Settings`] staged right before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Work {
    /// Open the passwordless window.
    Relax(RelaxMinutes),
    /// Close it now.
    RelaxEnd,
    /// Apply these settings, whole.
    Firewall(Box<Settings>),
    /// Enrol a finger.
    Enrol(Finger),
    /// Remove fingers.
    Delete(Fingers),
    /// Fingers for sudo in terminals, on or off.
    FingerSudo(bool),
}

impl Work {
    /// The helper's verb.
    #[must_use]
    pub const fn verb(&self) -> RootVerb {
        match self {
            Self::Relax(minutes) => RootVerb::Relax { minutes: *minutes },
            Self::RelaxEnd => RootVerb::RelaxEnd,
            Self::Firewall(_) => RootVerb::FirewallApply,
            Self::Enrol(finger) => RootVerb::FingerEnroll { finger: *finger },
            Self::Delete(which) => RootVerb::FingerDelete { which: *which },
            Self::FingerSudo(on) => RootVerb::FingerSudo { on: *on },
        }
    }

    /// What it says when it was done.
    const fn done_key(&self) -> &'static str {
        match self {
            Self::Relax(_) => key::ECHO_SECURITY_RELAXED,
            Self::RelaxEnd => key::ECHO_SECURITY_RELAX_ENDED,
            Self::Firewall(_) => key::ECHO_SECURITY_FIREWALL_APPLIED,
            Self::Enrol(_) => key::ECHO_SECURITY_FINGER_ENROLLED,
            Self::Delete(_) => key::ECHO_SECURITY_FINGER_REMOVED,
            Self::FingerSudo(_) => key::ECHO_SECURITY_FINGER_SUDO_CHANGED,
        }
    }
}

/// Runs `work` through the sudo door: the settings staged first (as the
/// person, checked before anybody types a password), then the helper — with
/// every progress word (an enrolment's touches) handed to `step`. BLOCKS: a
/// door's thread.
///
/// # Errors
///
/// The door's and the helper's refusals.
pub fn run_work(work: &Work, proof: &Proof, step: &mut dyn FnMut(&str)) -> Result<(), Why> {
    if let Work::Firewall(settings) = work {
        ronoctua_security::firewall::stage(settings).map_err(|r| r.why)?;
    }
    ronoctua_security::authority::run_with(work.verb(), proof, step)
        .map(|_| ())
        .map_err(|r| r.why)
}

/// What came back from a door's thread.
#[derive(Debug)]
pub enum Came {
    /// The scene, read.
    Scene(Box<Scene>),
    /// Something the page draws may have moved — a folder the helper
    /// writes, the checks, the keys, the watch's page — and the scene is to
    /// be read again ([`Job::Refresh`]).
    Stirred,
    /// The door ran `work`, and this is how it ended.
    Door {
        /// What it was.
        work: Work,
        /// How it ended.
        result: Result<(), Why>,
    },
    /// A progress word of the work in flight (an enrolment's touch).
    Step(String),
    /// The shields were raised, or not.
    Shields(Result<(), Why>),
    /// A file was hashed and asked about.
    Hashed {
        /// The file.
        path: PathBuf,
        /// Its hash, and what VirusTotal said of it.
        result: Result<(String, ronoctua_lookup::virustotal::Checked), ronoctua_lookup::Refusal>,
    },
    /// An address was asked about.
    Email {
        /// The address.
        address: String,
        /// The breaches it is in.
        result: Result<Vec<String>, ronoctua_lookup::Refusal>,
    },
    /// A password was asked about.
    Password(Result<u64, ronoctua_lookup::Refusal>),
}

/// Lets the host of a check the person asked for (`ronoctua-consent`): the
/// row said where it goes, and pressing it is the consent — recorded, as a
/// search the person configured is.
fn let_host(service: ronoctua_lookup::Service) -> Result<(), ronoctua_lookup::Refusal> {
    let hosts: Vec<&str> = service.hosts().iter().map(|h| h.name()).collect();
    ronoctua_consent::let_hosts(&hosts, true)
        .map_err(|_| ronoctua_lookup::Refusal::new(service, ronoctua_lookup::Why::NotLet))
}

/// Hashes a file and asks VirusTotal about the hash. BLOCKS: a thread.
///
/// # Errors
///
/// A file that cannot be read, no key, and the host's refusals.
pub fn hash_and_ask(
    path: &std::path::Path,
) -> Result<(String, ronoctua_lookup::virustotal::Checked), ronoctua_lookup::Refusal> {
    use ronoctua_lookup::{Refusal, Service, Why as W};
    let refused = |why| Refusal::new(Service::VirusTotal, why);
    let key = ronoctua_lookup::keys::needed(ronoctua_lookup::keys::Key::VirusTotal)?;
    let sha = ronoctua_lookup::virustotal::sha256_of(path, &mut |_, _| true)
        .map_err(|_| refused(W::Unreadable))?;
    let_host(Service::VirusTotal)?;
    let checked =
        ronoctua_lookup::virustotal::lookup(&ronoctua_lookup::internet::Internet, &key, &sha)?;
    let shown = path.to_string_lossy().into_owned();
    let state = match &checked {
        ronoctua_lookup::virustotal::Checked::Known(report) => {
            ronoctua_lookup::virustotal::State::Checked {
                report: report.clone(),
            }
        }
        ronoctua_lookup::virustotal::Checked::Unknown => {
            ronoctua_lookup::virustotal::State::Unknown
        }
    };
    let scan = ronoctua_lookup::virustotal::Scan {
        sha256: sha.clone(),
        path: shown,
        at_s: now_s(),
        state,
    };
    if let Err(e) = ronoctua_lookup::virustotal::keep(&ronoctua_lookup::virustotal::scans(), scan) {
        tracing::warn!(error = %e, "scans.json could not be written");
    }
    Ok((sha, checked))
}

/// Asks XposedOrNot about an address. BLOCKS: a thread.
///
/// # Errors
///
/// Not an address, and the host's refusals.
pub fn ask_email(address: &str) -> Result<Vec<String>, ronoctua_lookup::Refusal> {
    // CHECKED FIRST: an answer that is not one address is refused before the
    // host is let or asked.
    ronoctua_lookup::breach::request(address)?;
    let_host(ronoctua_lookup::Service::XposedOrNot)?;
    ronoctua_lookup::breach::check(&ronoctua_lookup::internet::Internet, address)
}

/// Asks Pwned Passwords about a password — five digits of its hash leave.
/// BLOCKS: a thread.
///
/// # Errors
///
/// An empty password, and the host's refusals.
pub fn ask_password(password: Secret) -> Result<u64, ronoctua_lookup::Refusal> {
    let_host(ronoctua_lookup::Service::PwnedPasswords)?;
    ronoctua_lookup::pwned::check(&ronoctua_lookup::internet::Internet, password)
}

/// Seconds since the epoch.
fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// What the view asks its door to do.
#[derive(Debug)]
pub enum Errand {
    /// Redraw and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Ask on the door's line: under `id`, hidden when `secret`, starting
    /// from `initial`.
    Ask {
        /// The question's id — its command.
        id: &'static str,
        /// Hidden while typed.
        secret: bool,
        /// What the line holds at first.
        initial: Option<String>,
    },
    /// Run this through the sudo door, on a thread.
    Door(Work, Proof),
    /// Raise the shields, on a thread.
    Shields,
    /// Hash a file and ask about it, on a thread.
    Hash(PathBuf),
    /// Queue a file's upload (`Work::Scan`).
    Upload {
        /// The file.
        path: PathBuf,
        /// Its hash when the question was asked.
        sha256: String,
    },
    /// Ask about an address, on a thread.
    Email(String),
    /// Ask about a password, on a thread.
    Password(Secret),
    /// Keep VirusTotal's key.
    KeepKey(Secret),
    /// Forget it.
    ForgetKey,
    /// Run a command of the registry (another view) — the page is left
    /// first in the footer, whose region the other view takes.
    Command(&'static str),
    /// Show the page on this section (a leader command with the view
    /// closed).
    Show(Section),
    /// Open an address in the browser.
    Url(String),
    /// Say this on the door's line.
    Say(String),
}

/// What a question on the page is about.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Confirm {
    /// Send this file to VirusTotal.
    Upload {
        /// The file.
        path: PathBuf,
        /// Its hash.
        sha256: String,
    },
    /// Remove this finger.
    RemoveFinger(Finger),
}

// ─── the rows ───────────────────────────────────────────────────────────────

/// What `RET` on a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Act {
    /// Open the page on a section.
    Show(Section),
    /// The firewall on or off.
    Firewall,
    /// A network's profile, the other way.
    Profile(String),
    /// Treat this connection as home.
    Home(String),
    /// Shields up or down.
    Shields,
    /// The log's level, the next one.
    Log,
    /// Add a rule.
    RuleAdd,
    /// Edit this rule.
    Rule(String),
    /// Allow a port on the networks of one profile — the one the refused
    /// connection came in on, or the one a listening port is refused in.
    Allow {
        /// `tcp`/`udp`.
        proto: Protocol,
        /// The port.
        port: u16,
        /// Where: home networks, or public ones.
        profile: Profile,
        /// Why, for the rule's note (data).
        note: String,
    },
    /// Open the window for these minutes.
    Relax(RelaxMinutes),
    /// End it.
    RelaxEnd,
    /// Enrol the chosen finger.
    Enrol,
    /// Remove this finger.
    Finger(Finger),
    /// Fingers for sudo, the other way.
    FingerSudo,
    /// Paste VirusTotal's key.
    VtKey,
    /// Forget it.
    VtForget,
    /// Check a file.
    CheckFile,
    /// Check an address.
    CheckEmail,
    /// Check a password.
    CheckPassword,
    /// Open a file's report in the browser.
    Report(String),
    /// Run a command of the registry.
    Command(&'static str),
    /// Open an address in the browser.
    Url(&'static str),
    /// The answer to the page's question.
    Answer(bool),
}

impl Act {
    /// The command a row runs, when it is one of the registry's — what the
    /// pointer's sentence explains.
    const fn command(&self) -> Option<&'static str> {
        Some(match self {
            Self::Firewall => "security-firewall",
            Self::Shields => "security-shields",
            Self::RuleAdd => "security-rule-add",
            Self::Relax(_) | Self::RelaxEnd => "security-relax",
            Self::Enrol => "security-enrol",
            Self::VtKey => VT_KEY,
            Self::VtForget => "security-vt-forget",
            Self::CheckFile => CHECK_FILE,
            Self::CheckEmail => CHECK_EMAIL,
            Self::CheckPassword => CHECK_PASSWORD,
            Self::Command(id) => *id,
            _ => return None,
        })
    }

    /// The sentence under the pointer for a row that is no command.
    const fn about(&self) -> &'static str {
        match self {
            Self::Show(_) => key::SECURITY_ABOUT_SHOW,
            Self::Profile(_) | Self::Home(_) => key::SECURITY_ABOUT_PROFILE,
            Self::Log => key::SECURITY_ABOUT_LOG,
            Self::Rule(_) => key::SECURITY_ABOUT_RULE,
            // WHERE IT ALLOWS, said under the pointer before the press: the
            // rule is for one profile, and a public network is not a home one.
            Self::Allow {
                profile: Profile::Home,
                ..
            } => key::SECURITY_ABOUT_ALLOW_HOME,
            Self::Allow {
                profile: Profile::Public,
                ..
            } => key::SECURITY_ABOUT_ALLOW_PUBLIC,
            Self::Finger(_) => key::SECURITY_ABOUT_FINGER,
            Self::FingerSudo => key::SECURITY_ABOUT_FINGER_SUDO,
            Self::Report(_) => key::SECURITY_ABOUT_REPORT,
            Self::Url(_) => key::SECURITY_ABOUT_URL,
            Self::Answer(_) => key::SECURITY_ABOUT_ANSWER,
            _ => key::SECURITY_ABOUT_SHOW,
        }
    }
}

/// One drawn row.
#[derive(Debug, Clone)]
pub struct Row {
    /// The pieces, placed.
    pub fields: Vec<Field>,
    /// What `RET` does — only on a row that acts.
    pub act: Option<Act>,
    /// A title, a note, a fact: the cursor skips it.
    pub heading: bool,
    /// The section it is in.
    pub section: Section,
}

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row that acts.
    Up,
    /// Down one.
    Down,
    /// The finger ring, back.
    Left,
    /// The finger ring, on.
    Right,
    /// `RET`.
    Enter,
    /// Close.
    Escape,
    /// The window: open, or end.
    Relax,
    /// Shields up, or down.
    Shields,
    /// Add a rule.
    RuleAdd,
    /// Remove the rule (or finger) under the cursor.
    RuleDelete,
    /// Enrol the chosen finger.
    Enrol,
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` is the way back whatever is held.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    press_of(id)
}

/// A command of [`OF_THE_VIEW`] as the view's press.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "security-down" => Press::Down,
        "security-up" => Press::Up,
        "security-enter" => Press::Enter,
        "security-left" => Press::Left,
        "security-right" => Press::Right,
        "security-relax" => Press::Relax,
        "security-shields" => Press::Shields,
        "security-rule-add" => Press::RuleAdd,
        "security-rule-delete" => Press::RuleDelete,
        "security-enrol" => Press::Enrol,
        _ => return None,
    })
}

/// One entry of a section, before the section is laid out whole.
enum Entry {
    /// A `label  value` row ([`Lines::pair`]).
    Pair {
        label: String,
        value: String,
        act: Option<Act>,
        status: Option<tokens::Color>,
    },
    /// A row of "Now" that is off ([`Lines::off`]).
    Off {
        label: String,
        value: String,
        act: Act,
    },
    /// A sentence in one colour, wrapped ([`Lines::line`]).
    Line(String, tokens::Color),
}

/// The builder of the rows.
///
/// A SECTION IS LAID OUT WHOLE (decision A2 of 2026-09-30: every view of
/// the wave): its entries wait until the next title or the end, and then
/// every value of the section starts in one column after its widest label
/// (`chrome::section_column`), a label or a value that does not fit wrapped
/// under itself (`chrome::aligned_pair`) — never a value in a column of its
/// own per row, and never one cut.
struct Lines<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    section: Section,
    texts: &'a ronoctua_i18n::Texts,
    /// The current section's entries, not laid out yet.
    pending: Vec<Entry>,
}

impl Lines<'_> {
    fn piece(&self, at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text: ronoctua_render::layout::elide_end(&text, self.room),
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    fn heading(&mut self, section: Section) {
        self.flush();
        self.section = section;
        // AIR BEFORE EVERY TITLE but the first — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                act: None,
                heading: true,
                section,
            });
        }
        let field = self.piece(
            self.left,
            self.texts.t(section.heading()),
            tokens::color::TEXT_HEADING,
        );
        self.out.push(Row {
            fields: vec![field],
            act: None,
            heading: true,
            section,
        });
    }

    /// A `label: value` row; it acts when `act` is given. `status` tints the
    /// value (danger, warning) except under the cursor.
    fn pair(&mut self, label: &str, value: &str, act: Option<Act>, status: Option<tokens::Color>) {
        self.pending.push(Entry::Pair {
            label: label.to_owned(),
            value: value.to_owned(),
            act,
            status,
        });
    }

    /// A row of "Now": something that is off, the WHOLE row in the danger
    /// colour — label and value, since the label is often all it says —
    /// except under the cursor, where the cursor's colour says it.
    fn off(&mut self, label: &str, value: &str, act: Act) {
        self.pending.push(Entry::Off {
            label: label.to_owned(),
            value: value.to_owned(),
            act,
        });
    }

    /// A row whose whole text is one sentence in `color`, wrapped.
    fn line(&mut self, text: &str, color: tokens::Color) {
        self.pending.push(Entry::Line(text.to_owned(), color));
    }

    /// The rows, the last section laid out.
    fn finish(mut self) -> Vec<Row> {
        self.flush();
        self.out
    }

    /// Lays the current section out: one value column for all its pairs.
    fn flush(&mut self) {
        let entries = std::mem::take(&mut self.pending);
        let end = self.left + self.room;
        let pairs: Vec<(u16, u16, u16)> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::Pair { label, value, .. } | Entry::Off { label, value, .. } => Some((
                    self.left,
                    ronoctua_render::layout::width(label),
                    ronoctua_render::layout::width(value),
                )),
                Entry::Line(..) => None,
            })
            .collect();
        let column = crate::chrome::section_column(self.left, self.room, &pairs);
        for entry in entries {
            match entry {
                Entry::Pair {
                    label,
                    value,
                    act,
                    status,
                } => {
                    let focused = act.is_some() && self.cursor == self.out.len();
                    let kind = crate::chrome::Kind::of(act.is_some());
                    let lines = crate::chrome::aligned_pair(
                        self.left, column, end, &label, &value, kind, focused,
                    );
                    self.push_lines(lines, act, |fields| {
                        crate::chrome::tinted(fields, status, focused)
                    });
                }
                Entry::Off { label, value, act } => {
                    let focused = self.cursor == self.out.len();
                    let lines = crate::chrome::aligned_pair(
                        self.left,
                        column,
                        end,
                        &label,
                        &value,
                        crate::chrome::Kind::of(true),
                        focused,
                    );
                    self.push_lines(lines, Some(act), |mut fields| {
                        if !focused {
                            for f in &mut fields {
                                f.color = tokens::color::STATE_ERROR;
                            }
                        }
                        fields
                    });
                }
                Entry::Line(text, color) => {
                    for l in crate::chrome::note_lines(&text, self.room) {
                        let field = self.piece(self.left, l, color);
                        self.out.push(Row {
                            fields: vec![field],
                            act: None,
                            heading: true,
                            section: self.section,
                        });
                    }
                }
            }
        }
    }

    /// The lines of one pair, every one through `paint`: the first carries
    /// the act (and is where the cursor lands), the ones it wrapped onto are
    /// read past, as a fact is.
    fn push_lines(
        &mut self,
        lines: Vec<Vec<Field>>,
        act: Option<Act>,
        paint: impl Fn(Vec<Field>) -> Vec<Field>,
    ) {
        let mut act = act;
        for fields in lines {
            let act = act.take();
            self.out.push(Row {
                fields: paint(fields),
                heading: act.is_none(),
                act,
                section: self.section,
            });
        }
    }

    /// A quieter note, wrapped and never cut.
    fn note(&mut self, text: &str) {
        self.line(text, tokens::color::TEXT_DIM);
    }

    fn t(&self, k: &str) -> String {
        self.texts.t(k)
    }
}

/// Where the page is to be brought, the next time it is drawn over a scene
/// that was read: somebody asked for it (a section's word, a check that
/// answered, a question), and only then does the page move.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Target {
    /// The section's title first on screen, and the cursor on its first row
    /// that acts — or the next one after it, for a section of facts only.
    Section(Section),
    /// This row first on screen, with the cursor on it — a check's row, with
    /// its answer drawn right under it.
    Row(Act),
    /// The top: the page's own question.
    Top,
}

/// The view: where the cursor is, what the page asks, and what is in flight.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    /// Brought into view at the next rows over a scene that was read.
    open_at: Option<Target>,
    /// The page's own yes-or-no question.
    confirm: Option<Confirm>,
    /// A sudo work waiting for the password question.
    pending: Option<Work>,
    /// A sudo work in flight.
    busy: Option<Work>,
    /// The last enrolment word.
    step: Option<String>,
    /// The finger the enrol row offers (the ring).
    finger_at: usize,
    /// The rule being edited through the question line.
    editing: Option<String>,
    /// The last answers of the online checks.
    email: Option<(String, Result<Vec<String>, ronoctua_lookup::Refusal>)>,
    password: Option<Result<u64, ronoctua_lookup::Refusal>>,
    /// An online check is in flight: its section says so until it answers.
    checking: bool,
}

impl View {
    /// Opens it on the first row that acts.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Opens it on a section.
    #[must_use]
    pub fn at(section: Option<Section>) -> Self {
        Self {
            open_at: section.map(Target::Section),
            ..Self::default()
        }
    }

    /// Moves it to a section at the next rows.
    pub fn open_on(&mut self, section: Section) {
        self.open_at = Some(Target::Section(section));
    }

    /// Lets every question of the page go — the footer's minibuffer was let
    /// go (`Esc`, `C-g`): a work waiting for a password, a rule being edited.
    /// An answer typed later under the same command, by name, must not be
    /// taken for them.
    pub fn let_go(&mut self) {
        self.pending = None;
        self.editing = None;
    }

    /// Where the cursor is.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The hint line, generated from [`OF_THE_VIEW`] — without a reader, the
    /// finger keys are not on it (there is nothing for them to do), and with
    /// one they are drawn as the passing guests they are: they act only on
    /// the enrol row.
    #[must_use]
    pub fn hint(scene: &Scene, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        let table: Vec<(&'static str, &'static str, bool)> = OF_THE_VIEW
            .iter()
            .filter_map(|&(notation, id)| {
                let finger = FINGER_KEYS.contains(&id);
                (!finger || scene.reader.is_some()).then_some((notation, id, finger))
            })
            .collect();
        crate::chrome::hint_of_marked(&table, |_| None, texts)
    }

    /// Puts the cursor where a click landed; `true` when it was already there
    /// (the second click, `RET`).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(|r| !r.heading) {
            let again = self.cursor == n;
            self.cursor = n;
            return again;
        }
        false
    }

    /// The sentence the pointer resting on row `n` reads.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let act = rows.get(n)?.act.as_ref()?;
        Some(match act.command() {
            Some(id) => texts.t(&format!("cmd-{id}")),
            None => texts.t(act.about()),
        })
    }

    /// The finger the enrol row offers: the chosen one of those not
    /// enrolled — none without a reader.
    fn offered(&self, scene: &Scene) -> Option<Finger> {
        scene.reader.as_ref()?;
        let free: Vec<Finger> = Finger::ALL
            .into_iter()
            .filter(|f| !scene.fingers.contains(f))
            .collect();
        (!free.is_empty()).then(|| free[self.finger_at % free.len()])
    }

    /// The rows, at this width, from the scene as it is.
    #[must_use]
    pub fn rows(
        &self,
        scene: &Scene,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
        zone: &ronoctua_files::when::Zone,
    ) -> Vec<Row> {
        let (left, room) = crate::power::extent(columns);
        let mut b = Lines {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            section: Section::Now,
            texts,
            pending: Vec::new(),
        };
        if let Some(confirm) = &self.confirm {
            self.question(&mut b, confirm);
            return b.finish();
        }
        let now = now_s();
        self.now(&mut b, scene, now, zone);
        self.firewall(&mut b, scene, zone);
        self.admin(&mut b, scene, now, zone);
        if scene.reader.is_some() {
            self.finger(&mut b, scene);
        }
        device(&mut b, scene);
        privacy(&mut b, scene);
        self.online(&mut b, scene);
        suggest(&mut b);
        b.finish()
    }

    /// The page's own question, "no" first, the cursor born on it.
    fn question(&self, b: &mut Lines<'_>, confirm: &Confirm) {
        let (sentence, warning) = match confirm {
            Confirm::Upload { path, .. } => (
                b.texts.t_with(
                    key::SECURITY_VT_ASK_UPLOAD,
                    &[("file", &path.to_string_lossy())],
                ),
                Some(b.t(key::SECURITY_VT_UPLOAD_WARNING)),
            ),
            Confirm::RemoveFinger(f) => (
                b.texts.t_with(
                    key::SECURITY_FINGER_ASK_REMOVE,
                    &[("finger", &b.t(crate::security_words::finger(*f)))],
                ),
                None,
            ),
        };
        b.line(&sentence, tokens::color::TEXT_HEADING);
        if let Some(w) = warning {
            b.line(&w, tokens::color::STATE_WARNING);
        }
        b.pair(
            &b.t(key::SECURITY_ANSWER_NO),
            "",
            Some(Act::Answer(false)),
            None,
        );
        b.pair(
            &b.t(key::SECURITY_ANSWER_YES),
            "",
            Some(Act::Answer(true)),
            None,
        );
    }

    fn now(&self, b: &mut Lines<'_>, scene: &Scene, now: i64, zone: &ronoctua_files::when::Zone) {
        b.heading(Section::Now);
        let settings = scene.settings();
        let mut off = 0;
        if let Some(status) = scene.status.as_ref().filter(|s| !s.ok) {
            off += 1;
            let why = status
                .why
                .map_or_else(String::new, |w| b.t(crate::security_words::refusal(w)));
            b.off(
                &b.t(key::SECURITY_NOW_FIREWALL_FAILED),
                &why,
                Act::Show(Section::Firewall),
            );
        }
        if let Some(why) = scene.settings_refused {
            off += 1;
            b.off(
                &b.t(key::SECURITY_NOW_SETTINGS_UNREADABLE),
                &b.t(crate::security_words::refusal(why)),
                Act::Show(Section::Firewall),
            );
        }
        if !settings.enabled {
            off += 1;
            b.off(&b.t(key::SECURITY_NOW_FIREWALL_OFF), "", Act::Firewall);
        }
        if let Some(until) = scene.until.filter(|u| *u > now) {
            off += 1;
            b.off(
                &b.t(key::SECURITY_NOW_SUDO_OPEN),
                &clock(until, zone, b.texts),
                Act::RelaxEnd,
            );
        }
        if settings.shields {
            off += 1;
            b.off(&b.t(key::SECURITY_NOW_SHIELDS), "", Act::Shields);
        }
        let recent: u64 = scene
            .blocked
            .iter()
            .filter(|x| x.at > now - RECENT_S)
            .map(|x| x.count)
            .sum();
        if recent > 0 {
            off += 1;
            b.off(
                &b.texts.t_args(
                    key::SECURITY_NOW_REFUSED,
                    &[(
                        "count",
                        Arg::Number(i64::try_from(recent).unwrap_or(i64::MAX)),
                    )],
                ),
                "",
                Act::Show(Section::Firewall),
            );
        }
        if off == 0 {
            if scene.read {
                // THE SHIELD, a space of air, and the sentence.
                b.note(&format!(
                    "{} {}",
                    tokens::icon::SECURITY_SHIELD,
                    b.t(key::SECURITY_NOW_NOTHING)
                ));
            } else {
                b.note(&b.t(key::SECURITY_NOW_READING));
            }
        }
    }

    fn firewall(&self, b: &mut Lines<'_>, scene: &Scene, zone: &ronoctua_files::when::Zone) {
        b.heading(Section::Firewall);
        let settings = scene.settings();
        // SETTINGS THAT CANNOT BE READ ARE NOT THE DEFAULTS: what they say is
        // drawn as unknown, and nothing that would write them is offered —
        // the shields only, which the helper raises over the file it keeps.
        let known = scene.settings_refused.is_none();
        if scene.settings_refused.is_some() {
            b.line(
                &b.t(key::SECURITY_NOW_SETTINGS_UNREADABLE),
                tokens::color::STATE_ERROR,
            );
        }
        let unknown = b.t(key::SECURITY_UNKNOWN);
        let switch = if !known {
            unknown.clone()
        } else if settings.enabled {
            // THE GLYPH, then a space of air, then the word.
            format!(
                "{} {}",
                tokens::icon::SECURITY_SHIELD,
                b.t(key::SECURITY_ON)
            )
        } else {
            format!(
                "{} {}",
                tokens::icon::SECURITY_SHIELD_OFF,
                b.t(key::SECURITY_OFF)
            )
        };
        b.pair(
            &b.t(key::SECURITY_FW_SWITCH),
            &switch,
            known.then_some(Act::Firewall),
            (known && !settings.enabled).then_some(tokens::color::STATE_ERROR),
        );
        // EACH NETWORK THE MACHINE IS ON, with its profile — and the ones it
        // SERVES (a hotspot) said as that, neither home nor public.
        if let Some(status) = &scene.status {
            for (iface, profile) in &status.profiles {
                let Some(uuid) = scene.links.get(iface) else {
                    continue;
                };
                b.pair(
                    &b.texts
                        .t_with(key::SECURITY_FW_NETWORK, &[("iface", iface)]),
                    &b.t(crate::security_words::profile(*profile)),
                    known.then(|| Act::Profile(uuid.clone())),
                    None,
                );
                if known && status.unplaced.contains(uuid) && *profile == Profile::Public {
                    b.pair(
                        &b.t(key::SECURITY_FW_TREAT_AS_HOME),
                        "",
                        Some(Act::Home(uuid.clone())),
                        None,
                    );
                }
            }
            for iface in &status.served {
                b.pair(
                    &b.texts
                        .t_with(key::SECURITY_FW_NETWORK, &[("iface", iface)]),
                    &b.t(key::SECURITY_FW_SERVED),
                    None,
                    None,
                );
            }
        }
        let shields_word = b.t(if settings.shields {
            key::SECURITY_ON
        } else {
            key::SECURITY_OFF
        });
        b.pair(
            &b.t(key::SECURITY_FW_SHIELDS),
            if known { &shields_word } else { &unknown },
            Some(Act::Shields),
            (known && settings.shields).then_some(tokens::color::STATE_ERROR),
        );
        b.note(&b.t(key::SECURITY_FW_SHIELDS_NOTE));
        if known {
            b.pair(
                &b.t(key::SECURITY_FW_LOG),
                &b.t(crate::security_words::log_level(settings.log)),
                Some(Act::Log),
                None,
            );
            // THE PERSON'S RULES, each one line of the grammar the question
            // takes.
            for rule in &settings.rules {
                b.pair(
                    &rule_text(rule, b.texts),
                    &rule.note,
                    Some(Act::Rule(rule.id.clone())),
                    None,
                );
            }
            b.pair(
                &b.t(key::SECURITY_FW_RULE_ADD),
                "",
                Some(Act::RuleAdd),
                None,
            );
        }
        // REFUSED LATELY, from the watch, each with "allow".
        if scene.watch_silent {
            b.note(&b.t(key::SECURITY_FW_WATCH_SILENT));
        } else if !scene.blocked.is_empty() {
            b.line(
                &b.t(key::SECURITY_FW_BLOCKED),
                tokens::color::TEXT_SECONDARY,
            );
            for x in scene.blocked.iter().take(BLOCKED_SHOWN) {
                let port = x.port.map_or_else(|| "-".to_owned(), |p| p.to_string());
                let label = b.texts.t_args(
                    key::SECURITY_FW_BLOCKED_ROW,
                    &[
                        ("proto", Arg::Text(&x.proto)),
                        ("port", Arg::Text(&port)),
                        ("source", Arg::Text(&x.source)),
                        ("iface", Arg::Text(&x.iface)),
                        (
                            "count",
                            Arg::Number(i64::try_from(x.count).unwrap_or(i64::MAX)),
                        ),
                    ],
                );
                let when = clock(x.at, zone, b.texts);
                // ALLOWED WHERE IT WAS REFUSED: on the networks of the
                // profile its interface has — a public one is not a home one,
                // and a home rule would leave the refusal as it was.
                let act = match (protocol_of(&x.proto), x.port, known) {
                    (Some(proto), Some(port), true) => Some(Act::Allow {
                        proto,
                        port,
                        profile: scene.profile_of_interface(&x.iface),
                        note: x.source.clone(),
                    }),
                    _ => None,
                };
                b.pair(&label, &when, act, None);
            }
        }
        // WHAT ANSWERS FROM OUTSIDE, and what the firewall does to it.
        if !scene.listening.is_empty() || !scene.published.is_empty() {
            b.line(&b.t(key::SECURITY_FW_PORTS), tokens::color::TEXT_SECONDARY);
        }
        let rows: Vec<(String, String, u16, String)> = scene
            .listening
            .iter()
            .map(|l| {
                (
                    l.program
                        .clone()
                        .unwrap_or_else(|| b.t(key::SECURITY_FW_PORT_UNKNOWN)),
                    l.proto.clone(),
                    l.port,
                    l.program.clone().unwrap_or_default(),
                )
            })
            .chain(scene.published.iter().map(|p| {
                (
                    b.texts
                        .t_with(key::SECURITY_FW_PORT_CONTAINER, &[("name", &p.name)]),
                    p.proto.clone(),
                    p.port,
                    p.name.clone(),
                )
            }))
            .collect();
        let in_use = scene.profiles_in_use();
        for (who, proto, port, note) in rows {
            let verdict = verdict_of(&settings, &in_use, &proto, port);
            // THE PORT AS TEXT: a number would be grouped by the language
            // (`8,080`), and a port is a name, not a quantity.
            let shown = port.to_string();
            let label = b.texts.t_args(
                key::SECURITY_FW_PORT_ROW,
                &[
                    ("who", Arg::Text(&who)),
                    ("proto", Arg::Text(&proto)),
                    ("port", Arg::Text(&shown)),
                ],
            );
            // ALLOW, where some network still refuses it: home first, the
            // looser step being the one a person takes on purpose.
            let act = match (
                known,
                protocol_of(&proto),
                refused_in(&settings, &in_use, &proto, port),
            ) {
                (true, Some(p), Some(profile)) if verdict != Verdict::Shielded => {
                    Some(Act::Allow {
                        proto: p,
                        port,
                        profile,
                        note,
                    })
                }
                _ => None,
            };
            let said = if known {
                b.t(verdict.key())
            } else {
                unknown.clone()
            };
            b.pair(&label, &said, act, None);
        }
    }

    fn admin(&self, b: &mut Lines<'_>, scene: &Scene, now: i64, zone: &ronoctua_files::when::Zone) {
        b.heading(Section::Admin);
        let open = scene.until.filter(|u| *u > now);
        match open {
            Some(until) => {
                b.pair(
                    &b.t(key::SECURITY_SUDO_PASSWORD),
                    &b.texts.t_with(
                        key::SECURITY_SUDO_NOT_ASKED_UNTIL,
                        &[("time", &clock(until, zone, b.texts))],
                    ),
                    None,
                    Some(tokens::color::STATE_ERROR),
                );
                b.pair(&b.t(key::SECURITY_SUDO_END), "", Some(Act::RelaxEnd), None);
            }
            None => {
                b.pair(
                    &b.t(key::SECURITY_SUDO_PASSWORD),
                    &b.t(key::SECURITY_SUDO_ASKED),
                    None,
                    None,
                );
                for minutes in RelaxMinutes::ALL {
                    b.pair(
                        &b.texts.t_args(
                            key::SECURITY_SUDO_OPEN_FOR,
                            &[("minutes", Arg::Number(i64::from(minutes.minutes())))],
                        ),
                        "",
                        Some(Act::Relax(minutes)),
                        None,
                    );
                }
            }
        }
        b.note(&b.t(key::SECURITY_SUDO_NOTE));
        if !scene.helper {
            b.line(
                &b.t(crate::security_words::refusal(Why::NoHelper)),
                tokens::color::STATE_WARNING,
            );
        }
        if let Some(work) = &self.busy {
            b.line(
                &b.t(match work {
                    Work::Enrol(_) => key::SECURITY_BUSY_ENROL,
                    _ => key::SECURITY_BUSY,
                }),
                tokens::color::TEXT_SECONDARY,
            );
        }
        if !scene.admin.is_empty() {
            b.line(
                &b.t(key::SECURITY_SUDO_RECENT),
                tokens::color::TEXT_SECONDARY,
            );
            for e in scene.admin.iter().take(ADMIN_SHOWN) {
                let label = format!("{} {}", b.t(&e.said), e.who);
                b.pair(
                    &label,
                    &when(e.at, zone, b.texts),
                    None,
                    e.warning.then_some(tokens::color::STATE_WARNING),
                );
            }
        }
    }

    fn finger(&self, b: &mut Lines<'_>, scene: &Scene) {
        b.heading(Section::Finger);
        if let Some(reader) = &scene.reader {
            b.pair(&b.t(key::SECURITY_FINGER_READER), &reader.name, None, None);
        }
        for f in &scene.fingers {
            // THE FINGERPRINT, a space of air, and the finger's name.
            b.pair(
                &format!(
                    "{} {}",
                    tokens::icon::SECURITY_FINGERPRINT,
                    b.t(crate::security_words::finger(*f))
                ),
                &b.t(key::SECURITY_FINGER_ENROLLED),
                Some(Act::Finger(*f)),
                None,
            );
        }
        if let Some(f) = self.offered(scene) {
            b.pair(
                &b.t(key::SECURITY_FINGER_ENROL),
                &b.t(crate::security_words::finger(f)),
                Some(Act::Enrol),
                None,
            );
            b.note(&b.t(key::SECURITY_FINGER_RING_NOTE));
        }
        if let Some(step) = &self.step {
            let e = ronoctua_security::fprint::Enrolling::of(step);
            b.line(
                &b.t(crate::security_words::enrolling(e)),
                tokens::color::STATE_ACCENT,
            );
        }
        b.pair(
            &b.t(key::SECURITY_FINGER_SUDO),
            &b.t(if scene.finger_sudo {
                key::SECURITY_ON
            } else {
                key::SECURITY_OFF
            }),
            Some(Act::FingerSudo),
            None,
        );
        b.note(&b.t(key::SECURITY_FINGER_SUDO_NOTE));
    }

    fn online(&self, b: &mut Lines<'_>, scene: &Scene) {
        b.heading(Section::Online);
        if self.checking {
            b.line(
                &b.t(key::ECHO_SECURITY_CHECKING),
                tokens::color::STATE_ACCENT,
            );
        }
        b.pair(
            &b.t(key::SECURITY_ONLINE_VT_KEY),
            &b.t(if scene.vt_key {
                key::SECURITY_ONLINE_KEY_KEPT
            } else {
                key::SECURITY_ONLINE_KEY_NONE
            }),
            Some(Act::VtKey),
            None,
        );
        if scene.vt_key {
            b.pair(
                &b.t(key::SECURITY_ONLINE_VT_FORGET),
                "",
                Some(Act::VtForget),
                None,
            );
        }
        b.pair(
            &b.t(key::SECURITY_ONLINE_CHECK_FILE),
            "",
            Some(Act::CheckFile),
            None,
        );
        b.note(&b.t(key::SECURITY_ONLINE_FILE_NOTE));
        for scan in scene.scans.iter().take(SCANS_SHOWN) {
            let name = std::path::Path::new(&scan.path)
                .file_name()
                .map_or_else(|| scan.path.clone(), |n| n.to_string_lossy().into_owned());
            let danger = matches!(
                &scan.state,
                ronoctua_lookup::virustotal::State::Checked { report }
                    if report.stats.verdict() != ronoctua_lookup::virustotal::Verdict::Clean
            );
            b.pair(
                &name,
                &crate::security_words::scan_state(&scan.state, b.texts),
                Some(Act::Report(scan.sha256.clone())),
                danger.then_some(tokens::color::STATE_ERROR),
            );
        }
        b.pair(
            &b.t(key::SECURITY_ONLINE_CHECK_EMAIL),
            "",
            Some(Act::CheckEmail),
            None,
        );
        b.note(&b.t(key::SECURITY_ONLINE_EMAIL_NOTE));
        if let Some((address, result)) = &self.email {
            let said = match result {
                Ok(names) if names.is_empty() => b
                    .texts
                    .t_with(key::SECURITY_ONLINE_EMAIL_CLEAN, &[("address", address)]),
                Ok(names) => b.texts.t_args(
                    key::SECURITY_ONLINE_EMAIL_FOUND,
                    &[
                        ("address", Arg::Text(address)),
                        (
                            "count",
                            Arg::Number(i64::try_from(names.len()).unwrap_or(0)),
                        ),
                        ("names", Arg::Text(&names.join(", "))),
                    ],
                ),
                Err(r) => crate::security_words::lookup_refusal(*r, b.texts),
            };
            let bad = !matches!(result, Ok(n) if n.is_empty());
            b.line(
                &said,
                if bad {
                    tokens::color::STATE_WARNING
                } else {
                    tokens::color::TEXT_SECONDARY
                },
            );
        }
        b.pair(
            &b.t(key::SECURITY_ONLINE_CHECK_PASSWORD),
            "",
            Some(Act::CheckPassword),
            None,
        );
        b.note(&b.t(key::SECURITY_ONLINE_PASSWORD_NOTE));
        if let Some(result) = &self.password {
            let (said, bad) = match result {
                Ok(0) => (b.t(key::SECURITY_ONLINE_PASSWORD_CLEAN), false),
                Ok(n) => (
                    b.texts.t_args(
                        key::SECURITY_ONLINE_PASSWORD_FOUND,
                        &[("count", Arg::Number(i64::try_from(*n).unwrap_or(i64::MAX)))],
                    ),
                    true,
                ),
                Err(r) => (crate::security_words::lookup_refusal(*r, b.texts), true),
            };
            b.line(
                &said,
                if bad {
                    tokens::color::STATE_WARNING
                } else {
                    tokens::color::TEXT_SECONDARY
                },
            );
        }
    }

    /// Settles the cursor on a row that acts — and, when somebody asked for
    /// a place (a section's word, a check that answered, a question), brings
    /// it into view: `Some(n)` is the row the door draws FIRST, and the
    /// cursor is on the place's row that acts. `None` leaves the page where
    /// it is: a scene read again never moves it.
    ///
    /// The place waits for a scene that was READ: over the empty one the
    /// sections are where they are not going to be once "Now" fills.
    pub fn settle_on(&mut self, rows: &[Row], scene: &Scene) -> Option<usize> {
        if scene.read
            && let Some(target) = self.open_at.take()
            && let Some((top, acting)) = place_of(&target, rows)
        {
            self.cursor = acting.unwrap_or(self.cursor);
            self.keep_cursor(rows);
            return Some(top);
        }
        self.keep_cursor(rows);
        None
    }

    /// The cursor on a row that acts, and nothing else moved.
    fn keep_cursor(&mut self, rows: &[Row]) {
        if rows.get(self.cursor).is_some_and(|r| !r.heading) {
            return;
        }
        self.cursor = rows.iter().position(|r| !r.heading).unwrap_or(0);
    }

    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows.get(at).is_some_and(|r| !r.heading) {
                self.cursor = at;
                return;
            }
        }
    }

    /// What a key does. THE WIDTH IS THE DOOR'S: the cursor is an index into
    /// the rows drawn.
    pub fn press(
        &mut self,
        p: Press,
        scene: &Scene,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
        zone: &ronoctua_files::when::Zone,
    ) -> Errand {
        let rows = self.rows(scene, columns, texts, zone);
        self.keep_cursor(&rows);
        let under = rows.get(self.cursor).and_then(|r| r.act.clone());
        match p {
            Press::Up => {
                self.walk(&rows, false);
                Errand::Nothing
            }
            Press::Down => {
                self.walk(&rows, true);
                Errand::Nothing
            }
            Press::Escape => {
                if self.confirm.take().is_some() {
                    self.cursor = 0;
                    return Errand::Nothing;
                }
                Errand::Close
            }
            Press::Left | Press::Right => {
                if under == Some(Act::Enrol) {
                    self.finger_at = if p == Press::Right {
                        self.finger_at.wrapping_add(1)
                    } else {
                        self.finger_at.wrapping_add(Finger::ALL.len() - 1)
                    };
                }
                Errand::Nothing
            }
            Press::Enter => match under {
                Some(act) => self.act(act, scene, texts),
                None => Errand::Nothing,
            },
            Press::Relax => relax_errand(scene),
            Press::Shields => shields_errand(scene),
            Press::RuleAdd => self.act(Act::RuleAdd, scene, texts),
            Press::RuleDelete => match under {
                Some(Act::Rule(id)) => changed(scene, texts, |settings| {
                    settings.rules.retain(|r| r.id != id);
                }),
                Some(Act::Finger(f)) => self.act(Act::Finger(f), scene, texts),
                _ => Errand::Say(texts.t(key::ECHO_SECURITY_NOTHING_TO_DELETE)),
            },
            Press::Enrol => self.act(Act::Enrol, scene, texts),
        }
    }

    /// What `RET` on a row does.
    fn act(&mut self, act: Act, scene: &Scene, texts: &ronoctua_i18n::Texts) -> Errand {
        match act {
            Act::Show(section) => {
                self.open_at = Some(Target::Section(section));
                Errand::Nothing
            }
            Act::Firewall => firewall_errand(scene, texts),
            Act::Profile(uuid) => changed(scene, texts, |settings| {
                let other = if settings.profile_of(&uuid) == Profile::Home {
                    Profile::Public
                } else {
                    Profile::Home
                };
                settings.networks.insert(uuid, other);
            }),
            Act::Home(uuid) => changed(scene, texts, |settings| {
                settings.networks.insert(uuid, Profile::Home);
            }),
            Act::Shields => shields_errand(scene),
            Act::Log => changed(scene, texts, |settings| {
                settings.log = match settings.log {
                    LogLevel::Off => LogLevel::Quiet,
                    LogLevel::Quiet => LogLevel::Chatty,
                    LogLevel::Chatty => LogLevel::Off,
                };
            }),
            Act::RuleAdd => {
                self.editing = None;
                Errand::Ask {
                    id: RULE,
                    secret: false,
                    initial: None,
                }
            }
            Act::Rule(id) => {
                let initial = scene
                    .settings()
                    .rules
                    .iter()
                    .find(|r| r.id == id)
                    .map(|r| rule_text(r, texts));
                self.editing = Some(id);
                Errand::Ask {
                    id: RULE,
                    secret: false,
                    initial,
                }
            }
            Act::Allow {
                proto,
                port,
                profile,
                note,
            } => changed(scene, texts, |settings| {
                let rule = Rule {
                    id: fresh_rule_id(settings),
                    action: Action::Allow,
                    protocol: proto,
                    ports: vec![(port, port)],
                    source: None,
                    interface: None,
                    profiles: vec![profile],
                    note: plain_note(&note),
                };
                settings.rules.push(rule);
            }),
            Act::Relax(minutes) => Errand::Door(Work::Relax(minutes), Proof::Cached),
            Act::RelaxEnd => Errand::Door(Work::RelaxEnd, Proof::Cached),
            // NO READER, NO PASSWORD ASKED: the refusal is said before the
            // door is knocked on, not after somebody typed for nothing.
            Act::Enrol if scene.reader.is_none() => {
                Errand::Say(texts.t(crate::security_words::refusal(Why::NoReader)))
            }
            Act::Enrol => match self.offered(scene) {
                Some(f) => {
                    self.step = None;
                    Errand::Door(Work::Enrol(f), Proof::Cached)
                }
                None => Errand::Nothing,
            },
            Act::Finger(f) => {
                self.confirm = Some(Confirm::RemoveFinger(f));
                self.cursor = 0;
                self.open_at = Some(Target::Top);
                Errand::Nothing
            }
            Act::FingerSudo => Errand::Door(Work::FingerSudo(!scene.finger_sudo), Proof::Cached),
            Act::VtKey => Errand::Ask {
                id: VT_KEY,
                secret: true,
                initial: None,
            },
            Act::VtForget => Errand::ForgetKey,
            Act::CheckFile => Errand::Ask {
                id: CHECK_FILE,
                secret: false,
                initial: None,
            },
            Act::CheckEmail => Errand::Ask {
                id: CHECK_EMAIL,
                secret: false,
                initial: None,
            },
            Act::CheckPassword => Errand::Ask {
                id: CHECK_PASSWORD,
                secret: true,
                initial: None,
            },
            Act::Report(sha) => Errand::Url(ronoctua_lookup::virustotal::page_of(&sha)),
            Act::Command(id) => Errand::Command(id),
            Act::Url(url) => Errand::Url(url.to_owned()),
            Act::Answer(yes) => {
                let confirm = self.confirm.take();
                self.cursor = 0;
                match (yes, confirm) {
                    (true, Some(Confirm::Upload { path, sha256 })) => {
                        Errand::Upload { path, sha256 }
                    }
                    (true, Some(Confirm::RemoveFinger(f))) => {
                        Errand::Door(Work::Delete(Fingers::One(f)), Proof::Cached)
                    }
                    _ => Errand::Nothing,
                }
            }
        }
    }

    /// A question of the page was answered under `id`.
    pub fn answered(
        &mut self,
        id: &str,
        text: &str,
        scene: &Scene,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match id {
            PASSWORD => match self.pending.take() {
                Some(work) => Errand::Door(work, Proof::Typed(Secret::new(text))),
                None => Errand::Nothing,
            },
            RULE => {
                let editing = self.editing.take();
                let id = editing
                    .clone()
                    .unwrap_or_else(|| fresh_rule_id(&scene.settings()));
                match rule_of(text, &id, texts) {
                    Some(rule) => changed(scene, texts, |settings| match &editing {
                        Some(old) => {
                            for r in &mut settings.rules {
                                if &r.id == old {
                                    // THE NOTE STAYS: the line edits what
                                    // the rule does, and the grammar has
                                    // no word for the note.
                                    let note = std::mem::take(&mut r.note);
                                    *r = Rule {
                                        note,
                                        ..rule.clone()
                                    };
                                }
                            }
                        }
                        None => settings.rules.push(rule.clone()),
                    }),
                    None => Errand::Say(texts.t(key::ECHO_SECURITY_RULE_NOT_UNDERSTOOD)),
                }
            }
            VT_KEY => Errand::KeepKey(Secret::new(text.trim())),
            CHECK_FILE => {
                let path = PathBuf::from(text.trim());
                if path.as_os_str().is_empty() {
                    Errand::Nothing
                } else {
                    Errand::Hash(path)
                }
            }
            CHECK_EMAIL => Errand::Email(text.trim().to_owned()),
            CHECK_PASSWORD => Errand::Password(Secret::new(text)),
            _ => Errand::Nothing,
        }
    }

    /// A question of the page was let go.
    pub fn unanswered(&mut self, id: &str) {
        match id {
            PASSWORD => self.pending = None,
            RULE => self.editing = None,
            _ => {}
        }
    }

    /// What a door's thread brought back: what the view keeps of it, what
    /// the door says, and what it does next.
    pub fn came(&mut self, came: Came, scene: &mut Scene, texts: &ronoctua_i18n::Texts) -> Outcome {
        match came {
            Came::Scene(fresh) => {
                *scene = *fresh;
                Outcome::quiet()
            }
            Came::Stirred => Outcome {
                read: true,
                ..Outcome::quiet()
            },
            Came::Step(word) => {
                self.step = Some(word);
                Outcome::quiet()
            }
            Came::Door { work, result } => {
                self.busy = None;
                match result {
                    Ok(()) => {
                        if matches!(work, Work::Enrol(_)) {
                            self.step = None;
                        }
                        Outcome {
                            say: Some(texts.t(work.done_key())),
                            errand: Errand::Nothing,
                            read: true,
                            show: false,
                        }
                    }
                    // THE PASSWORD, ASKED IN THIS DOOR, and the work kept for
                    // its answer.
                    Err(Why::NeedsPassword) => {
                        self.pending = Some(work);
                        Outcome {
                            say: None,
                            errand: Errand::Ask {
                                id: PASSWORD,
                                secret: true,
                                initial: None,
                            },
                            read: false,
                            show: false,
                        }
                    }
                    Err(why) => {
                        if matches!(work, Work::Enrol(_)) {
                            self.step = None;
                        }
                        Outcome {
                            say: Some(texts.t(crate::security_words::refusal(why))),
                            errand: Errand::Nothing,
                            read: true,
                            show: false,
                        }
                    }
                }
            }
            Came::Shields(result) => Outcome {
                say: Some(match result {
                    Ok(()) => texts.t(key::ECHO_SECURITY_SHIELDS_UP),
                    Err(why) => texts.t(crate::security_words::refusal(why)),
                }),
                errand: Errand::Nothing,
                read: true,
                show: false,
            },
            Came::Hashed { path, result } => {
                self.checking = false;
                self.hashed(path, result, texts)
            }
            // THE ANSWER UNDER ITS ROW, and the row first on screen: the
            // answer is the line right after the row's note.
            Came::Email { address, result } => {
                self.checking = false;
                self.email = Some((address, result));
                self.open_at = Some(Target::Row(Act::CheckEmail));
                Outcome::shown()
            }
            Came::Password(result) => {
                self.checking = false;
                self.password = Some(result);
                self.open_at = Some(Target::Row(Act::CheckPassword));
                Outcome::shown()
            }
        }
    }

    /// A file hashed and asked about: a verdict said, or — for a file never
    /// seen — the upload's question, which the page has to be seen for.
    fn hashed(
        &mut self,
        path: PathBuf,
        result: Result<(String, ronoctua_lookup::virustotal::Checked), ronoctua_lookup::Refusal>,
        texts: &ronoctua_i18n::Texts,
    ) -> Outcome {
        match result {
            Ok((sha256, ronoctua_lookup::virustotal::Checked::Unknown)) => {
                // NEVER SEEN: the upload is a question, "no" first, at the
                // top of the page.
                self.confirm = Some(Confirm::Upload { path, sha256 });
                self.cursor = 0;
                self.open_at = Some(Target::Top);
                Outcome {
                    say: None,
                    errand: Errand::Nothing,
                    read: true,
                    show: true,
                }
            }
            Ok((_, ronoctua_lookup::virustotal::Checked::Known(report))) => Outcome {
                say: Some(texts.t_with(
                    key::ECHO_SECURITY_FILE_CHECKED,
                    &[
                        ("file", &path.to_string_lossy()),
                        (
                            "verdict",
                            &crate::security_words::verdict(report.stats, texts),
                        ),
                    ],
                )),
                errand: Errand::Nothing,
                read: true,
                show: false,
            },
            Err(r) => Outcome {
                say: Some(crate::security_words::lookup_refusal(r, texts)),
                errand: Errand::Nothing,
                read: false,
                show: false,
            },
        }
    }

    /// A sudo work went to its thread.
    pub fn sent(&mut self, work: &Work) {
        self.busy = Some(work.clone());
    }

    /// An online check went to its thread: the section says so, and the page
    /// opens there, until it answers.
    pub fn check_started(&mut self) {
        self.checking = true;
        self.open_at = Some(Target::Section(Section::Online));
    }
}

/// What a door does after [`View::came`].
#[derive(Debug)]
pub struct Outcome {
    /// A sentence for the door's line.
    pub say: Option<String>,
    /// What to do next.
    pub errand: Errand,
    /// Read the scene again.
    pub read: bool,
    /// What came is drawn on the page — a question, a check's answer — so a
    /// door that is not showing it opens it (the footer, after a leader
    /// command run with the view closed).
    pub show: bool,
}

impl Outcome {
    const fn quiet() -> Self {
        Self {
            say: None,
            errand: Errand::Nothing,
            read: false,
            show: false,
        }
    }

    const fn shown() -> Self {
        Self {
            say: None,
            errand: Errand::Nothing,
            read: false,
            show: true,
        }
    }
}

/// The sections of facts, which keep no view state.
fn device(b: &mut Lines<'_>, scene: &Scene) {
    b.heading(Section::Device);
    let unknown = b.t(key::SECURITY_UNKNOWN);
    b.pair(
        &b.t(key::SECURITY_DEVICE_SECURE_BOOT),
        &scene.secure_boot.map_or_else(
            || unknown.clone(),
            |s| b.t(crate::security_words::secure_boot(s)),
        ),
        None,
        (scene.secure_boot == Some(SecureBoot::Disabled)).then_some(tokens::color::STATE_WARNING),
    );
    b.pair(
        &b.t(key::SECURITY_DEVICE_TPM),
        &scene.tpm.map_or_else(
            || unknown.clone(),
            |t| crate::security_words::tpm(t, b.texts),
        ),
        None,
        None,
    );
    b.pair(
        &b.t(key::SECURITY_DEVICE_ENCRYPTION),
        &match scene.encrypted {
            Some(true) => b.t(key::SECURITY_DEVICE_ENCRYPTED),
            Some(false) => b.t(key::SECURITY_DEVICE_NOT_ENCRYPTED),
            None => unknown.clone(),
        },
        None,
        (scene.encrypted == Some(false)).then_some(tokens::color::STATE_WARNING),
    );
    match &scene.firmware {
        Some(f) => {
            b.pair(
                &b.t(key::SECURITY_DEVICE_HSI),
                &f.hsi.clone().unwrap_or_else(|| unknown.clone()),
                None,
                None,
            );
            b.pair(
                &b.t(key::SECURITY_DEVICE_FIRMWARE_UPDATES),
                &b.texts.t_args(
                    key::SECURITY_DEVICE_FIRMWARE_UPDATES_COUNT,
                    &[("count", Arg::Number(i64::from(f.updates)))],
                ),
                None,
                None,
            );
        }
        None => b.pair(&b.t(key::SECURITY_DEVICE_HSI), &unknown, None, None),
    }
    b.note(&b.t(key::SECURITY_DEVICE_FIRMWARE_NOTE));
    b.pair(
        &b.t(key::SECURITY_DEVICE_UPDATES),
        "",
        Some(Act::Command("update-open")),
        None,
    );
    b.pair(
        &b.t(key::SECURITY_DEVICE_LOCK),
        &b.t(key::SECURITY_DEVICE_LOCK_NONE),
        None,
        None,
    );
    b.pair(
        &b.t(key::SECURITY_DEVICE_AUTOLOGIN),
        &b.t(key::SECURITY_DEVICE_AUTOLOGIN_GATE),
        None,
        None,
    );
}

fn privacy(b: &mut Lines<'_>, scene: &Scene) {
    use ronoctua_network::privacy::{Mac, Tls};
    b.heading(Section::Privacy);
    if scene.privacy.is_empty() {
        b.note(&b.t(key::SECURITY_PRIVACY_NONE));
    }
    for link in &scene.privacy {
        let mac = Mac::ALL.into_iter().find(|m| m.id() == link.mac);
        let dns = Tls::ALL.into_iter().find(|t| t.id() == link.dns);
        let unknown = b.t(key::SECURITY_UNKNOWN);
        b.pair(
            &b.texts
                .t_with(key::SECURITY_PRIVACY_MAC, &[("name", &link.name)]),
            &mac.map_or_else(|| unknown.clone(), |m| b.t(crate::security_words::mac(m))),
            None,
            (mac == Some(Mac::Permanent)).then_some(tokens::color::STATE_WARNING),
        );
        b.pair(
            &b.texts
                .t_with(key::SECURITY_PRIVACY_DNS, &[("name", &link.name)]),
            &dns.map_or(unknown, |t| b.t(crate::security_words::dns(t))),
            None,
            (dns == Some(Tls::No)).then_some(tokens::color::STATE_WARNING),
        );
    }
    b.pair(
        &b.t(key::SECURITY_PRIVACY_SSH),
        &b.t(if scene.ssh_listening() {
            key::SECURITY_PRIVACY_SSH_ON
        } else {
            key::SECURITY_OFF
        }),
        None,
        scene
            .ssh_listening()
            .then_some(tokens::color::STATE_WARNING),
    );
    b.note(&b.t(key::SECURITY_PRIVACY_NOTE));
}

fn suggest(b: &mut Lines<'_>) {
    b.heading(Section::Suggest);
    for (label, act) in [
        (key::SECURITY_SUGGEST_VPN, Act::Command("network-import")),
        (key::SECURITY_SUGGEST_TOR, Act::Command("network-tor-open")),
        (key::SECURITY_SUGGEST_TAILNET, Act::Command("network-open")),
        (
            key::SECURITY_SUGGEST_HIBP,
            Act::Url(ronoctua_lookup::breach::HIBP_PAGE),
        ),
    ] {
        b.pair(&b.t(label), "", Some(act), None);
    }
}

/// How long a refused connection counts as "lately" on the page — the watch's
/// own quarter hour.
const RECENT_S: i64 = 15 * 60;
/// The most refused connections the page lists.
const BLOCKED_SHOWN: usize = 12;
/// The most sudo events the page lists.
const ADMIN_SHOWN: usize = 12;
/// The most checked files the page lists.
const SCANS_SHOWN: usize = 8;

/// A clock, in the person's form.
fn clock(secs: i64, zone: &ronoctua_files::when::Zone, texts: &ronoctua_i18n::Texts) -> String {
    crate::chrome::clock_text(secs, zone, crate::locale::prefs::current().clock, texts)
}

/// A date and a clock.
fn when(secs: i64, zone: &ronoctua_files::when::Zone, texts: &ronoctua_i18n::Texts) -> String {
    crate::chrome::when_text(secs, zone, false, texts)
}

/// What the firewall does to a port that answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The firewall is off: it answers everybody.
    Open,
    /// A rule of the person's allows it, on every network the machine is on.
    Allowed,
    /// Allowed on some of them, or from some addresses, or through one
    /// interface — and refused otherwise.
    Partly,
    /// Refused — unless the system itself opens it.
    Refused,
    /// The shields refuse everything.
    Shielded,
}

impl Verdict {
    /// Every verdict, for the frames of every key.
    #[cfg(test)]
    pub const ALL: [Self; 5] = [
        Self::Open,
        Self::Allowed,
        Self::Partly,
        Self::Refused,
        Self::Shielded,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::Open => key::SECURITY_FW_VERDICT_OPEN,
            Self::Allowed => key::SECURITY_FW_VERDICT_ALLOWED,
            Self::Partly => key::SECURITY_FW_VERDICT_PARTLY,
            Self::Refused => key::SECURITY_FW_VERDICT_REFUSED,
            Self::Shielded => key::SECURITY_FW_VERDICT_SHIELDED,
        }
    }
}

/// What the person's table does to a new connection to `proto`/`port` on the
/// networks of these profiles — those the machine is on now, or both when it
/// is on none it knows. Pure.
#[must_use]
pub fn verdict_of(settings: &Settings, in_use: &[Profile], proto: &str, port: u16) -> Verdict {
    if settings.shields {
        return Verdict::Shielded;
    }
    if !settings.enabled {
        return Verdict::Open;
    }
    let mut each = profiles_or_both(in_use)
        .into_iter()
        .map(|p| verdict_in(settings, p, proto, port));
    let first = each.next().unwrap_or(Verdict::Refused);
    if each.all(|v| v == first) {
        first
    } else {
        Verdict::Partly
    }
}

/// The profile an "allow" for this port would open, when some network of
/// `in_use` still refuses it: home first. `None` when it is allowed on all of
/// them. Pure.
#[must_use]
pub fn refused_in(
    settings: &Settings,
    in_use: &[Profile],
    proto: &str,
    port: u16,
) -> Option<Profile> {
    if !settings.enabled {
        return None;
    }
    profiles_or_both(in_use)
        .into_iter()
        .find(|p| verdict_in(settings, *p, proto, port) != Verdict::Allowed)
}

/// The profiles to ask about: those in use, or both. Home first.
fn profiles_or_both(in_use: &[Profile]) -> Vec<Profile> {
    let mut out = if in_use.is_empty() {
        vec![Profile::Home, Profile::Public]
    } else {
        in_use.to_vec()
    };
    out.sort();
    out.dedup();
    out
}

/// What the table does in ONE profile's chain, read the way nftables reads
/// it: in order, the first rule that matches every packet decides; a rule
/// that matches only some (a source, an interface) lets those through or
/// refuses them, and the walk goes on for the rest. Pure.
fn verdict_in(settings: &Settings, profile: Profile, proto: &str, port: u16) -> Verdict {
    let mut some_allowed = false;
    let mut some_refused = false;
    let covers = |r: &Rule| {
        r.profiles.contains(&profile)
            && protocol_of(proto) == Some(r.protocol)
            && (r.ports.is_empty() || r.ports.iter().any(|&(a, b)| (a..=b).contains(&port)))
    };
    for r in settings.rules.iter().filter(|r| covers(r)) {
        let whole = r.source.is_none() && r.interface.is_none();
        match (r.action, whole) {
            (Action::Allow, true) => {
                return if some_refused {
                    Verdict::Partly
                } else {
                    Verdict::Allowed
                };
            }
            (Action::Deny, true) => {
                return if some_allowed {
                    Verdict::Partly
                } else {
                    Verdict::Refused
                };
            }
            (Action::Allow, false) => some_allowed = true,
            (Action::Deny, false) => some_refused = true,
        }
    }
    if some_allowed {
        Verdict::Partly
    } else {
        Verdict::Refused
    }
}

/// A protocol's word.
fn protocol_of(word: &str) -> Option<Protocol> {
    match word.trim().to_ascii_lowercase().as_str() {
        "tcp" | "tcp6" => Some(Protocol::Tcp),
        "udp" | "udp6" => Some(Protocol::Udp),
        _ => None,
    }
}

/// A rule id no rule of `settings` has.
fn fresh_rule_id(settings: &Settings) -> String {
    let base = format!("r-{:x}", now_s());
    let mut n = 0u32;
    loop {
        let id = if n == 0 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        if !settings.rules.iter().any(|r| r.id == id) {
            return id;
        }
        n += 1;
    }
}

/// A note as a rule may carry it: no control character, and not longer than
/// the helper takes.
fn plain_note(note: &str) -> String {
    note.chars()
        .filter(|c| !c.is_control())
        .take(ronoctua_security::firewall::NOTE_MOST)
        .collect()
}

// ─── rules as one line ──────────────────────────────────────────────────────

/// The words a typed rule is made of, in one language.
struct Words {
    allow: String,
    deny: String,
    from: String,
    on: String,
    home: String,
    public: String,
    everywhere: String,
    any: String,
}

impl Words {
    fn of(texts: &ronoctua_i18n::Texts) -> Self {
        let w = |k: &str| texts.t(k).to_lowercase();
        Self {
            allow: w(crate::security_words::action(Action::Allow)),
            deny: w(crate::security_words::action(Action::Deny)),
            from: w(key::SECURITY_RULE_WORD_FROM),
            on: w(key::SECURITY_RULE_WORD_ON),
            home: w(key::SECURITY_RULE_WORD_HOME),
            public: w(key::SECURITY_RULE_WORD_PUBLIC),
            everywhere: w(key::SECURITY_RULE_WORD_EVERYWHERE),
            any: w(key::SECURITY_RULE_WORD_ANY),
        }
    }

    /// The English words, which a line is always also read in.
    fn english() -> Self {
        ronoctua_i18n::Texts::fresh("en-GB").map_or_else(
            || Self {
                allow: "allow".into(),
                deny: "deny".into(),
                from: "from".into(),
                on: "via".into(),
                home: "home".into(),
                public: "public".into(),
                everywhere: "everywhere".into(),
                any: "any".into(),
            },
            |t| Self::of(&t),
        )
    }
}

/// A rule, as the line the question takes, in the person's language. Pure.
#[must_use]
pub fn rule_text(rule: &Rule, texts: &ronoctua_i18n::Texts) -> String {
    rule_line(rule, &Words::of(texts))
}

fn rule_line(rule: &Rule, w: &Words) -> String {
    let mut out = vec![
        match rule.action {
            Action::Allow => w.allow.clone(),
            Action::Deny => w.deny.clone(),
        },
        crate::security_words::protocol(rule.protocol).to_owned(),
    ];
    if rule.ports.is_empty() {
        out.push(w.any.clone());
    } else {
        out.push(
            rule.ports
                .iter()
                .map(|&(a, b)| {
                    if a == b {
                        a.to_string()
                    } else {
                        format!("{a}-{b}")
                    }
                })
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    if let Some(s) = &rule.source {
        out.push(w.from.clone());
        out.push(s.clone());
    }
    if let Some(i) = &rule.interface {
        out.push(w.on.clone());
        out.push(i.clone());
    }
    let mut profiles = rule.profiles.clone();
    profiles.sort();
    out.push(match profiles.as_slice() {
        [Profile::Home] => w.home.clone(),
        [Profile::Public] => w.public.clone(),
        _ => w.everywhere.clone(),
    });
    out.join(" ")
}

/// A typed line as a rule with this id — in the person's language or in
/// English — or `None` for a line that is not one. Pure.
///
/// `allow|deny [tcp|udp] <ports|any> [from <address[/prefix]>] [via
/// <interface>] [home|public|everywhere]`: TCP and home when not said.
#[must_use]
pub fn rule_of(line: &str, id: &str, texts: &ronoctua_i18n::Texts) -> Option<Rule> {
    let mine = Words::of(texts);
    let english = Words::english();
    let is = |word: &str, pick: fn(&Words) -> &String| {
        word == pick(&mine).as_str() || word == pick(&english).as_str()
    };
    let words: Vec<String> = line.split_whitespace().map(str::to_lowercase).collect();
    let mut it = words.iter().peekable();
    let action = match it.next()? {
        w if is(w, |x| &x.allow) => Action::Allow,
        w if is(w, |x| &x.deny) => Action::Deny,
        _ => return None,
    };
    let mut protocol = Protocol::Tcp;
    if let Some(p) = it.peek().and_then(|w| protocol_of(w)) {
        protocol = p;
        it.next();
    }
    let ports_word = it.next()?;
    let ports = if is(ports_word, |x| &x.any) {
        Vec::new()
    } else {
        let mut ports = Vec::new();
        for piece in ports_word.split(',') {
            let (a, b) = piece.split_once('-').unwrap_or((piece, piece));
            let (a, b): (u16, u16) = (a.parse().ok()?, b.parse().ok()?);
            if a == 0 || a > b {
                return None;
            }
            ports.push((a, b));
        }
        ports
    };
    let mut source = None;
    let mut interface = None;
    let mut profiles = vec![Profile::Home];
    while let Some(word) = it.next() {
        if is(word, |x| &x.from) {
            source = Some(ronoctua_security::firewall::source_of(it.next()?)?.0);
        } else if is(word, |x| &x.on) {
            let i = it.next()?;
            if !ronoctua_security::firewall::interface_is_plain(i) {
                return None;
            }
            interface = Some(i.clone());
        } else if is(word, |x| &x.home) {
            profiles = vec![Profile::Home];
        } else if is(word, |x| &x.public) {
            profiles = vec![Profile::Public];
        } else if is(word, |x| &x.everywhere) {
            profiles = vec![Profile::Home, Profile::Public];
        } else {
            return None;
        }
    }
    Some(Rule {
        id: id.to_owned(),
        action,
        protocol,
        ports,
        source,
        interface,
        profiles,
        note: String::new(),
    })
}

// ─── the doors' threads ─────────────────────────────────────────────────────

/// Work a door hands to a thread of its own — every one of them blocks.
#[derive(Debug)]
pub enum Job {
    /// Read the scene, all of it — the page opening.
    Read,
    /// Read again what moves in this scene ([`Scene::refreshed`]).
    Refresh(Box<Scene>),
    /// Run this through the sudo door.
    Door(Work, Proof),
    /// Raise the shields.
    Shields,
    /// Hash a file and ask about it.
    Hash(PathBuf),
    /// Ask about an address.
    Email(String),
    /// Ask about a password.
    Password(Secret),
}

/// Starts `job` on a thread; what comes back goes to `send` (the door's
/// channel). A thread that cannot start says so in the log and sends
/// nothing.
pub fn start(job: Job, send: impl Fn(Came) + Send + 'static) {
    let spawned = std::thread::Builder::new()
        .name("security".into())
        .spawn(move || match job {
            Job::Read => send(Came::Scene(Box::new(Scene::read()))),
            Job::Refresh(scene) => send(Came::Scene(Box::new(scene.refreshed()))),
            Job::Door(work, proof) => {
                let result = run_work(&work, &proof, &mut |word| send(Came::Step(word.to_owned())));
                send(Came::Door { work, result });
            }
            Job::Shields => send(Came::Shields(
                ronoctua_security::shields::raise().map_err(|r| r.why),
            )),
            Job::Hash(path) => {
                let result = hash_and_ask(&path);
                send(Came::Hashed { path, result });
            }
            Job::Email(address) => {
                let result = ask_email(&address);
                send(Came::Email { address, result });
            }
            Job::Password(secret) => send(Came::Password(ask_password(secret))),
        });
    if let Err(e) = spawned {
        tracing::warn!(error = %e, "could not start the security centre's thread");
    }
}

/// The folders whose change is a reason to read the scene again: the root
/// helper's two, the checks' `scans.json` under `state` (the person's state
/// directory, the caller's to find) and the kept keys. `stir` is called on a
/// change (coalesced); a folder that is not there is skipped.
#[must_use]
pub fn watches(
    state: Option<PathBuf>,
    stir: impl Fn() -> bool + Clone + Send + 'static,
) -> Vec<ronoctua_files::notice::Notice> {
    let parent = |p: &str| {
        std::path::Path::new(p)
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_default()
    };
    let mut dirs: Vec<PathBuf> = vec![
        parent(ronoctua_security::firewall::STATUS),
        parent(ronoctua_security::firewall::FILE),
    ];
    if let Some(state) = state {
        let scans = state.join(ronoctua_lookup::virustotal::SCANS_DIR);
        let _ = std::fs::create_dir_all(&scans);
        dirs.push(scans);
    }
    if let Some(keys) = ronoctua_lookup::keys::dir() {
        dirs.push(keys);
    }
    dirs.into_iter()
        .filter(|d| d.is_dir())
        .filter_map(|d| ronoctua_files::notice::Notice::watching(Some(&d), stir.clone()).ok())
        .collect()
}

/// Keeps VirusTotal's key, and says how it went.
#[must_use]
pub fn keep_key(secret: &Secret, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_lookup::keys::{self, Key, KeyTrouble};
    texts.t(match keys::keep(Key::VirusTotal, secret) {
        Ok(()) => key::ECHO_SECURITY_KEY_KEPT,
        Err(KeyTrouble::Empty | KeyTrouble::Malformed | KeyTrouble::TooLong) => {
            key::ECHO_SECURITY_KEY_NOT_A_KEY
        }
        Err(_) => key::ECHO_SECURITY_KEY_NOT_KEPT,
    })
}

/// Forgets it, and says how it went.
#[must_use]
pub fn forget_key(texts: &ronoctua_i18n::Texts) -> String {
    texts.t(
        match ronoctua_lookup::keys::forget(ronoctua_lookup::keys::Key::VirusTotal) {
            Ok(()) => key::ECHO_SECURITY_KEY_FORGOTTEN,
            Err(_) => key::ECHO_SECURITY_KEY_NOT_KEPT,
        },
    )
}

/// Queues a file's upload on the transfer queue, and says how it went.
#[must_use]
pub fn queue_upload(path: PathBuf, sha256: String, texts: &ronoctua_i18n::Texts) -> String {
    let name = path.to_string_lossy().into_owned();
    let work = ronoctua_transfers::Work::Scan { path, sha256 };
    match ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add { work }) {
        Ok(()) => texts.t_with(key::ECHO_SECURITY_UPLOAD_QUEUED, &[("file", &name)]),
        Err(e) => {
            tracing::warn!(error = %e, "the transfer queue did not take the upload");
            texts.t(key::TRANSFERS_OFFLINE)
        }
    }
}

/// The words that open the centre's window checking `path` — what the file
/// manager's "Check with VirusTotal" starts from either of its doors.
#[must_use]
pub fn window_checking(path: &std::path::Path) -> Vec<String> {
    vec![
        "security".to_owned(),
        Section::Online.word().to_owned(),
        "--check".to_owned(),
        path.to_string_lossy().into_owned(),
    ]
}

/// The sentence a door says when it starts a check.
#[must_use]
pub fn checking(texts: &ronoctua_i18n::Texts) -> String {
    texts.t(key::ECHO_SECURITY_CHECKING)
}

// ─── the leader's commands, with the view closed ────────────────────────────

/// `security-relax`: the window ends when it is open, and opens for
/// [`RELAX_DEFAULT`] when it is not.
#[must_use]
pub fn relax_errand(scene: &Scene) -> Errand {
    let open = scene
        .until
        .or_else(ronoctua_security::relax::open_until)
        .is_some_and(|u| u > now_s());
    Errand::Door(
        if open {
            Work::RelaxEnd
        } else {
            Work::Relax(RELAX_DEFAULT)
        },
        Proof::Cached,
    )
}

/// `security-shields`: up is free; down asks, as every loosening does. With
/// settings that cannot be read, the shields go UP: tightening is the one
/// change that writes nothing of the page's (the helper raises them over the
/// file it keeps), and whether they are up already is what cannot be known.
#[must_use]
pub fn shields_errand(scene: &Scene) -> Errand {
    match settings_now(scene) {
        Ok(mut settings) if settings.shields => {
            settings.shields = false;
            Errand::Door(Work::Firewall(Box::new(settings)), Proof::Cached)
        }
        _ => Errand::Shields,
    }
}

/// `security-firewall`: on or off, through the door.
#[must_use]
pub fn firewall_errand(scene: &Scene, texts: &ronoctua_i18n::Texts) -> Errand {
    changed(scene, texts, |settings| {
        settings.enabled = !settings.enabled
    })
}

/// `security-home`: the network that came up and was never placed — the one
/// the watch's event names — treated as home, through the door. With several
/// such networks the page opens where each has its own row; with none, it
/// says so.
#[must_use]
pub fn home_errand(scene: &Scene, texts: &ronoctua_i18n::Texts) -> Errand {
    let status = if scene.read {
        scene.status.clone()
    } else {
        ronoctua_security::firewall::read_status()
    };
    let links = if scene.read {
        scene.links.clone()
    } else {
        std::fs::read(ronoctua_security::firewall::LINKS)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    };
    let waiting = status
        .as_ref()
        .map(|s| ronoctua_security::firewall::public_unplaced(s, &links))
        .unwrap_or_default();
    match waiting.as_slice() {
        [] => Errand::Say(texts.t(key::ECHO_SECURITY_NO_NEW_NETWORK)),
        [(uuid, _)] => {
            let uuid = uuid.clone();
            changed(scene, texts, |settings| {
                settings.networks.insert(uuid, Profile::Home);
            })
        }
        _ => Errand::Show(Section::Firewall),
    }
}

/// A change of the firewall: the settings in force with `change` made, as
/// the sudo door's work — or, when the settings in force cannot be known, a
/// refusal said and NOTHING staged. The helper writes a wish whole, so a
/// wish built over defaults would erase every rule and placement of a file
/// that was only unreadable for a moment.
fn changed(
    scene: &Scene,
    texts: &ronoctua_i18n::Texts,
    change: impl FnOnce(&mut Settings),
) -> Errand {
    match settings_now(scene) {
        Ok(mut settings) => {
            change(&mut settings);
            Errand::Door(Work::Firewall(Box::new(settings)), Proof::Cached)
        }
        Err(why) => Errand::Say(texts.t(crate::security_words::refusal(why))),
    }
}

/// The settings in force, to change: the scene's while the page shows a
/// scene that was read (and kept fresh by the folders' watches), the file's
/// NOW otherwise — a leader command with the view closed. The defaults only
/// for a machine whose helper never wrote a file (`Ok(None)`); a file that is
/// there and cannot be read is a refusal, never the defaults.
///
/// # Errors
///
/// The reason the file cannot be read.
pub fn settings_now(scene: &Scene) -> Result<Settings, Why> {
    if scene.read {
        return match scene.settings_refused {
            Some(why) => Err(why),
            None => Ok(scene.settings()),
        };
    }
    ronoctua_security::firewall::read_settings()
        .map(Option::unwrap_or_default)
        .map_err(|r| r.why)
}

/// Where a place is among the drawn rows: the row the door draws first, and
/// the row that acts the cursor goes to. Pure.
fn place_of(target: &Target, rows: &[Row]) -> Option<(usize, Option<usize>)> {
    let acting_from = |n: usize| rows.iter().skip(n).position(|r| !r.heading).map(|k| k + n);
    match target {
        Target::Section(section) => {
            let first = rows.iter().position(|r| r.section == *section)?;
            // THE TITLE FIRST, and not the air above it.
            let top = if rows[first].fields.is_empty() {
                first + 1
            } else {
                first
            };
            Some((top, acting_from(top)))
        }
        Target::Row(act) => {
            let n = rows.iter().position(|r| r.act.as_ref() == Some(act))?;
            Some((n, Some(n)))
        }
        Target::Top => Some((0, acting_from(0))),
    }
}

/// A clock in the person's form and the door's zone — the footer's hint of
/// the window's end.
#[must_use]
pub fn clock_here(
    secs: i64,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    clock(secs, zone, texts)
}

/// The modeline's mark while the passwordless window is open: the unlocked
/// glyph and the END's hour — never a countdown (a clock that ticks is a
/// clock on the chrome). `None` at rest, which is nothing drawn.
///
/// Read from the helper's file every time the modeline is laid out: a tiny
/// read on `/run`, and what moves it is the brain's watch of that folder
/// (its shield comes and goes with the window), never a timer here.
#[must_use]
pub fn modeline_until(
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    let until = ronoctua_security::relax::open_until().filter(|u| *u > now_s())?;
    Some(format!(
        "{} {}",
        tokens::icon::SECURITY_SUDO_OPEN,
        clock_here(until, zone, texts)
    ))
}

/// The generation of the watch's page in a mind — what moves when the
/// connections refused lately or sudo's week change. `None` without the
/// module.
#[must_use]
pub fn watch_generation(mind: &ronoctua_brain::Mind) -> Option<u64> {
    mind.tasks
        .iter()
        .find(|t| t.id == WATCH)
        .map(|t| t.screen_generation)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn utc() -> ronoctua_files::when::Zone {
        ronoctua_files::when::Zone::utc()
    }

    fn scene() -> Scene {
        Scene {
            read: true,
            settings: Some(Settings::default()),
            helper: true,
            ..Scene::default()
        }
    }

    fn drawn(rows: &[Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    /// FAILS IF: a rule stops reading back as the line it is drawn as, in
    /// either language — or a line that is not a rule is taken for one.
    #[test]
    fn a_rule_is_one_line_both_ways_in_both_languages() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let rule = Rule {
                id: "r-1".into(),
                action: Action::Allow,
                protocol: Protocol::Udp,
                ports: vec![(8080, 8090), (22, 22)],
                source: Some("192.168.1.0/24".into()),
                interface: Some("wlan0".into()),
                profiles: vec![Profile::Home, Profile::Public],
                note: String::new(),
            };
            let line = rule_text(&rule, &t);
            assert_eq!(rule_of(&line, "r-1", &t), Some(rule), "{language}: {line}");
        }
        let t = texts("pt-BR");
        // ENGLISH IS ALWAYS READ TOO, and the defaults are TCP and home.
        let r = rule_of("allow 8080", "r-2", &t).expect("english in a pt-BR door");
        assert_eq!(
            (r.protocol, r.ports.clone(), r.profiles),
            (Protocol::Tcp, vec![(8080, 8080)], vec![Profile::Home])
        );
        for bad in [
            "",
            "open 22",
            "allow",
            "allow tcp 0",
            "allow tcp 90-80",
            "allow tcp 22 from 10.0.0.1/8",
            "allow tcp 22 via bad/name",
            "allow tcp 22 somewhere",
        ] {
            assert_eq!(rule_of(bad, "r-3", &t), None, "{bad}");
        }
    }

    /// FAILS IF: "Now" draws a line at rest, or stops listing what is off —
    /// the firewall off, the window open with its end, the shields, refused
    /// connections — in the danger colour.
    #[test]
    fn now_lists_only_what_is_off() {
        let t = texts("en-GB");
        let v = View::fresh();
        let calm = v.rows(&scene(), 100, &t, &utc());
        let now: Vec<&Row> = calm.iter().filter(|r| r.section == Section::Now).collect();
        assert!(now.iter().all(|r| r.act.is_none()), "an act at rest");
        assert!(drawn(&calm).contains(&t.t(key::SECURITY_NOW_NOTHING)));
        let mut off = scene();
        off.settings = Some(Settings {
            enabled: false,
            shields: true,
            ..Settings::default()
        });
        off.until = Some(now_s() + 600);
        off.blocked = vec![Blocked {
            proto: "tcp".into(),
            port: Some(22),
            iface: "wlan0".into(),
            source: "203.0.113.9".into(),
            at: now_s(),
            count: 3,
        }];
        let rows = v.rows(&off, 100, &t, &utc());
        let acts: Vec<Act> = rows
            .iter()
            .filter(|r| r.section == Section::Now)
            .filter_map(|r| r.act.clone())
            .collect();
        assert_eq!(
            acts,
            [
                Act::Firewall,
                Act::RelaxEnd,
                Act::Shields,
                Act::Show(Section::Firewall)
            ]
        );
        for r in rows
            .iter()
            .filter(|r| r.section == Section::Now && r.act.is_some())
        {
            assert!(
                r.fields
                    .iter()
                    .any(|f| f.color == tokens::color::STATE_ERROR)
                    || r.fields
                        .iter()
                        .any(|f| f.color == tokens::color::STATE_ACCENT),
                "an off line not in danger: {}",
                drawn(std::slice::from_ref(r))
            );
        }
    }

    /// FAILS IF: the fingerprint section shows without a reader, or the ring
    /// stops choosing among the fingers not enrolled.
    #[test]
    fn the_fingers_show_only_with_a_reader() {
        let t = texts("en-GB");
        let mut v = View::fresh();
        let none = v.rows(&scene(), 100, &t, &utc());
        assert!(!none.iter().any(|r| r.section == Section::Finger));
        let mut with = scene();
        with.reader = Some(Reader {
            name: "Synaptics".into(),
            stages: 5,
        });
        with.fingers = vec![Finger::LeftLittle];
        let rows = v.rows(&with, 100, &t, &utc());
        assert!(
            rows.iter()
                .any(|r| r.act == Some(Act::Finger(Finger::LeftLittle)))
        );
        assert_eq!(v.offered(&with), Some(Finger::LeftRing));
        v.open_on(Section::Finger);
        let rows = v.rows(&with, 100, &t, &utc());
        let _ = v.settle_on(&rows, &with);
        while rows.get(v.cursor()).and_then(|r| r.act.clone()) != Some(Act::Enrol) {
            v.walk(&rows, true);
        }
        let _ = v.press(Press::Right, &with, 100, &t, &utc());
        assert_eq!(v.offered(&with), Some(Finger::LeftMiddle));
        match v.press(Press::Enter, &with, 100, &t, &utc()) {
            Errand::Door(Work::Enrol(Finger::LeftMiddle), Proof::Cached) => {}
            other => panic!("enrol did {other:?}"),
        }
    }

    /// FAILS IF: a change of the firewall stops being the WHOLE settings
    /// through the door, or the shields' way up stops being free.
    #[test]
    fn loosening_goes_through_the_door_and_the_shields_go_up_free() {
        let s = scene();
        match firewall_errand(&s, &texts("en-GB")) {
            Errand::Door(Work::Firewall(settings), Proof::Cached) => {
                assert!(!settings.enabled);
                assert_eq!(settings.rules, s.settings().rules);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(shields_errand(&s), Errand::Shields));
        let mut up = scene();
        up.settings = Some(Settings {
            shields: true,
            ..Settings::default()
        });
        match shields_errand(&up) {
            Errand::Door(Work::Firewall(settings), _) => assert!(!settings.shields),
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: the password question stops being asked in the door that
    /// needed it, or its answer stops running the work that waited for it.
    #[test]
    fn a_needed_password_is_asked_and_the_work_waits_for_it() {
        let t = texts("en-GB");
        let mut v = View::fresh();
        let mut s = scene();
        let out = v.came(
            Came::Door {
                work: Work::Relax(RelaxMinutes::Thirty),
                result: Err(Why::NeedsPassword),
            },
            &mut s,
            &t,
        );
        assert!(matches!(
            out.errand,
            Errand::Ask {
                id: PASSWORD,
                secret: true,
                ..
            }
        ));
        match v.answered(PASSWORD, "hunter2", &s, &t) {
            Errand::Door(Work::Relax(RelaxMinutes::Thirty), Proof::Typed(p)) => {
                assert_eq!(p.expose(), "hunter2");
            }
            other => panic!("{other:?}"),
        }
        // A WRONG ONE is a sentence, never silence.
        let out = v.came(
            Came::Door {
                work: Work::RelaxEnd,
                result: Err(Why::WrongPassword),
            },
            &mut s,
            &t,
        );
        assert_eq!(out.say, Some(t.t(key::SECURITY_REFUSAL_WRONG_PASSWORD)));
    }

    /// FAILS IF: a file VirusTotal never saw is sent without a question, the
    /// question stops drawing "no" first with the cursor on it and the
    /// sharing warning, or `Esc` sends it.
    #[test]
    fn an_upload_is_a_question_with_no_first() {
        let t = texts("en-GB");
        let mut v = View::fresh();
        let mut s = scene();
        let out = v.came(
            Came::Hashed {
                path: "/home/p/a.exe".into(),
                result: Ok((
                    "a".repeat(64),
                    ronoctua_lookup::virustotal::Checked::Unknown,
                )),
            },
            &mut s,
            &t,
        );
        assert!(matches!(out.errand, Errand::Nothing));
        let rows = v.rows(&s, 100, &t, &utc());
        assert_eq!(v.settle_on(&rows, &s), Some(0), "the question on top");
        assert_eq!(rows[v.cursor()].act, Some(Act::Answer(false)), "no first");
        // THE WARNING, WRAPPED and never cut: its lines, read back as one.
        let sentence = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.trim()))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            sentence.contains(&t.t(key::SECURITY_VT_UPLOAD_WARNING)),
            "{sentence}"
        );
        assert!(matches!(
            v.press(Press::Enter, &s, 100, &t, &utc()),
            Errand::Nothing
        ));
        // YES, and only yes, queues it.
        let _ = v.came(
            Came::Hashed {
                path: "/home/p/a.exe".into(),
                result: Ok((
                    "a".repeat(64),
                    ronoctua_lookup::virustotal::Checked::Unknown,
                )),
            },
            &mut s,
            &t,
        );
        let rows = v.rows(&s, 100, &t, &utc());
        let _ = v.settle_on(&rows, &s);
        let _ = v.press(Press::Down, &s, 100, &t, &utc());
        match v.press(Press::Enter, &s, 100, &t, &utc()) {
            Errand::Upload { sha256, .. } => assert_eq!(sha256, "a".repeat(64)),
            other => panic!("{other:?}"),
        }
        let _ = v.came(
            Came::Hashed {
                path: "/x".into(),
                result: Ok((
                    "b".repeat(64),
                    ronoctua_lookup::virustotal::Checked::Unknown,
                )),
            },
            &mut s,
            &t,
        );
        assert!(matches!(
            v.press(Press::Escape, &s, 100, &t, &utc()),
            Errand::Nothing
        ));
        assert!(v.confirm.is_none(), "Esc let the question go");
    }

    /// FAILS IF: every section stops being drawn in both languages without a
    /// missing key — the served network, the suggestions at the end.
    #[test]
    fn every_section_is_drawn_in_both_languages() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut s = scene();
            s.status = Some(Status {
                format: 1,
                applied_s: 1,
                ok: true,
                why: None,
                profiles: [("wlan0".to_owned(), Profile::Public)].into(),
                unplaced: vec!["11111111-2222-3333-4444-555555555555".into()],
                served: vec!["wlan1".into()],
            });
            s.links = [(
                "wlan0".to_owned(),
                "11111111-2222-3333-4444-555555555555".to_owned(),
            )]
            .into();
            s.reader = Some(Reader {
                name: "Goodix".into(),
                stages: 5,
            });
            s.listening = vec![Listener {
                proto: "tcp".into(),
                port: 22,
                program: Some("sshd".into()),
            }];
            s.privacy = vec![Link {
                name: "home".into(),
                mac: "random".into(),
                dns: "no".into(),
            }];
            let rows = View::fresh().rows(&s, 100, &t, &utc());
            let text = drawn(&rows);
            assert!(!text.contains('['), "{language}: {text}");
            for section in Section::ALL {
                assert!(
                    rows.iter().any(|r| r.section == section),
                    "{language}: {section:?} is not drawn"
                );
            }
            assert!(text.contains(&t.t(key::SECURITY_FW_SERVED)));
            // EVERY VERDICT HAS ITS WORDS in this language.
            for v in Verdict::ALL {
                assert!(!t.t(v.key()).starts_with('['), "{language}: {v:?}");
            }
            assert_eq!(
                rows.last().map(|r| r.section),
                Some(Section::Suggest),
                "the suggestions come last"
            );
        }
    }

    /// FAILS IF: the watch's rows stop being read into the page's two lists
    /// by the ids it writes.
    #[test]
    fn the_watchs_page_is_read_by_its_row_ids() {
        use ronoctua_brain::module::{Cell, Row as R, Screen, Section as S, Tone};
        let screen = Screen {
            sections: vec![S {
                heading: "x".into(),
                rows: vec![
                    R {
                        id: "blocked:tcp:22:wlan0:2001:db8::1".into(),
                        value: Cell::Moment(100),
                        note: Cell::Count(3),
                        ..R::default()
                    },
                    R {
                        id: "sudo:0".into(),
                        label: Cell::Key("k-failed".into()),
                        value: Cell::Moment(90),
                        note: Cell::Data("noctua ls".into()),
                        tone: Tone::Warning,
                        ..R::default()
                    },
                ],
            }],
        };
        let (blocked, admin) = lists_of(&screen);
        assert_eq!(blocked.len(), 1);
        assert_eq!(blocked[0].source, "2001:db8::1");
        assert_eq!((blocked[0].port, blocked[0].count), (Some(22), 3));
        assert_eq!(admin.len(), 1);
        assert_eq!(
            (admin[0].said.as_str(), admin[0].warning),
            ("k-failed", true)
        );
    }

    /// FAILS IF: the watch's shield stops opening the section it is about, or
    /// another module's mark is taken for it.
    #[test]
    fn the_watchs_shield_opens_its_section() {
        let segment = |mark: &str| format!("brain:{WATCH}/{mark}");
        assert_eq!(
            section_of_mark(WATCH, &segment("sudo-open")),
            Some(Section::Admin)
        );
        for mark in ["firewall-failed", "firewall-off", "shields", "refused"] {
            assert_eq!(
                section_of_mark(WATCH, &segment(mark)),
                Some(Section::Firewall),
                "{mark}"
            );
        }
        assert_eq!(section_of_mark("fixture", "brain:fixture/full"), None);
    }

    fn allow(ports: (u16, u16), profiles: &[Profile]) -> Rule {
        Rule {
            id: "r-1".into(),
            action: Action::Allow,
            protocol: Protocol::Tcp,
            ports: vec![ports],
            source: None,
            interface: None,
            profiles: profiles.to_vec(),
            note: String::new(),
        }
    }

    /// FAILS IF: a port's verdict stops following the person's table — its
    /// profiles, its order, and a rule that lets in only some sources.
    #[test]
    fn a_ports_verdict_follows_the_table() {
        use Profile::{Home, Public};
        let mut s = Settings::default();
        assert_eq!(verdict_of(&s, &[Home], "tcp", 22), Verdict::Refused);
        s.rules.push(allow((20, 25), &[Home]));
        assert_eq!(verdict_of(&s, &[Home], "tcp", 22), Verdict::Allowed);
        assert_eq!(verdict_of(&s, &[Home], "udp", 22), Verdict::Refused);
        // A HOME RULE ON A PUBLIC NETWORK lets nothing in.
        assert_eq!(verdict_of(&s, &[Public], "tcp", 22), Verdict::Refused);
        assert_eq!(verdict_of(&s, &[Home, Public], "tcp", 22), Verdict::Partly);
        assert_eq!(verdict_of(&s, &[], "tcp", 22), Verdict::Partly);
        assert_eq!(refused_in(&s, &[Home, Public], "tcp", 22), Some(Public));
        assert_eq!(refused_in(&s, &[Home], "tcp", 22), None);
        // ONLY FROM ONE NETWORK is not "allowed".
        let mut from = Settings::default();
        from.rules.push(Rule {
            source: Some("192.168.1.0/24".into()),
            ..allow((22, 22), &[Home])
        });
        assert_eq!(verdict_of(&from, &[Home], "tcp", 22), Verdict::Partly);
        assert_eq!(refused_in(&from, &[Home], "tcp", 22), Some(Home));
        // THE FIRST RULE THAT TAKES EVERY PACKET decides.
        let mut first = Settings::default();
        first.rules.push(Rule {
            action: Action::Deny,
            ..allow((22, 22), &[Home])
        });
        first.rules.push(allow((22, 22), &[Home]));
        assert_eq!(verdict_of(&first, &[Home], "tcp", 22), Verdict::Refused);
        s.enabled = false;
        assert_eq!(verdict_of(&s, &[Public], "tcp", 80), Verdict::Open);
        s.shields = true;
        assert_eq!(verdict_of(&s, &[Home], "tcp", 22), Verdict::Shielded);
    }

    /// FAILS IF: "allow" on a connection refused on a public network makes a
    /// rule for home networks — which changes nothing in the table while the
    /// page says the port is allowed.
    #[test]
    fn allow_opens_where_it_was_refused() {
        let t = texts("en-GB");
        let mut s = scene();
        s.status = Some(Status {
            format: 1,
            applied_s: 1,
            ok: true,
            why: None,
            profiles: [("wlan0".to_owned(), Profile::Public)].into(),
            unplaced: Vec::new(),
            served: Vec::new(),
        });
        s.blocked = vec![Blocked {
            proto: "tcp".into(),
            port: Some(22),
            iface: "wlan0".into(),
            source: "203.0.113.9".into(),
            at: now_s(),
            count: 7,
        }];
        s.listening = vec![Listener {
            proto: "tcp".into(),
            port: 22,
            program: Some("sshd".into()),
        }];
        let mut v = View::fresh();
        let rows = v.rows(&s, 100, &t, &utc());
        let n = rows
            .iter()
            .position(|r| matches!(r.act, Some(Act::Allow { .. })))
            .expect("an allow row");
        assert!(matches!(
            rows[n].act,
            Some(Act::Allow {
                profile: Profile::Public,
                ..
            })
        ));
        assert_eq!(
            View::about(&rows, n, &t),
            Some(t.t(key::SECURITY_ABOUT_ALLOW_PUBLIC))
        );
        let _ = v.clicked(n, &rows);
        let Errand::Door(Work::Firewall(after), _) = v.press(Press::Enter, &s, 100, &t, &utc())
        else {
            panic!("allow did not go through the door");
        };
        assert_eq!(
            after.rules.last().map(|r| r.profiles.clone()),
            Some(vec![Profile::Public])
        );
        assert_eq!(
            verdict_of(&after, &s.profiles_in_use(), "tcp", 22),
            Verdict::Allowed
        );
    }

    /// FAILS IF: a firewall change is built over the defaults while the
    /// settings file cannot be read — the helper writes a wish whole, and
    /// that would erase every rule and placement of the person's.
    #[test]
    fn unreadable_settings_are_never_written_over() {
        let t = texts("en-GB");
        let mut s = scene();
        s.settings = None;
        s.settings_refused = Some(Why::Invalid);
        s.status = Some(Status {
            format: 1,
            applied_s: 1,
            ok: true,
            why: None,
            profiles: [("wlan0".to_owned(), Profile::Public)].into(),
            unplaced: vec!["11111111-2222-3333-4444-555555555555".into()],
            served: Vec::new(),
        });
        s.links = [(
            "wlan0".to_owned(),
            "11111111-2222-3333-4444-555555555555".to_owned(),
        )]
        .into();
        s.blocked = vec![Blocked {
            proto: "tcp".into(),
            port: Some(22),
            iface: "wlan0".into(),
            source: "203.0.113.9".into(),
            at: now_s(),
            count: 1,
        }];
        let is_write = |e: &Errand| matches!(e, Errand::Door(Work::Firewall(_), _));
        let mut v = View::fresh();
        for act in [
            Act::Firewall,
            Act::Log,
            Act::Profile("11111111-2222-3333-4444-555555555555".into()),
            Act::Home("11111111-2222-3333-4444-555555555555".into()),
            Act::Allow {
                proto: Protocol::Tcp,
                port: 22,
                profile: Profile::Home,
                note: String::new(),
            },
        ] {
            let e = v.act(act.clone(), &s, &t);
            assert!(!is_write(&e), "{act:?} wrote over: {e:?}");
            assert!(matches!(e, Errand::Say(_)), "{act:?} said nothing");
        }
        assert!(!is_write(&firewall_errand(&s, &t)));
        assert!(!is_write(&home_errand(&s, &t)));
        assert!(!is_write(&v.answered(RULE, "allow tcp 8080", &s, &t)));
        // THE SHIELDS STILL GO UP — the helper raises them over its own file.
        assert!(matches!(shields_errand(&s), Errand::Shields));
        // AND THE PAGE OFFERS NOTHING THAT WOULD WRITE.
        let rows = v.rows(&s, 100, &t, &utc());
        for r in &rows {
            assert!(
                !matches!(
                    r.act,
                    Some(
                        Act::Firewall
                            | Act::Log
                            | Act::RuleAdd
                            | Act::Rule(_)
                            | Act::Allow { .. }
                            | Act::Profile(_)
                            | Act::Home(_)
                    )
                ),
                "{:?} offered over unread settings",
                r.act
            );
        }
    }

    /// FAILS IF: the place somebody asked for is not brought into view — a
    /// section's title first, a section of facts only included, a check's
    /// answer under its row — or a scene read again moves the page.
    #[test]
    fn the_place_asked_for_is_brought_into_view() {
        let t = texts("en-GB");
        let s = scene();
        for section in [
            Section::Privacy,
            Section::Device,
            Section::Admin,
            Section::Suggest,
        ] {
            let mut v = View::at(Some(section));
            let rows = v.rows(&s, 100, &t, &utc());
            // OVER THE EMPTY SCENE, nothing: the sections are not where they
            // will be.
            assert_eq!(v.settle_on(&rows, &Scene::default()), None);
            let top = v.settle_on(&rows, &s).expect("brought into view");
            assert_eq!(rows[top].section, section, "{section:?}");
            assert_eq!(
                drawn(std::slice::from_ref(&rows[top])),
                t.t(section.heading()),
                "{section:?}: its title first"
            );
            assert!(v.cursor() >= top && !rows[v.cursor()].heading);
            // ONCE: a scene read again leaves the page alone.
            assert_eq!(v.settle_on(&rows, &s), None);
        }
        let mut v = View::fresh();
        let mut s = scene();
        let _ = v.came(
            Came::Email {
                address: "a@example.org".into(),
                result: Ok(vec!["Adobe".into()]),
            },
            &mut s,
            &t,
        );
        let rows = v.rows(&s, 100, &t, &utc());
        let top = v.settle_on(&rows, &s).expect("the answer in view");
        assert_eq!(rows[top].act, Some(Act::CheckEmail));
        assert_eq!(v.cursor(), top);
        let answer = t.t_args(
            key::SECURITY_ONLINE_EMAIL_FOUND,
            &[
                ("address", Arg::Text("a@example.org")),
                ("count", Arg::Number(1)),
                ("names", Arg::Text("Adobe")),
            ],
        );
        let near = drawn(&rows[top..(top + 5).min(rows.len())]);
        assert!(near.contains(&answer), "{near}");
    }

    /// FAILS IF: without a reader the finger keys are taught, or enrolling
    /// knocks on the sudo door for a reader that is not there.
    #[test]
    fn no_reader_no_finger_keys() {
        let t = texts("en-GB");
        let s = scene();
        let taught = |scene: &Scene| {
            View::hint(scene, &t)
                .iter()
                .map(|p| p.text.clone())
                .collect::<String>()
        };
        assert!(!taught(&s).contains(&t.t("security-enrol")));
        let mut v = View::fresh();
        match v.press(Press::Enrol, &s, 100, &t, &utc()) {
            Errand::Say(said) => {
                assert_eq!(said, t.t(crate::security_words::refusal(Why::NoReader)));
            }
            other => panic!("{other:?}"),
        }
        let mut with = scene();
        with.reader = Some(Reader {
            name: "Goodix".into(),
            stages: 5,
        });
        assert!(taught(&with).contains(&t.t("security-enrol")));
    }

    /// FAILS IF: editing a rule throws its note away.
    #[test]
    fn an_edited_rule_keeps_its_note() {
        let t = texts("en-GB");
        let mut s = scene();
        s.settings = Some(Settings {
            rules: vec![Rule {
                note: "the web page I test".into(),
                ..allow((8080, 8080), &[Profile::Home])
            }],
            ..Settings::default()
        });
        let mut v = View::fresh();
        let _ = v.act(Act::Rule("r-1".into()), &s, &t);
        match v.answered(RULE, "allow tcp 8081 home", &s, &t) {
            Errand::Door(Work::Firewall(after), _) => {
                assert_eq!(after.rules.len(), 1);
                assert_eq!(after.rules[0].ports, vec![(8081, 8081)]);
                assert_eq!(after.rules[0].note, "the web page I test");
            }
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: "treat as home" from the watch's event stops naming the one
    /// network it is about, or picks one of several by itself.
    #[test]
    fn treat_as_home_is_the_one_new_public_network() {
        let t = texts("en-GB");
        let uuid = |n: u8| format!("{n}1111111-2222-3333-4444-555555555555");
        let status = |profiles: &[(&str, Profile)], unplaced: Vec<String>| Status {
            format: 1,
            applied_s: 1,
            ok: true,
            why: None,
            profiles: profiles
                .iter()
                .map(|(i, p)| ((*i).to_owned(), *p))
                .collect(),
            unplaced,
            served: Vec::new(),
        };
        let mut s = scene();
        s.links = [
            ("wlan0".to_owned(), uuid(1)),
            ("enp3s0".to_owned(), uuid(2)),
        ]
        .into();
        s.status = Some(status(&[("enp3s0", Profile::Home)], Vec::new()));
        assert!(matches!(home_errand(&s, &t), Errand::Say(_)));
        s.status = Some(status(
            &[("wlan0", Profile::Public), ("enp3s0", Profile::Home)],
            vec![uuid(1)],
        ));
        match home_errand(&s, &t) {
            Errand::Door(Work::Firewall(after), _) => {
                assert_eq!(after.networks.get(&uuid(1)), Some(&Profile::Home));
            }
            other => panic!("{other:?}"),
        }
        s.status = Some(status(
            &[("wlan0", Profile::Public), ("enp3s0", Profile::Public)],
            vec![uuid(1), uuid(2)],
        ));
        assert!(matches!(
            home_errand(&s, &t),
            Errand::Show(Section::Firewall)
        ));
    }

    /// FAILS IF: a password question let go in the footer leaves its work
    /// armed for a later answer typed by name.
    #[test]
    fn a_question_let_go_takes_its_work_with_it() {
        let t = texts("en-GB");
        let mut v = View::fresh();
        let mut s = scene();
        let _ = v.came(
            Came::Door {
                work: Work::RelaxEnd,
                result: Err(Why::NeedsPassword),
            },
            &mut s,
            &t,
        );
        v.let_go();
        assert!(matches!(
            v.answered(PASSWORD, "hunter2", &s, &t),
            Errand::Nothing
        ));
    }

    /// FAILS IF: a command a key or a row runs is not in the registry.
    #[test]
    fn every_command_of_the_view_is_registered() {
        let (_, registry) = crate::leader::default_of().expect("the keymap builds");
        for id in reached() {
            assert!(registry.search(id).is_some(), "`{id}` is not registered");
        }
        for (_, id) in OF_THE_VIEW {
            assert!(press_of(id).is_some(), "`{id}` is no press");
        }
        for section in Section::ALL {
            assert_eq!(Section::of_word(section.word()), Some(section));
        }
    }
}
