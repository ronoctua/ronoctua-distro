//! The brain's line — the one under the minibuffer, and the host half feeding it.
//!
//! # Why a line of its own, and why UNDER the pair
//!
//! The owner asked for it in those words: when it is time to update, a line
//! appears below everything, in a colour of its own, saying what is about to
//! happen with a countdown and two answers.
//!
//! It is not a popup and not a card (invariants 14 and 15): it is the footer's
//! OWN surface growing, exactly as the notification band grows upwards and the
//! expansion grows downwards. One surface, one more region.
//!
//! Glued under the minibuffer and NOT at the bottom of the surface, and the
//! third reason is the one that decides it:
//!
//! 1. it is what was asked for — *below the minibuffer region*;
//! 2. it keeps the footer symmetric — band · modeline · minibuffer · brain ·
//!    expansion — so the fixed rows sit above the list and this line moves on
//!    screen exactly when the modeline does;
//! 3. a truly-last line would live at `first_of_expansion() + expansion_rows()`
//!    and would have to be recomputed in the painter, in the hit test and in
//!    the background quad, for ever. That is the defect
//!    [`crate::surface::Footer::first_of_expansion`] was written to remove —
//!    "two calculations of the same thing, and the one that gets it wrong is
//!    the one nobody looks at". Glued under the minibuffer, that one function
//!    gains ONE term and every reader downstream follows for free.
//!
//! # Why the layout half has no Wayland in it
//!
//! Everything above [`lines`] is `(what is showing, width) -> pieces with
//! columns`. The same two consequences [`crate::band`] claims: it is testable
//! headless, and it fits the frame budget because it is integer arithmetic over
//! the cell.
//!
//! # And the host half
//!
//! Below the layout: the thread that watches the daemon, the conversion from
//! the daemon's [`ronoctua_brain::Mind`] into the WIT's, and the two asks the
//! band's controls send. The same shape the transfer queue's panel already has for
//! the queue — one file per border, layout and wire together, because they are
//! two halves of one feature and splitting them would hide which is which.

use crate::brainlink::{Acting, glyph_of, interned, tone_colour};
use ronoctua_brain::mind::Posted;
use ronoctua_brain::module::{Level, Mark, Notice, Tone};
use ronoctua_brain::{Announcement, Mind};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// What a click on a piece of the strip does.
///
/// Every one has a key: the countdown's two and the notice's three are
/// registered commands with a sequence of their own inside the brain's buffer,
/// drawn on the strip itself, and the `+N` is `brain-open` — `LEADER b`. The
/// mouse reaches nothing the keyboard does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    /// Not now.
    ///
    /// # Why it postpones instead of aborting, while nothing is running
    ///
    /// Because taking a task out of the queue is not what stops it. The
    /// runner's own comment says so: `Runner::pump` enrols whatever is due on
    /// every beat, so a task unqueued while still due is back in the queue half
    /// a minute later, "and the person would answer the same question every
    /// half minute". What moves the next moment is the postponement, which the
    /// runner writes down as an event. See [`abort`].
    Abort,
    /// Do not wait the countdown out — start now.
    Proceed,
    /// The notice's FIRST control — `brain-act`.
    ///
    /// ONE variant and not one per action, because what it runs is named by
    /// the notice itself: an action of its module, which the brain resolves.
    Act,
    /// Put the notice away — `brain-dismiss`.
    Dismiss,
    /// Put it off by the first snooze it offers — `brain-snooze`.
    Snooze,
    /// The `+N`: every notice, on the brain's page — `brain-open`.
    More,
}

/// One drawn piece of the strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// Which line of the strip, counted from its first: `0`, or `1` when an
    /// alarm and a countdown share it.
    pub row: u16,
    /// First cell.
    pub at: u16,
    /// What is drawn, already elided.
    pub text: String,
    /// Which token colours it.
    pub color: tokens::Color,
    /// What a click does. `None` = it is text, not a control.
    pub hit: Option<Hit>,
    /// What the message line says while the pointer rests here.
    ///
    /// Already translated, for the reason [`crate::band::Piece::explain`] gives:
    /// the alternative is the drawing carrying keys and arguments around.
    pub explain: Option<String>,
    /// Was it placed counting cells from the surface's RIGHT EDGE?
    ///
    /// See [`crate::band::Piece::edge`], which says the whole of it: the grid
    /// does not reach the border, so the controls and the pie on this line
    /// are placed by `chrome::from_the_edge` and found by
    /// `chrome::at_the_edge`.
    pub edge: bool,
}

/// The strip, laid out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    /// Left to right, row by row.
    pub pieces: Vec<Piece>,
}

/// What the strip needs, resolved once.
///
/// A struct and not loose arguments, the same reason [`crate::band::Words`] is
/// one: they are always wanted together, and a caller that forgot the bundle
/// would get `[brain-band-soon]` on screen rather than a compile error.
pub struct Words<'a> {
    /// Where the sentences come from.
    pub texts: &'a ronoctua_i18n::Texts,
    /// The machine's zone, for a notice's moments.
    pub zone: &'a ronoctua_files::when::Zone,
    /// How `brain-abort` is typed — the brain's own `, a`.
    ///
    /// It comes from OUTSIDE because whoever knows it is whoever defines the
    /// bind (`crate::leader`), and a line that wrote it again would be a
    /// second answer to "which key stops it".
    pub abort_keys: &'a str,
    /// The same, for `brain-proceed`.
    pub proceed_keys: &'a str,
    /// And the notice's three: its first control, putting it away, putting it
    /// off — `, n`, `, x` and `, z` in the brain's buffer.
    pub act_keys: &'a str,
    /// See [`Words::act_keys`].
    pub dismiss_keys: &'a str,
    /// See [`Words::act_keys`].
    pub snooze_keys: &'a str,
}

/// WHAT THE STRIP SHOWS, out of one mind: the countdown, and every notice in
/// the order it ranks them (`mind_wire::ranked`).
///
/// # The precedence, and it is the owner's
///
/// An ALARM first — a reminder ringing is the one thing that may not wait even
/// for the machine's own countdown — then the countdown, then danger, warn and
/// info, the newest first among equals. With an alarm AND a countdown the
/// strip GROWS TO A SECOND LINE, the alarm above: neither may push the other
/// off, and a countdown hidden behind an alarm is a machine that rebuilds
/// itself without having said so.
pub struct Shown<'a> {
    /// The warning in flight, if any.
    pub countdown: Option<&'a Announcement>,
    /// Every notice, ranked.
    pub notices: Vec<&'a Posted<Notice>>,
    /// The mind they came from — what a notice's controls are resolved in.
    pub mind: Option<&'a Mind>,
}

impl<'a> Shown<'a> {
    /// What one mind shows; nothing without one.
    #[must_use]
    pub fn of(mind: Option<&'a Mind>) -> Self {
        Self {
            countdown: mind.and_then(|m| m.announcement.as_ref()),
            notices: mind.map(crate::mind_wire::ranked).unwrap_or_default(),
            mind,
        }
    }

    /// The loudest notice — the one the strip says.
    #[must_use]
    pub fn top(&self) -> Option<&'a Posted<Notice>> {
        self.notices.first().copied()
    }

    /// Whether the loudest notice rings.
    fn alarm(&self) -> bool {
        self.top().is_some_and(|n| n.what.level == Level::Alarm)
    }
}

/// How many lines the strip takes for what is showing.
///
/// # Why it does not depend on the width
///
/// The same reason [`crate::band::rows`] gives, and it is not weaker here: the
/// footer's height is the same on every output (see `Footer::apply_geometry`),
/// so a strip that wrapped on the laptop and not on the monitor would reflow
/// every buffer when focus changed screen, with nothing having changed in them.
///
/// So it is one line or none — and TWO only when an alarm and a countdown are
/// both standing (the owner's decision): what does not fit ELIDES.
#[must_use]
pub fn rows(shown: &Shown<'_>) -> u16 {
    match (shown.countdown.is_some(), shown.top().is_some()) {
        (true, true) if shown.alarm() => 2,
        (false, false) => 0,
        _ => 1,
    }
}

/// How much of the warning is left, as a fraction (1.0 → 0.0).
///
/// `now` is epoch seconds, and it comes from the caller rather than from a
/// clock in here: the footer already beats a one-second timer and this is
/// computed on that beat. A clock of its own would be a second wake-up for one
/// effect — the trap `surface`'s timer records for the blink.
#[must_use]
pub fn left_of(a: &Announcement, now: i64) -> f32 {
    if a.total_seconds == 0 {
        return 0.0;
    }
    let left = (a.deadline - now).max(0);
    // The ratio is clamped and not merely divided: a deadline further away than
    // the whole warning — a clock that stepped backwards, a daemon restarted
    // mid-countdown — would give a pie fuller than full.
    (left as f32 / f32::from(a.total_seconds)).clamp(0.0, 1.0)
}

/// How many seconds are left, for the sentence.
#[must_use]
pub fn seconds_left(a: &Announcement, now: i64) -> i64 {
    (a.deadline - now).max(0)
}

/// The strip, laid out for `columns` cells.
///
/// `now` is epoch seconds; everything the countdown shows is derived from it
/// and from the announcement's own `deadline`, because the brain sends the
/// warning ONCE and never a ticking number.
#[must_use]
pub fn lines(shown: &Shown<'_>, columns: u16, now: i64, words: &Words<'_>) -> Option<Line> {
    let mut line = Line::default();
    // THE ALARM FIRST, then the countdown, then any other notice — see
    // [`Shown`], which carries the whole precedence.
    match (shown.countdown, shown.top()) {
        (Some(a), Some(n)) if shown.alarm() => {
            notice_line(&mut line, 0, n, shown, columns, words);
            countdown_line(&mut line, (1, a), shown.mind, columns, now, words);
        }
        (Some(a), _) => countdown_line(&mut line, (0, a), shown.mind, columns, now, words),
        (None, Some(n)) => notice_line(&mut line, 0, n, shown, columns, words),
        (None, None) => return None,
    }
    Some(line)
}

/// THE MARGIN IS THE MODELINE'S, and not `chrome::INSET`.
///
/// This line is peripheral reference — one glances at it, one does not read
/// it — which is the whole of the exception `chrome`'s header names. It is the
/// fourth site on that list, and it is named there.
const fn margin() -> u16 {
    tokens::space::MODELINE_EDGELEFT
}

/// WHOSE LINE THIS IS, in the margin. It arrives without anybody having asked
/// for it, so the first thing on it says who is speaking — the same argument
/// `band::generic_glyph` makes for a notification's mark.
fn mark(line: &mut Line, row: u16) {
    line.pieces.push(Piece {
        row,
        at: margin(),
        text: tokens::icon::BRAIN.to_string(),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
        edge: false,
    });
}

/// The countdown, on one row of the strip.
fn countdown_line(
    line: &mut Line,
    (row, a): (u16, &Announcement),
    mind: Option<&Mind>,
    columns: u16,
    now: i64,
    words: &Words<'_>,
) {
    mark(line, row);
    let at = margin() + 2;

    // THE RIGHT-HAND CLUSTER FIRST, because it decides how much room the
    // sentence gets. Laying the text out first and then discovering the
    // controls do not fit is how a line comes to spill — `band.rs` records it,
    // and the answer is the same here. From the edge inwards: proceed, abort,
    // the pie — each placed by `chrome::from_the_edge` and carrying
    // `edge: true`.
    let mut right = crate::chrome::from_the_edge(columns, 0);
    right = control(
        line,
        row,
        right,
        words.proceed_keys,
        &words.texts.t(crate::key::BRAIN_BAND_PROCEED),
        Hit::Proceed,
        words.texts.t(crate::key::CMD_BRAIN_PROCEED),
    );
    right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP);
    right = control(
        line,
        row,
        right,
        words.abort_keys,
        &words.texts.t(crate::key::BRAIN_BAND_ABORT),
        Hit::Abort,
        words.texts.t(crate::key::CMD_BRAIN_ABORT),
    );

    // THE PIE, and it is `band::countdown_glyph` and not a second one: the
    // notification band and this line are two countdowns on the same screen,
    // and a person who learned one has learned the other.
    right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP + 1);
    line.pieces.push(Piece {
        row,
        at: right,
        text: crate::band::countdown_glyph(ronoctua_notify::eighths(left_of(a, now))).to_string(),
        color: tokens::color::BRAIN_COUNTDOWN,
        hit: None,
        explain: Some(words.texts.t(crate::key::HINT_BRAIN_COUNTDOWN)),
        edge: true,
    });

    // AND THE SENTENCE, in whatever is left. The subject is the TASK'S OWN
    // NAME, by the key its module declared: a module added tomorrow inherits
    // this line instead of needing a sentence of its own.
    let name = words.texts.t(&task_name_key(a.task.as_str(), mind));
    let sentence = words.texts.t_args(
        crate::key::BRAIN_BAND_SOON,
        &[
            ("task", ronoctua_i18n::Arg::Text(&name)),
            ("seconds", ronoctua_i18n::Arg::Number(seconds_left(a, now))),
        ],
    );
    let room = right.saturating_sub(at).saturating_sub(1);
    line.pieces.push(Piece {
        row,
        at,
        text: layout::elide_end(&sentence, room),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
        edge: false,
    });
}

/// The key a task's name lives under: its module's own when the mind has it,
/// the core's `brain-task-<id>` otherwise.
fn task_name_key(id: &str, mind: Option<&Mind>) -> String {
    mind.and_then(|m| m.task(id))
        .map(|t| t.name_key.clone())
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| format!("brain-task-{id}"))
}

/// How few cells the sentence may be left before a control gives way to it.
///
/// A notice whose sentence is three letters and an ellipsis beside four
/// buttons is four buttons about nothing: the controls are the extras, and
/// they go first — the outermost last, so `+N` and putting it away stay.
const SENTENCE_FLOOR: u16 = 12;

/// A notice, on one row of the strip: its sentence, and on the right what it
/// answers to — its first control, putting it off, putting it away — and how
/// many more there are.
///
/// # Why it is a second builder and not a branch in the first
///
/// Because almost nothing is shared. A countdown has a pie, two answers and a
/// deadline; a notice has a sentence and the controls ITS module declared.
/// What IS shared is the shape — the glyph in the margin, the cluster on the
/// right laid out before the sentence, the sentence eliding into what is left
/// — and that is shared by calling the same `control` and the same margins.
fn notice_line(
    line: &mut Line,
    row: u16,
    posted: &Posted<Notice>,
    shown: &Shown<'_>,
    columns: u16,
    words: &Words<'_>,
) {
    let n = &posted.what;
    mark(line, row);
    let at = margin() + 2;
    let floor = at + SENTENCE_FLOOR;
    let mut right = crate::chrome::from_the_edge(columns, 0);

    // HOW MANY MORE, at the edge: the strip says one, and the page says all.
    let more = shown.notices.len().saturating_sub(1);
    if more > 0 {
        let text = words.texts.t_args(
            crate::key::BRAIN_BAND_MORE,
            &[(
                "count",
                ronoctua_i18n::Arg::Number(i64::try_from(more).unwrap_or(i64::MAX)),
            )],
        );
        let start = right.saturating_sub(layout::width(&text));
        if start > floor {
            line.pieces.push(Piece {
                row,
                at: start,
                text,
                color: tokens::color::BRAIN_DIM,
                hit: Some(Hit::More),
                explain: Some(words.texts.t(crate::key::HINT_BRAIN_MORE)),
                edge: true,
            });
            right = start.saturating_sub(tokens::space::MODELINE_SEGMENTGAP);
        }
    }
    // THE CONTROLS, each only when the notice offers it — an alarm that may
    // not be put away draws no button that would — and each only while the
    // sentence keeps its floor.
    let mut offered: Vec<(&str, String, Hit, String)> = Vec::new();
    if n.dismiss {
        offered.push((
            words.dismiss_keys,
            words.texts.t(crate::key::BRAIN_BAND_DISMISS),
            Hit::Dismiss,
            words.texts.t(crate::key::CMD_BRAIN_DISMISS),
        ));
    }
    if let Some(minutes) = n.snooze.first() {
        offered.push((
            words.snooze_keys,
            words.texts.t_args(
                crate::key::BRAIN_BAND_SNOOZE,
                &[("minutes", ronoctua_i18n::Arg::Number(i64::from(*minutes)))],
            ),
            Hit::Snooze,
            words.texts.t(crate::key::CMD_BRAIN_SNOOZE),
        ));
    }
    if let Some(first) = n.controls.first() {
        // THE CONTROL'S OWN LABEL — the action its module declared, as the
        // brain's page names it — so the button says what it does.
        let offered_by = shown
            .mind
            .and_then(|m| m.task(&posted.module))
            .map(|t| t.actions.as_slice());
        let label = crate::mind_wire::control_label(&posted.module, first, offered_by);
        offered.push((
            words.act_keys,
            words.texts.t(&label),
            Hit::Act,
            words.texts.t(&label),
        ));
    }
    for (keys, label, hit, explain) in offered {
        let width = layout::width(&format!("[{keys}]")) + 1 + layout::width(&label);
        if right.saturating_sub(width) <= floor {
            break;
        }
        right = control(line, row, right, keys, &label, hit, explain);
        right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP);
    }
    // AND THE SENTENCE: the module's key and its values, made in the person's
    // language — in the loud tone for what costs something now.
    let said = crate::mind_wire::sentence(&n.says, &n.args, words.zone, words.texts);
    let color = match n.level {
        Level::Alarm | Level::Danger => tokens::color::BRAIN_COUNTDOWN,
        Level::Warn | Level::Info => tokens::color::BRAIN_TEXT,
    };
    let room = right.saturating_sub(at).saturating_sub(1);
    line.pieces.push(Piece {
        row,
        at,
        text: layout::elide_end(&said, room),
        color,
        hit: None,
        explain: None,
        edge: false,
    });
}

/// One control — the bracketed key and the label — placed with its right edge
/// at `right` on `row`. Gives back the cell its left edge landed on.
///
/// TWO pieces and not one, because the two are coloured differently: the key is
/// `brain.dim` and the label is `brain.text`, which is what that token says it
/// is for. Both carry the same `hit` and the same explanation, so the pointer
/// answers the same thing over either half.
fn control(
    line: &mut Line,
    row: u16,
    right: u16,
    keys: &str,
    label: &str,
    hit: Hit,
    explain: String,
) -> u16 {
    let bracketed = format!("[{keys}]");
    let width = layout::width(&bracketed) + 1 + layout::width(label);
    let start = right.saturating_sub(width);
    line.pieces.push(Piece {
        row,
        at: start,
        text: bracketed.clone(),
        color: tokens::color::BRAIN_DIM,
        hit: Some(hit),
        explain: Some(explain.clone()),
        edge: true,
    });
    line.pieces.push(Piece {
        row,
        at: start + layout::width(&bracketed) + 1,
        text: label.to_owned(),
        color: tokens::color::BRAIN_TEXT,
        hit: Some(hit),
        explain: Some(explain),
        edge: true,
    });
    start
}

/// What a control of the strip ACTS ON — the countdown's task and the moment
/// it starts, or the loudest notice by its module and id — so a pick survives
/// a strip redrawn every second and is gone with what it was about. `None` for
/// the `+N`, which acts on nothing, and for a control the strip does not show.
///
/// # Why a control is picked before it runs
///
/// Because the strip APPEARS BY ITSELF, under the minibuffer, where a hand may
/// be on its way to click something else — and a click on `now` starts the
/// machine's rebuild. So the first click picks a control and only a click on
/// the pick runs it (`crate::aim`), the band's rule. The `+N` keeps its one
/// click: it opens the brain's page, which a person reads and leaves.
#[must_use]
pub fn subject_of(shown: &Shown<'_>, hit: Hit) -> Option<String> {
    match hit {
        Hit::Abort | Hit::Proceed => shown
            .countdown
            .map(|a| format!("{}@{}", a.task, a.deadline)),
        Hit::Act | Hit::Dismiss | Hit::Snooze => {
            shown.top().map(|p| format!("{}/{}", p.module, p.what.id))
        }
        Hit::More => None,
    }
}

/// The picked control, in the cursor's ink — its key and its label both, as
/// the pointer answers the same thing over either half.
pub fn with_the_pick(line: &mut Line, hit: Hit) {
    for p in &mut line.pieces {
        if p.hit == Some(hit) {
            p.color = tokens::color::TEXT_HERE_ACTS;
        }
    }
}

/// Which piece of the strip is at cell `column` of its row `row`.
///
/// From what was DRAWN, and not from a second calculation of where things would
/// be — the rule the modeline's segments and the notification band already
/// follow, for the reason both write down: a `Motion` arrives per pixel of
/// movement, and two calculations of one position are two chances to point at
/// the wrong thing.
#[must_use]
pub fn at(
    line: Option<&Line>,
    row: u16,
    column: u16,
    from_the_edge: Option<u16>,
) -> Option<&Piece> {
    line?.pieces.iter().find(|p| {
        // THE READING THAT PLACED IT — see `crate::band::at`, which says the
        // whole of it: the grid's remainder sits between the two readings and
        // can be most of a cell, so a control compared against the wrong one
        // answers a cell away from where it was drawn.
        let at = if p.edge { from_the_edge } else { Some(column) };
        p.row == row && at.is_some_and(|c| c >= p.at && c < p.at + layout::width(&p.text))
    })
}

// ---------------------------------------------------------------------------
// The modeline's marks: whatever a module published, drawn the same way
// ---------------------------------------------------------------------------

/// The prefix of every mark a brain module puts on the modeline: the segment
/// is `brain:<module>/<mark>`, and nothing in the shell names a module.
pub const MARK_PREFIX: &str = "brain:";

// THE 2026-10-05 WAVE: a mark's instance (`brain:<module>/<mark>.<n>`) and
// its digit, after its anchor.
// ── anchor: L11 ──

/// What a mark's segment says: its glyph, and a space after it the
/// instance's digit or the count — the count's path for both numbers, each
/// from its own field ([`Drawn::instance`], [`Drawn::count`]).
///
/// A SPACE, NOT A DIGIT AGAINST THE GLYPH: a Nerd Font icon fills its cell to
/// the edge, and a digit in the next cell touched it — measured on the
/// footer's own photograph. And ONE number: the core refuses a count on an
/// instance (`Manifest::refusal_of`), since two numbers after one glyph would
/// read as one; a mind that carried both anyway draws the instance, which is
/// what the segment's id and its click are made of.
#[must_use]
pub fn mark_text(glyph: char, instance: u8, count: u32) -> String {
    let number = if instance > 0 {
        u32::from(instance)
    } else {
        count
    };
    if number > 0 {
        format!("{glyph} {number}")
    } else {
        glyph.to_string()
    }
}

/// The order the modeline PUSHES a mind's marks in, so that a mark's
/// instances read from the first to the last, left to right.
///
/// The right side is laid out FROM THE EDGE INWARDS (`layout::resolve`): the
/// first segment pushed sits against the edge and the next one to its left.
/// Pushed as published, three accounts read `3 2 1`. So each mark's
/// instances are pushed from the last to the first, together, where its
/// first published one was; every other mark keeps its place.
///
/// The price, said: at one rung the segment farthest from the edge is the
/// first to go when the line is short, so the FIRST instance is the one that
/// leaves first. Reading `1 2 3` is the order a person counts in; the line
/// keeps its one rule of which segment goes.
#[must_use]
pub fn from_the_edge(drawn: &[Drawn]) -> Vec<&Drawn> {
    // A MARK, whichever of its instances: `brain:<module>/<name>` — the id
    // without `.<n>`. Neither a module's id nor a mark's name has a dot.
    let mark = |d: &Drawn| -> &str {
        if d.instance > 0 {
            d.id.rsplit_once('.').map_or(d.id, |(mark, _)| mark)
        } else {
            d.id
        }
    };
    let first = |d: &Drawn| drawn.iter().position(|e| mark(e) == mark(d));
    let mut order: Vec<&Drawn> = drawn.iter().collect();
    // STABLE, so marks of one place keep the order they were published in.
    order.sort_by_key(|d| (first(d), std::cmp::Reverse(d.instance)));
    order
}
// ── anchor: L24 ──

/// A mark on the modeline, as the footer draws it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drawn {
    /// The segment's id: `brain:<module>/<mark>`.
    pub id: &'static str,
    /// Whose it is.
    pub module: String,
    /// What the segment draws: the glyph, and the count when there is one.
    pub text: String,
    /// Its colour, by meaning.
    pub tone: Tone,
    /// The key of the line that explains it under the pointer.
    pub hint: String,
    /// The number beside the glyph; `0` draws none.
    pub count: u32,
    /// The action a click runs; `None` opens the module's page.
    pub act: Option<String>,
    /// The shell's command a click runs instead, when the module's mark
    /// opens an app of the shell (`MarkId::opens`) — by name, as `LEADER :`.
    pub opens: Option<&'static str>,
    /// Which of its mark's instances it is, from 1 (`MarkId::instance`);
    /// `0` for a mark that is one. Drawn as the digit beside the glyph, and
    /// handed to [`Drawn::opens`] as its argument on a click.
    pub instance: u8,
    /// The instance's name as the person gave it — data, drawn as it is in
    /// the hint (`$label`, the instance's number when they gave none).
    pub label: Option<String>,
}

/// The marks one mind publishes, in the order the modules published them.
#[must_use]
pub fn marks(mind: Option<&Mind>) -> Vec<Drawn> {
    mind.map(|m| m.marks.iter().filter_map(drawn_of).collect())
        .unwrap_or_default()
}

/// Remembers the marks about to be drawn, for [`colour_of`], [`drawn_mark`]
/// and the hint: those are asked by the segment's ID alone, and the colour
/// table in `surface` is a free function with no mind in reach.
pub fn remember(drawn: &[Drawn]) {
    if let Ok(mut shown) = SHOWN.lock() {
        shown.clear();
        shown.extend_from_slice(drawn);
    }
}

/// One published mark, drawn — or none, past [`NAMES_KEPT`].
fn drawn_of(posted: &Posted<Mark>) -> Option<Drawn> {
    let mark = &posted.what;
    // `brain:<module>/<mark>`, and `brain:<module>/<mark>.<n>` for one of its
    // instances — each its own segment, hint and click.
    let id = interned(&format!(
        "{MARK_PREFIX}{}/{}",
        posted.module,
        mark.id.local()
    ))?;
    // THE GLYPH IS A TOKEN, by the name the module declared — measured into
    // the brain's catalogue. One the catalogue does not know draws the brain's own
    // mark rather than nothing: a mark exists only when it says something, so
    // it must at least be seen.
    let glyph = glyph_of(&mark.id.glyph).unwrap_or(tokens::icon::BRAIN);
    let text = mark_text(glyph, mark.id.instance, mark.count);
    Some(Drawn {
        id,
        module: posted.module.clone(),
        text,
        tone: mark.tone,
        hint: mark.id.hint.to_string(),
        count: mark.count,
        act: mark.act.as_ref().map(ToString::to_string),
        // A COMMAND BY NAME, interned like the segment's id: the click hands
        // it to `run_with`, which takes `&'static str`.
        opens: mark.id.opens.as_deref().and_then(interned),
        instance: mark.id.instance,
        label: mark.id.label.as_deref().map(ToOwned::to_owned),
    })
}

/// The marks last drawn — what [`colour_of`] and [`drawn_mark`] read.
static SHOWN: std::sync::Mutex<Vec<Drawn>> = std::sync::Mutex::new(Vec::new());

/// Whether a segment is a brain module's mark.
#[must_use]
pub fn is_mark(id: &str) -> bool {
    id.starts_with(MARK_PREFIX)
}

/// The mark last drawn under this id, if it still is.
#[must_use]
pub fn drawn_mark(id: &str) -> Option<Drawn> {
    find(&SHOWN.lock().ok()?, id).cloned()
}

/// A mark among these, by its segment id.
#[must_use]
pub fn find<'a>(drawn: &'a [Drawn], id: &str) -> Option<&'a Drawn> {
    drawn.iter().find(|d| d.id == id)
}

/// The colour of the mark under this segment id, as last drawn.
#[must_use]
pub fn colour_of(id: &str) -> tokens::Color {
    drawn_mark(id).map_or(tokens::color::TEXT_SECONDARY, |d| tone_colour(d.tone))
}

/// What the minibuffer says while the pointer rests on a mark: the module's
/// own hint, with the mark's count for a sentence that counts — and, for one
/// of its instances, the instance's number and the person's name for it.
#[must_use]
pub fn hint_of(id: &str, texts: &ronoctua_i18n::Texts) -> Option<String> {
    hint_in(&drawn_mark(id)?, texts)
}

/// The same, for a mark in hand.
#[must_use]
pub fn hint_in(d: &Drawn, texts: &ronoctua_i18n::Texts) -> Option<String> {
    (!d.hint.is_empty()).then(|| {
        let number = d.instance.to_string();
        texts.t_args(&d.hint, &hint_args(d, &number))
    })
}

/// The values a mark's hint is made with: `$count`; for one of its
/// instances `$instance` — A NUMBER, so a selector can branch on it — and
/// `$label`, the person's own word for it, or the instance's `number` when
/// they gave none: a Fluent selector cannot ask whether a text is empty, so
/// a sentence that names the instance always has something to name it by.
/// A BLANK word is none (`MarkId::numbered` drops one already; a mind written
/// by hand may still carry it): "Seat  is taken" would be a hole.
#[must_use]
pub fn hint_args<'a>(d: &'a Drawn, number: &'a str) -> [(&'static str, ronoctua_i18n::Arg<'a>); 3] {
    let label = d
        .label
        .as_deref()
        .filter(|l| !l.trim().is_empty())
        .unwrap_or(number);
    [
        ("count", ronoctua_i18n::Arg::Number(i64::from(d.count))),
        (
            "instance",
            ronoctua_i18n::Arg::Number(i64::from(d.instance)),
        ),
        ("label", ronoctua_i18n::Arg::Text(label)),
    ]
}

/// What a click on a mark runs, and with what: a command of the registry the
/// mark's id names (`MarkId::opens` — the ONE way a mark opens an app of the
/// shell), with the instance's number after it for one of a mark's
/// instances and nothing otherwise; else the mark's own action through
/// `brain-act`, or the module's page through `brain-open` — all reachable by
/// name, and `brain-open` is `LEADER b`.
#[must_use]
pub fn click_of(d: &Drawn) -> (&str, String) {
    // A MODULE WITH A CENTRE OF ITS OWN: the security centre's watch opens
    // the centre on the section its mark is about, which is where the thing
    // that is off can be put back (`security_words::section_of_mark`).
    if let Some(section) = crate::security_words::section_of_mark(&d.module, d.id) {
        return ("security-open", section.to_owned());
    }
    // AN APP OF THE SHELL, when the module's mark opens one: the command the
    // module named — with the instance's number after it for one of its
    // instances (the third account opens on the third), nothing otherwise.
    if let Some(command) = d.opens {
        let argument = if d.instance > 0 {
            d.instance.to_string()
        } else {
            String::new()
        };
        return (command, argument);
    }
    match d.act.as_deref() {
        Some(act) => ("brain-act", format!("{} {act}", d.module)),
        None => ("brain-open", d.module.clone()),
    }
}

/// WHAT OF A MIND THE FOOTER DRAWS: the countdown, the loudest notice, how
/// many there are, and the marks. Two minds with the same face draw the same
/// footer, and one that differs redraws it — however small the difference.
///
/// # Why it is not the announcement alone
///
/// Because it was, and a notice appearing or clearing ON ITS OWN — a reminder
/// ringing with no countdown in flight — changed nothing the comparison saw:
/// the strip stayed as it was, and its height with it, until something else
/// moved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Face {
    countdown: Option<Announcement>,
    top: Option<Posted<Notice>>,
    notices: usize,
    marks: Vec<Posted<Mark>>,
}

impl Face {
    /// The face of a mind, or of none.
    #[must_use]
    pub fn of(mind: Option<&Mind>) -> Self {
        let Some(m) = mind else {
            return Self::default();
        };
        Self {
            countdown: m.announcement.clone(),
            top: crate::mind_wire::ranked(m).first().map(|n| (*n).clone()),
            notices: m.notices.len(),
            marks: m.marks.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// The host half: the thread that listens, and what the two controls send
// ---------------------------------------------------------------------------

/// What the notice's three controls do to the loudest notice — the strip's
/// buttons, `LEADER :`'s commands and the brain's `, x` and `, z` all come
/// here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// Put it away.
    Dismiss,
    /// Put it off by the first snooze it offers.
    Snooze,
}

/// The ask that answers the loudest notice with `verb`, and the sentence to
/// say once it is sent — or the sentence that says why there is none.
///
/// # Errors
///
/// The refusal's text key: nothing is being said, or what is said does not
/// offer this. A key that does nothing and says nothing is indistinguishable
/// from a key that did not register.
pub fn notice_ask(
    mind: Option<&Mind>,
    verb: Verb,
) -> Result<(ronoctua_brain::Ask, &'static str), &'static str> {
    let shown = Shown::of(mind);
    let Some(top) = shown.top() else {
        return Err(crate::key::ECHO_BRAIN_NO_NOTICE);
    };
    let (module, notice) = (top.module.clone(), top.what.id.to_string());
    match verb {
        Verb::Dismiss if top.what.dismiss => Ok((
            ronoctua_brain::Ask::Dismiss { module, notice },
            crate::key::ECHO_BRAIN_DISMISSED,
        )),
        Verb::Snooze => match top.what.snooze.first() {
            Some(minutes) => Ok((
                ronoctua_brain::Ask::Snooze {
                    module,
                    notice,
                    minutes: *minutes,
                },
                crate::key::ECHO_BRAIN_SNOOZED,
            )),
            None => Err(crate::key::ECHO_BRAIN_NOT_OFFERED),
        },
        Verb::Dismiss => Err(crate::key::ECHO_BRAIN_NOT_OFFERED),
    }
}

/// What `brain-act` acts on: `module action` when it was named — a click on a
/// mark names it — and the loudest notice's first control when it was not.
///
/// # Errors
///
/// The refusal's text key: nothing to act on, or a module that offers no such
/// action — refused rather than sent, since the brain would refuse it too and
/// say so only in its log.
pub fn act_of(mind: Option<&Mind>, argument: Option<&str>) -> Result<Acting, &'static str> {
    let (module, action) = match argument.map(str::trim).filter(|a| !a.is_empty()) {
        Some(named) => {
            let Some((module, action)) = named.split_once(' ') else {
                return Err(crate::key::ECHO_BRAIN_NO_ACTION);
            };
            (module.to_owned(), action.trim().to_owned())
        }
        None => {
            let shown = Shown::of(mind);
            let Some(top) = shown.top() else {
                return Err(crate::key::ECHO_BRAIN_NO_NOTICE);
            };
            let Some(first) = top.what.controls.first() else {
                return Err(crate::key::ECHO_BRAIN_NOT_OFFERED);
            };
            (top.module.clone(), first.to_string())
        }
    };
    let declared = mind
        .and_then(|m| m.task(&module))
        .and_then(|t| t.actions.iter().find(|a| a.id == action));
    // AN ACTION THE MIND DOES NOT LIST is still sent when a notice or a mark
    // named it — the module published it, and the brain knows its own — but
    // one typed by name has to be one the mind lists.
    if declared.is_none() && argument.is_some() && mind.and_then(|m| m.task(&module)).is_some() {
        return Err(crate::key::ECHO_BRAIN_NO_ACTION);
    }
    Ok(Acting {
        command: format!("{module}-{action}"),
        asks: declared.and_then(|a| a.asks.clone()),
        module,
        action,
    })
}

/// The action a registry-style command names — `<module>-<action>` — among
/// the actions the mind lists; `None` for any other command.
///
/// It is how a question asked for an action comes back: the line asked under
/// the command's name, and the answer arrives as that command's argument.
#[must_use]
pub fn action_named(mind: Option<&Mind>, command: &str) -> Option<Acting> {
    mind?.tasks.iter().find_map(|t| {
        let action = command.strip_prefix(t.id.as_str())?.strip_prefix('-')?;
        let found = t.actions.iter().find(|a| a.id == action)?;
        Some(Acting {
            module: t.id.clone(),
            action: found.id.clone(),
            command: command.to_owned(),
            asks: found.asks.clone(),
        })
    })
}

// ---------------------------------------------------------------------------
// The daemon's mind, as the component draws it
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    /// THE COUNTDOWN'S TWO ANSWERS, one decision for every door: nothing to
    /// answer with no brain or nothing about to happen; proceed only over an
    /// announcement; stop as a postpone while it is announced and an abort
    /// once it runs.
    #[test]
    fn the_countdown_is_answered_one_way_from_every_door() {
        use ronoctua_brain::Ask;
        assert_eq!(crate::brainlink::answer_of(None, true), None);
        assert_eq!(crate::brainlink::answer_of(None, false), None);
        let idle = Mind::default();
        assert_eq!(crate::brainlink::answer_of(Some(&idle), true), None);
        assert_eq!(crate::brainlink::answer_of(Some(&idle), false), None);
        assert_eq!(
            crate::brainlink::answer(Some(&idle), false),
            crate::key::ECHO_BRAIN_IDLE
        );
        let running = Mind {
            running: "fixture".to_owned(),
            ..Mind::default()
        };
        assert_eq!(crate::brainlink::answer_of(Some(&running), true), None);
        assert_eq!(
            crate::brainlink::answer_of(Some(&running), false),
            Some(Ask::Abort {
                task: "fixture".to_owned()
            })
        );
    }

    use super::*;
    use crate::brainlink::act_ask;
    use crate::mind_wire::mind_of;
    use ronoctua_brain::module::{MarkId, Notice};

    fn words() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn announced(total: u16, deadline: i64) -> Announcement {
        Announcement {
            task: "fixture".to_owned(),
            total_seconds: total,
            deadline,
            why: "due".to_owned(),
        }
    }

    // FROM THE CONSTANTS THE KEYMAP BINDS, so that the day the keys move the
    // fixture moves with them: written as literals, this test went on proving
    // that `SPC s a` was drawn for a fortnight after `s a` stopped existing.
    static ABORT: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_ABORT_KEYS.to_owned());
    static PROCEED: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_PROCEED_KEYS.to_owned());
    static ACT: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_NOTICE_KEYS.to_owned());
    static DISMISS: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_DISMISS_KEYS.to_owned());
    static SNOOZE: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| crate::leader::BRAIN_SNOOZE_KEYS.to_owned());
    static ZONE: std::sync::LazyLock<ronoctua_files::when::Zone> =
        std::sync::LazyLock::new(ronoctua_files::when::Zone::utc);

    fn bundle(t: &ronoctua_i18n::Texts) -> Words<'_> {
        Words {
            texts: t,
            zone: &ZONE,
            abort_keys: &ABORT,
            proceed_keys: &PROCEED,
            act_keys: &ACT,
            dismiss_keys: &DISMISS,
            snooze_keys: &SNOOZE,
        }
    }

    /// A notice of a fictional module, at `level`, raised at `since`, offering
    /// everything: a control, putting it away, and putting it off ten minutes.
    fn a_notice(level: Level, since: i64) -> Posted<Notice> {
        Posted {
            module: "fixture".to_owned(),
            what: Notice {
                id: format!("n-{since}").into(),
                // A KEY THE BUNDLE HAS, so the sentence is a sentence and not
                // brackets — which one does not matter to the layout.
                says: crate::key::ECHO_BRAIN_IDLE.into(),
                level,
                since,
                controls: vec!["keep".into()],
                dismiss: true,
                snooze: vec![10, 60].into(),
                ..Notice::default()
            },
        }
    }

    fn a_mind(countdown: Option<Announcement>, notices: Vec<Posted<Notice>>) -> Mind {
        Mind {
            announcement: countdown,
            notices,
            ..Mind::default()
        }
    }

    fn hits(line: &Line, row: u16) -> Vec<Hit> {
        let mut out: Vec<Hit> = Vec::new();
        for p in line.pieces.iter().filter(|p| p.row == row) {
            if let Some(h) = p.hit
                && !out.contains(&h)
            {
                out.push(h);
            }
        }
        out
    }

    /// THE OWNER'S PRECEDENCE: an alarm beats the countdown, and with both the
    /// strip grows to a second line; the countdown beats every other notice;
    /// and a notice alone takes the line.
    #[test]
    fn an_alarm_and_a_countdown_share_the_strip_on_two_lines() {
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);

        let both = a_mind(Some(a.clone()), vec![a_notice(Level::Alarm, 5)]);
        let shown = Shown::of(Some(&both));
        assert_eq!(rows(&shown), 2, "an alarm and a countdown on one line");
        let line = lines(&shown, 120, 940, &w).expect("a strip");
        assert!(
            hits(&line, 0).contains(&Hit::Dismiss),
            "the alarm is not on top"
        );
        assert!(
            hits(&line, 1).contains(&Hit::Proceed),
            "the countdown went away"
        );

        let warn = a_mind(Some(a), vec![a_notice(Level::Danger, 5)]);
        let shown = Shown::of(Some(&warn));
        assert_eq!(rows(&shown), 1, "a danger took a second line");
        let line = lines(&shown, 120, 940, &w).expect("a strip");
        assert!(
            hits(&line, 0).contains(&Hit::Proceed),
            "a notice took the countdown's line"
        );

        let alone = a_mind(None, vec![a_notice(Level::Info, 5)]);
        let shown = Shown::of(Some(&alone));
        assert_eq!(rows(&shown), 1);
        assert!(lines(&shown, 120, 0, &w).is_some());

        let nothing = Mind::default();
        let shown = Shown::of(Some(&nothing));
        assert_eq!(rows(&shown), 0);
        assert!(lines(&shown, 120, 0, &w).is_none());
        assert_eq!(rows(&Shown::of(None)), 0);
    }

    /// FAILS IF: the loudest notice is not the one said — an alarm raised
    /// BEFORE a warning is still louder — or, among equals, not the newest.
    #[test]
    fn the_loudest_and_then_the_newest_is_said() {
        let m = a_mind(
            None,
            vec![
                a_notice(Level::Warn, 50),
                a_notice(Level::Alarm, 10),
                a_notice(Level::Warn, 90),
                a_notice(Level::Info, 99),
            ],
        );
        let order: Vec<i64> = Shown::of(Some(&m))
            .notices
            .iter()
            .map(|n| n.what.since)
            .collect();
        assert_eq!(order, vec![10, 90, 50, 99]);
    }

    /// A NOTICE DRAWS ONLY WHAT IT OFFERS, and says how many more there are.
    ///
    /// An alarm that may not be put away draws no button that would; one that
    /// offers nothing to do draws no control; and `+N` is there only while
    /// there is more than one.
    #[test]
    fn a_notice_draws_what_it_offers_and_how_many_more() {
        let t = words();
        let w = bundle(&t);
        let full = a_mind(
            None,
            vec![a_notice(Level::Warn, 5), a_notice(Level::Info, 1)],
        );
        let line = lines(&Shown::of(Some(&full)), 160, 0, &w).expect("a strip");
        let got = hits(&line, 0);
        for h in [Hit::More, Hit::Dismiss, Hit::Snooze, Hit::Act] {
            assert!(got.contains(&h), "{h:?} is not on the strip: {got:?}");
        }
        let drawn: String = line.pieces.iter().map(|p| p.text.as_str()).collect();
        for keys in [&*ACT, &*DISMISS, &*SNOOZE] {
            assert!(
                drawn.contains(&format!("[{keys}]")),
                "{keys} is not taught: {drawn}"
            );
        }
        // THE SNOOZE SAYS HOW LONG, in the language's own words.
        assert!(
            drawn.contains("10"),
            "the snooze does not say how long: {drawn}"
        );

        let mut bare = a_notice(Level::Alarm, 5);
        bare.what.dismiss = false;
        bare.what.snooze = Vec::new().into();
        bare.what.controls.clear();
        let alone = a_mind(None, vec![bare]);
        let line = lines(&Shown::of(Some(&alone)), 160, 0, &w).expect("a strip");
        assert!(
            line.pieces.iter().all(|p| p.hit.is_none()),
            "a notice that offers nothing drew a control anyway"
        );
    }

    #[test]
    fn nothing_spills_off_the_strip() {
        // The same property the modeline and the band are tested for, and for
        // the same reason: a compositor does not clip, so what spills is
        // rubbish at the screen's edge. AND NOTHING OVERLAPS: a control that
        // did not fit gives way to the sentence rather than landing on it.
        let t = words();
        let w = bundle(&t);
        let m = a_mind(
            Some(announced(60, 1_000)),
            vec![a_notice(Level::Alarm, 5), a_notice(Level::Warn, 1)],
        );
        let shown = Shown::of(Some(&m));
        for columns in [20_u16, 30, 40, 60, 80, 120, 200] {
            let line = lines(&shown, columns, 940, &w).expect("the strip went away");
            for row in 0..rows(&shown) {
                let mut on: Vec<&Piece> = line.pieces.iter().filter(|p| p.row == row).collect();
                on.sort_by_key(|p| p.at);
                for p in &on {
                    assert!(
                        p.at + layout::width(&p.text) <= columns,
                        "'{}' leaks at {columns} columns",
                        p.text
                    );
                }
                if columns >= 60 {
                    for pair in on.windows(2) {
                        assert!(
                            pair[0].at + layout::width(&pair[0].text) <= pair[1].at,
                            "'{}' runs into '{}' at {columns} columns",
                            pair[0].text,
                            pair[1].text
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_two_controls_are_reachable_and_say_what_they_do() {
        // THE MOUSE REACHES NOTHING THE KEYBOARD DOES NOT. Both targets are
        // registered commands, both are drawn with the sequence that runs them,
        // and both explain themselves on the message line — never in a tooltip
        // (invariant 14).
        let t = words();
        let w = bundle(&t);
        let m = a_mind(Some(announced(60, 1_000)), Vec::new());
        let line = lines(&Shown::of(Some(&m)), 120, 940, &w).expect("the line went away");
        for hit in [Hit::Abort, Hit::Proceed] {
            let mine: Vec<&Piece> = line.pieces.iter().filter(|p| p.hit == Some(hit)).collect();
            assert_eq!(mine.len(), 2, "{hit:?} does not have the key and the label");
            assert!(
                mine.iter().all(|p| p.explain.is_some()),
                "{hit:?} does not say what it does"
            );
        }
        let drawn: String = line.pieces.iter().map(|p| p.text.as_str()).collect();
        assert!(drawn.contains(&format!("[{}]", *ABORT)), "{drawn}");
        assert!(drawn.contains(&format!("[{}]", *PROCEED)), "{drawn}");
        // The margin carries the mark, and it is not a control.
        let first = line.pieces.first().unwrap();
        assert_eq!(first.at, tokens::space::MODELINE_EDGELEFT);
        assert_eq!(first.hit, None);
        assert_eq!(first.text, tokens::icon::BRAIN.to_string());
    }

    #[test]
    fn the_pointer_finds_what_was_drawn_on_its_own_row() {
        let t = words();
        let w = bundle(&t);
        let m = a_mind(Some(announced(60, 1_000)), vec![a_notice(Level::Alarm, 5)]);
        let line = lines(&Shown::of(Some(&m)), 120, 940, &w).expect("the line went away");
        let proceed = line
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Proceed))
            .expect("no proceed control");
        assert_eq!(proceed.row, 1);
        assert!(proceed.edge, "the control is not anchored to the border");
        assert_eq!(
            at(Some(&line), 1, 0, Some(proceed.at)).and_then(|p| p.hit),
            Some(Hit::Proceed)
        );
        // THE SAME CELL ON THE OTHER ROW is the alarm's, never the countdown's.
        assert_ne!(
            at(Some(&line), 0, 0, Some(proceed.at)).and_then(|p| p.hit),
            Some(Hit::Proceed)
        );
        assert!(
            at(Some(&line), 1, proceed.at, None).is_none(),
            "an edge-anchored control answered the left-counted cell"
        );
        let gap = tokens::space::MODELINE_EDGELEFT + 1;
        assert!(at(Some(&line), 0, gap, None).is_none());
        assert!(at(None, 0, 0, None).is_none());
    }

    /// FAILS IF: a pick is kept across a countdown or a notice it was not
    /// about, the `+N` asks for one, or the pick's ink lands anywhere but on
    /// its control's two halves.
    #[test]
    fn a_control_is_picked_by_what_it_acts_on() {
        let t = words();
        let w = bundle(&t);
        let m = a_mind(Some(announced(60, 1_000)), vec![a_notice(Level::Alarm, 5)]);
        let shown = Shown::of(Some(&m));
        let proceed = subject_of(&shown, Hit::Proceed).expect("the countdown");
        assert_eq!(subject_of(&shown, Hit::Abort), Some(proceed.clone()));
        let act = subject_of(&shown, Hit::Act).expect("the notice");
        assert_ne!(proceed, act, "the countdown and the notice are one subject");
        assert_eq!(subject_of(&shown, Hit::More), None);
        // ANOTHER COUNTDOWN, ANOTHER NOTICE: a pick made on the old one is
        // not one on these.
        let later = a_mind(Some(announced(60, 2_000)), vec![a_notice(Level::Alarm, 6)]);
        let later = Shown::of(Some(&later));
        assert_ne!(subject_of(&later, Hit::Proceed), Some(proceed));
        assert_ne!(subject_of(&later, Hit::Dismiss), Some(act));
        // AND THE INK, on both halves of the control and nothing else.
        let mut line = lines(&shown, 120, 940, &w).expect("the line");
        let before = line.clone();
        with_the_pick(&mut line, Hit::Proceed);
        let changed: Vec<_> = line
            .pieces
            .iter()
            .zip(&before.pieces)
            .filter(|(a, b)| a != b)
            .map(|(a, _)| (a.hit, a.color))
            .collect();
        assert_eq!(
            changed,
            vec![
                (Some(Hit::Proceed), tokens::color::TEXT_HERE_ACTS),
                (Some(Hit::Proceed), tokens::color::TEXT_HERE_ACTS)
            ]
        );
    }

    #[test]
    fn the_pie_is_the_bands_and_empties_as_the_time_goes() {
        // ONE FUNCTION FOR TWO PLACES. A mark that differed between the
        // notification's countdown and this one would make the person look
        // twice to decide they are the same thing.
        let a = announced(80, 1_000);
        for (now, fraction) in [(920_i64, 1.0_f32), (960, 0.5), (1_000, 0.0)] {
            assert!((left_of(&a, now) - fraction).abs() < 0.001);
            let mine = crate::band::countdown_glyph(ronoctua_notify::eighths(left_of(&a, now)));
            let theirs = crate::band::countdown_glyph(ronoctua_notify::eighths(fraction));
            assert_eq!(mine, theirs);
        }
        assert_eq!(seconds_left(&a, 2_000), 0);
        assert!((left_of(&a, 0) - 1.0).abs() < 0.001);
        assert!((left_of(&announced(0, 10), 0) - 0.0).abs() < 0.001);
    }

    /// A task of a fictional module, named by a key the bundle HAS — the
    /// strip draws the module's own name key and builds none.
    fn a_row(id: &str, name_key: &str) -> ronoctua_brain::TaskState {
        ronoctua_brain::TaskState {
            kind: ronoctua_brain::Kind::Routine,
            id: id.to_owned(),
            standing: ronoctua_brain::Standing::Idle,
            enabled: true,
            interval_days: 7,
            part_of_day: ronoctua_brain::PartOfDay::Afternoon,
            next: 0,
            hold: None,
            why_not: String::new(),
            history: Vec::new(),
            last: None,
            steps: Vec::new(),
            calendar: true,
            name_key: name_key.to_owned(),
            does_key: String::new(),
            why_key: String::new(),
            screen_generation: 0,
            actions: Vec::new(),
        }
    }

    #[test]
    fn the_sentence_says_the_task_and_the_seconds() {
        let t = words();
        let w = bundle(&t);
        let mut m = a_mind(Some(announced(60, 1_000)), Vec::new());
        // THE MODULE'S OWN NAME KEY, which the strip draws — never one built
        // out of the id.
        m.tasks.push(a_row("fixture", crate::key::BRAIN_TITLE));
        let line = lines(&Shown::of(Some(&m)), 200, 958, &w).expect("the line went away");
        // BY ITS COLUMN, and not by "the longest piece": the mark in the margin
        // is one character and four bytes.
        let sentence =
            at(Some(&line), 0, tokens::space::MODELINE_EDGELEFT + 2, None).expect("no phrase");
        assert!(sentence.text.contains("42"), "{}", sentence.text);
        assert!(
            !sentence.text.starts_with('['),
            "the sentence became a raw key: {}",
            sentence.text
        );
        assert!(
            sentence.text.starts_with(&t.t(crate::key::BRAIN_TITLE)),
            "the strip did not name the task by its module's key: {}",
            sentence.text
        );
    }

    /// FAILS IF: the strip's three controls ask for anything but the loudest
    /// notice, or ask for what it does not offer instead of saying so.
    #[test]
    fn the_notice_controls_answer_the_loudest_notice() {
        let m = a_mind(
            None,
            vec![a_notice(Level::Info, 99), a_notice(Level::Alarm, 1)],
        );
        let (ask, _) = notice_ask(Some(&m), Verb::Dismiss).expect("a dismissal");
        assert_eq!(
            ask,
            ronoctua_brain::Ask::Dismiss {
                module: "fixture".to_owned(),
                notice: "n-1".to_owned()
            }
        );
        let (ask, said) = notice_ask(Some(&m), Verb::Snooze).expect("a snooze");
        assert_eq!(
            ask,
            ronoctua_brain::Ask::Snooze {
                module: "fixture".to_owned(),
                notice: "n-1".to_owned(),
                minutes: 10
            }
        );
        assert_eq!(said, crate::key::ECHO_BRAIN_SNOOZED);
        let acting = act_of(Some(&m), None).expect("an action");
        assert_eq!(
            (acting.module.as_str(), acting.action.as_str()),
            ("fixture", "keep")
        );
        assert_eq!(acting.command, "fixture-keep");

        let mut shut = a_notice(Level::Warn, 5);
        shut.what.dismiss = false;
        shut.what.snooze = Vec::new().into();
        shut.what.controls.clear();
        let quiet = a_mind(None, vec![shut]);
        for verb in [Verb::Dismiss, Verb::Snooze] {
            assert_eq!(
                notice_ask(Some(&quiet), verb).map(|_| ()),
                Err(crate::key::ECHO_BRAIN_NOT_OFFERED)
            );
        }
        assert_eq!(
            act_of(Some(&quiet), None),
            Err(crate::key::ECHO_BRAIN_NOT_OFFERED)
        );
        assert_eq!(
            notice_ask(Some(&Mind::default()), Verb::Dismiss).map(|_| ()),
            Err(crate::key::ECHO_BRAIN_NO_NOTICE)
        );
    }

    /// FAILS IF: a module's mark is not drawn under `brain:<module>/<mark>`, in
    /// the colour its tone means, with its count — or a click runs anything
    /// but its own action or its module's page.
    #[test]
    fn a_mark_is_drawn_by_its_tone_and_clicked_to_its_action() {
        let m = Mind {
            marks: vec![
                Posted {
                    module: "fixture".to_owned(),
                    what: Mark {
                        id: MarkId::of("full", "fixture-full", "fixture-mark-full"),
                        tone: Tone::Danger,
                        count: 3,
                        act: None,
                    },
                },
                Posted {
                    module: "fixture-watch".to_owned(),
                    what: Mark {
                        id: MarkId::of("seen", "fixture-seen", "fixture-mark-seen"),
                        tone: Tone::Watching,
                        count: 0,
                        act: Some("seen".into()),
                    },
                },
            ],
            ..Mind::default()
        };
        let drawn = marks(Some(&m));
        assert_eq!(drawn.len(), 2);
        assert_eq!(drawn[0].id, "brain:fixture/full");
        assert!(is_mark(drawn[0].id));
        assert!(drawn[0].text.ends_with(" 3"), "{}", drawn[0].text);
        // THE COLOUR IS THE TONE'S — asked of the marks in hand, since the
        // table the footer reads is one per process and tests run side by
        // side.
        let colour = |id: &str| find(&drawn, id).map(|d| tone_colour(d.tone));
        assert_eq!(
            colour("brain:fixture/full"),
            Some(tokens::color::STATE_ERROR)
        );
        assert_eq!(
            colour("brain:fixture-watch/seen"),
            Some(tokens::color::MODELINE_WATCHING)
        );
        assert!(hint_in(&drawn[0], &words()).is_some_and(|h| !h.is_empty()));
        assert_eq!(click_of(&drawn[0]), ("brain-open", "fixture".to_owned()));
        assert_eq!(
            click_of(&drawn[1]),
            ("brain-act", "fixture-watch seen".to_owned())
        );
        // AN ACT IS THE MODULE'S, whatever it spells: a command of the
        // registry is the mark's ID's to name (`MarkId::opens`), the one way.
        let spelled = Drawn {
            act: Some("command:fixture-open".into()),
            ..drawn[1].clone()
        };
        assert_eq!(
            click_of(&spelled),
            ("brain-act", "fixture-watch command:fixture-open".to_owned())
        );
        // THE SAME ID IS THE SAME STATIC, however often it is asked for.
        let again = marks(Some(&m));
        assert!(std::ptr::eq(again[0].id, drawn[0].id));
        // AND AT REST, NOTHING — the modeline says nothing it does not mean.
        assert!(marks(Some(&Mind::default())).is_empty());
        assert!(marks(None).is_empty());
    }

    /// FAILS IF: a mark that OPENS an app of the shell is clicked to the
    /// module's page, or to anything but the command it named — by name, with
    /// nothing after it, as `LEADER :` would run it.
    #[test]
    fn a_mark_that_opens_an_app_is_clicked_to_its_command() {
        let m = Mind {
            marks: vec![Posted {
                module: "fixture".to_owned(),
                what: Mark {
                    id: MarkId::of("up", "fixture-up", "fixture-mark-up").opening("fixture-open"),
                    tone: Tone::Watching,
                    count: 2,
                    act: None,
                },
            }],
            ..Mind::default()
        };
        let drawn = marks(Some(&m));
        assert_eq!(click_of(&drawn[0]), ("fixture-open", String::new()));
    }

    /// FAILS IF: one of a mark's instances is not its own segment
    /// (`brain:<module>/<mark>.<n>`), its digit is not drawn a space after
    /// the glyph, its hint lacks the instance or the person's name for it,
    /// or a click does not hand the command the instance — the third account
    /// opening on the first.
    #[test]
    fn an_instance_is_its_own_segment_drawn_with_its_digit_and_clicked_with_it() {
        let seat = MarkId::declared("seat", "fixture-seat", "fixture-mark-seat", 9, true)
            .opening("fixture-open");
        let posted = |n: u8, label: Option<&str>, count: u32| Posted {
            module: "fixture".to_owned(),
            what: Mark {
                id: seat.numbered(n, label.map(ToOwned::to_owned)),
                tone: Tone::Quiet,
                count,
                act: None,
            },
        };
        let m = Mind {
            marks: vec![posted(1, Some("home"), 0), posted(3, None, 0)],
            ..Mind::default()
        };
        let drawn = marks(Some(&m));
        assert_eq!(
            drawn.iter().map(|d| d.id).collect::<Vec<_>>(),
            ["brain:fixture/seat.1", "brain:fixture/seat.3"]
        );
        assert!(drawn.iter().all(|d| is_mark(d.id)));
        let glyph = tokens::icon::BRAIN;
        assert_eq!(drawn[0].text, format!("{glyph} 1"));
        assert_eq!(drawn[1].text, format!("{glyph} 3"));
        assert_eq!(click_of(&drawn[0]), ("fixture-open", "1".to_owned()));
        assert_eq!(click_of(&drawn[1]), ("fixture-open", "3".to_owned()));
        // THE HINT'S VALUES: the number and the person's word, or nothing
        // where they gave none.
        let text = |d: &Drawn, name: &str| {
            let number = d.instance.to_string();
            hint_args(d, &number)
                .into_iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| format!("{v:?}"))
        };
        assert_eq!(text(&drawn[0], "label").as_deref(), Some(r#"Text("home")"#));
        assert_eq!(text(&drawn[0], "instance").as_deref(), Some("Number(1)"));
        assert_eq!(text(&drawn[1], "label").as_deref(), Some(r#"Text("3")"#));
        assert_eq!(text(&drawn[1], "count").as_deref(), Some("Number(0)"));
        // A BLANK WORD, from a mind written by hand, is none: the number
        // fills the sentence, never a hole.
        for blank in ["", "  "] {
            let unnamed = Drawn {
                label: Some(blank.to_owned()),
                ..drawn[1].clone()
            };
            assert_eq!(text(&unnamed, "label").as_deref(), Some(r#"Text("3")"#));
        }
        // A MARK THAT IS ONE draws its count where an instance draws its
        // digit, and nothing when it counts nothing; a mind carrying both
        // draws the instance.
        assert_eq!(mark_text(glyph, 0, 0), glyph.to_string());
        assert_eq!(mark_text(glyph, 0, 7), format!("{glyph} 7"));
        assert_eq!(mark_text(glyph, 2, 7), format!("{glyph} 2"));
    }

    /// FAILS IF: a mark's instances read on the modeline in any order but
    /// their own from the left — `3 2 1`, which the first photograph showed —
    /// or the marks around them change places.
    #[test]
    fn a_marks_instances_read_from_the_first_to_the_last() {
        let seat = MarkId::declared("seat", "fixture-seat", "fixture-mark-seat", 9, true);
        let posted = |module: &str, id: MarkId| Posted {
            module: module.to_owned(),
            what: Mark {
                id,
                tone: Tone::Warning,
                count: 0,
                act: None,
            },
        };
        let m = Mind {
            marks: vec![
                posted(
                    "fixture",
                    MarkId::of("full", "fixture-full", "fixture-mark-full"),
                ),
                posted("fixture", seat.numbered(1, None)),
                posted("fixture", seat.numbered(2, None)),
                posted("fixture", seat.numbered(3, None)),
                posted(
                    "other",
                    MarkId::of("unseen", "other-unseen", "other-mark-unseen"),
                ),
            ],
            ..Mind::default()
        };
        let drawn = marks(Some(&m));
        let pushed: Vec<&str> = from_the_edge(&drawn).iter().map(|d| d.id).collect();
        assert_eq!(
            pushed,
            [
                "brain:fixture/full",
                "brain:fixture/seat.3",
                "brain:fixture/seat.2",
                "brain:fixture/seat.1",
                "brain:other/unseen",
            ]
        );
        // AND ON THE LINE ITSELF, where the layout puts them: the column of
        // each instance, from the left.
        let segments = crate::modeline::segments(
            "",
            "",
            "",
            0,
            &crate::modeline::Marks {
                brain: &drawn,
                ..crate::modeline::Marks::default()
            },
        );
        let placed = ronoctua_render::layout::resolve(200, &segments);
        let column = |id: &str| {
            placed
                .iter()
                .find(|p| p.id == id)
                .map(|p| p.column)
                .unwrap_or_else(|| panic!("{id} was not placed"))
        };
        let seats: Vec<u16> = (1..=3)
            .map(|n| column(&format!("brain:fixture/seat.{n}")))
            .collect();
        assert!(seats.windows(2).all(|w| w[0] < w[1]), "{seats:?}");
    }

    /// FAILS IF: a notice appearing or going ON ITS OWN leaves the footer's
    /// face unchanged — the strip would stay as it was until something else
    /// moved, which is the defect `Face` was written for.
    #[test]
    fn a_notice_alone_changes_the_face() {
        let quiet = Mind::default();
        let said = a_mind(None, vec![a_notice(Level::Info, 1)]);
        assert_ne!(Face::of(Some(&quiet)), Face::of(Some(&said)));
        let two = a_mind(
            None,
            vec![a_notice(Level::Info, 1), a_notice(Level::Info, 0)],
        );
        assert_ne!(
            Face::of(Some(&said)),
            Face::of(Some(&two)),
            "a second notice changed nothing"
        );
        // AND A MIND THAT DIFFERS ONLY IN WHAT THE FOOTER DOES NOT DRAW is the
        // same face: the pulse moves every beat.
        let mut beat = said.clone();
        beat.now += 30;
        assert_eq!(Face::of(Some(&said)), Face::of(Some(&beat)));
    }

    #[test]
    fn what_was_never_measured_does_not_report_a_zero() {
        // The one thing the conversion must not do: a kernel without
        // `/proc/pressure/cpu` has no reading, and a zero there is a number
        // somebody would act on.
        let mind = Mind {
            pulse: ronoctua_brain::Pulse {
                at: 10,
                ram_used_pct: Some(0),
                memory_stall: None,
                cpu_stall: None,
                on_battery: Some(false),
                disk_free_per_mille: None,
                rooms: Vec::new(),
            },
            ..Mind::default()
        };
        let over = mind_of(&mind, &ronoctua_files::when::Zone::utc(), &words());
        let of: Vec<ronoctua_protocol::Vital> = over.pulse.signs.iter().map(|s| s.of).collect();
        assert_eq!(
            of,
            vec![
                ronoctua_protocol::Vital::RamUsed,
                ronoctua_protocol::Vital::OnBattery
            ]
        );
        assert_eq!(
            over.pulse.signs[0].amount, 0,
            "a measured zero became an absence"
        );
    }

    #[test]
    fn a_task_that_never_ran_says_so_instead_of_dating_1970() {
        let mind = Mind {
            tasks: vec![ronoctua_brain::TaskState {
                kind: ronoctua_brain::Kind::Routine,
                id: "fixture".to_owned(),
                standing: ronoctua_brain::Standing::Idle,
                enabled: true,
                interval_days: 7,
                part_of_day: ronoctua_brain::PartOfDay::Afternoon,
                next: 0,
                hold: None,
                why_not: "off".to_owned(),
                history: Vec::new(),
                last: None,
                steps: Vec::new(),
                calendar: true,
                name_key: String::new(),
                does_key: String::new(),
                why_key: String::new(),
                screen_generation: 0,
                actions: Vec::new(),
            }],
            ..Mind::default()
        };
        let over = mind_of(&mind, &ronoctua_files::when::Zone::utc(), &words());
        let row = &over.tasks[0];
        assert!(!row.ran, "a run that never happened became a run");
        assert!(row.next_text.is_empty(), "no next hour, and it dated one");
        assert_eq!(row.hold, ronoctua_protocol::HoldKind::None);
        // THE KEYS ARE FILLED, so the component builds none: the core's own
        // spelling of a name while the daemon has not sent the module's, and
        // the reason's key beside its word.
        assert_eq!(row.name_key, "brain-task-fixture");
        assert_eq!(row.does_key, "brain-task-fixture-does");
        assert_eq!(row.why_key, "brain-why-off");
        // And a moment that EXISTS is formatted, in ISO, because the component
        // has no calendar (invariant 11).
        let mut with = mind;
        with.tasks[0].next = 1_700_000_000;
        let over = mind_of(&with, &ronoctua_files::when::Zone::utc(), &words());
        assert_eq!(over.tasks[0].next_text, "2023-11-14 22:13");
    }

    /// FAILS IF: an action named by a module's command is not found among the
    /// actions the mind lists, a command of anything else is taken for one,
    /// or an action the module does not list is sent when typed by name.
    #[test]
    fn an_action_is_found_by_its_command() {
        let mut m = Mind::default();
        let mut task = ronoctua_brain::TaskState {
            kind: ronoctua_brain::Kind::Watch,
            id: "fixture-watch".to_owned(),
            standing: ronoctua_brain::Standing::Idle,
            enabled: true,
            interval_days: 7,
            part_of_day: ronoctua_brain::PartOfDay::Any,
            next: 0,
            hold: None,
            why_not: String::new(),
            history: Vec::new(),
            last: None,
            steps: Vec::new(),
            calendar: false,
            name_key: String::new(),
            does_key: String::new(),
            why_key: String::new(),
            screen_generation: 0,
            actions: Vec::new(),
        };
        task.actions.push(ronoctua_brain::mind::ActionState {
            id: "new".to_owned(),
            label: "cmd-fixture-watch-new".to_owned(),
            row: false,
            asks: Some("ask-fixture-watch-new".to_owned()),
        });
        m.tasks.push(task);
        let found = action_named(Some(&m), "fixture-watch-new").expect("the action");
        assert_eq!(found.asks.as_deref(), Some("ask-fixture-watch-new"));
        assert_eq!(found.module, "fixture-watch");
        assert!(action_named(Some(&m), "fixture-watch-old").is_none());
        assert!(action_named(Some(&m), "files-open").is_none());
        assert_eq!(
            act_of(Some(&m), Some("fixture-watch nothing")),
            Err(crate::key::ECHO_BRAIN_NO_ACTION)
        );
        let named = act_of(Some(&m), Some("fixture-watch new")).expect("named");
        assert_eq!(named.asks.as_deref(), Some("ask-fixture-watch-new"));
        assert_eq!(
            act_ask(&named, Some("tomorrow at nine")),
            ronoctua_brain::Ask::Act {
                module: "fixture-watch".to_owned(),
                action: "new".to_owned(),
                row: None,
                answer: Some("tomorrow at nine".to_owned())
            }
        );
    }
}
