//! `LEADER s v` — the belongings: the person's configuration, kept
//! encrypted in a private repository they name, in one view with two doors:
//! the footer's region (`s v v`) and a window (`s v w`,
//! `crate::belongings_window`). `s v s` syncs from anywhere.
//!
//! # What is here
//!
//! Everything the screen decides — which rows exist, what a key does to
//! them, what is asked on the door's line, and which files a gesture writes —
//! and nothing that talks to a network, runs `age` or knows what a window
//! is. The engine is `ronoctua-belongings`'s CLI, which the transfer queue
//! runs as a child (`Work::Belongings`); this crate links only its SHAPES
//! (the `prefs` feature), so `age` never enters the footer.
//!
//! # The file is the broadcast
//!
//! Both doors read the same files ([`Readings::read`]) and watch the same
//! three places ([`watch`]): `belongings.json` (what the person chose), the
//! state folder (`status.json`, how the last run ended; `last.json`, what the
//! last sync left) and the runtime folder (the passphrase's file, the
//! decisions, the session the CLI prepares). A door that writes one of them
//! is heard by the other the way every door hears a preference.
//!
//! # A sync, from the person's side
//!
//! `Sync now` asks the passphrase on the door's own line, writes it into a
//! `0600` runtime file born private ([`ronoctua_belongings::secret`]), and
//! puts `prepare` on the queue. With no conflict the CLI carries it through
//! and deletes the file. With conflicts it waits: this view lists EVERY one —
//! the path, both sides' size and time, the machine that changed the
//! repository's — and the person decides each (keep this machine's, take the
//! repository's, or keep both for what they added), in bulk or one by one,
//! with the difference of a text a `RET` away. `Apply` sends the decisions;
//! `Drop` sends `abandon`, which deletes the passphrase and the session.
//!
//! # Five more sections
//!
//! What is kept (the table `assets/belongings.nix` generates, each with its
//! reason, and what the person added or took out), how the repository is
//! reached (a deploy key this machine makes, or a token), the passphrase
//! (changing it re-encrypts one small file), and how often the brain reminds.

use std::path::{Path, PathBuf};

use ronoctua_belongings::Why;
use ronoctua_belongings::kept::{self, Pattern};
use ronoctua_belongings::order::Op;
use ronoctua_belongings::places::{Places, Root, covers};
use ronoctua_belongings::plan::{Conflict, Decision, Decisions, Plan, Side};
use ronoctua_belongings::prefs::{self, Auth, Belongings};
use ronoctua_belongings::secret::Typed;
use ronoctua_belongings::status::{Last, Outcome, Status};
use ronoctua_i18n::key;
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

use crate::belongings_words as words;
use crate::files::{Field, Hit as FilesHit, Ink};
use ronoctua_files::mode::Explanation;

/// The keys this view teaches, and where each one applies. The hint line is
/// GENERATED from it (`chrome::hint_of_marked`), so a key that moves here
/// moves on screen.
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "belongings-down", Where::Anywhere),
    ("<up>", "belongings-up", Where::Anywhere),
    // `S-<arrows>` TURN A VALUE — the period, the access — the system's rule
    // wherever a row has a value that turns.
    ("S-<left>", "belongings-less", Where::OnAValue),
    ("S-<right>", "belongings-more", Where::OnAValue),
    ("RET", "belongings-enter", Where::Anywhere),
    // THE THREE DECISIONS, on any row of a conflict — the path's, a
    // decision's, a line of its difference. `C-`, never `M-`: the `M-`
    // family is the compositor's and never reaches a window.
    ("C-l", "belongings-keep-local", Where::OnAConflict),
    ("C-r", "belongings-take-remote", Where::OnAConflict),
    ("C-b", "belongings-keep-both", Where::OnAConflictBoth),
    ("C-s", "belongings-sync", Where::Anywhere),
];

/// The commands the view's ROWS run — every one reachable by name too
/// (`LEADER :`), where it opens the view and does the same.
pub const OF_THE_ROWS: &[&str] = &[
    "belongings-apply",
    "belongings-abandon",
    "belongings-all-local",
    "belongings-all-remote",
    "belongings-repository",
    "belongings-add",
    "belongings-auth",
    "belongings-keygen",
    "belongings-copy-key",
    "belongings-token",
    "belongings-user",
    "belongings-passphrase",
    "belongings-period",
];

/// The command that opens the view in the footer (`LEADER s v v`).
pub const OPEN_COMMAND: &str = "belongings-open";
/// The command that opens it in a window (`LEADER s v w`).
pub const WINDOW_COMMAND: &str = "belongings-window";
/// The command that syncs: the view, asking the passphrase (`LEADER s v s`,
/// `C-s` inside it, and the brain's mark).
pub const SYNC_COMMAND: &str = "belongings-sync";

/// Every command this view's keys and rows reach — for `leader::reachable`,
/// which is what says a command has a key.
pub fn reached() -> impl Iterator<Item = &'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .chain(OF_THE_ROWS.iter().copied())
}

/// Which rows a key means anything on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a row whose value turns.
    OnAValue,
    /// On any row of a conflict.
    OnAConflict,
    /// On a conflict that may keep both sides.
    OnAConflictBoth,
}

/// A key, once the door has read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up one row.
    Up,
    /// Down one row.
    Down,
    /// The previous value.
    Less,
    /// The next value.
    More,
    /// Do what the row does.
    Enter,
    /// Decide the conflict under the cursor.
    Decide(Decision),
    /// Sync now.
    Sync,
    /// Back one level, and out.
    Escape,
}

/// A command of [`OF_THE_VIEW`] as the view's press — a click on the hint, or
/// the id run by name.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "belongings-down" => Press::Down,
        "belongings-up" => Press::Up,
        "belongings-less" => Press::Less,
        "belongings-more" => Press::More,
        "belongings-enter" => Press::Enter,
        "belongings-keep-local" => Press::Decide(Decision::KeepLocal),
        "belongings-take-remote" => Press::Decide(Decision::TakeRemote),
        "belongings-keep-both" => Press::Decide(Decision::KeepBoth),
        "belongings-sync" => Press::Sync,
        _ => return None,
    })
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` is the way back whatever is held.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    if key.sym == ronoctua_keymap::Sym::Esc {
        return Some(Press::Escape);
    }
    let notation = key.notation();
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _, _)| *taught == notation)
        .map(|(_, id, _)| *id)?;
    press_of(id)
}

// ---------------------------------------------------------------------------
// WHAT THE VIEW READS
// ---------------------------------------------------------------------------

/// What `last.json` says, in the few numbers the view shows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Summary {
    /// The repository it was made with.
    pub repository: String,
    /// When, seconds since the epoch.
    pub at_s: i64,
    /// How many paths are kept.
    pub files: u64,
    /// What they weigh.
    pub bytes: u64,
}

/// Everything both doors draw from — the files, read the same way by both.
#[derive(Debug, Clone, Default)]
pub struct Readings {
    /// Where everything is, on this machine; `None` with no home or no
    /// runtime directory, which the view says.
    pub places: Option<Places>,
    /// What the person chose.
    pub prefs: Belongings,
    /// `belongings.json` is there and does not parse: nothing is written
    /// over it, and the view says so.
    pub prefs_refused: bool,
    /// How the last run ended.
    pub status: Option<Status>,
    /// What the last sync left.
    pub last: Option<Summary>,
    /// A sync prepared and waiting for decisions.
    pub plan: Option<Plan>,
    /// What the person decided of it so far.
    pub decisions: Decisions,
    /// The deploy key's public half, when there is one.
    pub public_key: Option<String>,
    /// Whether the deploy key exists.
    pub has_key: bool,
    /// Whether a token is kept.
    pub has_token: bool,
    /// Whether a run of the belongings holds this machine's lock now
    /// (`ronoctua_belongings::running`): a second sync, an apply, a drop or a
    /// change of passphrase waits for it to end.
    pub busy: bool,
    /// What a person added on any machine that this machine followed at its
    /// last sync (`last.json`'s roots) — shown with what is kept, and taken
    /// out by the same row.
    pub followed: Vec<String>,
    /// Now, seconds since the epoch.
    pub now_s: i64,
}

impl Readings {
    /// Reads every file of the belongings under `places`, at `now_s`.
    ///
    /// Small files, read whole: the settings, two records, a plan. A file
    /// that is missing is the ordinary state of a machine that never synced.
    #[must_use]
    pub fn read(places: Option<Places>, now_s: i64) -> Self {
        let Some(p) = places else {
            return Self {
                now_s,
                ..Self::default()
            };
        };
        let (prefs, prefs_refused) = match store_of(&p).parsed() {
            Some(prefs) => (prefs, false),
            None => (Belongings::default(), true),
        };
        let last_file = Last::read(&p.last());
        let followed = last_file
            .as_ref()
            .filter(|l| l.repository == prefs.repository)
            .map(|l| l.roots.iter().cloned().collect())
            .unwrap_or_default();
        let last = last_file.map(|l| Summary {
            files: l.entries.len() as u64,
            bytes: l.entries.values().map(|k| k.size_bytes).sum(),
            repository: l.repository,
            at_s: l.at_s,
        });
        let plan: Option<Plan> = std::fs::read(p.plan())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let decisions: Decisions = std::fs::read(p.decisions())
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let public_key = std::fs::read_to_string(p.deploy_public())
            .ok()
            .map(|k| k.trim().to_owned())
            .filter(|k| !k.is_empty());
        Self {
            status: Status::read(&p.status()),
            has_key: p.deploy_key().is_file(),
            has_token: p.token().is_file(),
            busy: ronoctua_belongings::running::busy(&p),
            followed,
            public_key,
            last,
            plan,
            decisions,
            prefs,
            prefs_refused,
            places: Some(p),
            now_s,
        }
    }

    /// Whether this machine has never synced with the repository set now —
    /// its first sync restores.
    #[must_use]
    pub fn first_run(&self) -> bool {
        self.prefs.has_repository()
            && self
                .last
                .as_ref()
                .is_none_or(|l| l.repository != self.prefs.repository)
    }

    /// The decision a conflict has now: the person's, else its suggestion.
    #[must_use]
    pub fn decision_of(&self, c: &Conflict) -> Option<Decision> {
        self.decisions
            .get(&c.path)
            .copied()
            .filter(|d| d.allowed(c))
            .or(c.suggested)
    }

    /// Every decision as it stands — the person's over the suggestions.
    #[must_use]
    pub fn decided(&self) -> Decisions {
        let Some(plan) = self.plan.as_ref() else {
            return Decisions::new();
        };
        plan.conflicts
            .iter()
            .filter_map(|c| self.decision_of(c).map(|d| (c.path.clone(), d)))
            .collect()
    }

    /// Whether the access the settings ask for is here: the deploy key, or
    /// the token.
    #[must_use]
    pub const fn has_access(&self) -> bool {
        match self.prefs.auth {
            Auth::Ssh => self.has_key,
            Auth::Token => self.has_token,
        }
    }
}

/// `belongings.json`'s store, under the configuration root of `places`.
#[must_use]
pub fn store_of(places: &Places) -> ronoctua_store::Store<Belongings> {
    ronoctua_store::Store::at(Some(places.config.clone()), prefs::FILE)
}

/// Seconds since the epoch, now.
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// The watches that make a door hear the belongings' files: the settings,
/// the state folder and the runtime folder, each made first — a watch on a
/// folder that does not exist never fires. `ring` is told of every change.
///
/// The folders are made the way the CLI makes them (`0700`): they hold the
/// record of a sync and, in the runtime folder, the passphrase's file.
#[must_use]
pub fn watch<F>(places: &Places, ring: F) -> Vec<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + Clone + 'static,
{
    use ronoctua_belongings::secret::make_private_dir;
    use ronoctua_files::notice::{Notice, Pace};
    let mut out = Vec::new();
    if std::fs::create_dir_all(&places.config).is_ok()
        && let Ok(n) = Notice::on_file(&places.config, prefs::FILE, Pace::Coalesced, ring.clone())
    {
        out.push(n);
    }
    for dir in [places.own(), places.runtime.clone()] {
        if let Err(e) = make_private_dir(&dir) {
            tracing::warn!(dir = %dir.display(), error = %e, "no folder to watch the belongings in");
            continue;
        }
        match Notice::watching(Some(&dir), ring.clone()) {
            Ok(n) => out.push(n),
            Err(e) => {
                tracing::warn!(dir = %dir.display(), error = %e, "the belongings cannot be watched");
            }
        }
    }
    out
}

/// The view while the FOOTER shows it: what it reads, where from, the
/// watches that keep that current and their place on the loop — hung when
/// the view opens and taken down when it closes: a watch lives only while it
/// is needed.
pub struct Open {
    /// The view.
    pub view: View,
    /// What it draws.
    pub readings: Readings,
    /// Where it reads from.
    pub places: Option<Places>,
    /// The notices.
    pub watches: Vec<ronoctua_files::notice::Notice>,
    /// Their channel on the footer's loop.
    pub token: Option<smithay_client_toolkit::reexports::calloop::RegistrationToken>,
}

impl Open {
    /// Reads the files again, as they are now.
    pub fn reread(&mut self) {
        self.readings = Readings::read(self.places.clone(), now_s());
    }
}

/// Whether `id` is one of the belongings' commands — the ones a door runs
/// through the view ([`View::command`], [`View::answered`]).
#[must_use]
pub fn is_command(id: &str) -> bool {
    id == SYNC_COMMAND || reached().any(|c| c == id)
}

// ---------------------------------------------------------------------------
// ROWS
// ---------------------------------------------------------------------------

/// What the keys do on a row — and, since it carries what the row is about,
/// its identity across a rebuild.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A fact: `RET` copies its value. The word says which.
    Fact(String),
    /// The repository's address: `RET` asks it.
    Repository,
    /// Sync now.
    Sync,
    /// Apply the decisions.
    Apply,
    /// Drop the prepared sync.
    Abandon,
    /// Decide every conflict alike.
    All(Decision),
    /// A conflict's path: `RET` shows the difference of a text.
    Conflict(String),
    /// Line `n` of a conflict's difference: the conflict's keys decide it,
    /// `RET` copies the line.
    DiffLine(String, usize),
    /// One decision of a conflict.
    Decide(String, Decision),
    /// A path the table keeps: `RET` takes it out.
    Rule(String),
    /// A path the person added: `RET` stops keeping it.
    Kept(String),
    /// A path the person took out: `RET` keeps it again.
    Excluded(String),
    /// A path never kept by default, that a person may add: `RET` adds it.
    Never(String),
    /// Add a path: `RET` asks it.
    Add,
    /// How the repository is reached.
    Auth,
    /// Make the deploy key.
    Keygen,
    /// Copy the deploy key's public half.
    CopyKey,
    /// The token: `RET` asks it.
    Token,
    /// The HTTPS user: `RET` asks it.
    User,
    /// Change the passphrase.
    Passphrase,
    /// How often the brain reminds.
    Period,
}

impl Target {
    /// Whether `S-<arrows>` mean anything here.
    const fn turns(&self) -> bool {
        matches!(self, Self::Period | Self::Auth)
    }

    /// The conflict this row is about, when it is about one.
    fn conflict(&self) -> Option<&str> {
        match self {
            Self::Conflict(p) | Self::Decide(p, _) | Self::DiffLine(p, _) => Some(p),
            _ => None,
        }
    }

    /// The command this row's `RET` is, when it is one — what the pointer
    /// resting on it reads (`cmd-<id>`).
    const fn command(&self) -> Option<&'static str> {
        Some(match self {
            Self::Repository => "belongings-repository",
            Self::Sync => SYNC_COMMAND,
            Self::Apply => "belongings-apply",
            Self::Abandon => "belongings-abandon",
            Self::All(Decision::KeepLocal) => "belongings-all-local",
            Self::All(_) => "belongings-all-remote",
            Self::Decide(_, d) => words::decide_command(*d),
            Self::Add => "belongings-add",
            Self::Auth => "belongings-auth",
            Self::Keygen => "belongings-keygen",
            Self::CopyKey => "belongings-copy-key",
            Self::Token => "belongings-token",
            Self::User => "belongings-user",
            Self::Passphrase => "belongings-passphrase",
            Self::Period => "belongings-period",
            _ => return None,
        })
    }

    /// What the pointer resting on the row reads.
    fn about(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        if let Some(id) = self.command() {
            return Some(texts.t(&format!("cmd-{id}")));
        }
        Some(texts.t(match self {
            Self::Conflict(_) => key::ABOUT_BELONGINGS_CONFLICT,
            Self::Rule(_) => key::ABOUT_BELONGINGS_RULE,
            Self::Kept(_) => key::ABOUT_BELONGINGS_KEPT,
            Self::Excluded(_) => key::ABOUT_BELONGINGS_EXCLUDED,
            Self::Never(_) => key::ABOUT_BELONGINGS_NEVER,
            _ => return None,
        }))
    }
}

/// One drawn line.
#[derive(Debug, Clone)]
pub struct Row {
    /// The pieces, already placed and cut.
    pub fields: Vec<Field>,
    /// What a key does here — `None` on a title, a sentence, the rest of a
    /// wrapped label: the cursor skips it.
    pub target: Option<Target>,
}

impl Row {
    /// Whether the cursor skips it.
    #[must_use]
    pub const fn heading(&self) -> bool {
        self.target.is_none()
    }
}

/// One entry of a section, before the section is laid out whole.
enum Entry {
    /// A `label  value` row.
    Pair {
        label: String,
        value: String,
        kind: crate::chrome::Kind,
        target: Target,
        tint: Option<tokens::Color>,
        /// Cells in from the section's left: a decision under its conflict.
        indent: u16,
    },
    /// A line of air between two items of one list.
    Air,
    /// A sentence under the row before it, wrapped.
    Note(String, tokens::Color),
    /// Lines drawn as they are — a difference.
    Lines(Vec<(Vec<Field>, Target)>),
}

/// The rows, as they are built.
struct Builder<'a> {
    out: Vec<Row>,
    cursor: usize,
    left: u16,
    room: u16,
    texts: &'a ronoctua_i18n::Texts,
}

impl Builder<'_> {
    /// A section: its title, and its entries laid out whole — the values of
    /// every row in one column after the widest label (`section_column`),
    /// a label that does not fit wrapped under itself.
    fn section(&mut self, title: &str, entries: Vec<Entry>) {
        let end = self.left + self.room;
        // AIR BEFORE EVERY TITLE but the first — `chrome::AIR`.
        if !self.out.is_empty() {
            self.out.push(Row {
                fields: crate::chrome::AIR,
                target: None,
            });
        }
        self.out.push(Row {
            fields: vec![piece(
                self.left,
                title.to_owned(),
                tokens::color::TEXT_HEADING,
                self.room,
            )],
            target: None,
        });
        let pairs: Vec<(u16, u16, u16)> = entries
            .iter()
            .filter_map(|e| match e {
                Entry::Pair {
                    label,
                    value,
                    indent,
                    ..
                } => Some((
                    self.left + indent,
                    layout::width(label),
                    layout::width(value),
                )),
                _ => None,
            })
            .collect();
        let column = crate::chrome::section_column(self.left, self.room, &pairs);
        for entry in entries {
            match entry {
                Entry::Pair {
                    label,
                    value,
                    kind,
                    target,
                    tint,
                    indent,
                } => {
                    let focused = self.cursor == self.out.len();
                    let lines = crate::chrome::aligned_pair(
                        self.left + indent,
                        column,
                        end,
                        &label,
                        &value,
                        kind,
                        focused,
                    );
                    for (n, fields) in lines.into_iter().enumerate() {
                        let fields = if n == 0 {
                            crate::chrome::tinted(fields, tint, focused)
                        } else {
                            fields
                        };
                        self.out.push(Row {
                            fields,
                            target: (n == 0).then(|| target.clone()),
                        });
                    }
                }
                Entry::Air => self.out.push(Row {
                    fields: crate::chrome::AIR,
                    target: None,
                }),
                Entry::Note(text, colour) => {
                    for line in crate::chrome::note_lines(&text, self.room) {
                        self.out.push(Row {
                            fields: vec![piece(self.left, line, colour, self.room)],
                            target: None,
                        });
                    }
                }
                Entry::Lines(lines) => {
                    for (mut fields, target) in lines {
                        // THE CURSOR IS A COLOUR here too: a line of a
                        // difference is all label, and under the cursor it
                        // takes the accent a label takes — its `+` or `-`
                        // still says which side it is.
                        if self.cursor == self.out.len() {
                            for f in &mut fields {
                                f.color = crate::chrome::Kind::Setting.tones(true).0;
                            }
                        }
                        self.out.push(Row {
                            fields,
                            target: Some(target),
                        });
                    }
                }
            }
        }
    }

    fn t(&self, k: &str) -> String {
        self.texts.t(k)
    }
}

/// A pair entry.
fn pair(label: String, value: String, target: Target, acts: bool) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::of(acts),
        target,
        tint: None,
        indent: 0,
    }
}

/// A decision's row, in under its conflict.
fn indented(label: String, value: String, target: Target) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::Setting,
        target,
        tint: None,
        indent: DECISION_INDENT,
    }
}

/// How far in a conflict's decisions are.
const DECISION_INDENT: u16 = 2;

/// The same, with its value in a colour that says a state.
fn tinted(label: String, value: String, target: Target, acts: bool, tint: tokens::Color) -> Entry {
    Entry::Pair {
        label,
        value,
        kind: crate::chrome::Kind::of(acts),
        target,
        tint: Some(tint),
        indent: 0,
    }
}

/// A dim sentence.
fn note(text: String) -> Entry {
    Entry::Note(text, tokens::color::TEXT_DIM)
}

/// One piece of text, elided to `room`.
fn piece(at: u16, text: String, color: tokens::Color, room: u16) -> Field {
    Field {
        at,
        text: layout::elide_end(&text, room),
        color,
        hit: FilesHit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

/// A logical path as a person reads it: under their home, `~/…`; a folder
/// the table keeps whole ends in `/`.
#[must_use]
pub fn shown_path(logical: &str, places: Option<&Places>, folder: bool) -> String {
    let resolved = places.and_then(|p| p.resolve(logical).map(|at| (p, at)));
    let mut text = match resolved {
        Some((p, at)) => match at.strip_prefix(&p.home) {
            Ok(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => at.display().to_string(),
        },
        None => logical.to_owned(),
    };
    if folder && !text.ends_with('/') {
        text.push('/');
    }
    text
}

/// A size, in the language's own marks.
fn bytes_text(bytes: u64, texts: &ronoctua_i18n::Texts) -> String {
    crate::sizes::readable_bytes(bytes, texts.decimal_mark())
}

/// A moment, as the person reads dates.
fn when(secs: i64, zone: &ronoctua_files::when::Zone, texts: &ronoctua_i18n::Texts) -> String {
    crate::chrome::when_text(secs, zone, false, texts)
}

/// One side of a conflict, in words: its size and when it changed — or
/// "deleted".
fn side_text(
    side: Option<&Side>,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    match side {
        None => texts.t(key::BELONGINGS_SIDE_DELETED),
        Some(s) if s.modified_s > 0 => texts.t_with(
            key::BELONGINGS_SIDE,
            &[
                ("size", &bytes_text(s.size_bytes, texts)),
                ("when", &when(s.modified_s, zone, texts)),
            ],
        ),
        Some(s) => bytes_text(s.size_bytes, texts),
    }
}

/// The sentence a run's ending says.
#[must_use]
pub fn outcome_text(outcome: &Outcome, texts: &ronoctua_i18n::Texts) -> String {
    use ronoctua_i18n::Arg;
    let n = |v: u32| Arg::Number(i64::from(v));
    match outcome {
        Outcome::Synced {
            pulled,
            pushed,
            deleted_here,
            deleted_there,
            ..
        } => texts.t_args(
            key::BELONGINGS_OUTCOME_SYNCED,
            &[
                ("pulled", n(*pulled)),
                ("pushed", n(*pushed)),
                ("deleted", n(deleted_here + deleted_there)),
            ],
        ),
        Outcome::Waiting { conflicts } => {
            texts.t_args(key::BELONGINGS_OUTCOME_WAITING, &[("n", n(*conflicts))])
        }
        Outcome::Refused { why } => texts.t(words::refusal_key(*why)),
        Outcome::Abandoned => texts.t(key::BELONGINGS_OUTCOME_ABANDONED),
        Outcome::Keyed => texts.t(key::BELONGINGS_OUTCOME_KEYED),
        Outcome::Rekeyed => texts.t(key::BELONGINGS_OUTCOME_REKEYED),
    }
}

// ---------------------------------------------------------------------------
// QUESTIONS
// ---------------------------------------------------------------------------

/// Something the person types, and what it is for — asked on the line of
/// the door the key was pressed in, answered through [`View::answered`].
///
/// A SECRET is held only until it is written, and its bytes are overwritten
/// when it is dropped ([`Typed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ask {
    /// The repository's address.
    Repository,
    /// The passphrase, for a sync.
    Passphrase,
    /// The passphrase again, on a machine's first sync: an empty repository
    /// takes what is typed as its passphrase for good, and nothing recovers
    /// one that is lost.
    Confirm(Typed),
    /// The passphrase as it is, to change it.
    Old,
    /// The new passphrase, the old one held.
    New(Typed),
    /// The new one again.
    Again(Typed, Typed),
    /// The HTTPS token.
    Token,
    /// The HTTPS user.
    User,
    /// A path to keep.
    Path,
}

impl Ask {
    /// The command that asks — and that answers: the question is its
    /// `ask-<id>` key unless it has its own ([`Ask::asks`]).
    #[must_use]
    pub const fn command(&self) -> &'static str {
        match self {
            Self::Repository => "belongings-repository",
            Self::Passphrase | Self::Confirm(_) => SYNC_COMMAND,
            Self::Old | Self::New(_) | Self::Again(..) => "belongings-passphrase",
            Self::Token => "belongings-token",
            Self::User => "belongings-user",
            Self::Path => "belongings-add",
        }
    }

    /// The question's own key, when it is not `ask-<command>` — the three
    /// steps of a change of passphrase are one command.
    #[must_use]
    pub const fn asks(&self) -> Option<&'static str> {
        match self {
            Self::New(_) => Some(key::ASK_BELONGINGS_PASSPHRASE_NEW),
            Self::Again(..) => Some(key::ASK_BELONGINGS_PASSPHRASE_AGAIN),
            Self::Confirm(_) => Some(key::ASK_BELONGINGS_SYNC_AGAIN),
            _ => None,
        }
    }

    /// Whether what is typed is hidden.
    #[must_use]
    pub const fn secret(&self) -> bool {
        matches!(
            self,
            Self::Passphrase
                | Self::Confirm(_)
                | Self::Old
                | Self::New(_)
                | Self::Again(..)
                | Self::Token
        )
    }

    /// The line a door opens for it — a changed address starts from what it
    /// is.
    #[must_use]
    pub fn line(&self, r: &Readings) -> crate::line::Line {
        let initial = match self {
            Self::Repository => Some(r.prefs.repository.as_str()).filter(|a| !a.is_empty()),
            Self::User => r.prefs.user.as_deref(),
            _ => None,
        };
        let mut line = crate::line::Line::new(self.command(), self.secret(), initial);
        line.asks = self.asks();
        line
    }
}

// ---------------------------------------------------------------------------
// WHAT THE DOOR DOES
// ---------------------------------------------------------------------------

/// What the door has to do after a key, a click or an answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Shut the view.
    Close,
    /// Open this question on the door's own line.
    Ask(crate::line::Line),
    /// Say this sentence on the door's line.
    Say(String),
    /// Put this on the clipboard.
    Copy(String),
    /// Put this order on the queue — every file it reads is written already.
    Queue(Op),
}

/// Which page of the view is showing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Mode {
    /// Every section.
    #[default]
    List,
    /// The difference of one conflicting text: its path and its lines.
    Diff(String, Vec<String>),
}

/// How many lines of a difference are read.
const DIFF_MOST_LINES: usize = 4000;

/// The view: where the cursor is, which page shows, what is being asked.
/// Everything it DRAWS is read from the [`Readings`] each door hands it, so
/// the two doors draw one set of files.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    mode: Mode,
    asking: Option<Ask>,
    /// The row the cursor is on, by what it is — kept across a rebuild, so a
    /// list that changes under it does not move it to another row.
    aim: Option<Target>,
}

impl View {
    /// Opens it, on the first row that acts.
    #[must_use]
    pub fn fresh(
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Self {
        let mut me = Self::default();
        me.settle_in(r, zone, columns, texts);
        me
    }

    /// Where the cursor is, for the door that draws.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// The question a line is open for.
    #[must_use]
    pub const fn asking(&self) -> Option<&Ask> {
        self.asking.as_ref()
    }

    /// The hint line: the keys that work where the cursor is.
    #[must_use]
    pub fn hint(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(r, zone, columns, texts);
        let target = rows.get(self.cursor).and_then(|row| row.target.clone());
        let both = target
            .as_ref()
            .and_then(Target::conflict)
            .and_then(|p| conflict_of(r, p))
            .is_some_and(|c| Decision::KeepBoth.allowed(c));
        let here: Vec<(&str, &str, bool)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| match w {
                Where::Anywhere => true,
                Where::OnAValue => target.as_ref().is_some_and(Target::turns),
                Where::OnAConflict => target.as_ref().and_then(Target::conflict).is_some(),
                Where::OnAConflictBoth => both,
            })
            .map(|(k, id, w)| (*k, *id, !matches!(w, Where::Anywhere)))
            .collect();
        crate::chrome::hint_of_marked(&here, |_| None, texts)
    }

    /// Puts the cursor where a click landed. Answers whether the cursor was
    /// ALREADY on that row: that is the second click, which the door turns
    /// into `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        let Some(target) = rows.get(n).and_then(|r| r.target.clone()) else {
            return false;
        };
        let again = self.cursor == n;
        self.cursor = n;
        self.aim = Some(target);
        again
    }

    /// The sentence the pointer resting on row `n` reads.
    #[must_use]
    pub fn about(rows: &[Row], n: usize, texts: &ronoctua_i18n::Texts) -> Option<String> {
        rows.get(n)?.target.as_ref()?.about(texts)
    }

    /// Puts the cursor back on the row it was aimed at — found by what it is,
    /// since the list may have changed under it — or on the nearest row that
    /// acts. Called after every reading.
    pub fn settle_in(
        &mut self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        // A DIFFERENCE OF A CONFLICT THAT IS GONE goes back to the list.
        if let Mode::Diff(path, _) = &self.mode
            && conflict_of(r, path).is_none()
        {
            self.mode = Mode::List;
        }
        let rows = self.rows(r, zone, columns, texts);
        if let Some(aim) = self.aim.as_ref()
            && let Some(n) = rows.iter().position(|row| row.target.as_ref() == Some(aim))
        {
            self.cursor = n;
            return;
        }
        if rows.get(self.cursor).is_some_and(|row| !row.heading()) {
            self.aim = rows[self.cursor].target.clone();
            return;
        }
        let at = self.cursor.min(rows.len().saturating_sub(1));
        let found = (at..rows.len())
            .chain((0..at).rev())
            .find(|n| !rows[*n].heading());
        self.cursor = found.unwrap_or(0);
        self.aim = rows.get(self.cursor).and_then(|row| row.target.clone());
    }

    /// The rows to draw, at this width, from what is read now.
    #[must_use]
    pub fn rows(
        &self,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<Row> {
        let (left, room) = crate::power::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            cursor: self.cursor,
            left,
            room,
            texts,
        };
        if let Mode::Diff(path, lines) = &self.mode {
            diff_page(&mut b, r, path, lines);
            return b.out;
        }
        if r.places.is_none() {
            b.section(
                &b.t(key::BELONGINGS_STATUS),
                vec![note(texts.t(key::BELONGINGS_NO_PLACES))],
            );
            return b.out;
        }
        status_section(&mut b, r, zone);
        sync_section(&mut b, r);
        if r.plan.as_ref().is_some_and(|p| !p.conflicts.is_empty()) {
            conflicts_section(&mut b, r, zone);
        }
        kept_section(&mut b, r);
        access_section(&mut b, r);
        passphrase_section(&mut b);
        period_section(&mut b, r);
        b.out
    }

    /// What a key does. THE WIDTH IS THE DOOR'S: the cursor is an index into
    /// the rows that were DRAWN.
    pub fn press(
        &mut self,
        p: Press,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(r, zone, columns, texts);
        let target = rows.get(self.cursor).and_then(|row| row.target.clone());
        match p {
            Press::Up | Press::Down => {
                self.walk(&rows, p == Press::Down);
                Errand::Nothing
            }
            Press::Escape => match self.mode {
                Mode::Diff(ref path, _) => {
                    let path = path.clone();
                    self.mode = Mode::List;
                    self.aim = Some(Target::Conflict(path));
                    self.settle_in(r, zone, columns, texts);
                    Errand::Nothing
                }
                Mode::List => Errand::Close,
            },
            Press::Less | Press::More => match target {
                Some(Target::Period) => self.change(r, texts, |prefs| {
                    prefs.period_days = if p == Press::More {
                        Belongings::next_period(prefs.period_days)
                    } else {
                        previous_period(prefs.period_days)
                    };
                }),
                Some(Target::Auth) => self.change(r, texts, flip_auth),
                _ => Errand::Nothing,
            },
            Press::Decide(d) => match target.as_ref().and_then(Target::conflict) {
                Some(path) => {
                    let path = path.to_owned();
                    self.decide(r, &path, d, texts)
                }
                None => Errand::Say(texts.t(key::BELONGINGS_NO_CONFLICT_HERE)),
            },
            Press::Sync => self.start_sync(r, texts),
            Press::Enter => match target {
                Some(t) => self.enter(t, r, zone, columns, texts),
                None => Errand::Nothing,
            },
        }
    }

    /// A command by its name — `LEADER :`, a leader key, a click on a hint,
    /// the brain's mark. What `RET` on its row does, whatever row the cursor
    /// is on.
    pub fn command(
        &mut self,
        id: &str,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if let Some(p) = press_of(id) {
            return self.press(p, r, zone, columns, texts);
        }
        let target = match id {
            "belongings-apply" => Target::Apply,
            "belongings-abandon" => Target::Abandon,
            "belongings-all-local" => Target::All(Decision::KeepLocal),
            "belongings-all-remote" => Target::All(Decision::TakeRemote),
            "belongings-repository" => Target::Repository,
            "belongings-add" => Target::Add,
            "belongings-auth" => Target::Auth,
            "belongings-keygen" => Target::Keygen,
            "belongings-copy-key" => Target::CopyKey,
            "belongings-token" => Target::Token,
            "belongings-user" => Target::User,
            "belongings-passphrase" => Target::Passphrase,
            "belongings-period" => Target::Period,
            _ => return Errand::Nothing,
        };
        self.enter(target, r, zone, columns, texts)
    }

    /// `RET` on a row.
    fn enter(
        &mut self,
        target: Target,
        r: &Readings,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        match target {
            Target::Fact(word) => {
                let rows = self.rows(r, zone, columns, texts);
                rows.iter()
                    .find(|row| row.target.as_ref() == Some(&Target::Fact(word.clone())))
                    .and_then(|row| row.fields.last())
                    .map(|f| f.text.trim().to_owned())
                    .filter(|t| !t.is_empty())
                    .map_or(Errand::Nothing, Errand::Copy)
            }
            Target::DiffLine(path, n) => {
                let rows = self.rows(r, zone, columns, texts);
                let target = Target::DiffLine(path, n);
                rows.iter()
                    .find(|row| row.target.as_ref() == Some(&target))
                    .map(|row| {
                        row.fields
                            .iter()
                            .map(|f| f.text.as_str())
                            .collect::<String>()
                    })
                    .filter(|t| !t.trim().is_empty())
                    .map_or(Errand::Nothing, Errand::Copy)
            }
            Target::Repository => self.ask(Ask::Repository, r),
            Target::Sync => self.start_sync(r, texts),
            Target::Apply => self.apply(r, texts),
            Target::Abandon => {
                if r.plan.is_none() {
                    return Errand::Say(texts.t(key::BELONGINGS_NOTHING_PREPARED));
                }
                if r.busy {
                    return Errand::Say(texts.t(words::refusal_key(Why::Busy)));
                }
                Errand::Queue(Op::Abandon)
            }
            Target::All(d) => {
                let Some(plan) = r.plan.as_ref() else {
                    return Errand::Say(texts.t(key::BELONGINGS_NOTHING_PREPARED));
                };
                let mut decided = r.decided();
                decided.extend(plan.all(d));
                write_decisions(r, &decided, texts)
            }
            Target::Conflict(path) => self.open_diff(r, &path, zone, columns, texts),
            Target::Decide(path, d) => self.decide(r, &path, d, texts),
            Target::Rule(path) => self.change(r, texts, |p| {
                p.stop_keeping(&path);
            }),
            Target::Kept(path) => self.change(r, texts, |p| {
                p.stop_keeping(&path);
            }),
            Target::Excluded(path) | Target::Never(path) => self.change(r, texts, |p| {
                p.keep(&path);
            }),
            Target::Add => self.ask(Ask::Path, r),
            Target::Auth => self.change(r, texts, flip_auth),
            Target::Keygen => {
                if r.has_key {
                    return Errand::Say(texts.t(key::BELONGINGS_KEY_ALREADY));
                }
                Errand::Queue(Op::Keygen)
            }
            Target::CopyKey => r.public_key.clone().map_or_else(
                || Errand::Say(texts.t(key::BELONGINGS_NO_KEY_YET)),
                Errand::Copy,
            ),
            Target::Token => self.ask(Ask::Token, r),
            Target::User => self.ask(Ask::User, r),
            Target::Passphrase => {
                if !r.prefs.has_repository() {
                    return Errand::Say(texts.t(words::refusal_key(Why::NoRepository)));
                }
                if r.first_run() {
                    return Errand::Say(texts.t(key::BELONGINGS_PASSPHRASE_FIRST));
                }
                // A CHANGE OF PASSPHRASE WRITES THE PASSPHRASE'S FILE, which a
                // waiting sync and a running one are using.
                if let Some(said) = in_the_way(r, texts) {
                    return Errand::Say(said);
                }
                self.ask(Ask::Old, r)
            }
            Target::Period => self.change(r, texts, |p| {
                p.period_days = Belongings::next_period(p.period_days);
            }),
        }
    }

    /// Opens a question, remembered: the answer alone cannot say what it
    /// answers.
    fn ask(&mut self, ask: Ask, r: &Readings) -> Errand {
        let line = ask.line(r);
        self.asking = Some(ask);
        Errand::Ask(line)
    }

    /// Sync now: refused with its sentence when there is nothing to sync
    /// with, else the passphrase is asked.
    fn start_sync(&mut self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        if let Some(said) = unready(r, texts) {
            return Errand::Say(said);
        }
        self.ask(Ask::Passphrase, r)
    }

    /// Apply the decisions: refused while one is missing, or while a run
    /// holds the lock.
    fn apply(&self, r: &Readings, texts: &ronoctua_i18n::Texts) -> Errand {
        let Some(plan) = r.plan.as_ref() else {
            return Errand::Say(texts.t(key::BELONGINGS_NOTHING_PREPARED));
        };
        if r.busy {
            return Errand::Say(texts.t(words::refusal_key(Why::Busy)));
        }
        let decided = r.decided();
        if !plan.decided(&decided) {
            let left = plan
                .conflicts
                .iter()
                .filter(|c| !decided.get(&c.path).is_some_and(|d| d.allowed(c)))
                .count();
            return Errand::Say(texts.t_args(
                key::BELONGINGS_UNDECIDED_LEFT,
                &[(
                    "n",
                    ronoctua_i18n::Arg::Number(i64::try_from(left).unwrap_or(i64::MAX)),
                )],
            ));
        }
        // WHAT IS DECIDED, WRITTEN BEFORE THE ORDER: the CLI reads the file,
        // and a suggestion nobody touched is a decision too.
        match write_decisions_file(r, &decided) {
            Ok(()) => Errand::Queue(Op::Apply),
            Err(said) => Errand::Say(texts.t(said)),
        }
    }

    /// One conflict decided.
    fn decide(
        &self,
        r: &Readings,
        path: &str,
        d: Decision,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(c) = conflict_of(r, path) else {
            return Errand::Say(texts.t(key::BELONGINGS_NO_CONFLICT_HERE));
        };
        if !d.allowed(c) {
            return Errand::Say(texts.t(key::BELONGINGS_BOTH_ONLY_ADDED));
        }
        let mut decided = r.decided();
        decided.insert(path.to_owned(), d);
        write_decisions(r, &decided, texts)
    }

    /// The difference of a conflicting text, on a page of its own.
    fn open_diff(
        &mut self,
        r: &Readings,
        path: &str,
        zone: &ronoctua_files::when::Zone,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(c) = conflict_of(r, path) else {
            return Errand::Nothing;
        };
        let file = c
            .theirs
            .as_ref()
            .zip(r.places.as_ref())
            .map(|(name, p)| p.theirs(&format!("{name}.diff")));
        let Some(text) = file.and_then(|f| std::fs::read_to_string(f).ok()) else {
            return Errand::Say(texts.t(if c.text {
                key::BELONGINGS_NO_DIFFERENCE
            } else {
                key::BELONGINGS_NOT_TEXT
            }));
        };
        let lines: Vec<String> = text
            .lines()
            .take(DIFF_MOST_LINES)
            .map(|l| l.replace('\t', "    "))
            .collect();
        self.mode = Mode::Diff(path.to_owned(), lines);
        self.aim = Some(Target::Decide(path.to_owned(), Decision::KeepLocal));
        self.cursor = 0;
        self.settle_in(r, zone, columns, texts);
        Errand::Nothing
    }

    /// One change to `belongings.json`, laid over the file as it is NOW.
    fn change(
        &self,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
        change: impl FnOnce(&mut Belongings),
    ) -> Errand {
        match change_the_file(r.places.as_ref(), change) {
            Ok(()) => Errand::Nothing,
            Err(said) => Errand::Say(said.sentence(texts)),
        }
    }

    /// What was typed on a line, under the line's id — one call for both
    /// doors: the footer's minibuffer answers through the command of that
    /// id, the window through `Content::answered`.
    ///
    /// THE QUESTION THE VIEW ASKED ANSWERS, and only when the id is its own:
    /// a question let go, then another command typed, is not answered as the
    /// old one.
    pub fn answered(
        &mut self,
        id: &str,
        said: &str,
        r: &Readings,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let Some(ask) = self.asking.take().filter(|a| a.command() == id) else {
            return Errand::Nothing;
        };
        let typed = Typed::new(said.to_owned());
        match ask {
            Ask::Repository => {
                let address = said.trim();
                if let Err(why) = prefs::repository_is_plain(address) {
                    return Errand::Say(texts.t(words::invalid_key(why)));
                }
                self.change(r, texts, |p| address.clone_into(&mut p.repository))
            }
            Ask::User => {
                let user = said.trim().to_owned();
                self.change(r, texts, |p| p.user = (!user.is_empty()).then_some(user))
            }
            Ask::Path => {
                let Some(places) = r.places.as_ref() else {
                    return Errand::Say(texts.t(key::BELONGINGS_NO_PLACES));
                };
                match logical_of_typed(places, said.trim()) {
                    Some(logical) if kept::may_add(&logical) => self.change(r, texts, |p| {
                        p.keep(&logical);
                    }),
                    Some(_) => Errand::Say(texts.t(key::BELONGINGS_NEVER_KEPT)),
                    None => Errand::Say(texts.t(key::BELONGINGS_OUTSIDE_HOME)),
                }
            }
            Ask::Token => {
                if typed.expose().trim().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                match write_secret(r, Places::token, typed.expose().trim()) {
                    Ok(()) => Errand::Say(texts.t(key::BELONGINGS_TOKEN_KEPT)),
                    Err(said) => Errand::Say(texts.t(said)),
                }
            }
            Ask::Passphrase => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                if let Some(refused) = unready(r, texts) {
                    return Errand::Say(refused);
                }
                // A FIRST SYNC ASKS IT TWICE: when the repository is empty,
                // what is typed becomes its passphrase for good.
                if r.first_run() {
                    return self.ask(Ask::Confirm(typed), r);
                }
                send_passphrase(r, &typed, texts)
            }
            Ask::Confirm(first) => {
                if typed != first {
                    return Errand::Say(texts.t(key::BELONGINGS_FIRST_PASSPHRASES_DIFFER));
                }
                if let Some(refused) = unready(r, texts) {
                    return Errand::Say(refused);
                }
                send_passphrase(r, &typed, texts)
            }
            Ask::Old => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                self.ask(Ask::New(typed), r)
            }
            Ask::New(old) => {
                if typed.expose().is_empty() {
                    return Errand::Say(texts.t(key::BELONGINGS_EMPTY_SECRET));
                }
                self.ask(Ask::Again(old, typed), r)
            }
            Ask::Again(old, new) => {
                if typed != new {
                    return Errand::Say(texts.t(key::BELONGINGS_PASSPHRASES_DIFFER));
                }
                let written = write_secret(r, Places::passphrase, old.expose())
                    .and_then(|()| write_secret(r, Places::new_passphrase, new.expose()));
                match written {
                    Ok(()) => Errand::Queue(Op::Rekey),
                    Err(said) => Errand::Say(texts.t(said)),
                }
            }
        }
    }

    /// The line was let go — `Esc`, `C-g`, a click on `ESC` — and the
    /// question with it.
    pub fn let_go(&mut self) {
        self.asking = None;
    }

    /// Moves the cursor to the next row that acts, round the ends.
    fn walk(&mut self, rows: &[Row], down: bool) {
        let mut at = self.cursor;
        for _ in 0..rows.len() {
            at = if down {
                (at + 1) % rows.len().max(1)
            } else {
                at.checked_sub(1).unwrap_or(rows.len().saturating_sub(1))
            };
            if rows.get(at).is_some_and(|r| !r.heading()) {
                self.cursor = at;
                self.aim = rows[at].target.clone();
                return;
            }
        }
    }
}

/// Why a sync cannot start now — no repository, no access, a settings file
/// that does not parse, a sync prepared and waiting, a run going on — as its
/// sentence; `None` when it can.
fn unready(r: &Readings, texts: &ronoctua_i18n::Texts) -> Option<String> {
    if r.places.is_none() {
        return Some(texts.t(key::BELONGINGS_NO_PLACES));
    }
    if r.prefs_refused {
        return Some(texts.t(key::BELONGINGS_PREFS_REFUSED));
    }
    if !r.prefs.has_repository() {
        return Some(texts.t(words::refusal_key(Why::NoRepository)));
    }
    if !r.has_access() {
        return Some(texts.t(match r.prefs.auth {
            Auth::Ssh => key::BELONGINGS_NEEDS_KEY,
            Auth::Token => key::BELONGINGS_NEEDS_TOKEN,
        }));
    }
    in_the_way(r, texts)
}

/// What stands in the way of a new run: a sync prepared and waiting — a new
/// `prepare` would throw the person's decisions away — or a run that holds
/// the lock now.
fn in_the_way(r: &Readings, texts: &ronoctua_i18n::Texts) -> Option<String> {
    if r.plan.is_some() {
        return Some(texts.t(key::BELONGINGS_PLAN_WAITS));
    }
    r.busy.then(|| texts.t(words::refusal_key(Why::Busy)))
}

/// The passphrase into its runtime file, and the order after it.
fn send_passphrase(r: &Readings, typed: &Typed, texts: &ronoctua_i18n::Texts) -> Errand {
    match write_secret(r, Places::passphrase, typed.expose()) {
        Ok(()) => Errand::Queue(Op::Prepare),
        Err(said) => Errand::Say(texts.t(said)),
    }
}

/// The conflict of `path` in the plan that waits.
fn conflict_of<'a>(r: &'a Readings, path: &str) -> Option<&'a Conflict> {
    r.plan.as_ref()?.conflicts.iter().find(|c| c.path == path)
}

/// The period before `days`, round the list.
fn previous_period(days: Option<u16>) -> Option<u16> {
    let all = prefs::PERIODS;
    let at = all.iter().position(|p| *p == days).unwrap_or(0);
    all[(at + all.len() - 1) % all.len()]
}

/// The other access.
fn flip_auth(p: &mut Belongings) {
    p.auth = match p.auth {
        Auth::Ssh => Auth::Token,
        Auth::Token => Auth::Ssh,
    };
}

/// A path the person typed as a logical path: `~/notes`, `notes` (under the
/// home), or an absolute path under the home. `None` outside it.
#[must_use]
pub fn logical_of_typed(places: &Places, typed: &str) -> Option<String> {
    if typed.is_empty() {
        return None;
    }
    let path = if let Some(rest) = typed.strip_prefix("~/") {
        places.home.join(rest)
    } else if typed == "~" {
        places.home.clone()
    } else if Path::new(typed).is_absolute() {
        PathBuf::from(typed)
    } else {
        places.home.join(typed)
    };
    let clean = lexically_clean(&path)?;
    places.logical_of(&clean)
}

/// A path with `.` and `..` resolved by its words, never climbing past the
/// root — the file need not exist yet.
fn lexically_clean(path: &Path) -> Option<PathBuf> {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::RootDir => out.push("/"),
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    return None;
                }
            }
            Component::Normal(w) => out.push(w),
            Component::Prefix(_) => return None,
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------
// WRITES — the ones both doors make, so they are made once
// ---------------------------------------------------------------------------

/// Why a change to the settings was not written.
#[derive(Debug)]
pub enum NotChanged {
    /// No places.
    Nowhere,
    /// The file does not parse, and nothing was written over it.
    Unparsed,
    /// It could not be written.
    NotWritten(std::io::Error),
}

impl NotChanged {
    /// Its sentence, as a key and its arguments — what the file manager's
    /// line takes.
    #[must_use]
    pub fn explanation(&self) -> Explanation {
        match self {
            Self::Nowhere => said(key::BELONGINGS_NO_PLACES),
            Self::Unparsed => said(key::BELONGINGS_PREFS_REFUSED),
            Self::NotWritten(why) => Explanation {
                key: key::ECHO_SETTING_NOT_WRITTEN,
                args: vec![("file", prefs::FILE.to_owned()), ("why", why.to_string())],
            },
        }
    }

    /// Its sentence.
    #[must_use]
    pub fn sentence(&self, texts: &ronoctua_i18n::Texts) -> String {
        let e = self.explanation();
        texts.t_with(e.key, &e.borrowed())
    }
}

/// A sentence with no arguments.
const fn said(key: &'static str) -> Explanation {
    Explanation {
        key,
        args: Vec::new(),
    }
}

/// One change to `belongings.json`, read with `parsed` — never `read`: a
/// file that does not parse is refused, not replaced — laid over the file as
/// it is NOW, and written whole.
///
/// # Errors
///
/// A [`NotChanged`].
pub fn change_the_file(
    places: Option<&Places>,
    change: impl FnOnce(&mut Belongings),
) -> Result<(), NotChanged> {
    let places = places.ok_or(NotChanged::Nowhere)?;
    let store = store_of(places);
    let mut now = store.parsed().ok_or(NotChanged::Unparsed)?;
    change(&mut now);
    store.write(&now).map_err(NotChanged::NotWritten)
}

/// A secret into its runtime or access file, `0600` from birth.
fn write_secret(r: &Readings, at: fn(&Places) -> PathBuf, text: &str) -> Result<(), &'static str> {
    let places = r.places.as_ref().ok_or(key::BELONGINGS_NO_PLACES)?;
    ronoctua_belongings::secret::write_private(&at(places), text.as_bytes()).map_err(|e| {
        tracing::warn!(error = %e, "a belongings secret could not be written");
        key::BELONGINGS_NOT_WRITTEN
    })
}

/// The decisions into their file.
fn write_decisions_file(r: &Readings, decided: &Decisions) -> Result<(), &'static str> {
    let places = r.places.as_ref().ok_or(key::BELONGINGS_NO_PLACES)?;
    let text = serde_json::to_vec_pretty(decided).map_err(|_| key::BELONGINGS_NOT_WRITTEN)?;
    ronoctua_belongings::secret::write_private(&places.decisions(), &text).map_err(|e| {
        tracing::warn!(error = %e, "the belongings' decisions could not be written");
        key::BELONGINGS_NOT_WRITTEN
    })
}

/// The same, as an errand.
fn write_decisions(r: &Readings, decided: &Decisions, texts: &ronoctua_i18n::Texts) -> Errand {
    match write_decisions_file(r, decided) {
        Ok(()) => Errand::Nothing,
        Err(said) => Errand::Say(texts.t(said)),
    }
}

/// The order on the transfer queue — the one call both doors make. Nothing
/// secret crosses: the order names a verb. When the queue does not take it,
/// what the door typed for it is deleted at once ([`forget_unsent`]): nobody
/// else would.
///
/// # Errors
///
/// The queue is not there.
pub fn queue(op: Op, places: Option<&Places>) -> anyhow::Result<()> {
    let sent = ronoctua_transfers::client::tell(&ronoctua_transfers::Ask::Add {
        work: ronoctua_transfers::Work::Belongings { op },
    })
    .map(|_| ());
    if sent.is_err()
        && let Some(places) = places
    {
        forget_unsent(op, places);
    }
    sent
}

/// Deletes the secrets a door wrote for an order the queue did not take: the
/// passphrase of a sync, both of a change of passphrase. An apply's
/// passphrase is the waiting sync's, written before, and stays.
pub fn forget_unsent(op: Op, places: &Places) {
    let typed: Vec<PathBuf> = match op {
        Op::Prepare => vec![places.passphrase()],
        Op::Rekey => vec![places.passphrase(), places.new_passphrase()],
        Op::Apply | Op::Abandon | Op::Keygen => Vec::new(),
    };
    for file in typed {
        if let Err(e) = std::fs::remove_file(&file)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!(error = %e, "a belongings secret the queue did not take could not be deleted");
        }
    }
}

/// The sentence that says an order was taken.
#[must_use]
pub fn queued_said(op: Op, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_with(
        key::BELONGINGS_QUEUED,
        &[("what", &texts.t(words::op_key(op)))],
    )
}

/// The file manager's "keep in the belongings" (`C-m`, `k`): each path added
/// to what is kept — or, when all of them are kept already, taken out. The
/// sentence it comes to, for the manager's line.
#[must_use]
pub fn keep_from_the_files(paths: &[PathBuf]) -> Explanation {
    match Places::from_env() {
        Some(places) => keep_under(&places, paths),
        None => said(key::BELONGINGS_NO_PLACES),
    }
}

/// [`keep_from_the_files`], under `places` — what a test hands in.
#[must_use]
pub fn keep_under(places: &Places, paths: &[PathBuf]) -> Explanation {
    let logical: Vec<String> = paths
        .iter()
        .filter_map(|p| lexically_clean(p).and_then(|c| places.logical_of(&c)))
        .collect();
    if logical.is_empty() || logical.len() != paths.len() {
        return said(key::BELONGINGS_OUTSIDE_HOME);
    }
    if logical.iter().any(|l| !kept::may_add(l)) {
        return said(key::BELONGINGS_NEVER_KEPT);
    }
    let Some(prefs) = store_of(places).parsed() else {
        return said(key::BELONGINGS_PREFS_REFUSED);
    };
    // WHAT ANOTHER MACHINE ADDED, followed here, is kept here too.
    let followed: Vec<String> = Last::read(&places.last())
        .filter(|l| l.repository == prefs.repository)
        .map(|l| l.roots.into_iter().collect())
        .unwrap_or_default();
    // ALL KEPT ALREADY: the same gesture takes them out — one row of the
    // menu, and what it did is said.
    let all_kept = logical
        .iter()
        .all(|l| kept::kept_here(l, &prefs, followed.iter().any(|f| covers(f, l))));
    let changed = change_the_file(Some(places), |p| {
        for l in &logical {
            if all_kept {
                p.stop_keeping(l);
            } else {
                p.keep(l);
            }
        }
    });
    if let Err(why) = changed {
        return why.explanation();
    }
    let name = if logical.len() == 1 {
        shown_path(&logical[0], Some(places), false)
    } else {
        logical.len().to_string()
    };
    Explanation {
        key: if all_kept {
            key::BELONGINGS_STOPPED_KEEPING
        } else {
            key::BELONGINGS_NOW_KEPT
        },
        args: vec![("name", name)],
    }
}

// ---------------------------------------------------------------------------
// THE SECTIONS
// ---------------------------------------------------------------------------

/// Status: the repository, the last sync, the next reminder, the last run.
fn status_section(b: &mut Builder<'_>, r: &Readings, zone: &ronoctua_files::when::Zone) {
    let t = b.texts;
    let mut e = Vec::new();
    let repository = if r.prefs.has_repository() {
        r.prefs.repository.clone()
    } else {
        t.t(key::BELONGINGS_NONE_YET)
    };
    e.push(pair(
        t.t(key::BELONGINGS_REPOSITORY),
        repository,
        Target::Repository,
        true,
    ));
    if r.prefs_refused {
        e.push(Entry::Note(
            t.t(key::BELONGINGS_PREFS_REFUSED),
            tokens::color::STATE_WARNING,
        ));
    }
    let last = r
        .last
        .as_ref()
        .filter(|l| l.repository == r.prefs.repository);
    e.push(pair(
        t.t(key::BELONGINGS_LAST_SYNC),
        last.map_or_else(
            || t.t(key::BELONGINGS_NEVER_HERE),
            |l| when(l.at_s, zone, t),
        ),
        Target::Fact("last".to_owned()),
        false,
    ));
    let next = match (
        r.prefs.has_repository(),
        ronoctua_belongings::status::due_at(last.map(|l| l.at_s), r.prefs.period_days),
    ) {
        (false, _) => None,
        (true, Some(at)) if at <= r.now_s => Some(t.t(key::BELONGINGS_DUE_NOW)),
        (true, Some(at)) => Some(when(at, zone, t)),
        (true, None) if r.prefs.period_days.is_none() => Some(t.t(key::BELONGINGS_NO_REMINDER)),
        (true, None) => Some(t.t(key::BELONGINGS_DUE_NOW)),
    };
    if let Some(next) = next {
        e.push(pair(
            t.t(key::BELONGINGS_NEXT),
            next,
            Target::Fact("next".to_owned()),
            false,
        ));
    }
    if let Some(l) = last {
        e.push(pair(
            t.t(key::BELONGINGS_KEPT_NOW),
            t.t_args(
                key::BELONGINGS_FILES_WEIGH,
                &[
                    (
                        "n",
                        ronoctua_i18n::Arg::Number(i64::try_from(l.files).unwrap_or(i64::MAX)),
                    ),
                    ("size", ronoctua_i18n::Arg::Text(&bytes_text(l.bytes, t))),
                ],
            ),
            Target::Fact("kept".to_owned()),
            false,
        ));
    }
    if let Some(s) = r.status.as_ref() {
        // A SYNC THAT WAITED, ITS SESSION GONE — the runtime folder is emptied
        // when the person's session ends — waits for nothing any more.
        let said = match s.outcome {
            Outcome::Waiting { .. } if r.plan.is_none() => t.t(key::BELONGINGS_PREPARED_GONE),
            _ => outcome_text(&s.outcome, t),
        };
        if s.failed() {
            e.push(tinted(
                t.t(key::BELONGINGS_LAST_RUN),
                when(s.at_s, zone, t),
                Target::Fact("run".to_owned()),
                false,
                tokens::color::STATE_WARNING,
            ));
            e.push(Entry::Note(said, tokens::color::STATE_WARNING));
        } else {
            e.push(pair(
                t.t(key::BELONGINGS_LAST_RUN),
                when(s.at_s, zone, t),
                Target::Fact("run".to_owned()),
                false,
            ));
            e.push(note(said));
        }
        if let Outcome::Synced {
            large,
            unreadable,
            unwritten,
            ..
        } = &s.outcome
        {
            for (path, bytes) in large {
                e.push(Entry::Note(
                    t.t_with(
                        key::BELONGINGS_LARGE,
                        &[
                            ("name", &shown_path(path, r.places.as_ref(), false)),
                            ("size", &bytes_text(*bytes, t)),
                        ],
                    ),
                    tokens::color::STATE_WARNING,
                ));
            }
            if !unreadable.is_empty() {
                e.push(Entry::Note(
                    t.t_args(
                        key::BELONGINGS_UNREADABLE,
                        &[("n", count(unreadable.len()))],
                    ),
                    tokens::color::STATE_WARNING,
                ));
            }
            if !unwritten.is_empty() {
                e.push(Entry::Note(
                    t.t_args(key::BELONGINGS_UNWRITTEN, &[("n", count(unwritten.len()))]),
                    tokens::color::STATE_WARNING,
                ));
            }
        }
    }
    if r.first_run() {
        e.push(note(t.t(key::BELONGINGS_FIRST_RUN)));
    }
    b.section(&t.t(key::BELONGINGS_STATUS), e);
}

/// A count as a Fluent number.
fn count(n: usize) -> ronoctua_i18n::Arg<'static> {
    ronoctua_i18n::Arg::Number(i64::try_from(n).unwrap_or(i64::MAX))
}

/// Sync: now, or — while a sync waits — apply and drop.
fn sync_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let mut e = Vec::new();
    match r.plan.as_ref() {
        Some(plan) => {
            let decided = r.decided();
            let done = plan
                .conflicts
                .iter()
                .filter(|c| decided.get(&c.path).is_some_and(|d| d.allowed(c)))
                .count();
            e.push(pair(
                t.t(key::BELONGINGS_APPLY),
                t.t_args(
                    key::BELONGINGS_DECIDED_OF,
                    &[("n", count(done)), ("of", count(plan.conflicts.len()))],
                ),
                Target::Apply,
                true,
            ));
            e.push(pair(
                t.t(key::BELONGINGS_ABANDON),
                String::new(),
                Target::Abandon,
                true,
            ));
            e.push(note(t.t(key::BELONGINGS_WAITING_NOTE)));
            if r.busy {
                e.push(note(t.t(words::refusal_key(Why::Busy))));
            }
        }
        None => {
            e.push(pair(
                t.t(key::BELONGINGS_SYNC_NOW),
                String::new(),
                Target::Sync,
                true,
            ));
            if let Some(said) = unready(r, t) {
                e.push(note(said));
            }
        }
    }
    b.section(&t.t(key::BELONGINGS_SYNC_HEADING), e);
}

/// Conflicts: every one, with both sides, and its three decisions — and the
/// two in bulk.
fn conflicts_section(b: &mut Builder<'_>, r: &Readings, zone: &ronoctua_files::when::Zone) {
    let t = b.texts;
    let Some(plan) = r.plan.as_ref() else {
        return;
    };
    let mut e = vec![
        note(t.t(key::BELONGINGS_CONFLICTS_NOTE)),
        pair(
            t.t(key::BELONGINGS_ALL_KEEP_LOCAL),
            String::new(),
            Target::All(Decision::KeepLocal),
            true,
        ),
        pair(
            t.t(key::BELONGINGS_ALL_TAKE_REMOTE),
            String::new(),
            Target::All(Decision::TakeRemote),
            true,
        ),
    ];
    for c in &plan.conflicts {
        // AIR BETWEEN TWO CONFLICTS: each is a path, its two sides and its
        // decisions, read as one.
        e.push(Entry::Air);
        let decision = r.decision_of(c);
        let value = decision.map_or_else(
            || t.t(key::BELONGINGS_UNDECIDED),
            |d| t.t(words::decided_key(d)),
        );
        let label = shown_path(&c.path, r.places.as_ref(), false);
        e.push(match decision {
            Some(_) => pair(label, value, Target::Conflict(c.path.clone()), true),
            None => tinted(
                label,
                value,
                Target::Conflict(c.path.clone()),
                true,
                tokens::color::STATE_WARNING,
            ),
        });
        e.push(note(t.t_with(
            key::BELONGINGS_SIDES,
            &[
                ("here", &side_text(c.local.as_ref(), zone, t)),
                ("there", &side_text(c.remote.as_ref(), zone, t)),
                ("machine", &c.machine),
            ],
        )));
        for d in Decision::ALL.into_iter().filter(|d| d.allowed(c)) {
            let chosen = decision == Some(d);
            e.push(indented(
                t.t(words::decide_key(d)),
                if chosen {
                    t.t(key::BELONGINGS_CHOSEN)
                } else {
                    String::new()
                },
                Target::Decide(c.path.clone(), d),
            ));
        }
    }
    b.section(
        &t.t_args(
            key::BELONGINGS_CONFLICTS,
            &[("n", count(plan.conflicts.len()))],
        ),
        e,
    );
}

/// The difference of one conflicting text: its decisions first, then its
/// lines, in the diff's colours and the file's own.
fn diff_page(b: &mut Builder<'_>, r: &Readings, path: &str, lines: &[String]) {
    let t = b.texts;
    let places = r.places.as_ref();
    let mut e = vec![note(t.t(key::BELONGINGS_DIFF_NOTE))];
    if let Some(c) = conflict_of(r, path) {
        let decision = r.decision_of(c);
        for d in Decision::ALL.into_iter().filter(|d| d.allowed(c)) {
            e.push(pair(
                t.t(words::decide_key(d)),
                if decision == Some(d) {
                    t.t(key::BELONGINGS_CHOSEN)
                } else {
                    String::new()
                },
                Target::Decide(path.to_owned(), d),
                true,
            ));
        }
    }
    let language = language_of(path, lines);
    let drawn: Vec<(Vec<Field>, Target)> = lines
        .iter()
        .enumerate()
        .map(|(n, line)| {
            (
                diff_fields(line, language, b.left, b.room),
                Target::DiffLine(path.to_owned(), n),
            )
        })
        .collect();
    e.push(Entry::Lines(drawn));
    b.section(
        &t.t_with(
            key::BELONGINGS_DIFF_TITLE,
            &[("name", &shown_path(path, places, false))],
        ),
        e,
    );
}

/// The file's own language, for the unchanged lines: by its extension as a
/// fence would name it, else by what its text looks like.
fn language_of(path: &str, lines: &[String]) -> Option<ronoctua_syntax::Language> {
    let extension = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
    extension
        .as_deref()
        .and_then(ronoctua_syntax::of_tag)
        .or_else(|| {
            let body: String = lines
                .iter()
                .filter(|l| l.starts_with(' '))
                .map(|l| &l[1..])
                .collect::<Vec<_>>()
                .join("\n");
            ronoctua_syntax::guess(&body)
        })
}

/// One line of a difference as fields: a line that adds or removes, and the
/// headings, in the diff's own roles, whole; an unchanged line in the file's
/// language. Never past `room`.
fn diff_fields(
    line: &str,
    language: Option<ronoctua_syntax::Language>,
    left: u16,
    room: u16,
) -> Vec<Field> {
    let text = layout::elide_end(line, room);
    let changed = line.starts_with('+') || line.starts_with('-') || line.starts_with('@');
    // THE MARK OF AN UNCHANGED LINE is the diff's, dim; what follows it is the
    // file's own text.
    let (grammar, body_from) = if changed {
        (ronoctua_syntax::of_tag("diff"), 0)
    } else if text.starts_with(' ') {
        (language, 1)
    } else {
        (language, 0)
    };
    let ranges = grammar.map_or_else(Vec::new, |g| {
        ronoctua_syntax::line(
            &text[body_from..],
            g.grammar(),
            ronoctua_syntax::State::fresh(),
        )
        .0
    });
    let mut out = Vec::new();
    let mut at = left;
    let mut push = |piece_text: &str, color: tokens::Color, out: &mut Vec<Field>| {
        if piece_text.is_empty() {
            return;
        }
        out.push(Field {
            at,
            text: piece_text.to_owned(),
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        });
        at += layout::width(piece_text);
    };
    let plain = tokens::color::TEXT_SECONDARY;
    push(&text[..body_from], tokens::color::TEXT_DIM, &mut out);
    let body = &text[body_from..];
    let mut from = 0;
    for range in ranges {
        if range.start > from {
            push(&body[from..range.start], plain, &mut out);
        }
        push(
            &body[range.start..range.end],
            crate::chrome::syntax_color(range.role),
            &mut out,
        );
        from = range.end;
    }
    if from < body.len() {
        push(&body[from..], plain, &mut out);
    }
    out
}

/// What is kept: the table, each with its reason; what the person added and
/// took out; what is never kept.
fn kept_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let places = r.places.as_ref();
    let reason = |id: &str| t.t(&words::reason_key(id));
    let mut e = vec![note(t.t(key::BELONGINGS_WHAT_LEAKS))];
    for (place, folder) in kept::TREES
        .iter()
        .map(|p| (p, true))
        .chain(kept::FILES.iter().map(|p| (p, false)))
    {
        let label = shown_path(place.path, places, folder);
        if r.prefs.excluded.iter().any(|x| x == place.path) {
            e.push(pair(
                label,
                t.t(key::BELONGINGS_TAKEN_OUT),
                Target::Excluded(place.path.to_owned()),
                true,
            ));
        } else {
            e.push(pair(
                label,
                reason(place.reason),
                Target::Rule(place.path.to_owned()),
                true,
            ));
        }
    }
    for extra in &r.prefs.extra {
        e.push(pair(
            shown_path(extra, places, false),
            t.t(key::BELONGINGS_ADDED_BY_YOU),
            Target::Kept(extra.clone()),
            true,
        ));
    }
    // WHAT ANOTHER MACHINE ADDED, and this one follows: taken out here by the
    // same row, which takes it out of the repository for every machine.
    for root in r
        .followed
        .iter()
        .filter(|f| !r.prefs.extra.contains(f) && !r.prefs.excluded.contains(f))
    {
        e.push(pair(
            shown_path(root, places, false),
            t.t(key::BELONGINGS_ADDED_ELSEWHERE),
            Target::Kept(root.clone()),
            true,
        ));
    }
    for excluded in r.prefs.excluded.iter().filter(|x| {
        !kept::TREES
            .iter()
            .chain(kept::FILES)
            .any(|p| p.path == x.as_str())
    }) {
        e.push(pair(
            shown_path(excluded, places, false),
            t.t(key::BELONGINGS_TAKEN_OUT),
            Target::Excluded(excluded.clone()),
            true,
        ));
    }
    e.push(pair(
        t.t(key::BELONGINGS_ADD),
        String::new(),
        Target::Add,
        true,
    ));
    e.push(note(t.t_with(
        key::BELONGINGS_LARGE_NOTE,
        &[("size", &bytes_text(kept::LARGE_BYTES, t))],
    )));
    b.section(&t.t(key::BELONGINGS_KEPT), e);

    // NEVER KEPT: the rules that take things out, with their reasons. A
    // hard one is a fact; the rest a person may add anyway.
    let mut e = Vec::new();
    for never in kept::NEVER {
        let (label, target) = match never.pattern {
            Pattern::Name(name) => (name.to_owned(), Target::Fact(format!("never:{name}"))),
            Pattern::Path(path) => {
                let added = r.prefs.extra.iter().any(|x| covers(x, path));
                if added {
                    continue;
                }
                let target = if never.hard {
                    Target::Fact(format!("never:{path}"))
                } else {
                    Target::Never(path.to_owned())
                };
                (shown_path(path, places, is_folder(places, path)), target)
            }
        };
        let acts = matches!(target, Target::Never(_));
        e.push(pair(label, reason(never.reason), target, acts));
    }
    b.section(&t.t(key::BELONGINGS_NEVER), e);
}

/// Whether a logical path is a folder: one on this machine, or — when it is
/// not here — one whose last name has no extension (`syncthing`, never
/// `belongings.json`; a dot that begins a name is not an extension).
fn is_folder(places: Option<&Places>, logical: &str) -> bool {
    if let Some(at) = places.and_then(|p| p.resolve(logical))
        && std::fs::symlink_metadata(&at).is_ok()
    {
        return at.is_dir();
    }
    Root::split(logical).is_some_and(|(_, rest)| {
        rest.rsplit('/')
            .next()
            .is_some_and(|name| !name.trim_start_matches('.').contains('.'))
    })
}

/// Access: how the repository is reached — this machine's deploy key, or a
/// token.
fn access_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let mut e = vec![pair(
        t.t(key::BELONGINGS_AUTH),
        t.t(words::auth_key(r.prefs.auth)),
        Target::Auth,
        true,
    )];
    match r.prefs.auth {
        Auth::Ssh => {
            if r.has_key {
                e.push(pair(
                    t.t(key::BELONGINGS_DEPLOY_KEY),
                    t.t(key::BELONGINGS_COPY_IT),
                    Target::CopyKey,
                    true,
                ));
                if let Some(public) = r.public_key.as_ref() {
                    e.push(Entry::Note(public.clone(), tokens::color::TEXT_SECONDARY));
                }
                e.push(note(t.t(key::BELONGINGS_DEPLOY_NOTE)));
            } else {
                e.push(pair(
                    t.t(key::BELONGINGS_DEPLOY_KEY),
                    t.t(key::BELONGINGS_MAKE_IT),
                    Target::Keygen,
                    true,
                ));
                e.push(note(t.t(key::BELONGINGS_KEYGEN_NOTE)));
            }
        }
        Auth::Token => {
            e.push(pair(
                t.t(key::BELONGINGS_TOKEN),
                t.t(if r.has_token {
                    key::BELONGINGS_TOKEN_IS_KEPT
                } else {
                    key::BELONGINGS_NONE_YET
                }),
                Target::Token,
                true,
            ));
            e.push(pair(
                t.t(key::BELONGINGS_USER),
                r.prefs
                    .user
                    .clone()
                    .filter(|u| !u.is_empty())
                    .unwrap_or_else(|| t.t(key::BELONGINGS_NONE_YET)),
                Target::User,
                true,
            ));
            e.push(note(t.t(key::BELONGINGS_TOKEN_NOTE)));
        }
    }
    b.section(&t.t(key::BELONGINGS_ACCESS), e);
}

/// The passphrase: change it.
fn passphrase_section(b: &mut Builder<'_>) {
    let t = b.texts;
    let e = vec![
        pair(
            t.t(key::BELONGINGS_CHANGE_PASSPHRASE),
            String::new(),
            Target::Passphrase,
            true,
        ),
        note(t.t(key::BELONGINGS_PASSPHRASE_NOTE)),
    ];
    b.section(&t.t(key::BELONGINGS_PASSPHRASE), e);
}

/// How often the brain reminds.
fn period_section(b: &mut Builder<'_>, r: &Readings) {
    let t = b.texts;
    let e = vec![pair(
        t.t(key::BELONGINGS_REMIND_EVERY),
        words::period_text(r.prefs.period_days, t),
        Target::Period,
        true,
    )];
    b.section(&t.t(key::BELONGINGS_PERIOD), e);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_belongings::plan::Kind as PathKind;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a declared language")
    }

    fn zone() -> ronoctua_files::when::Zone {
        ronoctua_files::when::Zone::utc()
    }

    /// A scratch home, empty.
    fn scratch(name: &str) -> Places {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-shell-belongings-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let places = Places::under(&root);
        std::fs::create_dir_all(&places.config).expect("config");
        std::fs::create_dir_all(&places.home).expect("home");
        places
    }

    fn side(size: u64) -> Side {
        Side {
            size_bytes: size,
            modified_s: 1_700_000_000,
            sha256: "x".into(),
            kind: PathKind::File,
            mode: 0o644,
        }
    }

    /// A machine with a repository, a key, and a sync waiting on two
    /// conflicts — one of them a path the person added.
    fn waiting(places: &Places) -> Readings {
        let prefs = Belongings {
            repository: "git@example.org:someone/belongings.git".into(),
            extra: vec!["home/notes".into()],
            ..Belongings::default()
        };
        store_of(places).write(&prefs).expect("prefs");
        std::fs::create_dir_all(places.keys()).expect("keys");
        std::fs::write(places.deploy_key(), "k").expect("key");
        std::fs::write(
            places.deploy_public(),
            "ssh-ed25519 AAAA ronoctua-belongings\n",
        )
        .expect("public");
        let plan = Plan {
            remote_commit: Some("abc".into()),
            conflicts: vec![
                Conflict {
                    path: "config/theme.json".into(),
                    local: Some(side(40)),
                    remote: Some(side(42)),
                    machine: "laptop".into(),
                    text: true,
                    theirs: Some("0".into()),
                    ..Conflict::default()
                },
                Conflict {
                    path: "home/notes/todo.md".into(),
                    local: Some(side(10)),
                    remote: None,
                    machine: "laptop".into(),
                    added_by_person: true,
                    ..Conflict::default()
                },
            ],
            ..Plan::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.plan(),
            &serde_json::to_vec(&plan).expect("plan"),
        )
        .expect("plan written");
        ronoctua_belongings::secret::write_private(
            &places.theirs("0.diff"),
            b"@@ -1 +1 @@\n-{\"theme\":\"paper\"}\n+{\"theme\":\"owl\"}\n",
        )
        .expect("diff");
        Readings::read(Some(places.clone()), 1_700_000_500)
    }

    fn drawn(rows: &[Row]) -> String {
        rows.iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    fn stand_on(v: &mut View, r: &Readings, target: &Target, t: &ronoctua_i18n::Texts) {
        let rows = v.rows(r, &zone(), 100, t);
        let n = rows
            .iter()
            .position(|row| row.target.as_ref() == Some(target))
            .unwrap_or_else(|| panic!("no row is {target:?}: {}", drawn(&rows)));
        v.clicked(n, &rows);
    }

    /// FAILS IF: a section of the design is missing — status, sync, what is
    /// kept, never kept, access, passphrase, period — a word draws as its raw
    /// key in a language, or the cursor is born on a title.
    #[test]
    fn every_section_is_drawn_in_every_language() {
        let places = scratch("sections");
        let r = Readings::read(Some(places.clone()), 1_700_000_000);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let v = View::fresh(&r, &zone(), 100, &t);
            let rows = v.rows(&r, &zone(), 100, &t);
            let text = drawn(&rows);
            for heading in [
                key::BELONGINGS_STATUS,
                key::BELONGINGS_SYNC_HEADING,
                key::BELONGINGS_KEPT,
                key::BELONGINGS_NEVER,
                key::BELONGINGS_ACCESS,
                key::BELONGINGS_PASSPHRASE,
                key::BELONGINGS_PERIOD,
            ] {
                assert!(
                    text.contains(&t.t(heading)),
                    "{language}: no {heading}: {text}"
                );
            }
            assert!(!text.contains('['), "{language}: a raw key: {text}");
            assert!(
                !rows[v.cursor()].heading(),
                "the cursor was born on a title"
            );
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a waiting sync does not list EVERY conflict with both sides
    /// and the machine, offers "keep both" on a path the person did not add,
    /// or a decision, one by one or in bulk, is not written where the CLI
    /// reads it.
    #[test]
    fn every_conflict_is_listed_and_every_decision_is_written() {
        let places = scratch("conflicts");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        let text = drawn(&rows);
        assert!(
            text.contains("theme.json") && text.contains("todo.md"),
            "{text}"
        );
        assert!(text.contains("laptop"), "the machine is not said: {text}");
        let both: Vec<&Target> = rows
            .iter()
            .filter_map(|r| r.target.as_ref())
            .filter(|t| matches!(t, Target::Decide(_, Decision::KeepBoth)))
            .collect();
        assert_eq!(
            both,
            [&Target::Decide(
                "home/notes/todo.md".into(),
                Decision::KeepBoth
            )],
            "keep both on a path the person did not add"
        );
        // ONE BY ONE, by the row and by the key.
        stand_on(
            &mut v,
            &r,
            &Target::Decide("config/theme.json".into(), Decision::TakeRemote),
            &t,
        );
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            r.decisions.get("config/theme.json"),
            Some(&Decision::TakeRemote)
        );
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("home/notes/todo.md".into()),
            &t,
        );
        v.press(Press::Decide(Decision::KeepBoth), &r, &zone(), 100, &t);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(r.decisions.len(), 2, "{:?}", r.decisions);
        // `C-b` ON A PATH THE PERSON DID NOT ADD is refused, with its words.
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("config/theme.json".into()),
            &t,
        );
        assert_eq!(
            v.press(Press::Decide(Decision::KeepBoth), &r, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_BOTH_ONLY_ADDED))
        );
        // IN BULK, over what was decided.
        stand_on(&mut v, &r, &Target::All(Decision::KeepLocal), &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert!(r.decisions.values().all(|d| *d == Decision::KeepLocal));
        let mode = std::os::unix::fs::PermissionsExt::mode(
            &std::fs::metadata(places.decisions()).unwrap().permissions(),
        );
        assert_eq!(mode & 0o777, 0o600, "the decisions were born readable");
        // AND APPLY SENDS THE ORDER, the decisions written first.
        stand_on(&mut v, &r, &Target::Apply, &t);
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Queue(Op::Apply)
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: apply is sent with a conflict undecided, or the sentence
    /// does not say how many are left.
    #[test]
    fn apply_is_refused_while_a_conflict_is_undecided() {
        let places = scratch("undecided");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &Target::Apply, &t);
        match v.press(Press::Enter, &r, &zone(), 100, &t) {
            Errand::Say(said) => assert!(said.contains('2'), "{said}"),
            other => panic!("apply with nothing decided did {other:?}"),
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: `RET` on a conflicting text does not open its difference —
    /// the decisions first, the lines in the diff's colours — or `Esc` there
    /// closes the view instead of going back to the conflict.
    #[test]
    fn a_text_shows_its_difference_and_esc_goes_back_to_it() {
        let places = scratch("diff");
        let r = waiting(&places);
        let t = texts("pt-BR");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("config/theme.json".into()),
            &t,
        );
        assert_eq!(v.press(Press::Enter, &r, &zone(), 100, &t), Errand::Nothing);
        let rows = v.rows(&r, &zone(), 100, &t);
        let text = drawn(&rows);
        assert!(text.contains("owl") && text.contains("paper"), "{text}");
        let added = rows
            .iter()
            .flat_map(|r| &r.fields)
            .find(|f| f.text.contains("owl"))
            .expect("the added line");
        assert_eq!(added.color, tokens::color::SYNTAX_ADDED);
        assert!(matches!(
            rows[v.cursor()].target,
            Some(Target::Decide(_, _))
        ));
        // A DECISION ON THIS PAGE is the conflict's.
        assert_eq!(
            v.press(Press::Decide(Decision::KeepLocal), &r, &zone(), 100, &t),
            Errand::Nothing
        );
        assert_eq!(
            v.press(Press::Escape, &r, &zone(), 100, &t),
            Errand::Nothing
        );
        let rows = v.rows(&r, &zone(), 100, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Some(Target::Conflict("config/theme.json".into()))
        );
        assert_eq!(v.press(Press::Escape, &r, &zone(), 100, &t), Errand::Close);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: on a LINE of a difference the decisions the hint teaches are
    /// refused — a taught key works where it is taught — or keeping both is
    /// taught on a conflict that may not keep both.
    #[test]
    fn a_line_of_a_difference_decides_its_conflict() {
        let places = scratch("diff-line");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::Conflict("config/theme.json".into()),
            &t,
        );
        v.press(Press::Enter, &r, &zone(), 100, &t);
        stand_on(
            &mut v,
            &r,
            &Target::DiffLine("config/theme.json".into(), 1),
            &t,
        );
        let taught: Vec<&str> = v
            .hint(&r, &zone(), 100, &t)
            .iter()
            .filter_map(|p| p.id)
            .collect();
        assert!(
            taught.contains(&"belongings-keep-local") && taught.contains(&"belongings-take-remote"),
            "{taught:?}"
        );
        assert!(
            !taught.contains(&"belongings-keep-both"),
            "keep both taught on the configuration: {taught:?}"
        );
        // AND THE CURSOR SHOWS on it: the accent a label takes.
        let rows = v.rows(&r, &zone(), 100, &t);
        assert!(
            rows[v.cursor()]
                .fields
                .iter()
                .all(|f| f.color == crate::chrome::Kind::Setting.tones(true).0),
            "the cursor on a line of the difference is not drawn"
        );
        assert_eq!(
            v.press(Press::Decide(Decision::TakeRemote), &r, &zone(), 100, &t),
            Errand::Nothing
        );
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            r.decisions.get("config/theme.json"),
            Some(&Decision::TakeRemote)
        );
        // `RET` ON IT copies the line, whole.
        assert_eq!(
            v.press(Press::Enter, &r, &zone(), 100, &t),
            Errand::Copy("-{\"theme\":\"paper\"}".to_owned())
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a new sync, or a change of passphrase, starts while a sync
    /// waits for decisions — its `prepare` would throw them away — or while a
    /// run holds the lock; or apply and drop go while one runs.
    #[test]
    fn nothing_new_starts_over_a_waiting_or_a_running_sync() {
        let places = scratch("in-the-way");
        let r = waiting(&places);
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let waits = Errand::Say(t.t(key::BELONGINGS_PLAN_WAITS));
        assert_eq!(v.press(Press::Sync, &r, &zone(), 100, &t), waits);
        assert_eq!(v.command(SYNC_COMMAND, &r, &zone(), 100, &t), waits);
        let summary = Last {
            repository: r.prefs.repository.clone(),
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&summary).unwrap(),
        )
        .unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(
            v.command("belongings-passphrase", &r, &zone(), 100, &t),
            waits
        );
        // A RUN HOLDS THE LOCK: apply and drop wait for it too.
        let held = ronoctua_belongings::running::hold(&places).unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert!(r.busy);
        let busy = Errand::Say(t.t(words::refusal_key(Why::Busy)));
        let decided: Decisions = r.plan.as_ref().unwrap().all(Decision::KeepLocal);
        write_decisions_file(&r, &decided).unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(v.command("belongings-apply", &r, &zone(), 100, &t), busy);
        assert_eq!(v.command("belongings-abandon", &r, &zone(), 100, &t), busy);
        // AND WITH NOTHING WAITING, a sync is refused as busy.
        std::fs::remove_dir_all(places.session()).unwrap();
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert_eq!(v.press(Press::Sync, &r, &zone(), 100, &t), busy);
        drop(held);
        let r = Readings::read(Some(places.clone()), r.now_s);
        assert!(!r.busy);
        assert!(matches!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Ask(_)
        ));
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a status that says a sync waits is drawn as waiting once its
    /// session is gone (a reboot empties the runtime folder).
    #[test]
    fn a_waiting_sync_whose_session_is_gone_says_so() {
        let places = scratch("gone");
        let r = waiting(&places);
        let status = Status {
            at_s: 1_700_000_100,
            outcome: Outcome::Waiting { conflicts: 2 },
        };
        ronoctua_belongings::secret::write_private(
            &places.status(),
            &serde_json::to_vec(&status).unwrap(),
        )
        .unwrap();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = Readings::read(Some(places.clone()), r.now_s);
            // A NOTE WRAPS at a space: the rows joined by one space read whole.
            let text = drawn(&View::fresh(&r, &zone(), 100, &t).rows(&r, &zone(), 100, &t))
                .replace(" | ", " ");
            assert!(text.contains(&outcome_text(&status.outcome, &t)), "{text}");
        }
        std::fs::remove_dir_all(places.session()).unwrap();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let r = Readings::read(Some(places.clone()), r.now_s);
            let text = drawn(&View::fresh(&r, &zone(), 100, &t).rows(&r, &zone(), 100, &t))
                .replace(" | ", " ");
            assert!(
                !text.contains(&outcome_text(&status.outcome, &t)),
                "{language}: still waiting: {text}"
            );
            assert!(text.contains(&t.t(key::BELONGINGS_PREPARED_GONE)), "{text}");
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: what a door typed for an order the queue did not take stays
    /// in the runtime folder, or an apply's — the waiting sync's — goes.
    #[test]
    fn an_order_the_queue_did_not_take_leaves_no_secret() {
        let places = scratch("unsent");
        let write = |at: &Path| ronoctua_belongings::secret::write_private(at, b"x").unwrap();
        write(&places.passphrase());
        forget_unsent(Op::Apply, &places);
        assert!(
            places.passphrase().exists(),
            "the waiting sync's passphrase went"
        );
        forget_unsent(Op::Prepare, &places);
        assert!(!places.passphrase().exists());
        write(&places.passphrase());
        write(&places.new_passphrase());
        forget_unsent(Op::Rekey, &places);
        assert!(!places.passphrase().exists() && !places.new_passphrase().exists());
        forget_unsent(Op::Rekey, &places);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a folder another machine added, which this one follows, is
    /// not shown with what is kept, or its row does not take it out — or
    /// keeping a path the table keeps turns it into "added by you".
    #[test]
    fn what_another_machine_added_is_shown_and_taken_out_here() {
        let places = scratch("followed");
        let prefs = Belongings::with_repository("git@example.org:a/b.git");
        store_of(&places).write(&prefs).unwrap();
        let last = Last {
            repository: prefs.repository.clone(),
            roots: ["home/notes".to_owned()].into_iter().collect(),
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&last).unwrap(),
        )
        .unwrap();
        let r = Readings::read(Some(places.clone()), 1);
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let text = drawn(&View::fresh(&r, &zone(), 100, &t).rows(&r, &zone(), 100, &t));
            assert!(
                text.contains("~/notes") && text.contains(&t.t(key::BELONGINGS_ADDED_ELSEWHERE)),
                "{language}: {text}"
            );
        }
        let t = texts("en-GB");
        let mut v = View::fresh(&r, &zone(), 100, &t);
        stand_on(&mut v, &r, &Target::Kept("home/notes".into()), &t);
        v.press(Press::Enter, &r, &zone(), 100, &t);
        assert_eq!(store_of(&places).read().excluded, ["home/notes"]);
        // THE FILE MANAGER SEES IT KEPT, and its gesture is the same.
        let notes = places.home.join("notes");
        store_of(&places).write(&prefs).unwrap();
        assert_eq!(
            keep_under(&places, std::slice::from_ref(&notes)).key,
            key::BELONGINGS_STOPPED_KEEPING
        );
        // A PATH THE TABLE KEEPS, kept again, is never an addition.
        let theme = places.config.join("theme.json");
        assert_eq!(
            keep_under(&places, std::slice::from_ref(&theme)).key,
            key::BELONGINGS_STOPPED_KEEPING
        );
        assert_eq!(
            keep_under(&places, std::slice::from_ref(&theme)).key,
            key::BELONGINGS_NOW_KEPT
        );
        let now = store_of(&places).read();
        assert!(now.extra.is_empty(), "{now:?}");
        assert!(!now.excluded.contains(&"config/theme.json".to_owned()));
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a sync is started with no repository or no access instead of
    /// saying why, the passphrase is not asked hidden, or its answer is not
    /// written `0600` before the order goes to the queue.
    #[test]
    fn a_sync_asks_the_passphrase_hidden_and_writes_it_private() {
        let places = scratch("sync");
        let t = texts("en-GB");
        let r = Readings::read(Some(places.clone()), 1);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        assert!(matches!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Say(_)
        ));
        store_of(&places)
            .write(&Belongings::with_repository("git@example.org:a/b.git"))
            .unwrap();
        let r = Readings::read(Some(places.clone()), 1);
        assert_eq!(
            v.press(Press::Sync, &r, &zone(), 100, &t),
            Errand::Say(t.t(key::BELONGINGS_NEEDS_KEY)),
            "a sync with no deploy key"
        );
        std::fs::create_dir_all(places.keys()).unwrap();
        std::fs::write(places.deploy_key(), "k").unwrap();
        let r = Readings::read(Some(places.clone()), 1);
        assert!(r.first_run());
        let Errand::Ask(line) = v.press(Press::Sync, &r, &zone(), 100, &t) else {
            panic!("the passphrase was not asked");
        };
        assert!(line.secret);
        assert_eq!(line.id, SYNC_COMMAND);
        // A FIRST SYNC ASKS IT TWICE — an empty repository keeps it for good —
        // and two that differ send nothing.
        let Errand::Ask(again) = v.answered(line.id, "correct horse", &r, &t) else {
            panic!("a first sync did not ask the passphrase again");
        };
        assert!(again.secret);
        assert_eq!(again.asks, Some(key::ASK_BELONGINGS_SYNC_AGAIN));
        assert_eq!(
            v.answered(again.id, "correct h0rse", &r, &t),
            Errand::Say(t.t(key::BELONGINGS_FIRST_PASSPHRASES_DIFFER))
        );
        assert!(!places.passphrase().exists(), "written though they differ");
        let Errand::Ask(line) = v.press(Press::Sync, &r, &zone(), 100, &t) else {
            panic!("the passphrase was not asked");
        };
        let Errand::Ask(again) = v.answered(line.id, "correct horse", &r, &t) else {
            panic!("a first sync did not ask the passphrase again");
        };
        assert_eq!(
            v.answered(again.id, "correct horse", &r, &t),
            Errand::Queue(Op::Prepare)
        );
        let meta = std::fs::metadata(places.passphrase()).unwrap();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::read_to_string(places.passphrase()).unwrap(),
            "correct horse"
        );
        // AN ANSWER UNDER ANOTHER ID answers nothing.
        assert_eq!(v.answered("belongings-token", "x", &r, &t), Errand::Nothing);
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a change of passphrase does not ask the old one and the new
    /// one twice, goes on when the two new ones differ, or does not write
    /// both files private before the order.
    #[test]
    fn a_new_passphrase_is_asked_twice_and_both_are_written_private() {
        let places = scratch("rekey");
        let t = texts("en-GB");
        store_of(&places)
            .write(&Belongings::with_repository("git@example.org:a/b.git"))
            .unwrap();
        let summary = Last {
            repository: "git@example.org:a/b.git".into(),
            at_s: 5,
            ..Last::default()
        };
        ronoctua_belongings::secret::write_private(
            &places.last(),
            &serde_json::to_vec(&summary).unwrap(),
        )
        .unwrap();
        let r = Readings::read(Some(places.clone()), 10);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        let Errand::Ask(first) = v.command("belongings-passphrase", &r, &zone(), 100, &t) else {
            panic!("the old passphrase was not asked");
        };
        assert!(first.secret && first.asks.is_none());
        let Errand::Ask(second) = v.answered(first.id, "old one", &r, &t) else {
            panic!("the new passphrase was not asked");
        };
        assert_eq!(second.asks, Some(key::ASK_BELONGINGS_PASSPHRASE_NEW));
        let Errand::Ask(third) = v.answered(second.id, "new one", &r, &t) else {
            panic!("the new passphrase was not asked again");
        };
        assert_eq!(
            v.answered(third.id, "new 0ne", &r, &t),
            Errand::Say(t.t(key::BELONGINGS_PASSPHRASES_DIFFER))
        );
        assert!(!places.new_passphrase().exists());
        // AND AGAIN, THIS TIME ALIKE.
        let Errand::Ask(first) = v.command("belongings-passphrase", &r, &zone(), 100, &t) else {
            panic!();
        };
        let Errand::Ask(second) = v.answered(first.id, "old one", &r, &t) else {
            panic!();
        };
        let Errand::Ask(third) = v.answered(second.id, "new one", &r, &t) else {
            panic!();
        };
        assert_eq!(
            v.answered(third.id, "new one", &r, &t),
            Errand::Queue(Op::Rekey)
        );
        assert_eq!(
            std::fs::read_to_string(places.passphrase()).unwrap(),
            "old one"
        );
        assert_eq!(
            std::fs::read_to_string(places.new_passphrase()).unwrap(),
            "new one"
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: an address with a credential, `http://` or a local path is
    /// kept, or a plain one is not.
    #[test]
    fn only_a_plain_address_is_kept() {
        let places = scratch("address");
        let t = texts("en-GB");
        let r = Readings::read(Some(places.clone()), 1);
        let mut v = View::fresh(&r, &zone(), 100, &t);
        for (typed, refused) in [
            ("https://me:pw@example.org/b.git", true),
            ("http://example.org/b.git", true),
            ("/srv/b.git", true),
            ("git@example.org:me/b.git", false),
        ] {
            let Errand::Ask(line) = v.command("belongings-repository", &r, &zone(), 100, &t) else {
                panic!("the address was not asked");
            };
            assert!(!line.secret);
            let errand = v.answered(line.id, typed, &r, &t);
            assert_eq!(
                matches!(errand, Errand::Say(_)),
                refused,
                "{typed}: {errand:?}"
            );
        }
        assert_eq!(
            store_of(&places).read().repository,
            "git@example.org:me/b.git"
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a path typed or pressed in the file manager is kept under a
    /// name that is not its logical one, one outside the home or under the
    /// access folder is kept, or the same gesture on what is kept does not
    /// take it out.
    #[test]
    fn a_path_is_kept_by_its_logical_name_and_the_same_gesture_takes_it_out() {
        let places = scratch("keep");
        let t = texts("en-GB");
        assert_eq!(
            logical_of_typed(&places, "~/notes").as_deref(),
            Some("home/notes")
        );
        assert_eq!(
            logical_of_typed(&places, "notes/../music").as_deref(),
            Some("home/music")
        );
        assert_eq!(
            logical_of_typed(&places, &places.config.join("x.json").to_string_lossy()).as_deref(),
            Some("config/x.json")
        );
        assert_eq!(logical_of_typed(&places, "/etc/passwd"), None);
        assert_eq!(logical_of_typed(&places, "../../etc/passwd"), None);

        let notes = places.home.join("notes");
        let e = keep_under(&places, std::slice::from_ref(&notes));
        assert_eq!(e.key, key::BELONGINGS_NOW_KEPT);
        assert_eq!(e.args, [("name", "~/notes".to_owned())]);
        assert!(!t.t_with(e.key, &e.borrowed()).starts_with('['));
        assert_eq!(store_of(&places).read().extra, ["home/notes"]);
        let e = keep_under(&places, &[notes]);
        assert_eq!(e.key, key::BELONGINGS_STOPPED_KEEPING);
        let now = store_of(&places).read();
        assert!(now.extra.is_empty());
        assert_eq!(
            now.excluded,
            ["home/notes"],
            "stopped without being taken out: the next sync would send it again"
        );
        assert_eq!(
            keep_under(&places, &[PathBuf::from("/etc/hosts")]).key,
            key::BELONGINGS_OUTSIDE_HOME
        );
        assert_eq!(
            keep_under(&places, &[places.keys()]).key,
            key::BELONGINGS_NEVER_KEPT
        );
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: a row that acts has no sentence under the pointer, or a
    /// command a row runs, or a key of the view, is not in the registry.
    #[test]
    fn every_row_explains_itself_and_every_command_is_registered() {
        let places = scratch("about");
        let r = waiting(&places);
        let t = texts("en-GB");
        let v = View::fresh(&r, &zone(), 100, &t);
        let rows = v.rows(&r, &zone(), 100, &t);
        for (n, row) in rows.iter().enumerate() {
            if row
                .target
                .as_ref()
                .is_some_and(|t| !matches!(t, Target::Fact(_) | Target::DiffLine(..)))
            {
                let about = View::about(&rows, n, &t).expect("a row that acts says nothing");
                assert!(!about.starts_with('['), "{about}");
            }
        }
        let (_, registry) = crate::leader::default_of().expect("the keymap builds");
        for id in reached().chain([OPEN_COMMAND, WINDOW_COMMAND]) {
            assert!(registry.search(id).is_some(), "`{id}` is not registered");
        }
        let _ = std::fs::remove_dir_all(places.home.parent().unwrap());
    }

    /// FAILS IF: the period stops walking round, both ways.
    #[test]
    fn the_period_turns_both_ways() {
        for p in prefs::PERIODS {
            assert_eq!(previous_period(Belongings::next_period(p)), p);
        }
    }
}
