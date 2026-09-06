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

use crate::files::{Field, Hit, Row};

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
    /// The cursor moved; redraw.
    Changed,
    /// The view closed.
    Closed,
    /// Send this to the queue.
    ///
    /// The view decides WHAT, and the door sends it — the same split
    /// `files::Doing` has, and for the same reason: this view never opens a
    /// socket, so it can be tested without one.
    Send(Ask),
}

/// The view's own keys, as DATA.
///
/// All under `C-`, and **never under `M-`**: in the nested loop the leader's
/// chord family IS `Alt`, so `M-` never reaches the footer at all — it opens a
/// terminal. That was reported from the screen once already, and the file
/// manager's table carries the same warning.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "transfers-down"),
    ("C-n", "transfers-down"),
    ("<up>", "transfers-up"),
    ("C-p", "transfers-up"),
    // `RET` toggles, because holding and letting go are one gesture with two
    // names. Two keys would mean looking at the row to find out which one to
    // press, and the row is what one is trying to act on.
    ("RET", "transfers-pause"),
    ("C-c", "transfers-cancel"),
    ("C-k", "transfers-forget"),
    ("C-l", "transfers-sweep"),
];

/// The panel's whole state.
#[derive(Debug, Default)]
pub struct View {
    /// The last list the queue sent.
    jobs: Vec<Job>,
    /// Which JOB the cursor is on — see the module header for why it is not a
    /// row number.
    on: Option<u64>,
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
    pub fn refresh(&mut self, jobs: Vec<Job>) {
        let was = self
            .on
            .and_then(|id| self.jobs.iter().position(|j| j.id == id));
        self.jobs = jobs;
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

    /// How many rows there are. Zero means the "nothing under way" line.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.jobs.len()
    }

    /// The job the cursor is on.
    #[must_use]
    pub fn focused(&self) -> Option<&Job> {
        self.on.and_then(|id| self.jobs.iter().find(|j| j.id == id))
    }

    /// Puts the cursor on a row of the LIST — what a click does.
    ///
    /// The list's row and not the page's: what the click carries is
    /// `Click::Transfers(n)`, built by `expansion()` over the whole list before
    /// it was cut into a page. Taking a page index here would name a different
    /// transfer the moment the region scrolled.
    pub fn focus_row(&mut self, row: usize) {
        if let Some(j) = self.jobs.get(row) {
            self.on = Some(j.id);
        }
    }

    /// The job on a row of the LIST — see [`View::focus_row`] for which index
    /// this is, and why it matters.
    #[must_use]
    pub fn at(&self, row: usize) -> Option<&Job> {
        self.jobs.get(row)
    }

    /// Which row the cursor is on, for the drawing's highlight.
    #[must_use]
    pub fn focused_row(&self) -> Option<usize> {
        self.on
            .and_then(|id| self.jobs.iter().position(|j| j.id == id))
    }

    /// One key.
    pub fn key(&mut self, t: Key) -> Step {
        // ESC IS NOT IN THE TABLE, deliberately: every view in this system
        // closes with it, and a line for it would say what the whole system
        // already says. The file manager's own table carries the same note.
        if matches!(t.sym, ronoctua_keymap::Sym::Esc) {
            return Step::Closed;
        }
        let Some(id) = of_the_view(t) else {
            return Step::Nothing;
        };
        match id {
            "transfers-down" => self.step_by(1),
            "transfers-up" => self.step_by(-1),
            // TOGGLE, and the row decides which way. A held job resumes, a
            // running one is held, and one that has ended does neither —
            // pressing it on a finished row must not resurrect anything.
            "transfers-pause" => match self.focused().map(|j| (j.id, j.stage)) {
                Some((id, Stage::Paused)) => Step::Send(Ask::Resume { id }),
                Some((id, s)) if s.afoot() => Step::Send(Ask::Pause { id }),
                _ => Step::Nothing,
            },
            "transfers-cancel" => match self.focused() {
                Some(j) if j.afoot() => Step::Send(Ask::Cancel { id: j.id }),
                _ => Step::Nothing,
            },
            // FORGET only takes away a row that has ended. The queue refuses
            // the other case too — this is the half that keeps the key from
            // looking broken when it is being correct.
            "transfers-forget" => match self.focused() {
                Some(j) if !j.afoot() => Step::Send(Ask::Forget { id: j.id }),
                _ => Step::Nothing,
            },
            "transfers-sweep" => Step::Send(Ask::Sweep),
            _ => Step::Nothing,
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

    /// The rows, laid out for a screen this wide.
    #[must_use]
    pub fn rows(&self, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Row> {
        let focused = self.focused_row();
        self.jobs
            .iter()
            .enumerate()
            .map(|(n, j)| Row {
                fields: row_of(j, width, texts),
                marked: false,
                focused: Some(n) == focused,
                foot: false,
            })
            .collect()
    }

    /// The hint line: which key does what, with the keys read from the table.
    ///
    /// Interpolated and never written into the text, for the reason the file
    /// manager's own hint gives: keys move, and a sentence naming the old one
    /// is worse than no sentence at all.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        let of = |id: &str| key_that_runs(id).map_or_else(String::new, |t| t.notation());
        let (pause, cancel, sweep) = (
            of("transfers-pause"),
            of("transfers-cancel"),
            of("transfers-sweep"),
        );
        let marked = texts.t_marked(
            ronoctua_i18n::key::TRANSFERS_HINT,
            &[
                ("pause", pause.as_str()),
                ("cancel", cancel.as_str()),
                ("sweep", sweep.as_str()),
            ],
        );
        let mut out = Vec::new();
        crate::chrome::Piece::extend(
            &mut out,
            &marked,
            &[pause.as_str(), cancel.as_str(), sweep.as_str()],
        );
        out
    }
}

/// The KEY that runs a given id, read out of the same table. See
/// `files::key_that_runs` for why the mouse and the hint read it rather than
/// writing a key out by hand.
#[must_use]
pub fn key_that_runs(id: &str) -> Option<Key> {
    OF_THE_VIEW
        .iter()
        .find(|(_, command)| *command == id)
        .and_then(|(notation, _)| Key::parse(notation).ok())
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
fn row_of(job: &Job, width: u16, texts: &ronoctua_i18n::Texts) -> Vec<Field> {
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
    fields.push(plain(
        at,
        ronoctua_render::layout::elide_end(&sentence(job, texts), room),
        tokens::color::TEXT_PRIMARY,
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
    }
}

/// What the row says it is: the work, its subject, and where it lands.
fn sentence(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    let what = texts.t(&format!("transfers-work-{}", job.work.word()));
    let subject = job.work.subjects();
    let name = subject.first();
    let more = job.work.how_many().saturating_sub(1);
    let who = if more == 0 {
        name
    } else {
        texts.t_with(
            ronoctua_i18n::key::TRANSFERS_AND_MORE,
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
    match job.work.landing() {
        Some(where_) => {
            let there = where_.file_name().map_or_else(
                || where_.to_string_lossy().into_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            let into = texts.t_with(
                ronoctua_i18n::key::TRANSFERS_INTO,
                &[("where", there.as_str())],
            );
            format!("{what} {who} {into}")
        }
        None => format!("{what} {who}"),
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
        return crate::files::size_text(answer, texts.decimal_mark());
    }
    if job.total == 0 {
        return String::new();
    }
    match job.measure {
        Measure::Bytes => format!(
            "{} / {}",
            crate::files::size_text(job.done, texts.decimal_mark()),
            crate::files::size_text(job.total, texts.decimal_mark())
        ),
        Measure::Items => texts.t_with(
            ronoctua_i18n::key::TRANSFERS_ITEMS,
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

/// What the row says under the pointer, and what a finished job leaves behind.
///
/// A failure's own words, when there are any: a row that says only "failed" is
/// a row that sends somebody to the journal.
#[must_use]
pub fn under_the_pointer(job: &Job, texts: &ronoctua_i18n::Texts) -> String {
    if let Some(trouble) = &job.trouble {
        return trouble.clone();
    }
    let stage = texts.t(&format!("transfers-stage-{}", job.stage.word()));
    let engine = texts.t(&format!("transfers-engine-{}", job.engine.word()));
    format!("{stage} · {engine}")
}

// ---------------------------------------------------------------------------
// The thread that listens to the queue, and what the modeline says about it
// ---------------------------------------------------------------------------

/// Brings up the thread that watches the queue and pushes every list into
/// `calloop`.
///
/// It gives back `false` when there is nowhere to put a socket — and **that is
/// not fatal**. Without it the footer loses the percentage and the panel and
/// goes on drawing everything else; taking the shell down over a missing
/// `XDG_RUNTIME_DIR` would trade one feature for all of them. It is the same
/// criterion as `daemon::listen_to_niri` and the control socket.
///
/// # Why a thread and not the socket in `calloop`
///
/// The same two reasons the niri stream gives. A JSON line can arrive split
/// across two reads, which in the main loop is a state machine and on a thread
/// with a blocking `read_line` is one line of code. And the client RECONNECTS —
/// it starts the daemon when nobody is home — which is a sleep and a retry, and
/// a sleep in the loop that draws is a frozen footer.
pub fn listen(
    channel: smithay_client_toolkit::reexports::calloop::channel::Sender<Vec<Job>>,
) -> bool {
    if let Err(e) = ronoctua_transfers::wire::socket_path() {
        tracing::warn!(error = %e, "no transfer queue; the segment does not appear");
        return false;
    }
    let built = std::thread::Builder::new()
        .name("transfers".into())
        .spawn(move || {
            // The closure says "stop" only when the other end has gone, which
            // means the footer is already on its way out. There is nothing
            // useful to log in a shell that is quitting.
            let _ = ronoctua_transfers::client::watch(|jobs| channel.send(jobs).is_err());
        });
    match built {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(error = %e, "could not start the transfers thread");
            false
        }
    }
}

/// The modeline's segment text, or `None` when there is nothing to say.
///
/// # Why it disappears, when the notification's never does
///
/// Because they answer different questions. The bell is the only indicator that
/// something is PENDING, so a screen without it says "nothing is waiting" — and
/// it has to be able to say that. A transfer segment at rest would say `0%`
/// for ever, which is a number that means nothing and a column that costs
/// something on every screen this system runs on.
///
/// It stays for a FAILURE, though. A copy that died two minutes ago leaves
/// nothing running, and a segment that only showed work in flight would take
/// the failure off screen before anybody saw it.
#[must_use]
pub fn segment(jobs: &[Job]) -> Option<String> {
    use ronoctua_transfers::ledger;
    let afoot = ledger::afoot(jobs);
    if afoot == 0 {
        return ledger::troubled(jobs)
            .then(|| format!("{} {}", tokens::icon::ERROR, tokens::icon::TRANSFER));
    }
    // THE FIGURE WHEN THERE IS ONE, the COUNT when there is not. While
    // everything is still being weighed nothing knows its total, and `0%` would
    // be a number somebody could act on wrongly — `2` is the truth.
    Some(match ledger::along(jobs) {
        Some(per) => format!("{} {}%", tokens::icon::TRANSFER, per / 10),
        None => format!("{} {afoot}", tokens::icon::TRANSFER),
    })
}

/// What ENDED between two lists, as the file manager's own reports.
///
/// # Why this exists at all
///
/// The window door is a second process with no panel and no modeline: all it
/// needs from the queue is "something you asked for is over, read the directory
/// again" and "that folder turned out to be this big". Handing it the whole
/// job model would make it draw one, and there is nothing there to draw it in.
///
/// The comparison is against the PREVIOUS list, not against a stage: a finished
/// job stays in the history for a long time, and reacting to `Done` on every
/// beat would re-read the directory sixty times a minute for a copy that ended
/// an hour ago.
#[must_use]
pub fn what_ended(before: &[Job], now: &[Job]) -> Vec<crate::files::Report> {
    now.iter()
        .filter(|j| !j.afoot())
        .filter(|j| before.iter().any(|old| old.id == j.id && old.afoot()))
        .map(|j| match (&j.work, j.answer, j.stage) {
            (ronoctua_transfers::Work::Measure { path }, Some(bytes), _) => {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                crate::files::Report::Measured(name, bytes)
            }
            // A FAILURE CARRIES ITS OWN WORDS. `copiar /x/y` alone does not say
            // `No space left on device`, and the second half is the whole
            // diagnosis.
            (_, _, Stage::Failed) => crate::files::Report::Failed(
                j.trouble
                    .clone()
                    .unwrap_or_else(|| j.stage.word().to_owned()),
            ),
            _ => crate::files::Report::Done,
        })
        .collect()
}

/// Listens to the queue and turns each list into the file manager's reports.
///
/// What the WINDOW door uses — see [`what_ended`]. `false` when there is
/// nowhere to put a socket, and that is not fatal: the window goes on working,
/// and its listing refreshes on the next key instead of by itself.
pub fn listen_as_reports(
    channel: smithay_client_toolkit::reexports::calloop::channel::Sender<crate::files::Report>,
) -> bool {
    if ronoctua_transfers::wire::socket_path().is_err() {
        return false;
    }
    let built = std::thread::Builder::new()
        .name("transfers".into())
        .spawn(move || {
            let mut before: Vec<Job> = Vec::new();
            let _ = ronoctua_transfers::client::watch(|jobs| {
                for report in what_ended(&before, &jobs) {
                    if channel.send(report).is_err() {
                        return true;
                    }
                }
                before = jobs;
                false
            });
        });
    built.is_ok()
}

// ---------------------------------------------------------------------------
// The queue, in the shape the wasm buffer receives it.
//
// `LEADER t w` draws the same list as `LEADER t t`, and this is where the two
// stop sharing code: the footer's view builds `files::Field` rows, and a
// component builds nothing — it is handed data. The cutting happens HERE, on
// the host, because the buffer has no path type and would not know a separator
// if it saw one (invariant 11).
// ---------------------------------------------------------------------------

/// The queue's own model, as [`ronoctua_protocol::Errand`].
///
/// An exhaustive `match` and not `Work::word()` parsed back: a work variant
/// added without an arm here does not compile, where a string lookup would
/// simply draw the wrong word.
const fn errand_of(work: &ronoctua_transfers::Work) -> ronoctua_protocol::Errand {
    use ronoctua_protocol::Errand as E;
    use ronoctua_transfers::Work as W;
    match work {
        W::Paste { moves: true, .. } => E::MoveIt,
        W::Paste { moves: false, .. } => E::Copy,
        W::Delete { .. } => E::Delete,
        W::Shred { .. } => E::Shred,
        W::Trash { .. } => E::Trash,
        W::Restore { .. } => E::Restore,
        W::Compress { .. } => E::Compress,
        W::Extract { .. } => E::Extract,
        W::Measure { .. } => E::Measure,
        W::Fetch { .. } => E::Fetch,
        W::Media {
            want: ronoctua_transfers::Want::Video,
            ..
        } => E::Media,
        W::Media {
            want: ronoctua_transfers::Want::Audio,
            ..
        } => E::Audio,
        W::Media {
            want: ronoctua_transfers::Want::Subtitles,
            ..
        } => E::Subtitles,
        W::Torrent { .. } => E::Torrent,
    }
}

/// And the engine.
const fn engine_of(engine: ronoctua_transfers::Engine) -> ronoctua_protocol::Engine {
    use ronoctua_protocol::Engine as E;
    match engine {
        ronoctua_transfers::Engine::Local => E::Local,
        ronoctua_transfers::Engine::Fetch => E::Fetch,
        ronoctua_transfers::Engine::Media => E::Media,
        ronoctua_transfers::Engine::Torrent => E::Torrent,
    }
}

const fn stage_of(stage: Stage) -> ronoctua_protocol::Stage {
    use ronoctua_protocol::Stage as S;
    match stage {
        Stage::Weighing => S::Weighing,
        Stage::Running => S::Running,
        Stage::Paused => S::Paused,
        Stage::Done => S::Done,
        Stage::Failed => S::Failed,
        Stage::Cancelled => S::Cancelled,
    }
}

const fn amount_of(measure: Measure) -> ronoctua_protocol::Amount {
    match measure {
        Measure::Bytes => ronoctua_protocol::Amount::Bytes,
        Measure::Items => ronoctua_protocol::Amount::Items,
    }
}

/// What the row says it is working on.
///
/// The name it is on RIGHT NOW beats the set's first name, for the same reason
/// the footer's sentence prefers it: while twelve files are copying, "which one
/// is it on" is the question. Already cut to a file name — a component drawing
/// a whole path would push everything else off the row.
fn subject_of(job: &Job) -> String {
    if !job.now.is_empty() {
        return job.now.clone();
    }
    job.work.subjects().first()
}

/// Whether the work went out as the person.
///
/// It comes out of the WORK and not out of a setting: the request already
/// carried who was asking, and a screen that read a setting would say what is
/// true NOW rather than what was true when the job started.
const fn identified(work: &ronoctua_transfers::Work) -> bool {
    use ronoctua_transfers::Identity;
    use ronoctua_transfers::Work as W;
    matches!(
        work,
        W::Fetch {
            identity: Identity::Mine,
            ..
        } | W::Media {
            identity: Identity::Mine,
            ..
        }
    )
}

/// The queue as the buffer receives it.
#[must_use]
pub fn queue_of(jobs: &[Job]) -> ronoctua_protocol::Queue {
    ronoctua_protocol::Queue {
        jobs: jobs
            .iter()
            .map(|j| ronoctua_protocol::Job {
                id: j.id,
                engine: engine_of(j.engine),
                errand: errand_of(&j.work),
                stage: stage_of(j.stage),
                amount: amount_of(j.measure),
                done: j.done,
                total: j.total,
                subject: subject_of(j),
                // The failure is DATA, and it crosses as it is: it was written
                // by whoever failed, and a buffer that received a key here
                // would need the host to have a key for every `io::Error`
                // there will ever be.
                trouble: j.trouble.clone().unwrap_or_default(),
                answer: j.answer.unwrap_or(0),
                identified: identified(&j.work),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let mut view = View::fresh_one();
        for (notation, id) in OF_THE_VIEW {
            let key = Key::parse(notation).expect(notation);
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
        let ret = Key::parse("RET").unwrap();

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
        for columns in [20_u16, 40, 52, 76, 80, 200] {
            for row in view.rows(columns, &t) {
                for f in &row.fields {
                    assert!(
                        f.at + ronoctua_render::layout::width(&f.text) <= columns,
                        "a field leaks at {columns} columns: {f:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_segment_says_the_count_before_it_can_say_a_figure() {
        // THE DEFECT THIS CATCHES: `0%` while two folders are being weighed.
        // Nothing knows its total yet, and a zero is a number somebody would
        // read as "stuck" — the count is what is actually true.
        let weighing = vec![
            Job::fresh_one(
                1,
                Engine::Local,
                Work::Measure {
                    path: PathBuf::from("/a"),
                },
            ),
            Job::fresh_one(
                2,
                Engine::Local,
                Work::Measure {
                    path: PathBuf::from("/b"),
                },
            ),
        ];
        let said = segment(&weighing).expect("two running and there is no segment");
        assert!(said.ends_with('2'), "{said}");
        assert!(!said.contains('%'), "it invented a percentage: {said}");

        // With a total, the figure. Per mille in, per cent out.
        assert_eq!(
            segment(&[a_job(1, Stage::Running)]),
            Some(format!("{} 62%", tokens::icon::TRANSFER))
        );

        // AT REST THERE IS NO SEGMENT. A permanent `0%` is a column spent on a
        // number that means nothing.
        assert_eq!(segment(&[]), None);
        assert_eq!(segment(&[a_job(1, Stage::Done)]), None);

        // BUT A FAILURE STAYS. Nothing is running, and taking the segment away
        // would take the only sign off the screen before anybody saw it.
        let mut broke = a_job(1, Stage::Failed);
        broke.trouble = Some("no space".to_owned());
        assert!(
            segment(&[broke]).is_some(),
            "the failure went away from the modeline"
        );
    }

    #[test]
    fn what_ended_only_fires_once_per_job() {
        // A finished job stays in the history for a long time. Reacting to
        // `Done` on every beat would re-read the directory sixty times a minute
        // for a copy that ended an hour ago.
        let before = vec![a_job(1, Stage::Running)];
        let now = vec![a_job(1, Stage::Done)];
        assert_eq!(what_ended(&before, &now).len(), 1);
        // The same list again: nothing crossed the line this time.
        assert!(what_ended(&now, &now).is_empty());
    }

    #[test]
    fn a_failure_carries_its_own_words_across() {
        // `copy /x/y` alone does not say `No space left on device`, and the
        // second half is the whole diagnosis.
        let before = vec![a_job(1, Stage::Running)];
        let mut broke = a_job(1, Stage::Failed);
        broke.trouble = Some("no space left on the disk".to_owned());
        match what_ended(&before, &[broke]).first() {
            Some(crate::files::Report::Failed(why)) => assert_eq!(why, "no space left on the disk"),
            other => panic!("{other:?}"),
        }
    }
}
