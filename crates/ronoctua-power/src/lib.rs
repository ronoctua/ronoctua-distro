//! The one boundary with UPower, power-profiles-daemon and logind — the
//! battery, the charger, the power profile, and sleep.
//!
//! # What is here, and what is not
//!
//! `upower.rs`, `profiles.rs` and `login.rs` are the only modules that know
//! D-Bus exists; `sysfs.rs` is the one thing read and written on `/sys`
//! (through the supply walk `ronoctua-memory` owns); `prefs.rs` is the one
//! file. Everything in THIS file is a pure function over plain structs — which
//! rung a percentage is, whether the modeline shows anything, which profile is
//! the top one a daemon offers, what counts as a transition worth a
//! notification — and its tests run against property maps copied off a real
//! machine, with no bus, no battery and no root. It is the shape
//! `ronoctua-disks` has, and for the same reason.
//!
//! # Whole snapshots, on threads
//!
//! The footer receives a [`Snapshot`] — everything, every time — on a calloop
//! channel, and never touches the bus itself. Three threads feed it: one
//! follows UPower's signals (and, under the same path namespace,
//! power-profiles-daemon's), one follows logind's, and one carries out
//! [`Order`]s and re-reads after each. The inhibitor locks live on that third
//! thread: an inhibitor IS a file descriptor, and the thread that took it is
//! the one that keeps it.
//!
//! # Nothing here asks
//!
//! Every logind call goes with `interactive = false`; every profile write is a
//! property set. There is no polkit agent in this session
//! (`nix/modules/disks.nix` says why), so a call that could ask would hang
//! instead. Upstream's own defaults already answer `yes` to an active session
//! for every action this crate makes, and `nix/modules/power.nix` writes no
//! rule of its own — a check keeps it that way.

#![forbid(unsafe_code)]

pub mod login;
pub mod prefs;
pub mod profiles;
pub mod sysfs;
pub mod upower;

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context as _, Result};

use upower::Props;

/// From this percentage up the battery glyph is not drawn: 99 and 100 are
/// "full" to a person, and a glyph that never leaves says nothing.
pub const HIDE_FROM: u8 = 99;

/// Up to this percentage the number is drawn beside the glyph.
pub const PERCENT_UNTIL: u8 = 30;

/// Up to this percentage the segment takes the danger colour.
pub const DANGER_UNTIL: u8 = 15;

// ---------------------------------------------------------------------------
// UPower's vocabulary
// ---------------------------------------------------------------------------

/// What a UPower device is — `Type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Kind {
    #[default]
    /// The daemon said something this does not know. Not an error.
    Unknown,
    /// A charger — mains, USB-C, a dock.
    LinePower,
    /// A battery that feeds the machine.
    Battery,
    /// An uninterruptible supply.
    Ups,
    /// UPower's `Monitor`: a peripheral with a battery of its own.
    Monitor,
    /// UPower's `Mouse`: a peripheral with a battery of its own.
    Mouse,
    /// UPower's `Keyboard`: a peripheral with a battery of its own.
    Keyboard,
    /// UPower's `Pda`: a peripheral with a battery of its own.
    Pda,
    /// UPower's `Phone`: a peripheral with a battery of its own.
    Phone,
    /// UPower's `MediaPlayer`: a peripheral with a battery of its own.
    MediaPlayer,
    /// UPower's `Tablet`: a peripheral with a battery of its own.
    Tablet,
    /// UPower's `Computer`: a peripheral with a battery of its own.
    Computer,
    /// UPower's `GamingInput`: a peripheral with a battery of its own.
    GamingInput,
    /// UPower's `Pen`: a peripheral with a battery of its own.
    Pen,
    /// UPower's `Touchpad`: a peripheral with a battery of its own.
    Touchpad,
    /// UPower's `Modem`: a peripheral with a battery of its own.
    Modem,
    /// UPower's `Network`: a peripheral with a battery of its own.
    Network,
    /// UPower's `Headset`: a peripheral with a battery of its own.
    Headset,
    /// UPower's `Speakers`: a peripheral with a battery of its own.
    Speakers,
    /// UPower's `Headphones`: a peripheral with a battery of its own.
    Headphones,
    /// UPower's `Video`: a peripheral with a battery of its own.
    Video,
    /// UPower's `OtherAudio`: a peripheral with a battery of its own.
    OtherAudio,
    /// UPower's `RemoteControl`: a peripheral with a battery of its own.
    RemoteControl,
    /// UPower's `Printer`: a peripheral with a battery of its own.
    Printer,
    /// UPower's `Scanner`: a peripheral with a battery of its own.
    Scanner,
    /// UPower's `Camera`: a peripheral with a battery of its own.
    Camera,
    /// UPower's `Wearable`: a peripheral with a battery of its own.
    Wearable,
    /// UPower's `Toy`: a peripheral with a battery of its own.
    Toy,
    /// UPower's `BluetoothGeneric`: a peripheral with a battery of its own.
    BluetoothGeneric,
}

impl Kind {
    /// Every kind, for whoever has to prove each one has a word.
    ///
    /// FROM THE NUMBERS UPower USES, because that is where they come from:
    /// a list written out by hand beside `from_u32` is a list that falls
    /// behind it.
    #[must_use]
    pub fn all() -> Vec<Self> {
        let mut out: Vec<Self> = (0..=28).map(Self::from_u32).collect();
        out.dedup();
        out
    }

    /// UPower's number.
    #[must_use]
    pub const fn from_u32(n: u32) -> Self {
        match n {
            1 => Self::LinePower,
            2 => Self::Battery,
            3 => Self::Ups,
            4 => Self::Monitor,
            5 => Self::Mouse,
            6 => Self::Keyboard,
            7 => Self::Pda,
            8 => Self::Phone,
            9 => Self::MediaPlayer,
            10 => Self::Tablet,
            11 => Self::Computer,
            12 => Self::GamingInput,
            13 => Self::Pen,
            14 => Self::Touchpad,
            15 => Self::Modem,
            16 => Self::Network,
            17 => Self::Headset,
            18 => Self::Speakers,
            19 => Self::Headphones,
            20 => Self::Video,
            21 => Self::OtherAudio,
            22 => Self::RemoteControl,
            23 => Self::Printer,
            24 => Self::Scanner,
            25 => Self::Camera,
            26 => Self::Wearable,
            27 => Self::Toy,
            28 => Self::BluetoothGeneric,
            _ => Self::Unknown,
        }
    }

    /// The id the text keys use (`power-kind-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::LinePower => "line-power",
            Self::Battery => "battery",
            Self::Ups => "ups",
            Self::Monitor => "monitor",
            Self::Mouse => "mouse",
            Self::Keyboard => "keyboard",
            Self::Pda => "pda",
            Self::Phone => "phone",
            Self::MediaPlayer => "media-player",
            Self::Tablet => "tablet",
            Self::Computer => "computer",
            Self::GamingInput => "gaming-input",
            Self::Pen => "pen",
            Self::Touchpad => "touchpad",
            Self::Modem => "modem",
            Self::Network => "network",
            Self::Headset => "headset",
            Self::Speakers => "speakers",
            Self::Headphones => "headphones",
            Self::Video => "video",
            Self::OtherAudio => "other-audio",
            Self::RemoteControl => "remote-control",
            Self::Printer => "printer",
            Self::Scanner => "scanner",
            Self::Camera => "camera",
            Self::Wearable => "wearable",
            Self::Toy => "toy",
            Self::BluetoothGeneric => "bluetooth",
        }
    }
}

/// What the battery is doing — `State`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum State {
    #[default]
    /// The daemon said something this does not know. Not an error.
    Unknown,
    /// Energy going in.
    Charging,
    /// Energy going out.
    Discharging,
    /// Nothing left.
    Empty,
    /// Full — or held at a threshold, which the daemon reports the same way.
    FullyCharged,
    /// On the charger and not taking energy yet: a threshold, or a firmware pause.
    PendingCharge,
    /// On the charger and about to be drawn from.
    PendingDischarge,
}

impl State {
    /// Every one, for whoever has to prove each has a word — from the
    /// numbers UPower uses, so the list cannot fall behind `from_u32`.
    #[must_use]
    pub fn all() -> Vec<Self> {
        let mut out: Vec<Self> = (0..=6).map(Self::from_u32).collect();
        out.dedup();
        out
    }

    /// UPower's number.
    #[must_use]
    pub const fn from_u32(n: u32) -> Self {
        match n {
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::Empty,
            4 => Self::FullyCharged,
            5 => Self::PendingCharge,
            6 => Self::PendingDischarge,
            _ => Self::Unknown,
        }
    }

    /// The id the text keys use (`power-state-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::Empty => "empty",
            Self::FullyCharged => "full",
            Self::PendingCharge => "pending-charge",
            Self::PendingDischarge => "pending-discharge",
        }
    }

    /// Is energy going IN?
    #[must_use]
    pub const fn is_charging(self) -> bool {
        matches!(self, Self::Charging | Self::PendingCharge)
    }
}

/// How worried the daemon is — `WarningLevel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Warning {
    #[default]
    /// The daemon said something this does not know. Not an error.
    Unknown,
    /// Nothing to worry about.
    None,
    /// Energy going out.
    Discharging,
    /// The daemon's `percentageLow` line crossed.
    Low,
    /// The daemon's `percentageCritical` line crossed.
    Critical,
    /// The action level: the daemon is about to act on its own.
    Action,
}

impl Warning {
    /// Every one, from the numbers UPower uses, so the list cannot fall
    /// behind `from_u32`.
    ///
    /// # Why it is NOT "so that each has a word"
    ///
    /// It said that, and it is the one of these four enumerations that has no
    /// words: nothing draws a warning level. What draws is the PERCENTAGE and
    /// the danger colour, and `power.rs` says at length why — the level is
    /// the daemon's threshold and the colour is the owner's, and showing both
    /// would be two answers to "how bad is it". What this list is for is
    /// `alarms`, and the test that no enumeration lists a value twice.
    #[must_use]
    pub fn all() -> Vec<Self> {
        let mut out: Vec<Self> = (0..=5).map(Self::from_u32).collect();
        out.dedup();
        out
    }

    /// UPower's number.
    #[must_use]
    pub const fn from_u32(n: u32) -> Self {
        match n {
            1 => Self::None,
            2 => Self::Discharging,
            3 => Self::Low,
            4 => Self::Critical,
            5 => Self::Action,
            _ => Self::Unknown,
        }
    }
}

/// The chemistry — `Technology`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Technology {
    #[default]
    /// The daemon said something this does not know. Not an error.
    Unknown,
    /// Lithium-ion.
    LithiumIon,
    /// Lithium-polymer.
    LithiumPolymer,
    /// Lithium iron phosphate.
    LithiumIronPhosphate,
    /// Lead-acid.
    LeadAcid,
    /// Nickel-cadmium.
    NickelCadmium,
    /// Nickel-metal hydride.
    NickelMetalHydride,
}

impl Technology {
    /// Every one, for whoever has to prove each has a word — from the
    /// numbers UPower uses, so the list cannot fall behind `from_u32`.
    #[must_use]
    pub fn all() -> Vec<Self> {
        // 0..=6, and the six is not a typo: `from_u32` maps anything it does
        // not know to `Unknown`, so asking one past the last valid number
        // returns `Unknown` a SECOND time — and `dedup` removes only
        // CONSECUTIVE duplicates, so both survived. `Kind`, `State` and
        // `Warning` all stop at their own last number; this one went one
        // past, and the `dedup` beside it shows uniqueness was the intent.
        let mut out: Vec<Self> = (0..=6).map(Self::from_u32).collect();
        out.dedup();
        out
    }

    /// UPower's number.
    #[must_use]
    pub const fn from_u32(n: u32) -> Self {
        match n {
            1 => Self::LithiumIon,
            2 => Self::LithiumPolymer,
            3 => Self::LithiumIronPhosphate,
            4 => Self::LeadAcid,
            5 => Self::NickelCadmium,
            6 => Self::NickelMetalHydride,
            _ => Self::Unknown,
        }
    }

    /// The id the text keys use (`power-technology-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::LithiumIon => "lithium-ion",
            Self::LithiumPolymer => "lithium-polymer",
            Self::LithiumIronPhosphate => "lithium-iron-phosphate",
            Self::LeadAcid => "lead-acid",
            Self::NickelCadmium => "nickel-cadmium",
            Self::NickelMetalHydride => "nickel-metal-hydride",
        }
    }
}

// ---------------------------------------------------------------------------
// The model
// ---------------------------------------------------------------------------

/// One battery, as UPower reports it. Serial numbers are never read.
#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    /// The object path — the key, never drawn.
    pub at: String,
    /// The kernel's name for it (`BAT0`), through UPower.
    pub native_path: String,
    /// The maker, as the firmware spells it.
    pub vendor: String,
    /// The model, as the firmware spells it.
    pub model: String,
    /// What it is doing.
    pub state: State,
    /// 0–100.
    pub percentage: f64,
    /// The daemon's own worry level.
    pub warning: Warning,
    /// `None` when the daemon does not know (it reports zero).
    pub time_to_empty: Option<Duration>,
    /// How long until full, when the daemon knows.
    pub time_to_full: Option<Duration>,
    /// Watt-hours in, now.
    pub energy_wh: f64,
    /// Watt-hours a full charge holds today.
    pub energy_full_wh: f64,
    /// Watt-hours a full charge held when new.
    pub energy_full_design_wh: f64,
    /// Watts flowing, in or out.
    pub rate_w: f64,
    /// Volts, now.
    pub voltage_v: f64,
    /// `None` when the daemon reports zero, which is "not known".
    pub temperature_c: Option<f64>,
    /// The daemon's own health estimate, 0–100.
    ///
    /// It is NOT [`Battery::health`], which is this crate's own arithmetic
    /// over `EnergyFull` against `EnergyFullDesign`. UPower's number comes
    /// from the firmware where the firmware reports one and from the same
    /// division where it does not, so the two disagree on some machines —
    /// and when they do, that disagreement is the interesting fact. The view
    /// draws both.
    pub capacity_pct: f64,
    /// `None` when the daemon reports zero.
    pub cycles: Option<u32>,
    /// The chemistry.
    pub technology: Technology,
    /// Is there a battery in the bay at all?
    pub present: bool,
    /// Does it feed the machine (as against a mouse's)?
    pub power_supply: bool,
}

impl Battery {
    /// From one `GetAll` of `org.freedesktop.UPower.Device`.
    #[must_use]
    pub fn from_props(at: &str, p: &Props) -> Self {
        let seconds = |v: Option<&zbus::zvariant::OwnedValue>| -> Option<Duration> {
            let s = upower::i64_of(v);
            (s > 0).then(|| Duration::from_secs(s.unsigned_abs()))
        };
        let temperature = upower::f64_of(p.get("Temperature"));
        let cycles = upower::i32_of(p.get("ChargeCycles"));
        Self {
            at: at.to_owned(),
            native_path: upower::text_of(p.get("NativePath")),
            vendor: upower::text_of(p.get("Vendor")),
            model: upower::text_of(p.get("Model")),
            state: State::from_u32(upower::u32_of(p.get("State"))),
            percentage: upower::f64_of(p.get("Percentage")),
            warning: Warning::from_u32(upower::u32_of(p.get("WarningLevel"))),
            time_to_empty: seconds(p.get("TimeToEmpty")),
            time_to_full: seconds(p.get("TimeToFull")),
            energy_wh: upower::f64_of(p.get("Energy")),
            energy_full_wh: upower::f64_of(p.get("EnergyFull")),
            energy_full_design_wh: upower::f64_of(p.get("EnergyFullDesign")),
            rate_w: upower::f64_of(p.get("EnergyRate")),
            voltage_v: upower::f64_of(p.get("Voltage")),
            temperature_c: (temperature > 0.0).then_some(temperature),
            capacity_pct: upower::f64_of(p.get("Capacity")),
            cycles: u32::try_from(cycles).ok().filter(|c| *c > 0),
            technology: Technology::from_u32(upower::u32_of(p.get("Technology"))),
            present: upower::flag_of(p.get("IsPresent")),
            power_supply: upower::flag_of(p.get("PowerSupply")),
        }
    }

    /// The percentage as a whole number, the way a screen shows it.
    #[must_use]
    pub fn percent(&self) -> u8 {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let p = self.percentage.round().clamp(0.0, 100.0) as u8;
        p
    }

    /// Health: what a full charge holds today against what it held when new,
    /// as a percentage. `None` when either number is unknown.
    #[must_use]
    pub fn health(&self) -> Option<u8> {
        if self.energy_full_wh <= 0.0 || self.energy_full_design_wh <= 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let h = (self.energy_full_wh / self.energy_full_design_wh * 100.0)
            .round()
            .clamp(0.0, 100.0) as u8;
        Some(h)
    }
}

/// The charger.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Adapter {
    /// The object path — the key, never drawn.
    pub at: String,
    /// The kernel's name for it, through UPower.
    pub native_path: String,
    /// Plugged in and delivering.
    pub online: bool,
}

/// A device with a battery of its own that does not feed the machine — a
/// mouse, a headset, a keyboard.
#[derive(Debug, Clone, PartialEq)]
pub struct Peripheral {
    /// The object path — the key, never drawn.
    pub at: String,
    /// What it is.
    pub kind: Kind,
    /// The model, as the firmware spells it.
    pub model: String,
    /// Its charge, when it reports one.
    pub percentage: Option<f64>,
    /// What it is doing.
    pub state: State,
}

/// The lid, as UPower sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Lid {
    /// Is there a battery in the bay at all?
    pub present: bool,
    /// Is it closed?
    pub closed: bool,
}

/// The three profiles power-profiles-daemon knows, in ascending order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Profile {
    /// Slow and cool.
    PowerSaver,
    /// The daemon's middle.
    Balanced,
    /// Everything the platform offers — the distro's default.
    Performance,
}

impl Profile {
    /// Every one, ascending.
    pub const ALL: [Self; 3] = [Self::PowerSaver, Self::Balanced, Self::Performance];

    /// The daemon's own spelling, which is also the text key's tail
    /// (`power-profile-<id>`) and `power.json`'s value.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::PowerSaver => "power-saver",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }

    /// The reverse.
    #[must_use]
    pub fn by_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id() == id)
    }
}

/// Somebody holding the profile somewhere — a game holding `performance`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hold {
    /// Who holds it — an application id.
    pub who: String,
    /// The profile held, when it is one of the three.
    pub profile: Option<Profile>,
    /// Their reason, in their words.
    pub why: String,
}

/// What power-profiles-daemon says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profiles {
    /// The one in force.
    pub active: Profile,
    /// The ones this machine's drivers offer — a desktop with no platform
    /// profile may lack `performance`.
    pub offered: Vec<Profile>,
    /// Why `performance` is not performing (thermal, lap detection…).
    pub degraded: Option<String>,
    /// Why the daemon refuses `performance` outright.
    pub inhibited: Option<String>,
    /// Who holds the profile, and why.
    pub holds: Vec<Hold>,
    /// The daemon's own "lower the profile when the battery runs low"
    /// switch; `None` on a daemon too old to have it.
    pub battery_aware: Option<bool>,
    /// The daemon's version.
    pub version: String,
}

impl Profiles {
    /// From one `GetAll` of the daemon's interface.
    #[must_use]
    pub fn from_props(p: &Props) -> Self {
        let text_or_none = |k: &str| {
            let t = upower::text_of(p.get(k));
            (!t.is_empty()).then_some(t)
        };
        Self {
            active: Profile::by_id(&upower::text_of(p.get("ActiveProfile")))
                .unwrap_or(Profile::Balanced),
            offered: upower::dicts_of(p.get("Profiles"))
                .iter()
                .filter_map(|d| Profile::by_id(&upower::text_of(d.get("Profile"))))
                .collect(),
            degraded: text_or_none("PerformanceDegraded"),
            inhibited: text_or_none("PerformanceInhibited"),
            holds: upower::dicts_of(p.get("ActiveProfileHolds"))
                .iter()
                .map(|d| Hold {
                    who: upower::text_of(d.get("ApplicationId")),
                    profile: Profile::by_id(&upower::text_of(d.get("Profile"))),
                    why: upower::text_of(d.get("Reason")),
                })
                .collect(),
            battery_aware: upower::flag_if_there(p.get(profiles::BATTERY_AWARE)),
            version: upower::text_of(p.get("Version")),
        }
    }
}

/// logind's answer to "can you?".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Can {
    /// It can, and polkit lets this session ask.
    Yes,
    /// polkit refuses this session.
    No,
    /// The hardware cannot — no swap to hibernate into, say.
    #[default]
    NotAvailable,
    /// polkit would ask, and there is nobody to ask.
    Challenge,
}

impl Can {
    /// From logind's word.
    #[must_use]
    pub fn of(word: &str) -> Self {
        match word {
            "yes" => Self::Yes,
            "no" => Self::No,
            "challenge" => Self::Challenge,
            _ => Self::NotAvailable,
        }
    }
}

/// The four things one can ask logind to do from the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Suspend to memory.
    Suspend,
    /// Sleep now and hibernate later, on logind's own delay.
    ///
    /// THE ONE A LAPTOP WANTS: it wakes in a second like a suspend, and if
    /// nobody comes back it writes memory out before the battery goes. logind
    /// has offered it since 239 and this view asked `CanSuspendThenHibernate`
    /// on every reading while offering no row that could run it.
    SuspendThenHibernate,
    /// Suspend and hibernate at once, so a dead battery loses nothing.
    HybridSleep,
    /// Hibernate to the swapfile.
    Hibernate,
    /// Reboot.
    Reboot,
    /// Power off.
    PowerOff,
}

impl Action {
    /// In the order the view draws them — the two sleeps that come back
    /// fastest first, and the two that end the session last.
    pub const ALL: [Self; 6] = [
        Self::Suspend,
        Self::SuspendThenHibernate,
        Self::HybridSleep,
        Self::Hibernate,
        Self::Reboot,
        Self::PowerOff,
    ];

    /// The id the text keys and the commands use.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::SuspendThenHibernate => "suspend-then-hibernate",
            Self::HybridSleep => "hybrid-sleep",
            Self::Hibernate => "hibernate",
            Self::Reboot => "reboot",
            Self::PowerOff => "off",
        }
    }
}

/// What logind knows about sleeping and the lid.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sleep {
    /// logind's answer to `CanSuspend`.
    pub can_suspend: Can,
    /// logind's answer to `CanHibernate`.
    pub can_hibernate: Can,
    /// logind's answer to `CanHybridSleep`.
    pub can_hybrid_sleep: Can,
    /// logind's answer to `CanSuspendThenHibernate`.
    pub can_suspend_then_hibernate: Can,
    /// logind's answer to `CanPowerOff`.
    pub can_power_off: Can,
    /// logind's answer to `CanReboot`.
    pub can_reboot: Can,
    /// What closing the lid does, as configured — `suspend`, `ignore`…
    pub handle_lid: String,
    /// The same, on the charger; empty when not configured apart.
    pub handle_lid_external_power: String,
    /// The same, docked.
    pub handle_lid_docked: String,
    /// What the power key does.
    pub handle_power_key: String,
    /// What idling does.
    pub idle_action: String,
    /// After how long idling counts; `None` when logind reports zero.
    pub idle_after: Option<Duration>,
    /// Is the lid closed now, as logind sees it?
    pub lid_closed: bool,
    /// On the charger, as logind sees it.
    pub on_external_power: bool,
    /// Docked, as logind sees it.
    pub docked: bool,
    /// Between `PrepareForSleep(true)` and the sleep itself.
    pub preparing_for_sleep: bool,
    /// What is blocked right now (`sleep`, `shutdown`, `idle`…), as logind
    /// spells it.
    pub blocked: String,
}

impl Sleep {
    /// What one `GetAll` of the manager says; the `Can*` answers come from
    /// method calls and are filled in by the caller.
    #[must_use]
    pub fn from_props(p: &Props) -> Self {
        let idle = upower::u64_of(p.get("IdleActionUSec"));
        Self {
            handle_lid: upower::text_of(p.get("HandleLidSwitch")),
            handle_lid_external_power: upower::text_of(p.get("HandleLidSwitchExternalPower")),
            handle_lid_docked: upower::text_of(p.get("HandleLidSwitchDocked")),
            handle_power_key: upower::text_of(p.get("HandlePowerKey")),
            idle_action: upower::text_of(p.get("IdleAction")),
            idle_after: (idle > 0).then(|| Duration::from_micros(idle)),
            lid_closed: upower::flag_of(p.get("LidClosed")),
            on_external_power: upower::flag_of(p.get("OnExternalPower")),
            docked: upower::flag_of(p.get("Docked")),
            preparing_for_sleep: upower::flag_of(p.get("PreparingForSleep")),
            blocked: upower::text_of(p.get("BlockInhibited")),
            ..Default::default()
        }
    }

    /// What the view may ask for, and whether logind would do it.
    #[must_use]
    pub const fn can(&self, action: Action) -> Can {
        match action {
            Action::Suspend => self.can_suspend,
            Action::SuspendThenHibernate => self.can_suspend_then_hibernate,
            Action::HybridSleep => self.can_hybrid_sleep,
            Action::Hibernate => self.can_hibernate,
            Action::Reboot => self.can_reboot,
            Action::PowerOff => self.can_power_off,
        }
    }
}

/// `block` or `delay`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Holds the action off entirely.
    Block,
    /// Holds it off for at most `InhibitDelayMaxUSec`.
    Delay,
}

/// One inhibitor lock somebody holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inhibitor {
    /// `sleep`, `shutdown`, `idle`, `handle-lid-switch`… colon-joined.
    pub what: String,
    /// Who holds it — an application id.
    pub who: String,
    /// Their reason, in their words.
    pub why: String,
    /// Block or delay.
    pub mode: Mode,
    /// The holder's user.
    pub uid: u32,
    /// The holder's process.
    pub pid: u32,
}

impl Inhibitor {
    /// From one row of `ListInhibitors`.
    #[must_use]
    pub fn from_row((what, who, why, mode, uid, pid): login::Row) -> Self {
        Self {
            what,
            who,
            why,
            mode: if mode == "delay" {
                Mode::Delay
            } else {
                Mode::Block
            },
            uid,
            pid,
        }
    }

    /// Which of OUR two locks this is, if it is ours.
    #[must_use]
    pub fn ours(&self) -> Option<Keep> {
        if self.who != login::WHO {
            return None;
        }
        Keep::ALL
            .into_iter()
            .find(|k| self.what.split(':').any(|w| w == k.what()))
    }
}

/// The two things the view can hold the machine awake against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Keep {
    /// No sleep, no idle action.
    Sleep,
    /// Closing the lid does nothing.
    Lid,
}

impl Keep {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Sleep, Self::Lid];

    /// logind's `what`.
    #[must_use]
    pub const fn what(self) -> &'static str {
        match self {
            Self::Sleep => "sleep",
            Self::Lid => "handle-lid-switch",
        }
    }

    /// The id the text keys and the commands use.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Sleep => "awake",
            Self::Lid => "lid",
        }
    }
}

/// One whole reading of the machine's power.
///
/// Each part comes from a different daemon and each is read on its own: an
/// absent UPower (a VM) leaves `display` empty, an absent
/// power-profiles-daemon leaves `profiles` at `None`, and neither says
/// anything about the other.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Snapshot {
    /// What the modeline reads: the composite battery, when there is one.
    pub display: Option<Battery>,
    /// Every battery that feeds the machine, by native path.
    pub batteries: Vec<Battery>,
    /// The charger, when UPower has one.
    pub adapter: Option<Adapter>,
    /// Discharging, as UPower sums it over every battery.
    pub on_battery: bool,
    /// The lid.
    pub lid: Lid,
    /// Everything with a battery that does not feed the machine.
    pub peripherals: Vec<Peripheral>,
    /// `None` when the daemon is not on the bus.
    pub profiles: Option<Profiles>,
    /// What logind knows.
    pub sleep: Sleep,
    /// Every lock anybody holds.
    pub inhibitors: Vec<Inhibitor>,
    /// The charge pair, when a battery has one.
    pub thresholds: Option<sysfs::Thresholds>,
    /// What UPower will do at the action level, as it spells it.
    pub critical_action: Option<String>,
}

impl Snapshot {
    /// Is the charger delivering? `None` without a charger device.
    #[must_use]
    pub fn adapter_online(&self) -> Option<bool> {
        self.adapter.as_ref().map(|a| a.online)
    }

    /// Which of our locks are held right now — read from logind, never from
    /// memory: a footer that restarted holds nothing, whatever it remembers.
    #[must_use]
    pub fn held(&self) -> Vec<Keep> {
        let mut out: Vec<Keep> = self.inhibitors.iter().filter_map(Inhibitor::ours).collect();
        out.sort();
        out.dedup();
        out
    }
}

// ---------------------------------------------------------------------------
// The rules the screen draws by
// ---------------------------------------------------------------------------

/// Five steps of charge, the way the Wi-Fi has five of signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rung {
    /// 0–10%.
    Empty,
    /// 11–35%.
    Low,
    /// 36–65%.
    Half,
    /// 66–89%.
    High,
    /// 90% and up.
    Full,
}

impl Rung {
    /// Which step a percentage is on.
    #[must_use]
    pub const fn of(percent: u8) -> Self {
        match percent {
            0..=10 => Self::Empty,
            11..=35 => Self::Low,
            36..=65 => Self::Half,
            66..=89 => Self::High,
            _ => Self::Full,
        }
    }

    /// The id the icon tokens use (`battery-<id>`, `battery-charging-<id>`).
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Low => "low",
            Self::Half => "half",
            Self::High => "high",
            Self::Full => "full",
        }
    }
}

/// What the modeline draws for the battery, when it draws anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mark {
    /// Which step the glyph shows.
    pub rung: Rung,
    /// Energy is going in — the glyph gets the bolt.
    pub charging: bool,
    /// The number beside the glyph, only at [`PERCENT_UNTIL`] and below.
    pub percent: Option<u8>,
    /// The danger colour, at [`DANGER_UNTIL`] and below.
    pub danger: bool,
}

/// The modeline's battery mark, or nothing.
///
/// Nothing without a battery (a desktop, a VM), at [`HIDE_FROM`] and above,
/// and at `FullyCharged` — a battery held at an 80% threshold reports
/// `FullyCharged` at 80 for ever, and a glyph that never goes away says
/// nothing. `charging` is the STATE's word, or the charger's when the state
/// is not saying (a laptop docked at 100% says `Unknown` on some firmware).
#[must_use]
pub fn mark(display: Option<&Battery>, adapter_online: Option<bool>) -> Option<Mark> {
    let b = display.filter(|b| b.present)?;
    let percent = b.percent();
    if percent >= HIDE_FROM || b.state == State::FullyCharged {
        return None;
    }
    let charging =
        b.state.is_charging() || (b.state == State::Unknown && adapter_online == Some(true));
    Some(Mark {
        rung: Rung::of(percent),
        charging,
        percent: (percent <= PERCENT_UNTIL).then_some(percent),
        danger: percent <= DANGER_UNTIL,
    })
}

/// The profile a daemon calls "the top": `performance` when it is offered,
/// else `balanced`, else whatever comes first.
#[must_use]
pub fn top(offered: &[Profile]) -> Option<Profile> {
    offered.iter().copied().max()
}

/// The modeline's profile mark: the active profile, only when it is NOT the
/// top one — the distro's default is `performance`, and a machine at its
/// default says nothing. A desktop whose daemon offers only `balanced` and
/// `power-saver` is at its own top on `balanced`, and is quiet there too.
#[must_use]
pub fn profile_mark(p: &Profiles) -> Option<Profile> {
    let top = top(&p.offered)?;
    (p.active != top).then_some(p.active)
}

/// A crossing worth a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alarm {
    /// UPower's `Low` reached.
    Low,
    /// UPower's `Critical` or `Action` reached.
    Critical,
    /// Charging ended at full.
    Charged,
}

impl Alarm {
    /// Every one — the tags the catalogue recognises.
    pub const ALL: [Self; 3] = [Self::Low, Self::Critical, Self::Charged];

    /// The tag `assets/notifications.nix` matches.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Low => "battery-low",
            Self::Critical => "battery-critical",
            Self::Charged => "battery-charged",
        }
    }
}

/// Which alarms the step from `before` to `after` crosses.
///
/// TRANSITIONS, never levels: a battery that sits at `Low` for an hour is one
/// notification, not one per reading. `before` is `None` on the first reading
/// after the footer starts, and a battery found already low then IS worth
/// saying — a session that starts at 12% has not been told yet.
#[must_use]
pub fn alarms(before: Option<&Snapshot>, after: &Snapshot) -> Vec<Alarm> {
    let Some(now) = after.display.as_ref() else {
        return Vec::new();
    };
    let then = before.and_then(|s| s.display.as_ref());
    let mut out = Vec::new();
    let worry = |w: Warning| match w {
        Warning::Low => Some(Alarm::Low),
        Warning::Critical | Warning::Action => Some(Alarm::Critical),
        _ => None,
    };
    let now_alarm = worry(now.warning);
    let then_alarm = then.and_then(|b| worry(b.warning));
    if let Some(a) = now_alarm
        && then_alarm != Some(a)
    {
        out.push(a);
    }
    // "YOU CAN UNPLUG" — AND ONLY WHEN IT IS TRUE.
    //
    // UPower says `FullyCharged` when the battery stops taking charge, which
    // is exactly what an END THRESHOLD makes it do — and setting one is a
    // first-class feature of this same view (`Preset::Longevity` is 40-80).
    // So the owner picked 40-80, plugged in at 60%, and at 80% the machine
    // said "Battery charged. You can unplug." The body carries no number, so
    // nothing on screen contradicted it, and it happened once per charge.
    //
    // `mark()` already knows this — it hides the glyph for the same state,
    // and says why eighty lines up. This is the same fact, on the other side
    // of the file: a battery held at a threshold is not a battery that is
    // full, and the honest thing to say about it is nothing.
    //
    // THE THRESHOLD AND NOT THE PERCENTAGE decides, because the percentage
    // cannot: a healthy pack really does stop at 97% and a worn one at 88%,
    // and picking a number here would silence the true sentence on one
    // machine to silence the false one on another.
    let held = after.thresholds.as_ref().is_some_and(|t| t.end < 100);
    if now.state == State::FullyCharged && then.is_some_and(|b| b.state.is_charging()) && !held {
        out.push(Alarm::Charged);
    }
    out
}

// ---------------------------------------------------------------------------
// The conversation with the three daemons
// ---------------------------------------------------------------------------

/// What can be asked of the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Write `ActiveProfile`.
    SetProfile(Profile),
    /// Write `BatteryAware`.
    BatteryAware(bool),
    /// Take (`true`) or drop (`false`) one of our locks.
    KeepAwake(Keep, bool),
    /// Write the charge pair — see `sysfs`.
    Thresholds {
        /// The start threshold; `None` leaves it where it is.
        start: Option<u8>,
        /// The end threshold.
        end: u8,
    },
    /// Ask logind for one of the four actions.
    Do(Action),
}

/// What a daemon refused with, as a name rather than a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// polkit said no. With no agent in this session, this is final.
    NotAuthorized,
    /// The daemon is not on the bus, or the hardware cannot.
    NotAvailable,
    /// The pair or the value was refused by the driver or the daemon.
    Invalid,
    /// Anything else; the words travel alongside.
    Other,
}

/// The refusal a D-Bus error NAME stands for — a method call's answer.
fn refusal_named(name: &str) -> Refusal {
    match name {
        "org.freedesktop.PolicyKit1.Error.NotAuthorized"
        | "org.freedesktop.DBus.Error.AccessDenied"
        | "org.freedesktop.DBus.Error.AuthFailed"
        | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired" => Refusal::NotAuthorized,
        "org.freedesktop.DBus.Error.ServiceUnknown"
        | "org.freedesktop.DBus.Error.NameHasNoOwner"
        | "org.freedesktop.DBus.Error.UnknownObject"
        | "org.freedesktop.DBus.Error.UnknownMethod"
        | "org.freedesktop.DBus.Error.UnknownInterface"
        | "org.freedesktop.DBus.Error.UnknownProperty"
        | "org.freedesktop.DBus.Error.NotSupported"
        | "org.freedesktop.login1.SleepVerbNotSupported" => Refusal::NotAvailable,
        "org.freedesktop.DBus.Error.InvalidArgs"
        | "org.freedesktop.DBus.Error.PropertyReadOnly" => Refusal::Invalid,
        _ => Refusal::Other,
    }
}

/// Reads the refusal out of an error, whatever it is wrapped in.
#[must_use]
pub fn refusal(e: &anyhow::Error) -> Refusal {
    for cause in e.chain() {
        if let Some(zbus::Error::MethodError(name, _, _)) = cause.downcast_ref::<zbus::Error>() {
            return refusal_named(name.as_str());
        }
        // THE PROPERTY CALLS — `ActiveProfile`, `BatteryAware` — answer with
        // `fdo::Error`, a typed enum whose `source()` is empty: the chain
        // stops there, and every polkit "no" on the profile read as "refused,
        // for some reason". The named variants are the same names as above;
        // a method error under it is the method's.
        if let Some(f) = cause.downcast_ref::<zbus::fdo::Error>() {
            use zbus::fdo::Error as F;
            return match f {
                F::ZBus(zbus::Error::MethodError(name, _, _)) => refusal_named(name.as_str()),
                F::AccessDenied(_) | F::AuthFailed(_) | F::InteractiveAuthorizationRequired(_) => {
                    Refusal::NotAuthorized
                }
                F::ServiceUnknown(_)
                | F::NameHasNoOwner(_)
                | F::UnknownObject(_)
                | F::UnknownMethod(_)
                | F::UnknownInterface(_)
                | F::UnknownProperty(_)
                | F::NotSupported(_) => Refusal::NotAvailable,
                F::InvalidArgs(_) | F::PropertyReadOnly(_) => Refusal::Invalid,
                _ => Refusal::Other,
            };
        }
        // THE SYSFS WRITES fail as I/O errors, and the kind says which door
        // shut: EACCES is the udev rule not having reached the file, EINVAL
        // the driver refusing the value (some firmware takes 60/80/100 and
        // nothing between), ENOENT a start threshold the battery has not got.
        // Every one of them used to read as "not allowed for this session".
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return match io.kind() {
                std::io::ErrorKind::PermissionDenied => Refusal::NotAuthorized,
                std::io::ErrorKind::InvalidInput => Refusal::Invalid,
                std::io::ErrorKind::NotFound => Refusal::NotAvailable,
                _ => Refusal::Other,
            };
        }
        if cause.downcast_ref::<sysfs::Refused>().is_some() {
            return Refusal::Invalid;
        }
    }
    Refusal::Other
}

/// The conversation with the three daemons, on one system-bus connection.
pub struct Manager {
    connection: zbus::blocking::Connection,
    places: ronoctua_memory::Places,
}

impl Manager {
    /// Opens the connection to the system bus.
    ///
    /// # Errors
    ///
    /// No system bus.
    pub fn fresh() -> Result<Self> {
        let connection =
            zbus::blocking::Connection::system().context("no system bus to talk to UPower")?;
        Ok(Self {
            connection,
            places: ronoctua_memory::Places::default(),
        })
    }

    /// Everything, each daemon on its own — one absent leaves its part empty.
    #[must_use]
    pub fn read(&self) -> Snapshot {
        let mut out = Snapshot::default();
        self.read_upower(&mut out);
        self.read_profiles(&mut out);
        self.read_login(&mut out);
        out.thresholds = sysfs::read(&self.places);
        out
    }

    fn read_upower(&self, out: &mut Snapshot) {
        let Ok(daemon) = upower::UpowerProxyBlocking::new(&self.connection) else {
            return;
        };
        let Ok(paths) = daemon.enumerate_devices() else {
            tracing::debug!("UPower is not on the bus; no battery to read");
            return;
        };
        out.on_battery = daemon.on_battery().unwrap_or(false);
        out.lid = Lid {
            present: daemon.lid_is_present().unwrap_or(false),
            closed: daemon.lid_is_closed().unwrap_or(false),
        };
        out.critical_action = daemon.get_critical_action().ok();
        for path in paths {
            let Ok(props) = upower::get_all(
                &self.connection,
                upower::SERVICE,
                path.as_str(),
                upower::DEVICE,
            ) else {
                continue;
            };
            let kind = Kind::from_u32(upower::u32_of(props.get("Type")));
            let feeds = upower::flag_of(props.get("PowerSupply"));
            match kind {
                Kind::LinePower => {
                    let online = upower::flag_of(props.get("Online"));
                    // ONE charger on the screen: the first that is online, or
                    // the first at all — a laptop with three USB-C ports has
                    // three line-power devices and one of them is the cable.
                    let better = out.adapter.as_ref().is_none_or(|a| !a.online && online);
                    if better {
                        out.adapter = Some(Adapter {
                            at: path.as_str().to_owned(),
                            native_path: upower::text_of(props.get("NativePath")),
                            online,
                        });
                    }
                }
                Kind::Battery | Kind::Ups if feeds => {
                    out.batteries
                        .push(Battery::from_props(path.as_str(), &props));
                }
                _ => out.peripherals.push(Peripheral {
                    at: path.as_str().to_owned(),
                    kind,
                    model: upower::text_of(props.get("Model")),
                    percentage: {
                        let p = upower::f64_of(props.get("Percentage"));
                        (p > 0.0).then_some(p)
                    },
                    state: State::from_u32(upower::u32_of(props.get("State"))),
                }),
            }
        }
        out.batteries
            .sort_by(|a, b| a.native_path.cmp(&b.native_path));
        if let Ok(display) = daemon.get_display_device()
            && let Ok(props) = upower::get_all(
                &self.connection,
                upower::SERVICE,
                display.as_str(),
                upower::DEVICE,
            )
        {
            let b = Battery::from_props(display.as_str(), &props);
            if b.present {
                out.display = Some(b);
            }
        }
    }

    fn read_profiles(&self, out: &mut Snapshot) {
        match upower::get_all(
            &self.connection,
            profiles::SERVICE,
            profiles::PATH,
            profiles::INTERFACE,
        ) {
            Ok(props) => out.profiles = Some(Profiles::from_props(&props)),
            Err(e) => tracing::debug!(error = %e, "no power-profiles-daemon on the bus"),
        }
    }

    fn read_login(&self, out: &mut Snapshot) {
        let Ok(props) = upower::get_all(
            &self.connection,
            login::SERVICE,
            login::PATH,
            login::MANAGER,
        ) else {
            return;
        };
        let mut sleep = Sleep::from_props(&props);
        if let Ok(manager) = login::LoginProxyBlocking::new(&self.connection) {
            let ask = |r: zbus::Result<String>| Can::of(&r.unwrap_or_default());
            sleep.can_suspend = ask(manager.can_suspend());
            sleep.can_hibernate = ask(manager.can_hibernate());
            sleep.can_hybrid_sleep = ask(manager.can_hybrid_sleep());
            sleep.can_suspend_then_hibernate = ask(manager.can_suspend_then_hibernate());
            sleep.can_power_off = ask(manager.can_power_off());
            sleep.can_reboot = ask(manager.can_reboot());
            out.inhibitors = manager
                .list_inhibitors()
                .unwrap_or_default()
                .into_iter()
                .map(Inhibitor::from_row)
                .collect();
        }
        out.sleep = sleep;
    }

    /// A blocking stream of everything under UPower's path — the batteries'
    /// `PropertiesChanged`, `DeviceAdded`/`Removed`, and, one object over
    /// under the same namespace, power-profiles-daemon's.
    ///
    /// # Errors
    ///
    /// The match rule could not be installed.
    pub fn upower_signals(&self) -> Result<zbus::blocking::MessageIterator> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .path_namespace(upower::ROOT)?
            .build();
        Ok(zbus::blocking::MessageIterator::for_match_rule(
            rule,
            &self.connection,
            None,
        )?)
    }

    /// A blocking stream of logind's signals — `PropertiesChanged` on the
    /// manager (an inhibitor taken, the lid), and `PrepareForSleep`.
    ///
    /// # Errors
    ///
    /// The match rule could not be installed.
    pub fn login_signals(&self) -> Result<zbus::blocking::MessageIterator> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(login::SERVICE)?
            .path(login::PATH)?
            .build();
        Ok(zbus::blocking::MessageIterator::for_match_rule(
            rule,
            &self.connection,
            None,
        )?)
    }

    /// Writes `ActiveProfile`.
    ///
    /// # Errors
    ///
    /// The daemon refused, or is absent.
    pub fn set_profile(&self, profile: Profile) -> Result<()> {
        upower::set_one(
            &self.connection,
            profiles::SERVICE,
            profiles::PATH,
            profiles::INTERFACE,
            profiles::ACTIVE,
            zbus::zvariant::Value::from(profile.id()),
        )
        .context("setting the power profile")
    }

    /// Writes `BatteryAware`.
    ///
    /// # Errors
    ///
    /// The daemon refused, or is too old to have the switch.
    pub fn set_battery_aware(&self, on: bool) -> Result<()> {
        upower::set_one(
            &self.connection,
            profiles::SERVICE,
            profiles::PATH,
            profiles::INTERFACE,
            profiles::BATTERY_AWARE,
            zbus::zvariant::Value::from(on),
        )
        .context("setting battery awareness")
    }

    /// Takes one of our locks. The fd IS the lock.
    ///
    /// # Errors
    ///
    /// logind refused, or is absent.
    pub fn inhibit(&self, keep: Keep) -> Result<zbus::zvariant::OwnedFd> {
        let manager = login::LoginProxyBlocking::new(&self.connection)?;
        manager
            .inhibit(keep.what(), login::WHO, login::WHY, "block")
            .context("taking the inhibitor")
    }

    /// Asks logind for one of the four actions, never interactively.
    ///
    /// # Errors
    ///
    /// logind refused, or cannot.
    pub fn perform(&self, action: Action) -> Result<()> {
        let manager = login::LoginProxyBlocking::new(&self.connection)?;
        match action {
            Action::Suspend => manager.suspend(false),
            Action::SuspendThenHibernate => manager.suspend_then_hibernate(false),
            Action::HybridSleep => manager.hybrid_sleep(false),
            Action::Hibernate => manager.hibernate(false),
            Action::Reboot => manager.reboot(false),
            Action::PowerOff => manager.power_off(false),
        }
        .context("asking logind")
    }

    /// Writes the charge thresholds.
    ///
    /// # Errors
    ///
    /// As [`sysfs::write`].
    pub fn write_thresholds(&self, start: Option<u8>, end: u8) -> Result<sysfs::Thresholds> {
        sysfs::write(&self.places, start, end)
    }
}

/// What the watching threads send the footer.
#[derive(Debug)]
pub enum Event {
    /// A fresh reading. The last one wins.
    Snapshot(Box<Snapshot>),
    /// An order was carried out, or refused — the words are the daemon's.
    Done {
        /// The order that was carried out, or refused.
        order: Order,
        /// The refusal and the daemon's own words, when refused.
        refused: Option<(Refusal, String)>,
    },
}

/// The footer's side of the orders thread.
#[derive(Debug, Clone)]
pub struct Handle {
    orders: mpsc::Sender<Order>,
}

impl Handle {
    /// Sends one order; the answer comes back as a fresh [`Event::Snapshot`]
    /// — the reading the order produced — and then [`Event::Done`], so a
    /// door that draws on `Done` already has the truth in hand.
    pub fn send(&self, order: Order) {
        if self.orders.send(order).is_err() {
            tracing::warn!("the power thread is gone; the order was dropped");
        }
    }
}

/// Watches the three daemons and carries out orders, on three threads.
///
/// # Why whole snapshots, and why threads
///
/// The disks' reasons hold (see `ronoctua_disks::watch`), and one is stronger
/// here: `Device1.Connect` on Bluetooth and `Suspend` here are calls that
/// block, and a call that blocks inside the footer's loop is a frozen footer.
/// The orders thread also OWNS the inhibitor locks: an inhibitor is an fd, and
/// the thread that took it is the one that has to keep it open.
///
/// # Errors
///
/// No system bus, or a thread could not be raised. An absent daemon is not an
/// error: its part of the snapshot stays empty and the screen says so.
pub fn watch<F>(on_event: F) -> Result<Handle>
where
    F: Fn(Event) + Send + Sync + 'static,
{
    let on_event = std::sync::Arc::new(on_event);
    let (orders, inbox) = mpsc::channel::<Order>();

    let first = Manager::fresh()?;
    let feed = on_event.clone();
    std::thread::Builder::new()
        .name("power-upower".to_owned())
        .spawn(move || {
            // THE MATCH RULE BEFORE THE FIRST READING. The reading is some
            // fifteen round trips; a `PropertiesChanged` emitted during it —
            // the cable going in — would land before the rule and be lost,
            // and the modeline would keep the discharging glyph until
            // UPower's next poll thirty seconds later.
            let Ok(messages) = first.upower_signals() else {
                tracing::warn!("no UPower stream; the battery will not reread on its own");
                return;
            };
            feed(Event::Snapshot(Box::new(first.read())));
            for _ in messages {
                feed(Event::Snapshot(Box::new(first.read())));
            }
            tracing::info!("the UPower stream ended; no automatic reread");
        })
        .context("could not start the UPower thread")?;

    let second = Manager::fresh()?;
    let feed = on_event.clone();
    std::thread::Builder::new()
        .name("power-login".to_owned())
        .spawn(move || {
            let Ok(messages) = second.login_signals() else {
                tracing::warn!("no logind stream; inhibitors will not reread on their own");
                return;
            };
            for _ in messages {
                feed(Event::Snapshot(Box::new(second.read())));
            }
            tracing::info!("the logind stream ended; no automatic reread");
        })
        .context("could not start the logind thread")?;

    let third = Manager::fresh()?;
    let feed = on_event;
    std::thread::Builder::new()
        .name("power-orders".to_owned())
        .spawn(move || {
            // THE LOCKS LIVE HERE, for the life of the process. Dropping one
            // releases it; the map is what keeps them.
            let mut held: BTreeMap<Keep, zbus::zvariant::OwnedFd> = BTreeMap::new();
            for order in inbox {
                let result = match &order {
                    Order::SetProfile(p) => third.set_profile(*p),
                    Order::BatteryAware(on) => third.set_battery_aware(*on),
                    Order::KeepAwake(keep, true) => match third.inhibit(*keep) {
                        Ok(fd) => {
                            held.insert(*keep, fd);
                            Ok(())
                        }
                        Err(e) => Err(e),
                    },
                    Order::KeepAwake(keep, false) => {
                        // DROPPING A LOCK NOBODY HELD IS NOT "DONE".
                        //
                        // This answered `Ok(())` either way, so the door drew
                        // a result for a no-op: releasing a lock this process
                        // never took looked exactly like releasing one it did.
                        // The fd is the lock — `Drop` closes it, which is what
                        // logind sees — so "was there one" is `remove`'s own
                        // answer, and it is the honest one to give back.
                        if held.remove(keep).is_some() {
                            Ok(())
                        } else {
                            Err(anyhow::anyhow!(
                                "this session holds no {} lock to let go of",
                                keep.id()
                            ))
                        }
                    }
                    Order::Thresholds { start, end } => {
                        third.write_thresholds(*start, *end).map(|_| ())
                    }
                    Order::Do(action) => third.perform(*action),
                };
                let refused = result.err().map(|e| (refusal(&e), format!("{e:#}")));
                if let Some((why, words)) = &refused {
                    tracing::warn!(?order, ?why, %words, "the power order was refused");
                }
                // THE SCREEN SEES THE RESULT AT ONCE: a fresh reading follows
                // every order, refused or not — the truth is what the daemons
                // say, not what was asked.
                feed(Event::Snapshot(Box::new(third.read())));
                feed(Event::Done { order, refused });
            }
        })
        .context("could not start the power orders thread")?;

    Ok(Handle { orders })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_property_refusal_is_read_through_the_fdo_error() {
        use super::{Refusal, refusal};
        let wrapped = |f: zbus::fdo::Error| anyhow::Error::from(f).context("setting the profile");
        assert_eq!(
            refusal(&wrapped(zbus::fdo::Error::AccessDenied(
                "polkit said no".into()
            ))),
            Refusal::NotAuthorized
        );
        assert_eq!(
            refusal(&wrapped(zbus::fdo::Error::ServiceUnknown(
                "no daemon".into()
            ))),
            Refusal::NotAvailable
        );
        assert_eq!(
            refusal(&wrapped(zbus::fdo::Error::InvalidArgs(
                "not offered".into()
            ))),
            Refusal::Invalid
        );
        assert_eq!(
            refusal(&wrapped(zbus::fdo::Error::Failed("something".into()))),
            Refusal::Other
        );
        let io = |kind: std::io::ErrorKind| {
            anyhow::Error::from(std::io::Error::new(kind, "sysfs")).context("writing end")
        };
        assert_eq!(
            refusal(&io(std::io::ErrorKind::PermissionDenied)),
            Refusal::NotAuthorized
        );
        assert_eq!(
            refusal(&io(std::io::ErrorKind::InvalidInput)),
            Refusal::Invalid
        );
        assert_eq!(
            refusal(&io(std::io::ErrorKind::NotFound)),
            Refusal::NotAvailable
        );
        assert_eq!(
            refusal(&anyhow::Error::from(
                super::sysfs::Refused::StartNotBelowEnd
            )),
            Refusal::Invalid
        );
    }

    use super::*;
    use zbus::zvariant::{OwnedValue, Value};

    /// A property map in UPower's shape, with a laptop's numbers and nothing
    /// that names the laptop.
    fn battery_props(percentage: f64, state: u32, warning: u32) -> Props {
        let mut p = Props::new();
        let put = |p: &mut Props, k: &str, v: Value<'static>| {
            p.insert(k.to_owned(), OwnedValue::try_from(v).unwrap());
        };
        put(&mut p, "Type", Value::U32(2));
        put(&mut p, "State", Value::U32(state));
        put(&mut p, "Percentage", Value::F64(percentage));
        put(&mut p, "WarningLevel", Value::U32(warning));
        put(&mut p, "TimeToEmpty", Value::I64(4_920));
        put(&mut p, "TimeToFull", Value::I64(0));
        put(&mut p, "EnergyRate", Value::F64(8.4));
        put(&mut p, "Energy", Value::F64(29.1));
        put(&mut p, "EnergyFull", Value::F64(52.0));
        put(&mut p, "EnergyFullDesign", Value::F64(65.0));
        put(&mut p, "Voltage", Value::F64(11.7));
        put(&mut p, "Temperature", Value::F64(0.0));
        put(&mut p, "Capacity", Value::F64(80.0));
        put(&mut p, "ChargeCycles", Value::I32(312));
        put(&mut p, "Technology", Value::U32(2));
        put(&mut p, "IsPresent", Value::Bool(true));
        put(&mut p, "PowerSupply", Value::Bool(true));
        put(&mut p, "NativePath", Value::from("BAT0"));
        put(&mut p, "Vendor", Value::from("a vendor"));
        put(&mut p, "Model", Value::from("a model"));
        put(&mut p, "IconName", Value::from("battery-good-symbolic"));
        p
    }

    fn battery(percentage: f64, state: State) -> Battery {
        Battery::from_props("/b", &battery_props(percentage, state as u32, 1))
    }

    #[test]
    fn a_property_map_becomes_a_battery() {
        let b = Battery::from_props("/b", &battery_props(56.0, 2, 1));
        assert_eq!(b.state, State::Discharging);
        assert_eq!(b.percent(), 56);
        assert_eq!(b.time_to_empty, Some(Duration::from_secs(4_920)));
        assert_eq!(b.time_to_full, None, "zero is not a time");
        assert_eq!(b.temperature_c, None, "zero is not a temperature");
        assert_eq!(b.cycles, Some(312));
        assert_eq!(b.technology, Technology::LithiumPolymer);
        assert_eq!(b.health(), Some(80));
        assert_eq!(b.native_path, "BAT0");
    }

    #[test]
    fn the_mark_hides_at_the_top_and_at_full() {
        let discharging = |p: f64| mark(Some(&battery(p, State::Discharging)), Some(false));
        assert!(discharging(100.0).is_none());
        assert!(discharging(99.0).is_none());
        assert_eq!(discharging(98.0).map(|m| m.rung), Some(Rung::Full));
        // FULLY CHARGED HIDES AT ANY NUMBER: held at a threshold, the battery
        // says `FullyCharged` at 80 for ever.
        assert!(mark(Some(&battery(80.0, State::FullyCharged)), Some(true)).is_none());
        // And no battery is no mark.
        assert!(mark(None, Some(true)).is_none());
        let mut absent = battery(50.0, State::Discharging);
        absent.present = false;
        assert!(mark(Some(&absent), None).is_none());
    }

    #[test]
    fn the_number_appears_at_thirty_and_the_danger_at_fifteen() {
        let at = |p: f64| mark(Some(&battery(p, State::Discharging)), Some(false)).unwrap();
        assert_eq!(at(31.0).percent, None);
        assert_eq!(at(30.0).percent, Some(30));
        assert!(!at(16.0).danger);
        assert!(at(15.0).danger);
        assert_eq!(at(15.0).percent, Some(15));
        // The owner's rule is the LITERAL one: charging at 12% is still 12%.
        let charging = mark(Some(&battery(12.0, State::Charging)), Some(true)).unwrap();
        assert!(charging.danger && charging.charging);
    }

    #[test]
    fn the_rungs_have_five_steps() {
        assert_eq!(Rung::of(0), Rung::Empty);
        assert_eq!(Rung::of(10), Rung::Empty);
        assert_eq!(Rung::of(11), Rung::Low);
        assert_eq!(Rung::of(35), Rung::Low);
        assert_eq!(Rung::of(36), Rung::Half);
        assert_eq!(Rung::of(65), Rung::Half);
        assert_eq!(Rung::of(66), Rung::High);
        assert_eq!(Rung::of(89), Rung::High);
        assert_eq!(Rung::of(90), Rung::Full);
    }

    #[test]
    fn charging_comes_from_the_state_or_from_the_charger() {
        let b = battery(50.0, State::Unknown);
        assert!(mark(Some(&b), Some(true)).unwrap().charging);
        assert!(!mark(Some(&b), Some(false)).unwrap().charging);
        assert!(!mark(Some(&b), None).unwrap().charging);
        let b = battery(50.0, State::PendingCharge);
        assert!(mark(Some(&b), None).unwrap().charging);
    }

    fn profiles(active: Profile, offered: &[Profile]) -> Profiles {
        Profiles {
            active,
            offered: offered.to_vec(),
            degraded: None,
            inhibited: None,
            holds: Vec::new(),
            battery_aware: Some(true),
            version: "0.30".into(),
        }
    }

    #[test]
    fn the_profile_mark_is_quiet_at_the_top() {
        let all = Profile::ALL;
        assert_eq!(profile_mark(&profiles(Profile::Performance, &all)), None);
        assert_eq!(
            profile_mark(&profiles(Profile::Balanced, &all)),
            Some(Profile::Balanced)
        );
        assert_eq!(
            profile_mark(&profiles(Profile::PowerSaver, &all)),
            Some(Profile::PowerSaver)
        );
        // A DESKTOP WITH NO PERFORMANCE ON OFFER is at its top on balanced.
        let two = [Profile::PowerSaver, Profile::Balanced];
        assert_eq!(profile_mark(&profiles(Profile::Balanced, &two)), None);
        assert_eq!(
            profile_mark(&profiles(Profile::PowerSaver, &two)),
            Some(Profile::PowerSaver)
        );
        assert_eq!(top(&[]), None);
    }

    #[test]
    fn the_daemons_dictionary_becomes_profiles() {
        let dict = |profile: &str| {
            let mut d = Props::new();
            d.insert(
                "Profile".to_owned(),
                OwnedValue::try_from(Value::from(profile)).unwrap(),
            );
            d
        };
        let mut p = Props::new();
        p.insert(
            "ActiveProfile".to_owned(),
            OwnedValue::try_from(Value::from("balanced")).unwrap(),
        );
        p.insert(
            "Profiles".to_owned(),
            OwnedValue::try_from(Value::from(vec![dict("power-saver"), dict("balanced")])).unwrap(),
        );
        p.insert(
            "PerformanceDegraded".to_owned(),
            OwnedValue::try_from(Value::from("")).unwrap(),
        );
        p.insert(
            "Version".to_owned(),
            OwnedValue::try_from(Value::from("0.30")).unwrap(),
        );
        let got = Profiles::from_props(&p);
        assert_eq!(got.active, Profile::Balanced);
        assert_eq!(got.offered, vec![Profile::PowerSaver, Profile::Balanced]);
        assert_eq!(got.degraded, None, "an empty reason is no reason");
        assert_eq!(got.battery_aware, None, "an old daemon has no switch");
    }

    fn snapshot(b: Battery) -> Snapshot {
        Snapshot {
            display: Some(b),
            ..Default::default()
        }
    }

    fn warned(percentage: f64, state: State, warning: u32) -> Snapshot {
        snapshot(Battery::from_props(
            "/b",
            &battery_props(percentage, state as u32, warning),
        ))
    }

    #[test]
    fn every_enumeration_lists_each_of_its_words_once() {
        // `Technology::all()` asked `from_u32` for 0..=7 while its last valid
        // number is 6 — and `from_u32` answers `Unknown` for anything it does
        // not know, so `Unknown` came back twice. `dedup` removes only
        // CONSECUTIVE duplicates, and these were not consecutive.
        //
        // It is checked for all four, because the mistake is the same shape
        // in each and only one of them had it.
        fn once<T: std::fmt::Debug + PartialEq>(all: Vec<T>, what: &str) {
            for (i, a) in all.iter().enumerate() {
                for b in all.iter().skip(i + 1) {
                    assert!(a != b, "{what} lists {a:?} twice");
                }
            }
        }
        once(Kind::all(), "Kind");
        once(State::all(), "State");
        once(Warning::all(), "Warning");
        once(Technology::all(), "Technology");
    }

    #[test]
    fn alarms_fire_on_the_crossing_and_not_on_the_level() {
        let fine = warned(50.0, State::Discharging, 1);
        let low = warned(14.0, State::Discharging, 3);
        let critical = warned(5.0, State::Discharging, 4);
        assert_eq!(alarms(Some(&fine), &low), vec![Alarm::Low]);
        assert_eq!(alarms(Some(&low), &low), vec![], "the same level twice");
        assert_eq!(alarms(Some(&low), &critical), vec![Alarm::Critical]);
        assert_eq!(alarms(Some(&critical), &critical), vec![]);
        assert_eq!(
            alarms(Some(&critical), &fine),
            vec![],
            "recovering says nothing"
        );
        // A SESSION THAT STARTS LOW IS TOLD ONCE.
        assert_eq!(alarms(None, &low), vec![Alarm::Low]);
        assert_eq!(alarms(None, &fine), vec![]);
        // Charged: charging then full, and only that way round.
        let charging = warned(97.0, State::Charging, 1);
        let full = warned(100.0, State::FullyCharged, 1);
        assert_eq!(alarms(Some(&charging), &full), vec![Alarm::Charged]);
        assert_eq!(alarms(Some(&full), &full), vec![]);
        assert_eq!(
            alarms(None, &full),
            vec![],
            "a session that starts full is not told"
        );
        // AND NOT AT A CHARGE THRESHOLD, which is what makes the daemon say
        // `FullyCharged` at eighty per cent for ever. "You can unplug" is
        // false there, and the body carries no number to contradict it.
        let mut stopped = warned(80.0, State::FullyCharged, 1);
        stopped.thresholds = Some(sysfs::Thresholds {
            supply: "BAT0".to_owned(),
            start: Some(40),
            end: 80,
            writable: true,
            behaviour: None,
            types: None,
        });
        let mut climbing = warned(78.0, State::Charging, 1);
        climbing.thresholds = stopped.thresholds.clone();
        assert_eq!(
            alarms(Some(&climbing), &stopped),
            vec![],
            "a battery held at a threshold is not a battery that is full"
        );
        // AND WITH THE PAIR AT A HUNDRED IT IS THE ORDINARY CASE AGAIN.
        let mut open = stopped;
        if let Some(t) = open.thresholds.as_mut() {
            t.end = 100;
        }
        let mut before = climbing;
        before.thresholds = open.thresholds.clone();
        assert_eq!(alarms(Some(&before), &open), vec![Alarm::Charged]);
    }

    #[test]
    fn our_locks_are_told_apart_from_everybody_elses() {
        let row = |what: &str, who: &str| {
            Inhibitor::from_row((
                what.to_owned(),
                who.to_owned(),
                "why".to_owned(),
                "block".to_owned(),
                1000,
                1,
            ))
        };
        assert_eq!(row("sleep", login::WHO).ours(), Some(Keep::Sleep));
        assert_eq!(
            row("handle-lid-switch:sleep", login::WHO).ours(),
            Some(Keep::Sleep),
            "the first of ours that matches"
        );
        assert_eq!(row("handle-lid-switch", login::WHO).ours(), Some(Keep::Lid));
        assert_eq!(row("sleep", "somebody").ours(), None);
        let s = Snapshot {
            inhibitors: vec![
                row("sleep", login::WHO),
                row("sleep", "x"),
                row("sleep", login::WHO),
            ],
            ..Default::default()
        };
        assert_eq!(s.held(), vec![Keep::Sleep], "once, whatever the count");
    }

    #[test]
    fn logind_words_become_answers() {
        assert_eq!(Can::of("yes"), Can::Yes);
        assert_eq!(Can::of("no"), Can::No);
        assert_eq!(Can::of("na"), Can::NotAvailable);
        assert_eq!(Can::of("challenge"), Can::Challenge);
        assert_eq!(Can::of(""), Can::NotAvailable);
    }

    #[test]
    fn every_id_is_kebab_case_and_distinct() {
        let mut ids: Vec<&str> = Vec::new();
        ids.extend(Profile::ALL.iter().map(|p| p.id()));
        ids.extend(Action::ALL.iter().map(|a| a.id()));
        ids.extend(Keep::ALL.iter().map(|k| k.id()));
        ids.extend(Alarm::ALL.iter().map(|a| a.id()));
        ids.extend(sysfs::Preset::ALL.iter().map(|p| p.id()));
        for id in &ids {
            assert!(
                id.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
                "{id}"
            );
        }
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }
}
