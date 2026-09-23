//! The CONTRACT with the compositor — invariant 3.
//!
//! This crate is the vocabulary: what a window is, what a monitor is, what can
//! be asked of one, and what arrives when something changes. It knows **no**
//! protocol. There is no socket here, no JSON, no command string, and no
//! compositor's name — grep it and see.
//!
//! # Why a contract and not just a client
//!
//! Because everything this project builds has to keep working when the window
//! manager underneath it is replaced, and the way to mean that rather than
//! merely to say it is to write the boundary down as a type. One crate speaks
//! the wire; every other crate speaks this. When the compositor changes, one
//! crate is rewritten and the rest is recompiled unchanged.
//!
//! What keeps that honest is not good intentions: it is
//! `only-the-composition-root-names-the-compositor`, which refuses any mention
//! of the adapter outside the three files that are allowed to choose one.
//!
//! # The method that is deliberately absent
//!
//! There is no `fn run(&mut self, text: &str)`. The moment it exists, every
//! caller can send `"focus left"` and the boundary is decoration: the
//! vocabulary would be bypassed by whoever was in a hurry, and the next
//! compositor would break in a hundred places instead of one. Everything
//! reaches the wire either as an [`Ask`] about a monitor or as a [`Deed`] from
//! the declared table.
//!
//! # Tolerance of unknown variants
//!
//! What we do not recognise becomes [`Event::Ignored`] and life goes on. A
//! compositor adds events; an enum that refused them would break the desktop
//! over something nobody reads. The contract test proves the opposite thing —
//! that what we DO use is still there.
//!
//! It covers events and added fields, and deliberately not [`Transform`],
//! which is frozen by something outside any compositor: there are eight
//! Wayland transforms, and a ninth would be a protocol break worth failing on.

#![forbid(unsafe_code)]

pub mod input;

pub use input::{
    AccelProfile, ButtonMap, ClickMethod, Device, DeviceKind, DragLock, FingerDrag, FocusFollows,
    KeyboardSetting, PointerSettings, ScrollMethod, Seat, SeatAsk, SendEvents, Setting,
};

use std::collections::BTreeMap;
use std::time::Duration;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// A window, reduced to what the modeline uses (section 7.1 of the plan).
///
/// Fields a compositor has and we do not: position, size, column,
/// workspace, urgency, focus timestamp. They come in when a segment needs them, and
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
// These are OURS. A compositor answers something shaped like its own
// protocol and we map it here; no other crate sees that shape, which is the
// whole point of the boundary.
//
// THE ASYMMETRY, measured rather than guessed: a refresh rate is READ in
// millihertz as an integer and ASKED FOR in hertz as a float. It is written
// on the two fields it belongs to, because a caller who does not know it
// sends a number that matches no mode — and [`Mode::asked`] exists so that
// nobody has to do the conversion by hand.
//
// WHAT IS NOT HERE: the physical size. It is not on this type because a
// compositor need not report it, and ours does not; it is read from the
// EDID in `/sys/class/drm` by `ronoctua_display::connector`, which is
// already the place that joins a connector to the kernel's view of it.

/// A monitor, as the compositor sees it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Output {
    /// The connector — `eDP-1`, `DP-3`.
    ///
    /// It is what the kernel calls the port without the `cardN-` prefix
    /// `/sys/class/drm` puts on it, which is how a reading of that directory
    /// joins up with this.
    pub name: String,
    /// The manufacturer, or the word `Unknown` — the compositor writes the
    /// word rather than leaving the field out, so an empty-looking make is
    /// still a string.
    pub make: String,
    /// The model, under the same rule as [`Self::make`].
    pub model: String,
    /// The serial the EDID carries, when it carries one.
    pub serial: Option<String>,
    /// Every mode the monitor offers, in the order it offers them.
    pub modes: Vec<Mode>,
    /// Which of [`Self::modes`] is running.
    ///
    /// `None` while the output is off — and also, on an output that is very
    /// much on, when the mode it is running is not in the list it offers.
    /// That is not hypothetical: a compositor asked for a mode it has no
    /// entry for synthesises the current one from the width, height and
    /// refresh it ended up with, and no index can be found for it. So
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
    /// A compositor matches an `output` block by the connector OR by
    /// `"<make> <model> <serial>"`, substituting the word `Unknown` for a
    /// part it does not know. The description is the one worth writing down,
    /// because it survives the cable moving to another port — the connector
    /// does not.
    ///
    /// It is ALWAYS the description, even when every part of it is the word
    /// `Unknown` — this is what a monitor says about itself, and a screen
    /// showing it is showing the truth. It used to fall back to the connector
    /// in that case, which made it a filing key wearing a description's name;
    /// what decides a filing key is [`Output::naming`], and the trouble with
    /// three `Unknown`s is handled there.
    ///
    /// # IT NAMES A MODEL, WHICH IS NOT ALWAYS ONE MONITOR
    ///
    /// Two identical monitors with no serial in their EDID answer the same
    /// string, and that is the commonest two-monitor desk there is. A
    /// description is applied to EVERY output that answers to it, so both
    /// would be given the same position, the second would overlap the first,
    /// and the compositor would place it automatically with a line in its log
    /// and nothing on screen.
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
    /// No make, no model and no serial. A description of three `Unknown`s is
    /// refused as a filing key for the reason it would be wrong: it would
    /// match every anonymous monitor at once.
    Silent,
    /// A make and a model, but no serial: every unit of that product answers
    /// the same string.
    NoSerial,
}

/// The word a compositor writes where a monitor told it nothing.
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
}

impl Mode {
    /// The rate in hertz, ROUNDED, and only for showing a person.
    ///
    /// One decimal, which is what tells a 143,9 Hz mode from a 144,0 one on a
    /// screen without spilling millihertz into it.
    ///
    /// # Never ask for a mode with this
    ///
    /// A rate is matched by turning it back into an integer of millihertz —
    /// `(r * 1000.).round()` — and looking for the NEAREST among the modes of
    /// that size. A panel's 59951 mHz rounds to `60.0` here, which asks for
    /// 60000, which lands on a neighbouring rate rather than the one that was
    /// on screen. [`Mode::asked`] is the one that comes back unchanged.
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
    /// The only conversion that survives the round trip. Use it instead of
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
    /// A compositor transforms the size before it reports it, so on
    /// a quarter turn this is the physical HEIGHT over the scale. Anything
    /// drawing the arrangement uses these two as they are and must NOT swap
    /// them again — that is what [`Transform::turns_the_side`] is for, and it
    /// is for PREDICTING a turn that has not happened yet, never for
    /// interpreting one that has.
    pub width: u32,
    /// Height in logical pixels, under the same rule as [`Self::width`].
    pub height: u32,
    /// The scale factor, which may be fractional.
    pub scale: f64,
    /// How the picture is turned.
    ///
    /// It is NOT only what was asked for: a compositor adds the panel's own
    /// orientation to the configured transform, so on a convertible with a
    /// rotated panel this disagrees with the last [`Ask::Transform`]. A row
    /// that shows "the current rotation" from here is telling the truth about
    /// the picture and
    /// not about the setting, and those are two different rows.
    pub transform: Transform,
}

/// How an output's picture is turned, CLOCKWISE.
///
/// The direction is worth saying out loud because it is not universal: the
/// Wayland protocol counts anticlockwise and this compositor counts
/// clockwise, inverting on the way in and out. What is stored here is what a
/// person means by "turn it right", and the adapter is where the two meet.
///
/// Three of the names are bare digits, which is why they are renamed rather
/// than spelled as Rust identifiers — `90` is not one. The spelling here is
/// the ONE spelling: the same word goes over the wire, into a config file and
/// into a command. A vocabulary with two spellings for one thing is a trap
/// that springs quietly: the one they agree on is always the quarter turn,
/// which is exactly enough coincidence to survive a quick test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub enum Transform {
    /// Not turned.
    #[default]
    #[serde(rename = "normal")]
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
    #[serde(rename = "flipped")]
    Flipped,
    /// Mirrored, then a quarter turn.
    #[serde(rename = "flipped-90")]
    Flipped90,
    /// Flipped vertically — the same thing as mirrored and then turned upside
    /// down.
    #[serde(rename = "flipped-180")]
    Flipped180,
    /// Mirrored, then three quarters.
    #[serde(rename = "flipped-270")]
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

/// A change one may ask of an output.
///
/// It exists so that no other crate writes the compositor's commands. Each
/// variant maps to exactly one of them, and the adapter is the only place
/// that spelling is written down.
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
    /// Set the mode, or `None` to let the compositor pick.
    Mode(Option<ModeAsked>),
    /// Set the scale, or `None` to let the compositor pick.
    Scale(Option<f64>),
    /// Turn the picture.
    Transform(Transform),
    /// Place the output, or `None` to let the compositor place it.
    Position(Option<(i32, i32)>),
    /// Vary the refresh rate, or hold it steady.
    ///
    /// A plain toggle, and it is readable: an [`Output`] carries
    /// [`Output::vrr_enabled`], which is the state itself rather than a
    /// summary of it. A control whose value can be read back needs nothing
    /// remembered beside it.
    Vary(bool),
}

/// A mode being asked for, which is not shaped like a mode being read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModeAsked {
    /// Width in physical pixels.
    pub width: u16,
    /// Height in physical pixels.
    pub height: u16,
    /// The rate in **hertz**, as a float — while [`Mode::refresh_rate`] is an
    /// integer of millihertz. `None` lets the compositor choose among the
    /// rates that size offers.
    pub refresh: Option<f64>,
}

/// A compositor event, reduced to what the daemon consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// The complete window state. It is always the first to arrive.
    Windows(Vec<Window>),
    /// A window opened or had some property changed.
    WindowChanged(Window),
    /// A window closed.
    WindowClosed(u64),
    /// Focus changed. `None` when nothing is focused — for example, when a
    /// layer-shell surface (ours, with the leader active) takes it.
    FocusChanged(Option<u64>),
    /// Something about the outputs changed.
    ///
    /// Hotplug, a mode set, a scale, a reload — the compositor says only THAT
    /// something moved, never what, so the answer is to read the outputs
    /// again. It is the one signal that catches a change NOBODY HERE MADE: a
    /// command typed in a terminal, a lid, a cable.
    ///
    /// There is deliberately no event for "the configuration was read". A
    /// re-read is something we ASK for, and the refusal comes back on the
    /// same socket, to the caller that asked — see [`Compositor::reread`].
    /// An answer that arrives at the point of the gesture is worth more than
    /// one that arrives later on another thread.
    ScreensChanged,
    /// A device was plugged in, taken out, or had a setting changed.
    ///
    /// The same shape as [`Self::ScreensChanged`]: THAT something moved,
    /// never what, so the answer is to read [`Compositor::devices`] again.
    DevicesChanged,
    /// The compositor is going away, and said so before the socket died.
    ///
    /// It is the difference between a clean end and a stream that simply
    /// stops: with it, whoever is holding a preview can put it back.
    Leaving,
    /// Any other event. See the comment at the top of the module.
    Ignored,
}

// ---------------------------------------------------------------------
// THE DEEDS — what the compositor can be told to do.
// ---------------------------------------------------------------------
//
// A deed is an id of ours, a text key, and whether it takes an argument. What
// it SAYS to the compositor is not here: that is a spelling, it belongs to the
// adapter, and it is generated from `assets/compositor-deeds.nix`.

/// What kind of argument a deed takes, when it takes one.
///
/// It is not a `bool` because the minibuffer prompts with it — "number" and
/// "word" are different questions — and because it is what the adapter
/// VALIDATES against. A compositor whose commands are free text has an
/// injection surface that a protocol of enumerated actions does not, and the
/// answer is to refuse an argument that is not the shape it claims rather than
/// to interpolate whatever was typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    /// A number: a workspace, a fraction, a step.
    Number,
    /// A single bare word: a mark, a name.
    Word,
}

/// Something the compositor can be told to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deed {
    /// Our id — `wm-focus-left`. It is what a key binding and the command
    /// registry name.
    pub id: &'static str,
    /// The text key — `cmd-wm-focus-left`.
    pub key: &'static str,
    /// What it needs typed after it, if anything.
    pub argument: Option<Arity>,
}

// ---------------------------------------------------------------------
// THE CONTRACT ITSELF.
// ---------------------------------------------------------------------

/// A compositor, as everything above the boundary sees one.
///
/// Every method may fail, and failing is ORDINARY: there may be no compositor
/// at all. The footer runs in the installer and in a test with nothing
/// underneath it, and the shell degrades rather than dying — see [`Absent`].
pub trait Compositor: Send {
    /// What it calls itself. The contract test prints it, so a failure names
    /// which build it was talking to.
    fn version(&mut self) -> Result<String>;

    /// Every window it is showing.
    fn windows(&mut self) -> Result<Vec<Window>>;

    /// The focused window, and `None` is a legitimate answer — a layer-shell
    /// surface of ours can hold the keyboard.
    fn focused_window(&mut self) -> Result<Option<Window>>;

    /// Every monitor it can see, including the ones it is not driving, keyed
    /// by connector.
    fn outputs(&mut self) -> Result<BTreeMap<String, Output>>;

    /// The monitor the keyboard is on. `None` between one leaving and the next
    /// taking focus.
    fn focused_output(&mut self) -> Result<Option<Output>>;

    /// Change one thing about one monitor, right now and not for ever.
    ///
    /// It is TRANSIENT by contract: a re-read of the configuration drops it.
    /// That is what makes a preview a preview, and it is why [`Self::reread`]
    /// is the whole of undo.
    fn ask(&mut self, at: &str, ask: Ask) -> Result<()>;

    /// Do a deed, with the argument it asked for.
    ///
    /// The adapter refuses an argument that does not match [`Deed::argument`],
    /// and refuses one that is not the shape [`Arity`] names.
    fn deed(&mut self, deed: &Deed, argument: Option<&str>) -> Result<()>;

    /// Re-read the configuration.
    ///
    /// It drops every [`Self::ask`] made since the last read, which is the
    /// whole of undo.
    ///
    /// # What an `Ok` here does NOT say
    ///
    /// That anything parsed. The contract asks for the request to be carried,
    /// and the only implementation there is answers `success` whatever the
    /// files contained — measured, and written out in `ronoctua-scroll`'s own
    /// `reread`. A contract that promised the refusal would be promising on
    /// behalf of an adapter that cannot deliver it, and the caller would show
    /// a green sentence over a configuration that did not load.
    ///
    /// What proves the grammar is a build-time gate; what proves a change
    /// happened is the reading that follows.
    fn reread(&mut self) -> Result<()>;

    /// Every input device it can see.
    fn devices(&mut self) -> Result<Vec<Device>>;

    /// Every seat, with the devices attached to each.
    fn seats(&mut self) -> Result<Vec<Seat>>;

    /// Change one setting of one device, right now.
    ///
    /// Transient like [`Self::ask`]: a re-read puts back what the files say.
    fn ask_input(&mut self, device: &str, setting: Setting) -> Result<()>;

    /// Change something about a seat, creating it if it does not exist.
    fn ask_seat(&mut self, seat: &str, ask: SeatAsk) -> Result<()>;

    /// Whether focus follows the pointer — ONE setting for every seat.
    fn ask_focus(&mut self, follows: FocusFollows) -> Result<()>;

    /// Close the focused window.
    ///
    /// "The focused one" and not an id: between reading the id and sending the
    /// command the focus can move, and closing the window somebody just
    /// switched to is the worst possible way to be one event late.
    fn close_window(&mut self) -> Result<()>;

    /// Focus a window by id.
    fn focus_window(&mut self, id: u64) -> Result<()>;

    /// End the session.
    fn leave(&mut self) -> Result<()>;

    /// Turn this connection into an event stream. It consumes the connection,
    /// because a socket that is being read cannot also be asked.
    fn watch(self: Box<Self>) -> Result<Box<dyn Events>>;
}

/// A stream of what the compositor says happened.
pub trait Events: Send {
    /// The next one, waiting for as long as it takes.
    fn next_one(&mut self) -> Result<Option<Event>>;
    /// The next one, or `None` when the patience runs out.
    fn next_within(&mut self, patience: Duration) -> Result<Option<Event>>;
}

/// How a binary opens a connection.
///
/// A plain function pointer, handed down from whichever `main` chose an
/// adapter. Not a global, because two processes in this tree want different
/// subscriptions; not a generic, because the connection is opened per
/// operation and `dyn` costs one indirect call per keystroke.
pub type Opener = fn() -> Result<Box<dyn Compositor>>;

// ---------------------------------------------------------------------
// NO COMPOSITOR AT ALL — a state, not an error repeated in six places.
// ---------------------------------------------------------------------

/// The compositor that is not there.
///
/// The footer runs without one: in the installer, in a unit test, in a VM
/// that is only exercising the renderer. Before this, that fact was five
/// separate branches — a `bool` on the snapshot, a `false` from the listener,
/// an empty `Vec` from the pane list, a tuple of defaults from a `look`, and a
/// liveness probe. Now it is one type, and what it does is refuse in a way
/// that reads the same everywhere.
#[derive(Debug, Default, Clone, Copy)]
pub struct Absent;

impl Absent {
    /// The refusal, worded once.
    fn nothing_there<T>() -> Result<T> {
        anyhow::bail!("there is no compositor on this session")
    }
}

impl Compositor for Absent {
    fn version(&mut self) -> Result<String> {
        Self::nothing_there()
    }
    fn windows(&mut self) -> Result<Vec<Window>> {
        Ok(Vec::new())
    }
    fn focused_window(&mut self) -> Result<Option<Window>> {
        Ok(None)
    }
    fn outputs(&mut self) -> Result<BTreeMap<String, Output>> {
        Ok(BTreeMap::new())
    }
    fn focused_output(&mut self) -> Result<Option<Output>> {
        Ok(None)
    }
    fn ask(&mut self, _at: &str, _ask: Ask) -> Result<()> {
        Self::nothing_there()
    }
    fn deed(&mut self, _deed: &Deed, _argument: Option<&str>) -> Result<()> {
        Self::nothing_there()
    }
    fn reread(&mut self) -> Result<()> {
        Self::nothing_there()
    }
    fn devices(&mut self) -> Result<Vec<Device>> {
        Ok(Vec::new())
    }
    fn seats(&mut self) -> Result<Vec<Seat>> {
        Ok(Vec::new())
    }
    fn ask_input(&mut self, _device: &str, _setting: Setting) -> Result<()> {
        Self::nothing_there()
    }
    fn ask_seat(&mut self, _seat: &str, _ask: SeatAsk) -> Result<()> {
        Self::nothing_there()
    }
    fn ask_focus(&mut self, _follows: FocusFollows) -> Result<()> {
        Self::nothing_there()
    }
    fn close_window(&mut self) -> Result<()> {
        Self::nothing_there()
    }
    fn focus_window(&mut self, _id: u64) -> Result<()> {
        Self::nothing_there()
    }
    fn leave(&mut self) -> Result<()> {
        Self::nothing_there()
    }
    fn watch(self: Box<Self>) -> Result<Box<dyn Events>> {
        Self::nothing_there()
    }
}

// ---------------------------------------------------------------------
// A COMPOSITOR MADE OF A LIST — so that `cargo test` can drive one.
// ---------------------------------------------------------------------

/// A compositor that answers from memory and writes down what it was told.
///
/// It exists because the interesting code was untestable: the three-way merge
/// between what a door shows, what the disk says and what a person just chose
/// is where two settings-losing defects were found by READING, because
/// nothing could drive it without a live session and a monitor. With this, a
/// unit test arranges the outputs, performs the gesture and reads back the
/// exact sequence of commands that went out.
///
/// It is not a mock in the sense of asserting on calls: it is a compositor
/// with a very small memory, and what it remembers is [`Self::told`].
#[derive(Debug, Default)]
pub struct Scripted {
    /// What [`Compositor::outputs`] will answer.
    pub outputs: BTreeMap<String, Output>,
    /// What [`Compositor::windows`] will answer.
    pub windows: Vec<Window>,
    /// The connector [`Compositor::focused_output`] will name.
    pub focused: Option<String>,
    /// What [`Compositor::devices`] will answer.
    pub devices: Vec<Device>,
    /// What [`Compositor::seats`] will answer.
    pub seats: Vec<Seat>,
    /// Everything it has been told to do, in order.
    pub told: Vec<Said>,
    /// Events [`Compositor::watch`] will hand out, oldest first.
    pub coming: Vec<Event>,
    /// When set, every fallible method refuses with this, so a test can
    /// exercise the path where the compositor is there and says no.
    pub refusing: Option<String>,
}

/// One thing a [`Scripted`] was told.
#[derive(Debug, Clone, PartialEq)]
pub enum Said {
    /// An [`Ask`] about a monitor.
    Ask(String, Ask),
    /// A [`Deed`], by id, with whatever was typed after it.
    Deed(&'static str, Option<String>),
    /// A re-read, which is also the whole of undo.
    Reread,
    /// A setting asked of a device.
    Input(String, Setting),
    /// Something asked of a seat.
    Seat(String, SeatAsk),
    /// Focus-follows-mouse set.
    Focus(FocusFollows),
    /// The focused window was closed.
    Closed,
    /// A window was focused by id.
    Focused(u64),
    /// The session was ended.
    Left,
}

impl Scripted {
    /// One that answers with these monitors and nothing else.
    #[must_use]
    pub fn with(outputs: BTreeMap<String, Output>) -> Self {
        Self {
            outputs,
            ..Self::default()
        }
    }

    fn allowed(&self) -> Result<()> {
        match &self.refusing {
            Some(why) => anyhow::bail!("{why}"),
            None => Ok(()),
        }
    }
}

impl Compositor for Scripted {
    fn version(&mut self) -> Result<String> {
        self.allowed()?;
        Ok("scripted".into())
    }
    fn windows(&mut self) -> Result<Vec<Window>> {
        self.allowed()?;
        Ok(self.windows.clone())
    }
    fn focused_window(&mut self) -> Result<Option<Window>> {
        self.allowed()?;
        Ok(self.windows.iter().find(|w| w.is_focused).cloned())
    }
    fn outputs(&mut self) -> Result<BTreeMap<String, Output>> {
        self.allowed()?;
        Ok(self.outputs.clone())
    }
    fn focused_output(&mut self) -> Result<Option<Output>> {
        self.allowed()?;
        Ok(self
            .focused
            .as_ref()
            .and_then(|at| self.outputs.get(at))
            .cloned())
    }
    fn ask(&mut self, at: &str, ask: Ask) -> Result<()> {
        self.allowed()?;
        self.told.push(Said::Ask(at.to_owned(), ask));
        Ok(())
    }
    fn deed(&mut self, deed: &Deed, argument: Option<&str>) -> Result<()> {
        self.allowed()?;
        self.told
            .push(Said::Deed(deed.id, argument.map(ToOwned::to_owned)));
        Ok(())
    }
    fn reread(&mut self) -> Result<()> {
        self.allowed()?;
        self.told.push(Said::Reread);
        Ok(())
    }
    fn devices(&mut self) -> Result<Vec<Device>> {
        self.allowed()?;
        Ok(self.devices.clone())
    }
    fn seats(&mut self) -> Result<Vec<Seat>> {
        self.allowed()?;
        Ok(self.seats.clone())
    }
    fn ask_input(&mut self, device: &str, setting: Setting) -> Result<()> {
        self.allowed()?;
        self.told.push(Said::Input(device.to_owned(), setting));
        Ok(())
    }
    fn ask_seat(&mut self, seat: &str, ask: SeatAsk) -> Result<()> {
        self.allowed()?;
        // A SEAT ASKED FOR IS A SEAT THAT EXISTS, and an attach MOVES the
        // device — the compositor's own rules, kept here so a test that
        // attaches and then reads the seats sees what a live one would.
        if let SeatAsk::Attach(id) = &ask {
            for other in &mut self.seats {
                other.devices.retain(|d| d != id);
            }
            match self.seats.iter_mut().find(|x| x.name == seat) {
                Some(here) => here.devices.push(id.clone()),
                None => self.seats.push(Seat {
                    name: seat.to_owned(),
                    devices: vec![id.clone()],
                    focus: None,
                }),
            }
        }
        self.told.push(Said::Seat(seat.to_owned(), ask));
        Ok(())
    }
    fn ask_focus(&mut self, follows: FocusFollows) -> Result<()> {
        self.allowed()?;
        self.told.push(Said::Focus(follows));
        Ok(())
    }
    fn close_window(&mut self) -> Result<()> {
        self.allowed()?;
        self.told.push(Said::Closed);
        Ok(())
    }
    fn focus_window(&mut self, id: u64) -> Result<()> {
        self.allowed()?;
        self.told.push(Said::Focused(id));
        Ok(())
    }
    fn leave(&mut self) -> Result<()> {
        self.allowed()?;
        self.told.push(Said::Left);
        Ok(())
    }
    fn watch(self: Box<Self>) -> Result<Box<dyn Events>> {
        self.allowed()?;
        Ok(Box::new(Rehearsed {
            coming: self.coming.clone().into(),
        }))
    }
}

/// The event stream of a [`Scripted`].
#[derive(Debug, Default)]
struct Rehearsed {
    coming: std::collections::VecDeque<Event>,
}

impl Events for Rehearsed {
    fn next_one(&mut self) -> Result<Option<Event>> {
        Ok(self.coming.pop_front())
    }
    fn next_within(&mut self, _patience: Duration) -> Result<Option<Event>> {
        Ok(self.coming.pop_front())
    }
}
/// What the windows are, kept up to date by the events.
///
/// It exists so that the modeline does not ask: a segment redrawn on every
/// frame cannot afford a round trip, and the events already say everything it
/// needs. [`State::apply_to`] is the only way in.
#[derive(Debug, Default)]
pub struct State {
    windows: Vec<Window>,
    focus: Option<u64>,
}

impl State {
    /// Applies an event. It is the only way to change the state.
    pub fn apply_to(&mut self, event: &Event) {
        match event {
            Event::Windows(js) => {
                // The focus is INSIDE the window, and a focus event may not
                // come with it. Reading from there keeps the two coherent.
                self.focus = js.iter().find(|j| j.is_focused).map(|j| j.id);
                self.windows = js.clone();
            }
            Event::WindowChanged(j) => {
                if let Some(existing) = self.windows.iter_mut().find(|x| x.id == j.id) {
                    *existing = j.clone();
                } else {
                    self.windows.push(j.clone());
                }
                // A window that says it is focused means every other one is
                // not. Without this, a window that opens ALREADY focused only
                // shows up in the modeline if a separate focus event follows
                // — and depending on that is depending on an order nobody
                // promised.
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
            // A MONITOR MOVING SAYS NOTHING ABOUT THE WINDOWS, and neither
            // does the session ending: both are listened to elsewhere, and
            // here they have to be as inert as an unknown event.
            Event::ScreensChanged | Event::DevicesChanged | Event::Leaving | Event::Ignored => {}
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

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(make: &str, model: &str, serial: Option<&str>) -> Output {
        Output {
            name: "eDP-1".to_owned(),
            make: make.to_owned(),
            model: model.to_owned(),
            serial: serial.map(ToOwned::to_owned),
            modes: vec![Mode {
                width: 1920,
                height: 1200,
                refresh_rate: 59_951,
            }],
            current_mode: Some(0),
            is_custom_mode: false,
            vrr_supported: false,
            vrr_enabled: false,
            logical: Some(Logical {
                x: 0,
                y: 0,
                width: 1920,
                height: 1200,
                scale: 1.0,
                transform: Transform::Normal,
            }),
        }
    }

    fn pane(id: u64, focused: bool) -> Window {
        Window {
            id,
            title: Some(format!("window {id}")),
            app_id: Some("br.ronoctua.test".to_owned()),
            pid: Some(1000 + i32::try_from(id).unwrap()),
            is_focused: focused,
        }
    }

    #[test]
    fn a_monitor_is_named_by_what_it_says_about_itself() {
        // The name a config block must carry. It survives the cable moving
        // to another port, which the connector does not.
        let o = monitor("Maker", "Model-1", None);
        assert_eq!(o.identity(), "Maker Model-1 Unknown");
        // And it is NOT what its settings are filed under: no serial.
        assert!(!o.naming().follows_the_monitor());
    }

    #[test]
    fn a_monitor_with_no_serial_is_filed_under_its_port() {
        // A make and a model are shared by every unit of a product, so a
        // monitor that gives no serial is one an identical second could
        // answer for — and a description is applied to EVERY output that
        // answers to it, which would give the pair one position.
        //
        // THE RULE READS ONE MONITOR'S OWN EDID, and never "is there a twin
        // plugged in right now": that answer changes when a second one
        // arrives, and the settings filed under the old name would silently
        // stop being found and come back when it was unplugged.
        let o = monitor("Maker Inc.", "Model-3", None);
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
        let named = monitor("Maker Inc.", "Model-3", Some("ABC123"));
        assert_eq!(
            named.naming(),
            Naming::Description("Maker Inc. Model-3 ABC123".to_owned())
        );
        assert!(named.naming().follows_the_monitor());
    }

    #[test]
    fn a_monitor_that_says_nothing_is_named_by_its_port() {
        // Three `Unknown`s would match every anonymous monitor at once, so
        // the connector is the only honest key — and the reason is recorded
        // on the answer, because the screen says which of the two it is.
        let o = monitor("Unknown", "Unknown", None);
        assert_eq!(
            o.naming(),
            Naming::Connector {
                name: "eDP-1".to_owned(),
                why: Anonymous::Silent
            }
        );
        // The description is still all `Unknown` — that is what the monitor
        // said about itself, and a screen showing it is telling the truth.
        assert_eq!(o.identity(), "Unknown Unknown Unknown");
    }

    #[test]
    fn a_quarter_turn_swaps_the_sides_and_a_half_turn_does_not() {
        for t in [
            Transform::R90,
            Transform::R270,
            Transform::Flipped90,
            Transform::Flipped270,
        ] {
            assert!(t.turns_the_side(), "{t:?} turns the side");
        }
        for t in [
            Transform::Normal,
            Transform::R180,
            Transform::Flipped,
            Transform::Flipped180,
        ] {
            assert!(!t.turns_the_side(), "{t:?} does not");
        }
    }

    #[test]
    fn a_transform_has_one_spelling() {
        // The wire, a config file and a command all take the same word. Two
        // lists of names that agree only on the quarter turn is the trap this
        // asserts away — the coincidence is what makes it survive testing.
        for (t, word) in [
            (Transform::Normal, "normal"),
            (Transform::R90, "90"),
            (Transform::R180, "180"),
            (Transform::R270, "270"),
            (Transform::Flipped, "flipped"),
            (Transform::Flipped90, "flipped-90"),
            (Transform::Flipped180, "flipped-180"),
            (Transform::Flipped270, "flipped-270"),
        ] {
            let said = serde_json::to_string(&t).unwrap();
            assert_eq!(said, format!("\"{word}\""), "{t:?} is written {word}");
            let back: Transform = serde_json::from_str(&said).unwrap();
            assert_eq!(back, t, "and reads back the same");
        }
        // And all eight are in `ALL`, so a ring cannot silently drop one.
        assert_eq!(Transform::ALL.len(), 8);
    }

    #[test]
    fn the_rounded_rate_is_for_reading_and_the_exact_one_is_for_asking() {
        // A panel sold as 60 Hz reports 59 951 mHz. Rounding it for the
        // screen is right; rounding it for a REQUEST asks for a rate the
        // monitor does not have, and lands on a neighbouring mode.
        let m = Mode {
            width: 1920,
            height: 1200,
            refresh_rate: 59_951,
        };
        assert!((m.hertz() - 60.0).abs() < f64::EPSILON, "reads as 60,0");
        assert!((m.rate() - 59.951).abs() < f64::EPSILON, "asks as 59,951");
        assert_eq!(m.asked().refresh, Some(59.951));
        assert_eq!(m.asked().width, 1920);
    }

    #[test]
    fn the_focus_comes_from_windows_changed() {
        // The focus is inside the window, and a focus event may not come
        // with it.
        let mut s = State::default();
        s.apply_to(&Event::Windows(vec![pane(1, false), pane(2, true)]));
        assert_eq!(s.focused().map(|w| w.id), Some(2));
        assert_eq!(s.amount(), 2);
    }

    #[test]
    fn closing_the_focused_window_zeroes_the_focus() {
        let mut s = State::default();
        s.apply_to(&Event::Windows(vec![pane(1, true)]));
        s.apply_to(&Event::WindowClosed(1));
        assert!(s.focused().is_none());
        assert_eq!(s.amount(), 0);
    }

    #[test]
    fn a_window_that_opens_already_focused_gains_the_focus() {
        // A window that says it is focused means every other one is not —
        // without that, this only works if a separate focus event follows.
        let mut s = State::default();
        s.apply_to(&Event::Windows(vec![pane(1, true)]));
        s.apply_to(&Event::WindowChanged(pane(2, true)));
        assert_eq!(s.focused().map(|w| w.id), Some(2));
        assert_eq!(s.amount(), 2);
        assert!(
            s.all_windows().iter().filter(|w| w.is_focused).count() == 1,
            "exactly one window is focused"
        );
    }

    #[test]
    fn opening_the_same_window_twice_does_not_duplicate() {
        let mut s = State::default();
        s.apply_to(&Event::WindowChanged(pane(1, false)));
        s.apply_to(&Event::WindowChanged(pane(1, true)));
        assert_eq!(s.amount(), 1);
        assert_eq!(s.focused().map(|w| w.id), Some(1));
    }

    #[test]
    fn an_absent_compositor_refuses_without_pretending() {
        // Absence answers EMPTY where a list is wanted — the modeline draws
        // nothing rather than a lie — and REFUSES where an action is wanted,
        // because silently doing nothing is the failure this type exists to
        // stop being spread over six branches.
        let mut none = Absent;
        assert!(none.windows().unwrap().is_empty());
        assert!(none.outputs().unwrap().is_empty());
        assert!(none.focused_window().unwrap().is_none());
        assert!(none.reread().is_err());
        assert!(none.close_window().is_err());
        assert!(none.ask("eDP-1", Ask::On).is_err());
    }

    #[test]
    fn a_scripted_compositor_writes_down_what_it_was_told() {
        // This is what makes the display view testable without a monitor.
        let mut c = Scripted::with(BTreeMap::from([(
            "eDP-1".to_owned(),
            monitor("M", "1", None),
        )]));
        c.ask("eDP-1", Ask::Scale(Some(1.5))).unwrap();
        c.ask("eDP-1", Ask::Vary(true)).unwrap();
        c.reread().unwrap();
        assert_eq!(
            c.told,
            vec![
                Said::Ask("eDP-1".to_owned(), Ask::Scale(Some(1.5))),
                Said::Ask("eDP-1".to_owned(), Ask::Vary(true)),
                Said::Reread,
            ]
        );
    }

    #[test]
    fn a_compositor_that_says_no_is_not_a_compositor_that_is_gone() {
        // The two are different states and the code above the boundary has
        // to be able to tell them apart: one draws a refusal with a reason,
        // the other draws a modeline with no window in it.
        let mut saying_no = Scripted {
            refusing: Some("the output is not connected".to_owned()),
            ..Scripted::default()
        };
        let refused = saying_no.reread().unwrap_err().to_string();
        assert_eq!(refused, "the output is not connected");
        assert!(saying_no.windows().is_err(), "it refuses readings too");
    }
}
