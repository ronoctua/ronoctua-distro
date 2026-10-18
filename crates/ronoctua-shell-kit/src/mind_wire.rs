//! The mind, turned into the record that crosses to a component.
//!
//! # Why this is a file of its own
//!
//! Because two very different things needed it and only one of them could have
//! it. The footer's `brain` owns the LINE under the minibuffer — it
//! reaches for smithay's event loop, for `crate::band`'s countdown glyph and
//! for `ronoctua_shell_footer::leader`'s key constants — and none of that can be linked into
//! `ronoctua-render-png`, which has no compositor and no keymap.
//!
//! And `render-png` is exactly what needed the conversion: until it had one,
//! the brain was **the buffer this repository could not photograph**. It draws
//! its empty screen without a mind, so `nix run .#png` produced a title and the
//! words "no tasks yet" — an honest picture of nothing. The owner said that was
//! wrong, and they were right: a screen nobody can open is a screen nobody
//! reviews.
//!
//! So the pure half moved here. It knows the daemon's records and the WIT's,
//! and nothing else: no Wayland, no tokens, no keymap, no translator.
//!
//! # What crosses, and what does not
//!
//! The DATES are formatted on this side. A component has no zone and no
//! calendar (invariant 11), so it gets the number for the arithmetic and the
//! text for the date it cannot compute.

use ronoctua_brain::mind::Posted;
use ronoctua_brain::module::{Arg, Args, Cell, Level, Notice, Screen, Tone};
use ronoctua_brain::{Announcement, Mind};

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
pub fn mind_of(
    mind: &Mind,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::Mind {
    ronoctua_protocol::Mind {
        now: u64::try_from(mind.now).unwrap_or(0),
        pulse: pulse_of(&mind.pulse),
        queue: mind.queue.clone(),
        tasks: mind
            .tasks
            .iter()
            .map(|t| task_of(t, mind, zone, texts))
            .collect(),
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
        // EVERYTHING THE BRAIN HAS DONE, newest first and FOLDED — see
        // [`activity_of`].
        activity_rows: activity_of(mind, zone, texts),
        // EVERY NOTICE, in the order the strip ranks them — see [`ranked`].
        notices: ranked(mind)
            .into_iter()
            .map(|n| notice_of(n, mind, zone, texts))
            .collect(),
        // THE SAME CONSTANT THAT DID THE CUTTING, one line above. The screen
        // says what its window is onto, and a number typed into the two
        // translations would be a third place for it to disagree with itself.
        remembers: u32::try_from(ronoctua_brain::journal::ACTIVITY).unwrap_or(u32::MAX),
        recent: u32::try_from(ronoctua_brain::journal::RECENT).unwrap_or(u32::MAX),
    }
}

/// The daemon's own log, as the WIT carries it, oldest first.
///
/// The time is formatted HERE, with the machine's zone, for the reason every
/// date in this file is: the component has neither. The text crosses as it was
/// written — a log line is data (`ronoctua_brain::tail`).
#[must_use]
pub fn log_of<'a>(
    lines: impl IntoIterator<Item = &'a ronoctua_brain::tail::Line>,
    zone: &ronoctua_files::when::Zone,
) -> Vec<ronoctua_protocol::LogLine> {
    use ronoctua_brain::tail::Level as From;
    use ronoctua_protocol::LogLevel as To;
    lines
        .into_iter()
        .map(|l| {
            let c = ronoctua_files::when::civil(l.at_ms.div_euclid(1000), zone);
            ronoctua_protocol::LogLine {
                seq: l.seq,
                at_text: format!("{:02}:{:02}:{:02}", c.hour, c.minute, c.second),
                level: match l.level {
                    From::Error => To::Error,
                    From::Warn => To::Warn,
                    From::Info => To::Info,
                    From::Debug => To::Debug,
                    From::Trace => To::Trace,
                },
                text: l.text.clone(),
            }
        })
        .collect()
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

/// A task's name, in the person's language, by its KEY.
///
/// The one place a key becomes the word the component MEASURES — what it draws
/// is still the key, so a language changed at run time reaches the screen
/// without a new mind.
fn named(key: &str, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(key)
}

/// The key a task's name lives under: the one its module declared, and the
/// core's own spelling of it (`brain-task-<id>`, which is what the generator
/// declares for every module) while the daemon has not filled it in.
fn name_key_of(id: &str, mind: &Mind) -> String {
    mind.task(id)
        .map(|t| t.name_key.clone())
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| format!("brain-task-{id}"))
}

/// The key a WORD of the brain's is drawn with.
///
/// The task's own `why_key` when the word is the task's `why_not` — that is
/// the one the daemon resolved against the module's `whys` — and the core's
/// `brain-why-<word>` otherwise: `runaway`, `aborted`, `ram`… are the brain's
/// own vocabulary and belong to no module. Empty for no word.
fn why_key_of(word: &str, task: Option<&ronoctua_brain::TaskState>) -> String {
    if word.is_empty() {
        return String::new();
    }
    task.filter(|t| t.why_not == word && !t.why_key.is_empty())
        .map_or_else(|| format!("brain-why-{word}"), |t| t.why_key.clone())
}

/// One task's row.
fn task_of(
    t: &ronoctua_brain::TaskState,
    mind: &Mind,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::TaskState {
    let name_key = name_key_of(&t.id, mind);
    ronoctua_protocol::TaskState {
        id: t.id.clone(),
        // THE NAME, RESOLVED HERE, because the component cannot: a key only
        // becomes a word on this side, and a width is a fact about the word.
        name_text: named(&name_key, texts),
        kind: match t.kind {
            ronoctua_brain::Kind::Routine => ronoctua_protocol::TaskKind::Routine,
            ronoctua_brain::Kind::Once => ronoctua_protocol::TaskKind::Once,
            ronoctua_brain::Kind::Watch => ronoctua_protocol::TaskKind::Watch,
        },
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
        last: t
            .last
            .as_ref()
            .map(|r| run_of(r, mind, zone, texts))
            .unwrap_or_default(),
        // AND THE LAST FEW, each with its date already made into words on this
        // side: a component has no zone and no calendar (invariant 11), so what
        // crosses is the number for the arithmetic and the text for the date it
        // cannot compute — the rule `next_text` beside it already follows.
        history: t
            .history
            .iter()
            .map(|r| run_of(r, mind, zone, texts))
            .collect(),
        steps: t.steps.iter().map(step_of).collect(),
        calendar: t.calendar,
        does_key: if t.does_key.is_empty() {
            format!("{name_key}-does")
        } else {
            t.does_key.clone()
        },
        name_key,
        why_key: why_key_of(&t.why_not, Some(t)),
        screen_generation: t.screen_generation,
        actions: t.actions.iter().map(action_of).collect(),
    }
}

/// One of a module's actions, as the WIT carries it.
fn action_of(a: &ronoctua_brain::mind::ActionState) -> ronoctua_protocol::ModuleAction {
    ronoctua_protocol::ModuleAction {
        id: a.id.clone(),
        label_key: a.label.clone(),
        row: a.row,
        asks_key: a.asks.clone().unwrap_or_default(),
    }
}

/// One run of the journal, with the keys its task and step declared.
fn run_of(
    r: &ronoctua_brain::RunRecord,
    mind: &Mind,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::RunRecord {
    let task = mind.task(&r.task);
    let task_key = name_key_of(&r.task, mind);
    // THE STEP'S OWN KEY, from the task that declared it — never a key built
    // out of the id on this side. Empty when the step is not one the task
    // still declares, and the screen then draws nothing rather than brackets.
    let at_key = if r.at.is_empty() {
        String::new()
    } else {
        task.and_then(|t| t.steps.iter().find(|s| s.id == r.at))
            .map(|s| s.name_key.clone())
            .unwrap_or_default()
    };
    ronoctua_protocol::RunRecord {
        task: r.task.clone(),
        task_text: named(&task_key, texts),
        began: u64::try_from(r.began).unwrap_or(0),
        ended: u64::try_from(r.ended).unwrap_or(0),
        when_text: iso(r.began, zone),
        outcome: match r.kind {
            ronoctua_brain::OutcomeKind::Skipped => ronoctua_protocol::OutcomeKind::Skipped,
            ronoctua_brain::OutcomeKind::Done => ronoctua_protocol::OutcomeKind::Done,
            ronoctua_brain::OutcomeKind::Failed => ronoctua_protocol::OutcomeKind::Failed,
            ronoctua_brain::OutcomeKind::Held => ronoctua_protocol::OutcomeKind::Held,
            ronoctua_brain::OutcomeKind::Noted => ronoctua_protocol::OutcomeKind::Noted,
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
        task_key,
        at_key,
        why_key: why_key_of(&r.why, task),
        says: r.says.clone(),
        // WHAT A NOTE SAYS, made into the person's sentence with its values
        // here — the component cannot format a size or a moment.
        says_text: if r.says.is_empty() {
            String::new()
        } else {
            sentence(&r.says, &r.args, zone, texts)
        },
    }
}

/// One step of a task, with the keys its module declared.
fn step_of(s: &ronoctua_brain::StepState) -> ronoctua_protocol::TaskStep {
    ronoctua_protocol::TaskStep {
        id: s.id.clone(),
        command: s.command.clone(),
        needs_root: s.needs_root,
        on: s.on,
        on_by_default: s.on_by_default,
        name_key: s.name_key.clone(),
        does_key: s.does_key.clone(),
    }
}

/// EVERYTHING THE BRAIN HAS DONE, newest first, FOLDED.
///
/// The journal keeps it oldest-first because that is the order it is appended
/// in; the screen opens on "what has just happened". The cut is the brain's
/// own ceiling on RECORDS (`journal::ACTIVITY`), taken before folding, so the
/// window the title names is the same hundred records whatever folds; and the
/// fold is the brain's own (`journal::grouped`) — one rule for what "says the
/// same thing" means, in the crate that writes the records.
fn activity_of(
    mind: &Mind,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> Vec<ronoctua_protocol::ActivityRow> {
    let kept = mind
        .journal
        .len()
        .saturating_sub(ronoctua_brain::journal::ACTIVITY);
    ronoctua_brain::journal::grouped(&mind.journal[kept..])
        .into_iter()
        .map(|g| ronoctua_protocol::ActivityRow {
            last_text: iso(g.newest.began, zone),
            first_text: iso(g.first, zone),
            newest: run_of(&g.newest, mind, zone, texts),
            repeats: g.repeats,
            members: g
                .members
                .iter()
                .map(|r| run_of(r, mind, zone, texts))
                .collect(),
        })
        .collect()
}

/// EVERY NOTICE, in the order the strip ranks them: the loudest first — an
/// alarm, then danger, warn and info — and among equals the NEWEST first.
///
/// One order for every door: the strip says the first, the brain's page lists
/// them in this order, and an index a component hands back points into it.
#[must_use]
pub fn ranked(mind: &Mind) -> Vec<&Posted<Notice>> {
    let mut out: Vec<&Posted<Notice>> = mind.notices.iter().collect();
    out.sort_by(|a, b| {
        b.what
            .level
            .cmp(&a.what.level)
            .then(b.what.since.cmp(&a.what.since))
    });
    out
}

/// One notice, as the WIT carries it — its sentence made, its moment dated and
/// its controls resolved against what its module offers.
fn notice_of(
    posted: &Posted<Notice>,
    mind: &Mind,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::Notice {
    let n = &posted.what;
    let offered = mind.task(&posted.module).map(|t| t.actions.as_slice());
    ronoctua_protocol::Notice {
        module: posted.module.clone(),
        id: n.id.to_string(),
        says: n.says.to_string(),
        says_text: sentence(&n.says, &n.args, zone, texts),
        args: n
            .args
            .iter()
            .map(|(name, arg)| ronoctua_protocol::NoticeArg {
                name: name.to_string(),
                text: arg_text(arg, zone, texts),
            })
            .collect(),
        level: match n.level {
            Level::Info => ronoctua_protocol::NoticeLevel::Info,
            Level::Warn => ronoctua_protocol::NoticeLevel::Warn,
            Level::Danger => ronoctua_protocol::NoticeLevel::Danger,
            Level::Alarm => ronoctua_protocol::NoticeLevel::Alarm,
        },
        since: u64::try_from(n.since).unwrap_or(0),
        since_text: iso(n.since, zone),
        controls: n
            .controls
            .iter()
            .map(|id| control_of(&posted.module, id, offered))
            .collect(),
        dismissable: n.dismiss,
        snooze: n.snooze.to_vec(),
    }
}

/// One control of a notice: the action its module declared under that id, or
/// — for a module the mind does not list — the registry's own spelling of it,
/// `cmd-<module>-<action>`, which is what the generator names every action.
fn control_of(
    module: &str,
    id: &str,
    offered: Option<&[ronoctua_brain::mind::ActionState]>,
) -> ronoctua_protocol::ModuleAction {
    offered
        .and_then(|all| all.iter().find(|a| a.id == id))
        .map_or_else(
            || ronoctua_protocol::ModuleAction {
                id: id.to_owned(),
                label_key: control_label(module, id, None),
                row: false,
                asks_key: String::new(),
            },
            action_of,
        )
}

/// The key a control of a notice is labelled with — the same one the brain's
/// page and the strip draw, so one action has one name in every door.
#[must_use]
pub fn control_label(
    module: &str,
    id: &str,
    offered: Option<&[ronoctua_brain::mind::ActionState]>,
) -> String {
    offered
        .and_then(|all| all.iter().find(|a| a.id == id))
        .map(|a| a.label.clone())
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| format!("cmd-{module}-{id}"))
}

/// A module's own page, as the WIT carries it: the same rows, each cell's
/// moment dated and each size written in the person's units.
#[must_use]
pub fn screen_of(
    module: &str,
    generation: u64,
    screen: &Screen,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::ModuleScreen {
    ronoctua_protocol::ModuleScreen {
        module: module.to_owned(),
        generation,
        sections: screen
            .sections
            .iter()
            .map(|s| ronoctua_protocol::ModuleSection {
                heading: s.heading.to_string(),
                rows: s
                    .rows
                    .iter()
                    .map(|r| ronoctua_protocol::ModuleRow {
                        id: r.id.clone(),
                        label: cell_of(&r.label, zone, texts),
                        value: cell_of(&r.value, zone, texts),
                        note: cell_of(&r.note, zone, texts),
                        tone: tone_of(r.tone),
                        act: r.act.as_deref().unwrap_or_default().to_owned(),
                        also: r.also.as_deref().unwrap_or_default().to_owned(),
                    })
                    .collect(),
            })
            .collect(),
        refused: String::new(),
    }
}

/// A module's page that did NOT come: no rows, and the sentence that says
/// why in their place — the brain refused it, or did not answer. A page with
/// nothing on it and a page refused must not look the same.
#[must_use]
pub fn refused_screen(
    module: &str,
    generation: u64,
    said: String,
) -> ronoctua_protocol::ModuleScreen {
    ronoctua_protocol::ModuleScreen {
        module: module.to_owned(),
        generation,
        sections: Vec::new(),
        refused: said,
    }
}

/// One cell of a module's row.
fn cell_of(
    c: &Cell,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::ScreenCell {
    use ronoctua_protocol::ScreenCell as C;
    match c {
        Cell::Empty => C::Empty,
        Cell::Key(k) => C::Key(k.to_string()),
        Cell::Data(d) => C::Data(d.clone()),
        Cell::Moment(at) => C::Moment(ronoctua_protocol::Timed {
            at: u64::try_from(*at).unwrap_or(0),
            text: iso(*at, zone),
        }),
        Cell::Count(n) => C::Count(*n),
        Cell::Bytes(b) => C::Bytes(ronoctua_protocol::Measured {
            bytes: *b,
            text: crate::sizes::readable_bytes(*b, texts.decimal_mark()),
        }),
    }
}

/// A tone, as the WIT names it.
pub(crate) const fn tone_of(t: Tone) -> ronoctua_protocol::BrainTone {
    match t {
        Tone::Quiet => ronoctua_protocol::BrainTone::Quiet,
        Tone::Watching => ronoctua_protocol::BrainTone::Watching,
        Tone::Warning => ronoctua_protocol::BrainTone::Warning,
        Tone::Danger => ronoctua_protocol::BrainTone::Danger,
        Tone::Accent => ronoctua_protocol::BrainTone::Accent,
    }
}

/// One value of a module's sentence, as the person reads it.
///
/// The SHELL formats, never the module and never the component (invariants 8
/// and 11): a size in the person's units and decimal mark, a proportion with
/// that mark, a moment in the machine's zone, and a key in the person's
/// language. A text is DATA — a unit's name, what somebody typed — and goes
/// through as it is.
#[must_use]
pub fn arg_text(
    arg: &Arg,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    match arg {
        Arg::Text(t) => t.clone(),
        Arg::Key(k) => texts.t(k),
        Arg::Number(n) => n.to_string(),
        Arg::Bytes(b) => crate::sizes::readable_bytes(*b, texts.decimal_mark()),
        Arg::PerMille(p) => format!("{}{}{}", p / 10, texts.decimal_mark(), p % 10),
        Arg::Moment(at) => iso(*at, zone),
    }
}

/// A module's sentence, made: its key and its values, in the person's
/// language.
///
/// A COUNT CROSSES AS A NUMBER, and everything else as its text: a Fluent
/// selector with a numeric branch handed the string `"1"` looks for a variant
/// called `1`, finds none and falls to the default without a word
/// (`numeric-selectors-get-numbers`).
#[must_use]
pub fn sentence(
    key: &str,
    args: &Args,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let texts_of: Vec<Option<String>> = args
        .iter()
        .map(|(_, a)| match a {
            Arg::Number(_) => None,
            other => Some(arg_text(other, zone, texts)),
        })
        .collect();
    let fluent: Vec<(&str, ronoctua_i18n::Arg<'_>)> = args
        .iter()
        .zip(&texts_of)
        .map(|((name, a), text)| {
            let value = match (a, text) {
                (Arg::Number(n), _) => ronoctua_i18n::Arg::Number(*n),
                (_, Some(t)) => ronoctua_i18n::Arg::Text(t.as_str()),
                (_, None) => ronoctua_i18n::Arg::Text(""),
            };
            (name.as_ref(), value)
        })
        .collect();
    texts.t_args(key, &fluent)
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
pub const fn part_back(p: ronoctua_protocol::DayPart) -> ronoctua_brain::PartOfDay {
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

/// What became of an ask that CHANGES something — a notice put away or off, a
/// module's action, a preference — said in the door that asked.
///
/// # Why these are asked and not told
///
/// Because the brain can refuse every one of them — a notice that is no
/// longer up, a snooze it does not offer, an action its module declined — and
/// a refusal sent into a socket nobody reads was a footer saying "put away"
/// about a notice the brain kept. A refusal is a sentence (the owner's rule),
/// and it is said where the key was pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The brain took it: the text key of what the door says then, or `None`
    /// for an ask whose success is already on the page it came from.
    Taken(Option<&'static str>),
    /// Refused, and why: a text key when a module declined (its
    /// `<module>-why-<word>`), the daemon's own words when the ask made no
    /// sense to it.
    Refused(String),
    /// Nobody answered: not listening, or silent past the client's patience.
    Unreachable,
}

impl Answer {
    /// The sentence the door says, or `None` when there is nothing to say.
    ///
    /// A REASON THAT IS A KEY is translated; one that is not is the daemon's
    /// own words, and it goes inside a sentence of ours — the frame
    /// `studio-trouble-encoder` puts round what ffmpeg said.
    #[must_use]
    pub fn sentence(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        match self {
            Self::Taken(said) => said.map(|k| texts.t(k)),
            Self::Refused(why) if texts.has(why) => Some(texts.t(why)),
            Self::Refused(why) => {
                Some(texts.t_with(crate::key::ECHO_BRAIN_REFUSED, &[("why", why.as_str())]))
            }
            Self::Unreachable => Some(texts.t(crate::key::ECHO_BRAIN_UNREACHABLE)),
        }
    }

    /// Whether the brain did it.
    #[must_use]
    pub const fn taken(&self) -> bool {
        matches!(self, Self::Taken(_))
    }
}

/// What an answer read off the socket means for an ask that changes
/// something, with `taken` the sentence to say when it went well.
///
/// EXHAUSTIVE over [`ronoctua_brain::Say`] on purpose: a case the brain grows
/// is a build that stops HERE, and not an answer read as a success.
#[must_use]
pub fn answer_from(
    said: anyhow::Result<ronoctua_brain::Say>,
    taken: Option<&'static str>,
) -> Answer {
    use ronoctua_brain::Say;
    match said {
        Ok(Say::Refused { why }) => Answer::Refused(why),
        // A MODULE'S OWN REFUSAL comes as its text key, in a case of its own,
        // which `Answer::sentence` translates.
        Ok(Say::Declined { key }) => Answer::Refused(key),
        Ok(Say::Mind { .. }) => Answer::Taken(taken),
        // NEVER THE ANSWER TO A CHANGE: a page or the log on this socket is
        // the protocol drifting, and saying "done" about it would be a lie.
        Ok(other @ (Say::Screen { .. } | Say::Lines { .. })) => {
            tracing::warn!(?other, "the brain answered a change with something else");
            Answer::Unreachable
        }
        Err(e) => {
            tracing::warn!(error = %e, "the brain did not answer");
            Answer::Unreachable
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ronoctua_brain::module::{Arg, Cell, Level, Notice, Row, Section, Tone};

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").unwrap()
    }

    fn utc() -> ronoctua_files::when::Zone {
        ronoctua_files::when::Zone::utc()
    }

    fn a_task(id: &str) -> ronoctua_brain::TaskState {
        ronoctua_brain::TaskState {
            kind: ronoctua_brain::Kind::Routine,
            id: id.to_owned(),
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
            calendar: true,
            name_key: String::new(),
            does_key: String::new(),
            why_key: String::new(),
            screen_generation: 0,
            actions: Vec::new(),
        }
    }

    fn a_record(
        task: &str,
        at: i64,
        kind: ronoctua_brain::OutcomeKind,
    ) -> ronoctua_brain::RunRecord {
        ronoctua_brain::RunRecord {
            task: task.to_owned(),
            began: at,
            ended: at,
            kind,
            ..ronoctua_brain::RunRecord::default()
        }
    }

    /// FAILS IF: the activity crosses unfolded, folds records that do not say
    /// the same thing, or cuts by ROWS instead of by the brain's ceiling on
    /// RECORDS — the window the page's title names.
    #[test]
    fn the_activity_crosses_folded_and_cut_by_records() {
        use ronoctua_brain::OutcomeKind as K;
        let mut mind = Mind::default();
        mind.tasks.push(a_task("fixture"));
        // OLDEST FIRST, as the journal keeps it: a run, three holds, a run.
        mind.journal = vec![
            a_record("fixture", 10, K::Done),
            a_record("fixture", 20, K::Held),
            a_record("fixture", 30, K::Held),
            a_record("fixture", 40, K::Held),
            a_record("fixture", 50, K::Done),
        ];
        let over = mind_of(&mind, &utc(), &texts());
        let shape: Vec<(u32, u64)> = over
            .activity_rows
            .iter()
            .map(|r| (r.repeats, r.newest.began))
            .collect();
        assert_eq!(shape, vec![(1, 50), (3, 40), (1, 10)]);
        let held = &over.activity_rows[1];
        assert_eq!(
            held.members.len(),
            3,
            "the fold does not open into its records"
        );
        assert_eq!(held.last_text, iso(40, &utc()));
        assert_eq!(held.first_text, iso(20, &utc()));
        // THE KEYS CROSS WITH THE RECORD, the core's spelling while the
        // module has not sent its own.
        assert_eq!(held.newest.task_key, "brain-task-fixture");

        // AND THE CEILING IS ON RECORDS: past it, the oldest go before folding.
        let many = ronoctua_brain::journal::ACTIVITY + 5;
        mind.journal = (0..many)
            .map(|n| a_record("fixture", i64::try_from(n).unwrap(), K::Held))
            .collect();
        let over = mind_of(&mind, &utc(), &texts());
        assert_eq!(over.activity_rows.len(), 1);
        assert_eq!(
            over.activity_rows[0].repeats as usize,
            ronoctua_brain::journal::ACTIVITY
        );
    }

    /// FAILS IF: the notices cross in any order but the strip's — the loudest
    /// first, the newest among equals — or a notice's sentence is not made
    /// here with its values: a count as a NUMBER, so a plural picks its branch.
    #[test]
    fn the_notices_cross_ranked_with_their_sentences_made() {
        let notice = |level: Level, since: i64, unread: i64| Posted {
            module: "fixture".to_owned(),
            what: Notice {
                id: format!("n-{since}").into(),
                says: crate::key::HINT_NOTIF_BELL.into(),
                args: vec![
                    ("unread".into(), Arg::Number(unread)),
                    ("filtered".into(), Arg::Number(0)),
                ],
                level,
                since,
                controls: vec!["keep".into()],
                dismiss: true,
                snooze: vec![5].into(),
            },
        };
        let mut mind = Mind::default();
        let mut task = a_task("fixture");
        task.actions.push(ronoctua_brain::mind::ActionState {
            id: "keep".to_owned(),
            label: "cmd-fixture-keep".to_owned(),
            row: false,
            asks: Some("ask-fixture-keep".to_owned()),
        });
        mind.tasks.push(task);
        mind.notices = vec![
            notice(Level::Info, 1, 1),
            notice(Level::Alarm, 2, 2),
            notice(Level::Info, 3, 1),
        ];
        let over = mind_of(&mind, &utc(), &texts());
        let order: Vec<&str> = over.notices.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(order, vec!["n-2", "n-3", "n-1"]);
        let t = texts();
        assert!(
            over.notices[1].says_text.starts_with("one unread"),
            "a count crossed as text and missed its branch: {}",
            over.notices[1].says_text
        );
        assert!(over.notices[0].says_text.starts_with("2 unread"));
        assert_ne!(over.notices[0].says_text, t.t(crate::key::HINT_NOTIF_BELL));
        // ITS CONTROLS ARE ITS MODULE'S ACTIONS, with their labels and their
        // questions.
        assert_eq!(over.notices[0].controls[0].label_key, "cmd-fixture-keep");
        assert_eq!(over.notices[0].controls[0].asks_key, "ask-fixture-keep");
        assert_eq!(over.notices[0].level, ronoctua_protocol::NoticeLevel::Alarm);
    }

    /// FAILS IF: a value of a module's sentence is formatted anywhere but here,
    /// or in anything but the person's own units and decimal mark.
    #[test]
    fn a_value_is_formatted_in_the_persons_units() {
        let t = texts();
        let mark = t.decimal_mark();
        assert_eq!(
            arg_text(&Arg::Bytes(12_340_000_000), &utc(), &t),
            format!("12{mark}3 GB")
        );
        assert_eq!(
            arg_text(&Arg::PerMille(153), &utc(), &t),
            format!("15{mark}3")
        );
        assert_eq!(
            arg_text(&Arg::Moment(1_700_000_000), &utc(), &t),
            "2023-11-14 22:13"
        );
        assert_eq!(
            arg_text(&Arg::Text("as typed".to_owned()), &utc(), &t),
            "as typed"
        );
        assert_eq!(
            arg_text(&Arg::Key(crate::key::BRAIN_TITLE.into()), &utc(), &t),
            t.t(crate::key::BRAIN_TITLE)
        );
    }

    /// FAILS IF: a module's page crosses with a cell unformatted, a tone lost,
    /// or a row's action dropped — the id `act-row` hands back.
    #[test]
    fn a_module_page_crosses_cell_by_cell() {
        let screen = ronoctua_brain::module::Screen {
            sections: vec![Section {
                heading: "fixture-heading".into(),
                rows: vec![Row {
                    id: "r-1".to_owned(),
                    label: Cell::Key("fixture-row".into()),
                    value: Cell::Bytes(2_000_000),
                    note: Cell::Moment(1_700_000_000),
                    tone: Tone::Watching,
                    act: Some("forget".into()),
                    also: Some("ignore".into()),
                }],
            }],
        };
        let over = screen_of("fixture", 7, &screen, &utc(), &texts());
        assert_eq!((over.module.as_str(), over.generation), ("fixture", 7));
        let row = &over.sections[0].rows[0];
        assert_eq!(row.act, "forget");
        assert_eq!(row.also, "ignore", "the row's second action was dropped");
        assert_eq!(row.tone, ronoctua_protocol::BrainTone::Watching);
        assert_eq!(
            row.value,
            ronoctua_protocol::ScreenCell::Bytes(ronoctua_protocol::Measured {
                bytes: 2_000_000,
                text: "2 MB".to_owned()
            })
        );
        assert_eq!(
            row.note,
            ronoctua_protocol::ScreenCell::Moment(ronoctua_protocol::Timed {
                at: 1_700_000_000,
                text: "2023-11-14 22:13".to_owned()
            })
        );
    }

    /// FAILS IF: a record's keys are built out of ids on the component's side
    /// again — the step's name is the key its task declared, a reason the
    /// task's own `why-key` when it is its `why-not`, and a module's name its
    /// own key once the daemon sends one.
    #[test]
    fn a_records_keys_are_the_ones_its_task_declared() {
        let mut task = a_task("fixture");
        task.name_key = "fixture-name".to_owned();
        task.why_not = "when-short".to_owned();
        task.why_key = "fixture-why-when-short".to_owned();
        task.steps.push(ronoctua_brain::StepState {
            id: "build".to_owned(),
            command: "true".to_owned(),
            needs_root: false,
            on: true,
            on_by_default: true,
            name_key: "fixture-step-build".to_owned(),
            does_key: "fixture-step-build-does".to_owned(),
        });
        let mut mind = Mind::default();
        mind.tasks.push(task);
        let mut failed = a_record("fixture", 5, ronoctua_brain::OutcomeKind::Failed);
        failed.at = "build".to_owned();
        let mut refused = a_record("fixture", 6, ronoctua_brain::OutcomeKind::Skipped);
        refused.why = "when-short".to_owned();
        let mut aborted = a_record("fixture", 7, ronoctua_brain::OutcomeKind::Skipped);
        aborted.why = "aborted".to_owned();
        let run = |r| run_of(r, &mind, &utc(), &texts());
        assert_eq!(run(&failed).at_key, "fixture-step-build");
        assert_eq!(run(&failed).task_key, "fixture-name");
        assert_eq!(run(&refused).why_key, "fixture-why-when-short");
        assert_eq!(run(&aborted).why_key, "brain-why-aborted");
        let over = mind_of(&mind, &utc(), &texts());
        assert_eq!(over.tasks[0].why_key, "fixture-why-when-short");
        assert_eq!(over.tasks[0].does_key, "fixture-name-does");
        assert_eq!(over.tasks[0].steps[0].name_key, "fixture-step-build");
    }

    /// FAILS IF: what the brain answered a change is not what the door says
    /// — a refusal read as a success, a module's reason left untranslated,
    /// the daemon's words said bare, a success said by a door that asked to
    /// say nothing, or a page or the log on this socket read as "done".
    #[test]
    fn an_answer_is_said_in_the_door_that_asked() {
        use crate::key;
        use ronoctua_brain::Say;
        for language in ronoctua_i18n::LANGUAGES {
            let texts = ronoctua_i18n::Texts::fresh(language).expect(language);
            let words = "the notice `due` of `fixture` is not up";
            let refused = answer_from(
                Ok(Say::Refused {
                    why: words.to_owned(),
                }),
                Some(key::ECHO_BRAIN_DISMISSED),
            );
            assert_eq!(refused, Answer::Refused(words.to_owned()));
            assert_eq!(
                refused.sentence(&texts),
                Some(texts.t_with(key::ECHO_BRAIN_REFUSED, &[("why", words)]))
            );
            assert_ne!(
                refused.sentence(&texts),
                Some(words.to_owned()),
                "the daemon's words were said bare"
            );
            // A REASON THAT IS A KEY is the module's, and it is translated.
            assert_eq!(
                Answer::Refused(key::ECHO_BRAIN_NOT_OFFERED.to_owned()).sentence(&texts),
                Some(texts.t(key::ECHO_BRAIN_NOT_OFFERED))
            );
            let taken = answer_from(
                Ok(Say::Mind {
                    mind: Box::default(),
                }),
                Some(key::ECHO_BRAIN_DISMISSED),
            );
            assert_eq!(taken, Answer::Taken(Some(key::ECHO_BRAIN_DISMISSED)));
            assert_eq!(
                taken.sentence(&texts),
                Some(texts.t(key::ECHO_BRAIN_DISMISSED))
            );
            assert_eq!(Answer::Taken(None).sentence(&texts), None);
            let gone = answer_from(Err(anyhow::anyhow!("nobody home")), None);
            assert_eq!(gone, Answer::Unreachable);
            assert_eq!(
                gone.sentence(&texts),
                Some(texts.t(key::ECHO_BRAIN_UNREACHABLE))
            );
            assert_eq!(
                answer_from(
                    Ok(Say::Lines { lines: Vec::new() }),
                    Some(key::ECHO_BRAIN_TAKEN)
                ),
                Answer::Unreachable,
                "the log on this socket was read as done"
            );
        }
    }
}
