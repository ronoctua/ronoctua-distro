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

/// What ends a monitor that was started to end ([`crate::subscribe`]).
///
/// Raised once, it kills the child that is being read — its output ends, and
/// the reading with it — and wakes the pause between two lives, so the thread
/// leaves at once instead of starting another child.
#[derive(Debug, Default)]
pub(crate) struct Halt {
    raised: std::sync::atomic::AtomicBool,
    /// The child being read now, while there is one.
    child: std::sync::Mutex<Option<std::process::Child>>,
    /// Rung when raised, for the pause between two lives.
    bell: (std::sync::Mutex<()>, std::sync::Condvar),
}

impl Halt {
    /// Ends the monitor: the child killed now, the thread let out of its
    /// pause. The thread reaps the child; nothing here waits for it, because
    /// whoever raises may hold a lock the thread's last reading is waiting on.
    pub(crate) fn raise(&self) {
        self.raised.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Ok(mut child) = self.child.lock()
            && let Some(child) = child.as_mut()
        {
            let _ = child.kill();
        }
        let _held = self.bell.0.lock();
        self.bell.1.notify_all();
    }

    /// Whether it was raised.
    pub(crate) fn raised(&self) -> bool {
        self.raised.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Sleeps `pause`, or less when raised meanwhile.
    fn rest(&self, pause: std::time::Duration) {
        let Ok(held) = self.bell.0.lock() else {
            return;
        };
        let _ = self
            .bell
            .1
            .wait_timeout_while(held, pause, |()| !self.raised());
    }
}

/// Follow the graph for as long as the process lives — or, with a `halt`,
/// until it is raised.
///
/// Without one it never returns. The child is respawned when it dies, because
/// PipeWire being restarted — by an update, by `LEADER s q`, by the rescue
/// console's own "restart audio" — must not leave the screen frozen on the
/// last reading it ever saw. That is the failure that once killed the tunnel
/// thread and left two modeline marks lying for the rest of a session.
pub(crate) fn follow(on_event: &dyn Fn(crate::Event), halt: Option<&Halt>) {
    // A BUILD WITHOUT THE PROGRAM fails every life the same way a machine
    // without PipeWire does, and says the graph is empty the same way.
    let program = PW_DUMP.unwrap_or_default();
    follow_with(program, &["--monitor", "--no-colors"], on_event, halt);
}

/// [`follow`], reading `program args` — the monitor, or in a test a program
/// that prints what it would.
fn follow_with(program: &str, args: &[&str], on_event: &dyn Fn(crate::Event), halt: Option<&Halt>) {
    let mut pause = FIRST_PAUSE;
    let mut failures = 0_u32;
    let mut said_it_was_empty = false;
    let raised = || halt.is_some_and(Halt::raised);
    loop {
        if raised() {
            return;
        }
        let run = run_once(program, args, on_event, halt);
        if raised() {
            return;
        }
        match run {
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
        match halt {
            Some(h) => h.rest(pause),
            None => std::thread::sleep(pause),
        }
        pause = (pause * 2).min(MOST_PAUSE);
    }
}

/// One life of the monitor child. Returns how many readings it reported.
///
/// With a `halt`, the child is kept IN it while it is read, so raising it
/// kills the child from any thread; the reading ends with its output.
fn run_once(
    program: &str,
    args: &[&str],
    on_event: &dyn Fn(crate::Event),
    halt: Option<&Halt>,
) -> Result<u64> {
    let mut child = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("could not run {program} {}", args.join(" ")))?;
    let Some(out) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        anyhow::bail!("the monitor came up with no output to read");
    };
    // HANDED TO THE HALT, and asked again AFTER: a raise that came between
    // the spawn and here found no child to kill, and this one would
    // otherwise be read until it ended by itself.
    let mut own = None;
    match halt {
        Some(h) => {
            if let Ok(mut held) = h.child.lock() {
                *held = Some(child);
            }
            if h.raised()
                && let Ok(mut held) = h.child.lock()
                && let Some(c) = held.as_mut()
            {
                let _ = c.kill();
            }
        }
        None => own = Some(child),
    }

    // THE FRAMING IS `serde_json`'S OWN. `pw-dump --monitor` prints the whole
    // graph and then one array per change, back to back with no separator —
    // and a stream deserialiser is built for exactly that. Counting brackets
    // by hand would be a second framer to keep honest about strings and
    // escapes, for a problem the library already solved.
    let mut graph = crate::graph::Graph::fresh();
    let mut reported = 0_u64;
    let reader = std::io::BufReader::new(out);
    for value in serde_json::Deserializer::from_reader(reader).into_iter::<serde_json::Value>() {
        if halt.is_some_and(Halt::raised) {
            break;
        }
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
    // REAPED HERE, whoever killed it: the halt only kills.
    let child = own.or_else(|| halt.and_then(|h| h.child.lock().ok()?.take()));
    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }
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
    recorder(&record_args(target), "the meter", "the footer")
}

/// What the meter's `pw-cat` is told, after the shepherd — see [`record`].
fn record_args(target: u32) -> Vec<String> {
    recording_args(
        "8000",
        "1",
        &["--target", &target.to_string(), "-P", &own_properties("")],
    )
}

/// The `application.name` every stream this module opens gives itself.
///
/// # Why our streams carry a name of their own
///
/// Because the machine says in the minibuffer when a microphone opens and
/// closes (`ronoctua-shell`'s `events`), and two of the streams that open one
/// are OURS: the sound view's meter, which listens for as long as the cursor
/// rests on a microphone, and the studio's capture, whose recording is an event
/// of its own. Unnamed, both answered to `pw-cat` — the name ANY other
/// program's `pw-cat` or `pw-record` gives too — so telling ours apart meant
/// either missing a stranger or announcing the meter every time the cursor
/// moved. A name only this module writes is the one reading of the graph that
/// cannot mistake one for the other.
pub const OWN_STREAM: &str = "ronoctua-audio";

/// The `-P` of one of our recordings: our name, and whatever else it needs.
///
/// ONE OBJECT, because `pw-cat` merges each `-P` into the stream's properties
/// and a second one would work today and be a second place to look tomorrow.
fn own_properties(more: &str) -> String {
    format!("{{ application.name = \"{OWN_STREAM}\"{more} }}")
}

/// THE ONE PLACE `--record` IS WRITTEN, for the meter and the studio alike —
/// what opens a microphone is built once (`one-pipewire-boundary`).
///
/// A PURE FUNCTION, so what each recording is told is a value a test reads
/// (`every_stream_we_open_says_it_is_ours`): the name that keeps our own
/// streams out of the minibuffer's microphone events is in the ARGUMENTS, and
/// a test of the string alone passed with a caller that had dropped it.
fn recording_args(rate: &str, channels: &str, extra: &[&str]) -> Vec<String> {
    [
        "--record",
        "--format",
        "s16",
        "--rate",
        rate,
        "--channels",
        channels,
    ]
    .into_iter()
    .chain(extra.iter().copied())
    .chain(std::iter::once("-"))
    .map(ToOwned::to_owned)
    .collect()
}

/// Starts `pw-cat` with `args` (`recording_args`).
///
/// THE SHEPHERD FIRST, WHEN THERE IS ONE. `setpriv` sets the death signal and
/// then EXECS, so the recorder's parent is still this process — the guard
/// costs a process image, not a process.
fn recorder(args: &[String], what: &str, parent: &str) -> Result<std::process::Child> {
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
        .args(args)
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
    recorder(&capture_args(what), "a recording", "the studio")
}

/// What the studio's `pw-cat` is told, after the shepherd — see [`capture`].
fn capture_args(what: Capture) -> Vec<String> {
    let properties = own_properties(match what {
        Capture::Microphone => "",
        Capture::Desktop => " stream.capture.sink = true",
    });
    recording_args(
        &CAPTURE_RATE.to_string(),
        &CAPTURE_CHANNELS.to_string(),
        &["-P", &properties],
    )
}

/// The variable naming `pw-play`, read when a chime is asked for.
///
/// # Why the ENVIRONMENT and not the build, as the three above are
///
/// Because the process that chimes is the brain, a daemon whose unit is kept
/// narrow on purpose (`brain-unit-is-narrow`): a path compiled in is a closure
/// reference whether the feature is on or not, and the flake decides where the
/// sound and the player are. The brain's unit is given both, by STORE PATH,
/// by the module that rings.
pub const PW_PLAY_VARIABLE: &str = "RONOCTUA_PW_PLAY";

/// The variable naming the sound a chime plays — a file in the store.
pub const CHIME_VARIABLE: &str = "RONOCTUA_CHIME";

/// Plays the chime once, and returns at once: `pw-play` is started and left
/// to finish on its own, reaped by a thread of its own — a caller on a thread
/// that must never block (the brain's watch) is not held for a second and a
/// half of bell.
///
/// # Errors
///
/// When this process was not given `pw-play` or the sound, or the program
/// cannot be run. A caller says so in its log and goes on: a chime is never
/// the only way anything is told.
pub fn chime() -> Result<()> {
    play(
        std::env::var_os(PW_PLAY_VARIABLE),
        std::env::var_os(CHIME_VARIABLE),
    )
}

/// [`chime`], with the two paths handed in.
fn play(program: Option<std::ffi::OsString>, sound: Option<std::ffi::OsString>) -> Result<()> {
    let program = program.filter(|p| !p.is_empty()).with_context(|| {
        format!("{PW_PLAY_VARIABLE} is not set: nothing to play the chime with")
    })?;
    let sound = sound
        .filter(|s| !s.is_empty())
        .with_context(|| format!("{CHIME_VARIABLE} is not set: no chime to play"))?;
    let mut child = std::process::Command::new(&program)
        .args(chime_args(&sound))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        // ITS OWN GROUP, like every child here: a signal meant for the
        // parent's terminal is not the bell's.
        .process_group(0)
        .spawn()
        .with_context(|| format!("could not run {}", program.to_string_lossy()))?;
    let reaped = std::thread::Builder::new()
        .name("chime".into())
        .spawn(move || {
            let _ = child.wait();
        });
    reaped.context("could not start the thread that waits for the chime")?;
    Ok(())
}

/// What `pw-play` is told: a notification, not music — the role a session's
/// policy may route or duck differently — named as ours, and the file.
fn chime_args(sound: &std::ffi::OsStr) -> Vec<std::ffi::OsString> {
    vec![
        "--media-role".into(),
        "Notification".into(),
        "-P".into(),
        own_properties("").into(),
        sound.to_owned(),
    ]
}

/// Whether a PipeWire server may be listening for this process: the socket a
/// client of it would connect to is on the disk.
///
/// A client asked to connect where there is no socket fails at once, and
/// what fails at once can still hang: mpv's sound output opens a thread loop,
/// fails to connect and stops it in the same instant, and libpipewire 1.6's
/// stop, called before the loop's thread has entered the loop, sets the flag
/// in the CALLER and wakes nobody — the thread sleeps in `epoll_wait` for
/// ever and mpv's core joins it for ever. mpv tries that output again for
/// every file, so on a machine with no server each file is one more throw.
/// Absent is a fact; present is only "may": a socket left by a server that
/// died is still on the disk.
#[must_use]
pub fn server_may_listen() -> bool {
    sockets(|name| std::env::var_os(name)).is_none_or(|paths| paths.iter().any(|p| p.exists()))
}

/// Where libpipewire's client looks for its server
/// (`module-protocol-native/local-socket.c`): `PIPEWIRE_REMOTE`, or
/// `pipewire-0`, in `PIPEWIRE_RUNTIME_DIR`, else `XDG_RUNTIME_DIR`, else
/// `USERPROFILE`, and then in `/run/pipewire`; an absolute name is itself.
/// `None` for an abstract socket (`@…`), which no path can be asked about.
/// Pure but for `var`.
fn sockets(var: impl Fn(&str) -> Option<std::ffi::OsString>) -> Option<Vec<std::path::PathBuf>> {
    use std::path::{Path, PathBuf};
    let remote = var("PIPEWIRE_REMOTE").filter(|r| !r.is_empty());
    let name = remote.unwrap_or_else(|| "pipewire-0".into());
    if name.to_string_lossy().starts_with('@') {
        return None;
    }
    if Path::new(&name).is_absolute() {
        return Some(vec![PathBuf::from(name)]);
    }
    let runtime = var("PIPEWIRE_RUNTIME_DIR")
        .or_else(|| var("XDG_RUNTIME_DIR"))
        .or_else(|| var("USERPROFILE"));
    Some(
        runtime
            .into_iter()
            .map(PathBuf::from)
            .chain([PathBuf::from("/run/pipewire")])
            .map(|dir| dir.join(&name))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the search drifts from libpipewire's own — the runtime
    /// directories in another order, `/run/pipewire` forgotten, a remote's
    /// own name or an absolute one ignored — or an abstract socket, which no
    /// path answers for, is taken for "no server".
    #[test]
    fn the_server_is_looked_for_where_libpipewire_looks() {
        use std::path::PathBuf;
        let with = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| std::ffi::OsString::from(v))
            }
        };
        let paths = |v: &[&str]| Some(v.iter().map(PathBuf::from).collect::<Vec<_>>());
        assert_eq!(
            sockets(with(&[("XDG_RUNTIME_DIR", "/run/user/1000")])),
            paths(&["/run/user/1000/pipewire-0", "/run/pipewire/pipewire-0"])
        );
        assert_eq!(
            sockets(with(&[
                ("XDG_RUNTIME_DIR", "/run/user/1000"),
                ("PIPEWIRE_RUNTIME_DIR", "/run/pw"),
                ("PIPEWIRE_REMOTE", "other"),
            ])),
            paths(&["/run/pw/other", "/run/pipewire/other"])
        );
        assert_eq!(sockets(with(&[])), paths(&["/run/pipewire/pipewire-0"]));
        assert_eq!(
            sockets(with(&[("PIPEWIRE_REMOTE", "/tmp/a.sock")])),
            paths(&["/tmp/a.sock"])
        );
        assert_eq!(sockets(with(&[("PIPEWIRE_REMOTE", "@hidden")])), None);
    }

    #[test]
    fn a_chime_is_a_notification_of_ours_playing_the_file() {
        let args = chime_args(std::ffi::OsStr::new("/nix/store/x-chime/chime.wav"));
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            [
                "--media-role",
                "Notification",
                "-P",
                &own_properties(""),
                "/nix/store/x-chime/chime.wav"
            ]
        );
    }

    #[test]
    fn a_chime_with_nothing_to_play_says_which_is_missing() {
        let player = Some(std::ffi::OsString::from(
            "/nix/store/x-pipewire/bin/pw-play",
        ));
        let sound = Some(std::ffi::OsString::from("/nix/store/x-chime/chime.wav"));
        let said = |p, s| {
            play(p, s)
                .map_err(|e| e.to_string())
                .expect_err("it played")
        };
        assert!(said(None, sound.clone()).contains(PW_PLAY_VARIABLE));
        assert!(said(Some(std::ffi::OsString::new()), sound).contains(PW_PLAY_VARIABLE));
        assert!(said(player, None).contains(CHIME_VARIABLE));
    }

    /// The one `-P` object in a recording's arguments.
    fn properties_of(args: &[String]) -> &str {
        let at = args
            .iter()
            .position(|a| a == "-P")
            .unwrap_or_else(|| panic!("no -P in {args:?}"));
        assert_eq!(
            args.iter().filter(|a| *a == "-P").count(),
            1,
            "one object: {args:?}"
        );
        args.get(at + 1).map_or("", String::as_str)
    }

    /// FAILS IF: one of our recordings — the meter or either of the studio's
    /// — opens without our name, which is what keeps it out of the
    /// minibuffer's microphone events; or the desktop's capture loses the
    /// property that makes it record the sink. Read off the ARGUMENTS each
    /// caller passes, not the string alone.
    #[test]
    fn every_stream_we_open_says_it_is_ours() {
        let ours = format!("application.name = \"{OWN_STREAM}\"");
        for (what, args) in [
            ("the meter", record_args(7)),
            ("the microphone", capture_args(Capture::Microphone)),
            ("the desktop", capture_args(Capture::Desktop)),
        ] {
            // THE SAME RECORDING every caller builds — `recording_args` is the
            // one place its flag is written (`one-pipewire-boundary`).
            assert_eq!(
                args.first(),
                recording_args("1", "1", &[]).first(),
                "{what}"
            );
            assert_eq!(args.last().map(String::as_str), Some("-"), "{what}");
            let p = properties_of(&args);
            assert!(p.contains(&ours), "{what}: {p}");
            assert!(p.starts_with('{') && p.ends_with('}'), "{what}: {p}");
        }
        assert!(
            properties_of(&capture_args(Capture::Desktop)).contains("stream.capture.sink = true")
        );
        assert!(!properties_of(&capture_args(Capture::Microphone)).contains("capture.sink"));
        assert!(
            record_args(7)
                .windows(2)
                .any(|w| w[0] == "--target" && w[1] == "7")
        );
    }

    /// FAILS IF: a monitor that was started to end outlives its halt — its
    /// child still reading the graph after the one who held it let go, or
    /// its thread starting another life of it.
    #[test]
    fn a_halted_monitor_ends_its_child_and_its_thread() {
        // A MONITOR THAT PRINTS AN EMPTY GRAPH AND THEN WAITS, as `pw-dump
        // --monitor` does between two changes.
        let halt = std::sync::Arc::new(Halt::default());
        let (said, heard) = std::sync::mpsc::channel::<usize>();
        let held = halt.clone();
        let thread = std::thread::spawn(move || {
            follow_with(
                "sh",
                &["-c", "echo '[]'; exec sleep 60"],
                &|e| {
                    if let crate::Event::Snapshot(s) = e {
                        let _ = said.send(s.nodes.len());
                    }
                },
                Some(&held),
            );
        });
        assert_eq!(
            heard.recv_timeout(std::time::Duration::from_secs(10)),
            Ok(0),
            "the monitor's first reading never came"
        );
        let began = std::time::Instant::now();
        halt.raise();
        thread.join().expect("the monitor's thread");
        assert!(
            began.elapsed() < std::time::Duration::from_secs(5),
            "the halt waited {:?} — the child was not killed",
            began.elapsed()
        );
        assert!(
            halt.child.lock().expect("the child's lock").is_none(),
            "the child was left behind, unreaped"
        );
        // AND A HALT RAISED BEFORE THE FIRST LIFE starts none.
        let early = Halt::default();
        early.raise();
        let began = std::time::Instant::now();
        follow_with("sh", &["-c", "exec sleep 60"], &|_| {}, Some(&early));
        assert!(began.elapsed() < std::time::Duration::from_secs(1));
    }

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
