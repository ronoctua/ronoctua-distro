//! THE CONTRACT TEST — run against a live compositor.
//!
//! Usage: `ronoctua-contract <socket-path>`
//!
//! # What it is for
//!
//! `ronoctua-compositor` is a promise about what a compositor does. This is
//! what holds the promise to a real one: it exercises the trait, not the
//! adapter's insides, so any future adapter passes the same binary.
//!
//! # Why a binary and not a `#[test]`
//!
//! Because it needs a compositor, and a unit test that needs one is a unit
//! test that cannot run. The VM test starts a real session and hands the
//! socket to this.
//!
//! # The three passes
//!
//! 1. **The readings.** Every getter, and the types they come back as.
//! 2. **Every deed, parsed and refused on purpose.** A command is resolved by
//!    NAME before its criteria are tested, so a deed sent with a criterion
//!    that matches nothing distinguishes "this verb does not exist" from
//!    "this verb exists and had nothing to act on" — which is the whole of
//!    what a parse pass can prove, for all of them, with no side effects.
//! 3. **The asks, applied for real.** On a spare output the compositor makes
//!    for the pass, so every `Ask` is exercised against something that exists
//!    and is then unplugged — without pushing junk into the running
//!    configuration, which a probe against an impossible NAME would do. Which
//!    backend that output comes up on is the compositor's choice, so the
//!    spare is identified by BEING NEW and never by a name written here.

use std::path::Path;

use anyhow::{Context as _, Result, bail};
use ronoctua_compositor::{Ask, Compositor, ModeAsked, Transform};
use ronoctua_scroll::{Scroll, deeds};

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: ronoctua-contract <socket-path>")?;
    let mut wm = Scroll::connect_at(Path::new(&path))?;

    // ---- 1. the readings -------------------------------------------
    // PRINTED, so a failure names WHICH build it was talking to.
    let version = wm.version().context("asking what it is")?;
    println!("compositor: {version}");

    let windows = wm.windows().context("listing the windows")?;
    println!("windows: {}", windows.len());
    for w in &windows {
        // THE PID IS THE ONE FIELD NOTHING ELSE CAN REPLACE: the control
        // socket of a buffer is named after it, and so is the whole PID →
        // cgroup correlation. A window without one is a window the shell
        // cannot pair with anything.
        if w.pid.is_none() {
            bail!("the window {} came back with no pid", w.id);
        }
    }

    let focused = wm.focused_window().context("asking what has the focus")?;
    println!(
        "focused: {}",
        focused.map_or_else(|| "nothing".to_owned(), |w| w.id.to_string())
    );

    let outputs = wm.outputs().context("listing the outputs")?;
    println!("outputs: {}", outputs.len());
    for (at, o) in &outputs {
        println!(
            "  {at}: {} — {} modes, {}",
            o.identity(),
            o.modes.len(),
            if o.logical.is_some() { "lit" } else { "dark" }
        );
        // The filing key has to resolve to something, whichever kind it is.
        if o.naming().name().is_empty() {
            bail!("{at} has no name to file its settings under");
        }
    }
    let _ = wm
        .focused_output()
        .context("asking which output has focus")?;

    // THE WORKSPACES, which a column is bound to BY NAME and shown where
    // they are: every one named, and every one a screen shows says which.
    let workspaces = wm.workspaces().context("listing the workspaces")?;
    println!("workspaces: {}", workspaces.len());
    for w in &workspaces {
        if w.name.is_empty() {
            bail!("a workspace came back with no name");
        }
        if w.visible && w.output.is_none() {
            bail!("the workspace {} is shown and names no output", w.name);
        }
    }

    // ---- 2. every deed, by name --------------------------------------
    // A criterion that matches nothing: the handler is resolved BEFORE the
    // criteria are tested, so an unknown verb fails differently from a verb
    // with nothing to act on. `con_id` of `u32::MAX` is a container that
    // cannot exist.
    let mut unknown = Vec::new();
    for spoken in deeds::SPOKEN {
        let text = match spoken.deed.argument {
            // Something the validator accepts, since the pass is about the
            // VERB and not about the value.
            Some(ronoctua_compositor::Arity::Number) => spoken.text.replace("{n}", "1"),
            Some(ronoctua_compositor::Arity::Word) => spoken.text.replace("{n}", "probe"),
            None => spoken.text.to_owned(),
        };
        let probe = format!("[con_id=4294967295] {text}");
        match wm.run_text(&probe) {
            Ok(()) => {}
            Err(e) => {
                let said = format!("{e:#}").to_lowercase();
                // "no matching node" is the pass: the verb exists and had
                // nothing to act on. Anything about the command itself is not.
                if said.contains("unknown") || said.contains("invalid command") {
                    unknown.push((spoken.deed.id, text));
                }
            }
        }
    }
    if !unknown.is_empty() {
        for (id, text) in &unknown {
            eprintln!("  {id} says `{text}`, which this compositor does not have");
        }
        bail!("{} deeds name a command that does not exist", unknown.len());
    }
    println!(
        "deeds: {} — every one of them a command it has",
        deeds::SPOKEN.len()
    );

    // ---- 3. every ask, applied ---------------------------------------
    // AN OUTPUT OF ITS OWN, so nothing here touches a monitor somebody is
    // looking at. The compositor always adds a headless backend beside the
    // real one, which is what makes this possible on a live machine.
    //
    // AND ITS NAME IS THE ONE THAT APPEARED, never a name written here.
    // `create_output` walks the multi-backend and stops at the FIRST member
    // that can make one, so what comes up is `HEADLESS-1` on a machine driving
    // real monitors and `X11-2` under an Xvfb — a hardcoded `HEADLESS-1` said
    // "the spare output did not appear" about an output that was plainly
    // there, and the whole of section 3 went unrun in the one place it runs
    // automatically.
    let before = wm.outputs()?;
    wm.run_text("create_output")
        .context("making a spare output")?;
    let spare_now = wm.outputs()?;
    let spare: String = spare_now
        .keys()
        .find(|name| !before.contains_key(*name))
        .cloned()
        .context("the spare output did not appear; the asks cannot be exercised")?;
    let spare = spare.as_str();
    println!("spare: `{spare}`, made for this pass and taken away at the end");

    let asks: Vec<(&str, Ask)> = vec![
        ("on", Ask::On),
        ("mode automatic", Ask::Mode(None)),
        (
            "mode",
            Ask::Mode(Some(ModeAsked {
                width: 1280,
                height: 720,
                refresh: Some(60.0),
            })),
        ),
        ("scale automatic", Ask::Scale(None)),
        ("scale", Ask::Scale(Some(1.0))),
        ("position", Ask::Position(Some((0, 0)))),
        ("vary on", Ask::Vary(true)),
        ("vary off", Ask::Vary(false)),
        ("off", Ask::Off),
    ];
    for (what, ask) in asks {
        wm.ask(spare, ask)
            .with_context(|| format!("asking for {what}"))?;
    }
    // EVERY TRANSFORM, because the enum is closed: eight Wayland transforms
    // and a ninth would be a protocol break rather than a version to shrug at.
    wm.ask(spare, Ask::On)?;
    for t in Transform::ALL {
        wm.ask(spare, Ask::Transform(t))
            .with_context(|| format!("turning it {t:?}"))?;
    }
    println!("asks: every shape applied, and every transform");

    // AND THE READING IS WHAT CONFIRMS IT, never the answer.
    let after = wm.outputs()?;
    let seen = after
        .get(spare)
        .context("the spare output vanished mid-pass")?;
    if seen.logical.is_none() {
        bail!("the spare output is dark after being asked to come on");
    }

    wm.run_text(&format!("output {spare} unplug"))
        .context("taking the spare output away")?;
    if wm.outputs()?.contains_key(spare) {
        bail!("the spare output is still there; it would outlive this run");
    }

    // ---- 4. devices and seats ----------------------------------------
    // Every setting a device reports is asked back AT ITS OWN VALUE, so the
    // pass proves every spelling is one the compositor takes without
    // changing anything a person set. A seat of the pass's own is made, so
    // the seat verbs are exercised without touching `seat0` — and the
    // re-read below must take it away again, which is the only way this
    // compositor removes a seat.
    let devices = wm.devices().context("listing the input devices")?;
    let mut asked = 0_usize;
    for d in devices.iter().filter(|d| d.kind.points()) {
        for s in d.settings.present() {
            wm.ask_input(&d.id, s)
                .with_context(|| format!("{} refused its own {s:?}", d.id))?;
            asked += 1;
        }
    }
    println!(
        "devices: {} — {asked} settings asked back unchanged",
        devices.len()
    );
    let probe = "ronoctua-contract-probe";
    wm.ask_seat(
        probe,
        ronoctua_compositor::SeatAsk::Cursor {
            theme: "default".to_owned(),
            size: 24,
        },
    )
    .context("making a seat")?;
    wm.ask_seat(probe, ronoctua_compositor::SeatAsk::HideAfter(0))?;
    wm.ask_seat(probe, ronoctua_compositor::SeatAsk::HideWhileTyping(false))?;
    wm.ask_focus(ronoctua_compositor::FocusFollows::Yes)?;
    let seats = wm.seats().context("listing the seats")?;
    if !seats.iter().any(|s| s.name == probe) {
        bail!("a seat was asked for and is not in the list");
    }
    println!("seats: {} — the pass's own among them", seats.len());

    // ---- the gaps, set on every workspace ----------------------------
    // NOT a transient ask, and the re-read below does NOT undo it: the
    // theme's Spacing answer reaches the workspaces already on screen with
    // this, because the configuration's own `gaps` only touches the ones
    // born after it. Both values, so the pass proves the command parses
    // either way; an `Ok` says it was carried, as everywhere here.
    wm.set_gaps(0, 0).context("setting every gap to none")?;
    wm.set_gaps(8, 0).context("setting the gaps back")?;
    println!("gaps: set on every workspace, inner and outer");

    // ---- and the re-read, which is also the whole of undo -------------
    // AND AN `Ok` HERE SAYS THE REQUEST WAS CARRIED, nothing more: this
    // compositor answers `success` whatever parsed (measured — see
    // `Scroll::reread`). What the pass proves is that the verb exists and that
    // everything section 3 applied is gone afterwards, which is the whole of
    // undo.
    wm.reread().context("asking it to re-read its config")?;
    if wm.seats()?.iter().any(|s| s.name == probe) {
        bail!("the pass's seat outlived the re-read; the plan of seats relies on it going");
    }
    println!("reread: it took the request, and every transient ask is gone");

    println!("the contract holds");
    Ok(())
}
