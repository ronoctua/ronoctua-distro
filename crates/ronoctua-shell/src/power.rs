//! The power view — one implementation, two doors.
//!
//! # What it holds, and what it does not
//!
//! A cursor, an armed action and a pending threshold pair. It holds no
//! reading: the [`ronoctua_power::Snapshot`] lives in the footer (or the
//! window) and is handed in when rows are asked for, which is what lets both
//! doors draw the same thing without a second copy going stale. It is the
//! shape `notifications::View` has, for the reason its header gives.
//!
//! # One list, headings as rows
//!
//! Eight sections — the battery, the charger, the profile, the charge
//! thresholds, keeping the machine awake, the lid and idling, the four
//! actions, and the peripherals with batteries of their own — one under the
//! other, each heading a row like the manager's, so the region draws one list
//! and pages it the way it pages everything else.
//!
//! # Everything a screen can do here is a ROW
//!
//! `RET` on a profile applies it; on a preset writes the pair; on a switch
//! flips it; on an action ARMS it, and a second `RET` confirms — one confirm
//! mechanism for the four things that empty the screen, and nothing floats
//! (invariant 14). `Esc` walks back one level: a pending pair is dropped, an
//! armed action disarmed, and only then does the view close.

use ronoctua_power::sysfs::Preset;
use ronoctua_power::{Action, Can, Keep, Order, Profile, Snapshot, State};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit as FilesHit};

/// The keys this view answers to, and the hint line is generated from it —
/// see `chrome::hint_of`. Bare letters are free here (nothing is typed), but
/// everything a key does is a ROW's, so the table is the five gestures.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "power-down"),
    ("<up>", "power-up"),
    ("RET", "power-enter"),
    ("<left>", "power-less"),
    ("<right>", "power-more"),
];

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row, skipping headings.
    Up,
    /// Down one row, skipping headings.
    Down,
    /// Lower a threshold under the cursor by five.
    Left,
    /// Raise it by five.
    Right,
    /// Act on the row.
    Enter,
    /// One level back.
    Escape,
}

/// Which of the two thresholds a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// Below which charging starts again.
    Start,
    /// At which charging stops.
    End,
}

/// What `RET` on a row does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Nothing: a heading, a fact, a note.
    Nothing,
    /// Apply this profile.
    Profile(Profile),
    /// Flip the daemon's battery-aware switch to this.
    BatteryAware(bool),
    /// A threshold: `RET` writes the pending pair, the arrows move it.
    Threshold(Which),
    /// Write this pair.
    Preset(Preset),
    /// Take or drop one of our locks — `true` when it is held now.
    Keep(Keep, bool),
    /// Arm, then do; `Can` says whether logind would.
    Action(Action, Can),
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
    /// Drawn dim and REFUSING: an un-offered profile, a read-only threshold,
    /// an action logind says no to. Reachable, so the row can say why —
    /// the XFS precedent — and `RET` on it does nothing rather than send an
    /// order the daemon will refuse.
    pub disabled: bool,
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Put this on the system clipboard — what `RET` does on an INFORMATIVE
    /// row. See `chrome::information_of` for what "this" is and why.
    Copy(String),
    /// Send this to the power thread.
    Order(Order),
    /// The person CHOSE a profile: apply it AND write `power.json`, in both
    /// doors — `RET` on the row, or the arrows cycling the offered ones. An
    /// order alone would last until the next footer start, when the file
    /// and the variable are applied over it.
    Choose(Profile),
    /// Close the view.
    Close,
}

/// The view's state.
#[derive(Debug, Clone, Default)]
pub struct View {
    cursor: usize,
    /// The action `RET` was pressed on once; the second press does it.
    armed: Option<Action>,
    /// A pair the arrows moved and `RET` has not written yet.
    pending: Option<(Option<u8>, u8)>,
}

/// Where a row's cells begin and how wide they are, in the ROW's cells —
/// both doors add `chrome::FIELDS_LEAD`; see `notifications::extent_of`.
#[must_use]
pub fn extent(columns: u16) -> (u16, u16) {
    let left = crate::chrome::INSET.saturating_sub(crate::chrome::FIELDS_LEAD);
    let end = columns.saturating_sub(crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT);
    (left, end.saturating_sub(left))
}

/// Five-cell step for the arrows on a threshold: nobody tunes 61 against 62.
const STEP: u8 = 5;

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

/// How long the battery has, in words — or nothing, when there is nothing to
/// say.
///
/// # Why it is one function
///
/// Because it was two, and they disagreed. The view matched `State::Charging`
/// exactly; the pointer's sentence on the minibuffer matched
/// `state.is_charging()`, which also covers `PendingCharge` — the state a
/// charge threshold or a firmware pause produces. So a battery in
/// `PendingCharge` with a `TimeToFull` read "full in 1 h 20" under the
/// pointer and "unknown" in the view, two lines apart on the same screen.
///
/// `is_charging()` is the right one of the two: `PendingCharge` is charging
/// that has not started moving, and the daemon's own estimate is the best
/// answer there is. `FullyCharged` says nothing at all, because the state
/// word already says it.
#[must_use]
pub fn time_left(battery: &ronoctua_power::Battery, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::key;
    if battery.state == State::FullyCharged {
        return String::new();
    }
    match (
        battery.state.is_charging(),
        battery.time_to_empty,
        battery.time_to_full,
    ) {
        (false, Some(t), _) => {
            texts.t_with(key::POWER_TIME_EMPTY, &[("time", &duration_text(t, texts))])
        }
        (true, _, Some(t)) => {
            texts.t_with(key::POWER_TIME_FULL, &[("time", &duration_text(t, texts))])
        }
        _ => texts.t(key::POWER_TIME_UNKNOWN),
    }
}

impl View {
    /// A fresh view, at the top.
    #[must_use]
    pub fn fresh() -> Self {
        // ROW 1, NOT 0: row 0 is the battery heading, and a heading is not a
        // row one can stand on — nothing was lit, and the arrows' first step
        // had to walk off it before anything moved. The three first screen
        // proofs all showed the heading with no cursor anywhere.
        //
        // IT DOES NOT MAKE THE FIRST `RET` DO SOMETHING, and it never did:
        // row 1 is the charge FACT, whose target is `Target::Nothing`. That
        // is right — a fact is not a control — and this comment used to
        // claim the opposite.
        Self {
            cursor: 1,
            ..Self::default()
        }
    }

    /// A fresh view with the cursor on the first row that does `target`'s
    /// kind of thing — how `LEADER q s` lands on "suspend", armed.
    #[must_use]
    pub fn fresh_at(
        snap: Option<&Snapshot>,
        wanted: Profile,
        want: Target,
        texts: &ronoctua_i18n::Texts,
    ) -> Self {
        let mut v = Self::fresh();
        let rows = v.rows(snap, wanted, 80, texts);
        if let Some(i) = rows.iter().position(|r| r.target == want) {
            v.cursor = i;
        }
        v
    }

    /// Arms an action — `LEADER q s` opens the view already asking.
    ///
    /// ONLY WHAT LOGIND WOULD DO: armed, the row swaps its label for the
    /// question and the hint line leads with it, but `RET` on a disabled row
    /// is [`Errand::Nothing`] — the screen would ask something no key could
    /// answer, and hide the `not available` until `Esc`. `LEADER q h` on a
    /// machine without hibernation is exactly that.
    pub fn arm(&mut self, action: Action, can: Can) {
        if can == Can::Yes {
            self.armed = Some(action);
        }
    }

    /// The row the cursor is on — what the region scrolls to.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Is an action waiting for its second `RET`?
    #[must_use]
    pub const fn armed(&self) -> Option<Action> {
        self.armed
    }

    /// The hint line, generated from the table — the same pieces in both doors.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        // WITH AN ACTION ARMED the question comes first: the row says it too,
        // but the row may be off the page and the line never is.
        let mut out = Vec::new();
        if let Some(action) = self.armed() {
            let name = texts.t(&format!("power-action-{}", action.id()));
            out.push(crate::chrome::Piece::prose(texts.t_with(
                ronoctua_i18n::key::POWER_ACTION_CONFIRM,
                &[("action", &name)],
            )));
            out.push(crate::chrome::Piece::separator());
        }
        out.extend(crate::chrome::hint_of(OF_THE_VIEW, texts));
        out
    }

    /// Puts the cursor on row `n`, when it is a row one can stand on.
    pub fn clicked(&mut self, n: usize, rows: &[Row]) {
        if rows.get(n).is_some_and(|r| !r.heading) {
            // MOVING DISARMS, by the mouse as by the arrows: the question was
            // about the row that was left, and the pointer is not a second
            // vocabulary — clicking only does what a key does.
            if n != self.cursor {
                self.armed = None;
            }
            self.cursor = n;
        }
    }

    /// Keeps the cursor on a row that EXISTS.
    ///
    /// Rows come and go while the view is open — a peripheral switching off
    /// takes its heading and its row, a browser releasing `sleep` and `idle`
    /// takes two sentences — and a cursor past the end focuses nothing: the
    /// arrows' first step is out of range too, so neither of them moves and
    /// `RET` has no target. The view swallows the keyboard, so going dead on
    /// it leaves only `Esc`.
    pub fn settle(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
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
        // AND THE QUESTION FOLLOWS THE DOOR. logind can say no while the
        // question stands — the lid closes on a machine that loses
        // hibernation, the row goes dim — and an armed `RET` there would be
        // the dead question `arm` refuses to create in the first place.
        if let Some(a) = self.armed
            && !rows.iter().any(|r| r.target == Target::Action(a, Can::Yes))
        {
            self.armed = None;
        }
    }

    /// Drops a pair the arrows moved but nobody wrote.
    ///
    /// Called when the thresholds change under the view: the window writing
    /// them, or a preset applied there, would otherwise be masked in the
    /// footer by a stale "the pair you were editing".
    pub fn forget_pending(&mut self) {
        self.pending = None;
    }

    /// Acts on a key.
    #[must_use]
    pub fn key(
        &mut self,
        press: Press,
        snap: Option<&Snapshot>,
        wanted: Profile,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(snap, wanted, DECIDING_WIDTH, texts);
        self.settle(&rows);
        let target = rows.get(self.cursor).map_or(Target::Nothing, |r| r.target);
        let disabled = rows.get(self.cursor).is_some_and(|r| r.disabled);
        match press {
            Press::Up | Press::Down => {
                let step: isize = if press == Press::Down { 1 } else { -1 };
                let was = self.cursor;
                let mut i = self.cursor;
                loop {
                    let next = i.checked_add_signed(step);
                    match next {
                        Some(n) if n < rows.len() => i = n,
                        _ => break,
                    }
                    if !rows[i].heading {
                        self.cursor = i;
                        break;
                    }
                }
                // MOVING DISARMS: the question was about the row that was
                // left — and at the ends of the list nothing was left, so
                // the question stands. The rule the pointer follows too.
                if self.cursor != was {
                    self.armed = None;
                }
                Errand::Nothing
            }
            Press::Escape => {
                // ARMED FIRST, because it is the more recent question: moving
                // the cursor clears it, so anything armed was asked for after
                // the pair was moved — and it is the one the hint line is
                // showing an `Esc cancels` for.
                if self.armed.take().is_some() || self.pending.take().is_some() {
                    return Errand::Nothing;
                }
                Errand::Close
            }
            Press::Left | Press::Right => match target {
                // ON A PROFILE ROW the arrows cycle the ACTIVE profile through
                // the offered ones — and that is a choice,
                // so it is persisted like `RET`.
                Target::Profile(_) => {
                    // `disabled` IS NOT ASKED, and that is the point: the row
                    // is dim because THAT profile is not offered, and the
                    // arrows do not act on the row — they cycle the ACTIVE
                    // one, which is offered by construction. With a daemon
                    // that offers only balanced and power-saver,
                    // `performance` is drawn first and dim, and `→` on it did
                    // nothing while `→` one row down cycled: two behaviours
                    // for one key, with nothing on screen saying why, and the
                    // hint line still reading "← less · → more".
                    //
                    // `RET` still refuses on a dim row, and should: that one
                    // IS about the row.
                    let Some(p) = snap.and_then(|s| s.profiles.as_ref()) else {
                        return Errand::Nothing;
                    };
                    // ASCENDING, so `→ more` is the higher profile — the hint's
                    // words — even though the rows are drawn top first.
                    let offered: Vec<Profile> = Profile::ALL
                        .iter()
                        .copied()
                        .filter(|x| p.offered.contains(x))
                        .collect();
                    let Some(i) = offered.iter().position(|x| *x == p.active) else {
                        return Errand::Nothing;
                    };
                    let next = if press == Press::Right {
                        (i + 1) % offered.len()
                    } else {
                        (i + offered.len() - 1) % offered.len()
                    };
                    Errand::Choose(offered[next])
                }
                Target::Threshold(which) => {
                    let Some(t) = snap.and_then(|s| s.thresholds.as_ref()) else {
                        return Errand::Nothing;
                    };
                    if disabled {
                        return Errand::Nothing;
                    }
                    let (start, end) = self.pending.unwrap_or((t.start, t.end));
                    // WITHIN THE ADMISSIBLE SET, both ways: the end never comes
                    // down to the start (eight presses from 40/80 gave 40/40,
                    // and `RET` sent a pair the kernel refuses), and the start
                    // never climbs to the end.
                    let floor = start.map_or(STEP, |s| s.saturating_add(STEP)).min(100);
                    // EACH CLAMP IN THE DIRECTION OF TRAVEL. A firmware pair
                    // already closer together than one step — 78/80 — turned
                    // the floor into a ceiling: `←` on the end computed
                    // `max(83)` and RAISED it. A key that moves the number
                    // the other way is worse than one that does nothing.
                    let moved = match (which, press) {
                        (Which::End, Press::Right) => (start, end.saturating_add(STEP).min(100)),
                        (Which::End, _) => (start, end.saturating_sub(STEP).max(floor).min(end)),
                        (Which::Start, Press::Right) => (
                            start.map(|s| {
                                s.saturating_add(STEP).min(end.saturating_sub(STEP)).max(s)
                            }),
                            end,
                        ),
                        (Which::Start, _) => (start.map(|s| s.saturating_sub(STEP)), end),
                    };
                    self.pending = Some(moved);
                    Errand::Nothing
                }
                _ => Errand::Nothing,
            },
            // A DISABLED ROW REFUSES — it is drawn dim and says why; sending
            // the order would only bring the daemon's refusal back as an echo.
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
                Target::Profile(p) => Errand::Choose(p),
                Target::BatteryAware(on) => Errand::Order(Order::BatteryAware(on)),
                Target::Preset(preset) => {
                    let (start, end) = preset.pair();
                    self.pending = None;
                    Errand::Order(Order::Thresholds { start, end })
                }
                Target::Threshold(_) => match self.pending.take() {
                    Some((start, end)) => Errand::Order(Order::Thresholds { start, end }),
                    None => Errand::Nothing,
                },
                Target::Keep(keep, held) => Errand::Order(Order::KeepAwake(keep, !held)),
                Target::Action(action, _) => {
                    if self.armed == Some(action) {
                        self.armed = None;
                        Errand::Order(Order::Do(action))
                    } else {
                        self.armed = Some(action);
                        Errand::Nothing
                    }
                }
            },
        }
    }

    /// The rows to draw, for the width given.
    #[must_use]
    pub fn rows(
        &self,
        snap: Option<&Snapshot>,
        wanted: Profile,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        use ronoctua_i18n::key;
        let (left, room) = extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };

        // 1. THE BATTERY.
        b.heading(key::POWER_HEADING_BATTERY);
        let batteries: Vec<&ronoctua_power::Battery> = snap
            .map(|s| s.batteries.iter().collect())
            .unwrap_or_default();
        if batteries.is_empty() {
            b.note(key::POWER_NO_BATTERY);
        }
        for battery in batteries {
            self.battery_rows(&mut b, battery);
        }

        // 2. THE CHARGER.
        b.heading(key::POWER_HEADING_ADAPTER);
        match snap.and_then(Snapshot::adapter_online) {
            Some(true) => b.note(key::POWER_PLUGGED),
            Some(false) => b.note(key::POWER_UNPLUGGED),
            None => b.note(if snap.is_some_and(|s| s.sleep.on_external_power) {
                key::POWER_PLUGGED
            } else {
                key::POWER_NO_ADAPTER
            }),
        }

        // 3. THE PROFILE.
        b.heading(key::POWER_HEADING_PROFILE);
        match snap.and_then(|s| s.profiles.as_ref()) {
            None => b.note(key::POWER_NO_DAEMON),
            Some(p) => {
                for profile in Profile::ALL.iter().rev() {
                    let offered = p.offered.contains(profile);
                    let mark = if p.active == *profile {
                        tokens::icon::SUCCESS.to_string()
                    } else {
                        " ".to_owned()
                    };
                    // THE PROFILE'S OWN GLYPH, which is what `icons.nix` says
                    // this token family is for — a leaf, the middle gauge, the
                    // speedometer. It said "`performance` has a glyph for the
                    // view's rows" and the rows drew none: `profile_glyph` had
                    // exactly one caller, the modeline, which by construction
                    // never asks it for `Performance` (`profile_mark` returns
                    // `None` at the top profile). So one arm of that match and
                    // one token were drawn by nothing at all.
                    let mut label = format!(
                        "{mark} {} {}",
                        profile_glyph(*profile),
                        texts.t(&format!("power-profile-{}", profile.id()))
                    );
                    if *profile == wanted && p.active != wanted {
                        label.push(' ');
                        label.push_str(&texts.t(key::POWER_PROFILE_WANTED));
                    }
                    let value = if offered {
                        String::new()
                    } else {
                        texts.t(key::POWER_PROFILE_NOT_OFFERED)
                    };
                    b.pair(label, value, Target::Profile(*profile), !offered);
                }
                if let Some(why) = &p.degraded {
                    b.sentence(texts.t_with(key::POWER_DEGRADED, &[("why", why)]));
                }
                if let Some(why) = &p.inhibited {
                    b.sentence(texts.t_with(key::POWER_INHIBITED, &[("why", why)]));
                }
                for hold in &p.holds {
                    let profile = hold.profile.map_or_else(String::new, |p| {
                        texts.t(&format!("power-profile-{}", p.id()))
                    });
                    b.sentence(texts.t_with(
                        key::POWER_HOLD,
                        &[
                            ("who", &hold.who),
                            ("profile", &profile),
                            ("why", &hold.why),
                        ],
                    ));
                }
                if let Some(aware) = p.battery_aware {
                    b.pair(
                        texts.t(key::POWER_BATTERY_AWARE),
                        texts.t(if aware {
                            key::POWER_ON
                        } else {
                            key::POWER_OFF_LABEL
                        }),
                        Target::BatteryAware(!aware),
                        false,
                    );
                }
            }
        }

        // 4. THE CHARGE THRESHOLDS.
        b.heading(key::POWER_HEADING_THRESHOLDS);
        match snap.and_then(|s| s.thresholds.as_ref()) {
            None => b.note(key::POWER_THRESHOLDS_UNSUPPORTED),
            Some(t) => {
                if !t.writable {
                    b.note(key::POWER_THRESHOLDS_READONLY);
                }
                let (start, end) = self.pending.unwrap_or((t.start, t.end));
                let pending = self.pending.is_some();
                let value = |v: Option<u8>| -> String {
                    match v {
                        Some(n) if pending => texts.t_args(
                            key::POWER_THRESHOLD_PENDING,
                            &[("value", ronoctua_i18n::Arg::Number(i64::from(n)))],
                        ),
                        Some(n) => format!("{n}%"),
                        None => texts.t(key::POWER_THRESHOLD_NONE),
                    }
                };
                // A START THE BATTERY HAS NOT GOT is a row that says so and
                // takes no arrow — the End row would otherwise start saying
                // "RET writes" for a pair that had not moved.
                b.pair(
                    texts.t_with(key::POWER_THRESHOLD_START, &[("supply", &t.supply)]),
                    value(start),
                    Target::Threshold(Which::Start),
                    !t.writable || t.start.is_none(),
                );
                b.pair(
                    texts.t_with(key::POWER_THRESHOLD_END, &[("supply", &t.supply)]),
                    value(Some(end)),
                    Target::Threshold(Which::End),
                    !t.writable,
                );
                for preset in Preset::ALL {
                    // A PRESET WITH A START needs a battery that has one:
                    // `write_in` would skip the start and write only the end,
                    // which is not the pair the row promised.
                    let needs_start = preset.pair().0.is_some() && t.start.is_none();
                    b.pair(
                        texts.t(&format!("power-preset-{}", preset.id())),
                        String::new(),
                        Target::Preset(preset),
                        !t.writable || needs_start,
                    );
                }
                if let Some(behaviour) = &t.behaviour {
                    b.fact(
                        key::POWER_FACT_BEHAVIOUR,
                        crate::power_words::charge_behaviour(behaviour, texts),
                    );
                }
                if let Some(types) = &t.types {
                    b.fact(
                        key::POWER_FACT_CHARGE_TYPE,
                        crate::power_words::charge_type(types, texts),
                    );
                }
            }
        }

        // 5. KEEPING IT AWAKE.
        b.heading(key::POWER_HEADING_AWAKE);
        let held = snap.map(Snapshot::held).unwrap_or_default();
        for keep in Keep::ALL {
            let on = held.contains(&keep);
            b.pair(
                texts.t(&format!("power-keep-{}", keep.id())),
                texts.t(if on {
                    key::POWER_ON
                } else {
                    key::POWER_OFF_LABEL
                }),
                Target::Keep(keep, on),
                false,
            );
        }
        if let Some(s) = snap {
            for i in s.inhibitors.iter().filter(|i| i.ours().is_none()) {
                b.sentence(texts.t_with(
                    key::POWER_INHIBITOR,
                    &[
                        ("who", &i.who),
                        ("what", &crate::power_words::daemon_words(&i.what, texts)),
                        ("why", &i.why),
                    ],
                ));
            }
        }

        // 6. THE LID AND IDLING.
        //
        // THE HEADING IS INSIDE THE GUARD, like the peripherals' below: with
        // no reading at all — `ronoctua_power::watch` failed and the footer
        // only warned — every other section says so in a sentence, and this
        // one drew a title with nothing under it for the whole session.
        if let Some(s) = snap {
            b.heading(key::POWER_HEADING_LID);
            if s.lid.present {
                b.note(if s.lid.closed {
                    key::POWER_LID_CLOSED
                } else {
                    key::POWER_LID_OPEN
                });
            }
            let action = |word: &str| -> String {
                if word.is_empty() {
                    texts.t(key::POWER_ACTION_UNSET)
                } else {
                    crate::power_words::daemon_word(word, texts)
                }
            };
            b.fact(
                key::POWER_LID_ACTION,
                texts.t_with(
                    key::POWER_LID_ACTION_VALUE,
                    &[("action", &action(&s.sleep.handle_lid))],
                ),
            );
            if !s.sleep.handle_lid_external_power.is_empty() {
                b.fact(
                    key::POWER_LID_ACTION_EXTERNAL,
                    texts.t_with(
                        key::POWER_LID_ACTION_VALUE,
                        &[("action", &action(&s.sleep.handle_lid_external_power))],
                    ),
                );
            }
            if s.sleep.docked {
                b.fact(
                    key::POWER_LID_ACTION_DOCKED,
                    texts.t_with(
                        key::POWER_LID_ACTION_VALUE,
                        &[("action", &action(&s.sleep.handle_lid_docked))],
                    ),
                );
            }
            b.fact(
                key::POWER_POWER_KEY,
                texts.t_with(
                    key::POWER_LID_ACTION_VALUE,
                    &[("action", &action(&s.sleep.handle_power_key))],
                ),
            );
            let idle = match (&s.sleep.idle_action[..], s.sleep.idle_after) {
                ("" | "ignore", _) | (_, None) => texts.t(key::POWER_IDLE_NEVER),
                (word, Some(after)) => texts.t_with(
                    key::POWER_IDLE_ACTION,
                    &[
                        ("action", &crate::power_words::daemon_word(word, texts)),
                        ("time", &duration_text(after, texts)),
                    ],
                ),
            };
            b.fact(key::POWER_FACT_IDLE, idle);
            if let Some(what) = &s.critical_action {
                b.fact(
                    key::POWER_CRITICAL_ACTION,
                    crate::power_words::daemon_word(what, texts),
                );
            }
            if !s.sleep.blocked.is_empty() {
                b.fact(
                    key::POWER_BLOCKED,
                    crate::power_words::daemon_words(&s.sleep.blocked, texts),
                );
            }
        }

        // 7. THE FOUR ACTIONS.
        b.heading(key::POWER_HEADING_ACTIONS);
        let sleep = snap.map(|s| &s.sleep);
        for action in Action::ALL {
            let can = sleep.map_or(Can::NotAvailable, |s| s.can(action));
            let name = texts.t(&format!("power-action-{}", action.id()));
            let (label, value) = if self.armed == Some(action) {
                (
                    texts.t_with(key::POWER_ACTION_CONFIRM, &[("action", &name)]),
                    String::new(),
                )
            } else if can == Can::Yes {
                (name, String::new())
            } else {
                (name, texts.t(key::POWER_CANNOT))
            };
            b.pair(label, value, Target::Action(action, can), can != Can::Yes);
        }

        // 8. PERIPHERALS.
        if let Some(s) = snap
            && !s.peripherals.is_empty()
        {
            b.heading(key::POWER_HEADING_PERIPHERALS);
            for p in &s.peripherals {
                let kind = texts.t(&format!("power-kind-{}", p.kind.id()));
                let charge = p
                    .percentage
                    .map_or_else(String::new, |pct| format!("{}%", pct.round()));
                let state = texts.t(&format!("power-state-{}", p.state.id()));
                b.pair(
                    texts.t_with(
                        key::POWER_PERIPHERAL,
                        &[("kind", &kind), ("model", &p.model)],
                    ),
                    format!("{charge} {state}").trim().to_owned(),
                    Target::Nothing,
                    false,
                );
            }
        }

        b.out
    }

    fn battery_rows(&self, b: &mut Builder<'_>, battery: &ronoctua_power::Battery) {
        use ronoctua_i18n::{Arg, key};
        let texts = b.texts;
        let state = texts.t(&format!("power-state-{}", battery.state.id()));
        let percent = battery.percent();
        // THE LITERAL RULE, the modeline's: at or under the mark, charging or
        // not. Two rules for one number would have the row and the mark
        // disagree while the charger is in.
        let danger = percent <= ronoctua_power::DANGER_UNTIL;
        b.fact_coloured(
            key::POWER_FACT_CHARGE,
            format!("{percent}% {state}"),
            danger.then_some(tokens::color::STATE_ERROR),
        );
        let time = time_left(battery, texts);
        if !time.is_empty() {
            b.fact(key::POWER_FACT_TIME, time);
        }
        if battery.rate_w > 0.0 {
            b.fact(
                key::POWER_FACT_RATE,
                texts.t_args(key::POWER_RATE, &[("rate", Arg::Fraction(battery.rate_w))]),
            );
        }
        if battery.energy_full_wh > 0.0 {
            b.fact(
                key::POWER_FACT_ENERGY,
                texts.t_args(
                    key::POWER_ENERGY,
                    &[
                        ("now", Arg::Fraction(battery.energy_wh)),
                        ("full", Arg::Fraction(battery.energy_full_wh)),
                    ],
                ),
            );
        }
        if let Some(health) = battery.health() {
            b.fact(
                key::POWER_FACT_HEALTH,
                texts.t_args(
                    key::POWER_HEALTH,
                    &[
                        ("health", Arg::Number(i64::from(health))),
                        ("full", Arg::Fraction(battery.energy_full_wh)),
                        ("design", Arg::Fraction(battery.energy_full_design_wh)),
                    ],
                ),
            );
        }
        // AND THE DAEMON'S OWN NUMBER, when it is not ours. It was read on
        // every snapshot and drawn nowhere; where the two agree there is
        // nothing to say, and where they do not, the difference is the fact.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a percentage, and the daemon reports 0..=100"
        )]
        let theirs = battery.capacity_pct.round() as u8;
        if theirs > 0 && battery.health() != Some(theirs) {
            b.fact(
                key::POWER_FACT_HEALTH_DAEMON,
                texts.t_args(
                    key::POWER_PERCENT,
                    &[("percent", Arg::Number(i64::from(theirs)))],
                ),
            );
        }
        if let Some(cycles) = battery.cycles {
            b.fact(key::POWER_FACT_CYCLES, cycles.to_string());
        }
        if battery.voltage_v > 0.0 {
            b.fact(
                key::POWER_FACT_VOLTAGE,
                texts.t_args(
                    key::POWER_VOLTAGE,
                    &[("v", Arg::Fraction(battery.voltage_v))],
                ),
            );
        }
        if let Some(t) = battery.temperature_c {
            b.fact(
                key::POWER_FACT_TEMPERATURE,
                texts.t_args(key::POWER_TEMPERATURE, &[("t", Arg::Fraction(t))]),
            );
        }
        b.fact(
            key::POWER_FACT_TECHNOLOGY,
            texts.t(&format!("power-technology-{}", battery.technology.id())),
        );
        let make = format!("{} {}", battery.vendor, battery.model)
            .trim()
            .to_owned();
        if !make.is_empty() {
            b.fact(key::POWER_FACT_MAKE, make);
        }
        if !battery.native_path.is_empty() {
            b.fact(key::POWER_FACT_SUPPLY, battery.native_path.clone());
        }
    }
}

/// `1 h 23 min`, in the person's language.
#[must_use]
pub fn duration_text(d: std::time::Duration, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::{Arg, key};
    let minutes = d.as_secs() / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    let h = i64::try_from(h).unwrap_or(i64::MAX);
    let m = i64::try_from(m).unwrap_or(0);
    if h == 0 {
        texts.t_args(key::POWER_DURATION_MINUTES, &[("m", Arg::Number(m))])
    } else if m == 0 {
        texts.t_args(key::POWER_DURATION_HOURS, &[("h", Arg::Number(h))])
    } else {
        texts.t_args(
            key::POWER_DURATION,
            &[("h", Arg::Number(h)), ("m", Arg::Number(m))],
        )
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

    /// A sentence that stands for a whole section — "no battery".
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
                self.left,
                layout::elide_end(&text, self.room),
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
        let focused = self.cursor == self.out.len();
        // A FACT IS A ROW ONE CANNOT ACT ON, so its value is grey — see
        // `colors.nix`, `text.read`.
        let value_colour = colour.unwrap_or(if focused {
            tokens::color::STATE_ACCENT
        } else {
            tokens::color::TEXT_READ_STRONG
        });
        // THE SAME COLUMN THE ROW BELOW IT USES. A fact used to be placed at a
        // third of the row and a `pair` at the surface's right edge, in this
        // same file — so two neighbouring lines of one section put their
        // values in two different places.
        self.out.push(Row {
            fields: crate::chrome::field_pair(
                self.left,
                self.room,
                &label,
                &value,
                if focused {
                    tokens::color::STATE_ACCENT
                } else {
                    // A FACT, and the label says so — see `colors.nix`.
                    tokens::color::TEXT_LABEL_FACT
                },
                value_colour,
            ),
            focused,
            heading: false,
            target: Target::Nothing,
            disabled: false,
        });
    }

    /// A row `RET` acts on. `disabled` draws it reachable and refusing — the
    /// format buffer's XFS row, the same shape.
    fn pair(&mut self, label: String, value: String, target: Target, disabled: bool) {
        let focused = self.cursor == self.out.len();
        // THE LABEL SAYS WHETHER THE ROW IS A DOOR, in a hue no value wears —
        // see `colors.nix`, `label`. It replaced a mark in front of the row,
        // which the owner refused: a chevron is what a tree uses to open and
        // close, and this row does neither.
        let (label_colour, value_colour) = match (focused, disabled) {
            (true, _) => (tokens::color::STATE_ACCENT, tokens::color::STATE_ACCENT),
            (false, true) => (tokens::color::TEXT_DIM, tokens::color::TEXT_DIM),
            (false, false) => (
                if target == Target::Nothing {
                    tokens::color::TEXT_LABEL_FACT
                } else {
                    tokens::color::TEXT_LABEL_ACTS
                },
                // AND THE VALUE SAYS THE SAME FROM THE OTHER SIDE: a row one
                // cannot act on takes the colour out of its value — see
                // `colors.nix`, `text.read`.
                if target == Target::Nothing {
                    tokens::color::TEXT_READ_STRONG
                } else {
                    tokens::color::TEXT_ANSWER
                },
            ),
        };
        // ONE ARITHMETIC, and it is `chrome::field_pair` — see the comment on
        // `chrome::value_column` for the three answers this file and its two
        // neighbours used to give.
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
            target,
            disabled,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_power::{Battery, Profiles, Sleep};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    #[test]
    fn every_key_the_line_teaches_does_something() {
        // The rule the other views hold to, and this one had no test for it:
        // a key the hint line teaches has to be a key the view answers to.
        // Proving each id has a TEXT is not proving that pressing it does
        // anything.
        let t = texts();
        let snap = laptop();
        for (notation, id) in OF_THE_VIEW {
            let press = match *notation {
                "<down>" => Press::Down,
                "<up>" => Press::Up,
                "<left>" => Press::Left,
                "<right>" => Press::Right,
                "RET" => Press::Enter,
                other => panic!("`{other}` ({id}) is taught and this test cannot press it"),
            };
            // ON EVERY ROW, and it has to work on at least one: `RET` on a
            // fact does nothing, and should.
            let v = View::fresh();
            let rows = v.rows(Some(&snap), Profile::Performance, DECIDING_WIDTH, &t);
            let mut worked = false;
            for row in 0..rows.len() {
                let mut here = v.clone();
                here.clicked(row, &rows);
                let was = here.cursor();
                let did = here.key(press, Some(&snap), Profile::Performance, &t);
                worked |= did != Errand::Nothing || here.cursor() != was;
            }
            assert!(
                worked,
                "`{notation}` ({id}) does nothing on any row of this view"
            );
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
        let snap = laptop();
        let want = |columns: u16| -> Vec<(bool, bool, Target)> {
            View::fresh()
                .rows(Some(&snap), Profile::Performance, columns, &t)
                .into_iter()
                .map(|r| (r.heading, r.disabled, r.target))
                .collect()
        };
        let at_eighty = want(DECIDING_WIDTH);
        for columns in [40_u16, 60, 100, 160, 240] {
            assert_eq!(want(columns), at_eighty, "the rows differ at {columns}");
        }
    }

    fn laptop() -> Snapshot {
        let mut battery = Battery::from_props("/b", &ronoctua_power::upower::Props::new());
        battery.present = true;
        battery.percentage = 42.0;
        battery.state = State::Discharging;
        battery.energy_full_wh = 50.0;
        battery.energy_full_design_wh = 60.0;
        battery.native_path = "BAT0".into();
        let sleep = Sleep {
            can_suspend: Can::Yes,
            can_hibernate: Can::NotAvailable,
            can_reboot: Can::Yes,
            can_power_off: Can::Yes,
            handle_lid: "suspend".into(),
            ..Default::default()
        };
        Snapshot {
            display: Some(battery.clone()),
            batteries: vec![battery],
            profiles: Some(Profiles {
                active: Profile::Balanced,
                offered: Profile::ALL.to_vec(),
                degraded: None,
                inhibited: None,
                holds: Vec::new(),
                battery_aware: Some(true),
                version: "0.30".into(),
            }),
            sleep,
            thresholds: Some(ronoctua_power::sysfs::Thresholds {
                supply: "BAT0".into(),
                start: Some(40),
                end: 80,
                writable: true,
                behaviour: None,
                types: None,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn every_word_this_view_builds_exists_in_every_language() {
        // THE HOLE THIS FILLS: seven families of key are built by
        // interpolation — `power-kind-{id}`, `power-state-{id}` and five more
        // — and a missing one reaches the screen as `[power-kind-toy]`, in
        // BOTH languages, with every other check green. The radio's view has
        // had this test since it was written; this one had none.
        for language in ronoctua_i18n::LANGUAGES {
            let t = ronoctua_i18n::Texts::fresh(language).unwrap();
            let mut keys: Vec<String> = Vec::new();
            keys.extend(
                ronoctua_power::Kind::all()
                    .iter()
                    .map(|k| format!("power-kind-{}", k.id())),
            );
            keys.extend(
                ronoctua_power::State::all()
                    .iter()
                    .map(|k| format!("power-state-{}", k.id())),
            );
            // `Warning` IS NOT DRAWN — the alarms read it (`alarms`) and the
            // catalogue words them; there is no `power-warning-*` family to
            // prove, and inventing one here would be a test asserting a
            // screen nobody wrote.
            keys.extend(
                ronoctua_power::Technology::all()
                    .iter()
                    .map(|k| format!("power-technology-{}", k.id())),
            );
            keys.extend(
                Profile::ALL
                    .iter()
                    .map(|p| format!("power-profile-{}", p.id())),
            );
            keys.extend(
                Action::ALL
                    .iter()
                    .map(|a| format!("power-action-{}", a.id())),
            );
            keys.extend(Keep::ALL.iter().map(|k| format!("power-keep-{}", k.id())));
            keys.extend(
                Preset::ALL
                    .iter()
                    .map(|p| format!("power-preset-{}", p.id())),
            );
            for key in keys {
                assert!(
                    !t.t(&key).starts_with('['),
                    "`{key}` has no text in {language}"
                );
            }
        }
    }

    #[test]
    fn a_heading_is_never_alone() {
        // A TITLE WITH NOTHING UNDER IT is a thing to look at that does not
        // exist. Every section either has a row or says so in a sentence —
        // and the one that did not (the lid, with no reading at all) drew an
        // orphan heading for a whole session, because `watch` failing only
        // warns and the snapshot stays `None` for ever.
        let t = texts();
        for snap in [None, Some(laptop())] {
            let rows = View::fresh().rows(snap.as_ref(), Profile::Performance, 80, &t);
            for (i, row) in rows.iter().enumerate() {
                // THE AIR IS A HEADING WITH NO FIELDS (`chrome::AIR`), and
                // what it is under is the title it introduces.
                if !row.heading || row.fields.is_empty() {
                    continue;
                }
                let next = rows.get(i + 1);
                assert!(
                    next.is_some_and(|r| !r.heading),
                    "the heading `{}` has nothing under it",
                    row.fields.first().map(|f| f.text.as_str()).unwrap_or("")
                );
            }
        }
    }

    #[test]
    fn nothing_leaks_at_any_width() {
        let t = texts();
        let snap = laptop();
        for columns in [40_u16, 60, 80, 120, 200] {
            for row in View::fresh().rows(Some(&snap), Profile::Performance, columns, &t) {
                for f in &row.fields {
                    assert!(
                        f.at + layout::width(&f.text) + crate::chrome::FIELDS_LEAD <= columns,
                        "{:?} leaks at {columns}",
                        f.text
                    );
                }
            }
        }
        // AND WITH NOTHING READ AT ALL, every section that is DRAWN says
        // something. Six of the eight can speak without a reading — "no
        // battery", "no daemon" — and the two that cannot (the lid, the
        // peripherals) are not drawn at all rather than drawn empty: see
        // `a_heading_is_never_alone`, which is where that rule lives.
        let rows = View::fresh().rows(None, Profile::Performance, 80, &t);
        assert!(
            rows.iter()
                .filter(|r| r.heading && !r.fields.is_empty())
                .count()
                >= 6
        );
        // EVERY ROW SAYS SOMETHING, and the air says it by being air: it is
        // the one row in the list that is fieldless on purpose.
        assert!(
            rows.iter()
                .all(|r| !r.fields.is_empty() || (r.heading && r.target == Target::Nothing))
        );
    }

    #[test]
    fn the_cursor_skips_headings_and_esc_walks_one_level() {
        let t = texts();
        let snap = laptop();
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        assert!(rows[0].heading);
        // The cursor starts on the first real row, and Down walks on from it
        // without ever landing on a heading.
        let _ = v.key(Press::Down, Some(&snap), Profile::Performance, &t);
        assert!(!rows[v.cursor()].heading);
        // Walk to "suspend" and arm it.
        let suspend = rows
            .iter()
            .position(|r| r.target == Target::Action(Action::Suspend, Can::Yes))
            .unwrap();
        while v.cursor() < suspend {
            let _ = v.key(Press::Down, Some(&snap), Profile::Performance, &t);
        }
        assert_eq!(v.cursor(), suspend, "a heading was not skipped");
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Nothing
        );
        assert_eq!(v.armed(), Some(Action::Suspend));
        // Esc disarms, and only the next Esc closes.
        assert_eq!(
            v.key(Press::Escape, Some(&snap), Profile::Performance, &t),
            Errand::Nothing
        );
        assert_eq!(v.armed(), None);
        assert_eq!(
            v.key(Press::Escape, Some(&snap), Profile::Performance, &t),
            Errand::Close
        );
        // The second RET on an armed action DOES it.
        let _ = v.key(Press::Enter, Some(&snap), Profile::Performance, &t);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Order(Order::Do(Action::Suspend))
        );
    }

    #[test]
    fn a_thing_logind_cannot_do_is_reachable_and_refuses() {
        let t = texts();
        let snap = laptop();
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let hibernate = rows
            .iter()
            .position(|r| matches!(r.target, Target::Action(Action::Hibernate, _)))
            .unwrap();
        v.clicked(hibernate, &rows);
        assert_eq!(v.cursor(), hibernate);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Nothing
        );
        assert_eq!(v.armed(), None, "an unavailable action never arms");
        let cannot = t.t("power-cannot");
        assert!(rows[hibernate].fields.iter().any(|f| f.text == cannot));
    }

    #[test]
    fn the_arrows_move_a_threshold_and_ret_writes_the_pair() {
        let t = texts();
        let snap = laptop();
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let end = rows
            .iter()
            .position(|r| r.target == Target::Threshold(Which::End))
            .unwrap();
        v.clicked(end, &rows);
        let _ = v.key(Press::Right, Some(&snap), Profile::Performance, &t);
        let _ = v.key(Press::Right, Some(&snap), Profile::Performance, &t);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Order(Order::Thresholds {
                start: Some(40),
                end: 90
            })
        );
        // A preset writes its pair outright.
        let preset = rows
            .iter()
            .position(|r| r.target == Target::Preset(Preset::Desk))
            .unwrap();
        v.clicked(preset, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Order(Order::Thresholds {
                start: Some(60),
                end: 80
            })
        );
        // Esc drops a pending pair before it closes anything.
        v.clicked(end, &rows);
        let _ = v.key(Press::Left, Some(&snap), Profile::Performance, &t);
        assert_eq!(
            v.key(Press::Escape, Some(&snap), Profile::Performance, &t),
            Errand::Nothing
        );
        assert_eq!(
            v.key(Press::Escape, Some(&snap), Profile::Performance, &t),
            Errand::Close
        );
    }

    #[test]
    fn a_fresh_view_focuses_a_row_not_a_heading() {
        let t = texts();
        let snap = laptop();
        let v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        assert!(!rows[v.cursor()].heading, "the cursor sits on a heading");
        assert!(rows[v.cursor()].focused);
        // And with nothing known at all, the row under the heading is the note.
        let rows = v.rows(None, Profile::Performance, 80, &t);
        assert!(!rows[v.cursor()].heading);
    }

    #[test]
    fn the_arrows_cycle_the_active_profile_through_the_offered_ones() {
        let t = texts();
        let snap = laptop();
        let p = snap.profiles.as_ref().unwrap();
        let offered: Vec<Profile> = Profile::ALL
            .iter()
            .copied()
            .filter(|x| p.offered.contains(x))
            .collect();
        assert!(offered.len() >= 2, "the fixture offers one profile");
        let i = offered.iter().position(|x| *x == p.active).unwrap();
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let row = rows
            .iter()
            .position(|r| matches!(r.target, Target::Profile(_)))
            .unwrap();
        v.clicked(row, &rows);
        assert_eq!(
            v.key(Press::Right, Some(&snap), Profile::Performance, &t),
            Errand::Choose(offered[(i + 1) % offered.len()])
        );
        assert_eq!(
            v.key(Press::Left, Some(&snap), Profile::Performance, &t),
            Errand::Choose(offered[(i + offered.len() - 1) % offered.len()])
        );
    }

    #[test]
    fn a_disabled_row_is_reachable_and_refuses() {
        let t = texts();
        let mut snap = laptop();
        snap.profiles.as_mut().unwrap().offered = vec![Profile::Balanced];
        snap.thresholds.as_mut().unwrap().writable = false;
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let row = rows
            .iter()
            .position(|r| r.target == Target::Profile(Profile::Performance))
            .unwrap();
        assert!(
            rows[row].disabled,
            "an un-offered profile is not drawn disabled"
        );
        v.clicked(row, &rows);
        assert_eq!(v.cursor(), row, "a disabled row cannot take the cursor");
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Nothing,
            "RET on an un-offered profile sent an order"
        );
        let preset = rows
            .iter()
            .position(|r| matches!(r.target, Target::Preset(_)))
            .unwrap();
        assert!(
            rows[preset].disabled,
            "a read-only preset is not drawn disabled"
        );
        v.clicked(preset, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Nothing,
            "RET on a read-only preset sent an order"
        );
        let end = rows
            .iter()
            .position(|r| r.target == Target::Threshold(Which::End))
            .unwrap();
        v.clicked(end, &rows);
        assert_eq!(
            v.key(Press::Left, Some(&snap), Profile::Performance, &t),
            Errand::Nothing
        );
        assert!(v.pending.is_none(), "an arrow moved a read-only threshold");
    }

    #[test]
    fn an_arrow_never_moves_a_threshold_the_wrong_way() {
        let t = texts();
        let mut snap = laptop();
        let th = snap.thresholds.as_mut().unwrap();
        th.writable = true;
        // A PAIR TIGHTER THAN ONE STEP, which firmware does allow.
        th.start = Some(78);
        th.end = 80;
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let end = rows
            .iter()
            .position(|r| r.target == Target::Threshold(Which::End))
            .unwrap();
        v.clicked(end, &rows);
        assert_eq!(
            v.key(Press::Left, Some(&snap), Profile::Performance, &t),
            Errand::Nothing
        );
        let (_, moved) = v.pending.unwrap();
        assert!(moved <= 80, "`←` raised the end from 80 to {moved}");
        let start = rows
            .iter()
            .position(|r| r.target == Target::Threshold(Which::Start))
            .unwrap();
        v.clicked(start, &rows);
        v.forget_pending();
        assert_eq!(
            v.key(Press::Right, Some(&snap), Profile::Performance, &t),
            Errand::Nothing
        );
        let (moved, _) = v.pending.unwrap();
        assert!(moved.unwrap() >= 78, "`→` lowered the start from 78");
    }

    #[test]
    fn the_question_follows_the_door_it_asked_about() {
        let t = texts();
        let mut snap = laptop();
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let row = rows
            .iter()
            .position(|r| matches!(r.target, Target::Action(Action::Suspend, _)))
            .unwrap();
        v.clicked(row, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Nothing,
            "the first RET should arm, not act"
        );
        assert_eq!(v.armed(), Some(Action::Suspend));
        // logind stops saying yes while the question stands.
        snap.sleep.can_suspend = Can::No;
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        v.settle(&rows);
        assert_eq!(v.armed(), None, "a dead question survived the machine");
    }

    #[test]
    fn a_preset_with_a_start_needs_a_battery_that_has_one() {
        let t = texts();
        let mut snap = laptop();
        let th = snap.thresholds.as_mut().unwrap();
        th.writable = true;
        th.start = None;
        let v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        for preset in Preset::ALL {
            let row = rows
                .iter()
                .find(|r| r.target == Target::Preset(preset))
                .unwrap();
            assert_eq!(
                row.disabled,
                preset.pair().0.is_some(),
                "{preset:?}: disabled must follow whether the preset needs a start"
            );
        }
    }

    #[test]
    fn the_end_never_comes_down_to_the_start() {
        let t = texts();
        let mut snap = laptop();
        let th = snap.thresholds.as_mut().unwrap();
        th.writable = true;
        th.start = Some(40);
        th.end = 80;
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let end = rows
            .iter()
            .position(|r| r.target == Target::Threshold(Which::End))
            .unwrap();
        v.clicked(end, &rows);
        for _ in 0..20 {
            assert_eq!(
                v.key(Press::Left, Some(&snap), Profile::Performance, &t),
                Errand::Nothing
            );
        }
        assert_eq!(v.pending, Some((Some(40), 45)), "the end met the start");
        for _ in 0..20 {
            assert_eq!(
                v.key(Press::Right, Some(&snap), Profile::Performance, &t),
                Errand::Nothing
            );
        }
        assert_eq!(v.pending, Some((Some(40), 100)));
        // And the pair it sends is the one the kernel takes.
        assert!(ronoctua_power::sysfs::admissible(Some(40), 45).is_ok());
    }

    #[test]
    fn a_profile_row_applies_and_a_switch_flips() {
        let t = texts();
        let snap = laptop();
        let mut v = View::fresh();
        let rows = v.rows(Some(&snap), Profile::Performance, 80, &t);
        let saver = rows
            .iter()
            .position(|r| r.target == Target::Profile(Profile::PowerSaver))
            .unwrap();
        v.clicked(saver, &rows);
        // A CHOICE, not a bare order: the doors send it AND write the file.
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Choose(Profile::PowerSaver)
        );
        let aware = rows
            .iter()
            .position(|r| matches!(r.target, Target::BatteryAware(_)))
            .unwrap();
        v.clicked(aware, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Order(Order::BatteryAware(false))
        );
        let keep = rows
            .iter()
            .position(|r| r.target == Target::Keep(Keep::Sleep, false))
            .unwrap();
        v.clicked(keep, &rows);
        assert_eq!(
            v.key(Press::Enter, Some(&snap), Profile::Performance, &t),
            Errand::Order(Order::KeepAwake(Keep::Sleep, true))
        );
    }

    #[test]
    fn the_rows_land_at_the_inset_and_the_hint_teaches_every_key() {
        let t = texts();
        let (at, _) = extent(80);
        assert_eq!(at + crate::chrome::FIELDS_LEAD, crate::chrome::INSET);
        let pieces = View::fresh().hint(&t);
        let ids = OF_THE_VIEW
            .iter()
            .map(|(_, id)| *id)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(pieces.len(), 3 * ids.len() - 1);
        // ARMED, the question leads the line — two pieces more.
        let mut armed = View::fresh();
        armed.arm(Action::Suspend, Can::Yes);
        assert_eq!(armed.hint(&t).len(), 3 * ids.len() + 1);
        for (_, id) in OF_THE_VIEW {
            for key in [(*id).to_owned(), format!("cmd-{id}")] {
                // THE BRACKETS ARE THE ANSWER, not the key itself: a key
                // nobody wrote resolves to `[key]` (`Texts::t_marked`), so
                // comparing against the key could never fail — which is
                // what this assertion used to do.
                assert!(!t.t(&key).starts_with('['), "`{key}` has no text");
            }
        }
    }

    #[test]
    fn durations_read_as_hours_and_minutes() {
        let t = texts();
        assert_eq!(
            duration_text(std::time::Duration::from_secs(4_980), &t),
            "1 h 23 min"
        );
        assert_eq!(
            duration_text(std::time::Duration::from_secs(600), &t),
            "10 min"
        );
    }
}

/// The modeline's battery segment: the glyph, and the number at
/// [`ronoctua_power::PERCENT_UNTIL`] and below — one cell between them, the
/// bell's measured reason (`main.rs`).
///
/// One place for the ladder-to-token map: the rung and the charging flag pick
/// the drawing, and the danger colour is the paint's to add (`segment_color`).
#[must_use]
pub fn battery_segment(m: &ronoctua_power::Mark) -> String {
    use ronoctua_power::Rung;
    // DANGER IS A COLOUR, NOT A DRAWING — and this used to be both.
    //
    // The owner asked for the colour to change at 15% and below; the glyph
    // was ALSO swapped for `battery-danger`, which is the same picture
    // whether the machine is on the charger or not. Nine per cent draining
    // and nine per cent charging drew the identical mark, at the one moment
    // when "is it charging?" is the question. Two rungs of the ladder
    // (`battery-empty`, `battery-charging-empty`) were unreachable as well,
    // because `Rung::Empty` is 0-10 and danger is 0-15.
    //
    // The exclamation mark is kept for the one case it says something the
    // ladder cannot: low AND unplugged, which is the state nobody wants to
    // discover late. Charging at the same percentage draws the charging
    // rung, in the danger colour.
    let glyph = if m.danger && !m.charging {
        tokens::icon::BATTERY_DANGER
    } else {
        match (m.rung, m.charging) {
            // `Rung::Empty` CANNOT ARRIVE HERE UNPLUGGED, and that is the
            // owner's rule and not an oversight: the rung is 0-10 and danger
            // is 0-15, so the branch above has already taken it. It used to
            // have an arm of its own drawing `battery-empty`, a token nothing
            // could reach — with a comment and a TEST that both said it
            // could, the test by building a `Mark` that `mark()` cannot
            // produce (`rung: Empty, danger: false`). The two rungs share the
            // one drawing that IS reachable.
            (Rung::Empty | Rung::Low, false) => tokens::icon::BATTERY_LOW,
            (Rung::Half, false) => tokens::icon::BATTERY_HALF,
            (Rung::High, false) => tokens::icon::BATTERY_HIGH,
            (Rung::Full, false) => tokens::icon::BATTERY_FULL,
            (Rung::Empty, true) => tokens::icon::BATTERY_CHARGING_EMPTY,
            (Rung::Low, true) => tokens::icon::BATTERY_CHARGING_LOW,
            (Rung::Half, true) => tokens::icon::BATTERY_CHARGING_HALF,
            (Rung::High, true) => tokens::icon::BATTERY_CHARGING_HIGH,
            (Rung::Full, true) => tokens::icon::BATTERY_CHARGING_FULL,
        }
    };
    match m.percent {
        Some(p) => format!("{glyph} {p}%"),
        None => glyph.to_string(),
    }
}

/// The profile's glyph — the leaf, the middle gauge, the full gauge.
#[must_use]
pub const fn profile_glyph(p: Profile) -> char {
    match p {
        Profile::PowerSaver => tokens::icon::PROFILE_POWER_SAVER,
        Profile::Balanced => tokens::icon::PROFILE_BALANCED,
        Profile::Performance => tokens::icon::PROFILE_PERFORMANCE,
    }
}

#[cfg(test)]
mod segment_tests {
    use super::*;
    use ronoctua_power::{Mark, Rung};

    #[test]
    fn the_segment_is_one_glyph_and_the_number_only_when_low() {
        let quiet = Mark {
            rung: Rung::Half,
            charging: false,
            percent: None,
            danger: false,
        };
        assert_eq!(layout::width(&battery_segment(&quiet)), 1);
        let low = Mark {
            rung: Rung::Low,
            charging: true,
            percent: Some(22),
            danger: false,
        };
        let text = battery_segment(&low);
        assert!(text.ends_with(" 22%"), "{text}");
        assert!(
            text.starts_with(tokens::icon::BATTERY_CHARGING_LOW),
            "the bolt"
        );
        let danger = Mark {
            rung: Rung::Empty,
            charging: false,
            percent: Some(9),
            danger: true,
        };
        assert!(battery_segment(&danger).starts_with(tokens::icon::BATTERY_DANGER));
    }

    #[test]
    fn the_glyph_still_says_whether_it_is_charging_at_the_danger_mark() {
        // NINE PER CENT ON THE CHARGER AND NINE PER CENT DRAINING drew the
        // same picture, because the danger branch came first and
        // `battery-danger` is one drawing. That is the one moment when "is it
        // charging?" is the whole question. The COLOUR is what danger
        // changes — `segment_color` — and the drawing is the ladder's.
        let charging = Mark {
            rung: Rung::Empty,
            charging: true,
            percent: Some(9),
            danger: true,
        };
        assert!(
            battery_segment(&charging).starts_with(tokens::icon::BATTERY_CHARGING_EMPTY),
            "the danger mark hid the charging"
        );
        let draining = Mark {
            charging: false,
            ..charging
        };
        assert_ne!(
            battery_segment(&draining),
            battery_segment(&charging),
            "the two states draw the same thing"
        );
        // AND THE BOTTOM RUNG IS NOT DRAWN UNPLUGGED, which is the rule and
        // not an accident: `Rung::Empty` is 0-10 and danger is 0-15, so
        // `mark()` cannot produce an empty-and-not-in-danger battery. The
        // assertion that used to be here built exactly that impossible
        // `Mark` and called the result proof.
        for percent in 0..=15_u8 {
            let m = Mark {
                rung: Rung::of(percent),
                charging: false,
                percent: Some(percent),
                danger: true,
            };
            assert!(
                battery_segment(&m).starts_with(tokens::icon::BATTERY_DANGER),
                "{percent}% unplugged should be the alert glyph"
            );
        }
    }
}
