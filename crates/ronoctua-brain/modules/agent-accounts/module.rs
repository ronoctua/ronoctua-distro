//! The person's LLM accounts on the modeline — one mark each, coloured by how
//! much of the account's limits is used — and the usage unit, started after
//! an account was used.
//!
//! Why each piece is so is in `why.md`, beside this file. What is here is the
//! shape of it.
//!
//! # What it reads, and from whom
//!
//! Everything about an account comes from `ronoctua_llm`, the one crate that
//! reads the agents' state: the list (`Accounts`), what to watch for each
//! (`watched`), the limits Claude Code's status line kept or Codex wrote in
//! its logs (`statusline::limits`) and the usage endpoint's last answer
//! (`usage::kept`). The module names no file and no folder of the agents'.
//!
//! # Two halves
//!
//! What the accounts' view reads too — the person's lines, the colour of a
//! window, the marks the accounts wear and how a photograph reads them — is
//! `shared.rs`, compiled into the library the footer links. This file is the
//! daemon's: what it watches, when its run is wanted, and its page.
//!
//! # The marks
//!
//! One per account, AT REST TOO — the owner's exception
//! (`assets/brain-resting-marks.nix`): Claude Code's robot or Codex's, with
//! the account's place in the person's list as its digit, and the colour of
//! the most used of its windows that has not reset — quiet, a warning from
//! [`Lines::warn_percent`], danger from [`Lines::danger_percent`]. A click
//! opens the accounts' view on that account (`agents-open <n>`). Nine at most:
//! the digit is one.
//!
//! # Three watches of its own, and why
//!
//! The core reads a watcher's sources ONCE, when its thread starts, and the
//! accounts are the person's to add at any moment. So the core watches the
//! two files whose paths never change — the list and the lines, their
//! folders made first, since a file is watched from its folder — and the
//! module keeps three `Notice`s of its own over everything per account,
//! raised again whenever what they should cover changes: one over the
//! folders the numbers are kept in (a change there is re-read), one over the
//! agents' log trees (a change there is an account being USED), and one over
//! the accounts' own folders (where a log tree is born — the one change there
//! that is use). Use is never heard where the distro writes: the usage unit
//! writes the numbers, and the view writes an account's `settings.json` —
//! counted as use, either would start the unit with nobody working.
//!
//! # The run, and no clock at rest
//!
//! The usage unit counts the transcripts and asks the endpoint
//! (`ronoctua-llm refresh`). It is wanted only after an account was used
//! since the last start, and no sooner than [`AGAIN_AFTER_S`] after it; and
//! the one clock the module asks for is the nearest reset of a window it
//! shows, so a mark that is red because the five hours are used turns quiet
//! when they start over — and no clock at all when no window is known.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use ronoctua_files::notice::{Notice, Pace};
use ronoctua_llm::{Account, Kind as Agent, Watched, Window};

// THE SHARED HALF (`shared.rs`, compiled into the library beside the
// manifest): what the accounts' view reads of the same machine.
use super::{
    Lines, Seen, Which, accounts_now, lines_store, marked_most, marks_for, most_used, read_lines,
    tone_of, windows_now, write_line,
};
use crate::module::{
    Act, Acted, Birth, Cell, Ctx, Face, Heard, Look, Manifest, Mark, Module, Row, Run, Scheduled,
    Screen, Section, Source, Tone, Watcher,
};
use crate::moment::Moment;
use crate::prefs::TaskPrefs;
use crate::systemd;
use crate::task::{Due, Outcome};

/// How long after a start the run is wanted again while the accounts are
/// used: their numbers move by the minute, and a count every few minutes of
/// work is what the view needs — never one per answer.
pub const AGAIN_AFTER_S: i64 = 10 * 60;

/// The entry: the machine's accounts, the lines in the module's folder.
#[must_use]
pub fn new(birth: &Birth) -> impl Module + use<> {
    AgentAccounts::of(Box::new(Machine), birth.config_dir(super::ID))
}

// ---------------------------------------------------------------------------
// The machine, behind a trait a test hands its own of.
// ---------------------------------------------------------------------------

/// What the module asks of the machine.
trait Probe: Send + Sync {
    /// Where the list is.
    fn accounts_file(&self) -> Option<PathBuf>;
    /// The person's accounts; `None` when the list is there and does not
    /// read.
    fn accounts(&self) -> Option<Vec<Account>>;
    /// Every window an account's agent, or the endpoint, last said — reset
    /// or not.
    fn windows(&self, account: &Account) -> Vec<Window>;
    /// What to watch for it.
    fn watched(&self, account: &Account) -> Watched;
}

/// The real machine: `ronoctua_llm`, under the environment's roots.
struct Machine;

impl Probe for Machine {
    fn accounts_file(&self) -> Option<PathBuf> {
        ronoctua_llm::Accounts::store().path()
    }

    fn accounts(&self) -> Option<Vec<Account>> {
        accounts_now()
    }

    fn windows(&self, account: &Account) -> Vec<Window> {
        // THE STATUS LINE'S (Claude Code) OR THE LOGS' (Codex), AND THE
        // ENDPOINT'S LAST ANSWER — put together by the one function the view
        // asks too, the same a photograph reads.
        windows_now(account)
    }

    fn watched(&self, account: &Account) -> Watched {
        ronoctua_llm::watched(account)
    }
}

// ---------------------------------------------------------------------------
// What it keeps.
// ---------------------------------------------------------------------------

/// What the last reading found.
#[derive(Debug, Clone, PartialEq, Default)]
struct Kept {
    /// The accounts, in the person's order.
    accounts: Vec<Seen>,
    /// The list is there and does not read: the marks it had stay off, and
    /// the page says so.
    unreadable: bool,
    /// Why the list cannot be watched, in the machine's words.
    lost: Option<String>,
    /// Why the lines cannot be watched: a line changed in another door is
    /// not heard, and the page says so.
    lines_lost: Option<String>,
    /// The lines.
    lines: Lines,
}

/// What the module's own watches cover.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Targets {
    /// The folders an account's numbers are kept in — re-read on a change.
    numbers: Vec<PathBuf>,
    /// The agents' log trees, whole — a change is use.
    trees: Vec<PathBuf>,
    /// The accounts' own folders, alone — where a log tree is born, the one
    /// change there that is use; anything else there (the `settings.json`
    /// the view writes) is not.
    folders: Vec<PathBuf>,
}

/// The watches, and what they were raised for.
#[derive(Default)]
struct Watches {
    aimed: Option<Targets>,
    numbers: Option<Notice>,
    trees: Option<Notice>,
    folders: Option<Notice>,
}

/// Which of the core's two sources a heard path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ear {
    /// The list of accounts.
    Accounts,
    /// The person's lines.
    Lines,
}

/// The module.
struct AgentAccounts {
    inner: Arc<Inner>,
    /// The core's sources, in the order [`Watcher::sources`] gives them.
    ears: Vec<(Ear, PathBuf)>,
}

/// Its state, shared with its own watches' threads.
struct Inner {
    probe: Box<dyn Probe>,
    lines: ronoctua_store::Store<Lines>,
    kept: Mutex<Kept>,
    /// Held across a reading AND its keeping: the core's thread and the
    /// module's watches read on three threads, and a slow reading kept after
    /// a newer one would put older numbers back.
    reading: Mutex<()>,
    /// An account was used since the run last started.
    used: AtomicBool,
    /// The page's generation.
    generation: AtomicU64,
    watches: Mutex<Watches>,
}

impl AgentAccounts {
    fn of(probe: Box<dyn Probe>, config: Option<PathBuf>) -> Self {
        // THE FOLDERS FIRST: the core watches a file from its folder, and a
        // path whose folder is not there is down for good — on a fresh
        // machine the list's and the lines' would never be heard. Made here,
        // before the core asks for the sources.
        let folders = [
            config.clone(),
            probe
                .accounts_file()
                .and_then(|f| f.parent().map(Path::to_path_buf)),
        ];
        for folder in folders.into_iter().flatten() {
            if let Err(e) = std::fs::create_dir_all(&folder) {
                tracing::warn!(error = %e, folder = %folder.display(), "the LLM accounts' folder could not be made");
            }
        }
        let lines = lines_store(config);
        let mut ears = Vec::new();
        if let Some(path) = probe.accounts_file() {
            ears.push((Ear::Accounts, path));
        }
        if let Some(path) = lines.path() {
            ears.push((Ear::Lines, path));
        }
        Self {
            inner: Arc::new(Inner {
                probe,
                lines,
                kept: Mutex::new(Kept::default()),
                reading: Mutex::new(()),
                used: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                watches: Mutex::new(Watches::default()),
            }),
            ears,
        }
    }

    fn kept(&self) -> Kept {
        self.inner.kept()
    }
}

impl Inner {
    fn kept(&self) -> Kept {
        self.kept.lock().map(|k| k.clone()).unwrap_or_default()
    }

    /// Changes what is kept; `true` when it changed — and then the page's
    /// generation has moved.
    fn keep(&self, change: impl FnOnce(&mut Kept)) -> bool {
        let Ok(mut kept) = self.kept.lock() else {
            return false;
        };
        let before = kept.clone();
        change(&mut kept);
        let changed = *kept != before;
        if changed {
            self.generation.fetch_add(1, Ordering::Relaxed);
        }
        changed
    }

    /// The accounts and their windows, read again; `true` when anything moved.
    /// ONE READING AT A TIME, kept before the next starts.
    fn reread(&self) -> bool {
        let _one = self.reading.lock().unwrap_or_else(PoisonError::into_inner);
        let read = self.probe.accounts().map(|accounts| {
            accounts
                .iter()
                .map(|a| Seen::of(a, self.probe.windows(a)))
                .collect::<Vec<_>>()
        });
        self.keep(|k| match read {
            Some(accounts) => {
                k.accounts = accounts;
                k.unreadable = false;
            }
            // A LIST THAT DOES NOT READ keeps no marks it cannot vouch for.
            None => {
                k.accounts.clear();
                k.unreadable = true;
            }
        })
    }

    /// The lines, read again — in turn with the readings, for the same
    /// reason.
    fn reread_lines(&self) -> bool {
        let _one = self.reading.lock().unwrap_or_else(PoisonError::into_inner);
        let lines = read_lines(&self.lines);
        self.keep(|k| k.lines = lines)
    }

    /// What the watches should cover now, the folders the numbers are kept
    /// in made first: a folder that does not exist yet is never heard when
    /// the first number lands in it.
    fn targets(&self) -> Targets {
        let accounts = self.probe.accounts().unwrap_or_default();
        let mut t = Targets::default();
        for a in &accounts {
            let watched = self.probe.watched(a);
            for file in &watched.files {
                if let Some(folder) = file.parent() {
                    if let Err(e) = std::fs::create_dir_all(folder) {
                        tracing::warn!(error = %e, folder = %folder.display(), "an account's numbers cannot be watched");
                        continue;
                    }
                    t.numbers.push(folder.to_path_buf());
                }
            }
            // THE AGENT'S OWN FOLDERS ARE ITS OWN: watched where they are,
            // never made. A tree not there yet is heard being born from the
            // account's folder.
            t.trees
                .extend(watched.trees.iter().filter(|p| p.is_dir()).cloned());
            if a.dir.is_dir() {
                t.folders.push(a.dir.clone());
            }
        }
        for list in [&mut t.numbers, &mut t.trees, &mut t.folders] {
            list.sort();
            list.dedup();
        }
        t
    }
}

/// Raises the module's watches again when what they should cover moved, and
/// answers whether a log tree was BORN in a folder already watched — an
/// agent writing its first log there.
///
/// From the core's thread (the list changed) and from the folders' watch's
/// own thread (something changed in an account's folder): dropping a watch
/// from its own thread is safe, since it only closes the descriptors its
/// thread waits on.
fn rewatch(inner: &Arc<Inner>, ctx: &Ctx) -> bool {
    let targets = inner.targets();
    let Ok(mut watches) = inner.watches.lock() else {
        return false;
    };
    if watches.aimed.as_ref() == Some(&targets) {
        return false;
    }
    // BORN, not newly listed: a tree that was not watched before, inside a
    // folder that was — an account added with its logs already there was
    // used before it was added, not now.
    let born = watches.aimed.as_ref().is_some_and(|before| {
        targets
            .trees
            .iter()
            .any(|t| !before.trees.contains(t) && before.folders.iter().any(|f| t.starts_with(f)))
    });
    let raised = |dirs: &[PathBuf], trees: &[PathBuf], heard: fn(&Weak<Inner>, &Ctx) -> bool| {
        let (me, ctx) = (Arc::downgrade(inner), ctx.detached());
        Notice::on_dirs(dirs, trees, Pace::Coalesced, move || heard(&me, &ctx))
    };
    let said = |what: &str, e: &anyhow::Error| {
        tracing::warn!(error = %e, what, "the LLM accounts cannot be watched");
    };
    watches.numbers = raised(&targets.numbers, &[], numbers_moved)
        .map_err(|e| said("numbers", &e))
        .ok();
    watches.trees = raised(&[], &targets.trees, account_used)
        .map_err(|e| said("use", &e))
        .ok();
    watches.folders = raised(&targets.folders, &[], folder_stirred)
        .map_err(|e| said("folders", &e))
        .ok();
    watches.aimed = Some(targets);
    born
}

/// A folder an account's numbers are kept in changed: read them again.
/// `false` ends the watch — the module is gone.
fn numbers_moved(me: &Weak<Inner>, ctx: &Ctx) -> bool {
    let Some(inner) = me.upgrade() else {
        return false;
    };
    if inner.reread() {
        ctx.changed();
    }
    true
}

/// An agent wrote in its logs: the account is being used. Codex's limits
/// are in those logs, so they are read again too.
fn account_used(me: &Weak<Inner>, ctx: &Ctx) -> bool {
    let Some(inner) = me.upgrade() else {
        return false;
    };
    used_now(&inner, ctx);
    true
}

/// Something changed in an account's own folder. Only a log tree born there
/// is use — and from then on it is watched whole; anything else (the
/// `settings.json` the view writes, the agent's own odds and ends) is not.
fn folder_stirred(me: &Weak<Inner>, ctx: &Ctx) -> bool {
    let Some(inner) = me.upgrade() else {
        return false;
    };
    if rewatch(&inner, ctx) {
        used_now(&inner, ctx);
    }
    true
}

/// An account is being used: the run is wanted, and the numbers are read
/// again.
fn used_now(inner: &Arc<Inner>, ctx: &Ctx) {
    inner.used.store(true, Ordering::Release);
    inner.reread();
    // TOLD EVERY TIME: the runner asks `wanted_now` when the face moves.
    ctx.changed();
}

/// Whether the run is wanted: an account was used since the last start, and
/// that start was a while ago. Pure.
fn wanted(used: bool, now: Moment, last: Option<Moment>) -> bool {
    used && last.is_none_or(|l| now.saturating_sub(l) >= AGAIN_AFTER_S)
}

/// The nearest reset after `now` of any window shown: the one clock the
/// module asks for, and none when no window is known.
fn nearest_reset(kept: &Kept, now: Moment) -> Option<Moment> {
    kept.accounts
        .iter()
        .take(marked_most())
        .flat_map(|s| s.windows.iter())
        .map(|w| w.resets_at_s)
        .filter(|at| *at > now)
        .min()
}

/// The marks: one per account, by its place in the list — the shared half's
/// own (`marks_for`), so a photograph's marks are these.
fn marks_of(kept: &Kept, now: Moment) -> Vec<Mark> {
    marks_for(&kept.accounts, kept.lines, now)
}

/// A text key as a cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

/// The page's rows: each account with the most used of its windows, or why
/// there is none; and the two lines.
fn sections_of(kept: &Kept, now: Moment) -> Vec<Section> {
    let mut accounts = Vec::new();
    if let Some(why) = &kept.lost {
        accounts.push(Row {
            id: "lost".to_owned(),
            label: key(super::key::AGENT_ACCOUNTS_ROW_UNWATCHED),
            value: Cell::Data(why.clone()),
            tone: Tone::Warning,
            ..Row::default()
        });
    }
    if kept.unreadable {
        accounts.push(Row {
            id: "unreadable".to_owned(),
            label: key(super::key::AGENT_ACCOUNTS_ROW_UNREADABLE),
            tone: Tone::Warning,
            ..Row::default()
        });
    } else if kept.accounts.is_empty() {
        accounts.push(Row {
            id: "none".to_owned(),
            label: key(super::key::AGENT_ACCOUNTS_ROW_NONE),
            ..Row::default()
        });
    }
    for (n, seen) in kept.accounts.iter().enumerate() {
        let used = most_used(&seen.windows, now);
        let note = if n >= marked_most() {
            super::key::AGENT_ACCOUNTS_NOTE_NO_MARK
        } else {
            match seen.agent {
                Agent::Claude => super::key::AGENT_ACCOUNTS_NOTE_CLAUDE,
                Agent::Codex => super::key::AGENT_ACCOUNTS_NOTE_CODEX,
            }
        };
        accounts.push(Row {
            id: format!("account-{}", seen.id),
            label: Cell::Data(seen.name.clone()),
            value: used.map_or_else(
                || key(super::key::AGENT_ACCOUNTS_UNKNOWN),
                // A WHOLE PER CENT: the window's own number has a decimal,
                // and the page says how full, not to a tenth.
                |p| Cell::Count(rounded(p)),
            ),
            note: key(note),
            tone: tone_of(used, kept.lines),
            ..Row::default()
        });
    }
    let mut lines = Vec::new();
    // A LINE CHANGED IN ANOTHER DOOR would not reach the marks: said.
    if let Some(why) = &kept.lines_lost {
        lines.push(Row {
            id: "lines-lost".to_owned(),
            label: key(super::key::AGENT_ACCOUNTS_ROW_LINES_UNWATCHED),
            value: Cell::Data(why.clone()),
            tone: Tone::Warning,
            ..Row::default()
        });
    }
    lines.extend([
        Row {
            id: "warn".to_owned(),
            label: key(super::key::AGENT_ACCOUNTS_ROW_WARN),
            value: Cell::Count(u64::from(kept.lines.warn_percent)),
            tone: Tone::Warning,
            act: Some(super::action::WARN_AT.id.into()),
            ..Row::default()
        },
        Row {
            id: "danger".to_owned(),
            label: key(super::key::AGENT_ACCOUNTS_ROW_DANGER),
            value: Cell::Count(u64::from(kept.lines.danger_percent)),
            tone: Tone::Danger,
            act: Some(super::action::DANGER_AT.id.into()),
            ..Row::default()
        },
    ]);
    vec![
        Section {
            heading: super::key::AGENT_ACCOUNTS_HEADING_ACCOUNTS.into(),
            rows: accounts,
        },
        Section {
            heading: super::key::AGENT_ACCOUNTS_HEADING_LINES.into(),
            rows: lines,
        },
    ]
}

/// A per cent as a whole number, never below zero.
fn rounded(p: f32) -> u64 {
    // A PER CENT, bounded to a window's: the cast cannot truncate past it.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let whole = p.clamp(0.0, 1000.0).round() as u64;
    whole
}

/// A whole per cent typed on the line, from 1 to 100.
fn percent_typed(typed: Option<&str>) -> Option<u8> {
    typed?
        .trim()
        .trim_end_matches('%')
        .trim()
        .parse::<u8>()
        .ok()
        .filter(|p| (1..=100).contains(p))
}

impl Module for AgentAccounts {
    fn manifest(&self) -> &'static Manifest {
        &super::MANIFEST
    }

    fn scheduled(&self) -> Option<&dyn Scheduled> {
        Some(self)
    }

    fn watcher(&self) -> Option<&dyn Watcher> {
        Some(self)
    }

    fn face(&self, look: &Look<'_>) -> Face {
        let kept = self.kept();
        Face {
            marks: marks_of(&kept, look.now),
            screen: self.inner.generation.load(Ordering::Relaxed),
            ..Face::default()
        }
    }

    fn screen(&self, look: &Look<'_>) -> Screen {
        Screen {
            sections: sections_of(&self.kept(), look.now),
        }
    }

    fn act(&self, act: &Act, ctx: &Ctx) -> Acted {
        let a = super::action::WARN_AT.id;
        let b = super::action::DANGER_AT.id;
        if act.action != a && act.action != b {
            return Acted::Refused("unknown");
        }
        let Some(p) = percent_typed(act.answer.as_deref()) else {
            return Acted::Refused(super::why::NOT_A_PERCENT);
        };
        let which = if act.action == a {
            Which::Warn
        } else {
            Which::Danger
        };
        // OVER THE FILE AS IT IS NOW, not over what this page last heard.
        match write_line(&self.inner.lines, which, p) {
            Ok(_) => {
                if self.inner.reread_lines() {
                    ctx.changed();
                }
                Acted::Done
            }
            Err(unkept) => Acted::Refused(unkept.why()),
        }
    }
}

impl Scheduled for AgentAccounts {
    fn due(&self, _now: Moment, _mine: &TaskPrefs, _last: Option<Moment>) -> Due {
        if !systemd::reachable() {
            return Due::Not {
                why: "no-systemctl",
            };
        }
        // NEVER BY THE CALENDAR: the row says "after an account was used".
        Due::Not {
            why: super::why::WHEN_USED,
        }
    }

    fn wanted_now(
        &self,
        now: Moment,
        _pulse: &crate::pulse::Pulse,
        _mine: &TaskPrefs,
        last: Option<Moment>,
    ) -> bool {
        systemd::reachable() && wanted(self.inner.used.load(Ordering::Acquire), now, last)
    }

    fn run(&self, run: &Run) -> Outcome {
        // A BRAIN THAT ONLY REHEARSES asks nobody anything, and says so.
        if run.is_rehearsal() {
            return Outcome::Skipped {
                why: "rehearsing".to_owned(),
            };
        }
        // WHAT WAS USED UNTIL NOW is what this run counts: use heard from
        // here on wants the next one.
        self.inner.used.store(false, Ordering::Release);
        match systemd::start(super::unit::RUN) {
            Ok(()) => Outcome::Done(Vec::new()),
            Err(e) => Outcome::Failed {
                at: String::new(),
                why: e.to_string(),
            },
        }
    }
}

impl Watcher for AgentAccounts {
    /// The two files whose paths never change: the list, and the lines.
    fn sources(&self) -> Vec<Source> {
        self.ears
            .iter()
            .map(|(_, path)| Source::Path(path.clone()))
            .collect()
    }

    /// THE FIRST READING, after the core's watches are raised — and the
    /// module's own raised over what it found.
    fn started(&self, _now: Moment, ctx: &Ctx) {
        self.inner.reread_lines();
        self.inner.reread();
        rewatch(&self.inner, ctx);
        ctx.changed();
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        let ear = |source: usize| self.ears.get(source).map(|(e, _)| *e);
        match heard {
            Heard::Path { source, .. } => match ear(source) {
                Some(Ear::Accounts) => {
                    self.inner.reread();
                    rewatch(&self.inner, ctx);
                    ctx.changed();
                }
                Some(Ear::Lines) if self.inner.reread_lines() => ctx.changed(),
                Some(Ear::Lines) | None => {}
            },
            Heard::Lost { source, why } => {
                let why = Some(why.to_owned());
                let moved = match ear(source) {
                    Some(Ear::Accounts) => self.inner.keep(|k| k.lost = why),
                    Some(Ear::Lines) => self.inner.keep(|k| k.lines_lost = why),
                    None => false,
                };
                if moved {
                    ctx.changed();
                }
            }
            Heard::Back { source } => match ear(source) {
                Some(Ear::Accounts) => {
                    self.inner.keep(|k| k.lost = None);
                    self.inner.reread();
                    rewatch(&self.inner, ctx);
                    ctx.changed();
                }
                // WHAT WAS MISSED while it was down is read now.
                Some(Ear::Lines) => {
                    self.inner.keep(|k| k.lines_lost = None);
                    self.inner.reread_lines();
                    ctx.changed();
                }
                None => {}
            },
            // A WINDOW RESET: the face drops it by the moment it is asked at,
            // and the page — which shows how full it was — is another page
            // now: its generation moves, or the host never asks for it again.
            Heard::Wake => {
                self.inner.generation.fetch_add(1, Ordering::Relaxed);
                ctx.changed();
            }
            Heard::Journal { .. } | Heard::Line { .. } => {}
        }
    }

    /// The nearest reset of a window shown, and nothing when none is known.
    fn next_wake(&self, now: Moment) -> Option<Moment> {
        nearest_reset(&self.kept(), now)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::super::{LINES_FILE, OPENS};
    use super::*;
    use crate::pulse::Pulse;

    /// Noon on 2026-10-05, UTC.
    const NOON: Moment = 1_791_201_600;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-brain-agent-accounts-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    fn account(id: &str, agent: Agent, name: &str, dir: &Path) -> Account {
        Account {
            id: id.to_owned(),
            kind: agent,
            name: name.to_owned(),
            dir: dir.to_path_buf(),
            default: false,
        }
    }

    fn window(used_percent: f32, resets_at_s: i64) -> Window {
        Window {
            used_percent,
            resets_at_s,
        }
    }

    /// A machine the test sets.
    #[derive(Default)]
    struct Set {
        list: Mutex<Option<Vec<Account>>>,
        windows: Mutex<Vec<(String, Vec<Window>)>>,
        watched: Mutex<Vec<(String, Watched)>>,
        file: Option<PathBuf>,
    }

    impl Probe for Arc<Set> {
        fn accounts_file(&self) -> Option<PathBuf> {
            self.file.clone()
        }
        fn accounts(&self) -> Option<Vec<Account>> {
            self.list.lock().unwrap().clone()
        }
        fn windows(&self, a: &Account) -> Vec<Window> {
            let all = self.windows.lock().unwrap();
            all.iter()
                .find(|(id, _)| *id == a.id)
                .map(|(_, w)| w.clone())
                .unwrap_or_default()
        }
        fn watched(&self, a: &Account) -> Watched {
            let all = self.watched.lock().unwrap();
            all.iter()
                .find(|(id, _)| *id == a.id)
                .map(|(_, w)| w.clone())
                .unwrap_or_default()
        }
    }

    fn module(set: &Arc<Set>, config: Option<PathBuf>) -> AgentAccounts {
        AgentAccounts::of(Box::new(Arc::clone(set)), config)
    }

    fn face_at(m: &AgentAccounts, now: Moment) -> Face {
        let (pulse, mine) = (Pulse::default(), TaskPrefs::default());
        m.face(&Look {
            now,
            pulse: &pulse,
            mine: &mine,
            last: None,
        })
    }

    /// FAILS IF: an account has no mark, or one numbered by anything but its
    /// place in the person's whole list, named by anything but the person's
    /// word, opening anything but the accounts' view; a mark is coloured by
    /// a window that already reset or past the person's lines; a tenth
    /// account draws a digit the footer cannot; or the core would refuse one
    /// of them (`Manifest::refusal_of`).
    #[test]
    fn one_mark_per_account_by_its_place_coloured_by_its_most_used_window() {
        let set = Arc::new(Set::default());
        let home = Path::new("/home/p");
        let mut list = vec![
            account("claude-1", Agent::Claude, "Work", &home.join(".claude")),
            account("codex-1", Agent::Codex, "Home", &home.join(".codex")),
            account("claude-2", Agent::Claude, "Side", &home.join(".claude-2")),
        ];
        for n in 3..=10 {
            list.push(account(
                &format!("codex-{n}"),
                Agent::Codex,
                &format!("C{n}"),
                &home.join(format!(".codex-{n}")),
            ));
        }
        *set.list.lock().unwrap() = Some(list);
        *set.windows.lock().unwrap() = vec![
            // 85 % of the five hours: a warning at the distro's lines.
            (
                "claude-1".to_owned(),
                vec![window(85.0, NOON + 3600), window(40.0, NOON + 86_400)],
            ),
            // 99 % of a week that ALREADY RESET: not what the account is now.
            (
                "codex-1".to_owned(),
                vec![window(99.0, NOON - 1), window(10.0, NOON + 60)],
            ),
            ("claude-2".to_owned(), vec![window(96.0, NOON + 60)]),
        ];
        let m = module(&set, None);
        let (ctx, _) = Ctx::kept("agent-accounts", NOON);
        m.inner.reread();
        let face = face_at(&m, NOON);
        assert_eq!(face.marks.len(), 9, "one digit: nine marks at most");
        let first = &face.marks[0];
        assert_eq!(first.id.name, "claude");
        assert_eq!(
            (first.id.instance, first.id.label.as_deref()),
            (1, Some("Work"))
        );
        assert_eq!(first.id.opens.as_deref(), Some(OPENS));
        assert_eq!(
            (first.tone, first.count, first.act.as_ref()),
            (Tone::Warning, 0, None)
        );
        let second = &face.marks[1];
        assert_eq!((second.id.name.as_ref(), second.id.instance), ("codex", 2));
        assert_eq!(second.tone, Tone::Quiet, "a reset window does not colour");
        assert_eq!(face.marks[2].tone, Tone::Danger);
        assert_eq!(face.marks[8].id.instance, 9);
        // THE CORE TAKES EVERY ONE: declared, resting, numbered within nine.
        let shown: Vec<&Mark> = face.marks.iter().collect();
        for (n, mark) in face.marks.iter().enumerate() {
            assert_eq!(
                super::super::MANIFEST.refusal_of(mark, &shown[..n]),
                None,
                "{mark:?}"
            );
        }
        // THE CLOCK: past the five hours the first account is calm again —
        // its week is at 40 %.
        assert_eq!(face_at(&m, NOON + 3600).marks[0].tone, Tone::Quiet);
        // THE PERSON'S LINES move the colours: from 30, that week warns.
        let config = scratch("lines-face");
        let m = module(&set, Some(config.clone()));
        write_line(&m.inner.lines, Which::Warn, 30).unwrap();
        write_line(&m.inner.lines, Which::Danger, 90).unwrap();
        m.started(NOON, &ctx);
        assert_eq!(face_at(&m, NOON + 3600).marks[0].tone, Tone::Warning);
        assert_eq!(face_at(&m, NOON).marks[1].tone, Tone::Quiet);
        assert_eq!(face_at(&m, NOON).marks[2].tone, Tone::Danger);
        let _ = std::fs::remove_dir_all(config);
    }

    /// FAILS IF: the marks outlive a list that stopped reading, or an empty
    /// list draws any.
    #[test]
    fn a_list_that_does_not_read_draws_no_mark_and_says_so() {
        let set = Arc::new(Set::default());
        *set.list.lock().unwrap() = Some(vec![account(
            "claude-1",
            Agent::Claude,
            "Work",
            Path::new("/home/p/.claude"),
        )]);
        let m = module(&set, None);
        m.inner.reread();
        assert_eq!(face_at(&m, NOON).marks.len(), 1);
        *set.list.lock().unwrap() = None;
        assert!(m.inner.reread());
        assert!(face_at(&m, NOON).marks.is_empty());
        let rows = &sections_of(&m.kept(), NOON)[0].rows;
        assert_eq!(rows[0].id, "unreadable");
        assert_eq!(rows[0].tone, Tone::Warning);
        *set.list.lock().unwrap() = Some(Vec::new());
        m.inner.reread();
        assert!(face_at(&m, NOON).marks.is_empty());
        assert_eq!(sections_of(&m.kept(), NOON)[0].rows[0].id, "none");
    }

    /// FAILS IF: the lines turn the colour one per cent early or late.
    #[test]
    fn the_lines_are_where_the_colour_turns() {
        let lines = Lines::default();
        assert_eq!(tone_of(None, lines), Tone::Quiet);
        assert_eq!(tone_of(Some(79.9), lines), Tone::Quiet);
        assert_eq!(tone_of(Some(80.0), lines), Tone::Warning);
        assert_eq!(tone_of(Some(94.9), lines), Tone::Warning);
        assert_eq!(tone_of(Some(95.0), lines), Tone::Danger);
        assert_eq!(tone_of(Some(140.0), lines), Tone::Danger);
        assert!(lines.fine());
        for bad in [(0, 50), (50, 50), (60, 40), (10, 101)] {
            assert!(
                !Lines {
                    warn_percent: bad.0,
                    danger_percent: bad.1
                }
                .fine(),
                "{bad:?}"
            );
        }
        assert_eq!(most_used(&[], NOON), None);
        assert_eq!(
            most_used(&[window(f32::NAN, NOON + 1), window(12.0, NOON + 1)], NOON),
            Some(12.0)
        );
    }

    /// FAILS IF: the run is wanted with no use since the last start, sooner
    /// than the pause after it, or not at all after use; or a run does not
    /// take the use it counts.
    #[test]
    fn the_run_is_wanted_after_use_and_not_on_a_clock() {
        assert!(!wanted(false, NOON, None), "nothing used: never");
        assert!(wanted(true, NOON, None), "used, never started: now");
        assert!(!wanted(true, NOON, Some(NOON - AGAIN_AFTER_S + 1)));
        assert!(wanted(true, NOON, Some(NOON - AGAIN_AFTER_S)));
        let set = Arc::new(Set::default());
        let m = module(&set, None);
        m.inner.used.store(true, Ordering::Release);
        let (ctx, _) = Ctx::kept("agent-accounts", NOON);
        let report = crate::task::Report::at(
            PathBuf::from("/nonexistent/agent-accounts.json"),
            std::time::Duration::ZERO,
            std::time::Duration::ZERO,
            Arc::new(AtomicBool::new(false)),
        )
        .rehearsing();
        assert!(matches!(
            m.run(&Run::new(report, ctx)),
            Outcome::Skipped { .. }
        ));
        assert!(
            m.inner.used.load(Ordering::Acquire),
            "a rehearsal counts nothing"
        );
        let due = m.due(NOON, &TaskPrefs::default(), None);
        assert!(matches!(due, Due::Not { .. }), "never by the calendar");
    }

    /// FAILS IF: a clock is armed with no window known, for a window that
    /// already reset, or later than the nearest reset — or for an account
    /// that has no mark.
    #[test]
    fn the_one_clock_is_the_nearest_reset() {
        let set = Arc::new(Set::default());
        *set.list.lock().unwrap() = Some(vec![
            account("claude-1", Agent::Claude, "Work", Path::new("/a")),
            account("codex-1", Agent::Codex, "Home", Path::new("/b")),
        ]);
        let m = module(&set, None);
        m.inner.reread();
        assert_eq!(m.next_wake(NOON), None, "nothing known: no clock");
        *set.windows.lock().unwrap() = vec![
            (
                "claude-1".to_owned(),
                vec![window(5.0, NOON - 10), window(5.0, NOON + 7200)],
            ),
            ("codex-1".to_owned(), vec![window(5.0, NOON + 3600)]),
        ];
        m.inner.reread();
        assert_eq!(m.next_wake(NOON), Some(NOON + 3600));
        assert_eq!(m.next_wake(NOON + 3600), Some(NOON + 7200));
        assert_eq!(m.next_wake(NOON + 7200), None, "after the last reset: none");
    }

    /// FAILS IF: a line typed on the page is kept when it is not a per cent
    /// or would cross the other, the file is not what the view reads, or a
    /// kept one does not move the page.
    #[test]
    fn the_lines_are_typed_on_the_page_and_kept_for_the_view() {
        let config = scratch("act");
        let set = Arc::new(Set::default());
        let m = module(&set, Some(config.clone()));
        let (ctx, kept) = Ctx::kept("agent-accounts", NOON);
        m.started(NOON, &ctx);
        let ask = |action: &str, answer: &str| Act {
            action: action.to_owned(),
            row: None,
            answer: Some(answer.to_owned()),
        };
        let before = face_at(&m, NOON).screen;
        assert_eq!(m.act(&ask("warn-at", " 70 % "), &ctx), Acted::Done);
        assert_eq!(
            read_lines(&lines_store(Some(config.clone()))).warn_percent,
            70
        );
        assert_ne!(face_at(&m, NOON).screen, before, "the page did not move");
        assert!(!kept.changes().is_empty());
        for (action, answer, why) in [
            ("warn-at", "seventy", super::super::why::NOT_A_PERCENT),
            ("warn-at", "0", super::super::why::NOT_A_PERCENT),
            ("danger-at", "101", super::super::why::NOT_A_PERCENT),
            ("danger-at", "60", super::super::why::NOT_IN_ORDER),
            ("warn-at", "95", super::super::why::NOT_IN_ORDER),
        ] {
            assert_eq!(
                m.act(&ask(action, answer), &ctx),
                Acted::Refused(why),
                "{answer}"
            );
        }
        assert_eq!(
            read_lines(&lines_store(Some(config.clone()))),
            Lines {
                warn_percent: 70,
                danger_percent: 95
            }
        );
        assert_eq!(m.act(&ask("nothing", "1"), &ctx), Acted::Refused("unknown"));
        // ANOTHER DOOR moved the danger, and this page did not hear it: the
        // warning typed here is laid over the FILE, never over the page's
        // copy — the danger stays where the other door put it.
        lines_store(Some(config.clone()))
            .write(&Lines {
                warn_percent: 70,
                danger_percent: 90,
            })
            .unwrap();
        assert_eq!(m.kept().lines.danger_percent, 95, "the page's copy is old");
        assert_eq!(m.act(&ask("warn-at", "60"), &ctx), Acted::Done);
        assert_eq!(
            read_lines(&lines_store(Some(config.clone()))),
            Lines {
                warn_percent: 60,
                danger_percent: 90
            }
        );
        // A FILE THAT DOES NOT READ is never written over.
        std::fs::write(config.join(LINES_FILE), "{ not json").unwrap();
        assert_eq!(
            m.act(&ask("danger-at", "99"), &ctx),
            Acted::Refused(super::super::why::UNREADABLE)
        );
        assert_eq!(
            std::fs::read_to_string(config.join(LINES_FILE)).unwrap(),
            "{ not json"
        );
        // A FILE THAT DOES NOT HOLD TWO LINES is the distro's, not a crossing
        // — and a line typed over it is laid over the distro's.
        std::fs::write(
            config.join(LINES_FILE),
            r#"{"warn-percent": 99, "danger-percent": 3}"#,
        )
        .unwrap();
        assert_eq!(
            read_lines(&lines_store(Some(config.clone()))),
            Lines::default()
        );
        assert_eq!(m.act(&ask("warn-at", "70"), &ctx), Acted::Done);
        assert_eq!(
            read_lines(&lines_store(Some(config.clone()))),
            Lines {
                warn_percent: 70,
                danger_percent: 95
            }
        );
        let _ = std::fs::remove_dir_all(config);
    }

    /// FAILS IF: on a fresh machine — the module's folder and the list's not
    /// made yet — a file the core watches has no folder to be watched from
    /// (the core's path watch is then down for good, and a line set in the
    /// view never reaches the marks); or the lines' watch going down and
    /// coming back is not said on the page and read again.
    #[test]
    fn the_folders_are_made_first_and_a_lost_watch_is_said() {
        let root = scratch("fresh");
        let config = root.join("config/ronoctua/brain/agent-accounts");
        let set = Arc::new(Set {
            file: Some(root.join("config-list/ronoctua").join("llm-accounts.json")),
            ..Set::default()
        });
        assert!(!config.exists());
        let m = module(&set, Some(config));
        let sources = m.sources();
        assert_eq!(sources.len(), 2, "the list and the lines");
        for source in &sources {
            let Source::Path(path) = source else {
                panic!("{source:?} is not a path");
            };
            assert!(
                path.parent().is_some_and(Path::is_dir),
                "{} has no folder to be watched from",
                path.display()
            );
        }
        let lines_at = m
            .ears
            .iter()
            .position(|(e, _)| *e == Ear::Lines)
            .expect("the lines' ear");
        let (ctx, _) = Ctx::kept("agent-accounts", NOON);
        m.started(NOON, &ctx);
        let rows = |m: &AgentAccounts| sections_of(&m.kept(), NOON)[1].rows.clone();
        let before = face_at(&m, NOON).screen;
        m.heard(
            Heard::Lost {
                source: lines_at,
                why: "no more watches",
            },
            &ctx,
        );
        let lost = rows(&m);
        assert_eq!(lost[0].id, "lines-lost");
        assert_eq!(lost[0].value, Cell::Data("no more watches".to_owned()));
        assert_eq!(lost[0].tone, Tone::Warning);
        assert_ne!(face_at(&m, NOON).screen, before, "the page did not move");
        // WHAT CHANGED WHILE IT WAS DOWN is read when it is back.
        write_line(&m.inner.lines, Which::Warn, 30).unwrap();
        m.heard(Heard::Back { source: lines_at }, &ctx);
        assert!(rows(&m).iter().all(|r| r.id != "lines-lost"));
        assert_eq!(m.kept().lines.warn_percent, 30);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a window resetting leaves the page as the host last asked
    /// for it — still showing the full window in danger.
    #[test]
    fn a_reset_moves_the_page() {
        let set = Arc::new(Set::default());
        *set.list.lock().unwrap() = Some(vec![account(
            "claude-1",
            Agent::Claude,
            "Work",
            Path::new("/a"),
        )]);
        *set.windows.lock().unwrap() = vec![("claude-1".to_owned(), vec![window(96.0, NOON + 60)])];
        let m = module(&set, None);
        m.inner.reread();
        let row = |now| sections_of(&m.kept(), now)[0].rows[0].clone();
        assert_eq!(row(NOON).tone, Tone::Danger);
        assert_eq!(row(NOON + 60).tone, Tone::Quiet, "reset: not known");
        let before = face_at(&m, NOON).screen;
        let (ctx, kept) = Ctx::kept("agent-accounts", NOON + 60);
        m.heard(Heard::Wake, &ctx);
        assert_ne!(
            face_at(&m, NOON + 60).screen,
            before,
            "the host would not ask again"
        );
        assert!(!kept.changes().is_empty());
    }

    /// A machine whose first reading of windows waits to be let go — the
    /// slow Codex logs.
    struct Slow {
        set: Arc<Set>,
        hold: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
        holding: std::sync::mpsc::Sender<()>,
    }

    impl Probe for Arc<Slow> {
        fn accounts_file(&self) -> Option<PathBuf> {
            None
        }
        fn accounts(&self) -> Option<Vec<Account>> {
            self.set.accounts()
        }
        fn windows(&self, a: &Account) -> Vec<Window> {
            let read = self.set.windows(a);
            let hold = self.hold.lock().unwrap().take();
            if let Some(hold) = hold {
                let _ = self.holding.send(());
                let _ = hold.recv();
            }
            read
        }
        fn watched(&self, a: &Account) -> Watched {
            self.set.watched(a)
        }
    }

    /// FAILS IF: a slow reading, kept after a newer one, puts the older
    /// numbers back — three threads read, and the last to FINISH must be
    /// the last to have READ.
    #[test]
    fn a_slow_reading_never_puts_older_numbers_back() {
        let set = Arc::new(Set::default());
        *set.list.lock().unwrap() = Some(vec![account(
            "codex-1",
            Agent::Codex,
            "Home",
            Path::new("/b"),
        )]);
        *set.windows.lock().unwrap() = vec![("codex-1".to_owned(), vec![window(10.0, NOON + 60)])];
        let (let_go, hold) = std::sync::mpsc::channel();
        let (holding, held) = std::sync::mpsc::channel();
        let slow = Arc::new(Slow {
            set: Arc::clone(&set),
            hold: Mutex::new(Some(hold)),
            holding,
        });
        let m = Arc::new(AgentAccounts::of(Box::new(slow), None));
        let first = {
            let m = Arc::clone(&m);
            std::thread::spawn(move || m.inner.reread())
        };
        held.recv().expect("the first reading began");
        // THE NUMBERS MOVE while the first reading is still out.
        *set.windows.lock().unwrap() = vec![("codex-1".to_owned(), vec![window(96.0, NOON + 60)])];
        let second = {
            let m = Arc::clone(&m);
            std::thread::spawn(move || m.inner.reread())
        };
        std::thread::sleep(std::time::Duration::from_millis(150));
        let_go.send(()).unwrap();
        first.join().unwrap();
        second.join().unwrap();
        assert_eq!(
            m.kept().accounts[0].windows,
            [window(96.0, NOON + 60)],
            "the older reading was kept last"
        );
        assert_eq!(face_at(&m, NOON).marks[0].tone, Tone::Danger);
    }

    /// FAILS IF: the module's watches miss a folder the numbers are kept in
    /// (or do not make it first), watch a log tree that is not there instead
    /// of the account's folder, or are raised again for nothing.
    #[test]
    fn the_watches_cover_what_the_crate_names() {
        let root = scratch("targets");
        let set = Arc::new(Set::default());
        let claude = account(
            "claude-1",
            Agent::Claude,
            "Work",
            &root.join("home/.claude"),
        );
        let codex = account("codex-1", Agent::Codex, "Home", &root.join("home/.codex"));
        std::fs::create_dir_all(claude.dir.join("projects")).unwrap();
        std::fs::create_dir_all(&codex.dir).unwrap();
        *set.list.lock().unwrap() = Some(vec![claude.clone(), codex.clone()]);
        *set.watched.lock().unwrap() = vec![
            (
                "claude-1".to_owned(),
                Watched {
                    files: vec![
                        root.join("state/llm/claude-1/limits.json"),
                        root.join("state/llm/claude-1/usage.json"),
                        root.join("cache/llm/claude-1/tokens.json"),
                    ],
                    trees: vec![claude.dir.join("projects")],
                },
            ),
            (
                "codex-1".to_owned(),
                Watched {
                    files: vec![root.join("cache/llm/codex-1/tokens.json")],
                    trees: vec![
                        codex.dir.join("sessions"),
                        codex.dir.join("archived_sessions"),
                    ],
                },
            ),
        ];
        let m = module(&set, None);
        let t = m.inner.targets();
        assert_eq!(
            t.numbers,
            [
                root.join("cache/llm/claude-1"),
                root.join("cache/llm/codex-1"),
                root.join("state/llm/claude-1"),
            ]
        );
        assert!(t.numbers.iter().all(|f| f.is_dir()), "made before watched");
        assert_eq!(
            t.trees,
            [claude.dir.join("projects")],
            "only trees that are there"
        );
        assert_eq!(t.folders, [claude.dir, codex.dir]);
        let (ctx, _) = Ctx::kept("agent-accounts", NOON);
        rewatch(&m.inner, &ctx);
        assert_eq!(m.inner.watches.lock().unwrap().aimed.as_ref(), Some(&t));
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: an agent writing a transcript is not heard as use, a log
    /// tree born in an account's folder is not then watched whole, or the
    /// distro's own writes count as use — the usage unit's numbers (it would
    /// start itself for ever) and the `settings.json` the view writes in an
    /// account's folder (removing an account would ask the endpoint).
    #[test]
    fn a_transcript_written_is_use_and_the_units_own_writes_are_not() {
        let root = scratch("heard");
        let set = Arc::new(Set::default());
        let claude = account(
            "claude-1",
            Agent::Claude,
            "Work",
            &root.join("home/.claude"),
        );
        std::fs::create_dir_all(&claude.dir).unwrap();
        *set.list.lock().unwrap() = Some(vec![claude.clone()]);
        let numbers = root.join("state/llm/claude-1");
        *set.watched.lock().unwrap() = vec![(
            "claude-1".to_owned(),
            Watched {
                files: vec![numbers.join("usage.json")],
                trees: vec![claude.dir.join("projects")],
            },
        )];
        let m = module(&set, None);
        let (ctx, kept) = Ctx::kept("agent-accounts", NOON);
        m.started(NOON, &ctx);
        let until = |what: &dyn Fn() -> bool| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            while !what() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            what()
        };
        // A burst is reported at its start and once more at its end
        // (`Pace::Coalesced`): settled is after both.
        let settle = || std::thread::sleep(std::time::Duration::from_millis(800));
        // THE UNIT WRITES ITS NUMBERS: read again, and no use.
        std::fs::write(numbers.join("usage.json"), "{}").unwrap();
        settle();
        assert!(
            !m.inner.used.load(Ordering::Acquire),
            "the unit's own write counted as use"
        );
        // THE VIEW WRITES THE ACCOUNT'S SETTINGS (its status line put in
        // front, or given back): heard in the folder, and not use.
        std::fs::write(claude.dir.join("settings.json"), "{}\n").unwrap();
        settle();
        assert!(
            !m.inner.used.load(Ordering::Acquire),
            "a write in the account's folder that is no log counted as use"
        );
        // CLAUDE CODE STARTS: its log tree is born, then a transcript in it.
        let project = claude.dir.join("projects").join("-home-p-work");
        std::fs::create_dir_all(&project).unwrap();
        assert!(
            until(&|| m.inner.used.load(Ordering::Acquire)),
            "the tree's birth was not heard"
        );
        assert!(until(&|| m
            .inner
            .watches
            .lock()
            .unwrap()
            .aimed
            .as_ref()
            .is_some_and(|t| t
                .trees
                .contains(&claude.dir.join("projects")))));
        settle();
        m.inner.used.store(false, Ordering::Release);
        settle();
        assert!(
            !m.inner.used.load(Ordering::Acquire),
            "use heard from nothing"
        );
        std::fs::write(project.join("s1.jsonl"), "{}\n").unwrap();
        assert!(
            until(&|| m.inner.used.load(Ordering::Acquire)),
            "a transcript written was not heard as use"
        );
        assert!(!kept.changes().is_empty());
        let _ = std::fs::remove_dir_all(root);
    }
}
