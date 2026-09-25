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

use crate::door::{Canvas, Content, Door, Hand, LEFT_BUTTON, Ui};
use anyhow::Result;
use ronoctua_brain::tasks::update as declared;
use ronoctua_render::layout;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::{channel, timer},
    seat::{
        keyboard::{KeyEvent, Keysym, Modifiers},
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
    crate::door::run(|w| {
        // THE TWO JOURNALS, each on a thread of its own.
        //
        // The root half runs first and the person's half BLOCKS on it —
        // `systemctl start` on a `Type=oneshot` returns when the work is done —
        // so the two are never producing at the same time except at the
        // hand-over. Arrival order is the sequence's order, and nothing here
        // claims more than that.
        let (sender, said) = channel::channel::<String>();
        let mut children = Vec::new();
        children.extend(follow(SYSTEM_UNIT, false, &sender));
        children.extend(follow(declared::UNIT, true, &sender));
        w.handle
            .insert_source(said, |event, (), door: &mut Door<UpdateWindow>| {
                if let channel::Event::Msg(line) = event {
                    // NOTHING IS PAINTED HERE. Every line that has arrived
                    // lands in the scrollback, and the frame is owed once the
                    // queue is empty — see `Scrollback` for what a redraw per
                    // line costs, and `each_turn` for where the debt is paid.
                    door.content.scrollback.push(line);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the journal: {e}"))?;

        w.handle
            .insert_source(
                timer::Timer::from_duration(BEAT),
                |_, (), door: &mut Door<UpdateWindow>| {
                    door.content.reread_if_moved(&mut door.ui);
                    timer::TimeoutAction::ToDuration(BEAT)
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the clock: {e}"))?;

        Ok(UpdateWindow {
            steps: declared::steps(),
            verdicts: Verdicts::read(),
            stamp: report_stamp(),
            scrollback: Scrollback::default(),
            cursor: 0,
            step_offset: 0,
            output_up: 0,
            lines: 24,
            children,
        })
    })
}

/// The window.
struct UpdateWindow {
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
    /// The window's lines, as the door last counted them — kept here so the
    /// arithmetic of the two halves can be asked from a key, which does not
    /// carry the door's count with it.
    lines: u16,
    /// The two `journalctl -f`, so they can be killed.
    children: Vec<std::process::Child>,
}

impl Drop for UpdateWindow {
    /// WHAT I STARTED, I STOP. `journalctl -f` never ends on its own, and a
    /// door that leaves two of them behind every time it is closed is a door
    /// that leaks processes.
    fn drop(&mut self) {
        for mut child in self.children.drain(..) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
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
    fn reread_if_moved(&mut self, ui: &mut Ui) {
        let now = report_stamp();
        if now == self.stamp {
            return;
        }
        self.stamp = now;
        self.verdicts = Verdicts::read();
        ui.ask_paint();
    }

    /// The output's bar, as a scroll from the TOP.
    ///
    /// This window scrolls from the TAIL — `output_up` is how many lines above
    /// the end the band sits — so the offset from the top is what is left
    /// over. One arithmetic, read by the drawing and by the pointer.
    fn output_scroll(&self) -> ronoctua_design::Scroll {
        let room = self.output_room();
        let total = self.scrollback.amount();
        crate::chrome::list_scroll(total, room, total.saturating_sub(self.output_up + room))
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
    fn press(&mut self, ui: &mut Ui, key: &ronoctua_keymap::Key) {
        use ronoctua_keymap::Sym;
        let page = self.output_room().max(1);
        match key.sym {
            Sym::Esc => ui.quit = true,
            Sym::Char('q') => ui.quit = true,
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
        ui.ask_paint();
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
    /// table, which is what a list wants.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        if let Some(k) = key_of(event, mods) {
            self.press(ui, &k);
        }
    }

    /// Where content begins, in cells from the window's edge.
    fn margin(&self) -> u16 {
        CONTENT_MARGIN
    }

    /// THIS WINDOW DRAWS NO HEADER OF SHORTCUTS — line 0 is its name and line
    /// 1 the second register — so there is no drawer for the right `Ctrl` to
    /// walk, and `Esc` is this window's own: it closes it.
    fn header_keys(&self) -> bool {
        false
    }

    fn resized(&mut self, ui: &mut Ui) {
        self.lines = ui.lines;
        self.follow_the_cursor();
        self.follow_the_tail();
    }

    /// THE SCROLLBACK'S DEBT, paid once the queue is empty. A thousand lines of
    /// a rebuild arriving in one burst are one paint, not a thousand.
    fn each_turn(&mut self, ui: &mut Ui) {
        if self.scrollback.owes_a_frame() {
            self.follow_the_tail();
            ui.ask_paint();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        self.lines = ui.lines;
        let size = tokens::text::size();
        let line_px = tokens::metrics::line_height(size);
        let margin = ui.margin;
        let columns = ui.columns;
        let texts = ui.texts.clone();
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

        // LINE 0: whose window this is. A glyph and the sequence's own name —
        // no rule under it, which is the heading this project has (`chrome`).
        canvas.spans.push(span(
            tokens::icon::SYNC.to_string(),
            margin + GUTTER,
            0,
            tokens::color::TEXT_SECONDARY,
        ));
        canvas.spans.push(span(
            layout::elide_end(
                &texts.t(ronoctua_i18n::key::CMD_SYSTEM_UPDATE),
                columns.saturating_sub(GUTTER + COLUMN_GAP + EDGE_RIGHT),
            ),
            margin + GUTTER + COLUMN_GAP,
            0,
            tokens::color::TEXT_PRIMARY,
        ));

        // LINE 1: the second register — what the step under the cursor really
        // is, and with no steps to explain, how this window is driven. What the
        // last order said takes it when there is one.
        let second = ui.message.clone().unwrap_or_else(|| {
            self.steps.get(self.cursor).map_or_else(
                || texts.t(ronoctua_i18n::key::UPDATE_HINT),
                |step| {
                    let (verdict, _) = self.verdicts.of_step(step.id);
                    cursor_explains(step, &verdict, &texts)
                },
            )
        });
        canvas.spans.push(span(
            layout::elide_end(&second, columns),
            margin,
            1,
            tokens::color::TEXT_SECONDARY,
        ));

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
            let y = FIRST_OF_STEPS + u16::try_from(n).unwrap_or(0);
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
            //
            // AND THE BAND IS THE WHOLE WIDTH: the only bar this window draws
            // is the output's, below the table, so no column of these rows is
            // a bar's.
            let band = if row == self.cursor {
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
            let (verdict, seconds) = self.verdicts.of_step(step.id);
            for piece in step_row(step, &verdict, seconds, columns, &texts) {
                canvas
                    .spans
                    .push(span(piece.text, margin + piece.at, y, piece.color));
            }
        }

        // THE LINE THAT SAYS WHERE THE TABLE ENDS. A rule and nothing else —
        // no panel, no border, no shadow (invariant 15) — and it rides the top
        // edge of the output's first row rather than costing a row of its own.
        let top = self.output_top();
        canvas
            .under
            .push(crate::chrome::rule(width, f32::from(top) * line_px));

        // THE OUTPUT. Machine text, drawn as it came — sanitised for the grid
        // and never translated: a build log is not interface.
        if self.scrollback.is_empty() {
            canvas.spans.push(span(
                layout::elide_end(&texts.t(ronoctua_i18n::key::UPDATE_NO_OUTPUT), columns),
                margin + GUTTER,
                top,
                tokens::color::TEXT_DIM,
            ));
        } else {
            let room = self.output_room();
            for (n, said) in self
                .scrollback
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
        let before = (hand.row_under_pointer, self.cursor, self.output_up);
        let mut turned = false;
        let mut over_the_steps = false;
        // THE OUTPUT'S BAR IS A CONTROL. This window counts from the TAIL, so
        // an offset from the top has to be turned back into `output_up` — see
        // `output_scroll`, which does the same conversion the other way round.
        let room = self.output_room();
        let total = self.scrollback.amount();
        let scroll = self.output_scroll();
        let top = self.output_top();
        #[allow(clippy::cast_precision_loss)]
        let (bar_w, bar_h) = (ui.width as f32, ui.height as f32);
        let mut dragged: Option<usize> = None;
        for e in events {
            #[allow(clippy::cast_possible_truncation)]
            let (px, py) = (e.position.0 as f32, e.position.1 as f32);
            match e.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    if hand.dragging {
                        dragged =
                            Some(crate::chrome::list_offset_at(scroll, top, bar_w, bar_h, py));
                    } else {
                        hand.row_under_pointer = self.at(e.position.0, e.position.1);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    // AND THE DRAG ENDS WITH THE POINTER — see the same arm
                    // in the list windows.
                    hand.dragging = false;
                    hand.row_under_pointer = None;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    hand.dragging = false;
                }
                // A CLICK MOVES THE CURSOR, which is what an arrow does. There
                // is nothing else on this window to do to a step: switching one
                // on and off belongs to the brain's screen, where the person's
                // choice is written down.
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if crate::chrome::on_the_list_bar(scroll, top, bar_w, bar_h, px, py) {
                        hand.dragging = true;
                        dragged =
                            Some(crate::chrome::list_offset_at(scroll, top, bar_w, bar_h, py));
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
                    crate::surface::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
                    turned = true;
                }
                _ => {}
            }
        }
        if let Some(offset) = dragged {
            // BACK TO THE TAIL'S OWN COUNT. And nothing else this frame: a
            // wheel tick arriving during a drag would move the list twice,
            // which the list windows avoid by returning here.
            self.output_up = total.saturating_sub(offset + room);
            self.follow_the_tail();
            if before != (hand.row_under_pointer, self.cursor, self.output_up) {
                ui.ask_paint();
            }
            return;
        }
        let by = if turned {
            crate::surface::wheel_lines(&mut hand.lines_owed)
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
        if before != (hand.row_under_pointer, self.cursor, self.output_up) {
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
}
