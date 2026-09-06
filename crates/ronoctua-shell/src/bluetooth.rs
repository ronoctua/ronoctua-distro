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

use crate::files::{Field, Hit as FilesHit};

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
    /// Only where there is an audio card to move between profiles.
    ///
    /// A KEYBOARD HAS NONE, and the screen proof of a keyboard's own page
    /// showed the line teaching "the next audio profile" on it — the same
    /// defect as the radio's screen teaching `c connect`, one level down.
    /// `OnADevice` is not fine enough: it is about WHICH SCREEN, and this is
    /// about what the device on it can do.
    OnACard,
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
    ("f", "bluetooth-forget", Where::OnADevice),
    ("t", "bluetooth-trust", Where::OnADevice),
    ("b", "bluetooth-block", Where::OnADevice),
    ("r", "bluetooth-rename", Where::OnADevice),
    ("s", "bluetooth-scan", Where::OnTheRadio),
    ("o", "bluetooth-power", Where::OnTheRadio),
    ("v", "bluetooth-discoverable", Where::OnTheRadio),
    ("h", "bluetooth-unnamed", Where::OnTheList),
    ("<right>", "bluetooth-audio-next", Where::OnACard),
    ("<left>", "bluetooth-audio-previous", Where::OnACard),
    ("y", "bluetooth-accept", Where::WhileAsked),
    ("n", "bluetooth-refuse", Where::WhileAsked),
];

impl Where {
    /// Does a key of this kind mean anything on that screen?
    const fn shown(self, level: &Level, asking: bool, card: bool) -> bool {
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
            Self::OnACard => card && matches!(level, Level::Device(_)),
            Self::OnTheRadio => matches!(level, Level::List | Level::Radio),
            Self::OnTheList => matches!(level, Level::List),
        }
    }
}

/// The four device marks the modeline can carry at once.
///
/// FOUR literals and not a `format!`: `layout::Segment::id` is a
/// `&'static str`, for the reason the nine workspace digits are. Four is what
/// fits beside the battery, the profile and the bell on a narrow screen; the
/// fifth connected device is in the view, which is where the rest of it is.
pub const POOL: [&str; 4] = ["bt-1", "bt-2", "bt-3", "bt-4"];

/// The width `key()` builds rows at, and why a constant is honest here.
///
/// A key acts on the row under the cursor, so it has to know the rows — and it
/// does not know how wide the screen is: the two doors measure differently and
/// the footer's region is not the window's. It builds them at a fixed width
/// instead, which is only correct while the row COUNT and the row TARGETS do
/// not depend on the width. They do not: `rows()` reaches the width in
/// `elide_end` and in a field's `at`, both of which are about drawing.
///
/// That is an invariant, not an observation, so it is checked rather than
/// trusted: `the_rows_are_the_same_things_at_every_width` walks several
/// widths and compares the targets. Without it, the day one section drops a
/// row on a narrow screen, the key acts on a different row from the one the
/// footer drew — and nothing would say so.
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
    /// The next audio profile, and the one before.
    ///
    /// The ONLY pair of arrows in this view, and they are here because the
    /// question they answer has an order: off, music in stereo, a call with
    /// the microphone. `RET` would need somewhere to put a list.
    Right,
    /// The other way round `Right` goes.
    Left,
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
    /// May this device wake the machine from sleep?
    ///
    /// `readwrite` on the bus, and it was drawn as a FACT: on a Bluetooth
    /// keyboard it is the setting that decides whether pressing a key can
    /// bring the machine back at all.
    Wake(String, bool),
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
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Send this to the bluetooth thread.
    Order(Ask),
    /// Answer the question the other side is asking. It does NOT go through
    /// the orders thread: `Device1.Pair` is blocking that thread while the
    /// question stands, and an answer queued behind it would wait for the
    /// order that is waiting for the answer.
    Answer(bool),
    /// The other side wants digits typed here — a PIN, a passkey. The
    /// minibuffer asks, and the answer comes back as the same command's
    /// argument, the network password's path.
    Type,
    /// Ask for a name in the minibuffer, for the device at this path.
    Rename(String),
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
    /// It is the transfer queue's lesson, in `CLAUDE.md`: the cursor is the
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
}

/// Where a row's cells begin and how wide they are, in the ROW's cells —
/// both doors add `chrome::FIELDS_LEAD`; see `notifications::extent_of`.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
    let end = columns.saturating_sub(crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT);
    (left, end.saturating_sub(left))
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

/// The glyph of a kind — one table, and it is the kind's own id.
#[must_use]
pub fn glyph(kind: Kind) -> char {
    match kind {
        Kind::Headphones => tokens::icon::BLUETOOTH_DEVICE_HEADPHONES,
        Kind::Headset => tokens::icon::BLUETOOTH_DEVICE_HEADSET,
        Kind::Speaker => tokens::icon::BLUETOOTH_DEVICE_SPEAKER,
        Kind::Keyboard => tokens::icon::BLUETOOTH_DEVICE_KEYBOARD,
        Kind::Mouse => tokens::icon::BLUETOOTH_DEVICE_MOUSE,
        Kind::Gamepad => tokens::icon::BLUETOOTH_DEVICE_GAMEPAD,
        Kind::Tablet => tokens::icon::BLUETOOTH_DEVICE_TABLET,
        Kind::Phone => tokens::icon::BLUETOOTH_DEVICE_PHONE,
        Kind::Computer => tokens::icon::BLUETOOTH_DEVICE_COMPUTER,
        Kind::Watch => tokens::icon::BLUETOOTH_DEVICE_WATCH,
        Kind::Camera => tokens::icon::BLUETOOTH_DEVICE_CAMERA,
        Kind::Printer => tokens::icon::BLUETOOTH_DEVICE_PRINTER,
        Kind::Network => tokens::icon::BLUETOOTH_DEVICE_NETWORK,
        Kind::Display => tokens::icon::BLUETOOTH_DEVICE_DISPLAY,
        Kind::Unknown => tokens::icon::BLUETOOTH_DEVICE_UNKNOWN,
    }
}

/// One mark on the modeline: what to draw, and whether it is alarmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    /// The device's path — what a click on it opens.
    pub at: String,
    /// The glyph, and the percentage when it is low enough to matter.
    pub text: String,
    /// Is the battery at or under the danger mark?
    pub alarmed: bool,
}

/// What the modeline draws: one mark per CONNECTED device, four at most.
///
/// PURE, and tested beside the view: the rule is the owner's — the kind's
/// glyph, the percentage only at [`ronoctua_bluetooth::SHOWN_FROM`] and
/// under, the danger colour at [`ronoctua_bluetooth::DANGER_FROM`] and under
/// — and nothing about it needs a bus.
#[must_use]
pub fn modeline(scene: &Scene) -> Vec<Shown> {
    scene
        .connected()
        .into_iter()
        .take(POOL.len())
        .map(|d| Shown {
            at: d.at.clone(),
            text: match d.battery.filter(|_| d.shows_percent()) {
                Some(p) => format!("{} {p}%", glyph(d.kind)),
                None => glyph(d.kind).to_string(),
            },
            alarmed: d.alarmed(),
        })
        .collect()
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
    pub fn look_at_the_card(&mut self, scene: &Scene) {
        self.card = None;
        let Level::Device(at) = &self.level else {
            return;
        };
        let Some(address) = scene.device(at).map(|d| d.address.clone()) else {
            return;
        };
        if address.is_empty() {
            return;
        }
        match ronoctua_bluetooth::audio::card(&address) {
            Ok(card) => self.card = card,
            Err(e) => tracing::debug!(error = %e, "no audio card for this device"),
        }
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
    /// where it is without reaching into it.
    #[must_use]
    #[cfg_attr(not(test), expect(dead_code, reason = "the doors do not ask yet"))]
    pub const fn level(&self) -> &Level {
        &self.level
    }

    /// Is a question waiting?
    #[must_use]
    #[cfg_attr(not(test), expect(dead_code, reason = "the doors do not ask yet"))]
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

    /// Puts the cursor on row `n`, when it is a row one can stand on.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) {
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
                texts.t(ronoctua_i18n::key::BLUETOOTH_PAIRING_ANSWER),
            ));
            out.push(crate::chrome::Piece::separator());
        }
        let here: Vec<(&str, &str)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&self.level, self.question.is_some(), self.card.is_some()))
            .map(|(k, id, _)| (*k, *id))
            .collect();
        out.extend(crate::chrome::hint_of(&here, texts));
        out
    }

    /// Acts on a key.
    #[must_use]
    pub fn key(&mut self, press: Press, scene: &Scene, texts: &ronoctua_i18n::Texts) -> Errand {
        let rows = self.rows(scene, DECIDING_WIDTH, texts);
        self.settle(&rows);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.clone());
        let disabled = rows.get(self.cursor).is_some_and(|r| r.disabled);
        // THE RADIO'S OWN STATE, read once: the three bare letters below act
        // on the adapter and not on the row, so `disabled` — which is about
        // the row — cannot answer for them.
        let powered = scene.adapter().is_some_and(|a| a.powered);
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
                Target::Nothing => Errand::Nothing,
                Target::Device(at) => {
                    self.level = Level::Device(at);
                    self.cursor = 1;
                    self.at = None;
                    // THE CARD IS READ HERE and nowhere else: opening a
                    // device is the one moment its audio profile can have
                    // changed without this view knowing, and `pw-dump` is a
                    // process — asking it per frame would put a fork in the
                    // footer's loop.
                    self.look_at_the_card(scene);
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
                Target::Audio(card, profile) => Errand::Audio(card, profile),
                // THE MACHINE'S OWN NAME is typed in the minibuffer, the same
                // path a device's is — `Errand::Rename` with no path means
                // this machine.
                Target::Rename => Errand::Rename(String::new()),
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
            // POWER IS THE EXCEPTION, and has to be: it is the key that turns
            // the radio back ON, so a rule that refused it while the radio was
            // off would be a switch with no way back.
            Press::Scan if !powered => Errand::Nothing,
            Press::Scan => {
                let looking = scene.adapter().is_some_and(|a| a.discovering);
                Errand::Order(Ask::Scan(!looking))
            }
            Press::Power => {
                let on = scene.adapter().is_some_and(|a| a.powered);
                Errand::Order(Ask::Power(!on))
            }
            Press::Discoverable if !powered => Errand::Nothing,
            Press::Discoverable => {
                let on = scene.adapter().is_some_and(|a| a.discoverable);
                Errand::Order(Ask::Discoverable(!on))
            }
            Press::Unnamed => {
                self.unnamed = !self.unnamed;
                Errand::Nothing
            }
            // THE AUDIO PROFILE'S RING, and the only arrows in this view.
            //
            // It acts on the CARD the view is holding and not on the row, for
            // the power view's reason: the row is what says which profile is
            // on, and the arrows move between the ones the device can
            // actually do (`Card::next` skips the rest).
            Press::Right | Press::Left => {
                let Some(card) = self.card.as_ref() else {
                    return Errand::Nothing;
                };
                let Some(next) = card.next(press == Press::Right) else {
                    return Errand::Nothing;
                };
                Errand::Audio(card.id, next.index)
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
                    _ => Errand::Rename(at),
                }
            }
        }
    }

    /// Answers the waiting question.
    fn answer(&mut self, yes: bool) -> Errand {
        self.question = None;
        Errand::Answer(yes)
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
        };
        if let Some(q) = &self.question {
            self.prompt(&mut b, q, scene);
            return b.out;
        }
        match &self.level {
            Level::List => self.list(&mut b, scene),
            Level::Radio => self.radio(&mut b, scene),
            Level::Device(at) => match scene.device(at) {
                Some(d) => self.detail(&mut b, d, texts),
                // IT WENT AWAY WHILE THE DETAIL WAS OPEN: say so where the
                // facts were, rather than draw an empty screen.
                None => {
                    b.heading(ronoctua_i18n::key::BLUETOOTH_HEADING_PAIRED);
                    b.note(ronoctua_i18n::key::BLUETOOTH_NOTHING_HERE);
                }
            },
        }
        b.out
    }

    /// The question, and the two answers.
    fn prompt(&self, b: &mut Builder<'_>, q: &agent::Question, scene: &Scene) {
        use ronoctua_i18n::key;
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
        use ronoctua_i18n::key;
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
        use ronoctua_i18n::key;
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
        use ronoctua_i18n::{Arg, key};
        let yes_no = |on: bool| {
            texts.t(if on {
                key::BLUETOOTH_YES
            } else {
                key::BLUETOOTH_NO
            })
        };
        b.heading(key::BLUETOOTH_TITLE);
        b.pair(
            format!("{} {}", glyph(d.kind), d.label),
            texts.t(&format!("bluetooth-kind-{}", d.kind.id())),
            Target::Nothing,
            false,
        );
        b.fact_coloured(key::BLUETOOTH_FACT_CONNECTED, yes_no(d.connected), None);
        if let Some(p) = d.battery {
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
            b.fact(key::BLUETOOTH_FACT_BATTERY_SOURCE, source.clone());
        }
        b.fact(key::BLUETOOTH_FACT_PAIRED, yes_no(d.paired));
        b.fact(key::BLUETOOTH_FACT_BONDED, yes_no(d.bonded));
        b.fact(key::BLUETOOTH_FACT_TRUSTED, yes_no(d.trusted));
        b.fact(key::BLUETOOTH_FACT_BLOCKED, yes_no(d.blocked));
        if !d.name.is_empty() {
            b.fact(key::BLUETOOTH_FACT_NAME, d.name.clone());
        }
        if !d.alias.is_empty() && d.alias != d.name {
            b.fact(key::BLUETOOTH_FACT_ALIAS, d.alias.clone());
        }
        b.fact(key::BLUETOOTH_FACT_ADDRESS, d.address.clone());
        if !d.address_type.is_empty() {
            // A CLOSED VOCABULARY IS A TEXT KEY, not the daemon's word.
            //
            // `public`/`random` is a list of two, not free-form data like a
            // device's name — and drawn raw it put an English word on a pt-BR
            // screen ("tipo de endereço: random"). This file already
            // translates `PowerState` and every service UUID through a key;
            // three enumerations had been left out.
            b.fact(
                key::BLUETOOTH_FACT_ADDRESS_TYPE,
                word_or(texts, "bluetooth-address-type", &d.address_type),
            );
        }
        if let Some(rssi) = d.rssi {
            b.fact(
                key::BLUETOOTH_FACT_RSSI,
                texts.t_args(
                    key::BLUETOOTH_RSSI_VALUE,
                    &[("dbm", Arg::Number(i64::from(rssi)))],
                ),
            );
        }
        if let Some(tx) = d.tx_power {
            b.fact(
                key::BLUETOOTH_FACT_TX_POWER,
                texts.t_args(
                    key::BLUETOOTH_TX_POWER_VALUE,
                    &[("dbm", Arg::Number(i64::from(tx)))],
                ),
            );
        }
        if d.legacy {
            b.fact(key::BLUETOOTH_FACT_LEGACY, yes_no(true));
        }
        if let Some(wake) = d.wake {
            // A SWITCH AND NOT A FACT. `WakeAllowed` is `readwrite` on
            // `Device1`, and this drew it read-only — on a Bluetooth keyboard
            // it is the setting that decides whether the lid opening the
            // machine is a thing that can happen at all.
            b.pair(
                b.texts.t(key::BLUETOOTH_FACT_WAKE),
                yes_no(wake),
                Target::Wake(d.at.clone(), !wake),
                !d.paired,
            );
        }
        b.fact(key::BLUETOOTH_FACT_SERVICES, yes_no(d.services_resolved));
        if let Some(mode) = &d.reconnect {
            b.fact(
                key::BLUETOOTH_FACT_RECONNECT,
                word_or(texts, "bluetooth-reconnect", mode),
            );
        }
        if let Some(class) = d.class {
            b.fact(key::BLUETOOTH_FACT_CLASS, format!("0x{class:06x}"));
        }
        if let Some(appearance) = d.appearance {
            b.fact(key::BLUETOOTH_FACT_APPEARANCE, appearance.to_string());
        }
        if let Some(modalias) = &d.modalias {
            b.fact(key::BLUETOOTH_FACT_MODALIAS, modalias.clone());
        }
        if !d.adapter.is_empty() {
            b.fact(key::BLUETOOTH_FACT_ADAPTER, d.adapter.clone());
        }
        // THE AUDIO PROFILE, which is the one thing on this screen that is a
        // CHOICE and not a fact — and the only row in this view the arrows
        // move. BlueZ cannot answer it (`MediaTransport1` says what is
        // playing, never what could be), so it comes from PipeWire; see
        // `ronoctua_bluetooth::audio`.
        //
        // NO CARD, NO ROW. A device that is not audio has none, and a
        // PipeWire that is not answering is the same absence — offering a
        // switch that cannot move is worse than not offering it.
        if let Some(card) = self.card.as_ref()
            && let Some(now) = card.now()
        {
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
                b.fact(
                    key::BLUETOOTH_FACT_VOLUME,
                    texts.t_args(key::BLUETOOTH_PERCENT, &[("percent", Arg::Number(percent))]),
                );
            }
            if let Some(c) = t.codec {
                // THE NAME AND NOT THE BYTE. `0` is SBC and `2` is AAC; the
                // screen said `0`, which is true of nothing a person knows.
                b.fact(
                    key::BLUETOOTH_FACT_CODEC,
                    word_or(texts, "bluetooth-codec", &c.to_string()),
                );
            }
        }

        // WHAT IT CAN DO, one row each, and `RET` connects or drops that one.
        b.heading(key::BLUETOOTH_HEADING_PROFILES);
        if d.uuids.is_empty() {
            b.note(key::BLUETOOTH_NOTHING_HERE);
        }
        for uuid in &d.uuids {
            let said =
                ronoctua_bluetooth::profile_key(uuid).map_or_else(|| uuid.clone(), |k| texts.t(k));
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
            b.pair(
                said,
                texts.t(if running {
                    key::BLUETOOTH_YES
                } else {
                    key::BLUETOOTH_NO
                }),
                Target::Profile {
                    device: d.at.clone(),
                    uuid: uuid.clone(),
                    on: !running,
                },
                !d.paired,
            );
        }
    }

    /// The radio's own settings.
    fn radio(&self, b: &mut Builder<'_>, scene: &Scene) {
        use ronoctua_i18n::{Arg, key};
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
}

impl Builder<'_> {
    fn piece(at: u16, text: String, color: tokens::Color) -> Field {
        Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
        }
    }

    fn heading(&mut self, key: &str) {
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
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                self.texts.t(key),
                tokens::color::TEXT_GROUP,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A sentence that stands for a whole section — "nothing here".
    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.sentence(text);
    }

    fn sentence(&mut self, text: String) {
        let focused = self.cursor == self.out.len();
        self.out.push(Row {
            fields: vec![Self::piece(
                // A SENTENCE LINES UP WITH THE LABELS, not with the titles.
                // The mark's indent is paid by every row of these views on
                // purpose — an indent only some rows pay is a ragged edge —
                // and a note that skipped it sat two cells left of everything
                // around it.
                crate::chrome::text_left(self.left),
                layout::elide_end(&text, self.room.saturating_sub(crate::chrome::MARKER)),
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
        });
    }

    /// A label and a value, with nothing to do on `RET`.
    fn fact(&mut self, key: &str, value: String) {
        self.fact_coloured(key, value, None);
    }

    fn fact_coloured(&mut self, key: &str, value: String, colour: Option<tokens::Color>) {
        let label = self.texts.t(key);
        self.coloured(label, value, Target::Nothing, false, colour);
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
        let (label_colour, value_colour) = match (focused, disabled) {
            (true, _) => (tokens::color::STATE_ACCENT, tokens::color::STATE_ACCENT),
            (false, true) => (tokens::color::TEXT_DIM, tokens::color::TEXT_DIM),
            (false, false) => (tokens::color::TEXT_SECONDARY, tokens::color::TEXT_PRIMARY),
        };
        let value_colour = colour.filter(|_| !focused).unwrap_or(value_colour);
        // ONE ARITHMETIC, and it is `chrome::field_pair` — see the comment on
        // `chrome::value_column` for what the three copies of it had drifted
        // into.
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                target != Target::Nothing,
                &label,
                &value,
                label_colour,
                value_colour,
            ),
            focused,
            heading: false,
            target,
            disabled,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            ..Device::default()
        }
    }

    #[test]
    fn the_rows_are_the_same_things_at_every_width() {
        // `View::key` builds its rows at `DECIDING_WIDTH` because it does not
        // know how wide the screen is — and that is only honest while the row
        // COUNT and the row TARGETS do not depend on the width. The day one
        // section drops a row on a narrow screen, a key would act on a
        // different row from the one the footer drew, and nothing on screen
        // would say so.
        //
        // The FIELDS are allowed to differ — that is what eliding is.
        let t = texts();
        let scene = a_scene();
        for level in [View::fresh(), View::fresh_at("/d/1")] {
            let want = |columns: u16| -> Vec<(bool, bool, Target)> {
                level
                    .clone()
                    .rows(&scene, columns, &t)
                    .into_iter()
                    .map(|r| (r.heading, r.disabled, r.target))
                    .collect()
            };
            let at_eighty = want(DECIDING_WIDTH);
            for columns in [40_u16, 60, 100, 160, 240] {
                assert_eq!(want(columns), at_eighty, "the rows differ at {columns}");
            }
        }
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
                Where::OnACard | Where::OnADevice | Where::OnTheList | Where::Anywhere => {
                    let mut v = View::fresh();
                    // A CARD, because `pw-dump` cannot run in a test and the
                    // two arrows act on what it read. Two profiles, so the
                    // ring has somewhere to go.
                    if *column == Where::OnACard {
                        v.level = Level::Device("/d/1".to_owned());
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
                here.clicked(row, &rows);
                let (was, level, unnamed) = (here.cursor(), here.level.clone(), here.unnamed);
                let did = here.key(press, &scene, &t);
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
        // `profile_key` maps a UUID to a text key, and four of its fifteen
        // were tested — twice, once on each side. A missing `.ftl` entry for
        // any of the other eleven puts `[bluetooth-profile-sim-access]` on
        // screen, and nothing looked.
        //
        // THE UUIDs COME FROM THE FUNCTION, not from a list beside it: the
        // sixteen-bit shorts are read out of its own source, so a service
        // added there is demanded here on the next build.
        let t = texts();
        let source = include_str!("../../ronoctua-bluetooth/src/lib.rs");
        let table = source
            .split_once("pub fn profile_key(")
            .expect("profile_key is still there")
            .1
            .split_once("\n}")
            .expect("and it still closes")
            .0;
        // EVERY quoted short, not one per line: several arms bind two
        // (`"111e" | "111f"`), and reading only the first left six of the
        // twenty-one untested.
        let shorts: Vec<&str> = table
            .split('"')
            .skip(1)
            .step_by(2)
            .filter(|s| s.len() == 4 && s.chars().all(|c| c.is_ascii_hexdigit()))
            .collect();
        assert!(shorts.len() >= 20, "only {} shorts read", shorts.len());
        for short in shorts {
            let uuid = format!("0000{short}-0000-1000-8000-00805f9b34fb");
            let id = ronoctua_bluetooth::profile_key(&uuid)
                .unwrap_or_else(|| panic!("`{uuid}` is in the table and resolves to nothing"));
            assert!(
                !t.t(id).starts_with('['),
                "`{id}` names a service and has no sentence"
            );
        }
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
        // AND A DEVICE WITH NO AUDIO CARD IS NOT OFFERED THE PROFILE.
        //
        // The owner's own keyboard, photographed: `→ the next audio profile`
        // on the page of a thing that has none. `OnADevice` is about WHICH
        // SCREEN and this is about what the device on it can do, which is why
        // there is a column of its own for it.
        assert!(!said.contains(&t.t("bluetooth-audio-next")));
        one.card = Some(ronoctua_bluetooth::audio::Card {
            id: 1,
            active: 0,
            profiles: Vec::new(),
        });
        assert!(words(&one).contains(&t.t("bluetooth-audio-next")));

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
        v.clicked(speaker, &rows);
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
        v.clicked(at, &rows);
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
        let _ = open.key(Press::Unnamed, &a_scene(), &t);
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
        assert!(said(&no_daemon).contains(&t.t(ronoctua_i18n::key::BLUETOOTH_NO_DAEMON)));
        let running = Scene {
            daemon: true,
            ..Scene::default()
        };
        assert!(
            said(&v.rows(&running, 80, &t))
                .contains(&t.t(ronoctua_i18n::key::BLUETOOTH_NO_ADAPTER))
        );
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
        v.clicked(row, &rows);
        assert_eq!(v.key(Press::Enter, &scene, &t), Errand::Nothing);
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
        assert_eq!(v.key(Press::Escape, &scene, &t), Errand::Nothing);
        assert_eq!(v.level(), &Level::List);
        assert_eq!(v.key(Press::Escape, &scene, &t), Errand::Close);
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
        v.clicked(row, &rows);
        // It is connected, so `c` drops the link.
        assert_eq!(
            v.key(Press::Connect, &scene, &t),
            Errand::Order(Ask::Disconnect("/d/1".to_owned()))
        );
        assert_eq!(
            v.key(Press::Trust, &scene, &t),
            Errand::Order(Ask::Trust("/d/1".to_owned(), true))
        );
        assert_eq!(
            v.key(Press::Block, &scene, &t),
            Errand::Order(Ask::Block("/d/1".to_owned(), true))
        );
        assert_eq!(
            v.key(Press::Forget, &scene, &t),
            Errand::Order(Ask::Forget("/d/1".to_owned()))
        );
        assert_eq!(
            v.key(Press::Rename, &scene, &t),
            Errand::Rename("/d/1".to_owned())
        );
        // And inside the detail the same letters are about the same device.
        let _ = v.key(Press::Enter, &scene, &t);
        assert_eq!(
            v.key(Press::Connect, &scene, &t),
            Errand::Order(Ask::Disconnect("/d/1".to_owned()))
        );
    }

    #[test]
    fn the_radio_keys_read_the_radio_and_flip_it() {
        let t = texts();
        let mut scene = a_scene();
        let mut v = View::fresh();
        assert_eq!(
            v.key(Press::Power, &scene, &t),
            Errand::Order(Ask::Power(false)),
            "it is on, so `o` turns it off"
        );
        assert_eq!(
            v.key(Press::Scan, &scene, &t),
            Errand::Order(Ask::Scan(true))
        );
        scene.adapters[0].discovering = true;
        assert_eq!(
            v.key(Press::Scan, &scene, &t),
            Errand::Order(Ask::Scan(false))
        );
        assert_eq!(
            v.key(Press::Discoverable, &scene, &t),
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
        assert_eq!(v.key(Press::Connect, &scene, &t), Errand::Nothing);
        assert!(v.asking(), "a letter answered the question");
        // And `n` says no.
        assert_eq!(v.key(Press::Refuse, &scene, &t), Errand::Answer(false));
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
        v.clicked(no, &rows);
        assert_eq!(v.key(Press::Enter, &scene, &t), Errand::Answer(false));

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
        assert_eq!(v.key(Press::Enter, &scene, &t), Errand::Answer(false));

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
        v.clicked(yes, &rows);
        assert_eq!(v.key(Press::Enter, &scene, &t), Errand::Answer(true));
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
        v.clicked(yes, &rows);
        assert_eq!(v.key(Press::Enter, &scene, &t), Errand::Type);
        let mut v = View::fresh();
        v.asked(agent::Question {
            device: "/d/1".to_owned(),
            asked: agent::Asked::Pin,
        });
        assert_eq!(v.key(Press::Accept, &scene, &t), Errand::Type);
        // And `n` still refuses outright.
        assert_eq!(v.key(Press::Refuse, &scene, &t), Errand::Answer(false));
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
        assert_eq!(v.key(Press::Enter, &scene, &t), Errand::Nothing);
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
        assert_eq!(v.key(Press::Escape, &scene, &t), Errand::Answer(false));
        assert!(!v.asking());
    }

    #[test]
    fn the_cursor_stays_on_a_row_that_exists() {
        let t = texts();
        let scene = a_scene();
        let mut v = View::fresh();
        let rows = v.rows(&scene, 80, &t);
        v.clicked(rows.len() - 1, &rows);
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
        assert_eq!(v.key(Press::Down, &empty, &t), Errand::Nothing);
        assert_eq!(v.key(Press::Up, &empty, &t), Errand::Nothing);
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
        v.clicked(at, &rows);
        assert_eq!(v.key(Press::Enter, &scene, &t), Errand::Nothing);

        let rows = v.rows(&scene, 80, &t);
        let alias = rows
            .iter()
            .position(|r| r.target == Target::Rename)
            .expect("the machine's name cannot be changed");
        v.clicked(alias, &rows);
        // AN EMPTY PATH MEANS THIS MACHINE — the same door a device's name
        // takes, and the path is what tells them apart.
        assert_eq!(
            v.key(Press::Enter, &scene, &t),
            Errand::Rename(String::new())
        );

        let rows = v.rows(&scene, 80, &t);
        let timeout = rows
            .iter()
            .position(|r| matches!(r.target, Target::Timeout(_)))
            .expect("the visibility timeout cannot be changed");
        v.clicked(timeout, &rows);
        assert_eq!(
            v.key(Press::Enter, &scene, &t),
            Errand::Order(Ask::DiscoverableTimeout(180)),
            "the ring does not start at three minutes"
        );
        // AND THE RING COMES BACK ROUND.
        assert_eq!(next_timeout(180), 600);
        assert_eq!(next_timeout(600), 3600);
        assert_eq!(next_timeout(3600), 0);
        assert_eq!(next_timeout(0), 180);
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
