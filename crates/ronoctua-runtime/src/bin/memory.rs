//! Exercises the memory panel — `LEADER s m`.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its
//! own (invariant 12): a `cargo test` of it tests the logic OUTSIDE the
//! component, which is precisely what proves nothing. Here it is loaded the way
//! the host loads it, pushed a survey the way the host pushes one, and read
//! through `render`.
//!
//! # What only this can prove
//!
//! The three other drivers exercise buffers that carry their own database. This
//! one is the first whose screen is EMPTY until the host speaks: the survey
//! crosses the WIT as a record with lists inside, lowered field by field, and
//! `wasmtime` compares field names only AFTER zipping them positionally. A
//! field out of order fails here, with a message, instead of on somebody's
//! screen.
//!
//! Usage: `ronoctua-test-memory <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{
    Command, Consumer, ConsumerKind, Demand, Effect, Gauge, Guard, GuardMode, Input, Pressure,
    Reading, Request, Survey, Text,
};

/// The scale heights the production host sends.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 40;
const COLUMNS: u16 = 100;

/// A machine with 16 GiB, half of it free, under mild stall.
///
/// The numbers are round on purpose: what the screen prints from them is
/// checked literally below, and a number one has to compute in one's head is a
/// number nobody rechecks when the test fails.
fn survey() -> Survey {
    Survey {
        readings: vec![
            reading(Gauge::RamTotal, 16 * GIB),
            reading(Gauge::RamAvailable, 8 * GIB),
            reading(Gauge::RamAnonymous, 4 * GIB),
            reading(Gauge::RamCached, 3 * GIB),
            reading(Gauge::ZramOriginal, 2 * GIB),
            reading(Gauge::ZramCompressed, GIB),
            reading(Gauge::DiskTotal, 400 * GIB),
            reading(Gauge::DiskFree, 120 * GIB),
            reading(Gauge::VramTotal, GIB),
            reading(Gauge::VramUsed, GIB / 2),
        ],
        pressure: Pressure {
            some10: 15,
            some60: 3,
            ..Default::default()
        },
        consumers: vec![
            Consumer {
                kind: ConsumerKind::Scope,
                name: "ronoctua-9f3a.scope".to_owned(),
                bytes: 2 * GIB,
                hint: 0,
            },
            Consumer {
                kind: ConsumerKind::Process,
                name: "compilador".to_owned(),
                bytes: GIB,
                hint: 4321,
            },
        ],
        guard: Guard {
            mode: GuardMode::Strict,
            pressure_limit: 500,
            pressure_seconds: 15,
            ceiling: 700,
            acted: 2,
            zram: true,
            persisted: GuardMode::Strict,
        },
        scanning: false,
        progress: 0,
    }
}

const GIB: u64 = 1024 * 1024 * 1024;

const fn reading(of: Gauge, bytes: u64) -> Reading {
    Reading { of, bytes }
}

/// Everything the component drew, in one string per kind.
struct Screen {
    /// The literal data — numbers, names, paths.
    data: Vec<String>,
    /// The keys, which the host would translate.
    keys: Vec<String>,
    /// How many lines were highlighted (the cursor, a pending confirmation).
    selections: usize,
}

impl Screen {
    fn of(list: &ronoctua_protocol::DisplayList) -> Self {
        let mut me = Self {
            data: Vec::new(),
            keys: Vec::new(),
            selections: 0,
        };
        for c in &list.commands {
            match c {
                Command::Text { text, .. } => match text {
                    Text::Data(t) => me.data.push(t.clone()),
                    Text::Key(k) => me.keys.push(k.clone()),
                    Text::Icon(_) => {}
                },
                Command::Selection { .. } => me.selections += 1,
                _ => {}
            }
        }
        me
    }

    fn shows(&self, what: &str) -> bool {
        self.data.iter().any(|d| d.contains(what))
    }

    fn names(&self, key: &str) -> bool {
        self.keys.iter().any(|k| k == key)
    }
}

/// Draws, reads the screen, and refuses a frame where anything is drawn on top
/// of anything.
///
/// # Why the geometry rides along with every reading
///
/// Because this driver is the only one that fills this buffer with data, and a
/// collision needs the data: an empty list collides with nothing. The two
/// questions live in `ronoctua-protocol`, next to the display list they read —
/// sideways, whether a field's budget reaches into the next one; downwards,
/// whether a run's scale grew it into the cell below, which is what put a rule
/// through the title's descenders.
///
/// AT THREE WIDTHS, because a collision appears at the width where two fields
/// happen to meet, and that width depends on how long a sentence came out in
/// whichever language is loaded.
fn look(inst: &mut ronoctua_runtime::Instance) -> Result<Screen> {
    let list = inst.render(COLUMNS, ROWS, heights())?;
    for columns in [46, 80, COLUMNS] {
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
    Ok(Screen::of(&list))
}

/// Fires a command by id, the way the buffer's host does when a key completes.
fn run(inst: &mut ronoctua_runtime::Instance, id: &str) -> Result<Vec<Effect>> {
    inst.event(&Input::Command(id.to_owned()))
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-test-memory <component.wasm> <i18n-dir>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("usage: ronoctua-test-memory <component.wasm> <i18n-dir>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // ---- THE DESCRIPTOR ----
    let d = inst.init()?;
    if d.mode != "memory" {
        bail!("the buffer's mode is `{}`; the modeline shows that", d.mode);
    }
    // WITHOUT THE CAPABILITY THE SCREEN IS EMPTY, and the host would never
    // sample. It is the one line of the descriptor this buffer cannot lose.
    if !d
        .capabilities
        .contains(&ronoctua_protocol::Capability::System)
    {
        bail!("the panel does not declare `system`: the host would never sample");
    }
    for id in &d.commands {
        if !d.keymap.iter().any(|l| l.command == *id) {
            bail!("`{id}` has no key: nobody reaches it through which-key");
        }
    }
    if let Err(e) = d.validate() {
        bail!("the descriptor does not pass the section 7.9 guard: {e}");
    }
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for id in &d.commands {
            let key = format!("cmd-{id}");
            if !table.contains(&key) {
                bail!("`{key}` is missing from {language}.ftl — SPC m would show [{key}]");
            }
        }
    }
    println!(
        "descriptor: mode `{}`, {} commands with a key",
        d.mode,
        d.commands.len()
    );

    // ---- BEFORE THE SURVEY: no numbers ----
    let empty = look(&mut inst)?;
    if empty.shows("GiB") {
        bail!("the panel drew a size before taking any sample at all");
    }

    // ---- THE SURVEY CROSSES ----
    //
    // AND THIS IS THE ONLY PLACE IT IS LOWERED FOR REAL. `wasmtime` lowers a
    // record by zipping it POSITIONALLY against the WIT and checks the names
    // afterwards, so a field out of order fails when the guest is CALLED — an
    // error at run time and nothing at all at compile time. `ronoctua-runtime`'s
    // unit tests compare `val_of_survey` against a list written by hand; only
    // here is it compared against the WIT itself, through a component that was
    // built from it.
    //
    // It used to be a `#[test]` over there as well, reading the component from
    // `option_env!("RONOCTUA_BUFFER_MEMORY")`. That variable is what made every
    // buffer edit recompile the workspace, and the test was a copy of these
    // three lines — so it left, and the reason it existed lives here.
    let effects = inst.event(&Input::Survey(survey()))?;
    if effects != vec![Effect::Redraw] {
        bail!("the first sample had to ask for a frame; got {effects:?}");
    }
    // The SAME survey again asks for nothing: a screen that repaints every
    // second to show identical numbers spends a frame per second for nothing.
    if !inst.event(&Input::Survey(survey()))?.is_empty() {
        bail!("the same sample asked for a frame again");
    }

    let overview = look(&mut inst)?;
    // NO KEY DRAWN AS DATA, on any screen. A key in the wrong slot is not a
    // missing key: it is there, spelled right, and the screen prints its NAME —
    // `memory-guard-unknown` in front of the person. It happened, and the OCR of
    // `memory-vm` is what found it; this line is the cheap version of that
    // discovery, and it runs on every screen below.
    let leaked = |s: &Screen, where_at: &str| -> Result<()> {
        if let Some(d) = s.data.iter().find(|d| d.starts_with("memory-")) {
            bail!(
                "`{d}` was drawn as DATUM at {where_at}: it is a key, and the screen shows its name"
            );
        }
        Ok(())
    };
    leaked(&overview, "the overview")?;
    // 8 GiB available of 16, and the free-form numbers the panel computes.
    for expected in ["8.0 GiB", "16.0 GiB", "1.5%"] {
        if !overview.shows(expected) {
            bail!(
                "the overview does not show `{expected}`:\n{:?}",
                overview.data
            );
        }
    }
    // THE COMPRESSION RATIO IS MEASURED, not the module's nominal 50%: 2 GiB
    // written into 1 GiB is 2.0x, and the pair is what says so.
    if !overview.shows("2.0x") {
        bail!(
            "the zram's ratio did not come out measured:\n{:?}",
            overview.data
        );
    }
    // And the consumers are on the small screen.
    if !overview.shows("ronoctua-9f3a.scope") {
        bail!("the biggest consumer does not appear in the overview");
    }
    println!("overview: RAM, measured zram, pressure and consumers");

    // ---- THE FULL SCREEN, AND THE TABS ----
    run(&mut inst, "memory-full")?;
    let full = look(&mut inst)?;
    if !full.names("memory-anonymous") || !full.shows("4.0 GiB") {
        bail!(
            "the full screen does not break down the RAM:\n{:?}",
            full.keys
        );
    }
    // The graphics memory belongs on a screen about memory: on an APU it comes
    // out of the same RAM.
    if !full.names("memory-vram") {
        bail!("the VRAM does not appear in the detail");
    }
    leaked(&full, "the full screen")?;
    run(&mut inst, "memory-tab")?;
    let disk = look(&mut inst)?;
    leaked(&disk, "the disk tab")?;
    if !disk.shows("120.0 GiB") {
        bail!("the disk tab does not show what is free:\n{:?}", disk.data);
    }
    println!("full screen: RAM detail, VRAM and the disk tab");

    // ---- THE PROTECTION ----
    run(&mut inst, "memory-guard")?;
    let guard = look(&mut inst)?;
    // The description the owner asked for: brief, and present.
    for key in [
        "memory-guard-description",
        "memory-guard-limit",
        "memory-guard-acted",
        "memory-guard-cost",
    ] {
        if !guard.names(key) {
            bail!(
                "the protection screen does not carry `{key}`:\n{:?}",
                guard.keys
            );
        }
    }
    leaked(&guard, "the protection screen")?;
    // 500 per mille is 50,0% — the number the module configures.
    if !guard.shows("50.0%") {
        bail!("the limit in force does not appear:\n{:?}", guard.data);
    }
    println!("protection: description, numbers in force and the price of the lenient mode");

    // ---- WHAT IS IRREVERSIBLE TAKES TWO PRESSES ----
    let first = run(&mut inst, "memory-sweep-store")?;
    if first.iter().any(|e| matches!(e, Effect::Ask(_))) {
        bail!("a single key already asked for the sweep: the confirmation does not exist");
    }
    let armed = look(&mut inst)?;
    if !armed.names("memory-confirm-sweep") || armed.selections == 0 {
        bail!("the confirmation does not appear on screen");
    }
    // A DIFFERENT KEY DISARMS IT — nothing irreversible stays armed while the
    // person is doing something else.
    run(&mut inst, "memory-refresh")?;
    let disarmed = look(&mut inst)?;
    if disarmed.names("memory-confirm-sweep") {
        bail!("the confirmation stayed armed after another key");
    }
    // The same key twice fires.
    run(&mut inst, "memory-sweep-store")?;
    let fired = run(&mut inst, "memory-sweep-store")?;
    if !fired.contains(&Effect::Ask(Demand {
        what: Request::SweepStore,
        target: 0,
    })) {
        bail!("two identical keys did not fire the sweep: {fired:?}");
    }
    println!("sweep: one key arms, another disarms, the same one fires");

    // ---- CLOSING NAMES THE ROW, NOT THE PID ----
    run(&mut inst, "memory-full")?;
    inst.event(&Input::Down)?;
    run(&mut inst, "memory-stop")?;
    let stop = run(&mut inst, "memory-stop")?;
    // The SECOND row, 1-based. A pid would not name a scope, and a scope — one
    // per application — is what one usually wants to close.
    if !stop.contains(&Effect::Ask(Demand {
        what: Request::StopConsumer,
        target: 2,
    })) {
        bail!("stopping did not point at the focused row: {stop:?}");
    }
    println!("stop: points at the ROW of the last sample, and not at the pid");

    println!("OK: the memory panel answers the host, and what is irreversible asks twice");
    Ok(())
}

/// The keys of an `.ftl`, as one string.
fn ftl(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("could not read {path}"))
}
