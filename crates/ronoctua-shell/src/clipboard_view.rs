//! The clipboard's history, as a VIEW — which it had never been.
//!
//! # What was wrong, exactly
//!
//! The owner photographed the footer with an empty history: the words
//! `ClipCascade: not integrated` sitting on the MINIBUFFER's line, and the
//! actions strip cut off to the right of "Nothing copied yet". Neither was
//! where it was meant to be, and the cause was arithmetic.
//!
//! The panel reserved three lines and drew them at `end - 3`, `end - 2` and
//! `end - 1`, where `end = first_of_expansion + grid.lines`. With an empty
//! history `grid.lines` is ONE — and nothing clamped those numbers to the top
//! of the region. So `end - 2` landed on the minibuffer and `end - 1` on the
//! row that says the history is empty. It held for any history shorter than
//! four entries, and there was no test of the clipboard's layout at all.
//!
//! So nothing here computes a line. The view returns ROWS, the region places
//! them, and the two halves of each row are fields at known columns — which is
//! what `network.rs` and `notifications.rs` have always done and what the
//! clipboard alone did not.
//!
//! # And it was the only modal view without a module
//!
//! It had no `OF_THE_VIEW`, no `hint()`, and it was the one arm of the
//! footer's hint `match` that fell through to `None`. Its action strip was
//! hand-rolled from the LONG `cmd-…` labels, in one colour, with the separator
//! written into the Rust — which is why the photograph reads `d Remove from
//! history` where every other view reads `d remove`. The hint comes from
//! `chrome::hint_of` now, like all nine others, which also buys it the
//! elision and the drawer that make a long line reachable.

use ronoctua_clipboard::{Entry, Queue};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::files::{Field, Hit, Row};

/// The keys this view answers to, and what the hint line is generated from.
///
/// BARE LETTERS ARE FREE HERE because the view captures the whole keyboard
/// while it is open — `d` removes, and it is not `LEADER d`. They live in the
/// view now rather than in `leader.rs`: every other view keeps its own table,
/// and the one that did not was the one whose hint was written by hand.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "clipboard-down"),
    ("<up>", "clipboard-up"),
    ("RET", "clipboard-copy-of-new"),
    ("d", "clipboard-remove"),
    ("f", "clipboard-pin"),
    ("q", "clipboard-qrcode"),
    ("D", "clipboard-clear"),
    // THE LAST ONE USED TO BE THE FIRST THING ELIDED. `c` reaches the sync
    // settings, and in the old hand-written strip it sat 23 cells past the cut
    // at every real width — so the key that opened ClipCascade was the one the
    // line could never teach. `chrome::hint_spans` elides with an ellipsis and
    // the fold makes the rest reachable, which is the whole difference.
    ("c", "clipboard-sync"),
    // `S-<arrows>` TURN A VALUE, which is the house's rule wherever a row
    // carries a ring or a switch. TWO ROWS AND NOT ONE `S-<left>/<right>`:
    // this table is matched against a key's own notation, so a notation no key
    // can produce would be taught on the hint line and answer nothing.
    ("S-<right>", "clipboard-more"),
    ("S-<left>", "clipboard-less"),
];

/// Which text a mark's value is.
///
/// A `match` AND NOT A FORMATTED STEM: `format!("clipboard-mark-{}", m.id())`
/// would compile whatever the ring grew, and invariant 8 buys exactly the
/// guarantee that a missing text is a build error.
#[must_use]
pub const fn mark_key(mark: ronoctua_clipcascade::prefs::Mark) -> &'static str {
    use ronoctua_clipcascade::prefs::Mark;
    match mark {
        Mark::Never => ronoctua_i18n::key::CLIPBOARD_MARK_NEVER,
        Mark::WhenOff => ronoctua_i18n::key::CLIPBOARD_MARK_WHEN_OFF,
        Mark::WhenOn => ronoctua_i18n::key::CLIPBOARD_MARK_WHEN_ON,
    }
}

/// A key, reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Down one entry.
    Down,
    /// Up one entry.
    Up,
    /// Put this back on the clipboard.
    Enter,
    /// Drop it from the history.
    Remove,
    /// Pin it, or unpin it.
    Pin,
    /// Show it as a QR code.
    Qr,
    /// Empty the history.
    Clear,
    /// Open the sync settings.
    Sync,
    /// Turn this row's value forward.
    More,
    /// Turn it back.
    Less,
    /// One level back.
    Escape,
}

/// What the view wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Put this entry back on the system clipboard.
    Copy(usize),
    /// Drop it.
    Remove(usize),
    /// Pin or unpin it.
    Pin(usize),
    /// Draw it as a QR code, or stop.
    Qr(usize),
    /// Empty the history.
    Clear,
    /// Turn sync on or off.
    Sync(bool),
    /// Ask where the server is.
    AskServer,
    /// Ask who to log in as.
    AskAccount,
    /// Forget the account.
    LogOut,
    /// Keep the settings that are not the account's.
    Keep(Box<ronoctua_clipcascade::prefs::Settings>),
    /// Ask what the salt is.
    AskSalt,
    /// Ask how many rounds.
    AskRounds,
    /// Ask how large an entry may be.
    AskCap,
    /// Close the view.
    Close,
}

/// What `RET` does on a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A heading, or a fact.
    Nothing,
    /// One entry of the history.
    Entry(usize),
    /// The switch that turns syncing on.
    Sync(bool),
    /// The row that asks where the server is.
    Server,
    /// The row that asks for the account.
    Account,
    /// The row that forgets it.
    Forget,
    /// The salt that goes into the key.
    Salt,
    /// How many PBKDF2 rounds.
    Rounds,
    /// Should images cross the network too?
    Images,
    /// Should what arrives raise a notification?
    Notify,
    /// The largest entry this machine sends.
    Cap,
    /// Which modeline mark to draw.
    Mark,
}

/// Everything the rows are drawn from.
///
/// ONE PARAMETER AND NOT THREE, the reason `network::Facts` gives: a signature
/// that grows with the view has to be edited in both doors, the click and
/// every test each time a section is added.
pub struct Facts<'a> {
    /// The history.
    pub queue: &'a Queue,
    /// What ClipCascade says, when the crate is running.
    pub sync: &'a ronoctua_clipcascade::Snapshot,
}

/// The whole state of the view.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    /// Which entry is being shown as a QR code, when one is.
    qr: Option<usize>,
}

/// How wide the history column is, of a region this wide.
///
/// Two fifths, with a floor — the same split the clipboard has always had, now
/// written once instead of being recomputed by the painting code.
#[must_use]
pub fn history_columns(columns: u16) -> u16 {
    const FLOOR: u16 = 16;
    (columns * 2 / 5).max(FLOOR).min(columns.saturating_sub(2))
}

/// Where the second column starts.
///
/// One cell past the rule, which the region draws at `history_columns`. It is
/// the same seam the theme chooser uses, and it is the column the pointer
/// tests to decide which half it fell in.
#[must_use]
pub fn detail_at(columns: u16) -> u16 {
    history_columns(columns).saturating_add(2)
}

impl View {
    /// A fresh one.
    #[must_use]
    pub fn fresh() -> Self {
        Self::default()
    }

    /// Which entry the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Moves the cursor to the first row that answers a key.
    ///
    /// THE CURSOR IS BORN ON A ROW THAT ACTS. Row zero of this view is the
    /// `History` heading, so a fresh view lit nothing and the first `<down>`
    /// spent itself reaching the first entry — which made the door's key count
    /// one short of every other view's, and `clipboard-vm` timed out waiting
    /// for a copy that a `RET` on a heading had not made.
    ///
    /// It is the rule the locale view was reported for, and it lives here
    /// rather than in the doors so that neither can forget it: everything that
    /// reads the cursor asks this first.
    fn settle(&self, targets: &[Target]) -> usize {
        if targets
            .get(self.cursor)
            .is_some_and(|t| !matches!(t, Target::Nothing))
        {
            return self.cursor;
        }
        targets
            .iter()
            .position(|t| !matches!(t, Target::Nothing))
            .unwrap_or(self.cursor)
    }

    /// Which entry is showing as a QR code, if any.
    #[must_use]
    pub const fn qr(&self) -> Option<usize> {
        self.qr
    }

    /// Stops showing one.
    pub fn no_qr(&mut self) {
        self.qr = None;
    }

    /// The hint line, generated from the table — the same pieces in both doors.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(OF_THE_VIEW, texts)
    }

    /// The rows, both halves of each, for a region this wide.
    ///
    /// NOTHING HERE IS A LINE NUMBER. That is the whole fix: the old panel
    /// drew three of them against `end`, and with a short history `end` was
    /// above the top of the region.
    #[must_use]
    pub fn rows(&self, facts: &Facts<'_>, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let here = self.settle(&self.targets(facts, columns, texts));
        let left = crate::chrome::INSET;
        let right = detail_at(columns);
        let left_room = history_columns(columns).saturating_sub(left);
        let right_room = columns.saturating_sub(right);

        let history = self.history_side(facts, left, left_room, texts);
        let detail = Self::detail_side(facts, right, right_room, texts);

        // THE TWO HALVES ARE ZIPPED INTO ONE LIST OF ROWS, and the longer one
        // decides how many there are. Drawing them as two independent things
        // is what the old panel did, and it is why one of them could end up on
        // a line the other did not know about.
        let lines = history.len().max(detail.len());
        (0..lines)
            .map(|n| {
                let mut fields = Vec::with_capacity(4);
                let mut target = Target::Nothing;
                if let Some((side, what)) = history.get(n) {
                    fields.extend(side.iter().cloned());
                    target = *what;
                }
                if let Some(side) = detail.get(n) {
                    fields.extend(side.0.iter().cloned());
                    // THE RIGHT HALF ONLY ACTS WHERE THE LEFT HAS NOTHING.
                    // Lit side by side, one row would carry two answers to
                    // `RET` — the rule the Bluetooth view states for its own
                    // two columns.
                    if matches!(target, Target::Nothing) {
                        target = side.1;
                    }
                }
                Row {
                    fields,
                    marked: false,
                    focused: n == here && !matches!(target, Target::Nothing),
                    foot: false,
                }
            })
            .collect()
    }

    /// The left column: a heading, then one line per entry.
    fn history_side(
        &self,
        facts: &Facts<'_>,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<(Vec<Field>, Target)> {
        let mut out = vec![(
            vec![piece(
                left,
                texts.t(ronoctua_i18n::key::CLIPBOARD_HISTORY),
                tokens::color::TEXT_GROUP,
            )],
            Target::Nothing,
        )];
        if facts.queue.how_many() == 0 {
            out.push((
                vec![piece(
                    left,
                    texts.t(ronoctua_i18n::key::CLIPBOARD_EMPTY),
                    tokens::color::TEXT_DIM,
                )],
                Target::Nothing,
            ));
        }
        Self::entries(&mut out, facts, left, room, self.cursor, texts);

        // AIR, THEN THE SYNC'S NAME — and only its name. The owner's own
        // sketch put `Sincronização` on the left and `ClipCascade conectado`
        // on the right, on the same line, and that is not decoration: the left
        // column is two fifths of the footer, and `https://clip.example.org`
        // does not fit in it. A label whose value is elided says less than no
        // label at all.
        out.push((Vec::new(), Target::Nothing));
        out.push((
            vec![piece(
                left,
                texts.t(ronoctua_i18n::key::CLIPBOARD_HEADING_SYNC),
                tokens::color::TEXT_GROUP,
            )],
            Target::Nothing,
        ));
        out
    }

    /// The history's entries, one line each.
    fn entries(
        out: &mut Vec<(Vec<Field>, Target)>,
        facts: &Facts<'_>,
        left: u16,
        room: u16,
        cursor: usize,
        texts: &ronoctua_i18n::Texts,
    ) {
        for (n, entry) in facts.queue.inputs().iter().enumerate() {
            let here = n + 1 == cursor;
            let colour = if here {
                tokens::color::TEXT_HERE_ACTS
            } else {
                tokens::color::TEXT_SECONDARY
            };
            let mut fields = Vec::with_capacity(2);
            // THE PIN'S MARK GETS ITS OWN CELL, with the text after it. A
            // glyph against the letters is the "glifo encostado na borda" the
            // owner has had to point out before.
            if entry.pinned {
                fields.push(piece(
                    left,
                    tokens::icon::PINNED.to_string(),
                    tokens::color::STATE_SUCCESS,
                ));
            }
            fields.push(piece(
                left + 2,
                layout::elide_end(&one_line(entry, texts), room.saturating_sub(2)),
                colour,
            ));
            out.push((fields, Target::Entry(n)));
        }
    }

    /// The right column: what the entry is, and how the sync is doing.
    fn detail_side(
        facts: &Facts<'_>,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<(Vec<Field>, Target)> {
        let mut out: Vec<(Vec<Field>, Target)> = Vec::new();
        let fact = |label: &str, value: String| {
            (
                crate::chrome::field_pair(
                    at,
                    room,
                    label,
                    &value,
                    tokens::color::TEXT_LABEL_FACT,
                    tokens::color::TEXT_PRIMARY,
                ),
                Target::Nothing,
            )
        };
        out.push((
            vec![piece(
                at,
                texts.t(ronoctua_i18n::key::CLIPBOARD_ITEM),
                tokens::color::TEXT_GROUP,
            )],
            Target::Nothing,
        ));
        match facts.queue.at(0) {
            None => out.push((
                vec![piece(
                    at,
                    texts.t(ronoctua_i18n::key::CLIPBOARD_EMPTY),
                    tokens::color::TEXT_DIM,
                )],
                Target::Nothing,
            )),
            Some(entry) => {
                out.push(fact(
                    &texts.t(ronoctua_i18n::key::CLIPBOARD_CONTENT),
                    kind_of(entry, texts),
                ));
                out.push(fact(
                    &texts.t(ronoctua_i18n::key::CLIPBOARD_TYPES),
                    entry.kinds.join(", "),
                ));
                out.push(fact(
                    &texts.t(ronoctua_i18n::key::CLIPBOARD_SIZE),
                    size_of(entry, texts),
                ));
                if entry.pinned {
                    out.push((
                        vec![piece(
                            at,
                            texts.t(ronoctua_i18n::key::CLIPBOARD_PINNED),
                            tokens::color::STATE_SUCCESS,
                        )],
                        Target::Nothing,
                    ));
                }
            }
        }

        // AIR, THEN THE SYNC'S ROWS — beside the name the left column drew.
        out.push((Vec::new(), Target::Nothing));
        out.extend(Self::sync_rows(facts.sync, at, room, texts));
        out
    }

    /// What ClipCascade is doing, in the words for it.
    ///
    /// THE OLD LINE SAID `ClipCascade: not integrated`, IN THE WARNING TONE.
    /// It was honest and it was a build-time fact spending a STATE token — the
    /// same orange the network view uses for "there is no way out". Now the
    /// rows say what is true of this machine, and the tone follows the state.
    fn sync_rows(
        sync: &ronoctua_clipcascade::Snapshot,
        at: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<(Vec<Field>, Target)> {
        let pair = |label: &str, value: &str, colour: tokens::Color, target: Target| {
            (
                crate::chrome::field_pair(
                    at,
                    room,
                    &texts.t(label),
                    value,
                    tokens::color::TEXT_LABEL_ACTS,
                    colour,
                ),
                target,
            )
        };
        if !sync.configured {
            // NOT SET UP is not the same as OFF, and neither is an error.
            return vec![
                pair(
                    ronoctua_i18n::key::CLIPBOARD_SYNC_SERVER,
                    &texts.t(ronoctua_i18n::key::CLIPBOARD_SYNC_UNSET),
                    tokens::color::TEXT_DIM,
                    Target::Server,
                ),
                pair(
                    ronoctua_i18n::key::CLIPBOARD_SYNC_ACCOUNT,
                    &texts.t(ronoctua_i18n::key::CLIPBOARD_SYNC_UNSET),
                    tokens::color::TEXT_DIM,
                    Target::Account,
                ),
            ];
        }
        let (said, colour) = if sync.syncing() {
            (
                ronoctua_i18n::key::CLIPBOARD_SYNC_ON,
                tokens::color::STATE_SUCCESS,
            )
        } else if sync.enabled {
            // ENABLED AND NOT CONNECTED. The sentence underneath says why, in
            // the server's own words when it gave any.
            (
                ronoctua_i18n::key::CLIPBOARD_SYNC_DOWN,
                tokens::color::STATE_WARNING,
            )
        } else {
            (
                ronoctua_i18n::key::CLIPBOARD_SYNC_OFF,
                tokens::color::TEXT_DIM,
            )
        };
        let mut out = vec![
            pair(
                ronoctua_i18n::key::CLIPBOARD_SYNC_STATE,
                &texts.t(said),
                colour,
                Target::Sync(!sync.enabled),
            ),
            pair(
                ronoctua_i18n::key::CLIPBOARD_SYNC_SERVER,
                &sync.server,
                tokens::color::TEXT_PRIMARY,
                Target::Server,
            ),
            pair(
                ronoctua_i18n::key::CLIPBOARD_SYNC_ACCOUNT,
                &sync.username,
                tokens::color::TEXT_PRIMARY,
                Target::Account,
            ),
        ];
        if let Some(trouble) = sync.trouble.as_deref() {
            // THE SERVER'S OWN WORDS. "Sync is down" sends a person nowhere;
            // "that password does not open this" and "could not reach the
            // host" send them to two different places.
            out.push((
                vec![piece(
                    at,
                    layout::elide_end(trouble, room),
                    tokens::color::STATE_ERROR,
                )],
                Target::Nothing,
            ));
        }
        // WHAT THIS MACHINE DECIDES, which ClipCascade has no opinion about.
        // Every one of these was a field of `Preferences` with no row anywhere:
        // the ring the modeline reads, the two switches, the size and the two
        // numbers the key is derived from. A preference a person cannot reach
        // is a preference that has one value for ever.
        let word = |on: bool| {
            texts.t(if on {
                ronoctua_i18n::key::CLIPBOARD_SETTING_ON
            } else {
                ronoctua_i18n::key::CLIPBOARD_SETTING_OFF
            })
        };
        out.push(pair(
            ronoctua_i18n::key::CLIPBOARD_SYNC_IMAGES,
            &word(sync.images),
            tokens::color::TEXT_ANSWER,
            Target::Images,
        ));
        out.push(pair(
            ronoctua_i18n::key::CLIPBOARD_SYNC_NOTIFY,
            &word(sync.notify),
            tokens::color::TEXT_ANSWER,
            Target::Notify,
        ));
        // ZERO IS NOT A SIZE, it is "whatever the server carries" — and the
        // server's own number is what the row shows then, so the line answers
        // "how big may a copy be" either way.
        let cap = if sync.cap > 0 {
            crate::network::readable_bytes(sync.cap, texts.decimal_mark())
        } else if sync.most > 0 {
            texts.t_with(
                ronoctua_i18n::key::CLIPBOARD_SYNC_CAP_SERVER,
                &[(
                    "size",
                    &crate::network::readable_bytes(sync.most, texts.decimal_mark()),
                )],
            )
        } else {
            texts.t(ronoctua_i18n::key::CLIPBOARD_SYNC_CAP_UNSET)
        };
        out.push(pair(
            ronoctua_i18n::key::CLIPBOARD_SYNC_CAP,
            &cap,
            tokens::color::TEXT_ANSWER,
            Target::Cap,
        ));
        // THE SALT AND THE ROUNDS ARE EVERY DEVICE'S, and they are drawn
        // because a person joining an installation that changed either has no
        // other way to be told — the upstream asks for both at its own login,
        // and a machine given the wrong one syncs and reads noise.
        let salt = if sync.salt.is_empty() {
            texts.t(ronoctua_i18n::key::CLIPBOARD_SYNC_SALT_NONE)
        } else {
            sync.salt.clone()
        };
        out.push(pair(
            ronoctua_i18n::key::CLIPBOARD_SYNC_SALT,
            &salt,
            tokens::color::TEXT_ANSWER,
            Target::Salt,
        ));
        out.push(pair(
            ronoctua_i18n::key::CLIPBOARD_SYNC_ROUNDS,
            &sync.rounds.to_string(),
            tokens::color::TEXT_ANSWER,
            Target::Rounds,
        ));
        // OURS, AND THE ONLY ONE HERE THAT IS ABOUT THE SCREEN — the shape the
        // Syncthing view's mark row already has, in its words: everything
        // above belongs to ClipCascade and is kept by it; this is a decision
        // it has no opinion about.
        out.push(pair(
            ronoctua_i18n::key::CLIPBOARD_MARK,
            &texts.t(mark_key(sync.mark)),
            tokens::color::TEXT_ANSWER,
            Target::Mark,
        ));
        out.push(pair(
            ronoctua_i18n::key::CLIPBOARD_SYNC_FORGET,
            "",
            tokens::color::TEXT_PRIMARY,
            Target::Forget,
        ));
        out
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
        let rows = self.rows(facts, columns, texts);
        self.cursor = self.settle(&self.targets(facts, columns, texts));
        let target = self.target_at(facts, columns, texts);
        match press {
            Press::Up | Press::Down => {
                let step: isize = if press == Press::Down { 1 } else { -1 };
                let mut i = self.cursor;
                while let Some(next) = i.checked_add_signed(step) {
                    if next >= rows.len() {
                        break;
                    }
                    i = next;
                    // A HEADING AND A FACT ARE NOT STOPS. The cursor lands on
                    // rows that answer a key, which is what `focused` already
                    // draws — walking through eight rows where nothing lights
                    // up is what the owner read on the locale view.
                    if rows[i].focused || self.acts_at(facts, columns, texts, i) {
                        self.cursor = i;
                        break;
                    }
                }
                Errand::Nothing
            }
            // `Esc` PUTS THE QR AWAY BEFORE IT CLOSES THE VIEW. A code on
            // screen is a thing that is open, and the key that dismisses
            // things has to dismiss it first.
            Press::Escape if self.qr.is_some() => {
                self.qr = None;
                Errand::Nothing
            }
            Press::Escape => Errand::Close,
            Press::Clear => Errand::Clear,
            Press::Sync => match target {
                Target::Sync(on) => Errand::Sync(on),
                _ => Errand::AskServer,
            },
            // `RET` TURNS A RING FORWARD, because `RET` is what a CLICK
            // becomes: a row reachable only by a shifted arrow is a row the
            // mouse cannot use. The locale view's rule, in its words.
            Press::More | Press::Less => Self::turn(facts, target, press == Press::More),
            Press::Enter => match target {
                Target::Entry(n) => Errand::Copy(n),
                Target::Sync(on) => Errand::Sync(on),
                Target::Server => Errand::AskServer,
                Target::Account => Errand::AskAccount,
                Target::Forget => Errand::LogOut,
                Target::Salt => Errand::AskSalt,
                Target::Rounds => Errand::AskRounds,
                Target::Cap => Errand::AskCap,
                Target::Images | Target::Notify | Target::Mark => Self::turn(facts, target, true),
                Target::Nothing => Errand::Nothing,
            },
            Press::Remove | Press::Pin | Press::Qr => {
                let Target::Entry(n) = target else {
                    return Errand::Nothing;
                };
                match press {
                    Press::Remove => Errand::Remove(n),
                    Press::Pin => Errand::Pin(n),
                    _ => {
                        // `q` IS A TOGGLE. Pressing it on the code that is
                        // already showing puts it away, because otherwise the
                        // only way out is `Esc`, which also closes the view.
                        self.qr = if self.qr == Some(n) { None } else { Some(n) };
                        Errand::Qr(n)
                    }
                }
            }
        }
    }

    /// Turns one settings row's value, forward or back.
    ///
    /// THE WHOLE RECORD GOES BACK, not the field: `Settings` is what the
    /// boundary writes in one go, so a door cannot leave the file holding half
    /// of two changes.
    fn turn(facts: &Facts<'_>, target: Target, forward: bool) -> Errand {
        use ronoctua_clipcascade::prefs::{Mark, Settings};
        let sync = facts.sync;
        let mut next = Settings {
            salt: sync.salt.clone(),
            rounds: sync.rounds,
            images: sync.images,
            notify: sync.notify,
            cap: sync.cap,
            mark: sync.mark,
        };
        match target {
            Target::Images => next.images = !sync.images,
            Target::Notify => next.notify = !sync.notify,
            Target::Mark => {
                let all = Mark::ALL;
                let at = all.iter().position(|m| *m == sync.mark).unwrap_or(0);
                // BACKWARDS IS `len - 1` FORWARD, which is what keeps the
                // arithmetic in `usize` — `at - 1` underflows at zero.
                let step = if forward { 1 } else { all.len() - 1 };
                next.mark = all[(at + step) % all.len()];
            }
            _ => return Errand::Nothing,
        }
        Errand::Keep(Box::new(next))
    }

    /// What the cursor's row would do.
    #[must_use]
    pub fn target_at(
        &self,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Target {
        let targets = self.targets(facts, columns, texts);
        targets
            .get(self.settle(&targets))
            .copied()
            .unwrap_or(Target::Nothing)
    }

    /// Does row `n` answer a key?
    fn acts_at(
        &self,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
        n: usize,
    ) -> bool {
        !matches!(
            self.targets(facts, columns, texts).get(n),
            None | Some(Target::Nothing)
        )
    }

    /// Every row's target, in the order they are drawn.
    fn targets(
        &self,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Target> {
        let left = crate::chrome::INSET;
        let right = detail_at(columns);
        let left_room = history_columns(columns).saturating_sub(left);
        let right_room = columns.saturating_sub(right);
        let history = self.history_side(facts, left, left_room, texts);
        let detail = Self::detail_side(facts, right, right_room, texts);
        let lines = history.len().max(detail.len());
        (0..lines)
            .map(|n| {
                let from_left = history.get(n).map_or(Target::Nothing, |h| h.1);
                if matches!(from_left, Target::Nothing) {
                    detail.get(n).map_or(Target::Nothing, |d| d.1)
                } else {
                    from_left
                }
            })
            .collect()
    }

    /// Where a click fell.
    ///
    /// ONE FUNCTION FOR THE KEY AND FOR THE MOUSE — the `open_here` rule the
    /// notifications view states. A click that walked its own way to a target
    /// is a click that stops agreeing with `RET` the day a row moves.
    pub fn clicked(&mut self, row: usize) {
        self.cursor = row;
    }
}

/// One entry, in one line.
fn one_line(entry: &Entry, texts: &ronoctua_i18n::Texts) -> String {
    match entry.text() {
        Some(text) => text
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_owned(),
        None => kind_of(entry, texts),
    }
}

/// What an entry IS, in a word.
fn kind_of(entry: &Entry, texts: &ronoctua_i18n::Texts) -> String {
    match &entry.content {
        ronoctua_clipboard::Content::Text(_) => texts.t(ronoctua_i18n::key::CLIPBOARD_CONTENT_TEXT),
        ronoctua_clipboard::Content::Image(_) => texts.t(ronoctua_i18n::key::CLIPBOARD_IMAGE),
        ronoctua_clipboard::Content::Unreadable(what) => texts.t_with(
            ronoctua_i18n::key::CLIPBOARD_UNREADABLE,
            &[("kind", &what.kind)],
        ),
    }
}

/// How big it is, in the person's own separator.
fn size_of(entry: &Entry, texts: &ronoctua_i18n::Texts) -> String {
    let bytes = match &entry.content {
        ronoctua_clipboard::Content::Text(t) => t.len(),
        ronoctua_clipboard::Content::Image(b) => b.len(),
        ronoctua_clipboard::Content::Unreadable(u) => u.bytes,
    };
    // THE ONE FORMATTER, and it is the network view's: a second would
    // have a second answer about the decimal separator, which is the
    // LANGUAGE's and not the formatter's.
    crate::network::readable_bytes(bytes as u64, texts.decimal_mark())
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
        "clipboard-down" => Press::Down,
        "clipboard-up" => Press::Up,
        "clipboard-copy-of-new" => Press::Enter,
        "clipboard-remove" => Press::Remove,
        "clipboard-pin" => Press::Pin,
        "clipboard-qrcode" => Press::Qr,
        "clipboard-clear" => Press::Clear,
        "clipboard-sync" => Press::Sync,
        "clipboard-more" => Press::More,
        "clipboard-less" => Press::Less,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
