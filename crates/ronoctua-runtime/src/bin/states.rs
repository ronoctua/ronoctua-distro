//! Exercises the STATE contract on every buffer that declares one.
//!
//! # Why a binary, and not a `#[test]` in each buffer
//!
//! The components compile to `wasm32-wasip2` and live in workspaces of their own
//! (invariant 12): a `cargo test` over one of them exercises the logic OUTSIDE
//! the component, which is precisely what proves nothing about the boundary.
//! Here each is loaded the way the host loads it, sent `enter-state` the way the
//! host sends it, and read back through `render`.
//!
//! # What only this can prove
//!
//! That the two halves of the contract MEET. Each half is checkable on its own
//! and neither is worth much alone:
//!
//!   - the descriptor's states are guarded by `Descriptor::validate`;
//!   - `state-<id>` is guarded by `buffer-states-have-text`;
//!   - the host's `Esc` layer is unit-tested in `buffer.rs`.
//!
//! What none of them reaches is the round trip: send the id the buffer
//! declared, and get the SAME id back out of `effect::state-changed` — through
//! the WIT, in the component's own memory. A buffer that took `enter-state` and
//! did nothing would satisfy every check above and leave the modeline naming a
//! state the screen is not in.
//!
//! It runs over EVERY buffer given to it, and not one chosen buffer, because the
//! contract is the same for all of them and the six were written in one pass —
//! the class of mistake that pass invites is exactly the one that shows up in
//! whichever file the writer got tired on.
//!
//! Usage: `ronoctua-test-states <component.wasm> [<component.wasm> …]`

use anyhow::{Context as _, Result, bail};
use ronoctua_protocol::{BASE_STATE, Effect, Input};

/// The scale heights the production host sends.
fn heights() -> ronoctua_protocol::Heights {
    ronoctua_protocol::Heights::from_sizes(15.0, 18.0, 20.0, 40.0, 116.0, 1.45)
}

const ROWS: u16 = 30;
const COLUMNS: u16 = 110;

/// The id inside a `state-changed`, if that is what the effect is.
fn reported(effects: &[Effect]) -> Option<&str> {
    effects.iter().find_map(|e| match e {
        Effect::StateChanged(id) => Some(id.as_str()),
        _ => None,
    })
}

fn drive(path: &str) -> Result<()> {
    let wasm = std::fs::read(path).with_context(|| format!("nao consegui ler {path}"))?;
    let rt = ronoctua_runtime::Runtime::fresh()?;
    let cwasm = rt.precompile(&wasm)?;
    let component = rt.load(&cwasm)?;
    let mut inst = rt.open(&component)?;

    let d = inst.init()?;
    // The guard the host applies at load, applied here for the same reason: a
    // descriptor that would be refused on screen must be refused here too, or
    // this driver would go on to prove things about a buffer nobody can open.
    if let Err(e) = d.validate() {
        bail!("{}: o descritor nao passa na guarda: {e}", d.name);
    }
    let ids: Vec<String> = d.states.iter().map(|s| s.id.clone()).collect();
    println!("{}: estados {ids:?}", d.name);

    // The first `render` before anything else, because the host does it too —
    // and because a component that only computes its state lazily would other-
    // wise be measured from a screen it never drew.
    inst.render(COLUMNS, ROWS, heights())?;

    // ---- ONE STATE IS THE NORMAL CASE, AND IT IS NOT A FAILURE ----
    if d.states.len() == 1 {
        // It still has to ANSWER: being sent to the state it is already in must
        // not error, must not close, and must not redraw the world.
        let effects = inst.event(&Input::EnterState(BASE_STATE.to_owned()))?;
        if effects.iter().any(|e| matches!(e, Effect::Close)) {
            bail!("{}: `enter-state` fechou o buffer", d.name);
        }
        return Ok(());
    }

    // ---- EVERY DECLARED STATE IS REACHABLE, AND SAYS SO ----
    //
    // The check is on `state-changed` and not on the pixels: what the modeline
    // reads is the report, so a buffer that changed screens without reporting
    // would draw the right thing and name the wrong one. That is the failure
    // this driver exists for.
    for going in &ids {
        if going == BASE_STATE {
            continue;
        }
        // Back to the base first, so each state is entered from the SAME place.
        // Without this, `sheet` after `filter` would prove only that the second
        // change works from wherever the first one left off.
        inst.event(&Input::EnterState(BASE_STATE.to_owned()))?;
        inst.render(COLUMNS, ROWS, heights())?;

        let effects = inst.event(&Input::EnterState(going.clone()))?;
        inst.render(COLUMNS, ROWS, heights())?;
        match reported(&effects) {
            Some(said) if said == going => {}
            Some(said) => bail!("{}: pedi `{going}` e o componente relatou `{said}`", d.name),
            // SILENCE IS THE FAILURE, and this arm was written the other way
            // round first: it PRINTED that the state had refused and carried on,
            // reasoning that `sheet` with nothing under the cursor is a screen
            // of empty fields and may legitimately say no.
            //
            // Planting a buffer whose `enter-state` ignored the id showed what
            // that costs — the driver printed a line and exited zero, which is
            // the exact defect it exists to catch. A state nobody can reach from
            // the base state is a state nobody can reach, and the modeline would
            // go on offering it under `SPC b s`.
            //
            // If a buffer ever has a state that genuinely needs a setup first,
            // the answer is to give this driver the setup — not to let silence
            // count as success.
            None => bail!(
                "{}: pedi `{going}` e o componente nao relatou nada.\n\
                 Um estado declarado que nao se alcanca do base e um estado que \
                 o `SPC b s` oferece e ninguem entra.",
                d.name
            ),
        }

        // ---- AND `Esc` COMES BACK, ONE LAYER, ON ITS OWN ----
        //
        // `Cancel` is the WHOLE of what a tap of `Esc` delivers: the host does
        // not also send the base state, because doing that threw away the search
        // somebody had typed to find the thing whose sheet they were leaving.
        // So "`Esc` returns" is a promise the COMPONENT keeps, and this is where
        // it is collected.
        //
        // The report is what is checked, and not the pixels: the modeline reads
        // `state-changed`, so a buffer that walked back without saying so would
        // draw the grid and go on naming the sheet.
        let back = inst.event(&Input::Cancel)?;
        inst.render(COLUMNS, ROWS, heights())?;
        if reported(&back).is_none() {
            bail!(
                "{}: `Esc` em `{going}` nao voltou, ou voltou sem relatar.\n\
                 O rodape le o relato: sem ele a modeline segue nomeando a tela \
                 que ja saiu.",
                d.name
            );
        }
    }

    // ---- AN ID THE BUFFER NEVER DECLARED CHANGES NOTHING ----
    //
    // The host refuses these before they are sent, and the socket refuses them
    // before that. This is the third door, and it is the one that matters if the
    // other two are ever reordered: a component that took an unknown id and
    // guessed at the nearest state would put a screen up that no key returns
    // from.
    inst.event(&Input::EnterState(BASE_STATE.to_owned()))?;
    let effects = inst.event(&Input::EnterState("a-state-from-the-future".to_owned()))?;
    if let Some(said) = reported(&effects) {
        bail!("{}: um id inexistente virou `{said}`", d.name);
    }
    Ok(())
}

fn main() -> Result<()> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        bail!("uso: ronoctua-test-states <componente.wasm> [<componente.wasm> …]");
    }
    for p in &paths {
        drive(p)?;
    }
    println!(
        "OK: {} buffers, o contrato de estado fecha nos dois sentidos",
        paths.len()
    );
    Ok(())
}
