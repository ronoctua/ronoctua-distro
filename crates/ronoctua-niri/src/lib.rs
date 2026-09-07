//! The only boundary with niri's IPC — invariant 3, section 7.3.
//!
//! No other crate talks to the compositor. If some day a new piece of data from
//! niri is needed, it comes in HERE and goes out as a type of ours; a
//! `serde_json::Value` is not spread through the shell.
//!
//! # Why our own types, and not the `niri-ipc` crate
//!
//! Milestone 4's acceptance criterion is "the contract passes against the pinned tag
//! **and** against `niri-next`". Depending on the published `niri-ipc` crate would make
//! that impossible to verify: Cargo would pin ONE version of the types, and the
//! test against `niri-next` would be checking the old version of the types against
//! the new binary — that is, nothing. With our own types, the same code is
//! exercised against both actual binaries, which is the only way for the drift warning
//! to arrive BEFORE the release becomes mandatory (risk R2).
//!
//! # Tolerance of unknown variants
//!
//! Events are read as `serde_json::Value` and only then mapped. That is
//! deliberate: niri versions by calendar and ADDS `Event` variants
//! frequently. A Rust enum with `deny_unknown` would break on every
//! release over an event that does not interest us. Here, an unknown event
//! becomes [`Event::Ignored`] and life goes on — while the contract test
//! goes on proving that the ones we USE exist.
//!
//! It covers EVENTS and added FIELDS, and deliberately not the two fieldless
//! enums that are read back ([`Transform`] and [`Change`]). Both are frozen
//! by something outside niri — there are eight Wayland transforms and two
//! answers to "did the change land" — so an unknown variant there is a
//! protocol break worth failing on, not a version to shrug at.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

/// The variable niri exports to its children with the socket's path.
pub const SOCKET_ENV: &str = "NIRI_SOCKET";

/// A niri window, reduced to what the modeline uses (section 7.1 of the plan).
///
/// Fields niri has and we do not: position, size, column, workspace,
/// urgency, focus timestamp. They come in when a segment needs them, and
/// not before — every field here is contract surface to maintain.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Window {
    /// A stable id for as long as the window exists.
    pub id: u64,
    /// The title, when the window sets one.
    pub title: Option<String>,
    /// Application ID — it becomes the modeline's `major-mode` (section 4.3).
    pub app_id: Option<String>,
    /// The PID of whoever opened the Wayland connection.
    ///
    /// It is the key to section 7.5's PID -> cgroup correlation. It may be `None`;
    /// in that case the modeline degrades, it does not invent.
    pub pid: Option<i32>,
    /// Whether it is the focused window. There are zero or one.
    pub is_focused: bool,
}

// ---------------------------------------------------------------------
// OUTPUTS — the monitors the compositor draws onto.
// ---------------------------------------------------------------------
//
// niri answers `Outputs` with a map of these, and takes changes through
// `Output { output, action }`. Both are ours to speak, and nobody else's:
// building the action's JSON somewhere else would be a second crate talking
// the compositor's protocol, which invariant 3 does not allow.
//
// THE ASYMMETRY OF NIRI'S OWN, measured in its source rather than guessed: a
// refresh rate is READ in millihertz as an integer and ASKED FOR in hertz as
// a float. It is written on the two fields it belongs to, because a caller
// who does not know it sends a number niri cannot match to any mode — and
// [`Mode::asked`] exists so that nobody has to do the conversion by hand.
//
// WHAT IS NOT HERE, AND WHY: the bit depth. `niri-next` has `max_bpc` on the
// output and a `MaxBpc` action; the PINNED niri has neither, and answers
// `error parsing request` — which is how the contract test found this, on its
// first run, against a real compositor. A knob the compositor we ship cannot
// turn is a dead row on a screen, so it waits for the pin to move.
// `docs/PENDING.md` carries the trigger.

/// A monitor, as the compositor sees it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Output {
    /// The connector — `eDP-1`, `DP-3`.
    ///
    /// It is what the kernel calls the port without the `cardN-` prefix
    /// `/sys/class/drm` puts on it, which is how a reading of that directory
    /// joins up with this.
    pub name: String,
    /// The manufacturer, or the word `Unknown` — niri writes the word rather
    /// than leaving the field out, so an empty-looking make is still a string.
    pub make: String,
    /// The model, under the same rule as [`Self::make`].
    pub model: String,
    /// The serial the EDID carries, when it carries one.
    pub serial: Option<String>,
    /// The glass, in millimetres, when the EDID says. It is the only way to
    /// arrive at a real size or a real DPI; the pixels alone cannot.
    pub physical_size: Option<(u32, u32)>,
    /// Every mode the monitor offers, in the order it offers them.
    pub modes: Vec<Mode>,
    /// Which of [`Self::modes`] is running.
    ///
    /// `None` while the output is off — and also, on an output that is very
    /// much on, when the running mode is interlaced: niri drops those from
    /// `modes` and then cannot index the one it is using
    /// (`backend/tty.rs`, "connector mode list missing current mode"). So
    /// "no current mode" does not mean "dark".
    pub current_mode: Option<usize>,
    /// Whether the current mode came from a custom mode or a modeline.
    pub is_custom_mode: bool,
    /// Whether the hardware can vary its refresh rate.
    pub vrr_supported: bool,
    /// Whether it is varying it right now.
    pub vrr_enabled: bool,
    /// Where the output sits in the compositor's space, and at what scale.
    /// `None` while the output is off — an off output has no place.
    pub logical: Option<Logical>,
}

impl Output {
    /// The name a configuration block has to carry to mean THIS monitor.
    ///
    /// niri matches an `output` block by the connector OR by
    /// `"<make> <model> <serial>"`, substituting the word `Unknown` for a
    /// part it does not know (`niri-config`'s `OutputName::matches`). The
    /// description is the one worth writing down, because it survives the
    /// cable moving to another port — the connector does not.
    ///
    /// It is ALWAYS the description, even when every part of it is the word
    /// `Unknown` — this is what a monitor says about itself, and a screen
    /// showing it is showing the truth. It used to fall back to the connector
    /// in that case, which made it a filing key wearing a description's name;
    /// what decides a filing key is [`Output::naming`], and niri's own
    /// refusal to match three `Unknown`s is handled there.
    ///
    /// # IT NAMES A MODEL, WHICH IS NOT ALWAYS ONE MONITOR
    ///
    /// Two identical monitors with no serial in their EDID answer the same
    /// string, and that is the commonest two-monitor desk there is. niri
    /// resolves a name to the FIRST output that matches, so both would share
    /// one configuration block: they would be given the same position, the
    /// second would overlap the first, and niri would fall back to placing it
    /// automatically with a line in its log and nothing on screen.
    ///
    /// So this is the description, for SHOWING. What to file settings under
    /// is [`Output::naming`], which refuses a description that a second
    /// monitor could answer to.
    #[must_use]
    pub fn identity(&self) -> String {
        let serial = self.serial.as_deref().unwrap_or(UNKNOWN);
        format!("{} {} {}", self.make, self.model, serial)
    }

    /// The name to FILE this monitor's settings under, and why.
    ///
    /// The description when it can only be this monitor, and the CONNECTOR
    /// otherwise. What makes a description trustworthy is the SERIAL: a
    /// make and a model are shared by every unit of a product, and one that
    /// gives no serial is a monitor that a second identical one could answer
    /// for.
    ///
    /// # Why the serial and not "is there a twin plugged in right now"
    ///
    /// Because the answer is written to a file that outlives the session.
    /// Asking about the monitors attached at this moment gives a name that
    /// CHANGES when a second one is plugged in — and the settings filed under
    /// the old name silently stop being found, then come back when the twin
    /// is unplugged. A rule that reads one monitor's own EDID gives the same
    /// answer every time, which is what a filing key has to do.
    ///
    /// The cost is real and small: a monitor that gives no serial has its
    /// settings follow the PORT rather than the glass. The screen says so.
    ///
    /// WHAT IT STILL CANNOT CATCH, weighed and accepted: two units that
    /// report the SAME serial. Some vendors ship blanks and some ship
    /// duplicates. Looking at the monitors attached right now would catch it
    /// — and that is exactly the rule this replaced, because its answer moved
    /// under the file it was the key to. A duplicated serial is a monitor
    /// lying about itself; a key that changes when a cable is plugged in is
    /// us lying about where the settings went.
    #[must_use]
    pub fn naming(&self) -> Naming {
        let serial = self.serial.as_deref().unwrap_or(UNKNOWN);
        let silent = self.make == UNKNOWN && self.model == UNKNOWN && serial == UNKNOWN;
        if silent {
            return Naming::Connector {
                name: self.name.clone(),
                why: Anonymous::Silent,
            };
        }
        if serial == UNKNOWN {
            return Naming::Connector {
                name: self.name.clone(),
                why: Anonymous::NoSerial,
            };
        }
        Naming::Description(self.identity())
    }
}

/// The name a monitor's settings are filed under, and why it is that one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Naming {
    /// `"<make> <model> <serial>"`. The settings follow the monitor, through
    /// a change of port and a week in a drawer.
    Description(String),
    /// The connector. The settings follow the CABLE.
    Connector {
        /// The connector's name.
        name: String,
        /// What the monitor failed to say about itself.
        why: Anonymous,
    },
}

impl Naming {
    /// The name itself.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Description(d) => d,
            Self::Connector { name, .. } => name,
        }
    }

    /// Whether the settings follow the monitor rather than the cable.
    #[must_use]
    pub const fn follows_the_monitor(&self) -> bool {
        matches!(self, Self::Description(_))
    }
}

/// What a monitor did not say about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anonymous {
    /// No make, no model and no serial. niri will not match a description of
    /// three `Unknown`s — it would match every anonymous monitor at once.
    Silent,
    /// A make and a model, but no serial: every unit of that product answers
    /// the same string.
    NoSerial,
}

/// The word niri writes where a monitor told it nothing.
const UNKNOWN: &str = "Unknown";

/// A video mode a monitor offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Mode {
    /// Width in physical pixels.
    pub width: u16,
    /// Height in physical pixels.
    pub height: u16,
    /// The rate in **millihertz**.
    ///
    /// THE ASYMMETRY: read in millihertz as an integer, asked for in hertz as
    /// a float ([`ModeAsked::refresh`]). And the number is not round — a
    /// panel sold as 60 Hz reports `59951` — so a rounded hertz both merges
    /// two modes that are not the same one AND fails to name either of them.
    /// [`Mode::asked`] is the conversion that survives the round trip.
    pub refresh_rate: u32,
    /// Whether the monitor says this is the one it wants.
    pub is_preferred: bool,
}

impl Mode {
    /// The rate in hertz, ROUNDED, and only for showing a person.
    ///
    /// One decimal, which is what tells a 143,9 Hz mode from a 144,0 one on a
    /// screen without spilling millihertz into it.
    ///
    /// # Never ask for a mode with this
    ///
    /// niri matches a requested rate by turning it back into an integer of
    /// millihertz — `(r * 1000.).round()` — and comparing for EQUALITY
    /// (`backend/tty.rs`). A panel's 59951 mHz rounds to `60.0` here, which
    /// asks for 60000, which matches nothing; niri then falls back to the
    /// preferred mode, logs a warning nobody reads and answers
    /// [`Change::Applied`]. [`Mode::asked`] is the one that comes back.
    #[must_use]
    pub fn hertz(self) -> f64 {
        (f64::from(self.refresh_rate) / 100.0).round() / 10.0
    }

    /// The rate in hertz, EXACTLY.
    ///
    /// Millihertz over a thousand, so it never has more than three decimals
    /// and never loses one.
    #[must_use]
    pub fn rate(self) -> f64 {
        f64::from(self.refresh_rate) / 1000.0
    }

    /// This mode, as something to ask for.
    ///
    /// The only conversion that survives niri's matching. Use it instead of
    /// building a [`ModeAsked`] by hand.
    #[must_use]
    pub fn asked(self) -> ModeAsked {
        ModeAsked {
            width: self.width,
            height: self.height,
            refresh: Some(self.rate()),
        }
    }
}

impl From<Mode> for ModeAsked {
    fn from(m: Mode) -> Self {
        m.asked()
    }
}

/// Where an output sits in the compositor's coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Logical {
    /// Logical X of the top-left corner.
    pub x: i32,
    /// Logical Y of the top-left corner.
    pub y: i32,
    /// Width in logical pixels, WITH THE TURN ALREADY APPLIED.
    ///
    /// niri transforms the size before it reports it
    /// (`utils::output_size`: `output_transform.transform_size(...)`), so on
    /// a quarter turn this is the physical HEIGHT over the scale. Anything
    /// drawing the arrangement uses these two as they are and must NOT swap
    /// them again — that is what [`Transform::turns_the_side`] is for, and it
    /// is for PREDICTING a turn that has not happened yet, never for
    /// interpreting one that has.
    pub width: u32,
    /// Height in logical pixels, under the same rule as [`Self::width`].
    pub height: u32,
    /// The scale factor, which niri allows to be fractional.
    pub scale: f64,
    /// How the picture is turned.
    ///
    /// It is NOT only what was asked for: niri adds the panel's own
    /// orientation to the configured transform, and on the `winit` connector
    /// it FORCES `Flipped180` with a comment calling it a damage workaround.
    /// So in a nested session (`nix run .#dev`) this reads `flipped-180` no
    /// matter what was set, and on a convertible with a rotated panel it
    /// disagrees with the last [`Ask::Transform`]. A row that shows "the
    /// current rotation" from here is telling the truth about the picture and
    /// not about the setting, and those are two different rows.
    pub transform: Transform,
}

/// How an output's picture is turned, counter-clockwise.
///
/// The names on the wire are niri's, and three of them are bare digits —
/// which is why they are renamed here rather than spelled as Rust
/// identifiers, since `90` is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub enum Transform {
    /// Not turned.
    #[default]
    Normal,
    /// A quarter turn.
    #[serde(rename = "90")]
    R90,
    /// Upside down.
    #[serde(rename = "180")]
    R180,
    /// Three quarters.
    #[serde(rename = "270")]
    R270,
    /// Mirrored left to right.
    Flipped,
    /// Mirrored, then a quarter turn.
    Flipped90,
    /// Flipped vertically — niri's own word for it, and the same thing as
    /// mirrored then turned upside down.
    Flipped180,
    /// Mirrored, then three quarters.
    Flipped270,
}

impl Transform {
    /// Every one, in the order a person would step through them.
    ///
    /// The four turns first and the four mirrors after: turning is what
    /// somebody with a pivoting monitor wants, and mirroring is what somebody
    /// with a projector wants, and putting them in one alternating ring would
    /// make the common gesture pass through the rare one every time.
    pub const ALL: [Self; 8] = [
        Self::Normal,
        Self::R90,
        Self::R180,
        Self::R270,
        Self::Flipped,
        Self::Flipped90,
        Self::Flipped180,
        Self::Flipped270,
    ];

    /// The spelling a CONFIG FILE uses, which is not the one the wire uses.
    ///
    /// A third asymmetry of niri's, and the one most likely to be got wrong
    /// silently: over IPC a transform is `"Flipped90"`, and in KDL it is
    /// `"flipped-90"` — the config goes through `FromStr` and the wire
    /// through serde, and the two lists of names do not agree. A quarter turn
    /// is `"90"` in both, which is exactly enough coincidence to make the
    /// mistake survive a quick test.
    #[must_use]
    pub const fn kdl(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::R90 => "90",
            Self::R180 => "180",
            Self::R270 => "270",
            Self::Flipped => "flipped",
            Self::Flipped90 => "flipped-90",
            Self::Flipped180 => "flipped-180",
            Self::Flipped270 => "flipped-270",
        }
    }

    /// Whether the turn swaps width for height.
    ///
    /// A quarter turn does; a half turn does not. It answers about a turn
    /// that has NOT happened yet — "how big would this monitor be if I turned
    /// it" — because [`Logical::width`] already has the current turn in it.
    /// Asking it about the current transform and swapping again is the double
    /// rotation this note exists to prevent.
    #[must_use]
    pub const fn turns_the_side(self) -> bool {
        matches!(
            self,
            Self::R90 | Self::R270 | Self::Flipped90 | Self::Flipped270
        )
    }
}

/// What niri did with a change to an output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Change {
    /// The output was there and the change went in.
    Applied,
    /// The output was not connected. niri KEPT the change for when it comes
    /// back, so this is an answer and not an error — asking for the mode of a
    /// monitor that is unplugged is a legitimate thing to do.
    OutputWasMissing,
}

/// A change one may ask of an output.
///
/// It exists so that no other crate writes niri's JSON. Each variant maps to
/// exactly one `OutputAction`, and [`Ask::body`] is the only place the shape
/// is written down.
///
/// `Modeline` and `CustomMode` are deliberately absent. A modeline is eleven
/// VESA timings, which is not a preference anybody sets from a list; if a
/// monitor ever needs one it belongs in the config file by hand. `MaxBpc` is
/// absent for a different reason — the pinned compositor has no such action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ask {
    /// Drive this output again.
    On,
    /// Stop driving it. Its windows go to the other outputs.
    Off,
    /// Set the mode, or `None` to let niri pick.
    Mode(Option<ModeAsked>),
    /// Set the scale, or `None` to let niri pick from the physical size.
    Scale(Option<f64>),
    /// Turn the picture.
    Transform(Transform),
    /// Place the output, or `None` to let niri place it.
    Position(Option<(i32, i32)>),
    /// Vary the refresh rate.
    ///
    /// `on: false` DROPS `on_demand` on niri's side, and an [`Output`]
    /// carries only `vrr_enabled` — never the configured on-demand flag. So
    /// "off" and "on-demand, idle right now" read back the same, and a
    /// three-state control cannot be driven from the reading alone. What was
    /// chosen has to be remembered by whoever chose it.
    Vrr {
        /// Whether to vary it at all.
        on: bool,
        /// Only while a window that asked for it is on this output. Ignored
        /// when `on` is false.
        on_demand: bool,
    },
}

/// A mode being asked for, which is not shaped like a mode being read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeAsked {
    /// Width in physical pixels.
    pub width: u16,
    /// Height in physical pixels.
    pub height: u16,
    /// The rate in **hertz**, as a float — while [`Mode::refresh_rate`] is an
    /// integer of millihertz. `None` lets niri choose among the rates that
    /// size offers.
    pub refresh: Option<f64>,
}

impl Ask {
    /// The action's body, in niri's shape.
    ///
    /// One function, so the protocol is written once and a test can read it
    /// without a compositor.
    #[must_use]
    pub fn body(self) -> serde_json::Value {
        use serde_json::json;
        match self {
            Self::On => json!("On"),
            Self::Off => json!("Off"),
            Self::Mode(None) => json!({ "Mode": { "mode": "Automatic" } }),
            Self::Mode(Some(m)) => json!({ "Mode": { "mode": { "Specific": {
                "width": m.width,
                "height": m.height,
                "refresh": m.refresh,
            } } } }),
            Self::Scale(None) => json!({ "Scale": { "scale": "Automatic" } }),
            Self::Scale(Some(f)) => json!({ "Scale": { "scale": { "Specific": f } } }),
            Self::Transform(t) => json!({ "Transform": { "transform": t } }),
            Self::Position(None) => json!({ "Position": { "position": "Automatic" } }),
            Self::Position(Some((x, y))) => {
                json!({ "Position": { "position": { "Specific": { "x": x, "y": y } } } })
            }
            Self::Vrr { on, on_demand } => {
                json!({ "Vrr": { "vrr": { "vrr": on, "on_demand": on_demand } } })
            }
        }
    }
}

/// A compositor event, reduced to what the daemon consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The complete window state. It is always the first to arrive.
    WindowsChanged(Vec<Window>),
    /// A window opened or had some property changed.
    WindowOpenedOrChanged(Window),
    /// A window closed.
    WindowClosed(u64),
    /// Focus changed. `None` when nothing is focused — for example, when a
    /// layer-shell surface (ours, with the leader active) takes it.
    FocusChanged(Option<u64>),
    /// The compositor read its configuration.
    ///
    /// It arrives once on connecting to the stream, saying how the LAST load
    /// went, and again on every reload. Since niri watches the files it
    /// includes, writing one is what causes this — and `failed` is how a
    /// fragment of ours that does not parse becomes something the screen can
    /// say instead of a change that silently did nothing.
    ConfigLoaded {
        /// Whether the load failed.
        failed: bool,
    },
    /// Any other event. See the comment at the top of the module.
    Ignored,
}

/// A connection to niri's IPC.
pub struct Niri {
    stream: BufReader<UnixStream>,
}

impl Niri {
    /// Connects to the socket `NIRI_SOCKET` points at.
    pub fn connect() -> Result<Self> {
        let path = std::env::var_os(SOCKET_ENV)
            .map(PathBuf::from)
            .with_context(|| {
                format!(
                    "{SOCKET_ENV} is not set.\n\
                     \n\
                     niri sets that variable for the processes IT starts.\n\
                     Outside a niri session, point it at the socket by hand."
                )
            })?;
        Self::connect_at(&path)
    }

    /// Connects to a specific socket. Used by the contract test, which
    /// has to talk to two different niris on the same machine.
    pub fn connect_at(path: &std::path::Path) -> Result<Self> {
        let stream = UnixStream::connect(path)
            .with_context(|| format!("could not connect to {}", path.display()))?;
        Ok(Self {
            stream: BufReader::new(stream),
        })
    }

    /// Sends a request and returns the raw reply.
    ///
    /// The protocol is one JSON line per message: request, `\n`, and the reply
    /// comes on ONE line. niri's `Reply` is a serialised `Result`, so
    /// on the wire it appears as `{"Ok": …}` or `{"Err": "…"}`.
    fn request(&mut self, request: &serde_json::Value) -> Result<serde_json::Value> {
        let mut text = serde_json::to_string(request)?;
        text.push('\n');
        self.stream
            .get_mut()
            .write_all(text.as_bytes())
            .context("could not write to niri's socket")?;
        self.stream.get_mut().flush()?;

        let mut line = String::new();
        let n = self
            .stream
            .read_line(&mut line)
            .context("could not read niri's answer")?;
        if n == 0 {
            bail!("niri closed the connection without answering {request}");
        }

        let answer: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("niri's answer is not JSON: {line:?}"))?;

        if let Some(error) = answer.get("Err") {
            bail!("niri refused {request}: {error}");
        }
        answer
            .get("Ok")
            .cloned()
            .with_context(|| format!("niri's answer has neither `Ok` nor `Err`: {line:?}"))
    }

    /// The running compositor's version.
    pub fn version(&mut self) -> Result<String> {
        let ok = self.request(&serde_json::json!("Version"))?;
        ok.get("Version")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned)
            .with_context(|| format!("Version's answer has an unexpected shape: {ok}"))
    }

    /// Every open window.
    pub fn windows(&mut self) -> Result<Vec<Window>> {
        let ok = self.request(&serde_json::json!("Windows"))?;
        let raw = ok
            .get("Windows")
            .with_context(|| format!("Windows's answer has an unexpected shape: {ok}"))?;
        serde_json::from_value(raw.clone()).context("a niri window does not match `Window`")
    }

    /// The focused window, if there is one.
    pub fn focused_window(&mut self) -> Result<Option<Window>> {
        let ok = self.request(&serde_json::json!("FocusedWindow"))?;
        let raw = ok
            .get("FocusedWindow")
            .with_context(|| format!("FocusedWindow's answer has an unexpected shape: {ok}"))?;
        serde_json::from_value(raw.clone()).context("the focused window does not match `Window`")
    }

    /// Every output the compositor can SEE, by connector name.
    ///
    /// Including the ones it is not driving: an output turned off is in the
    /// map with `logical: None`. Connected, not necessarily lit.
    ///
    /// A `BTreeMap` and not the `HashMap` niri sends: a list on a screen needs
    /// an order, and the connector's name is the only stable one there is —
    /// a hash order would reshuffle the monitors between two readings of the
    /// same unchanged machine. It sorts as text, so `DP-10` comes before
    /// `DP-2`; whatever draws the list is free to order it better, and what
    /// this promises is only that two readings agree.
    ///
    /// # ASKING IS THE ONLY WAY TO KNOW
    ///
    /// niri publishes NO event about outputs — I diffed the whole `Event`
    /// enum of the pinned version against `niri-next`: nineteen variants,
    /// identical, and not one of them concerns a monitor. A change made
    /// through [`Niri::output_action`] fires nothing either.
    ///
    /// So there is nothing to wake on here, and a caller must not wait for
    /// one. The signal that a monitor arrived or left is WAYLAND's — the
    /// `wl_output` global appearing and going away — which the shell already
    /// receives, for free, because it is a Wayland client. That is what turns
    /// into a fresh call to this.
    ///
    /// # Errors
    ///
    /// niri answered something that is not a map of outputs, or the socket
    /// went down.
    pub fn outputs(&mut self) -> Result<BTreeMap<String, Output>> {
        let ok = self.request(&serde_json::json!("Outputs"))?;
        let raw = ok
            .get("Outputs")
            .with_context(|| format!("Outputs's answer has an unexpected shape: {ok}"))?;
        serde_json::from_value(raw.clone()).context("a niri output does not match `Output`")
    }

    /// The output the keyboard is on, when there is one.
    ///
    /// It is what a key that acts on "this screen" means — the brightness
    /// keys, above all: a machine with two monitors has two brightnesses, and
    /// the one to move is the one being looked at.
    ///
    /// `None` when niri has no active output, which happens between a monitor
    /// being unplugged and the next one taking focus.
    ///
    /// # Errors
    ///
    /// niri answered something that is not an output, or the socket went down.
    pub fn focused_output(&mut self) -> Result<Option<Output>> {
        let ok = self.request(&serde_json::json!("FocusedOutput"))?;
        let raw = ok
            .get("FocusedOutput")
            .with_context(|| format!("FocusedOutput's answer has an unexpected shape: {ok}"))?;
        serde_json::from_value(raw.clone()).context("the focused output does not match `Output`")
    }

    /// Asks for a change to one output.
    ///
    /// # It is TRANSIENT, and that is the whole design
    ///
    /// niri applies this to its in-memory configuration and drops it the next
    /// time it reads the config file (`niri.rs`: "transient changes dropped").
    /// So this is the PREVIEW, and a file is the memory: write the fragment
    /// niri includes and the reload makes the file's value the truth. It also
    /// means undoing a preview costs nothing — rewriting the fragment
    /// restores it, with nothing here having to remember what was there.
    ///
    /// # Pass the CONNECTOR here, not the description
    ///
    /// niri matches either. But this is the transient path, and a preview has
    /// to land on the monitor somebody is looking at: a description shared by
    /// two identical monitors resolves to whichever comes FIRST in niri's map
    /// (`niri.rs`, `output_by_name_match`), so the picture changes on the
    /// wrong screen. The connector is unambiguous by construction.
    ///
    /// The description belongs in the CONFIGURATION block, where the point is
    /// to survive the cable moving to another port — and where a shared one
    /// is a problem of its own. See [`Output::identity`].
    ///
    /// # `Ok` DOES NOT MEAN IT HAPPENED
    ///
    /// This is the sharpest edge in the whole boundary. niri validates only
    /// the modeline and the custom mode — neither of which [`Ask`] can build
    /// — so every shape here passes, and the answer is decided before
    /// anything is applied. A mode the monitor does not have falls back to
    /// the preferred one with a warning in niri's log
    /// (`backend/tty.rs`, "could not be found, falling back to preferred");
    /// a scale is silently snapped to the nearest representable one
    /// (`closest_representable_scale`). Both answer [`Change::Applied`].
    ///
    /// So WHAT CONFIRMS A CHANGE IS THE NEXT READING, never this return
    /// value. Anything that reports success from here reports it for changes
    /// that did not happen, which is the silence this crate exists to remove.
    ///
    /// # Errors
    ///
    /// In practice only the socket: niri closed it, or it was never there.
    /// An output that is not connected is NOT an error — see [`Change`].
    pub fn output_action(&mut self, output: &str, ask: Ask) -> Result<Change> {
        let ok = self.request(&serde_json::json!({
            "Output": { "output": output, "action": ask.body() }
        }))?;
        let raw = ok
            .get("OutputConfigChanged")
            .with_context(|| format!("Output's answer has an unexpected shape: {ok}"))?;
        serde_json::from_value(raw.clone())
            .context("niri answered `Output` with a change we do not know")
    }

    /// Sends an ACTION to the compositor.
    ///
    /// # Why the shell sends an action, and does not simulate a key
    ///
    /// Closing a window and switching focus are the compositor's business
    /// (invariant 4). The shell does not reimplement them: it asks, over the same IPC
    /// it already listens on. Simulating a key would go through niri's keymap, which is
    /// another configuration and another truth.
    ///
    /// The action goes as it is in niri's protocol. Building an enum of
    /// actions here would mirror theirs and would diverge on the first new version; what
    /// covers the shape is the contract test.
    ///
    /// # Errors
    ///
    /// niri refused the action, or the socket went down.
    pub fn action(&mut self, action: serde_json::Value) -> Result<()> {
        self.request(&serde_json::json!({ "Action": action }))?;
        Ok(())
    }

    /// Closes the focused window.
    ///
    /// # Errors
    ///
    /// The same as [`Niri::action`].
    pub fn close_window(&mut self) -> Result<()> {
        // `id: null` means "the focused one" in niri's protocol. Sending the id
        // the daemon keeps would be a race: between the event and the action the
        // focus may have changed, and then we would close the wrong window — which is
        // destructive and silent.
        self.action(serde_json::json!({ "CloseWindow": { "id": null } }))
    }

    /// Passes focus to the next window.
    ///
    /// # Errors
    ///
    /// The same as [`Niri::action`].
    pub fn next_window(&mut self) -> Result<()> {
        self.action(serde_json::json!({ "FocusWindowDown": {} }))
    }

    /// An action WITHOUT an argument, by the name niri uses in the protocol.
    ///
    /// # Why the raw name, and not an enum of ours
    ///
    /// niri has 142 actions. Mirroring them in an enum would give a table that
    /// diverges on the first new version of the compositor — and diverging here
    /// means a command that disappears from the modeline with nobody noticing. The name
    /// crosses as it is; what guarantees it exists is the contract
    /// test, which exercises each one against a actual niri.
    ///
    /// # Errors
    ///
    /// niri does not know the action, or the socket went down.
    pub fn plain_action(&mut self, name: &str) -> Result<()> {
        self.action(serde_json::json!({ name: {} }))
    }

    /// The layer-shell surfaces the compositor knows about.
    ///
    /// Each comes out as `namespace@layer/interactivity`.
    ///
    /// It does not feed the modeline. It exists for two testing reasons, both about
    /// questions no log of ours answers:
    ///
    /// - "does the compositor SEE our footer?" A footer that shows up here but
    ///   not on screen is a buffer problem; one that does not even show up is a
    ///   protocol problem. They are different fixes.
    /// - "did the compositor accept the keyboard request?" The interactivity IT
    ///   reports is milestone 5's criterion, and it is different from the one we asked for —
    ///   asking is not getting.
    pub fn layers(&mut self) -> Result<Vec<String>> {
        let ok = self.request(&serde_json::json!("Layers"))?;
        let raw = ok
            .get("Layers")
            .and_then(serde_json::Value::as_array)
            .with_context(|| format!("Layers's answer has an unexpected shape: {ok}"))?;
        Ok(raw
            .iter()
            .map(|c| {
                let ns = c
                    .get("namespace")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                let layer = c
                    .get("layer")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                let keyboard = c
                    .get("keyboard_interactivity")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("?");
                format!("{ns}@{layer}/{keyboard}")
            })
            .collect())
    }

    /// Turns the connection into an event stream.
    ///
    /// It consumes the connection on purpose: after `EventStream` the socket is only
    /// good for that. A request sent afterwards would be read by niri as
    /// garbage, and the error would show up far from the cause.
    pub fn event_stream(mut self) -> Result<Events> {
        let ok = self.request(&serde_json::json!("EventStream"))?;
        // `Response::Handled` is a UNIT variant, so on the wire it is the
        // string `"Handled"`, and not `{"Handled": …}` like the ones that carry
        // data. Looking for the key in an object would always fail, and the message
        // would say "niri did not confirm" when it did.
        anyhow::ensure!(
            ok.as_str() == Some("Handled"),
            "niri did not confirm the EventStream: {ok}"
        );
        Ok(Events {
            stream: self.stream,
            deadline: false,
        })
    }
}

/// An event stream already accepted by the compositor.
///
/// niri sends the complete state on the way out, so the reader does not have to ask for
/// `Windows` first: the first window event is already the whole list.
pub struct Events {
    stream: BufReader<UnixStream>,
    /// Whether a deadline is armed on the socket.
    ///
    /// It has to be tracked because `set_read_timeout` is a property of the
    /// FILE DESCRIPTOR and outlives the call that set it: without this,
    /// draining a replay with [`Events::next_within`] and then handing the
    /// stream to a daemon thread would give that thread a `next_one` which
    /// stops blocking and starts erroring — read as a broken socket, hours
    /// from the line that armed it.
    deadline: bool,
}

impl Events {
    /// Reads the next event. It blocks for as long as it takes. `Ok(None)`
    /// when niri shuts down.
    ///
    /// It is the DAEMON's method: a footer that lives as long as the session
    /// has nothing better to do than wait, and a deadline there would only
    /// invent a failure on a quiet desktop.
    pub fn next_one(&mut self) -> Result<Option<Event>> {
        // DISARM WHATEVER `next_within` LEFT BEHIND. The deadline lives on
        // the descriptor, not on the call.
        if self.deadline {
            self.stream
                .get_ref()
                .set_read_timeout(None)
                .context("could not take the deadline off niri's socket")?;
            self.deadline = false;
        }
        self.read_a_line()
    }

    /// The same, giving up after `patience`.
    ///
    /// `Ok(None)` means THE STREAM WENT QUIET — niri closed it, or nothing
    /// arrived in time. To a caller draining niri's opening replay those are
    /// the same fact: there is nothing more coming. Anything that needs to
    /// tell them apart wants [`Self::next_one`] and a thread.
    ///
    /// It exists because the replay is a fixed short prefix and then silence.
    /// A test waiting for an event that upstream removed would otherwise wait
    /// for ever and be killed by the harness — which reports a timeout
    /// instead of the sentence naming what disappeared, and that sentence is
    /// the whole point of a contract test.
    ///
    /// AFTER A QUIET ANSWER THE STREAM IS SPENT. A deadline can fall in the
    /// middle of a line, and the bytes already read are gone; keep reading
    /// and the next event parses as rubbish. Stop.
    ///
    /// `patience` bounds ONE READ, not one call: a line longer than the
    /// reader's buffer takes several, and each gets the whole of it. niri's
    /// `WindowsChanged` with many windows is longer than eight kibibytes, so
    /// this is reachable rather than theoretical — a caller that needs a
    /// bound on the CALL wants a clock of its own.
    ///
    /// The deadline stays armed on the socket afterwards. [`Self::next_one`]
    /// takes it off again, so the two can be mixed.
    ///
    /// # Errors
    ///
    /// The socket refused the deadline — `Duration::ZERO` is refused by the
    /// kernel, which reads "no timeout" and is the opposite of what asking
    /// for zero means — or the line that arrived is not an event.
    pub fn next_within(&mut self, patience: std::time::Duration) -> Result<Option<Event>> {
        self.stream
            .get_ref()
            .set_read_timeout(Some(patience))
            .context("could not put a deadline on niri's socket")?;
        self.deadline = true;
        match self.read_a_line() {
            Err(e) if quiet(&e) => Ok(None),
            otherwise => otherwise,
        }
    }

    fn read_a_line(&mut self) -> Result<Option<Event>> {
        let mut line = String::new();
        if self.stream.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let raw: serde_json::Value = serde_json::from_str(&line)
            .with_context(|| format!("niri's event is not JSON: {line:?}"))?;
        Ok(Some(interpret(&raw)?))
    }
}

/// Whether a failure is just the deadline going off.
///
/// Two kinds, and which one arrives depends on the platform's socket rather
/// than on anything here: Linux gives `WouldBlock` on a read deadline, and
/// the documentation of `set_read_timeout` names both.
fn quiet(e: &anyhow::Error) -> bool {
    e.downcast_ref::<std::io::Error>().is_some_and(|e| {
        matches!(
            e.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        )
    })
}

/// Maps an event's raw JSON to our narrow type.
///
/// Separated from the reading so it can be tested without a socket: the tests below pass
/// literal JSON, which is how a actual event arrives on the wire.
fn interpret(raw: &serde_json::Value) -> Result<Event> {
    // A UNIT VARIANT ARRIVES AS A BARE STRING, not as an object. niri has
    // none today, and the day it adds one this used to take the whole stream
    // down — which is the exact opposite of what the module's header
    // promises. An event we cannot name is an event we do not use.
    if raw.is_string() {
        return Ok(Event::Ignored);
    }
    let Some((name, body)) = raw.as_object().and_then(|o| o.iter().next()) else {
        bail!("an event with no variant: {raw}");
    };

    let event = match name.as_str() {
        "WindowsChanged" => {
            let windows = body
                .get("windows")
                .context("WindowsChanged with no `windows`")?;
            Event::WindowsChanged(
                serde_json::from_value(windows.clone())
                    .context("a WindowsChanged window does not match `Window`")?,
            )
        }
        "WindowOpenedOrChanged" => {
            let window = body
                .get("window")
                .context("WindowOpenedOrChanged with no `window`")?;
            Event::WindowOpenedOrChanged(
                serde_json::from_value(window.clone())
                    .context("WindowOpenedOrChanged's window does not match `Window`")?,
            )
        }
        "WindowClosed" => Event::WindowClosed(
            body.get("id")
                .and_then(serde_json::Value::as_u64)
                .context("WindowClosed with no `id`")?,
        ),
        "WindowFocusChanged" => {
            Event::FocusChanged(body.get("id").and_then(serde_json::Value::as_u64))
        }
        // A LOAD THAT FAILED IS THE INTERESTING ONE, so an absent `failed`
        // reads as `false`: niri promises the field, and inventing `true`
        // from its absence would put a refusal on screen over a reload that
        // went fine.
        "ConfigLoaded" => Event::ConfigLoaded {
            failed: body
                .get("failed")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
        },
        _ => Event::Ignored,
    };
    Ok(event)
}

/// The state the modeline reads, maintained by the event stream.
///
/// It lives in the DAEMON, not in the surface — section 9.2 of the plan decided it that way
/// so a prompt survives the focused output disappearing.
#[derive(Debug, Default)]
pub struct State {
    windows: Vec<Window>,
    focus: Option<u64>,
}

impl State {
    /// Applies an event. It is the only way to change the state.
    pub fn apply_to(&mut self, event: &Event) {
        match event {
            Event::WindowsChanged(js) => {
                // niri sends the focus INSIDE the window, and the focus event
                // may not come with it. Reading from there keeps the two coherent.
                self.focus = js.iter().find(|j| j.is_focused).map(|j| j.id);
                self.windows = js.clone();
            }
            Event::WindowOpenedOrChanged(j) => {
                if let Some(existing) = self.windows.iter_mut().find(|x| x.id == j.id) {
                    *existing = j.clone();
                } else {
                    self.windows.push(j.clone());
                }
                // The event's documentation is explicit: "If the window is
                // focused, all other windows are no longer focused." Without
                // this, a window that opens ALREADY focused only shows up in the modeline
                // if niri also sends `WindowFocusChanged` — and depending on
                // that is depending on an order nobody promised.
                if j.is_focused {
                    self.focus = Some(j.id);
                    for otherwise_one in &mut self.windows {
                        otherwise_one.is_focused = otherwise_one.id == j.id;
                    }
                }
            }
            Event::WindowClosed(id) => {
                self.windows.retain(|j| j.id != *id);
                if self.focus == Some(*id) {
                    self.focus = None;
                }
            }
            Event::FocusChanged(id) => self.focus = *id,
            // A RELOAD SAYS NOTHING ABOUT THE WINDOWS. It is listened to
            // elsewhere — it is how the display view learns its fragment
            // landed — and here it has to be as inert as an unknown event.
            Event::ConfigLoaded { .. } | Event::Ignored => {}
        }
    }

    /// The focused window, if there is one.
    #[must_use]
    pub fn focused(&self) -> Option<&Window> {
        let id = self.focus?;
        self.windows.iter().find(|j| j.id == id)
    }

    /// Every window the compositor knows about, in its own order.
    ///
    /// The buffer selector (`LEADER w w`) lists these: a "buffer" of this system
    /// IS a window of the compositor (invariant 4), so there is no second list
    /// to keep in step.
    #[must_use]
    pub fn all_windows(&self) -> &[Window] {
        &self.windows
    }

    /// How many windows the compositor has.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.windows.len()
    }
}

/// The compositor's ACTION TABLE, extracted from the PINNED niri.
///
/// Generated by `nix/scripts/gen-niri-actions.py` and copied by `build.rs` — see
/// `nix/niri-actions.nix` for why it is generated.
mod table {
    include!(concat!(env!("OUT_DIR"), "/niri-actions.rs"));
}

/// The compositor's DEFAULT BINDS, from the pinned commit's `default-config.kdl`.
///
/// It is the authoritative list of the shortcuts niri ships — and not one of ours, which
/// would age silently with every release.
mod default_of {
    include!(concat!(env!("OUT_DIR"), "/binds-default.rs"));
}

pub use default_of::{DEFAULT_BINDS, DefaultBind};
pub use table::{NIRI_ACTIONS, NiriAction};

/// The niri binary, by path and not by name.
///
/// The footer runs as a user unit, and a unit's `PATH` is minimal:
/// a bare name would fail with "No such file or directory" far from the cause. Outside
/// Nix it falls back to the name, and the shell's `PATH` resolves it.
const NIRI: &str = match option_env!("RONOCTUA_NIRI") {
    Some(c) => c,
    None => "niri",
};

/// Runs a compositor action by its kebab-case NAME.
///
/// # Why through niri's client, and not through our IPC
///
/// Our `Niri::action` sends `{"Name": {arguments}}` assembled here — and the
/// arguments are the problem. `Screenshot` requires `show_pointer` and
/// `MoveWindowToWorkspaceDown` requires `focus`, with defaults that exist in
/// **niri's clap** and nowhere we can read. Sending `{}` gives
/// `"error parsing request"`, which is the SAME error as a non-existent name.
///
/// For the 141 mirrored actions, the compositor's own client fills in
/// those defaults: `niri msg action <name> [argument]`. There is no conversion
/// of ours to get wrong, and the error that comes back is theirs, with their text.
///
/// The ~25 keymap commands still go through the direct IPC, which is the hot
/// path and is already proved, action by action, by `niri-contract`.
///
/// # Errors
///
/// The client cannot be run, or niri refused the action — and then their text
/// comes back whole, because it is the only clue.
pub fn run_through_client(name: &str, argument: Option<&str>) -> Result<()> {
    let mut cmd = std::process::Command::new(NIRI);
    cmd.args(["msg", "action", name]);
    if let Some(a) = argument {
        cmd.arg(a);
    }
    let output = cmd
        .output()
        .with_context(|| format!("could not run `{NIRI} msg action {name}`"))?;
    if output.status.success() {
        return Ok(());
    }
    let reason = String::from_utf8_lossy(&output.stderr);
    let reason = reason.trim();
    bail!(
        "{}",
        if reason.is_empty() {
            format!("`{name}` failed with {}", output.status)
        } else {
            reason.to_owned()
        }
    )
}

/// Does the action exist in this niri?
#[must_use]
pub fn action_exists(name: &str) -> bool {
    NIRI_ACTIONS.iter().any(|a| a.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_json(id: u64, focused: bool) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "title": "arq.rs",
            "app_id": "ghostty",
            "pid": 42,
            "workspace_id": 1,
            "is_focused": focused,
            "is_floating": false,
            "is_urgent": false,
            "layout": { "pos_in_scrolling_layout": null },
            "focus_timestamp": null
        })
    }

    #[test]
    fn an_unknown_event_is_not_an_error() {
        // niri ADDS variants with every release. If this became an error, the
        // distro would stop coming up on an upgrade over an event we do not use.
        let fresh = serde_json::json!({ "SomethingThatDoesNotExistYet": { "x": 1 } });
        assert_eq!(interpret(&fresh).unwrap(), Event::Ignored);
    }

    #[test]
    fn a_new_field_in_a_window_is_not_an_error() {
        // Same reason, one level down: `Window` ignores fields it does not
        // declare. Without that, a new field in `Window` would break everything.
        let mut j = window_json(1, true);
        j["a_field_from_the_future"] = serde_json::json!("value");
        let event = interpret(&serde_json::json!({ "WindowOpenedOrChanged": { "window": j } }));
        assert!(matches!(event.unwrap(), Event::WindowOpenedOrChanged(_)));
    }

    #[test]
    fn the_focus_comes_from_windows_changed() {
        // The initial `WindowsChanged` carries the focus embedded and may not come
        // accompanied by `WindowFocusChanged`. Reading only the focus event
        // would leave the modeline empty until the first Alt-Tab.
        let mut e = State::default();
        let raw = serde_json::json!({
            "WindowsChanged": { "windows": [window_json(1, false), window_json(7, true)] }
        });
        e.apply_to(&interpret(&raw).unwrap());
        assert_eq!(e.focused().map(|j| j.id), Some(7));
        assert_eq!(e.amount(), 2);
    }

    #[test]
    fn closing_the_focused_window_zeroes_the_focus() {
        let mut e = State::default();
        e.apply_to(
            &interpret(&serde_json::json!({
                "WindowsChanged": { "windows": [window_json(7, true)] }
            }))
            .unwrap(),
        );
        e.apply_to(&Event::WindowClosed(7));
        assert!(e.focused().is_none());
        assert_eq!(e.amount(), 0);
    }

    #[test]
    fn a_null_focus_is_valid() {
        // It really does happen: when the active leader takes the keyboard, focus
        // goes to our layer-shell and niri sends `id: null`. Treating that
        // as an error would take the shell down on its own main shortcut.
        let e = interpret(&serde_json::json!({ "WindowFocusChanged": { "id": null } }));
        assert_eq!(e.unwrap(), Event::FocusChanged(None));
    }

    #[test]
    fn a_window_that_opens_already_focused_gains_the_focus() {
        // niri promises "if the window is focused, all other windows are no
        // longer focused" IN THIS event. Waiting for a separate
        // `WindowFocusChanged` is depending on an order nobody promised — and that is
        // how the modeline ended up on `*scratch*` with a window open.
        let mut e = State::default();
        e.apply_to(
            &interpret(&serde_json::json!({
                "WindowsChanged": { "windows": [window_json(1, true)] }
            }))
            .unwrap(),
        );
        e.apply_to(
            &interpret(&serde_json::json!({
                "WindowOpenedOrChanged": { "window": window_json(2, true) }
            }))
            .unwrap(),
        );
        assert_eq!(e.focused().map(|j| j.id), Some(2));
        // And the previous one really did lose focus, not just in `self.focus`.
        assert!(!e.windows.iter().find(|j| j.id == 1).unwrap().is_focused);
    }

    #[test]
    fn opening_the_same_window_twice_does_not_duplicate() {
        // `WindowOpenedOrChanged` is also emitted for a CHANGE — the name says so.
        // Treating it as an insertion would fill the list with duplicates silently.
        let mut e = State::default();
        let open = interpret(&serde_json::json!({
            "WindowOpenedOrChanged": { "window": window_json(3, true) }
        }))
        .unwrap();
        e.apply_to(&open);
        e.apply_to(&open);
        assert_eq!(e.amount(), 1);
    }

    /// A monitor niri could report — INVENTED, and it has to be.
    ///
    /// The make, the model and the physical size are hardware identity, and
    /// this repository is public (invariants 2 and 13): a verbatim capture of
    /// the machine it was written on would put the owner's laptop model in it
    /// for ever. What the fixture actually needs is a NON-ROUND refresh rate
    /// beside a round one, which any two numbers give.
    fn output_json(on: bool) -> serde_json::Value {
        serde_json::json!({
            "name": "eDP-1",
            "make": "Maker",
            "model": "Model-1",
            "serial": null,
            "physical_size": [300, 200],
            "modes": [
                { "width": 1920, "height": 1200, "refresh_rate": 59951, "is_preferred": true },
                { "width": 1920, "height": 1080, "refresh_rate": 60000, "is_preferred": false }
            ],
            "current_mode": if on { serde_json::json!(0) } else { serde_json::json!(null) },
            "is_custom_mode": false,
            "vrr_supported": false,
            "vrr_enabled": false,
            "logical": if on {
                serde_json::json!({
                    "x": 0, "y": 0, "width": 1920, "height": 1200,
                    "scale": 1.0, "transform": "Normal"
                })
            } else {
                serde_json::json!(null)
            }
        })
    }

    #[test]
    fn every_transform_is_read_by_the_name_niri_writes() {
        // THREE OF THE EIGHT ARE BARE DIGITS on the wire, which is why they
        // are renamed rather than spelled. A wrong name here is not a
        // compilation error and not a panic: it is niri refusing a rotation
        // with a message about an unknown variant, months later.
        let wire = [
            (Transform::Normal, "\"Normal\""),
            (Transform::R90, "\"90\""),
            (Transform::R180, "\"180\""),
            (Transform::R270, "\"270\""),
            (Transform::Flipped, "\"Flipped\""),
            (Transform::Flipped90, "\"Flipped90\""),
            (Transform::Flipped180, "\"Flipped180\""),
            (Transform::Flipped270, "\"Flipped270\""),
        ];
        for (t, expected) in wire {
            assert_eq!(serde_json::to_string(&t).unwrap(), expected, "{t:?}");
            let back: Transform = serde_json::from_str(expected).unwrap();
            assert_eq!(back, t, "{expected} did not come back");
        }
        // And the array a person steps through is all of them, once each.
        assert_eq!(Transform::ALL.len(), wire.len());
        for (t, _) in wire {
            assert!(Transform::ALL.contains(&t), "{t:?} is not in ALL");
        }
    }

    #[test]
    fn a_config_file_spells_a_transform_differently_from_the_wire() {
        // Read off `niri-ipc`'s `impl FromStr for Transform`. The two lists
        // agree on three of the eight, which is what makes getting this wrong
        // survive a test that only tries a rotation.
        let kdl = [
            (Transform::Normal, "normal"),
            (Transform::R90, "90"),
            (Transform::R180, "180"),
            (Transform::R270, "270"),
            (Transform::Flipped, "flipped"),
            (Transform::Flipped90, "flipped-90"),
            (Transform::Flipped180, "flipped-180"),
            (Transform::Flipped270, "flipped-270"),
        ];
        for (t, expected) in kdl {
            assert_eq!(t.kdl(), expected, "{t:?}");
        }
        // And the two spellings really do differ, so neither can stand in for
        // the other.
        let wire = serde_json::to_string(&Transform::Flipped90).unwrap();
        assert_eq!(wire, "\"Flipped90\"");
        assert_ne!(wire.trim_matches('"'), Transform::Flipped90.kdl());
    }

    #[test]
    fn a_monitor_is_named_by_what_it_says_about_itself() {
        // The name a config block must carry. It survives the cable moving
        // to another port, which the connector does not.
        let o: Output = serde_json::from_value(output_json(true)).unwrap();
        assert_eq!(o.identity(), "Maker Model-1 Unknown");
        // And it is NOT what its settings are filed under: no serial.
        assert!(!o.naming().follows_the_monitor());
    }

    #[test]
    fn a_monitor_with_no_serial_is_filed_under_its_port() {
        // A make and a model are shared by every unit of a product, so a
        // monitor that gives no serial is one an identical second could
        // answer for — and niri would give them ONE configuration block, one
        // position, and place the second automatically.
        //
        // THE RULE READS ONE MONITOR'S OWN EDID, and never "is there a twin
        // plugged in right now": that answer changes when a second one
        // arrives, and the settings filed under the old name would silently
        // stop being found and come back when it was unplugged.
        let mut anonymous = output_json(true);
        anonymous["make"] = serde_json::json!("Maker Inc.");
        anonymous["model"] = serde_json::json!("Model-3");
        anonymous["serial"] = serde_json::json!(null);
        let o: Output = serde_json::from_value(anonymous.clone()).unwrap();
        assert_eq!(
            o.naming(),
            Naming::Connector {
                name: "eDP-1".to_owned(),
                why: Anonymous::NoSerial
            }
        );
        assert_eq!(o.naming().name(), "eDP-1");
        assert!(!o.naming().follows_the_monitor());
        // The description is still what it is — it is just not a filing key.
        assert_eq!(o.identity(), "Maker Inc. Model-3 Unknown");

        // With a serial it names one monitor, and the settings follow it.
        anonymous["serial"] = serde_json::json!("ABC123");
        let told_apart: Output = serde_json::from_value(anonymous).unwrap();
        assert_eq!(
            told_apart.naming(),
            Naming::Description("Maker Inc. Model-3 ABC123".to_owned())
        );
        assert!(told_apart.naming().follows_the_monitor());
    }

    #[test]
    fn a_monitor_that_says_nothing_is_named_by_its_port() {
        // niri refuses to match by description when all three parts are
        // unknown — it would match every anonymous monitor on the machine.
        // Writing `Unknown Unknown Unknown` into the config would be a block
        // that niri silently never applies.
        let mut j = output_json(true);
        j["make"] = serde_json::json!("Unknown");
        j["model"] = serde_json::json!("Unknown");
        j["serial"] = serde_json::json!(null);
        let o: Output = serde_json::from_value(j).unwrap();
        // The DESCRIPTION is what it says about itself, however empty.
        assert_eq!(o.identity(), "Unknown Unknown Unknown");
        // The FILING KEY is the connector, because niri refuses to match
        // three `Unknown`s — it would match every anonymous monitor at once.
        assert_eq!(
            o.naming(),
            Naming::Connector {
                name: "eDP-1".to_owned(),
                why: Anonymous::Silent
            }
        );
    }

    #[test]
    fn a_quarter_turn_swaps_the_sides_and_a_half_turn_does_not() {
        // Whatever draws the arrangement asks this. Without it a portrait
        // monitor is drawn lying down, in the one screen whose whole job is
        // to say where things are.
        for t in [
            Transform::R90,
            Transform::R270,
            Transform::Flipped90,
            Transform::Flipped270,
        ] {
            assert!(t.turns_the_side(), "{t:?}");
        }
        for t in [
            Transform::Normal,
            Transform::R180,
            Transform::Flipped,
            Transform::Flipped180,
        ] {
            assert!(!t.turns_the_side(), "{t:?}");
        }
    }

    #[test]
    fn an_output_that_is_off_has_no_place_and_no_mode() {
        // `logical` and `current_mode` are the two that vanish, and both are
        // `Option` for exactly this. Reading them as required would make a
        // second monitor that is turned off take the whole list down.
        let off: Output = serde_json::from_value(output_json(false)).unwrap();
        assert!(off.logical.is_none());
        assert!(off.current_mode.is_none());
        // The modes are still there: an off output still says what it could do.
        assert_eq!(off.modes.len(), 2);
    }

    #[test]
    fn a_new_field_in_an_output_is_not_an_error() {
        // The same tolerance `Window` has, and for the same reason: niri
        // versions by calendar and adds fields.
        let mut j = output_json(true);
        j["something_from_the_future"] = serde_json::json!(7);
        let o: Output = serde_json::from_value(j).unwrap();
        assert_eq!(o.name, "eDP-1");
    }

    #[test]
    fn the_rounded_rate_is_for_reading_and_the_exact_one_is_for_asking() {
        // A PANEL SOLD AS 60 Hz REPORTS 59951 mHz, which is the measurement
        // this fixture is built around — and the 1080p mode beside it reports
        // a true 60000. The
        // two are DIFFERENT modes that `hertz()` deliberately shows with the
        // same words, which is why it must never be what is asked for.
        let o: Output = serde_json::from_value(output_json(true)).unwrap();
        assert_eq!(o.modes[0].refresh_rate, 59951);
        assert!((o.modes[0].hertz() - 60.0).abs() < f64::EPSILON);
        assert!((o.modes[1].hertz() - 60.0).abs() < f64::EPSILON);
        assert_ne!(o.modes[0].refresh_rate, o.modes[1].refresh_rate);
        // One decimal is what separates the rates that a screen must
        // separate — 143,9 from 144,0 — without spilling millihertz.
        let fast = Mode {
            width: 2560,
            height: 1440,
            refresh_rate: 143_856,
            is_preferred: false,
        };
        assert!((fast.hertz() - 143.9).abs() < 0.001);
    }

    #[test]
    fn a_mode_asked_for_the_way_niri_matches_it_comes_back() {
        // NIRI MATCHES BY EQUALITY on an integer of millihertz: it computes
        // `(refresh * 1000.).round() as i32` and compares it to the mode's
        // own `refresh_rate` (`backend/tty.rs`). So the round trip through a
        // float has to be exact, and this reproduces niri's own arithmetic
        // rather than trusting that it is.
        //
        // The rounded `hertz()` fails it, which is the whole point: asking
        // with that number matches nothing, niri falls back to the preferred
        // mode, and the answer on the wire is still `Applied`.
        for rate in [59_951u32, 60_000, 143_856, 23_976, 239_760, 29_970] {
            let m = Mode {
                width: 1920,
                height: 1080,
                refresh_rate: rate,
                is_preferred: false,
            };
            let asked = m.asked().refresh.unwrap();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let back = (asked * 1000.0).round() as u32;
            assert_eq!(back, rate, "{rate} did not survive as {asked}");
            assert_eq!(ModeAsked::from(m), m.asked());
            assert_eq!(m.asked().width, 1920);
        }
        // And the rounded one really does miss, on the panel measured here.
        let m = Mode {
            width: 1920,
            height: 1200,
            refresh_rate: 59_951,
            is_preferred: true,
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let rounded = (m.hertz() * 1000.0).round() as u32;
        assert_ne!(rounded, m.refresh_rate);
    }

    #[test]
    fn both_answers_to_a_change_are_read() {
        // Every probe in the contract sees `OutputWasMissing`, because a name
        // that matches nothing is what makes them inert. So `Applied` is only
        // ever read HERE — and this compares our deserialiser against our own
        // literal, which means it CANNOT catch upstream renaming the variant.
        // Nothing can, short of a probe that changes a real monitor. What it
        // does catch is somebody changing this end.
        let yes: Change = serde_json::from_value(serde_json::json!("Applied")).unwrap();
        assert_eq!(yes, Change::Applied);
        let no: Change = serde_json::from_value(serde_json::json!("OutputWasMissing")).unwrap();
        assert_eq!(no, Change::OutputWasMissing);
        // And a third answer is a protocol break, not something to shrug at.
        assert!(serde_json::from_value::<Change>(serde_json::json!("Maybe")).is_err());
    }

    #[test]
    fn a_stream_that_goes_quiet_answers_instead_of_waiting_for_ever() {
        // THE DEADLINE, proved rather than described. niri's opening replay
        // is a fixed short prefix and then silence; a reader waiting for an
        // event upstream removed would block until the test harness killed
        // it, and a harness timeout says nothing about WHAT disappeared.
        //
        // A socket pair is the whole compositor this needs: one line, then
        // nothing.
        let pair = || {
            let (theirs, ours) = UnixStream::pair().unwrap();
            (
                theirs,
                Events {
                    stream: BufReader::new(ours),
                    deadline: false,
                },
            )
        };
        let patience = std::time::Duration::from_millis(120);

        let (theirs, mut events) = pair();
        (&theirs)
            .write_all(b"{\"WindowFocusChanged\":{\"id\":null}}\n")
            .unwrap();
        assert_eq!(
            events.next_within(patience).unwrap(),
            Some(Event::FocusChanged(None))
        );

        // And now the silence. It has to come back, and come back at all —
        // the upper bound is here to catch a wait that never ends, not to
        // measure how punctual the kernel is, so it is loose on purpose: this
        // runs beside fifteen hundred other tests on a loaded machine.
        let began = std::time::Instant::now();
        assert_eq!(events.next_within(patience).unwrap(), None);
        let waited = began.elapsed();
        assert!(waited >= patience, "it did not wait at all: {waited:?}");
        assert!(
            waited < std::time::Duration::from_secs(10),
            "it waited far too long: {waited:?}"
        );

        // A stream the other end CLOSED answers the same way, and that is
        // deliberate: to whoever is draining a replay the two are one fact.
        // A FRESH PAIR, because the method's own documentation says a stream
        // that answered quiet is spent — a test that ignored that would be
        // teaching the thing it warns about.
        let (theirs, mut ended) = pair();
        drop(theirs);
        assert_eq!(ended.next_within(patience).unwrap(), None);
    }

    #[test]
    fn a_deadline_does_not_outlive_the_call_that_wanted_it() {
        // `set_read_timeout` is a property of the DESCRIPTOR. Draining a
        // replay with a deadline and then handing the stream to the daemon
        // thread used to give that thread a `next_one` which stopped blocking
        // and started erroring — which reads as a broken socket, hours from
        // the line that armed it.
        let (theirs, ours) = UnixStream::pair().unwrap();
        let mut events = Events {
            stream: BufReader::new(ours),
            deadline: false,
        };
        assert_eq!(
            events
                .next_within(std::time::Duration::from_millis(30))
                .unwrap(),
            None
        );
        assert!(events.deadline);

        // The blocking method has to come back to blocking. It is proved by
        // it WAITING: a line sent from another thread after a pause is one it
        // could only have received by still being there.
        let sender = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(120));
            (&theirs)
                .write_all(b"{\"WindowClosed\":{\"id\":7}}\n")
                .unwrap();
            theirs
        });
        assert_eq!(
            events.next_one().unwrap(),
            Some(Event::WindowClosed(7)),
            "the deadline outlived the call that armed it"
        );
        assert!(!events.deadline);
        drop(sender.join().unwrap());
    }

    #[test]
    fn an_event_that_is_a_bare_word_is_ignored_and_does_not_end_the_stream() {
        // niri has no unit-variant event today. The day it adds one, this
        // used to `bail!` and take the whole stream down — which is the
        // opposite of what the module's header promises.
        assert_eq!(
            interpret(&serde_json::json!("SomethingNew")).unwrap(),
            Event::Ignored
        );
    }

    #[test]
    fn an_output_from_a_newer_niri_still_reads() {
        // `niri-next` adds `max_bpc`, which the pinned one does not send. The
        // day the pin moves, an output carrying it has to go on deserialising
        // — otherwise the upgrade takes the whole list down over a field we
        // deliberately do not read.
        let mut j = output_json(true);
        j["max_bpc"] = serde_json::json!(10);
        let o: Output = serde_json::from_value(j).unwrap();
        assert_eq!(o.name, "eDP-1");
    }

    #[test]
    fn every_ask_has_the_shape_niri_documents() {
        // Read off `niri-ipc`'s `OutputAction`, variant by variant. This is
        // the only place the protocol's shape is written, so it is the only
        // place it can be checked without a compositor.
        let cases = [
            (Ask::On, serde_json::json!("On")),
            (Ask::Off, serde_json::json!("Off")),
            (
                Ask::Mode(None),
                serde_json::json!({ "Mode": { "mode": "Automatic" } }),
            ),
            (
                Ask::Mode(Some(ModeAsked {
                    width: 1920,
                    height: 1200,
                    refresh: Some(59.951),
                })),
                serde_json::json!({ "Mode": { "mode": { "Specific": {
                    "width": 1920, "height": 1200, "refresh": 59.951 } } } }),
            ),
            (
                Ask::Scale(None),
                serde_json::json!({ "Scale": { "scale": "Automatic" } }),
            ),
            (
                Ask::Scale(Some(1.5)),
                serde_json::json!({ "Scale": { "scale": { "Specific": 1.5 } } }),
            ),
            (
                Ask::Transform(Transform::R270),
                serde_json::json!({ "Transform": { "transform": "270" } }),
            ),
            (
                Ask::Position(None),
                serde_json::json!({ "Position": { "position": "Automatic" } }),
            ),
            (
                Ask::Position(Some((1920, -180))),
                serde_json::json!({ "Position": { "position": { "Specific": {
                    "x": 1920, "y": -180 } } } }),
            ),
            (
                Ask::Vrr {
                    on: true,
                    on_demand: true,
                },
                serde_json::json!({ "Vrr": { "vrr": { "vrr": true, "on_demand": true } } }),
            ),
        ];
        for (ask, expected) in cases {
            assert_eq!(ask.body(), expected, "{ask:?}");
        }
    }

    #[test]
    fn a_reload_is_an_event_and_says_whether_it_failed() {
        // It is how a fragment of ours that does not parse becomes a sentence
        // on screen instead of a change that quietly did nothing.
        let bad = interpret(&serde_json::json!({ "ConfigLoaded": { "failed": true } }));
        assert_eq!(bad.unwrap(), Event::ConfigLoaded { failed: true });
        let good = interpret(&serde_json::json!({ "ConfigLoaded": { "failed": false } }));
        assert_eq!(good.unwrap(), Event::ConfigLoaded { failed: false });
        // An absent field is NOT a failure: inventing one would put a refusal
        // on screen over a reload that went fine.
        let bare = interpret(&serde_json::json!({ "ConfigLoaded": {} }));
        assert_eq!(bare.unwrap(), Event::ConfigLoaded { failed: false });
    }

    #[test]
    fn a_reload_does_not_disturb_the_windows() {
        // `State` has to ignore it. It used to fall into `Ignored`, and the
        // day it stopped doing that is the day it could start clearing the
        // window list by accident.
        let mut e = State::default();
        e.apply_to(
            &interpret(&serde_json::json!({
                "WindowsChanged": { "windows": [window_json(1, true)] }
            }))
            .unwrap(),
        );
        e.apply_to(&Event::ConfigLoaded { failed: false });
        assert_eq!(e.amount(), 1);
        assert_eq!(e.focused().map(|j| j.id), Some(1));
    }
}
