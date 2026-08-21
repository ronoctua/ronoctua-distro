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
        subject: format!("arquivo-{id}.bin"),
        trouble: String::new(),
        answer: 0,
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

fn look(inst: &mut ronoctua_runtime::Instance) -> Result<Screen> {
    Ok(Screen::of(&inst.render(COLUMNS, ROWS, heights())?))
}

/// Fires a command by id, the way the buffer's host does when a key completes.
fn run(inst: &mut ronoctua_runtime::Instance, id: &str) -> Result<Vec<Effect>> {
    inst.event(&Input::Command(id.to_owned()))
}

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("uso: ronoctua-test-transfers <componente.wasm> <dir-i18n>")?;
    let i18n = std::env::args()
        .nth(2)
        .context("uso: ronoctua-test-transfers <componente.wasm> <dir-i18n>")?;
    let wasm = std::fs::read(&path).with_context(|| format!("nao consegui ler {path}"))?;

    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    // ---- THE DESCRIPTOR ----
    let d = inst.init()?;
    if d.mode != "transfers" {
        bail!("o modo do buffer e `{}`; a modeline mostra isso", d.mode);
    }
    if !d
        .capabilities
        .contains(&ronoctua_protocol::Capability::Transfers)
    {
        bail!("a janela nao declara `transfers`: o host nunca assinaria a fila");
    }
    // AND IT DOES NOT DECLARE `system`. Two capabilities in one buffer would
    // give a queue viewer the right to close processes and sweep the store —
    // which is exactly why they were kept apart.
    if d.capabilities
        .contains(&ronoctua_protocol::Capability::System)
    {
        bail!("a janela pede `system`, que ela nao usa: seriam poderes de graca");
    }
    for id in &d.commands {
        if !d.keymap.iter().any(|l| l.command == *id) {
            bail!("`{id}` nao tem tecla: ninguem o alcanca pelo which-key");
        }
    }
    if let Err(e) = d.validate() {
        bail!("o descritor nao passa na guarda da secao 7.9: {e}");
    }
    for language in ["pt-BR", "en-GB"] {
        let table = ftl(&format!("{i18n}/{language}.ftl"))?;
        for id in &d.commands {
            let key = format!("cmd-{id}");
            if !table.contains(&key) {
                bail!("falta `{key}` em {language}.ftl — o SPC m mostraria [{key}]");
            }
        }
    }
    println!(
        "descritor: modo `{}`, {} comandos com tecla, e so a capacidade que usa",
        d.mode,
        d.commands.len()
    );

    // ---- BEFORE THE QUEUE: no numbers ----
    let empty = look(&mut inst)?;
    if empty.shows("GiB") {
        bail!("a janela desenhou tamanho antes de receber fila alguma");
    }
    if !empty.names("transfers-window-empty") {
        bail!("a janela vazia nao diz que esta vazia");
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
            bail!("a fila nao mostrou `{k}`: um campo do record cruzou trocado");
        }
    }
    if !full.shows("1.0 GiB") || !full.shows("4.0 GiB") {
        bail!(
            "os numeros da primeira linha nao apareceram: {:?}",
            full.data
        );
    }
    // The bar is drawn with glyphs from tokens, one command per cell, plus the
    // stage and engine drawings on every row.
    if full.icons < 12 {
        bail!("a barra nao foi desenhada: {} icones", full.icons);
    }
    println!(
        "fila: tres linhas, cinco enums pelo nome, {} icones de token",
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
        bail!("desistir disparou sem perguntar: {armed:?}");
    }
    let asking = look(&mut inst)?;
    if !asking.names("transfers-window-confirm-cancel") {
        bail!("a pergunta nao apareceu na tela");
    }

    // And the second press names the ID it was on — 2 — and not the row it now
    // occupies, which after the reversal is a different one.
    let fired = inst.event(&Input::Confirm)?;
    if !fired.contains(&Effect::Ask(Demand {
        what: Request::CancelJob,
        target: 2,
    })) {
        bail!("desistir apontou a linha, e nao o id, depois do reordenamento: {fired:?}");
    }
    println!("cursor: sobrevive ao reordenamento, e o pedido nomeia o id 2");

    // ---- HOLD READS THE STAGE ----
    //
    // Job 2 is paused, so the one key that holds and lets go must send
    // `resume` — never `pause`, which the queue would refuse.
    let held = run(&mut inst, "transfers-hold")?;
    if !held.contains(&Effect::Ask(Demand {
        what: Request::ResumeJob,
        target: 2,
    })) {
        bail!("segurar/soltar mandou o verbo errado para uma linha segurada: {held:?}");
    }
    println!("segurar: le a situacao da linha e nunca manda o verbo errado");

    // ---- A FINISHED JOB IS NOT HELD ----
    inst.event(&Input::Queue(Queue {
        jobs: vec![job(9, Engine::Media, Errand::Media, Stage::Done, GIB, GIB)],
    }))?;
    let nothing = run(&mut inst, "transfers-hold")?;
    if !nothing.is_empty() {
        bail!("segurar mandou pedido para uma linha que ja acabou: {nothing:?}");
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
        bail!("desenhou porcentagem sem total: {:?}", unknown.data);
    }
    if !unknown.names("transfers-stage-running") {
        bail!("sem total, a linha devia dizer a situacao em vez de fingir uma fracao");
    }
    println!("sem total: mostra a situacao, e nunca uma fracao inventada");

    // ---- THE DETAIL SCREEN ----
    let detailed = run(&mut inst, "transfers-detail")?;
    if !detailed.contains(&Effect::Redraw) {
        bail!("o detalhe nao pediu redesenho");
    }
    let detail = look(&mut inst)?;
    for k in [
        "transfers-window-field-engine",
        "transfers-window-field-stage",
        "transfers-window-field-total",
    ] {
        if !detail.names(k) {
            bail!("o detalhe nao mostrou `{k}`");
        }
    }
    if !detail.names("transfers-window-unknown") {
        bail!("o detalhe devia dizer que o total nao se sabe, em vez de imprimir 0");
    }
    println!("detalhe: campo a campo, e diz `nao se sabe` em vez de zero");

    // ---- THE CURSOR HIGHLIGHTS EXACTLY ONE ROW ----
    run(&mut inst, "transfers-detail")?;
    let back = look(&mut inst)?;
    if back.selections != 1 {
        bail!(
            "a lista destacou {} linhas; devia destacar exatamente a do cursor",
            back.selections
        );
    }

    println!("OK: a janela da fila recebe do host, e o que e irreversivel pede duas vezes");
    Ok(())
}

/// The keys of an `.ftl`, as one string.
fn ftl(path: &str) -> Result<String> {
    std::fs::read_to_string(path).with_context(|| format!("nao consegui ler {path}"))
}
