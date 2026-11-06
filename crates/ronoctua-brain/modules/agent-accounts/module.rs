//! The person's LLM accounts on the modeline — one set each: the account's
//! glyph, coloured by how much of its limits is used, and its agents — and
//! the usage unit, started after an account was used.
//!
//! Why each piece is so is in `why.md`, beside this file. What is here is the
//! shape of it.
//!
//! # What it reads, and from whom
//!
//! Everything about an account comes from `ronoctua_llm`, the one crate that
//! reads the agents' state: the list (`Accounts`), what to watch for each
//! (`watched`), the limits Claude Code's status line kept or Codex wrote in
//! its logs (`statusline::limits`), the usage endpoint's last answer
//! (`usage::kept`) and an API key's word on its credits (`credits`). The
//! module names no file and no folder of the agents'.
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
//! One SET per account, AT REST TOO — the owner's exception
//! (`assets/brain-resting-marks.nix`), in the account's place in the
//! person's list. It leads with the account's own glyph — Claude Code's
//! spark, Codex's ring, our dotted "A" for Claude Code on an API key — in the
//! colour of the most used of its windows that has not reset: quiet, a
//! warning from [`Lines::warn_percent`], danger from [`Lines::danger_percent`].
//! An API key has no window: its lead is danger while Claude Code says its
//! credits ran out, and quiet otherwise. A click on the lead opens the
//! accounts' view on that account (`agents-open <n>`). Nine sets at most.
//!
//! Then the agents working in the account, each a robot's head in the place
//! it keeps (`Places`) and in its state's tone, and past the ninth the count
//! of the rest — all made by the shared half (`marks_for`), so a
//! photograph's are these. Where the
//! agents are read from is one seam (`Probe::agents`, `agents_now`): the
//! records the managed hook writes, one per Claude Code session, and what a
//! program in one of our terminals said of itself (OSC 7501), which the
//! terminal's holder keeps in its state file — merged by `ronoctua_llm`'s one
//! reader. A click on an agent goes to it (`agents-show <session>`).
//!
//! # Three watches of its own, and why
//!
//! The core reads a watcher's sources ONCE, when its thread starts, and the
//! accounts are the person's to add at any moment. So the core watches the
//! paths that never change — the list and the lines, their folders made
//! first, since a file is watched from its folder, and the agents' two
//! folders, made private first (the hook's sessions, the terminals' state
//! files): a change there reads the agents alone — and the
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
//! the clock the module asks for is the nearest reset of a window it shows,
//! so a mark that is red because the five hours are used turns quiet when
//! they start over — and no clock at all when no window is known.
//!
//! One more, and only while it is needed: an agent OUTSIDE our terminals (a
//! Ghostty, an editor's) killed with no `SessionEnd` writes nothing anybody
//! hears — inside one of ours its holder's file moves when the program
//! does. So while such an agent is drawn, its process is asked again every
//! [`LIVENESS_S`]; with none drawn, no clock (owner, 2026-11-02).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use ronoctua_files::notice::{Notice, Pace};
use ronoctua_llm::{Account, Kind as Agent, Watched, Window};

// THE SHARED HALF (`shared.rs`, compiled into the library beside the
// manifest): what the accounts' view reads of the same machine.
use super::{
    AgentSeen, Lines, Places, Seen, Which, accounts_now, agents_now, credits_now, credits_tone,
    lines_store, make_agents_folders, marked_most, marks_for, most_used, outside_is_drawn,
    read_lines, sessions_folder, terminals_folder, tone_of, windows_now, write_line,
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

/// How long an agent outside our terminals is trusted to be alive after it
/// was last read: such an agent killed with no `SessionEnd` (a `kill -9`, a
/// terminal closed under it) writes nothing anybody hears, so its process is
/// asked again this often — and only while one is drawn.
pub const LIVENESS_S: i64 = 60;

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
    /// Since when an API key's credits ran out, as Claude Code last said;
    /// `None` for every other account.
    fn out_of_credits(&self, account: &Account) -> Option<Moment>;
    /// What to watch for it.
    fn watched(&self, account: &Account) -> Watched;
    /// The agents at work in these accounts now — the one seam their
    /// reading comes in by (`agents_now`).
    fn agents(&self, accounts: &[Account]) -> Vec<AgentSeen>;
    /// Where the agents are read from, watched by the core; `None` without a
    /// runtime folder, where the hook and the terminals write nothing.
    fn agents_folders(&self) -> Option<Folders>;
}

/// The two folders the agents are read from.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Folders {
    /// The hook's records, one per session.
    sessions: PathBuf,
    /// Our terminals' state files, where a program's OSC 7501 is kept.
    terminals: PathBuf,
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

    fn out_of_credits(&self, account: &Account) -> Option<Moment> {
        credits_now(account)
    }

    fn watched(&self, account: &Account) -> Watched {
        ronoctua_llm::watched(account)
    }

    fn agents(&self, accounts: &[Account]) -> Vec<AgentSeen> {
        agents_now(accounts)
    }

    fn agents_folders(&self) -> Option<Folders> {
        Some(Folders {
            sessions: sessions_folder()?,
            terminals: terminals_folder()?,
        })
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
    /// The agents at work in them.
    agents: Vec<AgentSeen>,
    /// The places the agents keep in their sets: kept across readings, so a
    /// glyph keeps its agent.
    places: Places,
    /// The list is there and does not read: the marks it had stay off, and
    /// the page says so.
    unreadable: bool,
    /// Why the list cannot be watched, in the machine's words.
    lost: Option<String>,
    /// Why the lines cannot be watched: a line changed in another door is
    /// not heard, and the page says so.
    lines_lost: Option<String>,
    /// Why a folder the agents are read from cannot be watched, by its ear:
    /// an agent that starts, waits or ends is not heard, and the page says
    /// so.
    agents_lost: Vec<(Ear, String)>,
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

/// Which of the core's sources a heard path is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ear {
    /// The list of accounts.
    Accounts,
    /// The person's lines.
    Lines,
    /// The hook's records of the sessions.
    Sessions,
    /// Our terminals' state files.
    Terminals,
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
    /// When the agents were last read by a watch of the core's — what the
    /// liveness clock counts from.
    agents_at: AtomicI64,
    /// When the module's clock last rang, or the module started — what tells
    /// a window's reset, which moves the page, from the liveness clock's
    /// ring, which moves nothing unless an agent went.
    rang_at: AtomicI64,
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
        // THE AGENTS' TWO FOLDERS, private: the hook's and the terminals'
        // writers keep `0700` folders, and one made here by `create_dir_all`
        // would be anybody's to read. A folder that cannot be made is still
        // watched — its birth is heard — and the page says why if it is not.
        let agents = probe.agents_folders();
        if let Some(f) = &agents
            && let Err(e) = make_agents_folders(&f.sessions, &f.terminals)
        {
            tracing::warn!(error = %e, "the agents' folders could not be made");
        }
        let lines = lines_store(config);
        let mut ears = Vec::new();
        if let Some(path) = probe.accounts_file() {
            ears.push((Ear::Accounts, path));
        }
        if let Some(path) = lines.path() {
            ears.push((Ear::Lines, path));
        }
        if let Some(f) = agents {
            ears.push((Ear::Sessions, f.sessions));
            ears.push((Ear::Terminals, f.terminals));
        }
        Self {
            inner: Arc::new(Inner {
                probe,
                lines,
                kept: Mutex::new(Kept::default()),
                reading: Mutex::new(()),
                used: AtomicBool::new(false),
                agents_at: AtomicI64::new(0),
                rang_at: AtomicI64::new(0),
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
            let seen = accounts
                .iter()
                .map(|a| Seen::of(a, self.probe.windows(a), self.probe.out_of_credits(a)))
                .collect::<Vec<_>>();
            (seen, self.probe.agents(&accounts))
        });
        self.keep(|k| match read {
            Some((accounts, agents)) => {
                k.places.settle(&agents);
                k.accounts = accounts;
                k.agents = agents;
                k.unreadable = false;
            }
            // A LIST THAT DOES NOT READ keeps no marks it cannot vouch for.
            None => {
                k.accounts.clear();
                k.agents.clear();
                k.places = Places::default();
                k.unreadable = true;
            }
        })
    }

    /// Everything read again, at `now` — the agents' clock counting from it.
    fn reread_at(&self, now: Moment) -> bool {
        let moved = self.reread();
        self.agents_at.store(now, Ordering::Relaxed);
        moved
    }

    /// The agents alone read again, at `now`, over the list as it says now —
    /// a session started, waited or ended, a program in one of our
    /// terminals said its state, the liveness clock rang. The accounts'
    /// numbers are not read: nothing of theirs moved. `true` when anything
    /// did. In turn with the other readings, for the same reason.
    fn reread_agents(&self, now: Moment) -> bool {
        let _one = self.reading.lock().unwrap_or_else(PoisonError::into_inner);
        self.agents_at.store(now, Ordering::Relaxed);
        // A LIST THAT DOES NOT READ is the list's ear's to say; its marks
        // are off, and no agent is put back under them.
        let Some(accounts) = self.probe.accounts() else {
            return false;
        };
        let agents = self.probe.agents(&accounts);
        self.keep(|k| {
            if k.unreadable {
                return;
            }
            k.places.settle(&agents);
            k.agents = agents;
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

/// Whether a window the page shows reset after `since` and by `now`: the
/// page says how full each account is, and a reset changes that — every
/// account's, the ones past the ninth too (the page lists them).
fn reset_between(kept: &Kept, since: Moment, now: Moment) -> bool {
    kept.accounts
        .iter()
        .flat_map(|s| s.windows.iter())
        .any(|w| w.resets_at_s > since && w.resets_at_s <= now)
}

/// The marks: one set per account, by its place in the list, its agents in
/// it — the shared half's own (`marks_for`), so a photograph's marks are
/// these.
fn marks_of(kept: &Kept, now: Moment) -> Vec<Mark> {
    marks_for(&kept.accounts, &kept.agents, &kept.places, kept.lines, now)
}

/// A text key as a cell.
fn key(k: &'static str) -> Cell {
    Cell::Key(k.into())
}

/// The page's rows: each account with the most used of its windows, or why
/// there is none — an API key's with its credits, which is all it has —
/// and the two lines.
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
    // AN AGENT'S FOLDER NOT WATCHED: its agents would stand still on the
    // modeline, said in the machine's words.
    if let Some((_, why)) = kept.agents_lost.first() {
        accounts.push(Row {
            id: "agents-lost".to_owned(),
            label: key(super::key::AGENT_ACCOUNTS_ROW_AGENTS_UNWATCHED),
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
        let note = if n >= marked_most() {
            super::key::AGENT_ACCOUNTS_NOTE_NO_MARK
        } else {
            match seen.agent {
                Agent::Claude => super::key::AGENT_ACCOUNTS_NOTE_CLAUDE,
                Agent::ClaudeApi => super::key::AGENT_ACCOUNTS_NOTE_CLAUDE_API,
                Agent::Codex => super::key::AGENT_ACCOUNTS_NOTE_CODEX,
            }
        };
        let (value, tone) = if seen.agent.has_windows() {
            let used = most_used(&seen.windows, now);
            (
                used.map_or_else(
                    || key(super::key::AGENT_ACCOUNTS_UNKNOWN),
                    // A WHOLE PER CENT: the window's own number has a
                    // decimal, and the page says how full, not to a tenth.
                    |p| Cell::Count(rounded(p)),
                ),
                tone_of(used, kept.lines),
            )
        } else {
            // A KEY HAS NO WINDOW: a row saying "not known yet" would never
            // become known. What it has is its credits' word.
            let out = seen.out_of_credits_since;
            (
                key(if out.is_some() {
                    super::key::AGENT_ACCOUNTS_CREDITS_OUT
                } else {
                    super::key::AGENT_ACCOUNTS_CREDITS_OK
                }),
                credits_tone(out),
            )
        };
        accounts.push(Row {
            id: format!("account-{}", seen.id),
            label: Cell::Data(seen.name.clone()),
            value,
            note: key(note),
            tone,
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
    /// The paths that never change: the list, the lines, and the agents'
    /// two folders.
    fn sources(&self) -> Vec<Source> {
        self.ears
            .iter()
            .map(|(_, path)| Source::Path(path.clone()))
            .collect()
    }

    /// THE FIRST READING, after the core's watches are raised — and the
    /// module's own raised over what it found.
    fn started(&self, now: Moment, ctx: &Ctx) {
        self.inner.rang_at.store(now, Ordering::Relaxed);
        self.inner.reread_lines();
        self.inner.reread_at(now);
        rewatch(&self.inner, ctx);
        ctx.changed();
    }

    fn heard(&self, heard: Heard<'_>, ctx: &Ctx) {
        let ear = |source: usize| self.ears.get(source).map(|(e, _)| *e);
        match heard {
            Heard::Path { source, .. } => match ear(source) {
                Some(Ear::Accounts) => {
                    self.inner.reread_at(ctx.now());
                    rewatch(&self.inner, ctx);
                    ctx.changed();
                }
                Some(Ear::Lines) if self.inner.reread_lines() => ctx.changed(),
                // AN AGENT MOVED — the hook wrote a session, a terminal's
                // holder wrote its state — and only the agents are read.
                Some(Ear::Sessions | Ear::Terminals) if self.inner.reread_agents(ctx.now()) => {
                    ctx.changed();
                }
                Some(Ear::Lines | Ear::Sessions | Ear::Terminals) | None => {}
            },
            Heard::Lost { source, why } => {
                let moved = match ear(source) {
                    Some(Ear::Accounts) => self.inner.keep(|k| k.lost = Some(why.to_owned())),
                    Some(Ear::Lines) => self.inner.keep(|k| k.lines_lost = Some(why.to_owned())),
                    Some(e @ (Ear::Sessions | Ear::Terminals)) => self.inner.keep(|k| {
                        k.agents_lost.retain(|(lost, _)| *lost != e);
                        k.agents_lost.push((e, why.to_owned()));
                    }),
                    None => false,
                };
                if moved {
                    ctx.changed();
                }
            }
            Heard::Back { source } => match ear(source) {
                Some(Ear::Accounts) => {
                    self.inner.keep(|k| k.lost = None);
                    self.inner.reread_at(ctx.now());
                    rewatch(&self.inner, ctx);
                    ctx.changed();
                }
                // WHAT AN AGENT DID while its folder was not heard is read
                // now.
                Some(e @ (Ear::Sessions | Ear::Terminals)) => {
                    self.inner
                        .keep(|k| k.agents_lost.retain(|(lost, _)| *lost != e));
                    self.inner.reread_agents(ctx.now());
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
            // OR THE LIVENESS CLOCK: the agents outside our terminals, whose
            // end nothing may have said, are asked again — and a ring that
            // finds every one alive tells nobody: a mind published, and the
            // page moved, every minute an agent is drawn, would be the clock
            // at rest this module has none of.
            Heard::Wake => {
                let now = ctx.now();
                let since = self.inner.rang_at.swap(now, Ordering::Relaxed);
                let went = self.inner.reread_agents(now);
                let reset = reset_between(&self.kept(), since, now);
                if reset {
                    self.inner.generation.fetch_add(1, Ordering::Relaxed);
                }
                if went || reset {
                    ctx.changed();
                }
            }
            Heard::Journal { .. } | Heard::Line { .. } => {}
        }
    }

    /// The nearest reset of a window shown — and, while an agent outside
    /// our terminals is drawn, the liveness clock ([`LIVENESS_S`] after the
    /// agents were last read): nothing at all when neither is.
    fn next_wake(&self, now: Moment) -> Option<Moment> {
        let kept = self.kept();
        let alive = outside_is_drawn(&kept.accounts, &kept.agents).then(|| {
            self.inner
                .agents_at
                .load(Ordering::Relaxed)
                .saturating_add(LIVENESS_S)
        });
        nearest_reset(&kept, now).into_iter().chain(alive).min()
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

    /// A machine the test sets. With `folders`, its agents are READ from
    /// them by the shared half's own reader (`agents_in`), every process
    /// alive but the `dead` ones; without, they are the planted `agents`.
    #[derive(Default)]
    struct Set {
        list: Mutex<Option<Vec<Account>>>,
        windows: Mutex<Vec<(String, Vec<Window>)>>,
        credits: Mutex<Vec<(String, Moment)>>,
        watched: Mutex<Vec<(String, Watched)>>,
        agents: Mutex<Vec<AgentSeen>>,
        folders: Option<Folders>,
        dead: Mutex<Vec<u32>>,
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
        fn out_of_credits(&self, a: &Account) -> Option<Moment> {
            let all = self.credits.lock().unwrap();
            all.iter().find(|(id, _)| *id == a.id).map(|(_, at)| *at)
        }
        fn watched(&self, a: &Account) -> Watched {
            let all = self.watched.lock().unwrap();
            all.iter()
                .find(|(id, _)| *id == a.id)
                .map(|(_, w)| w.clone())
                .unwrap_or_default()
        }
        fn agents(&self, accounts: &[Account]) -> Vec<AgentSeen> {
            let Some(f) = &self.folders else {
                return self.agents.lock().unwrap().clone();
            };
            let dead = self.dead.lock().unwrap().clone();
            super::super::agents_in(
                accounts,
                &f.sessions,
                Some(&f.terminals),
                Some(Path::new("/home/p")),
                &|pid, _| !dead.contains(&pid),
            )
        }
        fn agents_folders(&self) -> Option<Folders> {
            self.folders.clone()
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

    /// FAILS IF: an account has no set, or one numbered by anything but its
    /// place in the person's whole list, led by anything but its own mark
    /// named by the person's word and opening the accounts' view; a lead is
    /// coloured by a window that already reset or past the person's lines; a
    /// tenth account gets a set the modeline has no room for; or the core
    /// would refuse one of them (`Manifest::refusal_of`).
    #[test]
    fn one_set_per_account_by_its_place_led_by_its_most_used_window() {
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
        assert_eq!(face.marks.len(), 9, "nine sets at most");
        let first = &face.marks[0];
        assert_eq!(first.id.name, "claude");
        assert_eq!(
            (first.id.set, first.id.instance, first.id.label.as_deref()),
            (1, 0, Some("Work"))
        );
        assert_eq!(first.id.opens.as_deref(), Some(OPENS));
        assert_eq!(
            (first.tone, first.count, first.act.as_ref()),
            (Tone::Warning, 0, None)
        );
        let second = &face.marks[1];
        assert_eq!((second.id.name.as_ref(), second.id.set), ("codex", 2));
        assert_eq!(second.tone, Tone::Quiet, "a reset window does not colour");
        assert_eq!(face.marks[2].tone, Tone::Danger);
        assert_eq!(face.marks[8].id.set, 9);
        // THE CORE TAKES EVERY ONE: declared, resting, in a set within nine.
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

    /// FAILS IF: an API key's account draws no mark, is not danger while
    /// Claude Code says its credits ran out — on the modeline and on the
    /// page — stays danger once they are back, or keeps a "not known yet"
    /// row for windows a key never has; or the core would refuse its mark.
    #[test]
    fn an_api_keys_mark_is_danger_while_its_credits_are_out() {
        let set = Arc::new(Set::default());
        *set.list.lock().unwrap() = Some(vec![
            account(
                "claude-1",
                Agent::Claude,
                "Work",
                Path::new("/home/p/.claude"),
            ),
            account(
                "claude-api-1",
                Agent::ClaudeApi,
                "Console",
                Path::new("/home/p/.claude-api"),
            ),
        ]);
        let m = module(&set, None);
        m.inner.reread();
        let face = face_at(&m, NOON);
        let api = &face.marks[1];
        assert_eq!(
            (api.id.name.as_ref(), api.id.set, api.id.label.as_deref()),
            ("claude-api", 2, Some("Console"))
        );
        assert_eq!(api.tone, Tone::Quiet);
        let row = |m: &AgentAccounts| {
            sections_of(&m.kept(), NOON)[0]
                .rows
                .iter()
                .find(|r| r.id == "account-claude-api-1")
                .cloned()
                .expect("the API key's row")
        };
        assert_eq!(
            row(&m).value,
            key(super::super::key::AGENT_ACCOUNTS_CREDITS_OK)
        );
        assert_eq!(
            row(&m).note,
            key(super::super::key::AGENT_ACCOUNTS_NOTE_CLAUDE_API)
        );

        // OUT OF CREDITS: heard on a re-read, as the numbers' watch rings.
        *set.credits.lock().unwrap() = vec![("claude-api-1".to_owned(), NOON - 600)];
        assert!(m.inner.reread(), "the page did not move");
        let face = face_at(&m, NOON);
        assert_eq!(face.marks[1].tone, Tone::Danger);
        assert_eq!(
            face.marks[0].tone,
            Tone::Quiet,
            "the subscription is its own"
        );
        assert_eq!(
            (row(&m).value, row(&m).tone),
            (
                key(super::super::key::AGENT_ACCOUNTS_CREDITS_OUT),
                Tone::Danger
            )
        );
        let shown: Vec<&Mark> = face.marks.iter().collect();
        for (n, mark) in face.marks.iter().enumerate() {
            assert_eq!(super::super::MANIFEST.refusal_of(mark, &shown[..n]), None);
        }
        assert_eq!(m.next_wake(NOON), None, "a key has no window to wake for");

        set.credits.lock().unwrap().clear();
        m.inner.reread();
        assert_eq!(face_at(&m, NOON).marks[1].tone, Tone::Quiet);
    }

    /// FAILS IF: the agents the module hears through its one seam
    /// (`Probe::agents`) do not stand in their account's set, a glyph moves
    /// to another agent across readings when one before it ends, a lead in
    /// danger for its credits is quieted by its agents, or the core would
    /// refuse a mark of the set (`Manifest::refusal_of`).
    #[test]
    fn the_agents_heard_stand_in_their_accounts_set_and_keep_their_places() {
        use super::super::Doing;
        let set = Arc::new(Set::default());
        *set.list.lock().unwrap() = Some(vec![
            account("claude-1", Agent::Claude, "Work", Path::new("/a")),
            account("claude-api-1", Agent::ClaudeApi, "Console", Path::new("/b")),
        ]);
        *set.credits.lock().unwrap() = vec![("claude-api-1".to_owned(), NOON - 600)];
        let agent = |account: &str, n: u32, doing: Doing| AgentSeen {
            account: account.to_owned(),
            session: format!("{n:08x}-0000-4000-8000-000000000000"),
            folder: format!("~/p{n}"),
            doing,
            since: NOON + i64::from(n),
            terminal: None,
            pid: 0,
        };
        let (a, b, c) = (
            agent("claude-api-1", 1, Doing::Working),
            agent("claude-api-1", 2, Doing::Waiting),
            agent("claude-api-1", 3, Doing::Done),
        );
        *set.agents.lock().unwrap() = vec![a.clone(), b.clone(), c.clone()];
        let m = module(&set, None);
        m.inner.reread();
        let face = face_at(&m, NOON);
        let shown: Vec<&Mark> = face.marks.iter().collect();
        for (n, mark) in face.marks.iter().enumerate() {
            assert_eq!(
                super::super::MANIFEST.refusal_of(mark, &shown[..n]),
                None,
                "{mark:?}"
            );
        }
        let second: Vec<(&str, u8)> = face
            .marks
            .iter()
            .filter(|mk| mk.id.set == 2)
            .map(|mk| (mk.id.name.as_ref(), mk.id.instance))
            .collect();
        assert_eq!(
            second,
            [
                ("claude-api", 0),
                ("agent-working", 1),
                ("agent-waiting", 2),
                ("agent-done", 3)
            ]
        );
        assert_eq!(face.marks[1].tone, Tone::Danger, "the lead's danger went");
        assert!(
            face.marks.iter().filter(|mk| mk.id.set == 1).count() == 1,
            "an agent of the second account stood in the first's set"
        );
        // THE FIRST ENDS: the others keep their places, and a new one takes
        // the lowest free.
        let d = agent("claude-api-1", 4, Doing::Working);
        *set.agents.lock().unwrap() = vec![b.clone(), c.clone(), d.clone()];
        m.inner.reread();
        let keyed = |face: &Face, who: &AgentSeen| {
            face.marks
                .iter()
                .find(|mk| mk.id.key.as_deref() == Some(who.session.as_str()))
                .map(|mk| mk.id.instance)
        };
        let face = face_at(&m, NOON);
        assert_eq!(
            [&a, &b, &c, &d].map(|x| keyed(&face, x)),
            [None, Some(2), Some(3), Some(1)]
        );
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

    /// A session id of `n`, as Claude Code writes them.
    fn session_id(n: u32) -> String {
        format!("5f0c1e2a-0000-4000-8000-{n:012x}")
    }

    /// The hook's record of a session, written where the hook writes it.
    fn record(
        folder: &Path,
        n: u32,
        account_dir: &str,
        terminal: Option<&str>,
        status: ronoctua_llm::sessions::Status,
    ) {
        let session = ronoctua_llm::sessions::Session {
            format: ronoctua_llm::sessions::FORMAT,
            session: session_id(n),
            account_dir: account_dir.into(),
            pid: 100 + n,
            pid_start_ticks: 7,
            terminal: terminal.map(str::to_owned),
            folder: format!("/home/p/work-{n}").into(),
            transcript: None,
            status,
            said_at_ns: 1,
            started_at_ns: u64::try_from(NOON).unwrap() * 1_000_000_000 + u64::from(n),
            ended: false,
        };
        std::fs::write(
            folder.join(format!("{}.json", session.session)),
            serde_json::to_vec(&session).unwrap(),
        )
        .unwrap();
    }

    /// One of our terminals' state files, its program saying `state` of
    /// itself (OSC 7501) — written as its holder writes it.
    fn terminal_says(folder: &Path, id: &str, state: &str) {
        std::fs::write(
            folder.join(format!("{id}.json")),
            format!(
                r#"{{"protocol":1,"id":"{id}","group":"fedcba9876543210","order":0,"program":"zsh","started_s":5,"statuses":[{{"id":"","app":"claude-code","state":"{state}","at_ms":1}}]}}"#
            ),
        )
        .unwrap();
    }

    /// The marks of one set, as (name, instance, tone, key).
    fn set_of(face: &Face, set: u8) -> Vec<(String, u8, Tone, Option<String>)> {
        face.marks
            .iter()
            .filter(|mk| mk.id.set == set)
            .map(|mk| {
                (
                    mk.id.name.to_string(),
                    mk.id.instance,
                    mk.tone,
                    mk.id.key.as_ref().map(ToString::to_string),
                )
            })
            .collect()
    }

    /// FAILS IF: the agents are not read where the hook and our terminals
    /// write — both folders watched by the core and made private first — an
    /// agent does not land in its account's set in the place it keeps, the
    /// program's own word (OSC 7501) does not beat the hook's inside our
    /// terminal, a process that died (heard by the liveness clock) or a
    /// terminal whose holder's file went is still drawn, a billing failure
    /// no longer makes its lead danger beside its agents, or a folder that
    /// cannot be watched is not said on the page and read again when back.
    #[test]
    fn the_agents_are_read_where_the_hook_and_the_terminals_write() {
        use ronoctua_llm::sessions::Status;
        use std::os::unix::fs::PermissionsExt as _;
        const T1: &str = "0123456789abcdef";
        let root = scratch("agents");
        let folders = Folders {
            sessions: root.join("ronoctua-agents"),
            terminals: root.join("ronoctua-terms"),
        };
        let set = Arc::new(Set {
            folders: Some(folders.clone()),
            ..Set::default()
        });
        *set.list.lock().unwrap() = Some(vec![
            account(
                "claude-1",
                Agent::Claude,
                "Work",
                Path::new("/home/p/.claude"),
            ),
            account(
                "claude-api-1",
                Agent::ClaudeApi,
                "Console",
                Path::new("/home/p/.claude-api"),
            ),
        ]);
        *set.credits.lock().unwrap() = vec![("claude-api-1".to_owned(), NOON - 600)];
        let m = module(&set, None);
        // WATCHED BY THE CORE, AND MADE PRIVATE before it watches.
        let sources = m.sources();
        for folder in [&folders.sessions, &folders.terminals] {
            assert!(
                sources
                    .iter()
                    .any(|s| matches!(s, Source::Path(p) if p == folder)),
                "{} is not watched",
                folder.display()
            );
            let mode = std::fs::metadata(folder).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{}", folder.display());
        }
        let ear_of = |wanted: Ear| m.ears.iter().position(|(e, _)| *e == wanted).unwrap();
        // ONE IN OUR TERMINAL — the hook heard it working, the program says
        // it is blocked on the person — one outside it waiting, and one of
        // the API key's done.
        record(
            &folders.sessions,
            1,
            "/home/p/.claude",
            Some(T1),
            Status::Working,
        );
        record(
            &folders.sessions,
            2,
            "/home/p/.claude",
            None,
            Status::Waiting,
        );
        record(
            &folders.sessions,
            3,
            "/home/p/.claude-api",
            None,
            Status::Done,
        );
        terminal_says(&folders.terminals, T1, "blocked");
        let (ctx, _) = Ctx::kept("agent-accounts", NOON);
        m.started(NOON, &ctx);
        let face = face_at(&m, NOON);
        let shown: Vec<&Mark> = face.marks.iter().collect();
        for (n, mark) in face.marks.iter().enumerate() {
            assert_eq!(
                super::super::MANIFEST.refusal_of(mark, &shown[..n]),
                None,
                "{mark:?}"
            );
        }
        assert_eq!(
            set_of(&face, 1),
            [
                ("claude".to_owned(), 0, Tone::Quiet, None),
                (
                    "agent-waiting".to_owned(),
                    1,
                    Tone::Warning,
                    Some(session_id(1))
                ),
                (
                    "agent-waiting".to_owned(),
                    2,
                    Tone::Warning,
                    Some(session_id(2))
                ),
            ]
        );
        assert_eq!(
            set_of(&face, 2),
            [
                ("claude-api".to_owned(), 0, Tone::Danger, None),
                ("agent-done".to_owned(), 1, Tone::Quiet, Some(session_id(3))),
            ],
            "the billing failure's danger went"
        );
        let label = face
            .marks
            .iter()
            .find(|mk| mk.id.key.as_deref() == Some(session_id(2).as_str()))
            .and_then(|mk| mk.id.label.as_deref());
        assert_eq!(label, Some("~/work-2"), "the folder is the person's");

        // THE OUTSIDE ONE DIES with no word: the liveness clock rings, and
        // the one in our terminal keeps its place.
        set.dead.lock().unwrap().push(102);
        let (later, told) = Ctx::kept("agent-accounts", NOON + LIVENESS_S);
        m.heard(Heard::Wake, &later);
        let face = face_at(&m, NOON + LIVENESS_S);
        assert_eq!(
            set_of(&face, 1)
                .iter()
                .map(|(name, place, _, _)| (name.as_str(), *place))
                .collect::<Vec<_>>(),
            [("claude", 0), ("agent-waiting", 1)]
        );

        // THE TERMINAL'S HOLDER IS GONE, its file with it: heard there, and
        // its agent with it.
        std::fs::remove_file(folders.terminals.join(format!("{T1}.json"))).unwrap();
        let before = told.changes().len();
        m.heard(
            Heard::Path {
                source: ear_of(Ear::Terminals),
                path: &folders.terminals,
            },
            &later,
        );
        assert_eq!(
            set_of(&face_at(&m, NOON), 1).len(),
            1,
            "a gone terminal's agent stayed"
        );
        assert!(
            told.changes().len() > before,
            "the face moved and nobody was told"
        );

        // A FOLDER NOT WATCHED is said, and read again when it is back.
        m.heard(
            Heard::Lost {
                source: ear_of(Ear::Sessions),
                why: "no more watches",
            },
            &later,
        );
        let rows = &sections_of(&m.kept(), NOON)[0].rows;
        assert_eq!(rows[0].id, "agents-lost");
        assert_eq!(rows[0].value, Cell::Data("no more watches".to_owned()));
        record(
            &folders.sessions,
            4,
            "/home/p/.claude",
            None,
            Status::Working,
        );
        m.heard(
            Heard::Back {
                source: ear_of(Ear::Sessions),
            },
            &later,
        );
        assert!(
            sections_of(&m.kept(), NOON)[0]
                .rows
                .iter()
                .all(|r| r.id != "agents-lost")
        );
        assert!(
            set_of(&face_at(&m, NOON), 1)
                .iter()
                .any(|(name, _, _, key)| name == "agent-working"
                    && key.as_deref() == Some(session_id(4).as_str())),
            "what was missed while the folder was down was not read"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a clock is armed for the agents when none outside our
    /// terminals is drawn — inside one of ours its holder's file says when
    /// the program goes — or not armed while one is, or counted from
    /// anything but when the agents were last read; or an agent that is not
    /// drawn (an account past the ninth, or none listed) arms it.
    #[test]
    fn the_liveness_clock_is_armed_only_while_an_agent_outside_is_drawn() {
        use super::super::Doing;
        let set = Arc::new(Set::default());
        let mut list = vec![account("claude-1", Agent::Claude, "Work", Path::new("/a"))];
        for n in 2..=10 {
            list.push(account(
                &format!("claude-{n}"),
                Agent::Claude,
                "x",
                Path::new("/x"),
            ));
        }
        *set.list.lock().unwrap() = Some(list);
        let agent = |account: &str, terminal: Option<&str>| AgentSeen {
            account: account.to_owned(),
            session: "5f0c1e2a-0000-4000-8000-000000000001".to_owned(),
            folder: "~/p".to_owned(),
            doing: Doing::Working,
            since: NOON,
            terminal: terminal.map(str::to_owned),
            pid: 4242,
        };
        let m = module(&set, None);
        let (ctx, _) = Ctx::kept("agent-accounts", NOON);
        m.started(NOON, &ctx);
        assert_eq!(m.next_wake(NOON), None, "no agent: no clock");
        *set.agents.lock().unwrap() = vec![agent("claude-1", Some("0123456789abcdef"))];
        m.inner.reread_agents(NOON);
        assert_eq!(
            m.next_wake(NOON),
            None,
            "an agent in our terminal: no clock"
        );
        for undrawn in ["claude-10", "nobody"] {
            *set.agents.lock().unwrap() = vec![agent(undrawn, None)];
            m.inner.reread_agents(NOON);
            assert_eq!(m.next_wake(NOON), None, "{undrawn}'s agent is not drawn");
        }
        *set.agents.lock().unwrap() = vec![agent("claude-1", None)];
        m.inner.reread_agents(NOON);
        assert_eq!(m.next_wake(NOON), Some(NOON + LIVENESS_S));
        // RUNG: read again, and the next one counted from then.
        let (rung, _) = Ctx::kept("agent-accounts", NOON + LIVENESS_S);
        m.heard(Heard::Wake, &rung);
        assert_eq!(m.next_wake(NOON + LIVENESS_S), Some(NOON + 2 * LIVENESS_S));
        // AND A WINDOW'S RESET, nearer, is still the one asked for.
        *set.windows.lock().unwrap() = vec![("claude-1".to_owned(), vec![window(5.0, NOON + 70)])];
        m.inner.reread_at(NOON + LIVENESS_S);
        assert_eq!(m.next_wake(NOON + LIVENESS_S), Some(NOON + 70));
        // GONE: no clock.
        set.agents.lock().unwrap().clear();
        set.windows.lock().unwrap().clear();
        m.inner.reread_agents(NOON + 80);
        assert_eq!(m.next_wake(NOON + 80), None);
    }

    /// FAILS IF: the liveness clock's ring tells anybody when every agent it
    /// asked about is alive — a mind published and the page moved every
    /// minute an agent outside our terminals is drawn — or says nothing when
    /// one went, or a window that reset since the last ring no longer moves
    /// the page.
    #[test]
    fn a_liveness_ring_that_finds_every_agent_alive_tells_nobody() {
        use super::super::Doing;
        let set = Arc::new(Set::default());
        *set.list.lock().unwrap() = Some(vec![account(
            "claude-1",
            Agent::Claude,
            "Work",
            Path::new("/a"),
        )]);
        *set.agents.lock().unwrap() = vec![AgentSeen {
            account: "claude-1".to_owned(),
            session: "5f0c1e2a-0000-4000-8000-000000000001".to_owned(),
            folder: "~/p".to_owned(),
            doing: Doing::Working,
            since: NOON,
            terminal: None,
            pid: 4242,
        }];
        // A WINDOW resetting long after the rings below.
        *set.windows.lock().unwrap() = vec![(
            "claude-1".to_owned(),
            vec![window(50.0, NOON + 10 * LIVENESS_S)],
        )];
        let m = module(&set, None);
        let (start, _) = Ctx::kept("agent-accounts", NOON);
        m.started(NOON, &start);
        let before = face_at(&m, NOON).screen;
        let (ring, told) = Ctx::kept("agent-accounts", NOON + LIVENESS_S);
        m.heard(Heard::Wake, &ring);
        assert!(told.changes().is_empty(), "an idle ring was published");
        assert_eq!(
            face_at(&m, NOON + LIVENESS_S).screen,
            before,
            "an idle ring moved the page"
        );
        // THE AGENT WENT: said.
        set.agents.lock().unwrap().clear();
        let (ring, told) = Ctx::kept("agent-accounts", NOON + 2 * LIVENESS_S);
        m.heard(Heard::Wake, &ring);
        assert!(!told.changes().is_empty(), "a gone agent was not said");
        // AND THE WINDOW'S RESET, rung for, still moves the page.
        let before = face_at(&m, NOON + 2 * LIVENESS_S).screen;
        let (ring, told) = Ctx::kept("agent-accounts", NOON + 10 * LIVENESS_S);
        m.heard(Heard::Wake, &ring);
        assert!(!told.changes().is_empty(), "the reset was not said");
        assert_ne!(face_at(&m, NOON + 10 * LIVENESS_S).screen, before);
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
        fn out_of_credits(&self, a: &Account) -> Option<Moment> {
            self.set.out_of_credits(a)
        }
        fn watched(&self, a: &Account) -> Watched {
            self.set.watched(a)
        }
        fn agents(&self, accounts: &[Account]) -> Vec<AgentSeen> {
            self.set.agents(accounts)
        }
        fn agents_folders(&self) -> Option<Folders> {
            None
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
                    ..Watched::default()
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
                    ..Watched::default()
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
                ..Watched::default()
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
