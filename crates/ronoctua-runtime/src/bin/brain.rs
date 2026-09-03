//! Exercises the brain's panel — `LEADER s b`.
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

/// The one task this system declares today.
///
/// From `assets/updates.nix`, through the brain's own table. If it ever grows a
/// second, this driver keeps working — the mind is a list — and the check gains
/// its labels for free.
const TASK: &str = "system-update";

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

/// A task in whichever shape the screen is being asked about.
fn task(standing: Standing, hold: HoldKind, why_not: &str, ran: bool) -> TaskState {
    TaskState {
        id: TASK.to_owned(),
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
        last: RunRecord {
            began: NOW - 7200,
            ended: NOW - 7000,
            when_text: "yesterday, 03:00".to_owned(),
            outcome: OutcomeKind::Done,
            at: "flake-update".to_owned(),
            why: String::new(),
            trouble: String::new(),
            steps: vec![StepOutcome {
                id: "flake-update".to_owned(),
                ok: true,
                seconds: 42,
                trouble: String::new(),
            }],
        },
        steps: vec![
            TaskStep {
                id: "flake-update".to_owned(),
                command: "nix flake update".to_owned(),
                needs_root: false,
                on: true,
                changed: false,
            },
            TaskStep {
                id: "store-optimise".to_owned(),
                command: "nix store optimise".to_owned(),
                needs_root: true,
                on: false,
                changed: true,
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
        tasks: vec![one],
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
    let mut inst = rt.open(&component)?;
    let d = inst.init()?;
    ronoctua_runtime::driven::hand_phrases(&mut inst, &d, &i18n, "en-GB")?;

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
    let table = ronoctua_runtime::driven::ftl(&format!("{i18n}/en-GB.ftl"))?;
    for id in &d.commands {
        let key = format!("cmd-{id}");
        if !table.contains_key(&key) {
            bail!("`{key}` does not exist in en-GB.ftl");
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
        inst.event(&Input::Mind(mind(one, announcement)))?;
        let list = look(&mut inst)?;
        if list.commands.is_empty() {
            bail!("the `{what}` mind drew an empty screen");
        }
        println!("home      {what}");
    }

    // ---- AND THE UPDATE'S OWN PAGE ----
    //
    // It is a state the buffer DECLARED, so it is reached the way the footer
    // reaches it, and it is where the steps table and the run record are drawn
    // — three more key families, none of which the home page shows.
    let has_update = d.states.iter().any(|s| s.id == "update");
    if !has_update {
        bail!("the buffer no longer declares the `update` state this page lives in");
    }
    inst.event(&Input::EnterState("update".to_owned()))?;
    let behind = look(&mut inst)?;
    if behind.commands.is_empty() {
        bail!("the update's page drew nothing");
    }
    println!("page      the update's own, with its steps and its last run");

    // ---- AND EVERY LABEL FITS THE BUDGET, IN EVERY LANGUAGE ----
    //
    // The point of this whole binary. A buffer cannot measure a sentence it
    // does not have: it emits a KEY and the host translates (invariant 11), so
    // what it declares is what it RESERVED. Here the reservation is a number
    // inside a `Table` and the key is assembled at run time — neither is
    // readable from the source, and both are in the frame.
    let measured = ronoctua_runtime::driven::every_label_fits(
        &i18n,
        ronoctua_runtime::driven::LANGUAGES,
        ON_PURPOSE,
        COLUMNS,
    )?;
    println!(
        "labels    {measured} measured in {} languages, all inside their budget",
        ronoctua_runtime::driven::LANGUAGES.len()
    );

    println!("OK: the brain's panel takes from the host, and every label it draws fits");
    Ok(())
}
