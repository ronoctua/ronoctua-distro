//! Mice, touchpads and every cursor — one view, two doors (`LEADER s i`).
//!
//! # What is here
//!
//! Which rows exist, what a key does on each, and which order goes to the
//! pointer thread as a result. Nothing here reads a device or writes a file:
//! `crates/ronoctua-pointers` is the boundary, and `pointers_window.rs` is the
//! second door. It is the shape `audio.rs` has, down to the rings.
//!
//! # Every value is a ring
//!
//! `←`/`→` turn the value on the row and `RET` turns it forward, because
//! `RET` is what a CLICK becomes and a row only reachable with an arrow is a
//! row the mouse cannot use (invariant 19). A setting a device does not have
//! is not drawn at all — the compositor reports only what libinput says the
//! device supports, and a screen that offered the rest would offer switches
//! that do nothing.
//!
//! # A cursor's heading wears its colour
//!
//! Each cursor's glyph is painted in the colour that cursor has on the desk
//! (`cursor.fill.*`), which is how a person reading the list knows which of
//! the cursors is the one in their hand.

use crate::files::{Field, Hit as FilesHit, Ink};
use ronoctua_compositor::{
    AccelProfile, ButtonMap, ClickMethod, DeviceKind, DragLock, FingerDrag, FocusFollows,
    PointerSettings, ScrollMethod, SendEvents, Setting,
};
use ronoctua_pointers::{FIRST, Order, Place, Snapshot, prefs};
use ronoctua_tokens as tokens;

/// The keys this view teaches, and where each one applies.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "pointers-down", Where::Anywhere),
    ("<up>", "pointers-up", Where::Anywhere),
    ("<left>", "pointers-less", Where::OnAValue),
    ("<right>", "pointers-more", Where::OnAValue),
    ("RET", "pointers-enter", Where::OnAValue),
    ("d", "pointers-forget", Where::OnAnAbsentDevice),
];

/// Which rows a key means anything on — the hint line teaches only the keys
/// that WORK where the cursor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns.
    OnAValue,
    /// On a device that is not connected.
    OnAnAbsentDevice,
}

impl Where {
    fn shown(self, target: &Target) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnAValue => target.turns(),
            Self::OnAnAbsentDevice => matches!(target, Target::Away(_)),
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
    /// What a click is: the next value, or a copy of an informative row.
    Enter,
    /// Forget a device that is not connected.
    Forget,
    /// Back one level, and out.
    Escape,
}

/// What a row is, and what it holds now — so a key knows what it turns.
#[derive(Debug, Clone, PartialEq)]
pub enum Target {
    /// Nothing at all.
    Nothing,
    /// Whether a new device gets its own cursor.
    Detect(bool),
    /// Whether a virtual machine's own pointers move.
    Emulated(bool),
    /// Whether every mouse and touchpad moves one cursor.
    Together(bool),
    /// A cursor switched on or off.
    CursorOn(u16, bool),
    /// A cursor's colour.
    CursorColour(u16, u8),
    /// Which cursor a device drives.
    Place(String, u16),
    /// A device switched on or off.
    DeviceOn(String, bool),
    /// One setting — of one device, or of every device of a kind.
    Setting(Option<String>, DeviceKind, Setting),
    /// The cursor size.
    Size(u16),
    /// How long a still cursor waits before hiding.
    HideAfter(u32),
    /// Whether the cursor hides while typing.
    HideWhileTyping(bool),
    /// Whether focus follows the pointer.
    Focus(FocusFollows),
    /// Which cursor the keyboards type through.
    Typing(ronoctua_pointers::Typing),
    /// A device that is not connected.
    Away(String),
    /// The door to the keyboards' own screen (`LEADER s k`).
    Keyboards,
}

impl Target {
    /// Whether `←`/`→` mean anything here.
    const fn turns(&self) -> bool {
        !matches!(self, Self::Nothing | Self::Away(_) | Self::Keyboards)
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
    pub target: TargetCell,
    /// Its identity across a rebuild.
    pub spot: String,
}

/// A [`Target`] in a row that has to be `Eq` — the settings carry floats, and
/// a row is compared by what it DRAWS, which the target is not part of.
#[derive(Debug, Clone)]
pub struct TargetCell(pub Target);

impl PartialEq for TargetCell {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for TargetCell {}

/// What the door has to do about a key.
#[derive(Debug, Clone, PartialEq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Put this on the clipboard.
    Copy(String),
    /// Ask the pointer thread.
    Order(Order),
    /// Shut the view.
    Close,
    /// Run a command of the registry — the door to the keyboards' screen.
    Run(&'static str),
}

/// Where the rows start and how wide they are — the one arithmetic every
/// view of this shape shares.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    crate::power::extent(columns)
}

/// The view.
#[derive(Debug, Default)]
pub struct View {
    /// Which row the cursor is on.
    cursor: usize,
    /// Whether anybody has moved the cursor yet — see `audio::View::moved`.
    moved: bool,
}

impl View {
    /// A view with the cursor on the first row that does something.
    #[must_use]
    pub fn fresh() -> Self {
        Self {
            cursor: 1,
            moved: false,
        }
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(snap, columns, texts);
        let target = rows
            .get(self.cursor)
            .map_or(Target::Nothing, |r| r.target.0.clone());
        let here: Vec<(&str, &str)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(&target))
            .map(|(k, id, _)| (*k, *id))
            .collect();
        crate::chrome::hint_of(&here, texts)
    }

    /// Put the cursor on a row, if that row is one.
    ///
    /// Answers whether the cursor was ALREADY on that row: that is the second
    /// click, which the door turns into `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(|r| !r.heading) {
            let again = self.cursor == n;
            self.moved = true;
            self.cursor = n;
            return again;
        }
        false
    }

    /// Put the cursor somewhere real after the rows changed under it.
    pub fn settle_in(
        &mut self,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(snap, columns, texts);
        self.clamp(&rows);
        if !self.moved
            && rows.get(self.cursor).is_some_and(|r| !r.target.0.turns())
            && let Some(i) = rows.iter().position(|r| !r.heading && r.target.0.turns())
        {
            self.cursor = i;
        }
    }

    /// Keep the cursor on the row it was on, by identity, when the rows
    /// were rebuilt from a new reading.
    pub fn follow(
        &mut self,
        spot: &str,
        snap: Option<&Snapshot>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        let rows = self.rows(snap, columns, texts);
        if let Some(i) = rows.iter().position(|r| r.spot == spot) {
            self.cursor = i;
        }
        self.clamp(&rows);
    }

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

    /// A key. The width is the door's — see `audio::View::key`.
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
            return if press == Press::Escape {
                Errand::Close
            } else {
                Errand::Nothing
            };
        }
        let target = rows[self.cursor].target.0.clone();
        match press {
            Press::Escape => return Errand::Close,
            Press::Up | Press::Down => {
                self.moved = true;
                self.walk(&rows, press == Press::Down);
                return Errand::Nothing;
            }
            _ => {}
        }
        let forward = press != Press::Less;
        match (press, target) {
            (Press::Forget, Target::Away(id)) => Errand::Order(Order::Forget(id)),
            (Press::Enter, Target::Away(_) | Target::Nothing) => rows
                .get(self.cursor)
                .and_then(|r| crate::chrome::information_of(&r.fields))
                .map_or(Errand::Nothing, Errand::Copy),
            (Press::Forget, _) | (_, Target::Nothing | Target::Away(_)) => Errand::Nothing,
            (_, Target::Detect(on)) => Errand::Order(Order::Detect(!on)),
            (_, Target::Emulated(on)) => Errand::Order(Order::Emulated(!on)),
            // A DOOR OPENS ON `RET` — which is also the second click — and the
            // arrows have nothing to turn on it.
            (Press::Enter, Target::Keyboards) => Errand::Run(KEYBOARDS),
            (_, Target::Keyboards) => Errand::Nothing,
            (_, Target::Together(on)) => Errand::Order(Order::Together(!on)),
            (_, Target::CursorOn(n, on)) => Errand::Order(Order::CursorOn(n, !on)),
            (_, Target::CursorColour(n, c)) => {
                Errand::Order(Order::CursorColour(n, turned(c, prefs::COLOURS, forward)))
            }
            (_, Target::Place(id, now)) => {
                let numbers: Vec<u16> = snap
                    .map(|s| s.plan.cursors.iter().map(|c| c.number).collect())
                    .unwrap_or_default();
                Errand::Order(Order::DevicePlace(id, next_place(&numbers, now, forward)))
            }
            (_, Target::DeviceOn(id, on)) => Errand::Order(Order::DeviceOn(id, !on)),
            (_, Target::Setting(device, kind, s)) => {
                let next = next_setting(s, forward);
                Errand::Order(match device {
                    Some(id) => Order::DeviceSetting(id, next),
                    None => Order::KindSetting(kind, next),
                })
            }
            (_, Target::Size(now)) => {
                let at = prefs::SIZES.iter().position(|s| *s == now).unwrap_or(0);
                let n = u8::try_from(prefs::SIZES.len()).unwrap_or(1);
                let i = usize::from(turned(u8::try_from(at).unwrap_or(0), n, forward));
                Errand::Order(Order::Size(prefs::SIZES[i]))
            }
            (_, Target::HideAfter(now)) => {
                let at = HIDE_AFTER.iter().position(|s| *s == now).unwrap_or(0);
                let n = u8::try_from(HIDE_AFTER.len()).unwrap_or(1);
                let i = usize::from(turned(u8::try_from(at).unwrap_or(0), n, forward));
                Errand::Order(Order::HideAfter(HIDE_AFTER[i]))
            }
            (_, Target::HideWhileTyping(on)) => Errand::Order(Order::HideWhileTyping(!on)),
            (_, Target::Focus(f)) => {
                const RING: [FocusFollows; 3] =
                    [FocusFollows::No, FocusFollows::Yes, FocusFollows::Always];
                let at = RING.iter().position(|x| *x == f).unwrap_or(1);
                let i = usize::from(turned(u8::try_from(at).unwrap_or(0), 3, forward));
                Errand::Order(Order::Focus(RING[i]))
            }
            (_, Target::Typing(now)) => {
                let ring = typing_ring(snap);
                let at = ring.iter().position(|t| *t == now).unwrap_or(0);
                let n = u8::try_from(ring.len()).unwrap_or(1);
                let i = usize::from(turned(u8::try_from(at).unwrap_or(0), n, forward));
                Errand::Order(Order::Typing(ring[i]))
            }
        }
    }

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
            b.heading(ronoctua_i18n::key::POINTERS_TITLE);
            b.note(ronoctua_i18n::key::POINTERS_NO_COMPOSITOR);
            return b.out;
        };
        cursors(&mut b, s);
        for kind in [DeviceKind::Mouse, DeviceKind::Touchpad] {
            devices(&mut b, s, kind);
        }
        general(&mut b, s);
        away(&mut b, s);
        keyboards(&mut b);
        b.out
    }
}

/// The waits a still cursor can hide after, in milliseconds; `0` is never.
const HIDE_AFTER: [u32; 5] = [0, 1_000, 3_000, 5_000, 10_000];

/// The numbers this screen draws — see [`ronoctua_pointers::Shown`].
fn shown_of(s: &Snapshot) -> ronoctua_pointers::Shown {
    let keyboards: Vec<u16> = s.keyboards.iter().map(|k| k.cursor).collect();
    ronoctua_pointers::Shown::of(&s.plan, &s.prefs, &keyboards)
}

/// "cursor N" for kept number `n`, drawn as the screen numbers it — or, for
/// a cursor that is not there now, "a cursor of its own".
fn on_cursor(b: &Builder<'_>, shown: &ronoctua_pointers::Shown, n: u16) -> String {
    shown.label(n).map_or_else(
        || b.texts.t(ronoctua_i18n::key::POINTERS_OWN_CURSOR),
        |l| {
            b.texts.t_with(
                ronoctua_i18n::key::POINTERS_ON_CURSOR_VALUE,
                &[("number", &l.to_string())],
            )
        },
    )
}

/// The cursors, and the devices on each.
fn cursors(b: &mut Builder<'_>, s: &Snapshot) {
    let shown = shown_of(s);
    b.heading(ronoctua_i18n::key::POINTERS_HEADING_CURSORS);
    b.note(ronoctua_i18n::key::POINTERS_ABOUT_CURSORS);
    if !s.connected {
        b.note(ronoctua_i18n::key::POINTERS_NO_COMPOSITOR);
    }
    if s.settings_failed {
        b.note(ronoctua_i18n::key::POINTERS_SETTINGS_FAILED);
    }
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_TOGETHER),
        on_off(s.prefs.together, b.texts),
        Target::Together(s.prefs.together),
        "together".to_owned(),
    );
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_DETECT),
        on_off(s.prefs.detect, b.texts),
        Target::Detect(s.prefs.detect),
        "detect".to_owned(),
    );
    // ONLY WHERE THERE IS ONE: a switch for devices a machine does not have
    // is a row that changes nothing.
    if s.plan.devices.iter().any(|d| d.emulated) {
        b.note(ronoctua_i18n::key::POINTERS_ABOUT_EMULATED);
        b.pair(
            b.texts.t(ronoctua_i18n::key::POINTERS_EMULATED),
            on_off(s.prefs.emulated, b.texts),
            Target::Emulated(s.prefs.emulated),
            "emulated".to_owned(),
        );
    }
    for c in &s.plan.cursors {
        let title = b.texts.t_with(
            ronoctua_i18n::key::POINTERS_CURSOR,
            &[(
                "number",
                &shown.label(c.number).unwrap_or(c.number).to_string(),
            )],
        );
        b.under(
            tokens::icon::POINTERS_CURSOR,
            fill_of(c.colour),
            title,
            if !c.shown {
                Some(b.texts.t(ronoctua_i18n::key::POINTERS_NOT_DRAWN))
            } else {
                (c.number == FIRST).then(|| b.texts.t(ronoctua_i18n::key::POINTERS_IS_FIRST))
            },
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::POINTERS_CURSOR_ON),
            on_off(c.enabled, b.texts),
            Target::CursorOn(c.number, c.enabled),
            format!("cursor-on-{}", c.number),
        );
        b.pair(
            b.texts.t(ronoctua_i18n::key::POINTERS_COLOUR),
            b.texts.t_with(
                ronoctua_i18n::key::POINTERS_COLOUR_VALUE,
                &[("number", &(u16::from(c.colour) + 1).to_string())],
            ),
            Target::CursorColour(c.number, c.colour),
            format!("cursor-colour-{}", c.number),
        );
        for id in &c.devices {
            let name = s
                .plan
                .devices
                .iter()
                .find(|d| &d.id == id)
                .map_or_else(|| id.clone(), |d| d.name.clone());
            // ONE CURSOR FOR EVERY HAND: where a device goes is kept and
            // not used, so the row says where it is and moves nothing.
            let target = if s.prefs.together {
                Target::Nothing
            } else {
                Target::Place(id.clone(), c.number)
            };
            b.pair(
                name,
                on_cursor(b, &shown, c.number),
                target,
                format!("place-{id}"),
            );
        }
    }
}

/// The devices of one kind: the settings for every one of them, then each.
fn devices(b: &mut Builder<'_>, s: &Snapshot, kind: DeviceKind) {
    let here: Vec<_> = s.plan.devices.iter().filter(|d| d.kind == kind).collect();
    let (heading, about, every, glyph, defaults) = match kind {
        DeviceKind::Touchpad => (
            ronoctua_i18n::key::POINTERS_HEADING_TOUCHPADS,
            ronoctua_i18n::key::POINTERS_ABOUT_TOUCHPADS,
            ronoctua_i18n::key::POINTERS_EVERY_TOUCHPAD,
            tokens::icon::POINTERS_TOUCHPAD,
            &s.prefs.touchpads,
        ),
        _ => (
            ronoctua_i18n::key::POINTERS_HEADING_MICE,
            ronoctua_i18n::key::POINTERS_ABOUT_MICE,
            ronoctua_i18n::key::POINTERS_EVERY_MOUSE,
            tokens::icon::POINTERS_MOUSE,
            &s.prefs.mice,
        ),
    };
    b.heading(heading);
    if here.is_empty() {
        b.note(ronoctua_i18n::key::POINTERS_NO_DEVICES);
        return;
    }
    b.note(about);
    // "EVERY MOUSE": the settings any mouse here has, each showing the value
    // chosen for the kind, or — when none was chosen — what the first mouse
    // that has it is doing, which is what the kind's default leaves alone.
    b.under(glyph, tokens::color::TEXT_SECONDARY, b.texts.t(every), None);
    let mut shown = PointerSettings::default();
    for d in &here {
        for setting in d.now.present() {
            if !shown.has(setting) {
                shown.set(
                    defaults
                        .present()
                        .into_iter()
                        .find(|x| same(*x, setting))
                        .unwrap_or(setting),
                );
            }
        }
    }
    for setting in shown.present() {
        if kind == DeviceKind::Mouse && matches!(setting, Setting::SendEvents(_)) {
            continue;
        }
        b.setting(None, kind, setting);
    }
    for d in here {
        b.under(glyph, tokens::color::TEXT_SECONDARY, d.name.clone(), None);
        b.pair(
            b.texts.t(ronoctua_i18n::key::POINTERS_DEVICE_ON),
            on_off(d.enabled, b.texts),
            Target::DeviceOn(d.id.clone(), d.enabled),
            format!("device-on-{}", d.id),
        );
        let effective = d.now.under(&d.settings);
        for setting in effective.present() {
            // A MOUSE'S `events` IS SAID BY "SWITCHED ON" — one switch for one
            // thing. A touchpad's has a second value worth its own row.
            if matches!(setting, Setting::SendEvents(_)) && kind == DeviceKind::Mouse {
                continue;
            }
            if let Setting::SendEvents(SendEvents::Disabled) = setting {
                continue;
            }
            b.setting(Some(d.id.clone()), kind, setting);
        }
    }
}

/// The settings every cursor shares.
fn general(b: &mut Builder<'_>, s: &Snapshot) {
    b.heading(ronoctua_i18n::key::POINTERS_HEADING_GENERAL);
    b.note(ronoctua_i18n::key::POINTERS_ABOUT_GENERAL);
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_SIZE),
        b.texts.t_with(
            ronoctua_i18n::key::POINTERS_SIZE_VALUE,
            &[("pixels", &s.prefs.size.to_string())],
        ),
        Target::Size(s.prefs.size),
        "size".to_owned(),
    );
    let hide = if s.prefs.hide_after_ms == 0 {
        b.texts.t(ronoctua_i18n::key::POINTERS_NEVER)
    } else {
        b.texts.t_with(
            ronoctua_i18n::key::POINTERS_SECONDS,
            &[("seconds", &(s.prefs.hide_after_ms / 1000).to_string())],
        )
    };
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_HIDE_AFTER),
        hide,
        Target::HideAfter(s.prefs.hide_after_ms),
        "hide-after".to_owned(),
    );
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_HIDE_TYPING),
        on_off(s.prefs.hide_while_typing, b.texts),
        Target::HideWhileTyping(s.prefs.hide_while_typing),
        "hide-typing".to_owned(),
    );
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_FOCUS),
        b.texts.t(match s.prefs.focus {
            FocusFollows::No => ronoctua_i18n::key::POINTERS_FOCUS_NO,
            FocusFollows::Yes => ronoctua_i18n::key::POINTERS_FOCUS_YES,
            FocusFollows::Always => ronoctua_i18n::key::POINTERS_FOCUS_ALWAYS,
        }),
        Target::Focus(s.prefs.focus),
        "focus".to_owned(),
    );
    b.note(ronoctua_i18n::key::POINTERS_ABOUT_TYPING);
    let typing = match s.prefs.typing {
        ronoctua_pointers::Typing::First => b.texts.t(ronoctua_i18n::key::POINTERS_TYPING_FIRST),
        ronoctua_pointers::Typing::Cursor(n) => shown_of(s).label(n).map_or_else(
            || b.texts.t(ronoctua_i18n::key::POINTERS_OWN_CURSOR),
            |l| {
                b.texts.t_with(
                    ronoctua_i18n::key::POINTERS_TYPING_CURSOR,
                    &[("number", &l.to_string())],
                )
            },
        ),
        ronoctua_pointers::Typing::Latest => b.texts.t(ronoctua_i18n::key::POINTERS_TYPING_LATEST),
    };
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_TYPING),
        typing,
        Target::Typing(s.prefs.typing),
        "typing".to_owned(),
    );
}

/// Where typing can go: the first cursor, every other cursor there is, and
/// the last one to pick a window — in that order, so the way back to the
/// default is always "left until it stops".
fn typing_ring(snap: Option<&Snapshot>) -> Vec<ronoctua_pointers::Typing> {
    let mut ring = vec![ronoctua_pointers::Typing::First];
    if let Some(s) = snap {
        ring.extend(
            s.plan
                .cursors
                .iter()
                .map(|c| c.number)
                .filter(|n| *n != ronoctua_pointers::FIRST)
                .map(ronoctua_pointers::Typing::Cursor),
        );
        // A CURSOR CHOSEN AND SINCE EMPTIED stays on the ring, or the value
        // drawn would be one the ring cannot reach and the next turn would
        // jump to the start.
        if let ronoctua_pointers::Typing::Cursor(n) = s.prefs.typing
            && !ring.contains(&s.prefs.typing)
            && n != ronoctua_pointers::FIRST
        {
            ring.push(s.prefs.typing);
        }
    }
    ring.push(ronoctua_pointers::Typing::Latest);
    ring
}

/// The devices remembered and not plugged in.
fn away(b: &mut Builder<'_>, s: &Snapshot) {
    let gone: Vec<_> = s
        .prefs
        .devices
        .iter()
        .filter(|(id, _)| !s.plan.devices.iter().any(|d| &&d.id == id))
        .collect();
    if gone.is_empty() {
        return;
    }
    b.heading(ronoctua_i18n::key::POINTERS_HEADING_AWAY);
    b.note(ronoctua_i18n::key::POINTERS_ABOUT_AWAY);
    let shown = shown_of(s);
    for (id, d) in gone {
        let name = if d.name.is_empty() {
            id.clone()
        } else {
            d.name.clone()
        };
        let value = d
            .cursor
            .map_or_else(String::new, |n| on_cursor(b, &shown, n));
        b.pair(name, value, Target::Away(id.clone()), format!("away-{id}"));
    }
}

/// The command behind the door to the keyboards' screen.
const KEYBOARDS: &str = "keyboard-open";

/// Last, the door to the keyboards: a keyboard can type through a cursor of
/// its own, and that is set on THEIR screen — which this one would otherwise
/// never mention.
fn keyboards(b: &mut Builder<'_>) {
    b.heading(ronoctua_i18n::key::POINTERS_HEADING_KEYBOARDS);
    b.note(ronoctua_i18n::key::POINTERS_ABOUT_KEYBOARDS);
    // AN ACTION HAS NO VALUE: the value column is the row's answer everywhere
    // else on this screen, and a door has none.
    b.pair(
        b.texts.t(ronoctua_i18n::key::POINTERS_OPEN_KEYBOARDS),
        String::new(),
        Target::Keyboards,
        "keyboards".to_owned(),
    );
}

/// `n` turned one step round a ring of `size`.
const fn turned(n: u8, size: u8, forward: bool) -> u8 {
    if size == 0 {
        return 0;
    }
    if forward {
        (n + 1) % size
    } else {
        (n + size - 1) % size
    }
}

/// The cursor after `now` round the existing ones — and, last, a new one.
fn next_place(numbers: &[u16], now: u16, forward: bool) -> Place {
    let mut ring: Vec<Option<u16>> = numbers.iter().copied().map(Some).collect();
    ring.push(None);
    let at = ring.iter().position(|n| *n == Some(now)).unwrap_or(0);
    let size = ring.len();
    let i = if forward {
        (at + 1) % size
    } else {
        (at + size - 1) % size
    };
    ring[i].map_or(Place::New, Place::Cursor)
}

/// Whether two settings are the same SETTING, whatever their values.
fn same(a: Setting, b: Setting) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

/// A setting turned one step round its own ring.
#[must_use]
pub fn next_setting(s: Setting, forward: bool) -> Setting {
    fn step<T: Copy + PartialEq>(ring: &[T], now: T, forward: bool) -> T {
        let at = ring.iter().position(|x| *x == now).unwrap_or(0);
        let n = ring.len();
        ring[if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        }]
    }
    fn nearest(ring: &[f64], now: f64) -> usize {
        ring.iter()
            .enumerate()
            .min_by(|a, b| (a.1 - now).abs().total_cmp(&(b.1 - now).abs()))
            .map_or(0, |(i, _)| i)
    }
    fn stepf(ring: &[f64], now: f64, forward: bool) -> f64 {
        let at = nearest(ring, now);
        let n = ring.len();
        ring[if forward {
            (at + 1).min(n - 1)
        } else {
            at.saturating_sub(1)
        }]
    }
    const SPEEDS: [f64; 9] = [-1.0, -0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75, 1.0];
    const FACTORS: [f64; 6] = [0.5, 0.75, 1.0, 1.5, 2.0, 3.0];
    match s {
        Setting::SendEvents(e) => Setting::SendEvents(match e {
            SendEvents::DisabledOnExternalMouse => SendEvents::Enabled,
            _ => SendEvents::DisabledOnExternalMouse,
        }),
        Setting::AccelSpeed(v) => Setting::AccelSpeed(stepf(&SPEEDS, v, forward)),
        Setting::AccelProfile(p) => Setting::AccelProfile(step(
            &[AccelProfile::Flat, AccelProfile::Adaptive],
            p,
            forward,
        )),
        Setting::NaturalScroll(b) => Setting::NaturalScroll(!b),
        Setting::LeftHanded(b) => Setting::LeftHanded(!b),
        Setting::Tap(b) => Setting::Tap(!b),
        Setting::TapButtonMap(m) => {
            Setting::TapButtonMap(step(&[ButtonMap::Lrm, ButtonMap::Lmr], m, forward))
        }
        Setting::TapDrag(b) => Setting::TapDrag(!b),
        Setting::TapDragLock(l) => Setting::TapDragLock(step(
            &[DragLock::Disabled, DragLock::Enabled, DragLock::Sticky],
            l,
            forward,
        )),
        Setting::FingerDrag(d) => Setting::FingerDrag(step(
            &[
                FingerDrag::Disabled,
                FingerDrag::ThreeFinger,
                FingerDrag::FourFinger,
            ],
            d,
            forward,
        )),
        Setting::ClickMethod(m) => Setting::ClickMethod(step(
            &[
                ClickMethod::None,
                ClickMethod::ButtonAreas,
                ClickMethod::Clickfinger,
            ],
            m,
            forward,
        )),
        Setting::ClickfingerButtonMap(m) => {
            Setting::ClickfingerButtonMap(step(&[ButtonMap::Lrm, ButtonMap::Lmr], m, forward))
        }
        Setting::MiddleEmulation(b) => Setting::MiddleEmulation(!b),
        Setting::ScrollMethod(m) => Setting::ScrollMethod(step(
            &[
                ScrollMethod::None,
                ScrollMethod::TwoFinger,
                ScrollMethod::Edge,
                ScrollMethod::OnButtonDown,
            ],
            m,
            forward,
        )),
        // THE LINUX EVENT CODES of the three buttons a scroll button can be,
        // and `0` for none — the compositor's own word for it.
        Setting::ScrollButton(b) => Setting::ScrollButton(step(&[274, 275, 276, 0], b, forward)),
        Setting::ScrollButtonLock(b) => Setting::ScrollButtonLock(!b),
        Setting::DisableWhileTyping(b) => Setting::DisableWhileTyping(!b),
        Setting::DisableWhileTrackpointing(b) => Setting::DisableWhileTrackpointing(!b),
        Setting::ScrollFactor(v) => Setting::ScrollFactor(stepf(&FACTORS, v, forward)),
    }
}

/// The label key of a setting.
const fn label_of(s: Setting) -> &'static str {
    use ronoctua_i18n::key as k;
    match s {
        Setting::SendEvents(_) => k::POINTERS_SETTING_EVENTS,
        Setting::AccelSpeed(_) => k::POINTERS_SETTING_SPEED,
        Setting::AccelProfile(_) => k::POINTERS_SETTING_PROFILE,
        Setting::NaturalScroll(_) => k::POINTERS_SETTING_NATURAL,
        Setting::LeftHanded(_) => k::POINTERS_SETTING_LEFT,
        Setting::Tap(_) => k::POINTERS_SETTING_TAP,
        Setting::TapButtonMap(_) => k::POINTERS_SETTING_TAP_MAP,
        Setting::TapDrag(_) => k::POINTERS_SETTING_TAP_DRAG,
        Setting::TapDragLock(_) => k::POINTERS_SETTING_DRAG_LOCK,
        Setting::FingerDrag(_) => k::POINTERS_SETTING_FINGER_DRAG,
        Setting::ClickMethod(_) => k::POINTERS_SETTING_CLICK,
        Setting::ClickfingerButtonMap(_) => k::POINTERS_SETTING_CLICK_MAP,
        Setting::MiddleEmulation(_) => k::POINTERS_SETTING_MIDDLE,
        Setting::ScrollMethod(_) => k::POINTERS_SETTING_SCROLL,
        Setting::ScrollButton(_) => k::POINTERS_SETTING_SCROLL_BUTTON,
        Setting::ScrollButtonLock(_) => k::POINTERS_SETTING_SCROLL_LOCK,
        Setting::DisableWhileTyping(_) => k::POINTERS_SETTING_DWT,
        Setting::DisableWhileTrackpointing(_) => k::POINTERS_SETTING_DWTP,
        Setting::ScrollFactor(_) => k::POINTERS_SETTING_FACTOR,
    }
}

/// What a setting's value says.
fn value_of(s: Setting, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key as k;
    let word = |key: &str| texts.t(key);
    match s {
        Setting::SendEvents(e) => on_off(e == SendEvents::DisabledOnExternalMouse, texts),
        Setting::AccelSpeed(v) => {
            texts.t_with(k::POINTERS_SPEED_VALUE, &[("speed", &format!("{v:+.2}"))])
        }
        Setting::AccelProfile(p) => word(match p {
            AccelProfile::Flat => k::POINTERS_FLAT,
            AccelProfile::Adaptive => k::POINTERS_ADAPTIVE,
        }),
        Setting::TapButtonMap(m) | Setting::ClickfingerButtonMap(m) => word(match m {
            ButtonMap::Lrm => k::POINTERS_LRM,
            ButtonMap::Lmr => k::POINTERS_LMR,
        }),
        Setting::TapDragLock(l) => word(match l {
            DragLock::Disabled => k::POINTERS_LOCK_OFF,
            DragLock::Enabled => k::POINTERS_LOCK_ON,
            DragLock::Sticky => k::POINTERS_LOCK_STICKY,
        }),
        Setting::FingerDrag(d) => word(match d {
            FingerDrag::Disabled => k::POINTERS_FINGERS_OFF,
            FingerDrag::ThreeFinger => k::POINTERS_FINGERS_THREE,
            FingerDrag::FourFinger => k::POINTERS_FINGERS_FOUR,
        }),
        Setting::ClickMethod(m) => word(match m {
            ClickMethod::None => k::POINTERS_CLICK_NONE,
            ClickMethod::ButtonAreas => k::POINTERS_CLICK_AREAS,
            ClickMethod::Clickfinger => k::POINTERS_CLICK_FINGERS,
        }),
        Setting::ScrollMethod(m) => word(match m {
            ScrollMethod::None => k::POINTERS_SCROLL_NONE,
            ScrollMethod::TwoFinger => k::POINTERS_SCROLL_TWO,
            ScrollMethod::Edge => k::POINTERS_SCROLL_EDGE,
            ScrollMethod::OnButtonDown => k::POINTERS_SCROLL_BUTTON,
        }),
        Setting::ScrollButton(b) => word(match b {
            274 => k::POINTERS_BUTTON_MIDDLE,
            275 => k::POINTERS_BUTTON_SIDE,
            276 => k::POINTERS_BUTTON_EXTRA,
            _ => k::POINTERS_BUTTON_NONE,
        }),
        Setting::ScrollFactor(v) => {
            texts.t_with(k::POINTERS_FACTOR_VALUE, &[("factor", &format!("{v:.2}"))])
        }
        Setting::NaturalScroll(b)
        | Setting::LeftHanded(b)
        | Setting::Tap(b)
        | Setting::TapDrag(b)
        | Setting::MiddleEmulation(b)
        | Setting::ScrollButtonLock(b)
        | Setting::DisableWhileTyping(b)
        | Setting::DisableWhileTrackpointing(b) => on_off(b, texts),
    }
}

fn on_off(on: bool, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(if on {
        ronoctua_i18n::key::POINTERS_ON
    } else {
        ronoctua_i18n::key::POINTERS_OFF
    })
}

/// The colour a cursor's inside is painted, as a token.
#[must_use]
pub const fn fill_of(colour: u8) -> tokens::Color {
    const FILLS: [tokens::Color; 8] = [
        tokens::color::CURSOR_FILL_ONE,
        tokens::color::CURSOR_FILL_TWO,
        tokens::color::CURSOR_FILL_THREE,
        tokens::color::CURSOR_FILL_FOUR,
        tokens::color::CURSOR_FILL_FIVE,
        tokens::color::CURSOR_FILL_SIX,
        tokens::color::CURSOR_FILL_SEVEN,
        tokens::color::CURSOR_FILL_EIGHT,
    ];
    FILLS[(colour % 8) as usize]
}

/// Which key a command id is — the one map both doors reach.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "pointers-up" => Press::Up,
        "pointers-down" => Press::Down,
        "pointers-less" => Press::Less,
        "pointers-more" => Press::More,
        "pointers-enter" => Press::Enter,
        "pointers-forget" => Press::Forget,
        _ => return None,
    })
}

/// Which key a notation is, through [`OF_THE_VIEW`].
#[must_use]
pub fn pressed(notation: &str) -> Option<Press> {
    OF_THE_VIEW
        .iter()
        .find(|(k, _, _)| *k == notation)
        .and_then(|(_, id, _)| press_of(id))
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

    fn inert(&mut self, fields: Vec<Field>) {
        self.out.push(Row {
            fields,
            focused: false,
            heading: true,
            target: TargetCell(Target::Nothing),
            spot: String::new(),
        });
    }

    fn heading(&mut self, key: &str) {
        // AIR FIRST, and never before the first title — `chrome::AIR`.
        if !self.out.is_empty() {
            self.inert(crate::chrome::AIR);
        }
        let text = self.texts.t(key);
        self.inert(vec![Self::piece(
            self.left,
            text,
            tokens::color::TEXT_HEADING,
        )]);
    }

    /// A glyph and a name the rows below hang UNDER — a cursor, a device.
    /// Not a heading: the locale view's `under`, in the place's own tone and
    /// the nest it uses for a place, so a section title stays the one thing
    /// in `text.heading`.
    fn under(
        &mut self,
        glyph: char,
        glyph_colour: tokens::Color,
        text: String,
        aside: Option<String>,
    ) {
        let at = self.left + tokens::space::VIEW_NESTCELLS;
        let mut fields = vec![
            Self::piece(at, glyph.to_string(), glyph_colour),
            Self::piece(at + 2, text.clone(), tokens::color::TEXT_SECONDARY),
        ];
        if let Some(aside) = aside {
            let after = at + 2 + ronoctua_protocol::cells(&text) + 1;
            fields.push(Self::piece(
                after,
                format!("({aside})"),
                tokens::color::TEXT_DIM,
            ));
        }
        self.inert(fields);
    }

    /// A sentence that stands for a whole section. It WRAPS.
    fn note(&mut self, key: &str) {
        let text = self.texts.t(key);
        for line in crate::chrome::note_lines(&text, self.room) {
            self.inert(vec![Self::piece(self.left, line, tokens::color::TEXT_DIM)]);
        }
    }

    fn setting(&mut self, device: Option<String>, kind: DeviceKind, s: Setting) {
        let spot = format!(
            "set-{}-{:?}",
            device.as_deref().unwrap_or(match kind {
                DeviceKind::Touchpad => "every-touchpad",
                _ => "every-mouse",
            }),
            std::mem::discriminant(&s)
        );
        self.pair(
            self.texts.t(label_of(s)),
            value_of(s, self.texts),
            Target::Setting(device, kind, s),
            spot,
        );
    }

    /// A row a key acts on — the colours every view of this shape uses.
    fn pair(&mut self, label: String, value: String, target: Target, spot: String) {
        let focused = self.cursor == self.out.len();
        let acts = target.turns();
        let (label_colour, value_colour) = match (focused, acts) {
            (true, true) => (tokens::color::STATE_ACCENT, tokens::color::TEXT_HERE_ACTS),
            (true, false) => (tokens::color::STATE_ACCENT, tokens::color::TEXT_HERE_READ),
            (false, true) => (tokens::color::TEXT_LABEL_ACTS, tokens::color::TEXT_ANSWER),
            (false, false) => (tokens::color::TEXT_SECONDARY, tokens::color::TEXT_DIM),
        };
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                &label,
                &value,
                label_colour,
                value_colour,
            ),
            focused,
            heading: false,
            target: TargetCell(target),
            spot,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_compositor::Device;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("the base language")
    }

    fn device(id: &str, kind: DeviceKind) -> Device {
        Device {
            id: id.to_owned(),
            name: id.to_owned(),
            kind,
            settings: PointerSettings {
                send_events: Some(SendEvents::Enabled),
                natural_scroll: Some(false),
                tap: (kind == DeviceKind::Touchpad).then_some(false),
                accel_speed: Some(0.0),
                ..PointerSettings::default()
            },
        }
    }

    fn a_laptop() -> Snapshot {
        let devices = vec![
            device("2:2:Touchpad", DeviceKind::Touchpad),
            device("3:3:Dongle_Mouse", DeviceKind::Mouse),
        ];
        // Detection on, so the touchpad and the mouse are two cursors.
        let prefs = prefs::Preferences {
            detect: true,
            ..prefs::Preferences::default()
        };
        let plan = ronoctua_pointers::plan::plan(&devices, &prefs);
        Snapshot {
            devices,
            seats: Vec::new(),
            prefs,
            plan,
            settings_failed: false,
            connected: true,
            keyboards: Vec::new(),
        }
    }

    #[test]
    fn every_cursor_and_every_device_has_a_row() {
        let s = a_laptop();
        let rows = View::fresh().rows(Some(&s), 100, &texts());
        let spots: Vec<&str> = rows
            .iter()
            .map(|r| r.spot.as_str())
            .filter(|x| !x.is_empty())
            .collect();
        for want in [
            "detect",
            "cursor-on-1",
            "cursor-colour-2",
            "place-3:3:Dongle_Mouse",
            "device-on-2:2:Touchpad",
            "size",
            "focus",
            "typing",
        ] {
            assert!(spots.contains(&want), "no row for {want}: {spots:?}");
        }
    }

    #[test]
    fn a_setting_a_device_does_not_have_is_not_drawn() {
        let s = a_laptop();
        let rows = View::fresh().rows(Some(&s), 100, &texts());
        assert!(
            !rows
                .iter()
                .any(|r| r.spot.starts_with("set-3:3:Dongle_Mouse")
                    && matches!(r.target.0, Target::Setting(_, _, Setting::Tap(_)))),
            "a mouse was offered tapping"
        );
    }

    #[test]
    fn typing_turns_through_every_cursor_and_back() {
        let s = a_laptop();
        let ring = typing_ring(Some(&s));
        assert_eq!(
            ring,
            vec![
                ronoctua_pointers::Typing::First,
                ronoctua_pointers::Typing::Cursor(2),
                ronoctua_pointers::Typing::Latest,
            ]
        );
    }

    #[test]
    fn a_click_turns_the_value_forward() {
        let s = a_laptop();
        let tx = texts();
        let mut v = View::fresh();
        let rows = v.rows(Some(&s), 100, &tx);
        let at = rows.iter().position(|r| r.spot == "detect").unwrap();
        v.clicked(at, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 100, &tx),
            Errand::Order(Order::Detect(false))
        );
    }

    /// The first click selects, a click on the row already selected is its
    /// `RET` — and the doors ask for exactly that (invariant 19).
    #[test]
    fn the_second_click_on_a_row_is_its_return() {
        let s = a_laptop();
        let tx = texts();
        let mut v = View::fresh();
        let rows = v.rows(Some(&s), 100, &tx);
        let at = rows.iter().position(|r| r.spot == "together").unwrap();
        assert_ne!(v.cursor(), at, "the cursor must start somewhere else");
        assert!(!v.clicked(at, &rows), "the first click only selects");
        assert!(v.clicked(at, &rows), "the second one acts");
        assert_eq!(
            v.key(Press::Enter, Some(&s), 100, &tx),
            Errand::Order(Order::Together(true))
        );
    }

    /// The last rows are the door to the keyboards' screen, and `RET` on it
    /// runs that screen's command; the arrows turn nothing there.
    #[test]
    fn the_door_to_the_keyboards_opens_them() {
        let s = a_laptop();
        let tx = texts();
        let mut v = View::fresh();
        let rows = v.rows(Some(&s), 100, &tx);
        let at = rows.iter().position(|r| r.spot == "keyboards").unwrap();
        assert_eq!(at, rows.len() - 1, "the door is the last row");
        let _ = v.clicked(at, &rows);
        assert_eq!(v.key(Press::More, Some(&s), 100, &tx), Errand::Nothing);
        assert_eq!(
            v.key(Press::Enter, Some(&s), 100, &tx),
            Errand::Run("keyboard-open")
        );
    }

    #[test]
    fn a_device_goes_round_the_cursors_and_then_to_a_new_one() {
        assert_eq!(next_place(&[1, 2], 1, true), Place::Cursor(2));
        assert_eq!(next_place(&[1, 2], 2, true), Place::New);
        assert_eq!(next_place(&[1, 2], 1, false), Place::New);
    }

    #[test]
    fn every_ring_comes_back_round() {
        let s = Setting::ScrollButton(274);
        let mut x = s;
        for _ in 0..4 {
            x = next_setting(x, true);
        }
        assert_eq!(x, s);
        assert_eq!(
            next_setting(Setting::AccelSpeed(0.0), true),
            Setting::AccelSpeed(0.25)
        );
        assert_eq!(
            next_setting(Setting::AccelSpeed(1.0), true),
            Setting::AccelSpeed(1.0),
            "the top is a wall"
        );
    }

    #[test]
    fn every_hint_key_reaches_a_press() {
        for (k, id, _) in OF_THE_VIEW {
            assert!(pressed(k).is_some(), "{k} ({id}) answers nothing");
        }
    }
}
