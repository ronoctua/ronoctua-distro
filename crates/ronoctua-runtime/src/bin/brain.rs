//! Exercises the brain's panel — `LEADER b`.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its
//! own (invariant 12): a `cargo test` of it tests the logic OUTSIDE the
//! component, which is precisely what proves nothing. Here it is loaded the way
//! the host loads it, pushed a mind the way the host pushes one, and read
//! through `render`.
//!
//! # What only this can prove
//!
//! This is the buffer with NO width constant of its own. The other seven
//! declare what they reserved and can be read; here the widths are numbers
//! inside `Table`s (`Column::fixed("label", 20, 2)`) and the keys ARRIVE at run
//! time — a module's name, its reason, a step's name travel beside the ids, and
//! `brain-hold-{}` and `brain-outcome-{}` are assembled. No parser of this
//! file's source can pair a key with a budget.
//!
//! The frame can: `Command::Text` carries the key and its `max_width` side by
//! side, already assembled. So this driver walks the buffer's screens and hands
//! them to `driven::every_label_fits`, which is the only way a label of this
//! buffer has ever been measured against the language.
//!
//! Usage: `ronoctua-test-brain <component.wasm> <i18n-dir> [--modules <file>]`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{
    ActivityRow, Announcement, BrainTone, DayPart, Gate, HoldKind, Input, Mind, ModuleAction,
    ModuleRow, ModuleScreen, ModuleSection, Notice, NoticeLevel, OutcomeKind, Pulse, RunRecord,
    ScreenCell, Standing, StepOutcome, TaskKind, TaskState, TaskStep, Vital, VitalSign,
};

/// The labels this buffer cuts ON PURPOSE, and why.
///
/// `(key prefix, the reason)`. The reason is not decoration: it is what tells
/// the next reader whether an entry is a decision or a leftover — the same
/// bargain `generator-flags-agree` makes with `--reflink`.
const ON_PURPOSE: &[(&str, &str)] = &[];

/// The scale heights the production host sends.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 40;
const COLUMNS: u16 = 100;

/// A moment far enough from the epoch that a countdown is not negative.
const NOW: u64 = 1_800_000_000;

/// EVERY TASK THE BRAIN HAS, so every one of them is drawn — read at RUN TIME.
///
/// # Why this is not a list any more
///
/// It was: five ids written here, and the day a sixth task arrived (the
/// clock's watch) the list did not learn it — its row was never drawn, and a
/// label of its missing from a language would have passed. A list in the
/// driver is a second registry of what the brain has.
///
/// So the ids come from what the brain REPORTS: the output of
/// `ronoctua-brain modules` (one module per line, its id first) when the check
/// hands it over as `--modules <file>`, and otherwise the modules the texts
/// declare — every `brain-task-<id>` beside its `brain-task-<id>-does`, which is
/// the pair the brain's generator requires of every module, in the bundle the
/// check hands in (the merged texts, the modules' fragments included).
fn reported(i18n: &str, modules: Option<&str>) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    if let Some(file) = modules {
        let raw =
            std::fs::read_to_string(file).with_context(|| format!("could not read {file}"))?;
        for line in raw.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(id) = line.split_whitespace().next()
                && !out.iter().any(|o| o == id)
            {
                out.push(id.to_owned());
            }
        }
    } else {
        let base = ronoctua_runtime::driven::languages(i18n)?
            .into_iter()
            .next()
            .context("no language in the texts")?;
        let table = ronoctua_runtime::driven::ftl(&format!("{i18n}/{base}.ftl"))?;
        let mut ids: Vec<String> = table
            .keys()
            .filter_map(|k| k.strip_prefix("brain-task-"))
            .filter(|id| !id.ends_with("-does"))
            .filter(|id| table.contains_key(&format!("brain-task-{id}-does")))
            .map(ToOwned::to_owned)
            .collect();
        ids.sort();
        out = ids;
    }
    anyhow::ensure!(
        !out.is_empty(),
        "the brain reports no module at all — the check is reading the wrong texts, \
         or `ronoctua-brain modules` said nothing"
    );
    Ok(out)
}

/// THE STEPS A MODULE DECLARES, with the keys it declared for each — the first
/// module the build compiled that has steps.
///
/// # Why the compiled registry and not a table
///
/// Because a step's KEYS are the module's, and the host sends them beside the
/// id (`name-key`, `does-key`): a driver that invented ids would draw bracketed
/// keys, and one that copied the real ones — or read one module's table by
/// its name — would be a second list of what the brain has. So the steps come
/// from what the brain itself registers (`compiled::all`), found by what they
/// are and never by whose.
fn declared_steps() -> Vec<&'static ronoctua_brain::Step> {
    ronoctua_brain::compiled::all(&ronoctua_brain::Birth::detached())
        .iter()
        .map(|m| m.steps())
        .find(|steps| !steps.is_empty())
        .map(|steps| steps.iter().collect())
        .unwrap_or_default()
}

/// Looks at the screen, remembering its labels, and refuses a frame that
/// overlaps itself.
///
/// AT THREE WIDTHS, because a collision appears at the width where two fields
/// happen to meet, and that width depends on how long a sentence came out in
/// whichever language is loaded.
fn look(inst: &mut ronoctua_runtime::Instance) -> Result<ronoctua_protocol::DisplayList> {
    let list = inst.render(COLUMNS, ROWS, heights())?;
    ronoctua_runtime::driven::note(&list);
    for columns in [46, 80, COLUMNS] {
        // THE NARROW ONES ARE NOT MEASURED, only looked at. A `fill` column
        // gives way on purpose, so at forty-six columns every budget is "what
        // was left" and the host elides by design.
        let at = if columns == COLUMNS {
            list.clone()
        } else {
            inst.render(columns, ROWS, heights())?
        };
        if let Err(e) = at.nothing_overlaps(heights(), columns) {
            bail!("at {columns} columns, {e}");
        }
        if let Err(e) = at.nothing_is_covered(heights(), columns) {
            bail!("at {columns} columns, {e}");
        }
    }
    Ok(list)
}

/// Fill in the names the HOST would have resolved, in this language, by the
/// KEY each row carries — what `mind_wire` does in production.
///
/// # Why the driver has to do it at all
///
/// Because `TaskState::name_text` and `RunRecord::task_text` are the words the
/// component MEASURES — it draws the key beside them. A driver that sent the
/// ids instead would hand the buffer a thirteen-cell name and then measure a
/// seventeen-cell translation against the column it reserved.
fn named(mind: &mut Mind, words: &std::collections::HashMap<String, String>) {
    let say = |key: &str| words.get(key).cloned().unwrap_or_else(|| key.to_owned());
    for t in &mut mind.tasks {
        t.name_text = say(&t.name_key);
        t.last.task_text = say(&t.last.task_key);
        for r in &mut t.history {
            r.task_text = say(&r.task_key);
        }
    }
    for row in &mut mind.activity_rows {
        row.newest.task_text = say(&row.newest.task_key);
        for r in &mut row.members {
            r.task_text = say(&r.task_key);
        }
    }
}

/// A run of `task`, with the keys the host would have sent beside it.
fn a_run(task: &str, outcome: OutcomeKind, at: &str, why: &str) -> RunRecord {
    RunRecord {
        task: task.to_owned(),
        task_text: task.to_owned(),
        began: NOW - 7200,
        ended: NOW - 7000,
        when_text: "2026-08-23 13:00".to_owned(),
        outcome,
        at: at.to_owned(),
        why: why.to_owned(),
        trouble: String::new(),
        steps: Vec::new(),
        task_key: format!("brain-task-{task}"),
        // THE KEY THE STEP'S MODULE DECLARED, found by the step's id — what
        // `mind_wire` does in production. Empty for a word that is no step.
        at_key: declared_steps()
            .iter()
            .find(|s| !at.is_empty() && s.id == at)
            .map(|s| s.name_key.to_owned())
            .unwrap_or_default(),
        why_key: if why.is_empty() {
            String::new()
        } else {
            format!("brain-why-{why}")
        },
        says: String::new(),
        says_text: String::new(),
    }
}

/// A task in whichever shape the screen is being asked about.
fn task(id: &str, standing: Standing, hold: HoldKind, why_not: &str, ran: bool) -> TaskState {
    // THE STEPS COME FROM A MODULE, never from here — see `declared_steps`.
    // Two are enough for what this driver proves: that the panel draws a step
    // that ran and one that is switched off, each by the keys its module
    // declared.
    let declared = declared_steps();
    let first = declared
        .first()
        .map(|s| s.id.to_owned())
        .unwrap_or_default();
    let step = |n: usize, root: bool, on: bool| {
        declared.get(n).map(|s| TaskStep {
            id: s.id.to_owned(),
            command: s.command.to_owned(),
            needs_root: root,
            on,
            on_by_default: true,
            name_key: s.name_key.to_owned(),
            does_key: s.does_key.to_owned(),
        })
    };
    TaskState {
        id: id.to_owned(),
        // THE NAME AS THE HOST WOULD HAVE RESOLVED IT — see `named`.
        name_text: id.to_owned(),
        kind: TaskKind::Routine,
        standing,
        enabled: true,
        interval_days: 7,
        part_of_day: DayPart::Night,
        next: NOW + 3600,
        // ALREADY FORMATTED BY THE HOST: the component has no timezone and no
        // calendar (invariant 11), so a date crosses as the text it will show.
        next_text: "tomorrow, 03:00".to_owned(),
        hold,
        why_not: why_not.to_owned(),
        ran,
        history: vec![
            a_run(id, OutcomeKind::Done, "", ""),
            a_run(id, OutcomeKind::Failed, &first, ""),
            a_run(id, OutcomeKind::Held, "", "ram"),
        ],
        last: RunRecord {
            steps: vec![StepOutcome {
                id: first.clone(),
                ok: true,
                seconds: 42,
                trouble: String::new(),
            }],
            ..a_run(id, OutcomeKind::Done, &first, "")
        },
        steps: [step(0, false, true), step(1, true, false)]
            .into_iter()
            .flatten()
            .collect(),
        calendar: true,
        name_key: format!("brain-task-{id}"),
        does_key: format!("brain-task-{id}-does"),
        why_key: if why_not.is_empty() {
            String::new()
        } else {
            format!("brain-why-{why_not}")
        },
        screen_generation: 1,
        // ONE THING ITS MODULE OFFERS ON ITS PAGE, by a label the texts have,
        // so the page's `Actions` block is drawn and measured.
        actions: vec![ModuleAction {
            id: "fixture".to_owned(),
            label_key: "cmd-brain-run".to_owned(),
            row: false,
            asks_key: String::new(),
        }],
    }
}

/// Every other task the brain reports, drawn plainly — and one of each KIND,
/// so the labels and colours of a one-shot and of a watch are measured too.
fn others(ids: &[String]) -> Vec<TaskState> {
    ids.iter()
        .skip(1)
        .enumerate()
        .map(|(n, id)| TaskState {
            kind: match n % 3 {
                0 => TaskKind::Watch,
                1 => TaskKind::Once,
                _ => TaskKind::Routine,
            },
            standing: Standing::Idle,
            ran: false,
            hold: HoldKind::None,
            history: Vec::new(),
            last: RunRecord::default(),
            steps: Vec::new(),
            calendar: n % 2 == 0,
            actions: Vec::new(),
            ..task(id, Standing::Idle, HoldKind::None, "", false)
        })
        .collect()
}

/// A notice of every level, the loudest offering all three controls — its
/// sentence is DATA the host made, and the labels of its controls are keys.
fn notices(ids: &[String]) -> Vec<Notice> {
    let module = ids.first().cloned().unwrap_or_default();
    [
        NoticeLevel::Alarm,
        NoticeLevel::Danger,
        NoticeLevel::Warn,
        NoticeLevel::Info,
    ]
    .into_iter()
    .enumerate()
    .map(|(n, level)| Notice {
        module: module.clone(),
        id: format!("n{n}"),
        says: String::new(),
        says_text: "A notice the module raised, long enough to be cut at a narrow width".to_owned(),
        args: Vec::new(),
        level,
        since: NOW - 60,
        since_text: "2026-08-23 12:59".to_owned(),
        controls: vec![ModuleAction {
            id: "fixture".to_owned(),
            label_key: "cmd-brain-run".to_owned(),
            row: false,
            asks_key: String::new(),
        }],
        dismissable: true,
        snooze: vec![10],
    })
    .collect()
}

/// The activity log, FOLDED: a row that stands alone and one that folds three,
/// with every outcome in it — `noted` included, the one added last and
/// therefore the one a `.ftl` would be missing.
fn activity(ids: &[String]) -> Vec<ActivityRow> {
    let id = |n: usize| ids[n % ids.len()].clone();
    let one = |r: RunRecord| ActivityRow {
        last_text: r.when_text.clone(),
        first_text: r.when_text.clone(),
        repeats: 1,
        members: vec![r.clone()],
        newest: r,
    };
    let held = a_run(&id(1), OutcomeKind::Held, "", "ram");
    vec![
        one(a_run(&id(0), OutcomeKind::Done, "", "")),
        ActivityRow {
            newest: held.clone(),
            repeats: 3,
            last_text: "2026-08-23 13:00".to_owned(),
            first_text: "2026-08-21 03:00".to_owned(),
            members: vec![held.clone(), held.clone(), held],
        },
        one(a_run(&id(2), OutcomeKind::Failed, "", "")),
        one(a_run(&id(3), OutcomeKind::Skipped, "", "off")),
        one(RunRecord {
            says: "brain-outcome-noted".to_owned(),
            says_text: "Something a module noticed, as its sentence".to_owned(),
            ..a_run(&id(4), OutcomeKind::Noted, "", "")
        }),
    ]
}

/// The mind, with the first task in the shape asked for.
fn mind(ids: &[String], one: TaskState, announcement: Announcement, said: bool) -> Mind {
    let running = if one.standing == Standing::Running {
        one.id.clone()
    } else {
        String::new()
    };
    let queue = if one.standing == Standing::Queued {
        vec![one.id.clone()]
    } else {
        Vec::new()
    };
    Mind {
        // The journal's own cap, so the activity block's title can say what
        // its window is onto.
        remembers: 100,
        recent: 10,
        now: NOW,
        pulse: Pulse {
            at: NOW,
            signs: vec![
                VitalSign {
                    of: Vital::RamUsed,
                    amount: 620,
                },
                VitalSign {
                    of: Vital::CpuStall,
                    amount: 40,
                },
                VitalSign {
                    of: Vital::OnBattery,
                    amount: 0,
                },
            ],
        },
        // THE ONE BEING ASKED ABOUT, AND THEN EVERY OTHER TASK THE BRAIN HAS —
        // here for their LABELS: a module whose two keys were forgotten draws
        // a bracketed key, and this is the check positioned to see it.
        tasks: std::iter::once(one).chain(others(ids)).collect(),
        queue,
        running,
        rehearsing: String::new(),
        announcement,
        gate: Gate {
            ram_ceiling_pct: 80,
            cpu_stall_per_mille: 200,
            abort_on_battery: true,
        },
        cadence: vec![1, 7, 30],
        activity_rows: activity(ids),
        notices: if said { notices(ids) } else { Vec::new() },
    }
}

/// A module's own page, of two sections, by keys the texts have — one row
/// that acts and one that tells, every kind of cell among them.
fn screen(module: &str) -> ModuleScreen {
    let row = |id: &str, label: &str, value: ScreenCell, tone: BrainTone, act: &str| ModuleRow {
        id: id.to_owned(),
        label: ScreenCell::Key(label.to_owned()),
        value,
        note: ScreenCell::Data("a note the module wrote".to_owned()),
        tone,
        act: act.to_owned(),
        also: String::new(),
    };
    ModuleScreen {
        module: module.to_owned(),
        generation: 1,
        sections: vec![
            ModuleSection {
                heading: "brain-history".to_owned(),
                rows: vec![
                    row(
                        "a",
                        "brain-last-run",
                        ScreenCell::Moment(ronoctua_protocol::Timed {
                            at: NOW,
                            text: "2026-08-23 13:00".to_owned(),
                        }),
                        BrainTone::Watching,
                        "fixture",
                    ),
                    row(
                        "b",
                        "brain-next",
                        ScreenCell::Count(3),
                        BrainTone::Danger,
                        "",
                    ),
                ],
            },
            ModuleSection {
                heading: "brain-steps".to_owned(),
                rows: vec![row(
                    "c",
                    "brain-enabled",
                    ScreenCell::Bytes(ronoctua_protocol::Measured {
                        bytes: 12_345_678,
                        text: "12 MB".to_owned(),
                    }),
                    BrainTone::Quiet,
                    "",
                )],
            },
        ],
        refused: String::new(),
    }
}

/// Whether a frame WRITES this piece of data, as a line or as a paragraph.
fn writes(list: &ronoctua_protocol::DisplayList, data: &str) -> bool {
    use ronoctua_protocol::{Command, Text};
    list.commands.iter().any(|c| match c {
        Command::Text {
            text: Text::Data(d),
            ..
        }
        | Command::Paragraph {
            content: Text::Data(d),
            ..
        } => d == data,
        _ => false,
    })
}

fn main() -> Result<()> {
    const USAGE: &str = "usage: ronoctua-test-brain <component.wasm> <i18n-dir> [--modules <what `ronoctua-brain modules` printed>]";
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().context(USAGE)?.clone();
    let i18n = args.get(1).context(USAGE)?.clone();
    let modules = match args.get(2).map(String::as_str) {
        Some("--modules") => Some(args.get(3).context(USAGE)?.clone()),
        Some(other) => bail!("unknown option `{other}`.\n{USAGE}"),
        None => None,
    };
    let ids = reported(&i18n, modules.as_deref())?;
    println!("modules   {} reported: {}", ids.len(), ids.join(", "));
    // THE STEP ROWS ARE MEASURED TOO, or this says so: a build whose modules
    // declare no step would draw a page with none, and every step label would
    // go unmeasured without a word.
    let steps = declared_steps();
    anyhow::ensure!(
        steps.len() >= 2,
        "the brain compiled no module with two steps or more; the step rows would go unmeasured"
    );
    println!(
        "steps     {} of the first module that has them",
        steps.len()
    );

    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;
    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;

    // ---- ONCE PER LANGUAGE, AND A FRESH COMPONENT EACH TIME ----
    //
    // The brain's header does not reserve blind: `Input::Phrases` hands it
    // the words and it MEASURES them, so a measured budget is one language's
    // width and each pass is measured against itself. The guarantee is the
    // same, asked once per language rather than once for all of them.
    let spoken = ronoctua_runtime::driven::languages(&i18n)?;
    for language in &spoken {
        ronoctua_runtime::driven::forget();
        drive(&rt, &component, &i18n, language, &ids)?;
    }
    println!("OK: the brain's panel takes from the host, and every label it draws fits");
    Ok(())
}

/// Walk every screen of the brain, in one language, and measure what it drew.
fn drive(
    rt: &ronoctua_runtime::Runtime,
    component: &wasmtime::component::Component,
    i18n: &str,
    language: &str,
    ids: &[String],
) -> Result<()> {
    let i18n = i18n.to_owned();
    // THE LANGUAGE'S OWN WORDS, for the names the host would have resolved.
    // The same table `hand_phrases` reads one line below; see `named`.
    let words = ronoctua_runtime::driven::ftl(&format!("{i18n}/{language}.ftl"))?;
    let mut inst = rt.open(component)?;
    let d = inst.init()?;
    ronoctua_runtime::driven::hand_phrases(&mut inst, &d, &i18n, language)?;

    // ---- THE DESCRIPTOR ----
    if d.mode != "brain" {
        bail!("the mode is `{}` and the window rules match on it", d.mode);
    }
    if !d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Brain)
    {
        bail!("the panel does not declare the capability that feeds it");
    }
    // EVERY LANGUAGE, and not just the base one: a command whose label was
    // never translated would show the raw key in which-key.
    for language in ronoctua_runtime::driven::languages(&i18n)? {
        let table = ronoctua_runtime::driven::ftl(&format!("{i18n}/{language}.ftl"))?;
        for id in &d.commands {
            let key = format!("cmd-{id}");
            if !table.contains_key(&key) {
                bail!("`{key}` does not exist in {language}.ftl");
            }
        }
    }
    println!(
        "descriptor: mode `{}`, {} commands with a label",
        d.mode,
        d.commands.len()
    );

    // ---- THE SCREEN IS EMPTY UNTIL THE HOST SPEAKS ----
    let quiet = look(&mut inst)?;
    if quiet.commands.is_empty() {
        bail!("with no mind, the panel drew nothing at all — not even the word for it");
    }

    // ---- EVERY SHAPE OF THE HOME PAGE ----
    //
    // Each draws a family of keys the others do not: the hold's reason, the
    // refusal's reason, the running task's standing, the warning in flight —
    // and the notices, each shape with and without them.
    let first = ids[0].clone();
    let shapes: [(&str, TaskState, Announcement, bool); 4] = [
        (
            "idle, with notices",
            task(&first, Standing::Idle, HoldKind::None, "", true),
            Announcement::default(),
            true,
        ),
        (
            "held by ram",
            task(&first, Standing::Queued, HoldKind::Ram, "", true),
            Announcement::default(),
            false,
        ),
        (
            "refused",
            task(&first, Standing::Idle, HoldKind::None, "off", false),
            Announcement::default(),
            true,
        ),
        (
            "running, with a warning",
            task(&first, Standing::Running, HoldKind::None, "", true),
            Announcement {
                task: first.clone(),
                total_seconds: 30,
                deadline: NOW + 30,
                why: "postponed".to_owned(),
            },
            true,
        ),
    ];
    for (what, one, announcement, said) in shapes {
        let mut m = mind(ids, one, announcement, said);
        named(&mut m, &words);
        inst.event(&Input::Mind(Box::new(m)))?;
        let list = look(&mut inst)?;
        if list.commands.is_empty() {
            bail!("the `{what}` mind drew an empty screen");
        }
        println!("home      {what}");
    }

    // ---- AND EVERY REASON A TASK CAN BE HELD FOR ----
    //
    // `every_label_fits` measures what was DRAWN, and one shape above draws one
    // of these; in pt-BR `brain-hold-cpu` is nineteen cells, and it was cut on
    // every machine that ran out of processor until each was drawn here.
    for hold in [
        HoldKind::Ram,
        HoldKind::Cpu,
        HoldKind::Battery,
        HoldKind::Busy,
        HoldKind::Network,
        HoldKind::Asleep,
    ] {
        let one = task(&first, Standing::Queued, hold, "", true);
        let mut m = mind(ids, one, Announcement::default(), false);
        named(&mut m, &words);
        inst.event(&Input::Mind(Box::new(m)))?;
        if look(&mut inst)?.commands.is_empty() {
            bail!("a mind held by {hold:?} drew an empty screen");
        }
    }
    println!("held      all six reasons a task waits, each measured in every language");

    // ---- AND THE LOG, WITH A FOLDED ROW OPENED ----
    //
    // The fold's word before the date and its members are drawn only on a
    // row that folds, and only open: the page OPENED on its notices (the
    // first mind had some), so `Tab` twice goes to the log past the tasks,
    // `Down` to the folded row, `RET` opens it.
    let mut m = mind(
        ids,
        task(&first, Standing::Idle, HoldKind::None, "", true),
        Announcement::default(),
        true,
    );
    named(&mut m, &words);
    inst.event(&Input::Mind(Box::new(m)))?;
    inst.event(&Input::Toggle)?;
    inst.event(&Input::Toggle)?;
    inst.event(&Input::Down)?;
    inst.event(&Input::Confirm)?;
    for _ in 0..4 {
        inst.event(&Input::Down)?;
        if look(&mut inst)?.commands.is_empty() {
            bail!("the log drew nothing with a folded row open");
        }
    }
    println!("log       a folded row opened, and every line of it explained");

    // ---- AND A TASK'S OWN PAGE, with its module's ----
    //
    // It is a state the buffer DECLARED, so it is reached the way the footer
    // reaches it, and it is where the schedule, the actions, the module's own
    // page, the steps and the history are drawn.
    const PAGE: &str = "task";
    if !d.states.iter().any(|s| s.id == PAGE) {
        bail!("the buffer no longer declares the `{PAGE}` state this page lives in");
    }
    inst.event(&Input::Cancel)?;
    inst.event(&Input::EnterState(PAGE.to_owned()))?;
    inst.event(&Input::ModuleScreen(Box::new(screen(&first))))?;
    let behind = look(&mut inst)?;
    if behind.commands.is_empty() {
        bail!("a task's own page drew nothing");
    }
    println!(
        "page      a task's own, with its schedule, its module's page, its steps and its history"
    );

    // AND THE CURSOR WALKS IT, which is what puts the footer on each kind of
    // row in turn: a schedule row, an action, a module's row, a step, and a
    // run of the history. Each hands the footer a different family of keys.
    for _ in 0..20 {
        inst.event(&Input::Down)?;
        let walked = look(&mut inst)?;
        if walked.commands.is_empty() {
            bail!("a task's page drew nothing with the cursor moved");
        }
    }
    println!("walked    the cursor over every kind of row, measuring each footer");

    // ---- AND A PAGE THE BRAIN WOULD NOT GIVE ----
    //
    // A newer generation that is a SENTENCE and no rows: the page says why,
    // and a refused page never looks like an empty one.
    const REFUSED: &str = "the brain would not show this page";
    let mut refused = screen(&first);
    refused.generation += 1;
    refused.sections.clear();
    refused.refused = REFUSED.to_owned();
    inst.event(&Input::ModuleScreen(Box::new(refused)))?;
    if !writes(&look(&mut inst)?, REFUSED) {
        bail!("a page the brain refused did not say why");
    }
    println!("refused   a page the brain would not give says why, in place of its rows");

    // ---- AND EVERY LABEL FITS THE BUDGET, IN THIS LANGUAGE ----
    let measured =
        ronoctua_runtime::driven::every_label_fits_in(&i18n, ON_PURPOSE, COLUMNS, language)?;
    println!("labels    {measured} measured in {language}, all inside their budget");
    Ok(())
}
