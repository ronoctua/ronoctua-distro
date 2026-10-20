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
//!
//! # What is typed is asked by the view, in either door
//!
//! A login is four answers — the server, the account, the password, the key
//! — and a text setting is one. The VIEW asks each [`Question`] and keeps the
//! answers so far; the door only opens its own line (the footer's minibuffer,
//! the window's header) and hands what was typed back to
//! [`View::answered`]. So the two doors ask the same questions in the same
//! order and send the same order, and neither holds half a login.

use ronoctua_atuin::options::{self, Scope, Setting};
use ronoctua_options::Kind;
use ronoctua_tokens as tokens;

use crate::fields::{Field, Hit, Ink, Row};

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

/// Something the person types, and whether it is hidden.
///
/// The id is the line's: its question is `ask-<id>`, and the answer comes
/// back under it — to the footer's command of the same name, and to the
/// window's `Content::answered` — and from either to [`View::answered`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question {
    /// The line's id.
    pub id: &'static str,
    /// Whether what is typed is masked — on the screen and in the log.
    pub secret: bool,
}

impl Question {
    /// Where the sync server is.
    pub const SERVER: Self = Self {
        id: "atuin-server",
        secret: false,
    };
    /// Who to be on it.
    pub const ACCOUNT: Self = Self {
        id: "atuin-account",
        secret: false,
    };
    /// That account's password.
    ///
    /// An address and an account are typed to be READ BACK and corrected; a
    /// password is not.
    pub const PASSWORD: Self = Self {
        id: "atuin-password",
        secret: true,
    };
    /// The key from the first machine — empty on the first one.
    ///
    /// HIDDEN LIKE THE PASSWORD: it decrypts the whole history. In the clear
    /// it went to the journal with every other answer the footer logs, and it
    /// is pasted far more than it is typed, which a hidden line takes.
    pub const KEY: Self = Self {
        id: "atuin-key",
        secret: true,
    };
    /// A text, list or amount setting's new value.
    pub const SETTING: Self = Self {
        id: "atuin-setting",
        secret: false,
    };

    /// Every question this view asks — what the footer's command table
    /// answers by.
    pub const ALL: [Self; 5] = [
        Self::SERVER,
        Self::ACCOUNT,
        Self::PASSWORD,
        Self::KEY,
        Self::SETTING,
    ];

    /// The question with this id, when it is one of this view's.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.id == id)
    }
}

/// A secret on its way into the order that carries it.
///
/// `Debug` IS WRITTEN BY HAND: the view is `Debug`, and what a panic or a
/// `?view` prints must not be the password.
#[derive(Clone, PartialEq, Eq)]
struct Hidden(String);

impl std::fmt::Debug for Hidden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Hidden(•)")
    }
}

/// Which question is on a line, and what a login's earlier answers were.
///
/// THE ANSWERS SO FAR LIVE HERE, and not in a door: that is what lets the
/// footer and the window ask the same four questions and send the same order.
/// The password waits for the key and nothing else, and goes with the
/// question when it is let go.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Typing {
    /// The server's address.
    Server,
    /// The account on it.
    Account { address: String },
    /// That account's password.
    Password { address: String, username: String },
    /// The first machine's key — the last answer before the order.
    Key {
        address: String,
        username: String,
        password: Hidden,
    },
    /// This setting's value.
    Setting(&'static str),
}

impl Typing {
    /// The question this step is asked by.
    const fn question(&self) -> Question {
        match self {
            Self::Server => Question::SERVER,
            Self::Account { .. } => Question::ACCOUNT,
            Self::Password { .. } => Question::PASSWORD,
            Self::Key { .. } => Question::KEY,
            Self::Setting(_) => Question::SETTING,
        }
    }
}

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
        Mark::Never => crate::key::ATUIN_MARK_NEVER,
        Mark::WhenOff => crate::key::ATUIN_MARK_WHEN_OFF,
        Mark::WhenOn => crate::key::ATUIN_MARK_WHEN_ON,
    }
}

/// What the view wants done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing; redraw.
    Nothing,
    /// Send this to the thread.
    Order(ronoctua_atuin::Order),
    /// Open this question on the door's own line.
    Ask(Question),
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
    /// What the cursor is ON — the row it goes back to when the rows move.
    ///
    /// THE ROW'S IDENTITY AND NOT ONLY ITS INDEX, the rule for a list whose
    /// rows move under the cursor: the key's question grows a sentence ABOVE
    /// its row. By index the cursor was left on that sentence, `settle`
    /// carried it to the first row of the page that acts — the modeline's
    /// ring — and the second `RET`, meant as "yes, show it", turned the ring.
    on: Option<Target>,
    /// Is the drawer of rarely-touched settings open?
    ///
    /// IN THE VIEW AND NOT ON DISK — a drawer somebody opened once is not a
    /// preference, and closing the view is what forgets it.
    drawer: bool,
    /// Is the question about showing the key on screen?
    asking: bool,
    /// The key, once it has been asked for and answered.
    key: Option<String>,
    /// What a line is open for, if anything — see [`Typing`].
    typing: Option<Typing>,
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

    /// Where the cursor is: on the row it was put on, wherever that row went;
    /// else on a row that answers a key, the nearest below and then above.
    ///
    /// Called by everything that reads the cursor, so there is no order of
    /// operations to get wrong: a view whose door forgot to settle it would
    /// open on a heading, which is the defect this is named after.
    ///
    /// THE NEAREST, AND NOT THE PAGE'S FIRST: from a sentence the first row
    /// that acts is the modeline's ring, at the top, and a `RET` meant for
    /// the row the person was looking at turned it.
    fn settle(&self, built: &[(Vec<Field>, Target)]) -> usize {
        if let Some(on) = &self.on
            && let Some(at) = built.iter().position(|(_, t)| t == on)
        {
            return at;
        }
        let acts = |at: &usize| {
            built
                .get(*at)
                .is_some_and(|(_, t)| !matches!(t, Target::Nothing))
        };
        if acts(&self.cursor) {
            return self.cursor;
        }
        (self.cursor..built.len())
            .find(acts)
            .or_else(|| (0..self.cursor.min(built.len())).rev().find(acts))
            .unwrap_or(self.cursor)
    }

    /// Puts the cursor on row `at` of `built`, and remembers what that row IS.
    fn stand_on(&mut self, built: &[(Vec<Field>, Target)], at: usize) {
        self.cursor = at;
        self.on = built
            .get(at)
            .map(|(_, t)| t.clone())
            .filter(|t| !matches!(t, Target::Nothing));
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

    /// The question a line is open for — read by the doors' own tests.
    #[cfg(test)]
    pub(crate) fn question(&self) -> Option<Question> {
        self.typing.as_ref().map(Typing::question)
    }

    /// What was typed on the line this view asked for, under the line's id.
    ///
    /// ONE CALL FOR BOTH DOORS: the footer's minibuffer answers through the
    /// command of the same id, the window through `Content::answered`, and
    /// each does what comes back — the login's next question, or the order.
    ///
    /// AN ANSWER NOBODY IS WAITING FOR IS NOTHING. A question let go and a
    /// line answered later under another id must not be taken for the one
    /// that is gone — that is a path typed into a password.
    #[must_use]
    pub fn answered(&mut self, id: &str, text: &str) -> Errand {
        let Some(typing) = self.typing.take() else {
            return Errand::Nothing;
        };
        if typing.question().id != id {
            return Errand::Nothing;
        }
        match typing {
            // AN EMPTY ADDRESS OR ACCOUNT ENDS THE LOGIN: it is `RET` on
            // nothing, which is how a person says never mind.
            Typing::Server => {
                let address = text.trim();
                if address.is_empty() {
                    return Errand::Nothing;
                }
                self.typing = Some(Typing::Account {
                    address: address.to_owned(),
                });
                Errand::Ask(Question::ACCOUNT)
            }
            Typing::Account { address } => {
                let username = text.trim();
                if username.is_empty() {
                    return Errand::Nothing;
                }
                self.typing = Some(Typing::Password {
                    address,
                    username: username.to_owned(),
                });
                Errand::Ask(Question::PASSWORD)
            }
            // THE PASSWORD AS TYPED — a space is a password's own. It waits
            // here for the one question left, and goes into the order.
            Typing::Password { address, username } => {
                self.typing = Some(Typing::Key {
                    address,
                    username,
                    password: Hidden(text.to_owned()),
                });
                Errand::Ask(Question::KEY)
            }
            // THE KEY IS THE FOURTH, and it is the FIRST MACHINE'S. Empty is
            // the right answer on the machine that is registering; on the
            // second one, leaving it empty gives an account whose history will
            // not decrypt. The question says so. `cli::login` hands the
            // password to `atuin` on STDIN — `argv` is world-readable through
            // `/proc/*/cmdline`.
            Typing::Key {
                address,
                username,
                password,
            } => Errand::Order(ronoctua_atuin::Order::LogIn {
                address,
                username,
                password: password.0,
                key: text.trim().to_owned(),
            }),
            Typing::Setting(id) => {
                let Some(setting) = options::by_id(id) else {
                    return Errand::Nothing;
                };
                // NOTHING TYPED KEEPS WHAT IS THERE. Written, it was `key = `,
                // which is not TOML — and Atuin then falls back to its
                // defaults for EVERYTHING, silently.
                let value = setting.typed(text);
                if value.is_empty() {
                    return Errand::Nothing;
                }
                Errand::Order(ronoctua_atuin::Order::Set {
                    section: setting.within.map(ToOwned::to_owned),
                    key: setting.key.to_owned(),
                    value,
                })
            }
        }
    }

    /// The line was let go — `Esc`, `C-g`, a click on `ESC` — and the
    /// question goes with it, and with the question a password typed for it.
    pub fn let_go(&mut self) {
        self.typing = None;
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

    /// Which of [`View::rows`] a click takes: a row a key acts on — never a
    /// heading, a fact, a sentence or the air between them, where
    /// [`View::clicked`] takes nothing.
    ///
    /// AND SO WHAT A RESTING POINTER LIGHTS, in both doors: the region's
    /// `Row::takes` and the window's `Content::takes` read this, and the
    /// click reads the same targets — one answer for the light and for the
    /// click. The region had said yes for every row, and the window lit
    /// whatever row the pointer was on.
    #[must_use]
    pub fn takes(
        &self,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<bool> {
        self.build(facts, columns, texts)
            .iter()
            .map(|(_, target)| !matches!(target, Target::Nothing))
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
                vec![piece(at, texts.t(key), tokens::color::TEXT_HEADING)],
                Target::Nothing,
            ));
        };
        let fact = |label: &str, value: &str| {
            (
                crate::chrome::field_pair(at, room, label, value, crate::chrome::Kind::Fact, false),
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
                    crate::chrome::Kind::Setting,
                    false,
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
                    texts.t(crate::key::ATUIN_ABSENT),
                    tokens::color::TEXT_DIM,
                )],
                Target::Nothing,
            ));
            return out;
        }

        heading(&mut out, crate::key::ATUIN_HEADING_GLANCE);
        out.push(fact(
            // THE LABEL IS NOT THE VALUE. It read `atuin-syncing` on both
            // sides, so the row said "syncing   syncing" — a definition list
            // whose term and definition are one word says nothing.
            &texts.t(crate::key::ATUIN_STATE),
            &texts.t(if shot.crossing() {
                crate::key::ATUIN_SYNCING
            } else {
                crate::key::ATUIN_NOT_SYNCING
            }),
        ));
        out.push(fact(
            &texts.t(crate::key::ATUIN_ENTRIES),
            &shot.entries.to_string(),
        ));
        if shot.logged_in {
            // BOUND FIRST: `&texts.t(…)` inside the `if` is a temporary that
            // dies at the end of the expression, and the borrow outlives it.
            let never = texts.t(crate::key::ATUIN_NEVER);
            let last = if shot.last.is_empty() {
                never.as_str()
            } else {
                shot.last.as_str()
            };
            out.push(fact(&texts.t(crate::key::ATUIN_LAST_SYNC), last));
        }
        // OURS, AND THE ONLY ONE IN THIS VIEW — the shape the Syncthing view's
        // mark row has, in its words: everything else belongs to Atuin and is
        // kept by Atuin, and this is a decision it has no opinion about. It
        // had a ring, a default and a modeline reader, and no row anywhere:
        // a preference nobody can reach has one value for ever.
        out.push(door(
            &texts.t(crate::key::ATUIN_MARK),
            &texts.t(mark_key(shot.mark)),
            Target::Mark,
        ));

        heading(&mut out, crate::key::ATUIN_HEADING_ACCOUNT);
        if shot.logged_in {
            out.push(fact(&texts.t(crate::key::ATUIN_SERVER), &shot.address));
            out.push(fact(&texts.t(crate::key::ATUIN_ACCOUNT), &shot.username));
            out.push(door(&texts.t(crate::key::ATUIN_SYNC_NOW), "", Target::Sync));
            // THE KEY, BEHIND A QUESTION THAT OPENS ON NO. It is what
            // decrypts the history, and a second machine cannot sync without
            // it — so hiding it would send the person to a terminal, which is
            // the failure this shell exists to remove.
            //
            // THE ROW KEEPS ITS TARGET IN ALL THREE STATES — asked, asking,
            // shown — so the cursor stays on it while it changes; shown, its
            // `RET` puts the key away, as `Esc` does.
            match (self.asking, self.key.as_deref()) {
                (_, Some(key)) => {
                    out.push(door(&texts.t(crate::key::ATUIN_SHOW_KEY), key, Target::Key));
                }
                (true, None) => {
                    out.push((
                        vec![piece(
                            at,
                            texts.t(crate::key::ATUIN_KEY_ABOUT),
                            tokens::color::TEXT_DIM,
                        )],
                        Target::Nothing,
                    ));
                    out.push(door(&texts.t(crate::key::ATUIN_KEY_ASK), "", Target::Key));
                }
                (false, None) => {
                    out.push(door(&texts.t(crate::key::ATUIN_SHOW_KEY), "", Target::Key));
                }
            }
            out.push(door(
                &texts.t(crate::key::ATUIN_LOG_OUT),
                "",
                Target::LogOut,
            ));
        } else {
            out.push(fact(
                &texts.t(crate::key::ATUIN_ACCOUNT),
                &texts.t(crate::key::ATUIN_NOT_LOGGED_IN),
            ));
            out.push(door(&texts.t(crate::key::ATUIN_LOG_IN), "", Target::LogIn));
        }

        for (key, scope) in [
            (crate::key::ATUIN_HEADING_LEAVING, Scope::Leaving),
            (crate::key::ATUIN_HEADING_SEARCH, Scope::Search),
            (crate::key::ATUIN_HEADING_KEYS, Scope::Keys),
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
                    texts.t(crate::key::ATUIN_ADVANCED_ABOUT),
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
                    &texts.t(crate::key::ATUIN_FROM_FLAKE),
                    // FIXED BY THE FLAKE: drawn, and refusing (invariant 6).
                    crate::chrome::Kind::Disabled,
                    false,
                ),
                Target::Setting(setting.id),
            );
        }
        // NOT WRITTEN IS NOT OFF. Atuin's defaults differ per setting and are
        // not in the file, so a screen that showed one would state something
        // this system never wrote.
        let value = if !setting.written(&shot.settings) {
            texts.t(crate::key::ATUIN_NOT_SET)
        } else {
            match setting.kind {
                // A SWITCH SAYS ON OR OFF, not what the SERVICE is doing.
                // These two read `atuin-syncing`/`atuin-not-syncing`, which
                // are the glance's words: `update_check` drew "check for a new
                // version   staying on this machine".
                Kind::Switch => texts.t(if setting.is_on(&shot.settings) {
                    crate::key::ATUIN_ON
                } else {
                    crate::key::ATUIN_OFF
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
                crate::chrome::Kind::Setting,
                false,
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
                piece(at, glyph.to_string(), tokens::color::TEXT_HEADING),
                // THE GLYPH IS ONE CELL AND THE WORD STARTS AFTER IT, with a
                // space between: a mark against the letters is the "glifo
                // encostado na borda" the owner has had to point out before.
                piece(
                    at + 2,
                    ronoctua_render::layout::elide_end(
                        &texts.t(crate::key::ATUIN_HEADING_ADVANCED),
                        room.saturating_sub(2),
                    ),
                    tokens::color::TEXT_HEADING,
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

    /// Where a click fell, and whether the cursor was ALREADY there — the
    /// second click, which the door turns into `RET` (invariant 19).
    ///
    /// THE FIRST CLICK SELECTS. It used to act: the door pressed `RET` on
    /// every click, so a click on a ring turned it and a click on "log out"
    /// logged out. And a row no key acts on takes no click at all: a heading
    /// or a fact is not a place the cursor stops, and `settle` carried the
    /// click from there to the first row that acts.
    pub fn clicked(
        &mut self,
        row: usize,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        let built = self.build(facts, columns, texts);
        if built
            .get(row)
            .is_none_or(|(_, t)| matches!(t, Target::Nothing))
        {
            return false;
        }
        let again = self.settle(&built) == row;
        self.stand_on(&built, row);
        again
    }

    /// Acts on a key.
    ///
    /// A KEY THAT REACHES THE VIEW LETS ITS QUESTION GO: while a line is
    /// open every key is the line's, in both doors, so a key here means the
    /// line was answered or let go — and an answer arriving after that must
    /// not be taken for the question.
    #[must_use]
    pub fn key(
        &mut self,
        press: Press,
        facts: &Facts<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.typing = None;
        let built = self.build(facts, columns, texts);
        let here = self.settle(&built);
        self.stand_on(&built, here);
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
                        self.stand_on(&built, i);
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
                Target::LogIn => {
                    self.typing = Some(Typing::Server);
                    Errand::Ask(Question::SERVER)
                }
                Target::LogOut => {
                    self.key = None;
                    Errand::Order(ronoctua_atuin::Order::LogOut)
                }
                Target::Key => {
                    // THE QUESTION OPENS ON NO: the first `RET` asks, and only
                    // the second shows. It is the shape the Bluetooth pairing
                    // and the format buffer already have. A third puts it
                    // away.
                    if self.key.is_some() {
                        self.key = None;
                        self.asking = false;
                        Errand::Nothing
                    } else if self.asking {
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
            // TYPED, NOT TURNED — on the line of whichever door this is, and
            // the answer comes back to `answered`.
            None => {
                self.typing = Some(Typing::Setting(id));
                Errand::Ask(Question::SETTING)
            }
        }
    }
}

/// Which press a key is, by its whole notation — the one reader both doors
/// call.
///
/// BY THE WHOLE NOTATION, modifiers included: the window read a key by its
/// TEXT, which an arrow has none of, so `S-<right>` and `S-<left>` — the
/// value keys — did nothing there, from the keyboard or from a click on the
/// hint. And a chord that is not in the table (`C-s`) is not taken for the
/// letter under it. Caps Lock is undone by both readers already
/// (`door::keys::without_caps_lock`).
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    // ESCAPE, WHATEVER IS HELD: every view of this shape leaves one level
    // on it.
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    OF_THE_VIEW
        .iter()
        .find(|(taught, _)| *taught == notation)
        .and_then(|(_, id)| press_of(id))
}

/// One field.
fn piece(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        at,
        text,
        color,
        hit: Hit::Nothing,
        explain: None,
        ink: Ink::Grid,
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

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "history",
    commands: &[
        // THE SHELL'S HISTORY — the first screen this distro has ever had for
        // it. `atuin` was installed and configured by a rebuild, and choosing
        // between two search modes is not a decision the size of one.
        ronoctua_cmd::Command {
            id: "atuin-open",
            label: crate::key::CMD_ATUIN_OPEN,
        },
        ronoctua_cmd::Command {
            id: "atuin-window",
            label: crate::key::CMD_ATUIN_WINDOW,
        },
        ronoctua_cmd::Command {
            id: "atuin-sync",
            label: crate::key::CMD_ATUIN_SYNC,
        },
        ronoctua_cmd::Command {
            id: "atuin-down",
            label: crate::key::CMD_ATUIN_DOWN,
        },
        ronoctua_cmd::Command {
            id: "atuin-up",
            label: crate::key::CMD_ATUIN_UP,
        },
        ronoctua_cmd::Command {
            id: "atuin-enter",
            label: crate::key::CMD_ATUIN_ENTER,
        },
        ronoctua_cmd::Command {
            id: "atuin-more",
            label: crate::key::CMD_ATUIN_MORE,
        },
        ronoctua_cmd::Command {
            id: "atuin-less",
            label: crate::key::CMD_ATUIN_LESS,
        },
        ronoctua_cmd::Command {
            id: "atuin-back",
            label: crate::key::CMD_ATUIN_BACK,
        },
    ],
    bindings: &[
        // THE SHELL'S HISTORY. `s h` was the one letter of the `s` group still
        // free, and history is the machine doing something to ITSELF — which is
        // the sentence that defines the group.
        ("s h h", "atuin-open", None),
        ("s h w", "atuin-window", None),
        ("s h s", "atuin-sync", None),
    ],
    groups: &[("s h", crate::key::GROUP_ATUIN)],
    reached: keys_reached,
    ..crate::bindings::Contribution::NONE
};

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`).
fn keys_reached() -> Vec<&'static str> {
    crate::history::OF_THE_VIEW
        .iter()
        .map(|(_, id)| *id)
        .collect()
}

#[cfg(test)]
mod tests;
