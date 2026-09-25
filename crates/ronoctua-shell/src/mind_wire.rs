//! The mind, turned into the record that crosses to a component.
//!
//! # Why this is a file of its own
//!
//! Because two very different things needed it and only one of them could have
//! it. The footer's `crate::brain` owns the LINE under the minibuffer — it
//! reaches for smithay's event loop, for `crate::band`'s countdown glyph and
//! for `crate::leader`'s key constants — and none of that can be linked into
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
        tasks: mind.tasks.iter().map(|t| task_of(t, zone, texts)).collect(),
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
        // EVERYTHING THE BRAIN HAS DONE, newest first. The journal keeps it
        // oldest-first because that is the order it is appended in; the screen
        // opens on "what has just happened", so the reversal is here and not
        // in the component — a buffer that reversed a list the host could have
        // reversed would be a buffer doing the host's work.
        // A NOTICE THAT IS NOT THERE crosses as an empty `task`, the same
        // shape the announcement beside it uses: a record crosses and an
        // `Option` does not.
        notice: mind
            .notice
            .as_ref()
            .map_or_else(ronoctua_protocol::Notice::default, |n| {
                ronoctua_protocol::Notice {
                    task: n.task.clone(),
                    says: n.says.clone(),
                    act: n.act.clone().unwrap_or_default(),
                }
            }),
        activity: mind
            .journal
            .iter()
            .rev()
            .take(ronoctua_brain::journal::ACTIVITY)
            .map(|r| run_of(r, zone, texts))
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

/// A task's name, in the person's language.
///
/// The one place the brain's `brain-task-<id>` becomes a word. It is handed
/// over so the component can MEASURE it — what it draws is still the key, so a
/// language changed at run time reaches the screen without a new mind.
fn named(id: &str, texts: &ronoctua_i18n::Texts) -> String {
    texts.t(&alloc_key(id))
}

/// The key a task's name lives under.
fn alloc_key(id: &str) -> String {
    format!("brain-task-{id}")
}

/// One task's row.
fn task_of(
    t: &ronoctua_brain::TaskState,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::TaskState {
    ronoctua_protocol::TaskState {
        id: t.id.clone(),
        // THE NAME, RESOLVED HERE, because the component cannot: a key only
        // becomes a word on this side, and a width is a fact about the word.
        name_text: named(&t.id, texts),
        kind: match t.kind {
            ronoctua_brain::Kind::Routine => ronoctua_protocol::TaskKind::Routine,
            ronoctua_brain::Kind::Once => ronoctua_protocol::TaskKind::Once,
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
            .map(|r| run_of(r, zone, texts))
            .unwrap_or_default(),
        // AND THE LAST FEW, each with its date already made into words on this
        // side: a component has no zone and no calendar (invariant 11), so what
        // crosses is the number for the arithmetic and the text for the date it
        // cannot compute — the rule `next_text` beside it already follows.
        history: t.history.iter().map(|r| run_of(r, zone, texts)).collect(),
        steps: t.steps.iter().map(step_of).collect(),
    }
}

/// One run of the journal.
fn run_of(
    r: &ronoctua_brain::RunRecord,
    zone: &ronoctua_files::when::Zone,
    texts: &ronoctua_i18n::Texts,
) -> ronoctua_protocol::RunRecord {
    ronoctua_protocol::RunRecord {
        task: r.task.clone(),
        task_text: named(&r.task, texts),
        began: u64::try_from(r.began).unwrap_or(0),
        ended: u64::try_from(r.ended).unwrap_or(0),
        when_text: iso(r.began, zone),
        outcome: match r.kind {
            ronoctua_brain::OutcomeKind::Skipped => ronoctua_protocol::OutcomeKind::Skipped,
            ronoctua_brain::OutcomeKind::Done => ronoctua_protocol::OutcomeKind::Done,
            ronoctua_brain::OutcomeKind::Failed => ronoctua_protocol::OutcomeKind::Failed,
            ronoctua_brain::OutcomeKind::Held => ronoctua_protocol::OutcomeKind::Held,
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
        on_by_default: s.on_by_default,
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
