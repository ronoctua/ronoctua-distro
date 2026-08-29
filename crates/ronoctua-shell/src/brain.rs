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
//! band's controls send. The same shape [`crate::transfers`] already has for
//! the queue — one file per border, layout and wire together, because they are
//! two halves of one feature and splitting them would hide which is which.

use ronoctua_brain::{Announcement, Mind};
use ronoctua_render::layout;
use ronoctua_tokens as tokens;

/// What a click on a piece of the line does.
///
/// Exactly two, and each one has a key: `brain-abort` and `brain-proceed` are
/// registered commands with a sequence of their own, drawn on the line itself.
/// The mouse reaches nothing the keyboard does not.
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
}

/// One drawn piece of the line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
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
}

/// The line, laid out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Line {
    /// Left to right.
    pub pieces: Vec<Piece>,
}

/// What the line needs, resolved once.
///
/// A struct and not three arguments, the same reason [`crate::band::Words`] is
/// one: they are always wanted together, and a caller that forgot the bundle
/// would get `[brain-band-soon]` on screen rather than a compile error.
pub struct Words<'a> {
    /// Where the sentences come from.
    pub texts: &'a ronoctua_i18n::Texts,
    /// How `brain-abort` is typed, leader included — `SPC s a`.
    ///
    /// It comes from OUTSIDE because whoever knows it is whoever defines the
    /// bind (`crate::leader`) and the label of the leader itself is the
    /// environment's (`crate::leader()`): in the nested loop the trigger is not
    /// `SPC`, and a line that said so would be teaching a key that does not
    /// exist there.
    pub abort_keys: &'a str,
    /// The same, for `brain-proceed`.
    pub proceed_keys: &'a str,
}

/// How many lines this region takes for what is showing.
///
/// # Why it does not depend on the width
///
/// The same reason [`crate::band::rows`] gives, and it is not weaker here: the
/// footer's height is the same on every output (see `Footer::apply_geometry`),
/// so a line that wrapped on the laptop and not on the monitor would reflow
/// every buffer when focus changed screen, with nothing having changed in them.
///
/// So it is one line or none, and what does not fit ELIDES. There is no second
/// line to spill onto, which is what makes the sentence the thing that gives
/// way rather than the layout.
#[must_use]
pub const fn rows(showing: Option<&Announcement>) -> u16 {
    match showing {
        Some(_) => 1,
        None => 0,
    }
}

/// How much of the warning is left, as a fraction (1.0 → 0.0).
///
/// `now` is epoch seconds, and it comes from the caller rather than from a
/// clock in here: the footer already beats a one-second timer and this is
/// computed on that beat. A clock of its own would be a second wake-up for one
/// effect — the trap `surface.rs`'s timer records for the blink.
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

/// The line, laid out for `columns` cells.
///
/// `now` is epoch seconds; everything the countdown shows is derived from it
/// and from the announcement's own `deadline`, because the brain sends the
/// warning ONCE and never a ticking number.
#[must_use]
pub fn lines(
    showing: Option<&Announcement>,
    columns: u16,
    now: i64,
    words: &Words<'_>,
) -> Option<Line> {
    let a = showing?;

    // THE MARGIN IS THE MODELINE'S, and not `chrome::INSET`.
    //
    // This line is peripheral reference — one glances at it, one does not read
    // it — which is the whole of the exception `chrome`'s header names. It is
    // the fourth site on that list, and it is named there.
    let left = tokens::space::MODELINE_EDGELEFT;
    let mut line = Line::default();

    // WHOSE LINE THIS IS, in the margin. It arrives without anybody having
    // asked for it, so the first thing on it says who is speaking — the same
    // argument `band::generic_glyph` makes for a notification's mark.
    line.pieces.push(Piece {
        at: left,
        text: tokens::icon::BRAIN.to_string(),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
    });
    let at = left + 2;

    // THE RIGHT-HAND CLUSTER FIRST, because it decides how much room the
    // sentence gets. Laying the text out first and then discovering the
    // controls do not fit is how a line comes to spill — `band.rs` records it,
    // and the answer is the same here.
    //
    // From the edge inwards: proceed, abort, the pie.
    let mut right = columns.saturating_sub(tokens::space::MODELINE_EDGERIGHT);

    right = control(
        &mut line,
        right,
        words.proceed_keys,
        &words.texts.t(ronoctua_i18n::key::BRAIN_BAND_PROCEED),
        Hit::Proceed,
        words.texts.t(ronoctua_i18n::key::CMD_BRAIN_PROCEED),
    );
    right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP);
    right = control(
        &mut line,
        right,
        words.abort_keys,
        &words.texts.t(ronoctua_i18n::key::BRAIN_BAND_ABORT),
        Hit::Abort,
        words.texts.t(ronoctua_i18n::key::CMD_BRAIN_ABORT),
    );

    // THE PIE, and it is `band::countdown_glyph` and not a second one.
    //
    // One function serves both places precisely so a mark cannot differ between
    // them: the notification band and this line are two countdowns on the same
    // screen, and a person who learned one has learned the other.
    right = right.saturating_sub(tokens::space::MODELINE_SEGMENTGAP + 1);
    line.pieces.push(Piece {
        at: right,
        text: crate::band::countdown_glyph(ronoctua_notify::eighths(left_of(a, now))).to_string(),
        color: tokens::color::BRAIN_COUNTDOWN,
        hit: None,
        explain: Some(words.texts.t(ronoctua_i18n::key::HINT_BRAIN_COUNTDOWN)),
    });

    // AND THE SENTENCE, in whatever is left.
    //
    // The subject is the TASK'S OWN NAME, looked up as `brain-task-<id>`: a
    // task added tomorrow inherits this line instead of needing a sentence of
    // its own, which is the door `ronoctua-brain`'s header promises.
    let sentence = words.texts.t_args(
        ronoctua_i18n::key::BRAIN_BAND_SOON,
        &[
            (
                "task",
                ronoctua_i18n::Arg::Text(&words.texts.t(&format!("brain-task-{}", a.task))),
            ),
            ("seconds", ronoctua_i18n::Arg::Number(seconds_left(a, now))),
        ],
    );
    let room = right.saturating_sub(at).saturating_sub(1);
    line.pieces.push(Piece {
        at,
        text: layout::elide_end(&sentence, room),
        color: tokens::color::BRAIN_TEXT,
        hit: None,
        explain: None,
    });

    Some(line)
}

/// One control — the bracketed key and the label — placed with its right edge
/// at `right`. Gives back the cell its left edge landed on.
///
/// TWO pieces and not one, because the two are coloured differently: the key is
/// `brain.dim` and the label is `brain.text`, which is what that token says it
/// is for. Both carry the same `hit` and the same explanation, so the pointer
/// answers the same thing over either half.
fn control(line: &mut Line, right: u16, keys: &str, label: &str, hit: Hit, explain: String) -> u16 {
    let bracketed = format!("[{keys}]");
    let width = layout::width(&bracketed) + 1 + layout::width(label);
    let start = right.saturating_sub(width);
    line.pieces.push(Piece {
        at: start,
        text: bracketed.clone(),
        color: tokens::color::BRAIN_DIM,
        hit: Some(hit),
        explain: Some(explain.clone()),
    });
    line.pieces.push(Piece {
        at: start + layout::width(&bracketed) + 1,
        text: label.to_owned(),
        color: tokens::color::BRAIN_TEXT,
        hit: Some(hit),
        explain: Some(explain),
    });
    start
}

/// Which piece of the line is at cell `column`.
///
/// From what was DRAWN, and not from a second calculation of where things would
/// be — the rule the modeline's segments and the notification band already
/// follow, for the reason both write down: a `Motion` arrives per pixel of
/// movement, and two calculations of one position are two chances to point at
/// the wrong thing.
#[must_use]
pub fn at(line: Option<&Line>, column: u16) -> Option<&Piece> {
    line?
        .pieces
        .iter()
        .find(|p| column >= p.at && column < p.at + layout::width(&p.text))
}

// ---------------------------------------------------------------------------
// The host half: the thread that listens, and what the two controls send
// ---------------------------------------------------------------------------

/// The `systemctl`, by path and never by name.
///
/// A unit's `PATH` is minimal (`CLAUDE.md`), and a bare name fails far from its
/// cause. Outside Nix — `cargo run` — it falls back to the name and the shell's
/// own `PATH` resolves it, which is the shape `crate::keyboard` already uses.
const SYSTEMCTL: &str = match option_env!("RONOCTUA_SYSTEMCTL") {
    Some(c) => c,
    None => "systemctl",
};

/// The unit `nix/modules/brain.nix` defines.
const UNIT: &str = "ronoctua-brain.service";

/// Brings up the thread that watches the brain and pushes every mind into
/// `calloop`.
///
/// It gives back `false` when there is nowhere to put a socket — and **that is
/// not fatal**, for the reason `transfers::listen` writes down: without it the
/// footer loses one line and goes on drawing everything else, and taking the
/// shell down over a missing `XDG_RUNTIME_DIR` would trade one feature for all
/// of them.
///
/// # Why a thread and not the socket in `calloop`
///
/// The two reasons the queue's thread already gives, and one of its own. A JSON
/// line can arrive split across two reads, which in the main loop is a state
/// machine and on a thread with a blocking `read_line` is one line of code. The
/// client RECONNECTS, which is a sleep and a retry, and a sleep in the loop that
/// draws is a frozen footer. And `client::watch` is explicitly the SLOW half of
/// that crate's two connects — `connect_quick` is what the drawing loop uses,
/// and it is what [`tell`] below calls.
pub fn listen(channel: smithay_client_toolkit::reexports::calloop::channel::Sender<Mind>) -> bool {
    if let Err(e) = ronoctua_brain::wire::socket_path() {
        tracing::warn!(error = %e, "no brain; the band does not appear");
        return false;
    }
    let built = std::thread::Builder::new()
        .name("brain".into())
        .spawn(move || {
            // THE UNIT FIRST, and on THIS thread — which is the whole reason
            // the start is here and not beside the call. `systemctl start`
            // returns when systemd has taken the job, not when the daemon
            // answers; the connect that follows does have to wait, and waiting
            // is what this thread is for.
            //
            // A failure is not worth a word above `debug`: in the nested loop
            // there is no unit at all, and `client::watch` starts the daemon
            // beside this binary instead. On the installed system it is already
            // running before the shell is.
            wake_the_unit();
            // The closure says "stop" only when the other end has gone, which
            // means the footer is already on its way out.
            let _ = ronoctua_brain::client::watch(|mind| channel.send(mind).is_err());
        });
    match built {
        Ok(_) => true,
        Err(e) => {
            tracing::error!(error = %e, "could not start the brain's thread");
            false
        }
    }
}

/// Asks systemd for the user unit. Failure is expected off the installed system.
fn wake_the_unit() {
    match std::process::Command::new(SYSTEMCTL)
        .args(["--user", "start", UNIT])
        .output()
    {
        Ok(out) if out.status.success() => tracing::info!(unit = UNIT, "brain asked of systemd"),
        Ok(out) => tracing::debug!(
            unit = UNIT,
            why = %String::from_utf8_lossy(&out.stderr).trim(),
            "systemd did not bring the brain up; carrying on through the socket"
        ),
        Err(e) => tracing::debug!(error = %e, "no systemctl; carrying on through the socket"),
    }
}

/// Sends one ask, and says in the log when there is nobody to send it to.
///
/// `tell` and not `ask`: none of these answer with anything the caller needs,
/// and waiting for a reply inside the loop that draws is the frozen footer this
/// project has paid for twice. What comes back is the whole mind, on the watch
/// stream, which is where the screen reads it anyway.
///
/// # Errors
///
/// Nobody listening.
pub fn tell(what: &ronoctua_brain::Ask) -> Result<(), anyhow::Error> {
    ronoctua_brain::client::tell(what)
}

/// What the band's `not now` sends.
///
/// # Why `postpone` and not `abort`, and why the two are one command
///
/// While the countdown is running, nothing is running yet: the task is QUEUED,
/// and `Ask::Abort` on a queued task only takes it out of the queue. The runner
/// enrols whatever is due on the next beat, so it would be back — with a fresh
/// countdown — half a minute later. Its own comment says exactly that about the
/// ordering of `put_off` against `enrol`.
///
/// `Ask::Postpone` is what moves the next moment: the runner writes it down as
/// an event, and the row comes back at its own hour instead of immediately.
///
/// So one command, `brain-abort`, means *stop it* in both situations and picks
/// the ask that actually stops it: postpone while it is only announced, abort
/// once something is running. Two commands for that would make the person
/// choose between two words for one intention, and get it wrong at the only
/// moment it matters.
#[must_use]
pub fn abort(mind: &Mind) -> ronoctua_brain::Ask {
    if mind.running.is_empty() {
        ronoctua_brain::Ask::Postpone
    } else {
        ronoctua_brain::Ask::Abort {
            task: mind.running.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// The daemon's mind, as the component draws it
// ---------------------------------------------------------------------------

/// The whole mind, turned into the record the WIT carries.
///
/// # What crosses, and what does not
///
/// The daemon's internal flags stay here: `postponed` is a decision in flight
/// between two of its threads, and by the time anything is drawn the runner has
/// already turned it into a record and a new moment — a field showing it would
/// be a field that is empty every time anybody looks.
///
/// The DATES are formatted on this side. The component has no zone and no
/// calendar (invariant 11), so it gets the number for the arithmetic and the
/// text for the date it cannot compute. ISO, because the buffer draws it
/// exactly as it arrives and *19 Aug* against *19 ago* is the kind of thing it
/// must not invent.
///
/// And the two `Option`s become the shapes a record can carry: a hold that is
/// nothing is [`ronoctua_protocol::HoldKind::None`], and a run that never
/// happened is `ran: false` — `began: 0` would be a moment in 1970, which is a
/// date the screen would draw.
#[must_use]
pub fn mind_of(mind: &Mind, zone: &ronoctua_files::when::Zone) -> ronoctua_protocol::Mind {
    ronoctua_protocol::Mind {
        now: u64::try_from(mind.now).unwrap_or(0),
        pulse: pulse_of(&mind.pulse),
        tasks: mind.tasks.iter().map(|t| task_of(t, zone)).collect(),
        queue: mind.queue.clone(),
        running: mind.running.clone(),
        rehearsing: mind.rehearsing.clone().unwrap_or_default(),
        announcement: mind
            .announcement
            .as_ref()
            .map(announcement_of)
            .unwrap_or_default(),
        gate: ronoctua_protocol::Gate {
            ram_ceiling_pct: mind.prefs.gate.ram_ceiling_pct,
            cpu_stall_per_mille: mind.prefs.gate.cpu_stall_per_mille,
            abort_on_battery: mind.prefs.gate.abort_on_battery,
        },
        // THE LADDER CROSSES, and is not a constant on the other side: which
        // cadences are worth having is a policy with its reason written in
        // `ronoctua_brain::prefs`, and a screen that stepped by one would take
        // twenty-three presses to get from a month to a week.
        cadence: ronoctua_brain::prefs::CADENCE.to_vec(),
    }
}

/// The warning, as the WIT carries it.
fn announcement_of(a: &Announcement) -> ronoctua_protocol::Announcement {
    ronoctua_protocol::Announcement {
        task: a.task.clone(),
        total_seconds: a.total_seconds,
        deadline: u64::try_from(a.deadline).unwrap_or(0),
        why: a.why.clone(),
    }
}

/// The pulse, as a LIST of what was actually measured.
///
/// What the machine did not measure does not report, and zero is a real
/// reading: a kernel with no `/proc/pressure/cpu` simply has no `cpu-stall`
/// here, and the screen draws a dash instead of a number nobody took. Turning
/// the `None` into a zero is the one thing this conversion must not do.
fn pulse_of(p: &ronoctua_brain::Pulse) -> ronoctua_protocol::Pulse {
    use ronoctua_protocol::{Vital, VitalSign};
    let mut signs = Vec::new();
    if let Some(v) = p.ram_used_pct {
        signs.push(VitalSign {
            of: Vital::RamUsed,
            amount: v,
        });
    }
    if let Some(v) = p.memory_stall {
        signs.push(VitalSign {
            of: Vital::MemoryStall,
            amount: v,
        });
    }
    if let Some(v) = p.cpu_stall {
        signs.push(VitalSign {
            of: Vital::CpuStall,
            amount: v,
        });
    }
    if let Some(v) = p.on_battery {
        signs.push(VitalSign {
            of: Vital::OnBattery,
            amount: u16::from(v),
        });
    }
    ronoctua_protocol::Pulse {
        at: u64::try_from(p.at).unwrap_or(0),
        signs,
    }
}

/// One task's row.
fn task_of(
    t: &ronoctua_brain::TaskState,
    zone: &ronoctua_files::when::Zone,
) -> ronoctua_protocol::TaskState {
    ronoctua_protocol::TaskState {
        id: t.id.clone(),
        standing: match t.standing {
            ronoctua_brain::Standing::Idle => ronoctua_protocol::Standing::Idle,
            ronoctua_brain::Standing::Queued => ronoctua_protocol::Standing::Queued,
            ronoctua_brain::Standing::Running => ronoctua_protocol::Standing::Running,
        },
        enabled: t.enabled,
        interval_days: t.interval_days,
        part_of_day: part_of(t.part_of_day),
        next: u64::try_from(t.next).unwrap_or(0),
        // ZERO IS "THERE IS NONE", and it is not a moment: a task that is
        // switched off has no next hour, and 1970 is not the honest way to say
        // so. The text is empty in that case, and the screen has a word for it.
        next_text: if t.next == 0 {
            String::new()
        } else {
            iso(t.next, zone)
        },
        hold: t.hold.map_or(ronoctua_protocol::HoldKind::None, hold_of),
        why_not: t.why_not.clone(),
        ran: t.last.is_some(),
        last: t.last.as_ref().map(|r| run_of(r, zone)).unwrap_or_default(),
        steps: t.steps.iter().map(step_of).collect(),
    }
}

/// One run of the journal.
fn run_of(
    r: &ronoctua_brain::RunRecord,
    zone: &ronoctua_files::when::Zone,
) -> ronoctua_protocol::RunRecord {
    ronoctua_protocol::RunRecord {
        began: u64::try_from(r.began).unwrap_or(0),
        ended: u64::try_from(r.ended).unwrap_or(0),
        when_text: iso(r.began, zone),
        outcome: match r.kind {
            ronoctua_brain::OutcomeKind::Skipped => ronoctua_protocol::OutcomeKind::Skipped,
            ronoctua_brain::OutcomeKind::Done => ronoctua_protocol::OutcomeKind::Done,
            ronoctua_brain::OutcomeKind::Failed => ronoctua_protocol::OutcomeKind::Failed,
        },
        at: r.at.clone(),
        why: r.why.clone(),
        trouble: r.trouble.clone(),
        steps: r
            .steps
            .iter()
            .map(|s| ronoctua_protocol::StepOutcome {
                id: s.id.clone(),
                ok: s.ok,
                seconds: s.seconds,
                trouble: s.trouble.clone(),
            })
            .collect(),
    }
}

/// One step of a task.
fn step_of(s: &ronoctua_brain::StepState) -> ronoctua_protocol::TaskStep {
    ronoctua_protocol::TaskStep {
        id: s.id.clone(),
        command: s.command.clone(),
        needs_root: s.needs_root,
        on: s.on,
        changed: s.changed,
    }
}

/// What is in the way, as the WIT names it.
const fn hold_of(h: ronoctua_brain::Hold) -> ronoctua_protocol::HoldKind {
    match h {
        ronoctua_brain::Hold::Ram => ronoctua_protocol::HoldKind::Ram,
        ronoctua_brain::Hold::Cpu => ronoctua_protocol::HoldKind::Cpu,
        ronoctua_brain::Hold::Battery => ronoctua_protocol::HoldKind::Battery,
        ronoctua_brain::Hold::Busy => ronoctua_protocol::HoldKind::Busy,
        ronoctua_brain::Hold::Network => ronoctua_protocol::HoldKind::Network,
        ronoctua_brain::Hold::Asleep => ronoctua_protocol::HoldKind::Asleep,
    }
}

/// Which stretch of the day, as the WIT names it.
pub(crate) const fn part_of(p: ronoctua_brain::PartOfDay) -> ronoctua_protocol::DayPart {
    match p {
        ronoctua_brain::PartOfDay::Any => ronoctua_protocol::DayPart::Any,
        ronoctua_brain::PartOfDay::Night => ronoctua_protocol::DayPart::Night,
        ronoctua_brain::PartOfDay::Morning => ronoctua_protocol::DayPart::Morning,
        ronoctua_brain::PartOfDay::Afternoon => ronoctua_protocol::DayPart::Afternoon,
        ronoctua_brain::PartOfDay::Evening => ronoctua_protocol::DayPart::Evening,
    }
}

/// The reverse: what the component chose, back into the daemon's word.
///
/// `None` for a value this build does not know, and it is REFUSED rather than
/// rounded to a neighbour — rounding here would move a rebuild out of the small
/// hours into the middle of somebody's afternoon.
pub(crate) const fn part_back(p: ronoctua_protocol::DayPart) -> ronoctua_brain::PartOfDay {
    match p {
        ronoctua_protocol::DayPart::Any => ronoctua_brain::PartOfDay::Any,
        ronoctua_protocol::DayPart::Night => ronoctua_brain::PartOfDay::Night,
        ronoctua_protocol::DayPart::Morning => ronoctua_brain::PartOfDay::Morning,
        ronoctua_protocol::DayPart::Afternoon => ronoctua_brain::PartOfDay::Afternoon,
        ronoctua_protocol::DayPart::Evening => ronoctua_brain::PartOfDay::Evening,
    }
}

/// A moment as the component draws it: ISO, in the machine's own zone.
///
/// `ronoctua_files::when::civil` and not a second calendar — it is the one this
/// repository already checks against `coreutils date` on the real zone, and a
/// second implementation would be the same hundred lines with a different bug.
fn iso(at: i64, zone: &ronoctua_files::when::Zone) -> String {
    let c = ronoctua_files::when::civil(at, zone);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn announced(total: u16, deadline: i64) -> Announcement {
        Announcement {
            task: "system-update".to_owned(),
            total_seconds: total,
            deadline,
            why: "due".to_owned(),
        }
    }

    fn bundle(t: &ronoctua_i18n::Texts) -> Words<'_> {
        Words {
            texts: t,
            abort_keys: "SPC s a",
            proceed_keys: "SPC s p",
        }
    }

    #[test]
    fn it_is_one_line_at_every_width_or_none_at_all() {
        // THE HEIGHT DOES NOT DEPEND ON THE WIDTH, and it may not: the footer
        // is the same height on every output, and a line that wrapped on the
        // laptop and not on the monitor would reflow every buffer when the
        // focus changed screen.
        assert_eq!(rows(None), 0);
        let a = announced(60, 1_000);
        assert_eq!(rows(Some(&a)), 1);
        let t = words();
        let w = bundle(&t);
        for columns in [20_u16, 40, 80, 200] {
            assert!(
                lines(Some(&a), columns, 940, &w).is_some(),
                "it went away at {columns} columns"
            );
        }
        assert!(lines(None, 80, 940, &w).is_none());
    }

    #[test]
    fn nothing_spills_off_the_line() {
        // The same property the modeline and the band are tested for, and for
        // the same reason: a compositor does not clip, so what spills is
        // rubbish at the screen's edge.
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);
        for columns in [20_u16, 30, 40, 80, 120, 200] {
            let line = lines(Some(&a), columns, 940, &w).expect("the line went away");
            for p in &line.pieces {
                assert!(
                    p.at + layout::width(&p.text) <= columns,
                    "'{}' leaks at {columns} columns",
                    p.text
                );
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
        let a = announced(60, 1_000);
        let line = lines(Some(&a), 120, 940, &w).expect("the line went away");
        for hit in [Hit::Abort, Hit::Proceed] {
            let mine: Vec<&Piece> = line.pieces.iter().filter(|p| p.hit == Some(hit)).collect();
            assert_eq!(mine.len(), 2, "{hit:?} does not have the key and the label");
            assert!(
                mine.iter().all(|p| p.explain.is_some()),
                "{hit:?} does not say what it does"
            );
        }
        // AND THE SEQUENCES ARE ON THE LINE, in brackets: a control whose key
        // is only in the manual is a control nobody presses.
        let drawn: String = line.pieces.iter().map(|p| p.text.as_str()).collect();
        assert!(drawn.contains("[SPC s a]"), "{drawn}");
        assert!(drawn.contains("[SPC s p]"), "{drawn}");
        // The margin carries the mark, and it is not a control.
        let first = line.pieces.first().unwrap();
        assert_eq!(first.at, tokens::space::MODELINE_EDGELEFT);
        assert_eq!(first.hit, None);
        assert_eq!(first.text, tokens::icon::BRAIN.to_string());
    }

    #[test]
    fn the_pointer_finds_what_was_drawn() {
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);
        let line = lines(Some(&a), 120, 940, &w).expect("the line went away");
        let proceed = line
            .pieces
            .iter()
            .find(|p| p.hit == Some(Hit::Proceed))
            .expect("no proceed control");
        assert_eq!(
            at(Some(&line), proceed.at).and_then(|p| p.hit),
            Some(Hit::Proceed)
        );
        // A cell with nothing in it is nothing, and not the nearest thing.
        let gap = tokens::space::MODELINE_EDGELEFT + 1;
        assert!(at(Some(&line), gap).is_none());
        assert!(at(None, 0).is_none());
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
        // A deadline already past does not go negative, and a clock that
        // stepped backwards does not make the pie fuller than full.
        assert_eq!(seconds_left(&a, 2_000), 0);
        assert!((left_of(&a, 0) - 1.0).abs() < 0.001);
        // A warning with no length at all is not a division by zero.
        assert!((left_of(&announced(0, 10), 0) - 0.0).abs() < 0.001);
    }

    #[test]
    fn the_sentence_says_the_task_and_the_seconds() {
        let t = words();
        let w = bundle(&t);
        let a = announced(60, 1_000);
        let line = lines(Some(&a), 200, 958, &w).expect("the line went away");
        // BY ITS COLUMN, and not by "the longest piece": the mark in the margin
        // is one character and four bytes, and a test that went looking for a
        // long `String` found the glyph.
        let sentence = at(Some(&line), tokens::space::MODELINE_EDGELEFT + 2).expect("no phrase");
        assert!(sentence.text.contains("42"), "{}", sentence.text);
        assert!(
            !sentence.text.starts_with('['),
            "the sentence became a raw key: {}",
            sentence.text
        );
    }

    #[test]
    fn not_now_postpones_while_nothing_is_running() {
        // THE DEFECT THIS CATCHES: `Ask::Abort` on a task that is only QUEUED
        // takes it out of the queue, and the runner enrols it again on the next
        // beat — the countdown comes back half a minute later and the person
        // answers the same question for ever. What moves the next moment is the
        // postponement.
        let mut mind = Mind::default();
        assert_eq!(abort(&mind), ronoctua_brain::Ask::Postpone);
        mind.running = "system-update".to_owned();
        assert_eq!(
            abort(&mind),
            ronoctua_brain::Ask::Abort {
                task: "system-update".to_owned()
            }
        );
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
            },
            ..Mind::default()
        };
        let over = mind_of(&mind, &ronoctua_files::when::Zone::utc());
        let of: Vec<ronoctua_protocol::Vital> = over.pulse.signs.iter().map(|s| s.of).collect();
        assert_eq!(
            of,
            vec![
                ronoctua_protocol::Vital::RamUsed,
                ronoctua_protocol::Vital::OnBattery
            ]
        );
        assert_eq!(over.pulse.signs[0].amount, 0, "zero medido virou ausencia");
    }

    #[test]
    fn a_task_that_never_ran_says_so_instead_of_dating_1970() {
        let mind = Mind {
            tasks: vec![ronoctua_brain::TaskState {
                id: "system-update".to_owned(),
                standing: ronoctua_brain::Standing::Idle,
                enabled: true,
                interval_days: 7,
                part_of_day: ronoctua_brain::PartOfDay::Afternoon,
                next: 0,
                hold: None,
                why_not: "off".to_owned(),
                last: None,
                steps: Vec::new(),
            }],
            ..Mind::default()
        };
        let over = mind_of(&mind, &ronoctua_files::when::Zone::utc());
        let row = &over.tasks[0];
        assert!(!row.ran, "a run that never happened became a run");
        assert!(row.next_text.is_empty(), "no next hour, and it dated one");
        assert_eq!(row.hold, ronoctua_protocol::HoldKind::None);
        // And a moment that EXISTS is formatted, in ISO, because the component
        // has no calendar (invariant 11).
        let mut with = mind;
        with.tasks[0].next = 1_700_000_000;
        let over = mind_of(&with, &ronoctua_files::when::Zone::utc());
        assert_eq!(over.tasks[0].next_text, "2023-11-14 22:13");
    }
}
