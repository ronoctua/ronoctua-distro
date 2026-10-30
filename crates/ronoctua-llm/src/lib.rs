//! The person's LLM accounts — the one crate that reads Claude Code's and
//! Codex's state and asks Anthropic's usage endpoint.
//!
//! # What it reads, and how little
//!
//! - `llm-accounts.json` ([`Accounts`], through the person's store): which
//!   accounts, of which kind, in which folder, which is the default of its
//!   kind. Every edit is applied to what the file says NOW, so two doors
//!   editing two accounts keep both.
//! - [`credentials::facts`]: only the subscription, the tier and when the
//!   token expires — read into memory that is wiped, the token itself never
//!   deserialized.
//! - [`statusline`]: the official status line's `rate_limits`, written by
//!   `ronoctua-llm statusline` (which CHAINS the status line the person
//!   already had); Codex's, out of its own logs.
//! - [`usage::fetch`]: the UNOFFICIAL OAuth usage endpoint, with a consent
//!   `Pass`, never renewing a token (401 is [`Why::Expired`]); the screen
//!   says it is unofficial.
//! - [`tokens::by_day_and_model`]: the local transcripts, counted once each,
//!   incrementally, inside a bounded window.
//!
//! # And what it writes
//!
//! `llm-accounts.json` and `llm-default.env` (the default accounts, which
//! the shell sources so `claude` and `codex` typed in a terminal use them);
//! one field of a Claude account's `settings.json`, with the person's value
//! kept beside it (given back when the account is removed); and its own
//! state: `$XDG_STATE_HOME/ronoctua/llm/<id>/` (`limits.json`, `usage.json`)
//! and `$XDG_CACHE_HOME/ronoctua/llm/<id>/` (the token count), written by
//! [`refresh::run`] — and watched through [`watched`], so nobody else learns
//! those paths.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// THE HALF THAT READS A LOGIN AND ASKS THE NETWORK, behind `fetch`.
#[cfg(feature = "fetch")]
pub mod credentials;
mod files;
mod jsontext;
#[cfg(feature = "fetch")]
pub mod refresh;
pub mod statusline;
mod time;
pub mod tokens;
pub mod usage;

/// Why something was not read or done, as a closed set (`agents-refused-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// The host was not let in `consent.json`.
    NotLet,
    /// The account's token expired: Claude Code renews it the next time it
    /// runs, or the person logs in again (never renewed here).
    Expired,
    /// The account has no credentials (never logged in).
    NoCredentials,
    /// A file does not read.
    Unreadable,
    /// The network did not answer.
    Offline,
    /// No such account.
    Unknown,
    /// What was given is not an account: an id, a name or a folder that does
    /// not fit, one already taken — or a Codex account asked for a status
    /// line, which only Claude Code has.
    Invalid,
    /// The agent is not installed on this machine.
    NotInstalled,
    /// The agent is wanted and has not arrived yet: Claude Code is a part,
    /// realized after the installation (`nix/modules/dev.nix`).
    Arriving,
    /// Something else failed.
    Failed,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 10] = [
        Self::NotLet,
        Self::Expired,
        Self::NoCredentials,
        Self::Unreadable,
        Self::Offline,
        Self::Unknown,
        Self::Invalid,
        Self::NotInstalled,
        Self::Arriving,
        Self::Failed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotLet => "not-let",
            Self::Expired => "expired",
            Self::NoCredentials => "no-credentials",
            Self::Unreadable => "unreadable",
            Self::Offline => "offline",
            Self::Unknown => "unknown",
            Self::Invalid => "invalid",
            Self::NotInstalled => "not-installed",
            Self::Arriving => "arriving",
            Self::Failed => "failed",
        }
    }

    /// The one named `id`.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// A refusal for this reason.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self { why }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.why.id())
    }
}

impl std::error::Error for Refusal {}

/// Which agent an account is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Claude Code (`CLAUDE_CONFIG_DIR`).
    Claude,
    /// Codex (`CODEX_HOME`).
    Codex,
}

impl Kind {
    /// Both.
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];

    /// Its kebab-case name — also the stem of its accounts' ids.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    /// The variable that points the agent at an account's folder.
    const fn dir_variable(self) -> &'static str {
        match self {
            Self::Claude => "CLAUDE_CONFIG_DIR",
            Self::Codex => "CODEX_HOME",
        }
    }

    /// The folder the agent uses when no variable says otherwise, under the
    /// home.
    const fn native_folder(self) -> &'static str {
        match self {
            Self::Claude => ".claude",
            Self::Codex => ".codex",
        }
    }

    /// The variable the session hands the agent's store path in (set where
    /// the agents are installed, `nix/modules/dev.nix`).
    const fn program_variable(self) -> &'static str {
        match self {
            Self::Claude => "RONOCTUA_CLAUDE",
            Self::Codex => "RONOCTUA_CODEX",
        }
    }

    /// The folder the agent uses with no variable set: `~/.claude`,
    /// `~/.codex`.
    #[must_use]
    pub fn native_dir(self, home: &Path) -> PathBuf {
        home.join(self.native_folder())
    }
}

/// One account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Account {
    /// Its id (stable; the mark's instance follows its place in the list).
    pub id: String,
    /// Which agent.
    pub kind: Kind,
    /// What the person calls it, data.
    pub name: String,
    /// Its configuration folder.
    pub dir: PathBuf,
    /// Whether it is the default of its kind.
    pub default: bool,
}

/// `$XDG_CONFIG_HOME/ronoctua/llm-accounts.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Accounts {
    /// Always 1.
    pub format: u32,
    /// The accounts, in the person's order.
    pub accounts: Vec<Account>,
}

impl Default for Accounts {
    fn default() -> Self {
        Self {
            format: 1,
            accounts: Vec::new(),
        }
    }
}

/// The longest name an account may have, in characters.
pub const MOST_NAME_CHARS: usize = 64;

/// Whether `id` may name an account — and a folder: `[a-z0-9][a-z0-9-]*`, at
/// most 32.
pub(crate) fn fine_id(id: &str) -> bool {
    id.len() <= 32
        && id
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `<root>/llm/<id>` — an account's folder of ours, under a root — or
/// `None` for an id that is not a folder's name.
pub(crate) fn id_folder(root: &Path, id: &str) -> Option<PathBuf> {
    fine_id(id).then(|| root.join("llm").join(id))
}

/// Whether a name is one: not blank, no control character, at most
/// [`MOST_NAME_CHARS`].
fn fine_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.chars().count() <= MOST_NAME_CHARS
        && !name.chars().any(char::is_control)
}

/// Whether a folder may be an account's: absolute, text, no control
/// character (it goes into an environment and into `llm-default.env`).
fn fine_dir(dir: &Path) -> bool {
    dir.is_absolute()
        && dir
            .to_str()
            .is_some_and(|d| !d.chars().any(char::is_control))
        && !dir
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}

/// The refusal for an account that is not one, or none.
fn check_account(account: &Account) -> Result<(), Refusal> {
    if fine_id(&account.id) && fine_name(&account.name) && fine_dir(&account.dir) {
        Ok(())
    } else {
        Err(Refusal::of(Why::Invalid))
    }
}

/// The file the default accounts are exported in, beside the accounts.
pub const DEFAULT_ENV_FILE: &str = "llm-default.env";

impl Accounts {
    /// The file's name under `$XDG_CONFIG_HOME/ronoctua/`.
    pub const FILE: &'static str = "llm-accounts.json";

    /// One by id.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// The default of a kind.
    #[must_use]
    pub fn default_of(&self, kind: Kind) -> Option<&Account> {
        self.accounts.iter().find(|a| a.kind == kind && a.default)
    }

    /// Whether the list is one: format 1, every account fine, ids and
    /// folders distinct, at most one default of each kind.
    ///
    /// # Errors
    ///
    /// [`Why::Unreadable`] for another format; [`Why::Invalid`] otherwise.
    pub fn check(&self) -> Result<(), Refusal> {
        if self.format != 1 {
            return Err(Refusal::of(Why::Unreadable));
        }
        for (n, a) in self.accounts.iter().enumerate() {
            check_account(a)?;
            let later = &self.accounts[n + 1..];
            if later.iter().any(|b| b.id == a.id || b.dir == a.dir) {
                return Err(Refusal::of(Why::Invalid));
            }
        }
        for kind in Kind::ALL {
            let defaults = self
                .accounts
                .iter()
                .filter(|a| a.kind == kind && a.default)
                .count();
            if defaults > 1 {
                return Err(Refusal::of(Why::Invalid));
            }
        }
        Ok(())
    }

    /// Adds an account. The first of its kind, or one added as the default,
    /// becomes the default of its kind.
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`] when it is not an account, or its id or its folder is
    /// another's.
    pub fn add(&mut self, account: Account) -> Result<(), Refusal> {
        check_account(&account)?;
        if self
            .accounts
            .iter()
            .any(|a| a.id == account.id || a.dir == account.dir)
        {
            return Err(Refusal::of(Why::Invalid));
        }
        let (id, kind, default) = (account.id.clone(), account.kind, account.default);
        self.accounts.push(Account {
            default: false,
            ..account
        });
        if default || self.default_of(kind).is_none() {
            self.make_default(&id);
        }
        Ok(())
    }

    /// Renames one.
    ///
    /// # Errors
    ///
    /// [`Why::Unknown`] when there is no such account; [`Why::Invalid`] for a
    /// name that is not one.
    pub fn rename(&mut self, id: &str, name: &str) -> Result<(), Refusal> {
        if !fine_name(name) {
            return Err(Refusal::of(Why::Invalid));
        }
        let account = self
            .accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(Refusal::of(Why::Unknown))?;
        name.clone_into(&mut account.name);
        Ok(())
    }

    /// Removes one (its folder stays: it holds the person's sessions and
    /// login). When it was the default, the next of its kind becomes it.
    ///
    /// Through [`Accounts::keep`], a removed Claude Code account gets its own
    /// status line back first ([`statusline::unwrap`]): ours would otherwise
    /// stay in its `settings.json`, naming an account nobody lists.
    ///
    /// # Errors
    ///
    /// [`Why::Unknown`] when there is no such account.
    pub fn remove(&mut self, id: &str) -> Result<(), Refusal> {
        let at = self
            .accounts
            .iter()
            .position(|a| a.id == id)
            .ok_or(Refusal::of(Why::Unknown))?;
        let gone = self.accounts.remove(at);
        if gone.default
            && let Some(next) = self.accounts.iter_mut().find(|a| a.kind == gone.kind)
        {
            next.default = true;
        }
        Ok(())
    }

    /// Makes one the default of its kind.
    ///
    /// # Errors
    ///
    /// [`Why::Unknown`] when there is no such account.
    pub fn set_default(&mut self, id: &str) -> Result<(), Refusal> {
        if self.find(id).is_none() {
            return Err(Refusal::of(Why::Unknown));
        }
        self.make_default(id);
        Ok(())
    }

    /// `id` the default of its kind, and no other of that kind.
    fn make_default(&mut self, id: &str) {
        let Some(kind) = self.find(id).map(|a| a.kind) else {
            return;
        };
        for a in self.accounts.iter_mut().filter(|a| a.kind == kind) {
            a.default = a.id == id;
        }
    }

    /// A new account of `kind` named `name`, not yet added: the next free
    /// id (`claude-2`), the agent's own folder when no account has it yet
    /// and `~/.claude-2` otherwise (never one that exists), the default when
    /// its kind has none.
    ///
    /// # Errors
    ///
    /// [`Why::Invalid`] for a name that is not one.
    pub fn fresh(&self, kind: Kind, name: &str, home: &Path) -> Result<Account, Refusal> {
        let taken = |id: &str| self.accounts.iter().any(|a| a.id == id);
        let n = (1..)
            .find(|n| !taken(&format!("{}-{n}", kind.id())))
            .unwrap_or(1);
        let native = kind.native_dir(home);
        let used = |dir: &Path| self.accounts.iter().any(|a| a.dir == dir);
        let dir = if used(&native) {
            (2..)
                .map(|m| home.join(format!("{}-{m}", kind.native_folder())))
                .find(|d| !used(d) && !d.exists())
                .unwrap_or(native)
        } else {
            native
        };
        let account = Account {
            id: format!("{}-{n}", kind.id()),
            kind,
            name: name.trim().to_owned(),
            dir,
            default: self.default_of(kind).is_none(),
        };
        check_account(&account)?;
        Ok(account)
    }

    /// The store of this instance.
    #[must_use]
    pub fn store() -> ronoctua_store::Store<Self> {
        Self::store_in(ronoctua_store::config_dir())
    }

    /// The store under a config root — a caller that carries its roots (a
    /// test of a door's gestures) instead of the environment's.
    #[must_use]
    pub fn store_in(config: Option<PathBuf>) -> ronoctua_store::Store<Self> {
        ronoctua_store::Store::at(config, Self::FILE)
    }

    /// Applies `edit` to what the file says NOW and writes it back, whole
    /// and atomically — so two doors editing two accounts keep both — and
    /// writes the default accounts for the shell ([`DEFAULT_ENV_FILE`]).
    /// Answers the list as written.
    ///
    /// A Claude Code account the edit removes has its status line given back
    /// BEFORE the list is written ([`statusline::unwrap`]); when it cannot
    /// be, nothing is written and the removal is refused with its reason.
    ///
    /// # Errors
    ///
    /// [`Why::Unreadable`] when the file is there and does not parse (it is
    /// never overwritten then), or a removed account's `settings.json` does
    /// not read while its line is saved; `edit`'s refusal; [`Why::Invalid`]
    /// when the edit leaves a list that is not one; [`Why::Failed`] when
    /// there is no home or a file could not be written.
    pub fn keep(edit: impl FnOnce(&mut Self) -> Result<(), Refusal>) -> Result<Self, Refusal> {
        Self::keep_in(&Roots::of_env(), edit)
    }

    /// [`Accounts::keep`], under given roots — for a caller that carries
    /// them (a test of a door's gestures) instead of the environment's.
    ///
    /// # Errors
    ///
    /// As [`Accounts::keep`].
    pub fn keep_in(
        roots: &Roots,
        edit: impl FnOnce(&mut Self) -> Result<(), Refusal>,
    ) -> Result<Self, Refusal> {
        let store = Self::store_in(roots.config.clone());
        if store.path().is_none() {
            return Err(Refusal::of(Why::Failed));
        }
        // `parsed` and never `read`: this writes the whole list back.
        let mut now = store.parsed().ok_or(Refusal::of(Why::Unreadable))?;
        if now.format != 1 {
            return Err(Refusal::of(Why::Unreadable));
        }
        let before = now.accounts.clone();
        edit(&mut now)?;
        now.check()?;
        // The removed ones give their status line back before the list
        // forgets where their folder is.
        for gone in before
            .iter()
            .filter(|a| a.kind == Kind::Claude && now.find(&a.id).is_none())
        {
            statusline::unwrap(gone)?;
        }
        store.write(&now).map_err(|_| Refusal::of(Why::Failed))?;
        if let (Some(config), Some(home)) = (&roots.config, &roots.home) {
            files::write_keeping(
                &config.join(DEFAULT_ENV_FILE),
                now.default_env(home).as_bytes(),
                0o644,
            )
            .map_err(|_| Refusal::of(Why::Failed))?;
        }
        Ok(now)
    }

    /// What the shell sources so `claude` and `codex` typed in a terminal
    /// use the default accounts: an `export` for a default that is not the
    /// agent's own folder, and an `unset` otherwise — for the agent's own
    /// folder (setting the variable to it would move Claude Code's
    /// `~/.claude.json` into the folder) and for a kind with no default.
    ///
    /// The `unset` is what makes a CHANGE of default reach a shell that
    /// inherited the old one: a terminal multiplexer started while another
    /// account was the default keeps its export, and sourcing a file that
    /// merely says nothing would leave every new window on that account.
    #[must_use]
    pub fn default_env(&self, home: &Path) -> String {
        let mut text = String::from(
            "# Written by ronoctua-llm from llm-accounts.json; the shell sources it.\n",
        );
        for kind in Kind::ALL {
            let elsewhere = self
                .default_of(kind)
                .filter(|a| a.dir != kind.native_dir(home) && fine_dir(&a.dir))
                .and_then(|a| a.dir.to_str());
            match elsewhere {
                Some(dir) => text.push_str(&format!(
                    "export {}='{}'\n",
                    kind.dir_variable(),
                    dir.replace('\'', r"'\''")
                )),
                None => text.push_str(&format!("unset {}\n", kind.dir_variable())),
            }
        }
        text
    }
}

/// A rate-limit window.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Window {
    /// How much of it is used, in per cent.
    pub used_percent: f32,
    /// When it resets, in seconds since the epoch.
    pub resets_at_s: i64,
}

/// Where this crate keeps and finds things; `None` keeps nothing.
///
/// Carried rather than found, so a test writes into a folder of its own and
/// never into the configuration of whoever ran it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Roots {
    /// `$XDG_CONFIG_HOME/ronoctua` (the accounts, the default env).
    pub config: Option<PathBuf>,
    /// `$XDG_STATE_HOME/ronoctua` (limits, usage).
    pub state: Option<PathBuf>,
    /// `$XDG_CACHE_HOME/ronoctua` (the token counts).
    pub cache: Option<PathBuf>,
    /// The home (the agents' own folders).
    pub home: Option<PathBuf>,
}

impl Roots {
    /// The machine's, from the environment.
    #[must_use]
    pub fn of_env() -> Self {
        Self {
            config: ronoctua_store::config_dir(),
            state: ronoctua_store::state_dir(),
            cache: ronoctua_xdg::cache_dir(),
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|h| h.is_absolute()),
        }
    }
}

/// What a watcher of an account looks at, to know its numbers moved — so
/// nobody outside this crate learns where the agents keep their logs, or
/// where the numbers are kept (`one-llm-boundary`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Watched {
    /// The files of ours its numbers are kept in: the limits the status line
    /// wrote and the usage endpoint's last answer (a Claude Code account's),
    /// and the token count. Each is REPLACED by a rename, and may not exist
    /// yet: a watcher watches its folder.
    pub files: Vec<PathBuf>,
    /// The agent's log trees — Claude Code's transcripts, Codex's session
    /// logs (which carry its limits too) — whose files grow while it works.
    /// Folders appear under them as it does, at most [`tokens::DEPTH`] deep;
    /// a tree may not exist yet either.
    pub trees: Vec<PathBuf>,
}

/// What to watch for `account`, under the machine's roots.
#[must_use]
pub fn watched(account: &Account) -> Watched {
    watched_in(&Roots::of_env(), account)
}

/// [`watched`], under given roots: the very paths the crate writes and
/// reads, from the functions that write and read them.
pub(crate) fn watched_in(roots: &Roots, account: &Account) -> Watched {
    let mut files = Vec::new();
    if account.kind == Kind::Claude
        && let Some(state) = &roots.state
    {
        files.extend(statusline::limits_path(state, &account.id));
        files.extend(usage::kept_path(state, account));
    }
    if let Some(cache) = &roots.cache {
        files.extend(tokens::kept_path(cache, account));
    }
    Watched {
        files,
        trees: tokens::trees_of(&account.dir, account.kind),
    }
}

/// An environment variable, as the process sees it — or as a test says.
type Lookup<'a> = &'a dyn Fn(&str) -> Option<std::ffi::OsString>;

/// The agent's program. The app layer self-updates (owner amendment
/// 2026-10-30): a NATIVE install under `$HOME` (`~/.local/bin/claude`,
/// `~/.local/bin/codex`) keeps itself current, and the footer PREFERS it. The
/// variable (`RONOCTUA_CLAUDE`, `RONOCTUA_CODEX`) is the SEED — the flake's
/// first binary, at a part's stable path — used to bootstrap and as the
/// fallback until the native install is there; a seed path that names nothing
/// YET is an agent on its way.
fn agent_program(kind: Kind, var: Lookup<'_>, home: Option<&Path>) -> Result<String, Refusal> {
    if let Some(home) = home {
        let native = home.join(".local/bin").join(kind.id());
        if native.is_file()
            && let Some(s) = native.to_str()
        {
            return Ok(s.to_owned());
        }
    }
    let path = var(kind.program_variable())
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .ok_or(Refusal::of(Why::NotInstalled))?;
    if !path.exists() {
        return Err(Refusal::of(Why::Arriving));
    }
    Some(path)
        .filter(|p| p.is_file())
        .and_then(|p| p.to_str().map(str::to_owned))
        .ok_or(Refusal::of(Why::NotInstalled))
}

/// The program and environment that open a session of `account` (`env
/// CLAUDE_CONFIG_DIR=…` or `CODEX_HOME=…`, then the agent's store path; the
/// agent's own folder is opened with the variable UNSET, so a value
/// inherited from elsewhere does not move it).
///
/// # Errors
///
/// [`Why::Invalid`] for an account that is not one; [`Why::NotInstalled`]
/// when the session was not handed the agent (`RONOCTUA_CLAUDE`,
/// `RONOCTUA_CODEX`); [`Why::Failed`] for a build without `RONOCTUA_LLM_ENV`.
pub fn launch_arguments(account: &Account) -> Result<Vec<String>, Refusal> {
    launch_from(
        account,
        &|name| std::env::var_os(name),
        option_env!("RONOCTUA_LLM_ENV"),
        Roots::of_env().home.as_deref(),
    )
}

/// [`launch_arguments`], with the environment, `env`'s store path and the
/// home given.
fn launch_from(
    account: &Account,
    var: Lookup<'_>,
    env: Option<&str>,
    home: Option<&Path>,
) -> Result<Vec<String>, Refusal> {
    let program = agent_program(account.kind, var, home)?;
    let env = env.ok_or(Refusal::of(Why::Failed))?;
    launch_with(account, &program, env, home)
}

/// The arguments that log `account` in: Codex's `login`; Claude Code asks
/// for a login itself when it starts in a folder that has none.
///
/// # Errors
///
/// As [`launch_arguments`].
pub fn login_arguments(account: &Account) -> Result<Vec<String>, Refusal> {
    let mut argv = launch_arguments(account)?;
    if account.kind == Kind::Codex {
        argv.push("login".to_owned());
    }
    Ok(argv)
}

/// [`launch_arguments`], with the programs and the home given.
fn launch_with(
    account: &Account,
    program: &str,
    env: &str,
    home: Option<&Path>,
) -> Result<Vec<String>, Refusal> {
    check_account(account)?;
    let dir = account.dir.to_str().ok_or(Refusal::of(Why::Invalid))?;
    let variable = account.kind.dir_variable();
    let mut argv = vec![env.to_owned()];
    if home.is_some_and(|h| account.dir == account.kind.native_dir(h)) {
        argv.extend(["-u".to_owned(), variable.to_owned()]);
    } else {
        argv.push(format!("{variable}={dir}"));
    }
    argv.push(program.to_owned());
    Ok(argv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-llm-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn roots(dir: &Path) -> Roots {
        Roots {
            config: Some(dir.join("config")),
            state: Some(dir.join("state")),
            cache: Some(dir.join("cache")),
            home: Some(dir.join("home")),
        }
    }

    #[test]
    fn the_accounts_file_crosses_and_every_why_has_its_id() {
        let accounts = Accounts {
            format: 1,
            accounts: vec![Account {
                id: "claude-1".to_owned(),
                kind: Kind::Claude,
                name: "Work".to_owned(),
                dir: "/home/p/.claude".into(),
                default: true,
            }],
        };
        let text = serde_json::to_string(&accounts).unwrap();
        assert_eq!(
            text,
            r#"{"format":1,"accounts":[{"id":"claude-1","kind":"claude","name":"Work","dir":"/home/p/.claude","default":true}]}"#
        );
        assert_eq!(serde_json::from_str::<Accounts>(&text).unwrap(), accounts);
        assert!(
            serde_json::from_str::<Accounts>(r#"{"format":1,"accounts":[],"extra":1}"#).is_err(),
            "an unknown field is refused"
        );
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
    }

    /// FAILS IF: an edit made in one door erases one made in another, a
    /// file that does not parse is overwritten, or a kind ends with two
    /// defaults (or none while it has accounts).
    #[test]
    fn edits_land_on_what_the_file_says_now() {
        let dir = scratch("keep");
        let roots = roots(&dir);
        let home = dir.join("home");
        let a = Accounts::default()
            .fresh(Kind::Claude, " Work ", &home)
            .unwrap();
        assert_eq!(
            (a.id.as_str(), a.dir.clone(), a.default, a.name.as_str()),
            ("claude-1", home.join(".claude"), true, "Work")
        );
        Accounts::keep_in(&roots, |list| list.add(a.clone())).unwrap();
        // Every edit reads the file anew: what another door wrote is the
        // list this one edits.
        let b = Accounts::keep_in(&roots, |list| {
            let b = list.fresh(Kind::Claude, "Home", &home)?;
            list.add(b)
        })
        .unwrap();
        assert_eq!(b.accounts.len(), 2);
        assert_eq!(b.accounts[1].id, "claude-2");
        assert_eq!(b.accounts[1].dir, home.join(".claude-2"));
        assert!(!b.accounts[1].default, "the first stays the default");

        let c = Accounts::keep_in(&roots, |list| {
            list.add(Account {
                id: "codex-1".to_owned(),
                kind: Kind::Codex,
                name: "Codex".to_owned(),
                dir: dir.join("elsewhere").join("codex"),
                default: false,
            })?;
            list.set_default("claude-2")?;
            list.rename("claude-1", "Office")
        })
        .unwrap();
        assert_eq!(c.default_of(Kind::Claude).unwrap().id, "claude-2");
        assert_eq!(
            c.default_of(Kind::Codex).unwrap().id,
            "codex-1",
            "first of its kind"
        );
        assert_eq!(c.find("claude-1").unwrap().name, "Office");

        // The shell's file says the two defaults that need a variable.
        let env = std::fs::read_to_string(dir.join("config").join(DEFAULT_ENV_FILE)).unwrap();
        assert!(env.contains(&format!(
            "export CLAUDE_CONFIG_DIR='{}'\n",
            home.join(".claude-2").display()
        )));
        assert!(env.contains("export CODEX_HOME='"));

        // Removing the default hands it to the next of its kind.
        let d = Accounts::keep_in(&roots, |list| list.remove("claude-2")).unwrap();
        assert_eq!(d.default_of(Kind::Claude).unwrap().id, "claude-1");
        let env = std::fs::read_to_string(dir.join("config").join(DEFAULT_ENV_FILE)).unwrap();
        assert!(
            env.contains("unset CLAUDE_CONFIG_DIR\n") && !env.contains("export CLAUDE_CONFIG_DIR"),
            "the agent's own folder: the variable unset, not pointed at it: {env}"
        );

        // Refusals change nothing.
        let before = std::fs::read(dir.join("config").join(Accounts::FILE)).unwrap();
        for (what, why) in [
            (
                Accounts::keep_in(&roots, |list| list.rename("nobody", "x")),
                Why::Unknown,
            ),
            (
                Accounts::keep_in(&roots, |list| list.rename("claude-1", "  ")),
                Why::Invalid,
            ),
            (
                Accounts::keep_in(&roots, |list| {
                    let mut twin = list.find("claude-1").unwrap().clone();
                    twin.id = "claude-9".to_owned();
                    list.add(twin)
                }),
                Why::Invalid,
            ),
        ] {
            assert_eq!(what.unwrap_err().why, why);
        }
        assert_eq!(
            std::fs::read(dir.join("config").join(Accounts::FILE)).unwrap(),
            before
        );

        std::fs::write(dir.join("config").join(Accounts::FILE), "{ broken").unwrap();
        assert_eq!(
            Accounts::keep_in(&roots, |list| list.remove("claude-1"))
                .unwrap_err()
                .why,
            Why::Unreadable
        );
        assert_eq!(
            std::fs::read_to_string(dir.join("config").join(Accounts::FILE)).unwrap(),
            "{ broken",
            "never overwritten"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: removing a wrapped Claude Code account leaves ours in its
    /// `settings.json` (the person's own line would never run again), or a
    /// removal whose line cannot be given back goes through anyway.
    #[test]
    fn removing_an_account_gives_its_status_line_back() {
        const SETTINGS: &str = "{\n  \"statusLine\": {\n    \"type\": \"command\",\n    \"command\": \"~/bin/line.sh\"\n  },\n  \"model\": \"opus\"\n}\n";
        let dir = scratch("remove");
        let roots = roots(&dir);
        let account = Account {
            id: "claude-2".to_owned(),
            kind: Kind::Claude,
            name: "Home".to_owned(),
            dir: dir.join("home").join(".claude-2"),
            default: true,
        };
        std::fs::create_dir_all(&account.dir).unwrap();
        std::fs::write(account.dir.join(statusline::SETTINGS_FILE), SETTINGS).unwrap();
        Accounts::keep_in(&roots, |list| list.add(account.clone())).unwrap();
        statusline::wrap_as(&account, None).unwrap();
        assert_eq!(
            statusline::wrapped(&account).unwrap(),
            statusline::Wrapping::Ours
        );

        // The person is editing the file: the line cannot go back, so the
        // account stays, and so does its saved line.
        std::fs::write(account.dir.join(statusline::SETTINGS_FILE), "{ \"model\": ").unwrap();
        assert_eq!(
            Accounts::keep_in(&roots, |list| list.remove("claude-2"))
                .unwrap_err()
                .why,
            Why::Unreadable
        );
        assert!(account.dir.join(statusline::SAVED_FILE).exists());
        let kept = Accounts::store_in(roots.config.clone()).read();
        assert!(kept.find("claude-2").is_some(), "the removal was refused");

        // The file whole again: the removal gives the person's line back,
        // byte for byte, and forgets the saved copy.
        std::fs::write(account.dir.join(statusline::SETTINGS_FILE), SETTINGS).unwrap();
        statusline::wrap_as(&account, None).unwrap();
        let after = Accounts::keep_in(&roots, |list| list.remove("claude-2")).unwrap();
        assert!(after.find("claude-2").is_none());
        assert_eq!(
            std::fs::read_to_string(account.dir.join(statusline::SETTINGS_FILE)).unwrap(),
            SETTINGS
        );
        assert!(!account.dir.join(statusline::SAVED_FILE).exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a shell that inherited the old default (a terminal
    /// multiplexer started before the change) keeps it after sourcing the
    /// file — the default moved back to the agent's own folder, or the kind
    /// has no account any more.
    #[test]
    fn the_default_file_takes_an_inherited_folder_away() {
        let home = Path::new("/home/p");
        let mut list = Accounts::default();
        list.add(Account {
            id: "claude-1".to_owned(),
            kind: Kind::Claude,
            name: "Work".to_owned(),
            dir: home.join(".claude"),
            default: true,
        })
        .unwrap();
        let text = list.default_env(home);
        assert!(text.contains("unset CLAUDE_CONFIG_DIR\n"), "{text}");
        assert!(
            text.contains("unset CODEX_HOME\n"),
            "no Codex account: {text}"
        );
        let dir = scratch("env");
        let file = dir.join(DEFAULT_ENV_FILE);
        std::fs::write(&file, &text).unwrap();
        let shell = option_env!("RONOCTUA_LLM_SH").unwrap_or("/bin/sh");
        let said = std::process::Command::new(shell)
            .arg("-c")
            .arg(r#". "$1"; printf '%s|%s' "${CLAUDE_CONFIG_DIR-none}" "${CODEX_HOME-none}""#)
            .arg("sh")
            .arg(&file)
            .env("CLAUDE_CONFIG_DIR", "/home/p/.claude-2")
            .env("CODEX_HOME", "/home/p/.codex-2")
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&said.stdout), "none|none");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a new account is offered a folder that already exists on
    /// disk — another program's, or one left by an account removed earlier
    /// with the person's sessions in it.
    #[test]
    fn a_fresh_account_never_takes_a_folder_that_exists() {
        let dir = scratch("fresh");
        let home = dir.join("home");
        std::fs::create_dir_all(home.join(".claude-2")).unwrap();
        let mut list = Accounts::default();
        list.add(list.fresh(Kind::Claude, "Work", &home).unwrap())
            .unwrap();
        let next = list.fresh(Kind::Claude, "Home", &home).unwrap();
        assert_eq!(
            (next.id.as_str(), next.dir.clone(), next.default),
            ("claude-2", home.join(".claude-3"), false)
        );
        let codex = list.fresh(Kind::Codex, "Codex", &home).unwrap();
        assert_eq!(
            (codex.id.as_str(), codex.dir, codex.default),
            ("codex-1", home.join(".codex"), true)
        );
        assert_eq!(
            list.fresh(Kind::Claude, " ", &home).unwrap_err().why,
            Why::Invalid
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the agent's program is taken from anywhere but the
    /// variable the session hands it in, or a value that is not a program's
    /// absolute path is run.
    #[test]
    fn the_agent_is_the_one_the_session_hands_over() {
        let dir = scratch("program");
        let claude = dir.join("claude");
        std::fs::write(&claude, b"#!/bin/sh\n").unwrap();
        let account = Account {
            id: "claude-1".to_owned(),
            kind: Kind::Claude,
            name: "Work".to_owned(),
            dir: dir.join(".claude-2"),
            default: true,
        };
        let path = claude.to_str().unwrap().to_owned();
        let given = |value: Option<&str>| {
            let value = value.map(std::ffi::OsString::from);
            move |name: &str| (name == "RONOCTUA_CLAUDE").then(|| value.clone()).flatten()
        };
        assert_eq!(
            launch_from(
                &account,
                &given(Some(&path)),
                Some("/e"),
                Some(dir.as_path())
            )
            .unwrap(),
            [
                "/e".to_owned(),
                format!("CLAUDE_CONFIG_DIR={}", account.dir.display()),
                path.clone()
            ]
        );
        // A PATH THAT NAMES NOTHING YET is an agent on its way, not one
        // that is not installed: the variable is set where it is wanted.
        assert_eq!(
            launch_from(
                &account,
                &given(Some("/nowhere/claude")),
                Some("/e"),
                Some(dir.as_path())
            )
            .unwrap_err()
            .why,
            Why::Arriving
        );
        for wrong in [None, Some("claude"), Some(dir.to_str().unwrap())] {
            assert_eq!(
                launch_from(&account, &given(wrong), Some("/e"), Some(dir.as_path()))
                    .unwrap_err()
                    .why,
                Why::NotInstalled,
                "{wrong:?}"
            );
        }
        let codex = Account {
            kind: Kind::Codex,
            ..account.clone()
        };
        assert_eq!(
            launch_from(&codex, &given(Some(&path)), Some("/e"), Some(dir.as_path()))
                .unwrap_err()
                .why,
            Why::NotInstalled,
            "Claude Code's variable is not Codex's"
        );
        assert_eq!(
            launch_from(&account, &given(Some(&path)), None, Some(dir.as_path()))
                .unwrap_err()
                .why,
            Why::Failed,
            "a build without env"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: what a watcher is told to watch is not where the crate
    /// writes the numbers and reads the logs.
    #[test]
    fn what_is_watched_is_what_is_written_and_read() {
        let dir = scratch("watched");
        let roots = roots(&dir);
        let claude = Account {
            id: "claude-1".to_owned(),
            kind: Kind::Claude,
            name: "Work".to_owned(),
            dir: dir.join("home").join(".claude"),
            default: true,
        };
        let codex = Account {
            id: "codex-1".to_owned(),
            kind: Kind::Codex,
            name: "Home".to_owned(),
            dir: dir.join("home").join(".codex"),
            default: true,
        };
        let state = roots.state.as_deref().unwrap();
        let cache = roots.cache.as_deref().unwrap();
        usage::keep_in(state, &claude, Err(Refusal::of(Why::Offline)), 1).unwrap();
        tokens::keep_in(cache, &claude, &tokens::Cache::default()).unwrap();
        tokens::keep_in(cache, &codex, &tokens::Cache::default()).unwrap();
        // A log in every tree watched is a log counted.
        for account in [&claude, &codex] {
            let watched = watched_in(&roots, account);
            for tree in &watched.trees {
                let day = tree.join("2026").join("10").join("05");
                std::fs::create_dir_all(&day).unwrap();
                std::fs::write(
                    day.join(format!(
                        "rollout-{}.jsonl",
                        tree.display().to_string().len()
                    )),
                    b"",
                )
                .unwrap();
            }
            let mut counted = tokens::Cache::default();
            tokens::count(&account.dir, account.kind, &mut counted, 1_759_665_600).unwrap();
            assert_eq!(counted.offsets.len(), watched.trees.len(), "{account:?}");
        }
        let watched = watched_in(&roots, &claude);
        assert_eq!(watched.files.len(), 3);
        assert!(watched.files[0].ends_with("llm/claude-1/limits.json"));
        assert!(watched.files[1].exists(), "usage.json is where it is kept");
        assert!(watched.files[2].exists(), "tokens.json is where it is kept");
        assert_eq!(watched_in(&roots, &codex).files.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn ids_names_and_folders_are_checked() {
        for id in ["claude-1", "a", "9x", "codex-12"] {
            assert!(fine_id(id), "{id}");
        }
        for id in ["", "-a", "A", "a/b", "..", "a.b", &"x".repeat(33)] {
            assert!(!fine_id(id), "{id}");
        }
        assert!(fine_name("Trabalho ✓"));
        assert!(!fine_name("two\nlines"));
        assert!(!fine_name(&"x".repeat(MOST_NAME_CHARS + 1)));
        assert!(fine_dir(Path::new("/home/p/.claude-2")));
        assert!(!fine_dir(Path::new("relative")));
        assert!(!fine_dir(Path::new("/home/p/../q")));
        assert_eq!(
            id_folder(Path::new("/s"), "claude-1"),
            Some(PathBuf::from("/s/llm/claude-1"))
        );
        assert_eq!(id_folder(Path::new("/s"), "../x"), None);
        let quoted = Accounts {
            format: 1,
            accounts: vec![Account {
                id: "claude-2".to_owned(),
                kind: Kind::Claude,
                name: "Q".to_owned(),
                dir: "/home/p/it's".into(),
                default: true,
            }],
        };
        assert!(
            quoted
                .default_env(Path::new("/home/p"))
                .contains(r"export CLAUDE_CONFIG_DIR='/home/p/it'\''s'"),
            "a quote cannot end the value"
        );
    }

    #[test]
    fn a_session_is_opened_in_the_accounts_folder() {
        let home = Path::new("/home/p");
        let mut account = Account {
            id: "claude-2".to_owned(),
            kind: Kind::Claude,
            name: "Home".to_owned(),
            dir: home.join(".claude-2"),
            default: false,
        };
        assert_eq!(
            launch_with(
                &account,
                "/nix/store/c/bin/claude",
                "/nix/store/e/bin/env",
                Some(home)
            )
            .unwrap(),
            [
                "/nix/store/e/bin/env",
                "CLAUDE_CONFIG_DIR=/home/p/.claude-2",
                "/nix/store/c/bin/claude"
            ]
        );
        account.dir = home.join(".claude");
        assert_eq!(
            launch_with(&account, "/c", "/e", Some(home)).unwrap(),
            ["/e", "-u", "CLAUDE_CONFIG_DIR", "/c"],
            "the agent's own folder: the variable unset"
        );
        account.kind = Kind::Codex;
        account.dir = home.join("work-codex");
        assert_eq!(
            launch_with(&account, "/x", "/e", Some(home)).unwrap(),
            ["/e", "CODEX_HOME=/home/p/work-codex", "/x"]
        );
        account.dir = "relative".into();
        assert_eq!(
            launch_with(&account, "/x", "/e", Some(home))
                .unwrap_err()
                .why,
            Why::Invalid
        );
    }
}
