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
//!    there, asked of GitHub on a thread ([`status_of`]) — and then ONE list
//!    (`update_window_list`): what the machine wants said, what one does here, the
//!    options and the steps, each switched on or off where it is drawn. What
//!    the thing under the pointer or the cursor is, the door explains in its
//!    own place, never inside the list. Both doors show it — the footer's
//!    region (`LEADER s u u`) and the window (`LEADER s u w`) — with the
//!    same keys, every one a click, each taught only while it means
//!    something ([`View::taught`]).
//! 4. **The window**, which lays the view out.
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
use ronoctua_tokens as tokens;
use smithay_client_toolkit::{
    reexports::calloop::{channel, timer},
    seat::{
        keyboard::{KeyEvent, Modifiers},
        pointer::{PointerEvent, PointerEventKind},
    },
};

use crate::region::Post;
use crate::update_region::{Back, Open, Reach, carry_out};
use crate::update_window_list::{Item, ListRow};

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
pub fn begin(ended: Option<Post<Back>>) -> Started {
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
    begin_directly(ended)
}

/// Starts the whole sequence straight through systemd — [`begin`]'s second
/// door, and the one a "yes" to the network goes through: the brain would
/// hold it.
fn begin_directly(ended: Option<Post<Back>>) -> Started {
    start_unit(unit::RUN, ended)
}

/// Starts a REHEARSAL straight through systemd — the whole sequence printing
/// what it would do and changing nothing — for a door with no brain to ask:
/// the rehearsal's own unit, which the brain starts too.
pub fn rehearse_directly(ended: Option<Post<Back>>) -> Started {
    start_unit(unit::REHEARSE, ended)
}

/// Starts `unit` of the person's on a named thread — and, with `ended`,
/// waits there for it to end and says so ([`Back::Ended`]): no brain tells
/// the door when a run it started this way is over, and its live output
/// would stay for as long as the door is open.
fn start_unit(unit: &'static str, ended: Option<Post<Back>>) -> Started {
    // THE BUILD'S OWN ANSWER, before anything is spawned. `reachable` is a
    // `const fn` over an `option_env!`: outside Nix nothing baked the path, and
    // saying so is better than a key that fails silently every time.
    if !ronoctua_brain::systemd::reachable() {
        tracing::warn!("this build does not know where systemctl is");
        return Started::Nowhere;
    }
    let born = std::thread::Builder::new()
        .name("update".to_owned())
        .spawn(move || {
            // THE BRAIN'S OWN CALL, and not a copy of it. `--user` is
            // load-bearing and its reason is written where it lives: on the
            // SYSTEM bus `systemctl start` is authorised by polkit BEFORE
            // systemd looks the unit up, and with no agent in this session the
            // call would hang on an authentication request nobody asked for.
            started_then_ended(unit, ended.as_ref());
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
pub fn begin_on_battery(ended: Option<Post<Back>>) -> Started {
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
    // find one — would honour it. The run removes it however it ends, so
    // taking it back after one that failed changes nothing.
    let given = yes.clone();
    let born = std::thread::Builder::new()
        .name("update-on-battery".to_owned())
        .spawn(move || {
            if !started_then_ended(unit::RUN, ended.as_ref()) {
                let _ = std::fs::remove_file(&given);
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
pub fn begin_anyway(
    mind: Option<&ronoctua_brain::Mind>,
    on_battery: bool,
    ended: Option<Post<Back>>,
) -> Started {
    withdraw(mind);
    if on_battery {
        begin_on_battery(ended)
    } else {
        begin_directly(ended)
    }
}

/// `unit` started, on the thread that calls it: with `ended`, waited on and
/// its end said there — a start systemd refused ends at once —; without,
/// not waited on. Whether systemd took it and, waited on, it worked:
/// `systemctl` says a refusal and a failed run alike.
fn started_then_ended(unit: &'static str, ended: Option<&Post<Back>>) -> bool {
    let Some(post) = ended else {
        return match ronoctua_brain::systemd::start(unit) {
            Ok(()) => true,
            Err(e) => {
                tracing::error!(unit, error = %e, "systemd refused");
                false
            }
        };
    };
    let done = ronoctua_brain::systemd::start_and_wait(unit);
    if let Err(e) = &done {
        tracing::warn!(unit, error = %e, "systemd refused the run, or it failed");
    }
    let _ = post.send(Back::Ended);
    done.is_ok()
}

/// Stops the run this door started straight through systemd — no brain
/// holds it to stop it — the way the brain's own abort stops one
/// (`ronoctua_brain::systemd::stop`), on a named thread. Whether it was
/// asked: its end comes back as the start's ([`Back::Ended`]).
pub fn stop_here(here: RunHere) -> bool {
    let unit = match here {
        RunHere::Run => unit::RUN,
        RunHere::Rehearsal => unit::REHEARSE,
    };
    if !ronoctua_brain::systemd::reachable() {
        tracing::warn!("this build does not know where systemctl is");
        return false;
    }
    std::thread::Builder::new()
        .name("update-stop".to_owned())
        .spawn(move || {
            if let Err(e) = ronoctua_brain::systemd::stop(unit) {
                tracing::error!(unit, error = %e, "systemd refused to stop it");
            }
        })
        .is_ok()
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
    /// It was switched off when the run came to it — by this machine's
    /// table (`/etc/ronoctua/updates.conf`) or by the person.
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
            // PATH"*), which is right where the door explains it and far too wide
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
/// steps, the steps' two files and the last rehearsal's record — last
/// changed, so a beat can decide whether to re-read.
///
/// One `stat` per file per second — the device the file manager already uses to
/// notice a directory moved. Re-reading unconditionally would parse both files
/// once a second for a machine that updates once a week.
fn report_stamp() -> Vec<Option<std::time::SystemTime>> {
    let (table, flips) = step_files();
    report_paths()
        .into_iter()
        .chain(record_places().files())
        .chain([table, rehearsed_file()])
        .chain(flips)
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

// THE OUTPUT — one journal line, its colours and its priority, and the
// lines kept — is `crate::output`'s: it names no unit and no step.
pub use crate::output::{KEPT, OutputLine, Scrollback};

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
///
/// # JSON, and not the message alone
///
/// Because a line is more than its text: its PRIORITY (the runner's failure
/// lines and the machine's program say `<3>`, which journald keeps as the
/// priority and `-o cat` threw away), its time (the two halves' lines in the
/// order they were written, [`Scrollback::push`]) and its run (the last one
/// told from the ones before, [`Scrollback::last_run`]). And a message with
/// an escape in it is one journald keeps as BYTES, which only the JSON says
/// as they are ([`ronoctua_journal::read_entry`]) — the colours a program
/// wrote ([`ronoctua_shell_kit::ansi`]). `SYSTEMD_COLORS=0`: journalctl adds
/// none of its own.
///
/// # Nix colours nothing here
///
/// Checked against nix 2.34.9, the one the machine runs: it colours only a
/// terminal, its binaries do not even hold the words `FORCE_COLOR` or
/// `CLICOLOR_FORCE`, and through a pipe — a unit's journal — its `error:`
/// is plain (under `script`, a pseudo-terminal, it is `ESC [ 31 ; 1 m`). A
/// rebuild under a pseudo-terminal would colour it and flood the journal
/// with progress redrawn in place (`\r ESC [ K`), so it is not done: the
/// failures' red comes from their PRIORITY. Run that probe again when nix
/// moves.
fn follow(
    units: &[&str],
    of_the_person: bool,
    to: &crate::region::Post<Back>,
) -> Option<std::process::Child> {
    let first = units.first().copied().unwrap_or_default();
    let mut command = std::process::Command::new(JOURNALCTL);
    command
        .args(crate::output::journal_arguments(units, of_the_person))
        .env("SYSTEMD_COLORS", "0")
        .env("SYSTEMD_PAGER", "")
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
            let mut entries = std::io::BufReader::new(out);
            // THE END OF THE STREAM, or one that cannot be read: the follower
            // is gone, and so is this thread.
            while let Ok(Some(entry)) = ronoctua_journal::read_entry(&mut entries) {
                if !sender.send(Back::Line(OutputLine::of(&entry, !of_the_person))) {
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
// THE STEPS
// ---------------------------------------------------------------------------

/// How long a step took, as a clock and not as a sentence.
///
/// `15:00` needs no language and no key — digits and a colon are the same in
/// both, which is the same argument the brain's screen makes for drawing its
/// dates ISO rather than inventing *19 Aug* against *19 ago*.
#[must_use]
pub fn clock_of(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The declared step `id` is — the module's, with the keys it declared for
/// it. `None` for an id the declaration does not have (a mind from another
/// version of the brain).
#[must_use]
pub fn step_of(id: &str) -> Option<&'static ronoctua_brain::Step> {
    module::steps().iter().find(|s| s.id == id)
}

/// What a step really is, as the door explains it: the sentence the
/// declaration owes the step and, for the last run, what came of it
/// ([`last_run_explains`]).
#[must_use]
pub fn cursor_explains(
    step: &ronoctua_brain::Step,
    verdict: &Verdict,
    texts: &ronoctua_i18n::Texts,
) -> String {
    let mut said = texts.t(step.does_key);
    if let Some(why) = last_run_explains(step, verdict, texts) {
        said.push(' ');
        said.push_str(&why);
    }
    said
}

/// What the last run says of a step beyond its row's word: WHICH
/// precondition did not hold, why there was nothing new, or why its program
/// refused — `None` when the word says it all.
#[must_use]
pub fn last_run_explains(
    step: &ronoctua_brain::Step,
    verdict: &Verdict,
    texts: &ronoctua_i18n::Texts,
) -> Option<String> {
    let key = match verdict {
        // A KIND THE SEQUENCE DOES NOT HAVE SAYS NOTHING, rather than a key
        // made up of it: the row already says *skipped*.
        Verdict::Skipped(kind) => module::needs_key(kind),
        // AND NOTHING NEW SAYS WHY THERE WAS NOTHING, in the module's words.
        Verdict::Declined(word) => module::declined_key(word),
        // A REFUSAL SAYS WHY, in the words of the program that refused — the
        // machine's, or root's applier's of the person's declarations, as the
        // step declares (`refuses`) — and a word that is not that program's
        // says nothing more than the row's *failed*.
        Verdict::Refused(word) => module::refused_key(step.id, word),
        _ => None,
    };
    key.map(|k| texts.t(k))
}

/// Where this machine's table of steps and the person's switches are read
/// when no brain answers — this machine's, or the footer camera's room's
/// (the table under the room; the switches under the room's XDG state).
fn step_files() -> (PathBuf, Option<PathBuf>) {
    let table = match crate::photo::room() {
        Some(room) => room.join(module::MACHINE_TABLE.trim_start_matches('/')),
        None => PathBuf::from(module::MACHINE_TABLE),
    };
    let flips = ronoctua_xdg::state_dir().map(|d| d.join(module::STEPS_FILE));
    (table, flips)
}

/// The steps' rows from the two files the run reads, when no brain says them
/// ([`module::steps_read`]): a door with the daemon down still draws each
/// step the way the run will take it.
fn steps_from_the_files() -> Vec<ronoctua_brain::StepState> {
    let (table, flips) = step_files();
    let table = std::fs::read_to_string(table).ok();
    let flips = flips.and_then(|f| std::fs::read_to_string(f).ok());
    module::steps_read(table.as_deref(), flips.as_deref())
}

/// Where the last rehearsal left what it measured — the machine's run
/// folder, or the camera's room's.
fn rehearsed_file() -> PathBuf {
    let run = match crate::photo::room() {
        Some(room) => room.join(module::RUN_DIR.trim_start_matches('/')),
        None => PathBuf::from(module::RUN_DIR),
    };
    run.join(module::REHEARSED)
}

/// What the last rehearsal measured, when its record reads.
fn rehearsed_read() -> Option<module::Rehearsed> {
    let bytes = std::fs::read(rehearsed_file()).ok()?;
    serde_json::from_slice(&bytes).ok()
}

// ---------------------------------------------------------------------------
// THE VIEW — both doors
// ---------------------------------------------------------------------------

/// The keys the update teaches while nothing of it is under way, in both
/// doors — every one a click.
///
/// The arrows walk ONE list — the status, the notices, what to do, the
/// options, the steps — and `TAB` jumps to the next part of it (`S-TAB`, the
/// one before). `u` starts the update now — `LEADER s u n`'s command —, `s`
/// tries it changing nothing, `b` opens the brain on when it happens by
/// itself, and the four pages on their letters.
///
/// A KEY IS TAUGHT ONLY WHILE IT MEANS SOMETHING (the owner, 2026-11-08: `u`
/// and the countdown's "start now" side by side read as two words for one
/// thing). So each state of the update has its table — this one, the
/// queue's ([`OF_THE_QUEUE`]), the countdown's ([`OF_THE_COUNTDOWN`]) and
/// the run's ([`OF_THE_RUN`]) — and [`View::taught`] picks the one that is
/// true now; both doors read a key by all of them ([`press_of_key`]).
pub const OF_THE_VIEW: &[(&str, &str)] = &[
    ("<down>", NEXT),
    ("<up>", PREVIOUS),
    ("TAB", SECTION),
    ("S-TAB", SECTION),
    ("u", START),
    ("s", REHEARSE),
    ("b", SCHEDULE),
    // THE PERSON'S BINARY CACHE, a page of this view (`crate::binary_cache`):
    // `c` turns the view to it and back.
    ("c", crate::binary_cache::OPEN),
    // THE DISTRO'S APPS THAT FOLLOW THEIR RELEASES, another page
    // (`crate::follow`): `f` turns the view to it and back.
    ("f", crate::follow::OPEN),
    // THE MACHINE'S PARTS, a third (`crate::parts`): `d`, for the downloads
    // they wait for, turns the view to it and back.
    ("d", crate::parts::OPEN),
    // WHAT THE LAST UPDATE WROTE, a fourth: `o`, for its output.
    ("o", OUTPUT_OPEN),
];

/// The keys while the update waits in the brain's queue, not yet announced:
/// [`OF_THE_VIEW`]'s without `u` and `s` — asking again for a run that is
/// asked for already means nothing, and a rehearsal waits for it; their rows
/// say why.
pub const OF_THE_QUEUE: &[(&str, &str)] = &[
    ("<down>", NEXT),
    ("<up>", PREVIOUS),
    ("TAB", SECTION),
    ("S-TAB", SECTION),
    ("b", SCHEDULE),
    ("c", crate::binary_cache::OPEN),
    ("f", crate::follow::OPEN),
    ("d", crate::parts::OPEN),
    ("o", OUTPUT_OPEN),
];

/// The keys while the brain counts down to THIS update: `p` skips the
/// countdown, `a` puts the run off to its next moment — the band's two
/// buttons, by the brain's own commands (`brain-proceed`, `brain-abort`), so
/// that with this window in front `LEADER Alt` and the key answer the band,
/// and the band draws the key beside each (the footer's `brain::Words`).
pub const OF_THE_COUNTDOWN: &[(&str, &str)] = &[
    ("<down>", NEXT),
    ("<up>", PREVIOUS),
    ("TAB", SECTION),
    ("S-TAB", SECTION),
    ("p", PROCEED),
    ("a", ABORT),
    ("b", SCHEDULE),
    ("c", crate::binary_cache::OPEN),
    ("f", crate::follow::OPEN),
    ("d", crate::parts::OPEN),
    ("o", OUTPUT_OPEN),
];

/// The keys while THIS update runs: `a` stops it, and its live output moves
/// a page at a time on `C-j` and `C-k` and back to its end on `G`.
pub const OF_THE_RUN: &[(&str, &str)] = &[
    ("<down>", NEXT),
    ("<up>", PREVIOUS),
    ("TAB", SECTION),
    ("S-TAB", SECTION),
    ("a", STOP),
    ("C-j", OUTPUT_DOWN),
    ("C-k", OUTPUT_UP),
    ("G", OUTPUT_END),
    ("b", SCHEDULE),
    ("c", crate::binary_cache::OPEN),
    ("f", crate::follow::OPEN),
    ("d", crate::parts::OPEN),
    ("o", OUTPUT_OPEN),
];

/// The keys while a REHEARSAL runs: the run's, its stop the rehearsal's own
/// word (`a` stops it all the same).
pub const OF_THE_REHEARSAL: &[(&str, &str)] = &[
    ("<down>", NEXT),
    ("<up>", PREVIOUS),
    ("TAB", SECTION),
    ("S-TAB", SECTION),
    ("a", STOP_REHEARSAL),
    ("C-j", OUTPUT_DOWN),
    ("C-k", OUTPUT_UP),
    ("G", OUTPUT_END),
    ("b", SCHEDULE),
    ("c", crate::binary_cache::OPEN),
    ("f", crate::follow::OPEN),
    ("d", crate::parts::OPEN),
    ("o", OUTPUT_OPEN),
];

/// The keys of the page of what the last update wrote: the arrows a line,
/// `C-j` and `C-k` a page, `G` its end, and `o` back to the list (the way
/// back, `Esc`, too).
pub const OF_THE_OUTPUT: &[(&str, &str)] = &[
    ("<down>", OUTPUT_LINE_DOWN),
    ("<up>", OUTPUT_LINE_UP),
    ("C-j", OUTPUT_DOWN),
    ("C-k", OUTPUT_UP),
    ("G", OUTPUT_END),
    ("o", OUTPUT_CLOSE),
];

/// Every state's table and the output page's: what both doors read a key by
/// ([`press_of_key`]), and what the window keeps from the keyboard whatever
/// it teaches now (`UpdateWindow::barred_keys`) — a physical `u` never
/// starts an update.
pub const OF_EVERY_STATE: &[&[(&str, &str)]] = &[
    OF_THE_VIEW,
    OF_THE_QUEUE,
    OF_THE_COUNTDOWN,
    OF_THE_RUN,
    OF_THE_REHEARSAL,
    OF_THE_OUTPUT,
];

/// THE KEYS AFTER `LEADER Alt` that the rule for every window does not give
/// (`chrome::leader_alt_key`): the end of the output is `<end>`, where its own
/// key is vi's capital.
pub const LEADER_ALT_KEYS: &[(&str, &str)] = &[("G", "<end>")];

/// The keys the binary cache's page teaches, in both doors — every one a
/// click, pressed by its notation through [`press_of_key`]: the rows, the
/// change, and `c` back to the list (the way back, `Esc`, too).
pub const OF_THE_PAGE: &[(&str, &str)] = &[
    ("<down>", "binary-cache-down"),
    ("<up>", "binary-cache-up"),
    ("RET", "binary-cache-enter"),
    ("c", "binary-cache-steps"),
];

/// The keys the page of the apps that follow their releases teaches, in
/// both doors — every one a click, pressed by its notation through
/// [`press_of_key`]: the rows, the switch, and `f` back to the list (the
/// way back, `Esc`, too).
pub const OF_THE_FOLLOW: &[(&str, &str)] = &[
    ("<down>", "distro-follow-down"),
    ("<up>", "distro-follow-up"),
    ("RET", "distro-follow-enter"),
    ("f", "distro-follow-steps"),
];

/// The keys the parts' page teaches, in both doors — every one a click,
/// pressed by its notation through [`press_of_key`]: the rows, a try now, a
/// file handed in, and `d` back to the list (the way back, `Esc`, too).
pub const OF_THE_PARTS: &[(&str, &str)] = &[
    ("<down>", "distro-parts-down"),
    ("<up>", "distro-parts-up"),
    ("RET", "distro-parts-enter"),
    (crate::parts::HAND_KEY, HAND),
    ("d", "distro-parts-steps"),
];

/// The parts' page's own key that hands in a file — the one key of a page
/// the list does not teach.
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
/// The same, while what runs is a rehearsal.
pub const STOP_REHEARSAL: &str = "system-update-stop-rehearsal";
/// A run that changes nothing: what the update would download and build.
pub const REHEARSE: &str = "system-update-rehearse";
/// When the update happens by itself: the brain, on its task.
pub const SCHEDULE: &str = "system-update-schedule";
/// The page of what the last update wrote.
pub const OUTPUT_OPEN: &str = "system-update-output-open";
/// Back from that page to the list.
pub const OUTPUT_CLOSE: &str = "system-update-output-close";
/// The arrows: the next thing of the list.
const NEXT: &str = "system-update-next-step";
/// The arrows: the thing before.
const PREVIOUS: &str = "system-update-previous-step";
/// `TAB`: the next part of the list.
const SECTION: &str = "system-update-section";
/// The output, a page towards its end.
const OUTPUT_DOWN: &str = "system-update-output-down";
/// The output, a page towards its start.
const OUTPUT_UP: &str = "system-update-output-up";
/// The output's end, where a live run writes.
const OUTPUT_END: &str = "system-update-output-end";
/// The output's page, a line towards its end.
const OUTPUT_LINE_DOWN: &str = "system-update-output-line-down";
/// The output's page, a line towards its start.
const OUTPUT_LINE_UP: &str = "system-update-output-line-up";

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
    /// The thing above.
    Up,
    /// The thing below.
    Down,
    /// The output, a page towards its start.
    OutputUp,
    /// The output, a page towards its end.
    OutputDown,
    /// The output's end, where a live run is.
    OutputEnd,
    /// Start the update now.
    Start,
    /// Try the update, changing nothing.
    Rehearse,
    /// When the update happens by itself: the brain, on its task.
    Schedule,
    /// Stop the countdown, or the run.
    Abort,
    /// Start now what is counting down.
    Proceed,
    /// `TAB`: the first thing of the next part of the list.
    Section,
    /// `S-TAB`: the first thing of the part before.
    SectionBack,
    /// `Home`, `End`, `PgUp`, `PgDn`.
    Leap(Leap),
    /// `RET`: what the second click on the thing under the cursor does — a
    /// step switched, an action done, a page opened, a question answered.
    Enter,
    /// Out — a page first, and, with a question open, its "no".
    Escape,
    /// The person's binary cache: the page, or back to the list.
    Cache,
    /// The distro's apps that follow their releases: the page, or back to
    /// the list.
    Follow,
    /// The machine's parts: the page, or back to the list.
    Parts,
    /// What the last update wrote: the page, or back to the list.
    Output,
    /// On the parts' page, a file handed in for the part under the cursor.
    HandIn,
}

/// A leap of the cursor: to either end, or a page at a time — each door
/// counts its page in its own rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leap {
    /// `PgUp`.
    Up,
    /// `PgDn`.
    Down,
    /// `Home`.
    First,
    /// `End`.
    Last,
}

/// A key, as the view's press — the one reader both doors call.
///
/// By its whole notation through EVERY state's table ([`OF_EVERY_STATE`]):
/// what a state does not teach is still read, and the view answers it as
/// that state answers it — the countdown's answers with nothing of this
/// update counting down say so. And some that are not taught and that both
/// doors read the same way: `Esc` and `q` close, `j` and `k` walk as the
/// arrows do, the arrows under `Ctrl` move the output as `C-j` and `C-k` do
/// — the letters are what is taught, because a chord above the window may
/// take `Ctrl` with an arrow — and `Home`, `End`, `PgUp` and `PgDn` leap.
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
        Sym::Tab if plain && key.mods.shift => return Some(Press::SectionBack),
        Sym::PageUp if plain => return Some(Press::Leap(Leap::Up)),
        Sym::PageDown if plain => return Some(Press::Leap(Leap::Down)),
        Sym::Home if plain => return Some(Press::Leap(Leap::First)),
        Sym::End if plain => return Some(Press::Leap(Leap::Last)),
        _ => {}
    }
    let notation = key.notation();
    // THE VIEW'S KEYS FIRST: a page's key that shares the view's notation
    // (`d` back to the list) is the view's key; the parts' page adds one.
    let id = OF_EVERY_STATE
        .iter()
        .flat_map(|t| t.iter())
        .chain(OF_THE_PARTS)
        .find(|(taught, _)| *taught == notation)
        .map(|(_, id)| *id)?;
    Some(match id {
        NEXT | OUTPUT_LINE_DOWN => Press::Down,
        PREVIOUS | OUTPUT_LINE_UP => Press::Up,
        OUTPUT_DOWN => Press::OutputDown,
        OUTPUT_UP => Press::OutputUp,
        OUTPUT_END => Press::OutputEnd,
        SECTION => Press::Section,
        START => Press::Start,
        REHEARSE => Press::Rehearse,
        SCHEDULE => Press::Schedule,
        ABORT | STOP | STOP_REHEARSAL => Press::Abort,
        PROCEED => Press::Proceed,
        crate::binary_cache::OPEN => Press::Cache,
        crate::follow::OPEN => Press::Follow,
        crate::parts::OPEN => Press::Parts,
        OUTPUT_OPEN | OUTPUT_CLOSE => Press::Output,
        HAND => Press::HandIn,
        _ => return None,
    })
}

/// The view's old keys that no table teaches — `q` for `Esc`, `j` and `k`
/// for the arrows — which the footer's region still reads, and which the
/// window keeps from the keyboard (`UpdateWindow::barred_keys`): a `q` typed
/// for another window never closes this one past the second `Esc`.
const OLD_LETTERS: [&str; 3] = ["q", "j", "k"];

/// How the output moves — each door on its own page: the window's live
/// panel and the output's page, the footer's one list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// A page towards the start.
    Up,
    /// A page towards the end.
    Down,
    /// To the start.
    Start,
    /// To the end.
    End,
    /// So many lines towards the end; negative, towards the start.
    Lines(i32),
}

/// What the door has to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Errand {
    /// Nothing changed.
    Nothing,
    /// The cursor moved: the page follows it, and the door explains what
    /// it is on.
    Moved,
    /// The output moves.
    Output(Output),
    /// The cursor leaps, a page of the door's own rows at a time
    /// ([`View::leap`]).
    Leap(Leap),
    /// Start the update ([`begin`]), and say which door it went through.
    Start,
    /// Start it here and now, the person having said yes to what it spends
    /// ([`begin_anyway`]): on battery below the floor, or on the metered
    /// way out the brain held it for.
    StartHere {
        /// The yes is to a run on battery ([`begin_on_battery`]).
        on_battery: bool,
    },
    /// Try the update, changing nothing — through the brain, its refusal
    /// said; with no brain, the rehearsal's unit straight through systemd.
    Rehearse,
    /// Switch a step, by its id, to `on` — a state, never a flip, so a
    /// second press sent before the brain's new mind arrives asks the same.
    Switch {
        /// The step.
        id: &'static str,
        /// Whether it is to run.
        on: bool,
    },
    /// Open the brain on the update's task — the schedule's second click.
    OpenBrain,
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

/// One thing one does here — a row of the list, labelled with the same
/// short word its key teaches (`texts.t(id)`), so the keyboard and the mouse
/// reach the same deed by the same name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// The whole update, now ([`START`]).
    Start,
    /// Skip the brain's countdown of this update ([`PROCEED`]).
    StartNow,
    /// Put this update, counting down, off to its next moment ([`ABORT`]).
    NotNow,
    /// Stop this update, running ([`STOP`]).
    Stop,
    /// Try the update, changing nothing ([`REHEARSE`]).
    Rehearse,
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
            Self::Rehearse => REHEARSE,
        }
    }

    /// What it does, whole — where the door explains it, and under the
    /// pointer on its key.
    #[must_use]
    pub const fn does(self) -> &'static str {
        match self {
            Self::Start => module::key::SYSTEM_UPDATE_START_DOES,
            Self::StartNow => module::key::SYSTEM_UPDATE_PROCEED_DOES,
            Self::NotNow => module::key::SYSTEM_UPDATE_ABORT_DOES,
            Self::Stop => module::key::ABOUT_SYSTEM_UPDATE_STOP,
            Self::Rehearse => module::key::SYSTEM_UPDATE_REHEARSE_DOES,
        }
    }

    /// What `RET` or the second click on it does, in a few words.
    #[must_use]
    pub const fn act(self) -> &'static str {
        match self {
            Self::Start => module::key::SYSTEM_UPDATE_ACT_START,
            Self::StartNow => module::key::SYSTEM_UPDATE_ACT_PROCEED,
            Self::NotNow => module::key::SYSTEM_UPDATE_ACT_NOT_NOW,
            Self::Stop => module::key::SYSTEM_UPDATE_ACT_STOP,
            Self::Rehearse => module::key::SYSTEM_UPDATE_ACT_REHEARSE,
        }
    }

    /// Its id while what counts down or runs is a REHEARSAL: its stop is the
    /// rehearsal's own word.
    #[must_use]
    pub const fn id_in(self, rehearsal: bool) -> &'static str {
        match self {
            Self::Stop if rehearsal => STOP_REHEARSAL,
            other => other.id(),
        }
    }

    /// What it does while what counts down or runs is a REHEARSAL: it starts
    /// or stops the rehearsal, and "not now" lets it go — the update's own
    /// moment never moves for it.
    #[must_use]
    pub const fn does_in(self, rehearsal: bool) -> &'static str {
        match self {
            Self::StartNow if rehearsal => module::key::SYSTEM_UPDATE_PROCEED_REHEARSAL_DOES,
            Self::NotNow if rehearsal => module::key::SYSTEM_UPDATE_ABORT_REHEARSAL_DOES,
            Self::Stop if rehearsal => module::key::ABOUT_SYSTEM_UPDATE_STOP_REHEARSAL,
            other => other.does(),
        }
    }

    /// What `RET` or the second click on it does while what counts down or
    /// runs is a REHEARSAL.
    #[must_use]
    pub const fn act_in(self, rehearsal: bool) -> &'static str {
        match self {
            Self::NotNow if rehearsal => module::key::SYSTEM_UPDATE_ACT_LET_GO,
            Self::Stop if rehearsal => module::key::SYSTEM_UPDATE_ACT_STOP_REHEARSAL,
            other => other.act(),
        }
    }

    /// Every one.
    pub const ALL: [Self; 5] = [
        Self::Start,
        Self::StartNow,
        Self::NotNow,
        Self::Stop,
        Self::Rehearse,
    ];
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
    /// Its next run is a rehearsal somebody asked for.
    pub rehearsing: bool,
    /// The run under way IS that rehearsal. The brain forgets the ask as the
    /// run starts and says it of the run (`Mind::running_rehearsal`); from a
    /// brain that does not, the view remembers that the run it saw start had
    /// been asked as one ([`View::heard_the_brain`]).
    pub rehearsal: bool,
}

/// A run a door started straight through systemd, while it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunHere {
    /// The whole update.
    Run,
    /// Its rehearsal.
    Rehearsal,
}

impl Brain {
    /// Nothing under way, and nothing said.
    pub const IDLE: Self = Self {
        running: false,
        announced: false,
        queued: false,
        hold: None,
        failed: false,
        worked: None,
        next: 0,
        rehearsing: false,
        rehearsal: false,
    };

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
            rehearsing: mind.rehearsing.as_deref() == Some(module::ID),
            rehearsal: mind.running == module::ID && mind.running_rehearsal,
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
    /// A rehearsal of it runs: nothing on the machine changes.
    Rehearsing,
    /// The brain counts down to it.
    CountingDown,
    /// The brain counts down to a rehearsal of it.
    RehearsalSoon,
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
            Run::Running if b.rehearsal => return Status::Rehearsing,
            Run::Running => return Status::Running,
            Run::CountingDown if b.rehearsing => return Status::RehearsalSoon,
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
            Self::Rehearsing => module::key::SYSTEM_UPDATE_STATUS_REHEARSING,
            Self::CountingDown => module::key::SYSTEM_UPDATE_STATUS_COUNTING_DOWN,
            Self::RehearsalSoon => module::key::SYSTEM_UPDATE_STATUS_REHEARSAL_SOON,
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
            Self::Running | Self::Rehearsing | Self::CountingDown | Self::RehearsalSoon => {
                tokens::color::TEXT_PRIMARY
            }
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

    /// Its sentence.
    #[must_use]
    pub fn sentence(self, texts: &ronoctua_i18n::Texts) -> String {
        match self {
            Self::Unchecked(why) => texts.t_with(self.key(), &[("why", &texts.t(why.key()))]),
            _ => texts.t(self.key()),
        }
    }

    /// Every status the line can say, each reason it could not tell
    /// included — what the title's height is measured against, so a status
    /// that changes never moves the list under it.
    #[must_use]
    pub fn all() -> Vec<Self> {
        use ronoctua_brain::Hold;
        let mut out = vec![
            Self::Running,
            Self::Rehearsing,
            Self::CountingDown,
            Self::RehearsalSoon,
            Self::Queued(None),
            Self::Restart { menu_only: false },
            Self::Restart { menu_only: true },
            Self::Failed,
            Self::Held,
            Self::Checking,
            Self::Available(Moved::Distro),
            Self::Available(Moved::Nixpkgs),
            Self::Available(Moved::Both),
            Self::UpToDate,
            Self::Unasked,
        ];
        out.extend(
            [
                Hold::Ram,
                Hold::Cpu,
                Hold::Battery,
                Hold::Busy,
                Hold::Network,
                Hold::Asleep,
            ]
            .map(|h| Self::Queued(Some(h))),
        );
        out.extend(
            [
                Unchecked::Offline,
                Unchecked::RateLimited,
                Unchecked::Other,
                Unchecked::NoLock,
                Unchecked::Unreadable,
                Unchecked::Nothing,
            ]
            .map(Self::Unchecked),
        );
        out.extend(
            ronoctua_software::heads::Input::ALL.map(|i| Self::Unchecked(Unchecked::Unasked(i))),
        );
        out
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

/// The update, as both doors show it: the declared steps and which way each
/// is switched, what the two halves reported about each, what the cursor is
/// on and what the two journals said. What each door keeps of its own is
/// its page.
#[derive(Debug, Clone)]
pub struct View {
    /// The steps, each as switched — the brain's mind, or, with no brain,
    /// the two files the run reads.
    pub(crate) steps: Vec<ronoctua_brain::StepState>,
    /// The brain said the steps: its mind is their truth from then on, and
    /// the files are not read for them again.
    pub(crate) steps_said: bool,
    /// The view reads the machine's files when they move — none for a
    /// test's or a photograph's view.
    reads: bool,
    /// What the two halves reported, merged.
    pub(crate) verdicts: Verdicts,
    /// When each report file last moved.
    stamp: Vec<Option<std::time::SystemTime>>,
    /// The output, bounded.
    pub(crate) scrollback: Scrollback,
    /// What the PERSON put the cursor on — a key or a first click — by what
    /// it is; `None` where the view opens. A thing that went away gives the
    /// cursor back to where it rests (`View::focus`).
    pub(crate) focus: Option<crate::update_window_list::Item>,
    /// On a page, the cursor is on its way back, above its rows.
    pub(crate) page_back: bool,
    /// What the machine's records say beside the run.
    pub(crate) records: module::Records,
    /// The keys that open the belongings, as this leader teaches them — for
    /// the row that counts their conflicts.
    pub(crate) belongings_keys: String,
    /// The question, while it is asked.
    pub(crate) asking: Option<Question>,
    /// The battery as given — a test's, a photograph's — or `None` to read
    /// the machine's when a run is asked for.
    charge: Option<module::Charge>,
    /// The person's binary cache's page, while the view is turned to it.
    pub(crate) cache: Option<CachePage>,
    /// The page of the distro's apps that follow their releases, while the
    /// view is turned to it — never two pages at once.
    pub(crate) follow: Option<FollowPage>,
    /// The parts' page, while the view is turned to it.
    pub(crate) parts: Option<PartsPage>,
    /// The page of what the last update wrote, while the view is turned to
    /// it.
    pub(crate) output: bool,
    /// The door's wake, for the file picker the parts' page brings up.
    wake: Waking,
    /// The question about a gesture of the machine's, while it is asked —
    /// drawn where the gesture's row was, under the hand that clicked it.
    pub(crate) gesture: Option<crate::machine_gesture::Question>,
    /// A gesture said yes to whose verdict has not come: its row takes no
    /// click meanwhile.
    gesture_asked: Option<crate::machine_gesture::Gesture>,
    /// The person's brain modules that wait for an approval, as the machine
    /// says them (`crate::approval`).
    pub(crate) approvals: crate::approval::Readings,
    /// Where they are read, and when each file last moved — none for a
    /// test's or a photograph's view, which reads nothing of the machine.
    approval_places: Option<(crate::approval::Places, Vec<Option<std::time::SystemTime>>)>,
    /// The question about one of them, while it is asked — drawn where its
    /// row was, under the hand that clicked it.
    pub(crate) approval: Option<crate::approval::Question>,
    /// An approval said yes to whose verdict has not come: no row takes a
    /// click meanwhile.
    approval_asked: bool,
    /// What the brain last said of this task; `None` while it has said
    /// nothing (no brain, a test's view).
    pub(crate) brain: Option<Brain>,
    /// A run this door started straight through systemd, until it ends
    /// (`update_region::Back::Ended`) — no brain says that one runs.
    pub(crate) started_here: Option<RunHere>,
    /// When the brain runs it by itself, as it last said; `None` while it
    /// has said nothing.
    pub(crate) schedule: Option<crate::update_window_list::Schedule>,
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
    pub(crate) glance: Glance,
    /// Where it is read, and when each thing last moved — none for a test's
    /// or a photograph's view, which reads nothing of the machine.
    glance_places: Option<(GlancePlaces, Vec<Option<std::time::SystemTime>>)>,
    /// The zone the dates are drawn in.
    pub(crate) zone: ronoctua_files::when::Zone,
    /// What the last rehearsal measured, when it left a record.
    pub(crate) rehearsed: Option<module::Rehearsed>,
    /// Whether this person may read the system's journal, where the root
    /// half writes — the output's page says so when not.
    pub(crate) system_readable: bool,
    /// A rehearsal ended since the view last said one did: what it measured
    /// is said in the door once.
    rehearsal_news: bool,
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
    pub(crate) zone: ronoctua_files::when::Zone,
    /// The file picker, while it is up over the page (`crate::file_picker`):
    /// the file handed in, or a stick's folder.
    pub(crate) picking: Option<crate::file_picker::Hosted<crate::parts::Picking>>,
    /// What the picker's rows take, as they were last drawn — the one answer
    /// the painting writes and the pointer reads (a row of the picker is
    /// `Target::Page` of its index).
    pub(crate) drawn: std::cell::RefCell<Drawn>,
}

/// The picker's rows as they were drawn: what each takes, and which one the
/// cursor is on.
#[derive(Debug, Clone, Default)]
pub struct Drawn {
    pub(crate) targets: Vec<crate::file_picker::Target>,
    pub(crate) focused: Option<usize>,
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
    pub(crate) zone: ronoctua_files::when::Zone,
}

impl View {
    /// The declared steps, as this machine switches them, and what the
    /// reports and the records on this machine say; its battery is read when
    /// a run is asked for.
    #[must_use]
    pub fn read() -> Self {
        let mut v = Self::of(Verdicts::read(), report_stamp()).with_records(
            module::Records::read_at(&record_places()),
            belongings_keys(),
        );
        v.reads = true;
        v.steps = steps_from_the_files();
        v.rehearsed = rehearsed_read();
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
        v.system_readable = ronoctua_journal::system_readable(
            &crate::photo::room().unwrap_or_else(|| PathBuf::from("/")),
        );
        v
    }

    /// A view with `verdicts`, its steps as they ship — what a test builds,
    /// and nothing of the machine is read: no records, no table, no
    /// switches, and a machine on mains.
    #[must_use]
    pub fn of(verdicts: Verdicts, stamp: Vec<Option<std::time::SystemTime>>) -> Self {
        Self {
            steps: module::steps_read(None, None),
            steps_said: false,
            reads: false,
            verdicts,
            stamp,
            scrollback: Scrollback::default(),
            focus: None,
            page_back: false,
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
            output: false,
            wake: Waking::default(),
            gesture: None,
            gesture_asked: None,
            approvals: crate::approval::Readings::default(),
            approval_places: None,
            approval: None,
            approval_asked: false,
            brain: None,
            started_here: None,
            schedule: None,
            heads: None,
            lock: None,
            stale_heads: false,
            glance: Glance::default(),
            glance_places: None,
            zone: ronoctua_files::when::Zone::utc(),
            rehearsed: None,
            system_readable: true,
            rehearsal_news: false,
        }
    }

    /// The same view over these steps — a test's, a photograph's.
    #[must_use]
    pub fn with_steps(mut self, steps: Vec<ronoctua_brain::StepState>) -> Self {
        self.steps = steps;
        self
    }

    /// The same view over what the last rehearsal measured — a test's, a
    /// photograph's.
    #[must_use]
    pub fn with_rehearsed(mut self, rehearsed: Option<module::Rehearsed>) -> Self {
        self.rehearsed = rehearsed;
        self
    }

    /// The same view of a person who may read the system's journal or not
    /// — a test's, a photograph's.
    #[must_use]
    pub const fn with_system_readable(mut self, readable: bool) -> Self {
        self.system_readable = readable;
        self
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
    /// and this machine say — or, when it is open, back to the list.
    pub fn toggle_cache(&mut self) -> Errand {
        if self.cache.take().is_none() {
            self.open_cache();
        }
        self.page_back = false;
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
        self.output = false;
        self.page_back = false;
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
        self.output = false;
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
    /// table of where the update stands — `u` and `s` only while it can
    /// start, the countdown's two answers only while this update counts
    /// down, `a` to stop and the live output's keys only while it runs.
    #[must_use]
    pub const fn taught(&self) -> &'static [(&'static str, &'static str)] {
        if self.cache.is_some() {
            OF_THE_PAGE
        } else if self.follow.is_some() {
            OF_THE_FOLLOW
        } else if self.parts.is_some() {
            OF_THE_PARTS
        } else if self.output {
            OF_THE_OUTPUT
        } else {
            match self.run() {
                Run::Idle => OF_THE_VIEW,
                Run::Queued => OF_THE_QUEUE,
                Run::CountingDown => OF_THE_COUNTDOWN,
                Run::Running if self.rehearsal_now() => OF_THE_REHEARSAL,
                Run::Running => OF_THE_RUN,
            }
        }
    }

    /// Where the update stands in the brain — idle while the brain has
    /// said nothing.
    #[must_use]
    pub const fn run(&self) -> Run {
        match self.brain_now() {
            Some(b) => b.run(),
            None => Run::Idle,
        }
    }

    /// What the brain said, with a run this door started straight through
    /// systemd running in it while the brain's own is idle — so the status,
    /// the actions and the keys say that run as they say the brain's.
    const fn brain_now(&self) -> Option<Brain> {
        match (self.brain, self.started_here) {
            (Some(b), _) if !matches!(b.run(), Run::Idle) => Some(b),
            (said, Some(here)) => {
                let mut b = match said {
                    Some(b) => b,
                    None => Brain::IDLE,
                };
                b.running = true;
                b.rehearsal = matches!(here, RunHere::Rehearsal);
                Some(b)
            }
            (said, None) => said,
        }
    }

    /// What the status line says now ([`status_of`]).
    #[must_use]
    pub fn status(&self) -> Status {
        status_of(
            self.brain_now().as_ref(),
            &self.records,
            self.heads.as_ref(),
        )
    }

    /// The brain spoke: what it says of this task is kept — where the run
    /// stands, when it runs by itself, and every step as it switches it.
    /// Whether that changed anything the view draws.
    ///
    /// AN ANSWER TO A RUN THAT WENT AWAY IS FORGOTTEN: the cursor the person
    /// put on "not now" or "stop" ends with that countdown or that run, so
    /// the next one never finds the cursor waiting there — data from the
    /// brain never moves the cursor onto what changes a run.
    pub fn heard_the_brain(&mut self, mind: &ronoctua_brain::Mind) -> bool {
        let mut brain = Brain::of(mind);
        // A RUN THAT STARTS WHERE A REHEARSAL WAS ASKED IS THAT REHEARSAL —
        // the brain clears the ask as it starts it, and says so of the run
        // (`Mind::running_rehearsal`); a brain from before that word is read
        // by what this view saw — and it stays one while it runs.
        brain.rehearsal |= brain.running
            && self
                .brain
                .is_some_and(|b| b.rehearsal || (b.rehearsing && !b.running));
        let brain = Some(brain);
        let schedule = crate::update_window_list::Schedule::of(mind);
        // A MIND WITH NO STEPS FOR THE TASK (one from before steps) leaves
        // the steps as they were read.
        let steps = mind
            .task(module::ID)
            .map(|t| t.steps.clone())
            .filter(|s| !s.is_empty());
        let changed = brain != self.brain
            || schedule != self.schedule
            || steps.as_ref().is_some_and(|s| *s != self.steps);
        if !changed {
            return false;
        }
        self.brain = brain;
        self.schedule = schedule;
        if let Some(s) = steps {
            self.steps = s;
            self.steps_said = true;
        }
        if let Some(f) = &self.focus
            && f.changes_the_run()
            && !self.walkable().contains(f)
        {
            self.focus = None;
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

    /// Whether `action` can act now: "update now" cannot while the update
    /// already waits in the queue, and a rehearsal only while nothing of the
    /// update is under way and none is asked for already — their rows are
    /// drawn refusing, and say why.
    #[must_use]
    pub const fn acts(&self, action: Action) -> bool {
        match action {
            Action::Start => !matches!(self.run(), Run::Queued),
            Action::Rehearse => matches!(self.run(), Run::Idle) && !self.rehearsal_asked(),
            Action::StartNow | Action::NotNow | Action::Stop => true,
        }
    }

    /// Whether the brain's next run of the update is a rehearsal somebody
    /// asked for.
    #[must_use]
    pub const fn rehearsal_asked(&self) -> bool {
        matches!(
            self.brain,
            Some(Brain {
                rehearsing: true,
                ..
            })
        )
    }

    /// Whether what the brain counts down to or runs now is a REHEARSAL —
    /// the status's own reading ([`status_of`]) —, whose stop and "not now"
    /// are the rehearsal's.
    #[must_use]
    pub const fn rehearsal_now(&self) -> bool {
        matches!(
            self.brain_now(),
            Some(
                Brain {
                    running: true,
                    rehearsal: true,
                    ..
                } | Brain {
                    running: false,
                    announced: true,
                    rehearsing: true,
                    ..
                }
            )
        )
    }

    /// What an action does — its second click, its `RET` and its key.
    fn act(&mut self, action: Action) -> Errand {
        match action {
            // BELOW THE BATTERY FLOOR, A RUN BY HAND ASKS FIRST — the no
            // first — where a scheduled one would wait for the charger. And
            // one already in the queue is not asked for twice: why is said.
            Action::Start | Action::Rehearse if !self.acts(action) => {
                Errand::Say(self.why_not_said(action))
            }
            Action::Start => {
                if self.ask_on_battery() {
                    Errand::Moved
                } else {
                    Errand::Start
                }
            }
            Action::Rehearse => Errand::Rehearse,
            Action::StartNow => Errand::Answer(true),
            Action::NotNow | Action::Stop => Errand::Answer(false),
        }
    }

    /// `RET` on what the cursor is on: what its second click does.
    fn enter(&mut self) -> Errand {
        match self.focus() {
            Some(item) => self.activate(item),
            None => Errand::Nothing,
        }
    }

    /// What `item` does — its second click, and `RET` on it.
    fn activate(&mut self, item: crate::update_window_list::Item) -> Errand {
        use crate::update_window_list::{Item, PageOf};
        match item {
            Item::Status => Errand::Recheck,
            Item::Action(a) => self.act(a),
            Item::Schedule => Errand::OpenBrain,
            Item::Page(PageOf::Output) if !self.has_output() => {
                Errand::Say(module::key::SYSTEM_UPDATE_OUTPUT_NONE)
            }
            Item::Page(PageOf::Output) => self.toggle_output(),
            Item::Page(PageOf::Cache) => self.toggle_cache(),
            Item::Page(PageOf::Follow) => self.toggle_follow(),
            Item::Page(PageOf::Parts) => self.toggle_parts(),
            Item::Step(id) => self.switch(id),
            Item::Belongings => Errand::OpenBelongings,
            Item::Gesture(g) => self.ask_gesture_here(g),
            Item::Approve(id) => self.ask_approval_here(&id),
            Item::Back => self.close_the_page(),
            Item::Answer(kind, yes) => self.answer_of(kind, yes),
            Item::Row(n) => self.page_row_clicked(n),
            Item::Heading(_) | Item::Output => Errand::Nothing,
        }
    }

    /// Switches step `id` the other way from how it is now — a state the
    /// brain is asked for, never a flip. A step that runs as root is refused
    /// here with why: only the machine's configuration switches it.
    fn switch(&self, id: &'static str) -> Errand {
        match self.steps.iter().find(|s| s.id == id) {
            None => Errand::Nothing,
            Some(s) if s.needs_root => Errand::Say(module::key::SYSTEM_UPDATE_SWITCH_ROOT),
            Some(s) => Errand::Switch { id, on: !s.on },
        }
    }

    /// The question about gesture `what`, asked where its row is — the no
    /// first, the cursor born there. Asking is not doing: the yes is the
    /// second click, on the pick, and the sudo bar's.
    fn ask_gesture_here(&mut self, what: crate::machine_gesture::Gesture) -> Errand {
        if !self.gesture_takes(what) {
            return Errand::Nothing;
        }
        self.focus = Some(crate::update_window_list::Item::Gesture(what));
        self.gesture = Some(crate::machine_gesture::Question::of(what));
        Errand::Moved
    }

    /// The question about approving the module `id`, asked where its row
    /// is — the no first, as a gesture.
    fn ask_approval_here(&mut self, id: &str) -> Errand {
        let Some(w) = self.approvals.waiting.iter().find(|w| w.id == id).cloned() else {
            return Errand::Nothing;
        };
        if !self.quiet() || self.approvals.approved_as_shown(&w) {
            return Errand::Nothing;
        }
        self.focus = Some(crate::update_window_list::Item::Approve(w.id.clone()));
        self.approval = Some(crate::approval::Question::of(w));
        Errand::Moved
    }

    /// The answer `yes` of the question `kind`, given.
    fn answer_of(&mut self, kind: crate::update_window_list::Answering, yes: bool) -> Errand {
        use crate::update_window_list::Answering;
        match kind {
            Answering::Run => self.answer(yes),
            Answering::Gesture => self.gesture_answered(yes),
            Answering::Approval => self.approval_answered(yes),
            // THE ANSWER CHOSEN, then given: a click on the other one only
            // moves the cursor there.
            Answering::Grave => match self.cache.as_mut() {
                Some(c) => {
                    let e = match c.page.answer_clicked(yes) {
                        crate::binary_cache::Errand::Moved => c.page.answer_clicked(yes),
                        e => e,
                    };
                    self.page_errand(e)
                }
                None => Errand::Nothing,
            },
        }
    }

    /// A click on an answer: the first puts the cursor on it, the second —
    /// a click on the answer the cursor is on — answers (invariant 19).
    fn answer_clicked(&mut self, kind: crate::update_window_list::Answering, yes: bool) -> Errand {
        use crate::update_window_list::Answering;
        match kind {
            Answering::Run => match self.asking.as_mut() {
                Some(q) if q.yes == yes => self.answer(yes),
                Some(q) => {
                    q.yes = yes;
                    Errand::Moved
                }
                None => Errand::Nothing,
            },
            Answering::Gesture => {
                let Some(q) = self.gesture.as_mut() else {
                    return Errand::Nothing;
                };
                if q.clicked(yes) {
                    self.gesture_answered(yes)
                } else {
                    Errand::Moved
                }
            }
            Answering::Approval => {
                let Some(q) = self.approval.as_mut() else {
                    return Errand::Nothing;
                };
                if q.clicked(yes) {
                    self.approval_answered(yes)
                } else {
                    Errand::Moved
                }
            }
            Answering::Grave => match self.cache.as_mut() {
                Some(c) => {
                    let e = c.page.answer_clicked(yes);
                    self.page_errand(e)
                }
                None => Errand::Nothing,
            },
        }
    }

    // -----------------------------------------------------------------------
    // THE PAGE OF THE APPS THAT FOLLOW THEIR RELEASES (`crate::follow`)
    // -----------------------------------------------------------------------

    /// Turns the view to the page of the apps that follow their releases,
    /// reading what this person and this machine say — or, when it is open,
    /// back to the list.
    pub fn toggle_follow(&mut self) -> Errand {
        if self.follow.take().is_none() {
            self.open_follow();
        }
        self.page_back = false;
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
        self.output = false;
        self.page_back = false;
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
        self.output = false;
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
                self.page_back = false;
                Errand::Moved
            }
            other => Errand::Follow(other),
        }
    }

    // -----------------------------------------------------------------------
    // THE PARTS' PAGE (`crate::parts`)
    // -----------------------------------------------------------------------

    /// Turns the view to the parts' page, reading what this machine says —
    /// or, when it is open, back to the list.
    pub fn toggle_parts(&mut self) -> Errand {
        if self.parts.take().is_none() {
            self.open_parts();
        }
        self.page_back = false;
        Errand::Moved
    }

    /// Opens the page over what this machine says (kept open when it is).
    pub fn open_parts(&mut self) {
        if self.parts.is_some() {
            return;
        }
        self.cache = None;
        self.follow = None;
        self.output = false;
        self.page_back = false;
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
        self.output = false;
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
                self.page_back = false;
                Errand::Moved
            }
            // THE SYSTEM UPDATED AGAIN: the page gives way to the list,
            // which follows the run — started as `u` starts it, asking first
            // below the battery floor.
            Page::Update => {
                self.parts = None;
                self.page_back = false;
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
                self.page_back = false;
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
        self.cache.is_some() || self.follow.is_some() || self.parts.is_some() || self.output
    }

    /// Whether the view is turned to the page of what the last update wrote.
    #[must_use]
    pub const fn output_open(&self) -> bool {
        self.output
    }

    /// Whether the journals said anything of an update: the output's page
    /// opens only then, and says why not otherwise.
    #[must_use]
    pub fn has_output(&self) -> bool {
        !self.scrollback.is_empty()
    }

    /// Turns the view to the page of what the last update wrote — or, when
    /// it is open, back to the list.
    pub fn toggle_output(&mut self) -> Errand {
        if self.output {
            self.output = false;
            return Errand::Moved;
        }
        if !self.has_output() {
            return Errand::Say(module::key::SYSTEM_UPDATE_OUTPUT_NONE);
        }
        self.cache = None;
        self.follow = None;
        self.parts = None;
        self.page_back = false;
        self.output = true;
        Errand::Moved
    }

    /// Whether the cursor is on the open page's way back: on the output's
    /// page always — its lines take no click —, on another while the person
    /// put it there.
    #[must_use]
    pub const fn on_the_way_back(&self) -> bool {
        self.output || (self.page_back && self.page_open())
    }

    /// The open page shut — the list comes back.
    fn close_the_page(&mut self) -> Errand {
        if !self.page_open() {
            return Errand::Nothing;
        }
        self.cache = None;
        self.follow = None;
        self.parts = None;
        self.output = false;
        self.page_back = false;
        Errand::Moved
    }

    /// How many rows the open page's cursor walks.
    pub(crate) fn page_len(&self) -> usize {
        if let Some(c) = &self.cache {
            return crate::binary_cache::rows_of(&c.readings).len();
        }
        if let Some(f) = &self.follow {
            return crate::follow::rows_of(&f.readings).len();
        }
        if let Some(p) = &self.parts {
            if p.picking.is_some() {
                return p.drawn.borrow().targets.len().max(1);
            }
            return crate::parts::rows_of(&p.readings).len();
        }
        0
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

    /// The parts' page's lines, placed from `left` in `room`: the file
    /// picker's while it is up — each a row of its index, and what it takes
    /// written down for the pointer — or the page's own, the cursor's row a
    /// colour while it is `here`.
    pub(crate) fn parts_lines(
        &self,
        left: u16,
        room: u16,
        here: bool,
        texts: &ronoctua_i18n::Texts,
    ) -> Vec<crate::binary_cache::PageLine> {
        let Some(p) = &self.parts else {
            return Vec::new();
        };
        if let Some(h) = &p.picking {
            let cols = left + room + crate::chrome::FIELDS_LEAD + tokens::space::FILES_EDGERIGHT;
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
        p.page.lines(&p.readings, left, room, &p.zone, here, texts)
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
                self.page_back = false;
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
    pub(crate) const fn quiet(&self) -> bool {
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

    /// The steps, each as switched.
    #[must_use]
    pub fn steps(&self) -> &[ronoctua_brain::StepState] {
        &self.steps
    }

    /// What a rehearsal that ended since the view last said one measured —
    /// once, taken: the door says it.
    pub fn rehearsal_news(&mut self, texts: &ronoctua_i18n::Texts) -> Option<String> {
        if !std::mem::take(&mut self.rehearsal_news) {
            return None;
        }
        self.rehearsed_said(texts)
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

    /// One line of either journal.
    pub fn heard(&mut self, line: OutputLine) {
        self.scrollback.push(line);
    }

    /// Whether a frame is owed for what the journals said, and clears it.
    pub const fn owes_a_frame(&mut self) -> bool {
        self.scrollback.owes_a_frame()
    }

    /// Re-reads the reports, the records, the last rehearsal and — while no
    /// brain says them — the steps' two files when one moved, and only then;
    /// `true` when something did.
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
        // THE PAGES' VALUES beside their rows, the same way.
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
        if !self.reads {
            return moved;
        }
        let now = report_stamp();
        if now == self.stamp {
            return moved;
        }
        self.stamp = now;
        self.verdicts = Verdicts::read();
        self.records = module::Records::read_at(&record_places());
        let rehearsed = rehearsed_read();
        if rehearsed.is_some() && rehearsed != self.rehearsed {
            self.rehearsal_news = true;
        }
        self.rehearsed = rehearsed;
        if !self.steps_said {
            self.steps = steps_from_the_files();
        }
        true
    }

    /// One key — the same in both doors.
    pub fn key(&mut self, press: Press) -> Errand {
        // THE PAGES' LETTERS turn the view to a page and back, whatever is
        // open; the other keys stay the update's.
        match press {
            Press::Cache => {
                self.follow = None;
                self.parts = None;
                self.output = false;
                return self.toggle_cache();
            }
            Press::Follow => {
                self.parts = None;
                self.output = false;
                return self.toggle_follow();
            }
            Press::Output => return self.toggle_output(),
            _ => {}
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
            self.output = false;
            return self.toggle_parts();
        }
        if let Some(e) = self.page_key(press) {
            return e;
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
            // ONE LIST: the status, then every row a click takes.
            Press::Down => self.walk(true),
            Press::Up => self.walk(false),
            // A QUESTION HOLDS THE CURSOR: there is no other section to go
            // to until it is answered.
            Press::Section | Press::SectionBack if self.questioned() => Errand::Nothing,
            Press::Section => self.section(false),
            Press::SectionBack => self.section(true),
            Press::Leap(leap) => Errand::Leap(leap),
            Press::OutputUp => Errand::Output(Output::Up),
            Press::OutputDown => Errand::Output(Output::Down),
            Press::OutputEnd => Errand::Output(Output::End),
            Press::Start => self.act(Action::Start),
            Press::Rehearse => self.act(Action::Rehearse),
            Press::Schedule => Errand::OpenBrain,
            // THE COUNTDOWN'S ANSWERS: the door answers only this update's
            // (`answers_this_update`), and says so otherwise.
            Press::Abort => Errand::Answer(false),
            Press::Proceed => Errand::Answer(true),
            Press::Enter => self.enter(),
            Press::Escape => Errand::Close,
            Press::Cache => self.toggle_cache(),
            Press::Follow => self.toggle_follow(),
            Press::Parts => self.toggle_parts(),
            Press::Output => self.toggle_output(),
            // A FILE IS HANDED IN only on the parts' page.
            Press::HandIn => Errand::Nothing,
        }
    }

    /// A key on the open page — `None` when the page leaves it to the view.
    ///
    /// THE WAY BACK IS A ROW ABOVE THE PAGE'S: an arrow up from the page's
    /// first row goes to it, an arrow down from it to the page's rows, and
    /// `RET` on it — like `Esc` anywhere on the page — closes the page
    /// before the window.
    fn page_key(&mut self, press: Press) -> Option<Errand> {
        if self.output {
            return match press {
                Press::Up => Some(Errand::Output(Output::Lines(-1))),
                Press::Down => Some(Errand::Output(Output::Lines(1))),
                Press::Enter | Press::Escape => Some(self.close_the_page()),
                Press::Leap(leap) => Some(Errand::Leap(leap)),
                _ => None,
            };
        }
        if !self.page_open() {
            return None;
        }
        let asked = self
            .cache
            .as_ref()
            .is_some_and(|c| c.page.confirm().is_some());
        if self.page_back && !asked {
            return match press {
                Press::Down => {
                    self.page_back = false;
                    Some(Errand::Moved)
                }
                Press::Up => Some(Errand::Nothing),
                Press::Enter | Press::Escape => Some(self.close_the_page()),
                Press::Leap(leap) => Some(Errand::Leap(leap)),
                _ => None,
            };
        }
        if press == Press::Up && !asked && self.page_cursor() == Some(0) {
            self.page_back = true;
            return Some(Errand::Moved);
        }
        if let Press::Leap(leap) = press {
            return Some(Errand::Leap(leap));
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
            let e = page.map(|k| p.page.press(k, &p.readings))?;
            return Some(self.parts_errand(e));
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
            let e = page.map(|p| f.page.press(p, &f.readings))?;
            return Some(self.follow_errand(e));
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
            let e = page.map(|p| c.page.press(p, &c.readings))?;
            return Some(self.page_errand(e));
        }
        None
    }

    /// A click on what a row is, in either door.
    ///
    /// THE STATUS, AN ACTION, THE SCHEDULE, A PAGE, A STEP, THE CONFLICTS:
    /// the first click puts the cursor there — the door explains what it is
    /// and what the second click will do —, the second does what `RET` does
    /// (invariant 19): asks GitHub again, does the action, opens the brain or
    /// the page, switches the step, opens the belongings. A step's BOX is
    /// [`View::clicked_box`]: it switches at once.
    ///
    /// AN ANSWER: the first click puts the cursor on it, the second answers.
    /// A gesture's or a module's row asks at once, under the hand — asking is
    /// not doing — and takes the cursor as well. THE WAY BACK, the same: the
    /// first click selects it, the second closes the page.
    ///
    /// AN ANSWER TO THE RUN — start now, not now, stop — is done by a click
    /// only where the PERSON put the cursor, by a key or a first click; never
    /// where it rests by itself (`View::resting`).
    pub fn clicked(&mut self, item: crate::update_window_list::Item) -> Errand {
        use crate::update_window_list::Item;
        match item {
            Item::Answer(kind, yes) => self.answer_clicked(kind, yes),
            Item::Row(n) => self.page_row_clicked(n),
            Item::Back => {
                if !self.page_open()
                    || self
                        .cache
                        .as_ref()
                        .is_some_and(|c| c.page.confirm().is_some())
                {
                    return Errand::Nothing;
                }
                if self.on_the_way_back() {
                    self.close_the_page()
                } else {
                    self.page_back = true;
                    Errand::Moved
                }
            }
            Item::Gesture(g) => self.ask_gesture_here(g),
            Item::Approve(id) => self.ask_approval_here(&id),
            Item::Heading(_) | Item::Output => Errand::Nothing,
            other => {
                if !self.walkable().contains(&other) {
                    return Errand::Nothing;
                }
                let again = self.focus().as_ref() == Some(&other)
                    && (!other.changes_the_run() || self.focus.as_ref() == Some(&other));
                self.focus = Some(other.clone());
                if again {
                    self.activate(other)
                } else {
                    Errand::Moved
                }
            }
        }
    }

    /// A click on step `id`'s box: the step is switched at once — a box is
    /// a control of its own — and the cursor goes to the step.
    pub fn clicked_box(&mut self, id: &str) -> Errand {
        let Some(def) = step_of(id) else {
            return Errand::Nothing;
        };
        let item = crate::update_window_list::Item::Step(def.id);
        if !self.walkable().contains(&item) {
            return Errand::Nothing;
        }
        self.focus = Some(item);
        self.switch(def.id)
    }

    /// A click on row `n` of the open page: the first puts the page's cursor
    /// there, the second does what `RET` does — the page's own answer.
    fn page_row_clicked(&mut self, n: usize) -> Errand {
        // FROM THE WAY BACK the first click only selects: the page's own
        // cursor may still stand on row `n`, drawn unlit, and the page's
        // `clicked` would take this click for the second.
        let from_back = std::mem::replace(&mut self.page_back, false);
        if from_back && self.page_cursor() == Some(n) && self.page_takes(n) {
            return Errand::Moved;
        }
        match (self.cache.as_mut(), self.follow.as_mut()) {
            (Some(c), _) => {
                let e = c.page.clicked(n, &c.readings);
                self.page_errand(e)
            }
            (None, Some(f)) => {
                let e = f.page.clicked(n, &f.readings);
                self.follow_errand(e)
            }
            (None, None) => match self.parts.as_mut() {
                // A ROW OF THE PICKER is chosen there: its cursor, and the
                // second click its `RET`.
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
        }
    }

    /// Whether `what`'s row takes a click now: no question is asked — the
    /// run's, a gesture's or an approval's — no gesture or approval waits for
    /// its verdict, and the records offer it.
    pub(crate) fn gesture_takes(&self, what: crate::machine_gesture::Gesture) -> bool {
        self.quiet() && self.why_not(what).is_none()
    }
}

/// The view's own key for `command` (`OF_THE_VIEW`), as a key is drawn —
/// taught by a sentence, never written in one.
pub(crate) fn view_key_of(command: &str) -> String {
    OF_THE_VIEW
        .iter()
        .find(|(_, id)| *id == command)
        .map(|(notation, _)| (*notation).to_owned())
        .unwrap_or_default()
}

/// The keys that open the belongings, as this leader's trees teach them —
/// taught by the line that counts their conflicts, never written in a text.
fn belongings_keys() -> String {
    crate::bindings::taught(crate::belongings::OPEN_COMMAND, None).unwrap_or_default()
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
                        door.content.open.say_the_news(&mut door.ui);
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
/// same (`update_region::Open`) — and this door's own page ([`Layout`]): the
/// title, the list under it with its bar, and — only while a run goes — the
/// live output under the list, on a card of its own with its own bar. Under
/// them all the door keeps the window's foot ([`Content::foot_lines`]),
/// where the thing under the pointer, or the cursor's, is explained.
pub struct UpdateWindow {
    /// The list, its cursor, the output, the run followed and where a key
    /// reaches — the footer's region's too.
    open: Open,
    /// The first row of the list on screen, by its place among the rows.
    list_top: usize,
    /// THE SAME, BY WHAT IT IS: the nearest row at or above the page's top
    /// that is something, and how many rows under it the page starts. Rows
    /// that data brings or takes above it — a notice — never move what is
    /// on screen.
    anchor: Option<(Item, usize)>,
    /// A key or a click moved the cursor: the next settling brings its row
    /// on screen, once. Data, the wheel and the bar never set it.
    follow: Follow,
    /// What a cursor moved by the errand being carried out was moved by — a
    /// key, but while a click is carried out.
    moved_by: Follow,
    /// The live output's first line on screen; `None` follows its end.
    output_top: Option<usize>,
    /// What the pointer rests on — the foot's subject while no key came
    /// after it.
    pointed: Option<Spot>,
    /// A key came after the pointer last moved: the foot explains the
    /// cursor's thing until the pointer moves again.
    keyed: bool,
    /// Where the pointer rests, in the surface's pixels — what is under it
    /// is found again when the list moves under a still pointer.
    at: Option<(f64, f64)>,
    /// When the pointer came onto what it is on now, by the compositor's
    /// clock (a motion's `time`, in milliseconds).
    since_ms: u32,
    /// THE LAST THING THE POINTER RESTED ON — `space.window.footRestTenths`
    /// or longer — since the last key: what the foot explains once the
    /// pointer is in it, and not the rows it crossed to get there.
    rested: Option<Spot>,
    /// The pointer is in the foot, which hears its motions: the next one the
    /// window hears is the pointer coming back.
    in_the_foot: bool,
}

/// WHAT MOVED THE CURSOR, as the next settling follows it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Follow {
    /// Nothing did: data, the wheel and the bar never move the page to it.
    #[default]
    No,
    /// A key: its row comes on screen, with its section's heading when it is
    /// the section's first.
    Key,
    /// A click: the row was taken where it is drawn, so the page stays under
    /// the pointer — it moves only when the cursor's row is off it.
    Click,
}

/// WHAT THE POINTER RESTS ON, as the foot explains it: a thing of the title,
/// the list or the live output — or a step's box, which a click takes at
/// once.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Spot {
    /// A thing, and whether a click takes its row now: one that takes none
    /// — a heading, the output, a row a question dims — promises no click.
    Thing(Item, bool),
    /// The box of a step, by its id.
    Box(&'static str),
}

/// Which bar a pointer holds while it drags one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Grab {
    /// None.
    #[default]
    Nothing,
    /// The list's.
    List,
    /// The live output's.
    Output,
}

/// Lines `[first, first + lines)` of the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Band {
    first: u16,
    lines: u16,
}

impl Band {
    /// The line after its last.
    const fn end(self) -> u16 {
        self.first.saturating_add(self.lines)
    }

    /// Whether `line` is one of its.
    const fn has_line(self, line: u16) -> bool {
        line >= self.first && line < self.end()
    }
}

/// THE WINDOW LAID OUT, in its lines — the ONE answer the paint, the
/// pointer and the keys read: the title's rows from the first line under
/// the header, the list's band under them, and — while a run goes — the
/// live output's heading and the card its lines are drawn on, down to the
/// foot. As tall as the longest status, so a status that changes never
/// moves the list (`View::status_lines`).
#[derive(Debug, Clone)]
struct Layout {
    /// The title's rows.
    titled: Vec<ListRow>,
    /// Where they are drawn.
    title: Band,
    /// The list's band.
    list: Band,
    /// The live output's heading line and its card, while it shows.
    output: Option<(u16, Band)>,
}

/// A bar as it is drawn — the track a press grabs and the thumb painted —
/// and the scroll both are read from.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Bar {
    track: crate::chrome::Rect,
    thumb: Option<crate::chrome::Rect>,
    scroll: ronoctua_design::Scroll,
}

/// THE WINDOW'S BARS ([`UpdateWindow::bars`]): the list's and the live
/// output's, each only while what it scrolls is longer than its band.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Bars {
    list: Option<Bar>,
    output: Option<Bar>,
}

/// Where a line of the window is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// Row `n` of the title.
    Title(usize),
    /// Row `n` of the list.
    List(usize),
    /// The live output: its heading or its card.
    Output,
    /// Above it all, or past it — nothing of the window's.
    Nothing,
}

/// How many lines the live output takes of `room`, the lines under the
/// title: `space.update.outputPct` of them, never fewer than
/// `space.update.outputMinLines`, and never so many that the list keeps
/// fewer than `space.update.listMinLines` — half each when there is not
/// room for both.
fn pane_lines(room: u16) -> u16 {
    let least = tokens::space::UPDATE_OUTPUTMINLINES;
    let share = u16::try_from(u32::from(room) * u32::from(tokens::space::UPDATE_OUTPUTPCT) / 100)
        .unwrap_or(room);
    let most = room
        .saturating_sub(tokens::space::UPDATE_LISTMINLINES)
        .max(room / 2);
    share.max(least).min(most)
}

/// Where the page starts, by what it is: the nearest row at or above `top`
/// that is something, and how many rows under it `top` is.
fn anchor_of(rows: &[ListRow], top: usize) -> Option<(Item, usize)> {
    rows.iter()
        .take(top.saturating_add(1))
        .enumerate()
        .rev()
        .find(|(_, r)| !r.continues && r.item.is_some())
        .and_then(|(n, r)| r.item.clone().map(|i| (i, top - n)))
}

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
    let mut view = View::of(Verdicts::of(halves), Vec::new());
    // ONE LINE A MOMENT, in the order given, each as a unit writes it — a
    // `<N>` its priority — and of no run in particular.
    for (at_us, line) in (0_u64..).zip(said.lines()) {
        view.heard(OutputLine::written(at_us, false, "", line));
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
                                \"nixpkgs\": …},
                           \"steps\": {\"<id>\": {\"on\": true, \"here\": false}},
                           \"journal\": [{\"message\": \"<a line, SGR kept>\",
                             \"priority\": 3, \"root\": false}],
                           \"no-brain\": true,
                           \"system-hidden\": true}
                        `brain`: what the brain says of the update (any
                        field may be left out; a `done` last run is also
                        when it last worked), its calendar too —
                        `enabled`, `every` (days), `part` (night, morning,
                        afternoon, evening, any), `why-not` (the brain's
                        word: off, quiet, system-off…) and `now`; `steps`:
                        a step as this machine's table has it (`here`) and
                        as it is switched (`on`); `journal`: lines with
                        their priority; `no-brain`: no brain answers a key;
                        `system-hidden`: the system's journal is not this
                        person's to read; `heads`: what GitHub said of
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
  --header <how>        shown or hidden                (default: hidden)
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
        steps: std::collections::BTreeMap<String, StepScene>,
        journal: Vec<JournalScene>,
        #[serde(rename = "no-brain")]
        no_brain: bool,
        #[serde(rename = "system-hidden")]
        system_hidden: bool,
    }

    /// A step as the scene switches it: on this machine's table (`here`),
    /// and as it runs (`on`) — the person's flip is the difference.
    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StepScene {
        on: bool,
        here: bool,
    }

    /// A line of the journals, as the scene says it: its message with the
    /// escapes a program wrote, its priority, and which half wrote it.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct JournalScene {
        message: String,
        priority: Option<u8>,
        root: bool,
    }

    /// What the brain says of the update, as the scene says it.
    #[derive(Debug, Default, serde::Deserialize)]
    #[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
    struct BrainScene {
        running: bool,
        announced: bool,
        queued: bool,
        hold: Option<String>,
        last: Option<LastScene>,
        worked: Option<i64>,
        next: i64,
        rehearsing: bool,
        enabled: Option<bool>,
        every: Option<u16>,
        part: Option<String>,
        why_not: Option<String>,
        now: Option<i64>,
    }

    /// The update's last run, as the scene says it.
    #[derive(Debug, serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LastScene {
        kind: String,
        ended: i64,
    }

    impl BrainScene {
        /// The update's calendar, as the brain would say it: switched on,
        /// weekly, in the afternoon, its next moment three days from the
        /// scene's clock — unless the scene says otherwise.
        fn schedule(&self) -> Result<crate::update_window_list::Schedule> {
            use ronoctua_brain::PartOfDay;
            let part = match self.part.as_deref().unwrap_or("afternoon") {
                "any" => PartOfDay::Any,
                "night" => PartOfDay::Night,
                "morning" => PartOfDay::Morning,
                "afternoon" => PartOfDay::Afternoon,
                "evening" => PartOfDay::Evening,
                other => {
                    bail!("`part` is night, morning, afternoon, evening or any, not `{other}`")
                }
            };
            let why_not = self.why_not.clone().unwrap_or_default();
            let why_key = if why_not.is_empty() {
                String::new()
            } else {
                module::MANIFEST
                    .why_key(&why_not)
                    .map_or_else(|| format!("brain-why-{why_not}"), str::to_owned)
            };
            Ok(crate::update_window_list::Schedule {
                enabled: self.enabled.unwrap_or(true),
                interval_days: self.every.unwrap_or(7),
                part,
                next: self.next,
                why_not,
                why_key,
                now: self.now.unwrap_or(self.next - 3 * 86_400),
            })
        }

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
                // RUNNING, a rehearsal asked is the one that runs: the brain
                // has forgotten the ask by then.
                rehearsing: self.rehearsing && !self.running,
                rehearsal: self.rehearsing && self.running,
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
        /// `--header`, when the camera is handed it — the photograph's own
        /// reader takes it first (`photograph::run`), and absent is the
        /// distro's default, hidden.
        header: Option<ronoctua_shell_kit::header::Header>,
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
            header: None,
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
                "--header" => {
                    me.header = Some(match value()?.as_str() {
                        "shown" => ronoctua_shell_kit::header::Header::Shown,
                        "hidden" => ronoctua_shell_kit::header::Header::Hidden,
                        other => bail!("`--header` takes shown or hidden, not `{other}`"),
                    });
                }
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
        let schedule = scene.brain.as_ref().map(BrainScene::schedule).transpose()?;
        let brain = scene.brain.take().map(BrainScene::brain).transpose()?;
        let steps = steps_of(&std::mem::take(&mut scene.steps))?;
        let heads = scene.heads.take().as_ref().map(heads_of).transpose()?;
        let mut w = window_of_records(scene);
        if let Some(readings) = approvals {
            w.open.view = w.open.view.clone().with_approvals(readings);
        }
        if let Some(brain) = brain {
            w.open.view = w.open.view.clone().with_brain(brain);
        }
        w.open.view.schedule = schedule;
        if let Some(steps) = steps {
            w.open.view = w.open.view.clone().with_steps(steps);
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

    /// The steps as the scene switches them, over the declared ones — `None`
    /// when it switches none. An id the declaration does not have is
    /// refused, not drawn as if it were a step.
    fn steps_of(
        said: &std::collections::BTreeMap<String, StepScene>,
    ) -> Result<Option<Vec<ronoctua_brain::StepState>>> {
        if said.is_empty() {
            return Ok(None);
        }
        for id in said.keys() {
            ensure!(
                module::steps().iter().any(|s| s.id == *id),
                "`steps.{id}` is not a step of the update"
            );
        }
        Ok(Some(
            module::steps()
                .iter()
                .map(|s| {
                    said.get(s.id).map_or_else(
                        || ronoctua_brain::StepState::of(s, s.on_by_default, false),
                        |st| ronoctua_brain::StepState::of(s, st.here, st.on != st.here),
                    )
                })
                .collect(),
        ))
    }

    /// The window of the scene's reports, records and battery.
    fn window_of_records(scene: Scene) -> UpdateWindow {
        let mut w = super::photographed(&scene.reports, &scene.said);
        // THE JOURNALS' LINES, after what `said` said, each with its
        // priority as journald keeps it (`<N>`) and its escapes.
        let first = u64::try_from(scene.said.lines().count()).unwrap_or(0);
        for (at_us, line) in (first..).zip(&scene.journal) {
            let raw = line.priority.map_or_else(
                || line.message.clone(),
                |p| format!("<{p}>{}", line.message),
            );
            w.open
                .view
                .heard(super::OutputLine::written(at_us, line.root, "", &raw));
        }
        w.open.no_brain = scene.no_brain;
        w.open.view = w
            .open
            .view
            .clone()
            .with_system_readable(!scene.system_hidden);
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
        // THE HEADER AS ASKED, before the door is built — never as this
        // machine's `modeline.json` says.
        if let Some(header) = asked.header {
            ronoctua_shell_kit::header::set(ronoctua_shell_kit::header::Switches {
                header,
                ..ronoctua_shell_kit::header::Switches::default()
            });
        }
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
            list_top: 0,
            anchor: None,
            follow: Follow::No,
            moved_by: Follow::Key,
            output_top: None,
            pointed: None,
            keyed: false,
            at: None,
            since_ms: 0,
            rested: None,
            in_the_foot: false,
        }
    }

    /// The brain spoke: its mind is kept — the countdown's answers read it —
    /// what it says of the update drawn, and a run this window asked for is
    /// followed in it: held for a metered way out, the question, the no
    /// first.
    fn mind_arrived(&mut self, ui: &mut Ui, mind: ronoctua_brain::Mind) {
        let _ = self.open.heard_the_mind(mind, ui);
        ui.ask_paint();
    }

    /// What a photograph's window would have asked past the door, in order,
    /// as the photograph prints it — nothing for the window on a screen,
    /// which asked for real.
    #[must_use]
    pub fn would_have(&self) -> Vec<String> {
        self.open.would_have()
    }

    /// The title's rows: the status — with the window's name, while it
    /// wears no header — and when the last update worked.
    fn title(&self, ui: &Ui) -> Vec<ListRow> {
        let name = ui
            .hinted
            .is_hidden()
            .then(|| ui.texts.t(module::key::CMD_SYSTEM_UPDATE));
        self.open
            .view
            .status_lines(ui.columns, name.as_deref(), &ui.texts)
    }

    /// THE WINDOW LAID OUT ([`Layout`]): the live output only while a run
    /// goes and the view is on no page — a page is the whole of the list's
    /// room.
    fn layout(&self, ui: &Ui) -> Layout {
        let titled = self.title(ui);
        let first = ui.first_of_list();
        let tall = u16::try_from(titled.len()).unwrap_or(u16::MAX);
        let title = Band {
            first,
            lines: tall.min(ui.lines.saturating_sub(first)),
        };
        let room = ui.lines.saturating_sub(title.end());
        let pane = if self.open.output_live() && !self.open.view.page_open() {
            pane_lines(room)
        } else {
            0
        };
        let list = Band {
            first: title.end(),
            lines: room - pane,
        };
        let output = (pane > 0).then(|| {
            (
                list.end(),
                Band {
                    first: list.end() + 1,
                    lines: pane - 1,
                },
            )
        });
        Layout {
            titled,
            title,
            list,
            output,
        }
    }

    /// The list's rows, the window's width.
    fn rows(&self, ui: &Ui) -> Vec<ListRow> {
        self.open.view.list(0, ui.columns, &ui.texts)
    }

    /// How many rows the list has — counted, never built, on the output's
    /// page, which keeps thousands.
    fn list_len(&self, ui: &Ui) -> usize {
        self.open.view.list_len(ui.columns, &ui.texts)
    }

    /// Row `n` of the list — built alone on the output's page.
    fn row(&self, ui: &Ui, n: usize) -> Option<ListRow> {
        if self.open.view.output_open() {
            self.open
                .view
                .list_from(0, ui.columns, &ui.texts, n, 1)
                .pop()
        } else {
            self.rows(ui).into_iter().nth(n)
        }
    }

    /// The page settled where it is laid out now.
    fn settle(&mut self, ui: &Ui) {
        let lay = self.layout(ui);
        self.settle_in(ui, &lay);
    }

    /// THE PAGE SETTLED: its first row found again by what it is — data
    /// that came or went above it never moves what is on screen, but at the
    /// list's top, which stays the top —, the
    /// cursor's row brought on screen once after a key or a click (with the
    /// heading over it, when it is its section's first), and the page kept
    /// inside the list.
    fn settle_in(&mut self, ui: &Ui, lay: &Layout) {
        let room = usize::from(lay.list.lines).max(1);
        let follow = std::mem::take(&mut self.follow);
        if self.open.view.output_open() {
            // THE OUTPUT'S PAGE: its lines are nothing a cursor rests on, and
            // its place is a number.
            self.anchor = None;
            self.list_top = self.list_top.min(self.list_len(ui).saturating_sub(room));
            return;
        }
        let rows = self.rows(ui);
        if let Some((item, past)) = &self.anchor
            && let Some(at) = rows
                .iter()
                .position(|r| !r.continues && r.item.as_ref() == Some(item))
        {
            self.list_top = at + past;
        }
        if follow != Follow::No
            && let Some(at) = self.open.view.cursor_row(&rows)
        {
            // A ROW WRAPPED OVER SEVERAL LINES is followed to its last, and
            // the first of a section brings its heading with it.
            let mut last = at;
            while rows.get(last + 1).is_some_and(|r| r.continues) {
                last += 1;
            }
            // A ROW CLICKED WHERE IT IS DRAWN stays there, under the pointer
            // that took it.
            let on_the_page = last >= self.list_top && at < self.list_top + room;
            if follow == Follow::Key || !on_the_page {
                let mut first = at;
                if first > 0 && matches!(rows[first - 1].item, Some(Item::Heading(_))) {
                    first -= 1;
                }
                if first < self.list_top {
                    self.list_top = first;
                } else if last >= self.list_top + room {
                    self.list_top = (last + 1).saturating_sub(room).min(at);
                }
            }
        }
        self.list_top = self.list_top.min(rows.len().saturating_sub(room));
        // A PAGE AT ITS TOP STAYS AT THE TOP: a notice that comes there is
        // above everything, and anchored under it, it would come out of sight.
        self.anchor = if self.list_top == 0 {
            None
        } else {
            anchor_of(&rows, self.list_top)
        };
    }

    /// The list's page moved by `lines`, the cursor left where it is.
    fn scroll_by(&mut self, ui: &Ui, lines: i64) {
        let moved = i64::try_from(self.list_top)
            .unwrap_or(i64::MAX)
            .saturating_add(lines);
        self.list_top = usize::try_from(moved.max(0)).unwrap_or(0);
        // THE PERSON MOVED IT: its new first row is found by its place, and
        // anchored there.
        self.anchor = None;
        self.settle(ui);
    }

    /// The live output as a scroll in a card of `lines`: the last run's
    /// lines, from the first on screen — its end while it follows it.
    fn output_scroll(&self, lines: u16) -> ronoctua_design::Scroll {
        let total = self.open.view.scrollback().last_run().count();
        let lines = usize::from(lines);
        let tail = total.saturating_sub(lines);
        crate::chrome::list_scroll(total, lines, self.output_top.unwrap_or(tail).min(tail))
    }

    /// The live output put at `offset` in a card of `lines` — at its end, it
    /// follows the end again.
    fn output_to(&mut self, offset: usize, lines: u16) {
        let tail = self
            .open
            .view
            .scrollback()
            .last_run()
            .count()
            .saturating_sub(usize::from(lines));
        self.output_top = (offset < tail).then_some(offset);
    }

    /// The live output moved by `lines`, while it shows.
    fn output_by(&mut self, ui: &Ui, lines: i64) {
        let Some((_, card)) = self.layout(ui).output else {
            return;
        };
        let now = i64::try_from(self.output_scroll(card.lines).offset).unwrap_or(i64::MAX);
        let next = usize::try_from(now.saturating_add(lines).max(0)).unwrap_or(0);
        self.output_to(next, card.lines);
    }

    /// THE WINDOW'S BARS, placed — one answer the paint draws and records
    /// and the pointer grabs: the list's over its band, the live output's
    /// over its card, each in the window's last cell and only while what it
    /// scrolls is longer than its band (`chrome::band_bar`).
    fn bars(&self, ui: &Ui, lay: &Layout) -> Bars {
        let line = tokens::metrics::line_height(tokens::text::size());
        let cell = crate::chrome::cell_width();
        #[allow(clippy::cast_precision_loss)]
        let right = ui.width as f32;
        let bar = |scroll: ronoctua_design::Scroll, band: Band| {
            scroll.cut().then(|| {
                let (track, thumb) = crate::chrome::band_bar(
                    scroll,
                    right,
                    line * f32::from(band.first),
                    line * f32::from(band.end()),
                    cell,
                    line,
                );
                Bar {
                    track,
                    thumb,
                    scroll,
                }
            })
        };
        let list = bar(
            crate::chrome::list_scroll(
                self.list_len(ui),
                usize::from(lay.list.lines),
                self.list_top,
            ),
            lay.list,
        );
        let output = lay
            .output
            .and_then(|(_, card)| bar(self.output_scroll(card.lines), card));
        Bars { list, output }
    }

    /// Carries out what the view asked for: this window's own page — the
    /// list follows the cursor, the output's page and the live output move,
    /// the cursor leaps a page of this window's rows — and the rest through
    /// the footer's region's handler, with the same sentences.
    fn apply(&mut self, ui: &mut Ui, errand: Errand) {
        let lay = self.layout(ui);
        let errand = match errand {
            Errand::Leap(leap) => self.open.view.leap(leap, usize::from(lay.list.lines)),
            other => other,
        };
        match errand {
            Errand::Moved => self.follow = std::mem::replace(&mut self.moved_by, Follow::Key),
            // THE OUTPUT'S PAGE moves by the list's lines, the live output by
            // its card's.
            Errand::Output(how) => {
                let (page, live) = match lay.output {
                    _ if self.open.view.output_open() => (lay.list.lines, false),
                    Some((_, card)) => (card.lines, true),
                    None => (0, false),
                };
                let page = i64::from(page.max(1));
                let by = match how {
                    Output::Up => -page,
                    Output::Down => page,
                    Output::Start => i64::MIN / 2,
                    Output::End => i64::MAX / 2,
                    Output::Lines(n) => i64::from(n),
                };
                if live {
                    self.output_by(ui, by);
                } else if self.open.view.output_open() {
                    self.scroll_by(ui, by);
                }
            }
            _ => {}
        }
        carry_out(errand, &mut self.open, ui);
        self.settle(ui);
        ui.ask_paint();
    }

    /// One key, from the keyboard or from a click on the header — and the
    /// foot explains the cursor's thing again, until the pointer moves.
    fn pressed(&mut self, ui: &mut Ui, press: Press) {
        ui.message = None;
        self.keyed = true;
        self.rested = None;
        let errand = self.open.view.key(press);
        self.apply(ui, errand);
    }

    /// The line of a point of the surface, and its cell counted from where
    /// content begins.
    fn cell_at(ui: &Ui, (x, y): (f64, f64)) -> Option<(u16, u16)> {
        let px = tokens::text::size();
        let line_px = f64::from(tokens::metrics::line_height(px));
        let cell_px = f64::from(tokens::metrics::cell_width(px));
        if line_px <= 0.0 || cell_px <= 0.0 || x < 0.0 || y < 0.0 {
            return None;
        }
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let (line, cell) = ((y / line_px) as u16, (x / cell_px) as u16);
        Some((line, cell.saturating_sub(ui.margin)))
    }

    /// Where `line` is, as `lay` lays the window out.
    fn place(&self, lay: &Layout, line: u16) -> Place {
        if lay.title.has_line(line) {
            return Place::Title(usize::from(line - lay.title.first));
        }
        if lay.list.has_line(line) {
            return Place::List(self.list_top + usize::from(line - lay.list.first));
        }
        match lay.output {
            Some((heading, card)) if line == heading || card.has_line(line) => Place::Output,
            _ => Place::Nothing,
        }
    }

    /// The row drawn at `line`, as `lay` lays it out.
    fn row_on(&self, ui: &Ui, lay: &Layout, line: u16) -> Option<ListRow> {
        match self.place(lay, line) {
            Place::Title(n) => lay.titled.get(n).cloned(),
            Place::List(n) => self.row(ui, n),
            Place::Output | Place::Nothing => None,
        }
    }

    /// WHAT THE POINTER AT `at` RESTS ON — a thing of the title or the list,
    /// a step's box, the live output — and the line whose band lights for
    /// it: only a row a click takes lights; a heading or the output is
    /// explained and lights nothing.
    fn spot_at(&self, ui: &Ui, lay: &Layout, at: (f64, f64)) -> (Option<Spot>, Option<u16>) {
        let Some((line, cell)) = Self::cell_at(ui, at) else {
            return (None, None);
        };
        if self.place(lay, line) == Place::Output {
            return (Some(Spot::Thing(Item::Output, false)), None);
        }
        let Some(row) = self.row_on(ui, lay, line) else {
            return (None, None);
        };
        let Some(item) = row.item.clone() else {
            return (None, None);
        };
        let lit = row.takes.then_some(line);
        if row.takes
            && row.on_the_box(cell)
            && let Item::Step(id) = item
        {
            return (Some(Spot::Box(id)), lit);
        }
        (Some(Spot::Thing(item, row.takes)), lit)
    }

    /// The pointer left what it was on at `time` (`None`: a moment the
    /// compositor did not stamp): kept as what it rested on when it stayed
    /// there `space.window.footRestTenths` or longer.
    fn moved_on(&mut self, time: Option<u32>) {
        let rest_ms = u32::from(tokens::space::WINDOW_FOOTRESTTENTHS) * 100;
        if let (Some(t), Some(left)) = (time, &self.pointed)
            && t.wrapping_sub(self.since_ms) >= rest_ms
        {
            self.rested = Some(left.clone());
        }
        if let Some(t) = time {
            self.since_ms = t;
        }
    }

    /// A CLICK AT `at`, on what is drawn there: a step's box switches it at
    /// once; any other row a click takes goes through the view's own
    /// `clicked`, which the footer's click reaches too — the first selects,
    /// the second does what `RET` does.
    fn click_at(&mut self, ui: &mut Ui, lay: &Layout, at: (f64, f64)) {
        let Some((line, cell)) = Self::cell_at(ui, at) else {
            return;
        };
        let paged = self.open.view.page_open();
        let errand = match self.row_on(ui, lay, line) {
            Some(r) if r.takes && r.on_the_box(cell) => match r.item {
                Some(Item::Step(id)) => self.open.view.clicked_box(id),
                _ => Errand::Nothing,
            },
            Some(r) if r.takes => r
                .item
                .map_or(Errand::Nothing, |i| self.open.view.clicked(i)),
            _ => Errand::Nothing,
        };
        // A CLICK THAT OPENS OR CLOSES A PAGE leaves nothing under the
        // pointer to keep: the page is followed as a key's.
        if self.open.view.page_open() == paged {
            self.moved_by = Follow::Click;
        }
        self.apply(ui, errand);
        self.moved_by = Follow::Key;
    }
}

impl Content for UpdateWindow {
    type Hand = Grab;

    fn app_id(&self) -> &'static str {
        APP_ID
    }

    /// NOT `TAB` FROM SECTION TO SECTION (`section_ring`): it walks its sections itself.
    fn walks_sections(&self, _: &Ui) -> bool {
        false
    }

    /// `*name*` is Emacs's convention for a buffer that is not a file, and the
    /// name is the command's own label — one sentence, two doors.
    fn title(&self, texts: &ronoctua_i18n::Texts) -> String {
        format!("*{}*", texts.t(module::key::CMD_SYSTEM_UPDATE))
    }

    /// One key, from a press or from a REPEAT — holding an arrow walks the
    /// list, which is what a list wants — read by the reader the footer
    /// calls too.
    fn typed(&mut self, ui: &mut Ui, event: &KeyEvent, mods: Modifiers) {
        let Some(key) = crate::door::keys::key_of(event, mods) else {
            return;
        };
        // THE FILE PICKER, while it is up over the parts' page, reads its
        // keys before the view does.
        if self.open.view.picker_takes(key) {
            ui.message = None;
            self.keyed = true;
            self.rested = None;
            let errand = self.open.view.pick_key(key);
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

    /// EVERY STATE'S TABLE AND EVERY PAGE'S, kept from the keyboard: the view
    /// reads `u`, `a` and `p` whatever the update's state, while the line
    /// teaches each only where it means something — a physical `u` never
    /// starts an update, queued or not. Not while the file picker is up: its
    /// letters are its.
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
                .map(|r| r.0)
                .chain(OLD_LETTERS),
        )
    }

    fn hint(&self, ui: &Ui) -> Vec<crate::chrome::Piece> {
        self.open.view.hint(&ui.texts)
    }

    /// EVERY ACTION SAYS WHAT IT DOES — what the door explains of its row
    /// (`Action::does`), under the pointer on the header and beside it in the
    /// footer — where the registry's sentence for its command is a label
    /// (`cmd-system-update`, `cmd-brain-abort`). Every other key, the
    /// registry's or the view's (`chrome::hint_sentence`).
    fn explains(&self, id: &str, texts: &ronoctua_i18n::Texts) -> Option<String> {
        let rehearsal = self.open.view.rehearsal_now();
        match Action::ALL
            .into_iter()
            .find(|a| a.id() == id || a.id_in(true) == id)
        {
            Some(action) => Some(texts.t(action.does_in(rehearsal))),
            None => crate::chrome::hint_sentence(id, texts),
        }
    }

    /// THE FOOT THIS WINDOW KEEPS (the owner, 2026-11-08): always this tall,
    /// so nothing above it moves — what is explained there changes, the
    /// list does not.
    fn foot_lines(&self, _: &Ui) -> u16 {
        tokens::space::WINDOW_FOOTLINES
    }

    /// WHAT THE FOOT EXPLAINS: the thing the pointer rests on — a step's box
    /// switching at once, any other thing as a click takes it
    /// ([`View::gesture_for`]), and a row no click takes with no gesture —
    /// until a key comes; then, and with the pointer on nothing, the
    /// cursor's thing, which `RET` does.
    fn explained(&self, ui: &Ui) -> Option<crate::door::Explained> {
        let view = &self.open.view;
        if !self.keyed
            && let Some(spot) = &self.pointed
        {
            return Some(match spot {
                Spot::Box(id) => {
                    view.explained(&Item::Step(id), crate::door::Gesture::AtOnce, &ui.texts)
                }
                Spot::Thing(item, takes) => {
                    let mut e = view.explained(item, view.gesture_for(item), &ui.texts);
                    // A ROW NO CLICK TAKES promises none.
                    if !takes {
                        e.gesture = None;
                    }
                    e
                }
            });
        }
        let item = view.cursor_item()?;
        Some(view.explained(&item, crate::door::Gesture::Ret, &ui.texts))
    }

    /// THE POINTER WENT INTO THE FOOT: what the foot explains is what the
    /// pointer last rested on — not the rows it crossed on its way down —,
    /// held while the pointer is there, and the cursor's thing when it
    /// rested on none since the last key.
    fn pointer_went_to_the_foot(&mut self, ui: &mut Ui, _: &mut Hand<Grab>, time: Option<u32>) {
        self.moved_on(time);
        self.pointed = self.rested.clone();
        self.at = None;
        self.in_the_foot = true;
        ui.ask_paint();
    }

    fn resized(&mut self, ui: &mut Ui) {
        self.settle(ui);
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
            ui.ask_paint();
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&mut self, ui: &mut Ui, canvas: &mut Canvas<'_>) {
        let size = tokens::text::size();
        let line_px = tokens::metrics::line_height(size);
        let cell_px = crate::chrome::cell_width();
        let texts = ui.texts.clone();

        // LINE 0: whose window this is — a glyph and the sequence's own name —
        // and the keys, every one a click, in the header every window of this
        // system has, drawer and all. With no header, what the last order said
        // and a question go to the window's foot (`list::line_zero`).
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
        let lay = self.layout(ui);
        self.settle_in(ui, &lay);
        // WHAT THE POINTER RESTS ON, found again on what is drawn now: the
        // list may have moved under a still pointer.
        if let Some(at) = self.at {
            self.pointed = self.spot_at(ui, &lay, at).0;
        }
        let bars = self.bars(ui, &lay);
        let (margin, columns) = (ui.margin, ui.columns);
        #[allow(clippy::cast_precision_loss)]
        let width = ui.width as f32;
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
        let at_line =
            |n: usize, first: u16| first.saturating_add(u16::try_from(n).unwrap_or(u16::MAX));

        // WHAT IS DRAWN, line by line: the row, and where a band across it
        // ends — the list's at its bar while the bar is drawn.
        let band_right = bars.list.map_or(width, |b| b.track.0);
        let mut drawn: Vec<(u16, ListRow, f32)> = lay
            .titled
            .iter()
            .enumerate()
            .map(|(n, row)| (at_line(n, lay.title.first), row.clone(), width))
            .take(usize::from(lay.title.lines))
            .collect();
        // THE OUTPUT'S PAGE lays its lines on the terminal's card, as the
        // live output does: its way back, and why the administrator's half is
        // missing, stay on the window's ground above it.
        let head = if self.open.view.output_open() {
            self.list_len(ui)
                .saturating_sub(self.open.view.scrollback().last_run().count())
        } else {
            usize::MAX
        };
        let mut card_from: Option<u16> = None;
        let shown = self.open.view.list_from(
            0,
            columns,
            &texts,
            self.list_top,
            usize::from(lay.list.lines),
        );
        for (n, mut row) in shown.into_iter().enumerate() {
            let line = at_line(n, lay.list.first);
            if self.list_top + n >= head {
                card_from.get_or_insert(line);
                for f in &mut row.fields {
                    f.color = crate::output::on_a_card(f.color);
                }
            }
            drawn.push((line, row, band_right));
        }
        let card = |from: u16, to: u16| ronoctua_render::quad::Quad {
            x: 0.0,
            y: line_px * f32::from(from),
            width,
            height: line_px * f32::from(to.saturating_sub(from)),
            color: tokens::color::TERMINAL_BACKGROUND,
        };
        if let Some(from) = card_from {
            canvas.under.push(card(from, lay.list.end()));
        }

        // WHAT EVERY POINTER IS ON lights — only a row a click takes, every
        // line of it (a sentence wrapped over several is one control); on a
        // step's box, the box alone, which a click there switches at once.
        let lit: Vec<(tokens::Color, u16, Option<f32>)> = canvas
            .lit
            .iter()
            .enumerate()
            .filter_map(|(k, (color, under))| {
                let line = u16::try_from((*under)?).ok()?;
                Some((*color, line, canvas.lit_x.get(k).copied().flatten()))
            })
            .collect();
        for (color, line, x) in lit {
            let Some((_, row, _)) = drawn.iter().find(|(l, _, _)| *l == line) else {
                continue;
            };
            if !row.takes || row.item.is_none() {
                continue;
            }
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let cell = x.map(|x| ((x / cell_px) as u16).saturating_sub(margin));
            if let (Some((from, to)), Some(c)) = (row.boxed, cell)
                && (from..to).contains(&c)
            {
                canvas.under.push(ronoctua_render::quad::Quad {
                    x: f32::from(margin + from) * cell_px,
                    y: f32::from(line) * line_px,
                    width: f32::from(to - from) * cell_px,
                    height: line_px,
                    color,
                });
                continue;
            }
            let item = row.item.clone();
            for (l, r, right) in &drawn {
                if r.takes && r.item == item {
                    canvas.under.push(ronoctua_render::quad::Quad {
                        x: 0.0,
                        y: f32::from(*l) * line_px,
                        width: *right,
                        height: line_px,
                        color,
                    });
                }
            }
        }
        for (line, row, _) in &drawn {
            for f in &row.fields {
                canvas
                    .spans
                    .push(span(f.text.clone(), margin + f.at, *line, f.color));
            }
        }

        // THE LIST'S BAR, over its band — written down as it is drawn, so its
        // column is the bar's while it is there (`crate::bars`).
        ui.room = crate::chrome::Room {
            width,
            bar_from: bars.list.map(|b| b.track.0),
        };
        if let Some(b) = bars.list {
            if let Some((x, y, w, h)) = b.thumb {
                canvas.under.push(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: tokens::color::RULE_STRONG,
                });
            }
            canvas.bar(Some(ronoctua_shell_kit::bars::Track::of(b.track)));
        }

        // THE LIVE OUTPUT, only while a run goes: a rule over it, its heading
        // — and, when this person may not read the system's journal, that its
        // administrator's half is missing —, and the card its lines are
        // drawn on, each piece in its colour, following the end.
        let Some((heading, card_band)) = lay.output else {
            return;
        };
        canvas
            .under
            .push(crate::chrome::rule(width, line_px * f32::from(heading)));
        let named = ronoctua_render::layout::elide_end(
            &texts.t(module::key::SYSTEM_UPDATE_OUTPUT_LIVE),
            columns,
        );
        let named_w = ronoctua_render::layout::width(&named);
        canvas
            .spans
            .push(span(named, margin, heading, tokens::color::TEXT_HEADING));
        if !self.open.view.system_readable {
            let hidden = texts.t(module::key::SYSTEM_UPDATE_OUTPUT_SYSTEM_HIDDEN_SHORT);
            let hidden_w = ronoctua_render::layout::width(&hidden);
            if named_w + tokens::space::MODELINE_SEGMENTGAP + hidden_w <= columns {
                canvas.spans.push(span(
                    hidden,
                    margin + columns - hidden_w,
                    heading,
                    tokens::color::STATE_WARNING,
                ));
            }
        }
        if card_band.lines == 0 {
            return;
        }
        canvas.under.push(card(card_band.first, card_band.end()));
        let scroll = self.output_scroll(card_band.lines);
        if scroll.total == 0 {
            for (n, said) in crate::chrome::note_lines(
                &texts.t(module::key::SYSTEM_UPDATE_OUTPUT_WAITING),
                columns,
            )
            .into_iter()
            .take(usize::from(card_band.lines))
            .enumerate()
            {
                canvas.spans.push(span(
                    said,
                    margin,
                    at_line(n, card_band.first),
                    tokens::color::TERMINAL_ANSI_BRIGHT_BLACK,
                ));
            }
        }
        for (n, line) in self
            .open
            .view
            .scrollback()
            .last_run()
            .skip(scroll.offset)
            .take(usize::from(card_band.lines))
            .enumerate()
        {
            for f in ListRow::of_output(line, 0, columns).fields {
                canvas.spans.push(span(
                    f.text,
                    margin + f.at,
                    at_line(n, card_band.first),
                    crate::output::on_a_card(f.color),
                ));
            }
        }
        if let Some(b) = bars.output {
            if let Some((x, y, w, h)) = b.thumb {
                canvas.over.push(ronoctua_render::quad::Quad {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: tokens::color::RULE_STRONG,
                });
            }
            canvas.bar(Some(ronoctua_shell_kit::bars::Track::of(b.track)));
        }
    }

    fn pointer(&mut self, ui: &mut Ui, hand: &mut Hand<Grab>, events: &[PointerEvent]) {
        ui.hinted.swap_hand(&mut hand.hint_drag);
        self.pointer_here(ui, hand, events);
        ui.hinted.swap_hand(&mut hand.hint_drag);
    }
}

impl UpdateWindow {
    /// One pointer's frame: the header's controls first — the chevron, the
    /// drawer's bar and its wheel, and the sentence under a key — then the
    /// two bars, each grabbed where it is drawn (`UpdateWindow::bars`), then
    /// a row — a step's box switches at once — then the wheel, which moves
    /// the band under the pointer and never the cursor. What the pointer
    /// rests on is the foot's subject until a key.
    #[allow(clippy::too_many_lines)]
    fn pointer_here(&mut self, ui: &mut Ui, hand: &mut Hand<Grab>, events: &[PointerEvent]) {
        let before = (
            hand.row_under_pointer,
            self.list_top,
            self.output_top,
            self.pointed.clone(),
            self.keyed,
        );
        let mut turned = false;
        let mut header = false;
        let mut explained = false;
        let mut wheel_at: Option<(f64, f64)> = None;
        let mut clicked: Option<(f64, f64)> = None;
        let mut dragged: Option<usize> = None;
        let head = ui.frame(0);
        let lay = self.layout(ui);
        let bars = self.bars(ui, &lay);
        let line_px = tokens::metrics::line_height(tokens::text::size());
        let held = |grab: Grab| match grab {
            Grab::List => bars.list,
            Grab::Output => bars.output,
            Grab::Nothing => None,
        };
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
                    // A BAR HELD moves with the pointer, and nothing else does.
                    if let Some(b) = held(hand.extra) {
                        dragged = Some(b.scroll.offset_at(py - b.track.1, b.track.3, line_px));
                        continue;
                    }
                    // THE POINTER MOVED: what it rests on is the foot's again
                    // — and what it left, when it rested there long enough
                    // to be read, is what the foot keeps explaining once the
                    // pointer is in it.
                    self.at = Some(e.position);
                    self.keyed = false;
                    let (spot, lit) = self.spot_at(ui, &lay, e.position);
                    let time = match e.kind {
                        PointerEventKind::Motion { time } => Some(time),
                        _ => None,
                    };
                    if std::mem::take(&mut self.in_the_foot) {
                        // BACK FROM THE FOOT: what it is on starts now.
                        if let Some(t) = time {
                            self.since_ms = t;
                        }
                    } else if spot != self.pointed {
                        self.moved_on(time);
                    }
                    self.pointed = spot;
                    hand.row_under_pointer = lit.map(usize::from);
                    hand.x_under_pointer = lit.map(|_| px);
                }
                PointerEventKind::Leave { .. } => {
                    ui.hinted.let_go();
                    explained |= ui.hinted.say(None);
                    // AND THE DRAG ENDS WITH THE POINTER — see the same arm
                    // in the list windows.
                    hand.extra = Grab::Nothing;
                    hand.dragging = false;
                    hand.row_under_pointer = None;
                    hand.x_under_pointer = None;
                    self.at = None;
                    self.pointed = None;
                    self.rested = None;
                    self.in_the_foot = false;
                }
                PointerEventKind::Release { button, .. } if button == LEFT_BUTTON => {
                    ui.hinted.let_go();
                    hand.extra = Grab::Nothing;
                    hand.dragging = false;
                }
                PointerEventKind::Press { button, .. } if button == LEFT_BUTTON => {
                    if ui.hinted.pressed(head, e.position.0, e.position.1) {
                        header = true;
                        continue;
                    }
                    // A BAR IS GRABBED WHERE IT IS DRAWN: its whole track
                    // answers, and the page goes where the press is.
                    let grabbed = [(Grab::List, bars.list), (Grab::Output, bars.output)]
                        .into_iter()
                        .find_map(|(grab, bar)| {
                            bar.filter(|b| {
                                ronoctua_shell_kit::bars::Track::of(b.track)
                                    .holds(e.position.0, e.position.1)
                            })
                            .map(|b| (grab, b))
                        });
                    if let Some((grab, b)) = grabbed {
                        hand.extra = grab;
                        hand.dragging = true;
                        dragged = Some(b.scroll.offset_at(py - b.track.1, b.track.3, line_px));
                    } else {
                        clicked = Some(e.position);
                    }
                }
                PointerEventKind::Axis {
                    vertical,
                    horizontal,
                    ..
                } => {
                    wheel_at = Some(e.position);
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
            // THE BAR MOVES ITS BAND, and nothing else this frame: a wheel
            // tick arriving during a drag would move it twice.
            match hand.extra {
                Grab::List => {
                    self.list_top = offset;
                    self.anchor = None;
                    self.settle(ui);
                }
                Grab::Output => {
                    if let Some((_, card)) = lay.output {
                        self.output_to(offset, card.lines);
                    }
                }
                Grab::Nothing => {}
            }
        }
        if let Some(at) = clicked {
            self.click_at(ui, &lay, at);
        }
        let by = if turned {
            crate::wheel::wheel_lines(&mut hand.lines_owed)
        } else {
            0
        };
        // A WHEEL OVER THE OPEN DRAWER WALKS THE DRAWER, and not the list
        // under it: the gesture belongs to what is under the pointer.
        if by != 0 && wheel_at.is_some_and(|(_, y)| ui.hinted.turned(head, y, by)) {
            ui.ask_paint();
            return;
        }
        // THE WHEEL MOVES THE BAND UNDER THE POINTER, a line a notch-line,
        // and never the cursor: what the keyboard is on stays where the
        // person put it.
        if by != 0
            && let Some(at) = wheel_at
        {
            let place =
                Self::cell_at(ui, at).map_or(Place::Nothing, |(line, _)| self.place(&lay, line));
            match place {
                Place::Output => self.output_by(ui, by),
                Place::Title(_) | Place::List(_) => self.scroll_by(ui, by),
                Place::Nothing => {}
            }
        }
        // THE LIST MOVED UNDER A STILL POINTER: what it rests on now is
        // found again, and lit.
        if (dragged.is_some() || by != 0 || clicked.is_some())
            && let Some(at) = self.at
        {
            let lay = self.layout(ui);
            let (spot, lit) = self.spot_at(ui, &lay, at);
            self.pointed = spot;
            hand.row_under_pointer = lit.map(usize::from);
        }
        // Only redraws if something CHANGED: a `Motion` arrives on every pixel
        // of movement, and painting per pixel would spend the GPU drawing the
        // same thing.
        let after = (
            hand.row_under_pointer,
            self.list_top,
            self.output_top,
            self.pointed.clone(),
            self.keyed,
        );
        if explained || before != after {
            ui.ask_paint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::update_window_list::{Item, ListRow, Section};
    use ronoctua_render::layout;

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
            w.open
                .view
                .scrollback()
                .line(1)
                .map(OutputLine::text)
                .as_deref(),
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

    /// A machine running off a battery below the sequence's floor.
    fn flat() -> module::Charge {
        module::Charge {
            discharging: true,
            percent: Some(module::battery_floor().expect("a floor") - 1),
        }
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

    /// A REFUSAL OF THE MACHINE'S PROGRAM SAYS WHY, in the machine's words:
    /// every reason it can give has its sentence, the door's explanation
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

    /// A machine whose last trial failed, not held, on a revision.
    fn failed_unheld() -> module::Records {
        module::Records {
            trial: Some(module::TrialOutcome::Failed),
            revision: Some("613862c5a0ff613862c5a0ff".to_owned()),
            ..module::Records::default()
        }
    }

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
            rehearsing: false,
            rehearsal: false,
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
        // EVERY SENTENCE IS THERE, in both languages, and the title is
        // measured against each.
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
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for status in statuses.clone() {
                let said = status.sentence(&t);
                assert!(!said.contains('['), "{language}: {status:?}: {said}");
                assert!(
                    Status::all().contains(&status),
                    "{status:?} is not one the title is measured against"
                );
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

    // -----------------------------------------------------------------------
    // THE LIST
    // -----------------------------------------------------------------------

    /// A view of the declared steps as they ship, with nothing reported — no
    /// file of the machine is read.
    fn a_view() -> View {
        View::of(Verdicts::default(), Vec::new())
    }

    fn key(notation: &str) -> Option<Press> {
        press_of_key(ronoctua_keymap::Key::parse(notation).expect("a key"))
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

    /// The text of a row, piece after piece.
    fn said(row: &ListRow) -> String {
        row.fields
            .iter()
            .map(|f| f.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The text of a list, row after row.
    fn said_all(rows: &[ListRow]) -> String {
        rows.iter().map(said).collect::<Vec<_>>().join("\n")
    }

    /// The row about `item`, and where it is.
    fn row_of(rows: &[ListRow], item: &Item) -> (usize, ListRow) {
        rows.iter()
            .enumerate()
            .find(|(_, r)| r.item.as_ref() == Some(item))
            .map(|(n, r)| (n, r.clone()))
            .unwrap_or_else(|| panic!("no row of {item:?}:\n{}", said_all(rows)))
    }

    /// Steps switched as the table `table` says, the person's `flips` over
    /// it — ids from the declaration, never written here.
    fn steps_with(table: &str, flips: &str) -> Vec<ronoctua_brain::StepState> {
        module::steps_read(Some(table), Some(flips))
    }

    /// A step the distro ships on, and one it ships off — neither root's.
    fn shipped(on: bool) -> &'static ronoctua_brain::Step {
        module::steps()
            .iter()
            .find(|s| s.on_by_default == on && !s.needs_root)
            .expect("a step of that kind")
    }

    /// A step that runs as root.
    fn roots() -> &'static ronoctua_brain::Step {
        module::steps()
            .iter()
            .find(|s| s.needs_root)
            .expect("a root step")
    }

    /// BOTH DOORS READ A KEY ONE WAY: every row of every table is its own
    /// press, the untaught ones are the window's old keys and the page keys,
    /// and a chord that is not in the table is nothing.
    #[test]
    fn every_taught_key_is_a_press_and_a_chord_is_not_its_letter() {
        assert_eq!(key("<down>"), Some(Press::Down));
        assert_eq!(key("<up>"), Some(Press::Up));
        assert_eq!(key("C-j"), Some(Press::OutputDown));
        assert_eq!(key("C-k"), Some(Press::OutputUp));
        assert_eq!(key("G"), Some(Press::OutputEnd));
        assert_eq!(key("u"), Some(Press::Start));
        assert_eq!(key("s"), Some(Press::Rehearse));
        assert_eq!(key("b"), Some(Press::Schedule));
        assert_eq!(key("o"), Some(Press::Output));
        assert_eq!(key("a"), Some(Press::Abort));
        assert_eq!(key("p"), Some(Press::Proceed));
        assert_eq!(key("TAB"), Some(Press::Section));
        assert_eq!(key("S-TAB"), Some(Press::SectionBack));
        assert_eq!(key("ESC"), Some(Press::Escape));
        assert_eq!(key("q"), Some(Press::Escape));
        assert_eq!(key("j"), Some(Press::Down));
        assert_eq!(key("k"), Some(Press::Up));
        assert_eq!(key("C-<down>"), Some(Press::OutputDown));
        assert_eq!(key("<home>"), Some(Press::Leap(Leap::First)));
        assert_eq!(key("<end>"), Some(Press::Leap(Leap::Last)));
        assert_eq!(key("<prior>"), Some(Press::Leap(Leap::Up)));
        assert_eq!(key("<next>"), Some(Press::Leap(Leap::Down)));
        for chord in ["C-u", "M-a", "C-p", "C-q", "M-j", "C-s", "C-b", "C-o"] {
            assert_eq!(key(chord), None, "`{chord}` was taken for its letter");
        }
        for (taught, id) in OF_EVERY_STATE.iter().flat_map(|t| t.iter()) {
            assert!(
                key(taught).is_some(),
                "`{taught}` ({id}) is taught and reads as nothing"
            );
        }
        // …AND THE WINDOW KEEPS THE OLD LETTERS from the keyboard: no table
        // teaches them, and `q` would close it past the second `Esc`.
        let (w, ui) = a_window();
        let barred = w.barred_keys(&ui);
        for letter in OLD_LETTERS {
            let k = ronoctua_keymap::Key::parse(letter).expect("a key");
            assert!(barred.contains(&k), "`{letter}` reaches the window");
        }
    }

    /// FAILS IF: the list is not the four parts in their order — the
    /// notices only when there are some —, each under its heading with air
    /// before every heading but the first, the steps' heading counting the
    /// steps that are on, or a row runs past the width.
    #[test]
    fn the_list_is_four_parts_under_their_headings() {
        let t = texts();
        let quiet = a_view().list(0, 80, &t);
        let headings: Vec<Section> = quiet
            .iter()
            .filter_map(|r| match r.item {
                Some(Item::Heading(s)) => Some(s),
                _ => None,
            })
            .collect();
        assert_eq!(
            headings,
            [Section::Do, Section::Options, Section::Steps],
            "nothing to notice, and a notices' heading"
        );
        let noticed = a_view()
            .with_records(every_record(), "SUPER s v".to_owned())
            .list(0, 80, &t);
        let headings: Vec<(usize, Section)> = noticed
            .iter()
            .enumerate()
            .filter_map(|(n, r)| match r.item {
                Some(Item::Heading(s)) => Some((n, s)),
                _ => None,
            })
            .collect();
        assert_eq!(
            headings.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
            Section::ALL
        );
        assert_eq!(headings[0].0, 0, "the first heading has air before it");
        for (n, _) in &headings[1..] {
            assert!(
                noticed[n - 1].fields.is_empty() && noticed[n - 1].item.is_none(),
                "no air before the heading at {n}"
            );
        }
        // HIERARCHY IS COLOUR AND AIR: every heading is `text.heading`.
        for (n, _) in &headings {
            assert_eq!(noticed[*n].fields[0].color, tokens::color::TEXT_HEADING);
        }
        // THE STEPS' HEADING COUNTS what is on, as a number.
        let mut steps = steps_with("", "");
        let on = steps.len() / 2;
        for s in steps.iter_mut().take(on) {
            s.on = true;
        }
        for s in steps.iter_mut().skip(on) {
            s.on = false;
        }
        let v = a_view().with_steps(steps.clone());
        let rows = v.list(0, 80, &t);
        let (_, heading) = row_of(&rows, &Item::Heading(Section::Steps));
        let counted = t.t_args(
            module::key::SYSTEM_UPDATE_STEPS_COUNT,
            &[
                (
                    "on",
                    ronoctua_i18n::Arg::Number(i64::try_from(on).expect("small")),
                ),
                (
                    "all",
                    ronoctua_i18n::Arg::Number(i64::try_from(steps.len()).expect("small")),
                ),
            ],
        );
        assert!(said(&heading).contains(&counted), "{}", said(&heading));
        assert!(counted.contains(&on.to_string()), "{counted}");
        // NOTHING OF A ROW LEAKS PAST THE WIDTH, at any width.
        for width in [30_u16, 60, 90, 140] {
            for row in v.list(0, width, &t) {
                for f in &row.fields {
                    assert!(
                        f.at + layout::width(&f.text) <= width,
                        "{width}: {:?} at {}",
                        f.text,
                        f.at
                    );
                }
            }
        }
    }

    /// FAILS IF: a row says something else depending on where the cursor is
    /// — the owner's first complaint: the explanation sat in the list and
    /// pushed every row under it down as the cursor moved. Only a colour may
    /// change (the cursor is a colour).
    #[test]
    fn every_rows_text_is_the_same_wherever_the_cursor_is() {
        let t = texts();
        let mut v = a_view().with_records(every_record(), "SUPER s v".to_owned());
        let shape = |v: &View| -> Vec<Vec<(u16, String)>> {
            v.list(0, 90, &t)
                .iter()
                .map(|r| r.fields.iter().map(|f| (f.at, f.text.clone())).collect())
                .collect()
        };
        let first = shape(&v);
        let walk = v.walkable();
        assert!(walk.len() > 10, "too little to walk: {walk:?}");
        for _ in 0..walk.len() {
            v.key(Press::Down);
            assert_eq!(
                shape(&v),
                first,
                "the list moved with the cursor on {:?}",
                v.focus()
            );
        }
        assert_eq!(
            v.focus(),
            walk.last().cloned(),
            "the arrows did not reach the end"
        );
        // AND THE CURSOR IS A COLOUR: the row it is on wears the accent.
        let rows = v.list(0, 90, &t);
        let at = v.cursor_row(&rows).expect("a cursor row");
        assert!(
            rows[at]
                .fields
                .iter()
                .any(|f| f.color == crate::chrome::Kind::Setting.tones(true).0),
            "{:?}",
            rows[at].fields
        );
    }

    /// FAILS IF: what the status says moves the list under it — the title
    /// is as tall as the longest status the view can say, so a status that
    /// changes keeps every row where it was.
    #[test]
    fn a_status_change_never_moves_the_list() {
        let t = texts();
        let views = [
            a_view(),
            a_view().with_heads(Heads::Asking),
            a_view().with_heads(heard(Some(Ok(true)), Some(Ok(true)))),
            a_view().with_heads(heard(Some(Ok(false)), Some(Ok(false)))),
            a_view().with_heads(heard(
                Some(Err(ronoctua_software::Refusal::RateLimited)),
                None,
            )),
            a_view().with_brain(Brain {
                failed: true,
                ..Brain::default()
            }),
        ];
        for (room, name) in [
            (90_u16, Some("Atualizar o sistema")),
            (60, Some("Atualizar o sistema")),
            (90, None),
            (40, None),
        ] {
            let tall: Vec<usize> = views
                .iter()
                .map(|v| v.status_lines(room, name, &t).len())
                .collect();
            assert!(
                tall.windows(2).all(|w| w[0] == w[1]),
                "{room} {name:?}: the title's height moved with the status: {tall:?}"
            );
            let longest = Status::all()
                .into_iter()
                .map(|s| crate::chrome::note_lines(&s.sentence(&t), room).len())
                .max()
                .expect("statuses");
            assert!(tall[0] > longest, "{room}: shorter than the longest status");
        }
        // THE LIST UNDER IT IS THE SAME ROWS, but for the status's own.
        let lists: Vec<String> = views[..4]
            .iter()
            .map(|v| said_all(&v.list(0, 90, &t)))
            .collect();
        assert!(lists.windows(2).all(|w| w[0] == w[1]), "{lists:#?}");
        // NOR DOES WHAT THE RUN DOES: idle, counting down, queued and running
        // keep the options and the steps on the same rows — with a system
        // that waits for the restart as well, whose status gives way to the
        // run's.
        let staged = module::Records {
            staged: Some(module::NextBoot {
                toplevel: String::new(),
                guarded: true,
            }),
            ..module::Records::default()
        };
        let runs = [
            Brain::default(),
            Brain {
                announced: true,
                ..Brain::default()
            },
            Brain {
                queued: true,
                ..Brain::default()
            },
            Brain {
                running: true,
                ..Brain::default()
            },
        ];
        for records in [module::Records::default(), staged] {
            let placed: Vec<(usize, usize)> = runs
                .iter()
                .map(|b| {
                    let rows = a_view()
                        .with_records(records.clone(), String::new())
                        .with_brain(*b)
                        .list(0, 90, &t);
                    let options = rows
                        .iter()
                        .position(|r| r.item == Some(Item::Heading(Section::Options)))
                        .expect("the options");
                    (options, rows.len())
                })
                .collect();
            assert!(
                placed.windows(2).all(|w| w[0] == w[1]),
                "the run moved the rows under it: {placed:?}"
            );
        }
    }

    /// FAILS IF: the window's name sits beside the status when the longest
    /// status would not fit beside it, or alone on its row when it would —
    /// and the status line is a control only while it says GitHub's answer.
    #[test]
    fn the_name_is_alone_on_line_zero_when_the_longest_status_does_not_fit() {
        let t = texts();
        let name = t.t(module::key::CMD_SYSTEM_UPDATE);
        let longest = Status::all()
            .into_iter()
            .map(|s| layout::width(&s.sentence(&t)))
            .max()
            .expect("statuses");
        let v = a_view().with_heads(heard(Some(Ok(true)), None));
        let wide = longest + layout::width(&name) + 2 * tokens::space::MODELINE_SEGMENTGAP;
        let beside = v.status_lines(wide, Some(&name), &t);
        assert!(
            beside[0].item == Some(Item::Status) && said(&beside[0]).ends_with(&name),
            "{:?}",
            said(&beside[0])
        );
        let narrow = v.status_lines(60, Some(&name), &t);
        assert_eq!(said(&narrow[0]), name, "the name is not alone on its row");
        assert_eq!(narrow[1].item, Some(Item::Status));
        assert!(narrow[1].takes, "GitHub's answer takes no click");
        // A STATUS OF THE BRAIN'S is no control.
        let failed = a_view().with_brain(Brain {
            failed: true,
            ..Brain::default()
        });
        assert!(
            failed.status_lines(90, None, &t).iter().all(|r| !r.takes),
            "a failed run's status takes a click"
        );
        // WHEN THE LAST UPDATE WORKED is its own quiet row.
        let worked = a_view().with_brain(Brain {
            worked: Some(1_762_179_600),
            ..Brain::default()
        });
        let title = said_all(&worked.status_lines(90, None, &t));
        assert!(title.contains("2025"), "{title}");
        assert_eq!(
            worked.status_lines(90, None, &t).len(),
            a_view().status_lines(90, None, &t).len(),
            "the last update's row moved the list"
        );
    }

    /// FAILS IF: the arrows do not walk one list — the status, the actions,
    /// the schedule, the options, the steps —, `TAB` and `S-TAB` do not jump
    /// to the first row of the next part and of the one before, round from
    /// the last to the first, or `Home` and `End` do not reach the ends.
    #[test]
    fn the_keys_walk_the_list_and_tab_jumps_its_parts() {
        let mut v = a_view().with_heads(heard(Some(Ok(true)), None));
        let walk = v.walkable();
        let first_step = Item::Step(module::steps()[0].id);
        assert_eq!(walk[0], Item::Status);
        assert_eq!(
            walk[1..4],
            [
                Item::Action(Action::Start),
                Item::Action(Action::Rehearse),
                Item::Schedule
            ]
        );
        assert!(walk.contains(&first_step));
        assert_eq!(
            v.focus(),
            Some(Item::Action(Action::Start)),
            "the cursor does not rest on what starts"
        );
        v.key(Press::Up);
        assert_eq!(v.focus(), Some(Item::Status));
        // TAB: the first of each part, round.
        let mut firsts = Vec::new();
        for _ in 0..4 {
            v.key(Press::Section);
            firsts.push(v.focus().expect("a cursor"));
        }
        assert_eq!(
            firsts,
            [
                Item::Action(Action::Start),
                Item::Page(crate::update_window_list::PageOf::Cache),
                first_step.clone(),
                Item::Status,
            ]
        );
        v.key(Press::SectionBack);
        assert_eq!(v.focus(), Some(first_step), "S-TAB did not go back round");
        assert_eq!(v.key(Press::Leap(Leap::First)), Errand::Leap(Leap::First));
        v.leap(Leap::First, 10);
        assert_eq!(v.focus(), Some(Item::Status));
        v.leap(Leap::Last, 10);
        assert_eq!(v.focus(), walk.last().cloned());
        v.leap(Leap::Up, 3);
        assert_eq!(v.focus(), walk.get(walk.len() - 4).cloned());
        // A QUESTION HOLDS THE CURSOR: nothing is walked, TAB goes nowhere.
        let mut asked = a_view().on_charge(flat());
        asked.key(Press::Start);
        assert!(asked.walkable().is_empty());
        assert_eq!(asked.key(Press::Section), Errand::Nothing);
    }

    /// FAILS IF: the first click on a row does anything but put the cursor
    /// there, or the second (or `RET`) does not do what the row does — ask
    /// GitHub again, start, try it, open the brain, open a page, switch a
    /// step, answer the countdown, stop —, or a row that cannot act now does
    /// instead of saying why.
    #[test]
    fn the_second_click_and_ret_do_what_the_row_does() {
        use crate::update_window_list::PageOf;
        // THE CURSOR ELSEWHERE FIRST: a click on the row the cursor is on is
        // the second click already, as on any list's cursor row.
        let twice = |v: &mut View, item: Item| {
            let elsewhere = if item == Item::Schedule {
                Item::Action(Action::Rehearse)
            } else {
                Item::Schedule
            };
            let _ = v.clicked(elsewhere);
            assert_eq!(
                v.clicked(item.clone()),
                Errand::Moved,
                "{item:?}: the first click"
            );
            assert_eq!(v.focus(), Some(item.clone()));
            v.clicked(item)
        };
        let online = || a_view().with_heads(heard(Some(Ok(true)), None));
        let mut v = online();
        assert_eq!(twice(&mut v, Item::Status), Errand::Recheck);
        assert_eq!(v.key(Press::Enter), Errand::Recheck, "RET on the status");
        assert_eq!(twice(&mut v, Item::Action(Action::Start)), Errand::Start);
        assert_eq!(
            twice(&mut v, Item::Action(Action::Rehearse)),
            Errand::Rehearse
        );
        assert_eq!(twice(&mut v, Item::Schedule), Errand::OpenBrain);
        assert_eq!(v.key(Press::Enter), Errand::OpenBrain);
        let step = shipped(true);
        assert_eq!(
            twice(&mut v, Item::Step(step.id)),
            Errand::Switch {
                id: step.id,
                on: false
            }
        );
        type Opened = fn(&View) -> bool;
        let pages: [(PageOf, Opened); 3] = [
            (PageOf::Cache, |v| v.cache_page().is_some()),
            (PageOf::Follow, |v| v.follow_page().is_some()),
            (PageOf::Parts, |v| v.parts_page().is_some()),
        ];
        for (page, opened) in pages {
            let mut v = online();
            assert_eq!(twice(&mut v, Item::Page(page)), Errand::Moved);
            assert!(opened(&v), "{page:?}: the second click opened no page");
        }
        // THE COUNTDOWN'S TWO, and the run's stop.
        let mut v = online().with_brain(Brain {
            announced: true,
            queued: true,
            ..Brain::default()
        });
        assert_eq!(
            twice(&mut v, Item::Action(Action::StartNow)),
            Errand::Answer(true)
        );
        assert_eq!(
            twice(&mut v, Item::Action(Action::NotNow)),
            Errand::Answer(false)
        );
        assert_eq!(
            v.clicked(Item::Action(Action::Start)),
            Errand::Nothing,
            "a row that is not there"
        );
        // QUEUED: "update now" and the rehearsal are there, refusing, and
        // say why.
        let mut v = online().with_brain(Brain {
            queued: true,
            ..Brain::default()
        });
        assert_eq!(
            twice(&mut v, Item::Action(Action::Start)),
            Errand::Say(module::key::SYSTEM_UPDATE_START_QUEUED)
        );
        assert_eq!(
            twice(&mut v, Item::Action(Action::Rehearse)),
            Errand::Say(module::key::SYSTEM_UPDATE_REHEARSE_BUSY)
        );
        // NOT ON A RUN'S ANSWER THE PERSON NEVER CHOSE: a view that opens on
        // a run stops nothing with its `RET` or its first click on "stop".
        let running = Brain {
            running: true,
            ..Brain::default()
        };
        let mut v = online().with_brain(running);
        assert_ne!(v.key(Press::Enter), Errand::Answer(false), "RET stopped it");
        let mut v = online().with_brain(running);
        assert_eq!(v.clicked(Item::Action(Action::Stop)), Errand::Moved);
        assert_eq!(v.clicked(Item::Action(Action::Stop)), Errand::Answer(false));
        // AND THE PERSON'S CHOICE ENDS WITH ITS RUN.
        let mut v = online();
        let _ = v.heard_the_brain(&a_mind(module::ID, &[]));
        assert_eq!(v.clicked(Item::Action(Action::Stop)), Errand::Moved);
        let _ = v.heard_the_brain(&a_mind("", &[]));
        let _ = v.heard_the_brain(&a_mind(module::ID, &[]));
        assert_ne!(v.focus(), Some(Item::Action(Action::Stop)));
        assert_ne!(v.key(Press::Enter), Errand::Answer(false), "RET stopped it");
        // A REHEARSAL UNDER WAY is one for a view that opens in its middle —
        // the brain says it of the run — and its stop is the rehearsal's.
        let mut v = online();
        let mut mind = a_mind(module::ID, &[]);
        mind.running_rehearsal = true;
        let _ = v.heard_the_brain(&mind);
        assert_eq!(v.status(), Status::Rehearsing);
        assert_eq!(v.taught(), OF_THE_REHEARSAL);
        let rows = v.list(0, 90, &texts());
        let (_, stop) = row_of(&rows, &Item::Action(Action::Stop));
        assert!(
            said(&stop).contains(&texts().t(STOP_REHEARSAL)),
            "{}",
            said(&stop)
        );
        // A HEADING takes no click.
        assert_eq!(
            online().clicked(Item::Heading(Section::Steps)),
            Errand::Nothing
        );
    }

    /// FAILS IF: `RET` or a click on a step asks for a flip rather than the
    /// state the step is to be in, a step's box does not switch it at once
    /// while a click on its name only selects it, or a step that runs as
    /// root is asked of the brain instead of refused here with why.
    #[test]
    fn a_step_is_switched_to_a_state_and_a_root_step_is_refused_with_why() {
        let on = shipped(true);
        let off = shipped(false);
        let mut v = a_view();
        // THE BOX: at once, to the other state — and the cursor goes there.
        assert_eq!(
            v.clicked_box(off.id),
            Errand::Switch {
                id: off.id,
                on: true
            }
        );
        assert_eq!(v.focus(), Some(Item::Step(off.id)));
        assert_eq!(
            v.clicked_box(on.id),
            Errand::Switch {
                id: on.id,
                on: false
            }
        );
        // THE NAME: the first click selects, the second switches.
        assert_eq!(v.clicked(Item::Step(off.id)), Errand::Moved);
        assert_eq!(
            v.clicked(Item::Step(off.id)),
            Errand::Switch {
                id: off.id,
                on: true
            }
        );
        // A STATE, NOT A FLIP: pressed twice before the brain's new mind
        // comes, it asks the same.
        assert_eq!(
            v.key(Press::Enter),
            Errand::Switch {
                id: off.id,
                on: true
            }
        );
        // ROOT'S: refused here, with why.
        let root = roots();
        assert_eq!(
            v.clicked_box(root.id),
            Errand::Say(module::key::SYSTEM_UPDATE_SWITCH_ROOT)
        );
        let t = texts();
        let rows = v.list(0, 90, &t);
        let (_, row) = row_of(&rows, &Item::Step(root.id));
        assert!(
            said(&row).contains(&tokens::icon::READONLY.to_string()),
            "a root step wears no lock: {}",
            said(&row)
        );
        let explained = v.explained(&Item::Step(root.id), crate::door::Gesture::Ret, &t);
        assert!(explained.gesture.is_none(), "a root step offers a gesture");
        assert!(
            explained
                .said
                .iter()
                .any(|(s, c)| *s == t.t(module::key::SYSTEM_UPDATE_SWITCH_ROOT)
                    && *c == tokens::color::STATE_WARNING),
            "{explained:?}"
        );
        // THE BOX IS ITS OWN CELLS, at the head of the row.
        let (_, row) = row_of(&rows, &Item::Step(off.id));
        let (from, to) = row.boxed.expect("a box");
        assert!(row.on_the_box(from) && !row.on_the_box(to));
        assert!(
            row.fields.iter().any(|f| f.at == from),
            "nothing drawn in the box"
        );
    }

    /// FAILS IF: a step that is off does not say who turned it off — the
    /// distro, this machine's table (one that differs from what ships), or
    /// the person — or one turned on against what ships does not say who;
    /// or its name is not in the colour of whether it runs.
    #[test]
    fn every_off_step_says_who_turned_it_off() {
        let t = texts();
        let on = shipped(true);
        let off = shipped(false);
        let others: Vec<&ronoctua_brain::Step> = module::steps()
            .iter()
            .filter(|s| !s.needs_root && s.id != on.id && s.id != off.id)
            .collect();
        // THE MACHINE'S TABLE: every step as it ships, but `on` switched off
        // and `off` switched on — and the person's flips over two others.
        let mut table = String::new();
        for s in module::steps() {
            let here = if s.id == on.id {
                false
            } else if s.id == off.id {
                true
            } else {
                s.on_by_default
            };
            table.push_str(&format!("{} {}\n", s.id, if here { "on" } else { "off" }));
        }
        let by_you_off = others
            .iter()
            .find(|s| s.on_by_default)
            .expect("another step that ships on");
        let by_you_on = others
            .iter()
            .find(|s| !s.on_by_default)
            .expect("another step that ships off");
        let flips = format!(
            "{} {}\n{} {}\n",
            module::ID,
            by_you_off.id,
            module::ID,
            by_you_on.id
        );
        let v = a_view().with_steps(steps_with(&table, &flips));
        let rows = v.list(0, 120, &t);
        let short = |k: &str| t.t(k);
        for (step, word, runs) in [
            (on, module::key::SYSTEM_UPDATE_STEP_OFF_HERE, false),
            (off, module::key::SYSTEM_UPDATE_STEP_ON_HERE, true),
            (
                *by_you_off,
                module::key::SYSTEM_UPDATE_STEP_OFF_BY_YOU,
                false,
            ),
            (*by_you_on, module::key::SYSTEM_UPDATE_STEP_ON_BY_YOU, true),
        ] {
            let (_, row) = row_of(&rows, &Item::Step(step.id));
            assert!(
                said(&row).contains(&short(word)),
                "{}: {}",
                step.id,
                said(&row)
            );
            let name = row
                .fields
                .iter()
                .find(|f| f.text == t.t(step.name_key))
                .expect("its name");
            assert_eq!(
                name.color,
                if runs {
                    tokens::color::TEXT_PRIMARY
                } else {
                    tokens::color::TEXT_DIM
                },
                "{}: the name is not in the colour of whether it runs",
                step.id
            );
        }
        // ONE THE DISTRO SHIPS OFF, untouched: shipped off.
        let plain = a_view();
        let rows = plain.list(0, 120, &t);
        let (_, row) = row_of(&rows, &Item::Step(off.id));
        assert!(
            said(&row).contains(&short(module::key::SYSTEM_UPDATE_STEP_OFF_SHIPPED)),
            "{}",
            said(&row)
        );
        // AND THE DOOR SAYS IT WHOLE: what switching will do right under what
        // the step does — the first lines the foot shows —, the command last.
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for step in [on, off, *by_you_off, *by_you_on] {
                let e = v.explained(&Item::Step(step.id), crate::door::Gesture::Selects, &t);
                let whole: String = e.said.iter().map(|(s, _)| s.clone()).collect();
                assert!(!whole.contains('['), "{language}: {whole}");
                assert!(e.gesture.is_some(), "{language}: {}", step.id);
                let will = &e.said[1].0;
                let switched_on = !v
                    .steps()
                    .iter()
                    .find(|s| s.id == step.id)
                    .expect("the step")
                    .on;
                assert_eq!(
                    *will,
                    t.t_with(
                        module::key::SYSTEM_UPDATE_SWITCH_WILL,
                        &[("on", if switched_on { "on" } else { "off" })]
                    ),
                    "{language}: {}",
                    step.id
                );
                assert_eq!(e.said.last().map(|(s, _)| s.as_str()), Some(step.command));
            }
        }
    }

    /// FAILS IF: what a row is, explained where the door explains it, lacks
    /// its name or a sentence in either language, or a row a click takes
    /// says no gesture — or a disabled one offers one instead of why.
    #[test]
    fn every_row_is_explained_with_what_will_happen() {
        use crate::door::Gesture;
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            for v in [
                a_view().with_records(every_record(), "SUPER s v".to_owned()),
                a_view().with_brain(Brain {
                    queued: true,
                    ..Brain::default()
                }),
            ] {
                for row in v
                    .list(0, 90, &t)
                    .into_iter()
                    .chain(v.status_lines(90, None, &t))
                {
                    let Some(item) = row.item.clone() else {
                        continue;
                    };
                    let e = v.explained(&item, Gesture::Selects, &t);
                    assert!(!e.name.is_empty(), "{language}: {item:?} has no name");
                    assert!(!e.said.is_empty(), "{language}: {item:?} says nothing");
                    let whole: String = e
                        .said
                        .iter()
                        .map(|(s, _)| s.as_str())
                        .chain(e.gesture.as_deref())
                        .collect();
                    assert!(!whole.contains('['), "{language}: {item:?}: {whole}");
                    // A ROW THAT REFUSES — an action that cannot act now, the
                    // output with nothing written, a step only the machine's
                    // configuration switches — offers no gesture, and says
                    // why in warning.
                    let refuses = matches!(item, Item::Action(a) if !v.acts(a))
                        || (item == Item::Page(crate::update_window_list::PageOf::Output)
                            && !v.has_output())
                        || matches!(item, Item::Step(id) if step_of(id).is_some_and(|s| s.needs_root));
                    if refuses {
                        assert!(e.gesture.is_none(), "{language}: {item:?}");
                        assert!(
                            e.said
                                .iter()
                                .any(|(_, c)| *c == tokens::color::STATE_WARNING),
                            "{language}: {item:?} does not say why"
                        );
                    } else if row.takes && !matches!(item, Item::Heading(_)) {
                        assert!(e.gesture.is_some(), "{language}: {item:?} has no gesture");
                    }
                }
            }
        }
        // THE GESTURE SAYS WHAT THE HAND DOES: a box at once, the cursor's
        // thing on `RET`, another on the second click.
        let t = texts();
        let v = a_view();
        let does = t.t(module::key::SYSTEM_UPDATE_ACT_START);
        for how in [Gesture::Ret, Gesture::Selects, Gesture::AtOnce] {
            assert_eq!(
                v.explained(&Item::Action(Action::Start), how, &t).gesture,
                Some(crate::door::foot::gesture(&t, how, &does))
            );
        }
    }

    /// FAILS IF: a row that cannot act now is not drawn refusing, with a
    /// short reason as its value and the whole of it where the door explains.
    #[test]
    fn a_disabled_row_says_why() {
        let t = texts();
        let v = a_view().with_brain(Brain {
            queued: true,
            ..Brain::default()
        });
        let rows = v.list(0, 90, &t);
        for (action, short) in [
            (Action::Start, module::key::SYSTEM_UPDATE_START_QUEUED_SHORT),
            (
                Action::Rehearse,
                module::key::SYSTEM_UPDATE_REHEARSE_BUSY_SHORT,
            ),
        ] {
            let (_, row) = row_of(&rows, &Item::Action(action));
            assert!(said(&row).contains(&t.t(short)), "{}", said(&row));
            assert!(
                row.fields
                    .iter()
                    .all(|f| f.color == crate::chrome::Kind::Disabled.tones(false).0),
                "{action:?} is not drawn refusing"
            );
        }
        // NOTHING WRITTEN YET: the output's row refuses, and says why.
        let (_, row) = row_of(
            &rows,
            &Item::Page(crate::update_window_list::PageOf::Output),
        );
        assert!(
            said(&row).contains(&t.t(module::key::SYSTEM_UPDATE_OUTPUT_NONE_SHORT)),
            "{}",
            said(&row)
        );
        let mut v = a_view();
        assert_eq!(
            v.key(Press::Output),
            Errand::Say(module::key::SYSTEM_UPDATE_OUTPUT_NONE)
        );
        v.heard(OutputLine::written(1, false, "", "a line"));
        assert_eq!(v.key(Press::Output), Errand::Moved);
        assert!(v.output_open());
    }

    /// FAILS IF: a state teaches a key that does nothing in it, or a key
    /// any state reads is not kept from the keyboard in every state (a
    /// physical `u` must never start an update), or one is not one key after
    /// `LEADER Alt`.
    #[test]
    fn each_state_teaches_only_what_it_does() {
        let ids = |v: &View| -> Vec<&str> { v.taught().iter().map(|(_, id)| *id).collect() };
        let state = |brain: Brain| a_view().with_brain(brain);
        let idle = a_view();
        for id in [START, REHEARSE, SCHEDULE, OUTPUT_OPEN] {
            assert!(ids(&idle).contains(&id), "idle teaches no {id}");
        }
        for absent in [PROCEED, ABORT, STOP, "system-update-output-end"] {
            assert!(!ids(&idle).contains(&absent), "idle teaches {absent}");
        }
        let queued = state(Brain {
            queued: true,
            ..Brain::default()
        });
        assert_eq!(queued.taught(), OF_THE_QUEUE);
        for absent in [START, REHEARSE, PROCEED, ABORT, STOP] {
            assert!(!ids(&queued).contains(&absent), "queued teaches {absent}");
        }
        let counting = state(Brain {
            announced: true,
            queued: true,
            ..Brain::default()
        });
        assert_eq!(counting.taught(), OF_THE_COUNTDOWN);
        assert!(ids(&counting).contains(&PROCEED) && ids(&counting).contains(&ABORT));
        for absent in [START, REHEARSE, STOP] {
            assert!(
                !ids(&counting).contains(&absent),
                "counting teaches {absent}"
            );
        }
        let running = state(Brain {
            running: true,
            ..Brain::default()
        });
        assert_eq!(running.taught(), OF_THE_RUN);
        assert!(ids(&running).contains(&STOP));
        assert!(ids(&running).contains(&"system-update-output-end"));
        for absent in [START, REHEARSE, PROCEED, ABORT] {
            assert!(!ids(&running).contains(&absent), "running teaches {absent}");
        }
        let mut reading = a_view();
        reading.heard(OutputLine::written(1, false, "", "a line"));
        reading.key(Press::Output);
        assert_eq!(reading.taught(), OF_THE_OUTPUT);
        // EVERY STATE'S LETTERS ARE KEPT FROM THE KEYBOARD, whatever is
        // taught now.
        let union: Vec<ronoctua_keymap::Key> =
            ["u", "s", "b", "o", "a", "p", "c", "f", "d", "G", "C-j"]
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
        // AND EVERY KEY A STATE TEACHES SAYS WHAT IT DOES, in both
        // languages.
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let (w, _) = a_window();
            for (_, id) in OF_EVERY_STATE.iter().flat_map(|t| t.iter()) {
                let said = w.explains(id, &t).unwrap_or_default();
                assert!(
                    !said.is_empty() && !said.contains('['),
                    "{language}: {id}: {said}"
                );
            }
        }
    }

    /// THE WINDOW TEACHES WHAT THE FOOTER TEACHES, every key a click.
    #[test]
    fn the_windows_header_teaches_the_views_keys() {
        let (w, ui) = a_window();
        let hint = w.hint(&ui);
        let words: String = hint.iter().map(|p| p.text.clone()).collect();
        for id in [START, SECTION, REHEARSE, SCHEDULE] {
            assert!(
                words.contains(&ui.texts.t(id)),
                "`{id}` is not taught: {words}"
            );
        }
        assert!(w.header_keys(), "the header's drawer is not this window's");
    }

    /// FAILS IF: `Esc` on a page closes the window instead of the page, the
    /// way back is not the row above the page's own — an arrow up from the
    /// page's first row, `RET` or a second click on it closing the page —, or
    /// a page opens without it.
    #[test]
    fn esc_closes_a_page_first_and_every_page_opens_with_the_way_back() {
        let t = texts();
        let opened: [fn() -> View; 4] = [
            || a_view().with_cache(crate::binary_cache::Readings::default()),
            || a_view().with_follow(on_the_follow_page()),
            || a_view().with_parts(on_the_parts_page()),
            || {
                let mut v = a_view();
                v.heard(OutputLine::written(1, false, "", "a line"));
                let _ = v.key(Press::Output);
                v
            },
        ];
        for open in opened {
            let mut v = open();
            assert!(v.page_open());
            let rows = v.list(0, 90, &t);
            assert_eq!(rows[0].item, Some(Item::Back), "{}", said_all(&rows));
            assert!(
                said(&rows[0]).contains(&t.t(module::key::SYSTEM_UPDATE_BACK)),
                "{}",
                said(&rows[0])
            );
            assert_eq!(v.key(Press::Escape), Errand::Moved, "Esc closed the window");
            assert!(!v.page_open(), "Esc left the page open");
            assert_eq!(v.key(Press::Escape), Errand::Close);
            // THE WAY BACK BY THE ARROWS, then `RET`.
            let mut v = open();
            if !v.output_open() {
                assert!(!v.on_the_way_back());
                v.key(Press::Up);
                assert!(v.on_the_way_back(), "an arrow up from the first row");
                v.key(Press::Down);
                assert!(!v.on_the_way_back());
                v.key(Press::Up);
            }
            assert_eq!(v.key(Press::Enter), Errand::Moved);
            assert!(!v.page_open(), "RET on the way back left the page open");
            // AND BY THE POINTER: the first click selects, the second goes.
            let mut v = open();
            if !v.output_open() {
                assert_eq!(v.clicked(Item::Back), Errand::Moved);
                assert!(v.page_open());
            }
            assert_eq!(v.clicked(Item::Back), Errand::Moved);
            assert!(!v.page_open(), "the second click left the page open");
        }
    }

    /// FAILS IF: with the cursor on the way back, ONE click on the row the
    /// page's own cursor last stood on — drawn unlit — does what a second
    /// click does, while the foot says the click only selects.
    #[test]
    fn from_the_way_back_the_first_click_on_a_page_row_only_selects() {
        let opened: [fn() -> View; 2] = [
            || a_view().with_follow(on_the_follow_page()),
            || a_view().with_parts(on_the_parts_page()),
        ];
        for open in opened {
            let mut v = open();
            let n = v.page_cursor().expect("the page's cursor");
            assert!(v.page_takes(n), "row {n} takes no click");
            v.key(Press::Up);
            assert!(v.on_the_way_back());
            assert_eq!(v.gesture_for(&Item::Row(n)), crate::door::Gesture::Selects);
            assert_eq!(
                v.clicked(Item::Row(n)),
                Errand::Moved,
                "the first click acted"
            );
            assert!(!v.on_the_way_back());
            assert_eq!(v.page_cursor(), Some(n));
            assert_eq!(v.gesture_for(&Item::Row(n)), crate::door::Gesture::Ret);
            assert_ne!(v.clicked(Item::Row(n)), Errand::Moved, "the second did not");
        }
        let mut v = a_view().with_follow(on_the_follow_page());
        v.key(Press::Up);
        let _ = v.clicked(Item::Row(0));
        assert_eq!(
            v.clicked(Item::Row(0)),
            Errand::Follow(crate::follow::Errand::Turn(false))
        );
    }

    /// FAILS IF: the output's page is not the last run's lines, each piece
    /// in its colour — a priority-3 line in the error's — under the way back,
    /// or a person who may not read the system's journal is not told so.
    #[test]
    fn the_output_page_is_the_last_runs_lines_in_their_colours() {
        let t = texts();
        let mut v = a_view();
        v.heard(OutputLine::written(1, false, "inv", "plain"));
        v.heard(OutputLine::written(2, false, "inv", "<3>it broke"));
        v.heard(OutputLine::written(
            3,
            false,
            "inv",
            "\u{1b}[32mgreen\u{1b}[0m done",
        ));
        let _ = v.key(Press::Output);
        let rows = v.list(0, 90, &t);
        assert_eq!(rows.len(), 6, "{}", said_all(&rows));
        assert_eq!(said(&rows[2]), t.t(module::key::SYSTEM_UPDATE_OUTPUT_TITLE));
        assert_eq!(said(&rows[4]), "it broke");
        assert!(
            rows[4]
                .fields
                .iter()
                .all(|f| f.color == tokens::color::STATE_ERROR)
        );
        assert!(
            rows[5].fields.len() > 1 && rows[5].fields[0].color != rows[5].fields[1].color,
            "{:?}",
            rows[5].fields
        );
        assert_eq!(v.list_len(90, &t), rows.len());
        assert_eq!(v.list_from(0, 90, &t, 4, 1), rows[4..5].to_vec());
        let hidden = v.clone().with_system_readable(false);
        let rows = hidden.list(0, 90, &t);
        let warned: String = rows
            .iter()
            .filter(|r| {
                r.fields
                    .iter()
                    .any(|f| f.color == tokens::color::STATE_WARNING)
            })
            .map(said)
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(
            warned.split_whitespace().collect::<Vec<_>>(),
            t.t(module::key::SYSTEM_UPDATE_OUTPUT_SYSTEM_HIDDEN)
                .split_whitespace()
                .collect::<Vec<_>>(),
            "the person is not told the root half is hidden"
        );
        // THE ARROWS MOVE THE OUTPUT, never a cursor: its lines take nothing.
        assert_eq!(v.key(Press::Down), Errand::Output(Output::Lines(1)));
        assert_eq!(v.key(Press::OutputEnd), Errand::Output(Output::End));
    }

    /// FAILS IF: the schedule's row does not say when the update happens by
    /// itself — every so many days, in which part of the day, and the next
    /// time — or why it will not, in both languages; or its second click, its
    /// `RET` and `b` do not open the brain on its task.
    #[test]
    fn the_schedule_says_when_and_opens_the_brains_task() {
        let mut mind = a_mind_with(false, false, None, 1_762_600_000);
        let task = mind.task_mut(module::ID).expect("the task");
        task.interval_days = 7;
        task.part_of_day = ronoctua_brain::PartOfDay::Afternoon;
        task.next = 1_762_600_000 + 3 * 86_400;
        for language in ["en-GB", "pt-BR"] {
            let t = ronoctua_i18n::Texts::fresh(language).expect("a language");
            let mut v = a_view();
            assert_eq!(
                v.schedule_value(&t),
                t.t(module::key::SYSTEM_UPDATE_SCHEDULE_NO_BRAIN)
            );
            let _ = v.heard_the_brain(&mind);
            let said = v.schedule_value(&t);
            for needle in [
                t.t_args(
                    module::key::SYSTEM_UPDATE_SCHEDULE_EVERY,
                    &[("days", ronoctua_i18n::Arg::Number(7))],
                ),
                t.t_with(
                    module::key::SYSTEM_UPDATE_SCHEDULE_PART,
                    &[("part", "afternoon")],
                ),
            ] {
                assert!(said.contains(&needle), "{language}: `{needle}` in `{said}`");
            }
            assert!(!said.contains('['), "{language}: {said}");
            // THE BRAIN SAYS WHY NOT: off, quiet, overdue.
            let mut quiet = mind.clone();
            let task = quiet.task_mut(module::ID).expect("the task");
            task.next = 0;
            task.why_not = "quiet".to_owned();
            task.why_key = "brain-why-quiet".to_owned();
            let _ = v.heard_the_brain(&quiet);
            assert_eq!(
                v.schedule_value(&t),
                t.t(module::key::SYSTEM_UPDATE_SCHEDULE_QUIET),
                "a quiet brain is said in the row's own words"
            );
            let task = quiet.task_mut(module::ID).expect("the task");
            task.enabled = false;
            task.why_not = "off".to_owned();
            let _ = v.heard_the_brain(&quiet);
            assert_eq!(
                v.schedule_value(&t),
                t.t(module::key::SYSTEM_UPDATE_SCHEDULE_OFF)
            );
            let task = quiet.task_mut(module::ID).expect("the task");
            task.enabled = true;
            task.why_not = String::new();
            let _ = v.heard_the_brain(&quiet);
            assert_eq!(
                v.schedule_value(&t),
                t.t(module::key::SYSTEM_UPDATE_SCHEDULE_OVERDUE)
            );
        }
        let mut v = a_view();
        assert_eq!(v.key(Press::Schedule), Errand::OpenBrain);
        let _ = v.clicked(Item::Schedule);
        assert_eq!(v.clicked(Item::Schedule), Errand::OpenBrain);
        assert_eq!(v.key(Press::Enter), Errand::OpenBrain);
    }

    /// FAILS IF: a rehearsal is offered while an update is queued, counting
    /// down or running, or while one is asked for already — or `s` then
    /// does anything but say why.
    #[test]
    fn a_rehearsal_is_offered_only_while_nothing_is_under_way() {
        assert_eq!(a_view().key(Press::Rehearse), Errand::Rehearse);
        for brain in [
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
            assert_eq!(
                a_view().with_brain(brain).key(Press::Rehearse),
                Errand::Say(module::key::SYSTEM_UPDATE_REHEARSE_BUSY),
                "{brain:?}"
            );
        }
        let asked = a_view().with_brain(Brain {
            rehearsing: true,
            ..Brain::default()
        });
        assert!(!asked.acts(Action::Rehearse));
        // WHAT THE LAST REHEARSAL MEASURED is said with it.
        let t = texts();
        let v = a_view().with_rehearsed(Some(module::Rehearsed {
            at: ronoctua_person::Stamp::from_unix_s(1_762_179_600),
            download_bytes: 3 * 1024 * 1024,
            builds: 2,
        }));
        let e = v.explained(
            &Item::Action(Action::Rehearse),
            crate::door::Gesture::Ret,
            &t,
        );
        assert!(
            e.said
                .iter()
                .any(|(s, _)| s.contains("2025") && s.contains('2')),
            "{e:?}"
        );
        // NOTHING TO BUILD is none, never one — Portuguese's `one` holds 0.
        let pt = ronoctua_i18n::Texts::fresh("pt-BR").expect("pt-BR has to exist");
        let v = a_view().with_rehearsed(Some(module::Rehearsed {
            at: ronoctua_person::Stamp::from_unix_s(1_762_179_600),
            download_bytes: 0,
            builds: 0,
        }));
        let said = v.rehearsed_said(&pt).expect("the rehearsal");
        assert!(said.contains("nenhum pacote"), "{said}");
    }

    /// FAILS IF: what the machine's records say is not among the notices,
    /// each only when it is there — the conflicts with the keys that open
    /// them (and a click that does), the trial in the machine's words, why
    /// there is none, a hold with its short revision and its day, a checkout
    /// — or a system that waits is not one of them whatever the status says.
    #[test]
    fn the_records_are_said_as_notices() {
        let t = texts();
        let v = a_view().with_records(every_record(), "SUPER s v".to_owned());
        let rows = v.list(0, 200, &t);
        let notices: String = rows
            .iter()
            .filter(|r| r.section == Some(Section::Notices))
            .map(said)
            .collect::<Vec<_>>()
            .join("\n");
        for needle in [
            "SUPER s v",
            &t.t(key::MACHINE_TRIAL_FAILED),
            &t.t(key::MACHINE_NO_TRIAL_NOT_IN_MENU),
            "613862c ",
            "2026-10-05",
            &t.t(module::key::SYSTEM_UPDATE_RECORDS_CHECKOUT),
        ] {
            assert!(
                notices.contains(needle),
                "`{needle}` is not said: {notices}"
            );
        }
        assert!(
            !notices.contains("613862c5"),
            "the revision was not shortened"
        );
        assert!(!notices.contains("T12:00"), "the stamp was not made a day");
        assert!(!notices.contains('['), "a key with no text: {notices}");
        // THE SYSTEM THAT WAITS is a notice whatever the status says: a
        // run's status takes the restart's place, and a notice that came and
        // went with it would move every row under it.
        assert_eq!(v.status(), Status::Restart { menu_only: false });
        assert_eq!(
            notices
                .matches(&t.t(module::key::SYSTEM_UPDATE_RECORDS_STAGED_UNGUARDED))
                .count(),
            1,
            "{notices}"
        );
        // THE CONFLICTS' ROW: the first click selects it, the second opens
        // the belongings, as `RET` does.
        let mut v = v;
        assert_eq!(v.clicked(Item::Belongings), Errand::Moved);
        assert_eq!(v.clicked(Item::Belongings), Errand::OpenBelongings);
        assert_eq!(v.key(Press::Enter), Errand::OpenBelongings);
        // A LONG SENTENCE IS ONE CONTROL over the rows it wraps to.
        let narrow = v.list(0, 40, &t);
        let lines: Vec<&ListRow> = narrow
            .iter()
            .filter(|r| r.item == Some(Item::Belongings))
            .collect();
        assert!(lines.len() > 1, "the conflicts fit one row at 40 cells");
        assert!(!lines[0].continues && lines[1..].iter().all(|r| r.continues && r.takes));
    }

    /// FAILS IF: a gesture of the machine's is not offered where the records
    /// offer it — the try-again under a failed trial, the let-go under a
    /// hold, the hold among the options when nothing is held —, or its
    /// click does not ask first, the no first, its second click answering.
    #[test]
    fn a_gesture_is_asked_where_its_row_is_the_no_first() {
        use crate::machine_gesture::Gesture;
        let t = texts();
        let mut v = a_view().with_records(failed_unheld(), String::new());
        let rows = v.list(0, 120, &t);
        let (_, again) = row_of(&rows, &Item::Gesture(Gesture::TryAgain));
        assert_eq!(again.section, Some(Section::Notices));
        let (_, hold) = row_of(&rows, &Item::Gesture(Gesture::Hold));
        assert_eq!(hold.section, Some(Section::Options));
        assert!(said(&hold).contains("613862c"), "{}", said(&hold));
        assert_eq!(v.clicked(Item::Gesture(Gesture::Hold)), Errand::Moved);
        let rows = v.list(0, 120, &t);
        let answers: Vec<&ListRow> = rows
            .iter()
            .filter(|r| matches!(r.item, Some(Item::Answer(..))))
            .collect();
        assert_eq!(
            answers.iter().map(|r| r.item.clone()).collect::<Vec<_>>(),
            [
                Some(Item::Answer(
                    crate::update_window_list::Answering::Gesture,
                    false
                )),
                Some(Item::Answer(
                    crate::update_window_list::Answering::Gesture,
                    true
                )),
            ],
            "the no is not first"
        );
        assert_eq!(answers[0].fields[0].color, tokens::color::TEXT_HERE_ACTS);
        assert!(
            v.walkable().is_empty(),
            "a question lets the arrows walk the list"
        );
        assert_eq!(
            v.clicked(Item::Answer(
                crate::update_window_list::Answering::Gesture,
                true
            )),
            Errand::Moved
        );
        assert_eq!(
            v.clicked(Item::Answer(
                crate::update_window_list::Answering::Gesture,
                true
            )),
            Errand::Machine(Gesture::Hold)
        );
        // HELD: let go among the notices, no hold among the options.
        let held = a_view().with_records(
            module::Records {
                hold: Some(module::HeldAt {
                    rev: "613862c5a0ff".to_owned(),
                    since: "2026-10-05T12:00:00Z".to_owned(),
                }),
                revision: Some("613862c5a0ff".to_owned()),
                ..module::Records::default()
            },
            String::new(),
        );
        let rows = held.list(0, 120, &t);
        assert_eq!(
            row_of(&rows, &Item::Gesture(Gesture::LetGo)).1.section,
            Some(Section::Notices)
        );
        assert!(
            !rows
                .iter()
                .any(|r| r.item == Some(Item::Gesture(Gesture::Hold)))
        );
    }

    /// FAILS IF: a module of the person's that waits is not said with the
    /// hash an approval must name and a row that asks, one approved as
    /// shown is offered again, or the approval is not asked the no first.
    #[test]
    fn a_module_that_waits_is_approved_where_its_row_is() {
        let t = texts();
        let mut v = a_view().with_approvals(two_waiting());
        let rows = v.list(0, 200, &t);
        let notices = said_all(&rows);
        assert!(notices.contains(TEA_HASH), "{notices}");
        assert!(
            rows.iter()
                .any(|r| r.item == Some(Item::Approve("my-tea".to_owned())))
        );
        assert!(
            !rows
                .iter()
                .any(|r| r.item == Some(Item::Approve("my-cake".to_owned()))),
            "an approved module is offered again"
        );
        assert_eq!(v.clicked(Item::Approve("my-tea".to_owned())), Errand::Moved);
        assert!(v.approval_question().is_some());
        // THE NO FIRST, the cursor born on it: the first click on it picks
        // it — the screen put the cursor there, not the person —, the second
        // answers.
        let no = Item::Answer(crate::update_window_list::Answering::Approval, false);
        let rows = v.list(0, 200, &t);
        assert_eq!(
            rows.get(v.cursor_row(&rows).expect("a cursor"))
                .and_then(|r| r.item.clone()),
            Some(no.clone()),
            "the cursor was not born on the no"
        );
        assert_eq!(v.clicked(no.clone()), Errand::Moved);
        assert!(v.approval_question().is_some(), "the first click answered");
        assert_eq!(v.clicked(no), Errand::Moved);
        assert!(
            v.approval_question().is_none(),
            "the second click did not answer"
        );
    }

    /// FAILS IF: below the battery's floor a run by hand does not ask first,
    /// the no first and the cursor on it, under the row that asked — or the
    /// arrows, `RET`, `Esc` and two clicks do not answer it.
    #[test]
    fn a_run_by_hand_below_the_floor_asks_under_its_row_the_no_first() {
        let t = texts();
        let mut v = a_view().on_charge(flat());
        assert_eq!(
            v.key(Press::Start),
            Errand::Moved,
            "it started without asking"
        );
        let rows = v.list(0, 80, &t);
        let (start, _) = row_of(&rows, &Item::Action(Action::Start));
        let answers: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter(|(_, r)| matches!(r.item, Some(Item::Answer(..))))
            .map(|(n, _)| n)
            .collect();
        assert_eq!(answers.len(), 2);
        assert!(answers[0] > start, "the question is not under its row");
        assert_eq!(
            rows[answers[0]].item,
            Some(Item::Answer(
                crate::update_window_list::Answering::Run,
                false
            )),
            "the no is not first"
        );
        assert_eq!(
            v.cursor_row(&rows),
            Some(answers[0]),
            "the cursor is not on the no"
        );
        let Cost::Battery(Some(charge)) = v.asking().expect("asked").cost else {
            panic!("not the battery's question");
        };
        assert!(said_all(&rows).contains(&charge.to_string()));
        assert_eq!(v.key(Press::Down), Errand::Moved);
        assert_eq!(v.key(Press::Up), Errand::Moved);
        assert_eq!(
            v.key(Press::Enter),
            Errand::NotStarted,
            "the no started something"
        );
        v.key(Press::Start);
        assert_eq!(v.key(Press::Escape), Errand::NotStarted, "`Esc` is the no");
        v.key(Press::Start);
        let yes = Item::Answer(crate::update_window_list::Answering::Run, true);
        assert_eq!(v.clicked(yes.clone()), Errand::Moved);
        assert_eq!(v.clicked(yes), Errand::StartHere { on_battery: true });
        // THE METERED ONE: its own words, the no leaving it queued.
        let mut v = a_view();
        v.ask_metered();
        let rows = v.list(0, 80, &t);
        let text = said_all(&rows);
        assert!(
            text.contains(&t.t(module::key::SYSTEM_UPDATE_ANSWER_NO_METERED)),
            "{text}"
        );
        assert_eq!(v.key(Press::Escape), Errand::KeptWaiting);
        assert_eq!(
            a_view().key(Press::Start),
            Errand::Start,
            "on mains it asked"
        );
    }

    /// FAILS IF: on the binary cache's page a row is still what the click
    /// takes under its grave question — whose answers come under the row
    /// that asked, the no first —, or, with no question asked, no row is.
    #[test]
    fn the_cache_pages_rows_take_no_click_under_its_question() {
        use crate::binary_cache::{Readings, Row as PageRow, TokenState, rows_of};
        let t = texts();
        let readings = Readings {
            named: ronoctua_cachix::settings::Named::Cache(ronoctua_person::options::BinaryCache {
                name: "someone".to_owned(),
                public_key: "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ="
                    .to_owned(),
                visibility: ronoctua_person::options::Visibility::Public,
            }),
            token: TokenState::Kept,
            ..Readings::default()
        };
        let remove = rows_of(&readings)
            .iter()
            .position(|r| *r == PageRow::Remove)
            .expect("a cache is removable");
        let mut v = a_view().with_cache(readings);
        assert!(
            v.list(0, 90, &t)
                .iter()
                .any(|r| r.takes && matches!(r.item, Some(Item::Row(_))))
        );
        assert_eq!(v.clicked(Item::Row(remove)), Errand::Moved);
        assert_eq!(v.clicked(Item::Row(remove)), Errand::Moved);
        let rows = v.list(0, 90, &t);
        assert!(
            !rows
                .iter()
                .any(|r| r.takes && matches!(r.item, Some(Item::Row(_)))),
            "a row takes the click under the question"
        );
        let (asked_at, _) = row_of(&rows, &Item::Row(remove));
        let (no, _) = row_of(
            &rows,
            &Item::Answer(crate::update_window_list::Answering::Grave, false),
        );
        let (yes, _) = row_of(
            &rows,
            &Item::Answer(crate::update_window_list::Answering::Grave, true),
        );
        assert!(asked_at < no && no < yes, "{}", said_all(&rows));
        assert_eq!(
            v.cursor_row(&rows),
            Some(no),
            "the cursor is not born on the no"
        );
        assert_eq!(
            v.clicked(Item::Answer(
                crate::update_window_list::Answering::Grave,
                true
            )),
            Errand::Moved
        );
        assert!(matches!(
            v.clicked(Item::Answer(
                crate::update_window_list::Answering::Grave,
                true
            )),
            Errand::Cache(crate::binary_cache::Errand::Edit(
                crate::binary_cache::Edit::Remove
            ))
        ));
    }

    /// FAILS IF: `f` and `d` do not turn the view to their pages and back,
    /// a page does not teach its own keys, its rows are not the page's and
    /// what each is not said where the door explains — or the parts' `h`
    /// does not ask for the file.
    #[test]
    fn the_follow_and_parts_pages_are_turned_to_and_explained() {
        let t = texts();
        let mut v = a_view();
        assert_eq!(v.key(Press::Follow), Errand::Moved);
        assert!(v.follow_page().is_some());
        assert_eq!(v.taught(), OF_THE_FOLLOW);
        assert_eq!(v.key(Press::Follow), Errand::Moved);
        assert!(!v.page_open());
        let v = a_view().with_follow(on_the_follow_page());
        let rows = v.list(0, 90, &t);
        let page_rows: Vec<usize> = rows
            .iter()
            .filter_map(|r| match r.item {
                Some(Item::Row(n)) => Some(n),
                _ => None,
            })
            .collect();
        assert_eq!(
            page_rows,
            (0..crate::follow::rows_of(&on_the_follow_page()).len()).collect::<Vec<_>>()
        );
        for n in page_rows {
            let e = v.explained(&Item::Row(n), crate::door::Gesture::Selects, &t);
            assert!(!e.name.is_empty() && !e.said.is_empty(), "row {n}: {e:?}");
        }
        let mut v = a_view().with_parts(on_the_parts_page());
        assert_eq!(v.taught(), OF_THE_PARTS);
        let rows = v.list(0, 90, &t);
        assert!(rows.iter().any(|r| r.item == Some(Item::Row(1)) && r.takes));
        let e = v.explained(&Item::Row(1), crate::door::Gesture::Ret, &t);
        assert!(
            e.said
                .iter()
                .any(|(s, c)| *c == tokens::color::STATE_WARNING && !s.is_empty()),
            "{e:?}"
        );
        assert_eq!(v.clicked(Item::Row(1)), Errand::Moved);
        assert!(
            matches!(v.key(Press::HandIn), Errand::Moved),
            "h asked for nothing"
        );
        assert!(v.picking(), "no picker for the file");
    }

    // -----------------------------------------------------------------------
    // THE WINDOW: its foot, its bars, its wheel and its live output
    // -----------------------------------------------------------------------

    /// A pointer's event of `kind` at `at`, on the test's surface.
    fn event(ui: &Ui, at: (f64, f64), kind: PointerEventKind) -> PointerEvent {
        PointerEvent {
            surface: ui.surface.clone(),
            position: at,
            kind,
        }
    }

    /// The middle of cell `cell` — counted from where content begins — of
    /// line `line`.
    fn point(ui: &Ui, cell: u16, line: u16) -> (f64, f64) {
        let px = tokens::text::size();
        (
            (f64::from(ui.margin + cell) + 0.5) * f64::from(tokens::metrics::cell_width(px)),
            (f64::from(line) + 0.5) * f64::from(tokens::metrics::line_height(px)),
        )
    }

    /// The page turned so `item`'s row is on screen, the cursor left where it
    /// is — as the wheel would.
    fn shown(w: &mut UpdateWindow, ui: &Ui, item: &Item) {
        let (n, _) = row_of(&w.rows(ui), item);
        w.list_top = n.saturating_sub(2);
        w.anchor = None;
        w.settle(ui);
    }

    /// The line `item`'s row is drawn on, as the window lays itself out now.
    fn line_of(w: &UpdateWindow, ui: &Ui, item: &Item) -> u16 {
        let lay = w.layout(ui);
        if let Some(n) = lay
            .titled
            .iter()
            .position(|r| r.item.as_ref() == Some(item))
        {
            return lay.title.first + u16::try_from(n).expect("small");
        }
        let (n, _) = row_of(&w.rows(ui), item);
        assert!(
            n >= w.list_top && n < w.list_top + usize::from(lay.list.lines),
            "{item:?} is not on screen"
        );
        lay.list.first + u16::try_from(n - w.list_top).expect("small")
    }

    /// The pointer moved to `at`.
    fn moved(w: &mut UpdateWindow, ui: &mut Ui, hand: &mut Hand<Grab>, at: (f64, f64)) {
        let e = event(ui, at, PointerEventKind::Motion { time: 0 });
        w.pointer_here(ui, hand, &[e]);
    }

    /// A press and a release at `at`.
    fn clicked(w: &mut UpdateWindow, ui: &mut Ui, hand: &mut Hand<Grab>, at: (f64, f64)) {
        let events: Vec<PointerEvent> = [
            PointerEventKind::Motion { time: 0 },
            PointerEventKind::Press {
                time: 0,
                button: LEFT_BUTTON,
                serial: 0,
            },
            PointerEventKind::Release {
                time: 0,
                button: LEFT_BUTTON,
                serial: 0,
            },
        ]
        .into_iter()
        .map(|kind| event(ui, at, kind))
        .collect();
        w.pointer_here(ui, hand, &events);
    }

    /// The wheel turned `notches` at `at` — down when positive.
    fn wheeled(
        w: &mut UpdateWindow,
        ui: &mut Ui,
        hand: &mut Hand<Grab>,
        at: (f64, f64),
        notches: i32,
    ) {
        let e = event(
            ui,
            at,
            PointerEventKind::Axis {
                time: 0,
                horizontal: smithay_client_toolkit::seat::pointer::AxisScroll::default(),
                vertical: smithay_client_toolkit::seat::pointer::AxisScroll {
                    value120: notches * 120,
                    ..Default::default()
                },
                source: None,
            },
        );
        w.pointer_here(ui, hand, &[e]);
    }

    /// A photograph's window over `view`: what a key or a click would have
    /// asked past the door is kept.
    fn kept_window(view: View) -> (UpdateWindow, Ui) {
        (
            UpdateWindow::of(Open::new(view, Reach::Kept(Vec::new()), None)),
            Ui::for_tests(texts()),
        )
    }

    /// FAILS IF: the foot is not the window's own, always as tall; it does
    /// not explain the thing the pointer rests on over the cursor's — with
    /// what a click there does: a box switches at once, another thing is
    /// selected first, the cursor's does what `RET` does —; a key does not
    /// give it back to the cursor until the pointer moves again; or a
    /// heading, the live output or nothing under the pointer is not what it
    /// says.
    #[test]
    fn the_pointer_outranks_the_cursor_in_the_foot_until_a_key() {
        let (mut w, mut ui) = a_window();
        let mut hand = Hand::<Grab>::default();
        let t = texts();
        assert_eq!(w.foot_lines(&ui), tokens::space::WINDOW_FOOTLINES);
        let gesture = |how, does: &str| Some(crate::door::foot::gesture(&t, how, &t.t(does)));
        // THE CURSOR, with nothing under the pointer: where it rests.
        let said = w.explained(&ui).expect("the cursor's thing");
        assert_eq!(said.name, t.t(Action::Start.id()));
        assert_eq!(
            said.gesture,
            gesture(crate::door::Gesture::Ret, Action::Start.act())
        );
        // THE POINTER ON AN OFF STEP'S NAME: the step, a click selects it.
        let off = shipped(false);
        let step = Item::Step(off.id);
        shown(&mut w, &ui, &step);
        let line = line_of(&w, &ui, &step);
        let on_the_name = point(&ui, tokens::space::UPDATE_BOXCELLS + 2, line);
        moved(&mut w, &mut ui, &mut hand, on_the_name);
        let said = w.explained(&ui).expect("the pointer's thing");
        assert_eq!(said.name, t.t(off.name_key));
        assert_eq!(
            said.gesture,
            gesture(
                crate::door::Gesture::Selects,
                module::key::SYSTEM_UPDATE_ACT_SWITCH_ON
            )
        );
        assert_eq!(hand.row_under_pointer, Some(usize::from(line)), "not lit");
        // ON ITS BOX: switched at once.
        let on_the_box = point(&ui, 0, line);
        moved(&mut w, &mut ui, &mut hand, on_the_box);
        let said = w.explained(&ui).expect("the box");
        assert_eq!(
            said.gesture,
            gesture(
                crate::door::Gesture::AtOnce,
                module::key::SYSTEM_UPDATE_ACT_SWITCH_ON
            )
        );
        // A KEY gives the foot back to the cursor, the pointer where it was.
        typed(&mut w, &mut ui, "<down>");
        let cursor = w.open.view.cursor_item().expect("a cursor");
        assert_ne!(cursor, step);
        assert_eq!(
            w.explained(&ui).expect("the cursor's").subject,
            cursor.subject()
        );
        // …until the pointer moves again — onto the step, wherever the key
        // took the page.
        shown(&mut w, &ui, &step);
        let on_the_name = point(
            &ui,
            tokens::space::UPDATE_BOXCELLS + 2,
            line_of(&w, &ui, &step),
        );
        moved(&mut w, &mut ui, &mut hand, on_the_name);
        assert_eq!(w.explained(&ui).expect("the step").subject, step.subject());
        // THE FIRST CLICK selects it: now its click is `RET`'s.
        clicked(&mut w, &mut ui, &mut hand, on_the_name);
        assert_eq!(w.open.view.focus(), Some(step.clone()));
        assert_eq!(
            w.explained(&ui).expect("the step").gesture,
            gesture(
                crate::door::Gesture::Ret,
                module::key::SYSTEM_UPDATE_ACT_SWITCH_ON
            )
        );
        // A HEADING is explained, and nothing lights or is promised.
        let heading = Item::Heading(Section::Steps);
        shown(&mut w, &ui, &heading);
        let on_the_heading = point(&ui, 0, line_of(&w, &ui, &heading));
        moved(&mut w, &mut ui, &mut hand, on_the_heading);
        let said = w.explained(&ui).expect("the heading");
        assert_eq!(said.name, t.t(Section::Steps.title()));
        assert_eq!(said.gesture, None);
        assert_eq!(hand.row_under_pointer, None, "a heading takes no click");
        // AND THE POINTER GONE: the cursor's thing again.
        let e = event(&ui, on_the_name, PointerEventKind::Leave { serial: 0 });
        w.pointer_here(&mut ui, &mut hand, &[e]);
        assert_eq!(
            w.explained(&ui).expect("the cursor's").subject,
            step.subject()
        );
    }

    /// FAILS IF: while a question holds the cursor, the foot promises a
    /// click on a row no click takes — a step a question dims — or the click
    /// there does something.
    #[test]
    fn under_a_question_the_foot_promises_no_click() {
        let (mut w, mut ui) = kept_window(a_view());
        let mut hand = Hand::<Grab>::default();
        w.open.view.ask_metered();
        let step = Item::Step(shipped(false).id);
        shown(&mut w, &ui, &step);
        let line = line_of(&w, &ui, &step);
        let on_the_name = point(&ui, tokens::space::UPDATE_BOXCELLS + 2, line);
        moved(&mut w, &mut ui, &mut hand, on_the_name);
        let said = w.explained(&ui).expect("the step");
        assert_eq!(said.subject, step.subject());
        assert_eq!(said.gesture, None, "a click promised");
        let on_the_box = point(&ui, 0, line);
        clicked(&mut w, &mut ui, &mut hand, on_the_name);
        clicked(&mut w, &mut ui, &mut hand, on_the_box);
        assert!(w.would_have().is_empty(), "{:?}", w.would_have());
        assert_ne!(w.open.view.focus(), Some(step));
    }

    /// FAILS IF: the pointer going down into the foot to read the rest makes
    /// the foot explain the last row it crossed, and not the one it rested
    /// on — or, after a key, anything but the cursor's thing.
    #[test]
    fn the_foot_keeps_explaining_what_the_pointer_rested_on() {
        let (mut w, mut ui) = a_window();
        let mut hand = Hand::<Grab>::default();
        let step = Item::Step(shipped(false).id);
        shown(&mut w, &ui, &step);
        let line = line_of(&w, &ui, &step);
        let cell = tokens::space::UPDATE_BOXCELLS + 2;
        let rest_ms = u32::from(tokens::space::WINDOW_FOOTRESTTENTHS) * 100;
        let walk =
            |w: &mut UpdateWindow, ui: &mut Ui, hand: &mut Hand<Grab>, plan: &[(u16, u32)]| {
                for (down, time) in plan {
                    let e = event(
                        ui,
                        point(ui, cell, line + down),
                        PointerEventKind::Motion { time: *time },
                    );
                    w.pointer_here(ui, hand, &[e]);
                }
            };
        // RESTED ON THE STEP, then two rows crossed on the way down.
        walk(
            &mut w,
            &mut ui,
            &mut hand,
            &[(0, 0), (1, rest_ms), (2, rest_ms + 10)],
        );
        w.pointer_went_to_the_foot(&mut ui, &mut hand, Some(rest_ms + 20));
        assert_eq!(w.explained(&ui).expect("the step").subject, step.subject());
        // A KEY, and the foot is the cursor's once the pointer goes there
        // without resting on anything.
        typed(&mut w, &mut ui, "<down>");
        let at = 10 * rest_ms;
        walk(&mut w, &mut ui, &mut hand, &[(0, at), (1, at + 10)]);
        w.pointer_went_to_the_foot(&mut ui, &mut hand, Some(at + 20));
        let cursor = w.open.view.cursor_item().expect("a cursor");
        assert_eq!(
            w.explained(&ui).expect("the cursor's").subject,
            cursor.subject()
        );
    }

    /// FAILS IF: a click on a step's box does not switch it at once, a
    /// click on its name does before it is selected, or a root step's box
    /// switches anything.
    #[test]
    fn a_steps_box_switches_at_once_and_its_name_selects_first() {
        let (mut w, mut ui) = kept_window(a_view());
        let mut hand = Hand::<Grab>::default();
        let off = shipped(false);
        shown(&mut w, &ui, &Item::Step(off.id));
        let line = line_of(&w, &ui, &Item::Step(off.id));
        let on_the_box = point(&ui, 0, line);
        clicked(&mut w, &mut ui, &mut hand, on_the_box);
        assert_eq!(
            w.would_have(),
            vec![format!("would have switched {} on", off.id)]
        );
        let (mut w, mut ui) = kept_window(a_view());
        shown(&mut w, &ui, &Item::Step(off.id));
        let name = point(&ui, tokens::space::UPDATE_BOXCELLS + 2, line);
        clicked(&mut w, &mut ui, &mut hand, name);
        assert!(w.would_have().is_empty(), "the first click switched it");
        assert_eq!(w.open.view.focus(), Some(Item::Step(off.id)));
        clicked(&mut w, &mut ui, &mut hand, name);
        assert_eq!(
            w.would_have(),
            vec![format!("would have switched {} on", off.id)]
        );
        // A ROOT STEP'S BOX is no switch of the person's.
        let (mut w, mut ui) = kept_window(a_view());
        let root = roots();
        shown(&mut w, &ui, &Item::Step(root.id));
        let on_the_box = point(&ui, 0, line_of(&w, &ui, &Item::Step(root.id)));
        clicked(&mut w, &mut ui, &mut hand, on_the_box);
        assert!(w.would_have().is_empty(), "{:?}", w.would_have());
    }

    /// FAILS IF: a click on a row the page shows moves the page — the first
    /// row of a section pulling its heading, just above the page, down with
    /// it — so the second click of a double-click lands on another row.
    #[test]
    fn a_click_takes_a_row_where_it_is_drawn_and_the_page_stays() {
        let (mut w, mut ui) = kept_window(a_view());
        let mut hand = Hand::<Grab>::default();
        let rows = w.rows(&ui);
        let first = rows
            .iter()
            .find_map(|r| match r.item {
                Some(Item::Step(id)) => Some(id),
                _ => None,
            })
            .expect("a step");
        let (n, _) = row_of(&rows, &Item::Step(first));
        assert!(matches!(rows[n - 1].item, Some(Item::Heading(_))));
        w.list_top = n;
        w.anchor = None;
        w.settle(&ui);
        assert_eq!(w.list_top, n, "the list is too short for the test");
        let on_the_name = point(
            &ui,
            tokens::space::UPDATE_BOXCELLS + 2,
            line_of(&w, &ui, &Item::Step(first)),
        );
        clicked(&mut w, &mut ui, &mut hand, on_the_name);
        assert_eq!(w.open.view.focus(), Some(Item::Step(first)));
        assert_eq!(w.list_top, n, "the click moved the page");
        clicked(&mut w, &mut ui, &mut hand, on_the_name);
        let asked = w.would_have();
        assert_eq!(asked.len(), 1, "{asked:?}");
        assert!(
            asked[0].starts_with(&format!("would have switched {first} ")),
            "{asked:?}"
        );
    }

    /// FAILS IF: the list's bar is not where its press takes it — its track
    /// grabbed, dragged and let go — or the wheel moves the cursor instead
    /// of the page, or a key does not bring the cursor's row back on screen.
    #[test]
    fn the_lists_bar_is_grabbed_where_it_is_drawn_and_the_wheel_moves_only_the_page() {
        let (mut w, mut ui) = a_window();
        let mut hand = Hand::<Grab>::default();
        let lay = w.layout(&ui);
        let bar = w
            .bars(&ui, &lay)
            .list
            .expect("a list longer than the window");
        let (x, y, width, height) = bar.track;
        assert!(
            (f64::from(y)
                - f64::from(tokens::metrics::line_height(tokens::text::size()))
                    * f64::from(lay.list.first))
            .abs()
                < 0.5,
            "the bar is not the list's"
        );
        let at = |share: f32| (f64::from(x + width / 2.0), f64::from(y + height * share));
        let press = event(
            &ui,
            at(0.95),
            PointerEventKind::Press {
                time: 0,
                button: LEFT_BUTTON,
                serial: 0,
            },
        );
        w.pointer_here(&mut ui, &mut hand, &[press]);
        assert_eq!(hand.extra, Grab::List);
        let bottom = w.list_top;
        assert!(
            bottom > 0,
            "a press low on the track left the page at its top"
        );
        moved(&mut w, &mut ui, &mut hand, at(0.0));
        assert_eq!(w.list_top, 0, "the drag did not follow the pointer");
        let release = event(
            &ui,
            at(0.0),
            PointerEventKind::Release {
                time: 0,
                button: LEFT_BUTTON,
                serial: 0,
            },
        );
        w.pointer_here(&mut ui, &mut hand, &[release]);
        assert_eq!(hand.extra, Grab::Nothing);
        // THE WHEEL over the list moves the page, never the cursor.
        let cursor = w.open.view.focus();
        let over = point(&ui, 4, lay.list.first + 2);
        wheeled(&mut w, &mut ui, &mut hand, over, 2);
        assert!(w.list_top > 0, "the wheel did not move the page");
        assert_eq!(w.open.view.focus(), cursor, "the wheel moved the cursor");
        // A KEY brings the cursor's row on screen.
        typed(&mut w, &mut ui, "<end>");
        let rows = w.rows(&ui);
        let at = w.open.view.cursor_row(&rows).expect("a cursor");
        let room = usize::from(w.layout(&ui).list.lines);
        assert!(
            at >= w.list_top && at < w.list_top + room,
            "{at} {}",
            w.list_top
        );
    }

    /// FAILS IF: what is on screen moves when the machine's records bring a
    /// notice above it — data never moves the page, only a key does —, or
    /// one that comes while the page is at its top is put out of sight.
    #[test]
    fn a_notice_that_comes_never_moves_the_rows_on_screen() {
        let (mut w, ui) = a_window();
        w.scroll_by(&ui, 6);
        let on_screen = |w: &UpdateWindow| {
            let room = usize::from(w.layout(&ui).list.lines);
            said_all(&w.rows(&ui)[w.list_top..w.list_top + room])
        };
        let before = on_screen(&w);
        let records = module::Records {
            conflicts: 2,
            ..module::Records::default()
        };
        w.open.view = w.open.view.clone().with_records(records, String::new());
        w.settle(&ui);
        assert!(
            said_all(&w.rows(&ui)).contains(&texts().t(Section::Notices.title())),
            "no notice came"
        );
        assert_eq!(on_screen(&w), before, "the notice moved the page");
        // AT THE TOP, the page stays at the top, and the notice is seen.
        let (mut w, ui) = a_window();
        let records = module::Records {
            conflicts: 2,
            ..module::Records::default()
        };
        w.open.view = w.open.view.clone().with_records(records, String::new());
        w.settle(&ui);
        assert_eq!(w.list_top, 0, "the notice came above the page");
    }

    /// FAILS IF: the live output shows while nothing runs, or not while a
    /// run goes (under the list, on a page of its own lines); it does not
    /// follow its end; a turn of the wheel over it moves the list instead,
    /// or a line that comes moves what the person scrolled back to; `G`
    /// does not follow the end again; or a page leaves it there.
    #[test]
    fn the_live_output_shows_only_while_a_run_goes_and_follows_its_end() {
        let (w, ui) = a_window();
        assert_eq!(w.layout(&ui).output, None, "an output while nothing runs");
        let running = a_view().with_brain(Brain {
            running: true,
            ..Brain::default()
        });
        let mut w = UpdateWindow::of(Open::new(running, Reach::Machine, None));
        let mut ui = ui;
        let mut hand = Hand::<Grab>::default();
        let lay = w.layout(&ui);
        let (heading, card) = lay.output.expect("the live output");
        assert_eq!(heading, lay.list.end(), "not under the list");
        assert_eq!(card.end(), ui.lines, "not down to the foot");
        let room = lay.list.lines + card.lines + 1;
        assert!(lay.list.lines >= tokens::space::UPDATE_LISTMINLINES.min(room / 2));
        assert!(card.lines + 1 >= tokens::space::UPDATE_OUTPUTMINLINES);
        for n in 0..60 {
            w.open
                .view
                .heard(OutputLine::written(n, false, "run", &format!("line {n}")));
        }
        let tail = 60 - usize::from(card.lines);
        assert_eq!(w.output_scroll(card.lines).offset, tail, "not at its end");
        assert!(w.bars(&ui, &lay).output.is_some(), "no bar of its own");
        // THE WHEEL OVER IT moves it, and the list stays.
        let over = point(&ui, 4, card.first + 1);
        wheeled(&mut w, &mut ui, &mut hand, over, -1);
        assert_eq!(w.list_top, 0, "the wheel over the output moved the list");
        let back = w.output_scroll(card.lines).offset;
        assert!(back < tail, "the wheel did not move the output");
        w.open
            .view
            .heard(OutputLine::written(60, false, "run", "line 60"));
        assert_eq!(
            w.output_scroll(card.lines).offset,
            back,
            "a line moved the output"
        );
        // `G` follows the end again.
        typed(&mut w, &mut ui, "G");
        assert_eq!(w.output_top, None);
        assert_eq!(
            w.output_scroll(card.lines).offset,
            61 - usize::from(card.lines)
        );
        // A PAGE is the whole of the room.
        typed(&mut w, &mut ui, "c");
        assert!(w.open.view.page_open());
        assert_eq!(w.layout(&ui).output, None);
    }

    /// FAILS IF: a scene's steps, journal lines, calendar or brainless
    /// machine do not reach the window it describes, or a step that is not
    /// declared is drawn rather than refused.
    #[test]
    fn a_scene_switches_steps_says_its_journal_and_its_calendar() {
        let off = shipped(false);
        let json = format!(
            r#"{{"steps": {{"{id}": {{"on": true, "here": false}}}},
                "journal": [{{"message": "\u001b[32mok\u001b[0m", "priority": 3}}],
                "brain": {{"enabled": false, "every": 1, "part": "night", "next": 1762600000}},
                "no-brain": true, "system-hidden": true}}"#,
            id = off.id
        );
        let file = std::env::temp_dir().join(format!(
            "ronoctua-update-scene-steps-{}.json",
            std::process::id()
        ));
        std::fs::write(&file, &json).expect("a scene");
        let asked = camera::parse(&[
            "--scene".to_owned(),
            file.display().to_string(),
            "--header".to_owned(),
            "shown".to_owned(),
        ])
        .expect("asked");
        let w = camera::window_for_tests(&asked).expect("a window");
        let st = w
            .open
            .view
            .steps()
            .iter()
            .find(|s| s.id == off.id)
            .expect("the step");
        assert!(st.on && !st.here(), "{st:?}");
        assert_eq!(st.why(), ronoctua_brain::StepWhy::OnByYou);
        let line = w
            .open
            .view
            .scrollback()
            .line(0)
            .expect("the journal's line");
        assert_eq!(line.base, tokens::color::STATE_ERROR);
        assert_eq!(line.text(), "ok");
        let schedule = w.open.view.schedule.as_ref().expect("a calendar");
        assert!(!schedule.enabled);
        assert_eq!(schedule.interval_days, 1);
        assert_eq!(schedule.part, ronoctua_brain::PartOfDay::Night);
        assert!(w.open.no_brain);
        assert!(!w.open.view.system_readable);
        std::fs::write(
            &file,
            r#"{"steps": {"no-such-step": {"on": true, "here": true}}}"#,
        )
        .expect("a scene");
        assert!(
            camera::window_for_tests(&asked).is_err(),
            "a step nobody declared"
        );
        let _ = std::fs::remove_file(&file);
    }
}
