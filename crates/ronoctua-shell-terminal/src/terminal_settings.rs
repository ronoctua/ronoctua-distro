//! THE TERMINAL'S SETTINGS — one view, two doors (`LEADER RET s`): the
//! footer's region (`RET s s`, [`crate::terminal_settings_region`]) and a
//! window (`RET s w`, [`crate::terminal_settings_window`]).
//!
//! # What is here
//!
//! The few things Ghostty's configuration says that a person changes, in
//! `terminal.json` ([`Settings`], through `ronoctua-store`):
//!
//! - FOR EVERY NEW TERMINAL, taken by its holder at birth
//!   ([`Settings::born`]): how much history it keeps, in bytes (Ghostty's
//!   `scrollback-limit` — set before the first byte, so a terminal already
//!   running keeps what it was born with), and the cursor it starts with
//!   and a program's reset goes back to (`cursor-style`,
//!   `cursor-style-blink`; a program may still ask for another);
//! - IN EVERY TERMINAL WINDOW, now: where a selection is copied as it is
//!   made (`copy-on-select`), and the size of its text — the system's, or
//!   one of its own (`font-size`), which a window's own zoom (`C-=`) is
//!   counted from.
//!
//! # Every value is a ring
//!
//! `←`/`→` turn the value on the row and `RET` turns it forward, because
//! `RET` is what a click becomes (invariant 19). A value a hand wrote that
//! is on no ring is drawn as it is, and turns from the nearest.
//!
//! # Who writes, and who hears
//!
//! Each door writes the file itself, the change laid over the file as it is
//! now ([`change_the_file`]: `parsed`, never `read`, so a file that does not
//! read whole is refused and not replaced), and says on ITS OWN line when it
//! could not. Every other door hears the file (`Notice::on_file`) — the
//! other door of this view, and every terminal window, which reads its
//! settings again and applies what applies now.
//!
//! The read is the store's, lenient: a value a hand wrote that no ring has
//! is drawn as its repair, and the file keeps its word. So both doors read
//! through [`read`], which answers what the read owes the person
//! ([`Unread`]), and the view says it under the values — a part not read
//! (the kit's `partly_read`), or a file nothing of which read.

use crate::fields::{Field, Hit as FilesHit, Ink};
use ronoctua_i18n::Arg;
use ronoctua_terminals::spawn::{Born, CursorShape};
use ronoctua_tokens as tokens;
use serde::{Deserialize, Serialize};

/// The file, under the person's configuration folder.
pub const FILE: &str = "terminal.json";

/// The view in the footer's region.
pub const OPEN: &str = "terminal-settings-open";

/// The view in a window of its own.
pub const WINDOW: &str = "terminal-settings-window";

/// The keys this view teaches. The hint line is GENERATED from it, `Esc` in
/// front (`chrome::with_the_way_back`).
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "terminal-settings-down"),
    ("<up>", "terminal-settings-up"),
    ("<left>", "terminal-settings-less"),
    ("<right>", "terminal-settings-more"),
    ("RET", "terminal-settings-enter"),
];

/// How much history a terminal keeps, as the ring turns it, in bytes:
/// Ghostty's own ten megabytes in the middle, a tenth of it for a small
/// machine, and up to fifty times it for a person who keeps whole builds.
pub const SCROLLBACK_BYTES: [u64; 5] =
    [1_000_000, 10_000_000, 50_000_000, 100_000_000, 500_000_000];

/// One megabyte, as the value is said.
const MEGABYTE: u64 = 1_000_000;

/// Where a selection is copied as it is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CopyOnSelect {
    /// To the selection a middle click pastes — Ghostty's default: selecting
    /// is not copying, and the clipboard keeps what was copied.
    #[default]
    Primary,
    /// To the clipboard.
    Clipboard,
    /// To both.
    Both,
    /// Nowhere: only `C-S-c` copies.
    Off,
}

impl CopyOnSelect {
    /// Every one, in the order the ring turns them.
    pub const ALL: [Self; 4] = [Self::Primary, Self::Clipboard, Self::Both, Self::Off];

    /// Whether a selection goes to the clipboard, and to the primary
    /// selection.
    #[must_use]
    pub const fn targets(self) -> (bool, bool) {
        match self {
            Self::Primary => (false, true),
            Self::Clipboard => (true, false),
            Self::Both => (true, true),
            Self::Off => (false, false),
        }
    }

    const fn word(self) -> &'static str {
        match self {
            Self::Primary => crate::key::TERMINAL_SETTINGS_COPY_PRIMARY,
            Self::Clipboard => crate::key::TERMINAL_SETTINGS_COPY_CLIPBOARD,
            Self::Both => crate::key::TERMINAL_SETTINGS_COPY_BOTH,
            Self::Off => crate::key::TERMINAL_SETTINGS_COPY_OFF,
        }
    }
}

/// The person's terminal settings, as `terminal.json` keeps them. A field
/// the file does not say is its default: Ghostty's, but where this system
/// already says one (the size of the text, the system's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Settings {
    /// The most history a new terminal keeps, in bytes.
    pub scrollback_bytes: u64,
    /// Where a selection is copied as it is made.
    pub copy_on_select: CopyOnSelect,
    /// The cursor a new terminal starts with.
    pub cursor: CursorShape,
    /// Whether that cursor blinks.
    pub cursor_blinks: bool,
    /// The size of the text, in percent of the design's; `None` follows the
    /// system's (`text.json`).
    pub text_percent: Option<u16>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scrollback_bytes: SCROLLBACK_BYTES[1],
            copy_on_select: CopyOnSelect::default(),
            cursor: CursorShape::default(),
            cursor_blinks: true,
            text_percent: None,
        }
    }
}

impl Settings {
    /// What a new terminal is born with: its history's limit and its cursor.
    #[must_use]
    pub const fn born(&self) -> Born {
        Born {
            scrollback_bytes: Some(self.scrollback_bytes),
            cursor: Some((self.cursor, self.cursor_blinks)),
        }
    }

    /// The size of the text a terminal window draws at, in percent: its own,
    /// held to a size the person can reach, else the system's now.
    #[must_use]
    pub fn percent(&self) -> u16 {
        self.text_percent
            .map_or_else(tokens::text::percent, tokens::text::allowed)
    }
}

/// `terminal.json`'s store, in `dir` — the folder the door found and HANDS
/// in; `None` keeps nothing.
#[must_use]
pub fn store_in(dir: Option<std::path::PathBuf>) -> ronoctua_store::Store<Settings> {
    ronoctua_store::Store::at(dir, FILE)
}

/// The person's configuration folder, where `terminal.json` is — the one
/// lookup, which the application makes and hands in to what reads, writes
/// and watches.
#[must_use]
pub fn dir() -> Option<std::path::PathBuf> {
    ronoctua_store::config_dir()
}

/// A watch on `terminal.json` in `dir` — the folder made first, since a watch
/// on a folder that is not there never fires — calling `moved` each time the
/// file is written: the file is the broadcast every door hears.
#[must_use]
pub fn watch_in(
    dir: &std::path::Path,
    pace: ronoctua_files::notice::Pace,
    moved: impl Fn() -> bool + Send + 'static,
) -> Option<ronoctua_files::notice::Notice> {
    if let Err(e) = std::fs::create_dir_all(dir) {
        tracing::warn!(error = %e, "no configuration directory to watch terminal.json in");
        return None;
    }
    ronoctua_files::notice::Notice::on_file(dir, FILE, pace, moved)
        .map_err(|e| tracing::warn!(error = %e, "terminal.json is not watched"))
        .ok()
}

/// One setting of the view — one row that acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Setting {
    /// The history a new terminal keeps.
    Scrollback,
    /// The cursor a new terminal starts with.
    Cursor,
    /// Whether it blinks.
    Blinks,
    /// Where a selection is copied.
    Copy,
    /// The size of the text.
    Size,
}

impl Setting {
    /// The five, in the order drawn: the two sections, for new terminals
    /// and for every window.
    pub const ALL: [Self; 5] = [
        Self::Scrollback,
        Self::Cursor,
        Self::Blinks,
        Self::Copy,
        Self::Size,
    ];

    /// Its section's title.
    const fn heading(self) -> &'static str {
        match self {
            Self::Scrollback | Self::Cursor | Self::Blinks => crate::key::TERMINAL_SETTINGS_NEW,
            Self::Copy | Self::Size => crate::key::TERMINAL_SETTINGS_WINDOWS,
        }
    }

    /// What its row says.
    const fn label(self) -> &'static str {
        match self {
            Self::Scrollback => crate::key::TERMINAL_SETTINGS_SCROLLBACK,
            Self::Cursor => crate::key::TERMINAL_SETTINGS_CURSOR,
            Self::Blinks => crate::key::TERMINAL_SETTINGS_BLINKS,
            Self::Copy => crate::key::TERMINAL_SETTINGS_COPY,
            Self::Size => crate::key::TERMINAL_SETTINGS_SIZE,
        }
    }

    /// What its row's value says, as the file has it now.
    #[must_use]
    pub fn value(self, s: &Settings, texts: &ronoctua_i18n::Texts) -> String {
        let number = |n: u64| Arg::Number(i64::try_from(n).unwrap_or(i64::MAX));
        match self {
            Self::Scrollback => texts.t_args(
                crate::key::TERMINAL_SETTINGS_MEGABYTES,
                &[("megabytes", number(s.scrollback_bytes.div_ceil(MEGABYTE)))],
            ),
            Self::Cursor => texts.t(match s.cursor {
                CursorShape::Block => crate::key::TERMINAL_SETTINGS_CURSOR_BLOCK,
                CursorShape::Bar => crate::key::TERMINAL_SETTINGS_CURSOR_BAR,
                CursorShape::Underline => crate::key::TERMINAL_SETTINGS_CURSOR_UNDERLINE,
            }),
            Self::Blinks => texts.t(if s.cursor_blinks {
                crate::key::TERMINAL_SETTINGS_YES
            } else {
                crate::key::TERMINAL_SETTINGS_NO
            }),
            Self::Copy => texts.t(s.copy_on_select.word()),
            Self::Size => match s.text_percent {
                None => texts.t_args(
                    crate::key::TERMINAL_SETTINGS_SIZE_SYSTEM,
                    &[("percent", number(u64::from(tokens::text::percent())))],
                ),
                Some(_) => texts.t_args(
                    crate::key::TERMINAL_SETTINGS_SIZE_OWN,
                    &[("percent", number(u64::from(s.percent())))],
                ),
            },
        }
    }

    /// What `RET` and the arrows do to the file: the value turned one step
    /// round its ring, `forward` or back — laid over the file AS IT IS NOW,
    /// so a value another hand set is the one turned.
    pub fn change(self, s: &mut Settings, forward: bool) {
        match self {
            Self::Scrollback => {
                // FROM THE NEAREST: a value a hand wrote turns from where it
                // is closest to the ring.
                let at = SCROLLBACK_BYTES
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, b)| b.abs_diff(s.scrollback_bytes))
                    .map_or(0, |(i, _)| i);
                s.scrollback_bytes = SCROLLBACK_BYTES[turned(at, SCROLLBACK_BYTES.len(), forward)];
            }
            Self::Cursor => {
                let at = CursorShape::ALL
                    .iter()
                    .position(|c| *c == s.cursor)
                    .unwrap_or(0);
                s.cursor = CursorShape::ALL[turned(at, CursorShape::ALL.len(), forward)];
            }
            Self::Blinks => s.cursor_blinks = !s.cursor_blinks,
            Self::Copy => {
                let all = CopyOnSelect::ALL;
                let at = all.iter().position(|c| *c == s.copy_on_select).unwrap_or(0);
                s.copy_on_select = all[turned(at, all.len(), forward)];
            }
            Self::Size => s.text_percent = turned_size(s.text_percent, forward),
        }
    }
}

/// `at` turned one step round a ring of `size`.
const fn turned(at: usize, size: usize, forward: bool) -> usize {
    if size == 0 {
        return 0;
    }
    if forward {
        (at + 1) % size
    } else {
        (at + size - 1) % size
    }
}

/// The sizes of the text, as the ring turns them: every size the person can
/// reach (`font.scale*`), smallest first, and the system's — whatever it is
/// — right after the size the system draws at now. So from the system's,
/// `→` is a little larger and `←` is the same size kept as the terminal's
/// own: a person who asks for "a little larger" starts where they are, and
/// every step can be walked back.
fn sizes() -> Vec<Option<u16>> {
    let system = tokens::text::allowed(tokens::text::percent());
    let mut ring = Vec::new();
    for p in (tokens::font::SCALE_FLOOR..=tokens::font::SCALE_CEILING)
        .step_by(usize::from(tokens::font::SCALE_STEP.max(1)))
    {
        ring.push(Some(p));
        if p == system {
            ring.push(None);
        }
    }
    if !ring.contains(&None) {
        ring.insert(0, None);
    }
    ring
}

/// A size turned one step round [`sizes`]; a size a hand wrote that no key
/// reaches turns from the one the person can reach nearest it.
fn turned_size(now: Option<u16>, forward: bool) -> Option<u16> {
    let ring = sizes();
    let here = now.map(tokens::text::allowed);
    let at = ring.iter().position(|r| *r == here).unwrap_or(0);
    ring[turned(at, ring.len(), forward)]
}

/// A key, already reduced to what the view cares about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Up a row that acts.
    Up,
    /// Down a row that acts.
    Down,
    /// The previous value.
    Less,
    /// The next value.
    More,
    /// What a click is: the next value.
    Enter,
    /// Out.
    Escape,
}

/// Which key a command id is — the one map both doors reach.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "terminal-settings-up" => Press::Up,
        "terminal-settings-down" => Press::Down,
        "terminal-settings-less" => Press::Less,
        "terminal-settings-more" => Press::More,
        "terminal-settings-enter" => Press::Enter,
        _ => return None,
    })
}

/// A key, by its WHOLE notation, as the view's press — the one reader both
/// doors call. `Esc` is the way back whatever is held.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::{Key, Sym};
    let notation = match key.sym {
        Sym::Esc => return Some(Press::Escape),
        Sym::Char(_) => key.notation(),
        other => Key::simple(other).notation(),
    };
    OF_THE_VIEW
        .iter()
        .find(|(k, _)| *k == notation)
        .and_then(|(_, id)| press_of(id))
}

/// What the view wants done, after a key or a click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errand {
    /// Redraw and no more.
    Nothing,
    /// Close the view.
    Close,
    /// Turn this setting, forward or back, in the file.
    Change(Setting, bool),
}

/// What writing one change into `terminal.json` came to, for the door that
/// writes it and has to say so.
#[derive(Debug)]
pub enum Changed {
    /// Written: the file is this now.
    Written(Settings),
    /// The file does not parse, and nothing was written over it: writing the
    /// whole value back would replace what a hand put there.
    Unparsed,
    /// It could not be written — why.
    NotWritten(std::io::Error),
}

impl Changed {
    /// The sentence a door says when the change did not land — the file
    /// told as the name it is (`Arg::Name`).
    #[must_use]
    pub fn refusal(&self, texts: &ronoctua_i18n::Texts) -> Option<crate::chrome::Named> {
        match self {
            Self::Written(_) => None,
            Self::Unparsed => Some(
                texts
                    .t_args_marked(
                        crate::key::TERMINAL_SETTINGS_UNPARSED,
                        &[("file", Arg::Name(FILE))],
                    )
                    .into(),
            ),
            Self::NotWritten(why) => Some(
                texts
                    .t_args_marked(
                        crate::key::TERMINAL_SETTINGS_NOT_WRITTEN,
                        &[
                            ("file", Arg::Name(FILE)),
                            ("why", Arg::Text(&why.to_string())),
                        ],
                    )
                    .into(),
            ),
        }
    }
}

/// What a read of `terminal.json` owes the person, beside the values it
/// drew: the store's read is lenient, and a view that shows the file and
/// says nothing would show a repair, or the defaults, as if the file said
/// them (`partly_read`; a refusal is a value with a sentence).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Unread {
    /// It read whole, or there is no file yet: nothing to say.
    #[default]
    Nothing,
    /// A part of it did not read: what is drawn there is the repair, and
    /// the file keeps its own words (`Store::write` puts them back).
    Part(crate::partly_read::PartlyRead),
    /// Nothing of it read: what is drawn is the defaults, and a change is
    /// refused over it ([`change_the_file`]).
    All,
}

impl Unread {
    /// Its sentence, when it owes one — the file told as the name it is
    /// (`Arg::Name`).
    #[must_use]
    pub fn said(&self, texts: &ronoctua_i18n::Texts) -> Option<crate::chrome::Named> {
        match self {
            Self::Nothing => None,
            Self::Part(partly) => Some(partly.said_named(texts)),
            Self::All => Some(
                texts
                    .t_args_marked(
                        crate::key::TERMINAL_SETTINGS_UNREAD,
                        &[("file", Arg::Name(FILE))],
                    )
                    .into(),
            ),
        }
    }
}

/// The file as a view draws it: its values, and what the read owes the
/// person ([`Unread`]) — the one read both doors make of it.
#[must_use]
pub fn read(store: &ronoctua_store::Store<Settings>) -> (Settings, Unread) {
    let reading = store.reading();
    let unread = if reading.lost_everything() {
        Unread::All
    } else {
        crate::partly_read::PartlyRead::of(FILE, &reading).map_or(Unread::Nothing, Unread::Part)
    };
    (reading.value, unread)
}

/// What the file owes the person NOW — judged again after a door wrote it,
/// which keeps the places that did not read as the file had them.
#[must_use]
pub fn judged(store: &ronoctua_store::Store<Settings>) -> Unread {
    read(store).1
}

/// One setting turned in the file — read with `parsed`, never `read`,
/// changed by [`Setting::change`], written whole and atomically.
#[must_use]
pub fn change_the_file(
    store: &ronoctua_store::Store<Settings>,
    setting: Setting,
    forward: bool,
) -> Changed {
    let Some(mut now) = store.parsed() else {
        return Changed::Unparsed;
    };
    setting.change(&mut now, forward);
    match store.write(&now) {
        Ok(()) => Changed::Written(now),
        Err(e) => Changed::NotWritten(e),
    }
}

/// One drawn row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What is drawn, already placed and cut.
    pub fields: Vec<Field>,
    /// The setting it turns; `None` for a heading, the air, a sentence.
    pub setting: Option<Setting>,
}

impl Row {
    /// Whether a click takes it — a setting — and so whether a resting
    /// pointer lights it: ONE ANSWER for both doors' light and click.
    #[must_use]
    pub const fn takes(&self) -> bool {
        self.setting.is_some()
    }
}

/// The view: where the cursor is, and what the door's last read of the file
/// owes the person. Everything else it draws is read from the settings each
/// door hands it, so the two doors draw one file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct View {
    cursor: usize,
    unread: Unread,
}

impl View {
    /// Opens it, on the first row that acts.
    #[must_use]
    pub fn fresh(s: &Settings, columns: u16, texts: &ronoctua_i18n::Texts) -> Self {
        let mut v = Self::default();
        let rows = v.rows(s, columns, texts);
        v.settle_on(&rows);
        v
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// What the door's read of the file owes the person ([`read`],
    /// [`judged`]) — handed in on every read, by both doors.
    pub fn read_in_part(&mut self, unread: Unread) {
        self.unread = unread;
    }

    /// The hint line: `Esc` first, then the view's keys.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        crate::chrome::with_the_way_back(crate::chrome::hint_of(OF_THE_VIEW, texts), texts)
    }

    /// Puts the cursor on row `n`, when it is a setting. Answers whether the
    /// cursor was ALREADY there: the second click, which the door turns into
    /// `RET` (invariant 19).
    pub fn clicked(&mut self, n: usize, rows: &[Row]) -> bool {
        if rows.get(n).is_some_and(Row::takes) {
            let again = self.cursor == n;
            self.cursor = n;
            return again;
        }
        false
    }

    /// Puts the cursor on a row that acts — never on a title.
    pub fn settle_on(&mut self, rows: &[Row]) {
        if rows.get(self.cursor).is_some_and(Row::takes) {
            return;
        }
        self.cursor = rows.iter().position(Row::takes).unwrap_or(0);
    }

    fn walk(&mut self, rows: &[Row], down: bool) {
        let found = if down {
            (self.cursor + 1..rows.len()).find(|&i| rows[i].takes())
        } else {
            (0..self.cursor).rev().find(|&i| rows[i].takes())
        };
        if let Some(i) = found {
            self.cursor = i;
        }
    }

    /// What a key does. THE WIDTH IS THE DOOR'S: a note wraps to it, and the
    /// cursor counts in the rows drawn.
    pub fn press(
        &mut self,
        p: Press,
        s: &Settings,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        let rows = self.rows(s, columns, texts);
        self.settle_on(&rows);
        let here = rows.get(self.cursor).and_then(|r| r.setting);
        match (p, here) {
            (Press::Escape, _) => Errand::Close,
            (Press::Up | Press::Down, _) => {
                self.walk(&rows, p == Press::Down);
                Errand::Nothing
            }
            (Press::Less, Some(setting)) => Errand::Change(setting, false),
            (Press::More | Press::Enter, Some(setting)) => Errand::Change(setting, true),
            (_, None) => Errand::Nothing,
        }
    }

    /// The rows to draw, at this width, from the settings as they are: the
    /// two sections, and after them the sentences that say when each
    /// applies — LAST, so that how many lines a sentence wraps to at this
    /// width never moves a setting's row: the cursor is an index, and a
    /// window that is narrowed keeps it on its setting.
    #[must_use]
    pub fn rows(&self, s: &Settings, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = crate::chrome::extent(columns);
        let piece = |at: u16, text: String, color: tokens::Color| Field {
            at,
            text,
            color,
            hit: FilesHit::Nothing,
            explain: None,
            ink: Ink::Grid,
        };
        let inert = |fields: Vec<Field>| Row {
            fields,
            setting: None,
        };
        let mut out: Vec<Row> = Vec::new();
        let mut last_heading = None;
        for setting in Setting::ALL {
            let heading = setting.heading();
            if last_heading != Some(heading) {
                // AIR BEFORE EVERY TITLE but the first — `chrome::AIR`.
                if !out.is_empty() {
                    out.push(inert(crate::chrome::AIR));
                }
                out.push(inert(vec![piece(
                    left,
                    ronoctua_render::layout::elide_end(&texts.t(heading), room),
                    tokens::color::TEXT_HEADING,
                )]));
                last_heading = Some(heading);
            }
            let focused = self.cursor == out.len();
            out.push(Row {
                fields: crate::chrome::field_pair(
                    left,
                    room,
                    &texts.t(setting.label()),
                    &setting.value(s, texts),
                    crate::chrome::Kind::Setting,
                    focused,
                ),
                setting: Some(setting),
            });
        }
        // WHAT OF THE FILE DID NOT READ, under the values it explains —
        // after every setting, so a sentence that comes or goes with a read
        // never moves the cursor's row.
        if let Some(said) = self.unread.said(texts) {
            out.push(inert(crate::chrome::AIR));
            for fields in crate::chrome::note_fields(left, &said, tokens::color::TEXT_DIM, room) {
                out.push(inert(fields));
            }
        }
        // WHEN EACH APPLIES — the new terminals' values do not move a
        // terminal that runs. The note WRAPS, and is never cut.
        out.push(inert(crate::chrome::AIR));
        for note in [
            crate::key::TERMINAL_SETTINGS_NEW_NOTE,
            crate::key::TERMINAL_SETTINGS_WINDOWS_NOTE,
        ] {
            for line in crate::chrome::note_lines(&texts.t(note), room) {
                out.push(inert(vec![piece(left, line, tokens::color::TEXT_DIM)]));
            }
        }
        out
    }
}

/// What the view's own keys run, for the keymap's reachability (through the
/// crate's `KEYS`).
pub fn keys_reached() -> Vec<&'static str> {
    OF_THE_VIEW.iter().map(|(_, id)| *id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-terminal-settings-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// FAILS IF: a file that says nothing is not Ghostty's defaults — ten
    /// megabytes of history, copy to the primary selection, a blinking
    /// block — and the system's size, or a file of a hand that names only
    /// one field loses the rest to a refusal.
    #[test]
    fn a_file_that_says_nothing_is_the_defaults_and_a_part_reads_alone() {
        let d = Settings::default();
        assert_eq!(d.scrollback_bytes, 10_000_000);
        assert_eq!(d.copy_on_select, CopyOnSelect::Primary);
        assert_eq!((d.cursor, d.cursor_blinks), (CursorShape::Block, true));
        assert_eq!(d.text_percent, None);
        let dir = scratch("part");
        std::fs::write(dir.join(FILE), "{\"cursor\": \"bar\"}").unwrap();
        let read = store_in(Some(dir.clone())).read();
        assert_eq!(read.cursor, CursorShape::Bar);
        assert_eq!(read.scrollback_bytes, d.scrollback_bytes);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: where a selection is copied is not where the row says —
    /// the default the primary selection alone (selecting is not copying),
    /// "both" missing one, "nowhere" copying anything.
    #[test]
    fn a_selection_goes_where_the_setting_says() {
        assert_eq!(CopyOnSelect::Primary.targets(), (false, true));
        assert_eq!(CopyOnSelect::Clipboard.targets(), (true, false));
        assert_eq!(CopyOnSelect::Both.targets(), (true, true));
        assert_eq!(CopyOnSelect::Off.targets(), (false, false));
    }

    /// FAILS IF: a new terminal is not born with what the file says — its
    /// history's limit and its cursor reach the holder — or a size of its
    /// own that no key can reach is drawn as given.
    #[test]
    fn a_new_terminal_is_born_with_the_settings() {
        let s = Settings {
            scrollback_bytes: 50_000_000,
            cursor: CursorShape::Underline,
            cursor_blinks: false,
            text_percent: Some(103),
            ..Settings::default()
        };
        assert_eq!(
            s.born(),
            Born {
                scrollback_bytes: Some(50_000_000),
                cursor: Some((CursorShape::Underline, false)),
            }
        );
        assert_eq!(s.percent(), tokens::text::allowed(103));
        assert_ne!(s.percent(), 103, "a size no key reaches was kept");
    }

    /// FAILS IF: a ring does not come back to where it started, the arrows
    /// do not turn it both ways, or a value a hand wrote between two of the
    /// ring's turns from somewhere else than the nearest.
    #[test]
    fn every_ring_turns_both_ways_and_comes_back() {
        let start = Settings::default();
        for setting in Setting::ALL {
            let mut s = start;
            setting.change(&mut s, true);
            assert_ne!(s, start, "{setting:?} did not turn");
            setting.change(&mut s, false);
            assert_eq!(s, start, "{setting:?} did not turn back");
        }
        let mut s = Settings {
            scrollback_bytes: 48_000_000,
            ..start
        };
        Setting::Scrollback.change(&mut s, true);
        assert_eq!(s.scrollback_bytes, 100_000_000, "turned from no neighbour");
        let mut copy = start;
        for _ in CopyOnSelect::ALL {
            Setting::Copy.change(&mut copy, true);
        }
        assert_eq!(copy.copy_on_select, start.copy_on_select);
    }

    /// FAILS IF: the size's ring from the system's size starts at the
    /// smallest (a person asking for a little larger gets the least), its
    /// other way is not the same size kept as the terminal's own, or the
    /// ring has no way back to the system's.
    #[test]
    fn the_size_turns_from_where_the_system_is() {
        let system = tokens::text::allowed(tokens::text::percent());
        let mut s = Settings::default();
        Setting::Size.change(&mut s, true);
        assert_eq!(s.text_percent, Some(tokens::text::stepped(system, 1)));
        let mut own = Settings::default();
        Setting::Size.change(&mut own, false);
        assert_eq!(own.text_percent, Some(system));
        Setting::Size.change(&mut own, true);
        assert_eq!(
            own.text_percent, None,
            "the ring has no way back to the system's"
        );
    }

    /// FAILS IF: a change is written over a file that does not parse (what a
    /// hand wrote there would be lost to the defaults), a refusal is not a
    /// sentence, or a change is not laid over the file as it is NOW.
    #[test]
    fn a_change_is_laid_over_the_file_and_a_broken_file_is_refused() {
        let dir = scratch("change");
        let store = store_in(Some(dir.clone()));
        std::fs::write(dir.join(FILE), "{\"copy-on-select\": \"both\"}").unwrap();
        let Changed::Written(now) = change_the_file(&store, Setting::Blinks, true) else {
            panic!("not written");
        };
        assert_eq!(
            now.copy_on_select,
            CopyOnSelect::Both,
            "the file's own value was lost"
        );
        assert!(!now.cursor_blinks);
        assert_eq!(store.read(), now);
        std::fs::write(dir.join(FILE), "{ not json").unwrap();
        let refused = change_the_file(&store, Setting::Blinks, true);
        assert!(matches!(refused, Changed::Unparsed));
        assert!(
            refused
                .refusal(&texts("en-GB"))
                .is_some_and(|s| s.contains(FILE))
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(FILE)).unwrap(),
            "{ not json"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a file read only in part, or not at all, is drawn as if it
    /// said what the view shows — no sentence naming the file and the place
    /// that did not read, or the defaults shown without a word — or that
    /// sentence moves a setting's row (the cursor is an index), or it stays
    /// once the file reads whole again.
    #[test]
    fn a_file_read_in_part_or_not_at_all_is_said_under_its_values() {
        let t = texts("en-GB");
        let dir = scratch("unread");
        let store = store_in(Some(dir.clone()));
        let settings_at = |v: &View, s: &Settings| -> Vec<(usize, Setting)> {
            v.rows(s, 100, &t)
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.setting.map(|x| (i, x)))
                .collect()
        };
        let drawn = |v: &View, s: &Settings| -> String {
            v.rows(s, 100, &t)
                .iter()
                .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
                .collect::<Vec<_>>()
                .join(" ")
        };
        // A PART: a cursor no ring has, and the rest the file's own.
        std::fs::write(
            dir.join(FILE),
            "{\"cursor\": \"triangle\", \"copy-on-select\": \"both\"}",
        )
        .unwrap();
        let (s, unread) = read(&store);
        assert_eq!(
            s.copy_on_select,
            CopyOnSelect::Both,
            "the rest of the file was lost"
        );
        assert!(matches!(unread, Unread::Part(_)), "{unread:?}");
        let mut v = View::fresh(&s, 100, &t);
        let whole = settings_at(&v, &s);
        assert!(
            !drawn(&v, &s).contains(FILE),
            "said before it was handed in"
        );
        v.read_in_part(unread);
        let said = drawn(&v, &s);
        assert!(said.contains(FILE) && said.contains("/cursor"), "{said}");
        assert_eq!(settings_at(&v, &s), whole, "the sentence moved a setting");
        // NOTHING OF IT: not JSON — the defaults, said to be them.
        std::fs::write(dir.join(FILE), "{ not json").unwrap();
        let (s, unread) = read(&store);
        assert_eq!((s, &unread), (Settings::default(), &Unread::All));
        v.read_in_part(unread);
        let refused = t.t_with(crate::key::TERMINAL_SETTINGS_UNREAD, &[("file", FILE)]);
        // A LINE OF THE NOTE IS ITS FIELDS SIDE BY SIDE: the file is a field
        // of its own, in a name's ink.
        let rows = v.rows(&s, 100, &t);
        let lines: Vec<String> = rows
            .iter()
            .map(|r| r.fields.iter().map(|f| f.text.as_str()).collect())
            .collect();
        assert!(
            rows.iter()
                .flat_map(|r| &r.fields)
                .any(|f| f.text == FILE
                    && f.color == crate::chrome::quoted_ink(tokens::color::TEXT_DIM)),
            "the file is not told as a name: {lines:?}"
        );
        let (_, room) = crate::chrome::extent(100);
        assert!(
            crate::chrome::note_lines(&refused, room)
                .iter()
                .all(|l| lines.contains(l)),
            "{lines:?}"
        );
        // WHOLE AGAIN, and nothing said.
        std::fs::write(dir.join(FILE), "{\"cursor\": \"bar\"}").unwrap();
        let (s, unread) = read(&store);
        assert_eq!(unread, Unread::Nothing);
        v.read_in_part(unread);
        assert!(!drawn(&v, &s).contains(FILE), "{}", drawn(&v, &s));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the cursor sits on a title, `RET` or `→` does not turn the
    /// row forward and `←` back, or a click on a title moves the cursor —
    /// and the first click on a setting turns it (the first selects).
    #[test]
    fn a_key_or_a_second_click_turns_the_row_under_the_cursor() {
        let t = texts("en-GB");
        let s = Settings::default();
        let mut v = View::fresh(&s, 80, &t);
        let rows = v.rows(&s, 80, &t);
        assert!(rows[v.cursor()].takes());
        assert_eq!(
            v.press(Press::Enter, &s, 80, &t),
            Errand::Change(Setting::Scrollback, true)
        );
        assert_eq!(
            v.press(Press::Less, &s, 80, &t),
            Errand::Change(Setting::Scrollback, false)
        );
        let size = rows
            .iter()
            .position(|r| r.setting == Some(Setting::Size))
            .unwrap();
        assert!(!v.clicked(0, &rows), "a title took a click");
        assert!(!v.clicked(size, &rows), "the first click acted");
        assert!(v.clicked(size, &rows));
        assert_eq!(v.press(Press::Escape, &s, 80, &t), Errand::Close);
    }

    /// FAILS IF: how wide the door is moves a setting's row — the cursor is
    /// an index, and a window narrowed until a sentence wraps would leave it
    /// on another setting, or on a sentence.
    #[test]
    fn a_settings_row_is_where_it_is_at_every_width() {
        let t = texts("pt-BR");
        let s = Settings::default();
        let at = |columns: u16| -> Vec<(usize, Setting)> {
            View::default()
                .rows(&s, columns, &t)
                .iter()
                .enumerate()
                .filter_map(|(i, r)| r.setting.map(|x| (i, x)))
                .collect()
        };
        assert_eq!(at(30), at(200));
        assert_eq!(at(30).len(), Setting::ALL.len());
    }

    /// FAILS IF: two rows of the view say the same thing in a language, or a
    /// value of a ring has no words in one — a frame of every variant, in
    /// both languages.
    #[test]
    fn every_value_of_every_ring_is_said_in_every_language() {
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let mut said = std::collections::BTreeSet::new();
            for cursor in CursorShape::ALL {
                for copy in CopyOnSelect::ALL {
                    let s = Settings {
                        cursor,
                        copy_on_select: copy,
                        ..Settings::default()
                    };
                    for setting in Setting::ALL {
                        let value = setting.value(&s, &t);
                        assert!(!value.trim().is_empty(), "{language}: {setting:?}");
                        said.insert((format!("{setting:?}"), value));
                    }
                }
            }
            let copies: std::collections::BTreeSet<String> =
                CopyOnSelect::ALL.iter().map(|c| t.t(c.word())).collect();
            assert_eq!(copies.len(), CopyOnSelect::ALL.len(), "{language}");
            let labels: std::collections::BTreeSet<String> =
                Setting::ALL.iter().map(|s| t.t(s.label())).collect();
            assert_eq!(labels.len(), Setting::ALL.len(), "{language}");
        }
    }
}
