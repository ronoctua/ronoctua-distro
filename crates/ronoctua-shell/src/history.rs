//! The shell's history, and whether it leaves this machine — `LEADER s h`.
//!
//! # What this view is answering
//!
//! `nix/modules/home.nix` used to carry `auto_sync = false` with a reason in
//! words: *"syncing shell history to a third-party server would put every
//! command — including the ones with a token in argv — outside this machine"*.
//! It was right about the risk and it decided for the person, and the owner
//! asked for the other answer.
//!
//! So the section that turns it on is not a lone switch. It is drawn beside
//! the two settings that answer the objection — `secrets_filter` and
//! `history_filter`, the things that decide what is never RECORDED, and
//! therefore what cannot leave. A safeguard in a closed drawer is not one, so
//! neither is `advanced`.
//!
//! # The key is shown, and it is the only thing here behind a question
//!
//! `atuin key` prints the phrase that decrypts the history, and a second
//! machine cannot sync without it. A view that hid it would send the person to
//! a terminal, which is the failure this whole shell exists to remove. So it
//! is shown — behind a question that opens on NO, the way the Bluetooth
//! pairing and the format buffer already do.

use ronoctua_atuin::options::{self, Scope, Setting};
use ronoctua_options::Kind;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit, Row};

/// The keys this view answers to, and what the hint is generated from.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "atuin-down"),
    ("<up>", "atuin-up"),
    ("RET", "atuin-enter"),
    // `S-<arrows>` CHANGE THE VALUE, which is this system's rule everywhere a
    // row carries one: the bare arrows walk the list, and a row that took them
    // would be a row nobody could get past.
    ("S-<right>", "atuin-more"),
    ("S-<left>", "atuin-less"),
    ("s", "atuin-sync"),
    // `ESC`, AND IN CAPITALS: `ronoctua_keymap::Sym::Esc` writes itself that
    // way, and a notation the parser does not accept is a key the hint
    // silently drops — which the network view's own count caught once already.
    ("ESC", "atuin-back"),
];

/// A key, reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Down one row.
    Down,
    /// Up one row.
    Up,
    /// Act on the row.
    Enter,
    /// Turn the value forwards.
    More,
    /// Turn it back.
    Less,
    /// Sync now.
    Sync,
    /// One level back.
    Escape,
}

/// What `RET` does on a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A heading, or a fact.
    Nothing,
    /// One declared setting, by its id.
    Setting(&'static str),
    /// Sync now.
    Sync,
    /// Log in to a server.
    LogIn,
    /// Log out, and forget the key.
    LogOut,
    /// Show the encryption key.
    Key,
    /// Open or close the drawer.
    Drawer,
    /// Which modeline mark to draw.
    Mark,
}

/// Which text a mark's value is.
///
/// A `match` AND NOT A FORMATTED STEM: `format!("atuin-mark-{}", m.id())` would
/// compile whatever the ring grew, and invariant 8 buys exactly the guarantee
/// that a missing text is a build error.
#[must_use]
pub const fn mark_key(mark: ronoctua_atuin::prefs::Mark) -> &'static str {
    use ronoctua_atuin::prefs::Mark;
    match mark {
        Mark::Never => ronoctua_i18n::key::ATUIN_MARK_NEVER,
        Mark::WhenOff => ronoctua_i18n::key::ATUIN_MARK_WHEN_OFF,
        Mark::WhenOn => ronoctua_i18n::key::ATUIN_MARK_WHEN_ON,
    }
}

/// What the view wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Send this to the thread.
    Order(ronoctua_atuin::Order),
    /// Ask for this setting's value to be typed.
    Edit(&'static str),
    /// Ask where the server is, and who to be.
    AskServer,
    /// Show the key — the question has been answered yes.
    ShowKey,
    /// Close the view.
    Close,
}

/// Everything the rows are drawn from.
pub struct Facts<'a> {
    /// What Atuin says.
    pub shot: &'a ronoctua_atuin::Snapshot,
}

/// The whole state of the view.
#[derive(Debug, Default, Clone)]
pub struct View {
    cursor: usize,
    /// Is the drawer of rarely-touched settings open?
    ///
    /// IN THE VIEW AND NOT ON DISK — a drawer somebody opened once is not a
    /// preference, and closing the view is what forgets it.
    drawer: bool,
    /// Is the question about showing the key on screen?
    asking: bool,
    /// The key, once it has been asked for and answered.
    key: Option<String>,
}

impl View {
    /// A fresh one.
    ///
    /// The cursor is settled by [`View::settle`] the first time anything asks,
    /// and NOT left on row zero: that is a heading, and the locale view opened
    /// there once — the owner read a screen where no key did anything.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Moves the cursor to the first row that answers a key.
    ///
    /// Called by everything that reads the cursor, so there is no order of
    /// operations to get wrong: a view whose door forgot to settle it would
    /// open on a heading, which is the defect this is named after.
    fn settle(&self, built: &[(Vec<Field>, Target)]) -> usize {
        if built
            .get(self.cursor)
            .is_some_and(|(_, t)| !matches!(t, Target::Nothing))
        {
            return self.cursor;
        }
        built
            .iter()
            .position(|(_, t)| !matches!(t, Target::Nothing))
            .unwrap_or(self.cursor)
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Is the question up?
    ///
    /// Read by this view's own tests, which are what hold the rule that the
    /// key is shown only on the SECOND press.
    #[cfg(test)]
    pub(crate) const fn asking(&self) -> bool {
        self.asking
    }

    /// Takes the key the thread sent back.
    pub fn took_the_key(&mut self, key: String) {
        self.key = (!key.is_empty()).then_some(key);
        self.asking = false;
    }

    /// The hint line, generated from the table.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(OF_THE_VIEW, texts)
    }

    /// The rows, for a region this wide.
    #[must_use]
    pub fn rows(&self, facts: &Facts<'_>, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let built = self.build(facts, columns, texts);
        let here = self.settle(&built);
        built
            .into_iter()
            .enumerate()
            .map(|(n, (mut fields, target))| {
                let focused = n == here && !matches!(target, Target::Nothing);
                // THE CURSOR IS A COLOUR, because nothing else here could be
                // one. `Row::focused` is read by NO door in this system —
                // neither the footer nor the window draws a band behind the
                // cursor — so a view that only set the flag drew a screen with
                // no way to tell which line the keyboard was on. That is what
                // the owner read on this one; the other nine views have
                // carried the colour since `network.rs` arrived at the split.
                if focused {
                    Self::accent(&mut fields, &target);
                }
                Row {
                    fields,
                    marked: false,
                    focused,
                    foot: false,
                }
            })
            .collect()
    }

    /// Paints the row the keyboard is on.
    ///
    /// TWO HALVES AND TWO ANSWERS, which is the split `network.rs` arrived at
    /// and every view since repeats: the LABEL carries "the keyboard is here"
    /// (`state.accent`), and the VALUE goes on saying whether the answer can
    /// be changed. One accent over both made a door and a fact the same row at
    /// the very moment the cursor was on them — which is the moment the
    /// question is being asked.
    ///
    /// The drawer is the exception and takes the accent WHOLE: its chevron and
    /// its word are one label, not a label and a value, and colouring them
    /// apart would split a glyph from the noun it belongs to.
    fn accent(fields: &mut [Field], target: &Target) {
        if matches!(target, Target::Drawer) {
            for field in fields.iter_mut() {
                field.color = tokens::color::STATE_ACCENT;
            }
            return;
        }
        let answers = !Self::refuses(target);
        let mut halves = fields.iter_mut();
        if let Some(label) = halves.next() {
            label.color = tokens::color::STATE_ACCENT;
        }
        for value in halves {
            value.color = if answers {
                tokens::color::TEXT_HERE_ACTS
            } else {
                tokens::color::TEXT_HERE_READ
            };
        }
    }

    /// Does the row this target names only SAY its value?
    ///
    /// A setting the flake owns is drawn, reachable and REFUSING (invariant 6)
    /// — so under the cursor it has to go on saying that, which is
    /// `text.here.read` and not `text.here.acts`. Everything else in this view
    /// changes something.
    fn refuses(target: &Target) -> bool {
        match target {
            Target::Setting(id) => options::by_id(id).is_some_and(|s| s.fixed),
            Target::Nothing => true,
            _ => false,
        }
    }

    /// What every row is and what it does.
    fn build(
        &self,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<(Vec<Field>, Target)> {
        let at = crate::chrome::INSET;
        let room = columns.saturating_sub(at);
        let shot = facts.shot;
        let mut out: Vec<(Vec<Field>, Target)> = Vec::new();

        let heading = |out: &mut Vec<(Vec<Field>, Target)>, key: &str| {
            if !out.is_empty() {
                out.push((Vec::new(), Target::Nothing));
            }
            out.push((
                vec![piece(at, texts.t(key), tokens::color::TEXT_GROUP)],
                Target::Nothing,
            ));
        };
        let fact = |label: &str, value: &str| {
            (
                crate::chrome::field_pair(
                    at,
                    room,
                    label,
                    value,
                    tokens::color::TEXT_LABEL_FACT,
                    tokens::color::TEXT_PRIMARY,
                ),
                Target::Nothing,
            )
        };
        let door = |label: &str, value: &str, target: Target| {
            (
                crate::chrome::field_pair(
                    at,
                    room,
                    label,
                    value,
                    tokens::color::TEXT_LABEL_ACTS,
                    tokens::color::TEXT_ANSWER,
                ),
                target,
            )
        };

        // A MACHINE WITH NO `atuin` SAYS SO AND STOPS. Every row below would
        // be about a program that is not there.
        if !shot.present {
            out.push((
                vec![piece(
                    at,
                    texts.t(ronoctua_i18n::key::ATUIN_ABSENT),
                    tokens::color::TEXT_DIM,
                )],
                Target::Nothing,
            ));
            return out;
        }

        heading(&mut out, ronoctua_i18n::key::ATUIN_HEADING_GLANCE);
        out.push(fact(
            // THE LABEL IS NOT THE VALUE. It read `atuin-syncing` on both
            // sides, so the row said "syncing   syncing" — a definition list
            // whose term and definition are one word says nothing.
            &texts.t(ronoctua_i18n::key::ATUIN_STATE),
            &texts.t(if shot.crossing() {
                ronoctua_i18n::key::ATUIN_SYNCING
            } else {
                ronoctua_i18n::key::ATUIN_NOT_SYNCING
            }),
        ));
        out.push(fact(
            &texts.t(ronoctua_i18n::key::ATUIN_ENTRIES),
            &shot.entries.to_string(),
        ));
        if shot.logged_in {
            // BOUND FIRST: `&texts.t(…)` inside the `if` is a temporary that
            // dies at the end of the expression, and the borrow outlives it.
            let never = texts.t(ronoctua_i18n::key::ATUIN_NEVER);
            let last = if shot.last.is_empty() {
                never.as_str()
            } else {
                shot.last.as_str()
            };
            out.push(fact(&texts.t(ronoctua_i18n::key::ATUIN_LAST_SYNC), last));
        }
        // OURS, AND THE ONLY ONE IN THIS VIEW — the shape the Syncthing view's
        // mark row has, in its words: everything else belongs to Atuin and is
        // kept by Atuin, and this is a decision it has no opinion about. It
        // had a ring, a default and a modeline reader, and no row anywhere:
        // a preference nobody can reach has one value for ever.
        out.push(door(
            &texts.t(ronoctua_i18n::key::ATUIN_MARK),
            &texts.t(mark_key(shot.mark)),
            Target::Mark,
        ));

        heading(&mut out, ronoctua_i18n::key::ATUIN_HEADING_ACCOUNT);
        if shot.logged_in {
            out.push(fact(
                &texts.t(ronoctua_i18n::key::ATUIN_SERVER),
                &shot.address,
            ));
            out.push(fact(
                &texts.t(ronoctua_i18n::key::ATUIN_ACCOUNT),
                &shot.username,
            ));
            out.push(door(
                &texts.t(ronoctua_i18n::key::ATUIN_SYNC_NOW),
                "",
                Target::Sync,
            ));
            // THE KEY, BEHIND A QUESTION THAT OPENS ON NO. It is what
            // decrypts the history, and a second machine cannot sync without
            // it — so hiding it would send the person to a terminal, which is
            // the failure this shell exists to remove.
            match (self.asking, self.key.as_deref()) {
                (_, Some(key)) => {
                    out.push(fact(&texts.t(ronoctua_i18n::key::ATUIN_SHOW_KEY), key));
                }
                (true, None) => {
                    out.push((
                        vec![piece(
                            at,
                            texts.t(ronoctua_i18n::key::ATUIN_KEY_ABOUT),
                            tokens::color::TEXT_DIM,
                        )],
                        Target::Nothing,
                    ));
                    out.push(door(
                        &texts.t(ronoctua_i18n::key::ATUIN_KEY_ASK),
                        "",
                        Target::Key,
                    ));
                }
                (false, None) => {
                    out.push(door(
                        &texts.t(ronoctua_i18n::key::ATUIN_SHOW_KEY),
                        "",
                        Target::Key,
                    ));
                }
            }
            out.push(door(
                &texts.t(ronoctua_i18n::key::ATUIN_LOG_OUT),
                "",
                Target::LogOut,
            ));
        } else {
            out.push(fact(
                &texts.t(ronoctua_i18n::key::ATUIN_ACCOUNT),
                &texts.t(ronoctua_i18n::key::ATUIN_NOT_LOGGED_IN),
            ));
            out.push(door(
                &texts.t(ronoctua_i18n::key::ATUIN_LOG_IN),
                "",
                Target::LogIn,
            ));
        }

        for (key, scope) in [
            (ronoctua_i18n::key::ATUIN_HEADING_LEAVING, Scope::Leaving),
            (ronoctua_i18n::key::ATUIN_HEADING_SEARCH, Scope::Search),
            (ronoctua_i18n::key::ATUIN_HEADING_KEYS, Scope::Keys),
        ] {
            let plain: Vec<&Setting> = options::of_scope(scope).filter(|s| !s.advanced).collect();
            if plain.is_empty() {
                continue;
            }
            heading(&mut out, key);
            for setting in plain {
                out.push(Self::row_of(setting, shot, at, room, texts));
            }
        }

        // THE DRAWER, and the chevron is its and nobody else's.
        out.push((Vec::new(), Target::Nothing));
        out.push(Self::drawer_row(self.drawer, at, room, texts));
        if self.drawer {
            out.push((
                vec![piece(
                    at,
                    texts.t(ronoctua_i18n::key::ATUIN_ADVANCED_ABOUT),
                    tokens::color::TEXT_DIM,
                )],
                Target::Nothing,
            ));
            // THE SCOPE *AND* THE FLAG. A setting declared in the `advanced`
            // scope without the flag was drawn NOWHERE — not in its own
            // section, because the three above take only the plain ones, and
            // not in the drawer, because the drawer took only the flag.
            // `update_check` was exactly that, and the test that asks whether
            // a flake-owned row refuses is what found it.
            for setting in options::SETTINGS
                .iter()
                .filter(|s| s.advanced || s.scope == Scope::Advanced)
            {
                out.push(Self::row_of(setting, shot, at, room, texts));
            }
        }
        out
    }

    /// One declared setting's row.
    fn row_of(
        setting: &'static Setting,
        shot: &ronoctua_atuin::Snapshot,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> (Vec<Field>, Target) {
        let label = texts.t(setting.label);
        // WHAT THE FLAKE OWNS SAYS SO instead of saying its value: the value in
        // the file is not what is in force, so printing it would be printing a
        // number that does not apply.
        if setting.fixed {
            return (
                crate::chrome::field_pair(
                    at,
                    room,
                    &label,
                    &texts.t(ronoctua_i18n::key::ATUIN_FROM_FLAKE),
                    tokens::color::TEXT_LABEL_ACTS,
                    tokens::color::TEXT_DIM,
                ),
                Target::Setting(setting.id),
            );
        }
        // NOT WRITTEN IS NOT OFF. Atuin's defaults differ per setting and are
        // not in the file, so a screen that showed one would state something
        // this system never wrote.
        let value = if !setting.written(&shot.settings) {
            texts.t(ronoctua_i18n::key::ATUIN_NOT_SET)
        } else {
            match setting.kind {
                // A SWITCH SAYS ON OR OFF, not what the SERVICE is doing.
                // These two read `atuin-syncing`/`atuin-not-syncing`, which
                // are the glance's words: `update_check` drew "check for a new
                // version   staying on this machine".
                Kind::Switch => texts.t(if setting.is_on(&shot.settings) {
                    ronoctua_i18n::key::ATUIN_ON
                } else {
                    ronoctua_i18n::key::ATUIN_OFF
                }),
                Kind::Ring(_) => setting
                    .word(&shot.settings)
                    .map(|k| texts.t(&k))
                    .unwrap_or_default(),
                Kind::Number { unit, .. } => {
                    let now = setting.plain(&shot.settings).unwrap_or("").to_owned();
                    match unit {
                        Some(u) => format!("{now} {}", texts.t(u)),
                        None => now,
                    }
                }
                Kind::Text | Kind::List | Kind::Amount => {
                    setting.plain(&shot.settings).unwrap_or("").to_owned()
                }
            }
        };
        (
            crate::chrome::field_pair(
                at,
                room,
                &label,
                &value,
                tokens::color::TEXT_LABEL_ACTS,
                tokens::color::TEXT_ANSWER,
            ),
            Target::Setting(setting.id),
        )
    }

    /// The drawer's own line.
    fn drawer_row(
        open: bool,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> (Vec<Field>, Target) {
        let glyph = if open {
            tokens::icon::COLLAPSE
        } else {
            tokens::icon::EXPAND
        };
        (
            vec![
                piece(at, glyph.to_string(), tokens::color::TEXT_GROUP),
                // THE GLYPH IS ONE CELL AND THE WORD STARTS AFTER IT, with a
                // space between: a mark against the letters is the "glifo
                // encostado na borda" the owner has had to point out before.
                piece(
                    at + 2,
                    ronoctua_render::layout::elide_end(
                        &texts.t(ronoctua_i18n::key::ATUIN_HEADING_ADVANCED),
                        room.saturating_sub(2),
                    ),
                    tokens::color::TEXT_GROUP,
                ),
            ],
            Target::Drawer,
        )
    }

    /// Turns the modeline's ring.
    ///
    /// It reads the mark out of the SNAPSHOT and not out of a field of its
    /// own: the file is the broadcast, both doors write it, and a view holding
    /// its own copy would turn from a value the other door had already moved.
    fn turn_mark(facts: &Facts<'_>, forward: bool) -> Errand {
        let all = ronoctua_atuin::prefs::Mark::ALL;
        let at = all.iter().position(|m| *m == facts.shot.mark).unwrap_or(0);
        // BACKWARDS IS `len - 1` FORWARD, which is what keeps the arithmetic
        // in `usize` — `at - 1` underflows at zero.
        let step = if forward { 1 } else { all.len() - 1 };
        Errand::Order(ronoctua_atuin::Order::Mark(all[(at + step) % all.len()]))
    }

    /// What the cursor's row would do.
    ///
    /// Read by this view's own tests, which are what hold the rule that the
    /// cursor never stops where `RET` does nothing.
    #[cfg(test)]
    pub(crate) fn target_at(
        &self,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Target {
        let built = self.build(facts, columns, texts);
        let here = self.settle(&built);
        built.get(here).map_or(Target::Nothing, |(_, t)| t.clone())
    }

    /// Where a click fell.
    pub fn clicked(&mut self, row: usize) {
        self.cursor = row;
    }

    /// Acts on a key.
    #[must_use]
    pub fn key(
        &mut self,
        press: Press,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let built = self.build(facts, columns, texts);
        self.cursor = self.settle(&built);
        let target = built
            .get(self.cursor)
            .map_or(Target::Nothing, |(_, t)| t.clone());
        match press {
            Press::Up | Press::Down => {
                let step: isize = if press == Press::Down { 1 } else { -1 };
                let mut i = self.cursor;
                while let Some(next) = i.checked_add_signed(step) {
                    if next >= built.len() {
                        break;
                    }
                    i = next;
                    // THE CURSOR ONLY STOPS WHERE A KEY DOES SOMETHING.
                    // Walking through headings and facts with nothing lighting
                    // up is what the owner read on the locale view.
                    if !matches!(built[i].1, Target::Nothing) {
                        self.cursor = i;
                        break;
                    }
                }
                Errand::Nothing
            }
            // `Esc` PUTS THE KEY AWAY BEFORE IT CLOSES THE VIEW. A recovery
            // phrase on screen is a thing that is open.
            Press::Escape if self.key.is_some() || self.asking => {
                self.key = None;
                self.asking = false;
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            Press::Sync => Errand::Order(ronoctua_atuin::Order::Sync),
            Press::More | Press::Less => match target {
                Target::Setting(id) => self.turn(facts, id, press == Press::More),
                Target::Mark => Self::turn_mark(facts, press == Press::More),
                _ => Errand::Nothing,
            },
            Press::Enter => match target {
                Target::Nothing => Errand::Nothing,
                // `RET` TURNS IT FORWARD, because `RET` is what a CLICK
                // becomes: a row reachable only by a shifted arrow would be a
                // row the mouse cannot use.
                Target::Mark => Self::turn_mark(facts, true),
                Target::Sync => Errand::Order(ronoctua_atuin::Order::Sync),
                Target::LogIn => Errand::AskServer,
                Target::LogOut => {
                    self.key = None;
                    Errand::Order(ronoctua_atuin::Order::LogOut)
                }
                Target::Key => {
                    // THE QUESTION OPENS ON NO: the first `RET` asks, and only
                    // the second shows. It is the shape the Bluetooth pairing
                    // and the format buffer already have.
                    if self.asking {
                        Errand::ShowKey
                    } else {
                        self.asking = true;
                        Errand::Nothing
                    }
                }
                Target::Drawer => {
                    self.drawer = !self.drawer;
                    Errand::Nothing
                }
                Target::Setting(id) => self.turn(facts, id, true),
            },
        }
    }

    /// What a key does to one setting.
    fn turn(&mut self, facts: &Facts<'_>, id: &'static str, forwards: bool) -> Errand {
        let Some(setting) = options::by_id(id) else {
            debug_assert!(false, "a row named `{id}`, which is not in the table");
            return Errand::Nothing;
        };
        if setting.fixed {
            return Errand::Nothing;
        }
        match setting.turned(&facts.shot.settings, forwards) {
            Some(value) => Errand::Order(ronoctua_atuin::Order::Set {
                section: setting.within.map(ToOwned::to_owned),
                key: setting.key.to_owned(),
                value,
            }),
            // TYPED, NOT TURNED. The door decides where the typing happens,
            // and it is the footer's minibuffer.
            None => Errand::Edit(id),
        }
    }
}

/// One field.
fn piece(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        at,
        text,
        color,
        hit: Hit::Nothing,
        explain: None,
    }
}

/// The press an id runs.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "atuin-down" => Press::Down,
        "atuin-up" => Press::Up,
        "atuin-enter" => Press::Enter,
        "atuin-more" => Press::More,
        "atuin-less" => Press::Less,
        "atuin-sync" => Press::Sync,
        "atuin-back" => Press::Escape,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
