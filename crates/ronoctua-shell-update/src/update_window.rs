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
//! 3. **The view**, [`View`]: on top, the status — whether an update is
//!    there, asked of GitHub on a thread ([`status_of`]) — and the actions
//!    one does here, each a row; then what the focused line is, the steps
//!    with their verdicts and the two journals live. Both doors show it —
//!    the footer's region (`LEADER s u u`) as one list, the window
//!    (`LEADER s u w`) in two halves — with the same keys, every one a click:
//!    the arrows over the head's lines and the steps, `TAB` between them,
//!    the output, `u` to start the update now, and `a` and `p` to answer
//!    this update's countdown or stop its run — each taught only while it
//!    means something ([`View::taught`]).
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

/// When `path` last moved, if it is there.
fn modified(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
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
    /// for it, when the word is the machine's ([`module::machine_refusal_key`]).
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
pub(crate) fn record_places() -> module::RecordPlaces {
    let mut at = module::RecordPlaces::here();
    if let Some(room) = crate::photo::room() {
        at.machine_dir = room.join(crate::photo::MACHINE_DIR);
        at.machine_state = room.join(crate::photo::MACHINE_STATE);
        at.root = room;
    }
    at
}

/// The toplevel of the system that waits for the next restart under `root`
/// (`/`, or the camera's room), when one does — the machine's own answer,
/// read off the system profile and the system that runs through the
/// module's shared half: what the update's other pages ask before they say
/// "from the next update".
pub(crate) fn waiting_system(root: &std::path::Path) -> Option<String> {
    module::next_boot(root, None).map(|n| n.toplevel)
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
    // A REFUSAL SAYS WHY, in the words of the program that refused — the
    // machine's, or root's applier's of the person's declarations, as the
    // step declares (`refuses`) — and a word that is not that program's
    // says nothing more than the row's *failed*.
    if let Verdict::Refused(word) = verdict
        && let Some(why) = module::refused_key(step.id, word)
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

/// The keys the update teaches while nothing of it is under way, in both
/// doors — every one a click.
///
/// The arrows walk ONE list: the head's lines that act (the status, the
/// actions, the records' rows), then the steps; `TAB` jumps between the two,
/// each keeping its place. The output a page at a time on `C-j` and `C-k` and
/// back to its end on `G`; `u` starts the update now — `LEADER s u n`'s
/// command — and the three pages on their letters.
///
/// A KEY IS TAUGHT ONLY WHILE IT MEANS SOMETHING (the owner, 2026-11-08: `u`
/// and the countdown's "start now" side by side read as two words for one
/// thing). So each state of the update has its table — this one, the
/// queue's ([`OF_THE_QUEUE`]), the countdown's ([`OF_THE_COUNTDOWN`]) and
/// the run's ([`OF_THE_RUN`]) — and [`View::taught`] picks the one that is
/// true now; both doors read a key by all of them ([`press_of_key`]).
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", "system-update-next-step"),
    ("<up>", "system-update-previous-step"),
    ("TAB", "system-update-section"),
    ("S-TAB", "system-update-section"),
    ("C-j", "system-update-output-down"),
    ("C-k", "system-update-output-up"),
    ("G", "system-update-output-end"),
    ("u", START),
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

/// The keys while the update waits in the brain's queue, not yet announced:
/// [`OF_THE_VIEW`]'s without `u` — asking again for a run that is asked for
/// already means nothing, and its row says why.
pub const OF_THE_QUEUE: &[(&str, &str)] = &[
    ("<down>", "system-update-next-step"),
    ("<up>", "system-update-previous-step"),
    ("TAB", "system-update-section"),
    ("S-TAB", "system-update-section"),
    ("C-j", "system-update-output-down"),
    ("C-k", "system-update-output-up"),
    ("G", "system-update-output-end"),
    ("c", "binary-cache-open"),
    ("f", "distro-follow-open"),
    ("d", "distro-parts-open"),
];

/// The keys while the brain counts down to THIS update: `p` skips the
/// countdown, `a` puts the run off to its next moment — the band's two
/// buttons, by the brain's own commands (`brain-proceed`, `brain-abort`), so
/// that with this window in front `LEADER Alt` and the key answer the band,
/// and the band draws the key beside each (the footer's `brain::Words`).
pub const OF_THE_COUNTDOWN: &[(&str, &str)] = &[
    ("<down>", "system-update-next-step"),
    ("<up>", "system-update-previous-step"),
    ("TAB", "system-update-section"),
    ("S-TAB", "system-update-section"),
    ("C-j", "system-update-output-down"),
    ("C-k", "system-update-output-up"),
    ("G", "system-update-output-end"),
    ("p", PROCEED),
    ("a", ABORT),
    ("c", "binary-cache-open"),
    ("f", "distro-follow-open"),
    ("d", "distro-parts-open"),
];

/// The keys while THIS update runs: `a` stops it.
pub const OF_THE_RUN: &[(&str, &str)] = &[
    ("<down>", "system-update-next-step"),
    ("<up>", "system-update-previous-step"),
    ("TAB", "system-update-section"),
    ("S-TAB", "system-update-section"),
    ("C-j", "system-update-output-down"),
    ("C-k", "system-update-output-up"),
    ("G", "system-update-output-end"),
    ("a", "system-update-stop"),
    ("c", "binary-cache-open"),
    ("f", "distro-follow-open"),
    ("d", "distro-parts-open"),
];

/// Every state's table: what both doors read a key by ([`press_of_key`]),
/// and what the window keeps from the keyboard whatever it teaches now
/// (`UpdateWindow::barred_keys`) — a physical `u` never starts an update.
pub const OF_EVERY_STATE: &[&[(&str, &str)]] =
    &[OF_THE_VIEW, OF_THE_QUEUE, OF_THE_COUNTDOWN, OF_THE_RUN];

/// THE KEYS AFTER `LEADER Alt` that the rule for every window does not give
/// (`chrome::leader_alt_key`): the end of the output is `<end>`, where its own
/// key is vi's capital.
pub const LEADER_ALT_KEYS: &[(&str, &str)] = &[("G", "<end>")];

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
/// The countdown's "start now": the brain's countdown skipped — the band's
/// own command, which this view teaches only while THIS update counts down.
pub const PROCEED: &str = "brain-proceed";
/// The countdown's "not now": the run counting down put off to its next
/// moment — the band's own command, likewise.
pub const ABORT: &str = "brain-abort";
/// THIS update, running, stopped.
pub const STOP: &str = "system-update-stop";
/// `TAB`: between the head's lines that act and the steps.
const SECTION: &str = "system-update-section";

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
    // THE MACHINE'S OWN GESTURES (`crate::machine_gesture`), which the
    // records' rows ask: by name too, so the keyboard reaches them — each
    // opens the view on its question, the no first.
    ronoctua_cmd::Command {
        id: crate::machine_gesture::TRY_AGAIN,
        label: key::CMD_MACHINE_TRY_AGAIN,
    },
    ronoctua_cmd::Command {
        id: crate::machine_gesture::HOLD,
        label: key::CMD_MACHINE_HOLD,
    },
    ronoctua_cmd::Command {
        id: crate::machine_gesture::LET_GO,
        label: key::CMD_MACHINE_LET_GO,
    },
    // THE PERSON'S BRAIN MODULE THAT WAITS (`crate::approval`), approved by
    // name as its row approves it: the view on its question, the no first.
    ronoctua_cmd::Command {
        id: crate::approval::APPROVE,
        label: key::CMD_MACHINE_APPROVE_MODULE,
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
        // THE MACHINE'S OWN GESTURES (`crate::machine_gesture`): the records'
        // rows ask them, and so does their name — a key of each would ask
        // about a hold or a failed trial the page may not have.
        crate::machine_gesture::TRY_AGAIN,
        crate::machine_gesture::HOLD,
        crate::machine_gesture::LET_GO,
        // A BRAIN MODULE OF THE PERSON'S THAT WAITS (`crate::approval`): the
        // records' rows ask, and so does the name — a key would ask about a
        // module the page may not have.
        crate::approval::APPROVE,
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
    /// A gesture of the machine's (`crate::machine_gesture`): its question.
    Machine(crate::machine_gesture::Gesture),
    /// A brain module of the person's that waits (`crate::approval`): its
    /// question — the one the argument names, or the first.
    Approve,
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
        crate::approval::APPROVE => Some(Order::Approve),
        other => crate::machine_gesture::Gesture::of_command(other).map(Order::Machine),
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
    /// `TAB`: between the head's lines that act and the steps, each keeping
    /// its place.
    Section,
    /// `RET`: answers the question on battery with the answer under the
    /// cursor; on a head line, does what its second click does. On a step
    /// there is nothing more to do than the cursor did.
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
/// By its whole notation through EVERY state's table ([`OF_EVERY_STATE`]):
/// what a state does not teach is still read, and the view answers it as
/// that state answers it — the countdown's answers with nothing of this
/// update counting down say so. And three that are not
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
    let id = OF_EVERY_STATE
        .iter()
        .flat_map(|t| t.iter())
        .chain(OF_THE_PARTS)
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    Some(match id {
        "system-update-next-step" => Press::Down,
        "system-update-previous-step" => Press::Up,
        "system-update-output-down" => Press::OutputDown,
        "system-update-output-up" => Press::OutputUp,
        "system-update-output-end" => Press::OutputEnd,
        SECTION => Press::Section,
        START => Press::Start,
        ABORT | STOP => Press::Abort,
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
    /// A gesture of the machine's, said yes to on the page: asked again in
    /// the sudo bar through the door's gate, and what came of it said in the
    /// door (`crate::machine_gesture`).
    Machine(crate::machine_gesture::Gesture),
    /// A brain module of the person's said yes to on the page: its request
    /// staged and asked again in the sudo bar through the door's gate, and
    /// what came of it said in the door (`crate::approval`).
    Approve(crate::approval::Waiting),
    /// A sentence of this crate's, said in the door.
    Say(&'static str),
    /// Ask GitHub again where the branches the machine's flake follows are
    /// (`ronoctua_software::heads::look_again`): the status line's second
    /// click, and its `RET`.
    Recheck,
    /// The view closes.
    Close,
}

/// What a line of the view takes when it is clicked — the one answer the
/// painting, the pointer and the footer's click read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The status line, on top, while it says GitHub's answer: the first
    /// click puts the focus there, the second asks GitHub again
    /// ([`Errand::Recheck`]).
    Status,
    /// One of the things one does here, a row under the status: the first
    /// click puts the focus there, the second does it (invariant 19).
    Action(Action),
    /// A step: the cursor goes there.
    Step(usize),
    /// An answer to the question — `true` is yes. The first click puts the
    /// cursor on it, the second answers (invariant 19).
    Answer(bool),
    /// The belongings, whose conflicts a line counts: the first click puts
    /// the focus there, the second opens them, as `RET` and the key the line
    /// teaches do.
    Belongings,
    /// A row of the page the view is turned to (the binary cache's, or the
    /// apps' that follow their releases), by its index: the first click puts
    /// the cursor there, the second does what `RET` does.
    Page(usize),
    /// An answer of the binary cache page's grave question — `true` is yes.
    PageAnswer(bool),
    /// A gesture of the machine's the records offer: a click asks it there,
    /// under the hand — the no first (`crate::machine_gesture`) —, and takes
    /// the focus.
    Gesture(crate::machine_gesture::Gesture),
    /// An answer to a gesture's question — `true` is yes: a click picks it,
    /// and a click on the pick answers.
    GestureAnswer(bool),
    /// A brain module of the person's that waits, by its place among them:
    /// a click asks to approve it there, the no first (`crate::approval`),
    /// and takes the focus.
    Approve(usize),
    /// An answer to an approval's question — `true` is yes: a click picks
    /// it, and a click on the pick answers.
    ApproveAnswer(bool),
}

impl Target {
    /// Whether what it does answers a run — skips its countdown, puts it
    /// off, stops it: done only where the person put the focus.
    #[must_use]
    pub const fn changes_the_run(self) -> bool {
        matches!(
            self,
            Self::Action(Action::StartNow | Action::NotNow | Action::Stop)
        )
    }
}

/// One thing one does here — a row under the status line, labelled with the
/// same short word its key teaches (`texts.t(id)`), so the keyboard and the
/// mouse reach the same deed by the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The whole update, now ([`START`]).
    Start,
    /// Skip the brain's countdown of this update ([`PROCEED`]).
    StartNow,
    /// Put this update, counting down, off to its next moment ([`ABORT`]).
    NotNow,
    /// Stop this update, running ([`STOP`]).
    Stop,
    /// The binary cache's page (`crate::binary_cache`).
    Cache,
    /// The page of the apps that follow their releases (`crate::follow`).
    Follow,
    /// The parts' page (`crate::parts`).
    Parts,
}

impl Action {
    /// Its id: the command, or the view's key, whose short word labels it.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Start => START,
            Self::StartNow => PROCEED,
            Self::NotNow => ABORT,
            Self::Stop => STOP,
            Self::Cache => crate::binary_cache::OPEN,
            Self::Follow => crate::follow::OPEN,
            Self::Parts => crate::parts::OPEN,
        }
    }

    /// What the register says it does, under the actions, while the focus
    /// is on it.
    const fn does(self) -> &'static str {
        match self {
            Self::Start => module::key::SYSTEM_UPDATE_START_DOES,
            Self::StartNow => module::key::SYSTEM_UPDATE_PROCEED_DOES,
            Self::NotNow => module::key::SYSTEM_UPDATE_ABORT_DOES,
            Self::Stop => module::key::ABOUT_SYSTEM_UPDATE_STOP,
            Self::Cache => key::BINARY_CACHE_DOES,
            Self::Follow => key::DISTRO_FOLLOW_DOES,
            Self::Parts => key::DISTRO_PARTS_DOES,
        }
    }
}

/// Where the keys walk from: a line of the head that acts — by what it
/// takes, never by its place, for lines come and go with the state — or the
/// steps, at their own cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    /// A line of the head.
    Head(Target),
    /// The steps, at [`View::cursor`].
    Steps,
}

/// Where the update stands in the brain — which of the four tables the view
/// teaches ([`View::taught`]) and which actions it offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// Nothing of it is under way.
    Idle,
    /// It waits in the brain's queue, not yet announced.
    Queued,
    /// The brain counts down to it.
    CountingDown,
    /// It runs.
    Running,
}

/// What the brain says of THIS task — taken from each mind it publishes, so
/// both doors draw the same status and offer the same actions without
/// keeping the whole mind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Brain {
    /// It runs now.
    pub running: bool,
    /// The brain counts down to it now.
    pub announced: bool,
    /// It waits in the queue.
    pub queued: bool,
    /// What stopped it the last time its moment came, if anything.
    pub hold: Option<ronoctua_brain::Hold>,
    /// The newest run that RAN — worked or failed, a run the person
    /// stopped aside — failed.
    pub failed: bool,
    /// When the newest run that worked ended, in Unix seconds.
    pub worked: Option<i64>,
    /// Its next moment, in Unix seconds; `0` when there is none.
    pub next: i64,
}

impl Brain {
    /// What `mind` says of the update.
    #[must_use]
    pub fn of(mind: &ronoctua_brain::Mind) -> Self {
        use ronoctua_brain::task::OutcomeKind;
        let task = mind.task(module::ID);
        // THE RUNS, NEWEST FIRST: the last and the few before it. A run the
        // person stopped (`aborted`) is the person's decision, not a failure,
        // and says nothing of whether the machine updates.
        let runs: Vec<&ronoctua_brain::RunRecord> = task
            .map(|t| t.last.iter().chain(t.history.iter()).collect())
            .unwrap_or_default();
        let ran = runs.iter().find(|r| {
            r.kind == OutcomeKind::Done || (r.kind == OutcomeKind::Failed && r.why != ABORTED)
        });
        Self {
            running: mind.running == module::ID,
            announced: mind
                .announcement
                .as_ref()
                .is_some_and(|a| a.task == module::ID),
            queued: mind.queue.iter().any(|q| q == module::ID),
            hold: task.and_then(|t| t.hold),
            failed: ran.is_some_and(|r| r.kind == OutcomeKind::Failed),
            worked: runs
                .iter()
                .find(|r| r.kind == OutcomeKind::Done)
                .map(|r| r.ended),
            next: task.map_or(0, |t| t.next),
        }
    }

    /// Where the update stands.
    #[must_use]
    pub const fn run(&self) -> Run {
        if self.running {
            Run::Running
        } else if self.announced {
            Run::CountingDown
        } else if self.queued {
            Run::Queued
        } else {
            Run::Idle
        }
    }
}

/// The brain's word for a run the person stopped (`RunRecord::why`).
const ABORTED: &str = "aborted";

/// Whether a countdown's answer reaches THIS update: `proceeding` skips the
/// countdown only of this one, and stopping puts off this one counting down
/// (nothing else running) or stops this one running. The brain answers
/// whatever it announced (`brainlink::answer_of`), and a key of this view
/// must never answer another task's countdown.
#[must_use]
pub fn answers_this_update(mind: &ronoctua_brain::Mind, proceeding: bool) -> bool {
    let announced = mind
        .announcement
        .as_ref()
        .is_some_and(|a| a.task == module::ID);
    if proceeding {
        announced
    } else {
        mind.running == module::ID || (announced && mind.running.is_empty())
    }
}

/// What GitHub said of the branches the machine's flake follows, as the view
/// keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Heads {
    /// Asked on a thread; nothing came back yet.
    Asking,
    /// What came back: each branch's head, or the lock's refusal.
    Came(Result<Vec<ronoctua_software::heads::Heard>, ronoctua_software::heads::Unread>),
}

/// Which inputs a newer head was found for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// The distro's branch.
    Distro,
    /// nixpkgs'.
    Nixpkgs,
    /// Both.
    Both,
}

/// Why it could not be told whether an update is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unchecked {
    /// No network.
    Offline,
    /// GitHub's hourly allowance is spent.
    RateLimited,
    /// GitHub did not answer with a commit.
    Other,
    /// The machine has no flake of its own, so no lock.
    NoLock,
    /// Its lock does not read — the whole file, or the node of an input
    /// that names a GitHub branch: nobody was asked, and the fault is this
    /// machine's.
    Unreadable,
    /// Its lock follows no branch on GitHub.
    Nothing,
    /// The update moves this input, and it follows no branch on GitHub to
    /// ask about: only the update itself can tell.
    Unasked(ronoctua_software::heads::Input),
}

impl Unchecked {
    /// The reason, said inside the status's sentence.
    const fn key(self) -> &'static str {
        match self {
            Self::Offline => module::key::SYSTEM_UPDATE_UNCHECKED_OFFLINE,
            Self::RateLimited => module::key::SYSTEM_UPDATE_UNCHECKED_RATE_LIMITED,
            Self::Other => module::key::SYSTEM_UPDATE_UNCHECKED_OTHER,
            Self::NoLock => module::key::SYSTEM_UPDATE_UNCHECKED_NO_LOCK,
            Self::Unreadable => module::key::SYSTEM_UPDATE_UNCHECKED_UNREADABLE,
            Self::Nothing => module::key::SYSTEM_UPDATE_UNCHECKED_NOTHING,
            Self::Unasked(ronoctua_software::heads::Input::Distro) => {
                module::key::SYSTEM_UPDATE_UNCHECKED_UNASKED_DISTRO
            }
            Self::Unasked(ronoctua_software::heads::Input::Nixpkgs) => {
                module::key::SYSTEM_UPDATE_UNCHECKED_UNASKED_NIXPKGS
            }
        }
    }
}

/// What the status line says — one, by [`status_of`]'s precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The update runs.
    Running,
    /// The brain counts down to it.
    CountingDown,
    /// It waits in the brain's queue — held by this, when something holds it.
    Queued(Option<ronoctua_brain::Hold>),
    /// A new system waits for the restart.
    Restart {
        /// That restart has no way back but the boot menu — no boot trial
        /// is armed —, and no record says so with its reason (the machine's
        /// `no_trial`): the status line says it, once.
        menu_only: bool,
    },
    /// The last run failed.
    Failed,
    /// The machine is held at a revision: updates leave the inputs alone.
    Held,
    /// GitHub is being asked.
    Checking,
    /// A newer head of these branches is there.
    Available(Moved),
    /// Every branch is where the lock holds it.
    UpToDate,
    /// It could not be told, and why.
    Unchecked(Unchecked),
    /// Nobody asked yet (a test's view, a photograph's).
    Unasked,
}

/// What the status line says, by precedence: the brain's run (running,
/// counting down, queued or held), then what waits for the person (a new
/// system for the restart), what went wrong (the last run failed), what the
/// person chose (a hold) — and only then GitHub's answer. Pure.
#[must_use]
pub fn status_of(
    brain: Option<&Brain>,
    records: &module::Records,
    heads: Option<&Heads>,
) -> Status {
    if let Some(b) = brain {
        match b.run() {
            Run::Running => return Status::Running,
            Run::CountingDown => return Status::CountingDown,
            Run::Queued => return Status::Queued(b.hold),
            Run::Idle => {}
        }
    }
    if let Some(next) = &records.staged {
        return Status::Restart {
            menu_only: !next.guarded && records.no_trial.is_none(),
        };
    }
    if brain.is_some_and(|b| b.failed) {
        return Status::Failed;
    }
    if records.hold.is_some() {
        return Status::Held;
    }
    heads.map_or(Status::Unasked, online)
}

/// What GitHub's answer says: a branch that moved is an update, whatever
/// another one could not answer; every branch where the lock holds it is
/// none; and one that could not answer, with none moved, is "could not
/// check" — never "up to date" — and why: GitHub's refusal, this machine's
/// lock, or an input the update moves that no branch on GitHub says.
fn online(heads: &Heads) -> Status {
    use ronoctua_software::heads::{Input, Standing, Unlearnt, Unread};
    let heard = match heads {
        Heads::Asking => return Status::Checking,
        Heads::Came(Err(Unread::NoLock)) => return Status::Unchecked(Unchecked::NoLock),
        Heads::Came(Err(Unread::Unreadable)) => return Status::Unchecked(Unchecked::Unreadable),
        Heads::Came(Ok(heard)) => heard,
    };
    let moved = |input: Input| {
        heard.iter().any(|h| {
            h.input == input
                && h.head
                    .as_ref()
                    .is_ok_and(|head| head.standing() == Standing::Moved)
        })
    };
    match (moved(Input::Distro), moved(Input::Nixpkgs)) {
        (true, true) => return Status::Available(Moved::Both),
        (true, false) => return Status::Available(Moved::Distro),
        (false, true) => return Status::Available(Moved::Nixpkgs),
        (false, false) => {}
    }
    if heard.is_empty() {
        return Status::Unchecked(Unchecked::Nothing);
    }
    match heard
        .iter()
        .find_map(|h| h.head.as_ref().err().map(|e| (h.input, e)))
    {
        None => Status::UpToDate,
        Some((_, Unlearnt::Lock)) => Status::Unchecked(Unchecked::Unreadable),
        Some((input, Unlearnt::Unasked)) => Status::Unchecked(Unchecked::Unasked(input)),
        Some((_, Unlearnt::Asked(failure))) => Status::Unchecked(match failure.refusal {
            Some(ronoctua_software::Refusal::Offline) => Unchecked::Offline,
            Some(ronoctua_software::Refusal::RateLimited) => Unchecked::RateLimited,
            _ => Unchecked::Other,
        }),
    }
}

impl Status {
    /// Its sentence's key.
    const fn key(self) -> &'static str {
        use ronoctua_brain::Hold;
        match self {
            Self::Running => module::key::SYSTEM_UPDATE_STATUS_RUNNING,
            Self::CountingDown => module::key::SYSTEM_UPDATE_STATUS_COUNTING_DOWN,
            Self::Queued(None) => module::key::SYSTEM_UPDATE_STATUS_QUEUED,
            Self::Queued(Some(Hold::Ram)) => module::key::SYSTEM_UPDATE_STATUS_WAITING_RAM,
            Self::Queued(Some(Hold::Cpu)) => module::key::SYSTEM_UPDATE_STATUS_WAITING_CPU,
            Self::Queued(Some(Hold::Battery)) => module::key::SYSTEM_UPDATE_STATUS_WAITING_BATTERY,
            Self::Queued(Some(Hold::Busy)) => module::key::SYSTEM_UPDATE_STATUS_WAITING_BUSY,
            Self::Queued(Some(Hold::Network)) => module::key::SYSTEM_UPDATE_STATUS_WAITING_NETWORK,
            Self::Queued(Some(Hold::Asleep)) => module::key::SYSTEM_UPDATE_STATUS_WAITING_ASLEEP,
            Self::Restart { menu_only: false } => module::key::SYSTEM_UPDATE_STATUS_RESTART,
            Self::Restart { menu_only: true } => {
                module::key::SYSTEM_UPDATE_STATUS_RESTART_UNGUARDED
            }
            Self::Failed => module::key::SYSTEM_UPDATE_STATUS_FAILED,
            Self::Held => module::key::SYSTEM_UPDATE_STATUS_HELD,
            Self::Checking => module::key::SYSTEM_UPDATE_STATUS_CHECKING,
            Self::Available(Moved::Distro) => module::key::SYSTEM_UPDATE_STATUS_AVAILABLE_DISTRO,
            Self::Available(Moved::Nixpkgs) => module::key::SYSTEM_UPDATE_STATUS_AVAILABLE_NIXPKGS,
            Self::Available(Moved::Both) => module::key::SYSTEM_UPDATE_STATUS_AVAILABLE_BOTH,
            Self::UpToDate => module::key::SYSTEM_UPDATE_STATUS_UP_TO_DATE,
            Self::Unchecked(_) => module::key::SYSTEM_UPDATE_STATUS_UNCHECKED,
            Self::Unasked => module::key::SYSTEM_UPDATE_STATUS_UNASKED,
        }
    }

    /// Its colour, by what it means: what went wrong is danger, what waits
    /// for the person — an update to take, a restart — is a warning, all
    /// well is success, what is under way is the strongest text, and what
    /// only informs is quieter text. Never the accent: that is the cursor's
    /// (the status line is a control).
    #[must_use]
    pub const fn color(self) -> tokens::Color {
        match self {
            Self::Running | Self::CountingDown => tokens::color::TEXT_PRIMARY,
            Self::Queued(_) | Self::Held | Self::Unchecked(_) | Self::Unasked => {
                tokens::color::TEXT_SECONDARY
            }
            Self::Available(_) | Self::Restart { .. } => tokens::color::STATE_WARNING,
            Self::Failed => tokens::color::STATE_ERROR,
            Self::Checking => tokens::color::TEXT_DIM,
            Self::UpToDate => tokens::color::STATE_SUCCESS,
        }
    }

    /// Whether it is GitHub's answer — asking, an update there, none, could
    /// not tell, nobody asked yet —, the only status the line is a control
    /// for: asking again changes nothing a status of the brain's or of the
    /// machine's says.
    #[must_use]
    pub const fn is_online(self) -> bool {
        matches!(
            self,
            Self::Checking
                | Self::Available(_)
                | Self::UpToDate
                | Self::Unchecked(_)
                | Self::Unasked
        )
    }

    /// Its sentence — and, when one is known, the day the system last
    /// updated, in the zone and the shape the person reads dates in.
    #[must_use]
    pub fn sentence(
        self,
        worked: Option<i64>,
        zone: &ronoctua_files::when::Zone,
        texts: &ronoctua_i18n::Texts,
    ) -> String {
        let mut said = match self {
            Self::Unchecked(why) => texts.t_with(self.key(), &[("why", &texts.t(why.key()))]),
            _ => texts.t(self.key()),
        };
        if let Some(at) = worked {
            let when = crate::chrome::when_text(at, zone, false, texts);
            said.push(' ');
            said.push_str(&texts.t_with(
                module::key::SYSTEM_UPDATE_STATUS_LAST,
                &[("when", when.as_str())],
            ));
        }
        said
    }
}

/// What the three pages' rows say beside their names: the binary cache, the
/// follow of releases and how many parts are on their way — each read off
/// the one file a page reads first, so the head says it without opening it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Glance {
    /// The cache the person names.
    pub cache: ronoctua_cachix::settings::Named,
    /// Whether the distro's apps follow their releases.
    pub follow: crate::follow::Choice,
    /// How many of the machine's parts are not here yet.
    pub parts: usize,
}

/// Where the glance is read: the person's copy of the binary cache, their
/// copy of the follow, and the parts' records.
#[derive(Debug, Clone)]
pub struct GlancePlaces {
    /// The binary cache's copy (`binary_cache::Places::mirror`).
    cache: Option<PathBuf>,
    /// The follow's copy (`follow::Places::mirror`).
    follow: Option<PathBuf>,
    /// The parts' records (`parts::Places::records`).
    records: PathBuf,
}

impl GlancePlaces {
    /// This person's and this machine's — the pages' own places.
    #[must_use]
    pub fn here() -> Self {
        Self {
            cache: crate::binary_cache::Places::here().mirror,
            follow: crate::follow::Places::here().mirror,
            records: crate::parts::Places::here().records,
        }
    }

    /// When each thing the glance reads last moved. A record renamed into
    /// place moves its folder.
    #[must_use]
    pub fn stamp(&self) -> Vec<Option<std::time::SystemTime>> {
        let mtime = |p: &std::path::Path| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        [
            self.cache.clone(),
            self.follow.clone(),
            Some(self.records.clone()),
        ]
        .iter()
        .map(|p| p.as_deref().and_then(mtime))
        .collect()
    }

    /// What they say now: two small files and a folder of a few records.
    #[must_use]
    pub fn read(&self) -> Glance {
        use ronoctua_parts::record::Phase;
        Glance {
            cache: self
                .cache
                .as_deref()
                .map_or_else(Default::default, ronoctua_cachix::settings::read),
            follow: self
                .follow
                .as_deref()
                .map_or_else(Default::default, crate::follow::Choice::read),
            parts: ronoctua_parts::record::read_all(&self.records)
                .iter()
                .filter(|r| matches!(r.phase, Phase::Arriving | Phase::Waiting | Phase::NeedsYou))
                .count(),
        }
    }
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

/// One line at the top of the view — the status, the actions, the question
/// or what the focused thing is, then what the machine's records say —
/// placed and coloured, with what a click on it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLine {
    /// What it says from the line's first cell, already wrapped to the
    /// width — an action's label.
    pub text: String,
    /// Its colour: the cursor's on the focus and on an answer, danger's on
    /// what costs.
    pub color: tokens::Color,
    /// What a click on it takes, when anything.
    pub target: Option<Target>,
    /// What else the line draws, each at its own cell counted from the
    /// line's first: an action's value, and the window's name at the right
    /// end of the status while the window wears no header.
    pub beside: Vec<Piece>,
}

impl TopLine {
    /// The line as the footer's fields, from `left`: what it says, then what
    /// it draws beside — an empty piece is no piece.
    #[must_use]
    pub fn fields(&self, left: u16) -> Vec<crate::fields::Field> {
        std::iter::once((0, &self.text, self.color))
            .chain(self.beside.iter().map(|p| (p.at, &p.text, p.color)))
            .filter(|(_, text, _)| !text.is_empty())
            .map(|(at, text, color)| crate::fields::Field {
                at: left + at,
                text: text.clone(),
                color,
                hit: crate::fields::Hit::Nothing,
                explain: None,
                ink: crate::fields::Ink::Grid,
            })
            .collect()
    }
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
    /// The question about a gesture of the machine's, while it is asked —
    /// drawn where the gesture's row was, under the hand that clicked it.
    gesture: Option<crate::machine_gesture::Question>,
    /// A gesture said yes to whose verdict has not come: its row takes no
    /// click meanwhile.
    gesture_asked: Option<crate::machine_gesture::Gesture>,
    /// The person's brain modules that wait for an approval, as the machine
    /// says them (`crate::approval`).
    approvals: crate::approval::Readings,
    /// Where they are read, and when each file last moved — none for a
    /// test's or a photograph's view, which reads nothing of the machine.
    approval_places: Option<(crate::approval::Places, Vec<Option<std::time::SystemTime>>)>,
    /// The question about one of them, while it is asked — drawn where its
    /// row was, under the hand that clicked it.
    approval: Option<crate::approval::Question>,
    /// An approval said yes to whose verdict has not come: no row takes a
    /// click meanwhile.
    approval_asked: bool,
    /// What the brain last said of this task; `None` while it has said
    /// nothing (no brain, a test's view).
    brain: Option<Brain>,
    /// What GitHub said of the branches the machine's flake follows; `None`
    /// while nobody asked.
    heads: Option<Heads>,
    /// The machine's lock and when it last moved — none for a test's or a
    /// photograph's view. An update that moved it makes what GitHub said
    /// stale: the door asks again ([`View::heads_are_stale`]).
    lock: Option<(PathBuf, Option<std::time::SystemTime>)>,
    /// The heads were said before the lock last moved.
    stale_heads: bool,
    /// What the pages' rows say beside their names.
    glance: Glance,
    /// Where it is read, and when each thing last moved — none for a test's
    /// or a photograph's view, which reads nothing of the machine.
    glance_places: Option<(GlancePlaces, Vec<Option<std::time::SystemTime>>)>,
    /// The zone the status's date is drawn in.
    zone: ronoctua_files::when::Zone,
    /// Whether the keys walk the steps; otherwise a line of the head.
    on_steps: bool,
    /// The line of the head the PERSON put the focus on, by what it takes —
    /// a key or a first click; `None` where the view opens. A line that went
    /// away gives the focus back to where it rests ([`View::focus`]).
    head_focus: Option<Target>,
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
        let places = crate::approval::Places::here();
        v.approvals = crate::approval::Readings::read(&places);
        let stamp = places.stamp();
        v.approval_places = Some((places, stamp));
        let glance = GlancePlaces::here();
        v.glance = glance.read();
        let stamp = glance.stamp();
        v.glance_places = Some((glance, stamp));
        let lock = record_places()
            .machine_dir
            .join(ronoctua_software::heads::LOCK);
        let moved = modified(&lock);
        v.lock = Some((lock, moved));
        v.zone = ronoctua_files::when::Zone::read_from(std::path::Path::new("/etc/localtime"));
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
            gesture: None,
            gesture_asked: None,
            approvals: crate::approval::Readings::default(),
            approval_places: None,
            approval: None,
            approval_asked: false,
            brain: None,
            heads: None,
            lock: None,
            stale_heads: false,
            glance: Glance::default(),
            glance_places: None,
            zone: ronoctua_files::when::Zone::utc(),
            on_steps: false,
            head_focus: None,
        }
    }

    /// The same view over what the brain says of this task — a test's, a
    /// photograph's.
    #[must_use]
    pub const fn with_brain(mut self, brain: Brain) -> Self {
        self.brain = Some(brain);
        self
    }

    /// The same view over what GitHub said — a test's, a photograph's.
    #[must_use]
    pub fn with_heads(mut self, heads: Heads) -> Self {
        self.heads = Some(heads);
        self
    }

    /// The same view over these pages' values — a test's, a photograph's:
    /// read once, never again.
    #[must_use]
    pub fn with_glance(mut self, glance: Glance) -> Self {
        self.glance = glance;
        self.glance_places = None;
        self
    }

    /// The same view over these records, the belongings taught by `keys`.
    #[must_use]
    pub fn with_records(mut self, records: module::Records, keys: String) -> Self {
        self.records = records;
        self.belongings_keys = keys;
        self
    }

    /// The same view over these modules that wait — a test's, a
    /// photograph's: read once, never again.
    #[must_use]
    pub fn with_approvals(mut self, approvals: crate::approval::Readings) -> Self {
        self.approvals = approvals;
        self.approval_places = None;
        self
    }

    /// The same view over the modules that wait at `places` — a test's
    /// folder: read now, and again when a file there moves.
    #[cfg(test)]
    #[must_use]
    pub fn with_approval_places(mut self, places: crate::approval::Places) -> Self {
        self.approvals = crate::approval::Readings::read(&places);
        let stamp = places.stamp();
        self.approval_places = Some((places, stamp));
        self
    }

    /// Where the modules that wait are read and an approval's request is
    /// staged — none for a view that read them once.
    #[must_use]
    pub fn approval_places(&self) -> Option<&crate::approval::Places> {
        self.approval_places.as_ref().map(|(places, _)| places)
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

    /// The keys this view teaches now: the page's it is turned to, or the
    /// table of where the update stands — `u` only while it can start, the
    /// countdown's two answers only while this update counts down, `a` to
    /// stop only while it runs.
    #[must_use]
    pub const fn taught(&self) -> &'static [(&'static str, &'static str)] {
        if self.cache.is_some() {
            OF_THE_PAGE
        } else if self.follow.is_some() {
            OF_THE_FOLLOW
        } else if self.parts.is_some() {
            OF_THE_PARTS
        } else {
            match self.run() {
                Run::Idle => OF_THE_VIEW,
                Run::Queued => OF_THE_QUEUE,
                Run::CountingDown => OF_THE_COUNTDOWN,
                Run::Running => OF_THE_RUN,
            }
        }
    }

    /// Where the update stands in the brain — idle while the brain has
    /// said nothing.
    #[must_use]
    pub const fn run(&self) -> Run {
        match &self.brain {
            Some(b) => b.run(),
            None => Run::Idle,
        }
    }

    /// What the status line says now ([`status_of`]).
    #[must_use]
    pub fn status(&self) -> Status {
        status_of(self.brain.as_ref(), &self.records, self.heads.as_ref())
    }

    /// The brain spoke: what it says of this task is kept. Whether that
    /// changed anything the view draws.
    ///
    /// AN ANSWER TO A RUN THAT WENT AWAY IS FORGOTTEN: the focus the person
    /// put on "not now" or "stop" ends with that countdown or that run, so
    /// the next one never finds the cursor waiting there — data from the
    /// brain never moves the cursor onto what changes a run.
    pub fn heard_the_brain(&mut self, mind: &ronoctua_brain::Mind) -> bool {
        let now = Some(Brain::of(mind));
        if now == self.brain {
            return false;
        }
        self.brain = now;
        if let Some(t) = self.head_focus
            && t.changes_the_run()
            && !self.head_targets().contains(&t)
        {
            self.head_focus = None;
        }
        true
    }

    /// GitHub is being asked again.
    pub fn heads_asked(&mut self) {
        self.heads = Some(Heads::Asking);
        self.stale_heads = false;
    }

    /// What GitHub said came back.
    pub fn heads_came(
        &mut self,
        came: Result<Vec<ronoctua_software::heads::Heard>, ronoctua_software::heads::Unread>,
    ) {
        self.heads = Some(Heads::Came(came));
    }

    /// Whether what GitHub said was said against a lock that has moved
    /// since — an update moved the branches' revisions — and is to be asked
    /// again; taken.
    pub const fn heads_are_stale(&mut self) -> bool {
        std::mem::replace(&mut self.stale_heads, false)
    }

    /// The actions under the status line, in the update's state: start it,
    /// answer its countdown or stop it — then the three pages.
    #[must_use]
    pub fn actions(&self) -> Vec<Action> {
        let mut out = match self.run() {
            Run::Idle | Run::Queued => vec![Action::Start],
            Run::CountingDown => vec![Action::StartNow, Action::NotNow],
            Run::Running => vec![Action::Stop],
        };
        out.extend([Action::Cache, Action::Follow, Action::Parts]);
        out
    }

    /// Whether `action` can act now: "update now" cannot while the update
    /// already waits in the queue — its row is drawn refusing, and says why.
    const fn acts(&self, action: Action) -> bool {
        !matches!((action, self.run()), (Action::Start, Run::Queued))
    }

    /// The lines of the head that act and that the keys walk, in order: the
    /// status while it says GitHub's answer ([`Status::is_online`]), the
    /// actions, then the records' rows — the conflicts, the machine's
    /// gestures, the modules that wait. None while a question holds the
    /// cursor: an answer is the question's, never a step of the walk.
    #[must_use]
    pub fn head_targets(&self) -> Vec<Target> {
        if self.questioned() {
            return Vec::new();
        }
        let mut out = Vec::new();
        if self.status().is_online() {
            out.push(Target::Status);
        }
        out.extend(self.actions().into_iter().map(Target::Action));
        out.extend(self.record_targets());
        out
    }

    /// The records' rows that take a click now, in the order they are said
    /// ([`View::record_sentences`] draws each with its target).
    fn record_targets(&self) -> Vec<Target> {
        use crate::machine_gesture::Gesture;
        let r = &self.records;
        let mut out = Vec::new();
        if r.conflicts > 0 {
            out.push(Target::Belongings);
        }
        if r.trial == Some(module::TrialOutcome::Failed)
            && !r.again
            && self.gesture_takes(Gesture::TryAgain)
        {
            out.push(Target::Gesture(Gesture::TryAgain));
        }
        if r.hold.is_some() {
            if self.gesture_takes(Gesture::LetGo) {
                out.push(Target::Gesture(Gesture::LetGo));
            }
        } else if r.revision.is_some() && self.gesture_takes(Gesture::Hold) {
            out.push(Target::Gesture(Gesture::Hold));
        }
        if self.quiet() {
            for (n, w) in self.approvals.waiting.iter().enumerate() {
                if !self.approvals.approved_as_shown(w) {
                    out.push(Target::Approve(n));
                }
            }
        }
        out
    }

    /// Where the keys walk from now: the steps, or a line of the head — the
    /// one the person put the focus on while it is there, else where it
    /// rests ([`View::resting_focus`]).
    #[must_use]
    pub fn focus(&self) -> Focus {
        if self.on_steps {
            return Focus::Steps;
        }
        let targets = self.head_targets();
        self.head_focus
            .filter(|t| targets.contains(t))
            .or_else(|| self.resting_focus(&targets))
            .map_or(Focus::Steps, Focus::Head)
    }

    /// Where the focus rests when the person put it on no line that is
    /// there: "update now" while it is offered, else the status line while
    /// it is a control, else the binary cache's row.
    ///
    /// NEVER AN ANSWER THAT CHANGES A RUN — start now, not now, stop. The
    /// view opens while a run is under way, and a countdown or a run begins
    /// under a resting focus: landing there by itself, the focus would make
    /// a `RET`, or the click that only selects, stop a rebuild the person
    /// never chose to answer.
    fn resting_focus(&self, targets: &[Target]) -> Option<Target> {
        let start = Target::Action(Action::Start);
        [start, Target::Status, Target::Action(Action::Cache)]
            .into_iter()
            .filter(|t| *t != start || self.acts(Action::Start))
            .find(|t| targets.contains(t))
    }

    /// Whether the focus is on `target`'s line — the cursor's colour, never
    /// while a question holds the cursor. Every line a control is wrapped
    /// to asks the same, so they light together.
    fn focused(&self, target: Target) -> bool {
        !self.questioned() && self.focus() == Focus::Head(target)
    }

    /// Whether the steps wear the cursor: the focus is on them and no
    /// question holds it.
    #[must_use]
    pub fn on_the_steps(&self) -> bool {
        !self.questioned() && self.focus() == Focus::Steps
    }

    /// An arrow: ONE list, the head's lines that act and then the steps —
    /// down from the last of the head is the first step, up from the first
    /// step the last of the head.
    fn walk(&mut self, down: bool) -> Errand {
        let targets = self.head_targets();
        let last = self.steps.len().saturating_sub(1);
        match self.focus() {
            Focus::Steps if !down && self.cursor == 0 && !targets.is_empty() => {
                self.on_steps = false;
                self.head_focus = targets.last().copied();
            }
            Focus::Steps => {
                self.on_steps = true;
                self.cursor = if down {
                    (self.cursor + 1).min(last)
                } else {
                    self.cursor.saturating_sub(1)
                };
            }
            Focus::Head(at) => {
                let n = targets.iter().position(|t| *t == at).unwrap_or(0);
                if down && n + 1 >= targets.len() {
                    if !self.steps.is_empty() {
                        self.on_steps = true;
                        self.cursor = 0;
                    }
                } else {
                    let next = if down { n + 1 } else { n.saturating_sub(1) };
                    self.head_focus = targets.get(next).copied();
                }
            }
        }
        Errand::Moved
    }

    /// `TAB`: the other section — the head's lines, or the steps — each at
    /// the place it was left.
    const fn section(&mut self) -> Errand {
        self.on_steps = !self.on_steps;
        Errand::Moved
    }

    /// The wheel over the steps: the step cursor moves by one, and the
    /// focus goes to the steps.
    pub fn wheel_steps(&mut self, down: bool) -> Errand {
        self.on_steps = true;
        self.cursor = if down {
            (self.cursor + 1).min(self.steps.len().saturating_sub(1))
        } else {
            self.cursor.saturating_sub(1)
        };
        Errand::Moved
    }

    /// What an action does — its second click, its `RET` and its key.
    fn act(&mut self, action: Action) -> Errand {
        match action {
            // BELOW THE BATTERY FLOOR, A RUN BY HAND ASKS FIRST — the no
            // first — where a scheduled one would wait for the charger. And
            // one already in the queue is not asked for twice: why is said.
            Action::Start if !self.acts(Action::Start) => {
                Errand::Say(module::key::SYSTEM_UPDATE_START_QUEUED)
            }
            Action::Start => {
                if self.ask_on_battery() {
                    Errand::Moved
                } else {
                    Errand::Start
                }
            }
            Action::StartNow => Errand::Answer(true),
            Action::NotNow | Action::Stop => Errand::Answer(false),
            Action::Cache => self.toggle_cache(),
            Action::Follow => self.toggle_follow(),
            Action::Parts => self.toggle_parts(),
        }
    }

    /// `RET` on the line the focus is on: what its second click does. On a
    /// step there is nothing more to do than the cursor did.
    fn enter(&mut self) -> Errand {
        match self.focus() {
            Focus::Steps => Errand::Nothing,
            Focus::Head(Target::Status) => Errand::Recheck,
            Focus::Head(Target::Action(a)) => self.act(a),
            Focus::Head(other) => self.clicked(other),
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
            // THE SYSTEM UPDATED AGAIN: the page gives way to the steps,
            // which follow the run — started as `u` starts it, asking first
            // below the battery floor.
            Page::Update => {
                self.parts = None;
                if self.ask_on_battery() {
                    Errand::Moved
                } else {
                    Errand::Start
                }
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

    // -----------------------------------------------------------------------
    // THE MACHINE'S OWN GESTURES (`crate::machine_gesture`)
    // -----------------------------------------------------------------------

    /// The question about a gesture of the machine's, while it is asked.
    #[must_use]
    pub const fn gesture_question(&self) -> Option<&crate::machine_gesture::Question> {
        self.gesture.as_ref()
    }

    /// A gesture asked by its name (`LEADER :`): its question, the no first
    /// — when the records offer it now; otherwise the sentence of why not.
    pub fn ask_gesture(&mut self, what: crate::machine_gesture::Gesture) -> Errand {
        if let Some(why) = self.why_not(what) {
            return Errand::Say(why);
        }
        if self.asking.is_some() || self.gesture_asked.is_some() {
            return Errand::Nothing;
        }
        self.gesture = Some(crate::machine_gesture::Question::of(what));
        Errand::Moved
    }

    /// Why the records do not offer `what` now — `None` when they do.
    fn why_not(&self, what: crate::machine_gesture::Gesture) -> Option<&'static str> {
        use crate::machine_gesture::Gesture;
        let r = &self.records;
        match what {
            Gesture::TryAgain if r.trial != Some(module::TrialOutcome::Failed) => Some(
                module::machine_refusal_said(module::MachineRefusal::NoFailedTrial),
            ),
            Gesture::TryAgain if r.again => Some(key::MACHINE_SAID_TRY_AGAIN),
            Gesture::Hold if r.hold.is_some() => Some(key::MACHINE_SAID_HELD_ALREADY),
            Gesture::Hold if r.revision.is_none() => Some(key::MACHINE_SAID_NOTHING_TO_HOLD),
            Gesture::LetGo if r.hold.is_none() => Some(key::MACHINE_SAID_NOT_HELD),
            _ => None,
        }
    }

    /// The gesture's question answered: it closes, and a yes asks the
    /// machine — its row taking no click until the verdict comes.
    fn gesture_answered(&mut self, yes: bool) -> Errand {
        let Some(q) = self.gesture.take() else {
            return Errand::Nothing;
        };
        if !yes {
            return Errand::Moved;
        }
        self.gesture_asked = Some(q.what);
        Errand::Machine(q.what)
    }

    /// What came of the gesture said yes to was heard: its row takes clicks
    /// again (the records are read again when their files move).
    pub const fn gesture_heard(&mut self) {
        self.gesture_asked = None;
    }

    // -----------------------------------------------------------------------
    // THE PERSON'S BRAIN MODULES THAT WAIT (`crate::approval`)
    // -----------------------------------------------------------------------

    /// The question about a module that waits, while it is asked.
    #[must_use]
    pub const fn approval_question(&self) -> Option<&crate::approval::Question> {
        self.approval.as_ref()
    }

    /// Whether a question of the head holds the cursor: the run's, a
    /// gesture's or an approval's — no step wears the cursor meanwhile.
    #[must_use]
    pub const fn questioned(&self) -> bool {
        self.asking.is_some() || self.gesture.is_some() || self.approval.is_some()
    }

    /// Whether nothing is asked or waited for: a gesture's or an approval's
    /// row takes a click only then.
    const fn quiet(&self) -> bool {
        !self.questioned() && self.gesture_asked.is_none() && !self.approval_asked
    }

    /// An approval asked by its name (`LEADER :`): the question about the
    /// module `id` names, or the first still to approve, the no first;
    /// otherwise the sentence that none waits.
    pub fn ask_approval(&mut self, id: Option<&str>) -> Errand {
        let found = self
            .approvals
            .to_approve()
            .find(|w| id.is_none_or(|id| w.id == id))
            .cloned();
        let Some(w) = found else {
            return Errand::Say(key::MACHINE_APPROVE_SAID_NONE);
        };
        if !self.quiet() {
            return Errand::Nothing;
        }
        self.approval = Some(crate::approval::Question::of(w));
        Errand::Moved
    }

    /// The approval's question answered: it closes, and a yes asks the
    /// machine — no row taking a click until the verdict comes.
    fn approval_answered(&mut self, yes: bool) -> Errand {
        let Some(q) = self.approval.take() else {
            return Errand::Nothing;
        };
        if !yes {
            return Errand::Moved;
        }
        self.approval_asked = true;
        Errand::Approve(q.of)
    }

    /// What came of the approval said yes to was heard: the rows take
    /// clicks again, and the files are read again — an approval recorded
    /// moved one.
    pub fn approval_heard(&mut self) {
        self.approval_asked = false;
        self.approvals_reread();
    }

    /// The modules that wait read again when one of their files moved;
    /// `true` when one did.
    fn approvals_reread(&mut self) -> bool {
        let Some((places, stamp)) = self.approval_places.as_mut() else {
            return false;
        };
        let now = places.stamp();
        if now == *stamp {
            return false;
        }
        *stamp = now;
        self.approvals = crate::approval::Readings::read(places);
        if let Some(q) = &self.approval
            && !self.approvals.to_approve().any(|w| *w == q.of)
        {
            self.approval = None;
        }
        true
    }

    /// The modules that wait, as the records say them: each the sentence
    /// with the hash an approval must name and the row that asks — or its
    /// question while it is asked —, or, approved already at that hash, the
    /// sentence that it enters with the next update.
    fn approval_sentences(&self, texts: &ronoctua_i18n::Texts, out: &mut Vec<RecordLine>) {
        let acts = crate::chrome::Kind::Setting.tones(false).0;
        for (n, w) in self.approvals.waiting.iter().enumerate() {
            let args: [(&str, &str); 2] = [("id", &w.id), ("hash", &w.hash)];
            if self.approvals.approved_as_shown(w) {
                out.push(RecordLine {
                    text: texts.t_with(key::MACHINE_APPROVE_APPROVED, &args[..1]),
                    color: tokens::color::TEXT_SECONDARY,
                    target: None,
                    whole: true,
                });
                continue;
            }
            out.push(RecordLine {
                text: texts.t_with(key::MACHINE_APPROVE_WAITING, &args),
                color: tokens::color::STATE_WARNING,
                target: None,
                whole: true,
            });
            match self.approval.as_ref().filter(|q| q.of == *w) {
                Some(q) => {
                    out.push(RecordLine {
                        text: texts.t_with(key::MACHINE_APPROVE_QUESTION, &args),
                        color: tokens::color::QUESTION_ASKED,
                        target: None,
                        whole: true,
                    });
                    for (yes, answer) in [
                        (false, key::MACHINE_APPROVE_NO),
                        (true, key::MACHINE_APPROVE_YES),
                    ] {
                        let color = if q.yes() == yes {
                            tokens::color::TEXT_HERE_ACTS
                        } else if yes {
                            tokens::color::STATE_WARNING
                        } else {
                            acts
                        };
                        out.push(RecordLine {
                            text: texts.t(answer),
                            color,
                            target: Some(Target::ApproveAnswer(yes)),
                            whole: false,
                        });
                    }
                }
                None => out.push(RecordLine {
                    text: texts.t_with(key::MACHINE_APPROVE_ROW, &args[..1]),
                    color: acts,
                    target: self.quiet().then_some(Target::Approve(n)),
                    whole: true,
                }),
            }
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
        // AND THE PERSON'S MODULES THAT WAIT: the report, the approvals and
        // the approval's verdict, each one `stat`.
        moved |= self.approvals_reread();
        // THE PAGES' VALUES beside their actions, the same way.
        if let Some((places, stamp)) = self.glance_places.as_mut() {
            let now = places.stamp();
            if now != *stamp {
                *stamp = now;
                self.glance = places.read();
                moved = true;
            }
        }
        // THE LOCK MOVED — an update moved the branches' revisions: what
        // GitHub said, or is saying, was said against the old ones, and is
        // asked again.
        if let Some((lock, stamp)) = self.lock.as_mut() {
            let now = modified(lock);
            if now != *stamp {
                *stamp = now;
                self.stale_heads = self.heads.is_some();
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
        // A GESTURE'S QUESTION TAKES THE ARROWS, `RET` AND `Esc` while it is
        // asked, as the run's does: "no" above, "yes" below, `Esc` the "no".
        if let Some(q) = self.gesture.as_mut() {
            match press {
                Press::Up | Press::Down => {
                    q.moved(press == Press::Down);
                    return Errand::Moved;
                }
                Press::Enter => {
                    let yes = q.yes();
                    return self.gesture_answered(yes);
                }
                Press::Escape => return self.gesture_answered(false),
                _ => {}
            }
        }
        // AND AN APPROVAL'S, the same way.
        if let Some(q) = self.approval.as_mut() {
            match press {
                Press::Up | Press::Down => {
                    q.moved(press == Press::Down);
                    return Errand::Moved;
                }
                Press::Enter => {
                    let yes = q.yes();
                    return self.approval_answered(yes);
                }
                Press::Escape => return self.approval_answered(false),
                _ => {}
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
            // ONE LIST: the head's lines that act, then the steps.
            Press::Down => self.walk(true),
            Press::Up => self.walk(false),
            // A QUESTION HOLDS THE CURSOR: there is no other section to go
            // to until it is answered.
            Press::Section if self.questioned() => Errand::Nothing,
            Press::Section => self.section(),
            Press::OutputUp => Errand::Output(Output::Up),
            Press::OutputDown => Errand::Output(Output::Down),
            Press::OutputEnd => Errand::Output(Output::End),
            Press::Start => self.act(Action::Start),
            // THE COUNTDOWN'S ANSWERS: the door answers only this update's
            // (`answers_this_update`), and says so otherwise.
            Press::Abort => Errand::Answer(false),
            Press::Proceed => Errand::Answer(true),
            Press::Enter => self.enter(),
            Press::Escape => Errand::Close,
            Press::Cache => self.toggle_cache(),
            Press::Follow => self.toggle_follow(),
            Press::Parts => self.toggle_parts(),
            // A FILE IS HANDED IN only on the parts' page.
            Press::HandIn => Errand::Nothing,
        }
    }

    /// The head of the view, wrapped to `room` ([`View::top_lines_with`] with no
    /// name at the status's end — the footer's region has no header to hide).
    #[must_use]
    pub fn top_lines(&self, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<TopLine> {
        self.top_lines_with(room, None, texts)
    }

    /// The head of the view, wrapped to `room`: on a page, the page's; else
    /// the STATUS — whether an update is there, what the brain is doing
    /// with it — with `title` at its right end (the window's name, while it
    /// wears no header), a line of air, the ACTIONS one does here, a line of
    /// air, then the question while it is asked or what the focused thing
    /// is; then what the machine's records say, each line only when it says
    /// something. Notes are WRAPPED, never cut.
    #[must_use]
    pub fn top_lines_with(
        &self,
        room: u16,
        title: Option<&str>,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<TopLine> {
        let line = |text: String, color, target| TopLine {
            text,
            color,
            target,
            beside: Vec::new(),
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
        let mut out = self.status_lines(room, title, texts);
        // AIR, then THE ACTIONS — the same short words their keys teach —
        // then air again: three subjects, and hierarchy is colour and air,
        // never size.
        out.push(line(String::new(), tokens::color::TEXT_SECONDARY, None));
        for action in self.actions() {
            out.push(self.action_line(action, room, texts));
        }
        out.push(line(String::new(), tokens::color::TEXT_SECONDARY, None));
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
            // WHAT THE FOCUSED THING IS — the second register: a sentence,
            // and under a step the command it runs, quieter.
            for (said, color) in self.register(texts) {
                for wrapped in crate::chrome::note_lines(&said, room) {
                    out.push(line(wrapped, color, None));
                }
            }
        }
        // A LINE OF AIR between what the focused thing is and what the
        // machine's records say: two subjects. Only when there is a record
        // to say.
        let records = self.record_sentences(texts);
        if !records.is_empty() {
            out.push(line(String::new(), tokens::color::TEXT_SECONDARY, None));
        }
        for said in records {
            // THE FOCUS ON A RECORD'S ROW is the cursor's colour, as on an
            // action — on every line the sentence is wrapped to, and on both
            // lines of one control (the hold's sentence and the row that lets
            // it go).
            let color = if said.target.is_some_and(|t| self.focused(t)) {
                crate::chrome::Kind::Setting.tones(true).0
            } else {
                said.color
            };
            // A SENTENCE IS WRAPPED WHOLE; an answer is one line, as the
            // run's question draws its answers.
            if said.whole {
                for wrapped in crate::chrome::note_lines(&said.text, room) {
                    out.push(line(wrapped, color, said.target));
                }
            } else {
                out.push(line(
                    layout::elide_end(&said.text, room),
                    color,
                    said.target,
                ));
            }
        }
        out
    }

    /// The status, wrapped — in the room left of `title` when there is one,
    /// which is drawn at the right end of its first line in the quietest
    /// tone the window has: whose window this is, said once the header that
    /// said it is hidden. While it says GitHub's answer the line is a
    /// control: the first click focuses it, the second asks GitHub again.
    fn status_lines(
        &self,
        room: u16,
        title: Option<&str>,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<TopLine> {
        let status = self.status();
        let said = status.sentence(self.brain.and_then(|b| b.worked), &self.zone, texts);
        let color = if self.focused(Target::Status) {
            tokens::color::TEXT_HERE_ACTS
        } else {
            status.color()
        };
        // THE NAME TAKES THE RIGHT END only while it leaves the sentence at
        // least half the line, and ends where a step's verdict ends.
        let name = title.and_then(|t| {
            let cells = layout::width(t);
            let at = room.checked_sub(EDGE_RIGHT + cells)?;
            (cells + COLUMN_GAP <= room / 2).then(|| Piece {
                at,
                text: t.to_owned(),
                color: tokens::color::TEXT_DIM,
            })
        });
        let wrap = name
            .as_ref()
            .map_or(room, |n| n.at.saturating_sub(COLUMN_GAP));
        let target = (!self.questioned() && status.is_online()).then_some(Target::Status);
        crate::chrome::note_lines(&said, wrap)
            .into_iter()
            .enumerate()
            .map(|(n, text)| TopLine {
                text,
                color,
                target,
                beside: if n == 0 {
                    name.clone().into_iter().collect()
                } else {
                    Vec::new()
                },
            })
            .collect()
    }

    /// An action's row: its short word — and a page's value beside it, in
    /// the value's column — drawn as a setting, the focus the cursor's
    /// colour; one that cannot act now drawn refusing. No click while a
    /// question holds the cursor.
    fn action_line(&self, action: Action, room: u16, texts: &ronoctua_i18n::Texts) -> TopLine {
        let kind = if self.acts(action) {
            crate::chrome::Kind::Setting
        } else {
            crate::chrome::Kind::Disabled
        };
        let focused = self.focused(Target::Action(action));
        let (value, tint) = self.glance_of(action, texts);
        let fields = crate::chrome::tinted(
            crate::chrome::field_pair(0, room, &texts.t(action.id()), &value, kind, focused),
            tint,
            focused,
        );
        let mut pieces = fields.into_iter().map(|f| Piece {
            at: f.at,
            text: f.text,
            color: f.color,
        });
        let label = pieces.next();
        TopLine {
            text: label.as_ref().map_or_else(String::new, |p| p.text.clone()),
            color: label.map_or(kind.tones(focused).0, |p| p.color),
            target: (!self.questioned()).then_some(Target::Action(action)),
            beside: pieces.collect(),
        }
    }

    /// A page's value beside its action, and the colour of a state it says
    /// (a copy that does not read); nothing beside the update's own.
    fn glance_of(
        &self,
        action: Action,
        texts: &ronoctua_i18n::Texts,
    ) -> (String, Option<tokens::Color>) {
        use ronoctua_cachix::settings::Named;
        let unreadable = Some(tokens::color::STATE_WARNING);
        match action {
            Action::Cache => match &self.glance.cache {
                Named::None => (texts.t(key::BINARY_CACHE_NONE), None),
                Named::Cache(c) => (
                    texts.t_with(
                        match c.visibility {
                            ronoctua_person::options::Visibility::Public => {
                                key::BINARY_CACHE_GLANCE_PUBLIC
                            }
                            ronoctua_person::options::Visibility::Private => {
                                key::BINARY_CACHE_GLANCE_PRIVATE
                            }
                        },
                        &[("name", c.name.as_str())],
                    ),
                    None,
                ),
                Named::Unreadable => (texts.t(key::BINARY_CACHE_COPY_UNREADABLE), unreadable),
                Named::Newer => (texts.t(key::BINARY_CACHE_COPY_NEWER), unreadable),
            },
            Action::Follow => match self.glance.follow {
                crate::follow::Choice::On => (texts.t(key::DISTRO_FOLLOW_ON), None),
                crate::follow::Choice::Off => (texts.t(key::DISTRO_FOLLOW_OFF), None),
                crate::follow::Choice::Unreadable => {
                    (texts.t(key::DISTRO_FOLLOW_COPY_UNREADABLE), unreadable)
                }
                crate::follow::Choice::Newer => {
                    (texts.t(key::DISTRO_FOLLOW_COPY_NEWER), unreadable)
                }
            },
            Action::Parts => (
                texts.t_args(
                    key::DISTRO_PARTS_GLANCE,
                    &[(
                        "count",
                        ronoctua_i18n::Arg::Number(
                            i64::try_from(self.glance.parts).unwrap_or(i64::MAX),
                        ),
                    )],
                ),
                None,
            ),
            Action::Start | Action::StartNow | Action::NotNow | Action::Stop => {
                (String::new(), None)
            }
        }
    }

    /// The second register: what the focused thing is, as sentences and
    /// their tones — a step's sentence (why it was skipped, declined or
    /// refused with it) and, quieter, the command it runs; what an action
    /// does, or why it cannot now; what the status line is and what asking
    /// again does, with the next moment the brain has for the update; what
    /// choosing a record's row does.
    fn register(&self, texts: &ronoctua_i18n::Texts) -> Vec<(String, tokens::Color)> {
        let said = |k: &str| vec![(texts.t(k), tokens::color::TEXT_SECONDARY)];
        match self.focus() {
            Focus::Steps => self.steps.get(self.cursor).map_or_else(Vec::new, |step| {
                let (verdict, _) = self.verdicts.of_step(step.id);
                vec![
                    (
                        cursor_explains(step, &verdict, texts),
                        tokens::color::TEXT_SECONDARY,
                    ),
                    // THE COMMAND IS DATA, drawn as it is written: a command
                    // line is not translated.
                    (step.command.to_owned(), tokens::color::TEXT_DIM),
                ]
            }),
            Focus::Head(Target::Status) => {
                let mut sentence = texts.t(module::key::SYSTEM_UPDATE_STATUS_DOES);
                if let Some(next) = self.brain.map(|b| b.next).filter(|n| *n > 0) {
                    let when = crate::chrome::when_text(next, &self.zone, false, texts);
                    sentence.push(' ');
                    sentence.push_str(&texts.t_with(
                        module::key::SYSTEM_UPDATE_STATUS_NEXT,
                        &[("when", when.as_str())],
                    ));
                }
                vec![(sentence, tokens::color::TEXT_SECONDARY)]
            }
            Focus::Head(Target::Action(a)) if !self.acts(a) => {
                said(module::key::SYSTEM_UPDATE_START_QUEUED)
            }
            Focus::Head(Target::Action(a)) => said(a.does()),
            Focus::Head(Target::Belongings) => said(module::key::SYSTEM_UPDATE_REGISTER_BELONGINGS),
            Focus::Head(Target::Gesture(_) | Target::Approve(_)) => {
                said(module::key::SYSTEM_UPDATE_REGISTER_ASKS)
            }
            Focus::Head(_) => Vec::new(),
        }
    }

    /// The second register's lines: WRAPPED, never cut — a note is read
    /// whole. What a test reads the head against.
    #[cfg(test)]
    fn register_lines(&self, room: u16, texts: &ronoctua_i18n::Texts) -> Vec<String> {
        self.register(texts)
            .into_iter()
            .flat_map(|(said, _)| crate::chrome::note_lines(&said, room))
            .collect()
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
        if let Some(q) = self.gesture.as_ref()
            && let Some(n) = head
                .iter()
                .position(|l| l.target == Some(Target::GestureAnswer(q.yes())))
        {
            return n;
        }
        if let Some(q) = self.approval.as_ref()
            && let Some(n) = head
                .iter()
                .position(|l| l.target == Some(Target::ApproveAnswer(q.yes())))
        {
            return n;
        }
        if let Some(q) = self.asking
            && let Some(n) = head
                .iter()
                .position(|l| l.target == Some(Target::Answer(q.yes)))
        {
            return n;
        }
        // THE FOCUS: the first line of the head it is on, or the step.
        if let Focus::Head(at) = self.focus()
            && let Some(n) = head.iter().position(|l| l.target == Some(at))
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
                    fields: said.fields(left),
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
        let on_the_steps = self.on_the_steps();
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
                    fields: said.fields(left),
                    target: said.target,
                }
            } else if n < air_at {
                let at = n - steps_at;
                let step = &self.steps[at];
                let (verdict, seconds) = self.verdicts.of_step(step.id);
                let mut pieces = step_row(step, &verdict, seconds, room, texts);
                // THE NAME IS THE LAST PIECE, and the cursor is its colour —
                // while the focus is on the steps and no question holds it.
                if at == self.cursor
                    && on_the_steps
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
    /// THE STATUS AND AN ACTION: the first click puts the focus there and
    /// says what it is, the second does what `RET` does — asks GitHub again,
    /// does the action (invariant 19).
    ///
    /// A STEP: the focus goes to the steps and the cursor there, and what the
    /// step is is said. There is nothing else to do to a step here —
    /// switching one on or off belongs to the brain's screen, where the
    /// choice is written down — so a second click, like `RET`, does nothing
    /// more.
    ///
    /// AN ANSWER: the first click puts the cursor on it, the second answers,
    /// as `RET` does. THE BELONGINGS' LINE the same: the first click puts
    /// the focus there, the second opens them. A gesture's or a module's row
    /// asks at once, under the hand — asking is not doing — and takes the
    /// focus as well.
    ///
    /// AN ANSWER TO THE RUN — start now, not now, stop — is done by a click
    /// only where the PERSON put the focus, by a key or a first click; never
    /// where it rests by itself ([`View::resting_focus`]).
    pub fn clicked(&mut self, target: Target) -> Errand {
        // A LINE OF THE HEAD THAT ACTS is where the focus goes — and a
        // second click on the focused one does what it does.
        if matches!(
            target,
            Target::Status
                | Target::Action(_)
                | Target::Belongings
                | Target::Gesture(_)
                | Target::Approve(_)
        ) {
            if !self.head_targets().contains(&target) {
                return Errand::Nothing;
            }
            let again = self.focus() == Focus::Head(target)
                && (!target.changes_the_run() || self.head_focus == Some(target));
            self.on_steps = false;
            self.head_focus = Some(target);
            match target {
                Target::Status if again => return Errand::Recheck,
                Target::Action(a) if again => return self.act(a),
                Target::Belongings if again => return Errand::OpenBelongings,
                Target::Status | Target::Action(_) | Target::Belongings => return Errand::Moved,
                _ => {}
            }
        }
        match target {
            Target::Status | Target::Action(_) | Target::Belongings => Errand::Nothing,
            Target::Step(n) => {
                if n >= self.steps.len() || (n == self.cursor && self.on_steps) {
                    return Errand::Nothing;
                }
                self.on_steps = true;
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
            // A GESTURE OF THE MACHINE'S: the click asks it, where its row
            // was — the no first, the cursor born there. Asking is not doing:
            // the yes is the second click, on the pick, and the sudo bar's.
            Target::Gesture(what) => {
                if !self.quiet() {
                    return Errand::Nothing;
                }
                self.gesture = Some(crate::machine_gesture::Question::of(what));
                Errand::Moved
            }
            Target::GestureAnswer(yes) => {
                let Some(q) = self.gesture.as_mut() else {
                    return Errand::Nothing;
                };
                if q.clicked(yes) {
                    self.gesture_answered(yes)
                } else {
                    Errand::Moved
                }
            }
            // A BRAIN MODULE OF THE PERSON'S THAT WAITS: the click asks to
            // approve it, where its row was — the no first, as a gesture.
            Target::Approve(n) => {
                let Some(w) = self.approvals.waiting.get(n).cloned() else {
                    return Errand::Nothing;
                };
                if !self.quiet() || self.approvals.approved_as_shown(&w) {
                    return Errand::Nothing;
                }
                self.approval = Some(crate::approval::Question::of(w));
                Errand::Moved
            }
            Target::ApproveAnswer(yes) => {
                let Some(q) = self.approval.as_mut() else {
                    return Errand::Nothing;
                };
                if q.clicked(yes) {
                    self.approval_answered(yes)
                } else {
                    Errand::Moved
                }
            }
        }
    }

    /// What the machine's records say, as sentences: the belongings'
    /// conflicts (a click opens them), a new system that waits for the next
    /// restart, the boot trial or why there is none, and a hold — each only
    /// when it is there, in the colour of what it costs. And the gestures
    /// they offer, where each one's sentence is: the failed trial's, tried
    /// again at the next update (or the sentence that it is); the hold's,
    /// which a click lets go; and, not held, the revision a hold would keep
    /// — each a click that asks, or its question while it is asked.
    fn record_sentences(&self, texts: &ronoctua_i18n::Texts) -> Vec<RecordLine> {
        use crate::machine_gesture::Gesture;
        let r = &self.records;
        let note = |text: String, color, target| RecordLine {
            text,
            color,
            target,
            whole: true,
        };
        let mut out = Vec::new();
        if r.conflicts > 0 {
            out.push(note(
                texts.t_args(
                    module::key::SYSTEM_UPDATE_RECORDS_CONFLICTS,
                    &[
                        (
                            "count",
                            ronoctua_i18n::Arg::Number(
                                i64::try_from(r.conflicts).unwrap_or(i64::MAX),
                            ),
                        ),
                        ("keys", ronoctua_i18n::Arg::Text(&self.belongings_keys)),
                    ],
                ),
                tokens::color::STATE_WARNING,
                Some(Target::Belongings),
            ));
        }
        // A SYSTEM SET FOR THE NEXT BOOT: the update did its work, and the
        // machine uses it from the restart — said before the trial that guards
        // that restart, and promising its way back only when one is armed.
        // Not when the status line says it already, with what the trial
        // promises or does not (`Status::Restart`).
        if let Some(next) = r
            .staged
            .as_ref()
            .filter(|_| !matches!(self.status(), Status::Restart { .. }))
        {
            out.push(note(
                texts.t(if next.guarded {
                    module::key::SYSTEM_UPDATE_RECORDS_STAGED
                } else {
                    module::key::SYSTEM_UPDATE_RECORDS_STAGED_UNGUARDED
                }),
                tokens::color::STATE_WARNING,
                None,
            ));
        }
        if let Some(outcome) = r.trial {
            let color = match outcome {
                module::TrialOutcome::Failed => tokens::color::STATE_ERROR,
                _ => tokens::color::TEXT_SECONDARY,
            };
            out.push(note(texts.t(trial_key(outcome)), color, None));
            // THE SYSTEM THAT DID NOT START, tried again at the next update —
            // asked here, or said to be, with the key that updates now.
            if outcome == module::TrialOutcome::Failed {
                if r.again {
                    out.push(note(
                        texts.t_with(
                            key::MACHINE_TRY_AGAIN_ASKED,
                            &[("keys", &view_key_of(START))],
                        ),
                        tokens::color::TEXT_SECONDARY,
                        None,
                    ));
                } else {
                    self.gesture_lines(
                        Gesture::TryAgain,
                        key::MACHINE_TRY_AGAIN_ROW,
                        &[],
                        texts,
                        &mut out,
                    );
                }
            }
        }
        if let Some(why) = r.no_trial {
            out.push(note(
                texts.t(no_trial_key(why)),
                tokens::color::TEXT_SECONDARY,
                None,
            ));
        }
        if let Some(hold) = &r.hold {
            // THE REVISION AS GIT ABBREVIATES IT, and the day it was held: the
            // record's whole stamp is a machine's way of writing a date. The
            // sentence is the let-go's control, with the row under it; what a
            // hold means is the status line's to say (`Status::Held`).
            let rev: String = hold.rev.chars().take(REV_SHOWN).collect();
            let since = hold.since.split('T').next().unwrap_or_default();
            let lets_go = self
                .gesture_takes(Gesture::LetGo)
                .then_some(Target::Gesture(Gesture::LetGo));
            out.push(note(
                texts.t_with(
                    module::key::SYSTEM_UPDATE_RECORDS_HOLD,
                    &[("rev", &rev), ("since", since)],
                ),
                tokens::color::TEXT_SECONDARY,
                lets_go,
            ));
            self.gesture_lines(
                Gesture::LetGo,
                key::MACHINE_LET_GO_ROW,
                &[],
                texts,
                &mut out,
            );
        } else if let Some(rev) = &r.revision {
            let rev: String = rev.chars().take(REV_SHOWN).collect();
            self.gesture_lines(
                Gesture::Hold,
                key::MACHINE_HOLD_ROW,
                &[("rev", &rev)],
                texts,
                &mut out,
            );
        }
        // A FLAKE THAT FOLLOWS A LOCAL CHECKOUT: the update moves nixpkgs
        // alone, and what is committed there comes in through the sudo door.
        if r.checkout {
            out.push(note(
                texts.t(module::key::SYSTEM_UPDATE_RECORDS_CHECKOUT),
                tokens::color::TEXT_SECONDARY,
                None,
            ));
        }
        // THE PERSON'S BRAIN MODULES THAT WAIT FOR AN APPROVAL, last: what
        // the machine's last build left out, each approved where it is said.
        self.approval_sentences(texts, &mut out);
        out
    }

    /// Whether `what`'s row takes a click now: no question is asked — the
    /// run's, a gesture's or an approval's — and no gesture or approval
    /// waits for its verdict.
    fn gesture_takes(&self, what: crate::machine_gesture::Gesture) -> bool {
        self.quiet() && self.why_not(what).is_none()
    }

    /// A gesture's row, `row` said with `args` — or, while it is asked, its
    /// question in its place: the sentence, then "no" and "yes", the cursor's
    /// answer in the cursor's colour and the yes in the colour of what it
    /// costs.
    fn gesture_lines(
        &self,
        what: crate::machine_gesture::Gesture,
        row: &'static str,
        args: &[(&str, &str)],
        texts: &ronoctua_i18n::Texts,
        out: &mut Vec<RecordLine>,
    ) {
        let acts = crate::chrome::Kind::Setting.tones(false).0;
        match self.gesture.as_ref().filter(|q| q.what == what) {
            Some(q) => {
                out.push(RecordLine {
                    text: texts.t(what.question_key()),
                    color: tokens::color::QUESTION_ASKED,
                    target: None,
                    whole: true,
                });
                for yes in [false, true] {
                    let color = if q.yes() == yes {
                        tokens::color::TEXT_HERE_ACTS
                    } else if yes {
                        tokens::color::STATE_WARNING
                    } else {
                        acts
                    };
                    out.push(RecordLine {
                        text: texts.t(what.answer_key(yes)),
                        color,
                        target: Some(Target::GestureAnswer(yes)),
                        whole: false,
                    });
                }
            }
            None => out.push(RecordLine {
                text: texts.t_with(row, args),
                color: acts,
                target: self.gesture_takes(what).then_some(Target::Gesture(what)),
                whole: true,
            }),
        }
    }
}

/// One thing the machine's records say: a sentence wrapped whole, or a
/// question's answer on a line of its own — with what a click on it takes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordLine {
    text: String,
    color: tokens::Color,
    target: Option<Target>,
    /// A sentence, wrapped whole; otherwise one line, elided.
    whole: bool,
}

/// The view's own key for `command` (`OF_THE_VIEW`), as a key is drawn —
/// taught by a sentence, never written in one.
fn view_key_of(command: &str) -> String {
    OF_THE_VIEW
        .iter()
        .find(|(_, id)| *id == command)
        .map(|(notation, _)| (*notation).to_owned())
        .unwrap_or_default()
}

/// How many hex digits of a held revision are drawn — git's own short form.
const REV_SHOWN: usize = 7;

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
        // AND WHETHER AN UPDATE IS THERE, asked once as the window opens —
        // again only when the person asks, or when an update moved the lock.
        open.check_the_heads(false);

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
                    if door.content.open.reread_if_moved() {
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
                             \"trial\": \"failed\", \"again\": false,
                             \"no-trial\": \"not-in-menu\",
                             \"hold\": {\"rev\": \"<hex>\", \"since\": \"<stamp>\"},
                             \"revision\": \"<hex>\", \"checkout\": true},
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
                             \"registry\": {…distro-apps.json…},
                             \"applied\": {…person-applied.json…}},
                           \"parts\": {\"records\": [{…a part's record…}],
                             \"names\": {\"<id>\": \"<name>\"},
                             \"stick\": {…stick.json…},
                             \"mirrors\": [\"https://…\"],
                             \"last-apply\": {…state.json's last apply…},
                             \"here\": [\"<a missing download's output>\"]},
                           \"approvals\": {\"waiting\": [{\"id\": \"my-…\",
                               \"hash\": \"sha256-…\"}],
                             \"approved\": {\"my-…\": \"sha256-…\"}},
                           \"brain\": {\"running\": true, \"announced\": true,
                             \"queued\": true, \"hold\": \"battery\",
                             \"last\": {\"kind\": \"done\" | \"failed\",
                               \"ended\": <epoch>},
                             \"worked\": <epoch>, \"next\": <epoch>},
                           \"heads\": \"checking\" | \"no-lock\" | \"unreadable\"
                             | {\"ronoctua\": \"moved\" | \"same\" | \"<refusal>\",
                                \"nixpkgs\": …}}
                        `brain`: what the brain says of the update (any
                        field may be left out; a `done` last run is also
                        when it last worked); `heads`: what GitHub said of
                        the branches the machine's flake follows — a branch
                        that moved, one where the lock holds it, or a
                        refusal (`offline`, `rate-limited`, `tool-failed`…);
                        `metered`: the brain holds the run a `u` asks for,
                        for a metered way out; `cache`: the view opens on the
                        binary cache's page, over these readings; `follow`:
                        on the page of the apps that follow their releases;
                        `parts`: on the parts' page; `approvals`: the
                        person's brain modules that wait, on the records
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
        approvals: Option<ApprovalsScene>,
        brain: Option<BrainScene>,
        heads: Option<serde_json::Value>,
    }

    /// What the brain says of the update, as the scene says it.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct BrainScene {
        running: bool,
        announced: bool,
        queued: bool,
        hold: Option<String>,
        last: Option<LastScene>,
        worked: Option<i64>,
        next: i64,
    }

    /// The update's last run, as the scene says it.
    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LastScene {
        kind: String,
        ended: i64,
    }

    impl BrainScene {
        fn brain(self) -> Result<super::Brain> {
            use ronoctua_brain::Hold;
            let hold = match self.hold.as_deref() {
                None => None,
                Some(word) => Some(
                    [
                        Hold::Ram,
                        Hold::Cpu,
                        Hold::Battery,
                        Hold::Busy,
                        Hold::Network,
                        Hold::Asleep,
                    ]
                    .into_iter()
                    .find(|h| h.word() == word)
                    .with_context(|| format!("`hold` is a word of the brain's, not `{word}`"))?,
                ),
            };
            let (failed, done) = match &self.last {
                None => (false, None),
                Some(l) if l.kind == "done" => (false, Some(l.ended)),
                Some(l) if l.kind == "failed" => (true, None),
                Some(l) => bail!("`last.kind` is done or failed, not `{}`", l.kind),
            };
            Ok(super::Brain {
                running: self.running,
                announced: self.announced,
                queued: self.queued,
                hold,
                failed,
                worked: done.or(self.worked),
                next: self.next,
            })
        }
    }

    /// What GitHub said, as the scene says it: a word for the whole answer,
    /// or one per input of the machine's flake.
    fn heads_of(said: &serde_json::Value) -> Result<super::Heads> {
        use ronoctua_software::heads::{Branch, Head, Heard, Input, Unlearnt, Unread};
        const LOCKED: &str = "0000000000000000000000000000000000000000";
        const NEWER: &str = "1111111111111111111111111111111111111111";
        if let Some(word) = said.as_str() {
            return Ok(match word {
                "checking" => super::Heads::Asking,
                "no-lock" => super::Heads::Came(Err(Unread::NoLock)),
                "unreadable" => super::Heads::Came(Err(Unread::Unreadable)),
                other => bail!(
                    "`heads` is checking, no-lock, unreadable or one word per input, not `{other}`"
                ),
            });
        }
        let by_input = said
            .as_object()
            .context("`heads` is a word or one word per input")?;
        let mut heard = Vec::new();
        for (name, word) in by_input {
            let input = Input::ALL
                .into_iter()
                .find(|i| i.name() == name)
                .with_context(|| format!("`heads.{name}` is not an input the update moves"))?;
            let word = word
                .as_str()
                .with_context(|| format!("`heads.{name}` is a word"))?;
            let head = |sha: &str| Head {
                branch: Branch {
                    input,
                    owner: "owner".to_owned(),
                    repo: "repo".to_owned(),
                    branch: "branch".to_owned(),
                    locked: LOCKED.to_owned(),
                },
                sha: sha.to_owned(),
            };
            heard.push(Heard {
                input,
                head: match word {
                    "moved" => Ok(head(NEWER)),
                    "same" => Ok(head(LOCKED)),
                    "unasked" => Err(Unlearnt::Unasked),
                    "lock" => Err(Unlearnt::Lock),
                    refusal => Err(Unlearnt::Asked(ronoctua_software::Failure::refused(
                        serde_json::from_value(serde_json::Value::String(refusal.to_owned()))
                            .with_context(|| {
                                format!(
                                    "`heads.{name}` is moved, same, unasked, lock or a refusal, \
                                     not `{refusal}`"
                                )
                            })?,
                    ))),
                },
            });
        }
        heard.sort_by_key(|h| h.input);
        Ok(super::Heads::Came(Ok(heard)))
    }

    /// The person's brain modules that wait, as the scene says them: the
    /// report's `awaiting-approval` and root's approvals, by id.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct ApprovalsScene {
        waiting: Vec<ronoctua_person::reports::Awaiting>,
        approved: std::collections::BTreeMap<String, String>,
    }

    impl ApprovalsScene {
        fn readings(self) -> crate::approval::Readings {
            crate::approval::Readings {
                waiting: self
                    .waiting
                    .into_iter()
                    .map(|w| crate::approval::Waiting {
                        id: w.id,
                        hash: w.hash,
                    })
                    .collect(),
                approved: self.approved,
            }
        }
    }

    /// The parts' page, as the scene says it: the records root would have
    /// written, the plan's names, the last stick gesture and the media
    /// mounted — and the machine's last apply, with the downloads of it the
    /// store holds now (`here`, their outputs).
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct PartsScene {
        records: Vec<ronoctua_parts::record::Record>,
        names: std::collections::BTreeMap<String, String>,
        stick: Option<ronoctua_parts::stick::Said>,
        mirrors: Vec<String>,
        #[serde(rename = "last-apply")]
        last_apply: Option<ronoctua_person::state::LastApply>,
        here: Vec<String>,
    }

    /// The page of the apps that follow their releases, as the scene says
    /// it: the person's copy (none: following, the default), and the
    /// registry's report as the machine would have it.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct FollowScene {
        copy: Option<String>,
        registry: Option<ronoctua_person::reports::DistroApps>,
        applied: Option<ronoctua_person::reports::PersonApplied>,
        /// The registry of a system that waits for the next restart, when
        /// one does.
        waiting: Option<ronoctua_person::reports::DistroApps>,
    }

    impl FollowScene {
        fn readings(self) -> Result<crate::follow::Readings> {
            use crate::follow::Choice;
            let copy = match self.copy.as_deref() {
                None | Some("on") => Choice::On,
                Some("off") => Choice::Off,
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
                machine,
                apps,
                taken: self
                    .applied
                    .as_ref()
                    .map_or(crate::follow::Taken::Yes, crate::follow::Taken::of),
                waiting: self
                    .waiting
                    .as_ref()
                    .map(|r| crate::follow::of_registry(r).0),
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
        /// Whether a system waits for the next restart, and the cache it
        /// was built with (`waiting`, none when absent).
        waits: bool,
        waiting: Option<ronoctua_person::options::BinaryCache>,
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
                waiting: self.waits.then_some(self.waiting),
            })
        }
    }

    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
    struct Records {
        conflicts: usize,
        trial: Option<module::TrialOutcome>,
        /// The failed trial's system is tried again at the next update.
        again: bool,
        no_trial: Option<module::NoTrial>,
        staged: bool,
        hold: Option<Hold>,
        /// The distro revision the machine directory's lock names: what a
        /// hold would hold.
        revision: Option<String>,
        /// The machine directory's lock names a local checkout.
        checkout: bool,
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
            r.system = crate::parts::System::of(p.last_apply.as_ref(), |out| {
                p.here.iter().any(|h| h == out)
            });
            r
        });
        let approvals = scene.approvals.take().map(ApprovalsScene::readings);
        let brain = scene.brain.take().map(BrainScene::brain).transpose()?;
        let heads = scene.heads.take().as_ref().map(heads_of).transpose()?;
        let mut w = window_of_records(scene);
        if let Some(readings) = approvals {
            w.open.view = w.open.view.clone().with_approvals(readings);
        }
        if let Some(brain) = brain {
            w.open.view = w.open.view.clone().with_brain(brain);
        }
        if let Some(heads) = heads {
            w.open.view = w.open.view.clone().with_heads(heads);
        }
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
            trial: scene.records.trial,
            again: scene.records.again,
            revision: scene.records.revision,
            checkout: scene.records.checkout,
            no_trial: scene.records.no_trial,
            // THE SYSTEM THAT WAITS, as a scene says it: guarded unless the
            // scene says why no trial could be armed.
            staged: scene.records.staged.then(|| module::NextBoot {
                toplevel: String::new(),
                guarded: scene.records.no_trial.is_none(),
            }),
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

    /// The window a scene file describes, as `shoot` builds it — for a test.
    #[cfg(test)]
    pub(super) fn window_for_tests(asked: &Asked) -> Result<UpdateWindow> {
        let file = asked.scene.as_ref().context("a scene")?;
        let raw = std::fs::read_to_string(file)?;
        window_of(serde_json::from_str(&raw)?)
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
                r#"{"records": {"conflicts": 2,
                    "trial": "failed", "no-trial": "not-in-menu",
                    "hold": {"rev": "613862c", "since": "2026-10-05T12:00:00Z"}},
                    "battery": {"discharging": true, "percent": 20}, "keys": "SUPER s v"}"#,
            )
            .expect("a scene");
            let w = window_of(scene).expect("a window");
            let r = w.open.view.records();
            assert_eq!(r.conflicts, 2);
            assert_eq!(r.trial, Some(module::TrialOutcome::Failed));
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
    /// what it says of the update drawn, and a run this window asked for is
    /// followed in it: held for a metered way out, the question, the no
    /// first. The head is counted again either way: the status and the
    /// actions follow the brain.
    fn mind_arrived(&mut self, ui: &mut Ui, mind: ronoctua_brain::Mind) {
        let _ = self.open.heard_the_mind(mind, ui);
        self.measure(ui);
    }

    /// The head as this window draws it: the view's, with the window's name
    /// at the status's right end while the window wears no header — the
    /// one list the painting, the pointer and `measure` read.
    fn head(&self, ui: &Ui) -> Vec<TopLine> {
        let name = ui
            .hinted
            .is_hidden()
            .then(|| ui.texts.t(module::key::CMD_SYSTEM_UPDATE));
        self.open
            .view
            .top_lines_with(ui.columns, name.as_deref(), &ui.texts)
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
        let wrapped = self.head(ui).len();
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
        self.head(ui).get(n).and_then(|l| l.target)
    }

    /// The row a target is lit as under the pointer: a step by its index, a
    /// head line past the last step — one number, which the seats' `lit`
    /// carries.
    fn lit_row_of(&self, ui: &Ui, target: Target) -> Option<usize> {
        match target {
            Target::Step(n) => Some(n),
            Target::Page(n) => Some(PAGE_ROWS + n),
            other => self
                .head(ui)
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
    fn leader_alt_keys(&self) -> &'static [&'static [(&'static str, &'static str)]] {
        &[LEADER_ALT_KEYS]
    }

    /// EVERY STATE'S TABLE, kept from the keyboard: the view reads `u`, `a`
    /// and `p` whatever the update's state, while the line teaches each only
    /// where it means something — a physical `u` never starts an update,
    /// queued or not. Not while the file picker is up: its letters are its.
    fn barred_keys(&self, ui: &Ui) -> Vec<ronoctua_keymap::Key> {
        if self.open.view.picking() {
            return crate::chrome::shortcut_keys(&Content::hint(self, ui), self.leader_alt_keys());
        }
        crate::chrome::table_keys(
            OF_EVERY_STATE
                .iter()
                .flat_map(|t| t.iter())
                .chain(OF_THE_PAGE)
                .chain(OF_THE_FOLLOW)
                .chain(OF_THE_PARTS)
                .map(|r| r.0),
        )
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(&ui.texts)
    }

    /// EVERY ACTION SAYS WHAT IT DOES — what its line says under the actions
    /// (`Action::does`), under the pointer on the header and beside it in the
    /// footer — where the registry's sentence for its command is a label
    /// (`cmd-system-update`, `cmd-brain-abort`). Every other key, the
    /// registry's or the view's (`chrome::hint_sentence`).
    fn explains(&self, id: &str, texts: &ronoctua_i18n::Texts) -> Option<String> {
        match [
            Action::Start,
            Action::StartNow,
            Action::NotNow,
            Action::Stop,
            Action::Cache,
            Action::Follow,
            Action::Parts,
        ]
        .into_iter()
        .find(|a| a.id() == id)
        {
            Some(answer) => Some(texts.t(answer.does())),
            None => crate::chrome::hint_sentence(id, texts),
        }
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
        let heads = self.head(ui);
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
            // AN ACTION'S VALUE, and the window's name with no header.
            for piece in &head.beside {
                canvas
                    .spans
                    .push(span(piece.text.clone(), margin + piece.at, y, piece.color));
            }
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
            // answers, and no step wears the cursor's band — nor while a
            // gesture of the machine's, or an approval, is asked, nor while
            // the focus is on a line of the head.
            let band = if row == self.open.view.cursor() && self.open.view.on_the_steps() {
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
                    // WHAT THE KEY DOES, in the window's own words
                    // (`Content::explains`) — the same sentence the footer
                    // says beside it.
                    let texts = ui.texts.clone();
                    let now = ui
                        .hinted
                        .under(e.position.0, e.position.1)
                        .and_then(|id| Content::explains(self, id, &texts));
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
                // knows about. Over the steps it walks THE STEPS, and puts
                // the focus there: the arrows' list starts above them.
                //
                // AND IT DOES IT ONCE. `by` is LINES, which is what a viewport
                // moves; this half moves a CURSOR, and a cursor's unit is the
                // arrow's — one. Three steps per notch on a list of eight
                // would cross it in three turns of the wheel.
                let errand = self.open.view.wheel_steps(by > 0);
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
        // the door of the page of the apps that follow their releases, the
        // parts' five (its door and its four questions), the machine's three
        // gestures, which the records' rows ask, and the approval of a brain
        // module of the person's that waits, which they ask too.
        assert_eq!(COMMANDS.len(), 18);
        for what in crate::machine_gesture::Gesture::ALL {
            assert_eq!(order_of(what.command()), Some(Order::Machine(what)));
        }
        assert_eq!(order_of(crate::approval::APPROVE), Some(Order::Approve));
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
        assert_eq!(key("TAB"), Some(Press::Section));
        assert_eq!(key("S-TAB"), Some(Press::Section));
        assert_eq!(key("ESC"), Some(Press::Escape));
        assert_eq!(key("q"), Some(Press::Escape));
        assert_eq!(key("j"), Some(Press::Down));
        assert_eq!(key("k"), Some(Press::Up));
        assert_eq!(key("C-<down>"), Some(Press::OutputDown));
        for chord in ["C-u", "M-a", "C-p", "C-q", "M-j"] {
            assert_eq!(key(chord), None, "`{chord}` was taken for its letter");
        }
        for (taught, id) in OF_EVERY_STATE.iter().flat_map(|t| t.iter()) {
            assert!(
                key(taught).is_some(),
                "`{taught}` ({id}) is taught and reads as nothing"
            );
        }
    }

    /// WHAT A KEY ASKS FOR is the same errand in both doors: the arrows walk
    /// one list — the head's lines that act, then the steps —, the output
    /// moves on each door's own page, and the deeds are errands the door
    /// carries out with the same sentences.
    #[test]
    fn a_key_is_one_errand_in_both_doors() {
        let mut v = a_view();
        let last = v.steps().len() - 1;
        assert_eq!(v.focus(), Focus::Head(Target::Action(Action::Start)));
        assert_eq!(v.key(Press::Up), Errand::Moved);
        assert_eq!(v.focus(), Focus::Head(Target::Status));
        for _ in 0..v.steps().len() + 10 {
            v.key(Press::Down);
        }
        assert_eq!(v.focus(), Focus::Steps);
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

    /// FAILS IF: the arrows do not walk one list — the status, the actions,
    /// then the steps, and from the first step back up to the last line of
    /// the head —, `TAB` does not jump between the two keeping each one's
    /// place, the focus is kept by a line's place rather than by what it
    /// takes (a line that goes away gives it back to where it rests), or it
    /// rests by itself on an answer that changes a run.
    #[test]
    fn the_keys_walk_the_head_then_the_steps_and_tab_keeps_each_place() {
        let mut v = a_view();
        let head = v.head_targets();
        assert_eq!(
            head,
            [
                Target::Status,
                Target::Action(Action::Start),
                Target::Action(Action::Cache),
                Target::Action(Action::Follow),
                Target::Action(Action::Parts),
            ]
        );
        for want in &head[2..] {
            v.key(Press::Down);
            assert_eq!(v.focus(), Focus::Head(*want));
        }
        v.key(Press::Down);
        assert_eq!((v.focus(), v.cursor()), (Focus::Steps, 0), "below the head");
        v.key(Press::Up);
        assert_eq!(
            v.focus(),
            Focus::Head(Target::Action(Action::Parts)),
            "up from the first step"
        );
        // TAB: to the steps, where they were left — and back to the line of
        // the head it was on.
        v.key(Press::Section);
        v.key(Press::Down);
        v.key(Press::Down);
        assert_eq!((v.focus(), v.cursor()), (Focus::Steps, 2));
        v.key(Press::Section);
        assert_eq!(v.focus(), Focus::Head(Target::Action(Action::Parts)));
        v.key(Press::Section);
        assert_eq!((v.focus(), v.cursor()), (Focus::Steps, 2));
        // BY WHAT IT TAKES: on "update now", a countdown begins — the line is
        // gone, and the focus rests on the binary cache's row: never on the
        // countdown's answers, nor on a run's stop, by itself. A view that
        // opens on the queue rests there too: its "update now" refuses.
        v.key(Press::Section);
        assert_eq!(v.clicked(Target::Action(Action::Start)), Errand::Moved);
        let queued = Brain {
            queued: true,
            ..Brain::default()
        };
        for brain in [
            Brain {
                announced: true,
                ..queued
            },
            Brain {
                running: true,
                ..Brain::default()
            },
        ] {
            let resting = v.clone().with_brain(brain);
            assert_eq!(
                resting.focus(),
                Focus::Head(Target::Action(Action::Cache)),
                "{brain:?}"
            );
        }
        for brain in [
            queued,
            Brain {
                announced: true,
                ..queued
            },
            Brain {
                running: true,
                ..Brain::default()
            },
        ] {
            assert_eq!(
                a_view().with_brain(brain).focus(),
                Focus::Head(Target::Action(Action::Cache)),
                "{brain:?}: where the view opens"
            );
        }
        // A STATUS THAT IS NOT GITHUB'S ANSWER is no control: the arrows
        // pass it by.
        let failed = a_view().with_brain(Brain {
            failed: true,
            ..Brain::default()
        });
        assert_eq!(failed.status(), Status::Failed);
        assert_eq!(failed.head_targets()[0], Target::Action(Action::Start));
        // A QUESTION HOLDS THE CURSOR: no line of the head is walked, and
        // TAB goes nowhere.
        let mut asked = a_view().on_charge(flat());
        asked.key(Press::Start);
        assert!(asked.head_targets().is_empty());
        assert_eq!(asked.key(Press::Section), Errand::Nothing);
    }

    /// FAILS IF: the first click on the status line or an action does
    /// anything but put the focus there, the second (or `RET`) does not do
    /// what it does — ask GitHub again, start, answer the countdown, stop,
    /// turn to a page —, an action that cannot act now does instead of
    /// saying why, or a click on a step does not take the focus to the
    /// steps.
    #[test]
    fn the_second_click_and_ret_do_what_the_line_does() {
        // THE FOCUS ELSEWHERE FIRST: a click on the line the focus is on is
        // the second click already, as on any list's cursor row.
        let twice = |v: &mut View, target: Target| {
            let _ = v.clicked(Target::Step(0));
            assert_eq!(
                v.clicked(target),
                Errand::Moved,
                "{target:?}: the first click"
            );
            assert_eq!(v.focus(), Focus::Head(target));
            v.clicked(target)
        };
        let mut v = a_view();
        assert_eq!(twice(&mut v, Target::Status), Errand::Recheck);
        assert_eq!(v.key(Press::Enter), Errand::Recheck, "RET on the status");
        assert_eq!(twice(&mut v, Target::Action(Action::Start)), Errand::Start);
        assert_eq!(v.key(Press::Enter), Errand::Start);
        assert_eq!(twice(&mut v, Target::Action(Action::Cache)), Errand::Moved);
        assert!(v.cache_page().is_some(), "the second click opened no page");
        let mut v = a_view();
        assert_eq!(twice(&mut v, Target::Action(Action::Follow)), Errand::Moved);
        assert!(v.follow_page().is_some());
        let mut v = a_view();
        assert_eq!(twice(&mut v, Target::Action(Action::Parts)), Errand::Moved);
        assert!(v.parts_page().is_some());
        // THE COUNTDOWN'S TWO, and the run's stop.
        let mut v = a_view().with_brain(Brain {
            announced: true,
            queued: true,
            ..Brain::default()
        });
        assert_eq!(
            twice(&mut v, Target::Action(Action::StartNow)),
            Errand::Answer(true)
        );
        assert_eq!(
            twice(&mut v, Target::Action(Action::NotNow)),
            Errand::Answer(false)
        );
        assert_eq!(
            v.clicked(Target::Action(Action::Start)),
            Errand::Nothing,
            "a line that is not there"
        );
        let mut v = a_view().with_brain(Brain {
            running: true,
            ..Brain::default()
        });
        assert_eq!(
            twice(&mut v, Target::Action(Action::Stop)),
            Errand::Answer(false)
        );
        assert_eq!(v.key(Press::Enter), Errand::Answer(false));
        // QUEUED: "update now" is there, refusing, and says why.
        let mut v = a_view().with_brain(Brain {
            queued: true,
            ..Brain::default()
        });
        assert_eq!(
            twice(&mut v, Target::Action(Action::Start)),
            Errand::Say(module::key::SYSTEM_UPDATE_START_QUEUED)
        );
        // ON THE LINE THE FOCUS IS ON, the first click is the second: the
        // view opens on "update now".
        assert_eq!(
            a_view().clicked(Target::Action(Action::Start)),
            Errand::Start
        );
        // NOT ON A RUN'S ANSWER THE PERSON NEVER CHOSE. A view that opens on
        // a run stops nothing with its `RET` or its first click on "stop";
        // only the second, on the line the first chose, stops it.
        let running = Brain {
            running: true,
            ..Brain::default()
        };
        let mut v = a_view().with_brain(running);
        assert_ne!(v.key(Press::Enter), Errand::Answer(false), "RET stopped it");
        let mut v = a_view().with_brain(running);
        assert_eq!(v.clicked(Target::Action(Action::Stop)), Errand::Moved);
        assert_eq!(
            v.clicked(Target::Action(Action::Stop)),
            Errand::Answer(false)
        );
        // AND THE PERSON'S CHOICE ENDS WITH ITS RUN: the next run finds the
        // focus resting, not on its stop.
        let mut v = a_view();
        let _ = v.heard_the_brain(&a_mind(module::ID, &[]));
        assert_eq!(v.clicked(Target::Action(Action::Stop)), Errand::Moved);
        assert_eq!(v.focus(), Focus::Head(Target::Action(Action::Stop)));
        let _ = v.heard_the_brain(&a_mind("", &[]));
        let _ = v.heard_the_brain(&a_mind(module::ID, &[]));
        assert_ne!(v.focus(), Focus::Head(Target::Action(Action::Stop)));
        assert_ne!(v.key(Press::Enter), Errand::Answer(false), "RET stopped it");
        // THE STATUS LINE IS NO CONTROL while it says the brain's or the
        // machine's state: no click asks GitHub again.
        let mut v = a_view().with_brain(Brain {
            failed: true,
            ..Brain::default()
        });
        assert_eq!(v.clicked(Target::Status), Errand::Nothing);
        assert_eq!(v.clicked(Target::Status), Errand::Nothing);
        // A STEP: the focus goes to the steps.
        let mut v = a_view();
        assert_eq!(v.clicked(Target::Step(0)), Errand::Moved);
        assert_eq!((v.focus(), v.cursor()), (Focus::Steps, 0));
        assert_eq!(v.clicked(Target::Step(0)), Errand::Nothing);
        assert_eq!(v.wheel_steps(true), Errand::Moved);
        assert_eq!(v.cursor(), 1);
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
        let register = whole
            .iter()
            .position(|r| r.target == Some(Target::Step(0)))
            .expect("the steps are drawn");
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
        assert!(
            lit(&v).is_empty(),
            "a step wears the cursor while the focus is on the head"
        );
        v.key(Press::Section);
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
        for id in [START, SECTION, "system-update-output-end"] {
            assert!(
                words.contains(&ui.texts.t(id)),
                "`{id}` is not taught: {words}"
            );
        }
        assert!(w.header_keys(), "the header's drawer is not this window's");
    }

    /// FAILS IF: a key is taught where it means nothing — `u` while the
    /// update waits in the queue or runs, the countdown's answers with no
    /// countdown of this update, the stop with nothing of it running — or
    /// a key one of them reads is not kept from the keyboard in every
    /// state (a physical `u` must never start an update), or one of them is
    /// not one key after `LEADER Alt`.
    #[test]
    fn each_state_teaches_its_own_keys_and_the_keyboard_is_kept_from_all() {
        let ids = |v: &View| -> Vec<&str> { v.taught().iter().map(|(_, id)| *id).collect() };
        let state = |brain: Brain| a_view().with_brain(brain);
        let idle = a_view();
        assert!(ids(&idle).contains(&START));
        for absent in [PROCEED, ABORT, STOP] {
            assert!(!ids(&idle).contains(&absent), "idle teaches {absent}");
        }
        let queued = state(Brain {
            queued: true,
            ..Brain::default()
        });
        assert_eq!(queued.taught(), OF_THE_QUEUE);
        for absent in [START, PROCEED, ABORT, STOP] {
            assert!(!ids(&queued).contains(&absent), "queued teaches {absent}");
        }
        let counting = state(Brain {
            announced: true,
            queued: true,
            ..Brain::default()
        });
        assert_eq!(counting.taught(), OF_THE_COUNTDOWN);
        assert!(ids(&counting).contains(&PROCEED) && ids(&counting).contains(&ABORT));
        assert!(!ids(&counting).contains(&START) && !ids(&counting).contains(&STOP));
        let running = state(Brain {
            running: true,
            ..Brain::default()
        });
        assert_eq!(running.taught(), OF_THE_RUN);
        assert!(ids(&running).contains(&STOP));
        for absent in [START, PROCEED, ABORT] {
            assert!(!ids(&running).contains(&absent), "running teaches {absent}");
        }
        // EVERY STATE'S LETTERS ARE KEPT FROM THE KEYBOARD, whatever is
        // taught now.
        let union: Vec<ronoctua_keymap::Key> = ["u", "a", "p", "c", "f", "d", "G", "C-j"]
            .iter()
            .map(|n| ronoctua_keymap::Key::parse(n).expect("a key"))
            .collect();
        for brain in [
            Brain::default(),
            Brain {
                queued: true,
                ..Brain::default()
            },
            Brain {
                announced: true,
                queued: true,
                ..Brain::default()
            },
            Brain {
                running: true,
                ..Brain::default()
            },
        ] {
            let (mut w, ui) = a_window();
            w.open.view = w.open.view.clone().with_brain(brain);
            let barred = w.barred_keys(&ui);
            for k in &union {
                assert!(
                    barred.contains(k),
                    "{brain:?}: `{}` reaches the view",
                    k.notation()
                );
            }
            assert!(
                !barred.contains(&ronoctua_keymap::Key::simple(ronoctua_keymap::Sym::Down)),
                "an arrow is barred"
            );
        }
        // AND EVERY TABLE IS ONE KEY AFTER `LEADER Alt`.
        if let Err(wrong) = crate::chrome::leader_alt_census(OF_EVERY_STATE, &[LEADER_ALT_KEYS]) {
            panic!("{wrong}");
        }
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
        // ANOTHER TASK COUNTING DOWN is not this update's to answer: the
        // same sentence, and nothing sent.
        let mut other = a_mind("", &["brain-test-other"]);
        other.announcement = Some(ronoctua_brain::Announcement {
            task: "brain-test-other".to_owned(),
            total_seconds: 60,
            deadline: 100,
            why: "due".to_owned(),
        });
        let _ = w.open.heard_the_mind(other, &mut ui);
        for press in ["p", "a"] {
            ui.message = None;
            typed(&mut w, &mut ui, press);
            assert_eq!(
                ui.message.as_deref(),
                Some(ui.texts.t(crate::key::ECHO_BRAIN_IDLE).as_str()),
                "`{press}` answered another task's countdown"
            );
        }
        typed(&mut w, &mut ui, "ESC");
        assert!(ui.quit);
    }

    /// FAILS IF: a countdown's answer reaches another task than this update
    /// — its countdown skipped or put off, its run stopped — or does not
    /// reach this one: counting down (start now, not now) or running (stop).
    #[test]
    fn the_countdowns_answers_are_this_updates_alone() {
        let announce = |m: &mut ronoctua_brain::Mind, task: &str| {
            m.announcement = Some(ronoctua_brain::Announcement {
                task: task.to_owned(),
                total_seconds: 60,
                deadline: 100,
                why: "due".to_owned(),
            });
        };
        let mut ours = a_mind("", &[module::ID]);
        announce(&mut ours, module::ID);
        assert!(answers_this_update(&ours, true));
        assert!(answers_this_update(&ours, false));
        let mut theirs = a_mind("", &["brain-test-other"]);
        announce(&mut theirs, "brain-test-other");
        assert!(!answers_this_update(&theirs, true));
        assert!(!answers_this_update(&theirs, false));
        let running = a_mind(module::ID, &[]);
        assert!(!answers_this_update(&running, true), "nothing counts down");
        assert!(answers_this_update(&running, false));
        // OURS ANNOUNCED WHILE ANOTHER RUNS: the stop would be the other's.
        let mut busy = a_mind("brain-test-other", &[module::ID]);
        announce(&mut busy, module::ID);
        assert!(!answers_this_update(&busy, false));
        assert!(!answers_this_update(
            &a_mind("brain-test-other", &[]),
            false
        ));
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
    /// wraps it — the step's sentence and, under it, the command it runs —
    /// and the step rows start under the head's last line, where the pointer
    /// finds them too.
    ///
    /// THE DEFECT THIS CATCHES: the window cut the sentence with an ellipsis
    /// at its width, so the end of it — the trim's says what a rehearsal
    /// shows — was in the footer and never in the window.
    #[test]
    fn the_windows_second_register_is_wrapped_whole_and_the_rows_follow_it() {
        let (mut w, mut ui) = a_window();
        // A NARROW WINDOW, so the longest sentence wraps.
        ui.columns = 40;
        typed(&mut w, &mut ui, "TAB");
        let longest = (0..w.open.view.steps().len())
            .max_by_key(|&n| ui.texts.t(w.open.view.steps()[n].does_key).len())
            .expect("steps");
        for _ in 0..longest {
            typed(&mut w, &mut ui, "<down>");
        }
        assert_eq!(w.open.view.cursor(), longest);
        let wrapped = w.open.view.register_lines(ui.columns, &ui.texts);
        let step = &w.open.view.steps()[longest];
        let sentence = crate::chrome::note_lines(
            &w.open.view.explains(&ui.texts).expect("a sentence"),
            ui.columns,
        );
        assert!(
            sentence.len() > 1,
            "the longest sentence fits one line: {sentence:?}"
        );
        let words = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            words(&wrapped.join(" ")),
            words(&format!(
                "{} {}",
                w.open.view.explains(&ui.texts).expect("a sentence"),
                step.command
            )),
            "the register is not the whole sentence and the command"
        );
        // EVERY LINE OF IT IS IN THE HEAD, in order, and the head is what
        // the window counts.
        let head: Vec<String> = w.head(&ui).into_iter().map(|l| l.text).collect();
        assert!(
            head.windows(wrapped.len())
                .any(|run| run == wrapped.as_slice()),
            "{head:?}"
        );
        assert_eq!(usize::from(w.register), head.len());
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let middle = |line: u16| (f64::from(line) + 0.5) * line_px;
        let first = ui.first_of_list() + w.register;
        assert_eq!(w.at(&ui, 10.0, middle(first)), Some(w.step_offset));
        assert_eq!(
            w.at(&ui, 10.0, middle(first - 1)),
            None,
            "the head's last line is a step"
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
        typed(&mut w, &mut ui, "TAB");
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
            trial: Some(module::TrialOutcome::Failed),
            again: false,
            revision: Some("613862c5a0ff".to_owned()),
            no_trial: Some(module::NoTrial::NotInMenu),
            staged: Some(module::NextBoot {
                toplevel: "/nix/store/00000000000000000000000000000002-nixos-system-c".to_owned(),
                guarded: false,
            }),
            hold: Some(module::HeldAt {
                rev: "613862c5a0ff".to_owned(),
                since: "2026-10-05T12:00:00Z".to_owned(),
            }),
            checkout: true,
        }
    }

    /// WHAT THE MACHINE'S RECORDS SAY is on top of both doors, each only
    /// when it is there: the conflicts with the keys that open them (and a
    /// click that does), the boot trial in the machine's words, why there
    /// is none, and a hold with its short revision and its day.
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
            // A SYSTEM THAT WAITS, said by the status line — its only way
            // back by the machine's reason for arming no trial, below.
            &t.t(module::key::SYSTEM_UPDATE_STATUS_RESTART),
            &t.t(key::MACHINE_TRIAL_FAILED),
            &t.t(key::MACHINE_NO_TRIAL_NOT_IN_MENU),
            "613862c ",
            "2026-10-05",
            // A FLAKE THAT FOLLOWS A LOCAL CHECKOUT: nixpkgs alone moves.
            &t.t(module::key::SYSTEM_UPDATE_RECORDS_CHECKOUT),
        ] {
            assert!(said.contains(needle), "`{needle}` is not said: {said}");
        }
        assert!(
            !a_view()
                .with_records(
                    module::Records {
                        checkout: false,
                        ..every_record()
                    },
                    "SUPER s v".to_owned()
                )
                .top_lines(200, &t)
                .iter()
                .any(|l| l.text == t.t(module::key::SYSTEM_UPDATE_RECORDS_CHECKOUT)),
            "a release's machine said it follows a checkout"
        );
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
            region.contains(&t.t(key::MACHINE_TRIAL_FAILED)) && region.contains("SUPER s v"),
            "{region}"
        );
        // THE CONFLICTS' LINE: the first click selects it, the second opens
        // the belongings, as `RET` does (invariant 19).
        let mut v = v;
        assert_eq!(v.clicked(Target::Belongings), Errand::Moved);
        assert_eq!(v.focus(), Focus::Head(Target::Belongings));
        assert_eq!(v.clicked(Target::Belongings), Errand::OpenBelongings);
        assert_eq!(v.key(Press::Enter), Errand::OpenBelongings);
        // ONCE: the status line says the system that waits, so its record
        // line is not said under it — and a restart with no way back but the
        // boot menu is said by the machine's reason when it gives one, else
        // by the status line. A status that says something else (a run)
        // leaves the record to say it, its trial promised or not.
        let records_of = |guarded: bool, why: Option<module::NoTrial>, brain: Option<Brain>| {
            let mut r = every_record();
            r.staged = r.staged.map(|n| module::NextBoot { guarded, ..n });
            r.no_trial = why;
            let mut v = a_view().with_records(r, "SUPER s v".to_owned());
            if let Some(b) = brain {
                v = v.with_brain(b);
            }
            v.top_lines(200, &t)
                .into_iter()
                .map(|l| l.text)
                .collect::<Vec<String>>()
        };
        let staged = t.t(module::key::SYSTEM_UPDATE_RECORDS_STAGED);
        let unguarded = t.t(module::key::SYSTEM_UPDATE_RECORDS_STAGED_UNGUARDED);
        let restart = t.t(module::key::SYSTEM_UPDATE_STATUS_RESTART);
        let menu_only = t.t(module::key::SYSTEM_UPDATE_STATUS_RESTART_UNGUARDED);
        let why = Some(module::NoTrial::NotInMenu);
        for (guarded, why, status) in [
            (true, None, &restart),
            (false, why, &restart),
            (false, None, &menu_only),
        ] {
            let said = records_of(guarded, why, None);
            assert!(said.contains(status), "{guarded} {why:?}: {said:?}");
            assert!(
                !said.contains(&staged) && !said.contains(&unguarded),
                "said twice: {said:?}"
            );
        }
        let running = Some(Brain {
            running: true,
            ..Brain::default()
        });
        let said = records_of(true, None, running);
        assert!(said.contains(&staged) && !said.contains(&unguarded));
        let said = records_of(false, why, running);
        assert!(said.contains(&unguarded) && !said.contains(&staged));
        // AND NOTHING THERE IS NOTHING SAID: the head ends with the register.
        let quiet = a_view();
        let head: Vec<String> = quiet
            .top_lines(200, &t)
            .into_iter()
            .map(|l| l.text)
            .collect();
        let register = quiet.register_lines(200, &t);
        assert_eq!(
            head[head.len() - register.len()..],
            register[..],
            "something is said under the register: {head:?}"
        );
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
        for _ in 0..2 {
            let errand = w.open.view.clicked(Target::Belongings);
            w.apply(&mut ui, errand);
        }
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
    /// And root's applier's, on the step whose refusals are its own.
    #[test]
    fn a_refusal_says_the_machines_reason() {
        let t = texts();
        for why in module::MachineRefusal::ALL {
            let k = module::machine_refusal_key(why.id())
                .expect("every reason of the machine has a sentence");
            assert!(!t.t(k).starts_with('['), "{k}");
        }
        assert_eq!(
            module::machine_refusal_key("a-word-the-machine-never-says"),
            None
        );
        let speaking = |whose| {
            module::steps()
                .iter()
                .find(|s| module::refuses(s.id) == Some(whose))
                .expect("a step that speaks it")
        };
        let step = speaking(module::Refuses::Machine);
        let said = cursor_explains(step, &Verdict::Refused("build-failed".to_owned()), &t);
        assert!(
            said.contains(&t.t(module::machine_refusal_said(
                module::MachineRefusal::BuildFailed
            ))),
            "{said}"
        );
        let plain = cursor_explains(step, &Verdict::Refused("nonsense".to_owned()), &t);
        assert_eq!(plain, cursor_explains(step, &Verdict::Failed, &t));

        // AND ROOT'S APPLIER'S REASON, on the step that hands it the
        // person's declarations — every one it refuses a staging with, in
        // its own sentence; the machine's word is not its.
        let stage = speaking(module::Refuses::Applier);
        for why in ronoctua_person::tree::TreeWhy::ALL {
            let Some(k) = module::applier_refusal_key(why.id()) else {
                continue;
            };
            assert!(!t.t(k).starts_with('['), "{k}");
            let said = cursor_explains(stage, &Verdict::Refused(why.id().to_owned()), &t);
            assert!(said.contains(&t.t(k)), "{said}");
        }
        assert_eq!(module::refused_key(stage.id, "build-failed"), None);
        // A STEP WHOSE PROGRAM'S WORDS ARE NOT KNOWN says only that it
        // failed, whatever the word.
        let mute = module::steps()
            .iter()
            .find(|s| module::refuses(s.id).is_none())
            .expect("a step with no vocabulary");
        assert_eq!(module::refused_key(mute.id, "build-failed"), None);
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
            folds: std::collections::BTreeMap::new(),
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
        // THE QUESTION, from its first line: the status and the actions
        // stand above it.
        let asked = crate::chrome::note_lines(&t.t(module::key::SYSTEM_UPDATE_ASK_METERED), 80);
        let from = head
            .iter()
            .position(|l| l.text == asked[0])
            .expect("the question is asked");
        let said: String = head[from..]
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

    /// The follow page over two AppImages, the person following, the
    /// machine built following them.
    fn on_the_follow_page() -> crate::follow::Readings {
        crate::follow::Readings {
            copy: crate::follow::Choice::On,
            machine: Some(true),
            waiting: None,
            apps: ["CADCraft", "FigCraft"]
                .into_iter()
                .map(|name| crate::follow::App {
                    id: name.to_lowercase(),
                    name: name.to_owned(),
                    here: true,
                    pin: ronoctua_person::reports::PinFrom::Followed,
                    version: "0.9.0".to_owned(),
                    followed: Some("0.9.0".to_owned()),
                    arrival: crate::follow::Arrival::Here,
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

    // -----------------------------------------------------------------------
    // THE MACHINE'S OWN GESTURES (`crate::machine_gesture`)
    // -----------------------------------------------------------------------

    use crate::machine_gesture::Gesture;

    /// What each record's head line takes, with its text.
    fn takes(v: &View, t: &ronoctua_i18n::Texts) -> Vec<(String, Option<Target>)> {
        v.top_lines(200, t)
            .into_iter()
            .map(|l| (l.text, l.target))
            .collect()
    }

    /// FAILS IF: a record's row the keys walk onto — the conflicts, a
    /// gesture, a module that waits — does not wear the cursor's colour,
    /// one of the lines its sentence is wrapped to stays dark, or another
    /// line wears it too: the focus would be nowhere on the screen, and
    /// `RET` would act on a line nobody could see was chosen.
    #[test]
    fn a_records_row_under_the_focus_wears_the_cursor_on_every_line() {
        let t = texts();
        let mut v = a_view()
            .with_records(
                module::Records {
                    conflicts: 2,
                    ..failed_unheld()
                },
                "SUPER s v".to_owned(),
            )
            .with_approvals(two_waiting());
        let cursor = crate::chrome::Kind::Setting.tones(true).0;
        let records = [
            Target::Belongings,
            Target::Gesture(crate::machine_gesture::Gesture::TryAgain),
            Target::Gesture(crate::machine_gesture::Gesture::Hold),
            Target::Approve(0),
        ];
        assert_eq!(v.head_targets()[5..], records, "the walk's records");
        // FROM "update now", WHERE IT RESTS, to the last page's row.
        for _ in 0..3 {
            v.key(Press::Down);
        }
        let mut wrapped = false;
        for want in records {
            v.key(Press::Down);
            assert_eq!(v.focus(), Focus::Head(want));
            // NARROW, so every sentence is wrapped to more than one line.
            let head = v.top_lines(30, &t);
            let lit: Vec<Option<Target>> = head
                .iter()
                .filter(|l| l.color == cursor)
                .map(|l| l.target)
                .collect();
            let lines = head.iter().filter(|l| l.target == Some(want)).count();
            wrapped |= lines > 1;
            assert_eq!(lit, vec![Some(want); lines.max(1)], "{want:?}: {head:?}");
            // AND THE FOOTER'S REGION, the same lines.
            let rows = v.region_rows(0, head.len(), 30, &t);
            assert!(
                rows.iter()
                    .filter(|r| r.target == Some(want))
                    .all(|r| r.fields.first().is_some_and(|f| f.color == cursor)),
                "{want:?}: the region's lines are dark"
            );
        }
        assert!(wrapped, "no record's sentence was wrapped to two lines");
    }

    /// A machine whose last trial failed, not held, on a revision.
    fn failed_unheld() -> module::Records {
        module::Records {
            trial: Some(module::TrialOutcome::Failed),
            revision: Some("613862c5a0ff613862c5a0ff".to_owned()),
            ..module::Records::default()
        }
    }

    /// FAILS IF: a gesture's row is drawn where the records do not offer it
    /// — a try-again with no failed trial or once it was asked, a hold on a
    /// machine with no revision or already held, a let-go of no hold — or
    /// missing where they do; a row is drawn without the click it takes; the
    /// hold's own sentence does not let go; or a row says a key, a revision
    /// or a word that is not there, in either language.
    #[test]
    fn the_records_offer_each_gesture_where_its_sentence_is() {
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let offered = |r: module::Records| {
                let v = a_view().with_records(r, String::new());
                let lines = takes(&v, &t);
                for (text, _) in &lines {
                    assert!(!text.contains('['), "{language}: {text}");
                }
                let mut gestures: Vec<Gesture> = lines
                    .iter()
                    .filter_map(|(_, target)| match target {
                        Some(Target::Gesture(g)) => Some(*g),
                        _ => None,
                    })
                    .collect();
                gestures.dedup();
                (gestures, lines)
            };
            // FAILED, NOT HELD: try it again, and hold the revision it has.
            let (gestures, lines) = offered(failed_unheld());
            assert_eq!(gestures, [Gesture::TryAgain, Gesture::Hold], "{language}");
            let failed_at = lines
                .iter()
                .position(|(text, _)| *text == t.t(key::MACHINE_TRIAL_FAILED))
                .expect("the failure is said");
            assert_eq!(
                lines[failed_at + 1].1,
                Some(Target::Gesture(Gesture::TryAgain)),
                "the try-again is not under the failure's sentence"
            );
            let hold = lines
                .iter()
                .find(|(_, target)| *target == Some(Target::Gesture(Gesture::Hold)))
                .expect("a hold row");
            assert!(hold.0.contains("613862c"), "{}", hold.0);
            assert!(
                !hold.0.contains("613862c5"),
                "the revision was not shortened"
            );
            // ASKED ALREADY: no row, the sentence that it is tried again, with
            // the view's own key that updates now.
            let (gestures, lines) = offered(module::Records {
                again: true,
                ..failed_unheld()
            });
            assert_eq!(gestures, [Gesture::Hold]);
            let asked = t.t_with(key::MACHINE_TRY_AGAIN_ASKED, &[("keys", "u")]);
            assert!(
                lines
                    .iter()
                    .any(|(text, target)| *text == asked && target.is_none()),
                "{language}: {lines:?}"
            );
            // HELD: the hold's sentence and the row under it let go; nothing
            // offers to hold.
            let (gestures, lines) = offered(module::Records {
                trial: Some(module::TrialOutcome::Good),
                hold: Some(module::HeldAt {
                    rev: "613862c5a0ff".to_owned(),
                    since: "2026-10-05T12:00:00Z".to_owned(),
                }),
                ..failed_unheld()
            });
            assert_eq!(gestures, [Gesture::LetGo]);
            let letting_go = lines
                .iter()
                .filter(|(_, target)| *target == Some(Target::Gesture(Gesture::LetGo)))
                .count();
            assert!(
                letting_go >= 2,
                "{language}: the hold's sentence does not let go: {lines:?}"
            );
            // NO REVISION, NOTHING FAILED: nothing is offered at all.
            let (gestures, _) = offered(module::Records::default());
            assert!(gestures.is_empty(), "{gestures:?}");
        }
    }

    /// FAILS IF: a click on a gesture's row does anything but ask, there —
    /// its question with the `no` first and the cursor born on it; the
    /// first click on an answer answers; another gesture takes a click
    /// while one is asked or waits for its verdict; a key does not walk and
    /// answer it as the run's question does; or the footer's region draws
    /// another question than the window.
    #[test]
    fn a_gesture_asks_first_with_the_no_first_and_the_second_click_answers() {
        let t = texts();
        let mut v = a_view().with_records(failed_unheld(), String::new());
        assert_eq!(v.clicked(Target::Gesture(Gesture::TryAgain)), Errand::Moved);
        let lines = v.top_lines(200, &t);
        let question = lines
            .iter()
            .position(|l| l.text == t.t(key::MACHINE_TRY_AGAIN_QUESTION))
            .expect("the question is asked");
        assert_eq!(
            lines[question + 1].target,
            Some(Target::GestureAnswer(false))
        );
        assert_eq!(
            lines[question + 1].color,
            tokens::color::TEXT_HERE_ACTS,
            "the cursor is not born on no"
        );
        assert_eq!(
            lines[question + 2].target,
            Some(Target::GestureAnswer(true))
        );
        assert!(
            !lines
                .iter()
                .any(|l| matches!(l.target, Some(Target::Gesture(_)))),
            "another gesture takes a click while one is asked"
        );
        // THE FOOTER'S REGION ASKS THE SAME, its cursor on the answer — and no
        // step wears the cursor meanwhile.
        let rows = v.region_rows(0, usize::MAX, 200, &t);
        let cursor = v.region_cursor(200, &t);
        assert_eq!(rows[cursor].target, Some(Target::GestureAnswer(false)));
        let accent = crate::chrome::Kind::ListedFact.tones(true).0;
        assert!(
            !rows
                .iter()
                .any(|r| matches!(r.target, Some(Target::Step(_)))
                    && r.fields.iter().any(|f| f.color == accent)),
            "a step wears the cursor while a gesture is asked"
        );
        // THE BORN CURSOR IS THE SCREEN'S: one click picks, the next says no.
        assert_eq!(v.clicked(Target::GestureAnswer(false)), Errand::Moved);
        assert!(v.gesture_question().is_some(), "the first click answered");
        assert_eq!(v.clicked(Target::GestureAnswer(false)), Errand::Moved);
        assert!(v.gesture_question().is_none(), "no did not close it");
        // YES: picked, then answered — and the machine is asked.
        assert_eq!(v.clicked(Target::Gesture(Gesture::TryAgain)), Errand::Moved);
        assert_eq!(v.clicked(Target::GestureAnswer(true)), Errand::Moved);
        assert_eq!(
            v.clicked(Target::GestureAnswer(true)),
            Errand::Machine(Gesture::TryAgain)
        );
        // WAITING FOR THE VERDICT, no row takes a click.
        assert!(
            !takes(&v, &t)
                .iter()
                .any(|(_, x)| matches!(x, Some(Target::Gesture(_)))),
            "a row takes a click while the verdict is awaited"
        );
        assert_eq!(v.clicked(Target::Gesture(Gesture::Hold)), Errand::Nothing);
        v.gesture_heard();
        // BY KEY: down picks yes, `RET` answers; `Esc` is the no.
        assert_eq!(v.clicked(Target::Gesture(Gesture::Hold)), Errand::Moved);
        assert_eq!(v.key(Press::Down), Errand::Moved);
        assert_eq!(v.key(Press::Enter), Errand::Machine(Gesture::Hold));
        v.gesture_heard();
        assert_eq!(v.ask_gesture(Gesture::Hold), Errand::Moved);
        assert_eq!(v.key(Press::Escape), Errand::Moved);
        assert!(v.gesture_question().is_none());
        // BY NAME, only what the records offer — or why not.
        assert_eq!(
            v.ask_gesture(Gesture::LetGo),
            Errand::Say(key::MACHINE_SAID_NOT_HELD)
        );
        let mut quiet = a_view();
        assert_eq!(
            quiet.ask_gesture(Gesture::TryAgain),
            Errand::Say(module::machine_refusal_said(
                module::MachineRefusal::NoFailedTrial
            ))
        );
        assert_eq!(
            quiet.ask_gesture(Gesture::Hold),
            Errand::Say(key::MACHINE_SAID_NOTHING_TO_HOLD)
        );
    }

    /// FAILS IF: the window does not take a gesture's row where it draws it,
    /// or a photograph's yes reaches root instead of being kept.
    #[test]
    fn a_photographs_gesture_is_asked_where_it_is_drawn_and_reaches_nothing() {
        let mut w = photographed(&[], "");
        w.open.view = w
            .open
            .view
            .clone()
            .with_records(failed_unheld(), String::new());
        let mut ui = Ui::for_tests(texts());
        ui.lines = 40;
        w.measure(&ui);
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let at = |w: &UpdateWindow, ui: &Ui, wanted: Target| {
            (0..ui.lines)
                .map(|line| (f64::from(line) + 0.5) * line_px)
                .find(|y| w.target_at(ui, 40.0, *y) == Some(wanted))
                .expect("the row is taken where it is drawn")
        };
        let y = at(&w, &ui, Target::Gesture(Gesture::TryAgain));
        let errand = w
            .open
            .view
            .clicked(w.target_at(&ui, 40.0, y).expect("a target"));
        w.apply(&mut ui, errand);
        for _ in 0..2 {
            let y = at(&w, &ui, Target::GestureAnswer(true));
            let errand = w
                .open
                .view
                .clicked(w.target_at(&ui, 40.0, y).expect("a target"));
            w.apply(&mut ui, errand);
        }
        let kept = w.would_have();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert!(kept[0].contains(Gesture::TryAgain.word()), "{kept:?}");
        assert!(ui.orders().is_empty(), "the footer was told");
    }

    // -----------------------------------------------------------------------
    // THE PERSON'S BRAIN MODULES THAT WAIT (`crate::approval`)
    // -----------------------------------------------------------------------

    const TEA_HASH: &str = "sha256-bsxxoKoBi/zTX82E9vCaao7mCkH4nLxO3mU4EoeSeO8=";

    /// Two modules that wait: `my-tea`, to approve, and `my-cake`, approved
    /// already at the hash the machine reported.
    fn two_waiting() -> crate::approval::Readings {
        let waiting = |id: &str, hash: &str| crate::approval::Waiting {
            id: id.to_owned(),
            hash: hash.to_owned(),
        };
        crate::approval::Readings {
            waiting: vec![
                waiting("my-tea", TEA_HASH),
                waiting("my-cake", "sha256-cake"),
            ],
            approved: [("my-cake".to_owned(), "sha256-cake".to_owned())].into(),
        }
    }

    /// FAILS IF: a module that waits is not said with the hash an approval
    /// must name and the row under it that asks; one approved as shown is
    /// offered again instead of said to enter with the next update; or a
    /// sentence lacks its words, in either language.
    #[test]
    fn the_records_say_each_module_that_waits_with_its_hash() {
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let v = a_view().with_approvals(two_waiting());
            let lines = takes(&v, &t);
            for (text, _) in &lines {
                assert!(!text.contains('['), "{language}: {text}");
            }
            let said = lines
                .iter()
                .position(|(text, _)| text.contains("my-tea") && text.contains(TEA_HASH))
                .expect("the module and its hash are said");
            assert_eq!(lines[said].1, None, "the sentence is not the control");
            let rows: Vec<&(String, Option<Target>)> = lines
                .iter()
                .filter(|(_, target)| matches!(target, Some(Target::Approve(_))))
                .collect();
            assert_eq!(rows.len(), 1, "{language}: {lines:?}");
            assert_eq!(rows[0].1, Some(Target::Approve(0)));
            assert!(rows[0].0.contains("my-tea"), "{language}: {}", rows[0].0);
            assert!(
                lines.iter().any(|(text, target)| *text
                    == t.t_with(key::MACHINE_APPROVE_APPROVED, &[("id", "my-cake")])
                    && target.is_none()),
                "{language}: the approved module is not said to enter: {lines:?}"
            );
        }
        let none = takes(&a_view(), &texts());
        assert!(
            !none
                .iter()
                .any(|(_, target)| matches!(target, Some(Target::Approve(_)))),
            "{none:?}"
        );
    }

    /// FAILS IF: a click on a module's row does anything but ask, there —
    /// the no first and the cursor born on it —; the first click on an
    /// answer answers; a yes is not the module and the hash shown; another
    /// row takes a click while one is asked or waits for its verdict; or the
    /// name does not ask the same question, or says none waits when one
    /// does.
    #[test]
    fn an_approval_asks_first_with_the_no_first_and_the_second_click_answers() {
        let t = texts();
        let mut v = a_view()
            .with_records(failed_unheld(), String::new())
            .with_approvals(two_waiting());
        assert_eq!(
            v.clicked(Target::Approve(1)),
            Errand::Nothing,
            "approved already"
        );
        assert_eq!(v.clicked(Target::Approve(0)), Errand::Moved);
        let q = v.approval_question().expect("asked");
        assert_eq!(q.of.id, "my-tea");
        assert!(!q.yes(), "the cursor is not born on no");
        let lines = takes(&v, &t);
        assert!(
            lines.iter().any(|(text, _)| *text
                == t.t_with(
                    key::MACHINE_APPROVE_QUESTION,
                    &[("id", "my-tea"), ("hash", TEA_HASH)]
                )),
            "{lines:?}"
        );
        assert!(
            !lines
                .iter()
                .any(|(_, x)| matches!(x, Some(Target::Gesture(_) | Target::Approve(_)))),
            "another row takes a click while an approval is asked"
        );
        let rows = v.region_rows(0, 200, 100, &t);
        let cursor = v.region_cursor(100, &t);
        assert_eq!(rows[cursor].target, Some(Target::ApproveAnswer(false)));
        assert_eq!(v.clicked(Target::ApproveAnswer(true)), Errand::Moved);
        assert!(v.approval_question().is_some(), "the first click answered");
        let tea = crate::approval::Waiting {
            id: "my-tea".to_owned(),
            hash: TEA_HASH.to_owned(),
        };
        assert_eq!(
            v.clicked(Target::ApproveAnswer(true)),
            Errand::Approve(tea.clone())
        );
        assert!(
            !takes(&v, &t)
                .iter()
                .any(|(_, x)| matches!(x, Some(Target::Gesture(_) | Target::Approve(_)))),
            "a row takes a click while an approval waits for its verdict"
        );
        assert_eq!(v.clicked(Target::Approve(0)), Errand::Nothing);
        v.approval_heard();
        // THE KEYS: no, then Esc is the no.
        assert_eq!(v.clicked(Target::Approve(0)), Errand::Moved);
        assert_eq!(v.key(Press::Down), Errand::Moved);
        assert!(
            v.approval_question()
                .is_some_and(crate::approval::Question::yes)
        );
        assert_eq!(v.key(Press::Escape), Errand::Moved);
        assert!(v.approval_question().is_none(), "Esc did not close it");
        // BY NAME: the one named, the first, or none.
        assert_eq!(v.ask_approval(Some("my-tea")), Errand::Moved);
        assert_eq!(v.key(Press::Enter), Errand::Moved, "the no answered");
        assert_eq!(v.ask_approval(None), Errand::Moved);
        assert_eq!(v.approval_question().map(|q| q.of.clone()), Some(tea));
        assert_eq!(v.key(Press::Escape), Errand::Moved);
        assert_eq!(
            v.ask_approval(Some("my-cake")),
            Errand::Say(key::MACHINE_APPROVE_SAID_NONE),
            "an approved module was asked again"
        );
        assert_eq!(
            a_view().ask_approval(None),
            Errand::Say(key::MACHINE_APPROVE_SAID_NONE)
        );
    }

    /// FAILS IF: the window does not take a module's row and its answers
    /// where it draws them, or a photograph's yes reaches root instead of
    /// being kept.
    #[test]
    fn a_photographs_approval_is_asked_where_it_is_drawn_and_reaches_nothing() {
        let mut w = photographed(&[], "");
        w.open.view = w.open.view.clone().with_approvals(two_waiting());
        let mut ui = Ui::for_tests(texts());
        ui.lines = 40;
        w.measure(&ui);
        let line_px = f64::from(tokens::metrics::line_height(tokens::text::size()));
        let at = |w: &UpdateWindow, ui: &Ui, wanted: Target| {
            (0..ui.lines)
                .map(|line| (f64::from(line) + 0.5) * line_px)
                .find(|y| w.target_at(ui, 40.0, *y) == Some(wanted))
                .expect("the row is taken where it is drawn")
        };
        let y = at(&w, &ui, Target::Approve(0));
        let errand = w
            .open
            .view
            .clicked(w.target_at(&ui, 40.0, y).expect("a target"));
        w.apply(&mut ui, errand);
        for _ in 0..2 {
            let y = at(&w, &ui, Target::ApproveAnswer(true));
            let errand = w
                .open
                .view
                .clicked(w.target_at(&ui, 40.0, y).expect("a target"));
            w.apply(&mut ui, errand);
        }
        let kept = w.would_have();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert!(
            kept[0].contains("my-tea") && kept[0].contains(TEA_HASH),
            "{kept:?}"
        );
        assert!(ui.orders().is_empty(), "the footer was told");
    }

    // -----------------------------------------------------------------------
    // THE TOP: THE STATUS, THE ACTIONS
    // -----------------------------------------------------------------------

    /// What GitHub said: each input moved, still, or refused.
    fn heard(
        distro: Option<Result<bool, ronoctua_software::Refusal>>,
        nixpkgs: Option<Result<bool, ronoctua_software::Refusal>>,
    ) -> Heads {
        use ronoctua_software::heads::{Branch, Head, Heard, Input};
        let locked = "0000000000000000000000000000000000000000";
        let one = |input: Input, said: Result<bool, ronoctua_software::Refusal>| Heard {
            input,
            head: match said {
                Ok(moved) => Ok(Head {
                    branch: Branch {
                        input,
                        owner: "o".to_owned(),
                        repo: "r".to_owned(),
                        branch: "b".to_owned(),
                        locked: locked.to_owned(),
                    },
                    sha: if moved {
                        "1111111111111111111111111111111111111111".to_owned()
                    } else {
                        locked.to_owned()
                    },
                }),
                Err(refusal) => Err(ronoctua_software::heads::Unlearnt::Asked(
                    ronoctua_software::Failure::refused(refusal),
                )),
            },
        };
        Heads::Came(Ok(distro
            .map(|d| one(Input::Distro, d))
            .into_iter()
            .chain(nixpkgs.map(|n| one(Input::Nixpkgs, n)))
            .collect()))
    }

    /// FAILS IF: the status line's precedence is not the brain's run
    /// (running, counting down, queued and why), then a system that waits
    /// for the restart, a run that failed, a hold — and only then what
    /// GitHub said: an input that moved is an update whatever another could
    /// not answer, none moved and all answering is up to date, none moved
    /// and one refusing is "could not check" with its reason, never "up to
    /// date". Or a stopped run reads as a failed one.
    #[test]
    fn the_status_says_the_first_thing_that_is_true() {
        use ronoctua_brain::Hold;
        use ronoctua_software::Refusal;
        use ronoctua_software::heads::Unlearnt;
        let records = module::Records::default();
        let staged = module::Records {
            staged: Some(module::NextBoot {
                toplevel: String::new(),
                guarded: true,
            }),
            ..module::Records::default()
        };
        let held = module::Records {
            hold: Some(module::HeldAt {
                rev: "613862c".to_owned(),
                since: "2026-10-05T12:00:00Z".to_owned(),
            }),
            ..module::Records::default()
        };
        let up = heard(Some(Ok(false)), Some(Ok(false)));
        let brain = |b: Brain| Some(b);
        let all = Brain {
            running: true,
            announced: true,
            queued: true,
            hold: Some(Hold::Battery),
            failed: true,
            worked: None,
            next: 0,
        };
        assert_eq!(
            status_of(brain(all).as_ref(), &staged, Some(&up)),
            Status::Running
        );
        let counting = Brain {
            running: false,
            ..all
        };
        assert_eq!(
            status_of(Some(&counting), &staged, Some(&up)),
            Status::CountingDown
        );
        let queued = Brain {
            announced: false,
            ..counting
        };
        assert_eq!(
            status_of(Some(&queued), &staged, Some(&up)),
            Status::Queued(Some(Hold::Battery))
        );
        let failed = Brain {
            queued: false,
            ..queued
        };
        assert_eq!(
            status_of(Some(&failed), &staged, Some(&up)),
            Status::Restart { menu_only: false }
        );
        assert_eq!(status_of(Some(&failed), &held, Some(&up)), Status::Failed);
        assert_eq!(status_of(None, &held, Some(&up)), Status::Held);
        // GITHUB'S ANSWER.
        assert_eq!(status_of(None, &records, None), Status::Unasked);
        assert_eq!(
            status_of(None, &records, Some(&Heads::Asking)),
            Status::Checking
        );
        assert_eq!(status_of(None, &records, Some(&up)), Status::UpToDate);
        for (heads, want) in [
            (heard(Some(Ok(true)), Some(Ok(false))), Moved::Distro),
            (heard(Some(Ok(false)), Some(Ok(true))), Moved::Nixpkgs),
            (heard(Some(Ok(true)), Some(Ok(true))), Moved::Both),
            (heard(None, Some(Ok(true))), Moved::Nixpkgs),
            (
                heard(Some(Err(Refusal::Offline)), Some(Ok(true))),
                Moved::Nixpkgs,
            ),
        ] {
            assert_eq!(
                status_of(None, &records, Some(&heads)),
                Status::Available(want)
            );
        }
        for (heads, why) in [
            (
                heard(Some(Ok(false)), Some(Err(Refusal::Offline))),
                Unchecked::Offline,
            ),
            (
                heard(Some(Err(Refusal::RateLimited)), Some(Ok(false))),
                Unchecked::RateLimited,
            ),
            (
                heard(Some(Err(Refusal::ToolFailed)), None),
                Unchecked::Other,
            ),
            (Heads::Came(Ok(Vec::new())), Unchecked::Nothing),
            (
                Heads::Came(Err(ronoctua_software::heads::Unread::NoLock)),
                Unchecked::NoLock,
            ),
            (
                Heads::Came(Err(ronoctua_software::heads::Unread::Unreadable)),
                Unchecked::Unreadable,
            ),
        ] {
            assert_eq!(
                status_of(None, &records, Some(&heads)),
                Status::Unchecked(why)
            );
        }
        // NEVER GITHUB'S FAULT, and never "up to date", what nobody asked:
        // a node of this machine's lock it does not read, and an input the
        // update moves with no branch on GitHub — the installer's tree —,
        // whatever the other input says.
        let unlearnt = |distro: Unlearnt, nixpkgs: bool| {
            let Heads::Came(Ok(mut all)) = heard(None, Some(Ok(nixpkgs))) else {
                unreachable!("a test's heads came");
            };
            all.insert(
                0,
                ronoctua_software::heads::Heard {
                    input: ronoctua_software::heads::Input::Distro,
                    head: Err(distro),
                },
            );
            Heads::Came(Ok(all))
        };
        assert_eq!(
            status_of(None, &records, Some(&unlearnt(Unlearnt::Unasked, false))),
            Status::Unchecked(Unchecked::Unasked(ronoctua_software::heads::Input::Distro)),
            "an input nobody asked about let the other say up to date"
        );
        assert_eq!(
            status_of(None, &records, Some(&unlearnt(Unlearnt::Lock, false))),
            Status::Unchecked(Unchecked::Unreadable),
            "this machine's lock was said to be GitHub's fault"
        );
        assert_eq!(
            status_of(None, &records, Some(&unlearnt(Unlearnt::Unasked, true))),
            Status::Available(Moved::Nixpkgs),
            "a moved input is an update whatever the other is"
        );
        // EVERY SENTENCE IS THERE, in both languages, with its date.
        let statuses = [
            Status::Running,
            Status::CountingDown,
            Status::Queued(None),
            Status::Restart { menu_only: false },
            Status::Restart { menu_only: true },
            Status::Failed,
            Status::Held,
            Status::Checking,
            Status::Available(Moved::Both),
            Status::UpToDate,
            Status::Unchecked(Unchecked::RateLimited),
            Status::Unchecked(Unchecked::Unasked(ronoctua_software::heads::Input::Distro)),
            Status::Unchecked(Unchecked::Unasked(ronoctua_software::heads::Input::Nixpkgs)),
            Status::Unasked,
        ]
        .into_iter()
        .chain(
            [
                Hold::Ram,
                Hold::Cpu,
                Hold::Battery,
                Hold::Busy,
                Hold::Network,
                Hold::Asleep,
            ]
            .map(|h| Status::Queued(Some(h))),
        );
        let zone = ronoctua_files::when::Zone::utc();
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for status in statuses.clone() {
                let said = status.sentence(Some(1_762_179_600), &zone, &t);
                assert!(!said.contains('['), "{language}: {status:?}: {said}");
                assert!(said.contains("2025"), "{language}: no date: {said}");
            }
        }
        // A STOPPED RUN is the person's decision, not a failure — and what
        // worked last is the newest that did.
        let record = |kind, why: &str, ended| ronoctua_brain::RunRecord {
            task: module::ID.to_owned(),
            ended,
            kind,
            why: why.to_owned(),
            ..ronoctua_brain::RunRecord::default()
        };
        let mut mind = a_mind_with(false, false, None, 0);
        if let Some(task) = mind.task_mut(module::ID) {
            use ronoctua_brain::task::OutcomeKind::{Done, Failed};
            task.last = Some(record(Failed, ABORTED, 30));
            task.history = vec![
                record(Failed, ABORTED, 30),
                record(Done, "", 20),
                record(Failed, "", 10),
            ];
        }
        let b = Brain::of(&mind);
        assert!(!b.failed, "a stopped run read as a failure");
        assert_eq!(b.worked, Some(20));
        if let Some(task) = mind.task_mut(module::ID) {
            task.history.remove(1);
        }
        assert!(Brain::of(&mind).failed);
    }

    /// FAILS IF: what GitHub said is not asked again once the machine's
    /// lock moved under it — an update moved the branches' revisions —, is
    /// asked again for nothing, or a view that never heard GitHub asks.
    #[test]
    fn a_moved_lock_makes_what_github_said_stale_once() {
        let dir = std::env::temp_dir().join(format!("ronoctua-update-lock-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a room");
        let lock = dir.join(ronoctua_software::heads::LOCK);
        std::fs::write(&lock, b"{}").expect("a lock");
        let mut v = a_view().with_heads(heard(Some(Ok(false)), Some(Ok(true))));
        v.lock = Some((lock.clone(), modified(&lock)));
        let _ = v.reread_if_moved();
        assert!(!v.heads_are_stale(), "asked again with nothing moved");
        v.lock = Some((lock.clone(), None));
        let _ = v.reread_if_moved();
        assert!(v.heads_are_stale(), "the lock moved and nothing is asked");
        assert!(!v.heads_are_stale(), "asked again twice for one move");
        // A MOVE WHILE GITHUB IS BEING ASKED: that question read the old
        // lock, and is asked again.
        let mut asking = a_view();
        asking.heads_asked();
        asking.lock = Some((lock.clone(), None));
        let _ = asking.reread_if_moved();
        assert!(asking.heads_are_stale(), "a move during a check was let be");
        let mut never = a_view();
        never.lock = Some((lock, None));
        let _ = never.reread_if_moved();
        assert!(!never.heads_are_stale(), "a view that never asked asks");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the head is not the status, a line of air, the actions —
    /// each the short word its key teaches, a page's value beside it —, air
    /// and the register, in both doors with the same targets; or the
    /// focused line does not wear the cursor's colour.
    #[test]
    fn the_head_is_the_status_then_the_actions_in_both_doors() {
        let t = texts();
        let v = a_view()
            .with_heads(heard(Some(Ok(false)), Some(Ok(true))))
            .with_glance(Glance {
                cache: ronoctua_cachix::settings::Named::Cache(
                    ronoctua_person::options::BinaryCache {
                        name: "someone".to_owned(),
                        public_key: String::new(),
                        visibility: ronoctua_person::options::Visibility::Private,
                    },
                ),
                follow: crate::follow::Choice::Off,
                parts: 2,
            });
        let head = v.top_lines(100, &t);
        assert_eq!(head[0].target, Some(Target::Status));
        assert_eq!(
            head[0].text,
            t.t(module::key::SYSTEM_UPDATE_STATUS_AVAILABLE_NIXPKGS)
        );
        assert_eq!(head[1].text, "", "no air under the status");
        let actions: Vec<&TopLine> = head[2..6].iter().collect();
        assert_eq!(
            actions.iter().map(|l| l.target).collect::<Vec<_>>(),
            [Action::Start, Action::Cache, Action::Follow, Action::Parts]
                .map(|a| Some(Target::Action(a)))
        );
        for (line, action) in
            actions
                .iter()
                .zip([Action::Start, Action::Cache, Action::Follow, Action::Parts])
        {
            assert_eq!(line.text, t.t(action.id()));
        }
        let beside = |l: &TopLine| l.beside.iter().map(|p| p.text.clone()).collect::<String>();
        assert_eq!(beside(actions[0]), "", "update now has a value");
        assert!(beside(actions[1]).contains("someone"), "{:?}", actions[1]);
        assert_eq!(beside(actions[2]), t.t(key::DISTRO_FOLLOW_OFF));
        assert!(beside(actions[3]).contains('2'), "{:?}", actions[3]);
        assert_eq!(head[6].text, "", "no air under the actions");
        assert_eq!(
            actions[0].color,
            crate::chrome::Kind::Setting.tones(true).0,
            "the focus is not the cursor's colour"
        );
        assert_ne!(actions[1].color, actions[0].color);
        // THE REGISTER SAYS WHAT THE FOCUSED ACTION DOES.
        assert_eq!(
            head[7].text,
            crate::chrome::note_lines(&t.t(module::key::SYSTEM_UPDATE_START_DOES), 100)[0]
        );
        // THE FOOTER'S REGION: the same lines, the same targets.
        let rows = v.region_rows(0, head.len(), 100, &t);
        assert_eq!(
            rows.iter().map(|r| r.target).collect::<Vec<_>>(),
            v.top_lines(crate::chrome::extent(100).1, &t)
                .iter()
                .map(|l| l.target)
                .collect::<Vec<_>>()
        );
        let words: String = rows
            .iter()
            .flat_map(|r| r.fields.iter().map(|f| f.text.clone()))
            .collect();
        for needle in [
            t.t(module::key::SYSTEM_UPDATE_STATUS_AVAILABLE_NIXPKGS),
            t.t(START),
            t.t(key::DISTRO_FOLLOW_OFF),
        ] {
            assert!(words.contains(&needle), "`{needle}` is not in the region");
        }
        // AND THE REGION'S CURSOR IS THE FOCUS'S LINE.
        assert_eq!(
            rows[v.region_cursor(100, &t)].target,
            Some(Target::Action(Action::Start))
        );
        // A COUNTDOWN offers its two answers, a run its stop; a queue its
        // "update now", refusing.
        let offered = |b: Brain| a_view().with_brain(b).actions();
        assert_eq!(
            offered(Brain {
                announced: true,
                ..Brain::default()
            })[..2],
            [Action::StartNow, Action::NotNow]
        );
        assert_eq!(
            offered(Brain {
                running: true,
                ..Brain::default()
            })[0],
            Action::Stop
        );
        let mut queued = a_view().with_brain(Brain {
            queued: true,
            ..Brain::default()
        });
        assert_eq!(
            queued.clicked(Target::Action(Action::Start)),
            Errand::Moved,
            "the first click on the queue's update now did more than select it"
        );
        let start = queued.top_lines(100, &t)[2].clone();
        assert_eq!(start.target, Some(Target::Action(Action::Start)));
        assert_eq!(
            start.color,
            crate::chrome::Kind::Disabled.tones(true).0,
            "the queue's update now does not refuse"
        );
        assert!(
            queued.register_lines(100, &t).join(" ").starts_with(
                t.t(module::key::SYSTEM_UPDATE_START_QUEUED)
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
            ),
            "the refusal does not say why"
        );
    }

    /// FAILS IF: the window's name is drawn on the status line while the
    /// header that says it is shown, is missing while the header is hidden,
    /// is not at the right end in the quietest tone, or the status runs
    /// under it.
    #[test]
    fn the_name_is_on_the_status_line_only_without_a_header() {
        let (w, mut ui) = a_window();
        let name = ui.texts.t(module::key::CMD_SYSTEM_UPDATE);
        let shown = w.head(&ui);
        assert!(
            shown
                .iter()
                .all(|l| l.beside.iter().all(|p| p.text != name)),
            "the name is drawn under a header that says it"
        );
        ui.hinted.hide_header(true);
        let hidden = w.head(&ui);
        let piece = hidden[0]
            .beside
            .iter()
            .find(|p| p.text == name)
            .expect("the name is not on the status line");
        assert_eq!(piece.color, tokens::color::TEXT_DIM);
        assert_eq!(
            piece.at + layout::width(&piece.text) + EDGE_RIGHT,
            ui.columns,
            "the name is not at the right end"
        );
        for l in hidden.iter().filter(|l| l.target == Some(Target::Status)) {
            assert!(
                layout::width(&l.text) + COLUMN_GAP <= piece.at,
                "the status runs under the name: {}",
                l.text
            );
        }
        assert_eq!(
            hidden.iter().filter(|l| !l.beside.is_empty()).count(),
            hidden
                .iter()
                .filter(|l| matches!(
                    l.target,
                    Some(Target::Action(
                        Action::Cache | Action::Follow | Action::Parts
                    ))
                ))
                .count()
                + 1,
            "the name is drawn on another line too"
        );
    }

    /// FAILS IF: a scene cannot say what the brain is doing with the
    /// update, or what GitHub said — or one that says a word it does not
    /// have is drawn as if it said nothing.
    #[test]
    fn a_scene_says_the_brain_and_the_heads() {
        let scene = |json: &str| -> UpdateWindow {
            let file = std::env::temp_dir().join(format!(
                "ronoctua-update-scene-{}-{}.json",
                std::process::id(),
                json.len()
            ));
            std::fs::write(&file, json).expect("a scene");
            let asked =
                camera::parse(&["--scene".to_owned(), file.display().to_string()]).expect("asked");
            let w = camera::window_for_tests(&asked).expect("a window");
            let _ = std::fs::remove_file(&file);
            w
        };
        let w = scene(
            r#"{"brain": {"announced": true, "queued": true, "last": {"kind": "done", "ended": 1762179600}}, "heads": {"nixpkgs": "moved", "ronoctua": "offline"}}"#,
        );
        assert_eq!(w.open.view.status(), Status::CountingDown);
        assert_eq!(w.open.view.run(), Run::CountingDown);
        let w = scene(r#"{"heads": {"nixpkgs": "same", "ronoctua": "same"}}"#);
        assert_eq!(w.open.view.status(), Status::UpToDate);
        let w = scene(r#"{"heads": "checking"}"#);
        assert_eq!(w.open.view.status(), Status::Checking);
        let w = scene(r#"{"heads": {"nixpkgs": "rate-limited"}}"#);
        assert_eq!(
            w.open.view.status(),
            Status::Unchecked(Unchecked::RateLimited)
        );
        let w = scene(r#"{"heads": {"nixpkgs": "same", "ronoctua": "unasked"}}"#);
        assert_eq!(
            w.open.view.status(),
            Status::Unchecked(Unchecked::Unasked(ronoctua_software::heads::Input::Distro))
        );
        let w = scene(r#"{"heads": {"nixpkgs": "lock"}}"#);
        assert_eq!(
            w.open.view.status(),
            Status::Unchecked(Unchecked::Unreadable)
        );
        let w = scene(r#"{"brain": {"last": {"kind": "failed", "ended": 5}}}"#);
        assert_eq!(w.open.view.status(), Status::Failed);
        for wrong in [
            r#"{"heads": {"nixpkgs": "sideways"}}"#,
            r#"{"heads": {"elsewhere": "same"}}"#,
            r#"{"heads": "maybe"}"#,
            r#"{"brain": {"hold": "tired"}}"#,
            r#"{"brain": {"runing": true}}"#,
        ] {
            let file = std::env::temp_dir().join(format!(
                "ronoctua-update-scene-wrong-{}.json",
                std::process::id()
            ));
            std::fs::write(&file, wrong).expect("a scene");
            let asked =
                camera::parse(&["--scene".to_owned(), file.display().to_string()]).expect("asked");
            assert!(camera::window_for_tests(&asked).is_err(), "{wrong}");
            let _ = std::fs::remove_file(&file);
        }
    }
}
