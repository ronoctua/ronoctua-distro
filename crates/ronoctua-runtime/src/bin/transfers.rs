//! Exercises the transfer queue in a window — `LEADER t w`.
//!
//! # Why a binary, and not a `#[test]` in the buffer
//!
//! The component compiles to `wasm32-wasip2` and lives in a workspace of its
//! own (invariant 12): a `cargo test` of it tests the logic OUTSIDE the
//! component, which is precisely what proves nothing. Here it is loaded the way
//! the host loads it, pushed a queue the way the host pushes one, and read
//! through `render`.
//!
//! # What only this can prove
//!
//! The memory panel proved that a record with lists inside crosses the WIT
//! field by field. This one proves the thing that record could not: that a
//! demand comes back naming an **id** and not a row, **after the list has
//! reordered underneath the cursor**.
//!
//! That is not a hypothetical. The queue sorts what is afoot above what has
//! finished, so a job ending moves every row below it. A cursor that were an
//! index would, between the frame that drew it and the key that acted on it,
//! come to mean a different job — and the key in question deletes the piece
//! already written at the destination. The reorder is simulated here, in the
//! one place that can see both sides of the WIT at once.
//!
//! Usage: `ronoctua-test-transfers <component.wasm> <i18n-dir>`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{
    Amount, Command, Demand, Effect, Engine, Errand, Input, Job, Queue, Request, Stage, Text,
};

/// The scale heights the production host sends.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 30;
const COLUMNS: u16 = 110;
const GIB: u64 = 1024 * 1024 * 1024;

/// One row, with everything else left at a value the screen can be checked for.
fn job(id: u64, engine: Engine, errand: Errand, stage: Stage, done: u64, total: u64) -> Job {
    Job {
        id,
        engine,
        errand,
        stage,
        amount: Amount::Bytes,
        done,
        total,
        subject: format!("file-{id}.bin"),
        trouble: String::new(),
        answer: 0,
        // The one row that went out as the person, so the detail screen has
        // both words to say and the row has both colours to draw.
        identified: id == 2,
    }
}

/// Three jobs, one per engine that can be afoot, with round numbers.
///
/// The numbers are round on purpose: what the screen prints from them is
/// checked literally below, and a number one has to compute in one's head is a
/// number nobody rechecks when the test fails.
fn queue() -> Queue {
    Queue {
        jobs: vec![
            job(1, Engine::Local, Errand::Copy, Stage::Running, GIB, 4 * GIB),
            job(
                2,
                Engine::Torrent,
                Errand::Torrent,
                Stage::Paused,
                GIB / 2,
                2 * GIB,
            ),
            job(3, Engine::Media, Errand::Media, Stage::Done, GIB, GIB),
        ],
    }
}

/// Everything the component drew, in one string per kind.
struct Screen {
    data: Vec<String>,
    keys: Vec<String>,
    selections: usize,
    icons: usize,
}

impl Screen {
    fn of(list: &ronoctua_protocol::DisplayList) -> Self {
        let mut me = Self {
            data: Vec::new(),
            keys: Vec::new(),
            selections: 0,
            icons: 0,
        };
        for c in &list.commands {
            match c {
                Command::Text { text, .. } => match text {
                    Text::Data(t) => me.data.push(t.clone()),
                    Text::Key(k) => me.keys.push(k.clone()),
                    Text::Icon(_) => me.icons += 1,
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
        .context("usage: ronoctua-test-transfers <component.wasm> <i18n-dir>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("usage: ronoctua-test-transfers <component.wasm> <i18n-dir>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("could not read {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // ---- THE DESCRIPTOR ----
    let d = inst.init()?;

    // THE PHRASES, exactly as the host hands them over at load. Without this a
    // buffer that holds interface text would be driven against its fallback,
    // and the screen this test reads would not be the screen anybody sees.
    // en-GB is the canonical one; the pt-BR round trip is checked separately.
    ronoctua_runtime::driven::hand_phrases(&mut inst, &d, &i18n, "en-GB")?;
    if d.mode != "transfers" {
        bail!("the buffer's mode is `{}`; the modeline shows that", d.mode);
    }
    if !d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Transfers)
    {
        bail!(
            "the window does not declare `transfers`: the host would never subscribe it to the queue"
        );
    }
    // AND IT DOES NOT DECLARE `system`. Two capabilities in one buffer would
    // give a queue viewer the right to close processes and sweep the store —
    // which is exactly why they were kept apart.
    if d.capabilities
        .contains(&ronoctua_protocol::Capability::System)
    {
        bail!("the window asks for `system`, which it does not use: that would be powers for free");
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
        "descriptor: mode `{}`, {} commands with a key, and only the capability it uses",
        d.mode,
        d.commands.len()
    );

    // ---- BEFORE THE QUEUE: no numbers ----
    let empty = look(&mut inst)?;
    if empty.shows("GiB") {
        bail!("the window drew a size before taking any queue at all");
    }
    if !empty.names("transfers-window-empty") {
        bail!("the empty window does not say it is empty");
    }

    // ---- THE QUEUE CROSSES, RECORD BY RECORD ----
    inst.event(&Input::Queue(queue()))?;
    let full = look(&mut inst)?;
    // Every enum crossed by NAME. If any of the five had been zipped into the
    // wrong field, the words below would be the wrong words — and this is the
    // only place that can tell.
    for k in [
        "transfers-work-copy",
        "transfers-work-torrent",
        "transfers-work-media",
    ] {
        if !full.names(k) {
            bail!("the queue did not show `{k}`: a field of the record crossed over swapped");
        }
    }
    if !full.shows("1.0 GiB") || !full.shows("4.0 GiB") {
        bail!("the first row's numbers did not show up: {:?}", full.data);
    }
    // The bar is drawn with glyphs from tokens, one command per cell, plus the
    // stage and engine drawings on every row.
    if full.icons < 12 {
        bail!("the bar was not drawn: {} icons", full.icons);
    }
    println!(
        "queue: three rows, five enums by name, {} token icons",
        full.icons
    );

    // ---- THE CURSOR IS AN ID, AND THE REORDER PROVES IT ----
    //
    // Move to the second row (job 2), then hand the buffer the same queue with
    // the rows in a different order — which is what happens the moment a job
    // finishes and sinks below the ones still afoot.
    inst.event(&Input::Down)?;
    let mut shuffled = queue();
    shuffled.jobs.reverse();
    inst.event(&Input::Queue(shuffled))?;

    // Giving up asks first: the first press must NOT send anything.
    let armed = run(&mut inst, "transfers-give-up")?;
    if armed.iter().any(|e| matches!(e, Effect::Ask(_))) {
        bail!("giving up fired without asking: {armed:?}");
    }
    let asking = look(&mut inst)?;
    if !asking.names("transfers-window-confirm-cancel") {
        bail!("the question did not appear on screen");
    }

    // And the second press names the ID it was on — 2 — and not the row it now
    // occupies, which after the reversal is a different one.
    let fired = inst.event(&Input::Confirm)?;
    if !fired.contains(&Effect::Ask(Demand {
        what: Request::CancelJob,
        target: 2,
    })) {
        bail!("giving up pointed at the row, not the id, after the reordering: {fired:?}");
    }
    println!("cursor: survives the reordering, and the request names id 2");

    // ---- HOLD READS THE STAGE ----
    //
    // Job 2 is paused, so the one key that holds and lets go must send
    // `resume` — never `pause`, which the queue would refuse.
    let held = run(&mut inst, "transfers-hold")?;
    if !held.contains(&Effect::Ask(Demand {
        what: Request::ResumeJob,
        target: 2,
    })) {
        bail!("hold/release sent the wrong verb for a held row: {held:?}");
    }
    println!("hold: reads the row's situation and never sends the wrong verb");

    // ---- A FINISHED JOB IS NOT HELD ----
    inst.event(&Input::Queue(Queue {
        jobs: vec![job(9, Engine::Media, Errand::Media, Stage::Done, GIB, GIB)],
    }))?;
    let nothing = run(&mut inst, "transfers-hold")?;
    if !nothing.is_empty() {
        bail!("holding sent a request for a row that had already ended: {nothing:?}");
    }

    // ---- A TOTAL OF ZERO NEVER DRAWS A FRACTION ----
    //
    // A download with no `Content-Length` and a torrent whose metadata has not
    // arrived both look like this, and a bar there would be an invented number.
    inst.event(&Input::Queue(Queue {
        jobs: vec![job(
            10,
            Engine::Fetch,
            Errand::Fetch,
            Stage::Running,
            900 * 1024,
            0,
        )],
    }))?;
    let unknown = look(&mut inst)?;
    if unknown.shows("%") {
        bail!("it drew a percentage with no total: {:?}", unknown.data);
    }
    if !unknown.names("transfers-stage-running") {
        bail!("with no total, the row should say the situation instead of faking a fraction");
    }
    println!("no total: it shows the situation, and never an invented fraction");

    // ---- THE DETAIL SCREEN ----
    let detailed = run(&mut inst, "transfers-detail")?;
    if !detailed.contains(&Effect::Redraw) {
        bail!("the detail asked for no redraw");
    }
    let detail = look(&mut inst)?;
    for k in [
        "transfers-window-field-engine",
        "transfers-window-field-stage",
        "transfers-window-field-total",
        "transfers-window-field-identity",
    ] {
        if !detail.names(k) {
            bail!("the detail did not show `{k}`");
        }
    }
    if !detail.names("transfers-window-unknown") {
        bail!("the detail should say the total is not known, instead of printing 0");
    }
    if !detail.names("transfers-identity-anonymous") {
        bail!("the detail did not say who asked; an identified download cannot be undone");
    }
    println!("detail: field by field, who asked, and `not known` instead of zero");

    // ---- AND THE IDENTIFIED ONE SAYS SO ----
    inst.event(&Input::Queue(Queue {
        jobs: vec![Job {
            identified: true,
            ..job(11, Engine::Media, Errand::Media, Stage::Running, 1, 2)
        }],
    }))?;
    // The screen is still on the detail from just above, and the queue that
    // just arrived has one job in it — so the cursor is on it.
    let mine = look(&mut inst)?;
    if !mine.names("transfers-identity-mine") {
        bail!("the detail of an identified job did not say it was you");
    }
    if mine.names("transfers-identity-anonymous") {
        bail!("the detail said both things at once");
    }
    println!("identity: the row that was not anonymous says it was not");

    // ---- THE CURSOR HIGHLIGHTS EXACTLY ONE ROW ----
    run(&mut inst, "transfers-detail")?;
    let back = look(&mut inst)?;
    if back.selections != 1 {
        bail!(
            "the list highlighted {} rows; it should highlight exactly the cursor's",
            back.selections
        );
    }

    println!("OK: the queue window takes from the host, and what is irreversible asks twice");
    Ok(())
}

/// The keys of an `.ftl`, as one string.
fn ftl(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("could not read {path}"))
}
