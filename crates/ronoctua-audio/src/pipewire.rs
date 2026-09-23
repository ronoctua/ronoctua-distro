//! The one module that knows `pw-dump`, `wpctl` and `pw-cat` exist.
//!
//! # Why three programs and not a library
//!
//! The same measurement `ronoctua-bluetooth` waited on, and it still holds:
//! `nix path-info -r` on the system's own toplevel finds all three in
//! `sw/bin` already, because `services.pipewire` puts them there. Linking
//! PipeWire's C API would be a second FFI boundary in this repository — and
//! `nix/rust.nix` writes down what the first one costs — in exchange for
//! nothing this view needs.
//!
//! `pw-dump` and `pw-cat` READ, `wpctl` WRITES, and that is also the safety
//! line: nothing here changes anything until a key is pressed.
//!
//! # The trap
//!
//! **`wpctl` speaks the perceived scale.** `set-volume ID 0.38` stores an
//! amplitude of `0.0549`, not `0.38`. Handing it `channelVolumes` straight out
//! of `pw-dump` sets 38% to what 72% sounds like — see [`crate::stored`].

use crate::{Order, Refusal};
use anyhow::{Context as _, Result};
use std::os::unix::process::CommandExt as _;

/// Where `pw-dump` lives, baked at build time.
const PW_DUMP: Option<&str> = option_env!("RONOCTUA_PW_DUMP");

/// Where `wpctl` lives, baked at build time.
const WPCTL: Option<&str> = option_env!("RONOCTUA_WPCTL");

/// Where `pw-cat` lives, baked at build time.
const PW_CAT: Option<&str> = option_env!("RONOCTUA_PW_CAT");

/// Where `setpriv` lives, baked at build time.
///
/// It is what makes the recorder die with the footer — see [`record`].
const SETPRIV: Option<&str> = option_env!("RONOCTUA_SETPRIV");

/// How long to wait before trying the monitor again, the first time.
const FIRST_PAUSE: std::time::Duration = std::time::Duration::from_millis(500);

/// And the longest it ever waits.
///
/// PipeWire is a user service that systemd restarts in well under a second;
/// a ceiling much above this would mean a machine sitting with no sound long
/// after the daemon came back.
const MOST_PAUSE: std::time::Duration = std::time::Duration::from_secs(15);

/// How many failures in a row before the screen is told the graph is empty.
///
/// One is not enough: restarting PipeWire kills the monitor and the socket is
/// gone for the tenth of a second it takes to come back, and clearing the
/// marks for that long is a blink nobody asked for. Two means a real outage
/// empties the screen and a restart does not.
const BEFORE_GIVING_UP: u32 = 2;

/// A program refused, and this is what it said.
#[derive(Debug)]
pub struct Refused {
    /// The shape of the refusal, for the sentence on screen.
    pub why: Refusal,
    /// The program's own words, which travel as an argument and are never a key.
    pub said: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.said)
    }
}

impl std::error::Error for Refused {}

/// One whole `pw-dump`, as text.
///
/// # Errors
///
/// When this build has no `pw-dump`, or the program cannot be run, or it
/// refuses.
pub fn dump() -> Result<String> {
    let program = PW_DUMP.context("this build has no pw-dump")?;
    let out = std::process::Command::new(program)
        .output()
        .with_context(|| format!("could not run {program}"))?;
    anyhow::ensure!(
        out.status.success(),
        Refused {
            why: Refusal::NotAvailable,
            said: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        }
    );
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Follow the graph for as long as the process lives.
///
/// It never returns. The child is respawned when it dies, because PipeWire
/// being restarted — by an update, by `LEADER s q`, by the rescue console's
/// own "restart audio" — must not leave the screen frozen on the last reading
/// it ever saw. That is the failure that once killed the tunnel thread and
/// left two modeline marks lying for the rest of a session.
pub fn follow(on_event: &dyn Fn(crate::Event)) {
    let mut pause = FIRST_PAUSE;
    let mut failures = 0_u32;
    let mut said_it_was_empty = false;
    loop {
        match run_once(on_event) {
            Ok(saw) if saw > 0 => {
                // A run that reported something is a run that worked, so the
                // next outage starts its own backoff from the bottom.
                pause = FIRST_PAUSE;
                failures = 0;
                said_it_was_empty = false;
                tracing::info!(saw, "the PipeWire monitor ended; starting it again");
            }
            Ok(_) => failures += 1,
            Err(e) => {
                failures += 1;
                tracing::warn!(error = %e, "the PipeWire monitor would not start");
            }
        }
        if failures >= BEFORE_GIVING_UP && !said_it_was_empty {
            said_it_was_empty = true;
            on_event(crate::Event::Snapshot(Box::default()));
        }
        std::thread::sleep(pause);
        pause = (pause * 2).min(MOST_PAUSE);
    }
}

/// One life of the monitor child. Returns how many readings it reported.
fn run_once(on_event: &dyn Fn(crate::Event)) -> Result<u64> {
    let program = PW_DUMP.context("this build has no pw-dump")?;
    let mut child = std::process::Command::new(program)
        .args(["--monitor", "--no-colors"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("could not run {program} --monitor"))?;
    let Some(out) = child.stdout.take() else {
        let _ = child.kill();
        anyhow::bail!("the monitor came up with no output to read");
    };

    // THE FRAMING IS `serde_json`'S OWN. `pw-dump --monitor` prints the whole
    // graph and then one array per change, back to back with no separator —
    // and a stream deserialiser is built for exactly that. Counting brackets
    // by hand would be a second framer to keep honest about strings and
    // escapes, for a problem the library already solved.
    let mut graph = crate::graph::Graph::fresh();
    let mut reported = 0_u64;
    let reader = std::io::BufReader::new(out);
    for value in serde_json::Deserializer::from_reader(reader).into_iter::<serde_json::Value>() {
        match value {
            Ok(v) => {
                graph.took(&v);
                reported += 1;
                on_event(crate::Event::Snapshot(Box::new(graph.snapshot())));
            }
            Err(e) => {
                // A broken value ENDS the run rather than being skipped: the
                // deserialiser cannot tell us where the next one starts, so
                // carrying on would fold rubbish into a picture that is drawn.
                tracing::warn!(error = %e, "the monitor's output stopped making sense");
                break;
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    Ok(reported)
}

/// What `wpctl` is asked, for one order.
///
/// A pure function so the arguments are tested without PipeWire, and so the
/// one place a volume becomes a number is readable on its own.
/// [`Order::Listen`] has no `wpctl` behind it and answers `None`.
#[must_use]
pub fn argv(order: &Order) -> Option<Vec<String>> {
    Some(match *order {
        Order::Volume { node, percent } => vec![
            "set-volume".to_owned(),
            node.to_string(),
            // SIX PLACES, and the number is the PERCEIVED fraction. `wpctl`
            // parses a float; `{percent}%` would look right and mean the same
            // thing, but it also accepts `+`/`-` suffixes, and building a
            // string a person could accidentally make relative is how a step
            // becomes a jump.
            format!("{:.6}", crate::stored(percent)),
        ],
        Order::Mute { node, muted } => vec![
            "set-mute".to_owned(),
            node.to_string(),
            if muted { "1" } else { "0" }.to_owned(),
        ],
        Order::Default { node } => vec!["set-default".to_owned(), node.to_string()],
        Order::Profile { device, index } => vec![
            "set-profile".to_owned(),
            device.to_string(),
            index.to_string(),
        ],
        Order::Route { device, index } => vec![
            "set-route".to_owned(),
            device.to_string(),
            index.to_string(),
        ],
        Order::Listen(_) => return None,
    })
}

/// Do one order.
///
/// # Errors
///
/// When this build has no `wpctl`, or the program cannot be run, or it
/// refuses — and then the error carries [`Refused`] with what it said.
pub fn carry_out(order: &Order) -> Result<()> {
    let Some(args) = argv(order) else {
        // The meter is not a `wpctl` order; `watch` handles it before this is
        // reached, and answering "done" here keeps that a fact rather than an
        // assumption about a caller.
        return Ok(());
    };
    let program = WPCTL.context("this build has no wpctl")?;
    let out = std::process::Command::new(program)
        .args(&args)
        .output()
        .with_context(|| format!("could not run {program}"))?;
    if out.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&out.stderr).trim().to_owned();
    Err(anyhow::Error::new(Refused {
        why: why_of(&said),
        said,
    }))
}

/// Which refusal `wpctl`'s words are.
///
/// It says `Object '123' not found` for a node that was unplugged between the
/// key going down and the order arriving, which is a thing that happens to
/// anybody who pulls a headset out mid-gesture — and it is a different
/// sentence from "this build has no wpctl".
fn why_of(said: &str) -> Refusal {
    let lower = said.to_lowercase();
    if lower.contains("not found") || lower.contains("does not exist") {
        Refusal::Gone
    } else if lower.contains("invalid") || lower.contains("could not parse") {
        Refusal::Invalid
    } else {
        Refusal::Other
    }
}

/// Open one source and hand back the child spitting raw PCM.
///
/// **This is the only place in the repository that opens a microphone**, and
/// it is called from one place in turn — the meter, which the view starts on
/// the focused row and stops on leaving it.
///
/// 8 kHz, one channel, signed 16-bit: measured at 16 KB/s, which is what a
/// level meter costs. Asking for the device's own rate would be forty times
/// the bytes for a number that is drawn ten times a second.
///
/// # Why it runs under `setpriv --pdeathsig KILL`
///
/// **Because a killed footer left the microphone open.** Measured on this
/// machine: the parent `SIGKILL`ed, and `pw-cat` still sitting in
/// `epoll_wait` with the device held. The stdout pipe losing its reader does
/// NOT end it — it is event-driven and simply waits.
///
/// The `Drop` guard on [`crate::meter::Meter`] covers every ORDINARY path:
/// leaving the row, closing the view, shutting the footer down. It cannot
/// cover a signal, because nothing in the process runs after one.
///
/// `PR_SET_PDEATHSIG` is what the kernel offers, and it has to be set between
/// the fork and the exec. In Rust that is `Command::pre_exec`, which is
/// `unsafe`, in the one place in a program where nothing is safe — so this
/// borrows the terminal's answer to the identical problem (`ronoctua-term`'s
/// `setsid --ctty`): a small binary from util-linux that makes exactly that
/// call and is audited by far more people. One extra `exec` per meter.
///
/// Its absence is NOT fatal: without `setpriv` the recorder still starts and
/// still closes on every ordinary path. What is lost is the signal case, and
/// a machine with no microphone meter at all would be a worse answer.
///
/// # Errors
///
/// When this build has no `pw-cat`, or the program cannot be run.
pub fn record(target: u32) -> Result<std::process::Child> {
    recorder(
        "8000",
        "1",
        &["--target", &target.to_string()],
        "the meter",
        "the footer",
    )
}

/// THE ONE PLACE `--record` IS WRITTEN, for the meter and the studio alike —
/// what opens a microphone is built once (`one-pipewire-boundary`).
///
/// THE SHEPHERD FIRST, WHEN THERE IS ONE. `setpriv` sets the death signal and
/// then EXECS, so the recorder's parent is still this process — the guard
/// costs a process image, not a process.
fn recorder(
    rate: &str,
    channels: &str,
    extra: &[&str],
    what: &str,
    parent: &str,
) -> Result<std::process::Child> {
    let program = PW_CAT.context("this build has no pw-cat")?;
    let (spawn, lead) = match SETPRIV.filter(|p| !p.is_empty()) {
        Some(setpriv) => (setpriv, vec!["--pdeathsig", "KILL", "--", program]),
        None => {
            tracing::warn!("no setpriv: {what} will not be killed by a SIGNAL to {parent}");
            (program, Vec::new())
        }
    };
    std::process::Command::new(spawn)
        .args(lead)
        .args([
            "--record",
            "--format",
            "s16",
            "--rate",
            rate,
            "--channels",
            channels,
        ])
        .args(extra)
        .arg("-")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        // ITS OWN GROUP, the idiom `transfers/media.rs` and the film player
        // both use: a Ctrl+C in whatever terminal started the parent must not
        // reach a child it owns.
        .process_group(0)
        .spawn()
        .with_context(|| format!("could not run {program}"))
}

/// What the studio records: the microphone, or what the machine plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capture {
    /// The default source.
    Microphone,
    /// The default sink's MONITOR — every sound the machine plays.
    Desktop,
}

/// The rate and layout a recording is captured at: what every encoder and
/// every receiving site takes without resampling.
pub const CAPTURE_RATE: u32 = 48_000;
/// See [`CAPTURE_RATE`].
pub const CAPTURE_CHANNELS: u32 = 2;

/// Opens a recording of `what`, signed 16-bit interleaved stereo at 48 kHz on
/// stdout — the SECOND door that opens a microphone, and the same guard
/// [`record`] has: `setpriv --pdeathsig KILL`, its own process group, and a
/// caller that holds the child in a value whose `Drop` reaps it (the studio's
/// `audio.rs`).
///
/// The monitor is `stream.capture.sink`, measured with the pinned `pw-cat`:
/// with it and no target the stream links to the default SINK and records
/// what is played; without it, to the default SOURCE.
///
/// # Errors
///
/// When this build has no `pw-cat`, or the program cannot be run.
pub fn capture(what: Capture) -> Result<std::process::Child> {
    let extra: &[&str] = match what {
        Capture::Microphone => &[],
        Capture::Desktop => &["-P", "{ stream.capture.sink = true }"],
    };
    recorder(
        &CAPTURE_RATE.to_string(),
        &CAPTURE_CHANNELS.to_string(),
        extra,
        "a recording",
        "the studio",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_volume_argument_is_the_perceived_fraction() {
        let args = argv(&Order::Volume {
            node: 60,
            percent: 38,
        })
        .expect("a volume is a wpctl order");
        assert_eq!(args, vec!["set-volume", "60", "0.380000"]);
    }

    #[test]
    fn the_volume_argument_is_never_relative() {
        // `wpctl set-volume 60 5%+` steps. Every argument this builds is an
        // absolute number, so a repeat of the same order is idempotent —
        // which is what holding an arrow down against a slow daemon needs.
        for percent in [0_u8, 5, 100, 150] {
            let args = argv(&Order::Volume { node: 1, percent }).expect("an order");
            let last = args.last().expect("three arguments");
            assert!(
                !last.contains('+') && !last.contains('-') && !last.contains('%'),
                "{last} is relative"
            );
            assert!(last.parse::<f64>().is_ok(), "{last} is not a number");
        }
    }

    #[test]
    fn the_other_orders_name_the_right_thing() {
        // `set-profile` takes a DEVICE and `set-volume` takes a NODE, and the
        // two numbers are different on the same headset — 84 and 83 on the
        // one this repository measured.
        assert_eq!(
            argv(&Order::Profile {
                device: 84,
                index: 2
            }),
            Some(vec![
                "set-profile".to_owned(),
                "84".to_owned(),
                "2".to_owned()
            ])
        );
        assert_eq!(
            argv(&Order::Mute {
                node: 83,
                muted: true
            }),
            Some(vec!["set-mute".to_owned(), "83".to_owned(), "1".to_owned()])
        );
        assert_eq!(
            argv(&Order::Default { node: 60 }),
            Some(vec!["set-default".to_owned(), "60".to_owned()])
        );
        assert_eq!(
            argv(&Order::Route {
                device: 52,
                index: 3
            }),
            Some(vec![
                "set-route".to_owned(),
                "52".to_owned(),
                "3".to_owned()
            ])
        );
    }

    #[test]
    fn listening_is_not_a_wpctl_order() {
        assert_eq!(argv(&Order::Listen(Some(61))), None);
        assert_eq!(argv(&Order::Listen(None)), None);
        // And carrying it out is a no-op rather than a refusal, because the
        // meter is somebody else's job.
        assert!(carry_out(&Order::Listen(None)).is_ok());
    }

    #[test]
    fn a_headset_pulled_out_mid_gesture_is_not_a_broken_build() {
        assert_eq!(why_of("Object '83' not found"), Refusal::Gone);
        assert_eq!(why_of("Invalid volume"), Refusal::Invalid);
        assert_eq!(why_of("something else entirely"), Refusal::Other);
    }
}
