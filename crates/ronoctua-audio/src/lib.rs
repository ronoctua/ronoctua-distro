//! The one boundary with PipeWire.
//!
//! # Why one crate
//!
//! The same argument `ronoctua-disks` makes about udisks2: when two parts of
//! the system ask "what is the volume?" of different places, the day comes
//! when they disagree and nobody knows which to believe. This repository had
//! exactly that — `ronoctua-bluetooth` ran `pw-dump` of its own to read a
//! headset's card — and this crate is where that parsing now lives, with the
//! Bluetooth crate asking it by address.
//!
//! # What is here and what is not
//!
//! [`pipewire`] is the only module that knows `pw-dump`, `wpctl` and `pw-cat`
//! exist. [`graph`] is a pure merge over `serde_json::Value`. [`meter`] owns
//! the one child that opens a microphone. Everything in THIS file is a pure
//! function over plain structs, so the rules are tested with no daemon
//! running — which is what a build sandbox is.
//!
//! # The number PipeWire keeps is not the number a person reads
//!
//! `channelVolumes` is linear amplitude and `wpctl` speaks the CUBE ROOT of
//! it, in both directions. Measured on the owner's laptop: stored `0.056895`
//! reads back as `0.38`, and `wpctl set-volume 0.384614` stores `0.056895`
//! again. `pactl` shows a third convention (`6%`) and it is not ours.
//!
//! The conversion happens ONCE, at this boundary ([`perceived`] and
//! [`stored`]), and everything above it is a percentage a person would
//! recognise.
//!
//! # Why the whole reading, and why no clock
//!
//! `pw-dump --monitor` is a stream: the full graph once, then arrays holding
//! only what changed. Measured here — the first delta after a `wpctl` lands in
//! **6 ms**, and the child costs no measurable CPU while nothing moves. So
//! there is no timer, no `pw-dump` per frame, and what reaches the drawer is
//! always a whole [`Snapshot`]: the last one wins.

#![forbid(unsafe_code)]

pub mod graph;
pub mod meter;
pub mod pipewire;
pub mod prefs;

use anyhow::Result;
use std::sync::mpsc;

/// What a node is for.
///
/// The four `media.class` values this crate draws. Anything else PipeWire
/// reports — MIDI bridges, the dummy driver, video — is not audio and never
/// becomes a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// Something the machine plays THROUGH: `Audio/Sink`.
    Sink,
    /// Something the machine listens WITH: `Audio/Source`.
    Source,
    /// An application playing: `Stream/Output/Audio`.
    Playing,
    /// An application recording: `Stream/Input/Audio`.
    Recording,
}

impl Kind {
    /// The `media.class` this is, or `None` when the class is not audio.
    #[must_use]
    pub fn of(class: &str) -> Option<Self> {
        match class {
            "Audio/Sink" => Some(Self::Sink),
            "Audio/Source" => Some(Self::Source),
            "Stream/Output/Audio" => Some(Self::Playing),
            "Stream/Input/Audio" => Some(Self::Recording),
            _ => None,
        }
    }

    /// Whether this is a device the machine owns, rather than an application's
    /// stream through one.
    #[must_use]
    pub const fn is_a_device(self) -> bool {
        matches!(self, Self::Sink | Self::Source)
    }

    /// Whether sound travels INTO the machine through this.
    ///
    /// A source and a recording stream are the two shapes of "the microphone
    /// is in use", and the modeline's mark asks exactly this.
    #[must_use]
    pub const fn listens(self) -> bool {
        matches!(self, Self::Source | Self::Recording)
    }
}

/// Whether a thing can be chosen NOW.
///
/// PipeWire answers `"yes"`, `"no"` or `"unknown"`, and the three are not two:
/// a headphone port is `no` with nothing in the socket and `unknown` on
/// hardware that cannot tell. Collapsing `unknown` into either one would
/// either hide a working port or offer a dead one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Available {
    /// It can be chosen.
    Yes,
    /// It cannot — nothing is plugged in, or the other side does not offer it.
    No,
    /// The hardware does not say.
    #[default]
    Unknown,
}

impl Available {
    /// What PipeWire's word means.
    #[must_use]
    pub fn of(word: &str) -> Self {
        match word {
            "yes" => Self::Yes,
            "no" => Self::No,
            _ => Self::Unknown,
        }
    }

    /// Whether a row offering this may be pressed.
    ///
    /// `Unknown` counts as usable: refusing everything a laptop cannot report
    /// on would leave most machines with no ports at all.
    #[must_use]
    pub const fn usable(self) -> bool {
        !matches!(self, Self::No)
    }
}

/// Which way sound travels through a port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Direction {
    /// Out of the machine — a speaker, a headphone socket.
    #[default]
    Output,
    /// Into it — a microphone, a line in.
    Input,
}

impl Direction {
    /// What PipeWire's word means.
    #[must_use]
    pub fn of(word: &str) -> Self {
        if word == "Input" {
            Self::Input
        } else {
            Self::Output
        }
    }
}

/// One thing a card can be put in as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Profile {
    /// PipeWire's own index, which is what `wpctl set-profile` takes.
    pub index: i32,
    /// `off`, `HiFi (Mic1, Mic2, Speaker)`, `a2dp-sink` — the id, not a sentence.
    pub name: String,
    /// What PipeWire calls it, in its own words.
    pub description: String,
    /// Whether the card can be put in it now.
    pub available: Available,
}

/// One socket on a card.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Route {
    /// PipeWire's own index, which is what `wpctl set-route` takes.
    pub index: i32,
    /// `[Out] Speaker`, `[In] Mic1` — the name `assets/audio-names.nix` looks up.
    pub name: String,
    /// What PipeWire calls it, which is the fallback when we do not know the name.
    pub description: String,
    /// Which way sound goes through it.
    pub direction: Direction,
    /// Whether anything is plugged into it.
    pub available: Available,
    /// Which of the card's own device indices it belongs to.
    pub devices: Vec<i32>,
}

impl Route {
    /// The name with the direction and the punctuation taken off.
    ///
    /// ALSA has two schemes and PipeWire hands both on: the UCM one writes
    /// `[Out] Speaker`, the older one writes `analog-output-speaker`. Both mean
    /// the same socket, and [`PORTS`] is keyed on what this answers —
    /// `speaker`, in both cases.
    ///
    /// The DIRECTION comes off deliberately: it is a field of its own, and the
    /// list it is drawn in is already headed by it. `[Out]` in a row under
    /// "Output" is the same word twice.
    #[must_use]
    pub fn plain(&self) -> String {
        let mut out = String::with_capacity(self.name.len());
        let mut word = false;
        // The direction marker is `[Out] ` or `[In] ` and it is always first;
        // the older scheme spells it `analog-output-` in the middle, which is
        // not a prefix and is left alone — `analog-output-speaker` still ends
        // in the word that matters, and the table holds `speaker`.
        let bare = self
            .name
            .split_once(']')
            .map_or(self.name.as_str(), |(_, r)| r);
        for c in bare.chars() {
            if c.is_ascii_alphanumeric() {
                out.push(c.to_ascii_lowercase());
                word = true;
            } else if word {
                out.push('-');
                word = false;
            }
        }
        while out.ends_with('-') {
            out.pop();
        }
        // `analog-output-speaker` and `analog-input-mic` carry the direction
        // in the middle. Taking the LAST word alone would turn
        // `headset-mic` into `mic`, which is a different socket, so only the
        // known prefixes go.
        for lead in ["analog-output-", "analog-input-", "iec958-", "hdmi-output-"] {
            if let Some(rest) = out.strip_prefix(lead) {
                return rest.to_owned();
            }
        }
        out
    }

    /// Our own word for this port, when we have one.
    ///
    /// `None` means the screen draws [`Route::description`] — PipeWire's own
    /// words. That is the honest answer for a socket nobody named, and it is
    /// the same rule the notification list keeps for a foreign application's
    /// label.
    #[must_use]
    pub fn key(&self) -> Option<&'static str> {
        let plain = self.plain();
        PORTS
            .iter()
            .find(|(name, _)| *name == plain)
            .map(|(_, key)| *key)
    }
}

/// A card: the hardware a node hangs off.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Device {
    /// PipeWire's device id — the first argument of `wpctl set-profile`.
    pub id: u32,
    /// `alsa_card.pci-0000_c4_00.6`, `bluez_card.AA_BB_…`.
    pub name: String,
    /// What PipeWire calls it.
    pub description: String,
    /// Every profile, in PipeWire's own order.
    pub profiles: Vec<Profile>,
    /// The index of the one it is in.
    pub active_profile: i32,
    /// Every port, in PipeWire's own order.
    pub routes: Vec<Route>,
    /// The indices of the ports that are in use, one per direction at most.
    pub active_routes: Vec<i32>,
}

impl Device {
    /// The active profile, when it has one.
    #[must_use]
    pub fn profile(&self) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.index == self.active_profile)
    }

    /// The port in use in one direction.
    #[must_use]
    pub fn route(&self, way: Direction) -> Option<&Route> {
        self.routes
            .iter()
            .find(|r| r.direction == way && self.active_routes.contains(&r.index))
    }

    /// The next usable thing after `now` in a list, wrapping.
    ///
    /// `None` when there is nothing else to move to, which is the honest
    /// answer for a card with one profile and `off`.
    fn turn<T>(all: &[T], now: i32, forward: bool, index: fn(&T) -> i32) -> Option<&T> {
        let usable: Vec<&T> = all.iter().collect();
        if usable.len() < 2 {
            return None;
        }
        let at = usable.iter().position(|t| index(t) == now)?;
        let step = if forward { 1 } else { usable.len() - 1 };
        Some(usable[(at + step) % usable.len()])
    }

    /// The next profile the card can actually be put in.
    #[must_use]
    pub fn next_profile(&self, forward: bool) -> Option<&Profile> {
        let usable: Vec<Profile> = self
            .profiles
            .iter()
            .filter(|p| p.available.usable())
            .cloned()
            .collect();
        let at = Self::turn(&usable, self.active_profile, forward, |p| p.index)?.index;
        self.profiles.iter().find(|p| p.index == at)
    }

    /// The next port in one direction, INCLUDING the ones nothing is plugged
    /// into.
    ///
    /// A dead port is drawn and refuses — the shape `LEADER s f` uses for XFS
    /// with no program. Skipping it in the ring would leave the screen saying
    /// a socket does not exist, which is a different and wrong thing.
    #[must_use]
    pub fn next_route(&self, way: Direction, forward: bool) -> Option<&Route> {
        let here: Vec<Route> = self
            .routes
            .iter()
            .filter(|r| r.direction == way)
            .cloned()
            .collect();
        let now = self.route(way).map_or(-1, |r| r.index);
        let at = Self::turn(&here, now, forward, |r| r.index)?.index;
        self.routes.iter().find(|r| r.index == at)
    }
}

/// One thing sound goes through: a device's endpoint, or an application's stream.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Node {
    /// PipeWire's node id — the first argument of `wpctl set-volume`.
    pub id: u32,
    /// What it is for.
    pub kind: Option<Kind>,
    /// `alsa_output.pci-0000_c4_00.6.HiFi__Speaker__sink` — what the default
    /// metadata names, and therefore the only reliable way to say "this one".
    pub name: String,
    /// What PipeWire calls it, in its own words.
    pub description: String,
    /// The card it hangs off, when it has one. A stream has none.
    pub device: Option<u32>,
    /// The volume a PERSON reads: `channelVolumes` cube-rooted, times 100.
    pub volume: u8,
    /// Whether it is silenced.
    pub muted: bool,
    /// The application behind a stream, in its own words. Never touched — the
    /// same rule the notification list keeps for a foreign app's label.
    pub app: Option<String>,
    /// The Bluetooth address, when the card underneath is one. It is what
    /// `ronoctua-bluetooth` matches on, and PipeWire has never heard of a
    /// BlueZ object path.
    pub address: Option<String>,
}

/// Which node the machine reaches for when nobody says otherwise.
///
/// The two halves are different facts and the screen shows both: `sink` is
/// what is playing NOW, `configured_sink` is what was CHOSEN. They differ
/// whenever the chosen one is unplugged — measured on the owner's laptop, the
/// configured sink was a Bluetooth headset that was switched off.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Defaults {
    /// `default.audio.sink`.
    pub sink: Option<String>,
    /// `default.audio.source`.
    pub source: Option<String>,
    /// `default.configured.audio.sink`.
    pub configured_sink: Option<String>,
    /// `default.configured.audio.source`.
    pub configured_source: Option<String>,
}

/// One whole reading of the machine's sound.
///
/// Unfiltered on purpose, like `ronoctua_disks::Rack`: [`Snapshot::of`] is
/// what decides which nodes are rows, and keeping the whole reading means that
/// decision can change without another trip to PipeWire.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    /// Every card.
    pub devices: Vec<Device>,
    /// Every node, audio or not.
    pub nodes: Vec<Node>,
    /// What the machine reaches for by default.
    pub defaults: Defaults,
}

impl Snapshot {
    /// The nodes of one kind, in PipeWire's own order.
    #[must_use]
    pub fn of(&self, kind: Kind) -> Vec<&Node> {
        self.nodes.iter().filter(|n| n.kind == Some(kind)).collect()
    }

    /// The node one of the defaults names, if it is there at all.
    #[must_use]
    pub fn default_of(&self, kind: Kind) -> Option<&Node> {
        let wanted = match kind {
            Kind::Sink => self.defaults.sink.as_deref(),
            Kind::Source => self.defaults.source.as_deref(),
            Kind::Playing | Kind::Recording => None,
        }?;
        self.nodes
            .iter()
            .find(|n| n.kind == Some(kind) && n.name == wanted)
    }

    /// What was CHOSEN for one kind, when it is not what is playing.
    ///
    /// `None` when they agree, so a row only appears when there is something
    /// to say — the same rule the modeline keeps about being mute at rest.
    #[must_use]
    pub fn adrift(&self, kind: Kind) -> Option<&str> {
        let (now, chosen) = match kind {
            Kind::Sink => (
                self.defaults.sink.as_deref(),
                self.defaults.configured_sink.as_deref(),
            ),
            Kind::Source => (
                self.defaults.source.as_deref(),
                self.defaults.configured_source.as_deref(),
            ),
            Kind::Playing | Kind::Recording => (None, None),
        };
        let chosen = chosen?;
        (now != Some(chosen)).then_some(chosen)
    }

    /// A card by id.
    #[must_use]
    pub fn device(&self, id: u32) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }

    /// Whether the default sink is silenced — the modeline's `mute` mark.
    ///
    /// It asks the DEFAULT and not "any sink": a muted HDMI output nobody is
    /// using is not the machine being silent.
    #[must_use]
    pub fn is_silent(&self) -> bool {
        self.default_of(Kind::Sink).is_some_and(|n| n.muted)
    }

    /// Whether anything is recording — the modeline's `mic` mark.
    ///
    /// Derived from the graph, never guessed: a `Stream/Input/Audio` node
    /// exists exactly while some program holds a capture stream open.
    #[must_use]
    pub fn is_listening(&self) -> bool {
        self.nodes.iter().any(|n| n.kind == Some(Kind::Recording))
    }

    /// Whether anything that is NOT ours is recording — the microphone's
    /// event in the minibuffer.
    ///
    /// The modeline's mark asks [`Snapshot::is_listening`], and it is right to:
    /// a microphone the sound view's meter holds open IS open. The event is a
    /// different question — "did somebody start listening?" — and the meter
    /// and the studio's capture are this system answering a person who asked
    /// for them. Both name themselves [`pipewire::OWN_STREAM`].
    #[must_use]
    pub fn someone_else_listens(&self) -> bool {
        self.nodes.iter().any(|n| {
            n.kind == Some(Kind::Recording) && n.app.as_deref() != Some(pipewire::OWN_STREAM)
        })
    }
}

/// The volume a person reads, out of the amplitude PipeWire keeps.
///
/// See the module header for the measurement. Clamped at both ends: PipeWire
/// allows an amplitude above 1.0, and a percentage above 255 would not fit
/// the byte — nor would it be a volume anybody asked for.
#[must_use]
pub fn perceived(linear: f64) -> u8 {
    if !linear.is_finite() || linear <= 0.0 {
        return 0;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the cube root of a positive number, times 100, clamped to a byte"
    )]
    let percent = (linear.cbrt() * 100.0).round().clamp(0.0, 255.0) as u8;
    percent
}

/// The fraction `wpctl` takes, out of the volume a person read.
///
/// `wpctl` speaks the same perceived scale, so this is a division and not a
/// cube — writing the amplitude here would set 38% to what 72% sounds like.
#[must_use]
pub fn stored(percent: u8) -> f64 {
    f64::from(percent) / 100.0
}

/// What the footer asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Put one node at one perceived percentage.
    Volume {
        /// Which node.
        node: u32,
        /// How loud, as a person would read it.
        percent: u8,
    },
    /// Silence one node, or let it speak.
    Mute {
        /// Which node.
        node: u32,
        /// Whether it should be silent.
        muted: bool,
    },
    /// Make one node the one the machine reaches for.
    Default {
        /// Which node.
        node: u32,
    },
    /// Put a card in one of its profiles.
    Profile {
        /// Which card.
        device: u32,
        /// PipeWire's own index for the profile.
        index: i32,
    },
    /// Send a card's sound through one of its ports.
    Route {
        /// Which card.
        device: u32,
        /// PipeWire's own index for the port.
        index: i32,
    },
    /// Open one source and report its level, or close whatever is open.
    ///
    /// `None` is the only way the microphone closes, and the view sends it on
    /// leaving the row, on closing and on losing the keyboard.
    Listen(Option<u32>),
}

/// Why an order did not happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// There is no such node or card any more — it was unplugged mid-gesture.
    Gone,
    /// This build has no `wpctl`, or PipeWire is not running.
    NotAvailable,
    /// `wpctl` did not like the argument.
    Invalid,
    /// Anything else, and the daemon's own words travel beside it.
    Other,
}

/// Which refusal an error is.
///
/// It walks the chain the way `ronoctua_power::refusal` does, because the
/// shape of the answer matters more than its wording: an unplugged headset
/// and a broken build are two different sentences on screen.
#[must_use]
pub fn refusal(e: &anyhow::Error) -> Refusal {
    for cause in e.chain() {
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return match io.kind() {
                std::io::ErrorKind::NotFound => Refusal::NotAvailable,
                std::io::ErrorKind::InvalidInput => Refusal::Invalid,
                _ => Refusal::Other,
            };
        }
        if let Some(said) = cause.downcast_ref::<pipewire::Refused>() {
            return said.why;
        }
    }
    Refusal::Other
}

/// What the watching threads send the footer.
#[derive(Debug)]
pub enum Event {
    /// A fresh reading of the whole graph. The last one wins.
    Snapshot(Box<Snapshot>),
    /// The level of the source the view asked to listen to, 0..=100.
    ///
    /// It arrives on its own and NOT inside a snapshot: a level moves tens of
    /// times a second and a graph does not, and folding one into the other
    /// would make every meter frame look like the devices had changed.
    Level(u8),
    /// An order finished, and whether it was refused.
    Done {
        /// What was asked.
        order: Order,
        /// The refusal and the program's own words, when it was refused.
        refused: Option<(Refusal, String)>,
    },
}

/// The footer's side of the orders thread.
#[derive(Debug, Clone)]
pub struct Handle {
    orders: mpsc::Sender<Order>,
}

impl Handle {
    /// Ask for something.
    ///
    /// A dead thread is a warning and not a panic: sound is not worth taking
    /// the desktop down for.
    pub fn send(&self, order: Order) {
        if self.orders.send(order).is_err() {
            tracing::warn!("the audio thread is gone; the order was dropped");
        }
    }
}

/// Raise the threads and start reporting.
///
/// Two of them, and the split is the same one `ronoctua-power` keeps: one
/// watches, one carries out. The watcher owns the `pw-dump --monitor` child
/// and is the only thing that ever builds a [`Snapshot`]; the orders thread
/// owns the meter, because the meter is a child that must be started and
/// stopped by somebody, and starting it from the drawing thread would mean a
/// microphone opened inside a paint.
///
/// # Errors
///
/// When a thread cannot be started at all. A PipeWire that is not running is
/// NOT an error: the watcher says so and keeps trying, because the daemon
/// coming back is the common case.
pub fn watch<F>(on_event: F) -> Result<Handle>
where
    F: Fn(Event) + Send + Sync + 'static,
{
    let on_event = std::sync::Arc::new(on_event);
    let (orders, inbox) = mpsc::channel::<Order>();

    let feed = std::sync::Arc::clone(&on_event);
    std::thread::Builder::new()
        .name("audio-graph".to_owned())
        .spawn(move || pipewire::follow(&|e| feed(e)))
        .map_err(|e| anyhow::anyhow!("could not start the audio thread: {e}"))?;

    let feed = std::sync::Arc::clone(&on_event);
    std::thread::Builder::new()
        .name("audio-orders".to_owned())
        .spawn(move || {
            // THE METER LIVES HERE, for as long as somebody is looking at a
            // row. Dropping it closes the child and the microphone with it,
            // which is why it is a value and not a flag.
            let mut listening: Option<meter::Meter> = None;
            for order in inbox {
                if let Order::Listen(target) = order {
                    listening = match target {
                        None => None,
                        Some(id) => {
                            // DROP FIRST, THEN OPEN. Assigning over it would
                            // build the second child while the first still
                            // held the device, and two `pw-cat`s on one
                            // source is a race nobody would see.
                            drop(listening.take());
                            let feed = std::sync::Arc::clone(&feed);
                            meter::Meter::listen_to(id, move |level| feed(Event::Level(level)))
                                .map_err(|e| tracing::warn!(error = %e, id, "no meter"))
                                .ok()
                        }
                    };
                    feed(Event::Done {
                        order,
                        refused: None,
                    });
                    continue;
                }
                let result = pipewire::carry_out(&order);
                let refused = result.err().map(|e| (refusal(&e), format!("{e:#}")));
                if let Some((why, words)) = &refused {
                    tracing::warn!(?order, ?why, %words, "the order was refused");
                }
                // NO RE-READING HERE, and that is the difference with
                // `ronoctua-power`. The monitor reports what `wpctl` did in
                // about six milliseconds on its own; asking again would be a
                // second `pw-dump` per keypress, and holding an arrow down
                // would be a fork per frame.
                feed(Event::Done { order, refused });
            }
        })
        .map_err(|e| anyhow::anyhow!("could not start the audio orders thread: {e}"))?;

    Ok(Handle { orders })
}

include!(concat!(env!("OUT_DIR"), "/names.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_name_is_normalised_before_it_is_looked_up() {
        let ucm = Route {
            name: "[Out] Speaker".to_owned(),
            ..Route::default()
        };
        let older = Route {
            name: "analog-output-speaker".to_owned(),
            ..Route::default()
        };
        let bare = Route {
            name: "Speaker".to_owned(),
            ..Route::default()
        };
        assert_eq!(ucm.plain(), "speaker");
        assert_eq!(older.plain(), "speaker");
        assert_eq!(bare.plain(), "speaker");
        assert_eq!(ucm.key(), Some("audio-port-speaker"));
        assert_eq!(older.key(), ucm.key());
    }

    #[test]
    fn a_direction_in_the_middle_does_not_eat_the_socket() {
        // `headset-mic` and `mic` are different sockets. Taking the last word
        // would make one of them the other, and the row would say "microphone"
        // about the one in the headphone jack.
        let headset = Route {
            name: "analog-input-headset-mic".to_owned(),
            ..Route::default()
        };
        assert_eq!(headset.plain(), "headset-mic");
        assert_eq!(headset.key(), Some("audio-port-headset-mic"));
    }

    #[test]
    fn a_port_nobody_named_draws_in_pipewires_words() {
        // Measured on the owner's laptop: the HDMI card reports four of these,
        // and a table can never hold the numbered ones.
        let hdmi = Route {
            name: "[Out] HDMI / DisplayPort 3".to_owned(),
            description: "HDMI / DisplayPort 3 Output".to_owned(),
            ..Route::default()
        };
        assert_eq!(hdmi.plain(), "hdmi-displayport-3");
        assert_eq!(
            hdmi.key(),
            None,
            "and the row falls back to the description"
        );
    }

    #[test]
    fn the_two_microphones_of_this_laptop_keep_their_own_words() {
        // `Mic1` and `Mic2` are deliberately NOT in the table: their
        // descriptions are what tell them apart, and one shared word would
        // make two rows say the same thing.
        for name in ["[In] Mic1", "[In] Mic2"] {
            let r = Route {
                name: name.to_owned(),
                ..Route::default()
            };
            assert_eq!(r.key(), None, "{name} must keep PipeWire's description");
        }
    }

    #[test]
    fn the_perceived_volume_is_the_cube_root() {
        // The owner's laptop, measured: `pw-dump` said 0.056895 and `wpctl`
        // said 0.38.
        assert_eq!(perceived(0.056895), 38);
        // And the headset in the Bluetooth crate's comment.
        assert_eq!(perceived(0.367886), 72);
        assert_eq!(perceived(1.0), 100);
        assert_eq!(perceived(0.0), 0);
    }

    #[test]
    fn a_broken_amplitude_is_silence_and_not_a_panic() {
        assert_eq!(perceived(f64::NAN), 0);
        assert_eq!(perceived(-1.0), 0);
        // PipeWire allows an amplitude above one.
        assert_eq!(perceived(8.0), 200);
    }

    #[test]
    fn the_round_trip_holds() {
        // What the view reads, handed back to `wpctl`, has to be the same
        // volume. A product that dropped the cube root on one side would
        // still pass a one-sided test; this one asks for both.
        for percent in [0_u8, 1, 25, 38, 50, 72, 99, 100] {
            let linear = stored(percent).powi(3);
            assert_eq!(perceived(linear), percent, "at {percent}%");
        }
    }

    #[test]
    fn wpctl_is_not_given_an_amplitude() {
        // The failure this pins: `stored` cubing its argument would set 38%
        // to what 72% sounds like, and every volume in the system would be
        // wrong in a way that reads as "the slider is miscalibrated".
        assert!((stored(38) - 0.38).abs() < 1e-9);
        assert!((stored(100) - 1.0).abs() < 1e-9);
    }

    fn a_card() -> Device {
        Device {
            id: 52,
            name: "alsa_card.pci".to_owned(),
            description: "Ryzen".to_owned(),
            profiles: vec![
                Profile {
                    index: 0,
                    name: "off".to_owned(),
                    description: "Off".to_owned(),
                    available: Available::Yes,
                },
                Profile {
                    index: 1,
                    name: "headphones".to_owned(),
                    description: "Headphones".to_owned(),
                    available: Available::No,
                },
                Profile {
                    index: 2,
                    name: "speaker".to_owned(),
                    description: "Speaker".to_owned(),
                    available: Available::Yes,
                },
            ],
            active_profile: 2,
            routes: vec![
                Route {
                    index: 0,
                    name: "[Out] Speaker".to_owned(),
                    description: "Speaker".to_owned(),
                    direction: Direction::Output,
                    available: Available::Unknown,
                    devices: vec![0],
                },
                Route {
                    index: 3,
                    name: "[Out] Headphones".to_owned(),
                    description: "Headphones".to_owned(),
                    direction: Direction::Output,
                    available: Available::No,
                    devices: vec![3],
                },
                Route {
                    index: 2,
                    name: "[In] Mic1".to_owned(),
                    description: "Digital Microphone".to_owned(),
                    direction: Direction::Input,
                    available: Available::Unknown,
                    devices: vec![2],
                },
            ],
            active_routes: vec![0, 2],
        }
    }

    #[test]
    fn the_profile_ring_skips_what_cannot_be_chosen() {
        let card = a_card();
        // From `speaker` (2), forward, the only other usable one is `off` (0):
        // `headphones` reports `no` because nothing is in the socket.
        assert_eq!(card.next_profile(true).map(|p| p.index), Some(0));
        assert_eq!(card.next_profile(false).map(|p| p.index), Some(0));
    }

    #[test]
    fn the_port_ring_keeps_the_dead_socket() {
        let card = a_card();
        // The headphone port stays in the ring even though nothing is plugged
        // in — it is drawn and it refuses. Leaving it out would say the
        // socket does not exist.
        assert_eq!(
            card.next_route(Direction::Output, true).map(|r| r.index),
            Some(3)
        );
        // And one port in a direction is not a ring at all.
        assert_eq!(card.next_route(Direction::Input, true), None);
    }

    #[test]
    fn a_card_knows_which_port_is_in_use() {
        let card = a_card();
        assert_eq!(card.route(Direction::Output).map(|r| r.index), Some(0));
        assert_eq!(card.route(Direction::Input).map(|r| r.index), Some(2));
        assert_eq!(card.profile().map(|p| p.name.as_str()), Some("speaker"));
    }

    fn a_graph() -> Snapshot {
        Snapshot {
            devices: vec![a_card()],
            nodes: vec![
                Node {
                    id: 60,
                    kind: Some(Kind::Sink),
                    name: "speaker".to_owned(),
                    description: "Speaker".to_owned(),
                    device: Some(52),
                    volume: 38,
                    muted: false,
                    app: None,
                    address: None,
                },
                Node {
                    id: 61,
                    kind: Some(Kind::Source),
                    name: "mic".to_owned(),
                    description: "Microphone".to_owned(),
                    device: Some(52),
                    volume: 100,
                    muted: true,
                    app: None,
                    address: None,
                },
                Node {
                    id: 99,
                    kind: None,
                    name: "Dummy-Driver".to_owned(),
                    ..Node::default()
                },
            ],
            defaults: Defaults {
                sink: Some("speaker".to_owned()),
                source: Some("mic".to_owned()),
                configured_sink: Some("a headset that is switched off".to_owned()),
                configured_source: Some("mic".to_owned()),
            },
        }
    }

    #[test]
    fn what_is_not_audio_is_never_a_row() {
        let g = a_graph();
        assert_eq!(g.of(Kind::Sink).len(), 1);
        assert_eq!(g.of(Kind::Playing).len(), 0);
        // The dummy driver is in the reading and in no list.
        assert_eq!(g.nodes.len(), 3);
    }

    #[test]
    fn the_chosen_device_and_the_playing_one_are_two_facts() {
        let g = a_graph();
        assert_eq!(g.adrift(Kind::Sink), Some("a headset that is switched off"));
        // They agree for the microphone, so there is nothing to say.
        assert_eq!(g.adrift(Kind::Source), None);
    }

    #[test]
    fn the_marks_ask_the_default_and_the_streams() {
        let mut g = a_graph();
        // The microphone is muted and the speaker is not: the machine is not
        // silent, and a muted source is not the `mute` mark.
        assert!(!g.is_silent());
        assert!(!g.is_listening());

        g.nodes[0].muted = true;
        assert!(g.is_silent());

        g.nodes.push(Node {
            id: 120,
            kind: Some(Kind::Recording),
            app: Some("some recorder".to_owned()),
            ..Node::default()
        });
        assert!(g.is_listening());
    }

    /// FAILS IF: the sound view's meter or the studio's capture — both ours,
    /// both named `OWN_STREAM` — count as somebody starting to listen, or a
    /// stranger's stream stops counting while one of ours is open beside it.
    #[test]
    fn our_own_recordings_are_not_somebody_listening() {
        let mut g = a_graph();
        g.nodes.push(Node {
            id: 121,
            kind: Some(Kind::Recording),
            app: Some(pipewire::OWN_STREAM.to_owned()),
            ..Node::default()
        });
        assert!(g.is_listening(), "the mark still says a microphone is open");
        assert!(!g.someone_else_listens(), "the meter is not somebody else");
        // A STREAM WITH NO NAME AT ALL is a stranger: nothing says it is ours.
        g.nodes.push(Node {
            id: 122,
            kind: Some(Kind::Recording),
            app: None,
            ..Node::default()
        });
        assert!(g.someone_else_listens());
    }

    #[test]
    fn a_default_naming_something_that_is_gone_is_not_a_node() {
        let mut g = a_graph();
        g.defaults.sink = Some("a sink that was unplugged".to_owned());
        assert!(g.default_of(Kind::Sink).is_none());
        // And the mark does not light for a device that is not there.
        assert!(!g.is_silent());
    }
}
