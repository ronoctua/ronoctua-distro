//! The one boundary with BlueZ — the radio, the devices, and what each can do.
//!
//! # The shape, and why it is this one
//!
//! `ronoctua-disks` and `ronoctua-power`'s shape, for the third time: the bus
//! lives in [`bluez`] and the pairing questions in [`agent`], and everything
//! here is rules over plain structs. A [`Scene`] is the WHOLE radio as of one
//! reading, and it is what crosses the channel — never a delta. A delta would
//! have to be replayed in the same order it was produced, and during a scan
//! BlueZ produces hundreds a second.
//!
//! # Nothing here asks anybody for permission
//!
//! BlueZ has no polkit actions at all: its bus policy lets any client on the
//! system bus send to `org.bluez`, and that is the whole door. So this crate
//! writes no polkit rule (there is nothing to widen), spawns no `pkexec`, and
//! `bluetooth-bus-policy-is-open` is the check that pins the fact for the day
//! upstream changes its mind.
//!
//! # The kind of a device is DERIVED, and never its name
//!
//! What the modeline draws is what the thing IS — headphones as headphones, a
//! keyboard as a keyboard — and that comes from the device's own class (BR/EDR)
//! or appearance (LE), both of which are numbers the device announces. BlueZ's
//! own `Icon` is computed from those two and throws information away (a watch
//! is `None` in both of its tables), so it is read as a hint and never as the
//! answer. A NAME is never parsed: names are what people type.
//!
//! # Invariant 13
//!
//! An address is not a name, but it is an identifier of a machine, and this
//! repository is public: no fixture here carries a real one, and the tests are
//! written against an invented [`bluez::Managed`] map.

pub mod agent;
/// The audio profile, which PipeWire owns and BlueZ cannot answer.
pub mod audio;
pub mod bluez;

use std::collections::BTreeMap;
use std::sync::mpsc;

use anyhow::{Context, Result};

/// The percentage at and under which a device's battery is worth a number on
/// the modeline. The owner's rule, and the same number the machine's own
/// battery uses (`ronoctua_power::PERCENT_UNTIL`).
pub const SHOWN_FROM: u8 = 30;

/// The percentage at and under which that number turns to the danger colour.
/// Literal, like the battery's: charging or not, this is low.
pub const DANGER_FROM: u8 = 15;

/// What a device IS, which is what its glyph draws.
///
/// Fifteen, because that is how many drawings say something different at one
/// cell: a headset is not headphones (it has a microphone, and it is what a
/// call switches to), a tablet is not a phone, a watch is not a wristband.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Kind {
    /// Two cups and a band, no microphone.
    Headphones,
    /// Headphones with a microphone — what a call uses.
    Headset,
    /// A loudspeaker.
    Speaker,
    /// Something typed on.
    Keyboard,
    /// Something pointed with.
    Mouse,
    /// A game controller.
    Gamepad,
    /// A tablet, or a digitiser.
    Tablet,
    /// A telephone.
    Phone,
    /// Another computer.
    Computer,
    /// A watch or a band.
    Watch,
    /// A camera.
    Camera,
    /// A printer.
    Printer,
    /// Something that shares a network.
    Network,
    /// A screen.
    Display,
    /// Nothing said what it is.
    #[default]
    Unknown,
}

impl Kind {
    /// Every kind, in the order the list groups them.
    pub const ALL: [Self; 15] = [
        Self::Headphones,
        Self::Headset,
        Self::Speaker,
        Self::Keyboard,
        Self::Mouse,
        Self::Gamepad,
        Self::Tablet,
        Self::Phone,
        Self::Computer,
        Self::Watch,
        Self::Camera,
        Self::Printer,
        Self::Network,
        Self::Display,
        Self::Unknown,
    ];

    /// The kebab-case word behind `bluetooth-kind-<id>` and
    /// `icon.bluetooth-device-<id>`. One string, two tables.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Headphones => "headphones",
            Self::Headset => "headset",
            Self::Speaker => "speaker",
            Self::Keyboard => "keyboard",
            Self::Mouse => "mouse",
            Self::Gamepad => "gamepad",
            Self::Tablet => "tablet",
            Self::Phone => "phone",
            Self::Computer => "computer",
            Self::Watch => "watch",
            Self::Camera => "camera",
            Self::Printer => "printer",
            Self::Network => "network",
            Self::Display => "display",
            Self::Unknown => "unknown",
        }
    }

    /// What the device says it is: its CLASS first (BR/EDR announces the most
    /// detail), then its APPEARANCE (an LE device has no class), then BlueZ's
    /// own icon as a hint, and last the services it offers.
    ///
    /// The order is not taste. `Class` distinguishes headphones from a headset
    /// and a wristwatch from a jacket; `Appearance` separates a keyboard from a
    /// mouse; `Icon` is derived from those two and loses both of those
    /// distinctions; and a UUID says only what a thing can DO.
    ///
    /// EACH STEP ANSWERS OR STANDS ASIDE. `of_class` returns
    /// `Some(Kind::Unknown)` for a peripheral or a wearable whose shape it
    /// does not recognise — a class it CAN read and cannot name — and taking
    /// that as an answer meant those devices never reached the appearance,
    /// the icon or the services at all, which is the opposite of the order
    /// argued above. `Unknown` from any step is "I have nothing", and the
    /// next step is asked.
    #[must_use]
    pub fn of(class: Option<u32>, appearance: Option<u16>, icon: &str, uuids: &[String]) -> Self {
        let said = |k: Option<Self>| k.filter(|k| *k != Self::Unknown);
        if let Some(k) = said(class.and_then(Self::of_class)) {
            return k;
        }
        if let Some(k) = said(appearance.and_then(Self::of_appearance)) {
            return k;
        }
        match Self::of_icon(icon) {
            Self::Unknown => Self::of_uuids(uuids),
            k => k,
        }
    }

    /// The BR/EDR class of device: six bits of minor under five of major.
    fn of_class(class: u32) -> Option<Self> {
        let major = (class >> 8) & 0x1f;
        let minor = (class >> 2) & 0x3f;
        match major {
            1 => Some(Self::Computer),
            2 => Some(Self::Phone),
            3 => Some(Self::Network),
            // Audio and video: the minor is what separates four drawings.
            //
            // THE FALLBACK IS A SPEAKER, and that is a choice: the ones this
            // does not name are 4 (a microphone), 9 (a set-top box), 11 (a
            // VCR), 17 (a video conference unit) and 19 (a loudspeaker in a
            // car). Every one of them makes SOUND, and a speaker is the
            // drawing that says so — `Unknown` would put the generic
            // bluetooth glyph on something whose whole purpose is audio.
            //
            // A microphone would be worth its own glyph the day one turns up;
            // there is none in the token file, and inventing an approximate
            // one is worse than the honest speaker.
            4 => Some(match minor {
                1 | 2 => Self::Headset,
                5 | 7 | 8 | 10 => Self::Speaker,
                6 => Self::Headphones,
                12 | 13 => Self::Camera,
                14..=16 => Self::Display,
                18 => Self::Gamepad,
                _ => Self::Speaker,
            }),
            // Peripheral: the top two bits of the minor are the pointing and
            // typing flags, and the low nibble is the shape.
            5 => Some(match ((minor >> 4) & 0x3, minor & 0xf) {
                (1 | 3, _) => Self::Keyboard,
                (2, 1 | 2) => Self::Gamepad,
                (2, 5) => Self::Tablet,
                (2, _) => Self::Mouse,
                (_, 1 | 2) => Self::Gamepad,
                (_, 5) => Self::Tablet,
                _ => Self::Unknown,
            }),
            // Imaging: the minor is a set of FLAGS, and they are bits 4..7 of
            // the class — which is bits 2..5 of the minor, because the minor
            // itself starts at bit 2. Testing the class's own numbers against
            // the shifted minor drew a printer as a camera.
            //
            // THE FOUR FLAGS, in the order the specification lists them:
            // display, camera, scanner, printer. The scanner is drawn as a
            // CAMERA because there is no scanner glyph and the two are the
            // same gesture — light in, picture out — and a device with NO
            // flag set is drawn as a printer, which is what an imaging device
            // that will not say what it is almost always turns out to be.
            // The order below is deliberate: printer wins over camera, which
            // wins over display, because a multifunction machine sets more
            // than one and the printer is what somebody sends a job to.
            6 => Some(match minor {
                m if m & 0x20 != 0 => Self::Printer,
                m if m & 0x10 != 0 => Self::Camera,
                m if m & 0x08 != 0 => Self::Camera,
                m if m & 0x04 != 0 => Self::Display,
                _ => Self::Printer,
            }),
            7 => Some(match minor {
                1 => Self::Watch,
                _ => Self::Unknown,
            }),
            _ => None,
        }
    }

    /// The LE appearance: a category of ten bits over six of sub-value.
    fn of_appearance(appearance: u16) -> Option<Self> {
        match appearance {
            961 => return Some(Self::Keyboard),
            962 => return Some(Self::Mouse),
            963 | 964 => return Some(Self::Gamepad),
            965 => return Some(Self::Tablet),
            _ => {}
        }
        // THE SUB-VALUES OF THE AUDIO CATEGORIES, which say which drawing.
        //
        // Category 37 (`0x0940`) is what modern LE-Audio earbuds advertise,
        // and a pure LE device has NO `Class` and no BR/EDR service UUID — so
        // without these it fell all the way through `Kind::of` to `Unknown`
        // and drew the generic bluetooth glyph. That is requirement 2 failing
        // on the class of device most likely to be bought next, and BlueZ's
        // own `appearance_to_icon` does not cover them either.
        match appearance {
            0x0941 => return Some(Self::Headset),
            0x0942 | 0x0943 => return Some(Self::Headphones),
            0x0944 => return Some(Self::Speaker),
            _ => {}
        }
        match appearance >> 6 {
            1 => Some(Self::Phone),
            2 => Some(Self::Computer),
            3 | 4 => Some(Self::Watch),
            5 => Some(Self::Display),
            10 => Some(Self::Speaker),
            // 33 (`0x0840`) is Audio Sink and 37 (`0x0940`) is Wearable
            // Audio Device; the four sub-values above name which one it is,
            // and a category with no sub-value we know is still audio.
            33 => Some(Self::Speaker),
            37 => Some(Self::Headphones),
            _ => None,
        }
    }

    /// BlueZ's own word, which is a freedesktop icon name.
    fn of_icon(icon: &str) -> Self {
        match icon {
            "audio-headphones" => Self::Headphones,
            "audio-headset" => Self::Headset,
            "audio-card" | "audio-speakers" => Self::Speaker,
            "input-keyboard" => Self::Keyboard,
            "input-mouse" => Self::Mouse,
            "input-gaming" => Self::Gamepad,
            "input-tablet" => Self::Tablet,
            "phone" => Self::Phone,
            "computer" => Self::Computer,
            "camera-photo" | "camera-video" => Self::Camera,
            "printer" => Self::Printer,
            "network-wireless" => Self::Network,
            "video-display" => Self::Display,
            _ => Self::Unknown,
        }
    }

    /// The last resort: what it can DO. Only the two that are unambiguous.
    fn of_uuids(uuids: &[String]) -> Self {
        let has = |short: &str| {
            uuids
                .iter()
                .any(|u| u.get(4..8).is_some_and(|s| s == short))
        };
        if has("111e") || has("1108") {
            return Self::Headset;
        }
        if has("110b") {
            return Self::Speaker;
        }
        Self::Unknown
    }
}

/// One radio.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Adapter {
    /// Its object path — the id every order carries.
    pub at: String,
    /// The address, shown in the radio's own rows.
    pub address: String,
    /// What this machine calls itself to others.
    pub alias: String,
    /// Its own name, which the alias overrides.
    pub name: String,
    /// Is it on?
    pub powered: bool,
    /// `on`, `off`, `off-enabling`, `on-disabling`, `off-blocked` — the last
    /// is the hardware switch, and it is the honest answer a bare `Powered`
    /// cannot give.
    pub power_state: String,
    /// Can others see this machine?
    pub discoverable: bool,
    /// For how long, in seconds; zero is for ever.
    pub discoverable_timeout: u32,
    /// Can others pair with it?
    pub pairable: bool,
    /// Is it looking right now?
    pub discovering: bool,
    /// For how long others may pair, in seconds; zero is for ever.
    ///
    /// The discoverable one had a ring of its own from the first day and this
    /// had nothing — a pair of settings that answer the same shape of
    /// question, and only one of them was on screen.
    pub pairable_timeout: u32,
    /// The device class this machine advertises, as a number.
    pub class: u32,
    /// The manufacturer's own id and the HCI version, which is what a person
    /// looking up "does this radio do LE Audio" actually needs.
    pub manufacturer: u16,
    /// The HCI version, as the daemon numbers it.
    pub version: u16,
    /// `bluetooth:v001Dp1200d1436` — the string a bug report asks for.
    pub modalias: String,
    /// `central`, `peripheral`, `central-peripheral`.
    pub roles: Vec<String>,
    /// Every service THIS MACHINE offers.
    pub uuids: Vec<String>,
}

/// An audio stream to or from a device — a CHILD object, not a property.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Transport {
    /// The service it carries.
    pub uuid: String,
    /// `idle`, `pending` or `active`.
    pub state: String,
    /// The codec byte, as the daemon reports it.
    ///
    /// It is a NUMBER on the bus and a name on the screen (`codec_key`): `0`
    /// is SBC and `2` is AAC, and a row reading `0` says nothing to anybody.
    pub codec: Option<u8>,
    /// The stream's own volume, when the device lets it be set.
    pub volume: Option<u16>,
}

/// One device the radio knows about.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Device {
    /// Its object path — the id every order carries.
    pub at: String,
    /// The address. Never logged, never in a fixture.
    pub address: String,
    /// `public` or `random`.
    pub address_type: String,
    /// What we call it — the alias if it has one, else its own name.
    pub label: String,
    /// Its own name, when it gives one.
    pub name: String,
    /// The alias, when somebody set one.
    pub alias: String,
    /// What it is.
    pub kind: Kind,
    /// Is the link up?
    pub connected: bool,
    /// Did the pairing exchange happen?
    pub paired: bool,
    /// Were the keys KEPT? A device that is paired but not bonded is a
    /// device that will ask again after a reboot.
    pub bonded: bool,
    /// May it connect on its own?
    pub trusted: bool,
    /// Is it refused?
    pub blocked: bool,
    /// Old-style pairing, which is the one with a PIN.
    pub legacy: bool,
    /// May it wake this machine?
    pub wake: Option<bool>,
    /// Has the daemon read its services yet?
    pub services_resolved: bool,
    /// How much battery it says it has.
    pub battery: Option<u8>,
    /// Who says so — BlueZ's GATT client, or another program.
    pub battery_source: Option<String>,
    /// How loud it is heard, in dBm; absent until it is heard.
    pub rssi: Option<i16>,
    /// How loud it says it speaks.
    pub tx_power: Option<i16>,
    /// The BR/EDR class, when it has one.
    pub class: Option<u32>,
    /// The LE appearance, when it has one.
    pub appearance: Option<u16>,
    /// BlueZ's own icon word.
    pub icon: String,
    /// The firmware id — `usb:v05ACp024Fd011B`.
    pub modalias: Option<String>,
    /// The radio it belongs to.
    pub adapter: String,
    /// Everything it says it can do.
    pub uuids: Vec<String>,
    /// Classic HID's reconnect rule; an LE keyboard has none.
    pub reconnect: Option<String>,
    /// Its audio streams, when it has any.
    pub transports: Vec<Transport>,
}

impl Device {
    /// Does the battery deserve a number beside the glyph?
    #[must_use]
    pub fn shows_percent(&self) -> bool {
        self.battery.is_some_and(|p| p <= SHOWN_FROM)
    }

    /// Is it low enough for the danger colour?
    #[must_use]
    pub fn alarmed(&self) -> bool {
        self.battery.is_some_and(|p| p <= DANGER_FROM)
    }

    /// Has it a name of its own, or only an address?
    ///
    /// # Why the alias is not enough on its own
    ///
    /// Because BlueZ makes one up. `dev_property_get_alias` falls back, when
    /// there is neither an alias nor a name, to the ADDRESS with `:` replaced
    /// by `-` — which is why `bluetoothctl devices` prints nameless devices as
    /// `AA-BB-CC-DD-EE-FF`. Read literally, `!alias.is_empty()` is then always
    /// true, `hidden` is always zero, and `h`, `bluetooth-unnamed` and the
    /// whole sentence about hidden rows do nothing on a real radio.
    ///
    /// So the address in that spelling is not a name. Nothing is lost if the
    /// daemon changes its mind: a device whose alias really is its address is
    /// a device with no name either way.
    #[must_use]
    pub fn named(&self) -> bool {
        if !self.name.is_empty() {
            return true;
        }
        !self.alias.is_empty() && self.alias != self.address.replace(':', "-")
    }
}

/// The whole radio as of one reading.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Scene {
    /// Every radio this machine has, in path order.
    pub adapters: Vec<Adapter>,
    /// Every device the daemon knows, in path order.
    pub devices: Vec<Device>,
    /// Did the daemon answer at all?
    pub daemon: bool,
}

impl Scene {
    /// The first radio, which is the one everything acts on.
    #[must_use]
    pub fn adapter(&self) -> Option<&Adapter> {
        self.adapters.first()
    }

    /// The paired devices, connected first, then by kind, then by label.
    #[must_use]
    pub fn paired(&self) -> Vec<&Device> {
        let mut out: Vec<&Device> = self.devices.iter().filter(|d| d.paired).collect();
        out.sort_by(|a, b| {
            b.connected
                .cmp(&a.connected)
                .then(a.kind.cmp(&b.kind))
                .then(a.label.cmp(&b.label))
        });
        out
    }

    /// Everything else the radio has heard: the ones that give a name first,
    /// then alphabetically.
    ///
    /// NOT BY SIGNAL, which is the tempting order and the wrong one: the RSSI
    /// of a device across the room moves several dBm a second, so a list
    /// sorted by it rearranges itself under the hand that is reaching for a
    /// row. The signal is on the row, where it can be read without deciding
    /// where the row goes.
    #[must_use]
    pub fn nearby(&self) -> Vec<&Device> {
        let mut out: Vec<&Device> = self.devices.iter().filter(|d| !d.paired).collect();
        out.sort_by(|a, b| {
            b.named()
                .cmp(&a.named())
                .then(a.label.cmp(&b.label))
                .then(a.address.cmp(&b.address))
        });
        out
    }

    /// The connected devices, which is what the modeline draws.
    #[must_use]
    pub fn connected(&self) -> Vec<&Device> {
        let mut out: Vec<&Device> = self.devices.iter().filter(|d| d.connected).collect();
        // ALARMED FIRST: the one that is about to die is the one to see.
        out.sort_by(|a, b| {
            b.alarmed()
                .cmp(&a.alarmed())
                .then(a.kind.cmp(&b.kind))
                .then(a.label.cmp(&b.label))
        });
        out
    }

    /// The device at this path.
    #[must_use]
    pub fn device(&self, at: &str) -> Option<&Device> {
        self.devices.iter().find(|d| d.at == at)
    }

    /// Builds the scene out of the map `GetManagedObjects` answers with.
    #[must_use]
    pub fn of(managed: &bluez::Managed) -> Self {
        let mut adapters = Vec::new();
        let mut devices = Vec::new();
        let mut transports: BTreeMap<String, Vec<Transport>> = BTreeMap::new();

        for (at, faces) in managed {
            let at = at.as_str().to_owned();
            if let Some(f) = faces.get(bluez::TRANSPORT) {
                let owner = bluez::path(f, "Device").unwrap_or_default();
                transports.entry(owner).or_default().push(Transport {
                    uuid: bluez::text(f, "UUID"),
                    state: bluez::text(f, "State"),
                    codec: bluez::small(f, "Codec"),
                    volume: bluez::medium(f, "Volume"),
                });
            }
            if let Some(f) = faces.get(bluez::ADAPTER) {
                adapters.push(Adapter {
                    at: at.clone(),
                    address: bluez::text(f, "Address"),
                    alias: bluez::text(f, "Alias"),
                    name: bluez::text(f, "Name"),
                    powered: bluez::flag(f, "Powered"),
                    power_state: bluez::text(f, "PowerState"),
                    discoverable: bluez::flag(f, "Discoverable"),
                    discoverable_timeout: bluez::number(f, "DiscoverableTimeout").unwrap_or(0),
                    pairable: bluez::flag(f, "Pairable"),
                    discovering: bluez::flag(f, "Discovering"),
                    pairable_timeout: bluez::number(f, "PairableTimeout").unwrap_or(0),
                    class: bluez::number(f, "Class").unwrap_or(0),
                    manufacturer: bluez::medium(f, "Manufacturer").unwrap_or(0),
                    version: bluez::medium(f, "Version").unwrap_or(0),
                    modalias: bluez::text(f, "Modalias"),
                    roles: bluez::words(f, "Roles"),
                    uuids: bluez::words(f, "UUIDs"),
                });
            }
            if let Some(f) = faces.get(bluez::DEVICE) {
                let name = bluez::text(f, "Name");
                let alias = bluez::text(f, "Alias");
                let address = bluez::text(f, "Address");
                let class = bluez::number(f, "Class");
                let appearance = bluez::medium(f, "Appearance");
                let icon = bluez::text(f, "Icon");
                let uuids = bluez::words(f, "UUIDs");
                let battery = faces.get(bluez::BATTERY);
                devices.push(Device {
                    at: at.clone(),
                    // THE LABEL IS WHAT WE CALL IT, and the fallback is the
                    // address: a device with no name is still a row one can
                    // stand on, and it is the only thing it has.
                    // AND BLUEZ'S MADE-UP ALIAS IS NOT A LABEL EITHER: with
                    // no name it hands back the address with dashes, so a row
                    // read `AA-BB-CC-…` beside a facts panel whose "called:"
                    // line said the same thing twice. `named()` carries the
                    // argument.
                    label: if !alias.is_empty() && alias != address.replace(':', "-") {
                        alias.clone()
                    } else if !name.is_empty() {
                        name.clone()
                    } else {
                        address.clone()
                    },
                    address,
                    address_type: bluez::text(f, "AddressType"),
                    kind: Kind::of(class, appearance, &icon, &uuids),
                    connected: bluez::flag(f, "Connected"),
                    paired: bluez::flag(f, "Paired"),
                    bonded: bluez::flag(f, "Bonded"),
                    trusted: bluez::flag(f, "Trusted"),
                    blocked: bluez::flag(f, "Blocked"),
                    legacy: bluez::flag(f, "LegacyPairing"),
                    wake: f.get("WakeAllowed").and_then(|v| bool::try_from(v).ok()),
                    services_resolved: bluez::flag(f, "ServicesResolved"),
                    battery: battery.and_then(|b| bluez::small(b, "Percentage")),
                    battery_source: battery.and_then(|b| bluez::maybe_text(b, "Source")),
                    rssi: bluez::signed(f, "RSSI"),
                    tx_power: bluez::signed(f, "TxPower"),
                    class,
                    appearance,
                    icon,
                    modalias: bluez::maybe_text(f, "Modalias"),
                    adapter: bluez::path(f, "Adapter").unwrap_or_default(),
                    uuids,
                    reconnect: faces
                        .get(bluez::INPUT)
                        .and_then(|i| bluez::maybe_text(i, "ReconnectMode")),
                    transports: Vec::new(),
                    name,
                    alias,
                });
            }
        }
        adapters.sort_by(|a, b| a.at.cmp(&b.at));
        devices.sort_by(|a, b| a.at.cmp(&b.at));
        for d in &mut devices {
            if let Some(mut t) = transports.remove(&d.at) {
                // A STABLE ORDER: the map they came out of has none, and a
                // device with two streams would list them differently on
                // every reading.
                t.sort_by(|a, b| a.uuid.cmp(&b.uuid));
                d.transports = t;
            }
        }
        Self {
            adapters,
            devices,
            daemon: true,
        }
    }
}

/// What a service id means, as a text key — `None` for one nobody named.
///
/// The list is what a person meets: the audio profiles, the input one, the
/// file ones and the phone's three. The rest are drawn as the raw id, which
/// is the truth and is what a search engine takes.
#[must_use]
pub fn profile_key(uuid: &str) -> Option<&'static str> {
    let short = uuid.get(4..8)?;
    Some(match short {
        "1108" => "bluetooth-profile-headset",
        "111e" | "111f" => "bluetooth-profile-handsfree",
        "110b" => "bluetooth-profile-audio-sink",
        "110a" => "bluetooth-profile-audio-source",
        "110c" | "110e" => "bluetooth-profile-remote-control",
        "1124" => "bluetooth-profile-human-interface",
        "1106" => "bluetooth-profile-file-transfer",
        "1105" => "bluetooth-profile-object-push",
        "112f" | "1130" => "bluetooth-profile-phone-book",
        "1132" | "1133" => "bluetooth-profile-message-access",
        "1116" | "1115" => "bluetooth-profile-network",
        "180f" => "bluetooth-profile-battery",
        "180a" => "bluetooth-profile-device-information",
        "1101" => "bluetooth-profile-serial",
        "112d" => "bluetooth-profile-sim-access",
        _ => return None,
    })
}

/// What a daemon refused with, as a name rather than a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The bus would not let us — which BlueZ never says, and a changed
    /// policy would.
    NotAuthorized,
    /// There is no such device, adapter or profile any more.
    NotAvailable,
    /// It is not there to answer: out of range, or off.
    Away,
    /// Something else is already doing this.
    Busy,
    /// The other side said no.
    Rejected,
    /// Anything else; the words travel alongside.
    Other,
}

/// Reads the refusal out of an error, whatever it is wrapped in.
///
/// BlueZ answers with THIRTY names and no sentences worth showing, so the
/// name is what is read and the words only travel in the echo. Both spellings
/// of the invalid-argument error are real, from different call sites in the
/// same daemon.
#[must_use]
pub fn refusal(e: &anyhow::Error) -> Refusal {
    let named = |name: &str| match name {
        "org.freedesktop.DBus.Error.AccessDenied"
        | "org.freedesktop.DBus.Error.AuthFailed"
        | "org.freedesktop.DBus.Error.InteractiveAuthorizationRequired" => Refusal::NotAuthorized,
        "org.freedesktop.DBus.Error.ServiceUnknown"
        | "org.freedesktop.DBus.Error.NameHasNoOwner"
        | "org.freedesktop.DBus.Error.UnknownObject"
        | "org.freedesktop.DBus.Error.UnknownMethod"
        | "org.freedesktop.DBus.Error.InvalidArgs"
        | "org.bluez.Error.DoesNotExist"
        | "org.bluez.Error.NotSupported"
        | "org.bluez.Error.NotAvailable"
        | "org.bluez.Error.NotExist"
        | "org.bluez.Error.NotConnected"
        | "org.bluez.Error.NotPermitted"
        | "org.bluez.Error.InvalidArguments"
        | "org.bluez.Error.NotReady" => Refusal::NotAvailable,
        "org.bluez.Error.Failed"
        | "org.bluez.Error.ConnectionAttemptFailed"
        | "org.bluez.Error.AuthenticationTimeout" => Refusal::Away,
        "org.bluez.Error.InProgress" | "org.bluez.Error.Busy" | "org.bluez.Error.AlreadyExists" => {
            Refusal::Busy
        }
        "org.bluez.Error.AuthenticationRejected"
        | "org.bluez.Error.AuthenticationCanceled"
        | "org.bluez.Error.AuthenticationFailed"
        | "org.bluez.Error.Rejected"
        | "org.bluez.Error.Canceled" => Refusal::Rejected,
        _ => Refusal::Other,
    };
    for cause in e.chain() {
        if let Some(zbus::Error::MethodError(name, _, _)) = cause.downcast_ref::<zbus::Error>() {
            return named(name.as_str());
        }
        if let Some(f) = cause.downcast_ref::<zbus::fdo::Error>() {
            use zbus::fdo::Error as F;
            return match f {
                F::ZBus(zbus::Error::MethodError(name, _, _)) => named(name.as_str()),
                F::AccessDenied(_) | F::AuthFailed(_) => Refusal::NotAuthorized,
                F::ServiceUnknown(_) | F::UnknownObject(_) | F::UnknownMethod(_) => {
                    Refusal::NotAvailable
                }
                _ => Refusal::Other,
            };
        }
    }
    Refusal::Other
}

/// One thing to do, sent to the orders thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// Connect every profile of this device. Blocks for seconds.
    Connect(String),
    /// Drop the link.
    Disconnect(String),
    /// Connect or drop ONE profile of it.
    Profile {
        /// The device's path.
        device: String,
        /// The service id.
        uuid: String,
        /// True connects, false disconnects.
        on: bool,
    },
    /// Pair with it — the questions come through the agent.
    Pair(String),
    /// Forget it: the keys go.
    Forget(String),
    /// Trust it, or stop.
    Trust(String, bool),
    /// Block it, or stop.
    Block(String, bool),
    /// Call it something else.
    Rename(String, String),
    /// Turn the radio on or off.
    Power(bool),
    /// Let others see this machine, or stop.
    Discoverable(bool),
    /// Let others pair with this machine, or stop.
    Pairable(bool),
    /// Call THIS MACHINE something else — what other devices see.
    AdapterAlias(String),
    /// How long this machine stays visible, in seconds. Zero is for ever.
    DiscoverableTimeout(u32),
    /// How long this machine stays open to pairing. Zero is for ever.
    PairableTimeout(u32),
    /// May this device wake the machine from sleep?
    ///
    /// `WakeAllowed` is `readwrite` on `Device1` and both keyboards on the
    /// owner's radio carry it — it was DRAWN as a fact with no way to change
    /// it, which is the shape requirement 3 asks about: "every option and
    /// every setting".
    Wake(String, bool),
    /// Look for devices, or stop looking.
    Scan(bool),
}

/// What the watching threads send the footer.
#[derive(Debug)]
pub enum Event {
    /// A fresh reading. The last one wins.
    Scene(Box<Scene>),
    /// An order was carried out, or refused — the words are the daemon's.
    Done {
        /// What was asked.
        ask: Ask,
        /// The refusal and the daemon's own words, when refused.
        refused: Option<(Refusal, String)>,
    },
    /// The other side is asking something, and nothing pairs until it is
    /// answered. One at a time: bluetoothd asks one question per pairing.
    Asked(agent::Question),
    /// The question was taken back — the other side gave up, or timed out.
    Unasked,
}

/// The footer's side of the orders thread.
#[derive(Debug, Clone)]
pub struct Hand {
    orders: mpsc::Sender<Ask>,
}

impl Hand {
    /// Sends one order; the answer comes back as a fresh [`Event::Scene`] and
    /// then [`Event::Done`].
    pub fn send(&self, ask: Ask) {
        if self.orders.send(ask).is_err() {
            tracing::warn!("the bluetooth thread is gone; the order was dropped");
        }
    }
}

/// The conversation with BlueZ, on one system-bus connection.
pub struct Manager {
    connection: zbus::blocking::Connection,
}

impl Manager {
    /// Opens the system bus.
    ///
    /// # Errors
    ///
    /// There is no system bus. An ABSENT bluetoothd is not an error: it is an
    /// empty scene, and the view says so.
    pub fn fresh() -> Result<Self> {
        Ok(Self {
            connection: zbus::blocking::Connection::system()
                .context("no system bus to talk to bluez")?,
        })
    }

    /// The whole radio, in one call. INFALLIBLE: a daemon that is not there
    /// answers an empty scene, which is what the view draws.
    #[must_use]
    pub fn read(&self) -> Scene {
        let Ok(objects) = bluez::ObjectsProxyBlocking::new(&self.connection) else {
            return Scene::default();
        };
        match objects.get_managed_objects() {
            Ok(managed) => Scene::of(&managed),
            Err(e) => {
                tracing::debug!(error = %e, "bluez is not answering");
                Scene::default()
            }
        }
    }

    /// A blocking stream of everything BlueZ says.
    ///
    /// # Errors
    ///
    /// The match rule could not be installed.
    ///
    /// # Why two rules and not a path namespace
    ///
    /// `InterfacesAdded` and `InterfacesRemoved` are emitted from `/`, so a
    /// namespace under `/org/bluez` would carry every property change and no
    /// arrival at all — a device would appear on the screen only when it next
    /// changed something.
    pub fn signals(&self) -> Result<zbus::blocking::MessageIterator> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender(bluez::SERVICE)?
            .build();
        Ok(zbus::blocking::MessageIterator::for_match_rule(
            rule,
            &self.connection,
            None,
        )?)
    }

    /// A blocking stream of the BUS saying who came and went.
    ///
    /// # Errors
    ///
    /// The match rule could not be installed.
    ///
    /// # Why this one is separate
    ///
    /// A bluetoothd that DIES sends nothing: its objects do not go away one
    /// by one, the whole name does. That announcement comes from the broker
    /// (`org.freedesktop.DBus`), not from `org.bluez`, so the rule above
    /// cannot carry it — and without it the modeline would keep drawing a
    /// glyph for a link that no longer exists.
    pub fn owner_signals(&self) -> Result<zbus::blocking::MessageIterator> {
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender("org.freedesktop.DBus")?
            .interface("org.freedesktop.DBus")?
            .member("NameOwnerChanged")?
            .add_arg(bluez::SERVICE)?
            .build();
        Ok(zbus::blocking::MessageIterator::for_match_rule(
            rule,
            &self.connection,
            None,
        )?)
    }

    /// Carries out one order.
    ///
    /// # Errors
    ///
    /// Whatever the daemon said, by name — see [`refusal`].
    pub fn carry(&self, ask: &Ask, scene: &Scene) -> Result<()> {
        let device = |at: &str| -> Result<bluez::DeviceProxyBlocking<'_>> {
            Ok(bluez::DeviceProxyBlocking::builder(&self.connection)
                .path(at.to_owned())?
                .build()?)
        };
        let adapter = || -> Result<bluez::AdapterProxyBlocking<'_>> {
            let at = scene
                .adapter()
                .map(|a| a.at.clone())
                .context("there is no bluetooth adapter")?;
            Ok(bluez::AdapterProxyBlocking::builder(&self.connection)
                .path(at)?
                .build()?)
        };
        match ask {
            Ask::Connect(at) => device(at)?.connect()?,
            Ask::Disconnect(at) => device(at)?.disconnect()?,
            Ask::Profile {
                device: at,
                uuid,
                on,
            } => {
                if *on {
                    device(at)?.connect_profile(uuid)?;
                } else {
                    device(at)?.disconnect_profile(uuid)?;
                }
            }
            Ask::Pair(at) => device(at)?.pair()?,
            Ask::Forget(at) => {
                // THROUGH THE DEVICE'S OWN RADIO, and not the first one.
                //
                // `RemoveDevice` lives on `Adapter1`, so it needs a radio —
                // and it needs THE radio the device is under. With a
                // built-in `hci0` and a USB dongle `hci1`, forgetting
                // something paired through the dongle called
                // `hci0.RemoveDevice("/org/bluez/hci1/dev_…")`, which
                // answers `DoesNotExist`: on screen, "there is no such thing
                // on this machine", about a device drawn two rows above,
                // which then stayed paired for ever.
                //
                // `Device.adapter` is read from the bus and carried in the
                // scene precisely for this. The first radio is the fallback
                // for a device the reading has no adapter for, which is what
                // every other adapter-scoped order uses by design — those
                // are ABOUT the radio, and this one is not.
                let path = zbus::zvariant::ObjectPath::try_from(at.as_str())?;
                let owner = scene
                    .device(at)
                    .map(|d| d.adapter.clone())
                    .filter(|a| !a.is_empty());
                match owner {
                    Some(a) => bluez::AdapterProxyBlocking::builder(&self.connection)
                        .path(a)?
                        .build()?
                        .remove_device(&path)?,
                    None => adapter()?.remove_device(&path)?,
                }
            }
            Ask::Trust(at, on) => device(at)?.set_trusted(*on)?,
            Ask::Block(at, on) => device(at)?.set_blocked(*on)?,
            Ask::Rename(at, name) => device(at)?.set_alias(name)?,
            Ask::Wake(at, on) => device(at)?.set_wake_allowed(*on)?,
            Ask::Power(on) => adapter()?.set_powered(*on)?,
            Ask::Discoverable(on) => adapter()?.set_discoverable(*on)?,
            Ask::Pairable(on) => adapter()?.set_pairable(*on)?,
            Ask::AdapterAlias(name) => adapter()?.set_alias(name)?,
            Ask::DiscoverableTimeout(seconds) => {
                adapter()?.set_discoverable_timeout(*seconds)?;
            }
            Ask::PairableTimeout(seconds) => {
                adapter()?.set_pairable_timeout(*seconds)?;
            }
            Ask::Scan(on) => {
                let a = adapter()?;
                if *on {
                    // QUIET THE STORM BEFORE IT STARTS: with `DuplicateData`
                    // off, an advertisement that says nothing new says
                    // nothing at all, and a scan stops being hundreds of
                    // property changes a second.
                    let no = zbus::zvariant::Value::from(false);
                    let auto = zbus::zvariant::Value::from("auto");
                    let mut filter = std::collections::HashMap::new();
                    filter.insert("DuplicateData", &no);
                    filter.insert("Transport", &auto);
                    if let Err(e) = a.set_discovery_filter(filter) {
                        tracing::debug!(error = %e, "the discovery filter was refused");
                    }
                    a.start_discovery()?;
                } else {
                    a.stop_discovery()?;
                }
            }
        }
        Ok(())
    }
}

/// Everything a `watch` raised, kept alive by the door holding it.
pub struct Watch {
    /// Where orders go.
    pub hand: Hand,
    /// Where an ANSWER goes, and it does not go through the orders thread.
    ///
    /// `Device1.Pair` blocks that thread for as long as the pairing takes,
    /// and the question the daemon asks arrives in the MIDDLE of it — an
    /// answer queued behind the order it is about would wait for the order,
    /// which is waiting for the answer. This handle reaches the agent's own
    /// task directly, and answering is a channel send.
    pub questions: agent::Questions,
}

/// Watches BlueZ and carries out orders, on four threads.
///
/// # `serve_agent`, and why it is not always true
///
/// Only ONE door registers the pairing agent, and it is the footer: the
/// object has to outlive the question, `RequestDefaultAgent` gives the last
/// caller the questions, and a window that registered one would take them
/// away from the footer and give them back when it closed. The window opens
/// with `false` and says where the question is answered.
///
/// # Why four
///
/// One holds the signal stream and does nothing else, because reading the
/// scene inside it would make a scan — hundreds of advertisements a second —
/// into hundreds of round trips. One holds the BUS's own stream, which is
/// where a daemon that DIES is announced (`NameOwnerChanged`); without it the
/// modeline would keep drawing a device that no longer exists. One reads, at
/// most four times a second, when either of them says something changed. And
/// one carries out orders, because `Device1.Connect` blocks for seconds and a
/// call that blocks inside the footer's loop is a frozen footer. The orders
/// thread also SERVES the pairing agent: the object it registers has to
/// outlive the question.
///
/// # Errors
///
/// No system bus, or a thread could not be raised. An absent bluetoothd is
/// not an error: the scene is empty and the screen says so.
pub fn watch<F>(on_event: F, serve_agent: bool) -> Result<Watch>
where
    F: Fn(Event) + Send + Sync + 'static,
{
    let on_event = std::sync::Arc::new(on_event);
    let (orders, inbox) = mpsc::channel::<Ask>();
    let ring = std::sync::Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));

    let listener = Manager::fresh()?;
    let bell = ring.clone();
    std::thread::Builder::new()
        .name("bt-signals".to_owned())
        .spawn(move || {
            let Ok(messages) = listener.signals() else {
                tracing::warn!("no bluez stream; the radio will not reread on its own");
                return;
            };
            for _ in messages {
                let (lock, wake) = &*bell;
                if let Ok(mut dirty) = lock.lock() {
                    *dirty = true;
                    wake.notify_one();
                }
            }
            tracing::info!("the bluez stream ended; no automatic reread");
        })
        .context("could not start the bluetooth signal thread")?;

    let owner = Manager::fresh()?;
    let bell = ring.clone();
    let questions = agent::Questions::idle();
    let theirs = questions.clone();
    let feed = on_event.clone();
    std::thread::Builder::new()
        .name("bt-owner".to_owned())
        .spawn(move || {
            // THE PAIRING AGENT LIVES ON THIS THREAD'S CONNECTION, and this is
            // the thread that hears bluetoothd come and go. It used to be put
            // up once by the orders thread, which hears nothing: a rebuild
            // that touched `main.conf` restarted the daemon and the footer
            // spent the rest of the session unable to ask a single pairing
            // question, saying "the other side said no" instead.
            if serve_agent {
                agent::serve(&owner.connection, &theirs, feed);
            }
            let Ok(messages) = owner.owner_signals() else {
                tracing::warn!("no name watch; a daemon that dies will go unnoticed");
                return;
            };
            for _ in messages {
                // AND IT IS ASKED FOR AGAIN EVERY TIME THE NAME MOVES. The
                // object is already on our bus; what died with the daemon is
                // its side of the registration. Calling it when the daemon
                // just LEFT costs one refusal in the journal, which is
                // cheaper than deciding here whether the name was taken or
                // released and being wrong about it.
                if serve_agent {
                    agent::register(&owner.connection);
                }
                let (lock, wake) = &*bell;
                if let Ok(mut dirty) = lock.lock() {
                    *dirty = true;
                    wake.notify_one();
                }
            }
        })
        .context("could not start the bluetooth name thread")?;

    let reader = Manager::fresh()?;
    let feed = on_event.clone();
    let bell = ring;
    std::thread::Builder::new()
        .name("bt-scene".to_owned())
        .spawn(move || {
            feed(Event::Scene(Box::new(reader.read())));
            loop {
                {
                    let (lock, wake) = &*bell;
                    let Ok(mut dirty) = lock.lock() else { return };
                    while !*dirty {
                        let Ok(next) = wake.wait(dirty) else { return };
                        dirty = next;
                    }
                    *dirty = false;
                }
                feed(Event::Scene(Box::new(reader.read())));
                // THE FLOOR BETWEEN TWO READINGS. A scan changes something
                // hundreds of times a second and no eye reads that; four
                // readings a second is already more than the screen shows.
                std::thread::sleep(std::time::Duration::from_millis(250));
            }
        })
        .context("could not start the bluetooth reading thread")?;

    let hands = Manager::fresh()?;
    let feed = on_event;
    std::thread::Builder::new()
        .name("bt-orders".to_owned())
        .spawn(move || {
            for ask in inbox {
                // THE SCENE IS READ FRESH FOR EVERY ORDER: an adapter that
                // appeared after this thread started is one this thread has
                // to be able to act on, and a reading is one round trip.
                let scene = hands.read();
                let result = hands.carry(&ask, &scene);
                feed(Event::Scene(Box::new(hands.read())));
                let refused = result.err().map(|e| {
                    (
                        refusal(&e),
                        e.chain().last().map(|c| c.to_string()).unwrap_or_default(),
                    )
                });
                feed(Event::Done { ask, refused });
            }
        })
        .context("could not start the bluetooth orders thread")?;

    Ok(Watch {
        hand: Hand { orders },
        questions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

    /// An INVENTED map, in the shape `GetManagedObjects` answers with. No
    /// address here belongs to anything (invariant 13).
    /// One interface's properties, as the fixture writes them.
    type Props<'a> = Vec<(&'a str, Value<'static>)>;
    /// One object's interfaces.
    type Entry<'a> = (&'a str, Vec<(&'a str, Props<'a>)>);

    fn managed(entries: Vec<Entry<'_>>) -> bluez::Managed {
        entries
            .into_iter()
            .map(|(at, faces)| {
                let faces = faces
                    .into_iter()
                    .map(|(name, props)| {
                        let props = props
                            .into_iter()
                            .map(|(k, v)| (k.to_owned(), OwnedValue::try_from(v).unwrap()))
                            .collect();
                        (name.to_owned(), props)
                    })
                    .collect();
                (OwnedObjectPath::try_from(at).unwrap(), faces)
            })
            .collect()
    }

    fn a_keyboard() -> bluez::Managed {
        managed(vec![
            (
                "/org/bluez/hci0",
                vec![(
                    bluez::ADAPTER,
                    vec![
                        ("Address", Value::from("00:00:00:00:00:01")),
                        ("Alias", Value::from("a machine")),
                        ("Powered", Value::from(true)),
                        ("PowerState", Value::from("on")),
                        ("Discoverable", Value::from(false)),
                        ("DiscoverableTimeout", Value::from(180u32)),
                        ("Pairable", Value::from(true)),
                        ("Discovering", Value::from(false)),
                    ],
                )],
            ),
            (
                "/org/bluez/hci0/dev_00_00_00_00_00_02",
                vec![
                    (
                        bluez::DEVICE,
                        vec![
                            ("Address", Value::from("00:00:00:00:00:02")),
                            ("AddressType", Value::from("random")),
                            ("Alias", Value::from("a keyboard")),
                            ("Appearance", Value::from(961u16)),
                            ("Icon", Value::from("input-keyboard")),
                            ("Connected", Value::from(true)),
                            ("Paired", Value::from(true)),
                            ("Bonded", Value::from(true)),
                            ("Trusted", Value::from(true)),
                            (
                                "Adapter",
                                Value::from(
                                    zbus::zvariant::ObjectPath::try_from("/org/bluez/hci0")
                                        .unwrap(),
                                ),
                            ),
                            (
                                "UUIDs",
                                Value::from(vec![
                                    "00001124-0000-1000-8000-00805f9b34fb".to_owned(),
                                    "0000180f-0000-1000-8000-00805f9b34fb".to_owned(),
                                ]),
                            ),
                        ],
                    ),
                    (
                        bluez::BATTERY,
                        vec![
                            ("Percentage", Value::from(12u8)),
                            ("Source", Value::from("GATT Battery Service")),
                        ],
                    ),
                ],
            ),
        ])
    }

    #[test]
    fn a_scene_is_built_out_of_the_managed_map() {
        let s = Scene::of(&a_keyboard());
        assert!(s.daemon);
        assert_eq!(s.adapters.len(), 1);
        assert_eq!(s.devices.len(), 1);
        let a = s.adapter().unwrap();
        assert!(a.powered);
        assert_eq!(a.power_state, "on");
        let d = &s.devices[0];
        assert_eq!(d.kind, Kind::Keyboard);
        assert_eq!(d.label, "a keyboard");
        assert_eq!(d.battery, Some(12));
        assert_eq!(d.battery_source.as_deref(), Some("GATT Battery Service"));
        assert!(d.connected && d.paired && d.bonded && d.trusted);
        // The absent ones are absent, not zero.
        assert_eq!(d.rssi, None);
        assert_eq!(d.tx_power, None);
        assert_eq!(d.modalias, None);
        assert_eq!(d.reconnect, None, "an LE keyboard carries no Input1");
        assert_eq!(d.uuids.len(), 2);
    }

    #[test]
    fn bluez_own_made_up_alias_is_not_a_name() {
        // WHAT BLUEZ DOES: with neither an alias nor a name,
        // `dev_property_get_alias` hands back the ADDRESS with `:` turned into
        // `-`. Read literally, that made every device "named" — so `h` never
        // hid anything, the "N hidden" sentence never appeared, and the facts
        // panel drew a "called:" row repeating the address.
        //
        // AND IT GOES THROUGH `Scene::of`, which is where the whole fallback
        // chain lives: the other tests build `Device` by hand with `label`
        // already set, so not one of them ever ran it.
        let map = managed(vec![(
            "/org/bluez/hci0/dev_00_00_00_00_00_09",
            vec![(
                bluez::DEVICE,
                vec![
                    ("Address", Value::from("00:00:00:00:00:09")),
                    ("Alias", Value::from("00-00-00-00-00-09")),
                    ("Connected", Value::from(false)),
                ],
            )],
        )]);
        let s = Scene::of(&map);
        let d = &s.devices[0];
        assert!(!d.named(), "the address in dashes is not a name");
        assert_eq!(d.label, "00:00:00:00:00:09", "and the row says the address");

        // A REAL ALIAS STILL IS ONE.
        let map = managed(vec![(
            "/org/bluez/hci0/dev_00_00_00_00_00_09",
            vec![(
                bluez::DEVICE,
                vec![
                    ("Address", Value::from("00:00:00:00:00:09")),
                    ("Alias", Value::from("a speaker")),
                    ("Connected", Value::from(false)),
                ],
            )],
        )]);
        let s = Scene::of(&map);
        assert!(s.devices[0].named());
        assert_eq!(s.devices[0].label, "a speaker");
    }

    #[test]
    fn an_empty_map_is_a_radio_that_is_not_there() {
        let s = Scene::of(&bluez::Managed::new());
        assert!(s.adapters.is_empty() && s.devices.is_empty());
        assert!(s.adapter().is_none());
        assert!(s.connected().is_empty());
    }

    #[test]
    fn the_percentage_and_the_danger_are_the_owners_two_numbers() {
        let mut d = Device::default();
        for (percent, shown, alarmed) in [
            (100, false, false),
            (31, false, false),
            (30, true, false),
            (16, true, false),
            (15, true, true),
            (1, true, true),
        ] {
            d.battery = Some(percent);
            assert_eq!(d.shows_percent(), shown, "{percent}% shown");
            assert_eq!(d.alarmed(), alarmed, "{percent}% alarmed");
        }
        d.battery = None;
        assert!(!d.shows_percent() && !d.alarmed(), "no battery, no number");
    }

    #[test]
    fn the_kind_comes_from_the_class_first_and_the_appearance_second() {
        // A headset's class says headset; its icon would say headphones.
        assert_eq!(
            Kind::of(Some(0x24_0404), None, "audio-headphones", &[]),
            Kind::Headset
        );
        // Headphones.
        assert_eq!(Kind::of(Some(0x24_0418), None, "", &[]), Kind::Headphones);
        // A classic keyboard, by class.
        assert_eq!(Kind::of(Some(0x00_0540), None, "", &[]), Kind::Keyboard);
        // A classic mouse.
        assert_eq!(Kind::of(Some(0x00_2580), None, "", &[]), Kind::Mouse);
        // A phone.
        assert_eq!(Kind::of(Some(0x5a_020c), None, "", &[]), Kind::Phone);
        // A wristwatch, which BlueZ's own icon cannot say at all.
        assert_eq!(Kind::of(Some(0x00_0704), None, "", &[]), Kind::Watch);
        // LE, by appearance.
        assert_eq!(Kind::of(None, Some(961), "", &[]), Kind::Keyboard);
        assert_eq!(Kind::of(None, Some(962), "", &[]), Kind::Mouse);
        assert_eq!(Kind::of(None, Some(964), "", &[]), Kind::Gamepad);
        assert_eq!(Kind::of(None, Some(193), "", &[]), Kind::Watch);
        // The icon is the third word, and the services the fourth.
        assert_eq!(Kind::of(None, None, "printer", &[]), Kind::Printer);
        assert_eq!(
            Kind::of(
                None,
                None,
                "",
                &["0000110b-0000-1000-8000-00805f9b34fb".to_owned()]
            ),
            Kind::Speaker
        );
        // And nothing said anything.
        assert_eq!(Kind::of(None, None, "", &[]), Kind::Unknown);

        // A CLASS THAT CANNOT BE NAMED STANDS ASIDE, and the next word is
        // asked. `of_class` answers `Some(Unknown)` for a peripheral shape it
        // does not recognise — a class it CAN read and cannot name — and
        // taking that as an answer meant the appearance, the icon and the
        // services were never consulted for exactly the devices that need
        // them. Major 5, minor 0: a peripheral of no known shape.
        assert_eq!(
            Kind::of(Some(0x00_0500), Some(961), "", &[]),
            Kind::Keyboard
        );
        assert_eq!(
            Kind::of(Some(0x00_0500), None, "printer", &[]),
            Kind::Printer
        );
        // Major 7, minor 0: a wearable that is not a watch.
        assert_eq!(
            Kind::of(
                Some(0x00_0700),
                None,
                "",
                &["0000110b-0000-1000-8000-00805f9b34fb".to_owned()]
            ),
            Kind::Speaker
        );
        // AND WHEN NOTHING ELSE SPEAKS EITHER, it is still unknown.
        assert_eq!(Kind::of(Some(0x00_0500), None, "", &[]), Kind::Unknown);
    }

    #[test]
    fn le_audio_earbuds_are_not_an_unknown_thing() {
        // A PURE LE-AUDIO DEVICE HAS NO CLASS AND NO BR/EDR UUID — no
        // `0000110b` A2DP sink, no `0000111e` handsfree — so the appearance
        // is the only thing that says what it is. Categories 33 and 37 were
        // missing, and every one of these fell to `Kind::Unknown` and drew
        // the generic bluetooth glyph: the owner's second requirement failing
        // on the class of device most likely to be bought next.
        for (appearance, want) in [
            (0x0941_u16, Kind::Headset),
            (0x0942, Kind::Headphones),
            (0x0943, Kind::Headphones),
            (0x0944, Kind::Speaker),
            // A sub-value nobody has seen yet is still audio, and the
            // category alone says which half.
            (0x0940, Kind::Headphones),
            (0x0840, Kind::Speaker),
        ] {
            assert_eq!(
                Kind::of(None, Some(appearance), "", &[]),
                want,
                "appearance {appearance:#06x}"
            );
        }
        // AND THE CLASSIC ONES DID NOT MOVE.
        assert_eq!(Kind::of(None, Some(961), "", &[]), Kind::Keyboard);
        assert_eq!(Kind::of(None, Some(962), "", &[]), Kind::Mouse);
    }

    #[test]
    fn every_kind_has_an_id_and_they_are_all_different() {
        let mut seen = std::collections::BTreeSet::new();
        for k in Kind::ALL {
            assert!(seen.insert(k.id()), "{k:?} shares an id");
            assert!(!k.id().is_empty());
            assert!(k.id().chars().all(|c| c.is_ascii_lowercase() || c == '-'));
        }
        assert_eq!(seen.len(), Kind::ALL.len());
    }

    #[test]
    fn the_lists_are_sorted_the_way_a_hand_can_use() {
        let mut low = Device {
            at: "/d/1".to_owned(),
            label: "z".to_owned(),
            kind: Kind::Mouse,
            connected: true,
            paired: true,
            battery: Some(9),
            ..Device::default()
        };
        let full = Device {
            at: "/d/2".to_owned(),
            label: "a".to_owned(),
            kind: Kind::Headphones,
            connected: true,
            paired: true,
            battery: Some(90),
            ..Device::default()
        };
        let idle = Device {
            at: "/d/3".to_owned(),
            label: "b".to_owned(),
            paired: true,
            ..Device::default()
        };
        let stranger = Device {
            at: "/d/4".to_owned(),
            label: "00:00:00:00:00:09".to_owned(),
            ..Device::default()
        };
        let named = Device {
            at: "/d/5".to_owned(),
            label: "a name".to_owned(),
            name: "a name".to_owned(),
            ..Device::default()
        };
        let scene = Scene {
            adapters: Vec::new(),
            devices: vec![low.clone(), full.clone(), idle, stranger, named],
            daemon: true,
        };
        // The one about to die comes first, whatever its kind.
        assert_eq!(scene.connected()[0].at, "/d/1");
        low.battery = Some(90);
        let scene2 = Scene {
            devices: vec![low, full],
            ..scene.clone()
        };
        // With nobody alarmed, the kind decides: headphones before a mouse.
        assert_eq!(scene2.connected()[0].at, "/d/2");
        // Connected before idle, among the paired.
        let paired = scene.paired();
        assert_eq!(paired.len(), 3);
        assert!(paired[0].connected && paired[1].connected);
        // A stranger with a name comes before one with only an address.
        let nearby = scene.nearby();
        assert_eq!(nearby.len(), 2);
        assert_eq!(nearby[0].at, "/d/5");
    }

    #[test]
    fn a_service_id_is_a_key_or_it_is_the_id_itself() {
        assert_eq!(
            profile_key("0000110b-0000-1000-8000-00805f9b34fb"),
            Some("bluetooth-profile-audio-sink")
        );
        assert_eq!(
            profile_key("0000180f-0000-1000-8000-00805f9b34fb"),
            Some("bluetooth-profile-battery")
        );
        assert_eq!(profile_key("0000feed-0000-1000-8000-00805f9b34fb"), None);
        assert_eq!(profile_key("nonsense"), None);
    }

    #[test]
    fn a_refusal_is_read_as_a_name() {
        let wrapped = |name: &str| {
            anyhow::Error::from(zbus::Error::MethodError(
                zbus::names::OwnedErrorName::try_from(name).unwrap(),
                None,
                zbus::message::Message::method_call("/", "x")
                    .unwrap()
                    .build(&())
                    .unwrap(),
            ))
            .context("connecting")
        };
        assert_eq!(
            refusal(&wrapped("org.bluez.Error.InProgress")),
            Refusal::Busy
        );
        assert_eq!(
            refusal(&wrapped("org.bluez.Error.AuthenticationRejected")),
            Refusal::Rejected
        );
        assert_eq!(
            refusal(&wrapped("org.bluez.Error.ConnectionAttemptFailed")),
            Refusal::Away
        );
        assert_eq!(
            refusal(&wrapped("org.bluez.Error.DoesNotExist")),
            Refusal::NotAvailable
        );
        assert_eq!(
            refusal(&wrapped("org.freedesktop.DBus.Error.AccessDenied")),
            Refusal::NotAuthorized
        );
        assert_eq!(
            refusal(&anyhow::anyhow!("nothing of the bus")),
            Refusal::Other
        );
    }
}
