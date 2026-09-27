//! The update — the mark on the modeline, the door that starts it, and the
//! view that watches it happen, in the footer and in a window.
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
//! So `LEADER s u n` TRIGGERS; it never enumerates. What it starts is the unit
//! the brain starts, named by the brain's own constant, and the steps the view
//! draws are [`ronoctua_brain::tasks::update::steps`] and nothing else.
//! `update-has-one-list` is the check that keeps it that way: a step's id
//! written by hand anywhere outside the declaration takes the build down.
//!
//! # Four things live here, and they are one feature
//!
//! 1. **The mark**, twelve frames of a clock hand riding the one-second timer
//!    the footer already beats. It says the machine is still working — a still
//!    glyph and a hung rebuild look identical, and this is the one thing on
//!    this system that legitimately takes twenty minutes.
//! 2. **The door**, [`begin`]. The brain when it is there, systemd when it is
//!    not. The brain is a scheduler, not a gatekeeper.
//! 3. **The view**, [`View`]: the steps with their verdicts, what the step
//!    under the cursor is, and the two journals live. Both doors show it —
//!    the footer's region (`LEADER s u u`) as one list, the window
//!    (`LEADER s u w`) in two halves — with the same keys, every one a click:
//!    the steps, the output, `u` to start the update now and `a` and `p` to
//!    answer the brain's countdown, with the footer's sentences.
//! 4. **The window**, which lays the view out in its two halves.
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

use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, Ui};
use anyhow::Result;
use ronoctua_brain::tasks::update as declared;
use ronoctua_render::layout;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::{channel, timer},
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
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
// THE DOOR — `LEADER s u n`
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

    /// Line `n`, counted from the oldest kept.
    #[must_use]
    pub fn line(&self, n: usize) -> Option<&str> {
        self.said.get(n).map(String::as_str)
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
// THE VIEW — both doors
// ---------------------------------------------------------------------------

/// The keys the update teaches, in both doors — every one a click.
///
/// The step cursor on the arrows; the output a page at a time on `C-j` and
/// `C-k` and back to its end on `G`; and the three things one DOES here: `u`
/// starts the update now — `LEADER s u n`'s command — and `a` and `p` answer
/// the brain's countdown, the band's two buttons and the brain buffer's
/// `, a` and `, p`.
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "update-step-next"),
    ("<up>", "update-step-previous"),
    ("C-j", "update-output-down"),
    ("C-k", "update-output-up"),
    ("G", "update-output-end"),
    ("u", START),
    ("a", ABORT),
    ("p", PROCEED),
];

/// The command that starts the whole sequence now.
pub const START: &str = "system-update";
/// The countdown's "not now" — or "stop it", once it runs.
pub const ABORT: &str = "brain-abort";
/// The countdown's "start now".
pub const PROCEED: &str = "brain-proceed";

/// A key, read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The step above.
    Up,
    /// The step below.
    Down,
    /// The output, a page towards its start.
    OutputUp,
    /// The output, a page towards its end.
    OutputDown,
    /// The output's end, where a live run is.
    OutputEnd,
    /// Start the update now.
    Start,
    /// Stop the countdown, or the run.
    Abort,
    /// Start now what is counting down.
    Proceed,
    /// Out.
    Escape,
}

/// A key, as the view's press — the one reader both doors call.
///
/// By its whole notation through [`OF_THE_VIEW`], and three that are not
/// taught and that both doors read the same way: `Esc` and `q` close, `j` and
/// `k` walk the steps as the arrows do, and the arrows under `Ctrl` move the
/// output as `C-j` and `C-k` do — the letters are what is taught, because a
/// chord above the window may take `Ctrl` with an arrow.
#[must_use]
pub fn press_of_key(key: ronoctua_keymap::Key) -> Option<Press> {
    use ronoctua_keymap::Sym;
    let plain = !key.mods.ctrl && !key.mods.alt && !key.mods.logo;
    match key.sym {
        Sym::Esc => return Some(Press::Escape),
        Sym::Char('q') if plain => return Some(Press::Escape),
        Sym::Char('j') if plain => return Some(Press::Down),
        Sym::Char('k') if plain => return Some(Press::Up),
        Sym::Down if key.mods.ctrl => return Some(Press::OutputDown),
        Sym::Up if key.mods.ctrl => return Some(Press::OutputUp),
        _ => {}
    }
    let notation = key.notation();
    let id = OF_THE_VIEW
        .iter()
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    Some(match id {
        "update-step-next" => Press::Down,
        "update-step-previous" => Press::Up,
        "update-output-down" => Press::OutputDown,
        "update-output-up" => Press::OutputUp,
        "update-output-end" => Press::OutputEnd,
        START => Press::Start,
        ABORT => Press::Abort,
        PROCEED => Press::Proceed,
        _ => return None,
    })
}

/// How the output moves — each door on its own page: the window's lower half
/// counts from the tail, the footer's region is one list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A page towards the start.
    Up,
    /// A page towards the end.
    Down,
    /// To the end.
    End,
}

/// What the door has to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Errand {
    /// Nothing changed.
    Nothing,
    /// The step cursor moved: the page follows it, and the second register
    /// says what the new step is.
    Moved,
    /// The output moves.
    Output(Output),
    /// Start the update ([`begin`]), and say which door it went through.
    Start,
    /// Answer the countdown — `true` to start now, `false` for "not now".
    Answer(bool),
    /// The view closes.
    Close,
}

/// The update, as both doors show it: the declared steps, what the two
/// halves reported about each, the step under the cursor and what the two
/// journals said. What each door keeps of its own is its page.
#[derive(Debug, Clone)]
pub struct View {
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
}

impl View {
    /// The declared steps, and what the reports on this machine say.
    #[must_use]
    pub fn read() -> Self {
        Self::of(declared::steps(), Verdicts::read(), report_stamp())
    }

    /// A view of `steps` with `verdicts` — what a test builds, and nothing of
    /// the machine is read.
    #[must_use]
    pub fn of(
        steps: &'static [ronoctua_brain::Step],
        verdicts: Verdicts,
        stamp: Vec<Option<std::time::SystemTime>>,
    ) -> Self {
        Self {
            steps,
            verdicts,
            stamp,
            scrollback: Scrollback::default(),
            cursor: 0,
        }
    }

    /// The declared steps.
    #[must_use]
    pub const fn steps(&self) -> &'static [ronoctua_brain::Step] {
        self.steps
    }

    /// What the reports say, merged.
    #[must_use]
    pub const fn verdicts(&self) -> &Verdicts {
        &self.verdicts
    }

    /// The output.
    #[must_use]
    pub const fn scrollback(&self) -> &Scrollback {
        &self.scrollback
    }

    /// The step under the cursor.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// One line of either journal.
    pub fn heard(&mut self, line: String) {
        self.scrollback.push(line);
    }

    /// Whether a frame is owed for what the journals said, and clears it.
    pub const fn owes_a_frame(&mut self) -> bool {
        self.scrollback.owes_a_frame()
    }

    /// Re-reads the reports when a file moved, and only then; `true` when
    /// something did.
    pub fn reread_if_moved(&mut self) -> bool {
        let now = report_stamp();
        if now == self.stamp {
            return false;
        }
        self.stamp = now;
        self.verdicts = Verdicts::read();
        true
    }

    /// The second register: what the step under the cursor really is.
    #[must_use]
    pub fn explains(&self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        self.steps.get(self.cursor).map(|step| {
            let (verdict, _) = self.verdicts.of_step(step.id);
            cursor_explains(step, &verdict, texts)
        })
    }

    /// One key — the same in both doors.
    pub fn key(&mut self, press: Press) -> Errand {
        match press {
            Press::Down => {
                self.cursor = (self.cursor + 1).min(self.steps.len().saturating_sub(1));
                Errand::Moved
            }
            Press::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                Errand::Moved
            }
            Press::OutputUp => Errand::Output(Output::Up),
            Press::OutputDown => Errand::Output(Output::Down),
            Press::OutputEnd => Errand::Output(Output::End),
            Press::Start => Errand::Start,
            Press::Abort => Errand::Answer(false),
            Press::Proceed => Errand::Answer(true),
            Press::Escape => Errand::Close,
        }
    }

    /// The second register's lines in the footer's region: WRAPPED, never
    /// cut — a note is read whole.
    fn register_lines(&self, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<String> {
        crate::chrome::note_lines(&self.explains(texts).unwrap_or_default(), room)
    }

    /// How many rows the footer's region is: the second register, the steps,
    /// a line of air and the output — counted without building any, for the
    /// footer asks this on every pointer motion.
    #[must_use]
    pub fn region_len(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let (_, room) = crate::power::extent(columns);
        self.register_lines(room, texts).len()
            + self.steps.len()
            + 1
            + self.scrollback.amount().max(1)
    }

    /// The row of the region the cursor's step is on — what the footer's
    /// page follows after a key.
    #[must_use]
    pub fn region_cursor(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let (_, room) = crate::power::extent(columns);
        self.register_lines(room, texts).len() + self.cursor
    }

    /// The footer's region, rows `skip..skip + take` of it: ONE list — the
    /// second register on top, the steps with their verdicts under it (the
    /// cursor's name in the accent: the cursor is a colour), a line of air,
    /// and the output of both journals — which the region's own scroll, bar
    /// and wheel read, as they read every list of the footer. Only the rows
    /// asked for are built: the output keeps up to [`KEPT`] lines.
    #[must_use]
    pub fn region_rows(
        &self,
        skip: usize,
        take: usize,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<RegionRow> {
        let (left, room) = crate::power::extent(columns);
        let register = self.register_lines(room, texts);
        let steps_at = register.len();
        let air_at = steps_at + self.steps.len();
        let output_at = air_at + 1;
        let total = self.region_len(columns, texts);
        let field = |at: u16, text: String, color: tokens::Color| crate::files::Field {
            at: left + at,
            text,
            color,
            hit: crate::files::Hit::Nothing,
            explain: None,
            ink: crate::files::Ink::Grid,
        };
        let mut out = Vec::with_capacity(take.min(total));
        for n in skip..total.min(skip.saturating_add(take)) {
            let row = if let Some(said) = register.get(n) {
                RegionRow {
                    fields: vec![field(0, said.clone(), tokens::color::TEXT_SECONDARY)],
                    step: None,
                }
            } else if n < air_at {
                let at = n - steps_at;
                let step = &self.steps[at];
                let (verdict, seconds) = self.verdicts.of_step(step.id);
                let mut pieces = step_row(step, &verdict, seconds, room, texts);
                // THE NAME IS THE LAST PIECE, and the cursor is its colour.
                if at == self.cursor
                    && let Some(name) = pieces.last_mut()
                {
                    name.color = crate::chrome::Kind::ListedFact.tones(true).0;
                }
                RegionRow {
                    fields: pieces
                        .into_iter()
                        .map(|p| field(p.at, p.text, p.color))
                        .collect(),
                    step: Some(at),
                }
            } else if n < output_at {
                RegionRow {
                    fields: crate::chrome::AIR,
                    step: None,
                }
            } else if self.scrollback.is_empty() {
                RegionRow {
                    fields: vec![field(
                        GUTTER,
                        layout::elide_end(&texts.t(ronoctua_i18n::key::UPDATE_NO_OUTPUT), room),
                        tokens::color::TEXT_DIM,
                    )],
                    step: None,
                }
            } else {
                // MACHINE TEXT, drawn as it came — sanitised for the grid and
                // never translated: a build log is not interface.
                let line = self
                    .scrollback
                    .line(n - output_at)
                    .map_or_else(String::new, |l| layout::elide_end(l, room));
                RegionRow {
                    fields: vec![field(0, line, tokens::color::TEXT_SECONDARY)],
                    step: None,
                }
            };
            out.push(row);
        }
        out
    }

    /// A click on step `n`: the cursor goes there, and what the step is is
    /// said. There is nothing else to do to a step here — switching one on or
    /// off belongs to the brain's screen, where the choice is written down —
    /// so a second click, like `RET`, does nothing more (invariant 19).
    pub fn clicked(&mut self, n: usize) -> Errand {
        if n >= self.steps.len() || n == self.cursor {
            return Errand::Nothing;
        }
        self.cursor = n;
        Errand::Moved
    }
}

/// One row of the footer's region.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionRow {
    /// Placed and cut.
    pub fields: Vec<crate::files::Field>,
    /// The step it is, when it is one — what a click on it selects.
    pub step: Option<usize>,
}

/// What starting said, in the person's words: through the brain it is
/// QUEUED, straight to systemd it has STARTED, and with neither the screen
/// says so — a key that does nothing is indistinguishable from one that did
/// not register. Both doors say the same sentence.
#[must_use]
pub const fn said_of(started: Started) -> &'static str {
    match started {
        Started::Brain => ronoctua_i18n::key::ECHO_UPDATE_QUEUED,
        Started::Systemd => ronoctua_i18n::key::ECHO_UPDATE_STARTED,
        Started::Nowhere => ronoctua_i18n::key::ECHO_UPDATE_NOWHERE,
    }
}

/// Follows both journals — the root half's and the person's — each on a
/// thread of its own, sending every line down `to`. The children come back so
/// the door that opened them KILLS them: `-f` never ends by itself.
pub fn follow_both(to: &channel::Sender<String>) -> Vec<std::process::Child> {
    // The root half runs first and the person's half BLOCKS on it —
    // `systemctl start` on a `Type=oneshot` returns when the work is done —
    // so the two are never producing at the same time except at the
    // hand-over. Arrival order is the sequence's order, and nothing here
    // claims more than that.
    let mut children = Vec::new();
    children.extend(follow(SYSTEM_UNIT, false, to));
    children.extend(follow(declared::UNIT, true, to));
    children
}

/// Stops what [`follow_both`] started.
pub fn stop_following(children: &mut Vec<std::process::Child>) {
    for mut child in children.drain(..) {
        let _ = child.kill();
        let _ = child.wait();
    }
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

/// Where content begins, in cells from the window's edge.
///
/// [`crate::chrome::FIELDS_LEAD`], because every row here carries its own first
/// cell — the mark's, and the air its glyph needs. What this door owes is the
/// DIFFERENCE up to the content's inset, and the constant is where the three
/// readers of that difference are named.
const CONTENT_MARGIN: u16 = crate::chrome::FIELDS_LEAD;

/// The glyph before the window's name, as text — the `Title`'s shape.
static SYNC_GLYPH: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| tokens::icon::SYNC.to_string());

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
    crate::door::run(|w| {
        // THE TWO JOURNALS, each on a thread of its own.
        let (sender, said) = channel::channel::<String>();
        let children = follow_both(&sender);
        w.handle
            .insert_source(said, |event, (), door: &mut Door<UpdateWindow>| {
                if let channel::Event::Msg(line) = event {
                    // NOTHING IS PAINTED HERE. Every line that has arrived
                    // lands in the scrollback, and the frame is owed once the
                    // queue is empty — see `Scrollback` for what a redraw per
                    // line costs, and `each_turn` for where the debt is paid.
                    door.content.view.heard(line);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the journal: {e}"))?;

        // THE BRAIN'S MIND, for the countdown's two answers: what `a` sends
        // depends on whether a run is announced or already running, and only
        // the brain says which (`brain::answer_of`).
        let (minds, mind) = channel::channel::<ronoctua_brain::Mind>();
        w.handle
            .insert_source(mind, |event, (), door: &mut Door<UpdateWindow>| {
                if let channel::Event::Msg(m) = event {
                    door.content.mind = Some(m);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the brain: {e}"))?;
        crate::brain::listen(minds);

        w.handle
            .insert_source(
                timer::Timer::from_duration(BEAT),
                |_, (), door: &mut Door<UpdateWindow>| {
                    if door.content.view.reread_if_moved() {
                        door.ui.ask_paint();
                    }
                    timer::TimeoutAction::ToDuration(BEAT)
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the clock: {e}"))?;

        Ok(UpdateWindow::of(View::read(), children))
    })
}

/// The window: the view both doors show, and this door's own page.
struct UpdateWindow {
    /// The steps, their verdicts, the cursor and the output — the footer's
    /// view too.
    view: View,
    /// The brain's mind, as it last said — `None` while it is not answering.
    mind: Option<ronoctua_brain::Mind>,
    /// The first visible step, when they do not all fit.
    step_offset: usize,
    /// How many lines above the tail the output is showing. Zero follows.
    output_up: usize,
    /// The window's lines under its header, as the door last counted them —
    /// kept so the arithmetic of the two halves can be asked from a key.
    lines: u16,
    /// The two `journalctl -f`, so they can be killed.
    children: Vec<std::process::Child>,
}

impl Drop for UpdateWindow {
    /// WHAT I STARTED, I STOP. `journalctl -f` never ends on its own, and a
    /// door that leaves two of them behind every time it is closed is a door
    /// that leaks processes.
    fn drop(&mut self) {
        stop_following(&mut self.children);
    }
}

impl UpdateWindow {
    /// The window over `view`, following `children`.
    fn of(view: View, children: Vec<std::process::Child>) -> Self {
        Self {
            view,
            mind: None,
            step_offset: 0,
            output_up: 0,
            lines: 22,
            children,
        }
    }

    /// How many rows the step table gets, under the second register.
    ///
    /// # Why half and not all of them
    ///
    /// Because the window is two things at once and neither may starve the
    /// other: twelve steps on a twenty-four-line window would leave the output
    /// nine rows, which is not enough to read an error out of. Half each, and
    /// the table scrolls when it does not fit — which on any window worth
    /// opening it does.
    fn steps_room(&self) -> usize {
        let below = self.lines.saturating_sub(1);
        usize::from(below / 2).min(self.view.steps().len())
    }

    /// The first row of the output region, counted from the first line under
    /// the header.
    fn output_top(&self) -> u16 {
        1 + u16::try_from(self.steps_room()).unwrap_or(0)
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
        let cursor = self.view.cursor();
        if cursor < self.step_offset {
            self.step_offset = cursor;
        } else if cursor >= self.step_offset + room {
            self.step_offset = cursor + 1 - room;
        }
        self.step_offset = self
            .step_offset
            .min(self.view.steps().len().saturating_sub(room));
    }

    /// Keeps the output's offset inside what there is to show.
    fn follow_the_tail(&mut self) {
        let most = self
            .view
            .scrollback()
            .amount()
            .saturating_sub(self.output_room());
        self.output_up = self.output_up.min(most);
    }

    /// The output's bar, as a scroll from the TOP.
    ///
    /// This window scrolls from the TAIL — `output_up` is how many lines above
    /// the end the band sits — so the offset from the top is what is left
    /// over. One arithmetic, read by the drawing and by the pointer.
    fn output_scroll(&self) -> ronoctua_design::Scroll {
        let room = self.output_room();
        let total = self.view.scrollback().amount();
        crate::chrome::list_scroll(total, room, total.saturating_sub(self.output_up + room))
    }

    /// Which step row a pointer position lands on, if any.
    fn at(&self, ui: &Ui, x: f64, y: f64) -> Option<usize> {
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line_px <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_px) as u16;
        let first_step = ui.first_of_list() + 1;
        if line < first_step || line >= ui.first_of_list() + self.output_top() {
            return None;
        }
        let row = self.step_offset + usize::from(line - first_step);
        (row < self.view.steps().len()).then_some(row)
    }

    /// Carries out what the view asked for — the footer's `update_errand`, on
    /// this door's page, with the same sentences.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        match errand {
            Errand::Nothing => return,
            Errand::Moved => self.follow_the_cursor(),
            Errand::Output(how) => {
                let page = self.output_room().max(1);
                match how {
                    Output::Up => self.output_up = self.output_up.saturating_add(page),
                    Output::Down => self.output_up = self.output_up.saturating_sub(page),
                    Output::End => self.output_up = 0,
                }
                self.follow_the_tail();
            }
            // STARTED FROM HERE, through the door the footer's `LEADER s u n`
            // goes through, and said here in its words.
            Errand::Start => ui.message = Some(ui.texts.t(said_of(begin()))),
            Errand::Answer(proceeding) => {
                let said = crate::brain::answer(self.mind.as_ref(), proceeding);
                ui.message = Some(ui.texts.t(said));
            }
            Errand::Close => ui.quit = true,
        }
        ui.ask_paint();
    }

    /// One key, from the keyboard or from a click on the header.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        self.lines = ui.lines.saturating_sub(ui.first_of_list());
        let errand = self.view.key(press);
        self.apply(ui, errand);
    }
}

impl Content for UpdateWindow {
    type Hand = ();

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    /// `*name*` is Emacs's convention for a buffer that is not a file, and the
    /// name is the command's own label — one sentence, two doors.
    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(ronoctua_i18n::key::CMD_SYSTEM_UPDATE))
    }

    /// One key, from a press or from a REPEAT — holding an arrow walks the
    /// table, which is what a list wants — read by the reader the footer
    /// calls too.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(press) = crate::door::keys::key_of(event, mods).and_then(press_of_key) {
            self.pressed(ui, press);
        }
    }

    /// Where content begins, in cells from the window's edge.
    fn margin(&self) -> u16 {
        CONTENT_MARGIN
    }

    /// THE KEYS THE FOOTER TEACHES, on this window's header — every one a
    /// click, `u`, `a` and `p` included.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        crate::chrome::hint_of(OF_THE_VIEW, &ui.texts)
    }

    fn resized(&mut self, ui: &mut Ui) {
        self.lines = ui.lines.saturating_sub(ui.first_of_list());
        self.follow_the_cursor();
        self.follow_the_tail();
    }

    /// THE SCROLLBACK'S DEBT, paid once the queue is empty. A thousand lines of
    /// a rebuild arriving in one burst are one paint, not a thousand.
    fn each_turn(&mut self, ui: &mut Ui) {
        if self.view.owes_a_frame() {
            self.follow_the_tail();
            ui.ask_paint();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let size = tokens::text::size();
        let line_px = tokens::metrics::line_height(size);
        let texts = ui.texts.clone();

        // LINE 0: whose window this is — a glyph and the sequence's own name —
        // and the keys, every one a click, in the header every window of this
        // system has, drawer and all. What the last order said takes the keys'
        // place, and a question outranks both (`list::header_of`).
        let name = texts.t(ronoctua_i18n::key::CMD_SYSTEM_UPDATE);
        let hint = self.hint(ui);
        crate::door::list::header_of(
            ui,
            canvas,
            Some(crate::chrome::Title {
                glyph: Some(SYNC_GLYPH.as_str()),
                label: None,
                name: Some(&name),
            }),
            |_| hint,
        );
        let first = ui.first_of_list();
        self.lines = ui.lines.saturating_sub(first);
        let margin = ui.margin;
        let columns = ui.columns;
        #[allow(clippy::cast_precision_loss)]
        let (width, height) = (ui.width as f32, ui.height as f32);
        let span =
            |text: String, column: u16, line: u16, color: tokens::Color| ronoctua_render::Span {
                size: ronoctua_render::Size::Px(size),
                text,
                column,
                line,
                color,
                edge: false,
                foot: false,
            };

        // THE SECOND REGISTER — what the step under the cursor really is.
        if let Some(second) = self.view.explains(&texts) {
            canvas.spans.push(span(
                layout::elide_end(&second, columns),
                margin,
                first,
                tokens::color::TEXT_SECONDARY,
            ));
        }

        // THE STEPS, only the ones that fit.
        let room = self.steps_room();
        for (n, step) in self
            .view
            .steps()
            .iter()
            .skip(self.step_offset)
            .take(room)
            .enumerate()
        {
            let row = self.step_offset + n;
            let y = first + 1 + u16::try_from(n).unwrap_or(0);
            // THE CURSOR AND THE POINTER GET DIFFERENT BANDS, and each is a
            // flat quad with no border and no radius — invariant 15.
            //
            // They shared one colour, so a pointer resting away from the cursor
            // lit two rows identically and neither said "this is the one the
            // arrows move from". The file manager was reported for exactly that
            // and the split is the owner's: the focus keeps `surface.selection`
            // and the pointer takes the weaker `surface.hover`. EVERY CURSOR'S
            // HOVER in its own colour (`crate::seats`).
            //
            // AND THE BAND IS THE WHOLE WIDTH: the only bar this window draws
            // is the output's, below the table, so no column of these rows is
            // a bar's.
            let band = if row == self.view.cursor() {
                Some(tokens::color::SURFACE_SELECTION)
            } else {
                canvas
                    .lit
                    .iter()
                    .find(|(_, under)| *under == Some(row))
                    .map(|(color, _)| *color)
            };
            if let Some(color) = band {
                canvas.under.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: f32::from(y) * line_px,
                    width,
                    height: line_px,
                    color,
                });
            }
            let (verdict, seconds) = self.view.verdicts().of_step(step.id);
            for piece in step_row(step, &verdict, seconds, columns, &texts) {
                canvas
                    .spans
                    .push(span(piece.text, margin + piece.at, y, piece.color));
            }
        }

        // THE LINE THAT SAYS WHERE THE TABLE ENDS. A rule and nothing else —
        // no panel, no border, no shadow (invariant 15) — and it rides the top
        // edge of the output's first row rather than costing a row of its own.
        let top = first + self.output_top();
        canvas
            .under
            .push(crate::chrome::rule(width, f32::from(top) * line_px));

        // THE OUTPUT. Machine text, drawn as it came — sanitised for the grid
        // and never translated: a build log is not interface.
        if self.view.scrollback().is_empty() {
            canvas.spans.push(span(
                layout::elide_end(&texts.t(ronoctua_i18n::key::UPDATE_NO_OUTPUT), columns),
                margin + GUTTER,
                top,
                tokens::color::TEXT_DIM,
            ));
        } else {
            let room = self.output_room();
            for (n, said) in self
                .view
                .scrollback()
                .window(self.output_up, room)
                .iter()
                .enumerate()
            {
                let y = top + u16::try_from(n).unwrap_or(0);
                canvas.spans.push(span(
                    layout::elide_end(said, columns.saturating_sub(EDGE_RIGHT)),
                    margin,
                    y,
                    tokens::color::TEXT_SECONDARY,
                ));
            }
        }

        // THE OUTPUT'S BAR — `chrome::list_bar`, the same arithmetic the other
        // windows and the footer's region use — written down in `ui.room` as
        // it is drawn, so the bar's column is its own while it is there.
        let scroll = self.output_scroll();
        ui.room = crate::chrome::Room::of(scroll, width, height);
        canvas
            .under
            .extend(crate::chrome::list_bar(scroll, top, width, height));
        canvas.bar(crate::chrome::list_track(scroll, top, width, height));
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<()>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }
}

impl UpdateWindow {
    /// One pointer's frame: the header's controls first — the chevron, the
    /// drawer's bar and its wheel, and the sentence under a key — then the
    /// output's bar, then a step, then the wheel on whichever half it rests
    /// over.
    #[allow(clippy::too_many_lines)]
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<()>, events: &[PointerEvent]) {
        let before = (hand.row_under_pointer, self.view.cursor(), self.output_up);
        let mut turned = false;
        let mut over_the_steps = false;
        let mut header = false;
        let mut explained = false;
        let mut wheel_at: Option<f64> = None;
        let head = ui.frame(0);
        // THE OUTPUT'S BAR IS A CONTROL. This window counts from the TAIL, so
        // an offset from the top has to be turned back into `output_up` — see
        // `output_scroll`, which does the same conversion the other way round.
        let room = self.output_room();
        let total = self.view.scrollback().amount();
        let scroll = self.output_scroll();
        let top = ui.first_of_list() + self.output_top();
        #[allow(clippy::cast_precision_loss)]
        let (bar_w, bar_h) = (ui.width as f32, ui.height as f32);
        let mut dragged: Option<usize> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if ui.hinted.dragged(head, e.position.1) {
                        header = true;
                        continue;
                    }
                    let texts = ui.texts.clone();
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| crate::chrome::hint_sentence(id, &texts));
                    explained |= ui.hinted.say(now);
                    if hand.dragging {
                        dragged =
                            Some(crate::chrome::list_offset_at(scroll, top, bar_w, bar_h, py));
                    } else {
                        hand.row_under_pointer = self.at(ui, e.position.0, e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    ui.hinted.let_go();
                    explained |= ui.hinted.say(None);
                    // AND THE DRAG ENDS WITH THE POINTER — see the same arm
                    // in the list windows.
                    hand.dragging = false;
                    hand.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    hand.dragging = false;
                }
                // A CLICK PUTS THE CURSOR ON THE STEP, through the view's own
                // `clicked` — the footer's click reaches it too.
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        header = true;
                        continue;
                    }
                    if crate::chrome::on_the_list_bar(scroll, top, bar_w, bar_h, px, py) {
                        hand.dragging = true;
                        dragged =
                            Some(crate::chrome::list_offset_at(scroll, top, bar_w, bar_h, py));
                    } else if let Some(row) = self.at(ui, e.position.0, e.position.1) {
                        let errand = self.view.clicked(row);
                        self.apply(ui, errand);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    over_the_steps = self.at(ui, e.position.0, e.position.1).is_some();
                    wheel_at = Some(e.position.1);
                    // THE ARITHMETIC IS SHARED — see `surface::wheel_arrived`.
                    crate::surface::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    turned = true;
                }
                _ => {}
            }
        }
        if header {
            ui.ask_paint();
            return;
        }
        if let Some(offset) = dragged {
            // BACK TO THE TAIL'S OWN COUNT. And nothing else this frame: a
            // wheel tick arriving during a drag would move the list twice,
            // which the list windows avoid by returning here.
            self.output_up = total.saturating_sub(offset + room);
            self.follow_the_tail();
            if before != (hand.row_under_pointer, self.view.cursor(), self.output_up) {
                ui.ask_paint();
            }
            return;
        }
        let by = if turned {
            crate::surface::wheel_lines(&mut hand.lines_owed)
        } else {
            0
        };
        // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER, and not the list
        // under it: the gesture belongs to what is under the pointer.
        if by != 0 && wheel_at.is_some_and(|y| ui.hinted.turned(head, y, by)) {
            ui.ask_paint();
            return;
        }
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
                let press = if by > 0 { Press::Down } else { Press::Up };
                let errand = self.view.key(press);
                self.apply(ui, errand);
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
        if explained || before != (hand.row_under_pointer, self.view.cursor(), self.output_up) {
            ui.ask_paint();
        }
    }
}

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

    /// A view of the declared steps with nothing reported — no file of the
    /// machine is read.
    fn a_view() -> View {
        View::of(declared::steps(), Verdicts::default(), Vec::new())
    }

    fn key(notation: &str) -> Option<Press> {
        press_of_key(ronoctua_keymap::Key::parse(notation).expect("a key"))
    }

    /// BOTH DOORS READ A KEY ONE WAY: every row of the table is its own
    /// press, the three untaught ones are the window's old keys, and a chord
    /// that is not in the table is nothing.
    #[test]
    fn every_taught_key_is_a_press_and_a_chord_is_not_its_letter() {
        assert_eq!(key("<down>"), Some(Press::Down));
        assert_eq!(key("<up>"), Some(Press::Up));
        assert_eq!(key("C-j"), Some(Press::OutputDown));
        assert_eq!(key("C-k"), Some(Press::OutputUp));
        assert_eq!(key("G"), Some(Press::OutputEnd));
        assert_eq!(key("u"), Some(Press::Start));
        assert_eq!(key("a"), Some(Press::Abort));
        assert_eq!(key("p"), Some(Press::Proceed));
        assert_eq!(key("ESC"), Some(Press::Escape));
        assert_eq!(key("q"), Some(Press::Escape));
        assert_eq!(key("j"), Some(Press::Down));
        assert_eq!(key("k"), Some(Press::Up));
        assert_eq!(key("C-<down>"), Some(Press::OutputDown));
        for chord in ["C-u", "M-a", "C-p", "C-q", "M-j"] {
            assert_eq!(key(chord), None, "`{chord}` was taken for its letter");
        }
        for (taught, id) in OF_THE_VIEW {
            assert!(
                key(taught).is_some(),
                "`{taught}` ({id}) is taught and reads as nothing"
            );
        }
    }

    /// WHAT A KEY ASKS FOR is the same errand in both doors: the cursor moves
    /// inside the steps, the output moves on each door's own page, and the
    /// three deeds are errands the door carries out with the same sentences.
    #[test]
    fn a_key_is_one_errand_in_both_doors() {
        let mut v = a_view();
        let last = v.steps().len() - 1;
        assert_eq!(v.key(Press::Up), Errand::Moved);
        assert_eq!(v.cursor(), 0);
        for _ in 0..v.steps().len() + 3 {
            v.key(Press::Down);
        }
        assert_eq!(v.cursor(), last, "the cursor walked past the last step");
        assert_eq!(v.key(Press::OutputUp), Errand::Output(Output::Up));
        assert_eq!(v.key(Press::OutputDown), Errand::Output(Output::Down));
        assert_eq!(v.key(Press::OutputEnd), Errand::Output(Output::End));
        assert_eq!(v.key(Press::Start), Errand::Start);
        assert_eq!(v.key(Press::Abort), Errand::Answer(false));
        assert_eq!(v.key(Press::Proceed), Errand::Answer(true));
        assert_eq!(v.key(Press::Escape), Errand::Close);
        // A CLICK PUTS THE CURSOR ON THE STEP; on the step it is on, like
        // `RET`, it does nothing more.
        assert_eq!(v.clicked(1), Errand::Moved);
        assert_eq!(v.cursor(), 1);
        assert_eq!(v.clicked(1), Errand::Nothing);
        assert_eq!(v.clicked(999), Errand::Nothing);
    }

    /// THE FOOTER'S REGION IS ONE LIST — the register, the steps, air and the
    /// output — and a window of it is the same rows as the whole, cut: the
    /// footer builds only what it draws, and the count says the same as the
    /// rows.
    #[test]
    fn the_footers_region_is_the_steps_then_the_output() {
        let t = texts();
        let mut v = a_view();
        let steps = v.steps().len();
        let whole = v.region_rows(0, usize::MAX, 80, &t);
        assert_eq!(whole.len(), v.region_len(80, &t));
        let register = v.region_cursor(80, &t);
        assert!(register >= 1, "the second register has no line");
        for (i, row) in whole.iter().enumerate().skip(register).take(steps) {
            assert_eq!(row.step, Some(i - register), "row {i} is not its step");
        }
        // NOTHING SAID YET is said, once, under a line of air.
        assert_eq!(whole.len(), register + steps + 2);
        let said: String = whole[register + steps + 1]
            .fields
            .iter()
            .map(|f| f.text.as_str())
            .collect();
        assert_eq!(said, t.t(ronoctua_i18n::key::UPDATE_NO_OUTPUT));

        for n in 0..50 {
            v.heard(format!("line {n}"));
        }
        let whole = v.region_rows(0, usize::MAX, 80, &t);
        assert_eq!(whole.len(), register + steps + 1 + 50);
        let last: String = whole
            .last()
            .map(|r| r.fields.iter().map(|f| f.text.as_str()).collect())
            .unwrap_or_default();
        assert_eq!(last, "line 49", "the output's last row is not its tail");
        let cut = v.region_rows(register + steps + 5, 3, 80, &t);
        assert_eq!(
            cut,
            whole[register + steps + 5..register + steps + 8].to_vec()
        );
        // NOTHING OF A ROW LEAKS PAST THE WIDTH.
        for row in &whole {
            for f in &row.fields {
                assert!(f.at + layout::width(&f.text) <= 80, "{:?}", f.text);
            }
        }
    }

    /// THE CURSOR IS A COLOUR in the footer, as every list there draws it:
    /// the step under the cursor has its name in the accent, and moving the
    /// cursor moves the colour and the row the page follows.
    #[test]
    fn the_footers_cursor_is_the_accent_on_the_steps_name() {
        let t = texts();
        let mut v = a_view();
        let accent = crate::chrome::Kind::ListedFact.tones(true).0;
        let lit = |v: &View| -> Vec<usize> {
            v.region_rows(0, usize::MAX, 80, &t)
                .iter()
                .filter(|r| r.step.is_some() && r.fields.iter().any(|f| f.color == accent))
                .filter_map(|r| r.step)
                .collect()
        };
        assert_eq!(lit(&v), vec![0]);
        let before = v.region_cursor(80, &t);
        v.key(Press::Down);
        assert_eq!(lit(&v), vec![1]);
        let rows = v.region_rows(0, usize::MAX, 80, &t);
        assert_eq!(rows[v.region_cursor(80, &t)].step, Some(1));
        assert!(v.region_cursor(80, &t) >= before);
    }

    /// STARTING SAYS WHICH DOOR IT WENT THROUGH, in both doors' words.
    #[test]
    fn starting_says_which_door_it_went_through() {
        let t = texts();
        let said: Vec<String> = [Started::Brain, Started::Systemd, Started::Nowhere]
            .into_iter()
            .map(|s| t.t(said_of(s)))
            .collect();
        assert!(said.iter().all(|s| !s.starts_with('[')), "{said:?}");
        assert_ne!(said[0], said[1]);
        assert_ne!(said[1], said[2]);
    }

    /// A window over the declared steps, with nothing followed.
    fn a_window() -> (UpdateWindow, Ui) {
        (
            UpdateWindow::of(a_view(), Vec::new()),
            Ui::for_tests(texts()),
        )
    }

    fn typed(w: &mut UpdateWindow, ui: &mut Ui, notation: &str) {
        let (event, mods) =
            crate::door::keys::event_of(ronoctua_keymap::Key::parse(notation).expect("a key"));
        w.typed(ui, &event, mods);
    }

    /// THE WINDOW TEACHES WHAT THE FOOTER TEACHES, every key a click — `u`,
    /// `a` and `p` included.
    ///
    /// THE DEFECT THIS CATCHES: the window only watched. It had no header, so
    /// no clickable key, and it could neither start an update nor answer the
    /// countdown the footer answers.
    #[test]
    fn the_windows_header_teaches_the_views_keys() {
        let (w, ui) = a_window();
        let hint = w.hint(&ui);
        let words: String = hint.iter().map(|p| p.text.clone()).collect();
        for id in [START, ABORT, PROCEED, "update-output-end"] {
            assert!(
                words.contains(&ui.texts.t(id)),
                "`{id}` is not taught: {words}"
            );
        }
        assert!(w.header_keys(), "the header's drawer is not this window's");
    }

    /// THE COUNTDOWN IS ANSWERED HERE, with the footer's sentence: with no
    /// brain, or nothing announced, "there is nothing about to happen" — and
    /// nothing is handed to the footer.
    #[test]
    fn the_countdown_is_answered_here_with_the_footers_words() {
        let (mut w, mut ui) = a_window();
        typed(&mut w, &mut ui, "a");
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(ronoctua_i18n::key::ECHO_BRAIN_IDLE).as_str())
        );
        w.mind = Some(ronoctua_brain::Mind::default());
        typed(&mut w, &mut ui, "p");
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(ronoctua_i18n::key::ECHO_BRAIN_IDLE).as_str())
        );
        assert!(ui.orders().is_empty(), "an answer went to the footer");
        typed(&mut w, &mut ui, "ESC");
        assert!(ui.quit);
    }

    /// THE OUTPUT MOVES ON THIS DOOR'S PAGE: `C-k` a page up from the tail,
    /// `C-j` back down, `G` to the end — and an arrow walks the steps.
    #[test]
    fn the_output_and_the_steps_move_on_the_windows_page() {
        let (mut w, mut ui) = a_window();
        for n in 0..500 {
            w.view.heard(format!("{n}"));
        }
        typed(&mut w, &mut ui, "C-k");
        let page = w.output_room();
        assert!(page > 0);
        assert_eq!(w.output_up, page);
        typed(&mut w, &mut ui, "C-k");
        assert_eq!(w.output_up, 2 * page);
        typed(&mut w, &mut ui, "C-j");
        assert_eq!(w.output_up, page);
        typed(&mut w, &mut ui, "G");
        assert_eq!(w.output_up, 0);
        typed(&mut w, &mut ui, "<down>");
        typed(&mut w, &mut ui, "<down>");
        assert_eq!(w.view.cursor(), 2);
        assert!(ui.message.is_none());
    }
}
