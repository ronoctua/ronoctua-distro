//! Sound — one view, two doors, and the two marks the modeline draws.
//!
//! # What is here
//!
//! Everything the screen decides: which rows exist, what a key does to them,
//! and what has to be asked of PipeWire as a result. Nothing here runs a
//! program, opens a socket or knows what a window is —
//! `crates/ronoctua-audio` is the boundary and `audio_window.rs` is the second
//! door. It is the shape `power.rs` established and `locale.rs` repeated.
//!
//! # The row that comes and goes
//!
//! A microphone's level row exists only under the source the cursor is on,
//! because a level row is a microphone that is OPEN. That makes the row count
//! a function of the cursor, and moving the cursor by INDEX across a rebuild
//! would land it a row out — one down from a source is the level row's old
//! place, and one further is somebody else entirely.
//!
//! So every row a cursor can stop on carries a [`Spot`], which is its
//! identity, and the cursor is re-found by that after the rows are rebuilt.
//! Sentences that state a fact about a whole section are headings here rather
//! than rows: "there is no microphone" is not a thing to act on, and a cursor
//! resting on it would have no `Spot` to be re-found by.
//!
//! # The volume is a RING and the bar is a drawing
//!
//! `←`/`→` move the value on the row — the display view's arrangement and not
//! the Bluetooth one, because that page has two columns for the plain arrows
//! to cross and this has none. `RET` is what a CLICK becomes, so on a device
//! row it does the thing a person means by pointing at a device: play through
//! this one. The bar comes from `chrome::meter`, which is the one bar in the
//! system.

use crate::fields::{Field, Hit as FilesHit, Ink};
use crate::sizes::BAR_CELLS;
use ronoctua_audio::{Direction, Kind, Order, Snapshot, prefs::Preferences};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// The keys this view teaches, and where each one applies.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "audio-down", Where::Anywhere),
    ("<up>", "audio-up", Where::Anywhere),
    ("<left>", "audio-less", Where::OnAValue),
    ("<right>", "audio-more", Where::OnAValue),
    ("RET", "audio-enter", Where::OnADoor),
    ("m", "audio-mute", Where::OnADevice),
    ("d", "audio-default", Where::OnADevice),
];

/// Which rows a key means anything on.
///
/// The hint line teaches the keys that WORK where the cursor is. A view that
/// taught `m` while the cursor sat on a setting would be teaching a key that
/// does nothing, and the owner photographed exactly that in the Bluetooth
/// radio page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns.
    OnAValue,
    /// On a row `RET` does something to.
    OnADoor,
    /// On a sink or a source.
    OnADevice,
}

impl Where {
    /// Whether a key is worth teaching for this row.
    const fn shown(self, target: Target) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnAValue => target.turns(),
            Self::OnADoor => target.acts(),
            Self::OnADevice => target.is_a_device(),
        }
    }
}

/// A key, once the door has read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// The previous value on this row.
    Less,
    /// The next value on this row.
    More,
    /// Do what this row does.
    Enter,
    /// Silence what this row is about, or give it back.
    Mute,
    /// Make what this row is about the one the machine reaches for.
    Default,
    /// Back one level, and out.
    Escape,
}

/// What a row is, so the cursor can be found again after a rebuild.
///
/// Every row a cursor can stop on has one, and no two rows share one. See the
/// module header for the failure this exists to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spot {
    /// A heading, an empty line, or a sentence about a whole section.
    Nothing,
    /// A device that plays.
    Sink(u32),
    /// A device that listens.
    Source(u32),
    /// The level of the microphone the cursor is on.
    Level,
    /// An application's own stream.
    Stream(u32),
    /// The card's profile.
    Profile,
    /// One of the card's sockets.
    Port(Direction),
    /// One of this view's own settings.
    Knob(Knob),
}

/// The settings this view keeps of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Knob {
    /// How far one press of a volume key moves.
    Step,
    /// Whether the volume may pass what the hardware calls full.
    Over,
    /// Whether changing the volume draws the line above the modeline.
    Flash,
}

/// What `RET` and the value keys do on a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Nothing at all.
    Nothing,
    /// A device, and its volume — the node and where it is now.
    Loud(u32, u8, Kind),
    /// An application's stream, and its volume.
    Stream(u32, u8),
    /// The card's profile ring — the card and the index it is on.
    Profile(u32, i32),
    /// One of the card's socket rings.
    Port(u32, Direction),
    /// One of this view's settings.
    Knob(Knob),
}

impl Target {
    /// Whether `S-<arrows>` mean anything here.
    const fn turns(self) -> bool {
        !matches!(self, Self::Nothing)
    }

    /// Whether `RET` means anything here.
    const fn acts(self) -> bool {
        !matches!(self, Self::Nothing)
    }

    /// Whether this is a sink or a source, which is what `m` and `d` need.
    const fn is_a_device(self) -> bool {
        matches!(self, Self::Loud(_, _, _) | Self::Stream(_, _))
    }
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What is drawn, already placed and cut.
    pub fields: Vec<Field>,
    /// Whether the cursor is on it.
    pub focused: bool,
    /// Whether the cursor skips it.
    pub heading: bool,
    /// What a key does here.
    pub target: Target,
    /// Its identity across a rebuild.
    pub spot: Spot,
    /// Reachable and refusing — drawn dim, and `RET` does not move.
    pub disabled: bool,
}

impl Row {
    /// Whether a click takes it — a row the cursor stands on — and so
    /// whether a resting pointer lights it: ONE ANSWER for both doors' light
    /// and click (the region's `Row::takes`, the window's `Content::takes`,
    /// [`View::clicked`]).
    #[must_use]
    pub const fn takes(&self) -> bool {
        !self.heading
    }
}

/// What the door has to do about a key.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Put this on the clipboard.
    Copy(String),
    /// Ask PipeWire.
    Order(Order),
    /// Write the view's own settings.
    Keep(Preferences),
    /// Say this, and it is already a sentence.
    Speak(&'static str),
    /// Shut the view.
    Close,
}

/// Where the rows start and how wide they are.
///
/// The same arithmetic as every other view of this shape, and it is
/// `chrome::extent` because there is one of it.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    crate::chrome::extent(columns)
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

/// The view.
#[derive(Debug, Default)]
pub struct View {
    /// Which row the cursor is on.
    cursor: usize,
    /// The source whose meter is open, if any.
    ///
    /// It follows the cursor and nothing else. See the module header.
    listening: Option<u32>,
    /// The node the cursor is on, when it is on one.
    ///
    /// It is what "The card" is about. Following the DEFAULT sink instead
    /// looked right on a laptop with one card and was wrong on any machine
    /// with two: choosing the HDMI output and reading the card below it would
    /// show the built-in one, with its profiles and its sockets, and nothing
    /// on screen saying which card was being described.
    ///
    /// Set in `aim_at` beside `listening`, and for the same reason: it changes
    /// which rows exist, so it has to be decided before they are built.
    on_a_node: Option<u32>,
    /// The most recent level the meter reported, 0..=100.
    level: u8,
    /// This view's own settings.
    prefs: Preferences,
    /// Whether anybody has moved the cursor yet.
    ///
    /// It is what keeps "land on something that acts" a rule about ARRIVING
    /// rather than a rule about every settle: without it, a cursor a person
    /// put on a fact would be dragged off it the next time PipeWire reported
    /// anything, which is several times a minute.
    moved: bool,
}

impl View {
    /// A view with the cursor on the first row that DOES something.
    ///
    /// Not merely the first row that is not a heading. Row zero is always a
    /// heading — a cursor there is a cursor on nothing, which is the "no key
    /// works" the owner reported about the locale view — but landing on the
    /// first FACT is barely better: the hint line then teaches two keys, and
    /// the first thing a person came here to touch is a device.
    #[must_use]
    pub fn fresh(prefs: Preferences) -> Self {
        Self {
            cursor: 1,
            listening: None,
            on_a_node: None,
            level: 0,
            prefs,
            moved: false,
        }
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The source whose meter should be open.
    #[must_use]
    pub const fn listening(&self) -> Option<u32> {
        self.listening
    }

    /// This view's settings, as they stand.
    #[must_use]
    pub const fn preferences(&self) -> Preferences {
        self.prefs
    }

    /// Take a level from the meter.
    ///
    /// Returns whether the drawing changed, so a level that rounds to the same
    /// bar does not spend a frame.
    pub fn took_level(&mut self, level: u8, columns: u16) -> bool {
        let (_, room) = extent(columns);
        let cells = BAR_CELLS.min(room);
        let before = crate::chrome::meter(self.level, cells);
        self.level = level;
        before != crate::chrome::meter(level, cells)
    }

    /// Take the settings back off disk.
    pub const fn took_preferences(&mut self, prefs: Preferences) {
        self.prefs = prefs;
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        // THE WIDTH IS THE DOOR'S — see `View::key`: which keys this line
        // teaches comes from the row under the cursor, and which row that is
        // moves with the width now that a note wraps.
        let rows = self.rows(snap, columns, texts);
        let target = rows.get(self.cursor).map_or(Target::Nothing, |r| r.target);
        // A KEY THAT IS ONLY HERE BECAUSE OF THE ROW UNDER THE CURSOR reads as
        // one (`Register::Contextual`): the owner asked for the passing guests to
        // look apart from the keys that are always on the line.
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(target))
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// Put the cursor on a row, if that row is one.
    ///
    /// `rows` is what the door DREW and `columns` is the width it drew them
    /// at; the two travel together, because everything downstream rebuilds
    /// the list to find the row again.
    ///
    /// Answers whether the cursor was ALREADY on that row: that is the second
    /// click, which the door turns into `RET` (invariant 19).
    pub fn clicked(
        &mut self,
        n: usize,
        rows: &[Row],
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        let Some(row) = rows.get(n).filter(|r| r.takes()) else {
            return false;
        };
        let again = self.cursor == n;
        self.moved = true;
        self.aim_at(row.spot, snap, columns, texts);
        again
    }

    /// Put the cursor somewhere real, and the meter where the cursor is.
    ///
    /// The doors call this rather than clamping an index themselves: clamping
    /// alone would leave the meter pointing at whatever row the cursor USED to
    /// be on, and the rows are a function of that.
    pub fn settle_in(
        &mut self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(snap, columns, texts);
        self.clamp(&rows);
        // AND IF IT LANDED SOMEWHERE INERT, walk on to the first row that
        // acts. This only ever fires on the way in — once a person has moved
        // the cursor, `clamp` keeps it where they put it, including on a fact
        // they are reading.
        if !self.moved
            && rows.get(self.cursor).is_some_and(|r| !r.target.acts())
            && let Some(i) = rows.iter().position(|r| !r.heading && r.target.acts())
        {
            self.cursor = i;
        }
        let want = rows.get(self.cursor).map_or(Spot::Nothing, |r| r.spot);
        self.aim_at(want, snap, columns, texts);
    }

    /// Put the cursor on the row with this identity, and the meter with it.
    ///
    /// # Why the meter is decided FIRST
    ///
    /// Because the level row exists under the source the meter is open on, so
    /// deciding it is what changes the row list. Rebuilding first and reading
    /// the meter off the result is a step behind: the rows would still hold
    /// the level row of the source the cursor has just left, and everything
    /// below it would be one place out. That is the failure this whole
    /// arrangement exists to stop, and it went red exactly here before the
    /// order was fixed.
    fn aim_at(
        &mut self,
        want: Spot,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        self.listening = match want {
            Spot::Source(id) => Some(id),
            _ => None,
        };
        self.on_a_node = match want {
            Spot::Sink(id) | Spot::Source(id) | Spot::Stream(id) => Some(id),
            // THE LEVEL ROW BELONGS TO THE SOURCE ABOVE IT, so standing on it
            // keeps the card that source hangs off.
            Spot::Level => self.listening,
            _ => self.on_a_node,
        };
        let rows = self.rows(snap, columns, texts);
        if want != Spot::Nothing
            && let Some(i) = rows.iter().position(|r| r.spot == want)
        {
            self.cursor = i;
        }
        self.clamp(&rows);
    }

    /// Clamp the cursor into the rows, and off a heading. The INDEX only.
    fn clamp(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        if self.cursor >= rows.len() {
            self.cursor = rows.len() - 1;
        }
        if rows[self.cursor].heading {
            let after = (self.cursor..rows.len()).find(|&i| !rows[i].heading);
            let before = (0..=self.cursor).rev().find(|&i| !rows[i].heading);
            if let Some(i) = after.or(before) {
                self.cursor = i;
            }
        }
    }

    /// A key.
    ///
    /// THE WIDTH IS THE DOOR'S. A key acts on the row under the cursor and the
    /// cursor is an index into the rows that were DRAWN, so the two have to be
    /// the same list — and since a note wraps (`chrome::note_lines`), how many
    /// rows there are is a function of the width.
    pub fn key(
        &mut self,
        press: Press,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.settle_in(snap, columns, texts);
        let rows = self.rows(snap, columns, texts);
        if rows.is_empty() {
            return match press {
                Press::Escape => Errand::Close,
                _ => Errand::Nothing,
            };
        }
        let target = rows[self.cursor].target;
        let disabled = rows[self.cursor].disabled;

        match press {
            Press::Escape => return Errand::Close,
            Press::Up | Press::Down => {
                self.moved = true;
                self.walk(&rows, press == Press::Down);
                // THE CURSOR IS RE-FOUND BY IDENTITY, because the rows it was
                // measured against are not the rows that will be drawn: moving
                // onto or off a source adds or removes the level row, and
                // everything below it shifts by one.
                self.aim_at(rows[self.cursor].spot, snap, columns, texts);
                return Errand::Nothing;
            }
            _ => {}
        }

        if disabled {
            return Errand::Nothing;
        }

        match (press, target) {
            (Press::Less | Press::More, Target::Loud(node, now, _)) => {
                let percent = self.prefs.stepped(now, press == Press::More);
                Errand::Order(Order::Volume { node, percent })
            }
            (Press::Less | Press::More, Target::Stream(node, now)) => {
                let percent = self.prefs.stepped(now, press == Press::More);
                Errand::Order(Order::Volume { node, percent })
            }
            (Press::Less | Press::More, Target::Profile(device, _)) => snap
                .and_then(|s| s.device(device))
                .and_then(|c| c.next_profile(press == Press::More))
                .map_or(Errand::Nothing, |p| {
                    Errand::Order(Order::Profile {
                        device,
                        index: p.index,
                    })
                }),
            (Press::Less | Press::More, Target::Port(device, way)) => {
                let card = snap.and_then(|s| s.device(device));
                let next = card.and_then(|c| c.next_route(way, press == Press::More));
                match next {
                    // A SOCKET WITH NOTHING IN IT IS DRAWN AND REFUSES. Taking
                    // it out of the ring would say the socket does not exist,
                    // which is a different and wrong thing.
                    Some(r) if !r.available.usable() => Errand::Speak(SOCKET_EMPTY),
                    Some(r) => Errand::Order(Order::Route {
                        device,
                        index: r.index,
                    }),
                    None => Errand::Nothing,
                }
            }
            (Press::Less | Press::More, Target::Knob(knob)) => {
                self.turn(knob, press == Press::More);
                Errand::Keep(self.prefs)
            }
            // RET ON A DEVICE IS "PLAY THROUGH THIS ONE", because RET is what
            // a CLICK becomes and pointing at a device is how a person says
            // which one they mean.
            (Press::Enter | Press::Default, Target::Loud(node, _, _)) => {
                Errand::Order(Order::Default { node })
            }
            // AND ON A RING IT IS THE NEXT VALUE, for the same reason: a row
            // only reachable with a modifier is a row the mouse cannot use.
            (Press::Enter, Target::Knob(knob)) => {
                self.turn(knob, true);
                Errand::Keep(self.prefs)
            }
            (Press::Enter, Target::Profile(_, _) | Target::Port(_, _)) => {
                self.key(Press::More, snap, columns, texts)
            }
            // A stream has no "default": what a program plays through is the
            // program's business, and PipeWire has no key for it here.
            (Press::Enter | Press::Default, Target::Stream(_, _)) => Errand::Nothing,
            // AN INFORMATIVE ROW COPIES ITSELF, the rule every view of this
            // shape keeps.
            (Press::Enter, Target::Nothing) => rows
                .get(self.cursor)
                .and_then(|r| crate::chrome::information_of(&r.fields))
                .map_or(Errand::Nothing, Errand::Copy),
            (Press::Mute, Target::Loud(node, _, _) | Target::Stream(node, _)) => {
                let muted = snap
                    .and_then(|s| s.nodes.iter().find(|n| n.id == node))
                    .is_some_and(|n| n.muted);
                Errand::Order(Order::Mute {
                    node,
                    muted: !muted,
                })
            }
            _ => Errand::Nothing,
        }
    }

    /// Move the cursor one row, skipping headings.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        loop {
            let next = if down {
                if at + 1 >= rows.len() {
                    return;
                }
                at + 1
            } else {
                if at == 0 {
                    return;
                }
                at - 1
            };
            at = next;
            if !rows[at].heading {
                self.cursor = at;
                return;
            }
        }
    }

    /// Turn one of the settings.
    fn turn(&mut self, knob: Knob, forward: bool) {
        match knob {
            Knob::Step => {
                // The steps a person would choose, as a ring. A free number
                // would be a minibuffer question for a value with four
                // sensible answers.
                const STEPS: [u8; 4] = [1, 2, 5, 10];
                let at = STEPS
                    .iter()
                    .position(|s| *s == self.prefs.step)
                    .unwrap_or(2);
                let step = if forward { 1 } else { STEPS.len() - 1 };
                self.prefs.step = STEPS[(at + step) % STEPS.len()];
            }
            Knob::Over => self.prefs.over = !self.prefs.over,
            Knob::Flash => self.prefs.flash = !self.prefs.flash,
        }
    }

    /// Every row, in reading order.
    #[must_use]
    pub fn rows(
        &self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };
        let Some(s) = snap else {
            // THE VIEW'S OWN NAME over the one sentence there is to say. It
            // used to be the heading of a section that no longer exists, so
            // the screen read `Now` over `the sound server is not answering`.
            b.heading(crate::key::AUDIO_TITLE);
            b.note(crate::key::AUDIO_NO_PIPEWIRE);
            return b.out;
        };

        self.devices(&mut b, s, Kind::Sink);
        self.devices(&mut b, s, Kind::Source);
        self.programs(&mut b, s);
        self.card(&mut b, s);
        self.settings(&mut b);
        b.out
    }

    /// One section of devices, with the level row under the source in focus.
    fn devices(&self, b: &mut Builder<'_>, s: &Snapshot, kind: Kind) {
        b.heading(if kind == Kind::Sink {
            crate::key::AUDIO_HEADING_OUTPUT
        } else {
            crate::key::AUDIO_HEADING_INPUT
        });
        let here = s.of(kind);
        // THE CHOSEN ONE AND THE PLAYING ONE ARE TWO FACTS, and the line only
        // appears when they differ — measured on the owner's laptop, where
        // the chosen sink was a headset that was switched off. It sits under
        // the heading of the list it is about, and not in a summary section
        // of its own: a section that repeated the list below it was the first
        // thing this view was photographed doing wrong.
        if let Some(chosen) = s.adrift(kind) {
            let said = b
                .texts
                .t_with(crate::key::AUDIO_CHOSEN_ELSEWHERE, &[("device", chosen)]);
            b.aside(said);
        }
        if here.is_empty() {
            b.note(if kind == Kind::Sink {
                crate::key::AUDIO_NOTHING_PLAYS
            } else {
                crate::key::AUDIO_NOTHING_LISTENS
            });
            return;
        }
        let default = s.default_of(kind).map(|n| n.id);
        for n in here {
            let mut label = n.description.clone();
            if default == Some(n.id) {
                // THE ONE IN USE SAYS SO IN WORDS. A mark drawn in front of
                // the row was refused for the other views of this shape, and
                // the reason holds here: a chevron is what a tree opens with.
                label.push(' ');
                label.push(tokens::mark::SEPARATOR);
                label.push(' ');
                label.push_str(&b.texts.t(crate::key::AUDIO_IN_USE));
            }
            b.pair(
                label,
                loudness(n.volume, n.muted, b.room, b.texts),
                Target::Loud(n.id, n.volume, kind),
                if kind == Kind::Sink {
                    Spot::Sink(n.id)
                } else {
                    Spot::Source(n.id)
                },
                false,
            );
            // THE LEVEL ROW IS A MICROPHONE THAT IS OPEN, so it exists only
            // under the row the cursor is on, and the sentence says so.
            if kind == Kind::Source && self.listening == Some(n.id) {
                let bar = crate::chrome::meter(self.level, BAR_CELLS.min(b.room));
                b.level(b.texts.t(crate::key::AUDIO_LISTENING_NOW), bar);
            }
        }
    }

    /// The programs playing or recording, if any are.
    fn programs(&self, b: &mut Builder<'_>, s: &Snapshot) {
        b.heading(crate::key::AUDIO_HEADING_APPS);
        let mut any = false;
        for kind in [Kind::Playing, Kind::Recording] {
            for n in s.of(kind) {
                any = true;
                // AN APPLICATION'S OWN WORDS, untouched. `gtk-update` would
                // become `Gtk-update` under a capitalisation of ours, and the
                // notification list already keeps this rule.
                let mut label = n.app.clone().unwrap_or_else(|| n.description.clone());
                if kind == Kind::Recording {
                    label.push(' ');
                    label.push(tokens::icon::MICROPHONE);
                }
                b.pair(
                    label,
                    loudness(n.volume, n.muted, b.room, b.texts),
                    Target::Stream(n.id, n.volume),
                    Spot::Stream(n.id),
                    false,
                );
            }
        }
        if !any {
            b.note(crate::key::AUDIO_NOTHING_RUNS);
        }
    }

    /// The card behind whatever the cursor is on.
    fn card(&self, b: &mut Builder<'_>, s: &Snapshot) {
        let Some(device) = self.card_of(s) else {
            return;
        };
        b.heading(crate::key::AUDIO_HEADING_CARD);
        let profile = device
            .profile()
            .map_or_else(String::new, |p| p.description.clone());
        b.pair(
            b.texts.t(crate::key::AUDIO_PROFILE),
            profile,
            Target::Profile(device.id, device.active_profile),
            Spot::Profile,
            device.next_profile(true).is_none(),
        );
        let mut any = false;
        for way in [Direction::Output, Direction::Input] {
            let here: Vec<_> = device
                .routes
                .iter()
                .filter(|r| r.direction == way)
                .collect();
            if here.is_empty() {
                continue;
            }
            any = true;
            let now = device.route(way);
            let said = now.map_or_else(
                || b.texts.t(crate::key::AUDIO_PORT_NONE),
                |r| port_name(r, b.texts),
            );
            b.pair(
                // TWO SOCKETS, TWO WORDS. One label used for both directions
                // put two rows saying `Socket` one under the other, with
                // different values — the defect `labels-of-a-view-are-distinct`
                // exists for, read off the first photograph of this view.
                b.texts.t(if way == Direction::Output {
                    crate::key::AUDIO_SOCKET_OUT
                } else {
                    crate::key::AUDIO_SOCKET_IN
                }),
                said,
                Target::Port(device.id, way),
                Spot::Port(way),
                here.len() < 2,
            );
        }
        if !any {
            b.note(crate::key::AUDIO_CARD_OFF);
        }
    }

    /// Which card the cursor's row hangs off.
    fn card_of<'a>(&self, s: &'a Snapshot) -> Option<&'a ronoctua_audio::Device> {
        // THE SECTION FOLLOWS THE CURSOR. With nothing selected yet it is the
        // default sink's, which is what a person opening this view is most
        // likely asking about — but once they have moved, it is theirs.
        let node = self
            .on_a_node
            .or_else(|| s.default_of(Kind::Sink).map(|n| n.id))?;
        let device = s.nodes.iter().find(|n| n.id == node)?.device?;
        s.device(device)
    }

    /// This view's own settings.
    fn settings(&self, b: &mut Builder<'_>) {
        b.heading(crate::key::AUDIO_HEADING_SETTINGS);
        b.pair(
            b.texts.t(crate::key::AUDIO_SETTING_STEP),
            b.texts.t_with(
                crate::key::AUDIO_STEP_POINTS,
                &[("points", &self.prefs.step.to_string())],
            ),
            Target::Knob(Knob::Step),
            Spot::Knob(Knob::Step),
            false,
        );
        for (key, on, knob) in [
            (crate::key::AUDIO_SETTING_OVER, self.prefs.over, Knob::Over),
            (
                crate::key::AUDIO_SETTING_FLASH,
                self.prefs.flash,
                Knob::Flash,
            ),
        ] {
            b.pair(
                b.texts.t(key),
                b.texts.t(if on {
                    crate::key::AUDIO_ON
                } else {
                    crate::key::AUDIO_OFF
                }),
                Target::Knob(knob),
                Spot::Knob(knob),
                false,
            );
        }
    }
}

/// Which key a command id is.
///
/// THE ONE MAP, and both doors reach it. The footer reads a `Sym` and the
/// window reads a `Keysym`; what they share is the NOTATION in
/// [`OF_THE_VIEW`], so each door turns its own key into that notation and this
/// turns the notation into something the view understands. An id that grew a
/// notation and no arm here would be a key the hint line teaches and nothing
/// answers.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "audio-up" => Press::Up,
        "audio-down" => Press::Down,
        "audio-less" => Press::Less,
        "audio-more" => Press::More,
        "audio-enter" => Press::Enter,
        "audio-mute" => Press::Mute,
        "audio-default" => Press::Default,
        _ => return None,
    })
}

/// A key, as the view's press — the ONE reader both doors call: the window
/// with `door::keys::key_of`, the footer with the key its own reader built.
///
/// `Esc` is every level's way back, and the arrows and `RET` are read by their
/// symbol whatever is held, as both doors always read them. A LETTER is read
/// by its whole notation: `C-m` is not `m`. The footer used to
/// drop every modifier (`Key::simple`) and answer a letter held with `Ctrl` or
/// `Alt` as the bare letter, while the window read xkb's control character
/// and answered nothing — one key, two answers.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::{Key, Sym};
    let notation = match key.sym {
        Sym::Esc => return Some(Press::Escape),
        Sym::Char(_) => key.notation(),
        other => Key::simple(other).notation(),
    };
    pressed(&notation)
}

/// Which key a notation is, through [`OF_THE_VIEW`].
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
}

/// The sentence a socket with nothing in it answers with.
const SOCKET_EMPTY: &str = crate::key::AUDIO_UNPLUGGED;

/// The value of a device row: a bar, a number, or the word for silence.
fn loudness(percent: u8, muted: bool, room: u16, texts: &ronoctua_i18n::Texts) -> String {
    if muted {
        // SILENCE IS A WORD, not an empty bar. A bar at nothing and a device
        // that is off read the same, and only one of them comes back with a
        // key.
        return texts.t(crate::key::AUDIO_MUTED);
    }
    let bar = crate::chrome::meter(percent, BAR_CELLS.min(room));
    format!("{bar} {percent}%")
}

/// What a socket is called: our word if we have one, PipeWire's if not.
fn port_name(route: &ronoctua_audio::Route, texts: &ronoctua_i18n::Texts) -> String {
    let mut said = route
        .key()
        .map_or_else(|| route.description.clone(), |k| texts.t(k));
    if !route.available.usable() {
        said.push(' ');
        said.push(tokens::mark::SEPARATOR);
        said.push(' ');
        said.push_str(&texts.t(crate::key::AUDIO_UNPLUGGED));
    }
    said
}

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
            ink: Ink::Grid,
        }
    }

    fn heading(&mut self, key: &str) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: Spot::Nothing,
                disabled: false,
            });
        }
        self.out.push(Row {
            fields: vec![Self::piece(
                self.left,
                self.texts.t(key),
                tokens::color::TEXT_HEADING,
            )],
            focused: false,
            heading: true,
            target: Target::Nothing,
            spot: Spot::Nothing,
            disabled: false,
        });
    }

    /// A sentence that stands for a whole section.
    ///
    /// It is a HEADING and not a row, which is this view's one departure from
    /// `power.rs`: a cursor resting on "there is no microphone" would have no
    /// [`Spot`] to be found again by after the level row comes or goes, and
    /// there is nothing to do on it either.
    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        self.said(text, tokens::color::TEXT_DIM);
    }

    /// A second line under a fact, in the fact's own tone.
    fn aside(&mut self, text: String) {
        self.said(text, tokens::color::TEXT_DIM);
    }

    /// IT WRAPS, and it used to be cut.
    ///
    /// One `Row` per line, through the one function the eight views of this
    /// shape share — see [`crate::chrome::note_lines`] for why an ellipsis
    /// here takes exactly the half that matters.
    fn said(&mut self, text: String, colour: tokens::Color) {
        for line in crate::chrome::note_lines(&text, self.room) {
            self.out.push(Row {
                fields: vec![Self::piece(self.left, line, colour)],
                focused: false,
                heading: true,
                target: Target::Nothing,
                spot: Spot::Nothing,
                disabled: false,
            });
        }
    }

    /// The microphone's level, under the row it belongs to.
    ///
    /// A LABEL AND A VALUE, in the same two columns as every row above it —
    /// so the bar lines up with the volumes and the words with the names.
    /// That is also why the sentence is three words: read off the screen, a
    /// full one came out as `This microphone is open wh…`, because a label
    /// column is a label column whatever is written in it.
    fn level(&mut self, said: String, bar: String) {
        let value_at = crate::chrome::value_column(self.left, self.room);
        self.out.push(Row {
            fields: vec![
                Self::piece(
                    self.left,
                    layout::elide_end(&said, value_at.saturating_sub(self.left + 1)),
                    tokens::color::TEXT_DIM,
                ),
                Self::piece(value_at, bar, tokens::color::STATE_ACCENT),
            ],
            focused: false,
            heading: true,
            target: Target::Nothing,
            spot: Spot::Level,
            disabled: false,
        });
    }

    /// A row a key acts on.
    fn pair(&mut self, label: String, value: String, target: Target, spot: Spot, disabled: bool) {
        let focused = self.cursor == self.out.len();
        let kind = if disabled {
            crate::chrome::Kind::Disabled
        } else {
            crate::chrome::Kind::Setting
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(self.left, self.room, &label, &value, kind, focused),
            focused,
            heading: false,
            target,
            spot,
            disabled,
        });
    }
}

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "audio",
    commands: &[
        // `LEADER s a` — SOUND: what plays, what listens, and what each
        // program is doing with either. `s a a` opens it in the footer and
        // `s a w` in a window, the doubled letter and the `w` every view of
        // this shape uses.
        ronoctua_cmd::Command {
            id: "audio-open",
            label: crate::key::CMD_AUDIO_OPEN,
        },
        ronoctua_cmd::Command {
            id: "audio-window",
            label: crate::key::CMD_AUDIO_WINDOW,
        },
        // AND THE VIEW'S OWN GESTURES, reached through `audio::OF_THE_VIEW`.
        ronoctua_cmd::Command {
            id: "audio-up",
            label: crate::key::CMD_AUDIO_UP,
        },
        ronoctua_cmd::Command {
            id: "audio-down",
            label: crate::key::CMD_AUDIO_DOWN,
        },
        ronoctua_cmd::Command {
            id: "audio-enter",
            label: crate::key::CMD_AUDIO_ENTER,
        },
        ronoctua_cmd::Command {
            id: "audio-less",
            label: crate::key::CMD_AUDIO_LESS,
        },
        ronoctua_cmd::Command {
            id: "audio-more",
            label: crate::key::CMD_AUDIO_MORE,
        },
        ronoctua_cmd::Command {
            id: "audio-mute",
            label: crate::key::CMD_AUDIO_MUTE,
        },
        ronoctua_cmd::Command {
            id: "audio-default",
            label: crate::key::CMD_AUDIO_DEFAULT,
        },
    ],
    bindings: &[
        ("s a a", "audio-open", None),
        ("s a w", "audio-window", None),
    ],
    groups: &[("s a", crate::key::GROUP_AUDIO)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::audio::OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .collect()
}

/// One node of [`a_machine`], on its one card.
#[cfg(test)]
fn node(id: u32, kind: Kind, name: &str, volume: u8) -> ronoctua_audio::Node {
    ronoctua_audio::Node {
        id,
        kind: Some(kind),
        name: name.to_owned(),
        description: name.to_owned(),
        device: Some(52),
        volume,
        muted: false,
        app: None,
        address: None,
    }
}

/// A MACHINE OF ONE CARD — a speaker, two microphones, two sockets out —
/// built without PipeWire: what the view's tests and both doors' read, and
/// never this machine's.
#[cfg(test)]
pub(crate) fn a_machine() -> Snapshot {
    Snapshot {
        devices: vec![ronoctua_audio::Device {
            id: 52,
            name: "alsa_card".to_owned(),
            description: "The card".to_owned(),
            profiles: vec![
                ronoctua_audio::Profile {
                    index: 0,
                    name: "off".to_owned(),
                    description: "Off".to_owned(),
                    available: ronoctua_audio::Available::Yes,
                },
                ronoctua_audio::Profile {
                    index: 2,
                    name: "hifi".to_owned(),
                    description: "HiFi".to_owned(),
                    available: ronoctua_audio::Available::Yes,
                },
            ],
            active_profile: 2,
            routes: vec![
                ronoctua_audio::Route {
                    index: 0,
                    name: "[Out] Speaker".to_owned(),
                    description: "Speaker".to_owned(),
                    direction: Direction::Output,
                    available: ronoctua_audio::Available::Unknown,
                    devices: vec![0],
                },
                ronoctua_audio::Route {
                    index: 3,
                    name: "[Out] Headphones".to_owned(),
                    description: "Headphones".to_owned(),
                    direction: Direction::Output,
                    available: ronoctua_audio::Available::No,
                    devices: vec![3],
                },
            ],
            active_routes: vec![0],
        }],
        nodes: vec![
            node(60, Kind::Sink, "Speaker", 38),
            node(61, Kind::Source, "Stereo mic", 74),
            node(62, Kind::Source, "Digital mic", 100),
        ],
        defaults: ronoctua_audio::Defaults {
            sink: Some("Speaker".to_owned()),
            source: Some("Stereo mic".to_owned()),
            configured_sink: Some("Speaker".to_owned()),
            configured_source: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_audio::{Available, Node, Profile, Route};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn view() -> View {
        let mut v = View::fresh(Preferences::default());
        let s = a_machine();
        let t = texts();
        v.settle_in(Some(&s), DECIDING_WIDTH, &t);
        v
    }

    /// Walk the cursor down until the row carries `spot`.
    fn walk_to(v: &mut View, spot: Spot) {
        let s = a_machine();
        let t = texts();
        for _ in 0..200 {
            let rows = v.rows(Some(&s), DECIDING_WIDTH, &t);
            if rows.get(v.cursor()).is_some_and(|r| r.spot == spot) {
                return;
            }
            v.key(Press::Down, Some(&s), DECIDING_WIDTH, &t);
        }
        panic!("never reached {spot:?}");
    }

    #[test]
    fn the_cursor_starts_on_a_row_that_acts() {
        // ROW ZERO IS ALWAYS A HEADING, and a cursor there is a cursor on
        // nothing: every key does nothing and the screen reads as broken,
        // which is what the owner reported about the locale view.
        //
        // AND NOT MERELY ON A NON-HEADING. The first photograph of this view
        // landed on a fact, and the hint line then taught two keys — the two
        // that move the cursor — on a screen whose whole point is the
        // devices below.
        let v = view();
        let rows = v.rows(Some(&a_machine()), DECIDING_WIDTH, &texts());
        assert!(!rows[v.cursor()].heading);
        assert!(
            rows[v.cursor()].target.acts(),
            "it landed on {:?}, which does nothing",
            rows[v.cursor()].spot
        );
    }

    #[test]
    fn a_cursor_a_person_put_somewhere_stays_there() {
        // The rule above is about ARRIVING. Without that limit, a cursor
        // resting on a fact would be dragged off it every time PipeWire
        // reported anything — which is several times a minute.
        let mut v = view();
        let s = a_machine();
        let t = texts();
        walk_to(&mut v, Spot::Sink(60));
        let sink = v.cursor();
        v.settle_in(Some(&s), DECIDING_WIDTH, &t);
        assert_eq!(v.cursor(), sink);
    }

    #[test]
    fn the_card_is_the_one_the_cursor_is_on() {
        // THE DEFECT THIS PINS did not show on the machine it was written on,
        // which has ONE sound card: the section followed the DEFAULT sink,
        // so choosing an HDMI output and reading the card below it described
        // the built-in one — its profiles, its sockets — with nothing on
        // screen saying which card it was about.
        let mut two = a_machine();
        two.devices.push(ronoctua_audio::Device {
            id: 51,
            name: "hdmi".to_owned(),
            description: "The other card".to_owned(),
            profiles: vec![Profile {
                index: 0,
                name: "off".to_owned(),
                description: "Off".to_owned(),
                available: Available::Yes,
            }],
            active_profile: 0,
            routes: vec![Route {
                index: 0,
                name: "[Out] HDMI".to_owned(),
                description: "HDMI".to_owned(),
                direction: Direction::Output,
                available: Available::No,
                devices: vec![0],
            }],
            active_routes: vec![],
        });
        two.nodes.push(Node {
            id: 70,
            kind: Some(Kind::Sink),
            name: "hdmi-out".to_owned(),
            description: "HDMI out".to_owned(),
            device: Some(51),
            volume: 100,
            ..Node::default()
        });

        let t = texts();
        let mut v = View::fresh(Preferences::default());
        v.settle_in(Some(&two), DECIDING_WIDTH, &t);

        // THE PROFILE ROW IS THE UNAMBIGUOUS ONE: each card is in a different
        // profile, so the value says which card the section is about.
        let profile_of = |v: &View| {
            v.rows(Some(&two), DECIDING_WIDTH, &t)
                .iter()
                .find(|r| r.spot == Spot::Profile)
                .and_then(|r| r.fields.last())
                .map(|f| f.text.clone())
        };

        let rows = v.rows(Some(&two), DECIDING_WIDTH, &t);
        assert_eq!(rows[v.cursor()].spot, Spot::Sink(60));
        assert_eq!(profile_of(&v).as_deref(), Some("HiFi"));

        // Move to the other card's sink, and the section moves with it.
        v.key(Press::Down, Some(&two), DECIDING_WIDTH, &t);
        assert_eq!(
            v.rows(Some(&two), DECIDING_WIDTH, &t)[v.cursor()].spot,
            Spot::Sink(70)
        );
        assert_eq!(
            profile_of(&v).as_deref(),
            Some("Off"),
            "the card did not follow the cursor"
        );
    }

    #[test]
    fn the_two_sockets_do_not_say_the_same_word() {
        // A label used for both directions put two rows saying `Socket` one
        // under the other, with different values. Read off the first
        // photograph of this view.
        let v = view();
        let t = texts();
        let rows = v.rows(Some(&a_machine()), DECIDING_WIDTH, &t);
        let labels: Vec<&str> = rows
            .iter()
            .filter(|r| matches!(r.spot, Spot::Port(_)))
            .filter_map(|r| r.fields.first())
            .map(|f| f.text.as_str())
            .collect();
        let mut unique = labels.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), labels.len(), "{labels:?}");
    }

    #[test]
    fn the_chosen_device_is_said_where_the_list_it_is_about_is() {
        // The section that used to carry this repeated the list below it,
        // which is what the first photograph showed. The sentence stayed; the
        // section went.
        let v = view();
        let t = texts();
        let rows = v.rows(Some(&a_machine()), DECIDING_WIDTH, &t);
        let said: Vec<String> = rows
            .iter()
            .filter_map(|r| r.fields.first())
            .map(|f| f.text.clone())
            .collect();
        assert!(
            !said.iter().any(|w| w.contains("Chosen")),
            "the machine's chosen sink is the one playing, so nothing is said"
        );

        let mut adrift = a_machine();
        adrift.defaults.configured_sink = Some("a headset that is off".to_owned());
        let rows = v.rows(Some(&adrift), DECIDING_WIDTH, &t);
        let at = rows
            .iter()
            .position(|r| {
                r.fields
                    .first()
                    .is_some_and(|f| f.text.contains("a headset that is off"))
            })
            .expect("the sentence");
        let heading = rows[..at]
            .iter()
            .rev()
            .find(|r| r.heading && !r.fields.is_empty())
            .and_then(|r| r.fields.first())
            .map(|f| f.text.clone())
            .expect("a heading above it");
        assert_eq!(heading, t.t(crate::key::AUDIO_HEADING_OUTPUT));
    }

    #[test]
    fn the_things_a_key_can_act_on_are_the_same_at_every_width() {
        // WHAT A KEY MAY NOT DEPEND ON.
        //
        // A key acts on the row under the cursor, and the cursor is an index
        // into the rows that were DRAWN — so `View::key` is given the DOOR's
        // width and decides against that same list. What this holds down is
        // the half that no width may change: WHICH things can be acted on, and
        // in what ORDER.
        //
        // The row COUNT is allowed to differ now, and that is not a weakening:
        // a note WRAPS (`chrome::note_lines`), so one sentence is a row on a
        // wide screen and three on a narrow one. It used to be cut instead,
        // which kept the count the same and took the end off every explanation
        // this view has. The FIELDS are allowed to differ too — that is what
        // eliding is.
        let v = view();
        let s = a_machine();
        let t = texts();
        let want = |columns: u16| -> Vec<(Target, Spot)> {
            v.rows(Some(&s), columns, &t)
                .into_iter()
                .filter(|r| !r.heading)
                .map(|r| (r.target, r.spot))
                .collect()
        };
        let narrow = want(40);
        assert!(narrow.len() > 5, "only {} rows to stop on", narrow.len());
        for width in [DECIDING_WIDTH, 60, 100, 200] {
            assert_eq!(want(width), narrow, "at {width}");
        }
    }

    #[test]
    fn every_row_the_cursor_can_stop_on_has_its_own_identity() {
        // WHAT THE RE-FINDING DEPENDS ON. Two focusable rows sharing a `Spot`
        // would make the cursor jump to the first of them after a rebuild.
        let v = view();
        let rows = v.rows(Some(&a_machine()), DECIDING_WIDTH, &texts());
        let mut spots: Vec<Spot> = rows.iter().filter(|r| !r.heading).map(|r| r.spot).collect();
        assert!(!spots.is_empty());
        assert!(
            !spots.contains(&Spot::Nothing),
            "a focusable row with no identity"
        );
        let before = spots.len();
        spots.sort_by_key(|s| format!("{s:?}"));
        spots.dedup_by_key(|s| format!("{s:?}"));
        assert_eq!(spots.len(), before, "two rows share an identity");
    }

    #[test]
    fn the_meter_follows_the_cursor_and_the_row_comes_with_it() {
        let mut v = view();
        let s = a_machine();
        let t = texts();
        assert_eq!(v.listening(), None, "nothing is open to begin with");

        walk_to(&mut v, Spot::Source(61));
        assert_eq!(v.listening(), Some(61));
        let rows = v.rows(Some(&s), DECIDING_WIDTH, &t);
        let at = rows
            .iter()
            .position(|r| r.spot == Spot::Source(61))
            .unwrap();
        assert_eq!(rows[at + 1].spot, Spot::Level, "the level row is under it");
        assert_eq!(v.cursor(), at, "and the cursor is still on the source");

        // THE DEFECT THIS PINS: the level row shifts everything below it, so
        // a cursor moved by INDEX across the rebuild lands a row out. One
        // press down from the first microphone has to reach the SECOND.
        v.key(Press::Down, Some(&s), DECIDING_WIDTH, &t);
        assert_eq!(v.listening(), Some(62));
        let rows = v.rows(Some(&s), DECIDING_WIDTH, &t);
        assert_eq!(rows[v.cursor()].spot, Spot::Source(62));
        assert_eq!(
            rows.iter().filter(|r| r.spot == Spot::Level).count(),
            1,
            "one microphone open at a time"
        );
    }

    #[test]
    fn walking_off_the_microphones_closes_the_meter() {
        let mut v = view();
        let s = a_machine();
        let t = texts();
        walk_to(&mut v, Spot::Source(62));
        assert_eq!(v.listening(), Some(62));
        // Down from the last microphone leaves the section.
        v.key(Press::Down, Some(&s), DECIDING_WIDTH, &t);
        assert_eq!(v.listening(), None, "nothing holds it open now");
        let rows = v.rows(Some(&s), DECIDING_WIDTH, &t);
        assert!(!rows.iter().any(|r| r.spot == Spot::Level));
    }

    #[test]
    fn a_key_on_a_device_asks_for_the_right_thing() {
        let mut v = view();
        let s = a_machine();
        let t = texts();
        walk_to(&mut v, Spot::Sink(60));

        assert_eq!(
            v.key(Press::More, Some(&s), DECIDING_WIDTH, &t),
            Errand::Order(Order::Volume {
                node: 60,
                percent: 40
            }),
            "the step walks the grid from 38"
        );
        assert_eq!(
            v.key(Press::Mute, Some(&s), DECIDING_WIDTH, &t),
            Errand::Order(Order::Mute {
                node: 60,
                muted: true
            })
        );
        // RET ON A DEVICE IS "PLAY THROUGH THIS ONE", and it takes the NODE —
        // `set-default` and `set-volume` both do, while `set-profile` takes
        // the card.
        assert_eq!(
            v.key(Press::Enter, Some(&s), DECIDING_WIDTH, &t),
            Errand::Order(Order::Default { node: 60 })
        );
    }

    #[test]
    fn the_socket_with_nothing_in_it_is_drawn_and_refuses() {
        let mut v = view();
        let s = a_machine();
        let t = texts();
        walk_to(&mut v, Spot::Port(Direction::Output));
        // The ring holds both sockets; the headphone one reports `no`.
        assert_eq!(
            v.key(Press::More, Some(&s), DECIDING_WIDTH, &t),
            Errand::Speak(SOCKET_EMPTY),
            "it says why rather than doing nothing"
        );
    }

    #[test]
    fn the_hint_teaches_only_what_works_here() {
        let mut v = view();
        let s = a_machine();
        let t = texts();
        walk_to(&mut v, Spot::Sink(60));
        let on_a_device = v.hint(Some(&s), DECIDING_WIDTH, &t).len();
        walk_to(&mut v, Spot::Knob(Knob::Step));
        let on_a_setting = v.hint(Some(&s), DECIDING_WIDTH, &t).len();
        assert!(
            on_a_setting < on_a_device,
            "a setting has no `m` and no `d`: {on_a_setting} against {on_a_device}"
        );
    }

    #[test]
    fn a_setting_turns_and_asks_to_be_written() {
        let mut v = view();
        let s = a_machine();
        let t = texts();
        walk_to(&mut v, Spot::Knob(Knob::Flash));
        assert!(v.preferences().flash);
        let errand = v.key(Press::Enter, Some(&s), DECIDING_WIDTH, &t);
        assert_eq!(errand, Errand::Keep(v.preferences()));
        assert!(!v.preferences().flash, "and RET turned it");
    }

    #[test]
    fn with_no_sound_server_the_screen_says_so() {
        let v = View::fresh(Preferences::default());
        let rows = v.rows(None, DECIDING_WIDTH, &texts());
        assert_eq!(rows.len(), 2, "a heading and a sentence");
        assert!(rows.iter().all(|r| r.heading), "and nothing to act on");
    }

    #[test]
    fn every_key_the_hint_teaches_is_a_key_the_view_answers() {
        // A NOTATION WITH NO ARM would be a key on the hint line that does
        // nothing, and the hint line is generated so it cannot say otherwise.
        for (notation, id, _) in OF_THE_VIEW {
            assert!(press_of(id).is_some(), "`{id}` is taught and has no press");
            assert_eq!(
                pressed(notation),
                press_of(id),
                "`{notation}` and `{id}` disagree"
            );
        }
    }

    #[test]
    fn both_doors_reach_every_key_the_table_holds() {
        // WHAT BOTH DOORS DO, spelled out: each builds a `Key` — the footer
        // with its reader, the window with `door::keys::key_of` — and asks
        // `press_of_key`. Every row of the table is its own press.
        for (notation, id, _) in OF_THE_VIEW {
            let key = ronoctua_keymap::Key::parse(notation)
                .unwrap_or_else(|_| panic!("`{notation}` is not a key"));
            assert_eq!(
                press_of_key(key),
                press_of(id),
                "`{notation}` does not reach `{id}`"
            );
        }
        assert_eq!(
            press_of_key(ronoctua_keymap::Key::parse("ESC").expect("ESC")),
            Some(Press::Escape)
        );
    }

    /// A LETTER HELD WITH `Ctrl` OR `Alt` IS NOT THE LETTER. The footer
    /// dropped every modifier and muted a device on `C-m` and made it the
    /// default on `M-d`, while the window answered nothing for the same keys.
    #[test]
    fn a_chord_is_not_the_letter_under_it() {
        let read = |n: &str| press_of_key(ronoctua_keymap::Key::parse(n).expect("a key"));
        assert_eq!(read("m"), Some(Press::Mute));
        assert_eq!(read("d"), Some(Press::Default));
        for chord in ["C-m", "M-m", "C-d", "M-d", "s-d", "C-M-d"] {
            assert_eq!(read(chord), None, "`{chord}` was taken for its letter");
        }
        // THE ARROWS AND `RET` are read by their symbol, as both doors
        // always read them.
        assert_eq!(read("S-<down>"), Some(Press::Down));
        assert_eq!(read("C-RET"), Some(Press::Enter));
    }

    #[test]
    fn a_muted_device_says_a_word_and_not_an_empty_bar() {
        // An empty bar and a device that is off read the same, and only one
        // of them comes back with a key.
        let t = texts();
        let quiet = loudness(0, false, 60, &t);
        let off = loudness(38, true, 60, &t);
        assert!(quiet.contains(tokens::mark::METER_EMPTY));
        assert!(!off.contains(tokens::mark::METER_EMPTY));
        assert_ne!(quiet, off);
    }
}
