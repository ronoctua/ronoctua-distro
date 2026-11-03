//! `ronoctua-llm hook` — what Claude Code runs on a session's events, by the
//! distro's managed settings, for every account at once.
//!
//! # How Claude Code 2.1.314 runs it (read in the pinned binary)
//!
//! - **The drop-in.** On Linux the managed folder is fixed, `/etc/claude-code`
//!   (no variable moves it, so one file covers every account's
//!   `CLAUDE_CONFIG_DIR`), and every `managed-settings.d/*.json` that is a
//!   file OR A LINK is read, sorted. Its hooks are MERGED with the person's
//!   own — they survive the person's `disableAllHooks` — as long as the
//!   drop-in itself never sets `allowManagedHooksOnly`, which would silence
//!   theirs. The ownership check on a managed file is asked only when the
//!   person's organization asks for it, and then distrusts a file the user
//!   could write: a link from root's `/etc` into the read-only store passes
//!   it.
//! - **The command.** One with no `args` is spawned through `/bin/sh -c`
//!   (`child_process.spawn`, `shell: true`), detached into a session of its
//!   own, its working folder the project's. The drop-in says `exec`, so this
//!   program IS that shell, and its parent is Claude Code.
//! - **The environment** is Claude Code's own — so `CLAUDE_CONFIG_DIR` (the
//!   account), `RONOCTUA_TERMINAL` (our terminal), `RONOCTUA_INSTANCE` and
//!   `XDG_RUNTIME_DIR` arrive — plus `CLAUDE_PID`, its own pid, which is how
//!   a hook adopted by another process after Claude Code exited knows it
//!   ([`claude_of`]). A managed hook starts without the project settings'
//!   `env` and a plugin's.
//! - **The input** is the event as JSON on stdin, then a newline:
//!   `session_id`, `transcript_path`, `cwd`, `hook_event_name`, and
//!   `agent_id` only inside a subagent — whose events are not the session's
//!   and are ignored — plus each event's own (`notification_type`,
//!   `StopFailure`'s `error`, `SessionStart`'s `source`).
//! - **An async hook's output joins the conversation**: Claude Code attaches
//!   what it printed to the model's next turn, and exit code 2 is a blocking
//!   error. So this NEVER prints and ALWAYS exits 0 ([`serve`]) — a panic
//!   included — and an input it cannot read writes nothing.
//!
//! # What an event does
//!
//! [`effect_of`]: a start is ready ([`Status::Done`]); a prompt, and a denied
//! permission the model goes on past, work; a permission asked, and a
//! notification that the session needs the person, wait; the idle
//! notification and the end of an answer are done; an answer an API error
//! ended failed; the session's end is a tombstone. A TOOL's events say
//! "working" only to a session that was waiting — the approval it waited
//! for was given — since Claude Code asks the permission after `PreToolUse`,
//! and an unguarded "working" there hid every wait (Herdr's bug #198).
//!
//! Every write is the session's WHOLE record ([`crate::sessions`]), newer
//! wins under the folder's lock, and the same lock holds an API key's word
//! on its credits: a `billing_error` marks them out ([`crate::credits`]), and
//! an answer that goes through after that clears them — two sessions of one
//! account no longer lose one of the two words between a read and a write,
//! nor does one's answer from before they ran out clear the other's word.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::sessions::{self, Session, Status};
use crate::{Accounts, Kind, Roots};

/// The most an event may weigh on stdin. A tool's events carry its input and
/// its answer, and Claude Code cuts a tool's answer long before this; one
/// past it is not read, and writes nothing.
pub const MOST_INPUT: u64 = 4 * 1024 * 1024;

/// The events the managed drop-in subscribes this to — every one
/// [`effect_of`] maps, and no other (`claude-hook-is-managed` holds the
/// drop-in to this list).
pub const EVENTS: [&str; 11] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "PermissionDenied",
    "Notification",
    "Stop",
    "StopFailure",
];

/// The notifications that mean the session needs the person: a permission,
/// a dialog of an MCP server's, a question — its own or a worker's.
const WAITING_NOTICES: [&str; 5] = [
    "permission_prompt",
    "elicitation_dialog",
    "elicitation_url_dialog",
    "agent_needs_input",
    "worker_permission_prompt",
];

/// The notification of a session that sat idle at its prompt — the repair
/// of a "working" an interrupted answer never ended.
const IDLE_NOTICE: &str = "idle_prompt";

/// `StopFailure`'s error for an account whose credits ran out.
const BILLING_ERROR: &str = "billing_error";

/// `SessionStart`'s source when the session only compacted its context —
/// mid-answer, often: the session goes on as it was.
const COMPACTED: &str = "compact";

/// The variable Claude Code hands its hooks its own pid in.
const CLAUDE_PID: &str = "CLAUDE_PID";

/// The variable our terminals hand their programs their id in.
pub const TERMINAL: &str = "RONOCTUA_TERMINAL";

/// What an event says, as far as this reads it — the rest skipped unread.
/// Only the two fields every event has must be text; any other that is not
/// what this expects reads as unsaid, so a field Claude Code reshapes later
/// takes no event down with it.
#[derive(Debug, Deserialize)]
struct Input {
    session_id: String,
    hook_event_name: String,
    #[serde(default)]
    transcript_path: Option<serde_json::Value>,
    #[serde(default)]
    cwd: Option<serde_json::Value>,
    #[serde(default)]
    agent_id: Option<serde_json::Value>,
    #[serde(default)]
    notification_type: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<serde_json::Value>,
    #[serde(default)]
    source: Option<serde_json::Value>,
}

/// A field's text, when it is text.
fn text(field: Option<&serde_json::Value>) -> Option<&str> {
    field.and_then(serde_json::Value::as_str)
}

/// What an event does to its session's record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effect {
    /// Nothing is said.
    Nothing,
    /// The session starts — ready — or, when it only compacted, goes on as
    /// it was.
    Start {
        /// Whether it only compacted its context.
        compacted: bool,
    },
    /// Its status is now this.
    Set(Status),
    /// It works again, if it was waiting: a tool's event after an approval.
    Resume,
    /// It ended.
    End,
}

/// What an event says of an API key's credits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Credits {
    /// Nothing.
    Unsaid,
    /// They ran out (`billing_error`).
    Out,
    /// An answer went through.
    Fine,
}

/// What an event does: to the record, and to an API key's credits.
pub(crate) fn effect_of(
    event: &str,
    notice: Option<&str>,
    error: Option<&str>,
    source: Option<&str>,
) -> (Effect, Credits) {
    match event {
        "SessionStart" => (
            Effect::Start {
                compacted: source == Some(COMPACTED),
            },
            Credits::Unsaid,
        ),
        "SessionEnd" => (Effect::End, Credits::Unsaid),
        "UserPromptSubmit" | "PermissionDenied" => (Effect::Set(Status::Working), Credits::Unsaid),
        "PreToolUse" | "PostToolUse" | "PostToolUseFailure" => (Effect::Resume, Credits::Unsaid),
        "PermissionRequest" => (Effect::Set(Status::Waiting), Credits::Unsaid),
        "Notification" => match notice {
            Some(n) if WAITING_NOTICES.contains(&n) => {
                (Effect::Set(Status::Waiting), Credits::Unsaid)
            }
            Some(IDLE_NOTICE) => (Effect::Set(Status::Done), Credits::Unsaid),
            _ => (Effect::Nothing, Credits::Unsaid),
        },
        "Stop" => (Effect::Set(Status::Done), Credits::Fine),
        "StopFailure" => (
            Effect::Set(Status::Failed),
            if error == Some(BILLING_ERROR) {
                Credits::Out
            } else {
                Credits::Unsaid
            },
        ),
        _ => (Effect::Nothing, Credits::Unsaid),
    }
}

/// Who and where a session runs, besides its status — what this run knows
/// of it now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    /// The account's folder it runs in.
    pub account_dir: PathBuf,
    /// Claude Code's process.
    pub pid: u32,
    /// When the kernel started it.
    pub pid_start_ticks: u64,
    /// Our terminal it runs in, when it is one.
    pub terminal: Option<String>,
    /// The folder it works in.
    pub folder: PathBuf,
}

/// The record after an event, or `None` when nothing is written: an older
/// word than the record's, an event that says nothing, a session that ended
/// (only a new start revives it), a tool's event on a session that was not
/// waiting, or a live word whose Claude Code is gone (`who` is `None`).
pub(crate) fn next(
    before: Option<&Session>,
    effect: Effect,
    session: &str,
    who: Option<&Identity>,
    transcript: Option<&Path>,
    said_at_ns: u64,
) -> Option<Session> {
    if before.is_some_and(|b| b.said_at_ns > said_at_ns) {
        return None;
    }
    let live = before.filter(|b| !b.ended);
    let ended = before.is_some_and(|b| b.ended);
    let status = match effect {
        Effect::Nothing => return None,
        Effect::Start { compacted } => live
            .filter(|_| compacted)
            .map_or(Status::Done, |b| b.status),
        Effect::Set(_) | Effect::Resume if ended => return None,
        Effect::Set(status) => status,
        Effect::Resume => match live?.status {
            Status::Waiting => Status::Working,
            _ => return None,
        },
        Effect::End => before.map_or(Status::Done, |b| b.status),
    };
    let is_end = effect == Effect::End;
    // WHO, from this run when Claude Code is there to ask; a tombstone may
    // keep what the record said, and nothing else may be written blind.
    let (account_dir, pid, pid_start_ticks, terminal, folder) = match (who, before) {
        (Some(w), _) => (
            w.account_dir.clone(),
            w.pid,
            w.pid_start_ticks,
            w.terminal.clone(),
            w.folder.clone(),
        ),
        (None, Some(b)) if is_end => (
            b.account_dir.clone(),
            b.pid,
            b.pid_start_ticks,
            b.terminal.clone(),
            b.folder.clone(),
        ),
        (None, _) => return None,
    };
    // THE PLACE IS KEPT while it is the same process: an agent keeps its
    // place among the others, and a new process of the session starts anew.
    let started_at_ns = live
        .filter(|b| b.pid == pid && b.pid_start_ticks == pid_start_ticks)
        .map_or(said_at_ns, |b| b.started_at_ns);
    Some(Session {
        format: sessions::FORMAT,
        session: session.to_owned(),
        account_dir,
        pid,
        pid_start_ticks,
        terminal,
        folder,
        transcript: transcript
            .map(Path::to_path_buf)
            .or_else(|| before.and_then(|b| b.transcript.clone())),
        status,
        said_at_ns,
        started_at_ns,
        ended: is_end,
    })
}

/// Claude Code's process, as a hook run finds it: the pid it says
/// ([`CLAUDE_PID`]) when that is this run's parent or above it, and the
/// parent itself from a Claude Code that does not say — with the moment the
/// kernel started it. `None` when it is gone: a hook still running after
/// Claude Code exited is adopted, its parent is then whoever adopted it, and
/// a record naming that process would be an agent that never ends.
pub(crate) fn claude_of(
    said: Option<u32>,
    parent: u32,
    above: &dyn Fn(u32) -> Vec<u32>,
    started: &dyn Fn(u32) -> Option<u64>,
) -> Option<(u32, u64)> {
    let pid = match said {
        Some(pid) if pid > 1 && (pid == parent || above(parent).contains(&pid)) => pid,
        Some(_) => return None,
        None if parent > 1 => parent,
        None => return None,
    };
    Some((pid, started(pid)?))
}

/// What a run knows of where it is: carried, so a test plants its own.
#[derive(Debug, Clone, Default)]
pub(crate) struct Here {
    /// The sessions' folder ([`sessions::folder`]); `None` without a
    /// runtime folder, and then nothing is kept.
    pub folder: Option<PathBuf>,
    /// The roots: the accounts' list, and the state an API key's credits
    /// are kept in.
    pub roots: Roots,
    /// The account's folder Claude Code runs in.
    pub account_dir: Option<PathBuf>,
    /// Claude Code's process and its start ([`claude_of`]); `None` when it
    /// is gone.
    pub claude: Option<(u32, u64)>,
    /// Our terminal it runs in, when it is one ([`TERMINAL`]).
    pub terminal: Option<String>,
    /// The folder this runs in — Claude Code's project — for an event that
    /// does not say its own.
    pub cwd: Option<PathBuf>,
    /// When this run started, in nanoseconds since the epoch.
    pub said_at_ns: u64,
}

impl Here {
    /// The machine's, from this process's environment and `/proc`.
    pub(crate) fn of_env(said_at_ns: u64) -> Self {
        let roots = Roots::of_env();
        let account_dir =
            crate::statusline::agent_dir_of(&|name| std::env::var_os(name), roots.home.as_deref());
        let said = std::env::var(CLAUDE_PID).ok().and_then(|p| p.parse().ok());
        let claude = claude_of(
            said,
            std::os::unix::process::parent_id(),
            &ronoctua_memory::lineage::ancestry,
            &ronoctua_memory::lineage::start_ticks,
        );
        Self {
            folder: sessions::folder(),
            roots,
            account_dir,
            claude,
            terminal: std::env::var(TERMINAL)
                .ok()
                .filter(|t| sessions::fine_terminal(t)),
            cwd: std::env::current_dir().ok(),
            said_at_ns,
        }
    }
}

/// The hook's run, as the CLI makes it: stamped first, stdin read bounded,
/// a panic swallowed silently — and 0, whatever happened.
#[must_use]
pub fn serve() -> i32 {
    let said_at_ns = crate::time::now_ns();
    // SILENT: what a panic would print reaches the conversation.
    std::panic::set_hook(Box::new(|_| {}));
    guarded(move || {
        let mut input = Vec::new();
        if std::io::stdin()
            .lock()
            .take(MOST_INPUT + 1)
            .read_to_end(&mut input)
            .is_ok()
        {
            run(&input, &Here::of_env(said_at_ns));
        }
    })
}

/// `work`, and 0 — also when it panics.
fn guarded(work: impl FnOnce() + std::panic::UnwindSafe) -> i32 {
    let _ = std::panic::catch_unwind(work);
    0
}

/// One event, kept — or nothing, for anything this cannot vouch for. Never
/// prints, never fails.
pub(crate) fn run(input: &[u8], here: &Here) {
    if input.len() as u64 > MOST_INPUT {
        return;
    }
    let Ok(event) = serde_json::from_slice::<Input>(input) else {
        return;
    };
    // A SUBAGENT'S events are its own, not the session's.
    if event.agent_id.is_some() || !sessions::fine_session(&event.session_id) {
        return;
    }
    let (effect, credits) = effect_of(
        &event.hook_event_name,
        text(event.notification_type.as_ref()),
        text(event.error.as_ref()),
        text(event.source.as_ref()),
    );
    if effect == Effect::Nothing {
        return;
    }
    let Some(folder) = &here.folder else {
        return;
    };
    if sessions::make_folder(folder).is_err() {
        return;
    }
    let Ok(held) = sessions::Lock::take(folder) else {
        return;
    };
    let before = sessions::read_one(folder, &event.session_id);
    let who = identity(here, text(event.cwd.as_ref()).map(Path::new));
    let Some(after) = next(
        before.as_ref(),
        effect,
        &event.session_id,
        who.as_ref(),
        text(event.transcript_path.as_ref()).map(Path::new),
        here.said_at_ns,
    ) else {
        return;
    };
    if sessions::write(folder, &after).is_err() {
        return;
    }
    if matches!(effect, Effect::Start { .. }) {
        sessions::sweep(
            folder,
            &held,
            &after.session,
            here.said_at_ns,
            &sessions::running,
        );
    }
    tell_credits(
        &held,
        &here.roots,
        &after.account_dir,
        credits,
        here.said_at_ns,
    );
}

/// What this run knows of who the session is: nothing without Claude Code,
/// an account's folder or a folder to work in.
fn identity(here: &Here, cwd: Option<&Path>) -> Option<Identity> {
    let (pid, pid_start_ticks) = here.claude?;
    Some(Identity {
        account_dir: here.account_dir.clone()?,
        pid,
        pid_start_ticks,
        terminal: here.terminal.clone(),
        folder: cwd
            .filter(|c| c.is_absolute())
            .map(Path::to_path_buf)
            .or_else(|| here.cwd.clone())?,
    })
}

/// An API key's account's word on its credits, under the sessions' lock —
/// for the listed API key's account whose folder the session runs in, and
/// nobody else (a subscription has windows instead).
fn tell_credits(
    _held: &sessions::Lock,
    roots: &Roots,
    account_dir: &Path,
    credits: Credits,
    said_at_ns: u64,
) {
    if credits == Credits::Unsaid {
        return;
    }
    let (Some(config), Some(state)) = (&roots.config, &roots.state) else {
        return;
    };
    let accounts = Accounts::store_in(Some(config.clone())).read();
    let Some(account) = accounts
        .accounts
        .iter()
        .find(|a| a.kind == Kind::ClaudeApi && sessions::same_folder(&a.dir, account_dir))
    else {
        return;
    };
    let now_s = i64::try_from(said_at_ns / 1_000_000_000).unwrap_or(i64::MAX);
    let _ = match credits {
        Credits::Out => crate::credits::mark_out_in(state, account, now_s),
        // AN ANSWER THAT WENT THROUGH BEFORE THE CREDITS RAN OUT — another
        // session's, its run late to the lock — says nothing of them now; one
        // within the same second neither, since hiding "out of credits" from
        // the person costs more than the next answer's clearing it.
        Credits::Fine => match crate::credits::out_since_in(state, account) {
            Ok(Some(since)) if now_s <= since => Ok(()),
            _ => crate::credits::clear_in(state, account),
        },
        Credits::Unsaid => Ok(()),
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::tests::scratch;

    const SESSION: &str = "5f0c1e2a-0b1c-4d2e-8f3a-9b8c7d6e5f40";

    fn event(name: &str, extra: &str) -> Vec<u8> {
        event_of(SESSION, name, extra)
    }

    /// An event as Claude Code writes one, of `session`.
    fn event_of(session: &str, name: &str, extra: &str) -> Vec<u8> {
        format!(
            r#"{{"session_id":"{session}","transcript_path":"/home/p/.claude/projects/-w/{session}.jsonl","cwd":"/home/p/work","hook_event_name":"{name}"{extra}}}"#
        )
        .into_bytes()
    }

    /// A run in `root`: the folder, the roots, Claude Code alive as this
    /// test's own process.
    fn here(root: &Path, said_at_ns: u64) -> Here {
        let me = std::process::id();
        // THE RUNTIME FOLDER is the session's, there before any hook runs.
        std::fs::create_dir_all(root.join("runtime")).unwrap();
        Here {
            folder: Some(root.join("runtime").join("ronoctua-agents")),
            roots: Roots {
                config: Some(root.join("config")),
                state: Some(root.join("state")),
                cache: None,
                home: Some(root.join("home")),
            },
            account_dir: Some(root.join("home").join(".claude-api")),
            claude: Some((me, ronoctua_memory::lineage::start_ticks(me).unwrap())),
            terminal: Some("0123456789abcdef".to_owned()),
            cwd: Some(root.join("elsewhere")),
            said_at_ns,
        }
    }

    fn kept(here: &Here) -> Option<Session> {
        sessions::read_one(here.folder.as_deref()?, SESSION)
    }

    /// FAILS IF: an event is mapped to anything but what the hook promises —
    /// a tool's event that says "working" unguarded, a notification that
    /// does not need the person read as a wait, a compaction read as a new
    /// start, a failure other than billing taken for credits run out — or an
    /// event the drop-in subscribes is one this does not map.
    /// An event, its notification, error and source — and what it does.
    type Row = (
        &'static str,
        Option<&'static str>,
        Option<&'static str>,
        Option<&'static str>,
        Effect,
        Credits,
    );

    #[test]
    fn every_event_says_what_the_hook_maps_it_to() {
        use Credits::{Fine, Out, Unsaid};
        use Effect::{End, Nothing, Resume, Set, Start};
        let table: [Row; 19] = [
            (
                "SessionStart",
                None,
                None,
                Some("startup"),
                Start { compacted: false },
                Unsaid,
            ),
            (
                "SessionStart",
                None,
                None,
                Some("resume"),
                Start { compacted: false },
                Unsaid,
            ),
            (
                "SessionStart",
                None,
                None,
                Some("compact"),
                Start { compacted: true },
                Unsaid,
            ),
            ("SessionEnd", None, None, None, End, Unsaid),
            (
                "UserPromptSubmit",
                None,
                None,
                None,
                Set(Status::Working),
                Unsaid,
            ),
            (
                "PermissionDenied",
                None,
                None,
                None,
                Set(Status::Working),
                Unsaid,
            ),
            ("PreToolUse", None, None, None, Resume, Unsaid),
            ("PostToolUse", None, None, None, Resume, Unsaid),
            ("PostToolUseFailure", None, None, None, Resume, Unsaid),
            (
                "PermissionRequest",
                None,
                None,
                None,
                Set(Status::Waiting),
                Unsaid,
            ),
            (
                "Notification",
                Some("permission_prompt"),
                None,
                None,
                Set(Status::Waiting),
                Unsaid,
            ),
            (
                "Notification",
                Some("elicitation_url_dialog"),
                None,
                None,
                Set(Status::Waiting),
                Unsaid,
            ),
            (
                "Notification",
                Some("worker_permission_prompt"),
                None,
                None,
                Set(Status::Waiting),
                Unsaid,
            ),
            (
                "Notification",
                Some("idle_prompt"),
                None,
                None,
                Set(Status::Done),
                Unsaid,
            ),
            (
                "Notification",
                Some("auth_success"),
                None,
                None,
                Nothing,
                Unsaid,
            ),
            ("Stop", None, None, None, Set(Status::Done), Fine),
            (
                "StopFailure",
                None,
                Some("billing_error"),
                None,
                Set(Status::Failed),
                Out,
            ),
            (
                "StopFailure",
                None,
                Some("rate_limit"),
                None,
                Set(Status::Failed),
                Unsaid,
            ),
            ("SubagentStop", None, None, None, Nothing, Unsaid),
        ];
        for (event, notice, error, source, effect, credits) in table {
            assert_eq!(
                effect_of(event, notice, error, source),
                (effect, credits),
                "{event} {notice:?} {error:?} {source:?}"
            );
        }
        for subscribed in EVENTS {
            let mapped = effect_of(subscribed, Some("permission_prompt"), None, None).0;
            assert_ne!(
                mapped, Nothing,
                "{subscribed} is subscribed and maps to nothing"
            );
        }
    }

    fn who(pid: u32) -> Identity {
        Identity {
            account_dir: "/home/p/.claude".into(),
            pid,
            pid_start_ticks: 7,
            terminal: None,
            folder: "/home/p/work".into(),
        }
    }

    fn at(status: Status, said_at_ns: u64) -> Session {
        Session {
            status,
            said_at_ns,
            started_at_ns: 5,
            pid: 9,
            pid_start_ticks: 7,
            ..crate::sessions::tests::record(SESSION)
        }
    }

    /// FAILS IF: a tool's event says "working" of a session that was not
    /// waiting — every wait would be hidden the moment the tool was asked —
    /// or does not resume one that was.
    #[test]
    fn a_tool_event_resumes_only_a_wait() {
        let w = who(9);
        let step =
            |before: Option<&Session>| next(before, Effect::Resume, SESSION, Some(&w), None, 20);
        assert_eq!(
            step(Some(&at(Status::Waiting, 10))).map(|s| s.status),
            Some(Status::Working)
        );
        for not in [Status::Working, Status::Done, Status::Failed] {
            assert_eq!(step(Some(&at(not, 10))), None, "{not:?}");
        }
        assert_eq!(step(None), None, "no record: nothing to resume");
    }

    /// FAILS IF: a word older than the record's replaces it (two runs ended
    /// in the other order), an event after a session's end revives it — or a
    /// new start of it does not.
    #[test]
    fn an_older_word_never_overwrites_a_newer_one() {
        let w = who(9);
        let newer = at(Status::Waiting, 30);
        assert_eq!(
            next(
                Some(&newer),
                Effect::Set(Status::Done),
                SESSION,
                Some(&w),
                None,
                20
            ),
            None
        );
        let ended = Session {
            ended: true,
            ..newer
        };
        for effect in [Effect::Set(Status::Working), Effect::Resume] {
            assert_eq!(
                next(Some(&ended), effect, SESSION, Some(&w), None, 40),
                None
            );
        }
        let revived = next(
            Some(&ended),
            Effect::Start { compacted: false },
            SESSION,
            Some(&w),
            None,
            40,
        )
        .unwrap();
        assert!(!revived.ended);
        assert_eq!((revived.status, revived.started_at_ns), (Status::Done, 40));
    }

    /// FAILS IF: a compaction in the middle of an answer reads as a fresh,
    /// idle session, a session's place among the others moves while its
    /// process is the same, or a new process keeps the old one's place.
    #[test]
    fn a_compaction_goes_on_and_a_place_is_kept_by_its_process() {
        let w = who(9);
        let working = at(Status::Working, 10);
        let compacted = next(
            Some(&working),
            Effect::Start { compacted: true },
            SESSION,
            Some(&w),
            None,
            20,
        )
        .unwrap();
        assert_eq!(
            (compacted.status, compacted.started_at_ns),
            (Status::Working, 5)
        );
        let other = who(10);
        let restarted = next(
            Some(&working),
            Effect::Set(Status::Working),
            SESSION,
            Some(&other),
            None,
            20,
        )
        .unwrap();
        assert_eq!(restarted.started_at_ns, 20, "a new process starts anew");
    }

    /// FAILS IF: a live word is written for a Claude Code that is gone (the
    /// record would name whoever adopted the hook, and never end), or the
    /// end of one is not written from what the record knew.
    #[test]
    fn a_gone_claude_writes_only_its_end() {
        let working = at(Status::Working, 10);
        assert_eq!(
            next(
                Some(&working),
                Effect::Set(Status::Done),
                SESSION,
                None,
                None,
                20
            ),
            None
        );
        let end = next(Some(&working), Effect::End, SESSION, None, None, 20).unwrap();
        assert!(end.ended);
        assert_eq!((end.pid, end.status), (working.pid, Status::Working));
        assert_eq!(next(None, Effect::End, SESSION, None, None, 20), None);
    }

    /// FAILS IF: Claude Code's process is taken from anything but the pid it
    /// says when that is above the hook, or the hook's parent without it — a
    /// hook adopted after Claude Code exited would name its adopter.
    #[test]
    fn claude_is_the_process_above_the_hook() {
        let above = |pid: u32| if pid == 50 { vec![40, 30] } else { Vec::new() };
        let started = |pid: u32| (pid != 99).then_some(u64::from(pid) * 10);
        assert_eq!(claude_of(Some(50), 50, &above, &started), Some((50, 500)));
        assert_eq!(
            claude_of(Some(40), 50, &above, &started),
            Some((40, 400)),
            "a shell between"
        );
        assert_eq!(claude_of(Some(40), 77, &above, &started), None, "adopted");
        assert_eq!(claude_of(None, 50, &above, &started), Some((50, 500)));
        assert_eq!(
            claude_of(None, 1, &above, &started),
            None,
            "adopted by init"
        );
        assert_eq!(claude_of(Some(99), 99, &above, &started), None, "gone");
    }

    /// FAILS IF: a session's life on disk is not what each event says — the
    /// record whole, `0600` in a folder `0700`, our terminal and the
    /// session's own folder in it — or the end does not leave a tombstone.
    #[test]
    fn a_session_is_kept_whole_event_by_event() {
        let root = scratch("hook-life");
        let mut said = 100;
        let mut step = |name: &str, extra: &str| {
            said += 1;
            let h = here(&root, said);
            run(&event(name, extra), &h);
            kept(&h)
        };
        let started = step("SessionStart", r#","source":"startup""#).unwrap();
        assert_eq!(started.status, Status::Done);
        assert_eq!(started.terminal.as_deref(), Some("0123456789abcdef"));
        assert_eq!(
            started.folder,
            PathBuf::from("/home/p/work"),
            "the event's own folder"
        );
        assert_eq!(started.account_dir, root.join("home").join(".claude-api"));
        let steps = [
            ("UserPromptSubmit", "", Status::Working),
            ("PreToolUse", "", Status::Working),
            ("PermissionRequest", "", Status::Waiting),
            ("PostToolUse", "", Status::Working),
            (
                "Notification",
                r#","notification_type":"agent_needs_input""#,
                Status::Waiting,
            ),
            (
                "Notification",
                r#","notification_type":"idle_prompt""#,
                Status::Done,
            ),
            ("StopFailure", r#","error":"overloaded""#, Status::Failed),
            ("Stop", "", Status::Done),
        ];
        for (name, extra, status) in steps {
            let after = step(name, extra).unwrap();
            assert_eq!(after.status, status, "{name} {extra}");
            assert_eq!(
                after.started_at_ns, started.started_at_ns,
                "{name}: the same process"
            );
        }
        let end = step("SessionEnd", r#","reason":"prompt_input_exit""#).unwrap();
        assert!(end.ended && end.status == Status::Done);
        let folder = here(&root, 0).folder.unwrap();
        let mode = |p: &Path| {
            std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(p).unwrap().permissions())
                & 0o777
        };
        assert_eq!(mode(&folder), 0o700);
        assert_eq!(mode(&folder.join(format!("{SESSION}.json"))), 0o600);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: anything is written for a subagent's event, a session id
    /// that would be a path, an input that is not an event (empty, garbage,
    /// past the bound), or a machine with no runtime folder — or a run
    /// fails for it.
    #[test]
    fn what_is_not_a_sessions_event_writes_nothing() {
        let root = scratch("hook-nothing");
        let h = here(&root, 1);
        let folder = h.folder.clone().unwrap();
        let huge = event(
            "UserPromptSubmit",
            &format!(r#","prompt":"{}""#, "x".repeat(MOST_INPUT as usize)),
        );
        for input in [
            event(
                "UserPromptSubmit",
                r#","agent_id":"a1","agent_type":"general-purpose""#,
            ),
            event_of("../../escape", "UserPromptSubmit", ""),
            event_of("5F0C1E2A", "UserPromptSubmit", ""),
            Vec::new(),
            b"\x00\xffgarbage".to_vec(),
            b"{\"hook_event_name\":\"Stop\"}".to_vec(),
            huge,
        ] {
            run(&input, &h);
            assert!(
                !folder.exists(),
                "written for {:?}",
                String::from_utf8_lossy(&input[..input.len().min(80)])
            );
        }
        run(&event("UserPromptSubmit", ""), &Here { folder: None, ..h });
        assert!(!folder.exists());
        assert_eq!(guarded(|| panic!("a run that panics")), 0);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a field this reads loosely, reshaped by a later Claude
    /// Code, drops the whole event — or the two every event has are not
    /// required to be text.
    #[test]
    fn a_reshaped_field_takes_no_event_down() {
        let root = scratch("hook-reshaped");
        let h = here(&root, 9);
        run(
            &format!(
                r#"{{"session_id":"{SESSION}","cwd":42,"transcript_path":null,"hook_event_name":"StopFailure","error":{{"kind":"billing_error"}}}}"#
            )
            .into_bytes(),
            &h,
        );
        let failed = kept(&h).unwrap();
        assert_eq!((failed.status, failed.transcript), (Status::Failed, None));
        assert_eq!(
            failed.folder,
            root.join("elsewhere"),
            "the folder it runs in"
        );
        let later = Here {
            said_at_ns: 10,
            ..h
        };
        run(br#"{"session_id":7,"hook_event_name":"Stop"}"#, &later);
        run(
            format!(r#"{{"session_id":"{SESSION}","hook_event_name":["Stop"]}}"#).as_bytes(),
            &later,
        );
        assert_eq!(kept(&later).unwrap().status, Status::Failed);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a session's start does not sweep the settled tombstones and
    /// the records of processes that are gone, or sweeps a living one.
    #[test]
    fn a_start_sweeps_the_ended_and_the_dead() {
        let root = scratch("hook-sweep");
        let h = here(&root, 50 + sessions::TOMBSTONE_KEPT_NS);
        let folder = h.folder.clone().unwrap();
        sessions::make_folder(&folder).unwrap();
        let (me, ticks) = h.claude.unwrap();
        let living = Session {
            pid: me,
            pid_start_ticks: ticks,
            ..crate::sessions::tests::record("aa")
        };
        for r in [
            living.clone(),
            Session {
                session: "bb".to_owned(),
                ended: true,
                ..living.clone()
            },
            Session {
                session: "cc".to_owned(),
                pid_start_ticks: ticks + 1,
                ..living
            },
        ] {
            sessions::write(&folder, &r).unwrap();
        }
        run(&event("SessionStart", r#","source":"startup""#), &h);
        let left: Vec<String> = sessions::read(&folder)
            .into_iter()
            .map(|s| s.session)
            .collect();
        assert_eq!(left, vec![SESSION.to_owned(), "aa".to_owned()]);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a `/clear` — the session's end and the next one's start in
    /// the same process, at once — sweeps the tombstone a run of the ended
    /// session still waiting for the lock meets: that run would write the
    /// ended session back as a live agent of a process that lives on.
    #[test]
    fn a_late_word_after_a_clear_revives_nothing() {
        const NEXT: &str = "0d6b8e1c-4a2f-4c3d-9e8f-7a6b5c4d3e2f";
        let root = scratch("hook-clear");
        run(&event("UserPromptSubmit", ""), &here(&root, 100));
        run(
            &event("SessionEnd", r#","reason":"clear""#),
            &here(&root, 300),
        );
        run(
            &event_of(NEXT, "SessionStart", r#","source":"clear""#),
            &here(&root, 400),
        );
        // THE ENDED SESSION'S STOP, stamped before its end, last to the lock.
        run(&event("Stop", ""), &here(&root, 200));
        let h = here(&root, 0);
        assert!(kept(&h).is_some_and(|s| s.ended), "{:?}", kept(&h));
        let folder = h.folder.unwrap();
        assert_eq!(
            sessions::read_one(&folder, NEXT).map(|s| (s.status, s.ended)),
            Some((Status::Done, false))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn api_list(h: &Here) {
        let config = h.roots.config.clone().unwrap();
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join(Accounts::FILE),
            format!(
                r#"{{"format":2,"accounts":[{{"id":"claude-api-1","kind":"claude-api","name":"Console","dir":"{}/","default":true}}]}}"#,
                h.account_dir.as_ref().unwrap().display()
            ),
        )
        .unwrap();
    }

    fn api_account(h: &Here) -> crate::Account {
        Accounts::store_in(h.roots.config.clone())
            .read()
            .accounts
            .remove(0)
    }

    /// FAILS IF: a `billing_error` does not mark an API key's account out
    /// of credits, an answer that goes through does not clear it, a word
    /// older than the session's newest moves them, an answer of another
    /// session of the account that went through BEFORE they ran out (its run
    /// last to the lock) clears them, or a subscription's account is said to
    /// have credits at all.
    #[test]
    fn a_billing_failure_marks_an_api_key_and_an_answer_clears_it() {
        let root = scratch("hook-credits");
        let h = here(&root, 5_000_000_000);
        api_list(&h);
        let account = api_account(&h);
        let state = h.roots.state.clone().unwrap();
        run(&event("StopFailure", r#","error":"billing_error""#), &h);
        assert_eq!(crate::credits::out_since_in(&state, &account), Ok(Some(5)));
        // AN OLDER STOP of the same session, finishing last, says nothing.
        run(
            &event("Stop", ""),
            &Here {
                said_at_ns: 4_000_000_000,
                ..h.clone()
            },
        );
        assert_eq!(crate::credits::out_since_in(&state, &account), Ok(Some(5)));
        // NOR ANOTHER SESSION'S, of the same account: its answer went
        // through at 4 s, or in the second they ran out.
        for said_at_ns in [4_000_000_000, 5_900_000_000] {
            run(
                &event_of("0d6b8e1c-4a2f-4c3d-9e8f-7a6b5c4d3e2f", "Stop", ""),
                &Here {
                    said_at_ns,
                    ..h.clone()
                },
            );
            assert_eq!(
                crate::credits::out_since_in(&state, &account),
                Ok(Some(5)),
                "{said_at_ns}"
            );
        }
        run(
            &event("Stop", ""),
            &Here {
                said_at_ns: 6_000_000_000,
                ..h.clone()
            },
        );
        assert_eq!(crate::credits::out_since_in(&state, &account), Ok(None));
        // A SUBSCRIPTION'S folder: no account of credits is touched.
        let sub = Here {
            account_dir: Some(root.join("home").join(".claude")),
            said_at_ns: 7_000_000_000,
            ..h
        };
        run(&event("StopFailure", r#","error":"billing_error""#), &sub);
        assert_eq!(crate::credits::out_since_in(&state, &account), Ok(None));
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: two runs telling one account's credits at once can lose a
    /// word between a read and a write — the race the lock closes. Every
    /// word is told under the lock, with what the file said just before and
    /// just after it, and recorded under it: each after must be what the
    /// word makes of its before, and each before the after of the word told
    /// just ahead of it.
    #[test]
    fn the_credits_race_is_serialised() {
        let root = scratch("hook-race");
        let h = here(&root, 1);
        api_list(&h);
        let account = api_account(&h);
        let state = h.roots.state.clone().unwrap();
        let folder = h.folder.clone().unwrap();
        sessions::make_folder(&folder).unwrap();
        let told = std::sync::Mutex::new(Vec::new());
        std::thread::scope(|s| {
            for t in 0..8_u64 {
                let (h, told, folder, state, account) = (&h, &told, &folder, &state, &account);
                s.spawn(move || {
                    for round in 0..25_u64 {
                        let word = if (t + round) % 3 == 0 {
                            Credits::Fine
                        } else {
                            Credits::Out
                        };
                        let at_s = 1 + t * 100 + round;
                        let held = sessions::Lock::take(folder).unwrap();
                        let before = crate::credits::out_since_in(state, account).unwrap();
                        let dir = h.account_dir.as_ref().unwrap();
                        tell_credits(&held, &h.roots, dir, word, at_s * 1_000_000_000);
                        let after = crate::credits::out_since_in(state, account).unwrap();
                        told.lock().unwrap().push((word, at_s, before, after));
                    }
                });
            }
        });
        let told = told.into_inner().unwrap();
        assert_eq!(told.len(), 200);
        let mut now: Option<i64> = None;
        for (word, at_s, before, after) in told {
            assert_eq!(before, now, "a word was told over another's");
            let at_s = i64::try_from(at_s).unwrap();
            let made = match word {
                Credits::Out => Some(before.unwrap_or(at_s)),
                Credits::Fine if before.is_some_and(|since| at_s <= since) => before,
                Credits::Fine | Credits::Unsaid => None,
            };
            assert_eq!(after, made, "{word:?} at {at_s} over {before:?}");
            now = after;
        }
        assert_eq!(crate::credits::out_since_in(&state, &account), Ok(now));
        let _ = std::fs::remove_dir_all(root);
    }
}
