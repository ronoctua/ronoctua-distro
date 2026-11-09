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
use ronoctua_brain::module::{Arg, Args, Cell, Fold, Level, Notice, Screen, Section, Tone};
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
        // THE SAME CONSTANTS THAT DID THE CUTTING, in the person's words. The
        // screen says what its window is onto, and a number typed into the
        // two translations would be a third place for it to disagree with
        // itself.
        activity_reach: reach(ronoctua_brain::journal::ACTIVITY_DAYS, texts),
        history_reach: reach(ronoctua_brain::journal::RECENT_DAYS, texts),
    }
}

/// How far back a list reaches, in the person's words: `last 15 days`.
fn reach(days: u16, texts: &ronoctua_i18n::Texts) -> String {
    texts.t_args(
        crate::key::BRAIN_REACH_DAYS,
        &[("days", ronoctua_i18n::Arg::Number(i64::from(days)))],
    )
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
    let does_key = if t.does_key.is_empty() {
        format!("{name_key}-does")
    } else {
        t.does_key.clone()
    };
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
        does_key: does_key.clone(),
        // ITS LINE, when its module declared one — every module of the distro
        // does (`gen-brain-catalogue.py`); a person's may not, and its row
        // then says its description.
        brief_key: {
            let brief = format!("{name_key}-brief");
            if texts.has(&brief) {
                brief
            } else {
                String::new()
            }
        },
        // AND THE DESCRIPTION IN WORDS, for the page to give it as many rows
        // as it wraps to — resolved here for the reason the name is.
        does_text: if texts.has(&does_key) {
            named(&does_key, texts)
        } else {
            String::new()
        },
        name_key,
        why_key: why_key_of(&t.why_not, Some(t)),
        // AND THE REASON IN WORDS, resolved here for the reason the name is:
        // the component measures the column it sits in.
        why_text: {
            let key = why_key_of(&t.why_not, Some(t));
            if key.is_empty() {
                String::new()
            } else {
                named(&key, texts)
            }
        },
        screen_generation: t.screen_generation,
        actions: t.actions.iter().map(action_of).collect(),
        folds: t
            .folds
            .iter()
            .map(|(id, open)| ronoctua_protocol::SectionState {
                id: id.clone(),
                open: *open,
            })
            .collect(),
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
        when_stamp: stamp(r.began, mind.now, zone),
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
        on_here: s.here(),
        name_key: s.name_key.clone(),
        does_key: s.does_key.clone(),
    }
}

/// EVERYTHING THE BRAIN HAS DONE, newest first, FOLDED.
///
/// The journal keeps it oldest-first because that is the order it is appended
/// in; the screen opens on "what has just happened". The cut is the brain's
/// own reach (`journal::ACTIVITY_DAYS` back from the mind's `now`), taken
/// before folding, so the window the title names is the same days whatever
/// folds; and the fold is the brain's own (`journal::grouped`) — one rule for
/// what "says the same thing" means, in the crate that writes the records.
fn activity_of(
    mind: &Mind,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> Vec<ronoctua_protocol::ActivityRow> {
    let since = ronoctua_brain::journal::since(mind.now, ronoctua_brain::journal::ACTIVITY_DAYS);
    let kept: Vec<ronoctua_brain::RunRecord> = mind
        .journal
        .iter()
        .filter(|r| r.began >= since)
        .cloned()
        .collect();
    ronoctua_brain::journal::grouped(&kept)
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
/// moment dated and cut where its day is, each size written in the person's
/// units. `now` says which day is today.
#[must_use]
pub fn screen_of(
    module: &str,
    generation: u64,
    screen: &Screen,
    now: i64,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::ModuleScreen {
    let cell = |c: &Cell| cell_of(c, now, zone, texts);
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
                        label: cell(&r.label),
                        value: cell(&r.value),
                        note: cell(&r.note),
                        tone: tone_of(r.tone),
                        act: r.act.as_deref().unwrap_or_default().to_owned(),
                        also: r.also.as_deref().unwrap_or_default().to_owned(),
                        below: cell(&r.below),
                        foot: r
                            .foot
                            .iter()
                            .map(|f| ronoctua_protocol::RowFact {
                                label: f.label.to_string(),
                                value: cell(&f.value),
                                tone: tone_of(f.tone),
                            })
                            .collect(),
                    })
                    .collect(),
                id: s.id.to_string(),
                about: s.about.to_string(),
                fold: fold_of(s),
                tally: cell(&s.tally),
            })
            .collect(),
        refused: String::new(),
        legend: screen
            .legend
            .iter()
            .map(|swatch| ronoctua_protocol::LegendSwatch {
                tone: tone_of(swatch.tone),
                says: swatch.says.to_string(),
                about: swatch.about.to_string(),
            })
            .collect(),
        phrases: phrases_of(module, screen, texts),
    }
}

/// How a section folds, as the WIT names it. One with no id does not: nothing
/// could keep what the person chose.
fn fold_of(s: &Section) -> ronoctua_protocol::SectionFold {
    use ronoctua_protocol::SectionFold as F;
    match s.fold {
        _ if s.id.is_empty() => F::Never,
        Fold::Never => F::Never,
        Fold::Open => F::Open,
        Fold::Shut => F::Shut,
    }
}

/// THE KEYS A PAGE DRAWS, in the person's language and each once, for the
/// component to MEASURE: they come with the page, so it could not declare
/// them, and it has no translator (invariant 11). It still draws the key. One
/// the texts do not have stays out, and the page measures that one blind.
///
/// A ROW'S OTHER ACTION too, by the label every action of `module` has: the
/// page names it flush right, so the width is what says where it begins.
fn phrases_of(
    module: &str,
    screen: &Screen,
    texts: &ronoctua_i18n::Texts,
) -> Vec<ronoctua_protocol::Phrase> {
    fn key_of(c: &Cell) -> Option<&str> {
        match c {
            Cell::Key(k) => Some(k),
            _ => None,
        }
    }
    let mut keys: Vec<&str> = Vec::new();
    for section in &screen.sections {
        keys.push(&section.heading);
        keys.extend(key_of(&section.tally));
        for row in &section.rows {
            let cells = [&row.label, &row.value, &row.note, &row.below];
            keys.extend(cells.into_iter().filter_map(key_of));
            for fact in &row.foot {
                keys.push(&fact.label);
                keys.extend(key_of(&fact.value));
            }
        }
    }
    keys.extend(screen.legend.iter().map(|swatch| swatch.says.as_ref()));
    let labels: Vec<String> = screen
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .filter_map(|row| row.also.as_deref())
        .map(|action| format!("cmd-{module}-{action}"))
        .collect();
    keys.extend(labels.iter().map(String::as_str));
    let mut seen = std::collections::BTreeSet::new();
    keys.into_iter()
        .filter(|k| texts.has(k) && seen.insert(*k))
        .map(|k| ronoctua_protocol::Phrase {
            key: k.to_owned(),
            text: texts.t(k),
        })
        .collect()
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
        refused: said,
        ..ronoctua_protocol::ModuleScreen::default()
    }
}

/// One cell of a module's row.
fn cell_of(
    c: &Cell,
    now: i64,
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
            stamp: stamp(*at, now, zone),
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
        Tone::Working => ronoctua_protocol::BrainTone::Working,
        Tone::Critical => ronoctua_protocol::BrainTone::Critical,
        Tone::Minor => ronoctua_protocol::BrainTone::Minor,
        // THE BRAIN'S PAGE DRAWS A READY MARK AS A QUIET ONE: the faded
        // colour is the modeline's (`brainlink::tone_colour`), and the WIT
        // keeps its tones — a new one would rebuild every buffer.
        Tone::Ready => ronoctua_protocol::BrainTone::Quiet,
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
        Arg::Percent(p) => texts.t_args(
            crate::key::MIND_PERCENT,
            &[("percent", ronoctua_i18n::Arg::Number(i64::from(*p)))],
        ),
        // A MOMENT IN THE PERSON'S SHAPE (`when::prefs`), as every date this
        // interface writes: an account's week starting over read `2026-11-10
        // 14:00` beside a clock and a file manager that wrote `10-nov-2026`.
        Arg::Moment(at) => crate::chrome::when_text(*at, zone, false, texts),
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

/// What a mark tells the pointer ([`ronoctua_brain::module::Told`]), made as
/// the minibuffer draws it: the sentence in what the line says, and each
/// value the module stresses in an ink of its own
/// ([`crate::chrome::Register::Figure`]; owner, 2026-11-09).
#[must_use]
pub fn told_runs(
    told: &ronoctua_brain::module::Told,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> Vec<crate::chrome::Piece> {
    let texts_of: Vec<Option<String>> = told
        .args
        .iter()
        .map(|(_, a)| match a {
            Arg::Number(_) => None,
            other => Some(arg_text(other, zone, texts)),
        })
        .collect();
    let fluent: Vec<(&str, ronoctua_i18n::Arg<'_>)> = told
        .args
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
    crate::chrome::marked_sentence(
        texts,
        &told.says,
        &fluent,
        |name| {
            told.stressed
                .iter()
                .any(|s| s == name)
                .then_some(crate::chrome::Register::Figure)
        },
        crate::chrome::Register::Said,
    )
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

/// The same moment as [`iso`], cut where its day is, and whether that day is
/// the day `now` falls on — in the machine's zone, which the component does
/// not have. The three pieces put together ARE [`iso`]'s text.
fn stamp(at: i64, now: i64, zone: &ronoctua_files::when::Zone) -> ronoctua_protocol::Stamp {
    let c = ronoctua_files::when::civil(at, zone);
    let today = ronoctua_files::when::civil(now, zone);
    ronoctua_protocol::Stamp {
        lead: format!("{:04}-{:02}-", c.year, c.month),
        day: format!("{:02}", c.day),
        tail: format!(" {:02}:{:02}", c.hour, c.minute),
        today: (c.year, c.month, c.day) == (today.year, today.month, today.day),
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
    use ronoctua_brain::module::{Arg, Fact, Row, Swatch};

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
            folds: std::collections::BTreeMap::new(),
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
    /// the same thing, or cuts by anything but the brain's reach in DAYS —
    /// the window the page's title names, and says.
    #[test]
    fn the_activity_crosses_folded_and_cut_by_days() {
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

        // AND THE REACH IS IN DAYS, counted back from the mind's `now`: a
        // hold a day older than fifteen goes before folding, however few
        // records there are.
        let day = ronoctua_brain::moment::DAY;
        let days = i64::from(ronoctua_brain::journal::ACTIVITY_DAYS);
        mind.now = 40 * day;
        mind.journal = (0..30)
            .map(|n| a_record("fixture", (10 + n) * day, K::Held))
            .collect();
        let over = mind_of(&mind, &utc(), &texts());
        assert_eq!(over.activity_rows.len(), 1);
        assert_eq!(
            i64::from(over.activity_rows[0].repeats),
            days,
            "the activity is not the last fifteen days"
        );
        // AND THE TITLE SAYS SO, in the person's words.
        assert_eq!(over.activity_reach, "last 15 days");
        assert_eq!(over.history_reach, "last 20 days");
    }

    /// FAILS IF: a record's moment does not cross cut where its day is — the
    /// three pieces are not its ISO text — or today is not today in the
    /// machine's zone.
    #[test]
    fn a_moment_crosses_cut_where_its_day_is() {
        use ronoctua_brain::OutcomeKind as K;
        let day = ronoctua_brain::moment::DAY;
        let mut mind = Mind::default();
        mind.tasks.push(a_task("fixture"));
        mind.now = 20_000 * day + 3_600;
        // TWO THAT DO NOT FOLD — a fold would make them one row.
        mind.journal = vec![
            a_record("fixture", 20_000 * day - 60, K::Failed),
            a_record("fixture", 20_000 * day + 60, K::Done),
        ];
        let over = mind_of(&mind, &utc(), &texts());
        let yesterday = &over.activity_rows[1].newest;
        let today = &over.activity_rows[0].newest;
        let whole = |r: &ronoctua_protocol::RunRecord| {
            format!(
                "{}{}{}",
                r.when_stamp.lead, r.when_stamp.day, r.when_stamp.tail
            )
        };
        assert_eq!(whole(today), today.when_text);
        assert_eq!(whole(yesterday), yesterday.when_text);
        assert_eq!(today.when_stamp.day.len(), 2);
        assert!(today.when_stamp.today, "a run of today is not today");
        assert!(
            !yesterday.when_stamp.today,
            "a run a minute before midnight is today"
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
            opens: None,
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
                    ..Row::default()
                }],
                ..Section::default()
            }],
            ..ronoctua_brain::module::Screen::default()
        };
        let over = screen_of("fixture", 7, &screen, 1_700_000_000, &utc(), &texts());
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
                text: "2023-11-14 22:13".to_owned(),
                stamp: ronoctua_protocol::Stamp {
                    lead: "2023-11-".to_owned(),
                    day: "14".to_owned(),
                    tail: " 22:13".to_owned(),
                    today: true,
                },
            })
        );
    }

    /// A page with everything a page can say: a legend, a section that folds
    /// with a tally of its own, and a row of each new tone with a line under
    /// it and facts for the footer.
    fn a_full_page() -> Screen {
        let key = |k: &'static str| Cell::Key(k.into());
        Screen {
            sections: vec![Section {
                id: "older".into(),
                heading: crate::key::BRAIN_TITLE.into(),
                about: "fixture-about".into(),
                rows: vec![
                    Row {
                        id: "r-1".to_owned(),
                        label: key(crate::key::BRAIN_TITLE),
                        tone: Tone::Critical,
                        below: Cell::Data("why it is here".to_owned()),
                        foot: vec![Fact {
                            label: crate::key::BRAIN_TITLE.into(),
                            value: Cell::Count(4),
                            tone: Tone::Minor,
                        }],
                        ..Row::default()
                    },
                    Row {
                        id: "r-2".to_owned(),
                        tone: Tone::Minor,
                        ..Row::default()
                    },
                ],
                fold: Fold::Shut,
                tally: Cell::Count(12),
            }],
            legend: vec![Swatch {
                tone: Tone::Critical,
                says: crate::key::BRAIN_WHY_ABORTED.into(),
                about: "fixture-swatch-about".into(),
            }],
        }
    }

    /// FAILS IF: anything a module can say of a page stops at this side of
    /// the WIT, a new tone falls back to a neighbour, or a row that names
    /// none of it crosses with something invented.
    #[test]
    fn a_whole_page_crosses_and_a_plain_row_crosses_empty() {
        use ronoctua_protocol::{BrainTone, Phrase, RowFact, ScreenCell, SectionFold};
        let t = texts();
        let over = screen_of("fixture", 7, &a_full_page(), 0, &utc(), &t);
        let section = &over.sections[0];
        assert_eq!(section.id, "older");
        assert_eq!(section.about, "fixture-about");
        assert_eq!(section.fold, SectionFold::Shut);
        assert_eq!(section.tally, ScreenCell::Count(12));
        let (row, plain) = (&section.rows[0], &section.rows[1]);
        assert_eq!(row.tone, BrainTone::Critical);
        assert_eq!(row.below, ScreenCell::Data("why it is here".to_owned()));
        assert_eq!(
            row.foot,
            [RowFact {
                label: crate::key::BRAIN_TITLE.to_owned(),
                value: ScreenCell::Count(4),
                tone: BrainTone::Minor,
            }]
        );
        assert_eq!(plain.tone, BrainTone::Minor);
        assert_eq!(plain.below, ScreenCell::Empty);
        assert!(plain.foot.is_empty());
        assert_eq!(over.legend[0].tone, BrainTone::Critical);
        assert_eq!(over.legend[0].says, crate::key::BRAIN_WHY_ABORTED);
        assert_eq!(over.legend[0].about, "fixture-swatch-about");
        // AND THE WORDS THAT MEASURE IT come with the page, in this language.
        assert!(
            over.phrases.contains(&Phrase {
                key: crate::key::BRAIN_WHY_ABORTED.to_owned(),
                text: t.t(crate::key::BRAIN_WHY_ABORTED),
            }),
            "the legend's word crossed with no phrase: {:?}",
            over.phrases
        );
    }

    /// FAILS IF: a section that cannot keep a choice is offered as one that
    /// folds, or a module that said nothing of folding has a page that does.
    #[test]
    fn a_section_folds_only_with_an_id() {
        use ronoctua_protocol::SectionFold;
        let fold = |id: &'static str, fold: Fold| {
            fold_of(&Section {
                id: id.into(),
                fold,
                ..Section::default()
            })
        };
        assert_eq!(fold("older", Fold::Shut), SectionFold::Shut);
        assert_eq!(fold("older", Fold::Open), SectionFold::Open);
        assert_eq!(fold("older", Fold::Never), SectionFold::Never);
        assert_eq!(fold("", Fold::Shut), SectionFold::Never);
        assert_eq!(fold("", Fold::Open), SectionFold::Never);
    }

    /// FAILS IF: a key the page draws cannot be measured — here each place a
    /// page says one has a key of its own, so losing any of them shows — or
    /// one crosses twice, a key the texts do not have crosses with no words
    /// behind it, or data that spells a key is taken for one.
    #[test]
    fn a_pages_phrases_are_its_keys_once_and_only_those_with_text() {
        use crate::key as k;
        const GONE: &str = "fixture-gone-from-the-texts";
        let key = |name: &'static str| Cell::Key(name.into());
        let page = Screen {
            sections: vec![Section {
                heading: k::BRAIN_TITLE.into(),
                tally: key(k::BRAIN_WHY_ABORTED),
                rows: vec![Row {
                    label: key(k::BRAIN_WHY_ASLEEP),
                    value: key(k::BRAIN_WHY_BATTERY),
                    note: key(k::BRAIN_WHY_BUSY),
                    below: key(k::BRAIN_WHY_CPU),
                    also: Some("next".into()),
                    foot: vec![
                        Fact {
                            label: k::BRAIN_WHY_NETWORK.into(),
                            value: key(k::BRAIN_WHY_POSTPONED),
                            ..Fact::default()
                        },
                        Fact {
                            label: k::BRAIN_TITLE.into(),
                            value: Cell::Data(k::BRAIN_WHY_REFUSED.to_owned()),
                            ..Fact::default()
                        },
                        Fact {
                            label: GONE.into(),
                            value: key(GONE),
                            ..Fact::default()
                        },
                    ],
                    ..Row::default()
                }],
                ..Section::default()
            }],
            legend: vec![
                Swatch {
                    says: k::BRAIN_WHY_RUNAWAY.into(),
                    ..Swatch::default()
                },
                Swatch {
                    says: GONE.into(),
                    ..Swatch::default()
                },
            ],
        };
        let t = texts();
        let phrases: Vec<_> = [
            k::BRAIN_TITLE,
            k::BRAIN_WHY_ABORTED,
            k::BRAIN_WHY_ASLEEP,
            k::BRAIN_WHY_BATTERY,
            k::BRAIN_WHY_BUSY,
            k::BRAIN_WHY_CPU,
            k::BRAIN_WHY_NETWORK,
            k::BRAIN_WHY_POSTPONED,
            k::BRAIN_WHY_RUNAWAY,
            k::CMD_BUFFER_NEXT,
        ]
        .into_iter()
        .map(|key| ronoctua_protocol::Phrase {
            key: key.to_owned(),
            text: t.t(key),
        })
        .collect();
        assert_eq!(phrases_of("buffer", &page, &t), phrases);
    }

    /// FAILS IF: `today` on a module's date does not follow the clock the
    /// page was given, the one thing the component cannot work out.
    #[test]
    fn a_modules_date_is_today_by_the_mind_s_clock() {
        const DAY: i64 = 86_400;
        let at = 1_700_000_000;
        let today = |now: i64| {
            let ronoctua_protocol::ScreenCell::Moment(moment) =
                cell_of(&Cell::Moment(at), now, &utc(), &texts())
            else {
                panic!("a moment did not cross as a moment")
            };
            assert_eq!(
                format!(
                    "{}{}{}",
                    moment.stamp.lead, moment.stamp.day, moment.stamp.tail
                ),
                moment.text,
                "the three pieces are the same date"
            );
            moment.stamp.today
        };
        assert!(today(at));
        assert!(!today(at + DAY));
        assert!(!today(at - DAY));
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
            on_here: Some(true),
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
        // A MODULE THAT DECLARED NO LINE crosses none, and its row says its
        // description; one the texts do not have crosses no words to measure.
        assert_eq!(over.tasks[0].brief_key, "");
        assert_eq!(over.tasks[0].does_text, "");
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
