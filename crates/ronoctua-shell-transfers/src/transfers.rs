//! The transfer queue's panel — `LEADER t t`.
//!
//! # What is here and what is not
//!
//! Here: how a job is DRAWN and which key does what to it. Not here: the queue,
//! the copying, and the threads. Those belong to `ronoctua-transfers` and to
//! the unit that owns them, and this view can be exercised against a handful of
//! `Job` records with no daemon, no socket and no disk.
//!
//! It is the same split the file manager has, and the same reason: what decides
//! WHAT to do is a function of state, and a function of state is a thing a test
//! can hold in its hand.
//!
//! # Two doors, one table
//!
//! The window (`LEADER t w`) is a wasm buffer, and it answers the same keys
//! the same way: the table is `ronoctua_protocol::TRANSFERS_KEYS`, which both
//! compile, and so is the answer about which of them a row teaches. The pages
//! are the window's two — the list, and one job a field a line — and giving up
//! asks first in both, on a screen of the view with `no` under the cursor.
//!
//! # Why the cursor is a job ID and not a row number
//!
//! Because the list REORDERS underneath it. What is still happening sorts above
//! what has ended, so the moment a copy finishes every row below it moves —
//! and a cursor that was row three would now be pointing at a different job
//! while the hand had not moved. Aiming at `C-c` and giving up the wrong
//! transfer is exactly the kind of mistake this panel exists to prevent.
//!
//! # Why the drawing reuses the file manager's `Row` and `Field`
//!
//! Because they already are what a row of columns with its own colours is, and
//! the painter already knows how to put one on screen. A second row type would
//! be a second painter, and two painters are how two lists come to disagree
//! about where a column starts.

use ronoctua_keymap::Key;
use ronoctua_tokens as tokens;
use ronoctua_transfers::{Ask, Job, Measure, Stage};

use crate::fields::{Field, Hit, Ink};
use crate::{
    jobs::{identified, subject_of, under_the_pointer},
    queue::opens,
};

/// The bar's width in cells, from the token.
const BAR: u16 = tokens::space::TRANSFERS_BAR;

/// The air after the last column, so the size does not end flush against the
/// screen's edge — the same cell the file manager's table already keeps.
const EDGE_RIGHT: u16 = tokens::space::FILES_EDGERIGHT;

/// The cell before the icon. The file manager's gutter, for the same reason:
/// a glyph touching the border reads as clipped.
const GUTTER: u16 = tokens::space::FILES_GUTTER;

/// Below this many columns the amounts go, and the bar keeps its place.
///
/// The two numbers are what the arithmetic gives, not taste: a row needs the
/// icon, a name worth reading, the bar and the figure. Under [`NO_AMOUNTS`]
/// there is no room for `4,1 GiB / 6,6 GiB` and a name at the same time, and a
/// name cut to six characters names nothing.
const NO_AMOUNTS: u16 = 76;

/// And below this one the bar goes too, leaving the figure — which is the part
/// that actually answers "is it moving".
const NO_BAR: u16 = 52;

/// How wide the amounts column is.
const AMOUNTS: u16 = 20;

/// How wide the figure is: `100%`.
const FIGURE: u16 = 4;

/// What one key produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Nothing happened; do not redraw.
    Nothing,
    /// The cursor or the page moved; redraw.
    Changed,
    /// The view closed.
    Closed,
    /// Send this to the queue.
    ///
    /// The view decides WHAT, and the door sends it — the same split
    /// `report::Doing` has, and for the same reason: this view never opens a
    /// socket, so it can be tested without one.
    Send(Ask),
    /// Open this launcher entry (a desktop id): what a finished install made.
    Launch(String),
}

/// The view's own keys, as DATA — the SAME table the window's buffer binds
/// (`ronoctua_protocol::TRANSFERS_KEYS`), so a key means one thing in both
/// doors of the queue.
///
/// Chords under `C-`, and **never under `M-`**: in the nested loop the
/// leader's chord family IS `Alt`, so `M-` never reaches the footer at all —
/// it opens a terminal. That was reported from the screen once already, and
/// the file manager's table carries the same warning.
pub const OF_THE_VIEW: &[(&str, &str)] = ronoctua_protocol::TRANSFERS_KEYS;

/// Which page of the queue is up — the window's two faces, in this door too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Level {
    /// Every job, one a row.
    #[default]
    List,
    /// One job, one field a line — where the whole failure and the whole
    /// name fit, which a row cannot promise.
    Detail,
}

/// The give-up question on screen: which job, and whether the cursor is on
/// `yes`. It is born on `no` — giving up deletes what was already written, and
/// a question about something that cannot be undone starts on the answer that
/// keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GiveUp {
    id: u64,
    yes: bool,
}

/// What a line does under a click, or under `RET` with the cursor on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A fact, a heading, the question's sentence: nothing.
    Nothing,
    /// A job of the list, by its place in it.
    Job(usize),
    /// An answer to the give-up question.
    Answer(bool),
}

/// One drawn line of the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Placed and cut, in the row's own cells.
    pub fields: Vec<Field>,
    /// What a click on it does.
    pub target: Target,
}

/// The panel's whole state.
#[derive(Debug, Default)]
pub struct View {
    /// The last list the queue sent.
    jobs: Vec<Job>,
    /// Which JOB the cursor is on — see the module header for why it is not a
    /// row number.
    on: Option<u64>,
    /// Which page is up.
    level: Level,
    /// The give-up question, while it is on screen.
    asking: Option<GiveUp>,
}

impl View {
    /// An empty one, before the first list arrives.
    #[must_use]
    pub fn fresh_one() -> Self {
        Self::default()
    }

    /// Takes a new list, keeping the cursor where the person left it.
    ///
    /// If the job it was on is gone — swept, forgotten — it falls to whatever
    /// is now in that place rather than to the top: a cursor that jumps home
    /// every time something finishes is a cursor nobody can use while things
    /// are finishing.
    ///
    /// A QUESTION ABOUT A JOB THAT ENDED goes: there is nothing left to give
    /// up, and a `yes` taken later would be about nothing.
    pub fn refresh(&mut self, jobs: Vec<Job>) {
        let was = self
            .on
            .and_then(|id| self.jobs.iter().position(|j| j.id == id));
        self.jobs = jobs;
        if let Some(q) = self.asking
            && !self.jobs.iter().any(|j| j.id == q.id && j.afoot())
        {
            self.asking = None;
        }
        if let Some(id) = self.on
            && self.jobs.iter().any(|j| j.id == id)
        {
            return;
        }
        self.on = was
            .map(|at| at.min(self.jobs.len().saturating_sub(1)))
            .or(Some(0))
            .and_then(|at| self.jobs.get(at))
            .map(|j| j.id);
    }

    /// Which page is up.
    #[cfg(test)]
    #[must_use]
    pub const fn level(&self) -> Level {
        self.level
    }

    /// Whether the give-up question is on screen.
    #[cfg(test)]
    #[must_use]
    pub const fn asking(&self) -> bool {
        self.asking.is_some()
    }

    /// The job at place `i` of the list — what a row of the list is about.
    #[must_use]
    pub fn job_at(&self, i: usize) -> Option<&Job> {
        self.jobs.get(i)
    }

    /// The job the cursor is on.
    #[must_use]
    pub fn focused(&self) -> Option<&Job> {
        self.on.and_then(|id| self.jobs.iter().find(|j| j.id == id))
    }

    /// Puts the cursor on a job of the LIST, by its place in it.
    pub fn focus_row(&mut self, row: usize) {
        if let Some(j) = self.jobs.get(row) {
            self.on = Some(j.id);
        }
    }

    /// Which job of the list the cursor is on, by its place in it.
    #[must_use]
    pub fn focused_row(&self) -> Option<usize> {
        self.on
            .and_then(|id| self.jobs.iter().position(|j| j.id == id))
    }

    /// The job a drawn LINE is about, when it is one — what the pointer's
    /// sentence reads. The line's and not a job index: the page's lines are
    /// the list's rows only on the list.
    #[must_use]
    pub fn job_on(&self, line: usize, width: u16, texts: &ronoctua_i18n::Texts) -> Option<&Job> {
        match self.lines(width, texts).get(line)?.target {
            Target::Job(i) => self.jobs.get(i),
            Target::Nothing | Target::Answer(_) => None,
        }
    }

    /// How many lines the page has — without building them on the list,
    /// which is the page a footer measures on every motion of the pointer.
    #[must_use]
    pub fn line_count(&self, width: u16, texts: &ronoctua_i18n::Texts) -> usize {
        if self.asking.is_none() && self.level == Level::List {
            return self.jobs.len();
        }
        self.lines(width, texts).len()
    }

    /// Which drawn line the cursor is on: the job's on the list, the answer's
    /// under the question, and none on a job's page, which is all facts.
    #[must_use]
    pub fn cursor_line(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Option<usize> {
        if let Some(q) = self.asking {
            return self
                .lines(width, texts)
                .iter()
                .position(|l| l.target == Target::Answer(q.yes));
        }
        match self.level {
            Level::List => self.focused_row(),
            Level::Detail => None,
        }
    }

    /// One key.
    pub fn key(&mut self, t: Key) -> Step {
        use ronoctua_keymap::Sym;
        let id = of_the_view(t);
        // THE QUESTION TAKES THE KEYS while it is up: the arrows walk its two
        // answers, `RET` takes the one the cursor is on, and `Esc` is `no`.
        if let Some(q) = self.asking.as_mut() {
            return match (t.sym, id) {
                (Sym::Esc, _) => {
                    self.asking = None;
                    Step::Changed
                }
                (Sym::Enter, _) => self.answer(),
                (_, Some("transfers-up")) if q.yes => {
                    q.yes = false;
                    Step::Changed
                }
                (_, Some("transfers-down")) if !q.yes => {
                    q.yes = true;
                    Step::Changed
                }
                _ => Step::Nothing,
            };
        }
        // `Esc` GOES BACK ONE PAGE, and closes from the list.
        if matches!(t.sym, Sym::Esc) {
            if self.level == Level::Detail {
                self.level = Level::List;
                return Step::Changed;
            }
            return Step::Closed;
        }
        let Some(id) = id else {
            return Step::Nothing;
        };
        self.run(id)
    }

    /// What a command of the table does, on the row under the cursor.
    fn run(&mut self, id: &str) -> Step {
        let row = self.focused().map(facts);
        match id {
            "transfers-down" => self.step_by(1),
            "transfers-up" => self.step_by(-1),
            // THE JOB'S OWN PAGE, and back — what a second click on a row does.
            "transfers-detail" => match self.level {
                Level::List if row.is_some() => {
                    self.level = Level::Detail;
                    Step::Changed
                }
                Level::List => Step::Nothing,
                Level::Detail => {
                    self.level = Level::List;
                    Step::Changed
                }
            },
            // TOGGLE, and the row decides which way. A held job resumes, a
            // running one is held, and one that has ended does neither —
            // pressing it on a finished row must not resurrect anything.
            // AND ONLY WHERE THE ENGINE CAN HOLD: a program's order is one
            // `nix`, `flatpak` or `waydroid` run that nothing can freeze
            // halfway (`Engine::pauses`), and the queue refuses it anyway.
            "transfers-pause" => match (self.on, row) {
                (Some(id), Some(r)) if r.held => Step::Send(Ask::Resume { id }),
                (Some(id), Some(r)) if r.afoot && r.pauses => Step::Send(Ask::Pause { id }),
                _ => Step::Nothing,
            },
            // GIVING UP ASKS FIRST, in both doors: what was written is deleted.
            "transfers-cancel" => match (self.on, row) {
                (Some(id), Some(r)) if r.afoot => {
                    self.asking = Some(GiveUp { id, yes: false });
                    Step::Changed
                }
                _ => Step::Nothing,
            },
            // FORGET only takes away a row that has ended. The queue refuses
            // the other case too — this is the half that keeps the key from
            // looking broken when it is being correct.
            "transfers-forget" => match (self.on, row) {
                (Some(id), Some(r)) if !r.afoot => Step::Send(Ask::Forget { id }),
                _ => Step::Nothing,
            },
            "transfers-sweep" => Step::Send(Ask::Sweep),
            "transfers-launch" => self
                .focused()
                .and_then(opens)
                .map_or(Step::Nothing, |id| Step::Launch(id.to_owned())),
            _ => Step::Nothing,
        }
    }

    /// The question's answer, the one the cursor is on.
    fn answer(&mut self) -> Step {
        match self.asking.take() {
            Some(GiveUp { id, yes: true }) => Step::Send(Ask::Cancel { id }),
            Some(GiveUp { yes: false, .. }) => Step::Changed,
            None => Step::Nothing,
        }
    }

    /// A click on drawn line `n`: the FIRST click selects and the SECOND does
    /// what `RET` does there — the job's page, or the answer (invariant 19).
    pub fn clicked(&mut self, n: usize, width: u16, texts: &ronoctua_i18n::Texts) -> Step {
        let Some(target) = self.lines(width, texts).get(n).map(|l| l.target) else {
            return Step::Nothing;
        };
        match target {
            Target::Nothing => Step::Nothing,
            Target::Job(i) if self.focused_row() == Some(i) => self.run("transfers-detail"),
            Target::Job(i) => {
                self.focus_row(i);
                Step::Changed
            }
            Target::Answer(yes) => match self.asking.as_mut() {
                Some(q) if q.yes == yes => self.answer(),
                Some(q) => {
                    q.yes = yes;
                    Step::Changed
                }
                None => Step::Nothing,
            },
        }
    }

    /// Moves the cursor by one, without wrapping.
    ///
    /// No wrapping on purpose: this list changes under the hand, and a cursor
    /// that jumps from the last row to the first is a cursor that lands on a
    /// different job than the eye was following.
    fn step_by(&mut self, by: i32) -> Step {
        let Some(at) = self.focused_row() else {
            return Step::Nothing;
        };
        let next = if by < 0 {
            at.saturating_sub(1)
        } else {
            (at + 1).min(self.jobs.len().saturating_sub(1))
        };
        if next == at {
            return Step::Nothing;
        }
        self.focus_row(next);
        Step::Changed
    }

    /// The page's lines, laid out for a screen this wide: the question, the
    /// job's page, or the list.
    #[must_use]
    pub fn lines(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        if let Some(q) = self.asking {
            return self.question_lines(q, width, texts);
        }
        if self.level == Level::Detail
            && let Some(job) = self.focused()
        {
            return detail_lines(job, width, texts);
        }
        let focused = self.focused_row();
        self.jobs
            .iter()
            .enumerate()
            .map(|(n, j)| Line {
                fields: row_of(j, width, texts, Some(n) == focused),
                target: Target::Job(n),
            })
            .collect()
    }

    /// The question: the job it is about, the sentence, and `no` before
    /// `yes`.
    fn question_lines(&self, q: GiveUp, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
        use crate::key;
        let (left, room) = crate::chrome::extent(width);
        let mut out = Vec::new();
        if let Some(job) = self.jobs.iter().find(|j| j.id == q.id) {
            out.push(Line {
                fields: row_of(job, width, texts, false),
                target: Target::Nothing,
            });
        }
        for line in crate::chrome::note_lines(&texts.t(key::TRANSFERS_WINDOW_CONFIRM_CANCEL), room)
        {
            out.push(Line {
                fields: vec![plain(left, line, tokens::color::TEXT_SECONDARY)],
                target: Target::Nothing,
            });
        }
        for (yes, said) in [
            (false, key::TRANSFERS_GIVE_UP_NO),
            (true, key::TRANSFERS_GIVE_UP_YES),
        ] {
            out.push(Line {
                fields: crate::chrome::field_pair(
                    left,
                    room,
                    &texts.t(said),
                    "",
                    crate::chrome::Kind::Setting,
                    q.yes == yes,
                ),
                target: Target::Answer(yes),
            });
        }
        out
    }

    /// The hint line: the keys of the table this page and this row allow —
    /// `ronoctua_protocol::transfers_taught`, the answer the window's line
    /// reads too.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        if self.asking.is_some() {
            return crate::chrome::hint_of(ronoctua_protocol::TRANSFERS_ANSWER, texts);
        }
        let taught = ronoctua_protocol::transfers_taught(
            self.level == Level::Detail,
            self.focused().map(facts),
        );
        crate::chrome::hint_of_marked(&taught, |_| None, texts)
    }
}

/// What a job allows, as the table's shared answer reads it.
fn facts(job: &Job) -> ronoctua_protocol::TransferRow {
    ronoctua_protocol::TransferRow {
        afoot: job.afoot(),
        held: job.stage == Stage::Paused,
        pauses: job.engine.pauses(),
        opens: opens(job).is_some(),
    }
}

/// One job's page: what moves it, what it is, where it is, who asked, what it
/// is on, how far — the window's detail, field for field.
fn detail_lines(job: &Job, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Line> {
    use crate::key;
    let (left, room) = crate::chrome::extent(width);
    let fact = |label: &str, value: String| Line {
        fields: crate::chrome::field_pair(
            left,
            room,
            &texts.t(label),
            &value,
            crate::chrome::Kind::Fact,
            false,
        ),
        target: Target::Nothing,
    };
    let amount = |n: u64| match job.measure {
        Measure::Bytes => crate::sizes::size_text(n, texts.decimal_mark()),
        Measure::Items => n.to_string(),
    };
    let mut out = vec![Line {
        fields: vec![plain(
            left,
            ronoctua_render::layout::elide_end(&texts.t(key::TRANSFERS_WINDOW_DETAIL), room),
            tokens::color::TEXT_HEADING,
        )],
        target: Target::Nothing,
    }];
    out.push(fact(
        key::TRANSFERS_WINDOW_FIELD_ENGINE,
        texts.t(&format!("transfers-engine-{}", job.engine.word())),
    ));
    out.push(fact(
        key::TRANSFERS_WINDOW_FIELD_ERRAND,
        texts.t(&format!("transfers-work-{}", job.work.word())),
    ));
    out.push(fact(
        key::TRANSFERS_WINDOW_FIELD_STAGE,
        texts.t(&format!("transfers-stage-{}", job.stage.word())),
    ));
    out.push(fact(
        key::TRANSFERS_WINDOW_FIELD_IDENTITY,
        texts.t(if identified(&job.work) {
            key::TRANSFERS_IDENTITY_MINE
        } else {
            key::TRANSFERS_IDENTITY_ANONYMOUS
        }),
    ));
    out.push(fact(
        key::TRANSFERS_WINDOW_FIELD_SUBJECT,
        subject_of(job, texts),
    ));
    out.push(fact(key::TRANSFERS_WINDOW_FIELD_DONE, amount(job.done)));
    out.push(fact(
        key::TRANSFERS_WINDOW_FIELD_TOTAL,
        if job.total == 0 {
            texts.t(key::TRANSFERS_WINDOW_UNKNOWN)
        } else {
            amount(job.total)
        },
    ));
    if let Some(answer) = job.answer {
        out.push(fact(
            key::TRANSFERS_WINDOW_FIELD_ANSWER,
            crate::sizes::size_text(answer, texts.decimal_mark()),
        ));
    }
    if job.trouble.is_some() || job.refusal.is_some() || job.archive.is_some() || job.seal.is_some()
    {
        out.push(fact(
            key::TRANSFERS_WINDOW_FIELD_TROUBLE,
            under_the_pointer(job, texts),
        ));
    }
    // THE BAR AND ITS FIGURE, after a line of air — the window's own line —
    // and, where the bar does not fit, the figure alone: the part that
    // answers "is it moving".
    if job.along().is_some() {
        let at = crate::chrome::value_column(left, room);
        let end = left + room;
        let fields = if at + BAR + 1 + FIGURE <= end {
            let mut fields = bar(at, job);
            fields.push(plain(at + BAR + 1, figure(job), colour_of(job.stage)));
            fields
        } else if at + FIGURE <= end {
            vec![plain(at, figure(job), colour_of(job.stage))]
        } else {
            Vec::new()
        };
        if !fields.is_empty() {
            out.push(Line {
                fields: crate::chrome::AIR,
                target: Target::Nothing,
            });
            out.push(Line {
                fields,
                target: Target::Nothing,
            });
        }
    }
    out
}

/// The id a key runs, if the table has one for it.
#[must_use]
pub fn of_the_view(t: Key) -> Option<&'static str> {
    let notation = t.notation();
    OF_THE_VIEW
        .iter()
        .find(|(k, _)| *k == notation)
        .map(|(_, id)| *id)
}

/// One row, in cells.
///
/// The arithmetic is the same two phases the rest of the system uses: the
/// right-hand columns take fixed widths from the edge inwards, and the label
/// gets whatever is left. Nothing overflows because nothing is placed past what
/// was subtracted first.
fn row_of(job: &Job, width: u16, texts: &ronoctua_i18n::Texts, focused: bool) -> Vec<Field> {
    let mut fields = Vec::with_capacity(5);
    let edge = width.saturating_sub(EDGE_RIGHT);

    // --- from the right: amounts, figure, bar --------------------------
    let mut right = edge;
    if width >= NO_AMOUNTS {
        right = right.saturating_sub(AMOUNTS);
        fields.push(plain(right, amounts(job, texts), tokens::color::TEXT_DIM));
    }
    right = right.saturating_sub(FIGURE + 1);
    fields.push(plain(right, figure(job), colour_of(job.stage)));
    if width >= NO_BAR {
        right = right.saturating_sub(BAR + 1);
        fields.extend(bar(right, job));
    }

    // --- from the left: the stage's glyph, then the sentence ------------
    fields.push(plain(
        GUTTER,
        glyph_of(job.stage).to_string(),
        colour_of(job.stage),
    ));
    let at = GUTTER + 2;
    let room = right.saturating_sub(at + 1);
    // THE SENTENCE CARRIES THE CURSOR, and nothing else on the line does.
    //
    // `Row::focused` was set and drawn by NOBODY — no door in this system
    // paints a band behind the cursor — so this panel had no way to say which
    // job the keys were about. It is the defect the owner read on the shell
    // history's screen, one view over.
    //
    // The NAME takes it, because the rest of the line is fact: the stage's
    // glyph and its figure are coloured by what the job is DOING
    // (`colour_of`), and recolouring those would trade one thing said for
    // another. `text.here.acts` and not `text.here.read`, since every row here
    // answers three keys — pause, cancel and forget.
    fields.push(plain(
        at,
        ronoctua_render::layout::elide_end(&sentence(job, texts), room),
        if focused {
            tokens::color::TEXT_HERE_ACTS
        } else {
            tokens::color::TEXT_PRIMARY
        },
    ));
    fields
}

/// A field with nothing behind it: this panel acts by key, not by cell.
///
/// The file manager's rows carry a [`Hit`] per column because ten of its cells
/// are separate permission bits. A transfer has no such cells — a click here
/// means "this row", which the row itself already answers.
fn plain(at: u16, text: String, color: tokens::Color) -> Field {
    Field {
        at,
        text,
        color,
        hit: Hit::Nothing,
        explain: None,
        ink: Ink::Grid,
    }
}

/// What the row says it is: the work, its subject, and where it lands.
fn sentence(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    let what = texts.t(&format!("transfers-work-{}", job.work.word()));
    // THE BELONGINGS' ORDER IS A WORD OF OURS, not a name: said in the
    // person's language, after the work's.
    if matches!(job.work, ronoctua_transfers::Work::Belongings { .. }) {
        return format!("{what} · {}", subject_of(job, texts));
    }
    let subject = job.work.subjects();
    let name = subject.first();
    let more = job.work.how_many().saturating_sub(1);
    let who = if more == 0 {
        name
    } else {
        texts.t_with(
            crate::key::TRANSFERS_AND_MORE,
            &[("name", name.as_str()), ("rest", &more.to_string())],
        )
    };
    // The name the operation is ON RIGHT NOW beats the set's first name: while
    // twelve files are copying, "which one is it on" is the question, and the
    // set is already counted next to it.
    let who = if job.now.is_empty() || more == 0 {
        who
    } else {
        format!("{who} · {}", job.now)
    };
    let said = match job.work.landing() {
        Some(where_) => {
            let there = where_.file_name().map_or_else(
                || where_.to_string_lossy().into_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            let into = texts.t_with(crate::key::TRANSFERS_INTO, &[("where", there.as_str())]);
            format!("{what} {who} {into}")
        }
        None => format!("{what} {who}"),
    };
    // A PROGRAM'S ORDER SAYS WHERE INSIDE IT IS — downloading, building,
    // putting it in the launcher —, which is the one thing its figure cannot:
    // a build has no percentage anybody can compute honestly.
    match job.phase {
        Some(phase) if job.afoot() => format!(
            "{said} · {}",
            texts.t(crate::software_words::phase_key(phase))
        ),
        _ => said,
    }
}

/// The figure: `62%`, or the stage's name when there is no total to be a
/// percentage of.
fn figure(job: &Job) -> String {
    match job.along() {
        // Per mille in, per cent out: the extra digit is what keeps `99%` off
        // the screen while a gibibyte is still moving, and it is thrown away
        // exactly once, here.
        Some(per) => format!("{}%", per / 10),
        None => String::new(),
    }
}

/// The two amounts, or the count for the work that counts things.
fn amounts(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    // THE ANSWER WINS when there is one. A finished measurement's row should
    // say how big the folder turned out to be, which is the whole reason
    // somebody asked.
    if let Some(answer) = job.answer {
        return crate::sizes::size_text(answer, texts.decimal_mark());
    }
    if job.total == 0 {
        return String::new();
    }
    match job.measure {
        Measure::Bytes => format!(
            "{} / {}",
            crate::sizes::size_text(job.done, texts.decimal_mark()),
            crate::sizes::size_text(job.total, texts.decimal_mark())
        ),
        Measure::Items => texts.t_with(
            crate::key::TRANSFERS_ITEMS,
            &[
                ("done", &job.done.to_string()),
                ("total", &job.total.to_string()),
            ],
        ),
    }
}

/// The bar, TILED: one glyph per cell, because a cell grid does not stretch.
///
/// Two fields and not one string, so the part behind and the part ahead take
/// different token colours — which is the only thing that makes a bar readable
/// without a border, and invariant 15 forbids the border.
fn bar(at: u16, job: &Job) -> Vec<Field> {
    let Some(per) = job.along() else {
        return Vec::new();
    };
    let done = u16::try_from(u32::from(per) * u32::from(BAR) / 1000).unwrap_or(0);
    let left = BAR - done;
    let mut out = Vec::with_capacity(2);
    if done > 0 {
        out.push(plain(
            at,
            tokens::icon::PROGRESS_DONE
                .to_string()
                .repeat(done as usize),
            colour_of(job.stage),
        ));
    }
    if left > 0 {
        out.push(plain(
            at + done,
            tokens::icon::PROGRESS_LEFT
                .to_string()
                .repeat(left as usize),
            tokens::color::TEXT_DIM,
        ));
    }
    out
}

/// The glyph of a stage.
#[must_use]
pub const fn glyph_of(stage: Stage) -> char {
    match stage {
        Stage::Weighing => tokens::icon::TRANSFER_WEIGHING,
        Stage::Running => tokens::icon::TRANSFER_RUNNING,
        Stage::Paused => tokens::icon::TRANSFER_PAUSED,
        // Done and failed deliberately share the drawing of `success` and
        // `error`, which is already this system's rule for the same meaning in
        // two places.
        Stage::Done => tokens::icon::SUCCESS,
        Stage::Failed => tokens::icon::ERROR,
        Stage::Cancelled => tokens::icon::TRANSFER_CANCELLED,
    }
}

/// The colour of a stage.
#[must_use]
pub const fn colour_of(stage: Stage) -> tokens::Color {
    match stage {
        Stage::Running => tokens::color::STATE_ACCENT,
        Stage::Failed => tokens::color::STATE_ERROR,
        Stage::Weighing | Stage::Paused | Stage::Cancelled => tokens::color::TEXT_DIM,
        Stage::Done => tokens::color::TEXT_SECONDARY,
    }
}

// The modeline's segment is the queue's, beside the list the footer hears
// (`queue::segment`): the footer names no feature, and reads the one list.

// ---------------------------------------------------------------------------
// The queue, in the shape the wasm buffer receives it.
//
// `LEADER t w` draws the same list as `LEADER t t`, and this is where the two
// stop sharing code: the footer's view builds `fields::Field` rows, and a
// component builds nothing — it is handed data. The cutting happens HERE, on
// the host, because the buffer has no path type and would not know a separator
// if it saw one (invariant 11).
// ---------------------------------------------------------------------------

/// The transfer queue's footer panel, BESIDE its binding, in the full tree.
///
/// What a sentence teaches is NOT this constant but [`crate::bindings::taught`]'s answer
/// for `transfers-open`: `t` is the queue itself in the default set, and
/// `t t` typed there is the queue and then a key that leads nowhere.
pub const TRANSFERS_KEYS: &str = "t t";

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "transfers",
    commands: &[
        // THE TRANSFER QUEUE. `transfers-open` has a key in the tree
        // (`LEADER t t`); the keys that act on a row live INSIDE the view
        // (`transfers::OF_THE_VIEW`), the same arrangement the file manager
        // uses — they navigate a list rather than being commands of the
        // registry.
        ronoctua_cmd::Command {
            id: "transfers-open",
            label: crate::key::CMD_TRANSFERS_OPEN,
        },
        // AND THE SAME QUEUE IN A WINDOW OF ITS OWN (`LEADER t w`), which is a
        // wasm buffer and not a second implementation: it subscribes to the
        // same socket and draws the same rows with room to breathe. Its own
        // keys are in ITS descriptor, under `SPC m`, and are resolved by its
        // host while it holds the keyboard — not here.
        ronoctua_cmd::Command {
            id: "transfers-window",
            label: crate::key::CMD_TRANSFERS_WINDOW,
        },
        // AND THE EIGHT THAT PUT SOMETHING IN. What they read is the clipboard,
        // because an address is copied and never typed — see
        // `Footer::queue_the_clipboard`.
        //
        // FOUR AND FOUR, and the pairing is the point: everything that can be
        // done through the browser's extension can be done here, and every one
        // of them exists twice — anonymous, and as the person. The app does not
        // depend on the extension for anything.
        ronoctua_cmd::Command {
            id: "transfers-add",
            label: crate::key::CMD_TRANSFERS_ADD,
        },
        ronoctua_cmd::Command {
            id: "transfers-video",
            label: crate::key::CMD_TRANSFERS_VIDEO,
        },
        ronoctua_cmd::Command {
            id: "transfers-audio",
            label: crate::key::CMD_TRANSFERS_AUDIO,
        },
        ronoctua_cmd::Command {
            id: "transfers-subtitles",
            label: crate::key::CMD_TRANSFERS_SUBTITLES,
        },
        ronoctua_cmd::Command {
            id: "transfers-add-as-me",
            label: crate::key::CMD_TRANSFERS_ADD_AS_ME,
        },
        ronoctua_cmd::Command {
            id: "transfers-video-as-me",
            label: crate::key::CMD_TRANSFERS_VIDEO_AS_ME,
        },
        ronoctua_cmd::Command {
            id: "transfers-audio-as-me",
            label: crate::key::CMD_TRANSFERS_AUDIO_AS_ME,
        },
        ronoctua_cmd::Command {
            id: "transfers-subtitles-as-me",
            label: crate::key::CMD_TRANSFERS_SUBTITLES_AS_ME,
        },
    ],
    bindings: &[
        // `LEADER t` IS THE TRANSFER QUEUE. `t t` opens the panel in the
        // footer's region, doubling the key the way `s n n` and `. .` already do
        // — because `t` is a PREFIX, and hanging a command off a prefix is the
        // collision `PrefixWithCommand` exists to flag.
        (TRANSFERS_KEYS, "transfers-open", None),
        // `t w` — the same queue, in a window. `w` for window, and it is the
        // second door of one thing, exactly like the file manager's `. w`.
        ("t w", "transfers-window", None),
        // `t a` — add whatever was copied: a link, a magnet, a `.torrent`.
        ("t a", "transfers-add", None),
        // AND THE THREE THAT SAY WHAT TO TAKE OFF A PAGE. `v` for video, `u`
        // for the middle letter of áudio — `a` is already the plain add —, and
        // `l` for legenda.
        ("t v", "transfers-video", None),
        ("t u", "transfers-audio", None),
        ("t l", "transfers-subtitles", None),
        // THE SAME FOUR, AS THE PERSON. Age-restricted, private, a members'
        // club and 1080p Premium need an account; everything else is better
        // without one.
        ("t i a", "transfers-add-as-me", None),
        ("t i v", "transfers-video-as-me", None),
        ("t i u", "transfers-audio-as-me", None),
        ("t i l", "transfers-subtitles-as-me", None),
    ],
    groups: &[
        // `t` FOR TRANSFERS. It held the screen capture alone and gave the
        // letter up: a queue that everything long goes through earns a
        // first-level prefix, and a single screenshot does not.
        ("t", crate::key::GROUP_TRANSFER),
        // `t i` — AS ME. A prefix of its own and not a switch kept somewhere
        // else: what it costs is anonymity, and a cost paid by a setting is a
        // cost nobody notices paying. Four keys under it, one per thing that
        // can be asked for, so the sequence itself says which of the two
        // happened.
        ("t i", crate::key::GROUP_IDENTIFIED),
    ],
    ..crate::bindings::Contribution::NONE
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{ended, queue_of, what_ended};
    use ronoctua_transfers::{Engine, Work};
    use std::path::PathBuf;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR has to exist")
    }

    fn a_job(id: u64, stage: Stage) -> Job {
        let mut j = Job::fresh_one(
            id,
            Engine::Local,
            Work::Paste {
                paths: vec![PathBuf::from("/home/film.mkv")],
                into: PathBuf::from("/backup"),
                moves: false,
            },
        );
        j.stage = stage;
        j.total = 1000;
        j.done = 620;
        j
    }

    #[test]
    fn the_cursor_follows_the_job_and_not_the_row() {
        // THE DEFECT THIS CATCHES, and it is the reason the cursor is an ID.
        // What is still happening sorts above what has ended, so the moment a
        // copy finishes every row below it moves. A cursor that was row one
        // would now point at a different job with the hand not having moved —
        // and the next `C-c` would give up the wrong transfer.
        let mut view = View::fresh_one();
        view.refresh(vec![a_job(1, Stage::Running), a_job(2, Stage::Running)]);
        view.focus_row(1);
        assert_eq!(view.focused().map(|j| j.id), Some(2));

        // Job 1 ended, so the queue now sends it last.
        view.refresh(vec![a_job(2, Stage::Running), a_job(1, Stage::Done)]);
        assert_eq!(
            view.focused().map(|j| j.id),
            Some(2),
            "the cursor followed the ROW instead of the job"
        );
    }

    #[test]
    fn every_key_in_the_table_does_something_on_a_row_that_allows_it() {
        // The twin of `files::every_view_key_in_the_table_actually_does_something`:
        // a key in the table that no arm answers is a line promising something
        // that does not happen — and the manual would list it.
        for (notation, id) in OF_THE_VIEW {
            let mut view = View::fresh_one();
            let key = Key::parse(notation).expect(notation);
            // OPENING needs a program that was installed and made an entry.
            if *id == "transfers-launch" {
                view.refresh(vec![installed_gimp()]);
                assert_eq!(view.key(key), Step::Launch("org.gimp.GIMP.desktop".into()));
                continue;
            }
            // EACH KEY GETS THE ROW IT NEEDS. `forget` only takes a finished
            // one — that refusal is the point of the key, not a gap in it — and
            // `up` needs somewhere to come from.
            view.refresh(vec![
                a_job(
                    1,
                    if *id == "transfers-forget" {
                        Stage::Done
                    } else {
                        Stage::Running
                    },
                ),
                a_job(2, Stage::Paused),
            ]);
            view.focus_row(usize::from(*id == "transfers-up"));
            assert_ne!(
                view.key(key),
                Step::Nothing,
                "the key `{notation}` (`{id}`) does nothing"
            );
        }
    }

    #[test]
    fn pausing_a_held_job_resumes_it_and_a_finished_one_does_neither() {
        let mut view = View::fresh_one();
        let ret = Key::parse("SPC").unwrap();

        view.refresh(vec![a_job(1, Stage::Running)]);
        assert_eq!(view.key(ret), Step::Send(Ask::Pause { id: 1 }));

        view.refresh(vec![a_job(1, Stage::Paused)]);
        assert_eq!(view.key(ret), Step::Send(Ask::Resume { id: 1 }));

        // A finished row must not be resurrected by the same key that holds a
        // running one — the toggle reads the row, and the row says it is over.
        view.refresh(vec![a_job(1, Stage::Done)]);
        assert_eq!(view.key(ret), Step::Nothing);
    }

    #[test]
    fn nothing_spills_off_a_narrow_screen() {
        // The same guarantee the modeline's own test makes, and the same
        // arithmetic: the right-hand columns are subtracted from the edge
        // first, and the label takes what is left. A row that overflowed would
        // draw on top of the next screen's worth of nothing.
        let view = {
            let mut v = View::fresh_one();
            let mut long = a_job(1, Stage::Running);
            long.now = "n".repeat(300);
            long.work = Work::Paste {
                paths: vec![PathBuf::from("/home/".to_owned() + &"a".repeat(300))],
                into: PathBuf::from("/".to_owned() + &"b".repeat(300)),
                moves: false,
            };
            v.refresh(vec![long]);
            v
        };
        let t = texts();
        let mut detailed = View::fresh_one();
        detailed.refresh(view.jobs.clone());
        detailed.key(Key::parse("RET").unwrap());
        let mut asked = View::fresh_one();
        asked.refresh(view.jobs.clone());
        asked.key(Key::parse("C-c").unwrap());
        for columns in [20_u16, 40, 52, 76, 80, 200] {
            for row in [&view, &detailed, &asked]
                .iter()
                .flat_map(|v| v.lines(columns, &t))
            {
                for f in &row.fields {
                    assert!(
                        f.at + ronoctua_render::layout::width(&f.text) <= columns,
                        "a field leaks at {columns} columns: {f:?}"
                    );
                }
            }
        }
    }

    /// A finished install of GIMP, with the entry it made.
    fn installed_gimp() -> Job {
        let mut j = Job::fresh_one(
            7,
            Engine::Software,
            Work::Software {
                order: Box::new(ronoctua_software::Order::Install {
                    key: ronoctua_software::AppKey::Flatpak {
                        id: "org.gimp.GIMP".into(),
                    },
                    consent: ronoctua_software::order::Consent::default(),
                }),
                label: "GIMP".into(),
            },
        );
        j.stage = Stage::Done;
        j.outcome = Some(ronoctua_software::Outcome {
            key: None,
            desktop_ids: vec!["org.gimp.GIMP.desktop".into()],
            amount: None,
            rehearsed: false,
        });
        j
    }

    #[test]
    fn a_program_s_order_cannot_be_held_and_the_line_does_not_teach_it() {
        let mut view = View::fresh_one();
        let mut running = installed_gimp();
        running.stage = Stage::Running;
        running.outcome = None;
        running.phase = Some(ronoctua_software::progress::Phase::Downloading);
        view.refresh(vec![running]);
        assert_eq!(view.key(Key::parse("SPC").unwrap()), Step::Nothing);
        let t = texts();
        let taught: String = view.hint(&t).iter().map(|p| p.text.as_str()).collect();
        assert!(!taught.contains("SPC"), "{taught}");
        assert!(taught.contains("C-c"), "{taught}");
        // AND THE ROW SAYS WHERE INSIDE THE ORDER IT IS.
        let rows = view.lines(200, &t);
        let said: String = rows[0].fields.iter().map(|f| f.text.as_str()).collect();
        assert!(
            said.contains(&t.t(crate::key::SOFTWARE_PHASE_DOWNLOADING)),
            "{said}"
        );
    }

    #[test]
    fn a_finished_install_opens_from_its_row() {
        let mut view = View::fresh_one();
        view.refresh(vec![installed_gimp()]);
        let t = texts();
        let taught: String = view.hint(&t).iter().map(|p| p.text.as_str()).collect();
        assert!(taught.contains("C-o"), "{taught}");
        // A COPY THAT FINISHED opens nothing.
        view.refresh(vec![a_job(1, Stage::Done)]);
        assert_eq!(view.key(Key::parse("C-o").unwrap()), Step::Nothing);
    }

    /// FAILS IF: the panel loses the window's second page — `RET` opening
    /// the job one field a line, the failure and who asked on it, `Esc`
    /// going back one page and only then closing — or the second click on a
    /// row stops doing what `RET` does.
    #[test]
    fn a_job_has_its_own_page_and_the_second_click_opens_it() {
        let t = texts();
        let mut view = View::fresh_one();
        let mut broke = a_job(2, Stage::Failed);
        broke.trouble = Some("no space left on the disk".into());
        view.refresh(vec![a_job(1, Stage::Running), broke]);
        // THE FIRST CLICK SELECTS, the second opens.
        assert_eq!(view.clicked(1, 200, &t), Step::Changed);
        assert_eq!(view.level(), Level::List);
        assert_eq!(view.clicked(1, 200, &t), Step::Changed);
        assert_eq!(view.level(), Level::Detail);
        let said: String = view
            .lines(200, &t)
            .iter()
            .flat_map(|l| l.fields.iter().map(|f| f.text.clone()))
            .collect();
        for wanted in [
            t.t(crate::key::TRANSFERS_WINDOW_FIELD_ENGINE),
            t.t(crate::key::TRANSFERS_WINDOW_FIELD_IDENTITY),
            t.t(crate::key::TRANSFERS_IDENTITY_ANONYMOUS),
            "no space left on the disk".to_owned(),
        ] {
            assert!(
                said.contains(&wanted),
                "the page did not say {wanted}: {said}"
            );
        }
        // NO LINE OF THE PAGE is a job to click, and the cursor is on none.
        assert!(
            view.lines(200, &t)
                .iter()
                .all(|l| l.target == Target::Nothing)
        );
        assert_eq!(view.cursor_line(200, &t), None);
        // `Esc` GOES BACK ONE PAGE, and only then closes.
        let esc = Key::parse("ESC").unwrap();
        assert_eq!(view.key(esc), Step::Changed);
        assert_eq!(view.level(), Level::List);
        assert_eq!(view.key(esc), Step::Closed);
        // AND `RET` IS THE PAGE, not the hold it used to be.
        assert_eq!(view.key(Key::parse("RET").unwrap()), Step::Changed);
        assert_eq!(view.level(), Level::Detail);
    }

    /// FAILS IF: giving up stops asking first, or the question opens on
    /// `yes`, or `Esc` gives up, or the answer names a row and not the job,
    /// or a click takes an answer without selecting it first.
    #[test]
    fn giving_up_asks_first_with_no_under_the_cursor() {
        let t = texts();
        let mut view = View::fresh_one();
        view.refresh(vec![a_job(7, Stage::Running), a_job(8, Stage::Paused)]);
        let give_up = Key::parse("C-c").unwrap();
        // THE QUESTION, and nothing sent.
        assert_eq!(view.key(give_up), Step::Changed);
        assert!(view.asking());
        let lines = view.lines(200, &t);
        let on = view.cursor_line(200, &t).expect("a cursor on an answer");
        assert_eq!(lines[on].target, Target::Answer(false), "born on yes");
        let no = lines
            .iter()
            .position(|l| l.target == Target::Answer(false))
            .unwrap();
        let yes = lines
            .iter()
            .position(|l| l.target == Target::Answer(true))
            .unwrap();
        assert!(no < yes, "yes is drawn first");
        // `RET` ON NO keeps it; `Esc` is no too.
        assert_eq!(view.key(Key::parse("RET").unwrap()), Step::Changed);
        assert!(!view.asking());
        view.key(give_up);
        assert_eq!(view.key(Key::parse("ESC").unwrap()), Step::Changed);
        assert!(!view.asking());
        // `↓` AND `RET` give it up — the job, by its id.
        view.key(give_up);
        view.key(Key::parse("<down>").unwrap());
        assert_eq!(
            view.key(Key::parse("RET").unwrap()),
            Step::Send(Ask::Cancel { id: 7 })
        );
        // BY THE POINTER: the first click on `yes` selects, the second answers.
        view.key(give_up);
        assert_eq!(view.clicked(yes, 200, &t), Step::Changed);
        assert!(view.asking());
        assert_eq!(
            view.clicked(yes, 200, &t),
            Step::Send(Ask::Cancel { id: 7 })
        );
        // A QUESTION ABOUT A JOB THAT ENDED goes with it.
        view.key(give_up);
        view.refresh(vec![a_job(7, Stage::Done), a_job(8, Stage::Paused)]);
        assert!(!view.asking());
        // AND WHAT HAS ENDED is not asked about.
        assert_eq!(view.key(give_up), Step::Nothing);
    }

    /// FAILS IF: the panel's table stops being the window's — the one the
    /// buffer binds — or the hint teaches a key the row does nothing with.
    #[test]
    fn the_panel_teaches_the_windows_table() {
        assert!(std::ptr::eq(OF_THE_VIEW, ronoctua_protocol::TRANSFERS_KEYS));
        let t = texts();
        let mut view = View::fresh_one();
        view.refresh(vec![a_job(1, Stage::Done)]);
        let taught: String = view.hint(&t).iter().map(|p| p.text.as_str()).collect();
        assert!(taught.contains("C-k"), "{taught}");
        assert!(!taught.contains("C-c"), "{taught}");
        assert!(!taught.contains("SPC"), "{taught}");
        // EVERY ID OF THE TABLE says what it does, in both languages.
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect(language);
            for (_, id) in OF_THE_VIEW
                .iter()
                .chain(ronoctua_protocol::TRANSFERS_ANSWER)
            {
                for key in [(*id).to_owned(), format!("about-{id}")] {
                    assert_ne!(t.t(&key), key, "{language} has no `{key}`");
                }
            }
        }
    }

    #[test]
    fn a_program_s_refusal_is_a_sentence_and_not_its_word() {
        let mut broke = installed_gimp();
        broke.stage = Stage::Failed;
        broke.outcome = None;
        broke.refusal = Some(ronoctua_software::Refusal::DeclaredInTheFlake);
        broke.trouble = Some("declared-in-the-flake".into());
        let t = texts();
        let said = under_the_pointer(&broke, &t);
        assert_eq!(
            said,
            t.t(crate::software_words::refusal_key(
                ronoctua_software::Refusal::DeclaredInTheFlake
            ))
        );
        // AND ITS END IS NOT THE FILE MANAGER'S, which would echo the word.
        let mut before = broke.clone();
        before.stage = Stage::Running;
        assert!(
            what_ended(
                Some(std::slice::from_ref(&before)),
                std::slice::from_ref(&broke)
            )
            .is_empty()
        );
        assert_eq!(
            ended(
                Some(std::slice::from_ref(&before)),
                std::slice::from_ref(&broke)
            )
            .count(),
            1
        );
        // A FAILURE WITH MORE THAN THE WORD keeps the program's words after it.
        broke.refusal = Some(ronoctua_software::Refusal::ToolFailed);
        broke.trouble = Some("error: collision between two files".into());
        assert!(under_the_pointer(&broke, &t).ends_with("error: collision between two files"));
    }

    #[test]
    fn what_ended_only_fires_once_per_job() {
        // A finished job stays in the history for a long time. Reacting to
        // `Done` on every beat would re-read the directory sixty times a minute
        // for a copy that ended an hour ago.
        let before = vec![a_job(1, Stage::Running)];
        let now = vec![a_job(1, Stage::Done)];
        assert_eq!(what_ended(Some(&before), &now).len(), 1);
        // The same list again: nothing crossed the line this time.
        assert!(what_ended(Some(&now), &now).is_empty());
    }

    /// An archive's job, refused with `why`.
    fn an_archive_job(id: u64, work: Work, why: Option<ronoctua_archive::Why>) -> Job {
        let mut j = Job::fresh_one(id, Engine::Local, work);
        j.stage = if why.is_some() {
            Stage::Failed
        } else {
            Stage::Done
        };
        j.archive = why;
        j
    }

    #[test]
    fn an_archive_refusal_crosses_to_the_window_as_a_sentence_and_its_work_by_its_word() {
        // THE DEFECT: the window drew the refusal's bare word, `wrong-password`,
        // in English in every language, where the row's subject goes.
        let zip = PathBuf::from("/home/a/photos.zip");
        let refused = an_archive_job(
            7,
            Work::ArchiveTest {
                archive: zip.clone(),
                secret: None,
            },
            Some(ronoctua_archive::Why::WrongPassword),
        );
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect(language);
            let queue = queue_of(std::slice::from_ref(&refused), &t);
            let row = &queue.jobs[0];
            assert_eq!(
                row.trouble,
                t.t_with(
                    crate::key::FILES_ARCHIVE_WHY_WRONG_PASSWORD,
                    &[("name", "photos.zip")]
                ),
                "{language}"
            );
            assert!(!row.trouble.contains("wrong-password"), "{language}");
        }
        // EACH OF AN ARCHIVE'S WORKS BY ITS OWN WORD, the footer's own.
        let t = texts();
        let cases = [
            (
                Work::ArchiveEdit {
                    archive: zip.clone(),
                    add: vec![PathBuf::from("/home/a/x")],
                    remove: Vec::new(),
                    secret: None,
                },
                ronoctua_protocol::Errand::ArchiveAdd,
            ),
            (
                Work::ArchiveEdit {
                    archive: zip.clone(),
                    add: Vec::new(),
                    remove: vec!["x".to_owned()],
                    secret: None,
                },
                ronoctua_protocol::Errand::ArchiveRemove,
            ),
            (
                Work::ArchiveTest {
                    archive: zip.clone(),
                    secret: None,
                },
                ronoctua_protocol::Errand::ArchiveTest,
            ),
            (
                Work::ArchiveConvert {
                    from: zip,
                    from_secret: None,
                    to: PathBuf::from("/home/a/photos.7z"),
                    format: None,
                    level: ronoctua_archive::Level::Normal,
                    volume_bytes: None,
                    hide_names: false,
                    secret: None,
                },
                ronoctua_protocol::Errand::ArchiveConvert,
            ),
        ];
        for (work, errand) in cases {
            let word = work.word();
            let row = &queue_of(&[an_archive_job(1, work, None)], &t).jobs[0];
            assert_eq!(row.errand, errand, "{word}");
            assert_eq!(
                row.trouble, "",
                "{word}: a job that went well has no trouble"
            );
        }
    }

    #[test]
    fn a_job_born_and_refused_between_two_lists_still_ended() {
        // THE QUEUE SENDS THE LIST AS IT IS when its reader wakes: a refusal
        // in the instant a job starts was never afoot in any list, and the
        // door that sent it would never have been asked for the password.
        let history = vec![a_job(1, Stage::Done)];
        let zip = PathBuf::from("/home/a/photos.zip");
        let refused = an_archive_job(
            2,
            Work::Extract {
                archive: zip,
                into: PathBuf::from("/home/a/photos"),
                members: Vec::new(),
                secret: None,
            },
            Some(ronoctua_archive::Why::NeedsPassword),
        );
        let now = vec![a_job(1, Stage::Done), refused];
        match what_ended(Some(&history), &now).as_slice() {
            [crate::report::Report::Archive(r)] => assert!(matches!(
                **r,
                crate::report::ArchiveReport::JobRefused { .. }
            )),
            other => panic!("{other:?}"),
        }
        // THE FIRST LIST A DOOR GETS IS HISTORY, and none of it ended now.
        assert!(what_ended(None, &now).is_empty());
        // And the same list again says nothing twice.
        assert!(what_ended(Some(&now), &now).is_empty());
        // AN EMPTY LIST IS A LIST, and the usual one before a session's first
        // job: the queue numbers its jobs from one each time it starts.
        let first = vec![now[1].clone()];
        match what_ended(Some(&[]), &first).as_slice() {
            [crate::report::Report::Archive(r)] => assert!(matches!(
                **r,
                crate::report::ArchiveReport::JobRefused { .. }
            )),
            other => panic!("{other:?}"),
        }
        assert!(what_ended(None, &first).is_empty());
    }

    #[test]
    fn an_archive_job_that_failed_without_a_word_says_its_words_and_lets_go() {
        // THE PASSWORD KEPT FOR ASKING AGAIN went only with a refusal, a
        // success or a stop: a job that failed for any other reason left it in
        // the view for as long as the footer lived.
        let zip = PathBuf::from("/home/a/photos.zip");
        let work = Work::ArchiveTest {
            archive: zip,
            secret: None,
        };
        let mut before = an_archive_job(3, work.clone(), None);
        before.stage = Stage::Running;
        let mut broke = before.clone();
        broke.stage = Stage::Failed;
        broke.trouble = Some("the folder could not be read".to_owned());
        match what_ended(Some(std::slice::from_ref(&before)), &[broke]).as_slice() {
            [
                crate::report::Report::Failed(why),
                crate::report::Report::Archive(r),
            ] => {
                assert_eq!(why, "the folder could not be read");
                assert_eq!(**r, crate::report::ArchiveReport::Ended(Box::new(work)));
            }
            other => panic!("{other:?}"),
        }
        // AND ANY OTHER FAILURE stays one report.
        let before = vec![a_job(1, Stage::Running)];
        let mut copy = a_job(1, Stage::Failed);
        copy.trouble = Some("no space".to_owned());
        assert_eq!(what_ended(Some(&before), &[copy]).len(), 1);
    }

    #[test]
    fn a_failure_carries_its_own_words_across() {
        // `copy /x/y` alone does not say `No space left on device`, and the
        // second half is the whole diagnosis.
        let before = vec![a_job(1, Stage::Running)];
        let mut broke = a_job(1, Stage::Failed);
        broke.trouble = Some("no space left on the disk".to_owned());
        match what_ended(Some(&before), &[broke]).first() {
            Some(crate::report::Report::Failed(why)) => {
                assert_eq!(why, "no space left on the disk")
            }
            other => panic!("{other:?}"),
        }
    }

    /// FAILS IF: a belongings order is drawn as the CLI's English verb — on
    /// the footer's row, the window's page or the buffer's row — instead of
    /// in the person's language.
    #[test]
    fn a_belongings_order_is_said_in_the_persons_language() {
        let job = Job::fresh_one(
            9,
            Engine::Belongings,
            Work::Belongings {
                op: ronoctua_belongings::order::Op::Apply,
            },
        );
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a declared language");
            let said = t.t(crate::belongings_words::op_key(
                ronoctua_belongings::order::Op::Apply,
            ));
            let row = sentence(&job, &t);
            assert!(row.contains(&said), "{language}: {row}");
            assert!(!row.contains("apply"), "{language}: the CLI's verb: {row}");
            assert_eq!(
                queue_of(std::slice::from_ref(&job), &t).jobs[0].subject,
                said
            );
            assert_eq!(subject_of(&job, &t), said);
        }
    }
}
