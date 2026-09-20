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
//! inside four `Table`s (`Column::fixed("label", 16, 2)`) and half the keys are
//! ASSEMBLED at run time — `brain-why-{}`, `brain-hold-{}`, `brain-standing-{}`,
//! `update-{}`. No parser of this file's source can pair a key with a budget.
//!
//! The frame can: `Command::Text` carries the key and its `max_width` side by
//! side, already assembled. So this driver walks the buffer's screens and hands
//! them to `driven::every_label_fits`, which is the only way a label of this
//! buffer has ever been measured against the language.
//!
//! Usage: `ronoctua-test-brain <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{
    Announcement, DayPart, Gate, HoldKind, Input, Mind, OutcomeKind, Pulse, RunRecord, Standing,
    StepOutcome, TaskState, TaskStep, Vital, VitalSign,
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

/// The task whose STEPS this driver draws.
///
/// From `assets/updates.nix`, through the brain's own table.
const TASK: &str = "system-update";

/// EVERY TASK THE BRAIN DECLARES, so every one of them is drawn.
///
/// # Why this list is here and not one name
///
/// Because it was one name, and that made the check blind to the thing it is
/// best placed to catch. The buffer builds a task's label as
/// `format!("brain-task-{}", task.id)`, which `buffer-keys-have-text` cannot
/// see — it reads `const` literals — and `i18n-complete` only compares the two
/// languages with each other. So a task added without its two keys would have
/// drawn `[brain-task-cache-sweep]` on screen with every check in the tree
/// green. Driving all of them is what closes that.
///
/// A LIST AND NOT A READ of `crates/ronoctua-brain/src/tasks/`: this binary
/// runs inside a check with no source tree, and the day a third task arrives
/// the line here is the same line the registry needs.
const TASKS: &[&str] = &[
    "system-update",
    "cache-sweep",
    "trash-expire",
    "emacs-daemon",
];

/// The update sequence's table, as the build generated it.
///
/// # Why this file is read and not summarised
///
/// Because a step's id is a TEXT KEY once it reaches the panel — the buffer
/// builds `update-<id>` from it — so a driver that invented ids would draw
/// bracketed keys, and one that copied the real ones would be a second list of
/// the sequence. `update-has-one-list` refuses the second, and it is right to:
/// this crate carried three step names of its own once, and they were disjoint
/// from the twelve the unit actually runs.
///
/// So the ids come from the same generated table `ronoctua-brain` includes.
const TABLE: &str = include_str!(env!("RONOCTUA_UPDATES_RS"));

/// The declared step ids, in the order the sequence declares them.
///
/// A hand parse and not a regex: the shape is `id: "…",` on its own line, which
/// is what generated the file and what `update-has-one-list` reads it with.
fn declared_steps() -> Vec<String> {
    let mut out = Vec::new();
    for line in TABLE.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("id: \"") else {
            continue;
        };
        let Some(id) = rest.split('"').next() else {
            continue;
        };
        if !id.is_empty() && !out.iter().any(|x: &String| x == id) {
            out.push(id.to_owned());
        }
    }
    out
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
        // was left" and the host elides by design. The question the label
        // measurement asks is whether the buffer RESERVED enough, and that is
        // only answerable where it got what it asked for.
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

/// Fill in the names the HOST would have resolved, in this language.
///
/// # Why the driver has to do it at all
///
/// Because `TaskState::name_text` and `RunRecord::task_text` are the words the
/// component MEASURES — it draws the key beside them — and the production host
/// resolves them in `mind_wire::task_of`. A driver that sent the ids instead
/// would hand the buffer a thirteen-cell name and then measure a seventeen-cell
/// translation against the column it reserved, which is exactly what this
/// check caught the first time it ran: four labels reported as cut, on a
/// screen that cuts none of them.
fn named(mind: &mut Mind, words: &std::collections::HashMap<String, String>) {
    let say = |id: &str| {
        words
            .get(&format!("brain-task-{id}"))
            .cloned()
            .unwrap_or_else(|| id.to_owned())
    };
    for t in &mut mind.tasks {
        t.name_text = say(&t.id);
        t.last.task_text = say(&t.last.task);
        for r in &mut t.history {
            r.task_text = say(&r.task);
        }
    }
    for r in &mut mind.activity {
        r.task_text = say(&r.task);
    }
}

/// A task in whichever shape the screen is being asked about.
fn task(standing: Standing, hold: HoldKind, why_not: &str, ran: bool) -> TaskState {
    // THE IDS COME FROM THE DECLARATION, never from here — see `TABLE`. Two
    // are enough for what this driver proves: that the panel draws a step that
    // ran and one that is switched off.
    let declared = declared_steps();
    TaskState {
        id: TASK.to_owned(),
        // THE NAME AS THE HOST WOULD HAVE RESOLVED IT. This driver has no
        // bundle, and the point of the field is the WIDTH — so it carries a
        // word of the right shape rather than an empty string, which would
        // photograph a column the real screen never has.
        name_text: "Rebuild the system".to_owned(),
        // THE KIND, and the driver pushes BOTH: a one-shot row is drawn in
        // different colours and with a different glyph, and a driver that only
        // ever built routines would never measure the other half.
        kind: ronoctua_protocol::TaskKind::Routine,
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
        history: Vec::new(),
        last: RunRecord {
            task: TASK.to_owned(),
            task_text: "Rebuild the system".to_owned(),
            began: NOW - 7200,
            ended: NOW - 7000,
            when_text: "yesterday, 03:00".to_owned(),
            outcome: OutcomeKind::Done,
            at: declared.first().cloned().unwrap_or_default(),
            why: String::new(),
            trouble: String::new(),
            steps: vec![StepOutcome {
                id: declared.first().cloned().unwrap_or_default(),
                ok: true,
                seconds: 42,
                trouble: String::new(),
            }],
        },
        steps: vec![
            TaskStep {
                id: declared.first().cloned().unwrap_or_default(),
                command: "nix flake update".to_owned(),
                needs_root: false,
                on: true,
                on_by_default: true,
            },
            TaskStep {
                id: declared.get(1).cloned().unwrap_or_default(),
                command: "nix store optimise".to_owned(),
                needs_root: true,
                on: false,
                on_by_default: true,
            },
        ],
    }
}

/// The mind, with one task in the shape asked for.
fn mind(one: TaskState, announcement: Announcement) -> Mind {
    let running = if one.standing == Standing::Running {
        TASK.to_owned()
    } else {
        String::new()
    };
    let queue = if one.standing == Standing::Queued {
        vec![TASK.to_owned()]
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
        // THE ONE BEING ASKED ABOUT, AND THEN EVERY OTHER TASK THE BRAIN HAS.
        //
        // The extra rows are drawn plainly — nothing running, nothing held —
        // and they are here for one reason: their LABELS. A task whose two text
        // keys were forgotten draws a bracketed key, and this is the only check
        // in the tree positioned to see that, because the buffer assembles the
        // key with `format!` and no scan of `const` literals can find it.
        tasks: std::iter::once(one)
            .chain(TASKS.iter().filter(|id| **id != TASK).map(|id| TaskState {
                id: (*id).to_owned(),
                name_text: (*id).to_owned(),
                // ONE OF THEM IS A ONE-SHOT, so the labels and the colours of
                // that row are measured too.
                kind: if *id == TASKS[TASKS.len() - 1] {
                    ronoctua_protocol::TaskKind::Once
                } else {
                    ronoctua_protocol::TaskKind::Routine
                },
                standing: Standing::Idle,
                enabled: true,
                interval_days: 5,
                part_of_day: DayPart::Night,
                next: NOW + 3600,
                next_text: "tomorrow, 03:00".to_owned(),
                hold: HoldKind::None,
                why_not: String::new(),
                ran: false,
                history: Vec::new(),
                last: RunRecord::default(),
                steps: Vec::new(),
            }))
            .collect(),
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
        // NOTHING WRONG IN THE DRIVER'S OWN MIND. The notice's own shapes are
        // driven by `every_label_fits` through the `.ftl`, and a permanent one
        // here would put a danger line over every screen it photographs.
        notice: ronoctua_protocol::Notice::default(),
        // AND THE LOG, with every outcome in it — `held` included, which is
        // the one added last and therefore the one a `.ftl` would be missing.
        activity: [
            ronoctua_protocol::OutcomeKind::Done,
            ronoctua_protocol::OutcomeKind::Failed,
            ronoctua_protocol::OutcomeKind::Skipped,
            ronoctua_protocol::OutcomeKind::Held,
        ]
        .into_iter()
        .enumerate()
        .map(|(n, outcome)| RunRecord {
            task: TASKS[n % TASKS.len()].to_owned(),
            task_text: TASKS[n % TASKS.len()].to_owned(),
            began: NOW - 3600 * (n as u64 + 1),
            ended: NOW - 3600 * (n as u64 + 1) + 60,
            when_text: "2026-08-23 13:00".to_owned(),
            outcome,
            at: String::new(),
            why: if outcome == ronoctua_protocol::OutcomeKind::Held {
                "ram".to_owned()
            } else {
                String::new()
            },
            trouble: String::new(),
            steps: Vec::new(),
        })
        .collect(),
    }
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-test-brain <component.wasm> <i18n-dir>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("usage: ronoctua-test-brain <component.wasm> <i18n-dir>")?;

    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;
    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;

    // ---- ONCE PER LANGUAGE, AND A FRESH COMPONENT EACH TIME ----
    //
    // This used to hand the buffer English and then measure every language
    // against what it reserved. That was right for as long as a buffer could
    // only reserve BLIND — it emits a key, the host translates, and a fixed
    // budget is all anybody has to go on.
    //
    // The brain's header no longer reserves blind: `Input::Phrases` hands it
    // the words and it MEASURES them, which is what let its shortcuts be
    // packed as tightly as the minibuffer's instead of each being charged a
    // thirteen-cell column. A measured budget is one language's width, so
    // there is no single number that could satisfy every language — and there
    // does not need to be. The budget IS the text.
    //
    // So the drive happens once for each language, with that language's words,
    // and each pass is measured against itself. The guarantee is not weaker:
    // it is the same guarantee, asked once per language rather than once for
    // all of them.
    let spoken = ronoctua_runtime::driven::languages(&i18n)?;
    for language in &spoken {
        ronoctua_runtime::driven::forget();
        drive(&rt, &component, &i18n, language)?;
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
    // EVERY LANGUAGE, and not just the base one. Checking en-GB alone would
    // pass a command whose label was never translated, and which-key would show
    // the raw key to whoever reads the other one.
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
    //
    // The same criterion the memory panel has, and for the same reason: this
    // buffer owns no database. What it draws before the first `mind` must not
    // be an invented zero.
    let quiet = look(&mut inst)?;
    if quiet.commands.is_empty() {
        bail!("with no mind, the panel drew nothing at all — not even the word for it");
    }

    // ---- EVERY SHAPE OF THE HOME PAGE ----
    //
    // Four minds, because four different key families are only drawn in one of
    // them each: the hold's reason, the refusal's reason, the running task's
    // standing, and the warning in flight. A label nobody drew is a label
    // nobody measured.
    let shapes: [(&str, TaskState, Announcement); 4] = [
        (
            "idle",
            task(Standing::Idle, HoldKind::None, "", true),
            Announcement::default(),
        ),
        (
            "held by ram",
            task(Standing::Queued, HoldKind::Ram, "", true),
            Announcement::default(),
        ),
        (
            "refused",
            task(Standing::Idle, HoldKind::None, "off", false),
            Announcement::default(),
        ),
        (
            "running, with a warning",
            task(Standing::Running, HoldKind::None, "", true),
            Announcement {
                task: TASK.to_owned(),
                total_seconds: 30,
                deadline: NOW + 30,
                why: "postponed".to_owned(),
            },
        ),
    ];
    for (what, one, announcement) in shapes {
        let mut m = mind(one, announcement);
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
    // Because `every_label_fits` measures what was DRAWN, and one shape above
    // draws one of these. With only `Ram` among them the other five crossed
    // untouched, and the widest of the six was one of those five: in pt-BR
    // `brain-hold-cpu` is `Processador ocupado`, nineteen cells, against a
    // column that reserved twelve. It was cut on every machine that ever ran
    // out of processor, and the check that exists to catch exactly that had
    // never looked at it. One frame each is what makes the budget a measured
    // number rather than one somebody checked by hand once.
    for hold in [
        HoldKind::Ram,
        HoldKind::Cpu,
        HoldKind::Battery,
        HoldKind::Busy,
        HoldKind::Network,
        HoldKind::Asleep,
    ] {
        let one = task(Standing::Queued, hold, "", true);
        let mut m = mind(one, Announcement::default());
        named(&mut m, &words);
        inst.event(&Input::Mind(Box::new(m)))?;
        if look(&mut inst)?.commands.is_empty() {
            bail!("a mind held by {hold:?} drew an empty screen");
        }
    }
    println!("held      all six reasons a task waits, each measured in every language");

    // ---- AND A TASK'S OWN PAGE ----
    //
    // It is a state the buffer DECLARED, so it is reached the way the footer
    // reaches it, and it is where the schedule, the steps and the history are
    // drawn — key families the home page does not show.
    //
    // `task` and not `update`: the page belongs to whichever task is under the
    // cursor, and it always did — the state was called `update` back when the
    // update was the only task with a page. Four tasks later the name was
    // telling readers something false.
    const PAGE: &str = "task";
    if !d.states.iter().any(|s| s.id == PAGE) {
        bail!("the buffer no longer declares the `{PAGE}` state this page lives in");
    }
    inst.event(&Input::EnterState(PAGE.to_owned()))?;
    let behind = look(&mut inst)?;
    if behind.commands.is_empty() {
        bail!("a task's own page drew nothing");
    }
    println!("page      a task's own, with its schedule, its steps and its history");

    // AND THE CURSOR WALKS IT, which is what puts the footer on each kind of
    // row in turn: a schedule row, a step, and a run of the history. Each
    // hands the footer a different family of keys, and a page drawn only at
    // rest would measure none of them.
    for _ in 0..12 {
        inst.event(&Input::Down)?;
        let walked = look(&mut inst)?;
        if walked.commands.is_empty() {
            bail!("a task's page drew nothing with the cursor moved");
        }
    }
    println!("walked    the cursor over every kind of row, measuring each footer");

    // ---- AND EVERY LABEL FITS THE BUDGET, IN THIS LANGUAGE ----
    //
    // The point of this whole binary. What a buffer declares is what it
    // RESERVED — a number inside a `Table`, with the key assembled at run
    // time, so neither is readable from the source and both are in the frame.
    let measured =
        ronoctua_runtime::driven::every_label_fits_in(&i18n, ON_PURPOSE, COLUMNS, language)?;
    println!("labels    {measured} measured in {language}, all inside their budget");
    Ok(())
}
