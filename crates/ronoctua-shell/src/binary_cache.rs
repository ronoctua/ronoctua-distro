//! THE PERSON'S BINARY CACHE — a page of the update's view, in both doors.
//!
//! # Whose, and where
//!
//! The person's own Cachix cache (`ronoctua_cachix`): the update pushes to it
//! after every update that switched the machine, and the machine downloads
//! from it before `cache.nixos.org`. Its settings are the update's business,
//! so they live on the update's view: `c` turns the view to this page and
//! back, in the footer's region (`LEADER s u u`, or straight here with
//! `LEADER s u c`) and in the window (`LEADER s u w`), with the same rows,
//! the same keys and the same questions — typed on the minibuffer in the
//! footer and on the window's own line there.
//!
//! # The rows
//!
//! The cache's name and its public key (typed: the name, then its key), who
//! may read it (public or private, `RET` turns it), the push token (typed,
//! a secret: never shown, kept `0600` in the sealed part), root's netrc for a
//! private cache (`RET` writes it again through the sudo bar), what this
//! machine uses now, and the last push and what it said; then the two
//! gestures that do not come back — forget the token, stop using the cache —
//! each asked with the "no" first.
//!
//! # Root's netrc goes through the sudo bar
//!
//! A private cache is READ by the machine's Nix daemon through a netrc only
//! root writes (`ronoctua_cachix::netrc`). Writing it is a root gesture of
//! the person's, asked in the footer's sudo bar like every other
//! (`crate::sudo_bar`, [`door`]): the footer's page runs it through the
//! footer's gate, and the window through its own — the deed goes down the
//! footer's socket — and reads what came of it back ([`Verdict`]), saying it
//! on its own line. No door asks root's password on its own line. The token
//! never crosses an argument: the gesture stages a copy in the person's
//! runtime folder, which the door's helper takes and deletes.

use std::path::{Path, PathBuf};

use ronoctua_cachix::record::{Outcome, Record};
use ronoctua_cachix::settings::Named;
use ronoctua_consent::keys::{Dir, Secret};
use ronoctua_i18n::key;
use ronoctua_person::options::{BinaryCache, Visibility, is_cache_name, is_key_of};
use ronoctua_tokens as tokens;

/// The command that opens the update's view on this page (`LEADER s u c`).
pub const OPEN: &str = "binary-cache-open";
/// The question of the cache's name.
pub const NAME: &str = "binary-cache-name";
/// The question of the cache's public key.
pub const KEY: &str = "binary-cache-key";
/// The question of the push token — a secret.
pub const TOKEN: &str = "binary-cache-token";
/// Root's netrc made to agree with the person's choice, through the sudo
/// bar — the footer's page, or `LEADER :` by its name.
pub const NETRC: &str = "binary-cache-netrc";

/// The runtime file [`door`] leaves what came of root's netrc in, for the
/// window that asked for it.
const VERDICT_FILE: &str = "binary-cache-door.json";

// ---------------------------------------------------------------------------
// WHAT THE PAGE READS
// ---------------------------------------------------------------------------

/// Where the page reads — the person's roots, and two places of the
/// machine's, under the footer camera's room while it photographs.
#[derive(Debug, Clone)]
pub struct Places {
    /// The person's copy of their options.
    pub mirror: Option<PathBuf>,
    /// The token's folder.
    pub keys: Option<Dir>,
    /// The record of pushes.
    pub record: Option<PathBuf>,
    /// What the machine was built with (`ronoctua_cachix::SYSTEM_FILE`).
    pub system: PathBuf,
    /// Root's netrc (`ronoctua_cachix::NETRC`).
    pub netrc: PathBuf,
    /// What came of the last netrc the footer was asked for.
    pub verdict: Option<PathBuf>,
}

impl Places {
    /// This person's and this machine's.
    #[must_use]
    pub fn here() -> Self {
        let machine = |p: &str| match crate::footer_photograph::room() {
            Some(room) => room.join(p.trim_start_matches('/')),
            None => PathBuf::from(p),
        };
        Self {
            mirror: ronoctua_cachix::settings::mirror(),
            keys: ronoctua_cachix::token::dir(),
            record: ronoctua_cachix::record::path(),
            system: machine(ronoctua_cachix::SYSTEM_FILE),
            netrc: machine(ronoctua_cachix::NETRC),
            verdict: ronoctua_xdg::runtime_file(VERDICT_FILE),
        }
    }

    /// When each file the page reads last moved — what a door compares to
    /// read again only when something did.
    #[must_use]
    pub fn stamp(&self) -> Vec<Option<std::time::SystemTime>> {
        let mtime = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        let token = self
            .keys
            .as_ref()
            .map(|d| d.path().join(ronoctua_cachix::token::FILE));
        [
            self.mirror.clone(),
            token,
            self.record.clone(),
            Some(self.system.clone()),
            Some(self.netrc.clone()),
            self.verdict.clone(),
        ]
        .iter()
        .map(|p| p.as_deref().and_then(mtime))
        .collect()
    }
}

/// Whether a push token is kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TokenState {
    /// None.
    #[default]
    None,
    /// One, as it must be.
    Kept,
    /// One that is not of the person's alone, or not a token: refused.
    Refused(ronoctua_cachix::Why),
}

/// What came of the last netrc a door asked for — [`door`] writes it, a
/// window that asked reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Verdict {
    /// When, in milliseconds since the epoch.
    pub at_ms: u64,
    /// What came of it.
    pub came: Came,
}

/// What came of root's netrc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "came", content = "why", rename_all = "kebab-case")]
pub enum Came {
    /// Written for the person's private cache.
    Written,
    /// Removed: no private cache, or no token.
    Removed,
    /// The door refused — the sudo bar cancelled, the password wrong, the
    /// footer not there (`Why::NoFooter`).
    Refused(ronoctua_security::Why),
    /// The token could not be staged.
    Token(ronoctua_cachix::Why),
}

/// Everything the page draws.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Readings {
    /// What the person's copy names.
    pub named: Named,
    /// The cache this machine was built with.
    pub applied: Option<BinaryCache>,
    /// The push token.
    pub token: TokenState,
    /// Whether root's netrc is there.
    pub netrc: bool,
    /// The last push.
    pub last: Option<Record>,
    /// What came of the last netrc.
    pub verdict: Option<Verdict>,
}

impl Readings {
    /// What `places` say now.
    #[must_use]
    pub fn read(places: &Places) -> Self {
        let token = match places.keys.as_ref().map(ronoctua_cachix::token::read_from) {
            Some(Ok(Some(_))) => TokenState::Kept,
            Some(Err(why)) => TokenState::Refused(why),
            Some(Ok(None)) | None => TokenState::None,
        };
        Self {
            named: places
                .mirror
                .as_deref()
                .map_or(Named::None, ronoctua_cachix::settings::read),
            applied: ronoctua_cachix::system::System::read_at(&places.system)
                .ok()
                .map(|s| s.cache),
            token,
            netrc: ronoctua_cachix::netrc::present(&places.netrc),
            last: places
                .record
                .as_deref()
                .map(ronoctua_cachix::record::read_at)
                .and_then(|p| p.last),
            verdict: places
                .verdict
                .as_ref()
                .and_then(|p| std::fs::read(p).ok())
                .and_then(|b| serde_json::from_slice(&b).ok()),
        }
    }

    /// Whether a token is kept.
    #[must_use]
    pub const fn token_kept(&self) -> bool {
        matches!(self.token, TokenState::Kept)
    }
}

// ---------------------------------------------------------------------------
// THE PAGE
// ---------------------------------------------------------------------------

/// One row of the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The cache's name — typed.
    Cache,
    /// Its public key — typed.
    Key,
    /// Public or private — turned.
    Visibility,
    /// The push token — typed, a secret.
    Token,
    /// Root's netrc, for a private cache — written again.
    Access,
    /// What this machine uses now.
    Machine,
    /// The last push and what it said.
    Last,
    /// Forget the token — asked, "no" first.
    ForgetToken,
    /// Stop using the cache — asked, "no" first.
    Remove,
}

impl Row {
    /// Every one, in the page's order.
    pub const ALL: [Self; 9] = [
        Self::Cache,
        Self::Key,
        Self::Visibility,
        Self::Token,
        Self::Access,
        Self::Machine,
        Self::Last,
        Self::ForgetToken,
        Self::Remove,
    ];

    /// Its label.
    const fn label(self) -> &'static str {
        match self {
            Self::Cache => key::BINARY_CACHE_ROW_CACHE,
            Self::Key => key::BINARY_CACHE_ROW_KEY,
            Self::Visibility => key::BINARY_CACHE_ROW_VISIBILITY,
            Self::Token => key::BINARY_CACHE_ROW_TOKEN,
            Self::Access => key::BINARY_CACHE_ROW_ACCESS,
            Self::Machine => key::BINARY_CACHE_ROW_MACHINE,
            Self::Last => key::BINARY_CACHE_ROW_LAST,
            Self::ForgetToken => key::BINARY_CACHE_ROW_FORGET_TOKEN,
            Self::Remove => key::BINARY_CACHE_ROW_REMOVE,
        }
    }

    /// What it is, as the second register says it.
    const fn about(self) -> &'static str {
        match self {
            Self::Cache => key::BINARY_CACHE_ABOUT_CACHE,
            Self::Key => key::BINARY_CACHE_ABOUT_KEY,
            Self::Visibility => key::BINARY_CACHE_ABOUT_VISIBILITY,
            Self::Token => key::BINARY_CACHE_ABOUT_TOKEN,
            Self::Access => key::BINARY_CACHE_ABOUT_ACCESS,
            Self::Machine => key::BINARY_CACHE_ABOUT_MACHINE,
            Self::Last => key::BINARY_CACHE_ABOUT_LAST,
            Self::ForgetToken => key::BINARY_CACHE_ABOUT_FORGET_TOKEN,
            Self::Remove => key::BINARY_CACHE_ABOUT_REMOVE,
        }
    }

    /// Whether a `RET` on it does something.
    const fn acts(self) -> bool {
        !matches!(self, Self::Machine | Self::Last)
    }
}

/// The rows `r` has: the visibility and the removal only with a cache, root's
/// netrc only for a private one, the forgetting only with a token. Pure.
#[must_use]
pub fn rows_of(r: &Readings) -> Vec<Row> {
    let cache = r.named.cache();
    Row::ALL
        .into_iter()
        .filter(|row| match row {
            Row::Visibility | Row::Remove => cache.is_some(),
            Row::Access => cache.is_some_and(BinaryCache::is_private),
            Row::ForgetToken => r.token != TokenState::None,
            _ => true,
        })
        .collect()
}

/// The question the page has open on a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Asking {
    /// The name.
    Name,
    /// The key of the cache of this name.
    Key {
        /// The name the key must be of.
        name: String,
    },
    /// The token.
    Token,
}

/// A gesture that does not come back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grave {
    /// Forget the token.
    ForgetToken,
    /// Stop using the cache.
    Remove,
}

/// The question a grave gesture asks — "no" first, and the cursor born on
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirm {
    /// What it asks about.
    pub what: Grave,
    /// Which answer the cursor is on.
    pub yes: bool,
}

/// A key the page reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The row above.
    Up,
    /// The row below.
    Down,
    /// Does what the row says.
    Enter,
    /// Back to the steps — or the question's "no".
    Escape,
}

/// What a change does to the person's choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Edit {
    /// The cache of this name and key.
    Named {
        /// Its name.
        name: String,
        /// Its key.
        key: String,
    },
    /// Who may read it.
    Visibility(Visibility),
    /// The push token.
    Token(Secret),
    /// The token forgotten.
    ForgetToken,
    /// No cache.
    Remove,
    /// Root's netrc made to agree, and nothing else changed.
    Netrc,
}

impl Edit {
    /// What the change is, in words for a photograph's record of what would
    /// have happened — never the token.
    #[must_use]
    pub fn word(&self) -> String {
        match self {
            Self::Named { name, .. } => format!("the cache {name}"),
            Self::Visibility(v) => format!("visibility {}", v.id()),
            Self::Token(_) => "a new token".to_owned(),
            Self::ForgetToken => "the token forgotten".to_owned(),
            Self::Remove => "no cache".to_owned(),
            Self::Netrc => "root's netrc".to_owned(),
        }
    }
}

/// What the page asks of the door that holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing.
    Nothing,
    /// The cursor moved: the page follows, the register says the row.
    Moved,
    /// Back to the steps.
    Close,
    /// A question on the door's line.
    Ask(crate::line::Line),
    /// A sentence, by its key, said on the door's line.
    Say(&'static str),
    /// A change ([`apply`]).
    Edit(Edit),
}

/// The page: its cursor, a question open on a line, a grave question open on
/// the page.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Page {
    cursor: usize,
    asking: Option<Asking>,
    confirm: Option<Confirm>,
}

/// A line on top of the page: the grave question and its two answers, or
/// what the row under the cursor is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLine {
    /// The text, wrapped.
    pub text: String,
    /// Its colour.
    pub color: tokens::Color,
    /// The answer a click on it gives, when it is one.
    pub answer: Option<bool>,
}

/// A line of the page under the top lines: the heading, or a row.
#[derive(Debug, Clone, PartialEq)]
pub struct PageLine {
    /// Placed and cut.
    pub fields: Vec<crate::files::Field>,
    /// The row a click on it takes, by its index.
    pub row: Option<usize>,
}

impl Page {
    /// The row under the cursor, by its index.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The grave question, while it is asked.
    #[must_use]
    pub const fn confirm(&self) -> Option<Confirm> {
        self.confirm
    }

    /// The cursor kept on a row `r` has.
    pub fn settle(&mut self, r: &Readings) {
        self.cursor = self.cursor.min(rows_of(r).len().saturating_sub(1));
    }

    /// One key — the same in both doors.
    pub fn press(&mut self, press: Press, r: &Readings) -> Errand {
        if let Some(c) = self.confirm.as_mut() {
            return match press {
                Press::Up | Press::Down => {
                    c.yes = press == Press::Down;
                    Errand::Moved
                }
                Press::Enter => {
                    let yes = c.yes;
                    self.answer(yes)
                }
                Press::Escape => self.answer(false),
            };
        }
        let len = rows_of(r).len();
        match press {
            Press::Down => {
                self.cursor = (self.cursor + 1).min(len.saturating_sub(1));
                Errand::Moved
            }
            Press::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Errand::Moved
            }
            Press::Enter => self.enter(r),
            Press::Escape => Errand::Close,
        }
    }

    /// A click on the row `n`: the first puts the cursor there, the second
    /// does what `RET` does (invariant 19).
    pub fn clicked(&mut self, n: usize, r: &Readings) -> Errand {
        if self.confirm.is_some() || n >= rows_of(r).len() {
            return Errand::Nothing;
        }
        if n == self.cursor {
            self.enter(r)
        } else {
            self.cursor = n;
            Errand::Moved
        }
    }

    /// A click on an answer of the grave question: the first puts the cursor
    /// on it, the second answers.
    pub fn answer_clicked(&mut self, yes: bool) -> Errand {
        match self.confirm.as_mut() {
            Some(c) if c.yes == yes => self.answer(yes),
            Some(c) => {
                c.yes = yes;
                Errand::Moved
            }
            None => Errand::Nothing,
        }
    }

    /// The grave question answered.
    fn answer(&mut self, yes: bool) -> Errand {
        let Some(c) = self.confirm.take() else {
            return Errand::Nothing;
        };
        if !yes {
            return Errand::Moved;
        }
        Errand::Edit(match c.what {
            Grave::ForgetToken => Edit::ForgetToken,
            Grave::Remove => Edit::Remove,
        })
    }

    /// `RET` on the row under the cursor.
    fn enter(&mut self, r: &Readings) -> Errand {
        let Some(row) = rows_of(r).get(self.cursor).copied() else {
            return Errand::Nothing;
        };
        // A COPY THAT DOES NOT READ IS NOT REWRITTEN: what names the cache is
        // refused with its reason, and the token stays the person's to set.
        let unreadable = match r.named {
            Named::Unreadable => Some(key::BINARY_CACHE_WHY_UNREADABLE),
            Named::Newer => Some(key::BINARY_CACHE_WHY_NEWER),
            _ => None,
        };
        let touches_the_copy = matches!(row, Row::Cache | Row::Key | Row::Visibility | Row::Remove);
        if let (Some(why), true) = (unreadable, touches_the_copy) {
            return Errand::Say(why);
        }
        let cache = r.named.cache();
        match row {
            Row::Key if cache.is_some() => {
                let name = cache.map(|c| c.name.clone()).unwrap_or_default();
                self.ask_key(name, cache.map(|c| c.public_key.as_str()))
            }
            Row::Cache | Row::Key => {
                self.asking = Some(Asking::Name);
                Errand::Ask(crate::line::Line::new(
                    NAME,
                    false,
                    cache.map(|c| c.name.as_str()),
                ))
            }
            Row::Visibility => match cache {
                Some(c) => Errand::Edit(Edit::Visibility(match c.visibility {
                    Visibility::Public => Visibility::Private,
                    Visibility::Private => Visibility::Public,
                })),
                None => Errand::Nothing,
            },
            Row::Token => {
                self.asking = Some(Asking::Token);
                Errand::Ask(crate::line::Line::new(TOKEN, true, None))
            }
            Row::Access => Errand::Edit(Edit::Netrc),
            Row::ForgetToken => {
                self.confirm = Some(Confirm {
                    what: Grave::ForgetToken,
                    yes: false,
                });
                Errand::Moved
            }
            Row::Remove => {
                self.confirm = Some(Confirm {
                    what: Grave::Remove,
                    yes: false,
                });
                Errand::Moved
            }
            Row::Machine | Row::Last => Errand::Nothing,
        }
    }

    /// The question `id` asks, as `RET` on its row would — what the command
    /// does when it is run by its name with nothing typed (`LEADER :`).
    pub fn ask_for(&mut self, id: &str, r: &Readings) -> Errand {
        let wanted = match id {
            NAME => Row::Cache,
            KEY => Row::Key,
            TOKEN => Row::Token,
            _ => return Errand::Nothing,
        };
        if let Some(n) = rows_of(r).iter().position(|row| *row == wanted) {
            self.cursor = n;
            self.confirm = None;
            return self.enter(r);
        }
        Errand::Nothing
    }

    /// The key's question, for the cache `name`.
    fn ask_key(&mut self, name: String, now: Option<&str>) -> Errand {
        let mut line = crate::line::Line::new(KEY, false, now);
        line.asks_with = vec![("name", name.clone())];
        self.asking = Some(Asking::Key { name });
        Errand::Ask(line)
    }

    /// What was typed on the line, answering the question `id` — only the
    /// page's own open question takes it.
    pub fn answered(&mut self, id: &str, text: &str, r: &Readings) -> Errand {
        let typed = text.trim();
        match (self.asking.take(), id) {
            (Some(Asking::Name), NAME) => {
                if !is_cache_name(typed) {
                    return Errand::Say(key::BINARY_CACHE_SAID_NOT_A_NAME);
                }
                // THE KEY IS THE NAME'S: the same cache keeps its key as
                // the line's start; another name starts empty.
                let kept = r
                    .named
                    .cache()
                    .filter(|c| c.name == typed)
                    .map(|c| c.public_key.clone());
                self.ask_key(typed.to_owned(), kept.as_deref())
            }
            (Some(Asking::Key { name }), KEY) => {
                if is_key_of(&name, typed) {
                    Errand::Edit(Edit::Named {
                        name,
                        key: typed.to_owned(),
                    })
                } else {
                    Errand::Say(key::BINARY_CACHE_SAID_NOT_THE_KEY)
                }
            }
            (Some(Asking::Token), TOKEN) => Errand::Edit(Edit::Token(Secret::new(typed))),
            (other, _) => {
                self.asking = other;
                Errand::Nothing
            }
        }
    }

    /// The line was let go: its question goes.
    pub fn unanswered(&mut self) {
        self.asking = None;
    }

    /// The lines on top of the page: the grave question and its answers
    /// ("no" first), or what the row under the cursor is. Notes are WRAPPED,
    /// never cut. Not a heading (the page's heading is in [`Self::lines`]),
    /// so it is not named like one.
    #[must_use]
    pub fn top_lines(&self, r: &Readings, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<TopLine> {
        let line = |text, color, answer| TopLine {
            text,
            color,
            answer,
        };
        let mut out = Vec::new();
        if let Some(c) = self.confirm {
            let (asked, yes_key) = match c.what {
                Grave::ForgetToken => (
                    key::BINARY_CACHE_CONFIRM_FORGET,
                    key::BINARY_CACHE_ANSWER_YES_FORGET,
                ),
                Grave::Remove => (
                    key::BINARY_CACHE_CONFIRM_REMOVE,
                    key::BINARY_CACHE_ANSWER_YES_REMOVE,
                ),
            };
            for said in crate::chrome::note_lines(&texts.t(asked), room) {
                out.push(line(said, tokens::color::TEXT_PRIMARY, None));
            }
            for (yes, answer) in [(false, key::BINARY_CACHE_ANSWER_NO), (true, yes_key)] {
                let color = if c.yes == yes {
                    tokens::color::TEXT_HERE_ACTS
                } else if yes {
                    tokens::color::STATE_ERROR
                } else {
                    crate::chrome::Kind::Setting.tones(false).0
                };
                out.push(line(
                    ronoctua_render::layout::elide_end(&texts.t(answer), room),
                    color,
                    Some(yes),
                ));
            }
            return out;
        }
        if let Some(row) = rows_of(r).get(self.cursor).copied() {
            // WHAT THE ROW IS, and — where its value only names a refusal —
            // WHY, whole: a value is a few words, a reason is a note, and a
            // note is wrapped, never cut.
            let mut said = texts.t(row.about());
            if let Some(why) = reason_of(row, r) {
                said.push(' ');
                said.push_str(&texts.t(why_key(why)));
            }
            for wrapped in crate::chrome::note_lines(&said, room) {
                out.push(line(wrapped, tokens::color::TEXT_SECONDARY, None));
            }
        }
        out
    }

    /// The page under the top lines: a heading, and a row a line — the label and
    /// the value, the cursor a colour.
    #[must_use]
    pub fn lines(
        &self,
        r: &Readings,
        left: u16,
        room: u16,
        zone: &ronoctua_files::when::Zone,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<PageLine> {
        // A LINE OF AIR before the heading, which is how a heading stands
        // out: hierarchy is colour and air, never size.
        let mut out = vec![PageLine {
            fields: crate::chrome::AIR,
            row: None,
        }];
        out.push(PageLine {
            fields: vec![crate::files::Field {
                at: left,
                text: ronoctua_render::layout::elide_end(&texts.t(key::BINARY_CACHE_HEADING), room),
                color: tokens::color::TEXT_HEADING,
                hit: crate::files::Hit::Nothing,
                explain: None,
                ink: crate::files::Ink::Grid,
            }],
            row: None,
        });
        for (n, row) in rows_of(r).into_iter().enumerate() {
            let focused = n == self.cursor && self.confirm.is_none();
            let (value, tint) = value_of(row, r, zone, texts);
            let kind = crate::chrome::Kind::of(row.acts());
            let fields = if matches!(row, Row::ForgetToken | Row::Remove) {
                // A GESTURE IS ITS LABEL, in the colour of a setting — and,
                // since it does not come back, of danger when not under the
                // cursor.
                let (label, _) = kind.tones(focused);
                vec![crate::files::Field {
                    at: left,
                    text: ronoctua_render::layout::elide_end(&texts.t(row.label()), room),
                    color: if focused {
                        label
                    } else {
                        tokens::color::STATE_ERROR
                    },
                    hit: crate::files::Hit::Nothing,
                    explain: None,
                    ink: crate::files::Ink::Grid,
                }]
            } else {
                crate::chrome::tinted(
                    crate::chrome::field_pair(
                        left,
                        room,
                        &texts.t(row.label()),
                        &value,
                        kind,
                        focused,
                    ),
                    tint,
                    focused,
                )
            };
            out.push(PageLine {
                fields,
                row: Some(n),
            });
        }
        out
    }
}

/// A row's value, and the colour of a state it says (a refusal, a token
/// that is not the person's alone), when it says one.
fn value_of(
    row: Row,
    r: &Readings,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> (String, Option<tokens::Color>) {
    let cache = r.named.cache();
    let none = || texts.t(key::BINARY_CACHE_NONE);
    match row {
        Row::Cache => match &r.named {
            Named::Cache(c) => (c.host(), None),
            Named::None => (none(), None),
            Named::Unreadable => (
                texts.t(key::BINARY_CACHE_COPY_UNREADABLE),
                Some(tokens::color::STATE_WARNING),
            ),
            Named::Newer => (
                texts.t(key::BINARY_CACHE_COPY_NEWER),
                Some(tokens::color::STATE_WARNING),
            ),
        },
        // ITS OWN "NONE": the word agrees with the key's gender in a language
        // whose "none" has one (pt-BR "nenhuma chave", "nenhum cache").
        Row::Key => (
            cache.map_or_else(
                || texts.t(key::BINARY_CACHE_KEY_NONE),
                |c| c.public_key.clone(),
            ),
            None,
        ),
        Row::Visibility => (
            texts.t(match cache.map(|c| c.visibility) {
                Some(Visibility::Private) => key::BINARY_CACHE_PRIVATE,
                _ => key::BINARY_CACHE_PUBLIC,
            }),
            None,
        ),
        Row::Token => match r.token {
            TokenState::None => (texts.t(key::BINARY_CACHE_TOKEN_NONE), None),
            TokenState::Kept => (texts.t(key::BINARY_CACHE_TOKEN_KEPT), None),
            TokenState::Refused(_) => (
                texts.t(key::BINARY_CACHE_TOKEN_REFUSED),
                Some(tokens::color::STATE_WARNING),
            ),
        },
        Row::Access => {
            if r.netrc {
                (texts.t(key::BINARY_CACHE_ACCESS_WRITTEN), None)
            } else {
                (
                    texts.t(key::BINARY_CACHE_ACCESS_MISSING),
                    Some(tokens::color::STATE_WARNING),
                )
            }
        }
        Row::Machine => (
            texts.t(match (&r.named, r.applied.as_ref()) {
                (Named::Cache(c), Some(a)) if c == a => key::BINARY_CACHE_MACHINE_IN_USE,
                (Named::Cache(_), _) => key::BINARY_CACHE_MACHINE_NEXT,
                (Named::None, Some(_)) => key::BINARY_CACHE_MACHINE_LEAVING,
                // A COPY THAT DOES NOT READ says nothing of what comes next:
                // what the machine uses is all that is known.
                (Named::Unreadable | Named::Newer, Some(_)) => key::BINARY_CACHE_MACHINE_IN_USE,
                (_, None) => key::BINARY_CACHE_MACHINE_NONE,
            }),
            None,
        ),
        Row::Last => last_of(r.last.as_ref(), zone, texts),
        Row::ForgetToken | Row::Remove => (String::new(), None),
    }
}

/// Why a row's value is a refusal — said whole in the top lines while the cursor
/// is on it.
fn reason_of(row: Row, r: &Readings) -> Option<ronoctua_cachix::Why> {
    match (row, &r.named, r.token, r.last.as_ref().map(|l| l.outcome)) {
        (Row::Cache, Named::Unreadable, _, _) => Some(ronoctua_cachix::Why::Unreadable),
        (Row::Cache, Named::Newer, _, _) => Some(ronoctua_cachix::Why::Newer),
        (Row::Token, _, TokenState::Refused(why), _) => Some(why),
        (Row::Last, _, _, Some(Outcome::Refused { why })) => Some(why),
        _ => None,
    }
}

/// The last push, in a few words — why it was refused is the top lines'.
fn last_of(
    last: Option<&Record>,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> (String, Option<tokens::Color>) {
    let Some(r) = last else {
        return (texts.t(key::BINARY_CACHE_LAST_NEVER), None);
    };
    let when = crate::chrome::when_text(r.at.unix_s(), zone, false, texts);
    match r.outcome {
        Outcome::Pushed { paths, bytes } | Outcome::Rehearsed { paths, bytes } => (
            texts.t_args(
                key::BINARY_CACHE_LAST_PUSHED,
                &[
                    ("when", ronoctua_i18n::Arg::Text(&when)),
                    (
                        "paths",
                        ronoctua_i18n::Arg::Number(i64::try_from(paths).unwrap_or(i64::MAX)),
                    ),
                    ("size", ronoctua_i18n::Arg::Text(&size_said(bytes, texts))),
                ],
            ),
            None,
        ),
        Outcome::Refused { why } => (
            texts.t_with(key::BINARY_CACHE_LAST_REFUSED, &[("when", when.as_str())]),
            Some(
                if matches!(
                    why,
                    ronoctua_cachix::Why::NothingNew | ronoctua_cachix::Why::NoCache
                ) {
                    tokens::color::TEXT_SECONDARY
                } else {
                    tokens::color::STATE_WARNING
                },
            ),
        ),
    }
}

/// A size, in the language's own words (`files-size-…`).
fn size_said(bytes: u64, texts: &ronoctua_i18n::Texts) -> String {
    let h = ronoctua_files::size::human(bytes);
    let [(a, x), (b, y)] = h.args();
    texts.t_with(h.unit.label(), &[(a, x.as_str()), (b, y.as_str())])
}

/// The sentence of why the cache refused something.
#[must_use]
pub const fn why_key(why: ronoctua_cachix::Why) -> &'static str {
    use ronoctua_cachix::Why;
    match why {
        Why::NoCache => key::BINARY_CACHE_WHY_NO_CACHE,
        Why::NoToken => key::BINARY_CACHE_WHY_NO_TOKEN,
        Why::TokenOpen => key::BINARY_CACHE_WHY_TOKEN_OPEN,
        Why::NotAToken => key::BINARY_CACHE_WHY_NOT_A_TOKEN,
        Why::NotLet => key::BINARY_CACHE_WHY_NOT_LET,
        Why::NothingNew => key::BINARY_CACHE_WHY_NOTHING_NEW,
        Why::NoTools => key::BINARY_CACHE_WHY_NO_TOOLS,
        Why::StoreFailed => key::BINARY_CACHE_WHY_STORE_FAILED,
        Why::CachixFailed => key::BINARY_CACHE_WHY_CACHIX_FAILED,
        Why::Unwritable => key::BINARY_CACHE_WHY_UNWRITABLE,
        Why::Newer => key::BINARY_CACHE_WHY_NEWER,
        Why::Unreadable => key::BINARY_CACHE_WHY_UNREADABLE,
    }
}

/// What a change did: the sentence the door says, and whether root's netrc
/// must be made to agree now (the sudo door, through the footer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// The key of the sentence, with its name argument when it has one.
    pub said: String,
    /// Whether root's netrc no longer agrees with the person's choice.
    pub netrc: bool,
}

/// Carries out a change: the person's copy of their options, their token,
/// their consent — never root's netrc, which the footer's door does after
/// ([`Applied::netrc`]). `r` is what the page read before it.
#[must_use]
pub fn apply(edit: &Edit, places: &Places, r: &Readings, texts: &ronoctua_i18n::Texts) -> Applied {
    let said = |k: &str| Applied {
        said: texts.t(k),
        netrc: false,
    };
    let refused = |why: ronoctua_cachix::Why| said(why_key(why));
    let Some(mirror) = places.mirror.as_deref() else {
        return refused(ronoctua_cachix::Why::Unwritable);
    };
    let private_now = r.named.cache().is_some_and(BinaryCache::is_private);
    match edit {
        Edit::Named {
            name,
            key: public_key,
        } => {
            let visibility = r.named.cache().map_or(Visibility::Public, |c| c.visibility);
            let changed = ronoctua_cachix::settings::change(mirror, |c| {
                *c = Some(BinaryCache {
                    name: name.clone(),
                    public_key: public_key.clone(),
                    visibility,
                });
            });
            if let Err(why) = changed {
                return refused(why);
            }
            // NAMING THE CACHE IS THE CONSENT for every host a push reaches
            // (`ronoctua_cachix::hosts_of`), recorded like any other.
            let hosts = ronoctua_cachix::hosts_of(name);
            let hosts: Vec<&str> = hosts.iter().map(String::as_str).collect();
            if ronoctua_consent::let_hosts(&hosts, true).is_err() {
                return said(key::BINARY_CACHE_SAID_NOT_LET);
            }
            Applied {
                said: texts.t_with(key::BINARY_CACHE_SAID_NAMED, &[("name", name.as_str())]),
                netrc: visibility == Visibility::Private && r.token_kept(),
            }
        }
        Edit::Visibility(v) => {
            if let Err(why) = ronoctua_cachix::settings::change(mirror, |c| {
                if let Some(c) = c.as_mut() {
                    c.visibility = *v;
                }
            }) {
                return refused(why);
            }
            Applied {
                said: texts.t(match v {
                    Visibility::Public => key::BINARY_CACHE_SAID_PUBLIC,
                    Visibility::Private => key::BINARY_CACHE_SAID_PRIVATE,
                }),
                netrc: (*v == Visibility::Private && r.token_kept())
                    || (*v == Visibility::Public && r.netrc),
            }
        }
        Edit::Token(token) => {
            let Some(dir) = places.keys.as_ref() else {
                return refused(ronoctua_cachix::Why::Unwritable);
            };
            if let Err(why) = ronoctua_cachix::token::keep(dir, token) {
                return refused(why);
            }
            Applied {
                said: texts.t(key::BINARY_CACHE_SAID_TOKEN),
                netrc: private_now,
            }
        }
        Edit::ForgetToken => {
            let forgot = places
                .keys
                .as_ref()
                .map_or(Err(ronoctua_cachix::Why::Unwritable), |d| {
                    ronoctua_cachix::token::forget(d)
                });
            if let Err(why) = forgot {
                return refused(why);
            }
            Applied {
                said: texts.t(key::BINARY_CACHE_SAID_FORGOTTEN),
                netrc: r.netrc,
            }
        }
        Edit::Remove => {
            if let Err(why) = ronoctua_cachix::settings::change(mirror, |c| *c = None) {
                return refused(why);
            }
            Applied {
                said: texts.t(key::BINARY_CACHE_SAID_REMOVED),
                netrc: r.netrc,
            }
        }
        Edit::Netrc => Applied {
            said: String::new(),
            netrc: true,
        },
    }
}

// ---------------------------------------------------------------------------
// ROOT'S NETRC — a root gesture, asked in the sudo bar
// ---------------------------------------------------------------------------

/// Makes root's netrc agree with the person's choice, on a thread, with a
/// root `gesture` of the door's (`crate::sudo_bar`): for a private cache with
/// a token, the token staged in the person's runtime folder and
/// `binary-cache-netrc <name>`; otherwise `binary-cache-netrc-remove`. What
/// came of it goes to `tell`, and to the verdict file the window that asked
/// reads.
pub fn door(
    places: Places,
    gesture: crate::sudo_bar::Gesture,
    tell: impl FnOnce(Came) + Send + 'static,
) {
    // A PHOTOGRAPH NEVER REACHES ROOT: the footer's camera runs in a room of
    // its own, and a door from there would stage a token in the person's
    // real runtime folder and write the machine's real netrc.
    if crate::footer_photograph::room().is_some() {
        tell(Came::Refused(ronoctua_security::Why::NotAllowed));
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("binary-cache-netrc".to_owned())
        .spawn(move || {
            let came = door_now(&places, gesture);
            write_verdict(&places, came);
            tell(came);
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "could not start the netrc's thread");
    }
}

/// [`door`]'s work, on its thread.
fn door_now(places: &Places, mut gesture: crate::sudo_bar::Gesture) -> Came {
    let r = Readings::read(places);
    let runtime = ronoctua_cachix::token::my_runtime();
    let (verb, writes) = match ronoctua_cachix::settings::netrc_for(&r.named, r.token_kept()) {
        Some(name) => {
            let token = match places
                .keys
                .as_ref()
                .ok_or(ronoctua_cachix::Why::NoToken)
                .and_then(ronoctua_cachix::token::read_from)
            {
                Ok(Some(t)) => t,
                Ok(None) => return Came::Token(ronoctua_cachix::Why::NoToken),
                Err(why) => return Came::Token(why),
            };
            if let Err(why) = ronoctua_cachix::token::stage(&runtime, &token) {
                return Came::Token(why);
            }
            (
                ronoctua_security::verb::RootVerb::BinaryCacheNetrc { name },
                true,
            )
        }
        None => (
            ronoctua_security::verb::RootVerb::BinaryCacheNetrcRemove,
            false,
        ),
    };
    let ran = gesture.run(verb.into(), &mut |_| {});
    // THE COPY GOES however it ended: the helper took it, or nobody will.
    ronoctua_cachix::token::unstage(&runtime);
    match ran {
        Ok(()) if writes => Came::Written,
        Ok(()) => Came::Removed,
        Err(why) => Came::Refused(why),
    }
}

/// Now, in milliseconds since the epoch.
#[must_use]
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Leaves what came of the netrc for the window that asked for it.
fn write_verdict(places: &Places, came: Came) {
    let Some(path) = places.verdict.as_ref() else {
        return;
    };
    let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
        return;
    };
    let verdict = Verdict {
        at_ms: now_ms(),
        came,
    };
    if let Ok(text) = serde_json::to_vec(&verdict) {
        let _ = ronoctua_xdg::write_atomically(dir, file, &text);
    }
}

/// The sentence of what came of the netrc.
#[must_use]
pub fn came_said(came: Came, texts: &ronoctua_i18n::Texts) -> String {
    match came {
        Came::Written => texts.t(key::BINARY_CACHE_SAID_NETRC_WRITTEN),
        Came::Removed => texts.t(key::BINARY_CACHE_SAID_NETRC_REMOVED),
        Came::Refused(why) => texts.t(crate::security_words::refusal(why)),
        Came::Token(why) => texts.t(why_key(why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLIC_KEY: &str = "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=";

    fn cache(visibility: Visibility) -> BinaryCache {
        BinaryCache {
            name: "someone".to_owned(),
            public_key: PUBLIC_KEY.to_owned(),
            visibility,
        }
    }

    fn readings(named: Named, token: TokenState) -> Readings {
        Readings {
            named,
            token,
            ..Readings::default()
        }
    }

    /// FAILS IF: a row is offered that does nothing here — the visibility or
    /// the removal with no cache, root's netrc for a public cache, the
    /// forgetting with no token — or a row that acts goes missing.
    #[test]
    fn the_rows_are_the_ones_that_say_or_do_something() {
        assert_eq!(
            rows_of(&Readings::default()),
            [Row::Cache, Row::Key, Row::Token, Row::Machine, Row::Last]
        );
        assert_eq!(
            rows_of(&readings(
                Named::Cache(cache(Visibility::Public)),
                TokenState::Kept
            )),
            [
                Row::Cache,
                Row::Key,
                Row::Visibility,
                Row::Token,
                Row::Machine,
                Row::Last,
                Row::ForgetToken,
                Row::Remove
            ]
        );
        assert!(
            rows_of(&readings(
                Named::Cache(cache(Visibility::Private)),
                TokenState::None
            ))
            .contains(&Row::Access)
        );
    }

    /// FAILS IF: naming a cache does not ask its name and then ITS key, a
    /// name or a key that is not one is taken, an answer to another question
    /// is taken, or the token is asked in the clear.
    #[test]
    fn the_name_then_its_key_and_the_token_a_secret() {
        let r = Readings::default();
        let mut p = Page::default();
        let Errand::Ask(line) = p.press(Press::Enter, &r) else {
            panic!("RET on the cache asks its name");
        };
        assert_eq!(line.id, NAME);
        assert!(!line.secret);
        assert_eq!(
            p.answered(NAME, "Not_A_Name", &r),
            Errand::Say(key::BINARY_CACHE_SAID_NOT_A_NAME)
        );
        p.press(Press::Enter, &r);
        let Errand::Ask(key_line) = p.answered(NAME, " someone ", &r) else {
            panic!("a good name asks its key");
        };
        assert_eq!(key_line.id, KEY);
        assert_eq!(key_line.asks_with, [("name", "someone".to_owned())]);
        assert_eq!(
            p.answered(TOKEN, "x", &r),
            Errand::Nothing,
            "another question"
        );
        assert_eq!(
            p.answered(
                KEY,
                "another.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=",
                &r
            ),
            Errand::Say(key::BINARY_CACHE_SAID_NOT_THE_KEY)
        );
        p.press(Press::Enter, &r);
        p.answered(NAME, "someone", &r);
        assert_eq!(
            p.answered(KEY, PUBLIC_KEY, &r),
            Errand::Edit(Edit::Named {
                name: "someone".to_owned(),
                key: PUBLIC_KEY.to_owned()
            })
        );
        // THE TOKEN, on the third row of a page with no cache.
        p.press(Press::Down, &r);
        p.press(Press::Down, &r);
        let Errand::Ask(token) = p.press(Press::Enter, &r) else {
            panic!("RET on the token asks it");
        };
        assert_eq!(token.id, TOKEN);
        assert!(token.secret, "the token is asked in the clear");
    }

    /// FAILS IF: a grave gesture goes without its question, the question is
    /// not born on "no", or `Esc` answers anything but "no".
    #[test]
    fn a_grave_gesture_asks_with_the_no_first() {
        let r = readings(Named::Cache(cache(Visibility::Public)), TokenState::Kept);
        let mut p = Page::default();
        let remove = rows_of(&r).iter().position(|w| *w == Row::Remove).unwrap();
        p.clicked(remove, &r);
        assert_eq!(
            p.clicked(remove, &r),
            Errand::Moved,
            "the second click asks"
        );
        assert_eq!(
            p.confirm(),
            Some(Confirm {
                what: Grave::Remove,
                yes: false
            })
        );
        assert_eq!(p.press(Press::Escape, &r), Errand::Moved);
        assert_eq!(p.confirm(), None);
        p.press(Press::Enter, &r);
        p.press(Press::Down, &r);
        assert_eq!(p.press(Press::Enter, &r), Errand::Edit(Edit::Remove));
    }

    /// FAILS IF: a copy that does not read is rewritten by a gesture on the
    /// cache, instead of the gesture saying why.
    #[test]
    fn a_copy_that_does_not_read_is_never_rewritten() {
        let r = readings(Named::Newer, TokenState::None);
        let mut p = Page::default();
        assert_eq!(
            p.press(Press::Enter, &r),
            Errand::Say(key::BINARY_CACHE_WHY_NEWER)
        );
    }
}
