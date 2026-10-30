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
//!    without it (5, 15, 30 or 60 minutes, 24 or 48 hours, or until it is
//!    ended), its end and a live count of the time left — a clock armed only
//!    while the page is drawn — "start the count again", and a week of sudo.
//! 4. **Fingerprint** — only with a reader: the fingers enrolled, enrolling
//!    one (touch by touch), removing one, and fingers for sudo.
//! 5. **Security keys** — the person's keys that answer sudo here (root's
//!    list, `ronoctua_security::u2f`), registering one by a touch, removing
//!    one, turning them off and on, and bringing the ones registered on the
//!    person's other machines.
//! 6. **Device** — Secure Boot, the TPM, disk encryption, the firmware's
//!    security level and updates (read only), automatic updates, and two
//!    honest lines: there is no screen lock, and with autologin the boot
//!    passphrase is the only gate.
//! 7. **Network privacy** — per connection that is up: how the hardware
//!    address shows, whether DNS is encrypted; and the SSH server.
//! 8. **Online checks** — the VirusTotal key, a file, an e-mail address, a
//!    password; each row says what leaves the machine and where to.
//! 9. **Suggestions**, at the end — a VPN, Tor, the tailnet, Have I Been
//!    Pwned's own page.
//!
//! And, between them, **Encrypt** (a file, a folder, a `.age` to open —
//! chosen in the file picker hosted over the page, `crate::seal`) and
//! **Post-quantum** (every channel and what it does against a quantum
//! computer, `ronoctua_security::quantum`).
//!
//! # Tightening is free, loosening asks
//!
//! Raising the shields is the person's with no password (one narrow polkit
//! rule, `ronoctua_security::shields`); every other change of the firewall,
//! the passwordless window, the fingers and the keys go through the sudo
//! door, asked in the footer's SUDO BAR (`ronoctua_shell_footer::sudo_bar`) from either
//! door: `sudo -n` first, and the bar only when that is refused — the
//! password, a touch or a finger. A change of the firewall is a WHOLE
//! `Settings` staged right before the door runs, never a patch — the helper
//! validates the whole of it and writes it whole.
//!
//! # Both doors, one view
//!
//! What a key or a click does is decided here ([`View::press`]) and handed
//! back as an [`Errand`]; each door carries it out (a thread, a question on
//! its own line, the queue, a root gesture through its gate) and hands what
//! came back to [`View::came`]. The footer and the window therefore ask the
//! same questions, type the same answers and say the same sentences — and
//! root is asked for in one place, the sudo bar, whichever door pressed.
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

use crate::key;
use ronoctua_i18n::Arg;
use ronoctua_lookup::keys::Secret;
use ronoctua_security::Why;
use ronoctua_security::firewall::{Action, LogLevel, Profile, Protocol, Rule, Settings, Status};
use ronoctua_security::fprint::{Finger, Reader};
use ronoctua_security::platform::{SecureBoot, Tpm};
use ronoctua_security::relax::Opened;
use ronoctua_security::verb::{Fingers, RelaxSpan, RootVerb};
use ronoctua_tokens as tokens;
use serde::{Deserialize, Serialize};

use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::security_words::{VT_KEY, WATCH};

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
    ("a", "security-rule-add"),
    ("d", "security-rule-delete"),
    ("C-e", "security-enrol"),
];

/// The question a rule is typed into.
pub const RULE: &str = "security-rule";
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
pub const RELAX_DEFAULT: RelaxSpan = RelaxSpan::Fifteen;

/// Every command this view's keys, rows and questions reach — for
/// `bindings::reachable`.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW.iter().map(|(_, id)| *id).chain([
        "security-open",
        "security-window",
        "security-firewall",
        RULE,
        VT_KEY,
        CHECK_PASSWORD,
        CHECK_EMAIL,
        CHECK_FILE,
        "security-vt-forget",
        HOME,
        // THE VAULT'S: its leader commands, its row and its four questions.
        "private-open",
        "private-lock",
        "private-show",
        crate::vault::PASSPHRASE,
        crate::vault::NEW_PASSPHRASE,
        crate::vault::REPEAT_PASSPHRASE,
        crate::vault::CAP,
    ])
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "security",
    commands: &[
        // THE SECURITY CENTRE (`crate::security`): the view's two doors, the
        // leader's toggles and checks, the view's keys, and the questions its rows
        // ask — each an arm of `run_with`.
        ronoctua_cmd::Command {
            id: "security-open",
            label: crate::key::CMD_SECURITY_OPEN,
        },
        ronoctua_cmd::Command {
            id: "security-window",
            label: crate::key::CMD_SECURITY_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "security-relax",
            label: crate::key::CMD_SECURITY_RELAX,
        },
        ronoctua_cmd::Command {
            id: "security-shields",
            label: crate::key::CMD_SECURITY_SHIELDS,
        },
        ronoctua_cmd::Command {
            id: "security-firewall",
            label: crate::key::CMD_SECURITY_FIREWALL,
        },
        ronoctua_cmd::Command {
            id: "security-check-password",
            label: crate::key::CMD_SECURITY_CHECK_PASSWORD,
        },
        ronoctua_cmd::Command {
            id: "security-check-email",
            label: crate::key::CMD_SECURITY_CHECK_EMAIL,
        },
        ronoctua_cmd::Command {
            id: "security-check-file",
            label: crate::key::CMD_SECURITY_CHECK_FILE,
        },
        ronoctua_cmd::Command {
            id: "security-down",
            label: crate::key::CMD_SECURITY_DOWN,
        },
        ronoctua_cmd::Command {
            id: "security-up",
            label: crate::key::CMD_SECURITY_UP,
        },
        ronoctua_cmd::Command {
            id: "security-enter",
            label: crate::key::CMD_SECURITY_ENTER,
        },
        ronoctua_cmd::Command {
            id: "security-left",
            label: crate::key::CMD_SECURITY_LEFT,
        },
        ronoctua_cmd::Command {
            id: "security-right",
            label: crate::key::CMD_SECURITY_RIGHT,
        },
        ronoctua_cmd::Command {
            id: "security-rule-add",
            label: crate::key::CMD_SECURITY_RULE_ADD,
        },
        ronoctua_cmd::Command {
            id: "security-rule-delete",
            label: crate::key::CMD_SECURITY_RULE_DELETE,
        },
        ronoctua_cmd::Command {
            id: "security-enrol",
            label: crate::key::CMD_SECURITY_ENROL,
        },
        ronoctua_cmd::Command {
            id: "security-rule",
            label: crate::key::CMD_SECURITY_RULE,
        },
        ronoctua_cmd::Command {
            id: "security-vt-key",
            label: crate::key::CMD_SECURITY_VT_KEY,
        },
        ronoctua_cmd::Command {
            id: "security-vt-forget",
            label: crate::key::CMD_SECURITY_VT_FORGET,
        },
        ronoctua_cmd::Command {
            id: "security-home",
            label: crate::key::CMD_SECURITY_HOME,
        },
        ronoctua_cmd::Command {
            id: "private-open",
            label: crate::key::CMD_PRIVATE_OPEN,
        },
        ronoctua_cmd::Command {
            id: "private-lock",
            label: crate::key::CMD_PRIVATE_LOCK,
        },
        // THE VAULT'S ROW THAT SHOWS IT, and its four questions
        // (`crate::vault`).
        ronoctua_cmd::Command {
            id: "private-show",
            label: crate::key::CMD_PRIVATE_SHOW,
        },
        ronoctua_cmd::Command {
            id: "private-passphrase",
            label: crate::key::CMD_PRIVATE_PASSPHRASE,
        },
        ronoctua_cmd::Command {
            id: "private-new-passphrase",
            label: crate::key::CMD_PRIVATE_NEW_PASSPHRASE,
        },
        ronoctua_cmd::Command {
            id: "private-repeat-passphrase",
            label: crate::key::CMD_PRIVATE_REPEAT_PASSPHRASE,
        },
        ronoctua_cmd::Command {
            id: "private-cap",
            label: crate::key::CMD_PRIVATE_CAP,
        },
        ronoctua_cmd::Command {
            id: "security-encrypt",
            label: crate::key::CMD_SECURITY_ENCRYPT,
        },
        ronoctua_cmd::Command {
            id: "security-quantum",
            label: crate::key::CMD_SECURITY_QUANTUM,
        },
    ],
    bindings: &[
        // `s e` — THE SECURITY CENTRE: the doubled key the footer's door, `w`
        // the window's; then the window without a password, the shields, the
        // firewall, and the three online checks.
        ("s e e", "security-open", None),
        ("s e w", "security-window", None),
        ("s e r", "security-relax", None),
        ("s e b", "security-shields", None),
        ("s e f", "security-firewall", None),
        ("s e p", "security-check-password", None),
        ("s e m", "security-check-email", None),
        ("s e v", "security-check-file", None),
        ("s e o", "private-open", None),
        ("s e l", "private-lock", None),
        ("s e c", "security-encrypt", None),
        ("s e q", "security-quantum", None),
    ],
    groups: &[("s e", crate::key::GROUP_SECURITY)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::security::reached().collect()
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
    /// The `/private` vault (the private-volume package).
    Private,
    /// Encrypt anything (the encrypt-and-quantum package).
    Encrypt,
    /// What each channel does against a quantum computer (the
    /// encrypt-and-quantum package).
    Quantum,
    // ── anchor: L12 ──
    // ── anchor: L22 ──
    /// The security keys that answer sudo (the sudo bar's package).
    Keys,
}

impl Section {
    /// All, in the page's order.
    pub const ALL: [Self; 12] = [
        Self::Now,
        Self::Firewall,
        Self::Admin,
        Self::Finger,
        Self::Keys,
        Self::Private,
        Self::Encrypt,
        Self::Device,
        Self::Quantum,
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
            Self::Admin => crate::security_words::ADMIN_SECTION,
            Self::Finger => "finger",
            Self::Device => "device",
            Self::Privacy => "privacy",
            Self::Online => "online",
            Self::Suggest => "suggest",
            Self::Private => "private",
            Self::Encrypt => "encrypt",
            Self::Quantum => "quantum",
            Self::Keys => "keys",
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
            Self::Private => key::SECURITY_HEADING_PRIVATE,
            Self::Encrypt => key::SECURITY_HEADING_ENCRYPT,
            Self::Quantum => key::SECURITY_HEADING_QUANTUM,
            Self::Keys => key::SECURITY_HEADING_KEYS,
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
    /// The passwordless window open: its end (or no limit) and its span.
    pub window: Option<Opened>,
    /// The session was given the root helper.
    pub helper: bool,
    /// The fingerprint reader; `None` hides the section.
    pub reader: Option<Reader>,
    /// The enrolled fingers.
    pub fingers: Vec<Finger>,
    /// Fingers answer sudo in terminals.
    pub finger_sudo: bool,
    /// A security key may answer sudo here: `pam_u2f` is in sudo's stack,
    /// reading root's list (`u2f::answers_sudo`).
    pub keys_answer: bool,
    /// The person's keys in root's list — what answers sudo here, in order.
    pub keys: Vec<String>,
    /// Root's list is there and cannot be read.
    pub keys_unreadable: bool,
    /// The person's keys in their own list (`~/.config/Yubico/u2f_keys`,
    /// which travels with the belongings).
    pub person_keys: Vec<String>,
    /// The keys of the person's own list that root's list lacks — the ones
    /// registered on their other machines (`offers::keys_to_bring`).
    pub keys_away: Vec<String>,
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
    /// The `/private` vault's status, with what its image takes now; `None`
    /// without the vault's units on this machine (`crate::vault`).
    pub private: Option<ronoctua_private::Status>,
    /// Its status file is there and does not read.
    pub private_unreadable: bool,
    /// What is free on the disk its image is on: the cap's room.
    pub private_free: Option<u64>,
    /// What each channel is read from (`ronoctua_security::quantum`).
    pub quantum: ronoctua_security::quantum::Facts,
    /// The person's own recipient, for "for you" (`crate::seal::mine`).
    pub mine: crate::seal::Mine,
}

/// The variable a photograph of the FOOTER names its security scene with —
/// read only by a footer that is a photograph's (`Screen::photographing`),
/// never by one on a screen.
pub const SCENE_VARIABLE: &str = "RONOCTUA_PHOTOGRAPH_SECURITY_SCENE";

impl Scene {
    /// A photograph's footer's scene: the file [`SCENE_VARIABLE`] names, as
    /// the window's camera takes it (`--state`); `None` when it names none.
    /// A file that does not read is a refusal ON THE PICTURE, the reason in
    /// the log — never an empty scene that would pass for one.
    #[must_use]
    pub fn of_the_footer_camera() -> Option<Self> {
        let file = std::path::PathBuf::from(std::env::var_os(SCENE_VARIABLE)?);
        let read = std::fs::read_to_string(&file)
            .map_err(|e| e.to_string())
            .and_then(|raw| serde_json::from_str::<Self>(&raw).map_err(|e| e.to_string()));
        Some(read.unwrap_or_else(|e| {
            tracing::error!(file = %file.display(), error = %e, "the footer camera's security scene");
            Self {
                read: true,
                settings_refused: Some(Why::Unreadable),
                ..Self::default()
            }
        }))
    }

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
        // WHAT THIS GENERATION SHIPS, and what its ssh offers: neither moves
        // while the page is open.
        use ronoctua_security::quantum;
        self.quantum.ssh_kex = quantum::ssh_kex();
        self.quantum.built = quantum::built();
        self.quantum.sops = quantum::sops_in_use();
        self.quantum.root_disk = self.encrypted;
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
        self.window = ronoctua_security::relax::opened();
        self.helper = ronoctua_security::authority::has_helper();
        self.finger_sudo = fprint::sudo_allows(&user);
        self.read_keys(&user);
        self.privacy = links_up();
        self.listening = listening();
        self.vt_key = ronoctua_lookup::keys::has(ronoctua_lookup::keys::Key::VirusTotal);
        // THE VAULT: its status file and a `stat` of its image — never
        // anything under `/private`, which a suspended vault would freeze.
        (self.private, self.private_unreadable) = match crate::vault::status_now() {
            Ok(status) => (status, false),
            Err(_) => (None, true),
        };
        self.private_free = crate::vault::free_bytes();
        // THE BELONGINGS' RECIPIENT AND THE HOSTS HEARD: a sync and a request
        // move them while the page is open.
        self.mine = crate::seal::mine();
        self.quantum.belongings = match self.mine {
            crate::seal::Mine::Ready(_) => ronoctua_security::quantum::Belongings::PostQuantum,
            crate::seal::Mine::NotSynced => ronoctua_security::quantum::Belongings::Unsynced,
            crate::seal::Mine::NoBelongings => ronoctua_security::quantum::Belongings::None,
        };
        self.quantum.tls = tls_heard();
        self.read = true;
    }

    /// The security keys: whether they may answer sudo here, the person's in
    /// root's list (read as the person, `0644`), the person's own list, and
    /// the ones of it root's list lacks.
    fn read_keys(&mut self, user: &str) {
        use ronoctua_security::u2f;
        self.keys_answer = u2f::answers_sudo();
        (self.keys, self.keys_unreadable) = match u2f::listed(user) {
            Ok(keys) => (keys, false),
            Err(_) => (Vec::new(), true),
        };
        let own =
            std::env::var_os("HOME").map(|home| u2f::person_file(std::path::Path::new(&home)));
        self.person_keys = own
            .as_deref()
            .and_then(|file| u2f::listed_in(file, user).ok())
            .unwrap_or_default();
        // EVERY LINE OF THE PERSON'S OWN FILE IS THEIRS (written on a machine
        // where their name may differ), and every line of root's list counts:
        // the offers' own rule, so the page and the belongings offer agree.
        let person = own
            .and_then(|file| std::fs::read_to_string(file).ok())
            .unwrap_or_default();
        let root = std::fs::read_to_string(u2f::AUTHFILE).unwrap_or_default();
        self.keys_away = ronoctua_belongings::offers::keys_to_bring(&person, &root);
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

/// What the hosts our HTTPS talked to agreed (`ronoctua_http::groups`): how
/// many post-quantum, how many classical, and those, the latest first.
fn tls_heard() -> ronoctua_security::quantum::Tls {
    let groups = ronoctua_http::groups::store().parsed().unwrap_or_default();
    let (quantum, classical) = groups.counts();
    let mut old: Vec<(&String, i64)> = groups
        .hosts
        .iter()
        .filter(|(_, h)| !h.post_quantum)
        .map(|(name, h)| (name, h.at_s))
        .collect();
    old.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    ronoctua_security::quantum::Tls {
        quantum,
        classical,
        classical_hosts: old
            .into_iter()
            .take(ronoctua_security::quantum::NAMED_HOSTS)
            .map(|(name, _)| name.clone())
            .collect(),
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

// ─── what the doors do ──────────────────────────────────────────────────────

/// What changes in the security keys — what the door says when it is done.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyChange {
    /// A key was registered by a touch.
    Register,
    /// A key was removed.
    Remove,
    /// They stopped answering sudo here (root's line taken out; the person's
    /// own list kept, so they can come back).
    Off,
    /// The person's own list answers sudo here again.
    On,
    /// The keys registered on the person's other machines were brought.
    Bring,
}

/// What the sudo door is asked to do — each one a [`RootVerb`], and the
/// firewall's a whole [`Settings`] staged right before.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Work {
    /// Open the passwordless window.
    Relax(RelaxSpan),
    /// Open it again for the same span — "start the count again" (`sudo -n`
    /// lets it through while it is open, and the bar is never raised).
    Again(RelaxSpan),
    /// Root's list of the person's keys becomes `credentials`; then, when it
    /// is `Some`, the person's own list (which travels) becomes `person`.
    Keys {
        /// What changed, for what is said after.
        change: KeyChange,
        /// Root's list, whole.
        credentials: Vec<String>,
        /// The person's own list, whole, when it changes too.
        person: Option<Vec<String>>,
    },
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
    /// A request for the `/private` vault's root unit, with its secrets
    /// (`crate::vault`).
    Private(Box<crate::vault::Work>),
}

impl Work {
    /// The helper's verb.
    #[must_use]
    pub fn verb(&self) -> RootVerb {
        match self {
            Self::Relax(span) | Self::Again(span) => RootVerb::Relax { span: *span },
            Self::Keys { credentials, .. } => RootVerb::U2fKeys {
                credentials: credentials.clone(),
            },
            Self::RelaxEnd => RootVerb::RelaxEnd,
            Self::Firewall(_) => RootVerb::FirewallApply,
            Self::Enrol(finger) => RootVerb::FingerEnroll { finger: *finger },
            Self::Delete(which) => RootVerb::FingerDelete { which: *which },
            Self::FingerSudo(on) => RootVerb::FingerSudo { on: *on },
            Self::Private(_) => RootVerb::PrivateRoot,
        }
    }

    /// What it says when it was done.
    fn done_key(&self) -> &'static str {
        match self {
            Self::Relax(_) => key::ECHO_SECURITY_RELAXED,
            Self::Again(_) => key::ECHO_SECURITY_RELAX_AGAIN,
            Self::Keys { change, .. } => match change {
                KeyChange::Register => key::ECHO_SECURITY_KEYS_REGISTERED,
                KeyChange::Remove => key::ECHO_SECURITY_KEYS_REMOVED,
                KeyChange::Off => key::ECHO_SECURITY_KEYS_OFF,
                KeyChange::On => key::ECHO_SECURITY_KEYS_ON,
                KeyChange::Bring => key::ECHO_SECURITY_KEYS_BROUGHT,
            },
            Self::RelaxEnd => key::ECHO_SECURITY_RELAX_ENDED,
            Self::Firewall(_) => key::ECHO_SECURITY_FIREWALL_APPLIED,
            Self::Enrol(_) => key::ECHO_SECURITY_FINGER_ENROLLED,
            Self::Delete(_) => key::ECHO_SECURITY_FINGER_REMOVED,
            Self::FingerSudo(_) => key::ECHO_SECURITY_FINGER_SUDO_CHANGED,
            Self::Private(w) => crate::vault::done_key(w.op),
        }
    }
}

/// Runs `work` through the sudo door, as one root gesture asked in the sudo
/// bar (`root`, the door's gate's): the settings staged first (as the
/// person, checked before anybody is asked anything), then the helper — with
/// every progress word (an enrolment's touches) handed to `step` — and, for
/// the keys, the person's own list after root's took them. BLOCKS: a door's
/// thread.
///
/// # Errors
///
/// The door's and the helper's refusals; [`Why::Failed`] when root's list
/// changed and the person's own could not be written (the journal says why).
pub fn run_work(
    work: &mut Work,
    root: &mut dyn crate::sudo::Root,
    step: &mut dyn FnMut(&str),
) -> Result<(), Why> {
    match work {
        Work::Firewall(settings) => {
            ronoctua_security::firewall::stage(settings).map_err(|r| r.why)?;
        }
        // THE VAULT'S request and secrets, staged and taken away again
        // around the door — and why its unit refused, read into the work.
        Work::Private(w) => return crate::vault::run(w, root, step),
        _ => {}
    }
    root.run(work.verb().into(), step)?;
    // THE PERSON'S OWN LIST AFTER ROOT'S: a key that answers here travels
    // too — and nothing is written there for a gesture root refused.
    if let Work::Keys {
        person: Some(person),
        ..
    } = work
    {
        use ronoctua_security::u2f;
        let user = std::env::var("USER").unwrap_or_default();
        let file = std::env::var_os("HOME")
            .map(|home| u2f::person_file(std::path::Path::new(&home)))
            .ok_or(Why::Failed)?;
        u2f::set_person_keys(&file, &user, person).map_err(|r| {
            tracing::warn!(why = ?r.why, "the person's own list of keys could not be written");
            Why::Failed
        })?;
    }
    Ok(())
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
    /// A key's registration started: it waits for the touch, and this
    /// cancels it.
    Registering(ronoctua_security::u2f::Cancel),
    /// A key's registration ended: its credential, or why not.
    Registered(Result<String, Why>),
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
    /// The header of a `.age` was read (`None`: it could not be).
    Sniffed {
        /// The file.
        path: PathBuf,
        /// What its header says.
        kind: Option<ronoctua_pq::Kind>,
    },
    /// The queue refused a seal or an open — answered only when this page
    /// sent it ([`View::came`]).
    Sealed(crate::report::SealReport),
    /// What handing a seal to the queue answered ([`Job::Seal`]).
    SealHanded(ronoctua_files::mode::Explanation),
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
    /// Run this through the sudo door, on a thread — asked in the sudo bar.
    Door(Work),
    /// Register a security key by a touch, on a thread (as the person).
    Register,
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
    /// Hand this seal to the queue on a thread ([`Job::Seal`]), and say
    /// what it answered.
    Seal(crate::report::SealDoing),
    /// Read the header of this `.age` on a thread ([`Job::Sniff`]).
    Sniff(PathBuf),
    /// Read the clipboard for the file picker ([`View::picker_pasted`]).
    Paste,
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
    /// Remove this security key (its credential).
    RemoveKey(String),
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
    /// Open the window for this long.
    Relax(RelaxSpan),
    /// End it.
    RelaxEnd,
    /// Enrol the chosen finger.
    Enrol,
    /// Remove this finger.
    Finger(Finger),
    /// Fingers for sudo, the other way.
    FingerSudo,
    /// Start the window's count again.
    RelaxAgain,
    /// Register a security key.
    KeyRegister,
    /// Stop the registration that waits for a touch.
    KeyStop,
    /// Remove this key (its credential).
    Key(String),
    /// The keys answer sudo here, or they stop.
    KeysSwitch,
    /// Bring the keys registered on the person's other machines.
    KeysBring,
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
    /// A row of the `/private` vault's section.
    Private(crate::vault::Act),
    /// Choose what to seal, or to open, in the file picker.
    Seal(crate::seal::Pick),
    /// Seal what was chosen, this way.
    SealMode(crate::seal::Mode),
    /// A row of the file picker, while it is up.
    Pick(crate::file_picker::Target),
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
            Self::Private(a) => return a.command(),
            Self::Seal(crate::seal::Pick::Files) => "security-encrypt",
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
            Self::RelaxAgain => key::SECURITY_ABOUT_RELAX_AGAIN,
            Self::KeyRegister => key::SECURITY_ABOUT_KEY_REGISTER,
            Self::KeyStop => key::SECURITY_ABOUT_KEY_STOP,
            Self::Key(_) => key::SECURITY_ABOUT_KEY,
            Self::KeysSwitch => key::SECURITY_ABOUT_KEYS_SWITCH,
            Self::KeysBring => key::SECURITY_ABOUT_KEYS_BRING,
            Self::Report(_) => key::SECURITY_ABOUT_REPORT,
            Self::Url(_) => key::SECURITY_ABOUT_URL,
            Self::Answer(_) => key::SECURITY_ABOUT_ANSWER,
            Self::Private(a) => a.about(),
            Self::Seal(crate::seal::Pick::Folder) => key::SECURITY_ABOUT_SEAL_FOLDER,
            Self::Seal(crate::seal::Pick::Open) => key::SECURITY_ABOUT_SEAL_OPEN,
            Self::SealMode(_) => key::SECURITY_ABOUT_SEAL_MODE,
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

impl Row {
    /// Whether a click takes it — a row the cursor stands on — and so
    /// whether a resting pointer lights it: ONE ANSWER for both doors' light
    /// and click (the region's `Row::takes`, the window's `Content::takes`,
    /// [`View::clicked`]). A picker's row takes one by the same answer: it
    /// carries its act exactly when it is no heading.
    #[must_use]
    pub const fn takes(&self) -> bool {
        !self.heading
    }
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
    /// A key for the file picker while it is up — every letter is its.
    Pick(ronoctua_keymap::Key),
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
                        crate::chrome::tinted_value(fields, column, status, focused)
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
    /// What the row the cursor was last settled on does: the cursor keeps
    /// its ROW, never its index — a line said above it (a sudo work in
    /// flight, a scene read again) moves the rows under an index, and the
    /// cursor left there fell on a line that does not act, and from it to
    /// the page's first row.
    held: Option<Act>,
    /// What the row that takes the cursor at the next rows does: the row it
    /// was on became another (a key's registration starting, or ending).
    follow: Option<Act>,
    /// Brought into view at the next rows over a scene that was read.
    open_at: Option<Target>,
    /// The page's own yes-or-no question.
    confirm: Option<Confirm>,
    /// A sudo work in flight.
    busy: Option<Work>,
    /// A security key's registration waits for its touch.
    registering: bool,
    /// What cancels it, once its thread said it started.
    register_cancel: Option<ronoctua_security::u2f::Cancel>,
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
    /// The vault's questions, as far as they were answered.
    vault: Option<crate::vault::Asking>,
    /// The vault's work in flight got past sudo and its unit started
    /// (`ronoctua_private::STARTED`): an opening by the key waits for a
    /// touch NOW, and the section says so.
    vault_started: bool,
    /// The file picker, up over the page, for what to seal or open.
    picking: Option<crate::file_picker::Hosted<crate::seal::Pick>>,
    /// What was chosen to seal: "for whom" is asked over the page.
    seal_mode: Option<Vec<PathBuf>>,
    /// What a seal's question on the door's line waits for.
    sealing: Option<crate::seal::Wanting>,
    /// The seals and opens this page handed the queue: a refusal of one of
    /// them is said here — a wrong passphrase asked again — and only here.
    seal_sent: crate::seal::Sent,
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
    /// go (`Esc`, `C-g`): a rule being edited, the vault's questions, what a
    /// seal asks. An answer typed later under the same command, by name,
    /// must not be taken for them.
    pub fn let_go(&mut self) {
        self.editing = None;
        self.vault = None;
        self.sealing = None;
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
        // A ROW OF THE PICKER is chosen there: its cursor, and the second
        // click its `RET`.
        if let (Some(h), Some(Act::Pick(t))) = (
            self.picking.as_mut(),
            rows.get(n)
                .filter(|r| r.takes())
                .and_then(|r| r.act.as_ref()),
        ) {
            let again = h.clicked(t.clone());
            self.place(n);
            return again;
        }
        if rows.get(n).is_some_and(Row::takes) {
            let again = self.cursor == n;
            self.place(n);
            return again;
        }
        false
    }

    /// The sentence the pointer resting on row `n` reads.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let act = rows.get(n)?.act.as_ref()?;
        if matches!(act, Act::Pick(_)) {
            return None;
        }
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
        let (left, room) = crate::chrome::extent(columns);
        let mut b = Lines {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            section: Section::Now,
            texts,
            pending: Vec::new(),
        };
        // THE PICKER'S ROWS while it is up, as this page's own.
        if let Some(h) = self.picking.as_ref() {
            return h.rows(columns, texts, |p| Row {
                act: (!p.heading).then_some(Act::Pick(p.target)),
                heading: p.heading,
                fields: p.fields,
                section: Section::Encrypt,
            });
        }
        if let Some(confirm) = &self.confirm {
            self.question(&mut b, confirm);
            return b.finish();
        }
        // FOR WHOM, over the page, about what was chosen.
        if let Some(paths) = &self.seal_mode {
            seal_question(&mut b, paths, scene);
            return b.finish();
        }
        let now = now_s();
        self.now(&mut b, scene, now, zone);
        self.firewall(&mut b, scene, zone);
        self.admin(&mut b, scene, now, zone);
        if scene.reader.is_some() {
            self.finger(&mut b, scene);
        }
        self.keys(&mut b, scene);
        self.private(&mut b, scene);
        encrypt(&mut b, scene);
        device(&mut b, scene);
        quantum(&mut b, scene);
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
            Confirm::RemoveKey(credential) => (
                b.texts.t_with(
                    key::SECURITY_KEYS_ASK_REMOVE,
                    &[("key", &key_shown(credential))],
                ),
                Some(b.t(key::SECURITY_KEYS_REMOVE_WARNING)),
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
        if let Some(window) = scene.window.filter(|w| w.is_open(now)) {
            off += 1;
            b.off(
                &b.t(key::SECURITY_NOW_SUDO_OPEN),
                &window.until_unix_s.map_or_else(
                    || b.t(key::SECURITY_SUDO_UNTIL_YOU_END_IT),
                    |until| crate::security_words::end_text(until, now, zone, b.texts),
                ),
                Act::RelaxEnd,
            );
        }
        if settings.shields {
            off += 1;
            b.off(&b.t(key::SECURITY_NOW_SHIELDS), "", Act::Shields);
        }
        // THE VAULT, FROZEN since the machine slept with something in it:
        // whatever touches it waits until the passphrase resumes it.
        if scene
            .private
            .is_some_and(|p| p.state == ronoctua_private::State::SuspendedInUse)
        {
            off += 1;
            b.off(
                &b.t(key::SECURITY_NOW_PRIVATE_SUSPENDED),
                "",
                Act::Private(crate::vault::Act::Resume),
            );
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
        let open = scene.window.filter(|w| w.is_open(now));
        match open {
            Some(window) => {
                b.pair(
                    &b.t(key::SECURITY_SUDO_PASSWORD),
                    &window.until_unix_s.map_or_else(
                        || b.t(key::SECURITY_SUDO_NOT_ASKED_NO_LIMIT),
                        |until| {
                            b.texts.t_with(
                                key::SECURITY_SUDO_NOT_ASKED_UNTIL,
                                &[(
                                    "time",
                                    &crate::security_words::end_text(until, now, zone, b.texts),
                                )],
                            )
                        },
                    ),
                    None,
                    Some(tokens::color::STATE_ERROR),
                );
                // THE TIME LEFT, live: the door's clock repaints it while the
                // page is drawn (`View::counts_down`) — never on the chrome.
                if let Some(left) = window.left_s(now) {
                    b.pair(
                        &b.t(key::SECURITY_SUDO_LEFT),
                        &left_text(left),
                        None,
                        Some(tokens::color::STATE_ERROR),
                    );
                }
                if window.again().is_some() {
                    b.pair(
                        &b.t(key::SECURITY_SUDO_AGAIN),
                        "",
                        Some(Act::RelaxAgain),
                        None,
                    );
                }
                b.pair(&b.t(key::SECURITY_SUDO_END), "", Some(Act::RelaxEnd), None);
            }
            None => {
                b.pair(
                    &b.t(key::SECURITY_SUDO_PASSWORD),
                    &b.t(key::SECURITY_SUDO_ASKED),
                    None,
                    None,
                );
                for span in RelaxSpan::ALL {
                    b.pair(&span_label(span, b.texts), "", Some(Act::Relax(span)), None);
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
        // THE VAULT'S WORK says itself in its own section (`vault_busy`).
        if let Some(work) = &self.busy
            && !matches!(work, Work::Private(_))
        {
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

    /// The security keys: whether they answer sudo here, each key in root's
    /// list (removing one asks first), the ones of the person's other
    /// machines to bring, and registering one by a touch.
    fn keys(&self, b: &mut Lines<'_>, scene: &Scene) {
        use ronoctua_security::u2f::MOST_KEYS;
        b.heading(Section::Keys);
        // NOTHING OFFERED BEFORE THE LISTS ARE READ: "register" over an empty
        // scene would be a row for a machine that may have no keys at all.
        if !scene.read {
            b.note(&b.t(key::SECURITY_NOW_READING));
            return;
        }
        if !scene.keys_answer {
            b.note(&b.t(key::SECURITY_KEYS_NOT_HERE));
            return;
        }
        if scene.keys_unreadable {
            b.line(
                &b.t(crate::security_words::refusal(Why::Unreadable)),
                tokens::color::STATE_WARNING,
            );
        }
        let count = |n: usize| Arg::Number(i64::try_from(n).unwrap_or(i64::MAX));
        let on = !scene.keys.is_empty();
        let switch = if on {
            b.texts
                .t_args(key::SECURITY_KEYS_ON, &[("count", count(scene.keys.len()))])
        } else if scene.keys_away.is_empty() {
            b.t(key::SECURITY_KEYS_OFF)
        } else {
            // WHAT "ON" WOULD BRING, said before the press: the keys of the
            // person's own list.
            b.texts.t_args(
                key::SECURITY_KEYS_OFF_LISTED,
                &[("count", count(scene.keys_away.len()))],
            )
        };
        let switches = on || !scene.keys_away.is_empty();
        b.pair(
            &b.t(key::SECURITY_KEYS_SUDO),
            &switch,
            switches.then_some(Act::KeysSwitch),
            None,
        );
        for (n, credential) in scene.keys.iter().enumerate() {
            // THE KEY, a space of air, and its place in the list.
            b.pair(
                &format!(
                    "{} {}",
                    tokens::icon::SECURITY_KEY,
                    b.texts
                        .t_args(key::SECURITY_KEYS_KEY, &[("n", count(n + 1))])
                ),
                &key_shown(credential),
                Some(Act::Key(credential.clone())),
                None,
            );
        }
        if on && !scene.keys_away.is_empty() {
            b.pair(
                &b.t(key::SECURITY_KEYS_BRING),
                &b.texts.t_args(
                    key::SECURITY_KEYS_AWAY,
                    &[("count", count(scene.keys_away.len()))],
                ),
                Some(Act::KeysBring),
                None,
            );
        }
        if self.registering {
            b.line(
                &b.t(key::SECURITY_KEYS_TOUCH_NOW),
                tokens::color::STATE_ACCENT,
            );
            b.pair(&b.t(key::SECURITY_KEYS_STOP), "", Some(Act::KeyStop), None);
        } else if scene.keys.len() < MOST_KEYS {
            b.pair(
                &b.t(key::SECURITY_KEYS_REGISTER),
                "",
                Some(Act::KeyRegister),
                None,
            );
        } else {
            b.note(
                &b.texts
                    .t_args(key::SECURITY_KEYS_FULL, &[("count", count(MOST_KEYS))]),
            );
        }
        b.note(&b.t(key::SECURITY_KEYS_NOTE));
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
            if let Some(n) = acting {
                self.place(n);
            }
            self.keep_cursor(rows);
            return Some(top);
        }
        self.keep_cursor(rows);
        None
    }

    /// The cursor on a row that acts — the row it was on, wherever the rows
    /// moved it ([`View::held`]), or the one it was sent to
    /// ([`View::follow`]) — and nothing else moved.
    fn keep_cursor(&mut self, rows: &[Row]) {
        let row_of = |act: &Act| {
            rows.iter()
                .position(|r| !r.heading && r.act.as_ref() == Some(act))
        };
        if let Some(n) = self.follow.take().as_ref().and_then(row_of) {
            self.cursor = n;
        // THE PICKER keeps its own cursor (`pick_key`), row by row.
        } else if self.picking.is_none()
            && let Some(held) = &self.held
            && rows.get(self.cursor).and_then(|r| r.act.as_ref()) != Some(held)
            && let Some(n) = row_of(held)
        {
            self.cursor = n;
        }
        if !rows.get(self.cursor).is_some_and(|r| !r.heading) {
            self.cursor = rows.iter().position(|r| !r.heading).unwrap_or(0);
        }
        self.held = rows.get(self.cursor).and_then(|r| r.act.clone());
    }

    /// The cursor put on row `n` by the page itself — a click, its own
    /// question at the top, the picker's row — and held there from now on.
    fn place(&mut self, n: usize) {
        self.cursor = n;
        self.held = None;
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
                self.held = rows[at].act.clone();
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
        // THE PICKER takes its own keys, and the page's that walk, answer
        // and go back — a second click's `RET`, the pointer's way back.
        if self.picking.is_some() {
            use ronoctua_keymap::{Key, Sym};
            let key = match p {
                Press::Pick(key) => key,
                Press::Up => Key::simple(Sym::Up),
                Press::Down => Key::simple(Sym::Down),
                Press::Left => Key::simple(Sym::Left),
                Press::Right => Key::simple(Sym::Right),
                Press::Enter => Key::simple(Sym::Enter),
                Press::Escape => Key::simple(Sym::Esc),
                _ => return Errand::Nothing,
            };
            return self.pick_key(key, scene, columns, texts, zone);
        }
        let rows = self.rows(scene, columns, texts, zone);
        self.keep_cursor(&rows);
        let under = rows.get(self.cursor).and_then(|r| r.act.clone());
        match p {
            // "FOR WHOM" IS LEFT, and the page comes back where it was.
            Press::Escape if self.seal_mode.is_some() => {
                self.seal_mode = None;
                self.open_at = Some(Target::Section(Section::Encrypt));
                Errand::Nothing
            }
            Press::Pick(_) => Errand::Nothing,
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
                    self.place(0);
                    return Errand::Nothing;
                }
                // A REGISTRATION WAITING FOR ITS TOUCH is let go first.
                if self.registering {
                    self.stop_registering();
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
                // A KEY, asked first as `RET` on it asks.
                Some(Act::Key(credential)) => self.act(Act::Key(credential), scene, texts),
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
            Act::Relax(minutes) => Errand::Door(Work::Relax(minutes)),
            Act::RelaxEnd => Errand::Door(Work::RelaxEnd),
            // THE SAME SPAN AGAIN, from now — and only for a window still
            // open with an end: `sudo -n` lets it in, and nothing is asked.
            Act::RelaxAgain => match scene.window.filter(|w| w.is_open(now_s())) {
                Some(window) if window.again().is_some() => Errand::Door(Work::Again(window.span)),
                _ => Errand::Nothing,
            },
            Act::KeyRegister => {
                if self.registering {
                    Errand::Nothing
                } else if scene.keys.len() >= ronoctua_security::u2f::MOST_KEYS {
                    Errand::Say(keys_full(texts))
                } else {
                    self.registering = true;
                    // ITS ROW BECOMES THE LINE THAT ASKS FOR THE TOUCH: the
                    // cursor goes to the row that stops it, just under.
                    self.follow = Some(Act::KeyStop);
                    Errand::Register
                }
            }
            Act::KeyStop => {
                self.stop_registering();
                Errand::Nothing
            }
            Act::Key(credential) => {
                self.confirm = Some(Confirm::RemoveKey(credential));
                self.place(0);
                self.open_at = Some(Target::Top);
                Errand::Nothing
            }
            // OFF: root's line out, the person's own list kept — so ON brings
            // them back from it with no new touch.
            Act::KeysSwitch if !scene.keys.is_empty() => Errand::Door(Work::Keys {
                change: KeyChange::Off,
                credentials: Vec::new(),
                person: None,
            }),
            Act::KeysSwitch => keys_with(KeyChange::On, &scene.keys_away, texts),
            Act::KeysBring => {
                let mut all = scene.keys.clone();
                for credential in &scene.keys_away {
                    all = ronoctua_security::u2f::with_key(&all, credential);
                }
                keys_with(KeyChange::Bring, &all, texts)
            }
            // NO READER, NO PASSWORD ASKED: the refusal is said before the
            // door is knocked on, not after somebody typed for nothing.
            Act::Enrol if scene.reader.is_none() => {
                Errand::Say(texts.t(crate::security_words::refusal(Why::NoReader)))
            }
            Act::Enrol => match self.offered(scene) {
                Some(f) => {
                    self.step = None;
                    Errand::Door(Work::Enrol(f))
                }
                None => Errand::Nothing,
            },
            Act::Finger(f) => {
                self.confirm = Some(Confirm::RemoveFinger(f));
                self.place(0);
                self.open_at = Some(Target::Top);
                Errand::Nothing
            }
            Act::FingerSudo => Errand::Door(Work::FingerSudo(!scene.finger_sudo)),
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
            Act::Private(a) => self.vault_act(a, scene),
            Act::Report(sha) => Errand::Url(ronoctua_lookup::virustotal::page_of(&sha)),
            Act::Seal(pick) => {
                self.pick(pick);
                Errand::Nothing
            }
            Act::SealMode(mode) => {
                let paths = self.seal_mode.take().unwrap_or_default();
                self.open_at = Some(Target::Section(Section::Encrypt));
                self.seal_next(crate::seal::choose(mode, paths), texts)
            }
            // THE PICKER'S `RET` is its own, and reaches it above.
            Act::Pick(_) => Errand::Nothing,
            Act::Command(id) => Errand::Command(id),
            Act::Url(url) => Errand::Url(url.to_owned()),
            Act::Answer(yes) => {
                let confirm = self.confirm.take();
                self.place(0);
                match (yes, confirm) {
                    (true, Some(Confirm::Upload { path, sha256 })) => {
                        Errand::Upload { path, sha256 }
                    }
                    (true, Some(Confirm::RemoveFinger(f))) => {
                        Errand::Door(Work::Delete(Fingers::One(f)))
                    }
                    // OUT OF BOTH LISTS: it answers here no more, and it does
                    // not travel to answer anywhere else.
                    (true, Some(Confirm::RemoveKey(credential))) => {
                        use ronoctua_security::u2f::without_key;
                        Errand::Door(Work::Keys {
                            change: KeyChange::Remove,
                            credentials: without_key(&scene.keys, &credential),
                            person: Some(without_key(&scene.person_keys, &credential)),
                        })
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
            crate::vault::PASSPHRASE
            | crate::vault::NEW_PASSPHRASE
            | crate::vault::REPEAT_PASSPHRASE
            | crate::vault::CAP => self.vault_answered(id, text, scene, texts),
            // A SEAL'S QUESTION: the same state the file manager drives.
            id if crate::seal::QUESTIONS.contains(&id) => match self.sealing.take() {
                Some(wanting) => {
                    let next = crate::seal::answer(wanting, id, text);
                    self.seal_next(next, texts)
                }
                None => Errand::Nothing,
            },
            _ => Errand::Nothing,
        }
    }

    /// A question of the page was let go.
    pub fn unanswered(&mut self, id: &str) {
        match id {
            RULE => self.editing = None,
            crate::vault::PASSPHRASE
            | crate::vault::NEW_PASSPHRASE
            | crate::vault::REPEAT_PASSPHRASE
            | crate::vault::CAP => self.vault = None,
            id if crate::seal::QUESTIONS.contains(&id) => self.sealing = None,
            _ => {}
        }
    }

    /// What a door's thread brought back: what the view keeps of it, what
    /// the door says, and what it does next.
    pub fn came(&mut self, came: Came, scene: &mut Scene, texts: &ronoctua_i18n::Texts) -> Outcome {
        match came {
            // WHAT OPENS IT: the question on this door's line, or why not.
            Came::Sniffed { path, kind } => {
                let errand = self.seal_next(crate::seal::sniffed(path, kind), texts);
                Outcome {
                    errand,
                    ..Outcome::shown()
                }
            }
            // THE QUEUE ENDED A JOB THIS PAGE SENT — a seal or an open: a
            // wrong passphrase is asked again on this door's line, any other
            // refusal said there. Somebody else's job (the file manager's) is
            // theirs, and a header read is the manager's own question.
            Came::Sealed(report) => match crate::seal::job_ended(&mut self.seal_sent, &report) {
                Some(next) => Outcome {
                    errand: self.seal_next(next, texts),
                    ..Outcome::quiet()
                },
                None => Outcome::quiet(),
            },
            Came::SealHanded(e) => Outcome {
                say: Some(texts.t_with(e.key, &e.borrowed())),
                ..Outcome::quiet()
            },
            Came::Scene(fresh) => {
                *scene = *fresh;
                Outcome::quiet()
            }
            Came::Stirred => Outcome {
                read: true,
                ..Outcome::quiet()
            },
            // THE VAULT'S UNIT STARTED, sudo passed: an opening by the key
            // waits for its touch from now on, and EVERY door says so here
            // — never earlier, when a password question may still come.
            // A step of the vault's is never the fingerprint's.
            Came::Step(word) => {
                if let Some(Work::Private(w)) = &self.busy {
                    if word == ronoctua_private::STARTED && w.touches() {
                        self.vault_started = true;
                        return Outcome {
                            say: Some(texts.t(key::SECURITY_PRIVATE_TOUCH)),
                            ..Outcome::quiet()
                        };
                    }
                    return Outcome::quiet();
                }
                self.step = Some(word);
                Outcome::quiet()
            }
            // THE REGISTRATION STARTED: the section says to touch the key, and
            // its row stops it.
            Came::Registering(cancel) => {
                if self.registering {
                    self.register_cancel = Some(cancel);
                } else {
                    // LET GO BEFORE ITS THREAD SAID IT STARTED: stopped now.
                    cancel.cancel();
                }
                Outcome::quiet()
            }
            // TOUCHED: root's list takes the new key through the door, and the
            // person's own list after it, so it travels.
            Came::Registered(result) => {
                self.registration_over();
                self.register_cancel = None;
                match result {
                    Ok(credential) => {
                        use ronoctua_security::u2f::{MOST_KEYS, with_key};
                        let credentials = with_key(&scene.keys, &credential);
                        let errand = if credentials.len() > MOST_KEYS {
                            Errand::Say(keys_full(texts))
                        } else {
                            Errand::Door(Work::Keys {
                                change: KeyChange::Register,
                                credentials,
                                person: Some(with_key(&scene.person_keys, &credential)),
                            })
                        };
                        Outcome {
                            errand,
                            ..Outcome::quiet()
                        }
                    }
                    Err(why) => Outcome {
                        say: Some(texts.t(crate::security_words::refusal(why))),
                        ..Outcome::quiet()
                    },
                }
            }
            Came::Door { work, result } => {
                self.busy = None;
                self.vault_started = false;
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
                    Err(why) => {
                        if matches!(work, Work::Enrol(_)) {
                            self.step = None;
                        }
                        // THE VAULT SAYS WHY ITS UNIT REFUSED (a wrong
                        // passphrase, no key, something holding it) instead
                        // of the door's "it did not work".
                        let said = match &work {
                            Work::Private(w) => w.refused.map(crate::vault::refusal),
                            _ => None,
                        };
                        Outcome {
                            say: Some(
                                texts
                                    .t(said.unwrap_or_else(|| crate::security_words::refusal(why))),
                            ),
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
                self.place(0);
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

    /// Lets go of a registration that waits for its touch: its program is
    /// killed — now, or as soon as its thread says it started.
    fn stop_registering(&mut self) {
        self.registration_over();
        if let Some(cancel) = self.register_cancel.take() {
            cancel.cancel();
        }
    }

    /// No registration waits any more: a cursor on the row that stopped it
    /// goes back to the row that starts one, which takes its place.
    fn registration_over(&mut self) {
        self.registering = false;
        let stop = Some(Act::KeyStop);
        if self.held == stop || self.follow == stop {
            self.follow = Some(Act::KeyRegister);
        }
    }

    /// The page closed: a registration waiting for its touch goes with it.
    pub fn closed(&mut self) {
        self.stop_registering();
    }

    /// Whether the page's live count runs: a window open now, with an end.
    /// A door's clock beats once a second while this holds and the page is
    /// drawn, and is taken down when it stops holding.
    #[must_use]
    pub fn counts_down(scene: &Scene) -> bool {
        let now = now_s();
        scene
            .window
            .is_some_and(|w| w.is_open(now) && w.until_unix_s.is_some())
    }

    /// Whether the rows on screen — `shown`, a range of the rows drawn —
    /// hold the administrator's section, whose count would move.
    #[must_use]
    pub fn count_on_screen(rows: &[Row], shown: std::ops::Range<usize>) -> bool {
        rows.get(shown.start.min(rows.len())..shown.end.min(rows.len()))
            .is_some_and(|on| on.iter().any(|r| r.section == Section::Admin))
    }

    /// A photograph's step: a registration waiting for its touch.
    pub fn registering_shown(&mut self) {
        self.registering = true;
    }

    /// The sudo work in flight, if one is — what a photograph's refusal
    /// step answers.
    #[must_use]
    pub fn in_flight(&self) -> Option<Work> {
        self.busy.clone()
    }

    /// A sudo work went to its thread.
    pub fn sent(&mut self, work: &Work) {
        self.busy = Some(work.clone());
        self.vault_started = false;
    }

    /// Whether the view still WAITS for something with its page closed — a
    /// sudo work in flight, a registration's touch, an online check, a
    /// question on the door's line, a seal the queue has not ended: a door
    /// that is not showing it keeps it alive until it does not (the
    /// footer's hidden region, `Region::lingers`).
    #[must_use]
    pub fn waits(&self) -> bool {
        self.busy.is_some()
            || self.registering
            || self.checking
            || self.editing.is_some()
            || self.vault.is_some()
            || self.sealing.is_some()
            || !self.seal_sent.is_empty()
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

// THE 2026-10-05 WAVE'S SECTIONS, each saying "not yet" until its package
// lands — drawn where they will be, so the page's order is the final one.
// ── anchor: L12 ──
// THE /private VAULT'S SECTION, and the questions its rows ask
// (`crate::vault`): one flow for both doors and for the leader's two keys.
impl View {
    fn private(&self, b: &mut Lines<'_>, scene: &Scene) {
        use crate::vault::Act as V;
        use ronoctua_private::State;
        b.heading(Section::Private);
        let Some(status) = scene.private else {
            if scene.private_unreadable {
                b.line(
                    &b.t(key::SECURITY_PRIVATE_UNREADABLE),
                    tokens::color::STATE_WARNING,
                );
            } else {
                b.note(&b.t(if scene.read {
                    key::SECURITY_PRIVATE_MISSING
                } else {
                    key::SECURITY_PRIVATE_ABOUT
                }));
            }
            return;
        };
        let size = |bytes: u64| crate::sizes::readable_bytes(bytes, b.texts.decimal_mark());
        if status.state == State::Absent {
            b.note(&b.t(key::SECURITY_PRIVATE_ABOUT));
            let cap = ronoctua_private::suggested_cap(scene.private_free.unwrap_or(0));
            b.pair(
                &b.t(key::SECURITY_PRIVATE_CREATE),
                &b.texts
                    .t_with(key::SECURITY_PRIVATE_CREATE_VALUE, &[("size", &size(cap))]),
                Some(Act::Private(V::Create)),
                None,
            );
            self.vault_busy(b);
            return;
        }
        let (state, tint) = match status.state {
            State::Open => (key::SECURITY_PRIVATE_OPEN_AT, None),
            State::SuspendedInUse => (
                key::SECURITY_PRIVATE_SUSPENDED,
                Some(tokens::color::STATE_ERROR),
            ),
            State::Locked | State::Absent => (key::SECURITY_PRIVATE_LOCKED, None),
        };
        b.pair(&b.t(key::SECURITY_PRIVATE_STATE), &b.t(state), None, tint);
        match status.state {
            State::Open => {
                b.pair(
                    &b.t(key::SECURITY_PRIVATE_LOCK),
                    "",
                    Some(Act::Private(V::Lock)),
                    None,
                );
                b.pair(
                    &b.t(key::SECURITY_PRIVATE_SHOW),
                    "",
                    Some(Act::Private(V::Show)),
                    None,
                );
            }
            State::SuspendedInUse => b.pair(
                &b.t(key::SECURITY_PRIVATE_RESUME),
                "",
                Some(Act::Private(V::Resume)),
                None,
            ),
            State::Locked | State::Absent => {
                b.pair(
                    &b.t(key::SECURITY_PRIVATE_OPEN),
                    "",
                    Some(Act::Private(V::OpenPassphrase)),
                    None,
                );
                if status.fido2 {
                    b.pair(
                        &b.t(key::SECURITY_PRIVATE_OPEN_KEY),
                        "",
                        Some(Act::Private(V::OpenKey)),
                        None,
                    );
                }
            }
        }
        b.pair(
            &b.t(key::SECURITY_PRIVATE_SIZE),
            &b.texts.t_with(
                key::SECURITY_PRIVATE_SIZE_VALUE,
                &[
                    ("real", &size(status.real_bytes)),
                    ("cap", &size(status.cap_bytes)),
                ],
            ),
            None,
            None,
        );
        // MORE THAN THE DISK HAS FREE: allowed — it is sparse — and said.
        if let Some(free) = scene.private_free
            && status.cap_bytes > status.real_bytes.saturating_add(free)
        {
            b.line(
                &b.t(key::SECURITY_PRIVATE_OVERCOMMIT),
                tokens::color::STATE_WARNING,
            );
        }
        b.pair(
            &b.t(key::SECURITY_PRIVATE_GROW),
            &size(status.cap_bytes),
            Some(Act::Private(V::Grow)),
            None,
        );
        b.pair(
            &b.t(key::SECURITY_PRIVATE_CIPHER),
            &b.t(key::SECURITY_PRIVATE_CIPHER_VALUE),
            None,
            None,
        );
        if status.fido2 {
            b.pair(
                &b.t(key::SECURITY_PRIVATE_KEY),
                &b.t(key::SECURITY_PRIVATE_KEY_ENROLLED),
                None,
                None,
            );
            b.pair(
                &b.t(key::SECURITY_PRIVATE_FORGET),
                "",
                Some(Act::Private(V::Forget)),
                None,
            );
        } else {
            b.pair(
                &b.t(key::SECURITY_PRIVATE_ENROL),
                "",
                Some(Act::Private(V::Enrol)),
                None,
            );
        }
        b.pair(
            &b.t(key::SECURITY_PRIVATE_PASSPHRASE),
            "",
            Some(Act::Private(V::Passphrase)),
            None,
        );
        self.vault_busy(b);
        b.note(&b.t(key::SECURITY_PRIVATE_NOTE));
    }

    /// The line a request in flight draws: a touch asked for — once its unit
    /// started, never while sudo may still ask the password — or work.
    ///
    /// AFTER THE SECTION'S LAST ROW THAT ACTS, and the only line of the
    /// vault's work on the page (the Admin section's leaves it out): the
    /// cursor is an index into the rows, and a line that came and went above
    /// the row just pressed would move it onto another — a second `RET` on
    /// "open with the key" would then ask the passphrase.
    fn vault_busy(&self, b: &mut Lines<'_>) {
        if let Some(Work::Private(w)) = &self.busy {
            if w.touches() && self.vault_started {
                b.line(
                    &b.t(key::SECURITY_PRIVATE_TOUCH),
                    tokens::color::STATE_ACCENT,
                );
            } else {
                b.line(
                    &b.t(key::SECURITY_PRIVATE_BUSY),
                    tokens::color::TEXT_SECONDARY,
                );
            }
        }
    }

    /// What a row of the section does: a question first, or the door.
    fn vault_act(&mut self, a: crate::vault::Act, scene: &Scene) -> Errand {
        use crate::vault::{Act as V, Asking, CAP, PASSPHRASE, Work as Vault};
        use ronoctua_private::Op;
        let door = |work: Vault| Errand::Door(Work::Private(Box::new(work)));
        let ask = |id: &'static str, secret: bool, initial: Option<String>| Errand::Ask {
            id,
            secret,
            initial,
        };
        match a {
            V::Create => {
                self.vault = Some(Asking::Create {
                    cap: None,
                    new: None,
                });
                let cap = ronoctua_private::suggested_cap(scene.private_free.unwrap_or(0));
                ask(CAP, false, Some(crate::vault::cap_text(cap)))
            }
            V::OpenPassphrase | V::Resume => {
                self.vault = Some(Asking::Open);
                ask(PASSPHRASE, true, None)
            }
            V::OpenKey => {
                self.vault = None;
                door(Vault::bare(Op::Open))
            }
            V::Lock => door(Vault::bare(Op::Close)),
            V::Show => Errand::Command("private-show"),
            V::Grow => {
                self.vault = Some(Asking::Grow { cap: None });
                let cap = scene.private.map_or(0, |s| s.cap_bytes);
                ask(CAP, false, Some(crate::vault::cap_text(cap)))
            }
            V::Enrol => {
                self.vault = Some(Asking::Enrol);
                ask(PASSPHRASE, true, None)
            }
            V::Forget => door(Vault::bare(Op::ForgetFido2)),
            V::Passphrase => {
                self.vault = Some(Asking::Change {
                    old: None,
                    new: None,
                });
                ask(PASSPHRASE, true, None)
            }
        }
    }

    /// One of the vault's questions answered: the next question, or the
    /// door with everything answered. An answer nobody is waiting for (the
    /// question was let go) does nothing; an empty one lets it go.
    fn vault_answered(
        &mut self,
        id: &str,
        text: &str,
        scene: &Scene,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        use crate::vault::Work as Vault;
        use crate::vault::{Asking, CAP, NEW_PASSPHRASE, PASSPHRASE, REPEAT_PASSPHRASE};
        use ronoctua_private::Op;
        let door = |work: Vault| Errand::Door(Work::Private(Box::new(work)));
        let secret = |id: &'static str| Errand::Ask {
            id,
            secret: true,
            initial: None,
        };
        let Some(asking) = self.vault.take() else {
            return Errand::Nothing;
        };
        if text.is_empty() {
            return Errand::Nothing;
        }
        let cap_of = |text: &str| match ronoctua_private::parse_size(text) {
            None => Err(Errand::Say(texts.t(key::ECHO_PRIVATE_NOT_A_SIZE))),
            Some(c) if c < ronoctua_private::MIN_CAP_BYTES => Err(Errand::Say(texts.t_with(
                key::ECHO_PRIVATE_TOO_SMALL,
                &[(
                    "size",
                    &crate::sizes::readable_bytes(
                        ronoctua_private::MIN_CAP_BYTES,
                        texts.decimal_mark(),
                    ),
                )],
            ))),
            Some(c) => Ok(c),
        };
        let mismatch = || Errand::Say(texts.t(key::ECHO_PRIVATE_MISMATCH));
        match (id, asking) {
            (CAP, Asking::Create { .. }) => match cap_of(text) {
                Ok(cap) => {
                    self.vault = Some(Asking::Create {
                        cap: Some(cap),
                        new: None,
                    });
                    secret(NEW_PASSPHRASE)
                }
                Err(said) => said,
            },
            // AN OPEN VAULT's mapping grows with the passphrase; a locked
            // one grows into its new length at its next opening.
            (CAP, Asking::Grow { .. }) => match cap_of(text) {
                Ok(cap)
                    if vault_status(scene)
                        .ok()
                        .flatten()
                        .is_some_and(|s| s.state == ronoctua_private::State::Open) =>
                {
                    self.vault = Some(Asking::Grow { cap: Some(cap) });
                    secret(PASSPHRASE)
                }
                Ok(cap) => door(Vault::bare(Op::Grow { cap_bytes: cap })),
                Err(said) => said,
            },
            (PASSPHRASE, Asking::Grow { cap: Some(cap) }) => {
                door(Vault::with(Op::Grow { cap_bytes: cap }, Secret::new(text)))
            }
            (NEW_PASSPHRASE, Asking::Create { cap, .. }) => {
                self.vault = Some(Asking::Create {
                    cap,
                    new: Some(Secret::new(text)),
                });
                secret(REPEAT_PASSPHRASE)
            }
            (
                REPEAT_PASSPHRASE,
                Asking::Create {
                    cap: Some(cap),
                    new: Some(new),
                },
            ) => {
                if text == new.expose() {
                    door(Vault::with(Op::Create { cap_bytes: cap }, new))
                } else {
                    mismatch()
                }
            }
            (PASSPHRASE, Asking::Open) => door(Vault::with(Op::Open, Secret::new(text))),
            (PASSPHRASE, Asking::Enrol) => door(Vault::with(Op::EnrollFido2, Secret::new(text))),
            (PASSPHRASE, Asking::Change { .. }) => {
                self.vault = Some(Asking::Change {
                    old: Some(Secret::new(text)),
                    new: None,
                });
                secret(NEW_PASSPHRASE)
            }
            (NEW_PASSPHRASE, Asking::Change { old: Some(old), .. }) => {
                self.vault = Some(Asking::Change {
                    old: Some(old),
                    new: Some(Secret::new(text)),
                });
                secret(REPEAT_PASSPHRASE)
            }
            (
                REPEAT_PASSPHRASE,
                Asking::Change {
                    old: Some(old),
                    new: Some(new),
                },
            ) => {
                if text == new.expose() {
                    door(Vault {
                        op: Op::Passphrase,
                        passphrase: Some(old),
                        new_passphrase: Some(new),
                        refused: None,
                    })
                } else {
                    mismatch()
                }
            }
            _ => Errand::Nothing,
        }
    }

    /// `private-open` (`LEADER s e o`): opens the vault — by the enrolled
    /// key when there is one, else by the passphrase it asks; resumes a
    /// suspended one; shows an open one; and, with none, the section where
    /// it is made. The page need not be open: with no scene read, the status
    /// file is read now (one small read on `/run`).
    pub fn private_open(&mut self, scene: &Scene, texts: &ronoctua_i18n::Texts) -> Errand {
        use crate::vault::Act as V;
        use ronoctua_private::State;
        match vault_status(scene) {
            Err(()) => Errand::Say(texts.t(key::SECURITY_PRIVATE_UNREADABLE)),
            Ok(None) => Errand::Say(texts.t(key::SECURITY_PRIVATE_MISSING)),
            Ok(Some(s)) => match s.state {
                State::Absent => Errand::Show(Section::Private),
                State::Open => Errand::Command("private-show"),
                State::Locked if s.fido2 => self.vault_act(V::OpenKey, scene),
                State::Locked | State::SuspendedInUse => self.vault_act(V::OpenPassphrase, scene),
            },
        }
    }

    /// `private-lock` (`LEADER s e l`): locks an open vault; says why
    /// another is not locked by this.
    pub fn private_lock(&mut self, scene: &Scene, texts: &ronoctua_i18n::Texts) -> Errand {
        use ronoctua_private::{State, Why as V};
        match vault_status(scene) {
            Err(()) => Errand::Say(texts.t(key::SECURITY_PRIVATE_UNREADABLE)),
            Ok(None) => Errand::Say(texts.t(key::SECURITY_PRIVATE_MISSING)),
            Ok(Some(s)) => match s.state {
                State::Open => self.vault_act(crate::vault::Act::Lock, scene),
                State::SuspendedInUse => Errand::Say(texts.t(crate::vault::refusal(V::Busy))),
                State::Absent => Errand::Say(texts.t(crate::vault::refusal(V::Absent))),
                State::Locked => Errand::Say(texts.t(key::ECHO_PRIVATE_ALREADY_LOCKED)),
            },
        }
    }
}

/// The vault's status for a leader command: the scene's when the page shows
/// one that was read, the file's now otherwise.
fn vault_status(scene: &Scene) -> Result<Option<ronoctua_private::Status>, ()> {
    if scene.read {
        if scene.private_unreadable {
            return Err(());
        }
        return Ok(scene.private);
    }
    crate::vault::status_now().map_err(|_| ())
}

/// A scene that holds the vault's part alone — its status and what its image
/// takes (one small read on `/run`, one `stat`), and the room on its disk —
/// read now: what a door that does not show the page asks the vault's
/// questions over (the file manager's window, `m` and `l` on its row).
#[must_use]
pub fn vault_scene() -> Scene {
    let (private, private_unreadable) = match crate::vault::status_now() {
        Ok(status) => (status, false),
        Err(_) => (None, true),
    };
    Scene {
        read: true,
        private,
        private_unreadable,
        private_free: crate::vault::free_bytes(),
        ..Scene::default()
    }
}

/// [`vault_scene`] over a status handed in — the camera's (`--private`),
/// never this machine's.
#[must_use]
pub fn vault_scene_of(status: Option<ronoctua_private::Status>) -> Scene {
    Scene {
        read: true,
        private: status,
        ..Scene::default()
    }
}

// ── anchor: L22 ──
/// ENCRYPT ANYTHING: what "for you" can do now, the three ways in — each a
/// file picker over the page — and what every one of them promises.
fn encrypt(b: &mut Lines<'_>, scene: &Scene) {
    b.heading(Section::Encrypt);
    let ready = matches!(scene.mine, crate::seal::Mine::Ready(_));
    b.pair(
        &b.t(key::SECURITY_ENCRYPT_FOR_YOU),
        &b.t(scene.mine.value()),
        None,
        (!ready).then_some(tokens::color::STATE_WARNING),
    );
    for (label, pick) in [
        (key::SECURITY_ENCRYPT_FILE, crate::seal::Pick::Files),
        (key::SECURITY_ENCRYPT_FOLDER, crate::seal::Pick::Folder),
        (key::SECURITY_ENCRYPT_OPEN, crate::seal::Pick::Open),
    ] {
        b.pair(&b.t(label), "", Some(Act::Seal(pick)), None);
    }
    b.note(&b.t(key::SECURITY_ENCRYPT_NOTE));
}

/// "For whom", about what was chosen: the three ways, "for you" saying what
/// it can do now.
fn seal_question(b: &mut Lines<'_>, paths: &[PathBuf], scene: &Scene) {
    b.section = Section::Encrypt;
    let what = crate::seal::what_of(paths, b.texts);
    b.line(
        &b.texts.t_with(key::SEAL_ASK_MODE, &[("what", &what)]),
        tokens::color::TEXT_HEADING,
    );
    for mode in crate::seal::Mode::ALL {
        let value = match mode {
            crate::seal::Mode::Myself => b.t(scene.mine.value()),
            _ => String::new(),
        };
        b.pair(&b.t(mode.label()), &value, Some(Act::SealMode(mode)), None);
    }
}

/// POST-QUANTUM: every channel this machine has, what it does against a
/// quantum computer, and why — the table `ronoctua_security::quantum` reads
/// from the scene's facts.
fn quantum(b: &mut Lines<'_>, scene: &Scene) {
    use ronoctua_security::quantum::{self, Status};
    b.heading(Section::Quantum);
    b.note(&b.t(key::SECURITY_QUANTUM_NOTE));
    for r in quantum::channels(&scene.quantum) {
        // DANGER IS A COLOUR: classical in the warning's, not known dim.
        let tint = match r.status {
            Status::Classical => Some(tokens::color::STATE_WARNING),
            Status::Unknown => Some(tokens::color::TEXT_DIM),
            Status::PostQuantum | Status::Symmetric => None,
        };
        b.pair(
            &b.t(quantum_channel(r.channel)),
            &b.t(quantum_status(r.status)),
            quantum_act(r.channel),
            tint,
        );
        b.note(&quantum_sentence(&r, b.texts));
    }
}

/// Why a channel has its status, as its sentence: its words as text and its
/// counts as NUMBERS — a plural picks its branch by them, and `"1"` as text
/// is no `[one]` (`numeric-selectors-get-numbers`).
#[must_use]
pub fn quantum_sentence(
    r: &ronoctua_security::quantum::Reading,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let mut args: Vec<(&str, Arg<'_>)> = r
        .args
        .iter()
        .map(|(k, v)| (k.as_str(), Arg::Text(v.as_str())))
        .collect();
    args.extend(r.counts.iter().map(|(k, n)| {
        (
            k.as_str(),
            Arg::Number(i64::try_from(*n).unwrap_or(i64::MAX)),
        )
    }));
    texts.t_args(quantum_why(r.why), &args)
}

/// Where a channel is changed, when this system has a place for it: its
/// row acts (the second click, `RET`) and says so under the pointer. So the
/// section always has rows that act, and `LEADER s e q` lands its cursor —
/// and the page — on the section, never on the next one that acts.
const fn quantum_act(c: ronoctua_security::quantum::Channel) -> Option<Act> {
    use ronoctua_security::quantum::Channel;
    match c {
        Channel::Belongings | Channel::BelongingsTransport => Some(Act::Command("belongings-open")),
        Channel::SealToKey | Channel::SealPassphrase => Some(Act::Seal(crate::seal::Pick::Files)),
        Channel::Private => Some(Act::Command("private-open")),
        Channel::Syncthing => Some(Act::Command("syncthing-open")),
        Channel::Tailscale => Some(Act::Command("network-open")),
        Channel::Tor => Some(Act::Command("network-tor-open")),
        Channel::RootDisk
        | Channel::Https
        | Channel::OpenSsl
        | Channel::Ssh
        | Channel::Chrome
        | Channel::ClipCascade
        | Channel::Sops
        | Channel::Extension
        | Channel::FDroid
        | Channel::NixCache
        | Channel::SecureBoot => None,
    }
}

/// A channel's name.
const fn quantum_channel(c: ronoctua_security::quantum::Channel) -> &'static str {
    use ronoctua_security::quantum::Channel;
    match c {
        Channel::Belongings => key::SECURITY_QUANTUM_CHANNEL_BELONGINGS,
        Channel::BelongingsTransport => key::SECURITY_QUANTUM_CHANNEL_BELONGINGS_TRANSPORT,
        Channel::SealToKey => key::SECURITY_QUANTUM_CHANNEL_SEAL_TO_KEY,
        Channel::SealPassphrase => key::SECURITY_QUANTUM_CHANNEL_SEAL_PASSPHRASE,
        Channel::Private => key::SECURITY_QUANTUM_CHANNEL_PRIVATE,
        Channel::RootDisk => key::SECURITY_QUANTUM_CHANNEL_ROOT_DISK,
        Channel::Https => key::SECURITY_QUANTUM_CHANNEL_HTTPS,
        Channel::OpenSsl => key::SECURITY_QUANTUM_CHANNEL_OPENSSL,
        Channel::Ssh => key::SECURITY_QUANTUM_CHANNEL_SSH,
        Channel::Chrome => key::SECURITY_QUANTUM_CHANNEL_CHROME,
        Channel::Syncthing => key::SECURITY_QUANTUM_CHANNEL_SYNCTHING,
        Channel::Tailscale => key::SECURITY_QUANTUM_CHANNEL_TAILSCALE,
        Channel::Tor => key::SECURITY_QUANTUM_CHANNEL_TOR,
        Channel::ClipCascade => key::SECURITY_QUANTUM_CHANNEL_CLIPCASCADE,
        Channel::Sops => key::SECURITY_QUANTUM_CHANNEL_SOPS,
        Channel::Extension => key::SECURITY_QUANTUM_CHANNEL_EXTENSION,
        Channel::FDroid => key::SECURITY_QUANTUM_CHANNEL_F_DROID,
        Channel::NixCache => key::SECURITY_QUANTUM_CHANNEL_NIX_CACHE,
        Channel::SecureBoot => key::SECURITY_QUANTUM_CHANNEL_SECURE_BOOT,
    }
}

/// A status's word.
const fn quantum_status(s: ronoctua_security::quantum::Status) -> &'static str {
    use ronoctua_security::quantum::Status;
    match s {
        Status::PostQuantum => key::SECURITY_QUANTUM_STATUS_POST_QUANTUM,
        Status::Symmetric => key::SECURITY_QUANTUM_STATUS_SYMMETRIC,
        Status::Classical => key::SECURITY_QUANTUM_STATUS_CLASSICAL,
        Status::Unknown => key::SECURITY_QUANTUM_STATUS_UNKNOWN,
    }
}

/// Why a channel has its status.
const fn quantum_why(w: ronoctua_security::quantum::Why) -> &'static str {
    use ronoctua_security::quantum::Why;
    match w {
        Why::BelongingsPq => key::SECURITY_QUANTUM_WHY_BELONGINGS_PQ,
        Why::BelongingsUnsynced => key::SECURITY_QUANTUM_WHY_BELONGINGS_UNSYNCED,
        Why::BelongingsNew => key::SECURITY_QUANTUM_WHY_BELONGINGS_NEW,
        Why::BelongingsTransport => key::SECURITY_QUANTUM_WHY_BELONGINGS_TRANSPORT,
        Why::SealToKey => key::SECURITY_QUANTUM_WHY_SEAL_TO_KEY,
        Why::SealPassphrase => key::SECURITY_QUANTUM_WHY_SEAL_PASSPHRASE,
        Why::Private => key::SECURITY_QUANTUM_WHY_PRIVATE,
        Why::RootDisk => key::SECURITY_QUANTUM_WHY_ROOT_DISK,
        Why::RootDiskUnread => key::SECURITY_QUANTUM_WHY_ROOT_DISK_UNREAD,
        Why::HttpsAll => key::SECURITY_QUANTUM_WHY_HTTPS_ALL,
        Why::HttpsSome => key::SECURITY_QUANTUM_WHY_HTTPS_SOME,
        Why::HttpsNoneHeard => key::SECURITY_QUANTUM_WHY_HTTPS_NONE_HEARD,
        Why::OpenSsl => key::SECURITY_QUANTUM_WHY_OPENSSL,
        Why::OpenSslOld => key::SECURITY_QUANTUM_WHY_OPENSSL_OLD,
        Why::Ssh => key::SECURITY_QUANTUM_WHY_SSH,
        Why::SshClassical => key::SECURITY_QUANTUM_WHY_SSH_CLASSICAL,
        Why::SshNotFirst => key::SECURITY_QUANTUM_WHY_SSH_NOT_FIRST,
        Why::SshUnread => key::SECURITY_QUANTUM_WHY_SSH_UNREAD,
        Why::Chrome => key::SECURITY_QUANTUM_WHY_CHROME,
        Why::ChromeOld => key::SECURITY_QUANTUM_WHY_CHROME_OLD,
        Why::Syncthing => key::SECURITY_QUANTUM_WHY_SYNCTHING,
        Why::SyncthingPinned => key::SECURITY_QUANTUM_WHY_SYNCTHING_PINNED,
        Why::SyncthingOldGo => key::SECURITY_QUANTUM_WHY_SYNCTHING_OLD_GO,
        Why::Tailscale => key::SECURITY_QUANTUM_WHY_TAILSCALE,
        Why::Tor => key::SECURITY_QUANTUM_WHY_TOR,
        Why::ClipCascade => key::SECURITY_QUANTUM_WHY_CLIPCASCADE,
        Why::Sops => key::SECURITY_QUANTUM_WHY_SOPS,
        Why::Extension => key::SECURITY_QUANTUM_WHY_EXTENSION,
        Why::FDroid => key::SECURITY_QUANTUM_WHY_F_DROID,
        Why::NixCache => key::SECURITY_QUANTUM_WHY_NIX_CACHE,
        Why::SecureBoot => key::SECURITY_QUANTUM_WHY_SECURE_BOOT,
        Why::NotBuilt => key::SECURITY_QUANTUM_WHY_NOT_BUILT,
    }
}

impl View {
    /// The press a key is in this view: the file picker's, while it is up
    /// and claims it — every letter, its own keys — and the table's
    /// otherwise ([`press_of_key`]). The one reader both doors call.
    #[must_use]
    pub fn press_of(&self, key: ronoctua_keymap::Key) -> Option<Press> {
        if self.picking.as_ref().is_some_and(|h| h.picker.claims(key)) {
            return Some(Press::Pick(key));
        }
        press_of_key(key)
    }

    /// The hint line now: the file picker's while it is up, the page's
    /// ([`View::hint`]) otherwise.
    #[must_use]
    pub fn hint_now(
        &self,
        scene: &Scene,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        match self.picking.as_ref() {
            Some(h) => h.picker.hint(texts),
            None => Self::hint(scene, texts),
        }
    }

    /// What was copied, for the file picker.
    pub fn picker_pasted(&mut self, text: &str, texts: &ronoctua_i18n::Texts) -> Errand {
        use crate::file_picker::Hosting;
        let Some(h) = self.picking.as_mut() else {
            return Errand::Nothing;
        };
        match h.pasted(text) {
            Hosting::Say(e) => Errand::Say(texts.t_with(e.key, &e.borrowed())),
            _ => Errand::Nothing,
        }
    }

    /// `LEADER s e c`: the page on Encrypt, and the file picker up for what
    /// to seal — "file picker, then for whom".
    pub fn encrypt_now(&mut self) {
        self.open_on(Section::Encrypt);
        self.pick(crate::seal::Pick::Files);
    }

    /// What `paths` are sealed as — "for whom" asked over the page (the
    /// picker's answer, and a photograph's step).
    pub fn seal_these(&mut self, paths: Vec<PathBuf>) {
        if !paths.is_empty() {
            self.picking = None;
            self.seal_mode = Some(paths);
            self.place(0);
            self.open_at = Some(Target::Top);
        }
    }

    /// The file picker, up over the page for `pick` (a row's `RET`, and a
    /// photograph's step).
    pub fn pick(&mut self, pick: crate::seal::Pick) {
        use crate::file_picker::{EVERY_FILE, Hosted, Mode, Request, Start};
        let (purpose, mode, filters, title) = match pick {
            crate::seal::Pick::Files => (
                "security-encrypt-files",
                Mode::Open { many: true },
                EVERY_FILE,
                key::SECURITY_ENCRYPT_PICK_FILES,
            ),
            crate::seal::Pick::Folder => (
                "security-encrypt-folder",
                Mode::Folder,
                EVERY_FILE,
                key::SECURITY_ENCRYPT_PICK_FOLDER,
            ),
            crate::seal::Pick::Open => (
                "security-encrypt-open",
                Mode::Open { many: false },
                crate::seal::SEALED_FILES,
                key::SECURITY_ENCRYPT_PICK_OPEN,
            ),
        };
        self.seal_mode = None;
        self.picking = Some(Hosted::open(
            Request {
                purpose,
                mode,
                filters,
                start: Start::Remembered,
                addresses: false,
                extras: &[],
                alternate: None,
            },
            pick,
            title,
            None,
        ));
        self.place(0);
    }

    /// A key for the file picker: what it answers is sealed, or opened, and
    /// the page's rows come back.
    fn pick_key(
        &mut self,
        key: ronoctua_keymap::Key,
        scene: &Scene,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
        zone: &ronoctua_files::when::Zone,
    ) -> Errand {
        use crate::file_picker::{Hosting, Picked};
        let Some(h) = self.picking.as_mut() else {
            return Errand::Nothing;
        };
        let errand = match h.key(key) {
            Hosting::Nothing => Errand::Nothing,
            Hosting::Back => {
                self.picking = None;
                self.open_at = Some(Target::Section(Section::Encrypt));
                Errand::Nothing
            }
            Hosting::Say(e) => Errand::Say(texts.t_with(e.key, &e.borrowed())),
            Hosting::Paste => Errand::Paste,
            Hosting::Picked(pick, Picked::Paths { paths, .. }) => {
                self.picking = None;
                return match pick {
                    crate::seal::Pick::Open => match paths.into_iter().next() {
                        Some(path) => {
                            self.open_at = Some(Target::Section(Section::Encrypt));
                            Errand::Sniff(path)
                        }
                        None => Errand::Nothing,
                    },
                    crate::seal::Pick::Files | crate::seal::Pick::Folder => {
                        self.seal_these(paths);
                        Errand::Nothing
                    }
                };
            }
            Hosting::Picked(..) => Errand::Nothing,
        };
        // THE PAGE'S CURSOR ON THE PICKER'S OWN.
        if self.picking.is_some() {
            let rows = self.rows(scene, columns, texts, zone);
            let focused = self
                .picking
                .as_ref()
                .map(|h| h.rows(columns, texts, |p| p.focused))
                .and_then(|f| f.iter().position(|x| *x));
            self.place(focused.unwrap_or(0).min(rows.len().saturating_sub(1)));
        }
        errand
    }

    /// What a seal's step is to this page: its question on the door's line,
    /// the queue, or a sentence.
    /// A seal job of this view's ended while its page is CLOSED: its refusal,
    /// as the sentence the footer says on the minibuffer — a wrong
    /// passphrase's too, whose question again needs the page (the line that
    /// takes the answer is the page's, `ViewState::Region("security")`). `None` for a
    /// job this view did not send, or one that ended with no refusal (which
    /// only lets go of it).
    pub fn seal_ended_unseen(
        &mut self,
        report: &crate::report::SealReport,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<String> {
        crate::seal::job_ended(&mut self.seal_sent, report)?;
        let crate::report::SealReport::Refused { from, why, .. } = report else {
            return None;
        };
        Some(texts.t_with(
            crate::seal::why_key(*why),
            &[("name", &crate::seal::name_of(from))],
        ))
    }

    fn seal_next(&mut self, next: crate::seal::Next, texts: &ronoctua_i18n::Texts) -> Errand {
        use crate::seal::Next;
        match next {
            Next::Nothing => Errand::Nothing,
            Next::Ask {
                id,
                secret,
                wanting,
            } => {
                self.sealing = Some(wanting);
                Errand::Ask {
                    id,
                    secret,
                    initial: None,
                }
            }
            // EVERY JOB HANDED OVER, a seal as well as an open: what the
            // queue refuses of it is this page's to say.
            Next::Do(doing) => {
                self.seal_sent.record(&doing);
                Errand::Seal(doing)
            }
            Next::Say(e) => Errand::Say(texts.t_with(e.key, &e.borrowed())),
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
    crate::chrome::clock_text(secs, zone, crate::when::prefs::current().clock, texts)
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
    /// Run this through the sudo door, as one root gesture (the door's gate's).
    Door(Work, crate::sudo::Gesture),
    /// Register a security key by a touch, as the person.
    Register,
    /// Raise the shields.
    Shields,
    /// Hash a file and ask about it.
    Hash(PathBuf),
    /// Ask about an address.
    Email(String),
    /// Ask about a password.
    Password(Secret),
    /// Read the header of a `.age`.
    Sniff(PathBuf),
    /// Hand a seal to the queue (`crate::seal::carry_out`): the names it
    /// picks are asked of the folder, which is not the loop's to wait on.
    Seal(crate::report::SealDoing),
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
            Job::Door(mut work, mut gesture) => {
                let result = run_work(&mut work, gesture.as_mut(), &mut |word| {
                    send(Came::Step(word.to_owned()));
                });
                send(Came::Door { work, result });
            }
            // THE REGISTRATION, started and waited for on this one thread (its
            // death signal is this thread's), its cancel handed to the view.
            Job::Register => match ronoctua_security::u2f::Enrolment::start() {
                Err(r) => send(Came::Registered(Err(r.why))),
                Ok(enrolment) => {
                    send(Came::Registering(enrolment.cancel()));
                    send(Came::Registered(enrolment.wait().map_err(|r| r.why)));
                }
            },
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
            Job::Sniff(path) => {
                let kind = crate::seal::sniff_now(&path);
                send(Came::Sniffed { path, kind });
            }
            Job::Seal(doing) => send(Came::SealHanded(crate::seal::carry_out(doing))),
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
        // THE VAULT'S STATUS, which its units rewrite — the sleep hook's
        // included, while the page is open.
        parent(ronoctua_private::STATUS_FILE),
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
        .window
        .or_else(ronoctua_security::relax::opened)
        .is_some_and(|w| w.is_open(now_s()));
    Errand::Door(if open {
        Work::RelaxEnd
    } else {
        Work::Relax(RELAX_DEFAULT)
    })
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
            Errand::Door(Work::Firewall(Box::new(settings)))
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
            Errand::Door(Work::Firewall(Box::new(settings)))
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

/// What is left of the passwordless window, as a running clock — `1:02:03`,
/// `14:59` — the films' own arithmetic (`sizes::clock_of`). Pure.
#[must_use]
pub fn left_text(left_s: i64) -> String {
    #[allow(clippy::cast_precision_loss)]
    crate::sizes::clock_of(left_s.max(0) as f64)
}

/// A security key as the page shows it: the start of its key handle — the
/// part that tells one key from another — never the whole credential.
#[must_use]
pub fn key_shown(credential: &str) -> String {
    ronoctua_render::layout::elide_end(
        ronoctua_security::u2f::key_handle(credential),
        tokens::space::KEYS_HANDLECELLS,
    )
}

/// "At most eight keys", said.
fn keys_full(texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        key::SECURITY_KEYS_FULL,
        &[(
            "count",
            Arg::Number(i64::try_from(ronoctua_security::u2f::MOST_KEYS).unwrap_or(i64::MAX)),
        )],
    )
}

/// Root's list of keys made `credentials`, through the door — or, past the
/// most a person may list, said; nothing for none.
fn keys_with(change: KeyChange, credentials: &[String], texts: &ronoctua_i18n::Texts) -> Errand {
    if credentials.is_empty() {
        Errand::Nothing
    } else if credentials.len() > ronoctua_security::u2f::MOST_KEYS {
        Errand::Say(keys_full(texts))
    } else {
        Errand::Door(Work::Keys {
            change,
            credentials: credentials.to_vec(),
            person: None,
        })
    }
}

/// What offering the window for `span` says: its minutes, its hours, or no
/// limit.
#[must_use]
pub fn span_label(span: RelaxSpan, texts: &ronoctua_i18n::Texts) -> String {
    match span.minutes() {
        None => texts.t(key::SECURITY_SUDO_OPEN_NO_LIMIT),
        Some(minutes) if minutes > 60 => texts.t_args(
            key::SECURITY_SUDO_OPEN_FOR_HOURS,
            &[("hours", Arg::Number(i64::from(minutes / 60)))],
        ),
        Some(minutes) => texts.t_args(
            key::SECURITY_SUDO_OPEN_FOR,
            &[("minutes", Arg::Number(i64::from(minutes)))],
        ),
    }
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
mod seal_tests;
#[cfg(test)]
mod vault_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    /// FAILS IF: an open this page sent and the queue refused is said nowhere
    /// once the page closed — a wrong passphrase included — or a job another
    /// view sent, or one that ended with no refusal, is said here.
    #[test]
    fn a_refusal_after_the_page_closed_is_still_said() {
        use crate::report::{SealDoing, SealReport, With};
        let t = texts("en-GB");
        let mut v = View::default();
        let mine = std::path::PathBuf::from("/home/a/notes.txt.age");
        v.seal_sent.record(&SealDoing::Open {
            path: mine.clone(),
            with: With::Passphrase(ronoctua_consent::keys::Secret::new("x")),
        });
        let refused = |from: &std::path::Path| SealReport::Refused {
            from: from.to_path_buf(),
            opening: Some(crate::report::Opening::Passphrase),
            why: ronoctua_pq::Why::WrongPassphrase,
        };
        assert_eq!(
            v.seal_ended_unseen(&refused(std::path::Path::new("/elsewhere.age")), &t),
            None,
            "another view's job"
        );
        let said = v.seal_ended_unseen(&refused(&mine), &t).expect("said");
        assert!(said.contains("notes.txt.age"), "{said}");
        assert_eq!(v.seal_ended_unseen(&refused(&mine), &t), None, "said twice");
        v.seal_sent.record(&SealDoing::Open {
            path: mine.clone(),
            with: With::Passphrase(ronoctua_consent::keys::Secret::new("x")),
        });
        let ended = SealReport::Ended {
            from: mine.clone(),
            opens: true,
        };
        assert_eq!(v.seal_ended_unseen(&ended, &t), None);
        assert_eq!(v.seal_ended_unseen(&refused(&mine), &t), None, "let go of");
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
        off.window = Some(Opened::of(RelaxSpan::Fifteen, now_s() - 300));
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
            Errand::Door(Work::Enrol(Finger::LeftMiddle)) => {}
            other => panic!("enrol did {other:?}"),
        }
    }

    /// FAILS IF: a change of the firewall stops being the WHOLE settings
    /// through the door, or the shields' way up stops being free.
    #[test]
    fn loosening_goes_through_the_door_and_the_shields_go_up_free() {
        let s = scene();
        match firewall_errand(&s, &texts("en-GB")) {
            Errand::Door(Work::Firewall(settings)) => {
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
            Errand::Door(Work::Firewall(settings)) => assert!(!settings.shields),
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: a door asks the password itself again — a work refused for
    /// want of one is a sentence, never a question on the door's line — or a
    /// work does not reach the sudo door as the gesture of the screen that
    /// pressed, with its verb.
    #[test]
    fn no_door_asks_the_password_itself_and_root_is_asked_in_the_bar() {
        let t = texts("en-GB");
        let mut v = View::fresh();
        let mut s = scene();
        for why in [
            Why::NeedsPassword,
            Why::WrongPassword,
            Why::NoFooter,
            Why::Cancelled,
        ] {
            let out = v.came(
                Came::Door {
                    work: Work::Relax(RelaxSpan::Thirty),
                    result: Err(why),
                },
                &mut s,
                &t,
            );
            assert!(
                matches!(out.errand, Errand::Nothing),
                "{why:?}: {:?}",
                out.errand
            );
            assert_eq!(out.say, Some(t.t(crate::security_words::refusal(why))));
        }
        let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let gate = crate::sudo::Gate::fake(std::sync::Arc::clone(&log), Ok(()));
        let mut gesture = gate.gesture(crate::sudo::Asker::Security);
        let mut work = Work::Again(RelaxSpan::Day);
        run_work(&mut work, gesture.as_mut(), &mut |_| {}).unwrap();
        assert_eq!(
            *log.lock().unwrap(),
            [(
                crate::sudo::Asker::Security,
                RootVerb::Relax {
                    span: RelaxSpan::Day
                }
            )]
        );
        let refusing = crate::sudo::Gate::fake(log, Err(Why::Cancelled));
        let mut work = Work::RelaxEnd;
        assert_eq!(
            run_work(
                &mut work,
                refusing.gesture(crate::sudo::Asker::Security).as_mut(),
                &mut |_| {}
            ),
            Err(Why::Cancelled)
        );
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
            crate::security_words::section_of_mark(WATCH, &segment("sudo-open")),
            Some(Section::Admin.word())
        );
        for mark in ["firewall-failed", "firewall-off", "shields", "refused"] {
            assert_eq!(
                crate::security_words::section_of_mark(WATCH, &segment(mark)),
                Some(Section::Firewall.word()),
                "{mark}"
            );
        }
        assert_eq!(
            crate::security_words::section_of_mark("fixture", "brain:fixture/full"),
            None
        );
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
        let Errand::Door(Work::Firewall(after)) = v.press(Press::Enter, &s, 100, &t, &utc()) else {
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
        let is_write = |e: &Errand| matches!(e, Errand::Door(Work::Firewall(_)));
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
            Errand::Door(Work::Firewall(after)) => {
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
            Errand::Door(Work::Firewall(after)) => {
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

    const KEY_A: &str = "owBYtXj1Pq+Zb5c1Kd/AbQ==,04d6f1aa,es256,+presence";
    const KEY_B: &str = "Qm9vay1zaGVsZg==,9f00ee12,eddsa,+presence+pin";

    fn with_keys(keys: &[&str], person: &[&str], away: &[&str]) -> Scene {
        let owned = |l: &[&str]| l.iter().map(|k| (*k).to_owned()).collect::<Vec<_>>();
        Scene {
            keys_answer: true,
            keys: owned(keys),
            person_keys: owned(person),
            keys_away: owned(away),
            ..scene()
        }
    }

    /// FAILS IF: a touch's credential does not go to root's list WITH the
    /// keys there and to the person's own list with theirs, a ninth key is
    /// sent, a registration is not let go by `Esc` before the page closes,
    /// or a refusal of the registration is not said.
    #[test]
    fn a_registered_key_joins_both_lists_and_esc_lets_it_go() {
        let t = texts("en-GB");
        let mut s = with_keys(&[KEY_A], &[KEY_A], &[]);
        let mut v = View::fresh();
        assert!(matches!(v.act(Act::KeyRegister, &s, &t), Errand::Register));
        assert!(
            matches!(v.act(Act::KeyRegister, &s, &t), Errand::Nothing),
            "twice"
        );
        let rows = v.rows(&s, 100, &t, &utc());
        assert!(rows.iter().any(|r| r.act == Some(Act::KeyStop)));
        assert!(!rows.iter().any(|r| r.act == Some(Act::KeyRegister)));
        let out = v.came(Came::Registered(Ok(KEY_B.to_owned())), &mut s, &t);
        match out.errand {
            Errand::Door(Work::Keys {
                change: KeyChange::Register,
                credentials,
                person,
            }) => {
                assert_eq!(credentials, [KEY_A, KEY_B]);
                assert_eq!(person, Some(vec![KEY_A.to_owned(), KEY_B.to_owned()]));
            }
            other => panic!("{other:?}"),
        }
        // `Esc` LETS A WAITING REGISTRATION GO, and only the next closes.
        let _ = v.act(Act::KeyRegister, &s, &t);
        assert!(matches!(
            v.press(Press::Escape, &s, 100, &t, &utc()),
            Errand::Nothing
        ));
        assert!(matches!(
            v.press(Press::Escape, &s, 100, &t, &utc()),
            Errand::Close
        ));
        let out = v.came(Came::Registered(Err(Why::NoKey)), &mut s, &t);
        assert_eq!(
            out.say,
            Some(t.t(crate::security_words::refusal(Why::NoKey)))
        );
        // FULL: never a ninth.
        let full: Vec<String> = (0..ronoctua_security::u2f::MOST_KEYS)
            .map(|n| format!("h{n},pk,es256,+presence"))
            .collect();
        let mut s = Scene {
            keys: full,
            ..with_keys(&[], &[], &[])
        };
        assert!(matches!(v.act(Act::KeyRegister, &s, &t), Errand::Say(_)));
        let out = v.came(Came::Registered(Ok(KEY_B.to_owned())), &mut s, &t);
        assert!(matches!(out.errand, Errand::Say(_)), "{:?}", out.errand);
    }

    /// FAILS IF: the cursor keeps an INDEX — "register a key" becomes the
    /// line that asks for the touch and the cursor falls to the page's first
    /// row, scrolling the touch's line out of sight, instead of going to the
    /// row that stops it; the end of the registration, stopped or answered,
    /// leaves it anywhere but on "register a key"; or a line said above it
    /// (the sudo work in flight) moves it off the row that started the work.
    #[test]
    fn the_cursor_keeps_its_row_when_the_rows_move() {
        let t = texts("en-GB");
        let mut s = with_keys(&[KEY_A], &[KEY_A, KEY_B], &[KEY_B]);
        let mut v = View::fresh();
        let settled = |v: &mut View, s: &Scene| {
            let rows = v.rows(s, 100, &t, &utc());
            let _ = v.settle_on(&rows, s);
            rows
        };
        let under = |v: &View, rows: &[Row]| rows.get(v.cursor()).and_then(|r| r.act.clone());
        let row_of = |rows: &[Row], act: &Act| {
            rows.iter()
                .position(|r| r.act.as_ref() == Some(act))
                .unwrap_or_else(|| panic!("no row does {act:?}: {}", drawn(rows)))
        };
        let enter = |v: &mut View, s: &Scene| v.press(Press::Enter, s, 100, &t, &utc());
        // REGISTER: the cursor goes to "stop", under the touch's line.
        let rows = settled(&mut v, &s);
        assert!(!v.clicked(row_of(&rows, &Act::KeyRegister), &rows));
        assert!(matches!(enter(&mut v, &s), Errand::Register));
        let rows = settled(&mut v, &s);
        assert_eq!(under(&v, &rows), Some(Act::KeyStop), "{}", drawn(&rows));
        // STOPPED: back on "register".
        assert!(matches!(enter(&mut v, &s), Errand::Nothing));
        let rows = settled(&mut v, &s);
        assert_eq!(under(&v, &rows), Some(Act::KeyRegister));
        // OVER BY ITS THREAD'S ANSWER: the same.
        assert!(matches!(enter(&mut v, &s), Errand::Register));
        let _ = settled(&mut v, &s);
        let _ = v.came(Came::Registered(Err(Why::Cancelled)), &mut s, &t);
        let rows = settled(&mut v, &s);
        assert_eq!(under(&v, &rows), Some(Act::KeyRegister));
        // A LINE SAID ABOVE IT: "bring" sends a work, whose line goes to the
        // administrator's section, above the keys.
        assert!(!v.clicked(row_of(&rows, &Act::KeysBring), &rows));
        match enter(&mut v, &s) {
            Errand::Door(work) => v.sent(&work),
            other => panic!("{other:?}"),
        }
        let rows = settled(&mut v, &s);
        assert_eq!(under(&v, &rows), Some(Act::KeysBring), "{}", drawn(&rows));
    }

    /// FAILS IF: removing a key does not ask first, "no" first and the cursor
    /// on it, or takes it out of one list only; off takes root's line and
    /// leaves the person's list; on and "bring" do not take the keys of the
    /// person's other machines.
    #[test]
    fn keys_are_removed_after_a_question_turned_off_and_brought() {
        let t = texts("pt-BR");
        let s = with_keys(&[KEY_A, KEY_B], &[KEY_A, KEY_B], &[]);
        let mut v = View::fresh();
        assert!(matches!(
            v.act(Act::Key(KEY_A.to_owned()), &s, &t),
            Errand::Nothing
        ));
        let rows = v.rows(&s, 100, &t, &utc());
        let first = rows.iter().position(|r| !r.heading).unwrap();
        assert_eq!(rows[first].act, Some(Act::Answer(false)), "no first");
        assert_eq!(v.cursor(), 0);
        match v.act(Act::Answer(true), &s, &t) {
            Errand::Door(Work::Keys {
                change: KeyChange::Remove,
                credentials,
                person,
            }) => {
                assert_eq!(credentials, [KEY_B]);
                assert_eq!(person, Some(vec![KEY_B.to_owned()]));
            }
            other => panic!("{other:?}"),
        }
        match v.act(Act::KeysSwitch, &s, &t) {
            Errand::Door(Work::Keys {
                change: KeyChange::Off,
                credentials,
                person: None,
            }) => assert!(credentials.is_empty()),
            other => panic!("{other:?}"),
        }
        let off = with_keys(&[], &[KEY_A], &[KEY_A]);
        match v.act(Act::KeysSwitch, &off, &t) {
            Errand::Door(Work::Keys {
                change: KeyChange::On,
                credentials,
                person: None,
            }) => assert_eq!(credentials, [KEY_A]),
            other => panic!("{other:?}"),
        }
        let some_away = with_keys(&[KEY_A], &[KEY_A], &[KEY_B]);
        match v.act(Act::KeysBring, &some_away, &t) {
            Errand::Door(Work::Keys {
                change: KeyChange::Bring,
                credentials,
                person: None,
            }) => assert_eq!(credentials, [KEY_A, KEY_B]),
            other => panic!("{other:?}"),
        }
        // NOTHING HERE AND NOTHING AWAY: off, and nothing to switch.
        let none = with_keys(&[], &[], &[]);
        let rows = v.rows(&none, 100, &t, &utc());
        assert!(!rows.iter().any(|r| r.act == Some(Act::KeysSwitch)));
        assert!(rows.iter().any(|r| r.act == Some(Act::KeyRegister)));
        // A MACHINE WHERE NO KEY ANSWERS SUDO says so, and offers nothing.
        let not_here = Scene {
            keys_answer: false,
            ..with_keys(&[KEY_A], &[KEY_A], &[])
        };
        let rows = v.rows(&not_here, 100, &t, &utc());
        assert!(
            !rows
                .iter()
                .any(|r| r.section == Section::Keys && r.act.is_some())
        );
    }

    /// FAILS IF: an open window with an end does not count down, offer to
    /// start the count again and say the time left as a running clock; one
    /// with no limit counts or offers it; or the count runs with nothing
    /// open.
    #[test]
    fn a_timed_window_counts_down_and_starts_again() {
        let t = texts("en-GB");
        let now = now_s();
        let timed = Scene {
            window: Some(Opened::of(RelaxSpan::Day, now)),
            ..scene()
        };
        let mut v = View::fresh();
        let rows = v.rows(&timed, 100, &t, &utc());
        assert!(View::counts_down(&timed));
        assert!(rows.iter().any(|r| r.act == Some(Act::RelaxAgain)));
        let left = t.t(key::SECURITY_SUDO_LEFT);
        let row = rows
            .iter()
            .find(|r| r.fields.iter().any(|f| f.text == left))
            .expect("the time left");
        assert!(
            row.fields
                .iter()
                .any(|f| f.text.starts_with("23:59") || f.text.starts_with("24:00")),
            "{:?}",
            row.fields
        );
        assert!(matches!(
            v.act(Act::RelaxAgain, &timed, &t),
            Errand::Door(Work::Again(RelaxSpan::Day))
        ));
        let forever = Scene {
            window: Some(Opened::of(RelaxSpan::NoLimit, now)),
            ..scene()
        };
        assert!(!View::counts_down(&forever));
        let rows = v.rows(&forever, 100, &t, &utc());
        assert!(!rows.iter().any(|r| r.act == Some(Act::RelaxAgain)));
        assert!(!rows.iter().any(|r| r.fields.iter().any(|f| f.text == left)));
        assert!(matches!(
            v.act(Act::RelaxAgain, &forever, &t),
            Errand::Nothing
        ));
        assert!(!View::counts_down(&scene()));
        assert_eq!(left_text(3723), "1:02:03");
        assert_eq!(left_text(-5), "0:00");
        // THE COUNT IS ON SCREEN only while the administrator's rows are.
        let rows = v.rows(&timed, 100, &t, &utc());
        let admin = rows
            .iter()
            .position(|r| r.section == Section::Admin)
            .unwrap();
        assert!(View::count_on_screen(&rows, admin..admin + 1));
        assert!(!View::count_on_screen(&rows, 0..1));
    }

    /// FAILS IF: a command a key or a row runs is not in the registry.
    #[test]
    fn every_command_of_the_view_is_registered() {
        for id in reached() {
            assert!(
                KEYS.commands.iter().any(|c| c.id == id),
                "`{id}` is not registered"
            );
        }
        for (_, id) in OF_THE_VIEW {
            assert!(press_of(id).is_some(), "`{id}` is no press");
        }
        for section in Section::ALL {
            assert_eq!(Section::of_word(section.word()), Some(section));
        }
    }
}
