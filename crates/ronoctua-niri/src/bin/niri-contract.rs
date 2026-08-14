//! IPC contract test — milestone 4's acceptance criterion, risk R2.
//!
//! It runs INSIDE a niri session and exercises, against the actual
//! compositor, every single thing the shell depends on. It is run twice: against the
//! pinned tag and against `niri-next`. The second is what gives the warning — if
//! upstream removes or renames something, this fails months before the release
//! becomes mandatory, which is the only point at which there is still something to do without
//! violating invariant 3.
//!
//! It fails with a message that says WHAT disappeared, not "deserialisation error".
//!
//! Usage: `niri-contract <socket-path>`.

use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use ronoctua_niri::{Event, Niri, State};

/// How many events to read before giving up on seeing the mandatory ones.
///
/// niri sends the complete state on the way out, so the ones we expect come in the
/// first few. The slack covers configuration and cast events that show up
/// in between and vary between versions.
const EVENT_CEILING: usize = 40;

fn main() -> Result<()> {
    let path: PathBuf = std::env::args_os()
        .nth(1)
        .context("uso: niri-contract <caminho-do-socket>")?
        .into();

    let mut niri = Niri::connect_at(&path)?;

    // 1. Version. It serves as proof of life and goes into the log, so diagnostics
    //    can say WHICH niri the contract failed against.
    let version = niri.version().context("`Version` nao respondeu")?;
    println!("niri: {version}");

    // 2. `Windows` has to deserialise into `Window`. This is where a removed
    //    or renamed field shows up.
    let windows = niri
        .windows()
        .context("`Windows` nao casa mais com `Janela` — o contrato quebrou")?;
    println!("janelas: {}", windows.len());

    // 3. `FocusedWindow` answers, including with zero windows open. With no
    //    window at all the correct answer is `None`, not an error — and that is exactly the
    //    state the compositor starts in.
    let focused = niri
        .focused_window()
        .context("`FocusedWindow` nao casa mais com `Option<Janela>`")?;
    println!("focada: {:?}", focused.as_ref().map(|j| j.id));

    // 4. The layer-shell surfaces. With the footer running, ours shows up
    //    here — and it is what separates "the compositor does not see it" from "it sees it and does not paint it".
    let layers = niri
        .layers()
        .context("`Layers` nao casa mais com o esperado")?;
    println!("camadas: {layers:?}");

    // 5. The event stream. It is what the modeline lives on.
    let mut events = niri
        .event_stream()
        .context("`EventStream` nao foi aceito")?;

    let mut state = State::default();
    let mut viu_janelas = false;
    let mut lidos = 0usize;

    while lidos < EVENT_CEILING {
        let Some(event) = events.next_one()? else {
            bail!("o niri fechou o fluxo depois de {lidos} eventos");
        };
        lidos += 1;
        if matches!(event, Event::WindowsChanged(_)) {
            viu_janelas = true;
        }
        state.apply_to(&event);
        // The complete state comes out first; stopping at the first one that matters
        // avoids blocking on an event that would only come with interaction.
        if viu_janelas {
            break;
        }
    }

    if !viu_janelas {
        bail!(
            "nao veio `WindowsChanged` nos primeiros {lidos} eventos.\n\
             \n\
             A documentacao do niri promete o estado completo de saida. Se\n\
             isso mudou, o daemon precisa pedir `Windows` explicitamente."
        );
    }

    println!("contrato OK: {lidos} eventos, {} janelas", state.amount());
    Ok(())
}
