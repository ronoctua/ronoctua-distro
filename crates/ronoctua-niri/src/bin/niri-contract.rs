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
use ronoctua_niri::{Ask, Change, Event, ModeAsked, Niri, State, Transform};

/// How many events to read before giving up on seeing the mandatory ones.
///
/// niri sends the complete state on the way out, so the ones we expect come in the
/// first few. The slack covers configuration and cast events that show up
/// in between and vary between versions.
const EVENT_CEILING: usize = 40;

/// How long to wait for one event before deciding the replay is over.
///
/// niri sends its opening state as fast as it can parse the request; five
/// seconds is a machine under heavy load, not a machine thinking.
const PATIENCE: std::time::Duration = std::time::Duration::from_secs(5);

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

    // 5. `Outputs` has to deserialise into `Output` — the same kind of check
    //    as step 2, on the other half of the compositor's state.
    let outputs = niri
        .outputs()
        .context("`Outputs` no longer matches `Output` — the contract broke")?;
    for (name, o) in &outputs {
        println!(
            "output {name}: {} {} · {} modes · {}",
            o.make,
            o.model,
            o.modes.len(),
            o.logical
                .map_or_else(|| "off".to_owned(), |l| format!("{}x{}", l.width, l.height)),
        );
    }

    // 6. `FocusedOutput`, which answers `Option<Output>` — and answers
    //    `None` legitimately, between one monitor going away and the next
    //    taking focus. It is what "this screen" means to a brightness key.
    let focused = niri
        .focused_output()
        .context("`FocusedOutput` no longer matches `Option<Output>`")?;
    println!("focused output: {:?}", focused.as_ref().map(|o| &o.name));

    // 7. EVERY SHAPE OF `Output` ACTION, against a name that cannot exist.
    //
    //    WHAT REJECTS A DRIFTED SHAPE is serde failing to parse the whole
    //    `Request`, before the handler runs at all — NOT `OutputAction::
    //    validate`, which only constrains the modeline and the custom mode
    //    and passes everything `Ask` can build. That is why the name being
    //    unmatchable costs the step nothing.
    //
    //    IT IS NOT SIDE-EFFECT FREE, and the honest version is: for a name it
    //    does not know, niri pushes a fresh output section into its RUNNING
    //    configuration and reloads the output config once per probe
    //    (`niri.rs`, `modify_output_config` then `reload_output_config`).
    //    Nothing looks that section up — every consumer matches a real
    //    `OutputName` — so no monitor on the machine moves, which is the
    //    property that matters here. The junk section lives until the next
    //    read of the config file.
    //
    //    This is the one step that could not be a unit test: the shapes are
    //    checked against `niri-ipc`'s own parser, in the version that is
    //    actually going to run.
    const NOWHERE: &str = "ronoctua-contract-no-such-output";
    let asks = [
        ("on", Ask::On),
        ("off", Ask::Off),
        ("mode auto", Ask::Mode(None)),
        (
            "mode",
            Ask::Mode(Some(ModeAsked {
                width: 1920,
                height: 1080,
                refresh: Some(60.0),
            })),
        ),
        ("scale auto", Ask::Scale(None)),
        ("scale", Ask::Scale(Some(1.5))),
        // The transform is the one enum whose eight wire names could each
        // drift on their own, and three of them are bare digits. A unit test
        // can only compare our strings to our strings.
        ("transform normal", Ask::Transform(Transform::Normal)),
        ("transform 90", Ask::Transform(Transform::R90)),
        ("transform 180", Ask::Transform(Transform::R180)),
        ("transform 270", Ask::Transform(Transform::R270)),
        ("transform flipped", Ask::Transform(Transform::Flipped)),
        ("transform flipped-90", Ask::Transform(Transform::Flipped90)),
        (
            "transform flipped-180",
            Ask::Transform(Transform::Flipped180),
        ),
        (
            "transform flipped-270",
            Ask::Transform(Transform::Flipped270),
        ),
        ("position auto", Ask::Position(None)),
        ("position", Ask::Position(Some((1920, -180)))),
        (
            "vrr",
            Ask::Vrr {
                on: true,
                on_demand: true,
            },
        ),
    ];
    for (what, ask) in asks {
        let answer = niri.output_action(NOWHERE, ask).with_context(|| {
            format!("niri no longer accepts the `{what}` shape of an output action")
        })?;
        if answer != Change::OutputWasMissing {
            bail!(
                "the `{what}` probe answered {answer:?} for an output that cannot exist.\n\
                 \n\
                 Either niri stopped answering `OutputWasMissing` for an unknown\n\
                 name, or something on this machine really is called {NOWHERE}."
            );
        }
    }
    println!("output actions: {} shapes accepted", asks.len());

    // 8. The event stream. It is what the modeline lives on.
    let mut events = niri
        .event_stream()
        .context("`EventStream` was not accepted")?;

    let mut state = State::default();
    let mut saw_windows = false;
    let mut saw_config = false;
    let mut read_lines = 0usize;

    while read_lines < EVENT_CEILING {
        // WITH A DEADLINE, and this is not a detail. niri's replay is a
        // fixed prefix of five or six events and then silence, so an event
        // that upstream removed would leave a plain `read_line` blocked for
        // ever — and the harness would report a timeout instead of the
        // sentence below naming what disappeared, which is the only thing a
        // contract test is for.
        let Some(event) = events.next_within(PATIENCE)? else {
            break;
        };
        read_lines += 1;
        match event {
            Event::WindowsChanged(_) => saw_windows = true,
            Event::ConfigLoaded { failed } => {
                saw_config = true;
                println!("config loaded, failed: {failed}");
            }
            _ => {}
        }
        state.apply_to(&event);
        // BOTH, and not just the windows. niri replays its state in a fixed
        // order and `ConfigLoaded` comes AFTER `WindowsChanged`
        // (`niri-ipc`'s `state.rs`), so stopping at the first one that
        // mattered meant the event this crate reads to learn that its own
        // configuration fragment landed was never exercised here at all.
        if saw_windows && saw_config {
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

    if !saw_config {
        bail!(
            "no `ConfigLoaded` in {read_lines} events, and then the stream\n\
             went quiet — it stopped sending, or it closed.\n\
             \n\
             niri promises this event on connecting. It is how the display\n\
             view learns that the fragment it wrote was read, and whether it\n\
             parsed. Without it a bad fragment is silent."
        );
    }

    println!(
        "contract OK: {read_lines} events, {} windows, {} outputs",
        state.amount(),
        outputs.len()
    );
    Ok(())
}
