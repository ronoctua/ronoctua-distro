//! The Bluetooth view — one implementation, two doors.
//!
//! # What it holds, and what it does not
//!
//! A cursor, which level is open, and the question the other side is asking.
//! It holds no reading: the [`ronoctua_bluetooth::Scene`] lives in the footer
//! (or the window) and is handed in when rows are asked for — the shape
//! `power::View` and `notifications::View` already have, and what lets both
//! doors draw the same thing without a second copy going stale.
//!
//! # Three levels, and `Esc` walks back one
//!
//! The LIST is the radio's row, the paired devices and the ones nearby. `RET`
//! on a device opens its DETAIL — everything BlueZ knows about it, one fact
//! per line, and one row per thing it can do. `RET` on the radio's own row
//! opens the RADIO. A question from the other side takes the screen until it
//! is answered, because nothing pairs until it is.
//!
//! # A device is drawn as what it IS
//!
//! The glyph is the kind's, and the kind is the device's own class or
//! appearance — never its name. That is the same rule the modeline follows,
//! and it is the same function ([`ronoctua_bluetooth::Kind::of`]).

use ronoctua_bluetooth::{Ask, Device, Kind, Scene, agent};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::bluez_words::{AdapterKey, adapter_order, glyph};
use crate::fields::{Field, Hit as FilesHit, Ink};

/// WHERE a key means something.
///
/// The hint line is generated from the table, and the table used to be flat:
/// every screen taught all fifteen keys. On the radio's own screen `about()`
/// can never find a device, so `c p f t b r` were six taught keys that
/// returned `Nothing`, and `r rename` — the one a reader would most
/// reasonably expect to rename the machine — did not. The owner photographed
/// exactly that screen.
///
/// The file's own rule, already written over `OF_THE_SHEET` in the
/// notification manager: one key, one meaning per level, and the line says
/// which level it is on by teaching only that level's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Every screen: the arrows, and `RET`.
    Anywhere,
    /// The list of devices, and a device's own screen — both act on a device.
    OnADevice,
    /// The list and the radio's screen: these are about the radio.
    OnTheRadio,
    /// Only the list: it is the only screen with rows one can hide.
    OnTheList,
    /// Only while the other side is asking.
    WhileAsked,
    /// Only where the device has something to ADJUST.
    ///
    /// The same reasoning as `OnACard`, one step wider: a headset has a
    /// volume and a profile, a keyboard has whether it may wake the machine,
    /// and a thermometer has nothing. Teaching `TAB` on a page with one
    /// column is teaching a key that goes nowhere.
    OnTheSettings,
}

/// The keys this view answers to, and the hint line is generated from it —
/// see `chrome::hint_of`. Bare letters are free here (nothing is typed), and
/// the order is the order they are taught in.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "bluetooth-down", Where::Anywhere),
    ("<up>", "bluetooth-up", Where::Anywhere),
    ("RET", "bluetooth-device", Where::Anywhere),
    ("c", "bluetooth-connect", Where::OnADevice),
    ("p", "bluetooth-pair", Where::OnADevice),
    ("d", "bluetooth-forget", Where::OnADevice),
    ("t", "bluetooth-trust", Where::OnADevice),
    ("b", "bluetooth-block", Where::OnADevice),
    ("e", "bluetooth-rename", Where::OnADevice),
    ("s", "bluetooth-scan", Where::OnTheRadio),
    ("o", "bluetooth-power", Where::OnTheRadio),
    ("v", "bluetooth-discoverable", Where::OnTheRadio),
    ("h", "bluetooth-unnamed", Where::OnTheList),
    // THE SIDEWAYS ARROWS CROSS THE COLUMNS, and `TAB` still does too — the
    // file manager's rule, asked for here in the same words. Four notations
    // and one id: `hint_of` spends an id once, so the line teaches the arrow
    // and the other three go on working. With two halves there is nothing a
    // DIRECTION could mean, which is why both arrows are the same key.
    ("<right>", "bluetooth-settings", Where::OnTheSettings),
    ("<left>", "bluetooth-settings", Where::OnTheSettings),
    ("TAB", "bluetooth-settings", Where::OnTheSettings),
    ("S-TAB", "bluetooth-settings", Where::OnTheSettings),
    // AND WHAT THE BARE ARROWS USED TO DO moved one modifier up, which is
    // where the manager keeps the meaning that belongs to the focused thing.
    ("S-<right>", "bluetooth-more", Where::OnTheSettings),
    ("S-<left>", "bluetooth-less", Where::OnTheSettings),
    ("y", "bluetooth-accept", Where::WhileAsked),
    ("n", "bluetooth-refuse", Where::WhileAsked),
];

impl Where {
    /// Does a key of this kind mean anything on that screen?
    const fn shown(self, level: &Level, asking: bool, settings: bool) -> bool {
        match self {
            Self::Anywhere => true,
            Self::WhileAsked => asking,
            // A QUESTION TAKES THE SCREEN: while one is up the only two keys
            // that mean anything are its answers, and teaching a device's
            // six beside them is teaching six keys that do nothing.
            _ if asking => false,
            Self::OnADevice => matches!(level, Level::List | Level::Device(_)),
            // THE CARD DECIDES, not the screen: a keyboard's own page is a
            // device page with nothing to move between.
            Self::OnTheSettings => settings && matches!(level, Level::Device(_)),
            Self::OnTheRadio => matches!(level, Level::List | Level::Radio),
            Self::OnTheList => matches!(level, Level::List),
        }
    }
}

/// The width the tests build rows at.
///
/// # Why production no longer has one
///
/// A key acts on the row under the cursor, so it has to know the rows — and it
/// used to build them at a fixed eighty columns, on the argument that the row
/// COUNT and the row TARGETS do not depend on the width. That argument held
/// while every note was CUT: an ellipsis changes what a row SAYS and never how
/// many there are.
///
/// A note WRAPS now (`chrome::note_lines`), so one sentence is a row on a wide
/// screen and three on a narrow one, and every row under it moves. A key
/// deciding at eighty would act on a different row from the one the door drew
/// — a control taken somewhere other than where it was drawn, arriving on the
/// keyboard's axis. So the DOOR's width travels in, as it already did for the
/// Bluetooth view, and what is left here is a width the tests agree on.
#[cfg(test)]
const DECIDING_WIDTH: u16 = 80;

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row, skipping headings.
    Up,
    /// Down one row, skipping headings.
    Down,
    /// Open the row, or answer it.
    Enter,
    /// One level back.
    Escape,
    /// Connect, or drop the link.
    Connect,
    /// Pair.
    Pair,
    /// Forget.
    Forget,
    /// Trust, or stop.
    Trust,
    /// Block, or stop.
    Block,
    /// Rename.
    Rename,
    /// Look for devices, or stop.
    Scan,
    /// Turn the radio on or off.
    Power,
    /// MORE of whatever the cursor is on, and less.
    ///
    /// `S-<right>` and `S-<left>`, which is the file manager's rule: the
    /// bare arrows move between the columns and the SHIFTED pair acts on
    /// what the focused column has. They belong to the settings half — a
    /// volume goes up in steps and an audio profile has an order (off, music
    /// in stereo, a call with the microphone) — and `RET` would need
    /// somewhere to put a list. They were the bare arrows until the owner
    /// asked for those to change column.
    More,
    /// The other way round `More` goes.
    Less,
    /// Cross to the other half of a device's page, and back.
    ///
    /// `TAB` and the bare sideways arrows: with two halves there is nothing
    /// for a DIRECTION to mean, so both arrows are this one key.
    ///
    /// Side by side when the width affords it, stacked when it does not —
    /// and the key means the same thing either way, which is why it is not
    /// gated on the width. Stacked, it JUMPS: walking twelve facts with the
    /// arrow to reach the volume is not crossing, it is scrolling.
    Cross,
    /// Let others see this machine, or stop.
    Discoverable,
    /// Show the devices with no name, or hide them again.
    Unnamed,
    /// Yes.
    Accept,
    /// No.
    Refuse,
}

/// Which screen is open.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Level {
    /// The radio, the paired devices and the ones nearby.
    #[default]
    List,
    /// One device, whole.
    Device(String),
    /// The radio's own settings.
    Radio,
}

/// What `RET` on a row does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Nothing: a heading, a fact, a note.
    Nothing,
    /// Open this device.
    Device(String),
    /// Open the radio.
    Radio,
    /// Turn the radio on or off.
    Power(bool),
    /// Let others see this machine, or stop.
    Discoverable(bool),
    /// Let others pair, or stop.
    Pairable(bool),
    /// Look for devices, or stop.
    Scan(bool),
    /// Connect or drop ONE service of the open device.
    Profile {
        /// The device.
        device: String,
        /// The service id.
        uuid: String,
        /// True connects.
        on: bool,
    },
    /// Answer the question the other side is asking.
    Answer(bool),
    /// Call this MACHINE something else — the alias other devices see.
    Rename,
    /// How long this machine stays visible, in seconds.
    Timeout(u32),
    /// How long this machine stays open to pairing, in seconds.
    ///
    /// The same ring the visibility one has, and it had neither a row nor an
    /// order: two settings of the same shape, one of them on screen.
    PairableTimeout(u32),
    /// Put this device's card in another audio profile.
    ///
    /// PipeWire's device id and the profile's index, both already resolved:
    /// the view holds the CARD it read, so the row does not have to go
    /// looking when a key is pressed.
    Audio(u32, i32),
    /// Open the keyboards, which is where a key of this one is changed.
    ///
    /// A DOOR AND NOT A SETTING. What a key becomes is a screen of its own —
    /// seven keys and nine meanings — and the settings column is a handful of
    /// rows beside the facts. The row says where to go; the buffer is where
    /// one sits down.
    ///
    /// The string is the device's `Modalias`, which carries the same
    /// `vendor:product` keyd matches on — so the buffer opens with THIS
    /// keyboard under the cursor. Empty opens the list, which is the other
    /// question: what every keyboard on this machine does.
    Keyboards(String),
    /// Set this sink's volume, as a percentage.
    ///
    /// PipeWire's NODE id, already resolved — and never the card's: the two
    /// are different objects and `wpctl` takes a different one for each.
    Volume(u32, u8),
    /// Silence this sink, or give it back.
    Mute(u32, bool),
    /// May this device wake the machine from sleep?
    ///
    /// `readwrite` on the bus, and it was drawn as a FACT: on a Bluetooth
    /// keyboard it is the setting that decides whether pressing a key can
    /// bring the machine back at all.
    Wake(String, bool),
    /// Use this phone's internet, or stop — its hardware address, and what
    /// `RET` sets (the PAN; see [`Errand::Tether`]).
    Tether(String, bool),
}

/// How much one press of an arrow moves a volume.
///
/// FIVE and not one: a volume is read as a rough amount, and twenty presses to
/// cross the range is a key nobody uses. It rounds to the step first, so a
/// device that came back from PipeWire at 37 goes to 40 and then 45 rather
/// than 42 — the ladder is the same wherever you got on it.
const VOLUME_STEP: u8 = 5;

/// The volume one arrow away, clamped to the range.
#[must_use]
pub const fn step(now: u8, up: bool) -> u8 {
    let rung = now / VOLUME_STEP * VOLUME_STEP;
    if up {
        let next = rung + VOLUME_STEP;
        if next > 100 { 100 } else { next }
    } else if rung == 0 {
        0
    } else if rung == now {
        rung - VOLUME_STEP
    } else {
        rung
    }
}

/// The next visibility timeout in the ring — three minutes, ten, an hour,
/// for ever.
///
/// A RING AND NOT A NUMBER TO TYPE: the four values are what anybody
/// actually wants, and cycling them is one key rather than a prompt.
#[must_use]
pub const fn next_timeout(now: u32) -> u32 {
    match now {
        0 => 180,
        1..=180 => 600,
        181..=600 => 3600,
        _ => 0,
    }
}

/// One row the view draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The pieces, left to right, already placed in cells.
    pub fields: Vec<Field>,
    /// Is the cursor on it?
    pub focused: bool,
    /// A heading names the section and is not in it: the cursor skips it.
    pub heading: bool,
    /// What `RET` does here.
    pub target: Target,
    /// Drawn dim and REFUSING — a blocked device's connect, a radio that is
    /// off. Reachable, so the row can say why.
    pub disabled: bool,
    /// The whole sentence, for the line the footer explains things on.
    ///
    /// # Why a row carries one at all
    ///
    /// Because a label is two or three words and some of these rows are about
    /// things nobody meets twice a year. `services read` and `the keys were
    /// kept` are true and say nothing to somebody who has not read the
    /// Bluetooth specification — and the owner said so from the screen.
    ///
    /// It arrives by POINTING, which is where this footer already puts a
    /// description (`ExpansionRow::description`, and the leader's own list
    /// before it). Nothing floats: it is the message line, which is invariant
    /// 14 intact.
    pub description: Option<String>,
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Put this on the system clipboard — what `RET` does on an INFORMATIVE
    /// row. See `chrome::information_of` for what "this" is and why.
    Copy(String),
    /// Send this to the bluetooth thread.
    Order(Ask),
    /// Answer the question the other side is asking. It does NOT go through
    /// the orders thread: `Device1.Pair` is blocking that thread while the
    /// question stands, and an answer queued behind it would wait for the
    /// order that is waiting for the answer.
    Answer(bool),
    /// The other side wants digits typed here — a PIN, a passkey. The door
    /// asks on its own line, masked ([`ACCEPT`]): the footer's minibuffer,
    /// or the window's header (`Ui::ask`). The digits reach the agent as
    /// `bluetooth-accept`'s argument, in the footer's process.
    Type,
    /// A name is to be typed for the device at this path — or for this
    /// MACHINE, when the path is empty. The door asks on its own line
    /// ([`RENAME`]), starting from the name it has now
    /// ([`View::name_being_changed`]), and hands the answer back to
    /// [`View::renamed`].
    Rename(String),
    /// Run a command of the registry, by id, with an argument when it takes
    /// one.
    ///
    /// The footer runs it; the window SENDS it down the control socket
    /// (`Ui::tell_the_footer`). A second implementation in the second door is
    /// the second door disagreeing with the first.
    Run(&'static str, String),
    /// Set a sink's volume — PipeWire's NODE id, and the percentage.
    ///
    /// Beside `Audio` and not folded into it for the reason the two `wpctl`
    /// verbs are not the same call: one takes a device and the other a node.
    Volume(u32, u8),
    /// Silence a sink, or give it back.
    Mute(u32, bool),
    /// Use a phone's internet over Bluetooth (PAN), or stop — by its
    /// hardware address, which is the one name BlueZ and NetworkManager
    /// share.
    ///
    /// NOT an `Ask`: the orders thread speaks to BlueZ, and the link is
    /// NETWORKMANAGER's to make. BlueZ alone gives `bnep0` and no address;
    /// NetworkManager asks BlueZ for the link, runs DHCP on it and routes
    /// through it — so both views agree on who owns `bnep0`
    /// (`ronoctua_network::Manager::tether`, and [`tethered`], which both
    /// doors call).
    Tether {
        /// The phone's hardware address.
        address: String,
        /// Its name, for the sentence.
        name: String,
        /// Use it, or stop.
        on: bool,
    },
    /// Put this device's card in a profile — PipeWire's id, and the index.
    ///
    /// NOT an `Ask`: the orders thread speaks to BlueZ, and this is
    /// PipeWire's. It runs `wpctl` in a thread of the DOOR's, the way the
    /// network's `systemctl` orders do.
    Audio(u32, i32),
    /// Close the view.
    Close,
}

/// The view's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    cursor: usize,
    /// WHAT the cursor is on, and not only where.
    ///
    /// # Why an index is not enough here
    ///
    /// This list REORDERS under the cursor. `paired()` sorts by connected,
    /// then kind, then label; `nearby()` by named, then label; and the scene
    /// arrives up to four times a second while a scan runs. So the third row
    /// is the headphones now and the headset a moment later, and `f` on it
    /// forgets a device nobody aimed at — keys gone from the radio's store,
    /// with no confirmation and no way back.
    ///
    /// It is the transfer queue's lesson, in `AGENTS.md`: the cursor is the
    /// ID, which the list reorders. Only rows whose target NAMES something
    /// are anchored — a fact row's target is `Nothing`, and anchoring on that
    /// would pin the cursor to the first fact of any screen.
    at: Option<Target>,
    level: Level,
    /// The question the other side is asking, when it is asking one. It takes
    /// the screen: nothing pairs until it is answered.
    question: Option<agent::Question>,
    /// Are the nearby devices that give no name shown?
    unnamed: bool,
    /// The audio card of the device this view is looking at, when it has one.
    ///
    /// READ WHEN THE DEVICE LEVEL OPENS and not on every frame: `pw-dump`
    /// prints the whole graph, which is a process and a parse, and the
    /// profile of a headset does not change while nobody is pressing a key.
    /// `None` is both "not an audio device" and "PipeWire is not answering",
    /// and the row says which by not being there at all.
    card: Option<ronoctua_bluetooth::audio::Card>,
    /// The sink of the device this view is looking at, when it has one.
    ///
    /// Read beside the card, out of the SAME `pw-dump`: a volume and a
    /// profile are two objects of one graph, and printing it twice would be
    /// two processes for one screen.
    sink: Option<ronoctua_bluetooth::audio::Sink>,
    /// Which half of a device's page the keyboard is in.
    ///
    /// Only meaningful side by side. Stacked, the two halves are one list and
    /// the cursor's own index says which one it is in — so `TAB` moves the
    /// cursor there instead of moving the keyboard.
    pane: Pane,
    /// Has the device on screen anything to adjust?
    ///
    /// Learned when the level changes, beside the card, because the hint line
    /// has to know it and does not have the scene. Without it `TAB` would be
    /// taught on a thermometer's page.
    settings: bool,
    /// WHOSE NAME is being typed, while a door's line asks for it: a device's
    /// path, or empty for this machine.
    ///
    /// The VIEW keeps it, and not either door: the answer comes back as text
    /// alone, and both doors hand it to [`View::renamed`], which is what says
    /// what the text becomes. The next key that reaches the view lets it go —
    /// while a line asks, every key is the line's, in both doors.
    renaming: Option<String>,
}

/// Uses a phone's internet, or stops, through NetworkManager — and the
/// sentence the message line says about it. ONE FUNCTION FOR BOTH DOORS: the
/// footer and the window call it with the same errand and say the same
/// words. A call to the system bus that returns at once (the activation runs
/// in the daemon), so it does not hold the loop.
#[must_use]
pub fn tethered(address: &str, name: &str, on: bool, texts: &ronoctua_i18n::Texts) -> String {
    use crate::key;
    match ronoctua_network::Manager::fresh().and_then(|m| m.tether(address, on)) {
        Ok(()) => texts.t_with(
            if on {
                key::ECHO_BLUETOOTH_INTERNET_ON
            } else {
                key::ECHO_BLUETOOTH_INTERNET_OFF
            },
            &[("device", name)],
        ),
        Err(e) => texts.t_with(
            key::ECHO_BLUETOOTH_INTERNET_FAILED,
            &[
                ("device", name),
                ("reason", &crate::network_refusal::reason(&e, texts)),
            ],
        ),
    }
}

/// The two halves of a device's page.
///
/// Side by side above `space.bluetooth.settingsFrom`, stacked below it — and
/// stacked they are still two halves, because `TAB` crosses between them
/// either way and the cursor knows which one it is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    /// What this thing IS, and whether it is working.
    #[default]
    Facts,
    /// What you can turn, about this thing in particular.
    Settings,
}

/// Where a row's cells begin and how wide they are, in the ROW's cells —
/// both doors add `chrome::FIELDS_LEAD`; see `notifications::extent_of`.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
    let end = columns.saturating_sub(crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT);
    (left, end.saturating_sub(left))
}

/// Does the width afford the settings beside the facts?
///
/// The token is the cut and the arithmetic is here — the same division of
/// labour `notifications::two_columns` makes, and the same answer in the
/// footer's region and in the window.
#[must_use]
pub const fn two_columns(columns: u16) -> bool {
    columns >= tokens::space::BLUETOOTH_SETTINGSFROM
}

/// Where the settings column starts, in cells.
#[must_use]
pub fn split_at(columns: u16) -> u16 {
    columns * tokens::space::BLUETOOTH_SPLITATPCT / 100
}

/// One half's cells, when there are two halves.
///
/// Below the cut both halves answer the FULL width: they are stacked then, and
/// a stacked half that kept the narrow extent would draw its rows in the left
/// sixty per cent of a screen with nothing beside them.
#[must_use]
pub fn extent_of(pane: Pane, columns: u16) -> (u16, u16) {
    let (left, room) = extent(columns);
    if !two_columns(columns) {
        return (left, room);
    }
    let end = left + room;
    let split = split_at(columns);
    match pane {
        Pane::Facts => (
            left,
            split.saturating_sub(left + tokens::space::MODELINE_SEGMENTGAP),
        ),
        Pane::Settings => (split, end.saturating_sub(split)),
    }
}

/// A daemon's word, in the person's language — or the daemon's word.
///
/// # Why the fallback is the raw string and not `[key]`
///
/// Because these are BlueZ's enumerations, and BlueZ can grow a value. A
/// missing key resolves to `[bluetooth-transport-broadcasting]`, which is
/// worse on screen than the English word it stands for: one of them is
/// unreadable and the other is merely untranslated. The `.ftl` carries every
/// value the daemon documents today, and a new one degrades instead of
/// breaking.
fn word_or(texts: &ronoctua_i18n::Texts, family: &str, raw: &str) -> String {
    let said = texts.t(&format!("{family}-{raw}"));
    if said.starts_with('[') {
        return raw.to_owned();
    }
    said
}

impl View {
    /// A fresh view, on the list.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            cursor: 1,
            ..Self::default()
        }
    }

    /// A fresh view, open on one device — what a click on a modeline mark
    /// and what `bluetooth-device <path>` both do.
    #[must_use]
    pub fn fresh_at(at: &str) -> Self {
        Self {
            cursor: 1,
            level: Level::Device(at.to_owned()),
            ..Self::default()
        }
    }

    /// Reads the audio card of whatever device this view is on.
    ///
    /// Called when the level CHANGES and not on every frame: `pw-dump` prints
    /// the whole graph, and a headset's profile does not move while nobody is
    /// pressing a key. A failure is `None` and the row simply is not drawn —
    /// PipeWire not answering is not a reason for the view to say anything.
    pub fn look_at_the_device(&mut self, scene: &Scene) {
        self.card = None;
        self.sink = None;
        self.settings = false;
        self.pane = Pane::Facts;
        let Level::Device(at) = &self.level else {
            return;
        };
        let Some(d) = scene.device(at) else {
            return;
        };
        // WHETHER THERE IS A SECOND HALF AT ALL, and the hint line needs the
        // answer without having the scene. A device that exposes none of the
        // three is not offered `TAB`.
        self.settings = d.wake.is_some();
        let address = d.address.clone();
        if address.is_empty() {
            return;
        }
        match ronoctua_bluetooth::audio::card(&address) {
            Ok(card) => self.card = card,
            Err(e) => tracing::debug!(error = %e, "no audio card for this device"),
        }
        match ronoctua_bluetooth::audio::sink(&address) {
            Ok(sink) => self.sink = sink,
            Err(e) => tracing::debug!(error = %e, "no audio sink for this device"),
        }
        self.settings = self.settings || self.card.is_some() || self.sink.is_some();
    }

    /// The row the cursor is on — what the region scrolls to.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Puts the cursor on row `n` AND remembers what is there.
    ///
    /// Every move goes through here. Writing `self.cursor` on its own leaves
    /// the anchor pointing at the row somebody was on before, and the next
    /// `settle` walks the cursor back to it.
    fn put(&mut self, n: usize, rows: &[Row]) {
        self.cursor = n;
        self.at = rows
            .get(n)
            .map(|r| r.target.clone())
            .filter(|t| *t != Target::Nothing);
    }

    /// Which screen is open. Read by the tests, and by whoever asks the view
    /// where it is without reaching into it — no door asks yet.
    #[must_use]
    pub const fn level(&self) -> &Level {
        &self.level
    }

    /// Is a question waiting?
    ///
    /// Both doors ask before letting a CLICK act: a pairing question opens
    /// over whatever was on the screen, and yes must not be one click away
    /// from a device nobody has seen before.
    #[must_use]
    pub const fn asking(&self) -> bool {
        self.question.is_some()
    }

    /// The other side is asking something.
    pub fn asked(&mut self, question: agent::Question) {
        self.question = Some(question);
        self.at = None;
        // ROW 1 IS THE SENTENCE, whose target is `Nothing` — and `Enter` on
        // anything that is not `Answer(false)` used to fall through to YES.
        // A question that opens itself over whatever was on screen was then
        // one keypress from pairing an unknown device. The cursor lands on
        // the refusal, which is drawn first for exactly this.
        self.cursor = 2;
    }

    /// Which half a pointer at this cell is over.
    ///
    /// Separate from [`Self::clicked`] because the door has to ask it FIRST:
    /// the rows describe whichever half has the keyboard, so it has to know
    /// before it builds them.
    pub fn aim(&mut self, cell: Option<u16>, columns: u16) {
        if let Some(pane) = self.pane_at(cell, columns) {
            self.pane = pane;
        }
    }

    /// The half a pointer at `cell` is over — `None` where the line is not
    /// two halves side by side (no pointer, too narrow, not a device's page
    /// or one with nothing to turn, or a pairing question over it, whose
    /// rows are whole), and the line is the rows' own.
    fn pane_at(&self, cell: Option<u16>, columns: u16) -> Option<Pane> {
        let cell = cell?;
        (self.question.is_none()
            && two_columns(columns)
            && matches!(self.level, Level::Device(_))
            && self.settings)
            .then(|| {
                if cell >= split_at(columns) {
                    Pane::Settings
                } else {
                    Pane::Facts
                }
            })
    }

    /// The cells of the line a pointer at `cell` lights, when the line is
    /// two halves side by side: the half under it — a band across both
    /// would say the two rows on it were one. `None` for a line of one row.
    #[must_use]
    pub fn half_at(&self, cell: Option<u16>, columns: u16) -> Option<(u16, u16)> {
        Some(match self.pane_at(cell, columns)? {
            Pane::Facts => (0, split_at(columns)),
            Pane::Settings => (split_at(columns), columns),
        })
    }

    /// Whether a click on row `n` at `cell` takes something — what
    /// [`Self::clicked`] does there: aimed at the half under the cell, a row
    /// one can stand on; never a heading, a sentence or the blank beside a
    /// row of the other half. `None` for the cell is a caller with no
    /// pointer, and the half the keyboard is in.
    ///
    /// ONE ANSWER FOR THE LIGHT AND THE CLICK, in both doors. The rows
    /// describe the half the keyboard is in, so a row's own `heading` was the
    /// wrong question on a device's page side by side, in both directions:
    /// the pointer over the other half's control lit nothing, and over the
    /// blank beside the keyboard's row it lit a line whose click there did
    /// nothing.
    #[must_use]
    pub fn takes_at(
        &self,
        n: usize,
        cell: Option<u16>,
        scene: &Scene,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        let mut aimed = self.clone();
        aimed.aim(cell, columns);
        aimed
            .rows(scene, columns, texts)
            .get(n)
            .is_some_and(|r| !r.heading)
    }

    /// Which rows a click takes somewhere along them — the region's
    /// `Row::takes`, the window's `Content::takes`: a row whose half under
    /// the pointer the band then lights ([`Self::takes_at`],
    /// [`Self::half_at`]), side by side; the row itself, everywhere else.
    #[must_use]
    pub fn takes(&self, scene: &Scene, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<bool> {
        let rows = self.rows(scene, columns, texts);
        if self.half_at(Some(0), columns).is_none() {
            return rows.iter().map(|r| !r.heading).collect();
        }
        // EACH HALF'S ROWS, built once — as [`Self::takes_at`] builds them
        // for a click there.
        let halves: Vec<Vec<Row>> = [0, split_at(columns)]
            .into_iter()
            .map(|cell| {
                let mut aimed = self.clone();
                aimed.aim(Some(cell), columns);
                aimed.rows(scene, columns, texts)
            })
            .collect();
        (0..rows.len())
            .map(|n| {
                halves
                    .iter()
                    .any(|half| half.get(n).is_some_and(|r| !r.heading))
            })
            .collect()
    }

    /// Puts the cursor on row `n`, when it is a row one can stand on.
    ///
    /// `cell` is where along the line the pointer was, and it decides WHICH
    /// HALF of a device's page was clicked — the keyboard's `TAB` and the
    /// mouse have to reach the same two places, or the second column would be
    /// keyboard-only. `None` is a caller that has no pointer, which leaves
    /// the halves as they are.
    pub fn clicked(&mut self, n: usize, cell: Option<u16>, columns: u16, rows: &[Row]) {
        self.aim(cell, columns);
        if rows.get(n).is_some_and(|r| !r.heading) {
            self.put(n, rows);
        }
    }

    /// Keeps the cursor on a row that EXISTS.
    ///
    /// Devices come and go on their own — a scan finds four in a second, a
    /// headset switched off takes its row away — and a cursor past the end
    /// focuses nothing: the arrows' first step is out of range too, so
    /// neither of them moves and `RET` has no target.
    pub fn settle(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            self.at = None;
            return;
        }
        // THE ANCHOR FIRST: if what the cursor was on is still on the screen,
        // the cursor goes to where it is NOW. That is the whole point of
        // keeping it — the row moved, the aim did not.
        if let Some(at) = self.at.as_ref()
            && let Some(i) = rows.iter().position(|r| r.target == *at)
        {
            self.cursor = i;
            return;
        }
        // AND WHEN IT IS GONE — the headset was switched off — the index is
        // all that is left, so it is clamped and the anchor is taken from
        // wherever that lands.
        if self.cursor >= rows.len() {
            self.cursor = rows.len() - 1;
        }
        while self.cursor > 0 && rows[self.cursor].heading {
            self.cursor -= 1;
        }
        if rows[self.cursor].heading
            && let Some(i) = rows.iter().position(|r| !r.heading)
        {
            self.cursor = i;
        }
        self.at = rows
            .get(self.cursor)
            .map(|r| r.target.clone())
            .filter(|t| *t != Target::Nothing);
    }

    /// The hint line, generated from the table — the same pieces in both
    /// doors. With a question waiting, the two answers come first: the row
    /// says them too, but the row may be off the page and the line never is.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        let mut out = Vec::new();
        if self.question.is_some() {
            out.push(crate::chrome::Piece::prose(
                texts.t(crate::key::BLUETOOTH_PAIRING_ANSWER),
            ));
            out.push(crate::chrome::Piece::separator());
        }
        // A KEY THAT IS ONLY HERE BECAUSE OF THE ROW UNDER THE CURSOR reads as
        // one (`Register::Contextual`): the owner asked for the passing guests to
        // look apart from the keys that are always on the line.
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&self.level, self.question.is_some(), self.settings))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        out.extend(crate::chrome::hint_of_marked(&here, |_| None, texts));
        out
    }

    /// Acts on a key.
    #[must_use]
    pub fn key(
        &mut self,
        press: Press,
        scene: &Scene,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        // THE WIDTH IS THE DOOR'S, and it has to be: a device's page has two
        // halves that sit side by side above `space.bluetooth.settingsFrom`
        // and stack below it, and `TAB` means "move the keyboard" in one and
        // "move the cursor" in the other. This used to decide every key at a
        // fixed eighty columns, which was sound while every screen was one
        // column — the rows really were the same things at every width.
        let width = columns.max(1);
        // A NAME NOBODY TYPED is let go by the next key: while a line asks,
        // no key reaches the view, so a key here means the line is gone.
        self.renaming = None;
        let rows = self.rows(scene, width, texts);
        self.settle(&rows);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        let disabled = rows.get(self.cursor).is_some_and(|r| r.disabled);
        // THE QUESTION TAKES THE SCREEN. Nothing pairs until it is answered,
        // so while it stands only the two answers and the walk-back run.
        if let Some(q) = self.question.clone() {
            // A QUESTION THAT ONLY TELLS has no answer: six digits to type on
            // the other thing, and any key takes the sentence off the screen.
            if q.asked.only_tells() {
                return match press {
                    Press::Up | Press::Down => Errand::Nothing,
                    _ => {
                        self.question = None;
                        Errand::Nothing
                    }
                };
            }
            return match press {
                // `RET` ANSWERS THE ROW UNDER THE CURSOR, and anything that
                // is not the YES row is a NO. It used to be the other way
                // round — every target but `Answer(false)` fell through to
                // yes, including the SENTENCE the cursor started on.
                Press::Enter => match target {
                    Target::Answer(true) if q.asked.typed() => Errand::Type,
                    Target::Answer(true) => self.answer(true),
                    _ => self.answer(false),
                },
                Press::Accept if q.asked.typed() => Errand::Type,
                Press::Accept => self.answer(true),
                Press::Refuse | Press::Escape => self.answer(false),
                Press::Up | Press::Down => {
                    let step: isize = if press == Press::Down { 1 } else { -1 };
                    let mut i = self.cursor;
                    loop {
                        let next = i.checked_add_signed(step);
                        match next {
                            Some(x) if x < rows.len() => i = x,
                            _ => break,
                        }
                        if !rows[i].heading {
                            self.put(i, &rows);
                            break;
                        }
                    }
                    Errand::Nothing
                }
                _ => Errand::Nothing,
            };
        }
        match press {
            Press::Up | Press::Down => {
                let step: isize = if press == Press::Down { 1 } else { -1 };
                let mut i = self.cursor;
                loop {
                    let next = i.checked_add_signed(step);
                    match next {
                        Some(n) if n < rows.len() => i = n,
                        _ => break,
                    }
                    if !rows[i].heading {
                        self.put(i, &rows);
                        break;
                    }
                }
                Errand::Nothing
            }
            Press::Escape => {
                if self.level == Level::List {
                    return Errand::Close;
                }
                self.level = Level::List;
                self.cursor = 1;
                self.at = None;
                Errand::Nothing
            }
            Press::Enter if disabled => Errand::Nothing,
            Press::Enter => match target {
                // AN INFORMATIVE ROW COPIES ITSELF.
                //
                // `RET` here used to be the one key on the one row that did
                // nothing, in a list where half of what is drawn is a thing
                // somebody wants to paste. See `chrome::information_of`.
                Target::Nothing => rows
                    .get(self.cursor)
                    .and_then(|r| crate::chrome::information_of(&r.fields))
                    .map_or(Errand::Nothing, Errand::Copy),
                Target::Device(at) => {
                    self.level = Level::Device(at);
                    self.cursor = 1;
                    self.at = None;
                    // THE CARD IS READ HERE and nowhere else: opening a
                    // device is the one moment its audio profile can have
                    // changed without this view knowing, and `pw-dump` is a
                    // process — asking it per frame would put a fork in the
                    // footer's loop.
                    self.look_at_the_device(scene);
                    Errand::Nothing
                }
                Target::Radio => {
                    self.level = Level::Radio;
                    self.cursor = 1;
                    self.at = None;
                    Errand::Nothing
                }
                Target::Power(on) => Errand::Order(Ask::Power(on)),
                Target::Discoverable(on) => Errand::Order(Ask::Discoverable(on)),
                Target::Pairable(on) => Errand::Order(Ask::Pairable(on)),
                Target::Timeout(seconds) => Errand::Order(Ask::DiscoverableTimeout(seconds)),
                Target::PairableTimeout(seconds) => Errand::Order(Ask::PairableTimeout(seconds)),
                Target::Wake(at, on) => Errand::Order(Ask::Wake(at, on)),
                Target::Tether(address, on) => Errand::Tether {
                    name: scene
                        .devices
                        .iter()
                        .find(|d| d.address == address)
                        .map_or_else(|| address.clone(), |d| d.label.clone()),
                    address,
                    on,
                },
                // `RET` ON A VOLUME IS ONE STEP UP, and then it wraps to
                // nothing: a row that only ever goes up is a row you cannot
                // undo without reaching for the arrows. The arrows are the
                // proper answer and the hint teaches them; this is the one
                // key that is always on a row, so it does the obvious thing.
                Target::Keyboards(which) => Errand::Run("keyboard-open", which),
                Target::Volume(node, wanted) => {
                    if let Some(sink) = self.sink.as_mut() {
                        sink.volume = wanted;
                    }
                    Errand::Volume(node, wanted)
                }
                Target::Mute(node, on) => {
                    if let Some(sink) = self.sink.as_mut() {
                        sink.muted = on;
                    }
                    Errand::Mute(node, on)
                }
                Target::Audio(card, profile) => Errand::Audio(card, profile),
                // THE MACHINE'S OWN NAME is typed the way a device's is —
                // `Errand::Rename` with no path means this machine.
                Target::Rename => self.rename(String::new()),
                Target::Answer(_) => Errand::Nothing,
                Target::Scan(on) => Errand::Order(Ask::Scan(on)),
                Target::Profile { device, uuid, on } => {
                    Errand::Order(Ask::Profile { device, uuid, on })
                }
            },
            Press::Accept | Press::Refuse => Errand::Nothing,
            // THE RADIO'S THREE, and they obey the rule their own rows draw.
            //
            // `radio()` marks scan, discoverable and pairable `disabled` while
            // the adapter is off, and `RET` on a dim row does nothing. These
            // three letters read the adapter directly and sent the order
            // anyway: `v` with the radio off reached `org.bluez.Error.NotReady`
            // and came back as "there is no such thing on this machine" — the
            // wrong sentence entirely — while `RET` on the very same row, drawn
            // dim two lines away, did nothing at all. One action, one door, and
            // it was answering two different ways depending on how you got there.
            //
            // ONE ANSWER for this view and the footer's keys with the view
            // closed (`bluez_words::adapter_order`), power the exception —
            // read off the ADAPTER and not the row, which `disabled` is about.
            Press::Scan | Press::Power | Press::Discoverable => {
                let key = match press {
                    Press::Scan => AdapterKey::Scan,
                    Press::Power => AdapterKey::Power,
                    _ => AdapterKey::Discoverable,
                };
                adapter_order(key, scene).map_or(Errand::Nothing, Errand::Order)
            }
            Press::Unnamed => {
                self.unnamed = !self.unnamed;
                Errand::Nothing
            }
            // MORE AND LESS, of whatever the cursor is on.
            //
            // They used to be the audio profile's ring and nothing else,
            // which was true while the profile was the only thing in this
            // view that had an order. Now the settings column has a volume
            // too, so they ask the ROW — and a row that has no order simply
            // does not move, which is what this pair on a fact should do.
            //
            // The profile still acts on the CARD the view is holding rather
            // than on the row, for the power view's reason: the row says
            // which profile is on, and `Card::next` is what knows which ones
            // the device can actually do.
            Press::More | Press::Less => {
                let up = press == Press::More;
                match target {
                    Target::Volume(node, _) => {
                        let now = self.sink.as_ref().map_or(0, |s| s.volume);
                        let wanted = step(now, up);
                        if wanted == now {
                            return Errand::Nothing;
                        }
                        // THE VIEW MOVES ITS OWN NUMBER, and does not wait for
                        // PipeWire to be asked again. `pw-dump` is read when
                        // the level opens, so a row that waited for it would
                        // sit at the old percentage until somebody left the
                        // page and came back — one key press, no answer.
                        if let Some(sink) = self.sink.as_mut() {
                            sink.volume = wanted;
                        }
                        Errand::Volume(node, wanted)
                    }
                    _ => {
                        let Some(card) = self.card.as_ref() else {
                            return Errand::Nothing;
                        };
                        let Some(next) = card.next(up) else {
                            return Errand::Nothing;
                        };
                        Errand::Audio(card.id, next.index)
                    }
                }
            }
            // CROSSING TO THE OTHER HALF.
            //
            // Side by side it moves the KEYBOARD and leaves the cursor on its
            // line, which is what "act on the other column of this row" means.
            // Stacked, the two halves are one list, so it moves the CURSOR to
            // the top of the other block — walking twelve facts with an arrow
            // to reach the volume is not crossing, it is scrolling.
            Press::Cross => {
                if !self.settings || !matches!(self.level, Level::Device(_)) {
                    return Errand::Nothing;
                }
                if two_columns(width) {
                    self.pane = match self.pane {
                        Pane::Facts => Pane::Settings,
                        Pane::Settings => Pane::Facts,
                    };
                    let rows = self.rows(scene, width, texts);
                    // THE LINE IS KEPT, and the nearest row that can be stood
                    // on is taken: the other column is usually shorter, and a
                    // cursor left past its end would focus nothing.
                    let mut i = self.cursor.min(rows.len().saturating_sub(1));
                    while i > 0 && rows.get(i).is_some_and(|r| r.heading) {
                        i -= 1;
                    }
                    if rows.get(i).is_some_and(|r| r.heading) {
                        i = rows.iter().position(|r| !r.heading).unwrap_or(0);
                    }
                    self.put(i, &rows);
                } else {
                    let rows = self.rows(scene, width, texts);
                    let facts = self.facts_len_at(scene, width, texts);
                    let (from, to) = if self.cursor < facts {
                        (facts, rows.len())
                    } else {
                        (0, facts)
                    };
                    if let Some(i) = (from..to).find(|i| rows.get(*i).is_some_and(|r| !r.heading)) {
                        self.put(i, &rows);
                    }
                }
                Errand::Nothing
            }
            // THE REST ARE ABOUT THE DEVICE UNDER THE CURSOR, and the detail
            // is about the device it opened: one lookup, two levels.
            Press::Connect
            | Press::Pair
            | Press::Forget
            | Press::Trust
            | Press::Block
            | Press::Rename => {
                let Some(d) = self.about(&target, scene) else {
                    return Errand::Nothing;
                };
                let at = d.at.clone();
                match press {
                    Press::Connect if d.connected => Errand::Order(Ask::Disconnect(at)),
                    Press::Connect => Errand::Order(Ask::Connect(at)),
                    // ALREADY PAIRED IS NOTHING TO DO. `CancelPairing` was
                    // what this used to send, and BlueZ refuses it whenever
                    // no pairing is under way — which is always, here. The
                    // way to stop a pairing is to answer its question no.
                    Press::Pair if d.paired => Errand::Nothing,
                    Press::Pair => Errand::Order(Ask::Pair(at)),
                    Press::Forget if d.paired => Errand::Order(Ask::Forget(at)),
                    Press::Forget => Errand::Nothing,
                    Press::Trust => Errand::Order(Ask::Trust(at, !d.trusted)),
                    Press::Block => Errand::Order(Ask::Block(at, !d.blocked)),
                    _ => self.rename(at),
                }
            }
        }
    }

    /// Answers the waiting question.
    fn answer(&mut self, yes: bool) -> Errand {
        self.question = None;
        Errand::Answer(yes)
    }

    /// A name is to be typed for `at` — remembered here, so the answer can
    /// be told what it names.
    fn rename(&mut self, at: String) -> Errand {
        self.renaming = Some(at.clone());
        Errand::Rename(at)
    }

    /// The name the thing being renamed has NOW — what the door's line
    /// starts from, in both doors.
    ///
    /// The alias first, then its own name: `label` falls back to the address,
    /// and an address is not a name anybody would keep four letters of.
    #[must_use]
    pub fn name_being_changed(&self, scene: &Scene) -> Option<String> {
        let at = self.renaming.as_deref()?;
        let (alias, name) = if at.is_empty() {
            let a = scene.adapter()?;
            (a.alias.as_str(), a.name.as_str())
        } else {
            let d = scene.device(at)?;
            (d.alias.as_str(), d.name.as_str())
        };
        [alias, name]
            .into_iter()
            .find(|s| !s.trim().is_empty())
            .map(ToOwned::to_owned)
    }

    /// What a typed name becomes — the one answer both doors carry out.
    ///
    /// An EMPTY name is a no: the line was left with nothing on it, and a
    /// device keeps the name it had. A name nobody asked for (no rename under
    /// way) is nothing too: `bluetooth-rename <name>` from `LEADER :` with the
    /// view on another screen must not rename whatever was last in mind.
    pub fn renamed(&mut self, name: &str) -> Errand {
        let Some(at) = self.renaming.take() else {
            return Errand::Nothing;
        };
        let name = name.trim();
        if name.is_empty() {
            return Errand::Nothing;
        }
        if at.is_empty() {
            Errand::Order(Ask::AdapterAlias(name.to_owned()))
        } else {
            Errand::Order(Ask::Rename(at, name.to_owned()))
        }
    }

    /// Forgets the question — the door calls this when the digits have been
    /// sent, and when a reading says the device paired on its own.
    pub fn answered(&mut self) {
        self.question = None;
    }

    /// The device a question is about, for the door that has to say so.
    #[must_use]
    pub fn asking_about(&self) -> Option<&str> {
        self.question.as_ref().map(|q| q.device.as_str())
    }

    /// Which device a key is about: the one the detail is open on, or the
    /// one under the cursor in the list.
    fn about<'a>(&self, target: &Target, scene: &'a Scene) -> Option<&'a Device> {
        match &self.level {
            Level::Device(at) => scene.device(at),
            _ => match target {
                Target::Device(at) => scene.device(at),
                _ => None,
            },
        }
    }

    /// The rows, for whichever level is open.
    #[must_use]
    pub fn rows(&self, scene: &Scene, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
            said: None,
        };
        if let Some(q) = &self.question {
            self.prompt(&mut b, q, scene);
            return b.out;
        }
        match &self.level {
            Level::List => self.list(&mut b, scene),
            Level::Radio => self.radio(&mut b, scene),
            Level::Device(at) => match scene.device(at) {
                // THE DEVICE PAGE HAS TWO HALVES and builds its own rows: it
                // needs an extent per half, and the builder above has one.
                Some(d) => return self.device_rows(d, columns, texts),
                // IT WENT AWAY WHILE THE DETAIL WAS OPEN: say so where the
                // facts were, rather than draw an empty screen.
                None => {
                    b.heading(crate::key::BLUETOOTH_HEADING_PAIRED);
                    b.note(crate::key::BLUETOOTH_NOTHING_HERE);
                }
            },
        }
        b.out
    }

    /// The question, and the two answers.
    fn prompt(&self, b: &mut Builder<'_>, q: &agent::Question, scene: &Scene) {
        use crate::key;
        let name = scene
            .device(&q.device)
            .map_or_else(|| q.device.clone(), |d| d.label.clone());
        let said = match &q.asked {
            agent::Asked::Confirm(code) => b.texts.t_with(
                key::BLUETOOTH_PAIRING_CONFIRM,
                &[("name", &name), ("code", code)],
            ),
            agent::Asked::Passkey(code) => b.texts.t_with(
                key::BLUETOOTH_PAIRING_PASSKEY,
                &[("name", &name), ("code", code)],
            ),
            agent::Asked::Pin => b
                .texts
                .t_with(key::BLUETOOTH_PAIRING_PIN, &[("name", &name)]),
            // SIX DIGITS AS A NUMBER, which is not the same request as a PIN
            // and does not read the same: `RequestPasskey` used to be
            // labelled a PIN, and a keyboard has no code printed on it.
            agent::Asked::Typed => b
                .texts
                .t_with(key::BLUETOOTH_PAIRING_PASSKEY_HERE, &[("name", &name)]),
            agent::Asked::Authorize => b
                .texts
                .t_with(key::BLUETOOTH_PAIRING_AUTHORIZE, &[("name", &name)]),
            agent::Asked::Service(uuid) => {
                let service = ronoctua_bluetooth::profile_key(uuid)
                    .map_or_else(|| uuid.clone(), |k| b.texts.t(k));
                b.texts.t_with(
                    key::BLUETOOTH_PAIRING_SERVICE,
                    &[("name", &name), ("service", &service)],
                )
            }
        };
        b.heading(key::BLUETOOTH_TITLE);
        b.sentence(said);
        // `NO` IS THE FIRST ANSWER, and the cursor starts on it — the format
        // buffer's rule, and for its reason: a question that arrives on its
        // own, over whatever was on screen, must not be answered YES by the
        // next `RET` somebody presses. `View::asked` puts the cursor on this
        // row, which is why it is drawn before the other one.
        b.pair(
            b.texts.t(key::BLUETOOTH_REFUSE),
            String::new(),
            Target::Answer(false),
            false,
        );
        b.pair(
            b.texts.t(key::BLUETOOTH_ACCEPT),
            String::new(),
            Target::Answer(true),
            false,
        );
    }

    /// The radio, the paired devices, and what is nearby.
    fn list(&self, b: &mut Builder<'_>, scene: &Scene) {
        use crate::key;
        b.heading(key::BLUETOOTH_HEADING_ADAPTER);
        match scene.adapter() {
            None if scene.daemon => b.note(key::BLUETOOTH_NO_ADAPTER),
            None => b.note(key::BLUETOOTH_NO_DAEMON),
            Some(a) => {
                let state = b.texts.t(&format!(
                    "bluetooth-power-state-{}",
                    if a.power_state.is_empty() {
                        if a.powered { "on" } else { "off" }
                    } else {
                        a.power_state.as_str()
                    }
                ));
                let mark = if a.powered {
                    tokens::icon::BLUETOOTH_CONNECTED
                } else {
                    tokens::icon::BLUETOOTH_OFF
                };
                // NOT DISABLED WHEN IT IS OFF: the screen this row opens is
                // the one holding the switch that turns it on, and a door
                // that refuses while the thing behind it is off is a door
                // nobody can open.
                b.pair(format!("{mark} {}", a.alias), state, Target::Radio, false);
                if a.discovering {
                    b.note(key::BLUETOOTH_SCANNING);
                }
            }
        }

        b.heading(key::BLUETOOTH_HEADING_PAIRED);
        let paired = scene.paired();
        if paired.is_empty() {
            b.note(key::BLUETOOTH_NOTHING_HERE);
        }
        for d in paired {
            self.device_row(b, d);
        }

        b.heading(key::BLUETOOTH_HEADING_NEARBY);
        let nearby = scene.nearby();
        let hidden = nearby.iter().filter(|d| !d.named()).count();
        let shown: Vec<&Device> = nearby
            .into_iter()
            .filter(|d| self.unnamed || d.named())
            .collect();
        if shown.is_empty() {
            b.note(key::BLUETOOTH_NOTHING_HERE);
        }
        for d in shown {
            self.device_row(b, d);
        }
        if hidden > 0 && !self.unnamed {
            let count = i64::try_from(hidden).unwrap_or(i64::MAX);
            let said = b.texts.t_args(
                key::BLUETOOTH_UNNAMED_HIDDEN,
                &[("count", ronoctua_i18n::Arg::Number(count))],
            );
            b.sentence(said);
        }
    }

    /// One device, as the list draws it: the glyph, the label, and what is
    /// worth knowing at a glance.
    fn device_row(&self, b: &mut Builder<'_>, d: &Device) {
        use crate::key;
        let kind = b.texts.t(&format!("bluetooth-kind-{}", d.kind.id()));
        // THE SENTENCE IS THE LANGUAGE'S, punctuation included: an em dash
        // written here would be a visual literal in Rust (invariant 9), and
        // it is not the same dash in every language's typography.
        let label = format!(
            "{} {}",
            glyph(d.kind),
            b.texts.t_with(
                key::BLUETOOTH_ROW_KIND,
                &[("name", &d.label), ("kind", &kind)]
            )
        );
        let mut said = Vec::new();
        if d.connected {
            said.push(b.texts.t(key::BLUETOOTH_FACT_CONNECTED));
        }
        if let Some(p) = d.battery {
            said.push(b.texts.t_args(
                key::BLUETOOTH_PERCENT,
                &[("percent", ronoctua_i18n::Arg::Number(i64::from(p)))],
            ));
        }
        if d.blocked {
            said.push(b.texts.t(key::BLUETOOTH_FACT_BLOCKED));
        }
        if let Some(rssi) = d.rssi.filter(|_| !d.paired) {
            said.push(b.texts.t_args(
                key::BLUETOOTH_RSSI_VALUE,
                &[("dbm", ronoctua_i18n::Arg::Number(i64::from(rssi)))],
            ));
        }
        b.coloured(
            label,
            said.join(&format!(" {} ", tokens::mark::SEPARATOR)),
            Target::Device(d.at.clone()),
            d.blocked,
            d.alarmed().then_some(tokens::color::STATE_ERROR),
        );
    }

    /// One device, whole: every fact BlueZ has, and one row per service.
    fn detail(&self, b: &mut Builder<'_>, d: &Device, texts: &ronoctua_i18n::Texts) {
        use crate::key;
        use ronoctua_i18n::Arg;
        let yes_no = |on: bool| {
            texts.t(if on {
                key::BLUETOOTH_YES
            } else {
                key::BLUETOOTH_NO
            })
        };
        // THE PAGE IS TITLED WITH THE DEVICE, not with the word `Bluetooth` —
        // which was true of this screen, of the list, and of the radio, and
        // therefore told nobody which of the three they were on.
        b.heading_of(format!("{} {}", glyph(d.kind), d.label));
        // AND WHAT IT IS gets a label of its own. The name was the LABEL and
        // the kind the value, which reads as though the device were called
        // "headphones" — a row whose left half is data is a row with no
        // question in it.
        b.about_key(key::BLUETOOTH_ABOUT_KIND);
        b.fact(
            key::BLUETOOTH_FACT_KIND,
            texts.t(&format!("bluetooth-kind-{}", d.kind.id())),
        );
        b.about_key(key::BLUETOOTH_ABOUT_CONNECTED);
        b.fact_coloured(key::BLUETOOTH_FACT_CONNECTED, yes_no(d.connected), None);
        if let Some(p) = d.battery {
            b.about_key(key::BLUETOOTH_ABOUT_BATTERY);
            b.fact_coloured(
                key::BLUETOOTH_FACT_BATTERY,
                texts.t_args(
                    key::BLUETOOTH_PERCENT,
                    &[("percent", Arg::Number(i64::from(p)))],
                ),
                d.alarmed().then_some(tokens::color::STATE_ERROR),
            );
        }
        if let Some(source) = &d.battery_source {
            b.about_key(key::BLUETOOTH_ABOUT_BATTERY_SOURCE);
            b.fact(key::BLUETOOTH_FACT_BATTERY_SOURCE, source.clone());
        }
        b.about_key(key::BLUETOOTH_ABOUT_PAIRED);
        b.fact(key::BLUETOOTH_FACT_PAIRED, yes_no(d.paired));
        b.about_key(key::BLUETOOTH_ABOUT_BONDED);
        b.fact(key::BLUETOOTH_FACT_BONDED, yes_no(d.bonded));
        b.about_key(key::BLUETOOTH_ABOUT_TRUSTED);
        b.fact(key::BLUETOOTH_FACT_TRUSTED, yes_no(d.trusted));
        b.about_key(key::BLUETOOTH_ABOUT_BLOCKED);
        b.fact(key::BLUETOOTH_FACT_BLOCKED, yes_no(d.blocked));
        if !d.name.is_empty() {
            b.about_key(key::BLUETOOTH_ABOUT_NAME);
            b.fact(key::BLUETOOTH_FACT_NAME, d.name.clone());
        }
        if !d.alias.is_empty() && d.alias != d.name {
            b.about_key(key::BLUETOOTH_ABOUT_ALIAS);
            b.fact(key::BLUETOOTH_FACT_ALIAS, d.alias.clone());
        }
        b.about_key(key::BLUETOOTH_ABOUT_ADDRESS);
        b.fact(key::BLUETOOTH_FACT_ADDRESS, d.address.clone());
        if !d.address_type.is_empty() {
            // A CLOSED VOCABULARY IS A TEXT KEY, not the daemon's word.
            //
            // `public`/`random` is a list of two, not free-form data like a
            // device's name — and drawn raw it put an English word on a pt-BR
            // screen ("tipo de endereço: random"). This file already
            // translates `PowerState` and every service UUID through a key;
            // three enumerations had been left out.
            b.about_key(key::BLUETOOTH_ABOUT_ADDRESS_TYPE);
            b.fact(
                key::BLUETOOTH_FACT_ADDRESS_TYPE,
                word_or(texts, "bluetooth-address-type", &d.address_type),
            );
        }
        if let Some(rssi) = d.rssi {
            b.about_key(key::BLUETOOTH_ABOUT_RSSI);
            b.fact(
                key::BLUETOOTH_FACT_RSSI,
                texts.t_args(
                    key::BLUETOOTH_RSSI_VALUE,
                    &[("dbm", Arg::Number(i64::from(rssi)))],
                ),
            );
        }
        if let Some(tx) = d.tx_power {
            b.about_key(key::BLUETOOTH_ABOUT_TX_POWER);
            b.fact(
                key::BLUETOOTH_FACT_TX_POWER,
                texts.t_args(
                    key::BLUETOOTH_TX_POWER_VALUE,
                    &[("dbm", Arg::Number(i64::from(tx)))],
                ),
            );
        }
        if d.legacy {
            b.about_key(key::BLUETOOTH_ABOUT_LEGACY);
            b.fact(key::BLUETOOTH_FACT_LEGACY, yes_no(true));
        }
        if let Some(wake) = d.wake {
            // A SWITCH AND NOT A FACT, and it stays HERE among the facts.
            //
            // It was moved to the settings column for one afternoon, and the
            // owner sent it back: that column is for what this device adds,
            // not for things taken out of this one. `WakeAllowed` is a
            // property of `Device1` like every other row around it — the only
            // difference is that this one can be turned.
            b.about_key(key::BLUETOOTH_ABOUT_WAKE);
            b.pair(
                b.texts.t(key::BLUETOOTH_FACT_WAKE),
                yes_no(wake),
                Target::Wake(d.at.clone(), !wake),
                !d.paired,
            );
        }
        // ITS INTERNET, when it shares one (a phone's PAN): a switch, because
        // it is something this machine can DO with the device — and the
        // value names the interface while it is up, `bnep0`, the name the
        // network's view lists the same link under.
        if d.shares_internet() {
            let up = d.pan.as_ref().filter(|p| p.connected);
            b.about_key(key::BLUETOOTH_ABOUT_INTERNET);
            b.pair(
                b.texts.t(key::BLUETOOTH_FACT_INTERNET),
                up.map_or_else(
                    || yes_no(false),
                    |p| {
                        texts.t_with(
                            key::BLUETOOTH_INTERNET_THROUGH,
                            &[("interface", p.interface.as_str())],
                        )
                    },
                ),
                Target::Tether(d.address.clone(), up.is_none()),
                // ONLY A PAIRED PHONE: NetworkManager has no device for one
                // that is not, and the row says so by being dim.
                !d.paired,
            );
        }
        b.about_key(key::BLUETOOTH_ABOUT_SERVICES);
        b.fact(key::BLUETOOTH_FACT_SERVICES, yes_no(d.services_resolved));
        if let Some(mode) = &d.reconnect {
            b.about_key(key::BLUETOOTH_ABOUT_RECONNECT);
            b.fact(
                key::BLUETOOTH_FACT_RECONNECT,
                word_or(texts, "bluetooth-reconnect", mode),
            );
        }
        if let Some(class) = d.class {
            b.about_key(key::BLUETOOTH_ABOUT_CLASS);
            b.fact(key::BLUETOOTH_FACT_CLASS, format!("0x{class:06x}"));
        }
        if let Some(appearance) = d.appearance {
            b.about_key(key::BLUETOOTH_ABOUT_APPEARANCE);
            b.fact(key::BLUETOOTH_FACT_APPEARANCE, appearance.to_string());
        }
        if let Some(modalias) = &d.modalias {
            b.about_key(key::BLUETOOTH_ABOUT_MODALIAS);
            b.fact(key::BLUETOOTH_FACT_MODALIAS, modalias.clone());
        }
        if !d.adapter.is_empty() {
            b.about_key(key::BLUETOOTH_ABOUT_ADAPTER);
            b.fact(key::BLUETOOTH_FACT_ADAPTER, d.adapter.clone());
        }
        // WHICH WAY THE SOUND GOES, and it belongs here for the same reason
        // the wake switch does: it is what the device IS doing, which is the
        // question this column answers. BlueZ cannot say it (`MediaTransport1`
        // reports what is playing, never what could be), so it comes from
        // PipeWire — see `ronoctua_bluetooth::audio`.
        if let Some(card) = self.card.as_ref()
            && let Some(now) = card.now()
        {
            b.about_key(key::BLUETOOTH_ABOUT_AUDIO_PROFILE);
            b.pair(
                b.texts.t(key::BLUETOOTH_AUDIO_PROFILE),
                word_or(texts, "bluetooth-audio", now),
                card.next(true)
                    .map_or(Target::Nothing, |p| Target::Audio(card.id, p.index)),
                card.next(true).is_none(),
            );
        }
        for t in &d.transports {
            let what = ronoctua_bluetooth::profile_key(&t.uuid)
                .map_or_else(|| t.uuid.clone(), |k| texts.t(k));
            b.about_key(key::BLUETOOTH_ABOUT_AUDIO);
            b.fact(
                key::BLUETOOTH_FACT_AUDIO,
                texts.t_with(
                    key::BLUETOOTH_AUDIO_STATE,
                    &[
                        ("what", &what),
                        ("state", &word_or(texts, "bluetooth-transport", &t.state)),
                    ],
                ),
            );
            if let Some(v) = t.volume {
                // A PERCENTAGE, not the raw 0-127 the bus carries. "84" is a
                // number a person cannot place; the row said it for months.
                let percent = i64::from(v).min(127) * 100 / 127;
                b.about_key(key::BLUETOOTH_ABOUT_VOLUME);
                b.fact(
                    key::BLUETOOTH_FACT_VOLUME,
                    texts.t_args(key::BLUETOOTH_PERCENT, &[("percent", Arg::Number(percent))]),
                );
            }
            if let Some(c) = t.codec {
                // THE NAME AND NOT THE BYTE. `0` is SBC and `2` is AAC; the
                // screen said `0`, which is true of nothing a person knows.
                b.about_key(key::BLUETOOTH_ABOUT_CODEC);
                b.fact(
                    key::BLUETOOTH_FACT_CODEC,
                    word_or(texts, "bluetooth-codec", &c.to_string()),
                );
            }
        }

        // THE SERVICES IT ANNOUNCES, one row each, and `RET` connects or drops
        // that one. The heading said `What it can do`, which is the kind of
        // title that fits any list and names none.
        b.heading(key::BLUETOOTH_HEADING_PROFILES);
        // ONE ROW PER SENTENCE, and two things are dropped before that.
        //
        // THE PLUMBING, because every BLE device carries `1800` and `1801`
        // whether it wants to or not, and "says its own name" in a list headed
        // *what it can do* is a row that pushed the real answer down. And a
        // SENTENCE ALREADY SAID, because the ids are roles of one profile and a
        // device announces several: the owner's headphones carry `110c`,
        // `110e` and `110f`, which are three ids for one thing, and the screen
        // drew "its buttons control the player" three times.
        //
        // The row keeps the FIRST id that said it, which is the one the device
        // put first — as good an answer as any, and it is a real id, so
        // connecting it still means something.
        let mut already: Vec<String> = Vec::new();
        for uuid in &d.uuids {
            if ronoctua_bluetooth::profile_is_plumbing(uuid) {
                continue;
            }
            let said = ronoctua_bluetooth::profile_key(uuid).map(|k| texts.t(k));
            let gloss = said.clone().unwrap_or_default();
            // ONE ROW PER SENTENCE — and an id nobody named is its own
            // sentence, so two of those never collapse into one.
            let seen = said.clone().unwrap_or_else(|| uuid.clone());
            if already.contains(&seen) {
                continue;
            }
            already.push(seen);
            // WHETHER THIS ONE IS UP, not whether the device is: on a
            // connected headset every service row used to say "disconnect",
            // so a person could never add the microphone to the music.
            // A transport is the only per-profile answer BlueZ gives; for
            // the rest the device's link is the best there is.
            let running = d
                .transports
                .iter()
                .find(|t| t.uuid.eq_ignore_ascii_case(uuid))
                .map_or(d.connected, |t| t.state != "idle");
            // THE ID IS THE LABEL, AND THE WHOLE OF IT. It is what a person
            // searches for and what a manufacturer's page names; the sentence
            // we have is a gloss, and a gloss in the label's place reads as
            // though it were the thing.
            //
            // Not the four-digit short form: that is the SIG's half of the
            // string, and half an identifier is not one — what the device
            // announces, and what anything else will call it, is the whole of
            // it.
            let named = uuid.clone();
            b.about(match &said {
                Some(sentence) => texts.t_with(
                    key::BLUETOOTH_ABOUT_PROFILE,
                    &[("id", &named), ("said", sentence)],
                ),
                None => texts.t_with(key::BLUETOOTH_ABOUT_PROFILE_UNKNOWN, &[("id", &named)]),
            });
            b.triple(
                named,
                texts.t(if running {
                    key::BLUETOOTH_YES
                } else {
                    key::BLUETOOTH_NO
                }),
                gloss,
                Target::Profile {
                    device: d.at.clone(),
                    uuid: uuid.clone(),
                    on: !running,
                },
                !d.paired,
            );
        }
        // AND THE NOTE ASKS WHAT WAS DRAWN, not what was announced. A device
        // whose whole list is plumbing — a bare BLE peripheral is exactly that
        // — used to leave the heading standing over nothing at all.
        if already.is_empty() {
            b.note(key::BLUETOOTH_NOTHING_HERE);
        }
    }

    /// The other half of a device's page: what this KIND of thing adds.
    ///
    /// # What belongs here, and what does not
    ///
    /// Only things the facts column does not already have. It is not a second
    /// home for rows moved out of the first — that was the first attempt, and
    /// the owner sent the wake switch and the audio profile back: both are
    /// properties of `Device1`, and a column that empties its neighbour has
    /// added nothing.
    ///
    /// What goes here is what this device can do that a Bluetooth device in
    /// general cannot, INCLUDING what only something outside BlueZ can answer:
    /// a headset's volume is PipeWire's, and a keyboard's keys are keyd's.
    /// That is the shape a specific device gets a specific screen through —
    /// one arm here, and the row it draws.
    fn settings(&self, b: &mut Builder<'_>, d: &Device, texts: &ronoctua_i18n::Texts) {
        use crate::key;
        use ronoctua_i18n::Arg;
        let yes_no = |on: bool| {
            texts.t(if on {
                key::BLUETOOTH_YES
            } else {
                key::BLUETOOTH_NO
            })
        };
        b.heading(key::BLUETOOTH_HEADING_SETTINGS);
        let mut any = false;

        // HOW LOUD, and it is PipeWire's answer about a NODE — BlueZ has no
        // volume to give. `wpctl` shows the cube root of what the graph
        // stores, which is what every other volume on the machine shows;
        // `audio::Sink` has already done that.
        if let Some(sink) = self.sink.as_ref() {
            any = true;
            b.about_key(key::BLUETOOTH_ABOUT_SINK_VOLUME);
            b.pair(
                b.texts.t(key::BLUETOOTH_SETTING_VOLUME),
                texts.t_args(
                    key::BLUETOOTH_PERCENT,
                    &[("percent", Arg::Number(i64::from(sink.volume)))],
                ),
                Target::Volume(sink.id, step(sink.volume, true)),
                !d.connected,
            );
            b.about_key(key::BLUETOOTH_ABOUT_MUTE);
            b.pair(
                b.texts.t(key::BLUETOOTH_SETTING_MUTE),
                yes_no(sink.muted),
                Target::Mute(sink.id, !sink.muted),
                !d.connected,
            );
        }

        // AND A KEYBOARD'S KEYS ARE KEYD'S, which is a screen of its own —
        // seven keys and nine meanings do not fit beside the facts. Two doors,
        // because there are two questions: what THIS keyboard does, and what
        // every keyboard on the machine does.
        if d.kind == Kind::Keyboard {
            any = true;
            b.about_key(key::BLUETOOTH_ABOUT_THIS_KEYBOARD);
            b.door(
                b.texts.t(key::BLUETOOTH_DOOR_THIS_KEYBOARD),
                Target::Keyboards(d.modalias.clone().unwrap_or_default()),
            );
            b.about_key(key::BLUETOOTH_ABOUT_ALL_KEYBOARDS);
            b.door(
                b.texts.t(key::BLUETOOTH_DOOR_ALL_KEYBOARDS),
                Target::Keyboards(String::new()),
            );
        }

        if !any {
            b.note(key::BLUETOOTH_NOTHING_TO_ADJUST);
        }
    }

    /// A device's page: the facts, and what can be adjusted about it.
    ///
    /// # Why the halves are built separately and joined afterwards
    ///
    /// Because they are joined two different ways. Side by side, row `i` of
    /// each half shares a LINE, and only the half the keyboard is in carries
    /// the line's target — the other one is drawn and not acted on, which is
    /// what "the cursor is in one column" has to mean. Stacked, they are one
    /// list after another and every row keeps its own target.
    ///
    /// Either way each half is built with its OWN cursor, so the accent lands
    /// on the row somebody is on and on no other.
    fn device_rows(&self, d: &Device, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let facts = self.half(Pane::Facts, d, columns, texts);
        let settings = self.half(Pane::Settings, d, columns, texts);

        if !two_columns(columns) {
            // STACKED: one list, and the cursor's own index says which half
            // it is in. `half` was given the offset, so the accent is already
            // where it belongs.
            let mut out = facts;
            out.extend(settings);
            return out;
        }

        let tall = facts.len().max(settings.len());
        (0..tall)
            .map(|i| {
                let left = facts.get(i);
                let right = settings.get(i);
                let mine = match self.pane {
                    Pane::Facts => left,
                    Pane::Settings => right,
                };
                let mut fields = Vec::new();
                if let Some(l) = left {
                    fields.extend(l.fields.clone());
                }
                if let Some(r) = right {
                    fields.extend(r.fields.clone());
                }
                Row {
                    fields,
                    focused: mine.is_some_and(|m| m.focused),
                    // A LINE WHERE THE KEYBOARD'S HALF HAS NOTHING IS A
                    // HEADING, which is how the cursor skips it. Without this
                    // the arrows would stop on the blank beside a row of the
                    // other column, and `RET` there would do nothing while
                    // something was plainly drawn on the line.
                    heading: mine.is_none_or(|m| m.heading),
                    target: mine.map_or(Target::Nothing, |m| m.target.clone()),
                    disabled: mine.is_some_and(|m| m.disabled),
                    // THE SENTENCE OF THE HALF THE KEYBOARD IS IN, for the
                    // same reason its target is: pointing at a line asks about
                    // the column that acts.
                    description: mine.and_then(|m| m.description.clone()),
                }
            })
            .collect()
    }

    /// One half of a device's page, in its own cells and with its own cursor.
    fn half(&self, pane: Pane, d: &Device, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = extent_of(pane, columns);
        // WHOSE CURSOR THIS HALF CARRIES. Side by side it is the pane the
        // keyboard is in; stacked, the two halves are one list, so the second
        // one counts from where the first ended.
        let cursor = if two_columns(columns) {
            if self.pane == pane {
                self.cursor
            } else {
                usize::MAX
            }
        } else if pane == Pane::Facts {
            self.cursor
        } else {
            self.cursor
                .checked_sub(self.facts_len(d, columns, texts))
                .unwrap_or(usize::MAX)
        };
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor,
            texts,
            said: None,
        };
        match pane {
            Pane::Facts => self.detail(&mut b, d, texts),
            Pane::Settings => self.settings(&mut b, d, texts),
        }
        b.out
    }

    /// How many rows the facts take on the device this view is open on.
    fn facts_len_at(&self, scene: &Scene, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let Level::Device(at) = &self.level else {
            return 0;
        };
        scene
            .device(at)
            .map_or(0, |d| self.facts_len(d, columns, texts))
    }

    /// How many rows the facts take — what the stacked settings count from.
    fn facts_len(&self, d: &Device, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let (left, room) = extent_of(Pane::Facts, columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: usize::MAX,
            texts,
            said: None,
        };
        self.detail(&mut b, d, texts);
        b.out.len()
    }

    /// The radio's own settings.
    fn radio(&self, b: &mut Builder<'_>, scene: &Scene) {
        use crate::key;
        use ronoctua_i18n::Arg;
        b.heading(key::BLUETOOTH_HEADING_ADAPTER);
        let Some(a) = scene.adapter() else {
            b.note(key::BLUETOOTH_NO_ADAPTER);
            return;
        };
        let on_off = |on: bool| {
            b.texts.t(if on {
                key::BLUETOOTH_YES
            } else {
                key::BLUETOOTH_NO
            })
        };
        let state = b.texts.t(&format!(
            "bluetooth-power-state-{}",
            if a.power_state.is_empty() {
                if a.powered { "on" } else { "off" }
            } else {
                a.power_state.as_str()
            }
        ));
        b.pair(
            b.texts.t(key::BLUETOOTH_ADAPTER_POWER),
            state,
            Target::Power(!a.powered),
            false,
        );
        b.pair(
            b.texts.t(key::BLUETOOTH_ADAPTER_DISCOVERABLE),
            on_off(a.discoverable),
            Target::Discoverable(!a.discoverable),
            !a.powered,
        );
        // AND HOW LONG IT STAYS VISIBLE, which `RET` cycles: three minutes,
        // ten, an hour, for ever. It was a fact beside a switch that turns
        // visibility on — and the number that decides how long it lasts was
        // the one thing the screen would not let anybody touch.
        let timeout = if a.discoverable_timeout > 0 {
            b.texts.t_args(
                key::BLUETOOTH_SECONDS,
                &[("seconds", Arg::Number(i64::from(a.discoverable_timeout)))],
            )
        } else {
            b.texts.t(key::BLUETOOTH_ADAPTER_FOREVER)
        };
        b.pair(
            b.texts.t(key::BLUETOOTH_ADAPTER_TIMEOUT),
            timeout,
            Target::Timeout(next_timeout(a.discoverable_timeout)),
            !a.powered,
        );
        b.pair(
            b.texts.t(key::BLUETOOTH_ADAPTER_PAIRABLE),
            on_off(a.pairable),
            Target::Pairable(!a.pairable),
            !a.powered,
        );
        // AND HOW LONG PAIRING STAYS OPEN, on the same ring the visibility
        // one has. Two settings of the same shape, and only one of them was
        // on screen: `PairableTimeout` is `readwrite` on `Adapter1` and had
        // neither a row nor an order.
        let pairing_for = if a.pairable_timeout > 0 {
            b.texts.t_args(
                key::BLUETOOTH_SECONDS,
                &[("seconds", Arg::Number(i64::from(a.pairable_timeout)))],
            )
        } else {
            // ITS OWN WORD, and not the visibility row's: borrowing that one
            // put "open to pairing for: it stays visible" on the screen — two
            // rows about two different things, saying the same sentence, one
            // of them about the wrong thing.
            b.texts.t(key::BLUETOOTH_ADAPTER_PAIRABLE_FOREVER)
        };
        b.pair(
            b.texts.t(key::BLUETOOTH_ADAPTER_PAIRABLE_TIMEOUT),
            pairing_for,
            Target::PairableTimeout(next_timeout(a.pairable_timeout)),
            !a.powered,
        );
        b.pair(
            b.texts.t(key::BLUETOOTH_SCAN),
            on_off(a.discovering),
            Target::Scan(!a.discovering),
            !a.powered,
        );
        // WHAT THIS MACHINE IS CALLED, and it can be changed from here: the
        // row drew the name as a FACT while `Adapter::set_alias` sat in the
        // proxy with no caller at all — the one thing on this screen that
        // other people see, and the only one nobody could edit.
        b.pair(
            b.texts.t(key::BLUETOOTH_ADAPTER_ALIAS),
            a.alias.clone(),
            Target::Rename,
            false,
        );
        if !a.name.is_empty() && a.name != a.alias {
            b.fact(key::BLUETOOTH_FACT_NAME, a.name.clone());
        }
        b.fact(key::BLUETOOTH_FACT_ADDRESS, a.address.clone());
        // AND WHAT THE RADIO IS, which is what somebody looking up "does this
        // one do LE Audio" actually needs. Every one of these is on the bus
        // and none of them was drawn.
        if a.manufacturer > 0 {
            b.fact(key::BLUETOOTH_FACT_MANUFACTURER, a.manufacturer.to_string());
        }
        if a.version > 0 {
            b.fact(key::BLUETOOTH_FACT_VERSION, a.version.to_string());
        }
        if !a.modalias.is_empty() {
            b.fact(key::BLUETOOTH_FACT_MODALIAS, a.modalias.clone());
        }
        if !a.roles.is_empty() {
            b.fact(key::BLUETOOTH_FACT_ROLES, a.roles.join(" · "));
        }
        if a.class > 0 {
            b.fact(key::BLUETOOTH_FACT_CLASS, format!("{:#08x}", a.class));
        }
        // THE SERVICES THIS MACHINE OFFERS, by name where there is one — the
        // same table the device's own list reads.
        for uuid in &a.uuids {
            let said = ronoctua_bluetooth::profile_key(uuid)
                .map_or_else(|| uuid.clone(), |k| b.texts.t(k));
            b.fact(key::BLUETOOTH_FACT_SERVICE, said);
        }
    }
}

/// Builds the rows, keeping the arithmetic in one place.
struct Builder<'a> {
    out: Vec<Row>,
    left: u16,
    room: u16,
    cursor: usize,
    texts: &'a ronoctua_i18n::Texts,
    /// The sentence the NEXT row will carry.
    ///
    /// Staged rather than passed, because every row primitive would otherwise
    /// grow an argument that almost every caller passes `None` to — and a
    /// long tail of `None`s is how a parameter stops being read.
    said: Option<String>,
}

impl Builder<'_> {
    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        }
    }

    /// A heading whose words are the thing's own, not a key of ours.
    ///
    /// A device's page is titled with the device: the glyph for what it is,
    /// and the name it calls itself. It used to say `Bluetooth` — true of
    /// every screen in this view, and therefore of no use on any of them.
    fn heading_of(&mut self, text: String) {
        self.headed(text);
    }

    fn heading(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.headed(text);
    }

    fn headed(&mut self, text: String) {
        // AIR FIRST, and never before the first title. See `chrome::AIR`: a
        // section that starts immediately under the last row of the one above
        // does not start anywhere, and the owner read that off the screen.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                disabled: false,
                description: None,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                layout::elide_end(&text, self.room),
                tokens::color::TEXT_HEADING,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            disabled: false,
            description: None,
        });
    }

    /// A sentence that stands for a whole section — "nothing here".
    /// The sentence the NEXT row carries, for the line the footer explains
    /// things on.
    fn about(&mut self, text: String) {
        self.said = Some(text);
    }

    /// The same, out of a text key.
    fn about_key(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.about(text);
    }

    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.sentence(text);
    }

    /// # IT WRAPS, and it used to be cut
    ///
    /// One `Row` per line, through the one function the eight views of this
    /// shape share — see [`crate::chrome::note_lines`]. These sentences are
    /// about things nobody meets twice a year, which is the argument
    /// [`Row::description`] already makes; cut at the room's edge, the half
    /// that went was the end.
    ///
    /// # AND EVERY LINE IS THE SAME KIND OF ROW THE NOTE WAS
    ///
    /// The argument `power.rs::sentence` writes out in full: `RET` on an
    /// informative row copies what it says, so a line of a note is a line
    /// somebody can take, and a section whose only content is a note would
    /// leave its heading with nothing under it.
    ///
    /// THE DESCRIPTION GOES ON EVERY LINE. It is the long form of the note
    /// itself and it arrives by POINTING; pointing at the second line of a
    /// sentence and being told nothing would be a hole in the middle of a
    /// paragraph.
    fn sentence(&mut self, text: String) {
        let said = self.said.take();
        for line in crate::chrome::note_lines(&text, self.room) {
            let focused = self.cursor == self.out.len();
            self.out.push(Row {
                fields: vec![Self::piece(
                    // A SENTENCE LINES UP WITH THE LABELS, not with the titles.
                    // The mark's indent is paid by every row of these views on
                    // purpose — an indent only some rows pay is a ragged edge —
                    // and a note that skipped it sat two cells left of
                    // everything around it.
                    self.left,
                    line,
                    if focused {
                        tokens::color::STATE_ACCENT
                    } else {
                        tokens::color::TEXT_DIM
                    },
                )],
                focused,
                heading: false,
                target: Target::Nothing,
                disabled: false,
                description: said.clone(),
            });
        }
    }

    /// A label and a value, with nothing to do on `RET`.
    fn fact(&mut self, key: &str, value: String) {
        self.fact_coloured(key, value, None);
    }

    fn fact_coloured(&mut self, key: &str, value: String, colour: Option<tokens::Color>) {
        let label = self.texts.t(key);
        self.coloured(label, value, Target::Nothing, false, colour);
    }

    /// An id, its state, and a quiet gloss after it.
    ///
    /// # Why it does not go through `field_pair`
    ///
    /// That one puts the value at a fixed column and ELIDES the label to reach
    /// it — which is right for `trusted` beside `no`, and wrong for the only
    /// rows in this view whose label is the point. A service id is
    /// thirty-six characters, and cut at the value column it stops being an
    /// id: `0000110b-0000-1000-8000-00805f9b3` is not a thing anybody can
    /// look up. So here the label keeps what it needs and everything else
    /// follows it.
    ///
    /// Every id is the same length, so the values line up anyway.
    fn triple(
        &mut self,
        label: String,
        value: String,
        aside: String,
        target: Target,
        disabled: bool,
    ) {
        let focused = self.cursor == self.out.len();
        let kind = if disabled {
            crate::chrome::Kind::Disabled
        } else {
            crate::chrome::Kind::of(target != Target::Nothing)
        };
        let (label_colour, value_colour) = kind.tones(focused);
        let end = self.left + self.room;
        let mut fields = vec![Self::piece(
            self.left,
            layout::elide_end(&label, self.room),
            label_colour,
        )];
        let after = |at: u16, text: &str| at + ronoctua_protocol::cells(text) + 2;
        let at = after(self.left, &label);
        if at < end {
            fields.push(Self::piece(
                at,
                layout::elide_end(&value, end - at),
                value_colour,
            ));
            // AND THE GLOSS LAST, in the dim tone and only if there is room:
            // it is what we have to say ABOUT the id, not the id, and a
            // sentence that pushed the state off the line would have the
            // priorities backwards.
            let at = after(at, &value);
            if at < end && !aside.is_empty() {
                fields.push(Self::piece(
                    at,
                    layout::elide_end(&aside, end - at),
                    tokens::color::TEXT_DIM,
                ));
            }
        }
        self.out.push(Row {
            fields,
            focused,
            heading: false,
            target,
            disabled,
            description: self.said.take(),
        });
    }

    /// A row that is a WAY SOMEWHERE, and has no value to show.
    ///
    /// A `pair` would need one, and the first attempt gave it `opens` — which
    /// the owner read off the screen and could not parse: open what, and which
    /// keys? A door is a sentence and a target, and what it opens is said on
    /// the message line when the pointer is on it.
    fn door(&mut self, label: String, target: Target) {
        let focused = self.cursor == self.out.len();
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                layout::elide_end(&label, self.room),
                // A DOOR IS A SETTING'S LABEL with no value beside it.
                crate::chrome::Kind::Setting.tones(focused).0,
            )],
            focused,
            heading: false,
            target,
            disabled: false,
            description: self.said.take(),
        });
    }

    /// A row with something to do on `RET`.
    fn pair(&mut self, label: String, value: String, target: Target, disabled: bool) {
        self.coloured(label, value, target, disabled, None);
    }

    /// The one that draws them all — the power view's arithmetic, and the
    /// same reason: a short value sits at the right end and the label keeps
    /// the rest of the row; a long one takes the fact's third.
    fn coloured(
        &mut self,
        label: String,
        value: String,
        target: Target,
        disabled: bool,
        colour: Option<tokens::Color>,
    ) {
        let focused = self.cursor == self.out.len();
        // THE LABEL SAYS WHETHER THE ROW IS A DOOR, in a hue no value wears —
        // see `colors.nix`, `label`. It replaced a mark in front of the row,
        // which the owner refused: a chevron is what a tree uses to open and
        // close, and this row does neither.
        let kind = if disabled {
            crate::chrome::Kind::Disabled
        } else {
            crate::chrome::Kind::of(target != Target::Nothing)
        };
        // ONE ARITHMETIC, and it is `chrome::field_pair` — see the comment on
        // `chrome::value_column` for what the three copies of it had drifted
        // into.
        self.out.push(Row {
            fields: crate::chrome::tinted(
                crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
                colour,
                focused,
            ),
            focused,
            heading: false,
            target,
            disabled,
            description: self.said.take(),
        });
    }
}

/// The two sequences that answer a pairing question from anywhere, in the
/// full tree — the binding reads them.
///
/// The card that carries the question lives twenty seconds and the question
/// lives fifty-five, so the body names the keys — as ARGUMENTS, resolved
/// through `bindings::label()`, for the reason `notifications::NOTIF_OPEN_KEYS` gives. What
/// it names is [`crate::bindings::taught`]'s answer for the command and not this
/// constant: `s b` is the radio itself in the default set, so from there the
/// answer is the leader twice more and `s b y` (`SUPER SUPER s b y`).
pub const PAIRING_YES_KEYS: &str = "s b y";

/// The refusal's half of [`PAIRING_YES_KEYS`].
pub const PAIRING_NO_KEYS: &str = "s b n";

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "bluetooth",
    commands: &[
        // `LEADER s b` — THE RADIO, beside the machine's other hardware.
        //
        // `b` under `s` and not at the first level: what is under `s` is what
        // the MACHINE is doing (its memory, its disks, its power, its
        // network), and a radio with a keyboard on it is exactly that. The
        // group's own key is `s b b`, doubled the way `s n n`, `t t` and
        // `. .` are — a prefix cannot also be a command, and the tree refuses
        // it. That refusal is why the file manager's bare `.` had to become
        // `. .` the day `.` was asked to be a group. (`r r` was the network's,
        // and the same change that made this group killed the first-level
        // `r`.)
        // THE DEVICE PAGE'S THREE, which are the view's own arrows and the key
        // that crosses to the other half. They are in the registry for the
        // reason every view key is: the hint line is generated from the table
        // that names them, and a key the line teaches has to be a command the
        // manual can name.
        ronoctua_cmd::Command {
            id: "bluetooth-more",
            label: crate::key::CMD_BLUETOOTH_MORE,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-less",
            label: crate::key::CMD_BLUETOOTH_LESS,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-settings",
            label: crate::key::CMD_BLUETOOTH_SETTINGS,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-open",
            label: crate::key::CMD_BLUETOOTH_OPEN,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-window",
            label: crate::key::CMD_BLUETOOTH_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-scan",
            label: crate::key::CMD_BLUETOOTH_SCAN,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-power",
            label: crate::key::CMD_BLUETOOTH_POWER,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-discoverable",
            label: crate::key::CMD_BLUETOOTH_DISCOVERABLE,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-reconnect",
            label: crate::key::CMD_BLUETOOTH_RECONNECT,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-disconnect-all",
            label: crate::key::CMD_BLUETOOTH_DISCONNECT_ALL,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-accept",
            label: crate::key::CMD_BLUETOOTH_ACCEPT,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-refuse",
            label: crate::key::CMD_BLUETOOTH_REFUSE,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-device",
            label: crate::key::CMD_BLUETOOTH_DEVICE,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-connect",
            label: crate::key::CMD_BLUETOOTH_CONNECT,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-pair",
            label: crate::key::CMD_BLUETOOTH_PAIR,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-forget",
            label: crate::key::CMD_BLUETOOTH_FORGET,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-trust",
            label: crate::key::CMD_BLUETOOTH_TRUST,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-block",
            label: crate::key::CMD_BLUETOOTH_BLOCK,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-rename",
            label: crate::key::CMD_BLUETOOTH_RENAME,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-unnamed",
            label: crate::key::CMD_BLUETOOTH_UNNAMED,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-up",
            label: crate::key::CMD_BLUETOOTH_UP,
        },
        ronoctua_cmd::Command {
            id: "bluetooth-down",
            label: crate::key::CMD_BLUETOOTH_DOWN,
        },
    ],
    bindings: &[
        // THE RADIO. Nine keys, and the nine are the ones that act on the
        // machine rather than on a row: everything about ONE device is a
        // letter inside the view, where the row under the cursor says which
        // device the letter is about.
        ("s b b", "bluetooth-open", None),
        ("s b w", "bluetooth-window", None),
        ("s b s", "bluetooth-scan", None),
        ("s b p", "bluetooth-power", None),
        ("s b d", "bluetooth-discoverable", None),
        ("s b c", "bluetooth-reconnect", None),
        ("s b x", "bluetooth-disconnect-all", None),
        // THE TWO ANSWERS, reachable from anywhere: a pairing question can
        // arrive while one is somewhere else entirely, and the two keys that
        // answer it are the brain's `b a`/`b p` arrangement.
        (PAIRING_YES_KEYS, "bluetooth-accept", None),
        (PAIRING_NO_KEYS, "bluetooth-refuse", None),
    ],
    groups: &[("s b", crate::key::GROUP_BLUETOOTH)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::bluetooth::OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bluez_words::{POOL, modeline};
    use ronoctua_bluetooth::{Adapter, Scene};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn a_device(at: &str, kind: Kind, connected: bool, battery: Option<u8>) -> Device {
        Device {
            at: at.to_owned(),
            address: "00:00:00:00:00:00".to_owned(),
            label: format!("a {}", kind.id()),
            name: format!("a {}", kind.id()),
            kind,
            connected,
            paired: true,
            bonded: true,
            battery,
            uuids: vec!["0000110b-0000-1000-8000-00805f9b34fb".to_owned()],
            // EVERY DEVICE THE OWNER OWNS CARRIES THIS, and it is what the
            // settings half is built out of when there is no audio card:
            // `pw-dump` cannot run in a test.
            wake: Some(false),
            ..Device::default()
        }
    }

    #[test]
    fn the_rows_are_the_same_things_at_every_width_of_one_layout() {
        // `View::key` decides against the rows the DOOR drew, so a key must
        // never act on a different row from the one on screen. That used to
        // mean "the rows are the same at every width", and it was honest
        // while every screen was one column.
        //
        // A DEVICE'S PAGE NOW HAS TWO LAYOUTS. Above
        // `space.bluetooth.settingsFrom` the settings sit BESIDE the facts and
        // the two halves share lines; below it they stack, and the list is one
        // after the other. Those are different row lists on purpose — so the
        // invariant is asserted inside each layout, and the width is passed to
        // `key` for exactly this reason.
        //
        // WHAT IS COMPARED IS WHAT CAN BE ACTED ON, and in what order. The row
        // COUNT is allowed to differ: a note WRAPS (`chrome::note_lines`), so
        // one sentence is a row on a wide screen and three on a narrow one. It
        // used to be cut instead, which kept the count the same and took the
        // end off every explanation this view has.
        //
        // The FIELDS are allowed to differ too — that is what eliding is.
        let t = texts();
        let scene = a_scene();
        for level in [View::fresh(), View::fresh_at("/d/1")] {
            let want = |columns: u16| -> Vec<(bool, Target)> {
                level
                    .clone()
                    .rows(&scene, columns, &t)
                    .into_iter()
                    .filter(|r| r.target != Target::Nothing)
                    .map(|r| (r.disabled, r.target))
                    .collect()
            };
            // STACKED, and the deciding width is one of them.
            let narrow = want(DECIDING_WIDTH);
            for columns in [40_u16, 60, 87] {
                assert_eq!(want(columns), narrow, "the rows differ at {columns}");
            }
            // AND SIDE BY SIDE.
            let wide = want(120);
            for columns in [88_u16, 160, 240] {
                assert_eq!(want(columns), wide, "the rows differ at {columns}");
            }
        }
    }

    #[test]
    #[ignore = "a drawing, for reading with --nocapture"]
    fn look_at_the_device_page() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh_at("/d/1");
        v.settings = true;
        v.sink = Some(ronoctua_bluetooth::audio::Sink {
            id: 83,
            volume: 72,
            muted: false,
        });
        v.card = Some(ronoctua_bluetooth::audio::Card {
            id: 84,
            active: 1,
            profiles: vec![
                ronoctua_bluetooth::audio::Profile {
                    index: 1,
                    name: "a2dp-sink".to_owned(),
                    available: true,
                },
                ronoctua_bluetooth::audio::Profile {
                    index: 2,
                    name: "headset-head-unit".to_owned(),
                    available: true,
                },
            ],
        });
        // AND A KEYBOARD, which is the other shape the right-hand column
        // takes: two doors and no values.
        let mut k = View::fresh_at("/d/2");
        k.settings = true;
        println!("\n== A KEYBOARD, side by side ==");
        for (n, r) in k.rows(&scene, 120, &t).into_iter().enumerate() {
            let mut line = vec![' '; 128];
            for f in &r.fields {
                for (i, c) in f.text.chars().enumerate() {
                    if usize::from(f.at) + i < line.len() {
                        line[usize::from(f.at) + i] = c;
                    }
                }
            }
            println!(
                "{n:>2}|{}|{}",
                line.into_iter().collect::<String>().trim_end(),
                r.description.as_deref().map_or(String::new(), |d| format!(
                    "   ← {}",
                    d.chars().take(64).collect::<String>()
                ))
            );
        }

        for (label, columns, pane) in [
            ("STACKED, 80 columns", 80_u16, Pane::Facts),
            ("SIDE BY SIDE, 120 columns", 120, Pane::Facts),
            ("SIDE BY SIDE, the keyboard crossed", 120, Pane::Settings),
        ] {
            v.pane = pane;
            println!("\n== {label} ==");
            for (n, r) in v.rows(&scene, columns, &t).into_iter().enumerate() {
                let mut line = vec![' '; usize::from(columns) + 8];
                for f in &r.fields {
                    for (i, c) in f.text.chars().enumerate() {
                        let at = usize::from(f.at) + i;
                        if at < line.len() {
                            line[at] = c;
                        }
                    }
                }
                let mark = if r.focused {
                    ">"
                } else if r.heading {
                    "#"
                } else {
                    " "
                };
                println!(
                    "{n:>2}{mark}|{}|",
                    line.into_iter().collect::<String>().trim_end()
                );
            }
        }
    }

    #[test]
    fn the_two_layouts_hold_the_same_things() {
        // The halves are joined two different ways, and neither may LOSE a
        // row: stacked they are one list after another, side by side they
        // share lines. A device with more facts than settings — every device
        // — would hide the whole settings column if the merge counted the
        // wrong half.
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh_at("/d/1");
        // A SINK, because `pw-dump` cannot run in a test and the volume is
        // what the right-hand column holds for a headset. It is also the whole
        // point of that column: BlueZ has no volume to give, so this row
        // exists only because something OUTSIDE it was asked.
        v.settings = true;
        v.sink = Some(ronoctua_bluetooth::audio::Sink {
            id: 83,
            volume: 72,
            muted: false,
        });
        let stacked = v.rows(&scene, 80, &t).len();
        let beside = v.rows(&scene, 120, &t).len();
        // SIDE BY SIDE IS SHORTER BY WHAT THE SHORTER HALF HOLDS: the two
        // lists share lines instead of following one another.
        assert!(
            beside < stacked,
            "side by side ({beside}) is not shorter than stacked ({stacked})"
        );
        let has_volume = |v: &View, columns: u16| {
            v.rows(&scene, columns, &t)
                .iter()
                .any(|r| matches!(r.target, Target::Volume(_, _)))
        };
        // STACKED, EVERY ROW KEEPS ITS OWN TARGET: one list, one cursor, and
        // the volume is reachable by walking down.
        assert!(
            has_volume(&v, 80),
            "stacked, the settings are not reachable"
        );

        // SIDE BY SIDE, ONLY THE HALF THE KEYBOARD IS IN CARRIES THE LINE'S
        // TARGET. That is what "the cursor is in one column" has to mean: the
        // other column is drawn and not acted on, so `RET` cannot turn
        // something the eye is not on.
        assert!(
            !has_volume(&v, 120),
            "the settings act while the keyboard is on the facts"
        );
        v.pane = Pane::Settings;
        assert!(
            has_volume(&v, 120),
            "crossing to the settings does not make them act"
        );
        // AND THE FACTS STOP ACTING WHILE IT IS THERE — the same rule, read
        // from the other side.
        assert!(
            !v.rows(&scene, 120, &t)
                .iter()
                .any(|r| matches!(r.target, Target::Profile { .. })),
            "the facts act while the keyboard is on the settings"
        );
    }

    #[test]
    fn every_notation_is_spelt_the_way_the_keymap_writes_it() {
        // BOTH DOORS LOOK THIS TABLE UP BY STRING, and the string they
        // compare against is what `Key::notation` produced from the key that
        // was actually pressed. A row spelt any other way — `Shift-<right>`,
        // `<Right>` — matches nothing: the key is dead in both doors, and
        // `hint_of` drops it from the line as well, so the only symptom is a
        // sentence one item shorter than the table.
        for (notation, id, _) in OF_THE_VIEW {
            let key = ronoctua_keymap::Key::parse(notation).unwrap_or_else(|_| {
                panic!("`{notation}` ({id}) is not a key this repository can read")
            });
            assert_eq!(
                &key.notation(),
                notation,
                "`{notation}` ({id}) is not how `Key::notation` writes that key"
            );
        }
    }

    #[test]
    fn the_sideways_arrows_cross_and_the_shifted_pair_acts() {
        // THE OWNER'S RULE, which is the file manager's: the bare arrows move
        // between the two columns — `TAB` still does too, for whoever arrives
        // from the other manager — and `S-<arrow>` is what moves the value of
        // the row the keyboard is on. They were the other way round, and
        // reaching the second column meant knowing about `TAB`.
        let press = |notation: &str| {
            OF_THE_VIEW
                .iter()
                .find(|(t, _, _)| *t == notation)
                .and_then(|(_, id, _)| crate::bluetooth_words::press_of(id))
        };
        for crossing in ["<right>", "<left>", "TAB", "S-TAB"] {
            assert_eq!(
                press(crossing),
                Some(Press::Cross),
                "`{crossing}` does not cross to the other column"
            );
        }
        assert_eq!(press("S-<right>"), Some(Press::More));
        assert_eq!(press("S-<left>"), Some(Press::Less));
    }

    #[test]
    fn crossing_and_stepping_are_two_different_keys() {
        // The pair above, from the other end: what each one does to the view.
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh_at("/d/1");
        v.settings = true;
        v.sink = Some(ronoctua_bluetooth::audio::Sink {
            id: 83,
            volume: 72,
            muted: false,
        });
        assert_eq!(v.pane, Pane::Facts);
        assert_eq!(v.key(Press::Cross, &scene, 120, &t), Errand::Nothing);
        assert_eq!(v.pane, Pane::Settings, "crossing did not move the keyboard");

        let rows = v.rows(&scene, 120, &t);
        let at = rows
            .iter()
            .position(|r| matches!(r.target, Target::Volume(_, _)))
            .expect("the volume is drawn in the settings column");
        v.put(at, &rows);
        let Errand::Volume(node, wanted) = v.key(Press::More, &scene, 120, &t) else {
            panic!("`S-<right>` on the volume did not ask for one");
        };
        assert_eq!(node, 83, "the volume was sent to the card and not the node");
        assert!(wanted > 72, "the volume did not go up");

        // AND CROSSING BACK LEAVES THE NUMBER ALONE. While the bare arrow was
        // "more", these two were the same key; now one of them must not do
        // the other's work.
        let before = v.sink.expect("the sink is held").volume;
        assert_eq!(v.key(Press::Cross, &scene, 120, &t), Errand::Nothing);
        assert_eq!(v.pane, Pane::Facts);
        assert_eq!(v.sink.expect("the sink is held").volume, before);
    }

    #[test]
    fn a_click_acts_on_the_column_it_landed_in() {
        // WHAT THE TWO DOORS DO WITH A PRESS OF THE MOUSE, in the order they
        // do it: `aim` puts the keyboard in the half the pointer is over,
        // `clicked` puts the cursor on the row, and then `RET`. Without the
        // aim the click would move a row of the OTHER column; without the
        // `RET` it would take a hand back on the keyboard to do the thing.
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh_at("/d/1");
        v.settings = true;
        v.sink = Some(ronoctua_bluetooth::audio::Sink {
            id: 83,
            volume: 72,
            muted: false,
        });
        let cell = split_at(120) + 2;
        v.aim(Some(cell), 120);
        assert_eq!(v.pane, Pane::Settings, "the pointer did not reach the half");
        let rows = v.rows(&scene, 120, &t);
        let at = rows
            .iter()
            .position(|r| matches!(r.target, Target::Volume(_, _)))
            .expect("the volume is drawn in the settings column");
        v.clicked(at, Some(cell), 120, &rows);
        let Errand::Volume(node, _) = v.key(Press::Enter, &scene, 120, &t) else {
            panic!("the clicked row did not answer `RET`");
        };
        assert_eq!(node, 83);

        // AND A CLICK BACK ON THE LEFT-HAND HALF acts on THAT one — the same
        // two calls, the other side of the line.
        v.aim(Some(1), 120);
        assert_eq!(v.pane, Pane::Facts);

        // `clicked` AIMS AS WELL, and that is not a duplicate: the doors ask
        // first because the rows have to be BUILT for the right half, and
        // this second call is what keeps a caller that forgets from leaving
        // the cursor in the column the pointer is not over.
        v.clicked(at, Some(cell), 120, &rows);
        assert_eq!(v.pane, Pane::Settings);
    }

    #[test]
    fn the_settings_column_adds_and_never_moves() {
        // WHAT THE RIGHT-HAND COLUMN IS FOR, and the owner said it from the
        // screen: only what this KIND of thing adds, and never a row taken out
        // of the facts. The wake switch and the audio profile were moved there
        // for an afternoon and sent back — both are properties of `Device1`,
        // and a column that empties its neighbour has added nothing.
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh_at("/d/1");
        v.settings = true;
        v.card = Some(ronoctua_bluetooth::audio::Card {
            id: 84,
            active: 1,
            profiles: vec![
                ronoctua_bluetooth::audio::Profile {
                    index: 1,
                    name: "a2dp-sink".to_owned(),
                    available: true,
                },
                ronoctua_bluetooth::audio::Profile {
                    index: 2,
                    name: "headset-head-unit".to_owned(),
                    available: true,
                },
            ],
        });
        // STACKED, so both halves are in one list and every row keeps its own
        // target — which is what lets this ask where each one IS.
        let rows = v.rows(&scene, 80, &t);
        let facts = v.facts_len_at(&scene, 80, &t);
        let at = |wanted: fn(&Target) -> bool| rows.iter().position(|r| wanted(&r.target));

        let wake = at(|t| matches!(t, Target::Wake(_, _))).expect("the wake switch is drawn");
        let audio = at(|t| matches!(t, Target::Audio(_, _))).expect("the audio profile is drawn");
        assert!(wake < facts, "the wake switch left the facts");
        assert!(audio < facts, "the audio profile left the facts");
    }

    /// FAILS IF: a phone that shares its internet has no row for it, the
    /// row does not name the interface while the link is up, or `RET` on it
    /// is not the errand both doors hand to NetworkManager.
    #[test]
    fn a_phone_that_shares_its_internet_has_a_switch_naming_bnep0() {
        let t = texts();
        let mut phone = a_device("/d/9", Kind::Phone, true, None);
        phone.address = "00:00:00:00:00:09".to_owned();
        phone.uuids = vec![ronoctua_bluetooth::NAP.to_owned()];
        let mut scene = a_scene();
        scene.devices.push(phone.clone());
        let mut v = View::fresh_at("/d/9");
        let rows = v.rows(&scene, 80, &t);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Tether(phone.address.clone(), true))
            .expect("the internet switch is drawn");
        v.cursor = at;
        assert_eq!(
            v.key(Press::Enter, &scene, 80, &t),
            Errand::Tether {
                address: phone.address.clone(),
                name: phone.label.clone(),
                on: true,
            }
        );
        // UP, IT NAMES THE INTERFACE — the one the network's view lists —
        // and `RET` takes it down.
        phone.pan = Some(ronoctua_bluetooth::Pan {
            connected: true,
            interface: "bnep0".to_owned(),
        });
        scene.devices.pop();
        scene.devices.push(phone.clone());
        let rows = View::fresh_at("/d/9").rows(&scene, 80, &t);
        let row = rows
            .iter()
            .find(|r| r.target == Target::Tether(phone.address.clone(), false))
            .expect("the switch, up");
        assert!(row.fields.iter().any(|f| f.text.contains("bnep0")));
        // AND A DEVICE THAT SHARES NOTHING has no such row.
        let rows = View::fresh_at("/d/1").rows(&a_scene(), 80, &t);
        assert!(!rows.iter().any(|r| matches!(r.target, Target::Tether(..))));
    }

    fn a_scene() -> Scene {
        Scene {
            adapters: vec![Adapter {
                at: "/org/bluez/hci0".to_owned(),
                address: "00:00:00:00:00:01".to_owned(),
                alias: "this machine".to_owned(),
                powered: true,
                power_state: "on".to_owned(),
                pairable: true,
                ..Adapter::default()
            }],
            devices: vec![
                a_device("/d/1", Kind::Headphones, true, Some(80)),
                a_device("/d/2", Kind::Keyboard, true, Some(12)),
                Device {
                    at: "/d/3".to_owned(),
                    label: "00:00:00:00:00:09".to_owned(),
                    ..Device::default()
                },
            ],
            daemon: true,
        }
    }

    #[test]
    fn every_key_the_line_teaches_does_something() {
        // THE RULE, and it had no test in this view: a key the hint line
        // teaches has to be a key the view answers to. `notifications.rs` and
        // `files.rs` each hold themselves to it; the three views this work
        // added proved only that every id had a text and that the line
        // mentioned it — never that pressing it did anything.
        //
        // "Does something" is an errand, or a cursor that moved, or a level
        // that changed: a walking key answers `Nothing` and MOVES.
        let t = texts();
        // AND A STRANGER WITH A NAME, so that "Nearby" has a row one can
        // stand on: the fixture's only unpaired device has no name at all,
        // which means it is hidden until `h` — and `p` would have nothing to
        // act on anywhere, which is the fixture saying nothing rather than
        // the key doing nothing.
        let mut scene = a_scene();
        scene.devices.push(Device {
            at: "/d/4".to_owned(),
            address: "00:00:00:00:00:04".to_owned(),
            label: "a speaker".to_owned(),
            name: "a speaker".to_owned(),
            kind: Kind::Speaker,
            ..Device::default()
        });
        for (notation, id, column) in OF_THE_VIEW {
            let Some(press) = crate::bluetooth_words::press_of(id) else {
                panic!("`{id}` is taught and resolves to no press");
            };
            // ON A SCREEN WHERE IT MEANS SOMETHING. `Where` is what the line
            // reads, so it is what this reads too — a key taught nowhere is
            // the one thing that would make this vacuous.
            let v = match column {
                Where::OnTheRadio => {
                    let mut v = View::fresh();
                    v.level = Level::Radio;
                    v
                }
                Where::OnTheSettings | Where::OnADevice | Where::OnTheList | Where::Anywhere => {
                    let mut v = View::fresh();
                    // A CARD, because `pw-dump` cannot run in a test and the
                    // arrows act on what it read. Two profiles, so the ring
                    // has somewhere to go — and the settings half exists,
                    // which is what `TAB` and the arrows are taught on.
                    if *column == Where::OnTheSettings {
                        v.level = Level::Device("/d/1".to_owned());
                        v.settings = true;
                    }
                    v.card = Some(ronoctua_bluetooth::audio::Card {
                        id: 42,
                        active: 1,
                        profiles: vec![
                            ronoctua_bluetooth::audio::Profile {
                                index: 0,
                                name: "off".to_owned(),
                                available: true,
                            },
                            ronoctua_bluetooth::audio::Profile {
                                index: 1,
                                name: "a2dp-sink".to_owned(),
                                available: true,
                            },
                        ],
                    });
                    v
                }
                Where::WhileAsked => {
                    let mut v = View::fresh();
                    v.asked(agent::Question {
                        device: "/d/1".to_owned(),
                        asked: agent::Asked::Confirm("012345".to_owned()),
                    });
                    v
                }
            };
            // ON EVERY ROW OF THAT SCREEN, and it has to work on at least
            // ONE. A device key is honest about the row it is on — `p` does
            // nothing on a device that is already paired, and should — so the
            // property is "somewhere on the screen the line teaches it", not
            // "on whichever row the cursor happened to land".
            //
            // The first press from row one also gives `<up>` somewhere to go.
            let rows = v.rows(&scene, 80, &t);
            let mut worked = false;
            for row in 0..rows.len() {
                let mut here = v.clone();
                here.clicked(row, None, DECIDING_WIDTH, &rows);
                let (was, level, unnamed) = (here.cursor(), here.level.clone(), here.unnamed);
                let did = here.key(press, &scene, DECIDING_WIDTH, &t);
                worked |= did != Errand::Nothing
                    || here.cursor() != was
                    || here.level != level
                    || here.unnamed != unnamed;
            }
            assert!(
                worked,
                "`{notation}` ({id}) does nothing on any row of the screen the line teaches it"
            );
        }
    }

    #[test]
    fn every_service_this_table_names_has_a_sentence() {
        // A missing `.ftl` entry for any of these puts
        // `[bluetooth-profile-sim-access]` on screen, in a list nobody audits.
        //
        // THE IDS COME FROM THE TABLE ITSELF, and the table is now GENERATED
        // from `assets/bluetooth-profiles.nix`. This used to parse the body of
        // `profile_key` out of the crate's source, which worked only while the
        // table was a `match` written by hand.
        let t = texts();
        let table = ronoctua_bluetooth::PROFILES;
        assert!(
            table.len() >= 25,
            "only {} services in the table",
            table.len()
        );
        let mut named = 0;
        for (short, key) in table {
            let uuid = format!("0000{short}-0000-1000-8000-00805f9b34fb");
            match key {
                Some(key) => {
                    named += 1;
                    assert_eq!(
                        ronoctua_bluetooth::profile_key(&uuid),
                        Some(*key),
                        "`{uuid}` is in the table and does not resolve to its own key"
                    );
                    assert!(
                        !t.t(key).starts_with('['),
                        "`{key}` names a service and has no sentence"
                    );
                }
                // The plumbing: known, and deliberately not drawn. It owes no
                // sentence — and it has to be told apart from an id nobody
                // named, which IS drawn, raw.
                None => {
                    assert!(ronoctua_bluetooth::profile_is_plumbing(&uuid));
                    assert_eq!(ronoctua_bluetooth::profile_key(&uuid), None);
                }
            }
        }
        assert!(named >= 20, "only {named} services carry a sentence");
    }

    #[test]
    fn the_hint_teaches_only_the_keys_that_do_something_here() {
        let t = texts();
        let words = |v: &View| -> String {
            v.hint(&t)
                .iter()
                .map(|p| p.text.clone())
                .collect::<Vec<_>>()
                .join(" ")
        };

        // THE LIST teaches everything but the two answers.
        let list = words(&View::fresh());
        for id in ["bluetooth-connect", "bluetooth-scan", "bluetooth-unnamed"] {
            assert!(list.contains(&t.t(id)), "the list should teach `{id}`");
        }
        assert!(!list.contains(&t.t("bluetooth-accept")));

        // THE RADIO'S SCREEN has no device, so the six that act on one do
        // nothing there — and the owner photographed them being taught.
        let mut radio = View::fresh();
        radio.level = Level::Radio;
        let said = words(&radio);
        for id in [
            "bluetooth-connect",
            "bluetooth-pair",
            "bluetooth-forget",
            "bluetooth-trust",
            "bluetooth-block",
            "bluetooth-rename",
            "bluetooth-unnamed",
        ] {
            assert!(
                !said.contains(&t.t(id)),
                "the radio's screen should not teach `{id}`"
            );
        }
        assert!(said.contains(&t.t("bluetooth-power")));

        // A DEVICE'S SCREEN is the other way round.
        let mut one = View::fresh_at("/d/1");
        one.level = Level::Device("/d/1".to_owned());
        let said = words(&one);
        assert!(said.contains(&t.t("bluetooth-connect")));
        assert!(!said.contains(&t.t("bluetooth-scan")));
        // AND A DEVICE WITH NOTHING TO ADJUST IS NOT OFFERED THE OTHER HALF.
        //
        // The owner's own keyboard, photographed: `→ the next audio profile`
        // on the page of a thing that has none. `OnADevice` is about WHICH
        // SCREEN and this is about what the device on it can do — which is
        // now a column of its own, and the three keys that reach it are
        // taught together or not at all.
        for id in ["bluetooth-settings", "bluetooth-more", "bluetooth-less"] {
            assert!(!said.contains(&t.t(id)), "`{id}` on a page with one half");
        }
        // `settings` is what `look_at_the_device` learns; a test cannot run
        // `pw-dump`, so it says the same thing by hand.
        one.settings = true;
        let now = words(&one);
        for id in ["bluetooth-settings", "bluetooth-more", "bluetooth-less"] {
            assert!(now.contains(&t.t(id)), "`{id}` missing where it acts");
        }

        // AND A QUESTION TAKES THE SCREEN: two keys, and no others.
        let mut asked = View::fresh();
        asked.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Confirm("012345".to_owned()),
        });
        let said = words(&asked);
        assert!(said.contains(&t.t("bluetooth-accept")));
        assert!(said.contains(&t.t("bluetooth-refuse")));
        assert!(!said.contains(&t.t("bluetooth-forget")));
        assert!(!said.contains(&t.t("bluetooth-scan")));
    }

    #[test]
    fn the_cursor_stays_on_the_device_when_the_list_reorders_under_it() {
        // THE DEFECT THIS PINS: the cursor was an index, and `paired()` sorts
        // by connected, then kind, then label. A device connecting moves every
        // row below it, so the cursor ended up on a device nobody aimed at —
        // and the very next `f` forgot THAT one, keys and all.
        let t = texts();
        let mut scene = a_scene();
        // A speaker that is NOT connected, so it sorts below the two that are.
        scene
            .devices
            .push(a_device("/d/4", Kind::Speaker, false, None));
        let mut v = View::fresh();
        let rows = v.rows(&scene, 80, &t);
        v.settle(&rows);
        // Walk to the speaker.
        let speaker = rows
            .iter()
            .position(|r| r.target == Target::Device("/d/4".to_owned()))
            .expect("the speaker has a row");
        v.clicked(speaker, None, DECIDING_WIDTH, &rows);
        assert_eq!(v.cursor(), speaker);

        // NOW IT CONNECTS, which lifts it above every disconnected one and
        // pushes the rows around it.
        for d in &mut scene.devices {
            if d.at == "/d/4" {
                d.connected = true;
            }
        }
        let after = v.rows(&scene, 80, &t);
        v.settle(&after);
        assert_ne!(
            v.cursor(),
            speaker,
            "the test is not testing anything if the row did not move"
        );
        assert_eq!(
            after[v.cursor()].target,
            Target::Device("/d/4".to_owned()),
            "the cursor let go of the device it was on"
        );
    }

    #[test]
    fn a_device_that_goes_away_leaves_the_cursor_on_a_row_that_exists() {
        let t = texts();
        let mut scene = a_scene();
        let mut v = View::fresh();
        let rows = v.rows(&scene, 80, &t);
        v.settle(&rows);
        let at = rows
            .iter()
            .position(|r| r.target == Target::Device("/d/2".to_owned()))
            .expect("the keyboard has a row");
        v.clicked(at, None, DECIDING_WIDTH, &rows);
        scene.devices.retain(|d| d.at != "/d/2");
        let after = v.rows(&scene, 80, &t);
        v.settle(&after);
        assert!(v.cursor() < after.len());
        assert!(!after[v.cursor()].heading, "never on a heading");
    }

    #[test]
    fn the_modeline_draws_what_is_connected_and_the_owners_two_numbers() {
        let scene = a_scene();
        let shown = modeline(&scene);
        assert_eq!(shown.len(), 2, "only the connected ones");
        // The alarmed one comes first.
        assert_eq!(shown[0].at, "/d/2");
        assert!(shown[0].alarmed);
        assert!(shown[0].text.contains("12%"), "{}", shown[0].text);
        assert!(shown[0].text.starts_with(glyph(Kind::Keyboard)));
        // And the one with a healthy battery is only its glyph.
        assert!(!shown[1].alarmed);
        assert_eq!(shown[1].text, glyph(Kind::Headphones).to_string());
    }

    #[test]
    fn the_pool_holds_four_and_the_fifth_is_in_the_view() {
        let mut scene = a_scene();
        for i in 4..9 {
            scene
                .devices
                .push(a_device(&format!("/d/{i}"), Kind::Mouse, true, None));
        }
        assert_eq!(modeline(&scene).len(), POOL.len());
        assert_eq!(scene.connected().len(), 7, "the view still has them all");
    }

    #[test]
    fn nothing_connected_draws_nothing() {
        let scene = Scene::default();
        assert!(modeline(&scene).is_empty());
    }

    #[test]
    fn every_kind_has_a_glyph_of_its_own() {
        let mut seen = std::collections::BTreeSet::new();
        for k in Kind::ALL {
            assert!(seen.insert(glyph(k)), "{k:?} shares a glyph");
        }
    }

    #[test]
    fn the_list_has_the_three_sections_and_says_when_one_is_empty() {
        let t = texts();
        let v = View::fresh();
        let rows = v.rows(&a_scene(), 80, &t);
        // TITLES, not the air before them — see `chrome::AIR`.
        let headings = rows
            .iter()
            .filter(|r| r.heading && !r.fields.is_empty())
            .count();
        assert_eq!(headings, 3, "radio, paired, nearby");
        // The unnamed stranger is hidden until `h`.
        let mut open = View::fresh();
        let nearby_shown = |v: &View| {
            v.rows(&a_scene(), 80, &t)
                .iter()
                .filter(|r| matches!(&r.target, Target::Device(at) if at == "/d/3"))
                .count()
        };
        assert_eq!(nearby_shown(&open), 0);
        let _ = open.key(Press::Unnamed, &a_scene(), DECIDING_WIDTH, &t);
        assert_eq!(nearby_shown(&open), 1);
    }

    #[test]
    fn an_empty_scene_says_which_of_the_two_things_is_missing() {
        let t = texts();
        let v = View::fresh();
        let no_daemon = v.rows(&Scene::default(), 80, &t);
        let said = |rows: &[Row]| {
            rows.iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join("|")
        };
        assert!(said(&no_daemon).contains(&t.t(crate::key::BLUETOOTH_NO_DAEMON)));
        let running = Scene {
            daemon: true,
            ..Scene::default()
        };
        assert!(said(&v.rows(&running, 80, &t)).contains(&t.t(crate::key::BLUETOOTH_NO_ADAPTER)));
    }

    #[test]
    fn enter_opens_a_device_and_escape_walks_back() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        let rows = v.rows(&scene, 80, &t);
        let row = rows
            .iter()
            .position(|r| matches!(&r.target, Target::Device(at) if at == "/d/1"))
            .unwrap();
        v.clicked(row, None, DECIDING_WIDTH, &rows);
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert_eq!(v.level(), &Level::Device("/d/1".to_owned()));
        // The detail has the facts and the services.
        let detail = v.rows(&scene, 80, &t);
        assert!(detail.len() > 8, "the detail is one fact per line");
        assert!(
            detail
                .iter()
                .any(|r| matches!(r.target, Target::Profile { .. }))
        );
        // Esc goes back to the list, and only then closes.
        assert_eq!(
            v.key(Press::Escape, &scene, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert_eq!(v.level(), &Level::List);
        assert_eq!(
            v.key(Press::Escape, &scene, DECIDING_WIDTH, &t),
            Errand::Close
        );
    }

    #[test]
    fn a_letter_acts_on_the_device_under_the_cursor() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        let rows = v.rows(&scene, 80, &t);
        let row = rows
            .iter()
            .position(|r| matches!(&r.target, Target::Device(at) if at == "/d/1"))
            .unwrap();
        v.clicked(row, None, DECIDING_WIDTH, &rows);
        // It is connected, so `c` drops the link.
        assert_eq!(
            v.key(Press::Connect, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Disconnect("/d/1".to_owned()))
        );
        assert_eq!(
            v.key(Press::Trust, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Trust("/d/1".to_owned(), true))
        );
        assert_eq!(
            v.key(Press::Block, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Block("/d/1".to_owned(), true))
        );
        assert_eq!(
            v.key(Press::Forget, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Forget("/d/1".to_owned()))
        );
        assert_eq!(
            v.key(Press::Rename, &scene, DECIDING_WIDTH, &t),
            Errand::Rename("/d/1".to_owned())
        );
        // And inside the detail the same letters are about the same device.
        let _ = v.key(Press::Enter, &scene, DECIDING_WIDTH, &t);
        assert_eq!(
            v.key(Press::Connect, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Disconnect("/d/1".to_owned()))
        );
    }

    #[test]
    fn the_radio_keys_read_the_radio_and_flip_it() {
        let t = texts();
        let mut scene = a_scene();
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Power, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Power(false)),
            "it is on, so `o` turns it off"
        );
        assert_eq!(
            v.key(Press::Scan, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Scan(true))
        );
        scene.adapters[0].discovering = true;
        assert_eq!(
            v.key(Press::Scan, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Scan(false))
        );
        assert_eq!(
            v.key(Press::Discoverable, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::Discoverable(true))
        );
    }

    #[test]
    fn a_question_takes_the_screen_until_it_is_answered() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Confirm("012345".to_owned()),
        });
        assert!(v.asking());
        let rows = v.rows(&scene, 80, &t);
        let said = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("|");
        assert!(said.contains("012345"), "{said}");
        // The hint leads with the two answers.
        let hint = v.hint(&t);
        assert!(!hint.is_empty());
        // Every other key does nothing while it stands.
        assert_eq!(
            v.key(Press::Connect, &scene, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert!(v.asking(), "a letter answered the question");
        // And `n` says no.
        assert_eq!(
            v.key(Press::Refuse, &scene, DECIDING_WIDTH, &t),
            Errand::Answer(false)
        );
        assert!(!v.asking());
        // AND `RET` ANSWERS THE ROW: on "no" it says no, which is the whole
        // reason the prompt draws two rows rather than one.
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Confirm("012345".to_owned()),
        });
        let rows = v.rows(&scene, 80, &t);
        let no = rows
            .iter()
            .position(|r| r.target == Target::Answer(false))
            .unwrap();
        v.clicked(no, None, DECIDING_WIDTH, &rows);
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Answer(false)
        );

        // AND AN UNTOUCHED QUESTION ANSWERS NO.
        //
        // THE DEFECT THIS PREVENTS: the cursor started on the SENTENCE, whose
        // target is `Nothing`, and every target but `Answer(false)` fell
        // through to yes — so a request that opened itself over whatever was
        // on screen was one `RET` away from pairing an unknown device.
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Confirm("012345".to_owned()),
        });
        let rows = v.rows(&scene, 80, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Target::Answer(false),
            "the cursor does not start on the refusal"
        );
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Answer(false)
        );

        // SAYING YES TAKES AIM FIRST, which is one arrow or one `y`.
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Confirm("012345".to_owned()),
        });
        let rows = v.rows(&scene, 80, &t);
        let yes = rows
            .iter()
            .position(|r| r.target == Target::Answer(true))
            .unwrap();
        v.clicked(yes, None, DECIDING_WIDTH, &rows);
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Answer(true)
        );
    }

    #[test]
    fn a_question_that_wants_digits_asks_for_them() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Pin,
        });
        // AN UNTOUCHED QUESTION REFUSES, this one like every other: the
        // cursor starts on "no".
        let rows = v.rows(&scene, 80, &t);
        assert_eq!(rows[v.cursor()].target, Target::Answer(false));

        // AND SAYING YES ASKS FOR THE DIGITS, rather than sending a bare
        // yes: a PIN is a number the other side is waiting for, and
        // answering "yes" with nothing typed is a pairing that fails.
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Pin,
        });
        let rows = v.rows(&scene, 80, &t);
        let yes = rows
            .iter()
            .position(|r| r.target == Target::Answer(true))
            .unwrap();
        v.clicked(yes, None, DECIDING_WIDTH, &rows);
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Type
        );
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Pin,
        });
        assert_eq!(
            v.key(Press::Accept, &scene, DECIDING_WIDTH, &t),
            Errand::Type
        );
        // And `n` still refuses outright.
        assert_eq!(
            v.key(Press::Refuse, &scene, DECIDING_WIDTH, &t),
            Errand::Answer(false)
        );
    }

    #[test]
    fn a_question_that_only_tells_is_dismissed_by_any_key() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Passkey("012345".to_owned()),
        });
        assert!(v.asking());
        // Nothing to answer — the digits are typed on the other thing.
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert!(!v.asking());
    }

    #[test]
    fn escape_answers_no_rather_than_leaving_the_other_side_waiting() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Authorize,
        });
        assert_eq!(
            v.key(Press::Escape, &scene, DECIDING_WIDTH, &t),
            Errand::Answer(false)
        );
        assert!(!v.asking());
    }

    #[test]
    fn the_cursor_stays_on_a_row_that_exists() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        let rows = v.rows(&scene, 80, &t);
        v.clicked(rows.len() - 1, None, DECIDING_WIDTH, &rows);
        // Every device goes away under the cursor.
        let empty = Scene {
            daemon: true,
            ..Scene::default()
        };
        let rows = v.rows(&empty, 80, &t);
        v.settle(&rows);
        assert!(v.cursor() < rows.len());
        assert!(!rows[v.cursor()].heading);
        // And the arrows still move.
        assert_eq!(
            v.key(Press::Down, &empty, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
        assert_eq!(
            v.key(Press::Up, &empty, DECIDING_WIDTH, &t),
            Errand::Nothing
        );
    }

    #[test]
    fn the_machines_own_name_and_its_visibility_can_be_changed() {
        // BOTH WERE FACTS BESIDE A SWITCH: the alias is the one thing on this
        // screen that other people see, and the timeout decides how long they
        // can see it — and `Adapter::set_alias` and `set_discoverable_timeout`
        // sat in the proxy with no caller at all.
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        // Open the radio level, which is what `RET` on the adapter's row does.
        let rows = v.rows(&scene, 80, &t);
        let at = rows.iter().position(|r| r.target == Target::Radio).unwrap();
        v.clicked(at, None, DECIDING_WIDTH, &rows);
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Nothing
        );

        let rows = v.rows(&scene, 80, &t);
        let alias = rows
            .iter()
            .position(|r| r.target == Target::Rename)
            .expect("the machine's name cannot be changed");
        v.clicked(alias, None, DECIDING_WIDTH, &rows);
        // AN EMPTY PATH MEANS THIS MACHINE — the same door a device's name
        // takes, and the path is what tells them apart.
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Rename(String::new())
        );
        // THE LINE STARTS FROM THE NAME IT HAS, and the answer is the
        // machine's alias — the same two calls in both doors.
        assert_eq!(
            v.name_being_changed(&scene).as_deref(),
            Some("this machine")
        );
        assert_eq!(
            v.renamed("  the desk  "),
            Errand::Order(Ask::AdapterAlias("the desk".to_owned()))
        );
        assert_eq!(v.renamed("again"), Errand::Nothing, "one answer, once");

        let rows = v.rows(&scene, 80, &t);
        let timeout = rows
            .iter()
            .position(|r| matches!(r.target, Target::Timeout(_)))
            .expect("the visibility timeout cannot be changed");
        v.clicked(timeout, None, DECIDING_WIDTH, &rows);
        assert_eq!(
            v.key(Press::Enter, &scene, DECIDING_WIDTH, &t),
            Errand::Order(Ask::DiscoverableTimeout(180)),
            "the ring does not start at three minutes"
        );
        // AND THE RING COMES BACK ROUND.
        assert_eq!(next_timeout(180), 600);
        assert_eq!(next_timeout(600), 3600);
        assert_eq!(next_timeout(3600), 0);
        assert_eq!(next_timeout(0), 180);
    }

    /// A DEVICE'S NAME is typed from the one it has, and the answer renames
    /// THAT device — an empty answer, or one nobody asked for, renames
    /// nothing; and the next key lets an unanswered rename go.
    #[test]
    fn a_devices_new_name_is_what_the_line_answered() {
        let t = texts();
        let mut scene = a_scene();
        scene.devices[0].alias = "the old headset".to_owned();
        let mut v = View::fresh_at("/d/1");
        assert_eq!(v.renamed("nobody asked"), Errand::Nothing);
        assert_eq!(
            v.key(Press::Rename, &scene, DECIDING_WIDTH, &t),
            Errand::Rename("/d/1".to_owned())
        );
        assert_eq!(
            v.name_being_changed(&scene).as_deref(),
            Some("the old headset")
        );
        assert_eq!(
            v.renamed("desk"),
            Errand::Order(Ask::Rename("/d/1".to_owned(), "desk".to_owned()))
        );
        // AN EMPTY LINE KEEPS THE NAME.
        let _ = v.key(Press::Rename, &scene, DECIDING_WIDTH, &t);
        assert_eq!(v.renamed("   "), Errand::Nothing);
        // A LINE LET GO: the next key reaches the view, and the rename is over.
        let _ = v.key(Press::Rename, &scene, DECIDING_WIDTH, &t);
        let _ = v.key(Press::Down, &scene, DECIDING_WIDTH, &t);
        assert_eq!(v.renamed("late"), Errand::Nothing);
        assert_eq!(v.name_being_changed(&scene), None);
    }

    #[test]
    fn a_heading_is_never_alone() {
        // A TITLE WITH NOTHING UNDER IT is a thing to look at that does not
        // exist — see the power view's test of the same name for the session
        // that drew one.
        let t = texts();
        for scene in [Scene::default(), a_scene()] {
            let rows = View::fresh().rows(&scene, 80, &t);
            for (i, row) in rows.iter().enumerate() {
                // THE AIR IS A HEADING WITH NO FIELDS (`chrome::AIR`), and
                // what it is under is the title it introduces.
                if !row.heading || row.fields.is_empty() {
                    continue;
                }
                assert!(
                    rows.get(i + 1).is_some_and(|r| !r.heading),
                    "the heading `{}` has nothing under it",
                    row.fields.first().map(|f| f.text.as_str()).unwrap_or("")
                );
            }
        }
    }

    #[test]
    fn nothing_leaks_at_any_width() {
        let t = texts();
        let scene = a_scene();
        for columns in [40_u16, 60, 80, 120, 200] {
            let (left, room) = extent(columns);
            for v in [View::fresh(), View::fresh_at("/d/1")] {
                for row in v.rows(&scene, columns, &t) {
                    for f in &row.fields {
                        assert!(f.at >= left, "{columns}: a field starts before the inset");
                        assert!(
                            f.at + layout::width(&f.text) <= left + room,
                            "{columns}: `{}` runs past the room",
                            f.text
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_hint_teaches_every_key_and_every_id_has_two_texts() {
        let t = texts();
        for (_, id, _) in OF_THE_VIEW {
            // A KEY NOBODY WROTE resolves to `[key]`, never to the key — so
            // the brackets are what a missing text looks like, and the
            // comparison that used to be here could not fail.
            assert!(
                !t.t(id).starts_with('['),
                "`{id}` has no short text for the hint line"
            );
            let cmd = format!("cmd-{id}");
            assert!(
                !t.t(&cmd).starts_with('['),
                "`{cmd}` is not in the language"
            );
        }
        let v = View::fresh();
        assert!(!v.hint(&t).is_empty());
    }

    #[test]
    fn every_kind_and_every_state_has_a_text() {
        let t = texts();
        for k in Kind::ALL {
            let key = format!("bluetooth-kind-{}", k.id());
            assert!(
                !t.t(&key).starts_with('['),
                "`{key}` is not in the language"
            );
        }
        for state in ["on", "off", "off-enabling", "on-disabling", "off-blocked"] {
            let key = format!("bluetooth-power-state-{state}");
            assert!(
                !t.t(&key).starts_with('['),
                "`{key}` is not in the language"
            );
        }
        // AND THE KEYS THE ROWS BUILD BY HAND, which no check can see: the
        // profiles' names and the three sentences a question is drawn with.
        for uuid in [
            "0000110b-0000-1000-8000-00805f9b34fb",
            "0000111e-0000-1000-8000-00805f9b34fb",
            "00001124-0000-1000-8000-00805f9b34fb",
            "0000180f-0000-1000-8000-00805f9b34fb",
        ] {
            let key = ronoctua_bluetooth::profile_key(uuid).unwrap();
            assert!(!t.t(key).starts_with('['), "`{key}` is not in the language");
        }
    }
}
