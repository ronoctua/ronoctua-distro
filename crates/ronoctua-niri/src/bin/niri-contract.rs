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
        .context("usage: niri-contract <socket-path>")?
        .into();

    let mut niri = Niri::connect_at(&path)?;

    // 1. Version. It serves as proof of life and goes into the log, so diagnostics
    //    can say WHICH niri the contract failed against.
    let version = niri.version().context("`Version` did not answer")?;
    println!("niri: {version}");

    // 2. `Windows` has to deserialise into `Window`. This is where a removed
    //    or renamed field shows up.
    let windows = niri
        .windows()
        .context("`Windows` no longer matches `Window` — the contract broke")?;
    println!("windows: {}", windows.len());

    // 3. `FocusedWindow` answers, including with zero windows open. With no
    //    window at all the correct answer is `None`, not an error — and that is exactly the
    //    state the compositor starts in.
    let focused = niri
        .focused_window()
        .context("`FocusedWindow` no longer matches `Option<Window>`")?;
    println!("focused: {:?}", focused.as_ref().map(|j| j.id));

    // 4. The layer-shell surfaces. With the footer running, ours shows up
    //    here — and it is what separates "the compositor does not see it" from "it sees it and does not paint it".
    let layers = niri
        .layers()
        .context("`Layers` no longer matches what is expected")?;
    println!("layers: {layers:?}");

    // 5. The event stream. It is what the modeline lives on.
    let mut events = niri
        .event_stream()
        .context("`EventStream` was not accepted")?;

    let mut state = State::default();
    let mut saw_windows = false;
    let mut read_lines = 0usize;

    while read_lines < EVENT_CEILING {
        let Some(event) = events.next_one()? else {
            bail!("niri closed the stream after {read_lines} events");
        };
        read_lines += 1;
        if matches!(event, Event::WindowsChanged(_)) {
            saw_windows = true;
        }
        state.apply_to(&event);
        // The complete state comes out first; stopping at the first one that matters
        // avoids blocking on an event that would only come with interaction.
        if saw_windows {
            break;
        }
    }

    if !saw_windows {
        bail!(
            "no `WindowsChanged` came in the first {read_lines} events.\n\
             \n\
             niri's documentation promises the complete state on output. If\n\
             that changed, the daemon has to ask for `Windows` explicitly."
        );
    }

    println!(
        "contract OK: {read_lines} events, {} windows",
        state.amount()
    );
    Ok(())
}
