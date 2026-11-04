//! THE TERMINALS NO WINDOW SHOWS — one view, two doors (`LEADER RET b`): the
//! footer's region (`RET b b`, [`crate::terminals_region`], which the
//! modeline's mark opens) and a window (`RET b w`,
//! [`crate::terminals_window`]).
//!
//! # What it lists
//!
//! Every held terminal no window is attached to — the ones whose program
//! still runs, and the ones whose program ended there, kept until a window
//! has shown them once: what the modeline's mark counts, row for row. Each
//! row says what it is (its program's title, else the program), which
//! program, where, how long it has been without a window, and what the
//! program says it is doing (OSC 7501, the `program-*` glyphs) — or that it
//! rang, or that it ended and how.
//!
//! # As their windows had them
//!
//! A window closed with two or more terminals in it sent its whole group to
//! the background, and showing one of them brings the group back with the
//! one asked in front (`terminal-show`). So a group is listed together,
//! under a heading of its own, in its strip's order — and the terminals that
//! were alone in their windows come first, under one heading. The newest is
//! first: what was put away a moment ago is what a person reaches for.
//!
//! # What a key does
//!
//! `RET` (and the second click — invariant 19) shows the terminal: the
//! footer's `terminal-show`, which focuses its window or opens one for its
//! group. `d` ends it — a terminal's program and what it wrote are not
//! undone, so it is ASKED first, on a page of its own with the "no" first
//! and the cursor born on it; the end itself is an order to the terminal's
//! holder, on a thread (`Registry::end` waits for its answer).
//!
//! # What it reads, and from where
//!
//! Nothing of the machine: each door hands in the table it follows — the
//! footer's census, lent ([`crate::terminals_region`]), or the window's own
//! ([`crate::terminals_window`]) — with the time and the home it is drawn
//! against ([`Reading`]). The cursor follows its row by the terminal's id
//! ([`View::follow`]): a terminal put away adds a row above it.

use std::path::Path;

use crate::fields::{Field, Hit as FilesHit, Ink};
use ronoctua_i18n::Arg;
use ronoctua_render::layout::{elide_end, elide_start, width};
use ronoctua_terminals::state::{ProgramState, StatusRecord, TerminalState};
use ronoctua_terminals::{GroupId, TerminalId};
use ronoctua_tokens as tokens;

/// The view in the footer's region.
pub const OPEN: &str = "terminals-open";

/// The view in a window of its own.
pub const WINDOW: &str = "terminals-window";

/// The footer's command that shows a terminal — focuses its window, or opens
/// one for its group with it in front (`surface/terminals.rs`).
pub const SHOW: &str = "terminal-show";

/// The keys this view teaches, and where each one applies. The hint line is
/// GENERATED from it, `Esc` in front (`chrome::with_the_way_back`).
pub const OF_THE_VIEW: &[(&str, &str, Where)] = &[
    ("<down>", "terminals-down", Where::Anywhere),
    ("<up>", "terminals-up", Where::Anywhere),
    ("RET", "terminals-enter", Where::OnARow),
    ("d", "terminals-end", Where::OnATerminal),
];

/// Which rows a key means anything on — the hint teaches only the keys that
/// WORK where the cursor is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Where {
    /// Always.
    Anywhere,
    /// On a terminal, or on an answer of the question.
    OnARow,
    /// On a terminal.
    OnATerminal,
}

impl Where {
    fn shown(self, target: Option<&Target>) -> bool {
        match self {
            Self::Anywhere => true,
            Self::OnARow => target.is_some(),
            Self::OnATerminal => matches!(target, Some(Target::Terminal(_))),
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
    /// What a click is: show the terminal, or answer the question.
    Enter,
    /// End the terminal — asked first.
    End,
    /// Back one level, and out.
    Escape,
}

/// What a row is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A terminal.
    Terminal(TerminalId),
    /// An answer to the question: yes, end it.
    Answer(bool),
}

/// One drawn line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What is drawn, already placed and cut.
    pub fields: Vec<Field>,
    /// What a key does on it; `None` for a heading, the air, a sentence.
    pub target: Option<Target>,
}

impl Row {
    /// Whether a click takes it — a terminal or an answer, the rows the
    /// cursor stands on — and so whether a resting pointer lights it: ONE
    /// ANSWER for both doors' light and click (the region's `Row::takes`,
    /// the window's `Content::takes`, [`View::clicked`]).
    #[must_use]
    pub const fn takes(&self) -> bool {
        self.target.is_some()
    }
}

/// What the door has to do about a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Draw again, and nothing else.
    Nothing,
    /// Shut the view.
    Close,
    /// Show this terminal: its window focused, or one opened for its group.
    Show(TerminalId),
    /// End this terminal — the person said yes — and the name its row
    /// gave it, for the sentence that says so.
    End(TerminalId, String),
    /// The same, and shut the view, which was opened for that question.
    EndAndClose(TerminalId, String),
}

/// What the rows are drawn from: the table one door follows, the time it is
/// drawn at, and the home a folder is shown under.
#[derive(Debug, Clone, Copy)]
pub struct Reading<'a> {
    /// Every held terminal, as the census last read them.
    pub census: &'a [TerminalState],
    /// Now, in seconds since the epoch.
    pub now_s: i64,
    /// The person's home, when it is known.
    pub home: Option<&'a Path>,
}

/// The question on the view's page: which terminal, as its row named it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Question {
    terminal: TerminalId,
    name: String,
    /// Its program ended already: nothing is hung up, only what it wrote
    /// goes — another sentence.
    ended: bool,
    /// Asked from outside the view — a tab's middle button on the modeline
    /// (`terminals-end` with the terminal): the view was opened for this
    /// question alone, and the answer closes it.
    outside: bool,
}

/// The view: where the cursor is, and the question, while it is up.
#[derive(Debug, Default)]
pub struct View {
    cursor: usize,
    question: Option<Question>,
}

/// The terminals no window shows, grouped as their windows had them: those
/// alone first (`None`), then each group — newest first, and a group in its
/// strip's order.
#[must_use]
pub fn sections(census: &[TerminalState]) -> Vec<(Option<&GroupId>, Vec<&TerminalState>)> {
    let mut groups: Vec<(&GroupId, Vec<&TerminalState>)> = Vec::new();
    for t in census.iter().filter(|t| t.viewer.is_none()) {
        match groups.iter_mut().find(|(g, _)| *g == &t.group) {
            Some((_, members)) => members.push(t),
            None => groups.push((&t.group, vec![t])),
        }
    }
    let newest = |members: &[&TerminalState]| members.iter().map(|t| since_s(t)).max();
    let mut alone: Vec<&TerminalState> = Vec::new();
    let mut together: Vec<(Option<&GroupId>, Vec<&TerminalState>)> = Vec::new();
    for (group, mut members) in groups {
        if members.len() < 2 {
            alone.append(&mut members);
        } else {
            members.sort_by(|a, b| (a.order, &a.id).cmp(&(b.order, &b.id)));
            together.push((Some(group), members));
        }
    }
    alone.sort_by(|a, b| (since_s(b), &b.id).cmp(&(since_s(a), &a.id)));
    together.sort_by(|a, b| newest(&b.1).cmp(&newest(&a.1)).then(b.0.cmp(&a.0)));
    let mut out = Vec::with_capacity(together.len() + 1);
    if !alone.is_empty() {
        out.push((None, alone));
    }
    out.extend(together);
    out
}

/// Since when a terminal is without a window: its last window's leaving,
/// else its end, else its start (a terminal no window ever attached).
const fn since_s(t: &TerminalState) -> i64 {
    match (t.alone_since_s, t.ended) {
        (Some(s), _) => s,
        (None, Some(e)) => e.at_s,
        (None, None) => t.started_s,
    }
}

/// How a terminal goes by on its row: its program's title, else its program.
#[must_use]
pub fn name_of(t: &TerminalState) -> &str {
    if t.title.trim().is_empty() {
        &t.program
    } else {
        &t.title
    }
}

/// What a program reports of itself, as a row says it: the most recent of
/// its own records — never a child's (an agent's subagents are `<id>/<sub>`,
/// and theirs is not the terminal's state).
#[must_use]
pub fn status_of(t: &TerminalState) -> Option<&StatusRecord> {
    t.statuses
        .iter()
        .filter(|r| !r.id.contains('/'))
        .max_by_key(|r| r.at_ms)
}

/// The glyph a program's state is drawn with — `program-*`, one family, so
/// the states read as states of one thing; an error is the done glyph in the
/// danger colour (danger is a colour, never another drawing).
#[must_use]
pub const fn glyph_of(state: ProgramState) -> char {
    match state {
        ProgramState::Working => tokens::icon::PROGRAM_WORKING,
        ProgramState::Blocked => tokens::icon::PROGRAM_BLOCKED,
        ProgramState::Idle | ProgramState::Done | ProgramState::Error => tokens::icon::PROGRAM_DONE,
    }
}

/// The word a program's state is said with.
#[must_use]
pub const fn word_of(state: ProgramState) -> &'static str {
    match state {
        ProgramState::Working => crate::key::TERMINALS_WORKING,
        ProgramState::Blocked => crate::key::TERMINALS_BLOCKED,
        ProgramState::Done => crate::key::TERMINALS_DONE,
        ProgramState::Idle => crate::key::TERMINALS_IDLE,
        ProgramState::Error => crate::key::TERMINALS_FAILED,
    }
}

/// The colour a state is SAID in, beside the row's own tones — `None` for
/// the states that ask nothing of the person.
const fn tint_of(state: ProgramState) -> Option<tokens::Color> {
    match state {
        ProgramState::Blocked => Some(tokens::color::STATE_CAUTION),
        ProgramState::Error => Some(tokens::color::STATE_ERROR),
        ProgramState::Working | ProgramState::Done | ProgramState::Idle => None,
    }
}

/// What a row says last: the program's state, its glyph first — or, with
/// none, that it ended (how), or what it asked for while nobody looked. The
/// glyph and the word, and the colour they are said in.
fn state_said(t: &TerminalState, texts: &ronoctua_i18n::Texts) -> (String, Option<tokens::Color>) {
    if let Some(e) = t.ended {
        return if e.code == 0 {
            (texts.t(crate::key::TERMINALS_ENDED), None)
        } else {
            (
                texts.t_args(
                    crate::key::TERMINALS_ENDED_CODE,
                    &[("code", Arg::Number(i64::from(e.code)))],
                ),
                Some(tokens::color::STATE_ERROR),
            )
        };
    }
    if let Some(r) = status_of(t) {
        return (
            format!("{} {}", glyph_of(r.state), texts.t(word_of(r.state))),
            tint_of(r.state),
        );
    }
    let caution = Some(tokens::color::STATE_CAUTION);
    if t.attention.notice > 0 {
        (texts.t(crate::key::TERMINALS_NOTIFIED), caution)
    } else if t.attention.bell > 0 {
        (texts.t(crate::key::TERMINALS_RANG), caution)
    } else {
        (String::new(), None)
    }
}

/// How long it has been without a window, said the short way.
fn since_said(t: &TerminalState, now_s: i64, texts: &ronoctua_i18n::Texts) -> String {
    let seconds = now_s.saturating_sub(since_s(t)).max(0);
    let (minutes, hours, days) = (seconds / 60, seconds / 3600, seconds / 86_400);
    if days > 0 {
        texts.t_args(
            crate::key::TERMINALS_SINCE_DAYS,
            &[("days", Arg::Number(days))],
        )
    } else if hours > 0 {
        texts.t_args(
            crate::key::TERMINALS_SINCE_HOURS,
            &[("hours", Arg::Number(hours))],
        )
    } else if minutes > 0 {
        texts.t_args(
            crate::key::TERMINALS_SINCE_MINUTES,
            &[("minutes", Arg::Number(minutes))],
        )
    } else {
        texts.t(crate::key::TERMINALS_SINCE_MOMENTS)
    }
}

/// Where a terminal is, as a person reads it: under their home, `~/…`. A
/// folder is data, written as the shell writes it.
fn folder_said(t: &TerminalState, home: Option<&Path>) -> String {
    let Some(cwd) = &t.cwd else {
        return String::new();
    };
    match home.and_then(|h| cwd.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".to_owned(),
        Some(rest) => format!("~/{}", rest.display()),
        None => cwd.display().to_string(),
    }
}

/// One terminal's row, as words in its five columns: its name, its program
/// (when the name is not already it), its folder, how long without a
/// window, and its state — with the state's colour.
struct Said {
    terminal: TerminalId,
    cells: [String; 5],
    tint: Option<tokens::Color>,
}

impl Said {
    fn of(t: &TerminalState, r: &Reading<'_>, texts: &ronoctua_i18n::Texts) -> Self {
        let name = name_of(t);
        let program = if name == t.program {
            String::new()
        } else {
            t.program.clone()
        };
        let (state, tint) = state_said(t, texts);
        Self {
            terminal: t.id.clone(),
            cells: [
                name.to_owned(),
                program,
                folder_said(t, r.home),
                since_said(t, r.now_s, texts),
                state,
            ],
            tint,
        }
    }
}

/// The two cells of air between two columns.
const AIR: u16 = 2;

/// The columns' widths over `room` cells, given in the order a narrowing
/// gives them up:
///
/// - how long, and the state — short, and what a person scans the list for
///   (which agent waits): whole;
/// - the name: whole where it fits, else what is left of the row;
/// - the program: whole or not at all — a program's name cut is no name;
/// - the folder, with the rest — cut from its start, so the place keeps its
///   own name — and not at all where even its own name would be cut.
///
/// A column with nothing in any row takes no cell.
fn widths(said: &[&Said], room: u16) -> [u16; 5] {
    let widest = |i: usize| said.iter().map(|s| width(&s.cells[i])).max().unwrap_or(0);
    // THE FOLDER'S OWN NAME, the least a folder is drawn with: its last
    // part, and the mark that says something was cut before it.
    let least_folder = said
        .iter()
        .map(|s| {
            let folder = &s.cells[2];
            folder
                .rsplit_once('/')
                .map_or_else(|| width(folder), |(_, last)| width(last) + 1)
        })
        .max()
        .unwrap_or(0);
    let mut out = [0u16; 5];
    let mut left = room;
    // A COLUMN OF NOTHING costs no air either: the row skips it, so the
    // two cells are the folder's to keep.
    let mut take = |i: usize, w: u16, left: &mut u16| {
        out[i] = w;
        if w > 0 {
            *left = left.saturating_sub(w + AIR);
        }
    };
    for i in [3, 4] {
        take(i, widest(i).min(left), &mut left);
    }
    take(0, widest(0).min(left), &mut left);
    if widest(1) <= left {
        take(1, widest(1), &mut left);
    }
    if widest(2) > 0 && least_folder <= left {
        take(2, widest(2).min(left), &mut left);
    }
    out
}

impl View {
    /// A view, its cursor on the first row that takes one.
    #[must_use]
    pub fn fresh(r: &Reading<'_>, columns: u16, texts: &ronoctua_i18n::Texts) -> Self {
        let mut v = Self::default();
        v.settle_in(r, columns, texts);
        v
    }

    /// Which row the cursor is on.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Whether the question is up — what a door shows on its own line.
    #[must_use]
    pub const fn asks(&self) -> bool {
        self.question.is_some()
    }

    /// The hint line: `Esc` first, then the keys that work where the cursor
    /// is.
    #[must_use]
    pub fn hint(
        &self,
        r: &Reading<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::chrome::Piece> {
        let rows = self.rows(r, columns, texts);
        let target = rows.get(self.cursor).and_then(|row| row.target.as_ref());
        let here: Vec<(&str, &str)> = OF_THE_VIEW
            .iter()
            .filter(|(_, _, w)| w.shown(target))
            .map(|(k, id, _)| (*k, *id))
            .collect();
        crate::chrome::with_the_way_back(crate::chrome::hint_of(&here, texts), texts)
    }

    /// Puts the cursor on row `n`, when a click takes it. Answers whether the
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

    /// The cursor on a row that takes it, after the rows changed under it.
    pub fn settle_in(&mut self, r: &Reading<'_>, columns: u16, texts: &ronoctua_i18n::Texts) {
        let rows = self.rows(r, columns, texts);
        self.clamp(&rows);
    }

    /// What the cursor is on — what [`View::follow`] keeps it on across a
    /// new reading.
    #[must_use]
    pub fn on(
        &self,
        r: &Reading<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Option<Target> {
        self.rows(r, columns, texts)
            .get(self.cursor)
            .and_then(|row| row.target.clone())
    }

    /// The cursor back on `on`'s row after a new reading — by IDENTITY: a
    /// terminal put away adds a row above, and an index would drift. A
    /// question about a terminal that left the background is let go: gone,
    /// there is nothing left to end; shown in a window meanwhile, it is no
    /// row of this view any more, and a "yes" would end what a window is
    /// showing. One asked from outside ([`View::ask_to_end`]) is about a
    /// terminal its window shows on purpose, and goes only with it.
    pub fn follow(
        &mut self,
        on: Option<&Target>,
        r: &Reading<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) {
        if let Some(q) = &self.question
            && !r
                .census
                .iter()
                .any(|t| t.id == q.terminal && (q.outside || t.viewer.is_none()))
        {
            self.question = None;
        }
        let rows = self.rows(r, columns, texts);
        if let Some(i) =
            on.and_then(|on| rows.iter().position(|row| row.target.as_ref() == Some(on)))
        {
            self.cursor = i;
        }
        self.clamp(&rows);
    }

    fn clamp(&mut self, rows: &[Row]) {
        if rows.is_empty() {
            self.cursor = 0;
            return;
        }
        self.cursor = self.cursor.min(rows.len() - 1);
        if !rows[self.cursor].takes() {
            let after = (self.cursor..rows.len()).find(|&i| rows[i].takes());
            let before = (0..=self.cursor).rev().find(|&i| rows[i].takes());
            if let Some(i) = after.or(before) {
                self.cursor = i;
            }
        }
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

    /// A key. The width is the door's: the rows are an index the cursor
    /// counts in, and a question's sentence wraps to it.
    pub fn key(
        &mut self,
        press: Press,
        r: &Reading<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        self.settle_in(r, columns, texts);
        let rows = self.rows(r, columns, texts);
        if self.question.is_some() {
            return self.answer_key(press, &rows, r, columns, texts);
        }
        let here = rows.get(self.cursor).and_then(|row| row.target.clone());
        match (press, here) {
            (Press::Escape, _) => Errand::Close,
            (Press::Up | Press::Down, _) => {
                self.walk(&rows, press == Press::Down);
                Errand::Nothing
            }
            (Press::Enter, Some(Target::Terminal(id))) => Errand::Show(id),
            (Press::End, Some(Target::Terminal(id))) => {
                let Some(t) = r.census.iter().find(|t| t.id == id) else {
                    return Errand::Nothing;
                };
                self.question = Some(Question {
                    terminal: id,
                    name: name_of(t).to_owned(),
                    ended: t.ended.is_some(),
                    outside: false,
                });
                // "NO" FIRST, and the cursor is born on it.
                let asked = self.rows(r, columns, texts);
                if let Some(i) = asked
                    .iter()
                    .position(|row| row.target == Some(Target::Answer(false)))
                {
                    self.cursor = i;
                }
                Errand::Nothing
            }
            _ => Errand::Nothing,
        }
    }

    /// A key while the question is up: the arrows walk between "no" and
    /// "yes", `RET` answers, and every other key is "no" — `Esc` included,
    /// which takes the question down and leaves the view open.
    fn answer_key(
        &mut self,
        press: Press,
        rows: &[Row],
        r: &Reading<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Errand {
        if matches!(press, Press::Up | Press::Down) {
            self.walk(rows, press == Press::Down);
            return Errand::Nothing;
        }
        let yes = press == Press::Enter
            && rows.get(self.cursor).and_then(|row| row.target.as_ref())
                == Some(&Target::Answer(true));
        let Some(q) = self.question.take() else {
            return Errand::Nothing;
        };
        // ASKED FROM OUTSIDE, the answer is the view's last word.
        if q.outside {
            return if yes {
                Errand::EndAndClose(q.terminal, q.name)
            } else {
                Errand::Close
            };
        }
        // BACK ON THE TERMINAL that was asked about, when it is still there.
        let on = Target::Terminal(q.terminal.clone());
        self.follow(Some(&on), r, columns, texts);
        if yes {
            Errand::End(q.terminal, q.name)
        } else {
            Errand::Nothing
        }
    }

    /// Asks whether to end `id` — any held terminal, its window's or not —
    /// as the view's `d` asks of its row: `no` first, the cursor born on it.
    /// Asked from outside the view (`terminals-end` with the terminal), so
    /// the answer closes it. `false` when the census holds no such terminal.
    pub fn ask_to_end(
        &mut self,
        id: &TerminalId,
        r: &Reading<'_>,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> bool {
        let Some(t) = r.census.iter().find(|t| t.id == *id) else {
            return false;
        };
        self.question = Some(Question {
            terminal: id.clone(),
            name: name_of(t).to_owned(),
            ended: t.ended.is_some(),
            outside: true,
        });
        let asked = self.rows(r, columns, texts);
        if let Some(i) = asked
            .iter()
            .position(|row| row.target == Some(Target::Answer(false)))
        {
            self.cursor = i;
        }
        true
    }

    /// Every row, in reading order.
    #[must_use]
    pub fn rows(&self, r: &Reading<'_>, columns: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let (left, room) = crate::chrome::extent(columns);
        let mut b = Builder {
            out: Vec::new(),
            left,
            room,
            cursor: self.cursor,
            texts,
        };
        if let Some(q) = &self.question {
            b.question(q);
            return b.out;
        }
        let sections = sections(r.census);
        if sections.is_empty() {
            b.heading(texts.t(crate::key::TERMINALS_TITLE));
            b.note(&texts.t(crate::key::TERMINALS_NONE));
            return b.out;
        }
        // ONE SET OF COLUMNS for the whole view, so a column lines up across
        // the headings.
        let said: Vec<Vec<Said>> = sections
            .iter()
            .map(|(_, members)| members.iter().map(|t| Said::of(t, r, texts)).collect())
            .collect();
        let all: Vec<&Said> = said.iter().flatten().collect();
        let widths = widths(&all, room);
        let mut number = 0i64;
        for ((group, members), said) in sections.iter().zip(&said) {
            let heading = match group {
                None => texts.t(crate::key::TERMINALS_ALONE),
                Some(_) => {
                    number += 1;
                    let count = i64::try_from(members.len()).unwrap_or(i64::MAX);
                    texts.t_args(
                        crate::key::TERMINALS_GROUP,
                        &[
                            ("number", Arg::Number(number)),
                            ("count", Arg::Number(count)),
                        ],
                    )
                }
            };
            b.heading(heading);
            for s in said {
                b.terminal(s, &widths);
            }
        }
        b.out
    }
}

/// Which key a command id is — the one map both doors reach.
#[must_use]
pub fn press_of(id: &str) -> Option<Press> {
    Some(match id {
        "terminals-up" => Press::Up,
        "terminals-down" => Press::Down,
        "terminals-enter" => Press::Enter,
        "terminals-end" => Press::End,
        _ => return None,
    })
}

/// A key, as the view's press — the ONE reader both doors call. `Esc` is the
/// way back whatever is held; the arrows and `RET` are read by their symbol,
/// and a letter by its whole notation: `C-d` is not `d`.
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
            target: None,
        });
    }

    /// A section's title — air first, never before the first.
    fn heading(&mut self, text: String) {
        if !self.out.is_empty() {
            self.inert(crate::chrome::AIR);
        }
        let text = elide_end(&text, self.room);
        self.inert(vec![Self::piece(
            self.left,
            text,
            tokens::color::TEXT_HEADING,
        )]);
    }

    /// A sentence that stands for a section. It WRAPS.
    fn note(&mut self, text: &str) {
        for line in crate::chrome::note_lines(text, self.room) {
            self.inert(vec![Self::piece(self.left, line, tokens::color::TEXT_DIM)]);
        }
    }

    /// A terminal's row: an item of a list that acts (`RET` shows it), its
    /// name the strong half, its state in its own colour.
    fn terminal(&mut self, said: &Said, widths: &[u16; 5]) {
        let focused = self.cursor == self.out.len();
        let (name_tone, rest_tone) = crate::chrome::Kind::ListedSetting.tones(focused);
        let mut fields = Vec::with_capacity(5);
        let mut at = self.left;
        for (i, (text, &w)) in said.cells.iter().zip(widths).enumerate() {
            if w == 0 {
                continue;
            }
            // A FOLDER KEEPS ITS END, where the name of the place is; every
            // other column its start.
            let cut = if i == 2 {
                elide_start(text, w)
            } else {
                elide_end(text, w)
            };
            if !cut.is_empty() {
                let tone = if i == 0 { name_tone } else { rest_tone };
                fields.push(Self::piece(at, cut, tone));
            }
            at = at.saturating_add(w + AIR);
        }
        // THE STATE IN ITS OWN COLOUR, but under the cursor, where it says
        // what the cursor says.
        if let (Some(tint), false) = (said.tint, focused)
            && let Some(last) = fields.last_mut()
            && !said.cells[4].is_empty()
        {
            last.color = tint;
        }
        self.out.push(Row {
            fields,
            target: Some(Target::Terminal(said.terminal.clone())),
        });
    }

    /// The question's page: what is asked, then "no" and "yes" — "no" first,
    /// and the cursor is born on it; "yes" in the colour of danger.
    fn question(&mut self, q: &Question) {
        let t = self.texts;
        // A TERMINAL WITH A WINDOW is no terminal without one: asked from
        // outside, the page is titled by its question.
        self.heading(t.t(if q.outside {
            crate::key::TERMINALS_END_TITLE
        } else {
            crate::key::TERMINALS_TITLE
        }));
        let asked = t.t_with(
            if q.ended {
                crate::key::TERMINALS_CLOSE_QUESTION
            } else {
                crate::key::TERMINALS_END_QUESTION
            },
            &[("name", &q.name)],
        );
        self.note(&asked);
        self.inert(crate::chrome::AIR);
        for (yes, key) in [
            (false, crate::key::TERMINALS_ANSWER_NO),
            (true, crate::key::TERMINALS_ANSWER_YES),
        ] {
            let focused = self.cursor == self.out.len();
            let (tone, _) = crate::chrome::Kind::Setting.tones(focused);
            let tone = if yes && !focused {
                tokens::color::STATE_ERROR
            } else {
                tone
            };
            let text = elide_end(&t.t(key), self.room);
            self.out.push(Row {
                fields: vec![Self::piece(self.left, text, tone)],
                target: Some(Target::Answer(yes)),
            });
        }
    }
}

/// What the view's own keys and rows run, for the keymap's reachability
/// (`bindings::reachable`, through the crate's `KEYS`): its gestures, and
/// the footer's `terminal-show` that `RET` on a row runs.
pub fn keys_reached() -> Vec<&'static str> {
    OF_THE_VIEW
        .iter()
        .map(|(_, id, _)| *id)
        .chain([SHOW])
        .collect()
}

/// A terminal as a test plants it.
#[cfg(test)]
#[must_use]
pub fn planted(program: &str, group: &GroupId, order: u32, alone_since_s: i64) -> TerminalState {
    TerminalState {
        protocol: ronoctua_terminals::PROTOCOL,
        id: TerminalId::fresh().expect("an id"),
        group: group.clone(),
        order,
        viewer: None,
        front: false,
        seen: false,
        title: String::new(),
        program: program.to_owned(),
        cwd: None,
        started_s: alone_since_s - 60,
        alone_since_s: Some(alone_since_s),
        attention: ronoctua_terminals::state::Attention::default(),
        notice: None,
        command: None,
        front_asked: 0,
        ended: None,
        statuses: Vec::new(),
        progress: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_terminals::state::Ended;

    fn texts(language: &str) -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh(language).expect("a language")
    }

    const NOW: i64 = 1_800_000_000;

    fn group() -> GroupId {
        GroupId::fresh().expect("a group")
    }

    fn status(state: ProgramState, at_ms: u64) -> StatusRecord {
        StatusRecord {
            id: String::new(),
            app: "claude-code".to_owned(),
            state,
            kind: None,
            title: String::new(),
            msg: String::new(),
            percent: None,
            at_ms,
        }
    }

    /// Two alone (a shell and an agent waiting), a window of three, and one
    /// with a window — which is no row.
    fn census() -> Vec<TerminalState> {
        let (g1, g2, g3, g4) = (group(), group(), group(), group());
        let mut shell = planted("zsh", &g1, 1 << 16, NOW - 120);
        shell.cwd = Some("/home/p/prj/ronoctua".into());
        let mut agent = planted("claude", &g2, 1 << 16, NOW - 30);
        agent.title = "fixing the strip".to_owned();
        agent.statuses = vec![
            status(ProgramState::Working, 1),
            status(ProgramState::Blocked, 2),
        ];
        let mut together: Vec<TerminalState> = (0..3)
            .map(|n| planted("zsh", &g3, (3 - n) << 16, NOW - 4000))
            .collect();
        together[0].ended = Some(Ended {
            code: 2,
            at_s: NOW - 3990,
        });
        let mut shown = planted("htop", &g4, 1 << 16, NOW - 10);
        shown.viewer = Some(42);
        shown.alone_since_s = None;
        let mut all = vec![shell, agent, shown];
        all.extend(together);
        all
    }

    fn reading(census: &[TerminalState]) -> Reading<'_> {
        Reading {
            census,
            now_s: NOW,
            home: Some(Path::new("/home/p")),
        }
    }

    fn drawn(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| {
                r.fields
                    .iter()
                    .map(|f| f.text.as_str())
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .collect()
    }

    fn terminals_of(rows: &[Row]) -> Vec<&TerminalId> {
        rows.iter()
            .filter_map(|r| match &r.target {
                Some(Target::Terminal(id)) => Some(id),
                _ => None,
            })
            .collect()
    }

    /// FAILS IF: a terminal a window shows is listed (the view would list
    /// more than the mark counts), one ended in the background is not (it
    /// lingers, unreachable, until it is seen), a window's group is split or
    /// out of its strip's order, the newest is not first, or the alone come
    /// after the groups.
    #[test]
    fn the_background_is_listed_newest_first_and_a_window_together() {
        let all = census();
        let t = texts("en-GB");
        let v = View::fresh(&reading(&all), 120, &t);
        let rows = v.rows(&reading(&all), 120, &t);
        let listed = terminals_of(&rows);
        assert_eq!(listed.len(), 5, "{:?}", drawn(&rows));
        assert!(
            !listed.contains(&&all[2].id),
            "a terminal with a window is listed"
        );
        // ALONE FIRST, newest first: the agent (30 s), then the shell.
        assert_eq!(listed[0], &all[1].id);
        assert_eq!(listed[1], &all[0].id);
        // THE WINDOW'S THREE, in the strip's order (orders 1, 2, 3 ·2¹⁶ are
        // the last, the middle and the first planted).
        assert_eq!(listed[2..], [&all[5].id, &all[4].id, &all[3].id]);
        let text = drawn(&rows).join("\n");
        assert!(text.contains(&t.t(crate::key::TERMINALS_ALONE)), "{text}");
        assert!(
            text.contains("~/prj/ronoctua"),
            "the folder is not under ~: {text}"
        );
    }

    /// FAILS IF: a row does not say what the program reports of itself in
    /// its glyph and word (a subagent's record taken for the terminal's own,
    /// or the oldest record for the newest), a terminal that ended does not
    /// say so with its code, or a state that asks for the person is not in
    /// its colour off the cursor.
    #[test]
    fn a_row_says_what_its_program_reports_and_how_it_ended() {
        let mut all = census();
        all[1].statuses.push(StatusRecord {
            id: "agent/sub".to_owned(),
            ..status(ProgramState::Error, 9)
        });
        let t = texts("en-GB");
        let r = reading(&all);
        let v = View::fresh(&r, 160, &t);
        let rows = v.rows(&r, 160, &t);
        let agent = rows
            .iter()
            .find(|row| row.target == Some(Target::Terminal(all[1].id.clone())))
            .unwrap();
        let state = agent.fields.last().unwrap();
        assert_eq!(
            state.text,
            format!(
                "{} {}",
                tokens::icon::PROGRAM_BLOCKED,
                t.t(crate::key::TERMINALS_BLOCKED)
            )
        );
        let ended = rows
            .iter()
            .find(|row| row.target == Some(Target::Terminal(all[3].id.clone())))
            .unwrap();
        assert!(
            ended.fields.last().unwrap().text.contains('2'),
            "{:?}",
            ended.fields
        );
        assert_eq!(
            ended.fields.last().unwrap().color,
            tokens::color::STATE_ERROR
        );
        // THE CURSOR IS A COLOUR: under it, the name takes the accent and the
        // state says what the cursor says; off it, the state is said in its
        // own colour.
        assert_eq!(v.cursor(), 1, "{:?}", drawn(&rows));
        let (here_name, here_rest) = crate::chrome::Kind::ListedSetting.tones(true);
        assert_eq!(agent.fields[0].color, here_name);
        assert_eq!(state.color, here_rest, "{:?}", agent.fields);
        let mut moved = View::fresh(&r, 160, &t);
        let _ = moved.key(Press::Down, &r, 160, &t);
        let rows = moved.rows(&r, 160, &t);
        let agent = rows
            .iter()
            .find(|row| row.target == Some(Target::Terminal(all[1].id.clone())))
            .unwrap();
        assert_eq!(
            agent.fields[0].color,
            crate::chrome::Kind::ListedSetting.tones(false).0
        );
        assert_eq!(
            agent.fields.last().unwrap().color,
            tokens::color::STATE_CAUTION,
            "{:?}",
            agent.fields
        );
    }

    /// FAILS IF: the groups' headings of one view say the same thing (two
    /// windows of two terminals each would read alike), or a count reaches
    /// its selector as text.
    #[test]
    fn two_windows_of_one_size_are_told_apart() {
        let (a, b) = (group(), group());
        let all: Vec<TerminalState> = [&a, &a, &b, &b]
            .iter()
            .enumerate()
            .map(|(n, g)| planted("zsh", g, u32::try_from(n).unwrap() << 16, NOW - 100))
            .collect();
        for language in ["en-GB", "pt-BR"] {
            let t = texts(language);
            let rows = View::fresh(&reading(&all), 100, &t).rows(&reading(&all), 100, &t);
            let headings: Vec<String> = rows
                .iter()
                .filter(|r| {
                    r.fields
                        .first()
                        .is_some_and(|f| f.color == tokens::color::TEXT_HEADING)
                })
                .map(|r| r.fields[0].text.clone())
                .collect();
            assert_eq!(headings.len(), 2, "{headings:?}");
            assert_ne!(headings[0], headings[1], "{language}");
            assert!(headings.iter().all(|h| h.contains('2')), "{headings:?}");
        }
    }

    /// FAILS IF: with nothing in the background the view is empty instead
    /// of saying so (a refusal is a sentence, and so is nothing), or the
    /// cursor sits on a row that takes nothing.
    #[test]
    fn nothing_in_the_background_is_said() {
        let t = texts("en-GB");
        let r = reading(&[]);
        let mut v = View::fresh(&r, 80, &t);
        let rows = v.rows(&r, 80, &t);
        assert!(
            drawn(&rows)
                .iter()
                .any(|l| l.contains(&t.t(crate::key::TERMINALS_NONE)))
        );
        assert!(rows.iter().all(|r| !r.takes()));
        assert_eq!(v.key(Press::Enter, &r, 80, &t), Errand::Nothing);
        assert_eq!(v.key(Press::Escape, &r, 80, &t), Errand::Close);
    }

    /// FAILS IF: `RET` on a terminal does not show that terminal, a click on
    /// a row the cursor is not on acts (the first click selects — invariant
    /// 19), or a click on a heading moves the cursor.
    #[test]
    fn the_first_click_selects_and_the_second_shows() {
        let all = census();
        let t = texts("en-GB");
        let r = reading(&all);
        let mut v = View::fresh(&r, 120, &t);
        let rows = v.rows(&r, 120, &t);
        let shell = rows
            .iter()
            .position(|row| row.target == Some(Target::Terminal(all[0].id.clone())))
            .unwrap();
        assert!(!v.clicked(0, &rows), "a heading took a click");
        assert_ne!(v.cursor(), 0);
        assert!(!v.clicked(shell, &rows), "the first click acted");
        assert!(v.clicked(shell, &rows), "the second click did not");
        assert_eq!(
            v.key(Press::Enter, &r, 120, &t),
            Errand::Show(all[0].id.clone())
        );
    }

    /// FAILS IF: `d` ends a terminal without a question, the question does
    /// not draw "no" first with the cursor born on it, a key other than
    /// `RET` on "yes" ends it, "yes" does not — or the question about a
    /// terminal whose program already ended says it hangs a program up.
    #[test]
    fn ending_asks_first_and_no_is_first() {
        let all = census();
        let t = texts("en-GB");
        let r = reading(&all);
        let mut v = View::fresh(&r, 120, &t);
        assert_eq!(v.key(Press::End, &r, 120, &t), Errand::Nothing);
        assert!(v.asks());
        let rows = v.rows(&r, 120, &t);
        let answers: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| matches!(row.target, Some(Target::Answer(_))))
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            rows[answers[0]].target,
            Some(Target::Answer(false)),
            "no first"
        );
        assert_eq!(v.cursor(), answers[0], "the cursor is born on no");
        assert!(drawn(&rows).join(" ").contains("fixing the strip"));
        assert_eq!(v.key(Press::Enter, &r, 120, &t), Errand::Nothing);
        assert!(!v.asks(), "RET on no let the question go");
        assert_eq!(
            v.on(&r, 120, &t),
            Some(Target::Terminal(all[1].id.clone())),
            "the cursor is not back on the terminal asked about"
        );
        v.key(Press::End, &r, 120, &t);
        v.key(Press::Down, &r, 120, &t);
        assert_eq!(v.key(Press::Escape, &r, 120, &t), Errand::Nothing);
        assert!(!v.asks(), "Esc is no");
        v.key(Press::End, &r, 120, &t);
        v.key(Press::Down, &r, 120, &t);
        assert_eq!(
            v.key(Press::Enter, &r, 120, &t),
            Errand::End(all[1].id.clone(), "fixing the strip".to_owned())
        );
        // ONE THAT ENDED is asked as one whose program is gone.
        let mut v = View::fresh(&r, 120, &t);
        let rows = v.rows(&r, 120, &t);
        let ended = rows
            .iter()
            .position(|row| row.target == Some(Target::Terminal(all[3].id.clone())))
            .unwrap();
        v.clicked(ended, &rows);
        v.key(Press::End, &r, 120, &t);
        let said = drawn(&v.rows(&r, 120, &t)).join(" ");
        let close = t.t_with(crate::key::TERMINALS_CLOSE_QUESTION, &[("name", "zsh")]);
        assert!(said.contains(&close), "{said}");
    }

    /// FAILS IF: a terminal a window shows cannot be asked about from outside
    /// (a tab's middle button on the modeline), the question does not draw
    /// "no" first with the cursor on it, a new reading lets it go while the
    /// terminal is held, or the answer — either — leaves the view open,
    /// which was opened for the question alone.
    #[test]
    fn a_question_from_outside_asks_of_a_shown_terminal_and_closes() {
        let mut all = census();
        all[1].viewer = Some(4242);
        let t = texts("en-GB");
        let r = reading(&all);
        let id = all[1].id.clone();
        let mut v = View::fresh(&r, 120, &t);
        assert!(v.ask_to_end(&id, &r, 120, &t));
        assert!(v.asks());
        let rows = v.rows(&r, 120, &t);
        assert_eq!(
            rows[v.cursor()].target,
            Some(Target::Answer(false)),
            "no first"
        );
        assert!(
            drawn(&rows)
                .join(" ")
                .contains(&t.t(crate::key::TERMINALS_END_TITLE))
        );
        v.follow(None, &r, 120, &t);
        assert!(
            v.asks(),
            "a reading let the question about a shown terminal go"
        );
        assert_eq!(
            v.key(Press::Enter, &r, 120, &t),
            Errand::Close,
            "no keeps the view"
        );
        assert!(v.ask_to_end(&id, &r, 120, &t));
        v.key(Press::Down, &r, 120, &t);
        assert_eq!(
            v.key(Press::Enter, &r, 120, &t),
            Errand::EndAndClose(id.clone(), "fixing the strip".to_owned())
        );
        // GONE meanwhile, the question goes with it; never held, never asked.
        assert!(v.ask_to_end(&id, &r, 120, &t));
        let gone: Vec<TerminalState> = all.iter().filter(|t| t.id != id).cloned().collect();
        v.follow(None, &reading(&gone), 120, &t);
        assert!(!v.asks(), "a question outlived its terminal");
        assert!(!v.ask_to_end(&id, &reading(&gone), 120, &t));
    }

    /// FAILS IF: the cursor stays on an INDEX when a new reading puts a row
    /// above it (it would move to another terminal by itself), or a question
    /// outlives the terminal it asks about — gone, or shown in a window
    /// meanwhile, where a "yes" would end what a window shows.
    #[test]
    fn the_cursor_follows_its_terminal_and_a_question_goes_with_it() {
        let mut all = census();
        let t = texts("en-GB");
        let mut v = View::fresh(&reading(&all), 120, &t);
        v.key(Press::Down, &reading(&all), 120, &t);
        let on = v.on(&reading(&all), 120, &t);
        assert_eq!(on, Some(Target::Terminal(all[0].id.clone())));
        // A NEWER ONE, put away a moment ago, comes in above it.
        let newer = planted("vim", &group(), 1 << 16, NOW - 1);
        all.push(newer);
        v.follow(on.as_ref(), &reading(&all), 120, &t);
        assert_eq!(v.on(&reading(&all), 120, &t), on);
        v.key(Press::End, &reading(&all), 120, &t);
        assert!(v.asks());
        // SHOWN IN A WINDOW meanwhile — the modeline's mark, another door —
        // it left the view, and the question with it.
        let mut shown = all.clone();
        shown[0].viewer = Some(7);
        v.follow(None, &reading(&shown), 120, &t);
        assert!(
            !v.asks(),
            "a question about a terminal a window shows now outlived its row"
        );
        v.follow(on.as_ref(), &reading(&all), 120, &t);
        v.key(Press::End, &reading(&all), 120, &t);
        assert!(v.asks());
        let gone: Vec<TerminalState> = all.iter().filter(|x| x.id != all[0].id).cloned().collect();
        v.follow(None, &reading(&gone), 120, &t);
        assert!(!v.asks(), "a question about a terminal that is gone");
    }

    /// FAILS IF: a narrow door draws a column past its row's end, two
    /// columns run into each other, the state — what a person scans the
    /// list for — is cut before the name and the folder are, or a program
    /// or a folder is drawn cut to less than its own name (a column of
    /// ellipses, which says nothing).
    #[test]
    fn a_narrow_door_gives_way_from_the_name_and_the_folder() {
        let all = census();
        for columns in [40, 60, 90, 200] {
            let t = texts("pt-BR");
            let r = reading(&all);
            let rows = View::fresh(&r, columns, &t).rows(&r, columns, &t);
            let (left, room) = crate::chrome::extent(columns);
            for row in rows.iter().filter(|row| row.takes()) {
                let mut end = left;
                for f in &row.fields {
                    assert!(f.at >= end, "{columns}: {:?}", row.fields);
                    end = f.at + width(&f.text);
                }
                assert!(end <= left + room, "{columns}: {:?}", row.fields);
            }
            let agent = rows
                .iter()
                .find(|row| row.target == Some(Target::Terminal(all[1].id.clone())))
                .unwrap();
            assert!(
                agent
                    .fields
                    .last()
                    .unwrap()
                    .text
                    .contains(tokens::icon::PROGRAM_BLOCKED),
                "{columns}: the state was cut: {:?}",
                agent.fields
            );
            for f in rows.iter().flat_map(|row| row.fields.iter().skip(1)) {
                assert!(
                    !f.text.starts_with('…') || f.text.chars().count() > 2,
                    "{columns}: {f:?} is cut to nothing"
                );
            }
        }
    }

    /// FAILS IF: a column with nothing in any row — no program apart from
    /// the name, no state — still spends its two cells of air, which the
    /// folder then loses, or the row it lays out runs past its room.
    #[test]
    fn a_column_of_nothing_costs_no_air() {
        let said = Said {
            terminal: TerminalId::fresh().unwrap(),
            cells: [
                "zsh".to_owned(),
                String::new(),
                "~/a/very/long/folder/name/here".to_owned(),
                "3 min".to_owned(),
                String::new(),
            ],
            tint: None,
        };
        let room = 40;
        let w = widths(&[&said], room);
        assert_eq!((w[1], w[4]), (0, 0));
        // THE ROOM LESS THE NAME, HOW LONG AND THE AIR BETWEEN THE THREE.
        assert_eq!(w[2], room - 3 - 5 - 2 * AIR, "{w:?}");
        let drawn: u16 = w.iter().filter(|&&c| c > 0).map(|c| c + AIR).sum::<u16>() - AIR;
        assert!(drawn <= room, "{w:?}");
    }

    /// FAILS IF: a gesture by name presses another key than the table says,
    /// or a letter held with `Ctrl` is taken for the bare letter.
    #[test]
    fn a_key_is_read_by_its_whole_notation() {
        for (key, _, _) in OF_THE_VIEW {
            let k = ronoctua_keymap::Key::parse(key).unwrap();
            assert!(press_of_key(k).is_some(), "{key}");
        }
        assert_eq!(
            press_of_key(ronoctua_keymap::Key::parse("C-d").unwrap()),
            None
        );
        assert_eq!(
            press_of_key(ronoctua_keymap::Key::parse("ESC").unwrap()),
            Some(Press::Escape)
        );
        for id in [
            "terminals-up",
            "terminals-down",
            "terminals-enter",
            "terminals-end",
        ] {
            assert!(press_of(id).is_some(), "{id}");
        }
    }

    /// FAILS IF: the hint does not teach `Esc` first, or teaches `d` where
    /// there is no terminal to end (on an answer of the question).
    #[test]
    fn the_hint_teaches_the_way_back_first_and_only_what_works_here() {
        let all = census();
        let t = texts("en-GB");
        let r = reading(&all);
        let mut v = View::fresh(&r, 120, &t);
        let keys = |v: &View| -> Vec<String> {
            v.hint(&r, 120, &t)
                .iter()
                .filter_map(|p| match p.press {
                    Some(crate::chrome::Act::Key(k)) => Some(k.notation()),
                    _ => None,
                })
                .collect()
        };
        let here = keys(&v);
        assert_eq!(here.first().map(String::as_str), Some("ESC"), "{here:?}");
        assert!(here.iter().any(|k| k == "d"), "{here:?}");
        v.key(Press::End, &r, 120, &t);
        assert!(!keys(&v).iter().any(|k| k == "d"), "{:?}", keys(&v));
    }
}
