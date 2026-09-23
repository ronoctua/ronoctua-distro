//! The update — the mark on the modeline, the door that starts it, and the
//! window that watches it happen.
//!
//! # What this module may NOT know
//!
//! **A step.** Not one id, not one command, not one order. The sequence is
//! declared once, in `assets/updates.nix`, and everything downstream is
//! generated from it: the Rust table, the two text keys per step, the two shell
//! runners and the two systemd units. `ronoctua-brain` reads that table through
//! its `build.rs` — it used to declare three steps of its own, and they came
//! out DISJOINT from the twelve the sequence actually runs — and this module
//! reads the same table through the same crate.
//!
//! So `LEADER s u` TRIGGERS; it never enumerates. What it starts is the unit
//! the brain starts, named by the brain's own constant, and the steps this
//! window draws are [`ronoctua_brain::tasks::update::steps`] and nothing else.
//! `update-has-one-list` is the check that keeps it that way: a step's id
//! written by hand anywhere outside the declaration takes the build down.
//!
//! # Three things live here, and they are one feature
//!
//! 1. **The mark**, twelve frames of a clock hand riding the one-second timer
//!    the footer already beats. It says the machine is still working — a still
//!    glyph and a hung rebuild look identical, and this is the one thing on
//!    this system that legitimately takes twenty minutes.
//! 2. **The door**, [`begin`]. The brain when it is there, systemd when it is
//!    not. The brain is a scheduler, not a gatekeeper.
//! 3. **The window**, which shows the steps with their verdicts and the two
//!    journals live.
//!
//! Together in one file for the reason [`crate::transfers`] gives about the
//! queue: they are the halves of one feature, and splitting them would hide
//! which is which.
//!
//! # Nothing here asks anybody anything
//!
//! There is no polkit agent in this session, so a call that could ask is a call
//! that hangs. Every `systemctl` this module reaches for is
//! [`ronoctua_brain::systemd`]'s, which is `--user --no-block` and nothing else;
//! `journalctl` only reads. The one spawn that is not instant runs on a NAMED
//! thread, never on the loop that draws.

use std::collections::HashMap;
use std::io::BufRead as _;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use ronoctua_brain::tasks::update as declared;
use ronoctua_render::layout;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    output::{OutputHandler, OutputState},
    reexports::calloop::{self, EventLoop, channel, timer},
    reexports::calloop_wayland_source::WaylandSource,
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
    },
    shell::{
        WaylandSurface as _,
        xdg::{
            XdgShell,
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
        },
    },
};
use wayland_client::{
    Connection, Proxy as _, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};

// ---------------------------------------------------------------------------
// THE MARK ON THE MODELINE
// ---------------------------------------------------------------------------

/// The two frames of the mark: the same circle, filled and hollow.
///
/// # Why an arrow in a circle, and why it breathes
///
/// Because it was a clock face and the owner threw it out on sight — a clock
/// says what time it is, and nothing on that line is about the time. An arrow
/// in a circle is what an update is drawn as everywhere one has been drawn.
///
/// It still moves, because a still glyph and a hung update look identical and
/// this is the one thing here that legitimately takes twenty minutes. Filled
/// against hollow is the same shape at the same size, so it breathes rather
/// than twitches — which is what `chrome-icons-match-the-text` is measuring.
///
/// # Why not the pie
///
/// `assets/tokens/icons.nix` argues it where the tokens live, and the
/// distinction is the whole point: `countdown-*` is how much time is LEFT and
/// empties towards a deadline; this is something running whose end nobody
/// knows. Same subject, opposite question, so they are different drawings —
/// and reusing the pie here would promise a finish line the machine cannot see.
pub const FRAMES: [char; 2] = [tokens::icon::WORKING_ONE, tokens::icon::WORKING_TWO];

/// Which frame a tick of the footer's clock draws.
///
/// The tick is the footer's own one-second counter — see the timer in
/// `surface.rs`, which says why a second clock for the same second is refused.
/// It wraps at twelve, so the hand goes round once a minute.
#[must_use]
pub fn frame_of(tick: u64) -> char {
    // `as usize` on a remainder that is already below twelve: the cast cannot
    // truncate, and the modulo is what guarantees it.
    #[allow(clippy::cast_possible_truncation)]
    let n = (tick % FRAMES.len() as u64) as usize;
    FRAMES[n]
}

/// Whether the machine has an update in flight, as the brain reports it.
///
/// # Why the QUEUE counts, and not only what is running
///
/// Because a queued task is one the brain has already decided to run: nobody
/// is going to be asked again, and what it is waiting for is the machine — RAM,
/// processor, mains — which is exactly what `brain-hold-*` exists to say out
/// loud. Taking the mark off screen until the run starts would hide the wait
/// rather than the work, and "a scheduler that silently does nothing is a
/// scheduler nobody trusts" is this crate's own sentence about it.
///
/// The animation is what tells the two apart at a glance: it moves while the
/// unit is producing and it moves while the gate holds, and the window says
/// which. What it never does is stay on screen with nothing pending.
#[must_use]
pub fn running(mind: Option<&ronoctua_brain::Mind>) -> bool {
    let Some(m) = mind else {
        return false;
    };
    m.running == declared::ID || m.queue.iter().any(|q| q == declared::ID)
}

/// The modeline's mark, or `None` when there is nothing to say.
///
/// One cell and one glyph. There is no number beside it on purpose: a
/// percentage of a rebuild is a number nobody can compute honestly, and a step
/// count would put the sequence's ids on the line that has least room for them.
/// What the count is for is the window.
#[must_use]
pub fn mark(mind: Option<&ronoctua_brain::Mind>, tick: u64) -> Option<String> {
    running(mind).then(|| frame_of(tick).to_string())
}

// ---------------------------------------------------------------------------
// THE DOOR — `LEADER s u`
// ---------------------------------------------------------------------------

/// Which door opened when somebody asked for an update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Started {
    /// The brain took it. It goes in the queue, through the gate, with a
    /// countdown, and it is written down in the journal like every other run.
    Brain,
    /// No brain was listening, so systemd was asked straight out.
    Systemd,
    /// Neither door opened, and the screen has to say so.
    Nowhere,
}

/// Asks for a run of the whole sequence.
///
/// # Why the brain FIRST, and why it is not required
///
/// Through the brain the run is queued, held while the machine is busy,
/// announced with its countdown, abortable from the footer's own line and
/// written into the journal — all of which already exists and none of which a
/// second path could inherit. So when the daemon is listening, it decides.
///
/// And when it is not, this still starts. The brain is a scheduler, not a
/// gatekeeper: a machine whose daemon is down is a machine that still has to
/// be able to update itself, and a key that fails because a scheduler is
/// missing is a key that teaches people to use the terminal.
///
/// # Why the probe cannot be slow
///
/// [`ronoctua_brain::client::tell`] connects with `connect_quick`, whose own
/// doc says why: whoever sends a command is the loop that DRAWS, and a connect
/// that could take two seconds because the daemon needs starting is two seconds
/// of frozen footer. `client::watch` is the slow half, and it is on a thread.
///
/// The systemd half goes to a NAMED thread even though `--no-block` returns at
/// once: it is still a spawn, and `systemd-run --scope` already froze this
/// footer once by being synchronous inside a `calloop`.
pub fn begin() -> Started {
    let ask = ronoctua_brain::Ask::Run {
        task: declared::ID.to_owned(),
        rehearsal: false,
    };
    match ronoctua_brain::client::tell(&ask) {
        Ok(()) => {
            tracing::info!(task = declared::ID, "update asked of the brain");
            return Started::Brain;
        }
        Err(e) => tracing::debug!(error = %e, "no brain; asking systemd directly"),
    }
    // THE BUILD'S OWN ANSWER, before anything is spawned. `reachable` is a
    // `const fn` over an `option_env!`: outside Nix nothing baked the path, and
    // saying so is better than a key that fails silently every time.
    if !ronoctua_brain::systemd::reachable() {
        tracing::warn!("this build does not know where systemctl is");
        return Started::Nowhere;
    }
    let born = std::thread::Builder::new()
        .name("update".to_owned())
        .spawn(|| {
            // THE BRAIN'S OWN CALL, and not a copy of it. `--user` is
            // load-bearing and its reason is written where it lives: on the
            // SYSTEM bus `systemctl start` is authorised by polkit BEFORE
            // systemd looks the unit up, and with no agent in this session the
            // call would hang on an authentication request nobody asked for.
            if let Err(e) = ronoctua_brain::systemd::start(declared::UNIT) {
                tracing::error!(unit = declared::UNIT, error = %e, "systemd refused");
            }
        });
    match born {
        Ok(_) => Started::Systemd,
        Err(e) => {
            tracing::error!(error = %e, "could not start the update thread");
            Started::Nowhere
        }
    }
}

// ---------------------------------------------------------------------------
// WHAT THE UNITS REPORTED
// ---------------------------------------------------------------------------

/// The privileged half's unit.
///
/// # Why this name is written here at all
///
/// Because nothing exports it: `nix/modules/update.nix` holds it in a `let`
/// (`systemUnit`) and travels it three ways from there — into the unit, into
/// the polkit rule and into the generated runner. The person's half is
/// [`declared::UNIT`], which the brain owns and this file borrows; the root
/// half has no Rust owner. It is named once, here, and the report at the top of
/// this wave asks for a check that compares it against the module.
const SYSTEM_UNIT: &str = "ronoctua-update-system.service";

/// Where the root half leaves its report.
///
/// `/run`, because none of it survives a boot and none of it should — the
/// `stateDir` of `nix/modules/update.nix`, written `0755` root so the person
/// can READ it, which is what makes this window possible without privilege.
const SYSTEM_REPORT: &str = "/run/ronoctua-update/system.report";

/// The person's half's report, under their own runtime directory.
const PERSON_REPORT: &str = "ronoctua-update/user.report";

/// `journalctl`, by store path and never by name.
///
/// Under a systemd unit the `PATH` is minimal and a bare name fails far from
/// its cause. Outside Nix — `cargo run` — the shell's own `PATH` resolves it,
/// which is the shape `crate::brain` already uses for `systemctl`.
const JOURNALCTL: &str = match option_env!("RONOCTUA_JOURNALCTL") {
    Some(p) => p,
    None => "journalctl",
};

/// What a runner wrote beside a step.
///
/// # A closed vocabulary, and it is not ours
///
/// `nix/scripts/gen-updates.py` emits exactly these words — `off`,
/// `skipped-<kind>`, `rehearsed`, `ran`, `failed`, `stopped` — and its own
/// comment says so: *"Ids and verdicts are English and closed; the sentence on
/// screen comes from a text key, never from here."* [`Verdict::Pending`] is the
/// only one this side adds, and it is the absence of a line rather than a word
/// in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// It ran, and it worked.
    Ran,
    /// It ran, and it did not.
    Failed,
    /// This machine has it switched off, in `/etc/ronoctua/updates.conf`.
    Off,
    /// A precondition did not hold. The word is the KIND — `binary`,
    /// `directory`, `writable-file`, `network` — and it is the tail of
    /// `update-needs-<kind>`, which both languages already carry.
    Skipped(String),
    /// A rehearsal printed it and ran nothing.
    Rehearsed,
    /// The sequence stopped at this point.
    Stopped,
    /// The report has not reached it. Not a word in the file: the absence of
    /// one.
    Pending,
}

impl Verdict {
    /// The key the row's word comes from.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::Ran => "update-verdict-ran",
            Self::Failed => "update-verdict-failed",
            Self::Off => "update-verdict-off",
            // THE ROW SAYS *SKIPPED* AND THE CURSOR SAYS WHY. The kind's own
            // sentence is a whole line long (`update-needs-binary` is *"this
            // program is not on your PATH"*), which is right for the second
            // register and far too wide for a column.
            Self::Skipped(_) => "update-verdict-skipped",
            Self::Rehearsed => "update-verdict-rehearsed",
            Self::Stopped => "update-verdict-stopped",
            Self::Pending => "update-verdict-pending",
        }
    }

    /// The mark in the gutter, and the colour that carries it.
    ///
    /// # Why `off` and `skipped` share a glyph
    ///
    /// Because the MARK answers *did this happen?* — and for both the answer is
    /// no. What differs is why, and why is the word beside it. Two glyphs for
    /// one answer would be two things to learn for a distinction the column
    /// already makes.
    #[must_use]
    pub const fn mark(&self) -> (char, tokens::Color) {
        match self {
            Self::Ran => (tokens::icon::SUCCESS, tokens::color::STATE_SUCCESS),
            Self::Failed | Self::Stopped => (tokens::icon::ERROR, tokens::color::STATE_ERROR),
            Self::Rehearsed => (tokens::icon::INFO, tokens::color::TEXT_SECONDARY),
            Self::Off | Self::Skipped(_) => (tokens::icon::REMOVE, tokens::color::TEXT_DIM),
            // An empty box: the step exists and has not been reached.
            Self::Pending => (tokens::icon::PROGRESS_LEFT, tokens::color::TEXT_DIM),
        }
    }
}

/// One word of the report, or `None` when it is not one of the closed set.
///
/// An unknown word is NOT turned into a verdict. A silent fallback here would
/// draw *"it worked"* over a run that did something else — the failure mode
/// `AGENTS.md` names as needing a check rather than a shrug.
#[must_use]
pub fn verdict_of(word: &str) -> Option<Verdict> {
    match word {
        "ran" => Some(Verdict::Ran),
        "failed" => Some(Verdict::Failed),
        "off" => Some(Verdict::Off),
        "rehearsed" => Some(Verdict::Rehearsed),
        "stopped" => Some(Verdict::Stopped),
        other => other
            .strip_prefix("skipped-")
            .filter(|kind| !kind.is_empty())
            .map(|kind| Verdict::Skipped(kind.to_owned())),
    }
}

/// What one report file says, id by id.
///
/// One line per step, tab separated, `<id> <verdict> <seconds>` — the shape
/// `gen-updates.py`'s `record()` writes. A line that is not that shape is
/// logged and dropped: the file is APPENDED to while the sequence runs, so a
/// read can land in the middle of one, and the next beat has the whole of it.
#[must_use]
pub fn read_report(text: &str) -> Vec<(String, Verdict, u32)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut columns = line.split('\t');
        let (Some(id), Some(word), Some(seconds)) =
            (columns.next(), columns.next(), columns.next())
        else {
            continue;
        };
        let Some(verdict) = verdict_of(word) else {
            tracing::debug!(word, "a verdict I do not know in the report");
            continue;
        };
        out.push((id.to_owned(), verdict, seconds.parse().unwrap_or(0)));
    }
    out
}

/// The two report files, in the order the sequence writes them.
fn report_paths() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from(SYSTEM_REPORT)];
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        paths.push(PathBuf::from(runtime).join(PERSON_REPORT));
    }
    paths
}

/// Every verdict this machine has on record, from both halves at once.
///
/// # Why a map and not two lists
///
/// Because a step belongs to exactly one half — the generator refuses a
/// sequence that interleaves them — so the union cannot collide, and the
/// window draws ONE table in the declaration's order. Two lists would be two
/// orders to reconcile on screen for a sequence that has only one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Verdicts {
    by_step: HashMap<String, (Verdict, u32)>,
}

impl Verdicts {
    /// Builds it from the text of both files.
    #[must_use]
    pub fn of(halves: &[String]) -> Self {
        let mut by_step = HashMap::new();
        for text in halves {
            for (id, verdict, seconds) in read_report(text) {
                // THE LAST LINE WINS. The runner appends, and a step that was
                // rehearsed and then run for real leaves two lines; what
                // happened is the second.
                by_step.insert(id, (verdict, seconds));
            }
        }
        Self { by_step }
    }

    /// Reads both files off the disk. Absent is empty, which is what a machine
    /// that has never updated looks like.
    #[must_use]
    pub fn read() -> Self {
        let halves: Vec<String> = report_paths()
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap_or_default())
            .collect();
        Self::of(&halves)
    }

    /// What became of one step, and how long it took.
    #[must_use]
    pub fn of_step(&self, id: &str) -> (Verdict, u32) {
        self.by_step
            .get(id)
            .cloned()
            .unwrap_or((Verdict::Pending, 0))
    }
}

/// When the reports last changed, so a beat can decide whether to re-read.
///
/// One `stat` per file per second — the device the file manager already uses to
/// notice a directory moved. Re-reading unconditionally would parse both files
/// once a second for a machine that updates once a week.
fn report_stamp() -> Vec<Option<std::time::SystemTime>> {
    report_paths()
        .iter()
        .map(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
        .collect()
}

// ---------------------------------------------------------------------------
// THE OUTPUT, LIVE
// ---------------------------------------------------------------------------

/// How many lines of output the window keeps.
///
/// # Why two thousand, and why a ceiling at all
///
/// A `nixos-rebuild switch` of this workspace prints on the order of a few
/// thousand lines, and a window that kept all of them would be a memory leak
/// with a user interface — twenty minutes of rebuild, growing. Two thousand
/// lines at the length machine output actually has is a few hundred kibibytes,
/// which is the size of ONE of the file manager's preview thumbnails, and it is
/// some fifty screenfuls to page back through.
///
/// A ring and not a truncation, because what matters after a failed rebuild is
/// the TAIL: nix prints the last lines of the failing log, and those are the
/// last thing written.
pub const KEPT: usize = 2000;

/// The output, bounded, with one flag saying whether the screen owes a frame.
///
/// # Why the flag, and why a redraw per line is a frozen window
///
/// Because a rebuild prints thousands of lines in a burst, and painting once
/// per line is the defect `docs/ARCHITECTURE.md` records under *"the loop
/// waited past the version from BEFORE its own pass"*: the brain's daemon
/// published on every pass, the loop that draws was woken 248 088 times in
/// 300 ms, and what the owner saw was a window that would not resize. Same
/// failure, different producer.
///
/// So arrival and painting are separated: every line that has arrived is drained
/// into here, and the frame is owed ONCE, after the event queue is empty — the
/// shape `Footer::owes_paint` already has.
#[derive(Debug, Clone, Default)]
pub struct Scrollback {
    said: std::collections::VecDeque<String>,
    owes_a_frame: bool,
}

impl Scrollback {
    /// Takes one line, dropping the oldest when the ceiling is reached.
    pub fn push(&mut self, line: String) {
        if self.said.len() >= KEPT {
            self.said.pop_front();
        }
        self.said.push_back(line);
        self.owes_a_frame = true;
    }

    /// Whether a frame is owed, and clears the debt.
    pub const fn owes_a_frame(&mut self) -> bool {
        let owed = self.owes_a_frame;
        self.owes_a_frame = false;
        owed
    }

    /// How many lines are kept.
    #[must_use]
    pub fn amount(&self) -> usize {
        self.said.len()
    }

    /// Whether anything has arrived at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }

    /// `room` lines ending `up` lines above the tail.
    #[must_use]
    pub fn window(&self, up: usize, room: usize) -> Vec<&str> {
        let end = self.said.len().saturating_sub(up);
        let start = end.saturating_sub(room);
        self.said.range(start..end).map(String::as_str).collect()
    }
}

/// Machine output, made safe for a grid.
///
/// # Why this is not paranoia
///
/// The journal carries whatever the unit wrote, and what the unit runs is
/// `nix`, which emits ANSI escapes whenever it thinks it has a terminal, plus
/// tabs and carriage returns. Drawn as they are, an escape sequence becomes a
/// row of tofu that hides the line it was decorating — and the line it was
/// decorating is usually the error.
///
/// A tab becomes ONE space and not eight: this is a grid whose columns belong
/// to the table above, and a line that jumps eight cells reads as an indent
/// that means something.
#[must_use]
pub fn readable(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // CSI and the two-character escapes alike: swallow the introducer,
            // then everything up to and including the byte that ends it.
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if ('\u{40}'..='\u{7e}').contains(&next) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
            continue;
        }
        if c == '\t' {
            out.push(' ');
            continue;
        }
        if c.is_control() {
            continue;
        }
        out.push(c);
    }
    out
}

/// Starts one `journalctl -f` and reads it on a thread of its own.
///
/// # Why a child process and not a poll
///
/// Because following is what `journalctl` is for: it blocks until there is a
/// line, which costs nothing while nothing happens, whereas `journalctl` on a
/// clock is a process spawned per beat to parse prose upstream is free to
/// reword. The brain says the same about `systemctl show` in a loop.
///
/// # Why the pipe is never read on the loop that draws
///
/// A blocking `read_line` in `calloop` is a frozen window for as long as the
/// rebuild is silent — which, in the middle of compiling a kernel, is minutes.
/// The thread reads and the channel wakes the loop, the shape
/// `crate::apps::painter` and `crate::transfers::listen` already have.
///
/// The [`std::process::Child`] comes back so the window can KILL it: `-f`
/// never ends on its own, and a door that leaves two `journalctl` behind every
/// time it is closed is a door that leaks processes.
fn follow(
    unit: &str,
    of_the_person: bool,
    to: &channel::Sender<String>,
) -> Option<std::process::Child> {
    let mut command = std::process::Command::new(JOURNALCTL);
    if of_the_person {
        command.arg("--user");
    }
    command
        .args([
            "--no-pager",
            // Just the message. The window has narrow columns and the stamp,
            // the host and the unit are the same on every line of a run.
            "-o",
            "cat",
            "-n",
            &KEPT.to_string(),
            "-f",
            "-u",
            unit,
        ])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            // NOT FATAL. Without it the window loses the live half and still
            // draws the steps and their verdicts — the same criterion
            // `transfers::listen` uses about a missing socket.
            tracing::warn!(unit, error = %e, "no journalctl; the live output does not appear");
            return None;
        }
    };
    let out = child.stdout.take()?;
    let sender = to.clone();
    let named = std::thread::Builder::new()
        .name(format!("journal-{unit}"))
        .spawn(move || {
            for line in std::io::BufReader::new(out).lines() {
                let Ok(text) = line else {
                    return;
                };
                if sender.send(readable(&text)).is_err() {
                    // The other end has gone: the window is closing.
                    return;
                }
            }
        });
    if let Err(e) = named {
        tracing::error!(unit, error = %e, "could not start the journal thread");
        let _ = child.kill();
        return None;
    }
    Some(child)
}

// ---------------------------------------------------------------------------
// THE ROWS
// ---------------------------------------------------------------------------

/// One drawn piece of a row, in cells.
///
/// The same shape [`crate::brain::Piece`] has, and for the same reason: the
/// layout is `(what there is, width) -> pieces with columns`, which is testable
/// with no compositor and is integer arithmetic over the cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    /// First cell, counted from the row's own start.
    pub at: u16,
    /// What is drawn, already elided.
    pub text: String,
    /// Which token colours it.
    pub color: tokens::Color,
}

/// The row's own gutter — the mark's cell.
const GUTTER: u16 = tokens::space::FILES_GUTTER;

/// The air kept at the right edge of a row.
const EDGE_RIGHT: u16 = tokens::space::FILES_EDGERIGHT;

/// The gap between one column of a row and the next.
///
/// `notifications.rs` already spends this token on exactly that, and it is the
/// same number for the same reason: two cells is what reads as "these are two
/// things" without reading as a margin.
const COLUMN_GAP: u16 = tokens::space::MODELINE_SEGMENTGAP;

/// How long a step took, as a clock and not as a sentence.
///
/// `15:00` needs no language and no key — digits and a colon are the same in
/// both, which is the same argument the brain's screen makes for drawing its
/// dates ISO rather than inventing *19 Aug* against *19 ago*.
#[must_use]
pub fn clock_of(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// One step's row.
///
/// Two phases, the arithmetic the rest of the system uses: the right-hand
/// columns take their widths from the edge inwards, and the name gets what is
/// left. Nothing overflows because nothing is placed past what was subtracted
/// first.
#[must_use]
pub fn step_row(
    step: &ronoctua_brain::Step,
    verdict: &Verdict,
    seconds: u32,
    columns: u16,
    texts: &ronoctua_i18n::Texts,
) -> Vec<Piece> {
    let mut out = Vec::with_capacity(4);
    let (glyph, colour) = verdict.mark();
    out.push(Piece {
        at: GUTTER,
        text: glyph.to_string(),
        color: colour,
    });

    let mut right = columns.saturating_sub(EDGE_RIGHT);
    // THE WORD, CAPPED AT A THIRD OF THE LINE. A verdict is a short phrase and
    // the name is the subject; letting the phrase take the row would leave the
    // subject elided to nothing on a narrow window.
    let word = layout::elide_end(&texts.t(verdict.key()), (columns / 3).max(1));
    right = right.saturating_sub(layout::width(&word));
    out.push(Piece {
        at: right,
        text: word,
        color: colour,
    });
    // AND HOW LONG IT TOOK, only where that is a fact. Zero seconds against a
    // step that never ran is not "it was instantaneous".
    if matches!(verdict, Verdict::Ran | Verdict::Failed) {
        let elapsed = clock_of(seconds);
        right = right.saturating_sub(COLUMN_GAP + layout::width(&elapsed));
        out.push(Piece {
            at: right,
            text: elapsed,
            color: tokens::color::TEXT_DIM,
        });
    }

    // `update-<id>` AND NOT A FAMILY OF OUR OWN. The ids are not this shell's:
    // they come from `assets/updates.nix`, which already owes every one of them
    // a name and a sentence in both languages — the same keys the brain's own
    // screen draws. A second family for the same twelve steps would be twelve
    // more sentences to keep in step with these.
    let at = GUTTER + COLUMN_GAP;
    let room = right.saturating_sub(at.saturating_add(COLUMN_GAP));
    out.push(Piece {
        at,
        text: layout::elide_end(&texts.t(&format!("update-{}", step.id)), room),
        color: if step.on_by_default {
            tokens::color::TEXT_PRIMARY
        } else {
            tokens::color::TEXT_SECONDARY
        },
    });
    out
}

/// The second register: what the step under the cursor actually is.
///
/// The rule the disk formatter's two menus already follow — every row carries a
/// quieter line saying what it really means — and here it carries three things
/// a row cannot: the sentence the declaration owes the step, whether it runs as
/// root, and, for a skip, WHICH precondition did not hold.
#[must_use]
pub fn cursor_explains(
    step: &ronoctua_brain::Step,
    verdict: &Verdict,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let mut said = texts.t(&format!("update-does-{}", step.id));
    if let Verdict::Skipped(kind) = verdict {
        said.push(' ');
        said.push_str(&texts.t(&format!("update-needs-{kind}")));
    }
    if step.needs_root {
        said.push(' ');
        said.push_str(&texts.t(ronoctua_i18n::key::BRAIN_NEEDS_ROOT));
    }
    said
}

// ---------------------------------------------------------------------------
// THE WINDOW
// ---------------------------------------------------------------------------

/// The window's `app_id`.
///
/// Its own, like the file manager's and the terminal's: it is by this that the
/// modeline and the VM test tell one window from another, and `mode-update` is
/// the key `daemon::mode_label` looks up for it.
pub const APP_ID: &str = "br.ronoctua.update";

/// `BTN_LEFT`, from `linux/input-event-codes.h`.
const LEFT_BUTTON: u32 = 0x110;

/// The first row of the step table. Above it: the heading, and the line that
/// explains what the cursor is on.
///
/// The same arrangement as the two other own-window doors, and repeated
/// deliberately: a person should not have to learn a second shape for the same
/// thing.
const FIRST_OF_STEPS: u16 = 2;

/// Where content begins, in cells from the window's edge.
///
/// [`crate::chrome::FIELDS_LEAD`], because every row here carries its own first
/// cell — the mark's, and the air its glyph needs. What this door owes is the
/// DIFFERENCE up to the content's inset, and the constant is where the three
/// readers of that difference are named.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// How often the reports are looked at.
///
/// One second, the footer's own beat, and one `stat` per file: the sequence
/// rewrites the report as each step ends, and a table that only moved when a
/// key was pressed would be a table that lies quietly.
const BEAT: std::time::Duration = std::time::Duration::from_secs(1);

/// Opens the window that watches an update happen.
///
/// # Errors
///
/// When there is no Wayland compositor, or the compositor offers no
/// `xdg_shell`.
pub fn host_it() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("there is no reachable Wayland compositor (is WAYLAND_DISPLAY set?)")?;
    let (globals, queue) = registry_queue_init(&conn).context("Wayland's registry")?;
    let qh: QueueHandle<UpdateWindow> = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).context("no wl_compositor")?;
    let xdg = XdgShell::bind(&globals, &qh).context("the compositor does not expose xdg_shell")?;
    let texts = crate::language::texts();

    let surface_of = compositor.create_surface(&qh);
    // `RequestServer`: decoration belongs to the COMPOSITOR (invariant 4).
    let window = xdg.create_window(surface_of, WindowDecorations::RequestServer, &qh);
    // `*name*` is Emacs's convention for a buffer that is not a file, and the
    // name is the command's own label — one sentence, two doors.
    window.set_title(format!(
        "*{}*",
        texts.t(ronoctua_i18n::key::CMD_SYSTEM_UPDATE)
    ));
    window.set_app_id(APP_ID);
    window.commit();

    let mut cycle: EventLoop<UpdateWindow> = EventLoop::try_new().context("calloop")?;
    let handle = cycle.handle();

    // THE TWO JOURNALS, each on a thread of its own.
    //
    // The root half runs first and the person's half BLOCKS on it — `systemctl
    // start` on a `Type=oneshot` returns when the work is done — so the two are
    // never producing at the same time except at the hand-over. Arrival order
    // is the sequence's order, and nothing here claims more than that.
    let (sender, said) = channel::channel::<String>();
    let mut children = Vec::new();
    children.extend(follow(SYSTEM_UNIT, false, &sender));
    children.extend(follow(declared::UNIT, true, &sender));
    handle
        .insert_source(said, |event, (), state: &mut UpdateWindow| {
            if let channel::Event::Msg(line) = event {
                // NOTHING IS PAINTED HERE. Every line that has arrived lands in the
                // scrollback, and the frame is owed once the queue is empty — see
                // `Scrollback` for what a redraw per line costs.
                state.scrollback.push(line);
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the journal: {e}"))?;

    handle
        .insert_source(
            timer::Timer::from_duration(BEAT),
            |_, (), state: &mut UpdateWindow| {
                state.reread_if_moved();
                timer::TimeoutAction::ToDuration(BEAT)
            },
        )
        .map_err(|e| anyhow::anyhow!("could not hang the clock: {e}"))?;

    // THE THEME'S FILE, watched, for the same reason as every other door.
    let (theme_sender, theme_changed) = channel::channel::<()>();
    let _theme_watch = crate::theme::watch(move || theme_sender.send(()).is_ok());
    handle
        .insert_source(theme_changed, |event, (), state: &mut UpdateWindow| {
            if let channel::Event::Msg(()) = event
                && crate::theme::reload()
            {
                state.ask_paint();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the theme channel: {e}"))?;

    // AND THE SIZE OF THE TEXT, watched for the same reason and by the same
    // means. A size chosen anywhere moves this window without it being
    // reopened — and without it this door would sit beside the footer at a
    // different size, which is the photograph that made this system have one
    // size at all.
    let (size_sender, size_changed) = channel::channel::<()>();
    let _size_watch = crate::toolkits::watch(move || size_sender.send(()).is_ok());
    handle
        .insert_source(size_changed, |event, (), state: &mut UpdateWindow| {
            if let channel::Event::Msg(()) = event
                && crate::toolkits::reload()
            {
                state.text_size_changed();
            }
        })
        .map_err(|e| anyhow::anyhow!("could not hang the text size channel: {e}"))?;

    let mut state = UpdateWindow {
        conn: conn.clone(),
        handle: handle.clone(),
        _theme_watch,
        registry: RegistryState::new(&globals),
        outputs: OutputState::new(&globals, &qh),
        seats: SeatState::new(&globals, &qh),
        _compositor: compositor,
        keyboard: None,
        pointers: crate::seats::Seats::default(),
        here: PerSeat::default(),
        mods: Modifiers::default(),
        window,
        width: 0,
        height: 0,
        asked: (0, 0),
        // ONE TO ONE UNTIL A SCREEN SAYS OTHERWISE: the surface exists before
        // the compositor has said which output it is on, and
        // `scale_factor_changed` is what says so.
        scale: 1,
        scale_told: 1,
        columns: 80,
        lines: 24,
        quit: false,
        owed: false,
        gpu: None,
        text: None,
        quads: None,
        texts,
        steps: declared::steps(),
        verdicts: Verdicts::read(),
        stamp: report_stamp(),
        scrollback: Scrollback::default(),
        cursor: 0,
        step_offset: 0,
        output_up: 0,
        children,
    };

    WaylandSource::new(conn, queue)
        .insert(cycle.handle())
        .map_err(|e| anyhow::anyhow!("could not hang Wayland: {e}"))?;

    let signal = cycle.get_signal();
    cycle
        .run(None, &mut state, |state| {
            // THE FRAME OWED, once the queue is empty. A thousand lines of a
            // rebuild arriving in one burst are one paint, not a thousand —
            // and so is a held key, which is what `settle` added.
            state.settle();
            if state.quit {
                signal.stop();
            }
        })
        .context("event loop")?;
    // WHAT I STARTED, I STOP. `journalctl -f` never ends on its own.
    for mut child in state.children.drain(..) {
        let _ = child.kill();
        let _ = child.wait();
    }
    Ok(())
}

/// The window.
struct UpdateWindow {
    conn: Connection,
    /// The loop, for the ONE thing that needs it here: a keyboard that
    /// REPEATS. SCTK only generates repeat events for a keyboard made with
    /// `get_keyboard_with_repeat`, and that wants a `LoopHandle` — so this is
    /// carried rather than reached for. Without it `repeat_key` below is dead
    /// code and holding an arrow moves exactly one row, which is the trap
    /// AGENTS.md records and which nine windows inherited.
    handle: calloop::LoopHandle<'static, Self>,
    /// The watch on `theme.json` — the file is the broadcast (see `theme.rs`).
    _theme_watch: Option<ronoctua_files::notice::Notice>,
    registry: RegistryState,
    outputs: OutputState,
    seats: SeatState,
    _compositor: CompositorState,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// Every pointer this window was offered — one per cursor — with the
    /// state of each while another is `here`. See `crate::seats`.
    pointers: crate::seats::Seats<PerSeat>,
    /// The state of the pointer whose events are being handled now.
    here: PerSeat,
    mods: Modifiers,
    window: Window,
    /// The window's size in LOGICAL pixels — what the compositor configured
    /// it to, what the columns are counted from, what the pointer arrives in.
    width: u32,
    /// See the field above.
    height: u32,
    /// The logical size the COMPOSITOR configured, before any clamp.
    ///
    /// `width`/`height` are what the BUFFER can show — which is what the
    /// columns, the bars and the hit tests are laid out against; this is what
    /// the GPU is asked for. They are the same pair on every machine anybody
    /// has, and they come apart exactly at the GPU's texture ceiling. Keeping
    /// only one of them is a ratchet: a window clamped once at scale 3 would
    /// ask for the size it was cut down to and could never grow back. See
    /// [`crate::hidpi`].
    asked: (u32, u32),
    /// How many physical pixels one logical pixel is on the screen this
    /// window is on — see [`crate::hidpi`], where the rule and its
    /// measurement live.
    scale: i32,
    /// What the SURFACE has been told, which lags `scale` by one frame.
    ///
    /// `set_buffer_scale` has to travel in the commit that carries a buffer of
    /// the matching size, and the only such commit is the present inside
    /// `Gpu::frame` — see [`crate::hidpi::told_before_present`].
    scale_told: i32,
    columns: u16,
    lines: u16,
    quit: bool,
    /// Whether a frame is owed.
    ///
    /// # Why nothing here paints straight away
    ///
    /// The scrollback already had this shape for the lines a rebuild pours in
    /// — a thousand of them in one burst are one paint. The HANDLERS did not:
    /// a key and a click painted where they stood, and presentation blocks on
    /// the vertical blank, so a held key waited for the monitor once per
    /// repeat.
    ///
    /// Now there is one debt for both, drained in the one place that knows the
    /// batch is finished. Same shape as `files_window` and `Footer`.
    owed: bool,
    gpu: Option<ronoctua_render::gpu::Gpu>,
    text: Option<ronoctua_render::Text>,
    quads: Option<ronoctua_render::quad::Quads>,
    texts: ronoctua_i18n::Texts,
    /// The declared sequence, from the one table there is.
    steps: &'static [ronoctua_brain::Step],
    /// What the two halves reported, merged.
    verdicts: Verdicts,
    /// When each report file last moved.
    stamp: Vec<Option<std::time::SystemTime>>,
    /// The output, bounded.
    scrollback: Scrollback,
    /// Which step the second register is explaining.
    cursor: usize,
    /// The first visible step, when they do not all fit.
    step_offset: usize,
    /// How many lines above the tail the output is showing. Zero follows.
    output_up: usize,
    /// The two `journalctl -f`, so they can be killed.
    children: Vec<std::process::Child>,
}

/// What this window keeps for ONE pointer — see `crate::seats`.
#[derive(Debug, Default)]
struct PerSeat {
    /// Is the pointer dragging the output's bar?
    ///
    /// Held, because a drag is three events: the press that grabs it, every
    /// motion until the release, and the release.
    dragging: bool,
    /// How many LINES the wheel and the touchpad have asked for and not yet
    /// been paid.
    ///
    /// LINES, and the unit is in the name: the footer's accumulator holds
    /// DETENTS. See `surface::wheel_arrived` — spending only the whole part and
    /// keeping the rest is what makes a slow gesture move at the speed of the
    /// finger instead of standing still and then jumping.
    lines_owed: f64,
    row_under_pointer: Option<usize>,
}

impl UpdateWindow {
    /// How many rows the step table gets.
    ///
    /// # Why half and not all of them
    ///
    /// Because the window is two things at once and neither may starve the
    /// other: twelve steps on a twenty-four-line window would leave the output
    /// nine rows, which is not enough to read an error out of. Half each, and
    /// the table scrolls when it does not fit — which on any window worth
    /// opening it does.
    fn steps_room(&self) -> usize {
        let below = self.lines.saturating_sub(FIRST_OF_STEPS);
        usize::from(below / 2).min(self.steps.len())
    }

    /// The first row of the output region.
    fn output_top(&self) -> u16 {
        FIRST_OF_STEPS + u16::try_from(self.steps_room()).unwrap_or(0)
    }

    /// How many rows of output fit.
    fn output_room(&self) -> usize {
        usize::from(self.lines.saturating_sub(self.output_top()))
    }

    /// Keeps the step cursor on screen.
    fn follow_the_cursor(&mut self) {
        let room = self.steps_room();
        if room == 0 {
            return;
        }
        if self.cursor < self.step_offset {
            self.step_offset = self.cursor;
        } else if self.cursor >= self.step_offset + room {
            self.step_offset = self.cursor + 1 - room;
        }
        self.step_offset = self.step_offset.min(self.steps.len().saturating_sub(room));
    }

    /// Keeps the output's offset inside what there is to show.
    fn follow_the_tail(&mut self) {
        let most = self.scrollback.amount().saturating_sub(self.output_room());
        self.output_up = self.output_up.min(most);
    }

    /// Re-reads the reports when a file moved, and only then.
    fn reread_if_moved(&mut self) {
        let now = report_stamp();
        if now == self.stamp {
            return;
        }
        self.stamp = now;
        self.verdicts = Verdicts::read();
        self.ask_paint();
    }

    /// The size of the text changed under this door.
    ///
    /// THE SHAPER HAS TO HEAR IT TOO. It was built with a size and keeps it;
    /// moving everything around it — the columns, the cell, every hit test —
    /// and leaving the letters gives a grid at one size with text at another,
    /// which reads as a broken font rather than as a setting that took.
    fn text_size_changed(&mut self) {
        if let Some(t) = self.text.as_mut() {
            t.draw_at(ronoctua_tokens::text::size());
        }
        self.recount();
        self.ask_paint();
    }

    /// Columns and rows from the size in pixels.
    ///
    /// Integer arithmetic, without measuring text: the cell is exactly 0.600 ×
    /// the font size, including in the Nerd Fonts.
    fn recount(&mut self) {
        let cell = tokens::text::size() * tokens::font::CELL_WIDTH_EM;
        let line = tokens::metrics::line_height(tokens::text::size());
        #[allow(clippy::cast_precision_loss)]
        let width = self.width as f32 - 2.0 * cell * f32::from(CONTENT_MARGIN);
        #[allow(clippy::cast_precision_loss)]
        let height = self.height as f32;
        self.columns = if cell > 0.0 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = (width / cell).floor().max(1.0) as u16;
            n
        } else {
            1
        };
        self.lines = if line > 0.0 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let n = (height / line).floor().max(1.0) as u16;
            n
        } else {
            1
        };
        self.follow_the_cursor();
        self.follow_the_tail();
    }

    /// Which step row a pointer position lands on, if any.
    fn at(&self, x: f64, y: f64) -> Option<usize> {
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line_px <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_px) as u16;
        if line < FIRST_OF_STEPS || line >= self.output_top() {
            return None;
        }
        let row = self.step_offset + usize::from(line - FIRST_OF_STEPS);
        (row < self.steps.len()).then_some(row)
    }

    /// A key, through the one place this window decides anything.
    fn press(&mut self, key: &ronoctua_keymap::Key) {
        use ronoctua_keymap::Sym;
        let page = self.output_room().max(1);
        match key.sym {
            Sym::Esc => self.quit = true,
            Sym::Char('q') => self.quit = true,
            // THE STEP CURSOR. What moves with it is the second register, which
            // is the reason the cursor exists at all.
            Sym::Down | Sym::Char('j') if !key.mods.ctrl => {
                self.cursor = (self.cursor + 1).min(self.steps.len().saturating_sub(1));
                self.follow_the_cursor();
            }
            Sym::Up | Sym::Char('k') if !key.mods.ctrl => {
                self.cursor = self.cursor.saturating_sub(1);
                self.follow_the_cursor();
            }
            // AND THE OUTPUT, under `Ctrl`. The same key with a modifier and
            // not a second pair of letters: the two halves are one window, and
            // the hand should not have to move to change which it scrolls.
            //
            // WHAT THE HINT TEACHES IS `C-j` AND `C-k`, THE LETTERS. The arrow
            // is accepted beside them and is deliberately not advertised: the
            // compositor takes `Ctrl` with a SIDE arrow for paging and a window
            // does not track it, which is why the file manager's own commands
            // are letters (`C-o`, `C-l`, `C-e`, `C-u`). Teaching a key that a
            // chord above us might swallow is teaching a key that sometimes
            // does nothing, which is the one failure mode this project treats
            // as worse than a missing feature.
            Sym::Down | Sym::Char('j') => {
                self.output_up = self.output_up.saturating_sub(page);
                self.follow_the_tail();
            }
            Sym::Up | Sym::Char('k') => {
                self.output_up = self.output_up.saturating_add(page);
                self.follow_the_tail();
            }
            // BACK TO THE END, which is where a live run is. `G` is vi's, and
            // the output is the only thing here long enough to get lost in.
            Sym::Char('G') => self.output_up = 0,
            _ => return,
        }
        self.ask_paint();
    }

    fn assemble_gpu(&mut self) {
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        let (Some(disp), Some(surf)) = (
            std::ptr::NonNull::new(self.conn.backend().display_ptr().cast::<std::ffi::c_void>()),
            std::ptr::NonNull::new(
                self.window
                    .wl_surface()
                    .id()
                    .as_ptr()
                    .cast::<std::ffi::c_void>(),
            ),
        ) else {
            tracing::error!("null Wayland pointers");
            return;
        };
        // SAFETY: `conn` and `window` are fields of `self`, like the `Gpu` — the
        // handles do not outlive what produced them.
        // ONE CALL, not the same one twice: two calls to a pure
        // function are two places the arguments can be edited apart.
        let first = crate::hidpi::physical(self.asked, self.scale);
        #[allow(unsafe_code)]
        let gpu = unsafe {
            ronoctua_render::gpu::Gpu::new(
                RawDisplayHandle::Wayland(WaylandDisplayHandle::new(disp)),
                RawWindowHandle::Wayland(WaylandWindowHandle::new(surf)),
                first.0,
                first.1,
            )
        };
        let Ok(mut g) = gpu else {
            tracing::error!("no usable GPU; the window will not draw");
            return;
        };
        // AND THE FIRST BUFFER IS DIVISIBLE TOO. `Gpu::new` clamps at the
        // GPU's ceiling like `resize` does, and the first frame sends the
        // scale — see `crate::hidpi::resized`.
        let (shown_w, shown_h) = crate::hidpi::resized(&mut g, self.asked, self.scale);
        self.width = shown_w;
        self.height = shown_h;
        // AND THE COLUMNS ARE COUNTED AGAIN. `configure` counts them before the
        // GPU exists, against the size the compositor gave; if the buffer came
        // back smaller than that, the count that is on file is for a picture
        // wider than the one this door draws into.
        self.recount();
        // THE SYSTEM'S ONE SIZE — see `ronoctua_tokens::text::size`. It used to be
        // `SIZE_LARGE`, on the argument that the footer is peripheral
        // reference and this is what one reads.
        match ronoctua_render::Text::new(
            &g.device,
            &g.queue,
            g.config.format,
            self.scale,
            tokens::text::size(),
        ) {
            Ok(t) => self.text = Some(t),
            Err(e) => tracing::error!(error = %e, "assembling the text failed"),
        }
        match ronoctua_render::quad::Quads::new(&g.device, g.config.format) {
            Ok(q) => self.quads = Some(q),
            Err(e) => tracing::error!(error = %e, "the quad pipeline failed"),
        }
        self.gpu = Some(g);
    }

    /// One key, from a press or from a REPEAT.
    ///
    /// Both doors of every view take this path: SCTK delivers a repeat
    /// through the callback given to `get_keyboard_with_repeat`, which has no
    /// `Connection` and no `QueueHandle` to hand on — so the work cannot live
    /// in `press_key`, and while it did, `repeat_key` was dead code and
    /// holding an arrow moved exactly one row.
    fn typed(&mut self, event: KeyEvent) {
        if let Some(k) = key_of(&event, self.mods) {
            self.press(&k);
        }
    }

    /// Asks for a frame. It is drawn when the event queue empties.
    const fn ask_paint(&mut self) {
        self.owed = true;
    }

    /// Draws the owed frame, if one is owed — the scrollback's debt included.
    fn settle(&mut self) {
        if self.scrollback.owes_a_frame() {
            self.follow_the_tail();
            self.owed = true;
        }
        if std::mem::take(&mut self.owed) {
            self.paint();
        }
    }

    fn paint(&mut self) {
        if self.width == 0 {
            return;
        }
        if self.gpu.is_none() {
            self.assemble_gpu();
        }
        let size = tokens::text::size();
        let line_px = tokens::metrics::line_height(size);
        let margin = CONTENT_MARGIN;
        let mut spans: Vec<ronoctua_render::Span> = Vec::new();
        let mut quads: Vec<ronoctua_render::quad::Quad> = Vec::new();

        // LINE 0: whose window this is. A glyph and the sequence's own name —
        // no rule under it, which is the heading this project has (`chrome`).
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(size),
            text: tokens::icon::SYNC.to_string(),
            column: margin + GUTTER,
            line: 0,
            color: tokens::color::TEXT_SECONDARY,
            edge: false,
            foot: false,
        });
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(size),
            text: layout::elide_end(
                &self.texts.t(ronoctua_i18n::key::CMD_SYSTEM_UPDATE),
                self.columns
                    .saturating_sub(GUTTER + COLUMN_GAP + EDGE_RIGHT),
            ),
            column: margin + GUTTER + COLUMN_GAP,
            line: 0,
            color: tokens::color::TEXT_PRIMARY,
            edge: false,
            foot: false,
        });

        // LINE 1: the second register — what the step under the cursor really
        // is, and with no steps to explain, how this window is driven.
        let second = self.steps.get(self.cursor).map_or_else(
            || self.texts.t(ronoctua_i18n::key::UPDATE_HINT),
            |step| {
                let (verdict, _) = self.verdicts.of_step(step.id);
                cursor_explains(step, &verdict, &self.texts)
            },
        );
        spans.push(ronoctua_render::Span {
            size: ronoctua_render::Size::Px(size),
            text: layout::elide_end(&second, self.columns),
            column: margin,
            line: 1,
            color: tokens::color::TEXT_SECONDARY,
            edge: false,
            foot: false,
        });

        // THE STEPS, only the ones that fit.
        let room = self.steps_room();
        for (n, step) in self
            .steps
            .iter()
            .skip(self.step_offset)
            .take(room)
            .enumerate()
        {
            let row = self.step_offset + n;
            #[allow(clippy::cast_possible_truncation)]
            let y = FIRST_OF_STEPS + n as u16;
            // THE CURSOR AND THE POINTER GET DIFFERENT BANDS, and each is a
            // flat quad with no border and no radius — invariant 15.
            //
            // They shared one colour, so a pointer resting away from the cursor
            // lit two rows identically and neither said "this is the one the
            // `Enter` runs". The file manager was reported for exactly that and
            // the split is the owner's: the focus keeps `surface.selection` and
            // the pointer takes the weaker `surface.hover`.
            // EVERY CURSOR'S HOVER in its own colour (`crate::seats`); the
            // first pointer to rest on a row gives it its colour.
            let band = if row == self.cursor {
                Some(tokens::color::SURFACE_SELECTION)
            } else {
                self.pointers
                    .lit(&self.here)
                    .find(|(_, s)| s.row_under_pointer == Some(row))
                    .map(|(color, _)| color)
            };
            if let Some(color) = band {
                quads.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: f32::from(y) * line_px,
                    #[allow(clippy::cast_precision_loss)]
                    width: self.width as f32,
                    height: line_px,
                    color,
                });
            }
            let (verdict, seconds) = self.verdicts.of_step(step.id);
            for piece in step_row(step, &verdict, seconds, self.columns, &self.texts) {
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: piece.text,
                    column: margin + piece.at,
                    line: y,
                    color: piece.color,
                    edge: false,
                    foot: false,
                });
            }
        }

        // THE LINE THAT SAYS WHERE THE TABLE ENDS. A rule and nothing else —
        // no panel, no border, no shadow (invariant 15) — and it rides the top
        // edge of the output's first row rather than costing a row of its own.
        let top = self.output_top();
        #[allow(clippy::cast_precision_loss)]
        quads.push(crate::chrome::rule(
            self.width as f32,
            f32::from(top) * line_px,
        ));

        // THE OUTPUT. Machine text, drawn as it came — sanitised for the grid
        // and never translated: a build log is not interface.
        if self.scrollback.is_empty() {
            spans.push(ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text: layout::elide_end(
                    &self.texts.t(ronoctua_i18n::key::UPDATE_NO_OUTPUT),
                    self.columns,
                ),
                column: margin + GUTTER,
                line: top,
                color: tokens::color::TEXT_DIM,
                edge: false,
                foot: false,
            });
        } else {
            let room = self.output_room();
            for (n, said) in self
                .scrollback
                .window(self.output_up, room)
                .iter()
                .enumerate()
            {
                #[allow(clippy::cast_possible_truncation)]
                let y = top + n as u16;
                spans.push(ronoctua_render::Span {
                    size: ronoctua_render::Size::Px(size),
                    text: layout::elide_end(said, self.columns.saturating_sub(EDGE_RIGHT)),
                    column: margin,
                    line: y,
                    color: tokens::color::TEXT_SECONDARY,
                    edge: false,
                    foot: false,
                });
            }
        }

        // THE OUTPUT'S BAR. This window scrolls from the TAIL — `output_up` is
        // how many lines above the end the band sits — so the offset from the
        // top is what is left over. One arithmetic, `chrome::list_bar`, the
        // same one the other four windows and the footer's region use.
        {
            let room = self.output_room();
            let total = self.scrollback.amount();
            let scroll = crate::chrome::list_scroll(
                total,
                room,
                total.saturating_sub(self.output_up + room),
            );
            #[allow(clippy::cast_precision_loss)]
            if let Some(q) = crate::chrome::list_bar(
                scroll,
                self.output_top(),
                self.width as f32,
                self.height as f32,
            ) {
                quads.push(q);
            }
        }
        // THE SURFACE AND THE TWO SCALE NUMBERS, taken before the borrows:
        // `set_buffer_scale` has to travel in the commit `Gpu::frame` makes,
        // and nothing else in this function may borrow `self` by then. See
        // `crate::hidpi::told_before_present`.
        let surface = self.window.wl_surface().clone();
        let scale = self.scale;
        let told = &mut self.scale_told;
        let (Some(gpu), Some(text)) = (self.gpu.as_mut(), self.text.as_mut()) else {
            return;
        };
        // THE WINDOW'S OWN PIXELS — `resized` already made them what the
        // buffer can show, so there is one number and this is it.
        let (logical_w, logical_h) = (self.width, self.height);
        if let Err(e) = text.prepare(
            &gpu.device,
            &gpu.queue,
            gpu.config.width,
            gpu.config.height,
            ronoctua_render::Atlas::Pinned,
            &spans,
        ) {
            tracing::error!(error = %e, "preparing the text failed");
            return;
        }
        if let Some(q) = self.quads.as_mut() {
            q.prepare(&gpu.device, &gpu.queue, logical_w, logical_h, &quads);
        }
        let quads_ref = self.quads.as_ref();
        // QUADS AND THEN TEXT. There is no depth test: whoever draws later ends
        // up on top, and the text has to stay legible over the band.
        if let Err(e) = gpu.frame(
            || crate::hidpi::told_before_present(scale, told, &surface),
            |pass| {
                if let Some(q) = quads_ref {
                    q.draw(pass);
                }
                text.draw(ronoctua_render::Atlas::Pinned, pass)
            },
        ) {
            tracing::error!(error = %e, "drawing the frame failed");
            return;
        }
        if let Err(e) = self.conn.flush() {
            tracing::warn!(error = %e, "the flush failed");
        }
    }
}

/// A `KeyEvent` as this window's keymap sees it.
///
/// The same mapping the file manager's window does, minus the leader's
/// inheritance: that exists because the footer receives the keyboard with the
/// leader still held down, and this window is given focus like any other.
fn key_of(event: &KeyEvent, mods: Modifiers) -> Option<ronoctua_keymap::Key> {
    use ronoctua_keymap::{Key, Sym};
    let sym = match event.keysym {
        Keysym::Escape => Sym::Esc,
        Keysym::Return | Keysym::KP_Enter => Sym::Enter,
        Keysym::Tab | Keysym::ISO_Left_Tab => Sym::Tab,
        Keysym::BackSpace => Sym::Backspace,
        Keysym::space => Sym::Space,
        Keysym::Left => Sym::Left,
        Keysym::Right => Sym::Right,
        Keysym::Up => Sym::Up,
        Keysym::Down => Sym::Down,
        otherwise => Sym::Char(otherwise.key_char()?),
    };
    Some(Key {
        mods: ronoctua_keymap::Mods {
            ctrl: mods.ctrl,
            alt: mods.alt,
            // SHIFT DOES NOT ENTER: xkb has already applied it when producing
            // the character, so counting both would turn `M-S` into `M-S-S` and
            // never match. The doors have to answer the same key the same way.
            shift: false,
            logo: mods.logo,
        },
        sym,
    })
}

impl WindowHandler for UpdateWindow {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &Window) {
        self.quit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &Window,
        cfg: WindowConfigure,
        _: u32,
    ) {
        // WHAT THE COMPOSITOR SAID IS KEPT SEPARATELY, because what the
        // buffer can show is what comes back below — see `asked`. It is
        // taken from the CONFIGURE and from nothing else: built out of
        // the door's own fields it would be the size `resized` last
        // answered, which can only ever shrink.
        self.asked = (
            cfg.new_size.0.map_or(900, std::num::NonZeroU32::get),
            cfg.new_size.1.map_or(600, std::num::NonZeroU32::get),
        );
        self.width = self.asked.0;
        self.height = self.asked.1;
        let asked = self.asked;
        let scale = self.scale;
        if let Some(g) = self.gpu.as_mut() {
            // AND THE WINDOW KEEPS WHAT THE BUFFER CAN SHOW. Below the GPU's
            // ceiling that is exactly what the compositor configured; above
            // it, laying out against the bigger number would put the columns
            // and the bars in a space the buffer does not have.
            let (shown_w, shown_h) = crate::hidpi::resized(g, asked, scale);
            self.width = shown_w;
            self.height = shown_h;
        }
        self.recount();
        self.ask_paint();
    }
}

impl SeatHandler for UpdateWindow {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seats
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            // A KEYBOARD THAT REPEATS — see the `handle` field. The rate and
            // the delay are the COMPOSITOR's, arriving as `RepeatInfo`;
            // nothing here invents either.
            match self.seats.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.handle.clone(),
                Box::new(|state: &mut Self, _, event| state.typed(event)),
            ) {
                Ok(k) => self.keyboard = Some(k),
                Err(e) => tracing::warn!(error = %e, "no keyboard; there is no typing"),
            }
        }
        if cap == Capability::Pointer {
            match self.seats.get_pointer(qh, &seat) {
                Ok(p) => {
                    // EVERY POINTER, and not only the first: one per cursor.
                    let name = self.seats.info(&seat).and_then(|i| i.name);
                    self.pointers.took(&seat, p, name.as_deref());
                }
                Err(e) => tracing::warn!(error = %e, "no pointer; keyboard only"),
            }
        }
    }

    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Keyboard
            && let Some(k) = self.keyboard.take()
        {
            k.release();
        }
        if cap == Capability::Pointer {
            self.pointers.lost(&seat, &mut self.here);
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for UpdateWindow {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        // The modifiers whole: a state that only updates with focus cannot keep
        // holding without it, and a stuck `ctrl` would send every arrow to the
        // output instead of the cursor.
        self.mods = Modifiers::default();
    }

    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        self.typed(event);
    }

    fn repeat_key(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        kb: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: KeyEvent,
    ) {
        // Holding an arrow walks the table, which is what a list wants.
        self.press_key(conn, qh, kb, serial, event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        mods: Modifiers,
        _: RawModifiers,
        _: u32,
    ) {
        self.mods = mods;
    }
}

impl PointerHandler for UpdateWindow {
    /// EVERY CURSOR HAS ITS OWN STATE: the pointer that spoke is made `here`
    /// first, so a drag, a hover or a wheel's leftover never crosses from
    /// one cursor to another. See `crate::seats`.
    fn pointer_frame(
        &mut self,
        conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        self.pointers.switch_to(pointer, &mut self.here);
        self.pointer_frame_here(conn, qh, events);
    }
}

impl UpdateWindow {
    /// One pointer's frame, with its state already `here`.
    fn pointer_frame_here(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        events: &[PointerEvent],
    ) {
        let before = (self.here.row_under_pointer, self.cursor, self.output_up);
        let mut turned = false;
        let mut over_the_steps = false;
        // THE OUTPUT'S BAR IS A CONTROL. This window counts from the TAIL, so
        // an offset from the top has to be turned back into `output_up` — see
        // the drawing, which does the same conversion the other way round.
        let room = self.output_room();
        let total = self.scrollback.amount();
        let scroll =
            crate::chrome::list_scroll(total, room, total.saturating_sub(self.output_up + room));
        #[allow(clippy::cast_precision_loss)]
        let (bar_w, bar_h) = (self.width as f32, self.height as f32);
        let mut dragged: Option<usize> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if self.here.dragging {
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            self.output_top(),
                            bar_w,
                            bar_h,
                            py,
                        ));
                    } else {
                        self.here.row_under_pointer = self.at(e.position.0, e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    // AND THE DRAG ENDS WITH THE POINTER — see the same arm
                    // in the four list windows.
                    self.here.dragging = false;
                    self.here.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    self.here.dragging = false;
                }
                // A CLICK MOVES THE CURSOR, which is what an arrow does. There
                // is nothing else on this window to do to a step: switching one
                // on and off belongs to the brain's screen, where the person's
                // choice is written down.
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if crate::chrome::on_the_list_bar(
                        scroll,
                        self.output_top(),
                        bar_w,
                        bar_h,
                        px,
                        py,
                    ) {
                        self.here.dragging = true;
                        dragged = Some(crate::chrome::list_offset_at(
                            scroll,
                            self.output_top(),
                            bar_w,
                            bar_h,
                            py,
                        ));
                    } else if let Some(row) = self.at(e.position.0, e.position.1) {
                        self.cursor = row;
                        self.follow_the_cursor();
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    over_the_steps = self.at(e.position.0, e.position.1).is_some();
                    // THE ARITHMETIC IS SHARED — see `surface::wheel_arrived`.
                    crate::surface::wheel_arrived(
                        &mut self.here.lines_owed,
                        &vertical,
                        &horizontal,
                    );
                    turned = true;
                }
                _ => {}
            }
        }
        if let Some(offset) = dragged {
            // BACK TO THE TAIL'S OWN COUNT. And nothing else this frame: a
            // wheel tick arriving during a drag would move the list twice,
            // which the other four windows avoid by returning here.
            self.output_up = total.saturating_sub(offset + room);
            self.follow_the_tail();
            if before != (self.here.row_under_pointer, self.cursor, self.output_up) {
                self.ask_paint();
            }
            return;
        }
        let by = if turned {
            crate::surface::wheel_lines(&mut self.here.lines_owed)
        } else {
            0
        };
        if by != 0 {
            if over_the_steps {
                // THE WHEEL DOES WHAT THE ARROW DOES, on whichever half it is
                // resting over — the rule this project applies to every
                // pointer: a second meaning would be a door only the mouse
                // knows about.
                //
                // AND IT DOES IT ONCE. `by` is LINES, which is what a viewport
                // moves; this half moves a CURSOR, and a cursor's unit is the
                // arrow's — one. Three steps per notch on a list of eight
                // would cross it in three turns of the wheel.
                let moved = i64::try_from(self.cursor)
                    .unwrap_or(0)
                    .saturating_add(by.signum());
                self.cursor = usize::try_from(moved.max(0))
                    .unwrap_or(0)
                    .min(self.steps.len().saturating_sub(1));
                self.follow_the_cursor();
            } else {
                let moved = i64::try_from(self.output_up)
                    .unwrap_or(0)
                    .saturating_sub(by);
                self.output_up = usize::try_from(moved.max(0)).unwrap_or(0);
                self.follow_the_tail();
            }
        }
        // Only redraws if something CHANGED: a `Motion` arrives on every pixel
        // of movement, and painting per pixel would spend the GPU drawing the
        // same thing.
        if before != (self.here.row_under_pointer, self.cursor, self.output_up) {
            self.ask_paint();
        }
    }
}

impl CompositorHandler for UpdateWindow {
    /// The window moved to a screen of another scale — or was told its first.
    ///
    /// It was EMPTY here and in eight other doors, which is what made every
    /// one of them blurry on a HiDPI screen. The rule and the measurement are
    /// in [`crate::hidpi`].
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        // THE SURFACE IS NEEDED to ask its VERSION, not to tell it anything:
        // it is told its scale inside the frame, which is the only commit that
        // carries a matching buffer — see `crate::hidpi::told_before_present`.
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        if let Some((w, h)) = crate::hidpi::followed(
            &mut self.scale,
            factor,
            surface,
            self.gpu.as_mut(),
            self.text.as_mut(),
            self.asked,
        ) {
            // AND THE DOOR KEEPS WHAT THE BUFFER CAN SHOW. Below the
            // GPU's ceiling this is what the compositor configured;
            // above it, going on with the old number lays the picture
            // out in a space the buffer does not have.
            self.width = w;
            self.height = h;
            self.recount();
            self.ask_paint();
        }
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for UpdateWindow {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ProvidesRegistryState for UpdateWindow {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(UpdateWindow);
smithay_client_toolkit::delegate_dispatch2!(UpdateWindow);

#[cfg(test)]
mod tests {
    use super::*;

    fn texts() -> ronoctua_i18n::Texts {
        ronoctua_i18n::Texts::fresh("en-GB").expect("en-GB has to exist")
    }

    fn a_mind(running: &str, queue: &[&str]) -> ronoctua_brain::Mind {
        ronoctua_brain::Mind {
            running: running.to_owned(),
            queue: queue.iter().map(|q| (*q).to_owned()).collect(),
            ..ronoctua_brain::Mind::default()
        }
    }

    #[test]
    fn the_mark_is_there_only_while_something_is_in_flight() {
        // THE PROMISE THIS KEEPS: a machine with nothing running does not spend
        // a column, and does not wake the loop once a second for it.
        assert_eq!(mark(None, 0), None, "with no brain there is no mark");
        assert_eq!(
            mark(Some(&a_mind("", &[])), 0),
            None,
            "a stopped brain has no mark"
        );
        assert!(
            mark(Some(&a_mind(declared::ID, &[])), 0).is_some(),
            "running has a mark"
        );
        assert!(
            mark(Some(&a_mind("", &[declared::ID])), 0).is_some(),
            "queued has a mark"
        );
        // AND IT IS THIS TASK AND NOT ANY TASK. The brain's door is a `vec![]`
        // and the day a second feature goes in it, a mark that answered to
        // whatever was running would draw a rebuild's glyph over somebody
        // else's work.
        assert_eq!(mark(Some(&a_mind("something-else", &[])), 0), None);
    }

    #[test]
    fn one_frame_per_tick_and_it_breathes() {
        // TWO FRAMES: the same circle, filled and hollow. It was twelve
        // positions of a clock hand until the owner saw it — a clock says what
        // time it is, and nothing on that line is about the time.
        assert_eq!(FRAMES.len(), 2, "the pulse has two frames");
        for tick in 0..8u64 {
            assert_eq!(
                frame_of(tick),
                FRAMES[usize::try_from(tick % 2).unwrap()],
                "the frame of tick {tick} left the lap"
            );
        }
        // ONE PER TICK, and never two the same: the footer beats once a second,
        // and a mark that drew the same frame twice would stop breathing.
        for tick in 0..5u64 {
            assert_ne!(
                frame_of(tick),
                frame_of(tick + 1),
                "two ticks in a row drew the same frame"
            );
        }
        assert_eq!(frame_of(0), frame_of(2), "the lap did not close in two");
        // AND THEY ARE NOT THE PIE. The two answer opposite questions and
        // `icons.nix` says so; the day somebody reaches for the wrong family
        // this is what refuses.
        for frame in FRAMES {
            assert!(
                !matches!(
                    frame,
                    tokens::icon::COUNTDOWN_ONE
                        | tokens::icon::COUNTDOWN_TWO
                        | tokens::icon::COUNTDOWN_THREE
                        | tokens::icon::COUNTDOWN_FOUR
                        | tokens::icon::COUNTDOWN_FIVE
                        | tokens::icon::COUNTDOWN_SIX
                        | tokens::icon::COUNTDOWN_SEVEN
                        | tokens::icon::COUNTDOWN_EIGHT
                ),
                "the mark took a frame from the pie"
            );
        }
    }

    #[test]
    fn the_whole_report_vocabulary_is_understood() {
        // THE WORDS ARE THE GENERATOR'S, not ours: `gen-updates.py` emits `off`,
        // `skipped-<kind>`, `rehearsed`, `ran`, `failed` and `stopped`. A word
        // that stopped parsing would leave a step drawn as "still to come"
        // after it had already failed.
        for word in ["off", "rehearsed", "ran", "failed", "stopped"] {
            assert!(verdict_of(word).is_some(), "`{word}` was not understood");
        }
        for kind in ["always", "binary", "directory", "writable-file", "network"] {
            assert_eq!(
                verdict_of(&format!("skipped-{kind}")),
                Some(Verdict::Skipped(kind.to_owned())),
                "`skipped-{kind}` was not understood"
            );
        }
        // AND AN UNKNOWN WORD IS NOT A VERDICT. Turning it into one is how a
        // screen comes to say "it worked" about something else.
        assert_eq!(verdict_of("nonsense"), None);
        assert_eq!(verdict_of("skipped-"), None);
        assert_eq!(verdict_of(""), None);
    }

    #[test]
    fn the_report_is_read_line_by_line_and_a_half_written_one_is_dropped() {
        let text = "flake\tran\t12\nrebuild\tfailed\t900\nhalf-writ";
        let read = read_report(text);
        assert_eq!(read.len(), 2, "the half line got in");
        assert_eq!(read[0], ("flake".to_owned(), Verdict::Ran, 12));
        assert_eq!(read[1], ("rebuild".to_owned(), Verdict::Failed, 900));
    }

    #[test]
    fn the_last_line_about_a_step_is_the_one_that_counts() {
        // The runner APPENDS: a step rehearsed and then run for real leaves two
        // lines, and what happened is the second.
        let halves = vec!["a\trehearsed\t0\na\tran\t7\n".to_owned()];
        let v = Verdicts::of(&halves);
        assert_eq!(v.of_step("a"), (Verdict::Ran, 7));
        // AND A STEP NOBODY REPORTED HAS NOT HAPPENED. `Pending` is the absence
        // of a line, which is a different thing from a line saying it was
        // skipped — and the two draw different marks.
        assert_eq!(v.of_step("b"), (Verdict::Pending, 0));
    }

    #[test]
    fn both_halves_land_in_one_table() {
        let halves = vec!["root\tran\t3\n".to_owned(), "person\tran\t4\n".to_owned()];
        let v = Verdicts::of(&halves);
        assert_eq!(v.of_step("root"), (Verdict::Ran, 3));
        assert_eq!(v.of_step("person"), (Verdict::Ran, 4));
    }

    #[test]
    fn a_thousand_lines_owe_exactly_one_frame() {
        // THE DEFECT THIS CATCHES, and it has a name in `ARCHITECTURE.md`: a
        // producer that publishes faster than the loop draws. A rebuild prints
        // thousands of lines in a burst; painting per line is the frozen window
        // the brain's daemon already caused once.
        let mut back = Scrollback::default();
        for n in 0..1000 {
            back.push(format!("line {n}"));
        }
        assert!(
            back.owes_a_frame(),
            "a thousand rows asked for no frame at all"
        );
        assert!(
            !back.owes_a_frame(),
            "the frame was charged twice for the same burst"
        );
    }

    #[test]
    fn the_scrollback_is_bounded_and_keeps_the_tail() {
        let mut back = Scrollback::default();
        for n in 0..(KEPT * 3) {
            back.push(format!("{n}"));
        }
        assert_eq!(back.amount(), KEPT, "the window grew with no ceiling");
        // AND WHAT SURVIVED IS THE END, which is where the error is: nix prints
        // the last lines of the failing log, and those are written last.
        let tail = back.window(0, 2);
        assert_eq!(
            tail,
            vec![(KEPT * 3 - 2).to_string(), (KEPT * 3 - 1).to_string()]
        );
    }

    #[test]
    fn the_output_window_never_asks_for_what_is_not_there() {
        let mut back = Scrollback::default();
        for n in 0..5 {
            back.push(format!("{n}"));
        }
        assert_eq!(
            back.window(0, 100).len(),
            5,
            "it asked for more than exists"
        );
        assert_eq!(back.window(100, 2).len(), 0, "it went above the start");
        assert_eq!(back.window(2, 2), vec!["1", "2"]);
    }

    #[test]
    fn machine_output_is_made_safe_for_the_grid() {
        // Nix emits ANSI whenever it thinks it has a terminal. Drawn as it
        // came, the escape becomes a row of tofu over the line it decorated —
        // and that line is usually the error.
        assert_eq!(readable("\u{1b}[1;31merror:\u{1b}[0m boom"), "error: boom");
        // A tab is ONE space: this is a grid whose columns belong to the table
        // above, and a line that jumps eight cells reads as a real indent.
        assert_eq!(readable("a\tb"), "a b");
        assert_eq!(readable("plain"), "plain");
        assert_eq!(readable("carriage\rreturn"), "carriagereturn");
        // And nothing is invented: text with none of that comes back whole.
        // pt-BR: the accent is the subject — `readable` must not touch it.
        assert_eq!(readable("acentuação e ünicode"), "acentuação e ünicode");
    }

    #[test]
    fn nothing_in_a_step_row_goes_past_the_width() {
        // The property the whole layout exists for. The names come from the
        // declaration and can be long in either language; the window can be
        // narrow; neither may spill, because a compositor does not clip.
        let texts = texts();
        let steps = declared::steps();
        assert!(!steps.is_empty(), "the declared table came out empty");
        let verdicts = [
            Verdict::Ran,
            Verdict::Failed,
            Verdict::Off,
            Verdict::Skipped("writable-file".to_owned()),
            Verdict::Rehearsed,
            Verdict::Stopped,
            Verdict::Pending,
        ];
        for columns in [8_u16, 12, 20, 40, 80, 200] {
            for step in steps {
                for verdict in &verdicts {
                    for pieces in [step_row(step, verdict, 4321, columns, &texts)] {
                        for piece in pieces {
                            assert!(
                                piece.at + layout::width(&piece.text) <= columns,
                                "`{}` leaks at {columns} columns: {} + {}",
                                step.id,
                                piece.at,
                                layout::width(&piece.text)
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_step_row_says_the_same_thing_in_two_registers() {
        let texts = texts();
        let step = declared::steps().first().expect("there are steps");
        let pieces = step_row(step, &Verdict::Ran, 65, 80, &texts);
        // THE MARK, THE NAME, THE CLOCK AND THE WORD — four pieces, and the
        // clock only for a step that actually ran.
        assert_eq!(pieces.len(), 4);
        assert!(
            pieces.iter().any(|p| p.text == "1:05"),
            "the time did not come out as a clock: {pieces:?}"
        );
        let quiet = step_row(step, &Verdict::Pending, 0, 80, &texts);
        assert_eq!(quiet.len(), 3, "a step that did not run was given a clock");
        // AND THE SECOND REGISTER CARRIES WHAT THE ROW CANNOT.
        let said = cursor_explains(step, &Verdict::Skipped("network".to_owned()), &texts);
        assert!(
            said.contains(&texts.t(ronoctua_i18n::key::UPDATE_NEEDS_NETWORK)),
            "the reason for the skip did not reach the second record: {said}"
        );
        assert!(!said.starts_with('['), "a key with no text: {said}");
    }

    #[test]
    fn the_clock_reads_the_same_in_both_languages() {
        assert_eq!(clock_of(0), "0:00");
        assert_eq!(clock_of(9), "0:09");
        assert_eq!(clock_of(60), "1:00");
        assert_eq!(clock_of(3599), "59:59");
        // Twenty minutes of rebuild is the case this window exists for, and it
        // is still two fields and no word.
        assert_eq!(clock_of(1200), "20:00");
    }

    #[test]
    fn every_verdict_has_a_word_and_a_mark_of_its_own() {
        let texts = texts();
        for verdict in [
            Verdict::Ran,
            Verdict::Failed,
            Verdict::Off,
            Verdict::Skipped("binary".to_owned()),
            Verdict::Rehearsed,
            Verdict::Stopped,
            Verdict::Pending,
        ] {
            let said = texts.t(verdict.key());
            assert!(
                !said.starts_with('['),
                "`{}` has no text: {said}",
                verdict.key()
            );
            let (glyph, _) = verdict.mark();
            assert!(
                ('\u{e000}'..='\u{f8ff}').contains(&glyph)
                    || ('\u{f0000}'..='\u{ffffd}').contains(&glyph),
                "the mark for {verdict:?} is not a token glyph"
            );
        }
    }
}
