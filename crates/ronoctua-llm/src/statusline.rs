//! The status line: ours, in front of the person's.
//!
//! # Why a status line at all
//!
//! Claude Code hands its status line command, on stdin, the account's
//! `rate_limits` — the windows the plan has, in per cent, with the instant
//! each resets: five hours and a week, or the week alone (Claude's Ultra,
//! 2026-11-02) — after every answer. It is the official way to know
//! them, it costs nothing, and it is fresh exactly when it matters: while the
//! person is working.
//!
//! # Wrapped, never replaced
//!
//! [`wrap`] edits ONE field of `<dir>/settings.json`, in the text
//! (`jsontext`), and saves the value the person had beside it
//! (`ronoctua-statusline.json`, the exact text it was). Our command,
//! `/run/current-system/sw/bin/ronoctua-llm statusline --profile <id>`
//! — the system's PATH and not a store path, because a `settings.json`
//! outlives a generation — writes the limits and then CHAINS: it runs the
//! person's command with the same stdin, and its stdout and exit code are
//! ours ([`run`]). With nothing to chain it prints nothing. [`unwrap`] puts
//! the person's value back, byte for byte — and removing the account does it
//! too ([`crate::Accounts::keep`]). A line whose account the list no longer
//! names is still chained: the saved line is found in the folder Claude Code
//! runs in ([`Chain::agent_dir`]), so the person's line never goes blank
//! behind a name nobody lists.
//!
//! The person may change `statusLine` afterwards; [`wrapped`] says so
//! ([`Wrapping::Replaced`]) and the view offers to wrap again — it is never
//! done behind their back, and [`unwrap`] then leaves their newer choice
//! alone.
//!
//! # Never a loop
//!
//! Our own command is never saved as the one to chain (a wrap over ours
//! keeps what was saved before), a saved command that is ours is never run,
//! and the chained child carries `RONOCTUA_LLM_CHAINED`, which stops a
//! person's script that calls us from calling itself through us.
//!
//! # And Codex, and an API key
//!
//! Codex has no status line: it writes its `rate_limits` into its own
//! session logs, and [`limits`] reads them there. Claude Code on an API key
//! has one, and sends it `rate_limits` null — a key has no windows — so
//! nothing of ours is put in front of it: it would edit the person's
//! `settings.json` to keep nothing.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{Account, Kind, Refusal, Why, Window};

/// Where our command is, as `settings.json` names it.
pub const PROGRAM: &str = "/run/current-system/sw/bin/ronoctua-llm";

/// Claude Code's settings file, in the account's folder.
pub const SETTINGS_FILE: &str = "settings.json";

/// The person's own status line, saved beside it while ours is in front.
pub const SAVED_FILE: &str = "ronoctua-statusline.json";

/// The field of `settings.json` that is edited, and the only one.
const FIELD: &str = "statusLine";

/// Set on the chained child: a person's script that runs us is not chained
/// again.
pub const CHAINED: &str = "RONOCTUA_LLM_CHAINED";

/// The most a settings file, or a status line's stdin, may weigh.
const MOST_BYTES: u64 = 1024 * 1024;

/// What the status line last said of the limits.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Limits {
    /// The five-hour window.
    pub five_hour: Option<Window>,
    /// The weekly window.
    pub seven_day: Option<Window>,
    /// The agent's own word that the plan HAS no five-hour window: Claude's
    /// Ultra has the week alone, and its status line then carries
    /// `rate_limits` without `five_hour`. A window that is merely not known
    /// yet leaves this false, so its row still says "not known yet".
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub no_five_hour: bool,
}

impl Limits {
    /// The same, without a window whose reset has passed: it started over,
    /// and how much of the new one is used is not known yet.
    #[must_use]
    pub fn at(self, now_s: i64) -> Self {
        let running = |w: Option<Window>| w.filter(|w| w.resets_at_s > now_s);
        Self {
            five_hour: running(self.five_hour),
            seven_day: running(self.seven_day),
            // What the plan has does not reset with a window.
            no_five_hour: self.no_five_hour,
        }
    }
}

/// Whether our status line is in front of an account's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Wrapping {
    /// Ours is there, for this account.
    Ours,
    /// We wrapped it, and the person (or a program) put another one since —
    /// or ours for another account. Offered again; never redone silently.
    Replaced,
    /// Not wrapped.
    Not,
}

/// The person's status line, saved while ours is in front.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Saved {
    /// Always 1.
    format: u32,
    /// The account it was saved for.
    profile: String,
    /// Whether `settings.json` was there before we wrote it.
    settings_existed: bool,
    /// The person's `statusLine` value, as the exact text it was; `None`
    /// when there was none.
    original: Option<String>,
}

/// Our command, for an account — with the instance in front when this is a
/// second copy of the system (`.#dev`), so the limits land in its state.
fn command_for(id: &str, instance: Option<&str>) -> String {
    match instance {
        Some(i) => format!(
            "{}={i} {PROGRAM} statusline --profile {id}",
            ronoctua_xdg::INSTANCE
        ),
        None => format!("{PROGRAM} statusline --profile {id}"),
    }
}

/// The account a command is OURS for: `[RONOCTUA_INSTANCE=<i>] <…>/ronoctua-llm
/// statusline --profile <id>`, wherever the program is.
#[must_use]
pub fn ours(command: &str) -> Option<String> {
    let mut words: Vec<&str> = command.split_whitespace().collect();
    if words
        .first()
        .is_some_and(|w| w.starts_with(&format!("{}=", ronoctua_xdg::INSTANCE)))
    {
        words.remove(0);
    }
    match words.as_slice() {
        [program, "statusline", "--profile", id]
            if (*program == "ronoctua-llm" || program.ends_with("/ronoctua-llm"))
                && crate::fine_id(id) =>
        {
            Some((*id).to_owned())
        }
        _ => None,
    }
}

/// The command of a `statusLine` value, when it is a command.
fn command_of(value: &serde_json::Value) -> Option<&str> {
    let object = value.as_object()?;
    if object
        .get("type")
        .is_some_and(|t| t.as_str() != Some("command"))
    {
        return None;
    }
    object
        .get("command")?
        .as_str()
        .filter(|c| !c.trim().is_empty())
}

/// The settings file's text, and whether it was there.
fn settings_text(dir: &Path) -> Result<(String, bool), Refusal> {
    match crate::files::read_bounded(&dir.join(SETTINGS_FILE), MOST_BYTES) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(|t| (t, true))
            .map_err(|_| Refusal::of(Why::Unreadable)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(("{}\n".to_owned(), false)),
        Err(_) => Err(Refusal::of(Why::Unreadable)),
    }
}

/// What was saved beside the settings, when anything was.
fn saved(dir: &Path) -> Result<Option<Saved>, Refusal> {
    match crate::files::read_bounded(&dir.join(SAVED_FILE), MOST_BYTES) {
        Ok(bytes) => serde_json::from_slice::<Saved>(&bytes)
            .ok()
            .filter(|s| s.format == 1)
            .map(Some)
            .ok_or(Refusal::of(Why::Unreadable)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Refusal::of(Why::Unreadable)),
    }
}

/// The current `statusLine` value of a settings text, parsed.
fn current(text: &str) -> Option<serde_json::Value> {
    crate::jsontext::raw(text, FIELD).and_then(|raw| serde_json::from_str(raw).ok())
}

/// A Claude Code subscription's account, or the refusal for a Codex one (it
/// has no status line) and an API key's (its status line tells no window).
fn claude(account: &Account) -> Result<(), Refusal> {
    if account.kind == Kind::Claude && crate::fine_id(&account.id) {
        Ok(())
    } else {
        Err(Refusal::of(Why::Invalid))
    }
}

/// Puts our status line in the account's `settings.json`, saving the
/// person's to chain it. Doing it again changes nothing.
///
/// # Errors
///
/// [`Why::Invalid`] for a Codex account (it has no status line) or an API
/// key's (it tells no window); [`Why::Unreadable`] when `settings.json` is
/// not a JSON object — it is
/// never rewritten then; [`Why::Failed`] when a file could not be written:
/// the settings are untouched if the person's line could not be saved, and
/// the saved line is put back as it was if ours could not be written.
pub fn wrap(account: &Account) -> Result<(), Refusal> {
    wrap_as(account, ronoctua_xdg::instance().as_deref())
}

/// [`wrap`], for a given instance.
pub(crate) fn wrap_as(account: &Account, instance: Option<&str>) -> Result<(), Refusal> {
    claude(account)?;
    let dir = &account.dir;
    let (text, existed) = settings_text(dir)?;
    if crate::jsontext::object(&text).is_none() {
        return Err(Refusal::of(Why::Unreadable));
    }
    let now = current(&text);
    let mine = command_for(&account.id, instance);
    if now.as_ref().and_then(command_of) == Some(mine.as_str()) {
        return Ok(());
    }
    // What to chain: the person's value now — unless it is ours (for any
    // account), and then whatever was saved before, so ours never chains
    // itself.
    let theirs = now.as_ref().and_then(command_of).and_then(ours).is_none();
    let earlier = saved(dir).ok().flatten();
    let keep = if theirs {
        Saved {
            format: 1,
            profile: account.id.clone(),
            settings_existed: existed,
            original: crate::jsontext::raw(&text, FIELD).map(str::to_owned),
        }
    } else {
        Saved {
            format: 1,
            profile: account.id.clone(),
            settings_existed: earlier.as_ref().is_none_or(|s| s.settings_existed),
            original: earlier.and_then(|s| s.original),
        }
    };
    let bytes = serde_json::to_vec_pretty(&keep).map_err(|_| Refusal::of(Why::Failed))?;
    // Ours keeps the person's other fields (`padding`, `refreshInterval`).
    let mut value = now
        .filter(|v| theirs && v.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("type".to_owned(), "command".into());
        object.insert("command".to_owned(), mine.into());
    }
    // Everything that can be refused is, before anything is written.
    let wrapped = crate::jsontext::set(&text, FIELD, &value).ok_or(Refusal::of(Why::Unreadable))?;
    let saved_path = dir.join(SAVED_FILE);
    let saved_before = match crate::files::read_bounded(&saved_path, MOST_BYTES) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err(Refusal::of(Why::Unreadable)),
    };
    // The person's line first, so ours is never in front of a line that was
    // not saved...
    crate::files::write_keeping(&saved_path, &bytes, 0o600)
        .map_err(|_| Refusal::of(Why::Failed))?;
    if crate::files::write_keeping(&dir.join(SETTINGS_FILE), wrapped.as_bytes(), 0o644).is_err() {
        // ...and taken back as it was when ours did not land: a saved line
        // beside settings that do not name us would read as the person
        // having replaced ours (`Wrapping::Replaced`), which they did not.
        let _ = match saved_before {
            Some(before) => crate::files::write_keeping(&saved_path, &before, 0o600),
            None => std::fs::remove_file(&saved_path),
        };
        return Err(Refusal::of(Why::Failed));
    }
    Ok(())
}

/// Gives the person's status line back — the exact text it was — and
/// forgets the saved one. When the person has put another one since, theirs
/// stays and only the saved copy goes. An account that was never wrapped is
/// left as it is.
///
/// # Errors
///
/// [`Why::Invalid`] for a Codex account or an API key's; [`Why::Unreadable`]
/// when a file is not what it should be — a saved line beside a `settings.json` that is not
/// a JSON object included (the person may be editing it): nothing is written
/// or deleted then, so their line is never lost; [`Why::Failed`] when a file
/// could not be written.
pub fn unwrap(account: &Account) -> Result<(), Refusal> {
    claude(account)?;
    let dir = &account.dir;
    let earlier = saved(dir)?;
    let (text, existed) = settings_text(dir)?;
    if earlier.is_some() && existed && crate::jsontext::object(&text).is_none() {
        return Err(Refusal::of(Why::Unreadable));
    }
    let is_ours = current(&text)
        .as_ref()
        .and_then(command_of)
        .and_then(ours)
        .is_some();
    if existed && is_ours {
        let original = earlier.as_ref().and_then(|s| s.original.as_deref());
        let back = match original {
            Some(raw) => crate::jsontext::set_raw(&text, FIELD, raw),
            None => crate::jsontext::remove(&text, FIELD),
        }
        .ok_or(Refusal::of(Why::Unreadable))?;
        let created_by_us = earlier.as_ref().is_some_and(|s| !s.settings_existed);
        let empty = crate::jsontext::object(&back).is_some_and(|(m, _)| m.is_empty());
        if created_by_us && empty {
            std::fs::remove_file(dir.join(SETTINGS_FILE)).map_err(|_| Refusal::of(Why::Failed))?;
        } else {
            crate::files::write_keeping(&dir.join(SETTINGS_FILE), back.as_bytes(), 0o644)
                .map_err(|_| Refusal::of(Why::Failed))?;
        }
    }
    match std::fs::remove_file(dir.join(SAVED_FILE)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(Refusal::of(Why::Failed)),
    }
}

/// Whether ours is in front of the account's status line.
///
/// # Errors
///
/// [`Why::Invalid`] for a Codex account or an API key's;
/// [`Why::Unreadable`] when `settings.json` does not read.
pub fn wrapped(account: &Account) -> Result<Wrapping, Refusal> {
    claude(account)?;
    let (text, _) = settings_text(&account.dir)?;
    let now = current(&text);
    let theirs_or_ours = now.as_ref().and_then(command_of).and_then(ours);
    if theirs_or_ours.as_deref() == Some(account.id.as_str()) {
        return Ok(Wrapping::Ours);
    }
    let was = saved(&account.dir).ok().flatten().is_some();
    Ok(if was || theirs_or_ours.is_some() {
        Wrapping::Replaced
    } else {
        Wrapping::Not
    })
}

/// The limits as kept on disk (`$XDG_STATE_HOME/ronoctua/llm/<id>/limits.json`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Kept {
    /// 1, or 2 when [`Kept::no_five_hour`] is said: a reader from before
    /// refuses the field it does not know as unreadable, which is honest,
    /// and only an account whose plan lacks the window writes a 2.
    format: u32,
    /// When these numbers were first seen, in seconds since the epoch.
    at_s: i64,
    /// The five-hour window.
    five_hour: Option<Window>,
    /// The weekly window.
    seven_day: Option<Window>,
    /// [`Limits::no_five_hour`].
    #[serde(default, skip_serializing_if = "crate::is_false")]
    no_five_hour: bool,
}

/// The file's name, in the account's state folder.
const LIMITS_FILE: &str = "limits.json";

/// The limits an account's agent last said, without a window that has
/// reset since: Claude Code's from what our status line kept, Codex's from
/// its newest session logs — and none for an API key, which has no window.
///
/// # Errors
///
/// [`Why::Unknown`] for an id that is not a folder's name;
/// [`Why::Unreadable`] when the kept file does not read. Nothing kept yet is
/// no window, not an error.
pub fn limits(account: &Account) -> Result<Limits, Refusal> {
    let now = crate::time::now_s();
    match account.kind {
        Kind::Codex => crate::tokens::codex_limits(&account.dir).map(|l| l.at(now)),
        Kind::ClaudeApi => Ok(Limits::default()),
        Kind::Claude => {
            let Some(state) = ronoctua_store::state_dir() else {
                return Ok(Limits::default());
            };
            kept_limits(&state, &account.id).map(|l| l.at(now))
        }
    }
}

/// What the status line kept for an account, under a state root.
fn kept_limits(state: &Path, id: &str) -> Result<Limits, Refusal> {
    let path = limits_path(state, id).ok_or(Refusal::of(Why::Unknown))?;
    match crate::files::read_bounded(&path, MOST_BYTES) {
        Ok(bytes) => serde_json::from_slice::<Kept>(&bytes)
            .ok()
            .filter(|k| k.format == 1 || (k.format == 2 && k.no_five_hour))
            .map(|k| Limits {
                five_hour: k.five_hour,
                seven_day: k.seven_day,
                no_five_hour: k.no_five_hour,
            })
            .ok_or(Refusal::of(Why::Unreadable)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Limits::default()),
        Err(_) => Err(Refusal::of(Why::Unreadable)),
    }
}

/// Where an account's limits are kept, under a state root.
pub(crate) fn limits_path(state: &Path, id: &str) -> Option<PathBuf> {
    crate::id_folder(state, id).map(|d| d.join(LIMITS_FILE))
}

/// The `rate_limits` of a status line's stdin, when it carries them.
///
/// Each window is read alone ([`Window::read`]): one whose shape changed is
/// not known, and the others still are. Claude Code leaves out a window the
/// plan does not have rather than sending it empty, so `rate_limits` with
/// the week and no `five_hour` is the plan's own word ([`Limits::no_five_hour`]).
fn limits_of(input: &[u8]) -> Option<Limits> {
    let input: serde_json::Value = serde_json::from_slice(input).ok()?;
    let limits = input.get("rate_limits")?.as_object()?;
    let window = |name: &str| {
        limits
            .get(name)
            .and_then(|w| Window::read(w, "used_percentage"))
    };
    Some(Limits {
        five_hour: window("five_hour"),
        seven_day: window("seven_day"),
        no_five_hour: !limits.contains_key("five_hour") && limits.contains_key("seven_day"),
    })
}

/// Keeps what a status line's stdin says of the limits, when it says
/// anything and it changed; the file is written whole and atomically.
fn keep_limits(state: &Path, id: &str, input: &[u8], now_s: i64) -> std::io::Result<()> {
    let Some(limits) = limits_of(input) else {
        return Ok(());
    };
    let Some(path) = limits_path(state, id) else {
        return Ok(());
    };
    if kept_limits(state, id).ok() == Some(limits) {
        return Ok(());
    }
    let kept = Kept {
        format: if limits.no_five_hour { 2 } else { 1 },
        at_s: now_s,
        five_hour: limits.five_hour,
        seven_day: limits.seven_day,
        no_five_hour: limits.no_five_hour,
    };
    let bytes = serde_json::to_vec_pretty(&kept)?;
    crate::files::write_keeping(&path, &bytes, 0o644)
}

/// What the chained run needs: where the accounts and the state are, the
/// folder Claude Code runs in, and the shell the person's command runs in.
#[derive(Debug, Clone)]
pub struct Chain {
    /// The roots.
    pub roots: crate::Roots,
    /// The folder of the Claude Code that runs us: its `CLAUDE_CONFIG_DIR`,
    /// or `~/.claude` when that is not set. Where the person's line is
    /// found when the list no longer names the account (removed by hand,
    /// or the file unreadable) — so their line is never silently dropped.
    pub agent_dir: Option<PathBuf>,
    /// The shell (`RONOCTUA_LLM_SH`); `None` chains nothing.
    pub shell: Option<PathBuf>,
    /// Whether this run is itself a chained child's ([`CHAINED`] set).
    pub chained: bool,
}

impl Chain {
    /// The machine's: the roots and Claude Code's folder from the
    /// environment, the shell by store path.
    #[must_use]
    pub fn of_env() -> Self {
        let roots = crate::Roots::of_env();
        let agent_dir = agent_dir_of(&|name| std::env::var_os(name), roots.home.as_deref());
        Self {
            roots,
            agent_dir,
            shell: option_env!("RONOCTUA_LLM_SH").map(PathBuf::from),
            chained: std::env::var_os(CHAINED).is_some(),
        }
    }
}

/// The folder of the Claude Code a program of ours runs under — its status
/// line, its hook: `CLAUDE_CONFIG_DIR` when the environment says it, and
/// `~/.claude` when it does not (Claude Code's own, opened with the variable
/// unset). A relative folder is not one Claude Code could have meant for us:
/// nothing is guessed then.
pub(crate) fn agent_dir_of(var: crate::Lookup<'_>, home: Option<&Path>) -> Option<PathBuf> {
    match var(Kind::Claude.dir_variable()) {
        Some(dir) => Some(PathBuf::from(dir)).filter(|d| d.is_absolute()),
        None => home.map(|h| Kind::Claude.native_dir(h)),
    }
}

/// `ronoctua-llm statusline --profile <id>`: keeps the limits `input` says,
/// then runs the person's own status line with the same `input`, its stdout
/// going to `out` — and answers its exit code. Nothing to chain prints
/// nothing and answers 0.
///
/// Never fails the person's status line for a reason of ours: a limit that
/// cannot be kept is said on stderr (which Claude Code shows only under
/// `--debug`), and the chain still runs.
pub fn run(profile: &str, input: &[u8], chain: &Chain, out: std::process::Stdio) -> i32 {
    if !crate::fine_id(profile) {
        return 0;
    }
    if let Some(state) = &chain.roots.state
        && let Err(e) = keep_limits(state, profile, input, crate::time::now_s())
    {
        eprintln!("ronoctua-llm: the limits could not be kept: {e}");
    }
    if chain.chained {
        return 0;
    }
    let Some(command) = chained_command(chain, profile) else {
        return 0;
    };
    let Some(shell) = chain.shell.as_deref() else {
        eprintln!("ronoctua-llm: built without RONOCTUA_LLM_SH, so nothing is chained");
        return 0;
    };
    match run_chained(shell, &command, input, out) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("ronoctua-llm: the status line could not be run: {e}");
            1
        }
    }
}

/// The person's command to chain for an account, when there is one and it
/// is not ours.
///
/// The line saved FOR THIS ACCOUNT, in the account's folder as the list
/// says — or, when the list does not have it, in the folder Claude Code
/// runs in ([`Chain::agent_dir`]): our command naming this account is
/// still in that folder's `settings.json`, and its saved line is the
/// person's, whatever the list says.
fn chained_command(chain: &Chain, profile: &str) -> Option<String> {
    let accounts = crate::Accounts::store_in(chain.roots.config.clone()).read();
    let listed = accounts.find(profile).map(|a| a.dir.clone());
    let saved = listed
        .iter()
        .chain(chain.agent_dir.iter())
        .find_map(|dir| saved(dir).ok().flatten().filter(|s| s.profile == profile))?;
    let value: serde_json::Value = serde_json::from_str(saved.original.as_deref()?).ok()?;
    let command = command_of(&value)?;
    ours(command).is_none().then(|| command.to_owned())
}

/// Runs `command` in `shell` with `input` on its stdin; its stdout to
/// `out`, its stderr to ours; answers its exit code (128 + the signal, as a
/// shell says, when one ended it).
fn run_chained(
    shell: &Path,
    command: &str,
    input: &[u8],
    out: std::process::Stdio,
) -> std::io::Result<i32> {
    use std::os::unix::process::ExitStatusExt as _;
    let mut child = std::process::Command::new(shell)
        .arg("-c")
        .arg(command)
        .env(CHAINED, "1")
        .stdin(std::process::Stdio::piped())
        .stdout(out)
        .spawn()?;
    // From a thread: a command that reads nothing, or reads slowly, must not
    // stall the wait behind a full pipe.
    let feeding = child.stdin.take().map(|mut stdin| {
        let input = input.to_vec();
        std::thread::spawn(move || {
            // A command that does not read its stdin closes the pipe: not
            // an error of anybody's.
            let _ = stdin.write_all(&input);
        })
    });
    let status = child.wait()?;
    if let Some(feeding) = feeding {
        let _ = feeding.join();
    }
    Ok(status
        .code()
        .or_else(|| status.signal().map(|s| 128 + s))
        .unwrap_or(1))
}

/// The most of a status line's stdin that is read, in bytes: Claude Code's
/// is a few kilobytes.
pub const MOST_INPUT: u64 = MOST_BYTES;

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-llm-statusline-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn account(dir: &Path, id: &str) -> Account {
        Account {
            id: id.to_owned(),
            kind: Kind::Claude,
            name: "Work".to_owned(),
            dir: dir.join("account"),
            default: true,
        }
    }

    /// A shell for the chain: the store path the build hands over, or the
    /// one every POSIX system has when a test runs outside the flake.
    fn shell() -> PathBuf {
        PathBuf::from(option_env!("RONOCTUA_LLM_SH").unwrap_or("/bin/sh"))
    }

    /// The person's settings, as they wrote them.
    const SETTINGS: &str = "{\n  \"model\": \"opus\",\n  \"statusLine\": {\n    \"type\": \"command\",\n    \"command\": \"tr a-z A-Z; exit 3\",\n    \"padding\": 1\n  },\n  \"env\": {\n    \"DISABLE_TELEMETRY\": \"1\"\n  }\n}\n";

    /// A status line's stdin, its windows far in the future.
    const INPUT: &str = r#"{"model":{"id":"claude-opus","display_name":"Opus"},"rate_limits":{"five_hour":{"used_percentage":23.5,"resets_at":4102444800},"seven_day":{"used_percentage":41.2,"resets_at":4102531200}}}"#;

    fn chain_in(dir: &Path, accounts: &crate::Accounts) -> Chain {
        let config = dir.join("config");
        crate::Accounts::store_in(Some(config.clone()))
            .write(accounts)
            .unwrap();
        Chain {
            roots: crate::Roots {
                config: Some(config),
                state: Some(dir.join("state")),
                cache: Some(dir.join("cache")),
                home: Some(dir.to_path_buf()),
            },
            agent_dir: None,
            shell: Some(shell()),
            chained: false,
        }
    }

    fn run_to_file(dir: &Path, profile: &str, input: &str, chain: &Chain) -> (i32, String) {
        let path = dir.join("out.txt");
        let file = std::fs::File::create(&path).unwrap();
        let code = run(profile, input.as_bytes(), chain, file.into());
        (code, std::fs::read_to_string(&path).unwrap())
    }

    /// FAILS IF: the person's status line is not run behind ours with the
    /// same stdin, its stdout and exit code not passed through, the limits
    /// not kept — or the person's settings do not come back byte for byte.
    #[test]
    fn the_persons_status_line_is_chained_and_given_back() {
        let dir = scratch("chain");
        let account = account(&dir, "claude-1");
        std::fs::create_dir_all(&account.dir).unwrap();
        std::fs::write(account.dir.join(SETTINGS_FILE), SETTINGS).unwrap();
        let chain = chain_in(
            &dir,
            &crate::Accounts {
                format: 1,
                accounts: vec![account.clone()],
            },
        );

        assert_eq!(wrapped(&account).unwrap(), Wrapping::Not);
        wrap_as(&account, None).unwrap();
        assert_eq!(wrapped(&account).unwrap(), Wrapping::Ours);
        let text = std::fs::read_to_string(account.dir.join(SETTINGS_FILE)).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value["statusLine"],
            serde_json::json!({
                "type": "command",
                "command": "/run/current-system/sw/bin/ronoctua-llm statusline --profile claude-1",
                "padding": 1
            }),
            "ours, with the person's padding"
        );
        assert!(text.starts_with("{\n  \"model\": \"opus\",\n"), "{text}");
        assert!(text.ends_with("  \"env\": {\n    \"DISABLE_TELEMETRY\": \"1\"\n  }\n}\n"));

        // Twice is once: the saved line is still the person's.
        wrap_as(&account, None).unwrap();

        let (code, out) = run_to_file(&dir, "claude-1", INPUT, &chain);
        assert_eq!(code, 3, "the person's exit code");
        assert_eq!(
            out,
            INPUT.to_uppercase(),
            "the person's stdout, of the same stdin"
        );
        let kept = kept_limits(&dir.join("state"), "claude-1").unwrap();
        assert_eq!(
            kept,
            Limits {
                five_hour: Some(Window {
                    used_percent: 23.5,
                    resets_at_s: 4_102_444_800
                }),
                seven_day: Some(Window {
                    used_percent: 41.2,
                    resets_at_s: 4_102_531_200
                }),
                no_five_hour: false,
            }
        );
        assert_eq!(
            kept.at(4_102_500_000).five_hour,
            None,
            "a window past its reset"
        );

        unwrap(&account).unwrap();
        assert_eq!(
            std::fs::read_to_string(account.dir.join(SETTINGS_FILE)).unwrap(),
            SETTINGS,
            "given back byte for byte"
        );
        assert!(!account.dir.join(SAVED_FILE).exists());
        assert_eq!(wrapped(&account).unwrap(), Wrapping::Not);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: an account with no status line of its own gets one printed
    /// for it, or keeps a `settings.json` we created only to wrap it.
    #[test]
    fn nothing_to_chain_prints_nothing_and_leaves_no_file() {
        let dir = scratch("nothing");
        let account = account(&dir, "claude-2");
        std::fs::create_dir_all(&account.dir).unwrap();
        let chain = chain_in(
            &dir,
            &crate::Accounts {
                format: 1,
                accounts: vec![account.clone()],
            },
        );
        wrap_as(&account, None).unwrap();
        let (code, out) = run_to_file(&dir, "claude-2", INPUT, &chain);
        assert_eq!((code, out.as_str()), (0, ""));
        unwrap(&account).unwrap();
        assert!(
            !account.dir.join(SETTINGS_FILE).exists(),
            "the file we created goes with us"
        );

        // A file that had other things, and no status line: those stay.
        std::fs::write(
            account.dir.join(SETTINGS_FILE),
            "{\n  \"model\": \"opus\"\n}\n",
        )
        .unwrap();
        wrap_as(&account, None).unwrap();
        unwrap(&account).unwrap();
        assert_eq!(
            std::fs::read_to_string(account.dir.join(SETTINGS_FILE)).unwrap(),
            "{\n  \"model\": \"opus\"\n}\n"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: ours ever chains ours — by wrapping over itself, by a saved
    /// command that is ours, or by a chained child running the chain again.
    #[test]
    fn ours_never_chains_ours() {
        let dir = scratch("loop");
        let account = account(&dir, "claude-3");
        std::fs::create_dir_all(&account.dir).unwrap();
        let mut chain = chain_in(
            &dir,
            &crate::Accounts {
                format: 1,
                accounts: vec![account.clone()],
            },
        );
        // Settings that already carry OUR command, for another account.
        let foreign = "{\"statusLine\": {\"type\": \"command\", \"command\": \"/run/current-system/sw/bin/ronoctua-llm statusline --profile claude-9\"}}";
        std::fs::write(account.dir.join(SETTINGS_FILE), foreign).unwrap();
        assert_eq!(wrapped(&account).unwrap(), Wrapping::Replaced);
        wrap_as(&account, Some("dev")).unwrap();
        let saved = saved(&account.dir).unwrap().unwrap();
        assert_eq!(saved.original, None, "ours is never saved as the person's");
        assert_eq!(
            current(&std::fs::read_to_string(account.dir.join(SETTINGS_FILE)).unwrap())
                .as_ref()
                .and_then(command_of)
                .map(str::to_owned),
            Some(
                "RONOCTUA_INSTANCE=dev /run/current-system/sw/bin/ronoctua-llm statusline --profile claude-3"
                    .to_owned()
            )
        );
        let (code, out) = run_to_file(&dir, "claude-3", INPUT, &chain);
        assert_eq!((code, out.as_str()), (0, ""));

        // A person's line that runs us: the child carries the mark, and a
        // run that has it chains nothing.
        std::fs::write(
            account.dir.join(SETTINGS_FILE),
            r#"{"statusLine": {"type": "command", "command": "printf '%s' \"$RONOCTUA_LLM_CHAINED\""}}"#,
        )
        .unwrap();
        wrap_as(&account, None).unwrap();
        let (code, out) = run_to_file(&dir, "claude-3", INPUT, &chain);
        assert_eq!(
            (code, out.as_str()),
            (0, "1"),
            "the child knows it is chained"
        );
        chain.chained = true;
        let (code, out) = run_to_file(&dir, "claude-3", INPUT, &chain);
        assert_eq!(
            (code, out.as_str()),
            (0, ""),
            "a chained run chains nothing"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_line_the_person_put_since_is_theirs() {
        let dir = scratch("replaced");
        let account = account(&dir, "claude-4");
        std::fs::create_dir_all(&account.dir).unwrap();
        std::fs::write(account.dir.join(SETTINGS_FILE), SETTINGS).unwrap();
        wrap_as(&account, None).unwrap();
        let newer = "{\n  \"statusLine\": {\"type\": \"command\", \"command\": \"echo mine\"}\n}\n";
        std::fs::write(account.dir.join(SETTINGS_FILE), newer).unwrap();
        assert_eq!(wrapped(&account).unwrap(), Wrapping::Replaced);
        unwrap(&account).unwrap();
        assert_eq!(
            std::fs::read_to_string(account.dir.join(SETTINGS_FILE)).unwrap(),
            newer,
            "their newer choice is left alone"
        );
        assert!(!account.dir.join(SAVED_FILE).exists());

        std::fs::write(account.dir.join(SETTINGS_FILE), "{ broken").unwrap();
        assert_eq!(wrap_as(&account, None).unwrap_err().why, Why::Unreadable);
        assert_eq!(
            std::fs::read_to_string(account.dir.join(SETTINGS_FILE)).unwrap(),
            "{ broken",
            "a file that does not parse is never rewritten"
        );
        let codex = Account {
            kind: Kind::Codex,
            ..account
        };
        assert_eq!(wrap_as(&codex, None).unwrap_err().why, Why::Invalid);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: an account the list lost (removed by hand, or the file
    /// unreadable) silences the person's line while ours is still in its
    /// `settings.json` — or a folder's saved line is run for another account.
    #[test]
    fn a_line_whose_account_the_list_lost_is_still_chained() {
        let dir = scratch("lost");
        let account = account(&dir, "claude-5");
        std::fs::create_dir_all(&account.dir).unwrap();
        std::fs::write(account.dir.join(SETTINGS_FILE), SETTINGS).unwrap();
        let mut chain = chain_in(
            &dir,
            &crate::Accounts {
                format: 1,
                accounts: vec![account.clone()],
            },
        );
        wrap_as(&account, None).unwrap();
        // The list forgets it behind our back.
        crate::Accounts::store_in(chain.roots.config.clone())
            .write(&crate::Accounts::default())
            .unwrap();
        let (code, out) = run_to_file(&dir, "claude-5", INPUT, &chain);
        assert_eq!(
            (code, out.as_str()),
            (0, ""),
            "nowhere to look, nothing run"
        );
        // Claude Code runs us in the account's folder: the line is there.
        chain.agent_dir = Some(account.dir);
        let (code, out) = run_to_file(&dir, "claude-5", INPUT, &chain);
        assert_eq!((code, out), (3, INPUT.to_uppercase()));
        // The same folder, asked for another account: not its line.
        let (code, out) = run_to_file(&dir, "claude-6", INPUT, &chain);
        assert_eq!((code, out.as_str()), (0, ""));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a wrap whose settings could not be written leaves the saved
    /// line behind — then [`wrapped`] says the person replaced ours, which
    /// they did not — or loses a line saved by an earlier wrap.
    #[test]
    fn a_wrap_that_does_not_land_leaves_nothing_behind() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch("unlanded");
        let account = account(&dir, "claude-7");
        std::fs::create_dir_all(&account.dir).unwrap();
        // The settings live in a folder nobody may write (a dotfiles store),
        // linked from the account's.
        let store = dir.join("dotfiles");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join(SETTINGS_FILE), SETTINGS).unwrap();
        std::os::unix::fs::symlink(store.join(SETTINGS_FILE), account.dir.join(SETTINGS_FILE))
            .unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o555)).unwrap();
        if std::fs::write(store.join("probe"), b"").is_ok() {
            // Whoever runs this may write anywhere (root): nothing to prove.
            std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o755)).unwrap();
            let _ = std::fs::remove_dir_all(dir);
            return;
        }
        assert_eq!(wrap_as(&account, None).unwrap_err().why, Why::Failed);
        assert!(!account.dir.join(SAVED_FILE).exists(), "no saved line left");
        assert_eq!(wrapped(&account).unwrap(), Wrapping::Not);
        assert_eq!(
            std::fs::read_to_string(account.dir.join(SETTINGS_FILE)).unwrap(),
            SETTINGS
        );

        // A line saved before (ours was put in front, then the folder went
        // read-only and ours was replaced by ours for another account): the
        // failed wrap gives the earlier saved line back as it was.
        let earlier = br#"{"format":1,"profile":"claude-7","settings-existed":true,"original":"{\"type\":\"command\",\"command\":\"echo mine\"}"}"#;
        std::fs::write(account.dir.join(SAVED_FILE), earlier).unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o755)).unwrap();
        let foreign = "{\"statusLine\": {\"type\": \"command\", \"command\": \"/run/current-system/sw/bin/ronoctua-llm statusline --profile claude-9\"}}";
        std::fs::write(store.join(SETTINGS_FILE), foreign).unwrap();
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert_eq!(wrap_as(&account, None).unwrap_err().why, Why::Failed);
        assert_eq!(
            std::fs::read(account.dir.join(SAVED_FILE)).unwrap(),
            earlier,
            "the earlier saved line, as it was"
        );
        std::fs::set_permissions(&store, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: unwrapping beside a `settings.json` that does not parse
    /// (the person mid-edit) deletes the saved line it cannot put back.
    #[test]
    fn a_line_is_never_given_back_into_a_file_that_does_not_read() {
        let dir = scratch("midedit");
        let account = account(&dir, "claude-8");
        std::fs::create_dir_all(&account.dir).unwrap();
        std::fs::write(account.dir.join(SETTINGS_FILE), SETTINGS).unwrap();
        wrap_as(&account, None).unwrap();
        let saved_line = std::fs::read(account.dir.join(SAVED_FILE)).unwrap();
        std::fs::write(account.dir.join(SETTINGS_FILE), "{ \"statusLine\": ").unwrap();
        assert_eq!(unwrap(&account).unwrap_err().why, Why::Unreadable);
        assert_eq!(
            std::fs::read(account.dir.join(SAVED_FILE)).unwrap(),
            saved_line
        );
        // Never wrapped: nothing to give back, nothing refused.
        let other = Account {
            dir: dir.join("never"),
            ..account
        };
        std::fs::create_dir_all(&other.dir).unwrap();
        std::fs::write(other.dir.join(SETTINGS_FILE), "{ broken").unwrap();
        assert_eq!(unwrap(&other), Ok(()));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a window that does not say how much of it is used is read
    /// as 0 % — a calm mark while the person may be at the edge.
    #[test]
    fn a_window_without_its_percentage_is_not_known() {
        let input = br#"{"rate_limits":{"five_hour":{"resets_at":4102444800},"seven_day":{"used_percentage":95.5,"resets_at":4102531200}}}"#;
        assert_eq!(
            limits_of(input),
            Some(Limits {
                five_hour: None,
                seven_day: Some(Window {
                    used_percent: 95.5,
                    resets_at_s: 4_102_531_200
                }),
                // The window is THERE, only unreadable: not a plan without it.
                no_five_hour: false,
            })
        );
        let renamed = br#"{"rate_limits":{"five_hour":{"used_pct":12,"resets_at":4102444800}}}"#;
        assert_eq!(limits_of(renamed).unwrap().five_hour, None);
    }

    /// FAILS IF: an Ultra account — the week alone, the status line leaving
    /// `five_hour` out — is read as a five-hour window "not known yet"
    /// forever, or the plan's word is lost on the way to the file and back,
    /// or after the week resets.
    #[test]
    fn an_ultra_status_line_says_the_week_alone() {
        let input =
            br#"{"rate_limits":{"seven_day":{"used_percentage":71,"resets_at":1793728800}}}"#;
        let limits = limits_of(input).unwrap();
        assert_eq!(
            limits,
            Limits {
                five_hour: None,
                seven_day: Some(Window {
                    used_percent: 71.0,
                    resets_at_s: 1_793_728_800
                }),
                no_five_hour: true,
            }
        );
        assert!(
            limits.at(1_793_728_801).no_five_hour,
            "the plan does not reset with the week"
        );

        let dir = scratch("ultra");
        let state = dir.join("state");
        keep_limits(&state, "claude-1", input, 1_793_000_000).unwrap();
        let text = std::fs::read_to_string(limits_path(&state, "claude-1").unwrap()).unwrap();
        assert!(text.contains("\"format\": 2"), "{text}");
        assert_eq!(kept_limits(&state, "claude-1").unwrap(), limits);

        // A plan WITH the window writes what it always wrote.
        let max = br#"{"rate_limits":{"five_hour":{"used_percentage":12,"resets_at":1793000600},"seven_day":{"used_percentage":30,"resets_at":1793728800}}}"#;
        keep_limits(&state, "claude-1", max, 1_793_000_100).unwrap();
        let text = std::fs::read_to_string(limits_path(&state, "claude-1").unwrap()).unwrap();
        assert!(text.contains("\"format\": 1"), "{text}");
        assert!(!text.contains("no-five-hour"), "{text}");
        assert!(!kept_limits(&state, "claude-1").unwrap().no_five_hour);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: one window whose shape changed (a reset given as a text, a
    /// percentage given as a string) takes the readable one down with it.
    #[test]
    fn a_window_that_changed_its_shape_leaves_the_others() {
        let input = br#"{"rate_limits":{"five_hour":{"used_percentage":"12","resets_at":4102444800},"seven_day":{"used_percentage":40,"resets_at":"2100-01-02T00:00:00+00:00"}}}"#;
        let limits = limits_of(input).unwrap();
        assert_eq!(limits.five_hour, None);
        assert_eq!(
            limits.seven_day,
            Some(Window {
                used_percent: 40.0,
                resets_at_s: crate::time::parse_rfc3339("2100-01-02T00:00:00Z").unwrap(),
            })
        );
        assert!(!limits.no_five_hour);
        assert_eq!(limits_of(br#"{"rate_limits":null}"#), None);
        assert_eq!(limits_of(br#"{"model":"opus"}"#), None);
    }

    #[test]
    fn ours_is_recognised_wherever_the_program_is() {
        assert_eq!(
            ours("/run/current-system/sw/bin/ronoctua-llm statusline --profile claude-1"),
            Some("claude-1".to_owned())
        );
        assert_eq!(
            ours(
                "RONOCTUA_INSTANCE=dev /nix/store/x-ronoctua-llm/bin/ronoctua-llm statusline --profile c-2"
            ),
            Some("c-2".to_owned())
        );
        assert_eq!(ours("ronoctua-llm statusline --profile ../x"), None);
        assert_eq!(ours("~/bin/line.sh"), None);
        assert_eq!(ours("ronoctua-llm-other statusline --profile a"), None);
        assert_eq!(
            limits_of(br#"{"model":{}}"#),
            None,
            "no rate_limits says nothing"
        );
        assert_eq!(limits_of(b"not json"), None);
    }
}
