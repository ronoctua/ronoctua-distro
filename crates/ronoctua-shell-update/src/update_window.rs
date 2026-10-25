//! The update — the mark on the modeline, the door that starts it, and the
//! view that watches it happen, in the footer and in a window.
//!
//! # What this module may NOT know
//!
//! **A step.** Not one id, not one command, not one order. The sequence is
//! declared once, in the update's brain module (`system-update`, its
//! `steps.nix`), and everything downstream is generated from it: the Rust
//! table, the two text keys per step, the two shell runners and the systemd
//! units. The brain's module reads that table — the brain used to declare
//! three steps of its own, and they came out DISJOINT from the twelve the
//! sequence actually runs — and this file reads the same table through the
//! same module ([`module::steps`]), with the keys it declared for each step.
//!
//! So `LEADER s u n` TRIGGERS; it never enumerates. What it starts is the unit
//! the brain starts, named by the module's own manifest ([`unit::RUN`]), and
//! the steps the view draws are the module's and nothing else. The module's
//! `…-has-one-list` check keeps it that way: a step's id written by hand
//! anywhere outside the declaration takes the build down.
//!
//! # The one file outside the module that names it
//!
//! It is listed in the module's `namedBy`, and it is where the three commands
//! live — [`COMMANDS`] and their keys, [`BINDINGS`] — so the leader and the
//! footer reach them through these tables and never spell an id of the
//! module's themselves ([`order_of`] is how the footer asks).
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
//! Together in one file because they are the halves of one feature, and
//! splitting them would hide which is which.
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
use crate::key;
use anyhow::Result;
use ronoctua_brain::compiled::system_update as module;
/// The battery as the update weighs it — the footer asks it before `LEADER
/// s u n` starts anything, through this file, which is the one outside the
/// module that names it.
pub use ronoctua_brain::compiled::system_update::Charge;
use ronoctua_brain::compiled::system_update::unit;
use ronoctua_render::layout;
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::{channel, timer},
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

use crate::update_region::{Back, Open, Reach, carry_out};

// ---------------------------------------------------------------------------
// THE MARK ON THE MODELINE
// ---------------------------------------------------------------------------

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
        task: module::ID.to_owned(),
        rehearsal: false,
    };
    match ronoctua_brain::client::tell(&ask) {
        Ok(()) => {
            tracing::info!(task = module::ID, "update asked of the brain");
            return Started::Brain;
        }
        Err(e) => tracing::debug!(error = %e, "no brain; asking systemd directly"),
    }
    begin_directly()
}

/// Starts the whole sequence straight through systemd — [`begin`]'s second
/// door, and the one a "yes" to the network goes through: the brain would
/// hold it.
fn begin_directly() -> Started {
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
            if let Err(e) = ronoctua_brain::systemd::start(unit::RUN) {
                tracing::error!(unit = unit::RUN, error = %e, "systemd refused");
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

/// Starts the whole sequence ON BATTERY, after the person said yes.
///
/// # Why straight to systemd, and why a file
///
/// The brain WAITS on a battery below the floor — that is what a scheduled
/// run must do, and it asks nobody — so a yes given here would sit in its
/// queue until the charger came. The person answered now; the run starts
/// now, the way it starts with no brain at all ([`begin`]'s second door).
///
/// The yes itself is a file in the person's runtime directory
/// ([`ON_BATTERY`]): the sequence's person half reads it when it starts —
/// and tells its root half, which tests it — and takes it away however the
/// run ends. It is for this run only; nothing else writes it, and a
/// scheduled run never finds one.
pub fn begin_on_battery() -> Started {
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
        tracing::warn!("no runtime directory: the yes to a run on battery has nowhere to go");
        return Started::Nowhere;
    };
    let yes = PathBuf::from(runtime).join(ON_BATTERY);
    let written = yes
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&yes, b""));
    if let Err(e) = written {
        tracing::warn!(file = %yes.display(), error = %e, "the yes to a run on battery was not written");
        return Started::Nowhere;
    }
    if !ronoctua_brain::systemd::reachable() {
        let _ = std::fs::remove_file(&yes);
        tracing::warn!("this build does not know where systemctl is");
        return Started::Nowhere;
    }
    // A YES NO RUN TOOK IS TAKEN BACK: a start systemd refused leaves no run
    // to remove it, and the next run — a scheduled one, which must never
    // find one — would honour it.
    let given = yes.clone();
    let born = std::thread::Builder::new()
        .name("update-on-battery".to_owned())
        .spawn(move || {
            if let Err(e) = ronoctua_brain::systemd::start(unit::RUN) {
                let _ = std::fs::remove_file(&given);
                tracing::error!(unit = unit::RUN, error = %e, "systemd refused");
            }
        });
    match born {
        Ok(_) => Started::Systemd,
        Err(e) => {
            let _ = std::fs::remove_file(&yes);
            tracing::error!(error = %e, "could not start the update thread");
            Started::Nowhere
        }
    }
}

/// Starts the run the person said yes to, here and now: straight through
/// systemd — with the yes to a run on battery when it is that one
/// ([`begin_on_battery`]) — after taking the update out of the brain's
/// queue, where it would otherwise run AGAIN once what held it is gone.
pub fn begin_anyway(mind: Option<&ronoctua_brain::Mind>, on_battery: bool) -> Started {
    withdraw(mind);
    if on_battery {
        begin_on_battery()
    } else {
        begin_directly()
    }
}

/// Takes the update out of the brain's queue when it is waiting there — and
/// never when it is running: the brain's `abort` would stop it.
fn withdraw(mind: Option<&ronoctua_brain::Mind>) {
    let Some(m) = mind else {
        return;
    };
    if m.running == module::ID || !m.queue.iter().any(|q| q == module::ID) {
        return;
    }
    let ask = ronoctua_brain::Ask::Abort {
        task: module::ID.to_owned(),
    };
    if let Err(e) = ronoctua_brain::client::tell(&ask) {
        tracing::warn!(error = %e, "the update could not be taken out of the brain's queue");
    }
}

/// What the minibuffer says when the person answered no to a run on battery
/// — in both doors, from here: the footer names none of the module's words.
pub const NOT_STARTED: &str = module::key::ECHO_SYSTEM_UPDATE_NOT_STARTED;

/// What it says when the person answered no to a run the brain holds: it
/// stays in the queue, and goes by itself when nothing holds it.
pub const KEPT_WAITING: &str = module::key::ECHO_SYSTEM_UPDATE_KEPT_WAITING;

/// How long, in seconds, a run asked of the brain is followed before it was
/// even seen in the queue: the brain queues it before it answers, so a mind
/// that still does not show it this long after says nothing of it.
const FOLLOWED_FOR_S: i64 = 60;

/// A run asked for BY HAND through the brain, followed until the brain says
/// what became of it — so a hold for a metered way out
/// ([`module::HOLD_METERED`]), which is how the brain waits on a SCHEDULED
/// run, becomes a question in the door that asked: the person asked for
/// this one now.
///
/// It is the brain's own decision that is followed, from the mind both
/// doors already receive, and not a second reading of the network: the door
/// asks exactly when the brain holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Followed {
    /// When it was asked for, in Unix seconds — the clock the brain's `now`
    /// is on.
    since_s: i64,
    /// Whether a mind has shown it queued: before that, a mind that does
    /// not is one published before the ask.
    seen: bool,
}

/// What a mind says of a [`Followed`] run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Heard {
    /// Still to say: not seen yet, or queued and held by something else.
    Waiting,
    /// Queued, and held for a metered way out: ask.
    Metered,
    /// Running, or out of the queue: nothing more to follow.
    Over,
}

impl Followed {
    /// Asked for at `since_s`.
    #[must_use]
    pub const fn since(since_s: i64) -> Self {
        Self {
            since_s,
            seen: false,
        }
    }

    /// Asked for now.
    #[must_use]
    pub fn now() -> Self {
        Self::since(now_s())
    }

    /// What `mind` says of it.
    pub fn heard(&mut self, mind: &ronoctua_brain::Mind) -> Heard {
        if mind.running == module::ID {
            return Heard::Over;
        }
        if mind.queue.iter().any(|q| q == module::ID) {
            self.seen = true;
            let held = mind.task(module::ID).and_then(|t| t.hold);
            return if held == Some(module::HOLD_METERED) {
                Heard::Metered
            } else {
                Heard::Waiting
            };
        }
        if self.seen || mind.now > self.since_s.saturating_add(FOLLOWED_FOR_S) {
            Heard::Over
        } else {
            Heard::Waiting
        }
    }
}

/// Now, in Unix seconds.
fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Where the person's yes to a run on battery is, under their runtime
/// directory — the leaf the sequence's runner reads (`gen-updates.py`,
/// `ON_BATTERY`), which the module's `…-names-agree` check holds to this.
const ON_BATTERY: &str = "ronoctua-update/on-battery";

// ---------------------------------------------------------------------------
// WHAT THE UNITS REPORTED
// ---------------------------------------------------------------------------

// THE TWO HALVES' UNITS are the module's manifest's (`unit::RUN`, the
// person's half, and `unit::SYSTEM`, the root half): written there once, read
// by its NixOS half into the units and the rule, and by this file from the
// table generated out of it. They used to be typed here again, and a check
// compared the two spellings; now there is one.

/// Where the root half leaves its report.
///
/// `/run`, because none of it survives a boot and none of it should — the
/// `stateDir` of the module's NixOS half, written `0755` root so the person can
/// READ it, which is what makes this window possible without privilege. The
/// module's `…-names-agree` check holds the two to each other.
const SYSTEM_REPORT: &str = "/run/ronoctua-update/system.report";

/// The person's half's report, under their own runtime directory — where the
/// generated runner writes it, which the same check reads.
const PERSON_REPORT: &str = "ronoctua-update/user.report";

/// `journalctl`, by store path and never by name.
///
/// Under a systemd unit the `PATH` is minimal and a bare name fails far from
/// its cause. Outside Nix — `cargo run` — the shell's own `PATH` resolves it,
/// which is the shape `ronoctua_shell_door::brainlink` already uses for
/// `systemctl`.
const JOURNALCTL: &str = match option_env!("RONOCTUA_JOURNALCTL") {
    Some(p) => p,
    None => "journalctl",
};

/// What a runner wrote beside a step.
///
/// # A closed vocabulary, and it is not ours
///
/// the update's `gen-updates.py` emits exactly these words — `off`,
/// `skipped-<kind>`, `rehearsed`, `ran`, `failed`, `failed-<word>`,
/// `declined-<word>`, `stopped` — and its own
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
    /// It ran, and its program REFUSED with this word (`failed-<word>`): a
    /// failure, with the reason the program gave — the machine's sentence
    /// for it, when the word is the machine's ([`refusal_key`]).
    Refused(String),
    /// This machine has it switched off, in `/etc/ronoctua/updates.conf`.
    Off,
    /// A precondition did not hold. The word is the KIND — `binary`,
    /// `directory`, `writable-file`, `network` — and the module says which key
    /// explains it ([`module::needs_key`]).
    Skipped(String),
    /// A rehearsal printed it and ran nothing.
    Rehearsed,
    /// The program looked and had nothing new to do — a refusal the step
    /// declares as news (`declined-<word>`), and the module says which key
    /// explains it ([`module::declined_key`]).
    Declined(String),
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
            Self::Ran => module::key::SYSTEM_UPDATE_VERDICT_RAN,
            Self::Failed | Self::Refused(_) => module::key::SYSTEM_UPDATE_VERDICT_FAILED,
            Self::Off => module::key::SYSTEM_UPDATE_VERDICT_OFF,
            // THE ROW SAYS *SKIPPED* AND THE CURSOR SAYS WHY. The kind's own
            // sentence is a whole line long (*"this program is not on your
            // PATH"*), which is right for the second register and far too wide
            // for a column.
            Self::Skipped(_) => module::key::SYSTEM_UPDATE_VERDICT_SKIPPED,
            Self::Rehearsed => module::key::SYSTEM_UPDATE_VERDICT_REHEARSED,
            Self::Declined(_) => module::key::SYSTEM_UPDATE_VERDICT_DECLINED,
            Self::Stopped => module::key::SYSTEM_UPDATE_VERDICT_STOPPED,
            Self::Pending => module::key::SYSTEM_UPDATE_VERDICT_PENDING,
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
            Self::Failed | Self::Refused(_) | Self::Stopped => {
                (tokens::icon::ERROR, tokens::color::STATE_ERROR)
            }
            // NOTHING NEW is a fact about the machine, not a failure: the
            // mark a rehearsal has, quiet.
            Self::Rehearsed | Self::Declined(_) => {
                (tokens::icon::INFO, tokens::color::TEXT_SECONDARY)
            }
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
        other => {
            if let Some(kind) = other.strip_prefix("skipped-") {
                return (!kind.is_empty()).then(|| Verdict::Skipped(kind.to_owned()));
            }
            if let Some(word) = other.strip_prefix("failed-") {
                return (!word.is_empty()).then(|| Verdict::Refused(word.to_owned()));
            }
            other
                .strip_prefix("declined-")
                .filter(|word| !word.is_empty())
                .map(|word| Verdict::Declined(word.to_owned()))
        }
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
///
/// The root half's under the footer camera's room while it photographs
/// ([`crate::photo::room`], its `run/`), so a photograph of the
/// footer draws the reports it was given and never this machine's.
fn report_paths() -> Vec<PathBuf> {
    let system = match crate::photo::room() {
        Some(room) => room.join(SYSTEM_REPORT.trim_start_matches('/')),
        None => PathBuf::from(SYSTEM_REPORT),
    };
    let mut paths = vec![system];
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

/// When the reports — and the machine's records the view draws above the
/// steps — last changed, so a beat can decide whether to re-read.
///
/// One `stat` per file per second — the device the file manager already uses to
/// notice a directory moved. Re-reading unconditionally would parse both files
/// once a second for a machine that updates once a week.
fn report_stamp() -> Vec<Option<std::time::SystemTime>> {
    report_paths()
        .into_iter()
        .chain(record_places().files())
        .map(|p| std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
        .collect()
}

/// Where the machine's records are: this machine's — or, while the footer's
/// camera photographs, its room's (`machine/` for the machine directory,
/// `machine-state/` for the record's folder; the belongings already follow
/// the room's XDG directories). Absolute places of the machine are what a
/// room of XDG directories alone could not plant, and the footer's door of
/// this view drew only the conflicts in a photograph.
fn record_places() -> module::RecordPlaces {
    let mut at = module::RecordPlaces::here();
    if let Some(room) = crate::photo::room() {
        at.machine_dir = room.join(crate::photo::MACHINE_DIR);
        at.machine_state = room.join(crate::photo::MACHINE_STATE);
    }
    at
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
/// `ronoctua_shell_door::launch::painter` and `ronoctua_shell_door::queue::listen` already have.
///
/// The [`std::process::Child`] comes back so the window can KILL it: `-f`
/// never ends on its own, and a door that leaves two `journalctl` behind every
/// time it is closed is a door that leaks processes.
///
/// # Several units, one `journalctl`
///
/// A half is two units — the one that acts and the one that rehearses — and
/// `-u` given twice matches either, in the order the lines were written. One
/// child per half, then, not per unit: the backlog of `-n` is the half's last
/// lines, whichever of the two wrote them.
fn follow(
    units: &[&str],
    of_the_person: bool,
    to: &crate::region::Post<Back>,
) -> Option<std::process::Child> {
    let first = units.first().copied().unwrap_or_default();
    let mut command = std::process::Command::new(JOURNALCTL);
    if of_the_person {
        command.arg("--user");
    }
    command.args([
        "--no-pager",
        // Just the message. The window has narrow columns and the stamp,
        // the host and the unit are the same on every line of a run.
        "-o",
        "cat",
        "-n",
        &KEPT.to_string(),
        "-f",
    ]);
    for unit in units {
        command.args(["-u", unit]);
    }
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            // NOT FATAL. Without it the window loses the live half and still
            // draws the steps and their verdicts — the same criterion
            // `queue::listen` uses about a missing socket.
            tracing::warn!(unit = first, error = %e, "no journalctl; the live output does not appear");
            return None;
        }
    };
    let out = child.stdout.take()?;
    let sender = to.clone();
    let named = std::thread::Builder::new()
        .name(format!("journal-{first}"))
        .spawn(move || {
            for line in std::io::BufReader::new(out).lines() {
                let Ok(text) = line else {
                    return;
                };
                if !sender.send(Back::Line(readable(&text))) {
                    // The other end has gone: the door is closing.
                    return;
                }
            }
        });
    if let Err(e) = named {
        tracing::error!(unit = first, error = %e, "could not start the journal thread");
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
/// The same shape `ronoctua_shell_footer::brain::Piece` has, and for the same reason: the
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
    if matches!(
        verdict,
        Verdict::Ran | Verdict::Failed | Verdict::Refused(_)
    ) {
        let elapsed = clock_of(seconds);
        right = right.saturating_sub(COLUMN_GAP + layout::width(&elapsed));
        out.push(Piece {
            at: right,
            text: elapsed,
            color: tokens::color::TEXT_DIM,
        });
    }

    // THE STEP'S OWN KEY, AND NOT A FAMILY OF OUR OWN. The ids are not this
    // shell's: the module declared each step's name and sentence in both
    // languages — the same keys the brain's own screen draws — and the step
    // carries them. A key built here out of the id would be a second family
    // for the same steps, and a word nobody declared.
    let at = GUTTER + COLUMN_GAP;
    let room = right.saturating_sub(at.saturating_add(COLUMN_GAP));
    out.push(Piece {
        at,
        text: layout::elide_end(&texts.t(step.name_key), room),
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
    let mut said = texts.t(step.does_key);
    // A KIND THE SEQUENCE DOES NOT HAVE SAYS NOTHING, rather than a key made
    // up of it: the row already says *skipped*.
    if let Verdict::Skipped(kind) = verdict
        && let Some(why) = module::needs_key(kind)
    {
        said.push(' ');
        said.push_str(&texts.t(why));
    }
    // AND NOTHING NEW SAYS WHY THERE WAS NOTHING, in the module's words.
    if let Verdict::Declined(word) = verdict
        && let Some(why) = module::declined_key(word)
    {
        said.push(' ');
        said.push_str(&texts.t(why));
    }
    // A REFUSAL SAYS WHY, in the machine's own words — and a word that is
    // not the machine's says nothing more than the row's *failed*.
    if let Verdict::Refused(word) = verdict
        && let Some(why) = refusal_key(word)
    {
        said.push(' ');
        said.push_str(&texts.t(why));
    }
    if step.needs_root {
        said.push(' ');
        said.push_str(&texts.t(crate::key::BRAIN_NEEDS_ROOT));
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
    ("<down>", "system-update-next-step"),
    ("<up>", "system-update-previous-step"),
    ("C-j", "system-update-output-down"),
    ("C-k", "system-update-output-up"),
    ("G", "system-update-output-end"),
    ("u", START),
    ("a", ABORT),
    ("p", PROCEED),
    // THE PERSON'S BINARY CACHE, a page of this view (`crate::binary_cache`):
    // `c` turns the view to it and back.
    ("c", crate::binary_cache::OPEN),
    // THE DISTRO'S APPS THAT FOLLOW THEIR RELEASES, another page
    // (`crate::follow`): `f` turns the view to it and back.
    ("f", crate::follow::OPEN),
    // THE MACHINE'S PARTS, a third (`crate::parts`): `d`, for the downloads
    // they wait for, turns the view to it and back.
    ("d", crate::parts::OPEN),
];

/// The keys the binary cache's page teaches, in both doors — every one a
/// click, pressed by its notation through [`press_of_key`]: the rows, the
/// change, and `c` back to the steps (the way back, `Esc`, too).
pub const OF_THE_PAGE: &[(&str, &str)] = &[
    ("<down>", "binary-cache-down"),
    ("<up>", "binary-cache-up"),
    ("RET", "binary-cache-enter"),
    ("c", "binary-cache-steps"),
];

/// The keys the page of the apps that follow their releases teaches, in
/// both doors — every one a click, pressed by its notation through
/// [`press_of_key`]: the rows, the switch, and `f` back to the steps (the
/// way back, `Esc`, too).
pub const OF_THE_FOLLOW: &[(&str, &str)] = &[
    ("<down>", "distro-follow-down"),
    ("<up>", "distro-follow-up"),
    ("RET", "distro-follow-enter"),
    ("f", "distro-follow-steps"),
];

/// The keys the parts' page teaches, in both doors — every one a click,
/// pressed by its notation through [`press_of_key`]: the rows, a try now, a
/// file handed in, and `d` back to the steps (the way back, `Esc`, too).
pub const OF_THE_PARTS: &[(&str, &str)] = &[
    ("<down>", "distro-parts-down"),
    ("<up>", "distro-parts-up"),
    ("RET", "distro-parts-enter"),
    (crate::parts::HAND_KEY, HAND),
    ("d", "distro-parts-steps"),
];

/// The parts' page's own key that hands in a file — the one key of a page
/// the steps do not teach.
const HAND: &str = "distro-parts-hand";

/// The command that starts the whole sequence now — the module's own id, and
/// the word its key teaches (`update now`).
pub const START: &str = module::ID;
/// The command that opens the view in the footer's region.
pub const OPEN: &str = "system-update-open";
/// The command that opens the view in a window — the door the modeline's
/// mark opens (`update_mark::WATCH`, which the footer runs).
pub const WATCH: &str = crate::update_mark::WATCH;
/// The countdown's "not now" — or "stop it", once it runs.
pub const ABORT: &str = "brain-abort";
/// The countdown's "start now".
pub const PROCEED: &str = "brain-proceed";

/// The update's three commands, which the leader registers with its own
/// (`KEYS`) without spelling one of them.
///
/// `START` IS THE WHOLE UPDATE, BY HAND, WITH NO BRAIN IN IT. The brain decides
/// WHEN the machine updates itself; this is the person deciding it is now. The
/// two are not two sequences: this starts the same unit the brain starts, and
/// neither of them holds a list of steps — the module's `steps.nix` does, and
/// everything else is generated from it. It works with the daemon down, on
/// purpose ([`begin`]): a scheduler that is not running must not be able to
/// stop a person from updating their own machine.
///
/// `OPEN` AND `WATCH` ARE THE VIEW THAT WATCHES ONE HAPPEN, in the footer's
/// region and in a window. They are commands of the registry and not keys of
/// a view for the reason `brain-abort` is: the mark appears on the footer's
/// own line, with no window open, and the mouse may not reach a door the
/// keyboard does not. And the window opens with NOTHING running, which is when
/// somebody wants to read what the last run did.
pub const COMMANDS: &[ronoctua_cmd::Command] = &[
    ronoctua_cmd::Command {
        id: START,
        label: module::key::CMD_SYSTEM_UPDATE,
    },
    ronoctua_cmd::Command {
        id: OPEN,
        label: module::key::CMD_SYSTEM_UPDATE_OPEN,
    },
    ronoctua_cmd::Command {
        id: WATCH,
        label: module::key::CMD_SYSTEM_UPDATE_WATCH,
    },
    // THE PERSON'S BINARY CACHE, the view's page (`crate::binary_cache`): its
    // door (`LEADER s u c`), the three questions it asks — commands, so the
    // minibuffer's answer comes back by name, and `LEADER :` asks them — and
    // root's netrc, a root gesture asked in the sudo bar.
    ronoctua_cmd::Command {
        id: crate::binary_cache::OPEN,
        label: key::CMD_BINARY_CACHE_OPEN,
    },
    ronoctua_cmd::Command {
        id: crate::binary_cache::NAME,
        label: key::CMD_BINARY_CACHE_NAME,
    },
    ronoctua_cmd::Command {
        id: crate::binary_cache::KEY,
        label: key::CMD_BINARY_CACHE_KEY,
    },
    ronoctua_cmd::Command {
        id: crate::binary_cache::TOKEN,
        label: key::CMD_BINARY_CACHE_TOKEN,
    },
    ronoctua_cmd::Command {
        id: crate::binary_cache::NETRC,
        label: key::CMD_BINARY_CACHE_NETRC,
    },
    // THE DISTRO'S APPS THAT FOLLOW THEIR RELEASES, the view's other page
    // (`crate::follow`): its door (`LEADER s u f`).
    ronoctua_cmd::Command {
        id: crate::follow::OPEN,
        label: key::CMD_DISTRO_FOLLOW_OPEN,
    },
    // THE MACHINE'S PARTS, the view's third page (`crate::parts`): its door
    // (`LEADER s u d`) and the four questions it asks — a file handed in,
    // a stick's folder each way, and the person's mirrors.
    ronoctua_cmd::Command {
        id: crate::parts::OPEN,
        label: key::CMD_DISTRO_PARTS_OPEN,
    },
    ronoctua_cmd::Command {
        id: crate::parts::HAND_IN,
        label: key::CMD_DISTRO_PARTS_HAND_IN,
    },
    ronoctua_cmd::Command {
        id: crate::parts::IMPORT,
        label: key::CMD_DISTRO_PARTS_IMPORT,
    },
    ronoctua_cmd::Command {
        id: crate::parts::EXPORT,
        label: key::CMD_DISTRO_PARTS_EXPORT,
    },
    ronoctua_cmd::Command {
        id: crate::parts::MIRRORS,
        label: key::CMD_DISTRO_PARTS_MIRRORS,
    },
];

/// `LEADER s u` — THE UPDATE, a group under SYSTEM, beside the memory panel
/// and the disk formatter: `s u u` opens its view (the doubled letter every
/// view uses) and `s u w` its window — `w` for WINDOW, the letter every view
/// with a second door uses — and `s u n` starts it NOW, the key the view
/// itself teaches as `u`. The leader takes these whole, as it takes its own.
pub const BINDINGS: &[(&str, &str, Option<&str>)] = &[
    ("s u u", OPEN, None),
    ("s u w", WATCH, None),
    ("s u n", START, None),
    // THE PERSON'S BINARY CACHE, straight to the view's page.
    ("s u c", crate::binary_cache::OPEN, None),
    // THE DISTRO'S APPS THAT FOLLOW THEIR RELEASES, straight to theirs.
    ("s u f", crate::follow::OPEN, None),
    // THE MACHINE'S PARTS, straight to theirs.
    ("s u d", crate::parts::OPEN, None),
];

/// What this crate adds to the leader's keymap ([`crate::bindings::Contribution`]):
/// its commands, the sequences and groups that reach them, the ones only
/// `LEADER :` reaches by name, and what the view's own keys and rows run.
/// The binary assembles every crate's (its `keymap`); the footer's own are
/// its `leader::BASE`.
pub const KEYS: crate::bindings::Contribution = crate::bindings::Contribution {
    owner: "update",
    commands: COMMANDS,
    bindings: BINDINGS,
    groups: &[("s u", crate::key::GROUP_UPDATE)],
    by_name: &[
        // THE PERSON'S BINARY CACHE (`crate::binary_cache`, a page of the
        // update's view): the three questions its rows ask — each opens its
        // door's line (`ask-<id>`) and the answer comes back as the same
        // command's argument — and root's netrc, which the page's row runs (its
        // password is the sudo bar's to ask). The page is `LEADER s u c`; a key
        // of each of these would ask a key of a cache nobody named.
        crate::binary_cache::NAME,
        crate::binary_cache::KEY,
        crate::binary_cache::TOKEN,
        crate::binary_cache::NETRC,
        // THE PARTS' PAGE (`crate::parts`): its four questions, each asked
        // by a key or a row of the page and answered by name.
        crate::parts::HAND_IN,
        crate::parts::IMPORT,
        crate::parts::EXPORT,
        crate::parts::MIRRORS,
    ],
    ..crate::bindings::Contribution::NONE
};

/// What one of the update's commands asks of the footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    /// The whole sequence, now ([`START`]).
    Start,
    /// The view, in the footer's region ([`OPEN`]).
    Open,
    /// The view, in a window ([`WATCH`]).
    Watch,
    /// One of the binary cache's commands (`crate::binary_cache`), by its id.
    Cache(&'static str),
    /// The page of the distro's apps that follow their releases
    /// (`crate::follow`).
    Follow,
    /// One of the parts' page's commands (`crate::parts`), by its id.
    Parts(&'static str),
}

/// Which of the update's commands an id is — the one question the footer asks
/// to dispatch them, so it spells none.
#[must_use]
pub fn order_of(id: &str) -> Option<Order> {
    match id {
        START => Some(Order::Start),
        OPEN => Some(Order::Open),
        WATCH => Some(Order::Watch),
        crate::binary_cache::OPEN => Some(Order::Cache(crate::binary_cache::OPEN)),
        crate::binary_cache::NAME => Some(Order::Cache(crate::binary_cache::NAME)),
        crate::binary_cache::KEY => Some(Order::Cache(crate::binary_cache::KEY)),
        crate::binary_cache::TOKEN => Some(Order::Cache(crate::binary_cache::TOKEN)),
        crate::binary_cache::NETRC => Some(Order::Cache(crate::binary_cache::NETRC)),
        crate::follow::OPEN => Some(Order::Follow),
        crate::parts::OPEN => Some(Order::Parts(crate::parts::OPEN)),
        crate::parts::HAND_IN => Some(Order::Parts(crate::parts::HAND_IN)),
        crate::parts::IMPORT => Some(Order::Parts(crate::parts::IMPORT)),
        crate::parts::EXPORT => Some(Order::Parts(crate::parts::EXPORT)),
        crate::parts::MIRRORS => Some(Order::Parts(crate::parts::MIRRORS)),
        _ => None,
    }
}

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
    /// `RET`: answers the question on battery with the answer under the
    /// cursor. On a step there is nothing more to do than the cursor did.
    Enter,
    /// Out — and, with the question open, its "no".
    Escape,
    /// The person's binary cache: the page, or back to the steps.
    Cache,
    /// The distro's apps that follow their releases: the page, or back to
    /// the steps.
    Follow,
    /// The machine's parts: the page, or back to the steps.
    Parts,
    /// On the parts' page, a file handed in for the part under the cursor.
    HandIn,
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
        Sym::Enter if plain => return Some(Press::Enter),
        Sym::Char('q') if plain => return Some(Press::Escape),
        Sym::Char('j') if plain => return Some(Press::Down),
        Sym::Char('k') if plain => return Some(Press::Up),
        Sym::Down if key.mods.ctrl => return Some(Press::OutputDown),
        Sym::Up if key.mods.ctrl => return Some(Press::OutputUp),
        _ => {}
    }
    let notation = key.notation();
    // THE VIEW'S KEYS FIRST: a page's key that shares the view's notation
    // (`d` back to the steps) is the view's key; the parts' page adds one.
    let id = OF_THE_VIEW
        .iter()
        .chain(OF_THE_PARTS)
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    Some(match id {
        "system-update-next-step" => Press::Down,
        "system-update-previous-step" => Press::Up,
        "system-update-output-down" => Press::OutputDown,
        "system-update-output-up" => Press::OutputUp,
        "system-update-output-end" => Press::OutputEnd,
        START => Press::Start,
        ABORT => Press::Abort,
        PROCEED => Press::Proceed,
        crate::binary_cache::OPEN => Press::Cache,
        crate::follow::OPEN => Press::Follow,
        crate::parts::OPEN => Press::Parts,
        HAND => Press::HandIn,
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
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Start it here and now, the person having said yes to what it spends
    /// ([`begin_anyway`]): on battery below the floor, or on the metered
    /// way out the brain held it for.
    StartHere {
        /// The yes is to a run on battery ([`begin_on_battery`]).
        on_battery: bool,
    },
    /// The person said no to a run on battery: say it did not start.
    NotStarted,
    /// The person said no to a run the brain holds: say it stays queued.
    KeptWaiting,
    /// Open the belongings, whose conflicts a row counts — the footer's
    /// region, or the belongings' window from the window.
    OpenBelongings,
    /// Answer the countdown — `true` to start now, `false` for "not now".
    Answer(bool),
    /// What the binary cache's page asks of the door: a question on its
    /// line, a sentence, a change (`crate::binary_cache::apply`).
    Cache(crate::binary_cache::Errand),
    /// What the page of the apps that follow their releases asks of the
    /// door: a sentence, a turn of the switch (`crate::follow::apply`).
    Follow(crate::follow::Errand),
    /// What the parts' page asks of the door: a question on its line, a
    /// sentence, a gesture for the machine (`crate::parts::carry`).
    Parts(crate::parts::Errand),
    /// The view closes.
    Close,
}

/// What a line of the view takes when it is clicked — the one answer the
/// painting, the pointer and the footer's click read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A step: the cursor goes there.
    Step(usize),
    /// An answer to the question — `true` is yes. The first click puts the
    /// cursor on it, the second answers (invariant 19).
    Answer(bool),
    /// The belongings, whose conflicts a line counts: one click opens them,
    /// as a key the line teaches does.
    Belongings,
    /// A row of the page the view is turned to (the binary cache's, or the
    /// apps' that follow their releases), by its index: the first click puts
    /// the cursor there, the second does what `RET` does.
    Page(usize),
    /// An answer of the binary cache page's grave question — `true` is yes.
    PageAnswer(bool),
}

/// What a question of the view is about: what a run asked for BY HAND would
/// spend that a scheduled one waits for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cost {
    /// A battery below the floor — its charge, in per cent, when it says it.
    Battery(Option<u8>),
    /// A metered way out, on which the brain holds the run asked for.
    Metered,
}

/// The question a run by hand asks before it spends what a scheduled one
/// waits for — "no" first, and the cursor born on it: it is a question that
/// appears on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Question {
    /// What it would spend.
    pub cost: Cost,
    /// Whether the brain holds this run in its queue: a "no" then leaves it
    /// there, waiting, and a "yes" takes it out before starting it here.
    pub queued: bool,
    /// Which answer the cursor is on.
    pub yes: bool,
}

/// One line at the top of the view — the question or the second register,
/// then what the machine's records say — placed and coloured, with what a
/// click on it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLine {
    /// What it says, already wrapped to the width.
    pub text: String,
    /// Its colour: the cursor's on an answer, danger's on what costs.
    pub color: tokens::Color,
    /// What a click on it takes, when anything.
    pub target: Option<Target>,
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
    /// What the machine's records say beside the run.
    records: module::Records,
    /// The keys that open the belongings, as this leader teaches them — for
    /// the line that counts their conflicts.
    belongings_keys: String,
    /// The question, while it is asked.
    asking: Option<Question>,
    /// The battery as given — a test's, a photograph's — or `None` to read
    /// the machine's when a run is asked for.
    charge: Option<module::Charge>,
    /// The person's binary cache's page, while the view is turned to it.
    cache: Option<CachePage>,
    /// The page of the distro's apps that follow their releases, while the
    /// view is turned to it — never two pages at once.
    follow: Option<FollowPage>,
    /// The parts' page, while the view is turned to it.
    parts: Option<PartsPage>,
    /// The door's wake, for the file picker the parts' page brings up.
    wake: Waking,
}

/// The parts' page as the view holds it: the page, what it read, where it
/// reads again (none for a test's or a photograph's page, which reads
/// nothing of the machine) and the zone its times are drawn in.
#[derive(Debug, Clone)]
pub struct PartsPage {
    /// The page's cursor and question.
    pub page: crate::parts::Page,
    /// What it draws.
    pub readings: crate::parts::Readings,
    /// Where it reads, and when each thing last moved.
    places: Option<(crate::parts::Places, Vec<Option<std::time::SystemTime>>)>,
    /// The zone its times are drawn in.
    zone: ronoctua_files::when::Zone,
    /// The file picker, while it is up over the page (`crate::file_picker`):
    /// the file handed in, or a stick's folder.
    picking: Option<crate::file_picker::Hosted<crate::parts::Picking>>,
    /// What the picker's rows take, as they were last drawn — the one answer
    /// the painting writes and the pointer reads (a row of the picker is
    /// `Target::Page` of its index).
    drawn: std::cell::RefCell<Drawn>,
}

/// The picker's rows as they were drawn: what each takes, and which one the
/// cursor is on.
#[derive(Debug, Clone, Default)]
pub struct Drawn {
    targets: Vec<crate::file_picker::Target>,
    focused: Option<usize>,
}

/// The door's wake, for the file picker's folder and devices — kept by the
/// view so every picker it opens is watched through it.
#[derive(Clone, Default)]
pub struct Waking(Option<crate::wake::Wake>);

impl std::fmt::Debug for Waking {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Waking(door)"
        } else {
            "Waking(none)"
        })
    }
}

/// The page of the apps that follow their releases as the view holds it:
/// the page, what it read, and where it reads again (none for a test's or a
/// photograph's page, which reads nothing of the machine).
#[derive(Debug, Clone)]
pub struct FollowPage {
    /// The page's cursor.
    pub page: crate::follow::Page,
    /// What it draws.
    pub readings: crate::follow::Readings,
    /// Where it reads, and when each file last moved.
    places: Option<(crate::follow::Places, Vec<Option<std::time::SystemTime>>)>,
}

/// The binary cache's page as the view holds it: the page, what it read,
/// and where it reads again (none for a test's or a photograph's page, which
/// reads nothing of the machine).
#[derive(Debug, Clone)]
pub struct CachePage {
    /// The page's cursor and questions.
    pub page: crate::binary_cache::Page,
    /// What it draws.
    pub readings: crate::binary_cache::Readings,
    /// Where it reads, and when each file last moved.
    places: Option<(
        crate::binary_cache::Places,
        Vec<Option<std::time::SystemTime>>,
    )>,
    /// The zone its dates are drawn in.
    zone: ronoctua_files::when::Zone,
}

impl View {
    /// The declared steps, and what the reports and the records on this
    /// machine say; its battery is read when a run is asked for.
    #[must_use]
    pub fn read() -> Self {
        let mut v = Self::of(module::steps(), Verdicts::read(), report_stamp()).with_records(
            module::Records::read_at(&record_places()),
            belongings_keys(),
        );
        v.charge = None;
        v
    }

    /// A view of `steps` with `verdicts` — what a test builds, and nothing of
    /// the machine is read: no records, and a machine on mains.
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
            records: module::Records::default(),
            belongings_keys: String::new(),
            asking: None,
            charge: Some(module::Charge {
                discharging: false,
                percent: None,
            }),
            cache: None,
            follow: None,
            parts: None,
            wake: Waking::default(),
        }
    }

    /// The same view over these records, the belongings taught by `keys`.
    #[must_use]
    pub fn with_records(mut self, records: module::Records, keys: String) -> Self {
        self.records = records;
        self.belongings_keys = keys;
        self
    }

    /// The same view on this battery — a test's, a photograph's.
    #[must_use]
    pub const fn on_charge(mut self, charge: module::Charge) -> Self {
        self.charge = Some(charge);
        self
    }

    /// What the machine's records say — what a test reads back.
    #[cfg(test)]
    #[must_use]
    pub const fn records(&self) -> &module::Records {
        &self.records
    }

    // -----------------------------------------------------------------------
    // THE BINARY CACHE'S PAGE (`crate::binary_cache`)
    // -----------------------------------------------------------------------

    /// Turns the view to the binary cache's page, reading what this person
    /// and this machine say — or, when it is open, back to the steps.
    pub fn toggle_cache(&mut self) -> Errand {
        if self.cache.take().is_none() {
            self.open_cache();
        }
        Errand::Moved
    }

    /// Opens the page over what this person and this machine say (kept open
    /// when it is).
    pub fn open_cache(&mut self) {
        if self.cache.is_some() {
            return;
        }
        self.follow = None;
        self.parts = None;
        let places = crate::binary_cache::Places::here();
        let readings = crate::binary_cache::Readings::read(&places);
        let stamp = places.stamp();
        self.cache = Some(CachePage {
            page: crate::binary_cache::Page::default(),
            readings,
            places: Some((places, stamp)),
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
        });
    }

    /// The same view on the page, over these readings — a test's, a
    /// photograph's: nothing of the machine is read, and its dates are UTC.
    #[must_use]
    pub fn with_cache(mut self, readings: crate::binary_cache::Readings) -> Self {
        self.follow = None;
        self.parts = None;
        self.cache = Some(CachePage {
            page: crate::binary_cache::Page::default(),
            readings,
            places: None,
            zone: ronoctua_files::when::Zone::utc(),
        });
        self
    }

    /// The page, while the view is turned to it.
    #[must_use]
    pub const fn cache_page(&self) -> Option<&CachePage> {
        self.cache.as_ref()
    }

    /// Whether a click takes row `n` of the binary cache's page — the page's
    /// one answer (`binary_cache::Page::takes`), which both doors light and
    /// click by: none while its grave question is asked.
    #[must_use]
    pub fn cache_takes(&self, n: usize) -> bool {
        self.cache
            .as_ref()
            .is_some_and(|c| c.page.takes(n, &c.readings))
    }

    /// The hint both doors draw: the file picker's own while it is up, or
    /// the keys this view teaches now.
    #[must_use]
    pub fn hint(&self, texts: &ronoctua_i18n::Texts) -> Vec<crate::chrome::Piece> {
        if let Some(h) = self.parts.as_ref().and_then(|p| p.picking.as_ref()) {
            return h.picker.hint(texts);
        }
        crate::chrome::hint_of(self.taught(), texts)
    }

    /// The keys this view teaches now: the steps', or the page's it is
    /// turned to.
    #[must_use]
    pub const fn taught(&self) -> &'static [(&'static str, &'static str)] {
        if self.cache.is_some() {
            OF_THE_PAGE
        } else if self.follow.is_some() {
            OF_THE_FOLLOW
        } else if self.parts.is_some() {
            OF_THE_PARTS
        } else {
            OF_THE_VIEW
        }
    }

    // -----------------------------------------------------------------------
    // THE PAGE OF THE APPS THAT FOLLOW THEIR RELEASES (`crate::follow`)
    // -----------------------------------------------------------------------

    /// Turns the view to the page of the apps that follow their releases,
    /// reading what this person and this machine say — or, when it is open,
    /// back to the steps.
    pub fn toggle_follow(&mut self) -> Errand {
        if self.follow.take().is_none() {
            self.open_follow();
        }
        Errand::Moved
    }

    /// Opens the page over what this person and this machine say (kept open
    /// when it is).
    pub fn open_follow(&mut self) {
        if self.follow.is_some() {
            return;
        }
        self.cache = None;
        self.parts = None;
        let places = crate::follow::Places::here();
        let readings = crate::follow::Readings::read(&places);
        let stamp = places.stamp();
        self.follow = Some(FollowPage {
            page: crate::follow::Page::default(),
            readings,
            places: Some((places, stamp)),
        });
    }

    /// The same view on the page, over these readings — a test's, a
    /// photograph's: nothing of the machine is read.
    #[must_use]
    pub fn with_follow(mut self, readings: crate::follow::Readings) -> Self {
        self.cache = None;
        self.parts = None;
        self.follow = Some(FollowPage {
            page: crate::follow::Page::default(),
            readings,
            places: None,
        });
        self
    }

    /// The page, while the view is turned to it.
    #[must_use]
    pub const fn follow_page(&self) -> Option<&FollowPage> {
        self.follow.as_ref()
    }

    /// Where the page reads — `None` for a test's or a photograph's.
    #[must_use]
    pub fn follow_places(&self) -> Option<&crate::follow::Places> {
        self.follow
            .as_ref()
            .and_then(|f| f.places.as_ref())
            .map(|(p, _)| p)
    }

    /// Reads the page again, now — after a change this door made.
    pub fn follow_reread(&mut self) {
        if let Some(f) = self.follow.as_mut()
            && let Some((places, stamp)) = f.places.as_mut()
        {
            f.readings = crate::follow::Readings::read(places);
            *stamp = places.stamp();
            f.page.settle(&f.readings);
        }
    }

    /// What the page asks of the door, as the view's errand.
    fn follow_errand(&mut self, e: crate::follow::Errand) -> Errand {
        use crate::follow::Errand as Page;
        match e {
            Page::Nothing => Errand::Nothing,
            Page::Moved => Errand::Moved,
            Page::Close => {
                self.follow = None;
                Errand::Moved
            }
            other => Errand::Follow(other),
        }
    }

    // -----------------------------------------------------------------------
    // THE PARTS' PAGE (`crate::parts`)
    // -----------------------------------------------------------------------

    /// Turns the view to the parts' page, reading what this machine says —
    /// or, when it is open, back to the steps.
    pub fn toggle_parts(&mut self) -> Errand {
        if self.parts.take().is_none() {
            self.open_parts();
        }
        Errand::Moved
    }

    /// Opens the page over what this machine says (kept open when it is).
    pub fn open_parts(&mut self) {
        if self.parts.is_some() {
            return;
        }
        self.cache = None;
        self.follow = None;
        let places = crate::parts::Places::here();
        let readings = crate::parts::Readings::read(&places);
        let stamp = places.stamp();
        self.parts = Some(PartsPage {
            page: crate::parts::Page::default(),
            readings,
            places: Some((places, stamp)),
            zone: ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime")),
            picking: None,
            drawn: std::cell::RefCell::default(),
        });
    }

    /// The same view on the page, over these readings — a test's, a
    /// photograph's: nothing of the machine is read, and its times are UTC.
    #[must_use]
    pub fn with_parts(mut self, readings: crate::parts::Readings) -> Self {
        self.cache = None;
        self.follow = None;
        self.parts = Some(PartsPage {
            page: crate::parts::Page::default(),
            readings,
            places: None,
            zone: ronoctua_files::when::Zone::utc(),
            picking: None,
            drawn: std::cell::RefCell::default(),
        });
        self
    }

    /// The door's wake: every file picker the parts' page brings up after
    /// this is watched through it — its folder, and the devices that are
    /// its places (`file_picker::FilePicker::watch_with`).
    pub fn watch_with(&mut self, wake: crate::wake::Wake) {
        self.wake = Waking(Some(wake));
    }

    /// What the wake brought the file picker, taken — `true` when its rows
    /// changed and the door draws again.
    pub fn look(&mut self) -> bool {
        self.parts
            .as_mut()
            .and_then(|p| p.picking.as_mut())
            .is_some_and(crate::file_picker::Hosted::look)
    }

    /// Whether the file picker is up and takes `key` — the door hands it to
    /// [`Self::pick_key`] before anything else reads it.
    #[must_use]
    pub fn picker_takes(&self, key: ronoctua_keymap::Key) -> bool {
        self.parts
            .as_ref()
            .and_then(|p| p.picking.as_ref())
            .is_some_and(|h| h.picker.claims(key))
    }

    /// Whether the file picker is up.
    #[must_use]
    pub fn picking(&self) -> bool {
        self.parts.as_ref().is_some_and(|p| p.picking.is_some())
    }

    /// What was copied, for the file picker.
    pub fn picker_pasted(&mut self, text: &str) {
        if let Some(h) = self.parts.as_mut().and_then(|p| p.picking.as_mut()) {
            let _ = h.pasted(text);
        }
    }

    /// A key for the file picker: its answer goes to the page
    /// (`crate::parts::picked`), and going back brings the page's rows back.
    pub fn pick_key(&mut self, key: ronoctua_keymap::Key) -> Errand {
        use crate::file_picker::{Hosting, Picked};
        let Some(h) = self.parts.as_mut().and_then(|p| p.picking.as_mut()) else {
            return Errand::Nothing;
        };
        let e = match h.key(key) {
            Hosting::Nothing => crate::parts::Errand::Moved,
            Hosting::Back => {
                self.close_the_picker();
                crate::parts::Errand::Moved
            }
            Hosting::Say(e) => crate::parts::Errand::Explain(e),
            Hosting::Paste => crate::parts::Errand::Paste,
            Hosting::Picked(asked, Picked::Paths { paths, .. }) => {
                self.close_the_picker();
                crate::parts::picked(asked, &paths)
            }
            Hosting::Picked(..) => crate::parts::Errand::Nothing,
        };
        self.parts_errand(e)
    }

    /// The picker put down: the page's own rows come back.
    fn close_the_picker(&mut self) {
        if let Some(p) = self.parts.as_mut() {
            p.picking = None;
            *p.drawn.borrow_mut() = Drawn::default();
        }
    }

    /// The page, while the view is turned to it.
    #[must_use]
    pub const fn parts_page(&self) -> Option<&PartsPage> {
        self.parts.as_ref()
    }

    /// Where the page reads — `None` for a test's or a photograph's.
    #[must_use]
    pub fn parts_places(&self) -> Option<&crate::parts::Places> {
        self.parts
            .as_ref()
            .and_then(|p| p.places.as_ref())
            .map(|(places, _)| places)
    }

    /// Reads the page again, now — after a gesture this door carried out.
    pub fn parts_reread(&mut self) {
        if let Some(p) = self.parts.as_mut()
            && let Some((places, stamp)) = p.places.as_mut()
        {
            p.readings = crate::parts::Readings::read(places);
            *stamp = places.stamp();
            p.page.settle(&p.readings);
        }
    }

    /// What the page asks of the door, as the view's errand — the file
    /// picker brought up over the page here, watched through the door's wake.
    fn parts_errand(&mut self, e: crate::parts::Errand) -> Errand {
        use crate::parts::Errand as Page;
        match e {
            Page::Nothing => Errand::Nothing,
            Page::Moved => Errand::Moved,
            Page::Close => {
                self.parts = None;
                Errand::Moved
            }
            Page::Pick(asked) => {
                let wake = self.wake.0.clone();
                let Some(p) = self.parts.as_mut() else {
                    return Errand::Nothing;
                };
                let media = p
                    .places
                    .as_ref()
                    .and_then(|(places, _)| places.media.clone());
                p.picking = Some(crate::file_picker::Hosted::open(
                    crate::parts::request_of(&asked, media.as_deref()),
                    asked.clone(),
                    crate::parts::title_of(&asked),
                    wake.as_ref(),
                ));
                Errand::Moved
            }
            other => Errand::Parts(other),
        }
    }

    /// The question `id` of the parts' page, asked as its key or row would —
    /// a command run by its name with nothing typed.
    pub fn parts_ask_for(&mut self, id: &str) -> Errand {
        self.open_parts();
        let Some(p) = self.parts.as_mut() else {
            return Errand::Nothing;
        };
        let e = p.page.ask_for(id, &p.readings);
        self.parts_errand(e)
    }

    /// What was typed on a line, answering the open page's question `id` —
    /// the parts' mirrors, or the binary cache's.
    pub fn page_answered(&mut self, id: &str, text: &str) -> Errand {
        if id == crate::parts::MIRRORS {
            let Some(p) = self.parts.as_mut() else {
                return Errand::Nothing;
            };
            let e = p.page.answered(id, text);
            return self.parts_errand(e);
        }
        self.cache_answered(id, text)
    }

    /// A path typed after one of the parts' picker commands, run by its name
    /// — the picker's answer without the picker, which goes down.
    pub fn parts_typed(&mut self, id: &str, text: &str) -> Errand {
        self.open_parts();
        self.close_the_picker();
        let Some(p) = self.parts.as_mut() else {
            return Errand::Nothing;
        };
        let e = p.page.typed(id, text, &p.readings);
        self.parts_errand(e)
    }

    /// The line was let go: the open page's question goes with it.
    pub fn page_unanswered(&mut self) {
        self.cache_unanswered();
        if let Some(p) = self.parts.as_mut() {
            p.page.unanswered();
        }
    }

    // -----------------------------------------------------------------------
    // EITHER PAGE — what both doors draw and click by
    // -----------------------------------------------------------------------

    /// Whether the view is turned to a page.
    #[must_use]
    pub const fn page_open(&self) -> bool {
        self.cache.is_some() || self.follow.is_some() || self.parts.is_some()
    }

    /// The row under the open page's cursor.
    #[must_use]
    pub fn page_cursor(&self) -> Option<usize> {
        // THE PICKER'S CURSOR, as its rows were last drawn — and its first
        // row before they ever were: a door paints a page only while it has
        // a cursor, and the rows are written down by that painting.
        if let Some(p) = self.parts.as_ref().filter(|p| p.picking.is_some()) {
            return Some(p.drawn.borrow().focused.unwrap_or(0));
        }
        self.cache
            .as_ref()
            .map(|c| c.page.cursor())
            .or_else(|| self.follow.as_ref().map(|f| f.page.cursor()))
            .or_else(|| self.parts.as_ref().map(|p| p.page.cursor()))
    }

    /// Whether a click takes row `n` of the open page — the page's one
    /// answer, which both doors light and click by.
    #[must_use]
    pub fn page_takes(&self, n: usize) -> bool {
        // THE PICKER'S ROWS take what they were drawn taking: a title and a
        // sentence take nothing.
        if let Some(p) = self.parts.as_ref().filter(|p| p.picking.is_some()) {
            return p
                .drawn
                .borrow()
                .targets
                .get(n)
                .is_some_and(|t| *t != crate::file_picker::Target::Nothing);
        }
        self.cache_takes(n)
            || self
                .follow
                .as_ref()
                .is_some_and(|f| f.page.takes(n, &f.readings))
            || self
                .parts
                .as_ref()
                .is_some_and(|p| p.page.takes(n, &p.readings))
    }

    /// The open page's lines under the head, placed from `left` in `room`.
    #[must_use]
    pub fn page_lines(
        &self,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::binary_cache::PageLine> {
        if self.cache.is_some() {
            return self.cache_lines(left, room, texts);
        }
        if let Some(p) = &self.parts {
            // THE PICKER'S ROWS, while it is up, as the page's own — each
            // `Target::Page` of its index, and what it takes written down for
            // the pointer.
            if let Some(h) = &p.picking {
                let cols =
                    left + room + crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT;
                let rows = h.rows(cols, texts, |r| r);
                let mut drawn = Drawn::default();
                let lines = rows
                    .into_iter()
                    .enumerate()
                    .map(|(n, r)| {
                        if r.focused {
                            drawn.focused = Some(n);
                        }
                        let takes = !r.heading && r.target != crate::file_picker::Target::Nothing;
                        drawn.targets.push(if takes {
                            r.target
                        } else {
                            crate::file_picker::Target::Nothing
                        });
                        crate::binary_cache::PageLine {
                            fields: r.fields,
                            row: takes.then_some(n),
                        }
                    })
                    .collect();
                *p.drawn.borrow_mut() = drawn;
                return lines;
            }
            return p.page.lines(&p.readings, left, room, &p.zone, texts);
        }
        self.follow
            .as_ref()
            .map_or_else(Vec::new, |f| f.page.lines(&f.readings, left, room, texts))
    }

    /// Where the page reads — `None` for a test's or a photograph's.
    #[must_use]
    pub fn cache_places(&self) -> Option<&crate::binary_cache::Places> {
        self.cache
            .as_ref()
            .and_then(|c| c.places.as_ref())
            .map(|(p, _)| p)
    }

    /// Reads the page again, now — after a change this door made.
    pub fn cache_reread(&mut self) {
        if let Some(c) = self.cache.as_mut()
            && let Some((places, stamp)) = c.places.as_mut()
        {
            c.readings = crate::binary_cache::Readings::read(places);
            *stamp = places.stamp();
            c.page.settle(&c.readings);
        }
    }

    /// What the page asks of the door, as the view's errand.
    fn page_errand(&mut self, e: crate::binary_cache::Errand) -> Errand {
        use crate::binary_cache::Errand as Page;
        match e {
            Page::Nothing => Errand::Nothing,
            Page::Moved => Errand::Moved,
            Page::Close => {
                self.cache = None;
                Errand::Moved
            }
            other => Errand::Cache(other),
        }
    }

    /// What was typed on a line, answering the page's question `id`.
    pub fn cache_answered(&mut self, id: &str, text: &str) -> Errand {
        let Some(c) = self.cache.as_mut() else {
            return Errand::Nothing;
        };
        let e = c.page.answered(id, text, &c.readings);
        self.page_errand(e)
    }

    /// The question `id`, asked as `RET` on its row would — a command run by
    /// its name with nothing typed.
    pub fn cache_ask_for(&mut self, id: &str) -> Errand {
        self.open_cache();
        let Some(c) = self.cache.as_mut() else {
            return Errand::Nothing;
        };
        let e = c.page.ask_for(id, &c.readings);
        self.page_errand(e)
    }

    /// The line was let go.
    pub fn cache_unanswered(&mut self) {
        if let Some(c) = self.cache.as_mut() {
            c.page.unanswered();
        }
    }

    /// The page's lines under the head, placed from `left` in `room`.
    #[must_use]
    pub fn cache_lines(
        &self,
        left: u16,
        room: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::binary_cache::PageLine> {
        self.cache.as_ref().map_or_else(Vec::new, |c| {
            c.page.lines(&c.readings, left, room, &c.zone, texts)
        })
    }

    /// The question, while it is asked.
    #[must_use]
    pub const fn asking(&self) -> Option<Question> {
        self.asking
    }

    /// The battery, as given or as the machine has it now.
    fn charge(&self) -> module::Charge {
        self.charge.unwrap_or_else(module::Charge::read)
    }

    /// Asks the question on battery — what `LEADER s u n` does in the
    /// footer below the floor, before anything starts — when the battery is
    /// below the floor; `false` when it is not and nothing was asked.
    pub fn ask_on_battery(&mut self) -> bool {
        let charge = self.charge();
        if !charge.below_the_floor() {
            return false;
        }
        self.asking = Some(Question {
            cost: Cost::Battery(charge.percent),
            queued: false,
            yes: false,
        });
        true
    }

    /// Asks whether to spend a metered way out on the run the brain holds
    /// for it — what a door does when a run it asked for by hand is held
    /// that way ([`Followed`]).
    pub const fn ask_metered(&mut self) {
        self.asking = Some(Question {
            cost: Cost::Metered,
            queued: true,
            yes: false,
        });
    }

    /// The question answered: it closes, and the door starts the run here,
    /// or says it did not start — or that it stays queued, when the brain
    /// holds it. A yes to the network on a battery below the floor asks
    /// about the battery next: one yes is not the other.
    fn answer(&mut self, yes: bool) -> Errand {
        let Some(q) = self.asking.take() else {
            return Errand::Nothing;
        };
        match (q.cost, yes) {
            (Cost::Metered, true) => {
                let charge = self.charge();
                if charge.below_the_floor() {
                    self.asking = Some(Question {
                        cost: Cost::Battery(charge.percent),
                        queued: true,
                        yes: false,
                    });
                    Errand::Moved
                } else {
                    Errand::StartHere { on_battery: false }
                }
            }
            (Cost::Battery(_), true) => Errand::StartHere { on_battery: true },
            (_, false) if q.queued => Errand::KeptWaiting,
            (_, false) => Errand::NotStarted,
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

    /// Re-reads the reports and the records when a file moved, and only
    /// then; `true` when something did.
    pub fn reread_if_moved(&mut self) -> bool {
        // THE BINARY CACHE'S PAGE reads its own files, the same way: only
        // when one moved — and so does the page of the apps that follow
        // their releases.
        let mut moved = false;
        if let Some(p) = self.parts.as_mut()
            && let Some((places, stamp)) = p.places.as_mut()
        {
            let now = places.stamp();
            if now != *stamp {
                *stamp = now;
                p.readings = crate::parts::Readings::read(places);
                p.page.settle(&p.readings);
                moved = true;
            }
        }
        if let Some(f) = self.follow.as_mut()
            && let Some((places, stamp)) = f.places.as_mut()
        {
            let now = places.stamp();
            if now != *stamp {
                *stamp = now;
                f.readings = crate::follow::Readings::read(places);
                f.page.settle(&f.readings);
                moved = true;
            }
        }
        if let Some(c) = self.cache.as_mut()
            && let Some((places, stamp)) = c.places.as_mut()
        {
            let now = places.stamp();
            if now != *stamp {
                *stamp = now;
                c.readings = crate::binary_cache::Readings::read(places);
                c.page.settle(&c.readings);
                moved = true;
            }
        }
        let now = report_stamp();
        if now == self.stamp {
            return moved;
        }
        self.stamp = now;
        self.verdicts = Verdicts::read();
        self.records = module::Records::read_at(&record_places());
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
        // THE BINARY CACHE'S PAGE takes the arrows, `RET` and `Esc` while the
        // view is turned to it, and `c` turns it back; the other keys stay
        // the update's.
        if press == Press::Cache {
            self.follow = None;
            self.parts = None;
            return self.toggle_cache();
        }
        if press == Press::Follow {
            self.parts = None;
            return self.toggle_follow();
        }
        // THE FILE PICKER takes the view's keys that walk, answer and go
        // back while it is up — a second click's `RET`, the pointer's way
        // back —, and `d` leaves it with the page.
        if self.picking() {
            use ronoctua_keymap::{Key, Sym};
            let key = match press {
                Press::Up => Key::simple(Sym::Up),
                Press::Down => Key::simple(Sym::Down),
                Press::Enter => Key::simple(Sym::Enter),
                Press::Escape => Key::simple(Sym::Esc),
                _ => return Errand::Nothing,
            };
            return self.pick_key(key);
        }
        if press == Press::Parts {
            return self.toggle_parts();
        }
        // THE PARTS' PAGE takes the arrows, `RET`, `h` and `Esc` while the
        // view is turned to it, and `d` turns it back.
        if let Some(p) = self.parts.as_mut() {
            use crate::parts::Press as Page;
            let page = match press {
                Press::Up => Some(Page::Up),
                Press::Down => Some(Page::Down),
                Press::Enter => Some(Page::Enter),
                Press::HandIn => Some(Page::HandIn),
                Press::Escape => Some(Page::Escape),
                _ => None,
            };
            if let Some(k) = page {
                let e = p.page.press(k, &p.readings);
                return self.parts_errand(e);
            }
        }
        // THE PAGE OF THE APPS THAT FOLLOW THEIR RELEASES takes the arrows,
        // `RET` and `Esc` while the view is turned to it, and `f` turns it
        // back.
        if let Some(f) = self.follow.as_mut() {
            use crate::follow::Press as Page;
            let page = match press {
                Press::Up => Some(Page::Up),
                Press::Down => Some(Page::Down),
                Press::Enter => Some(Page::Enter),
                Press::Escape => Some(Page::Escape),
                _ => None,
            };
            if let Some(p) = page {
                let e = f.page.press(p, &f.readings);
                return self.follow_errand(e);
            }
        }
        if let Some(c) = self.cache.as_mut() {
            use crate::binary_cache::Press as Page;
            let page = match press {
                Press::Up => Some(Page::Up),
                Press::Down => Some(Page::Down),
                Press::Enter => Some(Page::Enter),
                Press::Escape => Some(Page::Escape),
                _ => None,
            };
            if let Some(p) = page {
                let e = c.page.press(p, &c.readings);
                return self.page_errand(e);
            }
        }
        // THE QUESTION TAKES THE ARROWS, `RET` AND `Esc` while it is asked:
        // two answers, "no" above and "yes" below, and `Esc` is the "no".
        if let Some(q) = self.asking.as_mut() {
            match press {
                Press::Up | Press::Down => {
                    q.yes = press == Press::Down;
                    return Errand::Moved;
                }
                Press::Enter => {
                    let yes = q.yes;
                    return self.answer(yes);
                }
                Press::Escape => return self.answer(false),
                _ => {}
            }
        }
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
            // BELOW THE BATTERY FLOOR, A RUN BY HAND ASKS FIRST — the no
            // first — where a scheduled one would wait for the charger.
            Press::Start => {
                if self.ask_on_battery() {
                    Errand::Moved
                } else {
                    Errand::Start
                }
            }
            Press::Abort => Errand::Answer(false),
            Press::Proceed => Errand::Answer(true),
            Press::Enter => Errand::Nothing,
            Press::Escape => Errand::Close,
            Press::Cache => self.toggle_cache(),
            Press::Follow => self.toggle_follow(),
            Press::Parts => self.toggle_parts(),
            // A FILE IS HANDED IN only on the parts' page.
            Press::HandIn => Errand::Nothing,
        }
    }

    /// The head of the view, wrapped to `room`: the question while it is
    /// asked — its sentence, "no", "yes" — or else the second register; then
    /// what the machine's records say, each line only when it says
    /// something. Notes are WRAPPED, never cut.
    #[must_use]
    pub fn top_lines(&self, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<TopLine> {
        let line = |text: String, color, target| TopLine {
            text,
            color,
            target,
        };
        // THE BINARY CACHE'S PAGE: its grave question, or what the row under
        // its cursor is.
        if let Some(c) = &self.cache {
            return c
                .page
                .top_lines(&c.readings, room, texts)
                .into_iter()
                .map(|h| line(h.text, h.color, h.answer.map(Target::PageAnswer)))
                .collect();
        }
        // THE PARTS' PAGE: what the row under its cursor is, why it failed,
        // what it is missing.
        if let Some(p) = &self.parts {
            if let Some(h) = &p.picking {
                return crate::parts::picking_lines(&h.asked, room, texts)
                    .into_iter()
                    .map(|h| line(h.text, h.color, None))
                    .collect();
            }
            return p
                .page
                .top_lines(&p.readings, room, &p.zone, texts)
                .into_iter()
                .map(|h| line(h.text, h.color, None))
                .collect();
        }
        // THE PAGE OF THE APPS THAT FOLLOW THEIR RELEASES: what the row
        // under its cursor is.
        if let Some(f) = &self.follow {
            return f
                .page
                .top_lines(&f.readings, room, texts)
                .into_iter()
                .map(|h| line(h.text, h.color, None))
                .collect();
        }
        let mut out = Vec::new();
        if let Some(q) = self.asking {
            let asked = match q.cost {
                Cost::Battery(Some(percent)) => texts.t_args(
                    module::key::SYSTEM_UPDATE_ASK_ON_BATTERY,
                    &[
                        ("percent", ronoctua_i18n::Arg::Number(i64::from(percent))),
                        (
                            "floor",
                            ronoctua_i18n::Arg::Number(i64::from(
                                module::battery_floor().unwrap_or_default(),
                            )),
                        ),
                    ],
                ),
                Cost::Battery(None) => texts.t(module::key::SYSTEM_UPDATE_ASK_ON_BATTERY_UNKNOWN),
                Cost::Metered => texts.t(module::key::SYSTEM_UPDATE_ASK_METERED),
            };
            for said in crate::chrome::note_lines(&asked, room) {
                out.push(line(said, tokens::color::TEXT_PRIMARY, None));
            }
            // "NO" FIRST, and the cursor is born on it. "Yes" is in the
            // colour of what it costs: a battery spent on a build, a
            // connection paid by the byte.
            let (no, yes_key) = match q.cost {
                Cost::Battery(_) => (
                    module::key::SYSTEM_UPDATE_ANSWER_NO,
                    module::key::SYSTEM_UPDATE_ANSWER_YES,
                ),
                Cost::Metered => (
                    module::key::SYSTEM_UPDATE_ANSWER_NO_METERED,
                    module::key::SYSTEM_UPDATE_ANSWER_YES_METERED,
                ),
            };
            for (yes, answer) in [(false, no), (true, yes_key)] {
                let color = if q.yes == yes {
                    tokens::color::TEXT_HERE_ACTS
                } else if yes {
                    tokens::color::STATE_WARNING
                } else {
                    crate::chrome::Kind::Setting.tones(false).0
                };
                out.push(line(
                    layout::elide_end(&texts.t(answer), room),
                    color,
                    Some(Target::Answer(yes)),
                ));
            }
        } else {
            for said in self.register_lines(room, texts) {
                out.push(line(said, tokens::color::TEXT_SECONDARY, None));
            }
        }
        // A LINE OF AIR between what the cursor's step is and what the
        // machine's records say: two subjects, and hierarchy is colour and
        // air, never size. Only when there is a record to say.
        let records = record_sentences(&self.records, &self.belongings_keys, texts);
        if !records.is_empty() && !out.is_empty() {
            out.push(line(String::new(), tokens::color::TEXT_SECONDARY, None));
        }
        for (said, color, target) in records {
            for wrapped in crate::chrome::note_lines(&said, room) {
                out.push(line(wrapped, color, target));
            }
        }
        out
    }

    /// The second register's lines in the footer's region: WRAPPED, never
    /// cut — a note is read whole.
    fn register_lines(&self, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<String> {
        crate::chrome::note_lines(&self.explains(texts).unwrap_or_default(), room)
    }

    /// How many rows the footer's region is: the head (the question or the
    /// second register, and the records), the steps, a line of air and the
    /// output — counted without building the output, for the footer asks
    /// this on every pointer motion.
    #[must_use]
    pub fn region_len(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let (left, room) = crate::chrome::extent(columns);
        if self.page_open() {
            return self.top_lines(room, texts).len() + self.page_lines(left, room, texts).len();
        }
        self.top_lines(room, texts).len() + self.steps.len() + 1 + self.scrollback.amount().max(1)
    }

    /// The row of the region the cursor is on — the answer under it while
    /// the question is asked, else the cursor's step: what the footer's page
    /// follows after a key.
    #[must_use]
    pub fn region_cursor(&self, columns: u16, texts: &ronoctua_i18n::Texts) -> usize {
        let (_, room) = crate::chrome::extent(columns);
        let head = self.top_lines(room, texts);
        if let Some(cursor) = self.page_cursor() {
            // THE PAGE'S CURSOR, under its heading — or the answer of the
            // binary cache's grave question the cursor is on.
            if let Some(q) = self.cache.as_ref().and_then(|c| c.page.confirm())
                && let Some(n) = head
                    .iter()
                    .position(|l| l.target == Some(Target::PageAnswer(q.yes)))
            {
                return n;
            }
            let (left, room) = crate::chrome::extent(columns);
            let below = self
                .page_lines(left, room, texts)
                .iter()
                .position(|l| l.row == Some(cursor))
                .unwrap_or(0);
            return head.len() + below;
        }
        if let Some(q) = self.asking
            && let Some(n) = head
                .iter()
                .position(|l| l.target == Some(Target::Answer(q.yes)))
        {
            return n;
        }
        head.len() + self.cursor
    }

    /// The footer's region, rows `skip..skip + take` of it: ONE list — the
    /// head on top (the question or the second register, then the records),
    /// the steps with their verdicts under it (the cursor's name in the
    /// accent: the cursor is a colour), a line of air, and the output of both
    /// journals — which the region's own scroll, bar and wheel read, as they
    /// read every list of the footer. Only the rows asked for are built: the
    /// output keeps up to [`KEPT`] lines.
    #[must_use]
    pub fn region_rows(
        &self,
        skip: usize,
        take: usize,
        columns: u16,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<RegionRow> {
        let (left, room) = crate::chrome::extent(columns);
        let head = self.top_lines(room, texts);
        // THE PAGE THE VIEW IS TURNED TO: its head, then its heading and its
        // rows.
        if self.page_open() {
            let page = self.page_lines(left, room, texts);
            let rows = head
                .iter()
                .map(|said| RegionRow {
                    fields: vec![crate::fields::Field {
                        at: left,
                        text: said.text.clone(),
                        color: said.color,
                        hit: crate::fields::Hit::Nothing,
                        explain: None,
                        ink: crate::fields::Ink::Grid,
                    }],
                    target: said.target,
                })
                .chain(page.into_iter().map(|l| RegionRow {
                    fields: l.fields,
                    // A ROW TAKES THE CLICK ONLY WHERE THE PAGE SAYS SO —
                    // never under the grave question (`View::page_takes`).
                    target: l.row.filter(|n| self.page_takes(*n)).map(Target::Page),
                }));
            return rows.skip(skip).take(take).collect();
        }
        let steps_at = head.len();
        let air_at = steps_at + self.steps.len();
        let output_at = air_at + 1;
        let total = air_at + 1 + self.scrollback.amount().max(1);
        let field = |at: u16, text: String, color: tokens::Color| crate::fields::Field {
            at: left + at,
            text,
            color,
            hit: crate::fields::Hit::Nothing,
            explain: None,
            ink: crate::fields::Ink::Grid,
        };
        let mut out = Vec::with_capacity(take.min(total));
        for n in skip..total.min(skip.saturating_add(take)) {
            let row = if let Some(said) = head.get(n) {
                RegionRow {
                    fields: vec![field(0, said.text.clone(), said.color)],
                    target: said.target,
                }
            } else if n < air_at {
                let at = n - steps_at;
                let step = &self.steps[at];
                let (verdict, seconds) = self.verdicts.of_step(step.id);
                let mut pieces = step_row(step, &verdict, seconds, room, texts);
                // THE NAME IS THE LAST PIECE, and the cursor is its colour —
                // while no question holds the cursor.
                if at == self.cursor
                    && self.asking.is_none()
                    && let Some(name) = pieces.last_mut()
                {
                    name.color = crate::chrome::Kind::ListedFact.tones(true).0;
                }
                RegionRow {
                    fields: pieces
                        .into_iter()
                        .map(|p| field(p.at, p.text, p.color))
                        .collect(),
                    target: Some(Target::Step(at)),
                }
            } else if n < output_at {
                RegionRow {
                    fields: crate::chrome::AIR,
                    target: None,
                }
            } else if self.scrollback.is_empty() {
                RegionRow {
                    fields: vec![field(
                        GUTTER,
                        layout::elide_end(&texts.t(module::key::SYSTEM_UPDATE_NO_OUTPUT), room),
                        tokens::color::TEXT_DIM,
                    )],
                    target: None,
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
                    target: None,
                }
            };
            out.push(row);
        }
        out
    }

    /// A click on what a line takes, in either door.
    ///
    /// A STEP: the cursor goes there, and what the step is is said. There is
    /// nothing else to do to a step here — switching one on or off belongs
    /// to the brain's screen, where the choice is written down — so a second
    /// click, like `RET`, does nothing more (invariant 19).
    ///
    /// AN ANSWER: the first click puts the cursor on it, the second answers,
    /// as `RET` does. THE BELONGINGS' LINE: one click opens them — it is the
    /// key the line teaches.
    pub fn clicked(&mut self, target: Target) -> Errand {
        match target {
            Target::Step(n) => {
                if n >= self.steps.len() || n == self.cursor {
                    return Errand::Nothing;
                }
                self.cursor = n;
                Errand::Moved
            }
            Target::Answer(yes) => match self.asking.as_mut() {
                Some(q) if q.yes == yes => self.answer(yes),
                Some(q) => {
                    q.yes = yes;
                    Errand::Moved
                }
                None => Errand::Nothing,
            },
            Target::Belongings => Errand::OpenBelongings,
            Target::Page(n) => match (self.cache.as_mut(), self.follow.as_mut()) {
                (Some(c), _) => {
                    let e = c.page.clicked(n, &c.readings);
                    self.page_errand(e)
                }
                (None, Some(f)) => {
                    let e = f.page.clicked(n, &f.readings);
                    self.follow_errand(e)
                }
                (None, None) => match self.parts.as_mut() {
                    // A ROW OF THE PICKER is chosen there: its cursor, and
                    // the second click its `RET`.
                    Some(p) if p.picking.is_some() => {
                        let target = p.drawn.borrow().targets.get(n).cloned();
                        let again = match (p.picking.as_mut(), target) {
                            (Some(h), Some(t)) if t != crate::file_picker::Target::Nothing => {
                                h.clicked(t)
                            }
                            _ => return Errand::Nothing,
                        };
                        if again {
                            self.pick_key(ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Enter))
                        } else {
                            Errand::Moved
                        }
                    }
                    Some(p) => {
                        let e = p.page.clicked(n, &p.readings);
                        self.parts_errand(e)
                    }
                    None => Errand::Nothing,
                },
            },
            Target::PageAnswer(yes) => match self.cache.as_mut() {
                Some(c) => {
                    let e = c.page.answer_clicked(yes);
                    self.page_errand(e)
                }
                None => Errand::Nothing,
            },
        }
    }
}

/// What the machine's records say, as sentences: the belongings' conflicts
/// (a click opens them), each declared program the verification held back,
/// the boot trial or why there is none, and a hold — each only when it is
/// there, in the colour of what it costs.
fn record_sentences(
    r: &module::Records,
    belongings_keys: &str,
    texts: &ronoctua_i18n::Texts,
) -> Vec<(String, tokens::Color, Option<Target>)> {
    let mut out = Vec::new();
    if r.conflicts > 0 {
        out.push((
            texts.t_args(
                module::key::SYSTEM_UPDATE_RECORDS_CONFLICTS,
                &[
                    (
                        "count",
                        ronoctua_i18n::Arg::Number(i64::try_from(r.conflicts).unwrap_or(i64::MAX)),
                    ),
                    ("keys", ronoctua_i18n::Arg::Text(belongings_keys)),
                ],
            ),
            tokens::color::STATE_WARNING,
            Some(Target::Belongings),
        ));
    }
    for held in &r.held {
        out.push((
            texts.t_with(
                module::key::SYSTEM_UPDATE_RECORDS_HELD,
                &[("id", &held.id), ("why", &texts.t(held_key(held.why)))],
            ),
            tokens::color::STATE_WARNING,
            None,
        ));
    }
    if let Some(outcome) = r.trial {
        let color = match outcome {
            module::TrialOutcome::Failed => tokens::color::STATE_ERROR,
            _ => tokens::color::TEXT_SECONDARY,
        };
        out.push((texts.t(trial_key(outcome)), color, None));
    }
    if let Some(why) = r.no_trial {
        out.push((
            texts.t(no_trial_key(why)),
            tokens::color::TEXT_SECONDARY,
            None,
        ));
    }
    if let Some(hold) = &r.hold {
        // THE REVISION AS GIT ABBREVIATES IT, and the day it was held: the
        // record's whole stamp is a machine's way of writing a date.
        let rev: String = hold.rev.chars().take(REV_SHOWN).collect();
        let since = hold.since.split('T').next().unwrap_or_default();
        out.push((
            texts.t_with(
                module::key::SYSTEM_UPDATE_RECORDS_HOLD,
                &[("rev", &rev), ("since", since)],
            ),
            tokens::color::TEXT_SECONDARY,
            None,
        ));
    }
    out
}

/// How many hex digits of a held revision are drawn — git's own short form.
const REV_SHOWN: usize = 7;

/// The machine's own sentence for a refusal of its program
/// (`ronoctua-machine`'s `refused <word>`), or `None` for a word that is not
/// one of the machine's: a silent fallback would say a reason the program
/// never gave.
#[must_use]
pub fn refusal_key(word: &str) -> Option<&'static str> {
    use module::MachineRefusal as Why;
    Some(match Why::of_id(word)? {
        Why::NotRoot => key::MACHINE_REFUSED_NOT_ROOT,
        Why::Busy => key::MACHINE_REFUSED_BUSY,
        Why::NothingToMigrate => key::MACHINE_REFUSED_NOTHING_TO_MIGRATE,
        Why::BuildFailed => key::MACHINE_REFUSED_BUILD_FAILED,
        Why::SwitchFailed => key::MACHINE_REFUSED_SWITCH_FAILED,
        Why::DidNotBoot => key::MACHINE_REFUSED_DID_NOT_BOOT,
        Why::LastGoodWouldFallOut => key::MACHINE_REFUSED_LAST_GOOD_WOULD_FALL_OUT,
        Why::Held => key::MACHINE_REFUSED_HELD,
        Why::Unreadable => key::MACHINE_REFUSED_UNREADABLE,
        Why::Failed => key::MACHINE_REFUSED_FAILED,
    })
}

/// The sentence of a reason the verification held a program back.
const fn held_key(why: module::HeldWhy) -> &'static str {
    match why {
        module::HeldWhy::EvalFailed => module::key::SYSTEM_UPDATE_HELD_EVAL_FAILED,
        module::HeldWhy::FetchFailed => module::key::SYSTEM_UPDATE_HELD_FETCH_FAILED,
        module::HeldWhy::UpstreamChanged => module::key::SYSTEM_UPDATE_HELD_UPSTREAM_CHANGED,
        module::HeldWhy::BuildFailed => module::key::SYSTEM_UPDATE_HELD_BUILD_FAILED,
    }
}

/// The machine's own sentence of a boot trial's outcome.
const fn trial_key(outcome: module::TrialOutcome) -> &'static str {
    match outcome {
        module::TrialOutcome::Pending => key::MACHINE_TRIAL_PENDING,
        module::TrialOutcome::Good => key::MACHINE_TRIAL_GOOD,
        module::TrialOutcome::Failed => key::MACHINE_TRIAL_FAILED,
        module::TrialOutcome::BootIdentical => key::MACHINE_TRIAL_BOOT_IDENTICAL,
    }
}

/// The machine's own sentence of why an apply armed no trial.
const fn no_trial_key(why: module::NoTrial) -> &'static str {
    match why {
        module::NoTrial::NoEfiVariables => key::MACHINE_NO_TRIAL_NO_EFI_VARIABLES,
        module::NoTrial::NotInMenu => key::MACHINE_NO_TRIAL_NOT_IN_MENU,
        module::NoTrial::NoLastGood => key::MACHINE_NO_TRIAL_NO_LAST_GOOD,
    }
}

/// The keys that open the belongings, as this leader's trees teach them —
/// taught by the line that counts their conflicts, never written in a text.
fn belongings_keys() -> String {
    crate::bindings::taught(crate::belongings::OPEN_COMMAND, None).unwrap_or_default()
}

/// One row of the footer's region.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionRow {
    /// Placed and cut.
    pub fields: Vec<crate::fields::Field>,
    /// What a click on it takes, when anything — the window's click reads
    /// the same answer ([`View::clicked`]).
    pub target: Option<Target>,
}

/// What starting said, in the person's words: through the brain it is
/// QUEUED, straight to systemd it has STARTED, and with neither the screen
/// says so — a key that does nothing is indistinguishable from one that did
/// not register. Both doors say the same sentence.
#[must_use]
pub const fn said_of(started: Started) -> &'static str {
    match started {
        Started::Brain => module::key::ECHO_SYSTEM_UPDATE_QUEUED,
        Started::Systemd => module::key::ECHO_SYSTEM_UPDATE_STARTED,
        Started::Nowhere => module::key::ECHO_SYSTEM_UPDATE_NOWHERE,
    }
}

/// What starting it here said ([`begin_anyway`]): it went straight to
/// systemd, as the person's yes asked — on battery, or on this connection —
/// and both doors say the same.
#[must_use]
pub const fn said_anyway(started: Started, on_battery: bool) -> &'static str {
    match (started, on_battery) {
        (Started::Nowhere, _) => module::key::ECHO_SYSTEM_UPDATE_NOWHERE,
        (_, true) => module::key::ECHO_SYSTEM_UPDATE_ON_BATTERY,
        (_, false) => module::key::ECHO_SYSTEM_UPDATE_ON_METERED,
    }
}

/// Follows both journals — the root half's and the person's — each on a
/// thread of its own, sending every line down `to`. The children come back so
/// the door that opened them KILLS them: `-f` never ends by itself.
///
/// Each half is followed in both its units, the run's and the rehearsal's
/// ([`HALVES`]): a rehearsal is a run the verdicts already show — both write
/// the same reports — and its output is what it is for. A step whose
/// rehearsal is a unit of its own (the trim of the system's generations)
/// relays what that unit said into the rehearsing half's output, so which
/// generations would go is on screen here, in both doors.
pub fn follow_both(to: &crate::region::Post<Back>) -> Vec<std::process::Child> {
    // The root half runs first and the person's half BLOCKS on it —
    // `systemctl start` on a `Type=oneshot` returns when the work is done —
    // so the two are never producing at the same time except at the
    // hand-over. Arrival order is the sequence's order, and nothing here
    // claims more than that.
    HALVES
        .iter()
        .filter_map(|(units, of_the_person)| follow(units, *of_the_person, to))
        .collect()
}

/// The two halves the view follows, root's first, each as the units it runs
/// in — the one that acts and the one that rehearses — and whether it is the
/// person's (`journalctl --user`).
const HALVES: [(&[&str], bool); 2] = [
    (&[unit::SYSTEM, unit::SYSTEM_REHEARSAL], false),
    (&[unit::RUN, unit::REHEARSE], true),
];

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
        // THE TWO JOURNALS, each on a thread of its own, handed back through
        // the door's wake (`Content::woken`) — and root's netrc's verdict.
        let mut open = Open::new(View::read(), Reach::Machine, Some(w.wake()));
        open.follow_the_journals();

        // THE BRAIN'S MIND, for the countdown's two answers: what `a` sends
        // depends on whether a run is announced or already running, and only
        // the brain says which (`brainlink::answer_of`).
        let (minds, mind) = channel::channel::<ronoctua_brain::Mind>();
        w.handle
            .insert_source(mind, |event, (), door: &mut Door<UpdateWindow>| {
                if let channel::Event::Msg(m) = event {
                    door.content.mind_arrived(&mut door.ui, m);
                }
            })
            .map_err(|e| anyhow::anyhow!("could not hang the brain: {e}"))?;
        crate::brainlink::listen(minds);

        w.handle
            .insert_source(
                timer::Timer::from_duration(BEAT),
                |_, (), door: &mut Door<UpdateWindow>| {
                    if door.content.open.view.reread_if_moved() {
                        door.ui.ask_paint();
                    }
                    timer::TimeoutAction::ToDuration(BEAT)
                },
            )
            .map_err(|e| anyhow::anyhow!("could not hang the clock: {e}"))?;

        Ok(UpdateWindow::of(open))
    })
}

/// The window: the view both doors show — the footer's region holds the
/// same (`update_region::Open`) — and this door's own page: two halves.
pub struct UpdateWindow {
    /// The steps, their verdicts, the cursor, the output, the run followed
    /// and where a key reaches — the footer's region's too.
    open: Open,
    /// The first visible step, when they do not all fit.
    step_offset: usize,
    /// How many lines above the tail the output is showing. Zero follows.
    output_up: usize,
    /// The window's lines under its header, as the door last counted them —
    /// kept so the arithmetic of the two halves can be asked from a key.
    lines: u16,
    /// How many of those lines the second register takes, wrapped whole
    /// ([`UpdateWindow::measure`]) — at least one.
    register: u16,
    /// The first line of the binary cache's page drawn, when it does not
    /// all fit: the page follows its cursor. One number, which the painting
    /// sets and the pointer reads.
    page_offset: usize,
}

/// Where a page's rows are counted from among the rows the seats light: far
/// past any step and any head line, so a row of the page and a step are
/// never the same number.
const PAGE_ROWS: usize = 1 << 20;

/// The window a photograph takes (`ronoctua-shell photograph --door update`).
///
/// The declared steps — the module's, as the window on a screen has them —
/// with the verdicts of the `halves` given, each a report as its runner
/// writes it, and `said` as the two journals would have printed it, a line
/// each. Nothing of this machine's is read: no report under `/run` or the
/// runtime directory, no journal followed, no brain asked. What a key would
/// have started or answered is kept (`would_have`), never done.
#[must_use]
pub fn photographed(halves: &[String], said: &str) -> UpdateWindow {
    let mut view = View::of(module::steps(), Verdicts::of(halves), Vec::new());
    for line in said.lines() {
        view.heard(readable(line));
    }
    UpdateWindow::of(Open::new(view, Reach::Kept(Vec::new()), None))
}

/// `ronoctua-shell photograph --door update-scene …`: the update's window
/// over a scene handed in whole — the reports, what the journals said, the
/// machine's records and its battery — walked by keys and the pointer, and
/// developed by the scenes' camera. Nothing of this machine is read, and a
/// key that would start or open something is kept and printed, never done.
pub fn photographed_scene(rest: &[String]) -> ! {
    let line: Vec<String> = std::env::args().skip(2).collect();
    let words = if line.ends_with(rest) {
        line
    } else {
        rest.to_vec()
    };
    match camera::parse(&words).and_then(|asked| camera::shoot(&asked)) {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("Error: {e:?}");
            std::process::exit(1);
        }
    }
}

/// The window's scene camera.
mod camera {
    use anyhow::{Context as _, Result, bail, ensure};
    use ronoctua_tokens as tokens;
    use smithay_client_toolkit::seat::pointer::{PointerEvent, PointerEventKind};

    use super::{UpdateWindow, View, module};
    use crate::door::{Content as _, Detached, Ui};

    const USAGE: &str = "\
usage: ronoctua-shell photograph --door update-scene --scene <file> [options] [steps]

  the update's window over a scene given whole (JSON): the two halves'
  reports, what the journals said, the machine's records and its battery

  --scene <file>        the scene:
                          {\"reports\": [\"<id>\\t<verdict>\\t<s>\\n…\"],
                           \"said\": \"<a line>\\n…\",
                           \"records\": {\"conflicts\": 2,
                             \"held\": [{\"id\": \"nix:hello\", \"why\": \"build-failed\"}],
                             \"trial\": \"failed\", \"no-trial\": \"not-in-menu\",
                             \"hold\": {\"rev\": \"<hex>\", \"since\": \"<stamp>\"}},
                           \"battery\": {\"discharging\": true, \"percent\": 20},
                           \"metered\": true,
                           \"keys\": \"SUPER s v\",
                           \"cache\": {\"named\": {\"name\": \"someone\",
                               \"public-key\": \"someone.cachix.org-1:…=\",
                               \"visibility\": \"private\"},
                             \"copy\": \"unreadable\" | \"newer\",
                             \"applied\": {…the same shape…},
                             \"token\": \"kept\" | \"token-open\" | …,
                             \"netrc\": true,
                             \"last\": {\"at\": \"<stamp>\", \"cache\": \"someone\",
                               \"visibility\": \"private\", \"applied-at\": null,
                               \"outcome\": \"pushed\", \"paths\": 3, \"bytes\": 9}}}
                           \"follow\": {\"copy\": \"on\" | \"off\" | \"unreadable\" | \"newer\",
                             \"let\": true, \"registry\": {…distro-apps.json…},
                             \"applied\": {…person-applied.json…}},
                           \"parts\": {\"records\": [{…a part's record…}],
                             \"names\": {\"<id>\": \"<name>\"},
                             \"stick\": {…stick.json…},
                             \"mirrors\": [\"https://…\"]}}
                        `metered`: the brain holds the run a `u` asks for,
                        for a metered way out; `cache`: the view opens on the
                        binary cache's page, over these readings; `follow`:
                        on the page of the apps that follow their releases;
                        `parts`: on the parts' page
  --out <file>          where to write                 (default: update.png)
  --columns <n>         the window's width, in cells   (default: 90)
  --lines <n>           the height, in lines           (default: 30)
  --language <id>       pt-BR or en-GB                 (default: pt-BR)
  --theme <id>          a theme of assets/themes       (default: the default)
  --scale <n>           physical px per logical        (default: 1)
  --list                prints the frame instead of drawing it

Steps — walked in the order they appear:

  --key <notation>      a key, as the keymap writes it: u, <down>, RET
  --hover <col,line>    rests the pointer on a cell of the surface
  --click <col,line>    a press and a release there";

    /// The scene: what the window would have read.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct Scene {
        reports: Vec<String>,
        said: String,
        records: Records,
        battery: Option<Battery>,
        metered: bool,
        keys: String,
        cache: Option<CacheScene>,
        follow: Option<FollowScene>,
        parts: Option<PartsScene>,
    }

    /// The parts' page, as the scene says it: the records root would have
    /// written, the plan's names, the last stick gesture and the media
    /// mounted.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct PartsScene {
        records: Vec<ronoctua_parts::record::Record>,
        names: std::collections::BTreeMap<String, String>,
        stick: Option<ronoctua_parts::stick::Said>,
        mirrors: Vec<String>,
    }

    /// The page of the apps that follow their releases, as the scene says
    /// it: the person's copy, their consent, and the registry's report as
    /// the machine would have it.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct FollowScene {
        copy: Option<String>,
        #[serde(rename = "let")]
        let_in: bool,
        registry: Option<ronoctua_person::reports::DistroApps>,
        applied: Option<ronoctua_person::reports::PersonApplied>,
    }

    impl FollowScene {
        fn readings(self) -> Result<crate::follow::Readings> {
            use crate::follow::Choice;
            let copy = match self.copy.as_deref() {
                None | Some("off") => Choice::Off,
                Some("on") => Choice::On,
                Some("unreadable") => Choice::Unreadable,
                Some("newer") => Choice::Newer,
                Some(other) => bail!("`copy` is on, off, unreadable or newer, not `{other}`"),
            };
            let (machine, apps) = self
                .registry
                .as_ref()
                .map_or((None, Vec::new()), crate::follow::of_registry);
            Ok(crate::follow::Readings {
                copy,
                let_in: self.let_in,
                machine,
                apps,
                taken: self
                    .applied
                    .as_ref()
                    .map_or(crate::follow::Taken::Yes, crate::follow::Taken::of),
            })
        }
    }

    /// The binary cache's page, as the scene says it.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct CacheScene {
        named: Option<ronoctua_person::options::BinaryCache>,
        copy: Option<String>,
        applied: Option<ronoctua_person::options::BinaryCache>,
        token: Option<String>,
        netrc: bool,
        last: Option<ronoctua_cachix::record::Record>,
    }

    impl CacheScene {
        fn readings(self) -> Result<crate::binary_cache::Readings> {
            use crate::binary_cache::TokenState;
            use ronoctua_cachix::settings::Named;
            let named = match (self.copy.as_deref(), self.named) {
                (Some("unreadable"), _) => Named::Unreadable,
                (Some("newer"), _) => Named::Newer,
                (Some(other), _) => bail!("`copy` is unreadable or newer, not `{other}`"),
                (None, Some(c)) => Named::Cache(c),
                (None, None) => Named::None,
            };
            let token = match self.token.as_deref() {
                None => TokenState::None,
                Some("kept") => TokenState::Kept,
                Some(word) => TokenState::Refused(
                    ronoctua_cachix::Why::of_id(word)
                        .with_context(|| format!("`token` is kept or a refusal, not `{word}`"))?,
                ),
            };
            Ok(crate::binary_cache::Readings {
                named,
                applied: self.applied,
                token,
                netrc: self.netrc,
                last: self.last,
                verdict: None,
            })
        }
    }

    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
    struct Records {
        conflicts: usize,
        held: Vec<Held>,
        trial: Option<module::TrialOutcome>,
        no_trial: Option<module::NoTrial>,
        hold: Option<Hold>,
    }

    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Held {
        id: String,
        why: module::HeldWhy,
    }

    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Hold {
        rev: String,
        since: String,
    }

    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Battery {
        discharging: bool,
        percent: Option<u8>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum Step {
        Key(ronoctua_keymap::Key),
        Hover(u16, u16),
        Click(u16, u16),
    }

    #[derive(Debug)]
    pub(super) struct Asked {
        scene: Option<std::path::PathBuf>,
        out: std::path::PathBuf,
        columns: u16,
        lines: u16,
        language: String,
        theme: tokens::theme::Theme,
        scale: u16,
        list: bool,
        steps: Vec<Step>,
    }

    fn number(flag: &str, raw: &str, range: std::ops::RangeInclusive<u16>) -> Result<u16> {
        let n: u16 = raw
            .trim()
            .parse()
            .ok()
            .with_context(|| format!("`{flag} {raw}` is not a whole number"))?;
        ensure!(
            range.contains(&n),
            "`{flag} {n}` is out of range {}..={}",
            range.start(),
            range.end()
        );
        Ok(n)
    }

    fn cell_of(flag: &str, raw: &str) -> Result<(u16, u16)> {
        let (c, l) = raw
            .split_once(',')
            .with_context(|| format!("`{flag}` wants `column,line`, not `{raw}`"))?;
        Ok((number(flag, c, 0..=999)?, number(flag, l, 0..=999)?))
    }

    pub(super) fn parse(words: &[String]) -> Result<Asked> {
        let mut me = Asked {
            scene: None,
            out: std::path::PathBuf::from("update.png"),
            columns: 90,
            lines: 30,
            language: "pt-BR".to_owned(),
            theme: tokens::theme::default_one(),
            scale: 1,
            list: false,
            steps: Vec::new(),
        };
        let mut words = words.iter();
        while let Some(flag) = words.next() {
            let mut value = || {
                words
                    .next()
                    .cloned()
                    .with_context(|| format!("`{flag}` with no value"))
            };
            match flag.as_str() {
                "--help" | "-h" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                "--door" => {
                    let said = value()?;
                    ensure!(
                        said == "update-scene",
                        "`--door {said}` is not this camera's door"
                    );
                }
                "--scene" => me.scene = Some(value()?.into()),
                "--out" | "-o" => me.out = value()?.into(),
                "--columns" => me.columns = number("--columns", &value()?, 1..=1000)?,
                "--lines" => me.lines = number("--lines", &value()?, 1..=1000)?,
                "--language" => me.language = value()?,
                "--theme" => {
                    let id = value()?;
                    me.theme = tokens::theme::by_id(&id)
                        .with_context(|| format!("unknown theme `{id}`"))?;
                }
                "--scale" => {
                    let most = u16::try_from(ronoctua_render::MOST_SCALE).unwrap_or(1);
                    me.scale = number("--scale", &value()?, 1..=most)?;
                }
                "--list" => me.list = true,
                "--key" => {
                    let notation = value()?;
                    let key = ronoctua_keymap::Key::parse(&notation)
                        .map_err(|e| anyhow::anyhow!("`--key {notation}` is not a key: {e}"))?;
                    me.steps.push(Step::Key(key));
                }
                "--hover" => {
                    let (c, l) = cell_of("--hover", &value()?)?;
                    me.steps.push(Step::Hover(c, l));
                }
                "--click" => {
                    let (c, l) = cell_of("--click", &value()?)?;
                    me.steps.push(Step::Click(c, l));
                }
                other => bail!("unknown option `{other}`.\n\n{USAGE}"),
            }
        }
        Ok(me)
    }

    fn middle_of(column: u16, line: u16) -> (f64, f64) {
        let px = tokens::text::size();
        (
            (f64::from(column) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
            (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
        )
    }

    /// The window the scene describes.
    fn window_of(mut scene: Scene) -> Result<UpdateWindow> {
        let cache = scene.cache.take().map(CacheScene::readings).transpose()?;
        let follow = scene.follow.take().map(FollowScene::readings).transpose()?;
        let parts = scene.parts.take().map(|p| {
            let mut r = crate::parts::Readings::of(p.records, &p.names, p.stick);
            r.mirrors = crate::parts::Mirrors::Named(p.mirrors);
            r
        });
        let mut w = window_of_records(scene);
        if let Some(readings) = cache {
            w.open.view = w.open.view.clone().with_cache(readings);
        }
        if let Some(readings) = follow {
            w.open.view = w.open.view.clone().with_follow(readings);
        }
        if let Some(readings) = parts {
            w.open.view = w.open.view.clone().with_parts(readings);
        }
        Ok(w)
    }

    /// The window of the scene's reports, records and battery.
    fn window_of_records(scene: Scene) -> UpdateWindow {
        let mut w = super::photographed(&scene.reports, &scene.said);
        let records = module::Records {
            conflicts: scene.records.conflicts,
            held: scene
                .records
                .held
                .into_iter()
                .map(|h| module::Held {
                    id: h.id,
                    why: h.why,
                })
                .collect(),
            trial: scene.records.trial,
            no_trial: scene.records.no_trial,
            hold: scene.records.hold.map(|h| module::HeldAt {
                rev: h.rev,
                since: h.since,
            }),
        };
        let charge = scene.battery.map_or(
            module::Charge {
                discharging: false,
                percent: None,
            },
            |b| module::Charge {
                discharging: b.discharging,
                percent: b.percent,
            },
        );
        let view: View = w
            .open
            .view
            .clone()
            .with_records(records, scene.keys)
            .on_charge(charge);
        w.open.view = view;
        w.open.held_metered = scene.metered;
        w
    }

    pub(super) fn shoot(asked: &Asked) -> Result<()> {
        tokens::theme::set(asked.theme);
        tokens::theme::set_ground(tokens::theme::Ground::Chrome);
        tokens::text::set(tokens::text::NORMAL);
        crate::when::prefs::set(crate::when::Choice::default());
        let texts = ronoctua_i18n::Texts::fresh(&asked.language)
            .with_context(|| format!("the language `{}` does not exist", asked.language))?;
        let scene: Scene = match &asked.scene {
            Some(file) => {
                let raw = std::fs::read_to_string(file)
                    .with_context(|| format!("could not read `{}`", file.display()))?;
                serde_json::from_str(&raw)
                    .with_context(|| format!("`{}` is not an update scene", file.display()))?
            }
            None => Scene::default(),
        };
        let ui = Ui::detached(texts, (1, 1), i32::from(asked.scale))?;
        let mut d = Detached::new(ui, window_of(scene)?);
        let px = tokens::text::size();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let size = (
            (f32::from(asked.columns + 2 * d.content.margin()) * tokens::metrics::cell_width(px))
                .ceil() as u32,
            (f32::from(asked.lines) * tokens::metrics::line_height(px)).ceil() as u32,
        );
        d.lay_out_at(size);
        d.turn();
        let mut gpu = ronoctua_render::offscreen::Offscreen::new(1, 1)?;
        let format = ronoctua_render::offscreen::FORMAT;
        let mut images = ronoctua_render::image::Images::new(&gpu.device, format)?;
        // A FRAME BEFORE EVERY STEP: a key of the header is grabbed where the
        // last paint drew it, so a pointer that moves before anything was
        // composed finds no key under it.
        let mut composed = d.compose(&gpu.device, &gpu.queue, &mut images);
        let mut entered = false;
        for step in &asked.steps {
            match *step {
                Step::Key(key) => d.press(key),
                Step::Hover(c, l) | Step::Click(c, l) => {
                    let event = |kind| {
                        vec![PointerEvent {
                            surface: d.ui.surface.clone(),
                            position: middle_of(c, l),
                            kind,
                        }]
                    };
                    let first = if std::mem::replace(&mut entered, true) {
                        PointerEventKind::Motion { time: 0 }
                    } else {
                        PointerEventKind::Enter { serial: 0 }
                    };
                    let mut frames = vec![event(first)];
                    if matches!(step, Step::Click(..)) {
                        for kind in [
                            PointerEventKind::Press {
                                time: 0,
                                button: crate::door::LEFT_BUTTON,
                                serial: 0,
                            },
                            PointerEventKind::Release {
                                time: 0,
                                button: crate::door::LEFT_BUTTON,
                                serial: 0,
                            },
                        ] {
                            frames.push(event(kind));
                        }
                    }
                    for events in frames {
                        d.pointer(&events);
                    }
                }
            }
            d.turn();
            if d.ui.quit {
                break;
            }
            composed = d.compose(&gpu.device, &gpu.queue, &mut images);
        }
        let film = crate::photo::Film {
            list: asked.list,
            by: asked.scale,
            language: &asked.language,
            out: asked.out.clone(),
        };
        crate::photo::draw(
            &mut gpu,
            &mut images,
            &composed,
            (d.ui.width, d.ui.height),
            &film,
            i32::from(asked.scale),
        )?;
        if let Some(m) = d.ui.message.as_ref() {
            println!("the message line says: {m}");
        }
        for asked in d.content.would_have() {
            println!("{asked}");
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// A SCENE IS READ WHOLE, and a field it does not have is refused
        /// rather than drawn as if the record said nothing.
        #[test]
        fn a_scene_is_its_records_and_nothing_else() {
            let scene: Scene = serde_json::from_str(
                r#"{"records": {"conflicts": 2, "held": [{"id": "nix:hello", "why": "build-failed"}],
                    "trial": "failed", "no-trial": "not-in-menu",
                    "hold": {"rev": "613862c", "since": "2026-10-05T12:00:00Z"}},
                    "battery": {"discharging": true, "percent": 20}, "keys": "SUPER s v"}"#,
            )
            .expect("a scene");
            let w = window_of(scene).expect("a window");
            let r = w.open.view.records();
            assert_eq!(r.conflicts, 2);
            assert_eq!(r.trial, Some(module::TrialOutcome::Failed));
            assert_eq!(r.held.len(), 1);
            assert!(serde_json::from_str::<Scene>(r#"{"recrods": {}}"#).is_err());
            assert!(parse(&["--door".to_owned(), "update".to_owned()]).is_err());
        }
    }
}

impl UpdateWindow {
    /// The window over `open`.
    const fn of(open: Open) -> Self {
        Self {
            open,
            step_offset: 0,
            output_up: 0,
            lines: 22,
            register: 1,
            page_offset: 0,
        }
    }

    /// The brain spoke: its mind is kept — the countdown's answers read it —
    /// and a run this window asked for is followed in it: held for a metered
    /// way out, the question, the no first.
    fn mind_arrived(&mut self, ui: &mut Ui, mind: ronoctua_brain::Mind) {
        if self.open.heard_the_mind(mind, ui) {
            self.measure(ui);
        }
    }

    /// What a photograph's window would have asked past the door, in order,
    /// as the photograph prints it — nothing for the window on a screen,
    /// which asked for real.
    #[must_use]
    pub fn would_have(&self) -> Vec<String> {
        self.open.would_have()
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
        let below = self.lines.saturating_sub(self.register);
        usize::from(below / 2).min(self.open.view.steps().len())
    }

    /// The first row of the output region, counted from the first line under
    /// the header.
    fn output_top(&self) -> u16 {
        self.register + u16::try_from(self.steps_room()).unwrap_or(0)
    }

    /// Counts the window's lines under its header and the second register's
    /// share of them — the one count the painting, the keys and the pointer
    /// read, so a step is clicked on the row it is drawn on.
    ///
    /// # Why the register is wrapped and not cut
    ///
    /// Because it is a note, and a note wraps whole (`chrome::note_lines`):
    /// the end of a step's sentence is where its consequence is — the trim's
    /// says what a rehearsal shows — and the footer's region already wraps
    /// the same sentence (`View::register_lines`). Cut here, the two doors
    /// would say two different things about one step.
    ///
    /// AT MOST ALL BUT TWO LINES, a step's and the output's: what does not fit
    /// a window that small is past its edge — nothing leaks off the screen —
    /// and never an ellipsis.
    fn measure(&mut self, ui: &Ui) {
        self.lines = ui.lines.saturating_sub(ui.first_of_list());
        let wrapped = self.open.view.top_lines(ui.columns, &ui.texts).len();
        self.register = u16::try_from(wrapped)
            .unwrap_or(u16::MAX)
            .clamp(1, self.lines.saturating_sub(2).max(1));
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
        let cursor = self.open.view.cursor();
        if cursor < self.step_offset {
            self.step_offset = cursor;
        } else if cursor >= self.step_offset + room {
            self.step_offset = cursor + 1 - room;
        }
        self.step_offset = self
            .step_offset
            .min(self.open.view.steps().len().saturating_sub(room));
    }

    /// Keeps the output's offset inside what there is to show.
    fn follow_the_tail(&mut self) {
        let most = self
            .open
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
        let total = self.open.view.scrollback().amount();
        crate::chrome::list_scroll(total, room, total.saturating_sub(self.output_up + room))
    }

    /// Which step row a pointer position lands on, if any.
    fn at(&self, ui: &Ui, x: f64, y: f64) -> Option<usize> {
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line_px <= 0.0 || x < 0.0 || y < 0.0 || self.open.view.page_open() {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_px) as u16;
        let first_step = ui.first_of_list() + self.register;
        if line < first_step || line >= ui.first_of_list() + self.output_top() {
            return None;
        }
        let row = self.step_offset + usize::from(line - first_step);
        (row < self.open.view.steps().len()).then_some(row)
    }

    /// Which line of the head a pointer position lands on — the question,
    /// the register, the records — if any. The painting draws the same
    /// lines in the same rows (`top_lines`, `measure`).
    fn head_at(&self, ui: &Ui, y: f64) -> Option<usize> {
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line_px <= 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_px) as u16;
        let first = ui.first_of_list();
        (line >= first && line < first + self.register).then(|| usize::from(line - first))
    }

    /// What a pointer position takes: a step, or a head line that takes
    /// something — the one answer the painting's band and the click read.
    fn target_at(&self, ui: &Ui, x: f64, y: f64) -> Option<Target> {
        if let Some(row) = self.at(ui, x, y) {
            return Some(Target::Step(row));
        }
        if let Some(row) = self.page_row_at(ui, x, y) {
            return Some(Target::Page(row));
        }
        let n = self.head_at(ui, y)?;
        self.open
            .view
            .top_lines(ui.columns, &ui.texts)
            .get(n)
            .and_then(|l| l.target)
    }

    /// The row a target is lit as under the pointer: a step by its index, a
    /// head line past the last step — one number, which the seats' `lit`
    /// carries.
    fn lit_row_of(&self, ui: &Ui, target: Target) -> Option<usize> {
        match target {
            Target::Step(n) => Some(n),
            Target::Page(n) => Some(PAGE_ROWS + n),
            other => self
                .open
                .view
                .top_lines(ui.columns, &ui.texts)
                .iter()
                .position(|l| l.target == Some(other))
                .map(|n| self.open.view.steps().len() + n),
        }
    }

    /// Which row of the page the view is turned to a pointer position lands
    /// on — the lines the painting draws under the head, from the same
    /// function.
    fn page_row_at(&self, ui: &Ui, x: f64, y: f64) -> Option<usize> {
        if !self.open.view.page_open() {
            return None;
        }
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        if line_px <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let line = (y / line_px) as u16;
        let first = ui.first_of_list() + self.register;
        let n = usize::from(line.checked_sub(first)?) + self.page_offset;
        self.open
            .view
            .page_lines(0, ui.columns, &ui.texts)
            .get(n)
            .and_then(|l| l.row)
            .filter(|row| self.open.view.page_takes(*row))
    }

    /// The first line of the page that keeps its cursor in `room` lines.
    fn page_offset_for(
        lines: &[crate::binary_cache::PageLine],
        cursor: usize,
        room: usize,
    ) -> usize {
        if lines.len() <= room || room == 0 {
            return 0;
        }
        let at = lines
            .iter()
            .position(|l| l.row == Some(cursor))
            .unwrap_or(0);
        at.saturating_sub(room - 1).min(lines.len() - room)
    }

    /// Carries out what the view asked for: this window's own page — the
    /// step cursor followed, the output a page up or down from its tail, or
    /// at its end — and the rest through the footer's region's handler, with
    /// the same sentences.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        if errand == Errand::Moved {
            self.follow_the_cursor();
        }
        if let Errand::Output(how) = errand {
            let page = self.output_room().max(1);
            match how {
                Output::Up => self.output_up = self.output_up.saturating_add(page),
                Output::Down => self.output_up = self.output_up.saturating_sub(page),
                Output::End => self.output_up = 0,
            }
            self.follow_the_tail();
        }
        carry_out(errand, &mut self.open, ui);
        self.measure(ui);
        ui.ask_paint();
    }

    /// One key, from the keyboard or from a click on the header.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        let errand = self.open.view.key(press);
        // AFTER THE KEY: the cursor's new step has a sentence of its own, and
        // the room the page follows it in is what is left under it.
        self.measure(ui);
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
        format!("*{}*", texts.t(module::key::CMD_SYSTEM_UPDATE))
    }

    /// One key, from a press or from a REPEAT — holding an arrow walks the
    /// table, which is what a list wants — read by the reader the footer
    /// calls too.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        // THE FILE PICKER, while it is up over the parts' page, reads its
        // keys before the view does.
        if self.open.view.picker_takes(key) {
            ui.message = None;
            let errand = self.open.view.pick_key(key);
            self.measure(ui);
            self.apply(ui, errand);
            return;
        }
        if let Some(press) = press_of_key(key) {
            self.pressed(ui, press);
        }
    }

    /// What was copied, for the file picker.
    fn pasted(&mut self, ui: &mut Ui, text: &str) {
        self.open.view.picker_pasted(text);
        self.measure(ui);
        ui.ask_paint();
    }

    /// Where content begins, in cells from the window's edge.
    fn margin(&self) -> u16 {
        CONTENT_MARGIN
    }

    /// THE PAGES' QUESTIONS — the binary cache's, the parts' — typed on
    /// this window's line: the footer's minibuffer answers into the same
    /// `View::page_answered`.
    fn answered(&mut self, ui: &mut Ui, id: &'static str, text: &str) {
        let errand = self.open.view.page_answered(id, text);
        self.measure(ui);
        self.apply(ui, errand);
    }

    /// The line was let go: the page's question goes with it.
    fn unanswered(&mut self, ui: &mut Ui, _: &'static str) {
        self.open.view.page_unanswered();
        ui.ask_paint();
    }

    /// THE KEYS THE FOOTER TEACHES, on this window's header — every one a
    /// click, `u`, `a` and `p` included.
    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(&ui.texts)
    }

    fn resized(&mut self, ui: &mut Ui) {
        self.measure(ui);
        self.follow_the_cursor();
        self.follow_the_tail();
    }

    /// What the journals and root's netrc handed back through the door's
    /// wake. NOTHING IS PAINTED HERE for a line: it lands in the scrollback,
    /// and the frame is owed once the queue is empty (`each_turn`) — see
    /// `Scrollback` for what a redraw per line costs.
    fn woken(&mut self, ui: &mut Ui) {
        let _ = self.open.woken(ui);
    }

    /// THE SCROLLBACK'S DEBT, paid once the queue is empty. A thousand lines of
    /// a rebuild arriving in one burst are one paint, not a thousand.
    fn each_turn(&mut self, ui: &mut Ui) {
        if self.open.view.owes_a_frame() {
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
        let name = texts.t(module::key::CMD_SYSTEM_UPDATE);
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
        self.measure(ui);
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

        // THE HEAD — the question on battery, or the second register (what
        // the step under the cursor really is) — and what the machine's
        // records say, WRAPPED WHOLE, on the lines `measure` counted for it.
        // A line that takes something lights under the pointer, as a step
        // does: the pointer lights what the click takes.
        // A SENTENCE WRAPPED OVER TWO LINES IS ONE CONTROL: the pointer on
        // either lights both — what a click takes is the sentence, and the
        // seat carries the line it rests on.
        let steps_len = self.open.view.steps().len();
        let heads = self.open.view.top_lines(columns, &texts);
        let target_of_row = |row: Option<usize>| {
            row.and_then(|r| r.checked_sub(steps_len))
                .and_then(|h| heads.get(h))
                .and_then(|l| l.target)
        };
        for (n, head) in heads.iter().take(usize::from(self.register)).enumerate() {
            let y = first + u16::try_from(n).unwrap_or(0);
            if head.target.is_some()
                && let Some((color, _)) = canvas
                    .lit
                    .iter()
                    .find(|(_, under)| target_of_row(*under) == head.target)
            {
                canvas.under.push(ronoctua_render::quad::Quad {
                    x: 0.0,
                    y: f32::from(y) * line_px,
                    width,
                    height: line_px,
                    color: *color,
                });
            }
            canvas
                .spans
                .push(span(head.text.clone(), margin, y, head.color));
        }

        // THE PAGE THE VIEW IS TURNED TO, in place of the steps and the
        // output: its heading and its rows under the head, the cursor a
        // colour and the row under the pointer lit — from the lines the
        // pointer reads too (`page_row_at`).
        if let Some(cursor) = self.open.view.page_cursor() {
            let below = first + self.register;
            let lines = self.open.view.page_lines(0, columns, &texts);
            // A WINDOW TOO SHORT FOR THE PAGE keeps its cursor in sight: the
            // page follows the cursor, which the arrows and the wheel walk.
            let room = usize::from(ui.lines.saturating_sub(below));
            self.page_offset = Self::page_offset_for(&lines, cursor, room);
            for (n, l) in lines.into_iter().skip(self.page_offset).enumerate() {
                let y = below + u16::try_from(n).unwrap_or(u16::MAX);
                if y >= ui.lines {
                    break;
                }
                if let Some(row) = l.row
                    && self.open.view.page_takes(row)
                    && let Some((color, _)) = canvas
                        .lit
                        .iter()
                        .find(|(_, under)| *under == Some(PAGE_ROWS + row))
                {
                    canvas.under.push(ronoctua_render::quad::Quad {
                        x: 0.0,
                        y: f32::from(y) * line_px,
                        width,
                        height: line_px,
                        color: *color,
                    });
                }
                for f in l.fields {
                    canvas.spans.push(span(f.text, margin + f.at, y, f.color));
                }
            }
            // NO BAR: the page fits, so no column of it is a bar's.
            ui.room = crate::chrome::Room {
                width,
                bar_from: None,
            };
            return;
        }

        // THE STEPS, only the ones that fit.
        let room = self.steps_room();
        for (n, step) in self
            .open
            .view
            .steps()
            .iter()
            .skip(self.step_offset)
            .take(room)
            .enumerate()
        {
            let row = self.step_offset + n;
            let y = first + self.register + u16::try_from(n).unwrap_or(0);
            // THE CURSOR AND THE POINTER GET DIFFERENT BANDS, and each is a
            // flat quad with no border and no radius — invariant 15.
            //
            // They shared one colour, so a pointer resting away from the cursor
            // lit two rows identically and neither said "this is the one the
            // arrows move from". The file manager was reported for exactly that
            // and the split is the owner's: the focus keeps `surface.selection`
            // and the pointer takes the weaker `surface.hover`. EVERY CURSOR'S
            // HOVER in its own colour (`ronoctua_shell_kit::seats`).
            //
            // AND THE BAND IS THE WHOLE WIDTH: the only bar this window draws
            // is the output's, below the table, so no column of these rows is
            // a bar's.
            // WHILE THE QUESTION ON BATTERY IS ASKED the cursor is on its
            // answers, and no step wears the cursor's band.
            let band = if row == self.open.view.cursor() && self.open.view.asking().is_none() {
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
            let (verdict, seconds) = self.open.view.verdicts().of_step(step.id);
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
        if self.open.view.scrollback().is_empty() {
            canvas.spans.push(span(
                layout::elide_end(&texts.t(module::key::SYSTEM_UPDATE_NO_OUTPUT), columns),
                margin + GUTTER,
                top,
                tokens::color::TEXT_DIM,
            ));
        } else {
            let room = self.output_room();
            for (n, said) in self
                .open
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
        let before = (
            hand.row_under_pointer,
            self.open.view.cursor(),
            self.output_up,
        );
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
        let total = self.open.view.scrollback().amount();
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
                        hand.row_under_pointer = self
                            .target_at(ui, e.position.0, e.position.1)
                            .and_then(|t| self.lit_row_of(ui, t));
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
                    } else if let Some(target) = self.target_at(ui, e.position.0, e.position.1) {
                        let errand = self.open.view.clicked(target);
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
                    // THE ARITHMETIC IS SHARED — see `wheel::wheel_arrived`.
                    crate::wheel::wheel_arrived(&mut hand.lines_owed, &vertical, &horizontal);
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
            if before
                != (
                    hand.row_under_pointer,
                    self.open.view.cursor(),
                    self.output_up,
                )
            {
                ui.ask_paint();
            }
            return;
        }
        let by = if turned {
            crate::wheel::wheel_lines(&mut hand.lines_owed)
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
            // ON A PAGE the wheel walks its rows, once a notch, as the
            // arrows do — and the page follows its cursor.
            if self.open.view.page_open() {
                let press = if by > 0 { Press::Down } else { Press::Up };
                let errand = self.open.view.key(press);
                self.apply(ui, errand);
            } else if over_the_steps {
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
                let errand = self.open.view.key(press);
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
        if explained
            || before
                != (
                    hand.row_under_pointer,
                    self.open.view.cursor(),
                    self.output_up,
                )
        {
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
        use crate::update_mark::mark;
        // THE PROMISE THIS KEEPS: a machine with nothing running does not spend
        // a column, and does not wake the loop once a second for it.
        assert_eq!(mark(None, 0), None, "with no brain there is no mark");
        assert_eq!(
            mark(Some(&a_mind("", &[])), 0),
            None,
            "a stopped brain has no mark"
        );
        assert!(
            mark(Some(&a_mind(module::ID, &[])), 0).is_some(),
            "running has a mark"
        );
        assert!(
            mark(Some(&a_mind("", &[module::ID])), 0).is_some(),
            "queued has a mark"
        );
        // AND IT IS THIS TASK AND NOT ANY TASK. The brain's door is a `vec![]`
        // and the day a second feature goes in it, a mark that answered to
        // whatever was running would draw a rebuild's glyph over somebody
        // else's work.
        assert_eq!(mark(Some(&a_mind("something-else", &[])), 0), None);
    }

    #[test]
    fn the_view_follows_the_rehearsal_as_well_as_the_run() {
        // A REHEARSAL IS A RUN THE VERDICTS ALREADY SHOW, and what it says —
        // the trim's list of generations that would go among it — is what it
        // is for. Each half is followed in both its units, each in the
        // journal it writes to: the person's with `--user`, root's without.
        let followed = |person: bool| -> Vec<&str> {
            HALVES
                .iter()
                .filter(|(_, of_the_person)| *of_the_person == person)
                .flat_map(|(units, _)| units.iter().copied())
                .collect()
        };
        let mut person = followed(true);
        person.sort_unstable();
        let mut run_and_rehearsal = [unit::RUN, unit::REHEARSE];
        run_and_rehearsal.sort_unstable();
        assert_eq!(person, run_and_rehearsal, "the person's half, in --user");
        let mut root = followed(false);
        root.sort_unstable();
        let mut system = [unit::SYSTEM, unit::SYSTEM_REHEARSAL];
        system.sort_unstable();
        assert_eq!(root, system, "the root half, in the system journal");
        // THE MANIFEST'S TWO FOR THE PERSON are the brain's own, so the
        // units the brain starts are the ones followed.
        assert_eq!(module::MANIFEST.units.run, Some(unit::RUN));
        assert_eq!(module::MANIFEST.units.rehearse, Some(unit::REHEARSE));
        // ROOT FIRST: the person's half blocks on it, so the arrival order of
        // the two is the sequence's.
        assert!(!HALVES[0].1 && HALVES[1].1, "root's half is followed first");
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
        assert_eq!(
            verdict_of("declined-did-not-boot"),
            Some(Verdict::Declined("did-not-boot".to_owned()))
        );
        assert_eq!(verdict_of("declined-"), None);
        assert_eq!(
            verdict_of("failed-build-failed"),
            Some(Verdict::Refused("build-failed".to_owned()))
        );
        assert_eq!(verdict_of("failed-"), None);
        for kind in [
            "always",
            "binary",
            "directory",
            "writable-file",
            "network",
            "file",
            "power",
            "free-space",
            "not-held",
        ] {
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
        let steps = module::steps();
        assert!(!steps.is_empty(), "the declared table came out empty");
        let verdicts = [
            Verdict::Ran,
            Verdict::Failed,
            Verdict::Off,
            Verdict::Skipped("writable-file".to_owned()),
            Verdict::Rehearsed,
            Verdict::Declined("did-not-boot".to_owned()),
            Verdict::Refused("build-failed".to_owned()),
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
        let step = module::steps().first().expect("there are steps");
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
            said.contains(&texts.t(module::key::SYSTEM_UPDATE_NEEDS_NETWORK)),
            "the reason for the skip did not reach the second record: {said}"
        );
        assert!(!said.starts_with('['), "a key with no text: {said}");
        // AND A KIND THE SEQUENCE DOES NOT HAVE ADDS NOTHING — never a key
        // made up of the word.
        let unknown = cursor_explains(
            step,
            &Verdict::Skipped("a-kind-nobody-writes".to_owned()),
            &texts,
        );
        assert_eq!(unknown, cursor_explains(step, &Verdict::Pending, &texts));
        assert!(!unknown.contains('['), "a made-up key reached the screen");
    }

    /// THE NAME ON THE ROW IS THE STEP'S OWN KEY, the one the module declared
    /// and the brain's page draws too — never a key this file builds.
    #[test]
    fn a_step_is_named_by_the_key_its_module_declared() {
        let texts = texts();
        for step in module::steps() {
            let pieces = step_row(step, &Verdict::Pending, 0, 200, &texts);
            let name = pieces.last().expect("a name");
            assert_eq!(name.text, texts.t(step.name_key), "{}", step.id);
            assert!(!name.text.starts_with('['), "`{}` has no name", step.id);
            assert!(
                !cursor_explains(step, &Verdict::Pending, &texts).starts_with('['),
                "`{}` has no sentence",
                step.id
            );
        }
    }

    /// FAILS IF: the binary cache's page, in a window too short for it,
    /// leaves its cursor's row out of sight — or scrolls a page that fits.
    #[test]
    fn the_cache_page_follows_its_cursor_in_a_short_window() {
        let line = |row| crate::binary_cache::PageLine {
            fields: Vec::new(),
            row,
        };
        let lines: Vec<_> = [None, None]
            .into_iter()
            .chain((0..9).map(Some))
            .map(line)
            .collect();
        assert_eq!(UpdateWindow::page_offset_for(&lines, 8, 20), 0, "it fits");
        for (cursor, room) in [(0, 4), (5, 4), (8, 4), (8, 1), (3, 6)] {
            let offset = UpdateWindow::page_offset_for(&lines, cursor, room);
            let at = lines.iter().position(|l| l.row == Some(cursor)).unwrap();
            assert!(
                (offset..offset + room).contains(&at),
                "cursor {cursor} in {room} lines from {offset}"
            );
            assert!(offset + room <= lines.len());
        }
    }

    /// THE THREE COMMANDS ARE THIS FILE'S, and the footer finds each by the
    /// one question it asks — so neither it nor the leader spells an id.
    #[test]
    fn the_three_commands_are_one_table_and_the_footer_finds_each() {
        let mut registry = ronoctua_cmd::Registry::fresh();
        for c in COMMANDS {
            registry
                .register(*c)
                .expect("a command the registry refuses");
        }
        // THE UPDATE'S THREE, the binary cache's five (its page's door, its
        // questions and root's netrc — whose password the sudo bar asks),
        // the door of the page of the apps that follow their releases, and
        // the parts' five (its door and its four questions).
        assert_eq!(COMMANDS.len(), 14);
        assert_eq!(order_of(crate::follow::OPEN), Some(Order::Follow));
        assert_eq!(
            order_of(crate::parts::OPEN),
            Some(Order::Parts(crate::parts::OPEN))
        );
        for c in COMMANDS {
            assert!(order_of(c.id).is_some(), "`{}` is not dispatched", c.id);
        }
        assert_eq!(order_of(START), Some(Order::Start));
        assert_eq!(order_of(OPEN), Some(Order::Open));
        assert_eq!(order_of(WATCH), Some(Order::Watch));
        assert_eq!(order_of("brain-open"), None);
        // EVERY BINDING IS ONE OF THEM, and each of them has a key.
        for (_, id, argument) in BINDINGS {
            assert!(order_of(id).is_some(), "`{id}` is bound and not ours");
            assert!(argument.is_none());
        }
        // EVERY ONE HAS A KEY, but the binary cache's questions, which its
        // page's rows ask (`KEYS.by_name`).
        for c in COMMANDS {
            assert!(
                BINDINGS.iter().any(|(_, id, _)| *id == c.id) || KEYS.by_name.contains(&c.id),
                "`{}` has no key",
                c.id
            );
        }
        // AND THE COMMAND THAT STARTS IT IS THE KEY THE VIEW TEACHES AS `u`.
        assert!(OF_THE_VIEW.contains(&("u", START)));
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
            Verdict::Declined("did-not-boot".to_owned()),
            Verdict::Refused("build-failed".to_owned()),
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
        View::of(module::steps(), Verdicts::default(), Vec::new())
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
        assert_eq!(
            v.key(Press::Enter),
            Errand::Nothing,
            "`RET` on a step does nothing more"
        );
        // A CLICK PUTS THE CURSOR ON THE STEP; on the step it is on, like
        // `RET`, it does nothing more.
        assert_eq!(v.clicked(Target::Step(1)), Errand::Moved);
        assert_eq!(v.cursor(), 1);
        assert_eq!(v.clicked(Target::Step(1)), Errand::Nothing);
        assert_eq!(v.clicked(Target::Step(999)), Errand::Nothing);
        assert_eq!(
            v.clicked(Target::Answer(true)),
            Errand::Nothing,
            "no question, no answer"
        );
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
            assert_eq!(
                row.target,
                Some(Target::Step(i - register)),
                "row {i} is not its step"
            );
        }
        // NOTHING SAID YET is said, once, under a line of air.
        assert_eq!(whole.len(), register + steps + 2);
        let said: String = whole[register + steps + 1]
            .fields
            .iter()
            .map(|f| f.text.as_str())
            .collect();
        assert_eq!(said, t.t(module::key::SYSTEM_UPDATE_NO_OUTPUT));

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
                .filter(|r| r.fields.iter().any(|f| f.color == accent))
                .filter_map(|r| match r.target {
                    Some(Target::Step(n)) => Some(n),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(lit(&v), vec![0]);
        // THE STEP'S PLACE AMONG THE STEPS moves by one; the head above them
        // is the new step's sentence, which wraps to its own length.
        let (_, room) = crate::chrome::extent(80);
        let below_the_head = |v: &View| v.region_cursor(80, &t) - v.top_lines(room, &t).len();
        let before = below_the_head(&v);
        v.key(Press::Down);
        assert_eq!(lit(&v), vec![1]);
        let rows = v.region_rows(0, usize::MAX, 80, &t);
        assert_eq!(rows[v.region_cursor(80, &t)].target, Some(Target::Step(1)));
        assert_eq!(below_the_head(&v), before + 1);
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
            UpdateWindow::of(Open::new(a_view(), Reach::Machine, None)),
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
        for id in [START, ABORT, PROCEED, "system-update-output-end"] {
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
            Some(ui.texts.t(crate::key::ECHO_BRAIN_IDLE).as_str())
        );
        let _ = w
            .open
            .heard_the_mind(ronoctua_brain::Mind::default(), &mut ui);
        typed(&mut w, &mut ui, "p");
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(crate::key::ECHO_BRAIN_IDLE).as_str())
        );
        assert!(ui.orders().is_empty(), "an answer went to the footer");
        typed(&mut w, &mut ui, "ESC");
        assert!(ui.quit);
    }

    /// A PHOTOGRAPH'S WINDOW STARTS NOTHING (`photograph --door update`): its
    /// `u` and the countdown's two answers are kept, in order, and nothing is
    /// said as if they had gone out — and what it draws is what it was given,
    /// the verdicts of the reports and the journals' lines, none of this
    /// machine's.
    ///
    /// THE DEFECT THIS CATCHES: a camera that rebuilds the system. The
    /// window's `u` is `begin`, which asks the brain and, without one,
    /// systemd.
    #[test]
    fn a_photographs_window_reaches_nothing_and_draws_what_it_was_given() {
        let steps = module::steps();
        let (first, second) = (steps[0].id, steps[1].id);
        let halves = vec![
            format!("{first}\tran\t12\n"),
            format!("{second}\trehearsed\t0\n"),
        ];
        let mut w = photographed(
            &halves,
            "would remove system generations 3 4\n\u{1b}[1mkept\u{1b}[0m 5 6\n",
        );
        assert_eq!(w.open.view.verdicts().of_step(first), (Verdict::Ran, 12));
        assert_eq!(
            w.open.view.verdicts().of_step(second),
            (Verdict::Rehearsed, 0)
        );
        assert_eq!(
            w.open.view.scrollback().amount(),
            2,
            "one line per line said"
        );
        assert_eq!(
            w.open.view.scrollback().line(1),
            Some("kept 5 6"),
            "made safe as a journal's line is"
        );
        assert!(
            !w.open.follows_the_journals(),
            "a photograph follows no journal"
        );
        let mut ui = Ui::for_tests(texts());
        for press in ["u", "a", "p"] {
            typed(&mut w, &mut ui, press);
            assert!(ui.message.is_none(), "`{press}` said something went out");
        }
        let kept = w.would_have();
        assert_eq!(kept.len(), 3, "{kept:?}");
        assert!(
            kept[0].contains(module::ID)
                && kept[1].ends_with("abort")
                && kept[2].ends_with("proceed"),
            "{kept:?}"
        );
        // AND THE WINDOW ON A SCREEN KEEPS NOTHING: it asked for real.
        assert!(a_window().0.would_have().is_empty());
    }

    /// THE WINDOW'S SECOND REGISTER IS WRAPPED WHOLE, as the footer's region
    /// wraps it, and the step rows start under its last line — where the
    /// pointer finds them too.
    ///
    /// THE DEFECT THIS CATCHES: the window cut the sentence with an ellipsis
    /// at its width, so the end of it — the trim's says what a rehearsal
    /// shows — was in the footer and never in the window.
    #[test]
    fn the_windows_second_register_is_wrapped_whole_and_the_rows_follow_it() {
        let (mut w, mut ui) = a_window();
        let longest = (0..w.open.view.steps().len())
            .max_by_key(|&n| ui.texts.t(w.open.view.steps()[n].does_key).len())
            .expect("steps");
        for _ in 0..longest {
            typed(&mut w, &mut ui, "<down>");
        }
        assert_eq!(w.open.view.cursor(), longest);
        let wrapped = w.open.view.register_lines(ui.columns, &ui.texts);
        assert!(
            wrapped.len() > 1,
            "the longest sentence fits one line: {wrapped:?}"
        );
        assert_eq!(usize::from(w.register), wrapped.len());
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            words(&wrapped.join(" ")),
            words(&w.open.view.explains(&ui.texts).expect("a sentence")),
            "the register is not the whole sentence"
        );
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let middle = |line: u16| (f64::from(line) + 0.5) * line_px;
        let first = ui.first_of_list() + w.register;
        assert_eq!(w.at(&ui, 10.0, middle(first)), Some(w.step_offset));
        assert_eq!(
            w.at(&ui, 10.0, middle(first - 1)),
            None,
            "the register's last line is a step"
        );
        assert_eq!(
            w.output_top(),
            w.register + u16::try_from(w.steps_room()).expect("small")
        );
    }

    /// THE OUTPUT MOVES ON THIS DOOR'S PAGE: `C-k` a page up from the tail,
    /// `C-j` back down, `G` to the end — and an arrow walks the steps.
    #[test]
    fn the_output_and_the_steps_move_on_the_windows_page() {
        let (mut w, mut ui) = a_window();
        for n in 0..500 {
            w.open.view.heard(format!("{n}"));
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
        assert_eq!(w.open.view.cursor(), 2);
        assert!(ui.message.is_none());
    }

    /// A machine running off a battery below the sequence's floor.
    fn flat() -> module::Charge {
        module::Charge {
            discharging: true,
            percent: Some(module::battery_floor().expect("a floor") - 1),
        }
    }

    /// A RUN BY HAND BELOW THE FLOOR ASKS FIRST, the no first and the
    /// cursor born on it: `u` opens the question and starts nothing; the
    /// arrows move between the two answers, `RET` answers the one under the
    /// cursor, `Esc` is the no. On mains — or above the floor — `u` starts.
    #[test]
    fn a_run_by_hand_below_the_floor_asks_with_the_no_first() {
        let t = texts();
        let mut v = a_view().on_charge(flat());
        assert_eq!(
            v.key(Press::Start),
            Errand::Moved,
            "it started without asking"
        );
        let q = v.asking().expect("a question");
        assert!(!q.yes, "the cursor was not born on the no");
        let head = v.top_lines(80, &t);
        let answers: Vec<&TopLine> = head.iter().filter(|l| l.target.is_some()).collect();
        assert_eq!(answers.len(), 2);
        assert_eq!(
            answers[0].target,
            Some(Target::Answer(false)),
            "the no is not first"
        );
        assert_eq!(answers[1].target, Some(Target::Answer(true)));
        assert_eq!(
            answers[0].color,
            tokens::color::TEXT_HERE_ACTS,
            "the cursor is a colour"
        );
        let said: String = head
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let Cost::Battery(Some(charge)) = q.cost else {
            panic!("not the battery's question: {q:?}");
        };
        let percent = charge.to_string();
        assert!(said.contains(&percent), "the charge is not said: {said}");
        assert!(!said.contains('['), "a key with no text: {said}");

        assert_eq!(v.key(Press::Down), Errand::Moved);
        assert!(v.asking().expect("asked").yes);
        assert_eq!(v.key(Press::Up), Errand::Moved);
        assert_eq!(
            v.key(Press::Enter),
            Errand::NotStarted,
            "the no started something"
        );
        assert!(v.asking().is_none());

        v.key(Press::Start);
        assert_eq!(v.key(Press::Escape), Errand::NotStarted, "`Esc` is the no");
        v.key(Press::Start);
        v.key(Press::Down);
        assert_eq!(v.key(Press::Enter), Errand::StartHere { on_battery: true });

        // THE POINTER: the first click on an answer puts the cursor there,
        // the second answers — invariant 19.
        v.key(Press::Start);
        assert_eq!(v.clicked(Target::Answer(true)), Errand::Moved);
        assert_eq!(
            v.clicked(Target::Answer(true)),
            Errand::StartHere { on_battery: true }
        );

        let mut plugged = a_view();
        assert_eq!(
            plugged.key(Press::Start),
            Errand::Start,
            "on mains it asked"
        );
        let mut above = a_view().on_charge(module::Charge {
            discharging: true,
            percent: module::battery_floor(),
        });
        assert_eq!(
            above.key(Press::Start),
            Errand::Start,
            "at the floor it asked"
        );
    }

    /// THE QUESTION IS ONE LIST IN THE FOOTER TOO: its answers are rows a
    /// click takes, and the page follows the answer under the cursor.
    #[test]
    fn the_footers_region_asks_the_same_question() {
        let t = texts();
        let mut v = a_view().on_charge(flat());
        v.key(Press::Start);
        let rows = v.region_rows(0, usize::MAX, 80, &t);
        let at = v.region_cursor(80, &t);
        assert_eq!(rows[at].target, Some(Target::Answer(false)));
        v.key(Press::Down);
        let at = v.region_cursor(80, &t);
        assert_eq!(rows[at].target, Some(Target::Answer(true)));
        assert_eq!(rows.len(), v.region_len(80, &t));
    }

    /// Records that say everything a row can say.
    fn every_record() -> module::Records {
        module::Records {
            conflicts: 2,
            held: vec![module::Held {
                id: "nix:hello".to_owned(),
                why: module::HeldWhy::BuildFailed,
            }],
            trial: Some(module::TrialOutcome::Failed),
            no_trial: Some(module::NoTrial::NotInMenu),
            hold: Some(module::HeldAt {
                rev: "613862c5a0ff".to_owned(),
                since: "2026-10-05T12:00:00Z".to_owned(),
            }),
        }
    }

    /// WHAT THE MACHINE'S RECORDS SAY is on top of both doors, each only
    /// when it is there: the conflicts with the keys that open them (and a
    /// click that does), what the verification held and why, the boot
    /// trial in the machine's words, why there is none, and a hold with its
    /// short revision and its day.
    #[test]
    fn the_records_are_said_above_the_steps_in_both_doors() {
        let t = texts();
        let v = a_view().with_records(every_record(), "SUPER s v".to_owned());
        let head = v.top_lines(200, &t);
        let said: String = head
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        for needle in [
            "SUPER s v",
            "nix:hello",
            &t.t(module::key::SYSTEM_UPDATE_HELD_BUILD_FAILED),
            &t.t(key::MACHINE_TRIAL_FAILED),
            &t.t(key::MACHINE_NO_TRIAL_NOT_IN_MENU),
            "613862c ",
            "2026-10-05",
        ] {
            assert!(said.contains(needle), "`{needle}` is not said: {said}");
        }
        assert!(!said.contains("613862c5"), "the revision was not shortened");
        assert!(!said.contains("T12:00"), "the stamp was not made a day");
        assert!(!said.contains('['), "a key with no text: {said}");
        assert!(
            head.iter().any(|l| l.target == Some(Target::Belongings)),
            "the conflicts' line takes nothing"
        );
        // THE SAME LINES, IN THE FOOTER'S REGION, above its steps.
        let rows = v.region_rows(0, usize::MAX, 200, &t);
        let first_step = rows
            .iter()
            .position(|r| matches!(r.target, Some(Target::Step(_))))
            .expect("steps");
        let region: String = rows[..first_step]
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            region.contains("nix:hello") && region.contains("SUPER s v"),
            "{region}"
        );
        let mut v = v;
        assert_eq!(v.clicked(Target::Belongings), Errand::OpenBelongings);
        // AND NOTHING THERE IS NOTHING SAID.
        let quiet = a_view();
        assert_eq!(quiet.top_lines(200, &t), {
            let mut only_register = Vec::new();
            for said in quiet.register_lines(200, &t) {
                only_register.push(TopLine {
                    text: said,
                    color: tokens::color::TEXT_SECONDARY,
                    target: None,
                });
            }
            only_register
        });
    }

    /// A PHOTOGRAPH'S WINDOW STARTS NOTHING ON BATTERY EITHER, and opens no
    /// belongings: both are kept, and said nowhere as if they had gone out.
    #[test]
    fn a_photographs_yes_and_its_belongings_reach_nothing() {
        let mut w = photographed(&[], "");
        w.open.view = w
            .open
            .view
            .clone()
            .on_charge(flat())
            .with_records(every_record(), String::new());
        let mut ui = Ui::for_tests(texts());
        typed(&mut w, &mut ui, "u");
        typed(&mut w, &mut ui, "<down>");
        typed(&mut w, &mut ui, "RET");
        let errand = w.open.view.clicked(Target::Belongings);
        w.apply(&mut ui, errand);
        let kept = w.would_have();
        assert_eq!(kept.len(), 2, "{kept:?}");
        assert!(kept[0].contains("on battery"), "{kept:?}");
        assert!(
            kept[1].contains(crate::belongings::WINDOW_COMMAND),
            "{kept:?}"
        );
        assert!(ui.orders().is_empty(), "the footer was told");
    }

    /// A REFUSAL OF THE MACHINE'S PROGRAM SAYS WHY, in the machine's words:
    /// every reason it can give has its sentence, the cursor's register
    /// carries it, and a word that is not the machine's says nothing more.
    #[test]
    fn a_refusal_says_the_machines_reason() {
        let t = texts();
        for why in module::MachineRefusal::ALL {
            let k = refusal_key(why.id()).expect("every reason of the machine has a sentence");
            assert!(!t.t(k).starts_with('['), "{k}");
        }
        assert_eq!(refusal_key("a-word-the-machine-never-says"), None);
        let step = &module::steps()[0];
        let said = cursor_explains(step, &Verdict::Refused("build-failed".to_owned()), &t);
        assert!(
            said.contains(&t.t(key::MACHINE_REFUSED_BUILD_FAILED)),
            "{said}"
        );
        let plain = cursor_explains(step, &Verdict::Refused("nonsense".to_owned()), &t);
        assert_eq!(plain, cursor_explains(step, &Verdict::Failed, &t));
    }

    /// WHAT STARTING IT HERE SAID is its own sentence — on battery, on this
    /// connection — in both doors, and so are the two noes.
    #[test]
    fn starting_it_here_says_so() {
        let t = texts();
        for on_battery in [true, false] {
            assert_ne!(
                t.t(said_anyway(Started::Systemd, on_battery)),
                t.t(said_of(Started::Systemd))
            );
            assert_eq!(
                said_anyway(Started::Nowhere, on_battery),
                said_of(Started::Nowhere)
            );
        }
        assert_ne!(
            said_anyway(Started::Systemd, true),
            said_anyway(Started::Systemd, false)
        );
        for k in [
            said_anyway(Started::Systemd, true),
            said_anyway(Started::Systemd, false),
            NOT_STARTED,
            KEPT_WAITING,
        ] {
            assert!(!t.t(k).starts_with('['), "{k}");
        }
    }

    /// The update's row in a mind: queued, held as given, or running.
    fn a_mind_with(
        running: bool,
        queued: bool,
        hold: Option<ronoctua_brain::Hold>,
        now: i64,
    ) -> ronoctua_brain::Mind {
        let mut mind = a_mind(
            if running { module::ID } else { "" },
            if queued { &[module::ID] } else { &[] },
        );
        mind.now = now;
        mind.tasks.push(ronoctua_brain::TaskState {
            id: module::ID.to_owned(),
            kind: ronoctua_brain::task::Kind::Routine,
            standing: ronoctua_brain::Standing::Queued,
            enabled: true,
            interval_days: 7,
            part_of_day: ronoctua_brain::PartOfDay::Afternoon,
            next: 0,
            hold,
            why_not: String::new(),
            last: None,
            history: Vec::new(),
            steps: Vec::new(),
            calendar: true,
            name_key: String::new(),
            does_key: String::new(),
            why_key: String::new(),
            screen_generation: 0,
            actions: Vec::new(),
        });
        mind
    }

    /// A RUN ASKED FOR BY HAND IS FOLLOWED IN THE BRAIN'S MIND, and the
    /// brain's hold for a metered way out — how it waits on a scheduled run
    /// — is what turns it into the question. A mind from before the ask
    /// says nothing; a run held by something else waits; running, or gone
    /// from the queue once seen, is over; and one never seen is let go.
    ///
    /// THE DEFECT THIS CATCHES: `u` on a phone's hotspot said "queued — it
    /// starts when the machine is free" and the brain held it for as long as
    /// the hotspot lasted, the person never asked.
    #[test]
    fn a_run_asked_by_hand_and_held_for_the_network_becomes_the_question() {
        use ronoctua_brain::Hold;
        let mut f = Followed::since(1000);
        assert_eq!(
            f.heard(&a_mind_with(false, false, None, 1000)),
            Heard::Waiting,
            "a mind from before the ask"
        );
        assert_eq!(
            f.heard(&a_mind_with(false, true, Some(Hold::Ram), 1001)),
            Heard::Waiting,
            "held by the gate"
        );
        assert_eq!(
            f.heard(&a_mind_with(false, true, Some(module::HOLD_METERED), 1002)),
            Heard::Metered
        );
        assert_eq!(
            f.heard(&a_mind_with(false, false, None, 1003)),
            Heard::Over,
            "gone from the queue once seen"
        );
        let mut f = Followed::since(1000);
        assert_eq!(
            f.heard(&a_mind_with(true, false, None, 1001)),
            Heard::Over,
            "running"
        );
        let mut f = Followed::since(1000);
        assert_eq!(
            f.heard(&a_mind_with(false, false, None, 1000 + FOLLOWED_FOR_S + 1)),
            Heard::Over,
            "a run never seen is followed for ever"
        );
        // A SCHEDULED RUN ALREADY HELD that way, when `u` is pressed: asked
        // at once, from the mind the door already has.
        let mut f = Followed::since(1000);
        assert_eq!(
            f.heard(&a_mind_with(false, true, Some(module::HOLD_METERED), 900)),
            Heard::Metered
        );
    }

    /// THE METERED QUESTION, the no first: the no leaves the run in the
    /// queue, the yes starts it here on mains — and on a battery below the
    /// floor asks about the battery next, whose no also leaves it queued.
    #[test]
    fn the_metered_question_starts_it_here_or_leaves_it_queued() {
        let t = texts();
        let mut v = a_view();
        v.ask_metered();
        let q = v.asking().expect("a question");
        assert_eq!((q.cost, q.queued, q.yes), (Cost::Metered, true, false));
        let head = v.top_lines(80, &t);
        let answers: Vec<&TopLine> = head.iter().filter(|l| l.target.is_some()).collect();
        assert_eq!(answers.len(), 2);
        assert_eq!(
            answers[0].target,
            Some(Target::Answer(false)),
            "the no is not first"
        );
        assert_eq!(
            answers[0].text,
            t.t(module::key::SYSTEM_UPDATE_ANSWER_NO_METERED)
        );
        let said: String = head
            .iter()
            .map(|l| l.text.clone())
            .collect::<Vec<_>>()
            .join(" ");
        let expected: Vec<String> = [
            t.t(module::key::SYSTEM_UPDATE_ASK_METERED),
            t.t(module::key::SYSTEM_UPDATE_ANSWER_NO_METERED),
            t.t(module::key::SYSTEM_UPDATE_ANSWER_YES_METERED),
        ]
        .join(" ")
        .split_whitespace()
        .map(str::to_owned)
        .collect();
        let got: Vec<String> = said.split_whitespace().map(str::to_owned).collect();
        assert_eq!(got, expected, "the question is not the metered one, whole");
        assert_eq!(
            v.key(Press::Escape),
            Errand::KeptWaiting,
            "the no stays in the queue"
        );
        v.ask_metered();
        assert_eq!(v.clicked(Target::Answer(true)), Errand::Moved);
        assert_eq!(
            v.clicked(Target::Answer(true)),
            Errand::StartHere { on_battery: false }
        );

        let mut flat_one = a_view().on_charge(flat());
        flat_one.ask_metered();
        flat_one.key(Press::Down);
        assert_eq!(
            flat_one.key(Press::Enter),
            Errand::Moved,
            "the battery was not asked next"
        );
        let q = flat_one.asking().expect("the battery's question");
        assert!(
            matches!(q.cost, Cost::Battery(Some(_))) && q.queued && !q.yes,
            "{q:?}"
        );
        assert_eq!(
            flat_one.key(Press::Enter),
            Errand::KeptWaiting,
            "its no left the queue"
        );
        flat_one.ask_metered();
        flat_one.key(Press::Down);
        flat_one.key(Press::Enter);
        flat_one.key(Press::Down);
        assert_eq!(
            flat_one.key(Press::Enter),
            Errand::StartHere { on_battery: true }
        );
    }

    /// A PHOTOGRAPH'S WINDOW WHOSE BRAIN HOLDS THE RUN FOR THE NETWORK asks,
    /// and what the answers would have done is kept, never done.
    #[test]
    fn a_photographs_metered_question_reaches_nothing() {
        let mut w = photographed(&[], "");
        w.open.held_metered = true;
        let mut ui = Ui::for_tests(texts());
        typed(&mut w, &mut ui, "u");
        assert_eq!(w.open.view.asking().map(|q| q.cost), Some(Cost::Metered));
        typed(&mut w, &mut ui, "RET");
        assert_eq!(
            ui.message.as_deref(),
            Some(ui.texts.t(KEPT_WAITING).as_str())
        );
        typed(&mut w, &mut ui, "u");
        typed(&mut w, &mut ui, "<down>");
        typed(&mut w, &mut ui, "RET");
        let kept = w.would_have();
        assert_eq!(kept.len(), 3, "{kept:?}");
        assert!(kept[2].contains("on this connection"), "{kept:?}");
    }

    /// FAILS IF: under the binary cache page's grave question a row of the
    /// page is still what the window's pointer takes — lit, while the click
    /// there is the question's — or, with no question asked, no row of the
    /// page is. The band and the click read one answer (`target_at`).
    #[test]
    fn the_windows_cache_rows_take_no_click_under_the_question() {
        use crate::binary_cache::{Readings, Row as PageRow, TokenState, rows_of};
        for grave in [false, true] {
            let readings = Readings {
                named: ronoctua_cachix::settings::Named::Cache(
                    ronoctua_person::options::BinaryCache {
                        name: "someone".to_owned(),
                        public_key:
                            "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ="
                                .to_owned(),
                        visibility: ronoctua_person::options::Visibility::Public,
                    },
                ),
                token: TokenState::Kept,
                ..Readings::default()
            };
            let (mut w, mut ui) = a_window();
            ui.lines = 60;
            w.open.view = w.open.view.clone().with_cache(readings);
            if grave && let Some(c) = w.open.view.cache.as_mut() {
                let remove = rows_of(&c.readings)
                    .iter()
                    .position(|r| *r == PageRow::Remove)
                    .expect("a cache is removable");
                let _ = c.page.clicked(remove, &c.readings);
                let _ = c.page.clicked(remove, &c.readings);
                assert!(c.page.confirm().is_some(), "the removal asked nothing");
            }
            let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
            let taken = (0..ui.lines)
                .filter(|line| {
                    matches!(
                        w.target_at(&ui, 40.0, (f64::from(*line) + 0.5) * line_px),
                        Some(Target::Page(_))
                    )
                })
                .count();
            if grave {
                assert_eq!(
                    taken, 0,
                    "a row of the page takes the click under the question"
                );
            } else {
                assert!(taken >= 3, "{taken} rows of the page take a click");
            }
        }
    }

    /// The follow page over two AppImages, the person following, GitHub
    /// let, the machine built following them.
    fn on_the_follow_page() -> crate::follow::Readings {
        crate::follow::Readings {
            copy: crate::follow::Choice::On,
            let_in: true,
            machine: Some(true),
            apps: ["CADCraft", "FigCraft"]
                .into_iter()
                .map(|name| crate::follow::App {
                    name: name.to_owned(),
                    here: true,
                    pin: ronoctua_person::reports::PinFrom::Followed,
                    version: "0.9.0".to_owned(),
                    followed: Some("0.9.0".to_owned()),
                    held: None,
                    waiting: false,
                    refused: None,
                })
                .collect(),
            taken: crate::follow::Taken::Yes,
        }
    }

    /// FAILS IF: `f` does not turn the view to the page of the apps that
    /// follow their releases and back, the page does not teach its own keys,
    /// its rows are not lit and clicked where they are drawn — in the
    /// footer's list and in the window alike — or the second click on the
    /// switch does not turn it.
    #[test]
    fn the_follow_page_is_turned_to_and_clicked_where_it_is_drawn() {
        let texts = texts();
        let mut view = a_view();
        assert_eq!(view.key(Press::Follow), Errand::Moved);
        assert!(view.follow_page().is_some() && view.page_open());
        assert_eq!(view.taught(), OF_THE_FOLLOW);
        assert_eq!(view.key(Press::Follow), Errand::Moved);
        assert!(!view.page_open(), "`f` turns it back");
        let mut view = a_view().with_follow(on_the_follow_page());
        // THE FOOTER'S LIST: the head, the heading and a row a line, each
        // row taking the click.
        let rows = view.region_rows(0, 100, 100, &texts);
        let taking: Vec<Target> = rows.iter().filter_map(|r| r.target).collect();
        assert_eq!(
            taking,
            (0..5).map(Target::Page).collect::<Vec<_>>(),
            "the switch, the hosts, the machine and two apps"
        );
        let words: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(
            words.contains("CADCraft") && words.contains("api.github.com"),
            "{words}"
        );
        assert_eq!(
            view.clicked(Target::Page(0)),
            Errand::Follow(crate::follow::Errand::Turn(false))
        );
        // THE WINDOW: a row is taken where it is drawn.
        let (mut w, mut ui) = a_window();
        ui.lines = 40;
        w.open.view = w.open.view.clone().with_follow(on_the_follow_page());
        w.measure(&ui);
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let taken: Vec<Target> = (0..ui.lines)
            .filter_map(|line| w.target_at(&ui, 40.0, (f64::from(line) + 0.5) * line_px))
            .collect();
        assert_eq!(taken, (0..5).map(Target::Page).collect::<Vec<_>>());
        typed(&mut w, &mut ui, "f");
        assert!(
            !w.open.view.page_open(),
            "the page's own `f` goes back to the steps"
        );
    }

    /// The parts' page over two parts: one here, one that needs the person
    /// for a download that is one file, and no stick mounted.
    fn on_the_parts_page() -> crate::parts::Readings {
        let record = |id: &str, phase: &str, attempts: u32, missing: &str| {
            let json = format!(
                r#"{{"format":1,"id":"{id}","wanted":{{"drv":"/nix/store/0123456789abcdfghijklmnpqrsvwxyz-{id}.drv","out":"/nix/store/0123456789abcdfghijklmnpqrsvwxyz-{id}","interface":1}},"current":null,"previous":null,"phase":"{phase}","waiting-for":null,"attempts":{attempts},"last-failure":{failure},"missing":[{missing}]}}"#,
                failure = if attempts > 0 {
                    r#"{"why":"build-failed","at":"2026-10-24T01:04:32Z","detail":"curl: (22) 404"}"#
                } else {
                    "null"
                },
            );
            ronoctua_person::parse::<ronoctua_parts::record::Record>(json.as_bytes())
                .expect("a record")
        };
        let missing = r#"{"drv":"/nix/store/0123456789abcdfghijklmnpqrsvwxyz-yt-dl.drv","out":"/nix/store/0123456789abcdfghijklmnpqrsvwxyz-yt-dl","name":"yt-dlp.tar.gz","hash":"sha256-AAAA","mode":"flat","urls":["https://github.com/yt-dlp/yt-dlp.tar.gz"]}"#;
        crate::parts::Readings::of(
            vec![
                record("pot", "here", 0, ""),
                record("yt", "needs-you", 5, missing),
            ],
            &std::collections::BTreeMap::from([("yt".to_owned(), "yt-dlp".to_owned())]),
            None,
        )
    }

    /// FAILS IF: `d` does not turn the view to the parts' page and back, the
    /// page does not teach its own keys, its rows are not lit and clicked
    /// where they are drawn — in the footer's list and in the window alike —
    /// the second click on a part does not try it, or `h` on it does not ask
    /// for the file by the download's name.
    #[test]
    fn the_parts_page_is_turned_to_and_clicked_where_it_is_drawn() {
        let texts = texts();
        let mut view = a_view();
        assert_eq!(view.key(Press::Parts), Errand::Moved);
        assert!(view.parts_page().is_some() && view.page_open());
        assert_eq!(view.taught(), OF_THE_PARTS);
        assert_eq!(view.key(Press::Parts), Errand::Moved);
        assert!(!view.page_open(), "`d` turns it back");
        assert_eq!(
            view.key(Press::HandIn),
            Errand::Nothing,
            "`h` is the page's"
        );
        let mut view = a_view().with_parts(on_the_parts_page());
        // THE FOOTER'S LIST: the head, the heading and a row a line, each
        // row taking the click.
        let rows = view.region_rows(0, 100, 100, &texts);
        let taking: Vec<Target> = rows.iter().filter_map(|r| r.target).collect();
        assert_eq!(
            taking,
            (0..5).map(Target::Page).collect::<Vec<_>>(),
            "two parts, the stick's two rows and the mirrors'"
        );
        let words: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(
            words.contains("yt-dlp") && words.contains("needs you"),
            "{words}"
        );
        assert_eq!(view.clicked(Target::Page(1)), Errand::Moved);
        assert_eq!(
            view.clicked(Target::Page(1)),
            Errand::Parts(crate::parts::Errand::Run(crate::parts::Act::Retry {
                id: "yt".into(),
                name: "yt-dlp".into(),
            }))
        );
        let top: String = view
            .top_lines(100, &texts)
            .into_iter()
            .map(|l| l.text)
            .collect();
        assert!(
            top.contains("yt-dlp.tar.gz") && top.contains("github.com"),
            "{top}"
        );
        // `h` BRINGS UP THE FILE PICKER over the page: its rows are drawn
        // in place of the page's, it teaches its own keys, the top says what
        // to choose, and `Esc` puts it down and brings the page back.
        assert_eq!(view.key(Press::HandIn), Errand::Moved);
        assert!(view.picking());
        assert!(
            view.page_cursor().is_some(),
            "a door paints the picker before it drew it once"
        );
        let top: String = view
            .top_lines(100, &texts)
            .into_iter()
            .map(|l| l.text)
            .collect();
        assert!(top.contains("yt-dlp.tar.gz"), "{top}");
        let picker = view.region_rows(0, 100, 100, &texts);
        let words: String = picker
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        assert!(
            words.contains(&texts.t(crate::key::DISTRO_PARTS_PICK_HAND_IN)),
            "{words}"
        );
        assert!(
            picker
                .iter()
                .any(|r| matches!(r.target, Some(Target::Page(_)))),
            "the picker's rows take the click"
        );
        assert_ne!(
            view.hint(&texts),
            crate::chrome::hint_of(OF_THE_PARTS, &texts)
        );
        assert_eq!(view.key(Press::Escape), Errand::Moved);
        assert!(
            !view.picking() && view.parts_page().is_some(),
            "`Esc` puts the picker down"
        );
        // THE WINDOW: a row is taken where it is drawn.
        let (mut w, mut ui) = a_window();
        ui.lines = 40;
        w.open.view = w.open.view.clone().with_parts(on_the_parts_page());
        w.measure(&ui);
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let taken: Vec<Target> = (0..ui.lines)
            .filter_map(|line| w.target_at(&ui, 40.0, (f64::from(line) + 0.5) * line_px))
            .collect();
        assert_eq!(taken, (0..5).map(Target::Page).collect::<Vec<_>>());
        typed(&mut w, &mut ui, "d");
        assert!(
            !w.open.view.page_open(),
            "the page's own `d` goes back to the steps"
        );
    }
}
