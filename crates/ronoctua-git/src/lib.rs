//! git — the one crate that runs it.
//!
//! # Why hardened
//!
//! A folder somebody downloaded can carry a `.git/config` that RUNS A
//! COMMAND the moment git reads that repository — and the file manager
//! reads every repository it lists. MEASURED (git 2.52, and again on the pinned
//! 2.54): a `filter.<driver>.clean` in the repository's config and a `.gitattributes`
//! naming the driver make a plain `git status` run it. So every call goes
//! through [`base`], which turns each such door off on the command line (`-c`
//! is read after the repository's config, so it wins):
//!
//! | Runs a command | Closed by |
//! |---|---|
//! | `core.fsmonitor` | `-c core.fsmonitor=false` |
//! | `core.pager`, `pager.*` | `-c core.pager=cat` (and no terminal) |
//! | `diff.external`, `diff.<driver>.command` | `-c diff.external=`, `--no-ext-diff` |
//! | `diff.<driver>.textconv` | `--no-textconv` (diff family and blame) |
//! | hooks (`reference-transaction` on a fetch…) | `-c core.hooksPath=/dev/null` |
//! | `credential.helper`, `core.askPass` | both emptied |
//! | `filter.<driver>.clean/smudge/process` | emptied, by name, as bytes ([`Repo::command`]) |
//! | `core.sshCommand` | `GIT_SSH_COMMAND` always set (the environment wins) |
//! | `core.gitProxy` | `GIT_PROXY_COMMAND` set empty (read first, it wins) |
//! | `ext::` remotes, `remote.<name>.vcs` helpers | `GIT_ALLOW_PROTOCOL`, `-c protocol.ext.allow=never` |
//! | `remote.<name>.uploadpack` | `fetch --upload-pack=git-upload-pack` |
//! | the same, through a partial clone's lazy fetch | `GIT_NO_LAZY_FETCH=1` |
//! | `maintenance.*` after a fetch | `-c maintenance.auto=false` |
//! | `core.alternateRefsCommand` | `-c core.alternateRefsCommand=exit 0` |
//! | `log.showSignature` (`gpg.program`) | `-c log.showSignature=false` |
//! | submodules (their own config) | never recursed into |
//!
//! Every door in this table is shown OPEN on the pinned git, unhardened, and
//! shut through this crate, by `a_hostile_repository_runs_nothing`.
//!
//! It also never lets git prompt, takes the program from its store path
//! ([`Tools`]), reads pathspecs literally (`GIT_LITERAL_PATHSPECS`), never
//! colours, and never takes an optional lock (a status does not write the
//! index under the person's feet). A commit id handed in is checked to be
//! one before it becomes an argument.
//!
//! # Credentials
//!
//! SSH through `GIT_SSH_COMMAND` with the person's key, `IdentitiesOnly`,
//! `BatchMode` and a known-hosts file of our own; HTTPS through `GIT_ASKPASS`
//! (`ronoctua-git-askpass`) with only the PATH of a `0600` token file in the
//! environment ([`Credentials`]). The secret itself never reaches argv or
//! the environment.
//!
//! # Read-only where the person reads
//!
//! The file manager's line and browser ([`Repo::status`], [`Repo::log`],
//! [`Repo::show`], [`Repo::diff`], [`Repo::blame`]…) change nothing; the one
//! network call, [`Repo::fetch`], never prompts — a remote that needs
//! credentials it was not given is a refusal that says so. Every call blocks
//! until git ends: a thread, never the event loop.

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub mod askpass;
mod parse;

/// The variable naming git's store path.
pub const GIT_VARIABLE: &str = "RONOCTUA_GIT";
/// The variable naming ssh's store path.
pub const SSH_VARIABLE: &str = "RONOCTUA_SSH";
/// The variable naming the askpass helper's store path (`nix/rust.nix`
/// compiles it in; the environment wins).
pub const ASKPASS_VARIABLE: &str = "RONOCTUA_GIT_ASKPASS";
/// The variable the askpass helper reads: the PATH of the token file.
pub const SECRET_VARIABLE: &str = "RONOCTUA_GIT_SECRET";
/// The variable the askpass helper answers a user-name prompt with.
pub const USER_VARIABLE: &str = "RONOCTUA_GIT_USER";
/// The configuration every call overrides, in `-c` pairs: nothing the
/// repository's config names is run (see the table at the top).
pub const HARDENING: [&str; 28] = [
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.pager=cat",
    "-c",
    "diff.external=",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "credential.helper=",
    "-c",
    "core.askPass=",
    "-c",
    "protocol.ext.allow=never",
    "-c",
    "core.alternateRefsCommand=exit 0",
    "-c",
    "log.showSignature=false",
    "-c",
    "color.ui=never",
    "-c",
    "submodule.recurse=false",
    "-c",
    "fetch.recurseSubmodules=false",
    "-c",
    "gc.auto=0",
    "-c",
    "maintenance.auto=false",
];

/// What a diff-family subcommand also takes: no external diff, no textconv
/// driver. They are the subcommand's options, not git's, so [`subcommand`]
/// adds them after its name.
pub const DIFF_GUARDS: [&str; 2] = ["--no-ext-diff", "--no-textconv"];

/// The transports a call may use (`GIT_ALLOW_PROTOCOL`): the environment
/// beats every `protocol.*.allow` of the repository's config, so neither
/// `ext::` nor a `remote.<name>.vcs` helper is ever run.
pub const PROTOCOLS: &str = "file:git:http:https:ssh";

/// The subcommands that take [`DIFF_GUARDS`].
pub const DIFF_FAMILY: [&str; 6] = [
    "diff",
    "show",
    "log",
    "diff-tree",
    "diff-index",
    "diff-files",
];

/// What `blame` takes: it runs textconv drivers too, and has no external
/// diff to turn off.
pub const BLAME_GUARDS: [&str; 1] = ["--no-textconv"];

/// Inherited variables that would point git elsewhere or run something:
/// removed from every call.
pub const UNSET: [&str; 22] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_EXEC_PATH",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_EXTERNAL_DIFF",
    "GIT_PAGER",
    "GIT_ASKPASS",
    "SSH_ASKPASS",
    "GIT_SSH",
    "GIT_SSH_COMMAND",
    "GIT_EDITOR",
    "GIT_ATTR_SOURCE",
    "GIT_GLOB_PATHSPECS",
    "GIT_NOGLOB_PATHSPECS",
    "GIT_ICASE_PATHSPECS",
    "GIT_REPLACE_REF_BASE",
];

/// How long ssh waits for a host that does not answer, in seconds.
pub const SSH_CONNECT_TIMEOUT_S: u32 = 20;

/// A program's path: this process's environment first (a test's, a unit's),
/// then what `nix/rust.nix` compiled in.
fn program_path(variable: &str, built: Option<&'static str>) -> Option<PathBuf> {
    std::env::var_os(variable)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| built.filter(|p| !p.is_empty()).map(PathBuf::from))
}

/// Where the programs are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    /// git.
    pub git: PathBuf,
    /// ssh, when this process was given it (a remote over SSH needs it).
    pub ssh: Option<PathBuf>,
    /// Our askpass helper, when given (a remote over HTTPS with a token).
    pub askpass: Option<PathBuf>,
}

impl Tools {
    /// The tools this process was given: each variable from the
    /// environment, else as `nix/rust.nix` compiled it in — the askpass
    /// helper too, which is built there on its own (from this crate's
    /// sources alone) so that any program linking this crate, extracted or
    /// not, finds it. `None` without git — never a bare name: a unit's
    /// `PATH` is minimal.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        Some(Self {
            git: program_path(GIT_VARIABLE, option_env!("RONOCTUA_GIT"))?,
            ssh: program_path(SSH_VARIABLE, option_env!("RONOCTUA_SSH")),
            askpass: program_path(ASKPASS_VARIABLE, option_env!("RONOCTUA_GIT_ASKPASS")),
        })
    }
}

/// How a remote is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Credentials {
    /// SSH with the person's key (a deploy key, for the belongings).
    Ssh {
        /// The private key (`0600`).
        key: PathBuf,
        /// Our own known-hosts file (new hosts accepted, changed ones refused).
        known_hosts: PathBuf,
    },
    /// HTTPS with a token kept in a `0600` file; only its path travels.
    Token {
        /// The token's file.
        file: PathBuf,
        /// The user name a prompt is answered with, when the host wants one.
        user: Option<String>,
    },
}

/// Why git answered nothing, as a closed set (`files-git-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Why {
    /// This process was not given git.
    NoGit,
    /// SSH credentials and no ssh.
    NoSsh,
    /// Token credentials and no askpass helper.
    NoAskpass,
    /// The folder is not in a repository (any more).
    NotARepo,
    /// git could not be started.
    Unstartable,
    /// git ended with a failure.
    Failed,
    /// git answered something the parser does not know — or what was handed
    /// in is not a commit id.
    Garbled,
    /// The remote wants credentials that were not given.
    NeedsCredentials,
    /// The remote refused the credentials.
    Refused,
    /// The remote could not be reached.
    Unreachable,
    /// The answer is larger than a view shows.
    TooLarge,
}

impl Why {
    /// Every refusal, for the keys' census.
    pub const ALL: [Self; 11] = [
        Self::NoGit,
        Self::NoSsh,
        Self::NoAskpass,
        Self::NotARepo,
        Self::Unstartable,
        Self::Failed,
        Self::Garbled,
        Self::NeedsCredentials,
        Self::Refused,
        Self::Unreachable,
        Self::TooLarge,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoGit => "no-git",
            Self::NoSsh => "no-ssh",
            Self::NoAskpass => "no-askpass",
            Self::NotARepo => "not-a-repo",
            Self::Unstartable => "unstartable",
            Self::Failed => "failed",
            Self::Garbled => "garbled",
            Self::NeedsCredentials => "needs-credentials",
            Self::Refused => "refused",
            Self::Unreachable => "unreachable",
            Self::TooLarge => "too-large",
        }
    }
}

/// A refusal of git.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

const fn refused(why: Why) -> Refusal {
    Refusal { why }
}

/// Text quoted for the shell git runs `GIT_SSH_COMMAND` through.
fn quoted(text: &OsStr) -> String {
    format!("'{}'", text.to_string_lossy().replace('\'', r"'\''"))
}

/// The one hardened command every call starts from: git by its store path,
/// in `repo` (`-C`), with [`HARDENING`], no prompt (`GIT_TERMINAL_PROMPT=0`),
/// no optional lock (`GIT_OPTIONAL_LOCKS=0`), literal pathspecs, no lazy
/// fetch, only [`PROTOCOLS`], [`UNSET`] removed, stdin closed — and the
/// credentials' environment. Add the subcommand with [`subcommand`].
///
/// # Errors
///
/// [`Why::NoSsh`] for SSH credentials without ssh, [`Why::NoAskpass`] for a
/// token without the helper.
pub fn base(
    tools: &Tools,
    repo: &Path,
    credentials: Option<&Credentials>,
) -> Result<Command, Refusal> {
    let mut command = Command::new(&tools.git);
    command.arg("-C").arg(repo).args(HARDENING);
    for name in UNSET {
        command.env_remove(name);
    }
    command
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_LITERAL_PATHSPECS", "1")
        // A PARTIAL CLONE fetches a missing object on demand, and that
        // fetch is a child `git fetch` that runs the repository's
        // `remote.<name>.uploadpack` (MEASURED, through `Repo::diff`). With
        // this, a missing object is a failure and nothing is fetched.
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", PROTOCOLS)
        // Read before the configuration and kept when set: empty, it names
        // no proxy, and the repository's `core.gitProxy` is never consulted.
        .env("GIT_PROXY_COMMAND", "")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .stdin(Stdio::null());
    // ALWAYS SET: the environment beats the repository's `core.sshCommand`.
    let ssh = tools
        .ssh
        .as_deref()
        .map_or_else(|| "ssh".to_owned(), |s| quoted(s.as_os_str()));
    let timeout = format!("-o ConnectTimeout={SSH_CONNECT_TIMEOUT_S}");
    match credentials {
        Some(Credentials::Ssh { key, known_hosts }) => {
            if tools.ssh.is_none() {
                return Err(refused(Why::NoSsh));
            }
            command.env(
                "GIT_SSH_COMMAND",
                format!(
                    "{ssh} -i {} -o IdentitiesOnly=yes -o BatchMode=yes -o UserKnownHostsFile={} \
                     -o StrictHostKeyChecking=accept-new {timeout} -F none",
                    quoted(key.as_os_str()),
                    quoted(known_hosts.as_os_str()),
                ),
            );
        }
        Some(Credentials::Token { file, user }) => {
            let askpass = tools.askpass.as_deref().ok_or(refused(Why::NoAskpass))?;
            command
                .env(
                    "GIT_SSH_COMMAND",
                    format!("{ssh} -o BatchMode=yes {timeout}"),
                )
                .env("GIT_ASKPASS", askpass)
                .env(SECRET_VARIABLE, file);
            if let Some(user) = user {
                command.env(USER_VARIABLE, user);
            }
        }
        // No credentials: the person's own SSH setup, but never a question.
        None => {
            command.env(
                "GIT_SSH_COMMAND",
                format!("{ssh} -o BatchMode=yes {timeout}"),
            );
        }
    }
    Ok(command)
}

/// Adds the subcommand `name` to a [`base`] command — and [`DIFF_GUARDS`]
/// when it is of the [`DIFF_FAMILY`], [`BLAME_GUARDS`] for `blame`.
pub fn subcommand<'a>(command: &'a mut Command, name: &str) -> &'a mut Command {
    command.arg(name);
    if DIFF_FAMILY.contains(&name) {
        command.args(DIFF_GUARDS);
    }
    if name == "blame" {
        command.args(BLAME_GUARDS);
    }
    command
}

/// What a finished git said.
struct Ran {
    ok: bool,
    out: Vec<u8>,
    err: String,
}

/// The largest stderr kept to read a refusal from.
const ERR_MOST_BYTES: usize = 64 * 1024;

/// Runs `command` to its end, stdout capped at `most` bytes (more is
/// [`Why::TooLarge`], and git is stopped).
fn run(mut command: Command, most: usize) -> Result<Ran, Refusal> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| refused(Why::Unstartable))?;
    let (Some(mut out), Some(mut err)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(refused(Why::Unstartable));
    };
    // stderr on its own thread, drained to the end so git never blocks on it.
    let errors = std::thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buffer = [0u8; 4096];
        while let Ok(n) = err.read(&mut buffer) {
            if n == 0 {
                break;
            }
            if kept.len() < ERR_MOST_BYTES {
                kept.extend_from_slice(&buffer[..n]);
            }
        }
        String::from_utf8_lossy(&kept).into_owned()
    });
    let mut bytes = Vec::new();
    let read = (&mut out)
        .take(u64::try_from(most).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes);
    let too_large = bytes.len() > most;
    if too_large {
        let _ = child.kill();
    }
    drop(out);
    let status = child.wait().map_err(|_| refused(Why::Failed))?;
    let err = errors.join().unwrap_or_default();
    if too_large {
        return Err(refused(Why::TooLarge));
    }
    read.map_err(|_| refused(Why::Failed))?;
    Ok(Ran {
        ok: status.success(),
        out: bytes,
        err,
    })
}

/// What a failed git's stderr means; `network` for a call that reached a
/// remote (only there is a "permission denied" the remote's). Pure.
#[must_use]
fn verdict(err: &str, network: bool) -> Why {
    let says = |words: &[&str]| words.iter().any(|w| err.contains(w));
    if says(&["not a git repository"]) {
        Why::NotARepo
    } else if !network {
        Why::Failed
    } else if says(&[
        "terminal prompts disabled",
        "could not read Username",
        "could not read Password",
    ]) {
        Why::NeedsCredentials
    } else if says(&[
        "Permission denied",
        "Authentication failed",
        "Host key verification failed",
        "HTTP Basic: Access denied",
        "The requested URL returned error: 403",
        "The requested URL returned error: 401",
    ]) {
        Why::Refused
    } else if says(&[
        "Could not resolve host",
        "Connection refused",
        "Connection timed out",
        "Network is unreachable",
        "No route to host",
        "Failed to connect",
        "Could not read from remote repository",
        "unable to access",
        "does not appear to be a git repository",
    ]) {
        Why::Unreachable
    } else {
        Why::Failed
    }
}
/// A commit's id, as git prints it (hexadecimal, SHA-1 or SHA-256).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Oid(pub String);

/// The state of the work tree — the file manager's git line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Status {
    /// The branch; `None` when HEAD is detached.
    pub branch: Option<String>,
    /// The commit HEAD is on; `None` on an unborn branch.
    pub head: Option<Oid>,
    /// The upstream, when there is one.
    pub upstream: Option<String>,
    /// Commits ahead of the upstream.
    pub ahead: u32,
    /// Commits behind it.
    pub behind: u32,
    /// Paths with staged changes.
    pub staged: u32,
    /// Paths changed and not staged.
    pub changed: u32,
    /// Untracked paths.
    pub untracked: u32,
    /// Paths in conflict.
    pub conflicted: u32,
    /// Every path the counts count, in git's order — the browser's Changes.
    pub entries: Vec<Entry>,
}

/// A path the work tree's state names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path, relative to the repository (bytes: any name git accepts).
    pub path: PathBuf,
    /// What is different about it.
    pub state: EntryState,
}

/// What is different about a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryState {
    /// Known to git: what is staged, and what is changed and not staged.
    Tracked {
        /// The change in the index against HEAD.
        staged: Option<Change>,
        /// The change in the work tree against the index.
        unstaged: Option<Change>,
    },
    /// In conflict (a merge or a rebase stopped here).
    Conflicted,
    /// Not known to git.
    Untracked,
}

/// One commit of a history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    /// Its id.
    pub id: Oid,
    /// Its parents (none for a root, two for a merge).
    pub parents: Vec<Oid>,
    /// Its author's name — data.
    pub author: String,
    /// When it was authored, seconds since the epoch.
    pub at_s: i64,
    /// Its subject — data.
    pub subject: String,
}

/// How a path changed in a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// Added.
    Added,
    /// Modified.
    Modified,
    /// Deleted.
    Deleted,
    /// Renamed from a path.
    Renamed {
        /// The old path.
        from: PathBuf,
    },
    /// Copied from a path.
    Copied {
        /// The source path.
        from: PathBuf,
    },
    /// Its type changed (file, link, submodule).
    TypeChanged,
    /// Unmerged.
    Unmerged,
}

/// A commit of one path's history ([`Repo::file_history_named`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCommit {
    /// The commit.
    pub commit: Commit,
    /// The path's name in it — before a rename, the old one — or `None`
    /// where git printed no change of it (a merge).
    pub name: Option<PathBuf>,
}

/// A changed path of a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    /// The path, relative to the repository.
    pub path: PathBuf,
    /// How it changed.
    pub change: Change,
}

/// A commit, whole — the browser's detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitDetail {
    /// Its row.
    pub commit: Commit,
    /// The whole message — data.
    pub message: String,
    /// What it changed (`--root` for the first commit).
    pub files: Vec<ChangedFile>,
}

/// One line of a blame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    /// The commit the line comes from.
    pub commit: Oid,
    /// Its author — data.
    pub author: String,
    /// When, seconds since the epoch.
    pub at_s: i64,
    /// The line's number, from 1.
    pub number: u32,
    /// The line — data.
    pub text: String,
}

/// What kind of ref.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RefKind {
    /// A local branch.
    Branch,
    /// A remote-tracking branch.
    Remote,
    /// A tag.
    Tag,
}

/// A branch or a tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    /// Its short name.
    pub name: String,
    /// What kind.
    pub kind: RefKind,
    /// The commit it points at.
    pub target: Oid,
    /// Its upstream, for a local branch that has one.
    pub upstream: Option<String>,
    /// The subject of its commit — data.
    pub subject: String,
    /// When its commit was made, seconds since the epoch.
    pub at_s: i64,
}

/// A stash entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stash {
    /// Its index (`stash@{n}`).
    pub index: u32,
    /// Its commit.
    pub id: Oid,
    /// Its message — data.
    pub subject: String,
}

/// A remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    /// Its name.
    pub name: String,
    /// Where it fetches from.
    pub fetch_url: String,
    /// Where it pushes to, when different.
    pub push_url: Option<String>,
}

/// A repository: the work tree and its git directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repo {
    /// The work tree's root.
    pub root: PathBuf,
    /// The git directory (`.git`, or where a `gitdir:` file points).
    pub git_dir: PathBuf,
    tools: Option<Tools>,
}

/// The longest `.git` FILE read (a worktree's or submodule's `gitdir:`).
const GITDIR_FILE_MOST_BYTES: u64 = 4096;

/// The most a listing answer may be (a status, a page of history, the refs).
pub const LISTING_MOST_BYTES: usize = 16 * 1024 * 1024;

/// The most a diff or a blame may be before it is [`Why::TooLarge`]: a view
/// shows a page, and a generated file's diff can be hundreds of megabytes.
pub const TEXT_MOST_BYTES: usize = 4 * 1024 * 1024;

/// A commit id as an argument, or [`Why::Garbled`]: what is handed in
/// becomes an argument only if it is hexadecimal (never `--output=…`).
fn commit_arg(commit: &Oid) -> Result<&str, Refusal> {
    parse::is_oid(&commit.0, true)
        .then_some(commit.0.as_str())
        .ok_or(refused(Why::Garbled))
}

/// The pages git counts in, as arguments.
fn page(skip: usize, most: usize) -> [String; 2] {
    [format!("--skip={skip}"), format!("--max-count={most}")]
}

/// Keeps git inside `root`: without this, a `.git` that is not a
/// repository (left empty, half deleted) makes git walk up and answer for
/// whatever repository holds the folder — the wrong one, silently.
fn ceiling(command: &mut Command, root: &Path) {
    if let Some(parent) = root.parent().filter(|p| p.is_absolute()) {
        command.env("GIT_CEILING_DIRECTORIES", parent);
    }
}

impl Repo {
    /// The repository `path` is in, found by walking up for `.git` (a folder,
    /// or a file that says `gitdir: …`) — in Rust, so git is started only
    /// inside a repository. The tools come from the environment.
    #[must_use]
    pub fn discover(path: &Path) -> Option<Self> {
        let absolute;
        let path = if path.is_absolute() {
            path
        } else {
            absolute = std::env::current_dir().ok()?.join(path);
            &absolute
        };
        for dir in path.ancestors() {
            let dot = dir.join(".git");
            let Ok(meta) = std::fs::metadata(&dot) else {
                continue;
            };
            let git_dir = if meta.is_dir() {
                dot
            } else if meta.is_file() && meta.len() <= GITDIR_FILE_MOST_BYTES {
                let text = std::fs::read_to_string(&dot).ok()?;
                let named = text.strip_prefix("gitdir:")?.trim();
                dir.join(named)
            } else {
                continue;
            };
            return Some(Self {
                root: dir.to_path_buf(),
                git_dir,
                tools: Tools::from_env(),
            });
        }
        None
    }

    /// The same repository, run with these tools (a test's).
    #[must_use]
    pub fn with_tools(mut self, tools: Tools) -> Self {
        self.tools = Some(tools);
        self
    }

    /// A [`base`] command in this repository — with every filter driver its
    /// configuration names emptied, by name: a `filter.<name>.clean` runs
    /// on a plain `status` (see the table at the top). Reading the names is
    /// itself a hardened call, and `git config` runs nothing.
    ///
    /// THROUGH `GIT_CONFIG_KEY_<n>`, NEVER `-c`. git splits a `-c` at its
    /// first `=`, so for a driver named `x=y` the pair `filter.x=y.clean=`
    /// sets `filter.x` and leaves the driver's command in place; and a name
    /// that is not UTF-8 cannot be written into a `String` at all. MEASURED,
    /// both ran their clean filter on a `status`. The environment's key is
    /// the bytes, whole.
    ///
    /// # Errors
    ///
    /// [`Why::NoGit`], what [`base`] refuses — and whatever kept the names
    /// from being read: without the list, no call is made.
    pub fn command(&self, credentials: Option<&Credentials>) -> Result<Command, Refusal> {
        use std::os::unix::ffi::OsStringExt as _;
        let tools = self.tools.as_ref().ok_or(refused(Why::NoGit))?;
        let mut asking = base(tools, &self.root, None)?;
        ceiling(&mut asking, &self.root);
        asking.args([
            "config",
            "--null",
            "--name-only",
            "--get-regexp",
            r"^filter\.",
        ]);
        let ran = run(asking, LISTING_MOST_BYTES)?;
        // No match is exit 1 with nothing said: no drivers. Anything said is
        // a failure, and the call is not made blind.
        if !ran.ok && !ran.err.is_empty() {
            return Err(refused(verdict(&ran.err, false)));
        }
        let drivers = parse::filter_drivers(&ran.out).ok_or(refused(Why::Garbled))?;
        let mut command = base(tools, &self.root, credentials)?;
        ceiling(&mut command, &self.root);
        let mut count = 0usize;
        for name in &drivers {
            for (key, value) in [
                (&b"clean"[..], ""),
                (b"smudge", ""),
                (b"process", ""),
                (b"required", "false"),
            ] {
                let mut full = b"filter.".to_vec();
                full.extend_from_slice(name);
                full.push(b'.');
                full.extend_from_slice(key);
                command
                    .env(format!("GIT_CONFIG_KEY_{count}"), OsString::from_vec(full))
                    .env(format!("GIT_CONFIG_VALUE_{count}"), value);
                count += 1;
            }
        }
        command.env("GIT_CONFIG_COUNT", count.to_string());
        Ok(command)
    }

    /// Runs the subcommand `name` with `args` (through [`Repo::command`] and
    /// [`subcommand`]) and hands back what it printed; a failure is a
    /// refusal read from what it said. The one way this crate runs git —
    /// and the way a caller (the belongings) runs what this crate has no
    /// method for.
    ///
    /// # Errors
    ///
    /// [`Why::TooLarge`] past `most` bytes, and what [`Repo::command`] and
    /// git refuse.
    pub fn output<S: AsRef<OsStr>>(
        &self,
        credentials: Option<&Credentials>,
        name: &str,
        args: &[S],
        most: usize,
    ) -> Result<Vec<u8>, Refusal> {
        let ran = self.ran(credentials, name, args, most)?;
        if ran.ok {
            Ok(ran.out)
        } else {
            Err(refused(verdict(
                &ran.err,
                credentials.is_some() || name == "fetch",
            )))
        }
    }

    fn ran<S: AsRef<OsStr>>(
        &self,
        credentials: Option<&Credentials>,
        name: &str,
        args: &[S],
        most: usize,
    ) -> Result<Ran, Refusal> {
        let mut command = self.command(credentials)?;
        subcommand(&mut command, name).args(args);
        run(command, most)
    }

    /// The work tree's state (`status --porcelain=v2 --branch -z`), every
    /// untracked path listed and no submodule entered.
    ///
    /// # Errors
    ///
    /// [`Why::NotARepo`], [`Why::Garbled`], and what git refuses.
    pub fn status(&self) -> Result<Status, Refusal> {
        let out = self.output(
            None,
            "status",
            &[
                "--porcelain=v2",
                "--branch",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=all",
            ],
            LISTING_MOST_BYTES,
        )?;
        parse::status(&out).ok_or(refused(Why::Garbled))
    }

    /// Whether HEAD names a commit: `false` on an unborn branch.
    fn born(&self) -> Result<bool, Refusal> {
        let ran = self.ran(
            None,
            "rev-parse",
            &["--verify", "--quiet", "HEAD^{commit}"],
            LISTING_MOST_BYTES,
        )?;
        if ran.ok {
            return Ok(true);
        }
        match verdict(&ran.err, false) {
            Why::NotARepo => Err(refused(Why::NotARepo)),
            _ => Ok(false),
        }
    }

    /// HEAD's commit; `None` on an unborn branch.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`].
    pub fn head_commit(&self) -> Result<Option<Commit>, Refusal> {
        Ok(self.log(0, 1)?.into_iter().next())
    }

    /// A page of history from HEAD: `most` commits after the first `skip`.
    /// Empty on an unborn branch.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`].
    pub fn log(&self, skip: usize, most: usize) -> Result<Vec<Commit>, Refusal> {
        if most == 0 || !self.born()? {
            return Ok(Vec::new());
        }
        let [skip, count] = page(skip, most);
        let out = self.output(
            None,
            "log",
            &["-z", parse::COMMIT_FORMAT, &skip, &count, "HEAD", "--"],
            LISTING_MOST_BYTES,
        )?;
        parse::commits(&out).ok_or(refused(Why::Garbled))
    }

    /// A commit, whole: its row, its message, and what it changed against
    /// its first parent (everything, for the first commit), renames found.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`], and [`Why::Garbled`] for what is not a commit id.
    pub fn show(&self, commit: &Oid) -> Result<CommitDetail, Refusal> {
        let at = commit_arg(commit)?;
        let out = self.output(
            None,
            "show",
            &[
                "-s",
                "-z",
                "--format=%H%x00%P%x00%an%x00%at%x00%s%x00%B",
                at,
                "--",
            ],
            LISTING_MOST_BYTES,
        )?;
        let mut fields = out.splitn(6, |&b| b == 0);
        let head: Vec<&[u8]> = fields.by_ref().take(5).collect();
        let message = fields.next().ok_or(refused(Why::Garbled))?;
        let mut record = head.join(&0u8);
        record.push(0);
        let row = parse::commits(&record)
            .and_then(|mut c| c.pop())
            .ok_or(refused(Why::Garbled))?;
        let message = String::from_utf8_lossy(message)
            .trim_end_matches(['\0', '\n'])
            .to_owned();
        let parent = row.parents.first().map(|p| p.0.clone());
        let mut args = vec!["-r", "-z", "--name-status", "-M", "--no-commit-id"];
        match parent.as_deref() {
            Some(parent) => args.extend([parent, at]),
            None => args.extend(["--root", at]),
        }
        let out = self.output(None, "diff-tree", &args, LISTING_MOST_BYTES)?;
        let files = parse::name_status(&out).ok_or(refused(Why::Garbled))?;
        Ok(CommitDetail {
            commit: row,
            message,
            files,
        })
    }

    /// What `commit` changed in `path`, as a unified diff — data, highlighted
    /// by the door. Against its FIRST PARENT, as [`Repo::show`] lists the
    /// files: a merge (and a stash, which is one) is otherwise shown as a
    /// combined diff with a sign column per parent, which is not what its
    /// list of files said changed.
    ///
    /// # Errors
    ///
    /// As [`Repo::show`], and [`Why::TooLarge`].
    pub fn diff(&self, commit: &Oid, path: &Path) -> Result<String, Refusal> {
        let at = commit_arg(commit)?;
        let args: [&OsStr; 7] = [
            "--format=".as_ref(),
            "--patch".as_ref(),
            "-M".as_ref(),
            "--diff-merges=first-parent".as_ref(),
            at.as_ref(),
            "--".as_ref(),
            path.as_os_str(),
        ];
        let out = self.output(None, "show", &args, TEXT_MOST_BYTES)?;
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    /// What is different about `path` in the work tree, as a unified diff:
    /// what is staged (the index against HEAD) when `staged`, what is not
    /// (the work tree against the index) otherwise. Empty when nothing is.
    /// A `diff` that finds differences still ends well (no `--exit-code`).
    ///
    /// # Errors
    ///
    /// As [`Repo::status`], and [`Why::TooLarge`].
    pub fn work_diff(&self, path: &Path, staged: bool) -> Result<String, Refusal> {
        // THE PLUMBING, NEVER `git diff`. MEASURED (the pinned 2.54): the
        // porcelain REWRITES THE INDEX when it met entries whose stat was
        // stale and whose content was not (`refresh_index_quietly`), and
        // `GIT_OPTIONAL_LOCKS` does not stop it — a read that writes under
        // the person's feet. `diff-files` and `diff-index` never refresh.
        let out = if staged {
            // AGAINST HEAD, or against the empty tree on an unborn branch:
            // everything staged there is new.
            let base = if self.born()? {
                OsString::from("HEAD")
            } else {
                self.empty_tree()?
            };
            let args: [&OsStr; 6] = [
                "--cached".as_ref(),
                "--patch".as_ref(),
                "-M".as_ref(),
                base.as_os_str(),
                "--".as_ref(),
                path.as_os_str(),
            ];
            self.output(None, "diff-index", &args, TEXT_MOST_BYTES)?
        } else {
            let args: [&OsStr; 4] = [
                "--patch".as_ref(),
                "-M".as_ref(),
                "--".as_ref(),
                path.as_os_str(),
            ];
            self.output(None, "diff-files", &args, TEXT_MOST_BYTES)?
        };
        Ok(String::from_utf8_lossy(&out).into_owned())
    }

    /// The id of the empty tree in this repository's hash (SHA-1 or
    /// SHA-256), asked of git and written nowhere (`hash-object` without
    /// `-w`, the empty stdin every call has).
    fn empty_tree(&self) -> Result<OsString, Refusal> {
        let out = self.output(
            None,
            "hash-object",
            &["-t", "tree", "--stdin"],
            LISTING_MOST_BYTES,
        )?;
        let id = String::from_utf8_lossy(&out).trim().to_owned();
        if parse::is_oid(&id, false) {
            Ok(OsString::from(id))
        } else {
            Err(refused(Why::Garbled))
        }
    }

    /// The bytes of `path` as `commit` has it — the one way to read an old
    /// version, for a read-only copy the person opens. `cat-file blob` runs
    /// no filter and no textconv (neither is asked for), and the path is
    /// from the repository's root, as a commit's changed files name it.
    ///
    /// # Errors
    ///
    /// [`Why::Garbled`] for what is not a commit id, [`Why::TooLarge`] past
    /// `most` bytes, [`Why::Failed`] when the commit has no such path, and
    /// as [`Repo::status`].
    pub fn blob(&self, commit: &Oid, path: &Path, most: usize) -> Result<Vec<u8>, Refusal> {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
        let at = commit_arg(commit)?;
        // ONE ARGUMENT, `<id>:<path>`, and it cannot be taken for an option:
        // it begins with the id, which was checked to be hexadecimal.
        let mut spec = at.as_bytes().to_vec();
        spec.push(b':');
        spec.extend_from_slice(path.as_os_str().as_bytes());
        let spec = OsString::from_vec(spec);
        self.output(None, "cat-file", &["blob".as_ref(), spec.as_os_str()], most)
    }

    /// The history of one path, following renames: `most` after `skip`.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`].
    pub fn file_history(
        &self,
        path: &Path,
        skip: usize,
        most: usize,
    ) -> Result<Vec<Commit>, Refusal> {
        Ok(self
            .file_history_named(path, skip, most)?
            .into_iter()
            .map(|f| f.commit)
            .collect())
    }

    /// The history of one path, following renames, with the NAME the path had
    /// in each commit — before a rename, the old one: what the commit's own
    /// list of files calls it. `most` after `skip`.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`].
    pub fn file_history_named(
        &self,
        path: &Path,
        skip: usize,
        most: usize,
    ) -> Result<Vec<FileCommit>, Refusal> {
        if most == 0 || !self.born()? {
            return Ok(Vec::new());
        }
        // THE PAGE IS CUT HERE, not by `--skip`. MEASURED on git 2.52, on a
        // history with a rename: with `--follow`, `--skip` counted a commit
        // that never touched the path, and the page began one too early.
        // `--max-count` counts after the filter, so the first `skip + most`
        // are asked for and the first `skip` dropped — right on either
        // version, whatever git does with `--skip` next.
        let count = format!("--max-count={}", skip.saturating_add(most));
        // `--name-status` says the path's name in each commit: `--follow`
        // limits it to the one path, under whichever name it had then.
        let args: [&OsStr; 8] = [
            "-z".as_ref(),
            parse::COMMIT_FORMAT.as_ref(),
            "--follow".as_ref(),
            "--name-status".as_ref(),
            count.as_ref(),
            "HEAD".as_ref(),
            "--".as_ref(),
            path.as_os_str(),
        ];
        let out = self.output(None, "log", &args, LISTING_MOST_BYTES)?;
        let all = parse::named_commits(&out).ok_or(refused(Why::Garbled))?;
        Ok(all
            .into_iter()
            .skip(skip)
            .map(|(commit, name)| FileCommit { commit, name })
            .collect())
    }

    /// Who wrote each line of `path` as it is in the work tree (on demand:
    /// it is slow on a long file). A line not committed yet carries the
    /// zero id.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`], and [`Why::TooLarge`].
    pub fn blame(&self, path: &Path) -> Result<Vec<BlameLine>, Refusal> {
        let args: [&OsStr; 3] = ["--porcelain".as_ref(), "--".as_ref(), path.as_os_str()];
        let out = self.output(None, "blame", &args, TEXT_MOST_BYTES)?;
        parse::blame(&out).ok_or(refused(Why::Garbled))
    }

    /// Branches, remote branches and tags, the most recent first.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`].
    pub fn refs(&self) -> Result<Vec<Ref>, Refusal> {
        let out = self.output(
            None,
            "for-each-ref",
            &[
                parse::REF_FORMAT,
                "--sort=-creatordate",
                "refs/heads",
                "refs/remotes",
                "refs/tags",
            ],
            LISTING_MOST_BYTES,
        )?;
        parse::refs(&out).ok_or(refused(Why::Garbled))
    }

    /// The stash, newest first (`stash@{0}`).
    ///
    /// # Errors
    ///
    /// As [`Repo::status`].
    pub fn stashes(&self) -> Result<Vec<Stash>, Refusal> {
        let has = self.ran(
            None,
            "rev-parse",
            &["--verify", "--quiet", "refs/stash"],
            LISTING_MOST_BYTES,
        )?;
        if !has.ok {
            return match verdict(&has.err, false) {
                Why::NotARepo => Err(refused(Why::NotARepo)),
                _ => Ok(Vec::new()),
            };
        }
        let out = self.output(
            None,
            "log",
            &[
                "-z",
                "--walk-reflogs",
                "--format=%H%x00%gs",
                "refs/stash",
                "--",
            ],
            LISTING_MOST_BYTES,
        )?;
        parse::stashes(&out).ok_or(refused(Why::Garbled))
    }

    /// The remotes, as configured.
    ///
    /// # Errors
    ///
    /// As [`Repo::status`].
    pub fn remotes(&self) -> Result<Vec<Remote>, Refusal> {
        let ran = self.ran(
            None,
            "config",
            &["--null", "--get-regexp", r"^remote\..*\.(url|pushurl)$"],
            LISTING_MOST_BYTES,
        )?;
        // No match is exit 1 with nothing said: no remotes.
        if !ran.ok && !ran.err.is_empty() {
            return Err(refused(verdict(&ran.err, false)));
        }
        Ok(parse::remotes(&ran.out))
    }

    /// Fetches every remote. Never prompts, never enters a submodule, and
    /// runs the remote's `git-upload-pack` whatever the repository's config
    /// names for it. Blocks on the network: a thread.
    ///
    /// ONE REMOTE AT A TIME, BY NAME, and on purpose. MEASURED (git 2.52 and
    /// the pinned 2.54): with two remotes or more, `fetch --all` hands each
    /// one to a child `git fetch` without the `--upload-pack` it was given,
    /// and the child runs the repository's `remote.<name>.uploadpack` — a
    /// command, on this machine, for a local remote. Every remote is still
    /// tried when one fails; the first refusal is the answer.
    ///
    /// # Errors
    ///
    /// [`Why::NeedsCredentials`], [`Why::Refused`], [`Why::Unreachable`],
    /// and as [`Repo::status`].
    pub fn fetch(&self, credentials: Option<&Credentials>) -> Result<(), Refusal> {
        let mut first = None;
        for remote in self.remotes()? {
            // A name is an argument after `--`, and never one with a line
            // end (it could not have come from a remote section anyway).
            if remote.name.contains('\n') {
                first.get_or_insert(refused(Why::Garbled));
                continue;
            }
            let done = self.output(
                credentials,
                "fetch",
                &[
                    "--quiet",
                    "--no-recurse-submodules",
                    "--upload-pack=git-upload-pack",
                    "--",
                    remote.name.as_str(),
                ],
                LISTING_MOST_BYTES,
            );
            if let Err(refusal) = done {
                first.get_or_insert(refusal);
            }
        }
        first.map_or(Ok(()), Err)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    fn fake_tools() -> Tools {
        Tools {
            git: PathBuf::from("/nix/store/x-git/bin/git"),
            ssh: Some(PathBuf::from("/nix/store/x-openssh/bin/ssh")),
            askpass: Some(PathBuf::from(
                "/nix/store/x-askpass/bin/ronoctua-git-askpass",
            )),
        }
    }

    fn envs(command: &Command) -> BTreeMap<String, Option<String>> {
        command
            .get_envs()
            .map(|(k, v)| {
                (
                    k.to_string_lossy().into_owned(),
                    v.map(|v| v.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    #[test]
    fn every_call_is_hardened_and_never_asks() {
        let key = Credentials::Ssh {
            key: PathBuf::from("/home/someone/.config/ronoctua/belongings-keys/id_ed25519"),
            known_hosts: PathBuf::from("/home/someone/it's/known_hosts"),
        };
        let mut command = base(&fake_tools(), Path::new("/home/someone/repo"), Some(&key)).unwrap();
        subcommand(&mut command, "show");
        assert_eq!(command.get_program(), "/nix/store/x-git/bin/git");
        let args: Vec<OsString> = command.get_args().map(OsStr::to_os_string).collect();
        let mut wanted: Vec<&str> = vec!["-C", "/home/someone/repo"];
        wanted.extend(HARDENING);
        wanted.extend(["show", "--no-ext-diff", "--no-textconv"]);
        assert_eq!(args, wanted);
        for pair in [
            "core.fsmonitor=false",
            "core.hooksPath=/dev/null",
            "credential.helper=",
            "core.askPass=",
            "protocol.ext.allow=never",
        ] {
            assert!(HARDENING.contains(&pair), "{pair}");
        }
        let env = envs(&command);
        assert_eq!(env["GIT_TERMINAL_PROMPT"].as_deref(), Some("0"));
        assert_eq!(env["GIT_OPTIONAL_LOCKS"].as_deref(), Some("0"));
        assert_eq!(env["GIT_LITERAL_PATHSPECS"].as_deref(), Some("1"));
        assert_eq!(env["GIT_PROXY_COMMAND"].as_deref(), Some(""));
        assert_eq!(
            env["GIT_SSH_COMMAND"].as_deref(),
            Some(
                "'/nix/store/x-openssh/bin/ssh' -i \
                 '/home/someone/.config/ronoctua/belongings-keys/id_ed25519' \
                 -o IdentitiesOnly=yes -o BatchMode=yes \
                 -o UserKnownHostsFile='/home/someone/it'\\''s/known_hosts' \
                 -o StrictHostKeyChecking=accept-new -o ConnectTimeout=20 -F none"
            ),
            "every path quoted for the shell git runs it through"
        );
        for name in [
            "GIT_DIR",
            "GIT_CONFIG_PARAMETERS",
            "GIT_ASKPASS",
            "GIT_EXTERNAL_DIFF",
            "GIT_EXEC_PATH",
        ] {
            assert_eq!(env[name], None, "{name} is removed");
        }

        let token = Credentials::Token {
            file: PathBuf::from("/run/user/1000/ronoctua-belongings/token"),
            user: Some("someone".to_owned()),
        };
        let mut command = base(&fake_tools(), Path::new("/r"), Some(&token)).unwrap();
        subcommand(&mut command, "status");
        let env = envs(&command);
        assert_eq!(
            env["GIT_ASKPASS"].as_deref(),
            Some("/nix/store/x-askpass/bin/ronoctua-git-askpass")
        );
        assert_eq!(
            env[SECRET_VARIABLE].as_deref(),
            Some("/run/user/1000/ronoctua-belongings/token"),
            "the path, never the token"
        );
        assert!(
            env["GIT_SSH_COMMAND"].is_some(),
            "always set: the repository's core.sshCommand never runs"
        );
        assert_eq!(
            command.get_args().last().unwrap(),
            "status",
            "no diff guard on status"
        );
        let mut command = base(&fake_tools(), Path::new("/r"), None).unwrap();
        subcommand(&mut command, "blame");
        assert_eq!(command.get_args().last().unwrap(), "--no-textconv");

        let bare = Tools {
            ssh: None,
            askpass: None,
            ..fake_tools()
        };
        assert_eq!(
            base(&bare, Path::new("/r"), Some(&key)).unwrap_err().why,
            Why::NoSsh
        );
        assert_eq!(
            base(&bare, Path::new("/r"), Some(&token)).unwrap_err().why,
            Why::NoAskpass
        );
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len());
    }

    #[test]
    fn a_failure_is_read_as_one_refusal() {
        assert_eq!(
            verdict("fatal: not a git repository (or any parent)", false),
            Why::NotARepo
        );
        assert_eq!(
            verdict(
                "fatal: could not read Username for 'https://h': terminal prompts disabled",
                true
            ),
            Why::NeedsCredentials
        );
        assert_eq!(
            verdict(
                "git@h: Permission denied (publickey).\nfatal: Could not read from remote repository.",
                true
            ),
            Why::Refused,
            "refused before unreachable: both lines come together"
        );
        assert_eq!(
            verdict(
                "fatal: unable to access 'https://h/': Could not resolve host: h",
                true
            ),
            Why::Unreachable
        );
        assert_eq!(
            verdict("error: open(\"x\"): Permission denied", false),
            Why::Failed,
            "a local permission is not the remote's"
        );
    }

    #[test]
    fn a_repository_is_found_by_walking_up() {
        let root = std::env::temp_dir().join(format!("ronoctua-git-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let deep = root.join("work/src/deep");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(root.join("work/.git")).unwrap();
        let repo = Repo::discover(&deep).unwrap();
        assert_eq!(repo.root, root.join("work"));
        assert_eq!(repo.git_dir, root.join("work/.git"));

        let linked = root.join("linked");
        std::fs::create_dir_all(&linked).unwrap();
        std::fs::write(
            linked.join(".git"),
            "gitdir: ../work/.git/worktrees/linked\n",
        )
        .unwrap();
        let repo = Repo::discover(&linked).unwrap();
        assert_eq!(repo.git_dir, linked.join("../work/.git/worktrees/linked"));
        let _ = std::fs::remove_dir_all(root);
    }

    // ---- with the real git -------------------------------------------

    /// The real git: the environment's or what `nix/rust.nix` compiled in.
    /// A test without it FAILS — returning early would prove nothing.
    fn real() -> Tools {
        let tools = Tools::from_env()
            .expect("RONOCTUA_GIT is not set: run inside `nix develop` or the tests derivation");
        assert!(tools.git.exists(), "{} does not exist", tools.git.display());
        tools
    }

    /// A folder of this test's own.
    fn place(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ronoctua-git-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Writes a fixture the way a person would, with NOBODY's configuration:
    /// the global and system files are shut out (a signing setup on the
    /// machine running the test would otherwise ask for a key), and the
    /// dates are fixed. The one place this crate runs git unhardened, and
    /// only on repositories it made.
    fn fixture(tools: &Tools, dir: &Path, args: &[&str]) -> String {
        let out = Command::new(&tools.git)
            .arg("-C")
            .arg(dir)
            .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Ada")
            .env("GIT_AUTHOR_EMAIL", "ada@example.org")
            .env("GIT_COMMITTER_NAME", "Ada")
            .env("GIT_COMMITTER_EMAIL", "ada@example.org")
            .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
            .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    fn opened(tools: &Tools, dir: &Path) -> Repo {
        Repo::discover(dir).unwrap().with_tools(tools.clone())
    }

    #[test]
    fn a_repository_is_read_as_git_wrote_it() {
        let tools = real();
        let dir = place("read");
        let bare = dir.join("remote.git");
        let work = dir.join("work");
        std::fs::create_dir_all(&work).unwrap();
        fixture(
            &tools,
            &dir,
            &["init", "-q", "--bare", "-b", "main", "remote.git"],
        );
        fixture(&tools, &work, &["init", "-q", "-b", "main"]);
        std::fs::write(work.join("a.txt"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
        std::fs::create_dir_all(work.join("dir")).unwrap();
        std::fs::write(work.join("dir/b.txt"), "b\n").unwrap();
        fixture(&tools, &work, &["add", "-A"]);
        fixture(&tools, &work, &["commit", "-qm", "first"]);
        let first = fixture(&tools, &work, &["rev-parse", "HEAD"]);
        // A rename into a name with a line end in it.
        fixture(&tools, &work, &["mv", "a.txt", "re named\n.txt"]);
        fixture(&tools, &work, &["commit", "-qm", "rename"]);
        let second = fixture(&tools, &work, &["rev-parse", "HEAD"]);
        fixture(&tools, &work, &["tag", "-a", "-m", "the first tag", "v1"]);
        std::fs::write(
            work.join("re named\n.txt"),
            "one\ntwo\nthree\nfour\nfive\nmore\n",
        )
        .unwrap();
        fixture(&tools, &work, &["commit", "-qam", "more"]);
        let third = fixture(&tools, &work, &["rev-parse", "HEAD"]);
        fixture(&tools, &work, &["branch", "side", &first]);
        fixture(
            &tools,
            &work,
            &["remote", "add", "origin", bare.to_str().unwrap()],
        );
        fixture(&tools, &work, &["push", "-q", "-u", "origin", "main"]);
        std::fs::write(work.join("dir/b.txt"), "stashed\n").unwrap();
        fixture(&tools, &work, &["stash", "push", "-q", "-m", "wip"]);
        // A commit the remote does not have, and a work tree with a bit of
        // everything.
        std::fs::write(work.join("c.txt"), "c\n").unwrap();
        fixture(&tools, &work, &["add", "c.txt"]);
        fixture(&tools, &work, &["commit", "-qm", "ahead"]);
        std::fs::write(work.join("staged.txt"), "s\n").unwrap();
        fixture(&tools, &work, &["add", "staged.txt"]);
        std::fs::write(work.join("dir/b.txt"), "changed\n").unwrap();
        std::fs::write(work.join("new\nfile"), "u\n").unwrap();

        let repo = opened(&tools, &work.join("dir"));
        assert_eq!(repo.root, work);
        let s = repo.status().unwrap();
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!((s.ahead, s.behind), (1, 0));
        assert_eq!(
            (s.staged, s.changed, s.untracked, s.conflicted),
            (1, 1, 1, 0)
        );
        assert!(
            s.entries
                .iter()
                .any(|e| e.path == Path::new("new\nfile") && e.state == EntryState::Untracked)
        );
        assert!(s.entries.iter().any(|e| e.path == Path::new("staged.txt")
            && e.state
                == EntryState::Tracked {
                    staged: Some(Change::Added),
                    unstaged: None
                }));

        // History, in pages.
        let head = repo.head_commit().unwrap().unwrap();
        assert_eq!(head.subject, "ahead");
        assert_eq!(head.author, "Ada");
        assert_eq!(head.at_s, 1_700_000_000);
        let page1 = repo.log(0, 2).unwrap();
        let page2 = repo.log(2, 10).unwrap();
        let subjects: Vec<&str> = page1
            .iter()
            .chain(&page2)
            .map(|c| c.subject.as_str())
            .collect();
        assert_eq!(subjects, ["ahead", "more", "rename", "first"]);
        assert_eq!(page2[0].id.0, second);
        assert_eq!(page2[0].parents, [Oid(first.clone())]);
        assert!(page2[1].parents.is_empty());

        // A commit whole, its rename found.
        let detail = repo.show(&Oid(second.clone())).unwrap();
        assert_eq!(detail.message, "rename");
        assert_eq!(
            detail.files,
            [ChangedFile {
                path: PathBuf::from("re named\n.txt"),
                change: Change::Renamed {
                    from: PathBuf::from("a.txt")
                }
            }]
        );
        let root = repo.show(&Oid(first.clone())).unwrap();
        assert_eq!(root.files.len(), 2, "the first commit against nothing");
        assert!(root.files.iter().all(|f| f.change == Change::Added));
        assert_eq!(
            repo.show(&Oid("--output=/tmp/x".to_owned()))
                .unwrap_err()
                .why,
            Why::Garbled,
            "what is not an id never becomes an argument"
        );

        let diff = repo
            .diff(&Oid(third.clone()), Path::new("re named\n.txt"))
            .unwrap();
        assert!(diff.contains("+more"), "{diff}");
        // A pathspec is read literally: `:(glob)*` names a file called that.
        assert_eq!(
            repo.diff(&Oid(third.clone()), Path::new(":(glob)*"))
                .unwrap(),
            ""
        );

        let history = repo
            .file_history(Path::new("re named\n.txt"), 0, 10)
            .unwrap();
        let ids: Vec<&str> = history.iter().map(|c| c.id.0.as_str()).collect();
        assert_eq!(
            ids,
            [third.as_str(), second.as_str(), first.as_str()],
            "followed across the rename"
        );
        // Paged, and the page is of the FILE's history (see `file_history`).
        assert_eq!(
            repo.file_history(Path::new("re named\n.txt"), 1, 1)
                .unwrap()[0]
                .id
                .0,
            second
        );
        assert_eq!(
            repo.file_history(Path::new("re named\n.txt"), 2, 5)
                .unwrap()[0]
                .id
                .0,
            first
        );
        // AND EACH COMMIT NAMES THE PATH AS IT KNEW IT: the old name before
        // the rename, the new one from it on.
        let named: Vec<Option<PathBuf>> = repo
            .file_history_named(Path::new("re named\n.txt"), 0, 10)
            .unwrap()
            .into_iter()
            .map(|f| f.name)
            .collect();
        assert_eq!(
            named,
            [
                Some(PathBuf::from("re named\n.txt")),
                Some(PathBuf::from("re named\n.txt")),
                Some(PathBuf::from("a.txt")),
            ],
            "a commit from before the rename names the old path"
        );

        // THE WORK TREE'S TWO DIFFS: what is staged and what is not.
        let staged = repo.work_diff(Path::new("staged.txt"), true).unwrap();
        assert!(staged.contains("+s"), "{staged}");
        assert_eq!(
            repo.work_diff(Path::new("staged.txt"), false).unwrap(),
            "",
            "nothing of it is unstaged"
        );
        let unstaged = repo.work_diff(Path::new("dir/b.txt"), false).unwrap();
        assert!(
            unstaged.contains("-b") && unstaged.contains("+changed"),
            "{unstaged}"
        );
        // AND A READ WRITES NOTHING. A file whose stat is stale and whose
        // content is not is what makes the porcelain `git diff` rewrite the
        // index; through this crate the index stays byte for byte as it was.
        let tracked = work.join("re named\n.txt");
        std::fs::File::options()
            .append(true)
            .open(&tracked)
            .unwrap()
            .set_modified(std::time::SystemTime::UNIX_EPOCH)
            .unwrap();
        let index = std::fs::read(work.join(".git/index")).unwrap();
        let quiet = repo.work_diff(Path::new("re named\n.txt"), false).unwrap();
        assert!(!quiet.contains("@@"), "a stat is not a change: {quiet}");
        let _ = repo.work_diff(Path::new("re named\n.txt"), true).unwrap();
        let _ = repo.status().unwrap();
        assert_eq!(
            std::fs::read(work.join(".git/index")).unwrap(),
            index,
            "a read rewrote the index"
        );
        // AN OLD VERSION, whole: the file as the first commit had it — a
        // path the commit does not have is a refusal, never an empty file.
        assert_eq!(
            repo.blob(&Oid(first.clone()), Path::new("a.txt"), 1024)
                .unwrap(),
            b"one\ntwo\nthree\nfour\nfive\n"
        );
        assert_eq!(
            repo.blob(&Oid(third.clone()), Path::new("a.txt"), 1024)
                .unwrap_err()
                .why,
            Why::Failed
        );
        assert_eq!(
            repo.blob(&Oid(first.clone()), Path::new("a.txt"), 4)
                .unwrap_err()
                .why,
            Why::TooLarge
        );
        assert_eq!(
            repo.blob(&Oid("-p".to_owned()), Path::new("a.txt"), 1024)
                .unwrap_err()
                .why,
            Why::Garbled,
            "what is not an id never becomes an argument"
        );

        let blame = repo.blame(Path::new("re named\n.txt")).unwrap();
        assert_eq!(blame.len(), 6);
        assert_eq!(blame[0].commit.0, first);
        assert_eq!(blame[5].commit.0, third);
        assert_eq!(blame[5].text, "more");
        assert_eq!(blame[5].number, 6);
        assert_eq!(blame[0].author, "Ada");

        let refs = repo.refs().unwrap();
        let find = |name: &str, kind| refs.iter().find(|r| r.name == name && r.kind == kind);
        assert_eq!(
            find("main", RefKind::Branch).unwrap().upstream.as_deref(),
            Some("origin/main")
        );
        assert_eq!(find("side", RefKind::Branch).unwrap().target.0, first);
        assert_eq!(find("v1", RefKind::Tag).unwrap().target.0, second, "peeled");
        assert_eq!(find("v1", RefKind::Tag).unwrap().subject, "the first tag");
        assert_eq!(
            find("origin/main", RefKind::Remote).unwrap().target.0,
            third
        );

        let stashes = repo.stashes().unwrap();
        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].subject.contains("wip"), "{:?}", stashes[0]);
        // A STASH IS A MERGE, and its diff is against its first parent — one
        // sign column, as its list of files says.
        let stashed = repo.diff(&stashes[0].id, Path::new("dir/b.txt")).unwrap();
        assert!(stashed.contains("\n+stashed"), "{stashed}");
        assert!(!stashed.contains("@@@"), "a combined diff: {stashed}");
        assert_eq!(
            repo.show(&stashes[0].id).unwrap().files.len(),
            1,
            "the stash changed one file against its base"
        );

        let remotes = repo.remotes().unwrap();
        assert_eq!(remotes.len(), 1);
        assert_eq!(remotes[0].name, "origin");
        assert_eq!(remotes[0].fetch_url, bare.to_str().unwrap());

        // The remote moves on; a fetch sees it, and never asks.
        let other = dir.join("other");
        fixture(
            &tools,
            &dir,
            &["clone", "-q", bare.to_str().unwrap(), "other"],
        );
        std::fs::write(other.join("d.txt"), "d\n").unwrap();
        fixture(&tools, &other, &["add", "d.txt"]);
        fixture(&tools, &other, &["commit", "-qm", "elsewhere"]);
        fixture(&tools, &other, &["push", "-q"]);
        repo.fetch(None).unwrap();
        let s = repo.status().unwrap();
        assert_eq!((s.ahead, s.behind), (1, 1));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn detached_and_unborn_are_states_not_errors() {
        let tools = real();
        let dir = place("states");
        fixture(&tools, &dir, &["init", "-q", "-b", "main"]);
        let repo = opened(&tools, &dir);
        let s = repo.status().unwrap();
        assert_eq!((s.head, s.branch.as_deref()), (None, Some("main")));
        assert_eq!(repo.head_commit().unwrap(), None);
        assert!(repo.log(0, 10).unwrap().is_empty());
        assert!(repo.file_history(Path::new("x"), 0, 10).unwrap().is_empty());
        assert!(repo.refs().unwrap().is_empty());
        assert!(repo.stashes().unwrap().is_empty());
        assert!(repo.remotes().unwrap().is_empty());

        std::fs::write(dir.join("a"), "a\n").unwrap();
        fixture(&tools, &dir, &["add", "a"]);
        // STAGED ON AN UNBORN BRANCH is staged against nothing at all.
        let staged = repo.work_diff(Path::new("a"), true).unwrap();
        assert!(staged.contains("+a"), "{staged}");
        fixture(&tools, &dir, &["commit", "-qm", "one"]);
        fixture(&tools, &dir, &["checkout", "-q", "--detach"]);
        let s = repo.status().unwrap();
        assert_eq!(s.branch, None, "detached");
        assert!(s.head.is_some());
        assert_eq!(repo.head_commit().unwrap().unwrap().subject, "one");

        let gone = dir.join("gone");
        std::fs::create_dir_all(gone.join(".git")).unwrap();
        assert_eq!(
            opened(&tools, &gone).status().unwrap_err().why,
            Why::NotARepo
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A script that leaves its mark in `marks`, then does `then` — what
    /// git expects of the program it thinks it is running. Written before
    /// any of them runs (a script still open for writing in another test's
    /// fork cannot be run: "text file busy").
    fn door(doors: &Path, marks: &Path, name: &str, then: &str) -> PathBuf {
        let path = doors.join(name);
        std::fs::write(
            &path,
            format!("#!/bin/sh\ntouch '{}/{name}'\n{then}\n", marks.display()),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn marks_made(marks: &Path) -> std::collections::BTreeSet<String> {
        std::fs::read_dir(marks)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect()
    }

    fn wipe(marks: &Path) {
        for e in std::fs::read_dir(marks).unwrap().flatten() {
            std::fs::remove_file(e.path()).unwrap();
        }
    }

    /// git WITHOUT this crate: what a person's own `git` would do in the
    /// folder. Its exit does not matter; what it ran does.
    fn unhardened(tools: &Tools, dir: &Path, args: &[&OsStr]) {
        let _ = Command::new(&tools.git)
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap();
    }

    /// The same on a terminal, which is the only place git starts a pager:
    /// `script` (util-linux, beside `setsid`) gives it one.
    fn on_a_terminal(tools: &Tools, dir: &Path, args: &[&str]) {
        let script = std::env::var_os("RONOCTUA_SCRIPT")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("RONOCTUA_SETSID")
                    .map(|s| PathBuf::from(s).with_file_name("script"))
            })
            .filter(|p| p.is_file())
            .expect("no `script` (RONOCTUA_SCRIPT, or beside RONOCTUA_SETSID): a pager needs a terminal");
        let line: Vec<String> = [tools.git.as_os_str(), "-C".as_ref(), dir.as_os_str()]
            .into_iter()
            .chain(args.iter().map(OsStr::new))
            .map(quoted)
            .collect();
        let _ = Command::new(script)
            .args(["-q", "-e", "-c", &line.join(" "), "/dev/null"])
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap();
    }

    /// A web server that answers every request with "401, who are you?" —
    /// a remote that wants credentials.
    struct Refuser {
        port: u16,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Refuser {
        fn start() -> Self {
            use std::io::Write as _;
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stopping = stop.clone();
            let thread = std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stopping.load(std::sync::atomic::Ordering::SeqCst) {
                        break;
                    }
                    let Ok(mut stream) = stream else { continue };
                    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                    let mut seen = Vec::new();
                    let mut buffer = [0u8; 4096];
                    while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
                        match stream.read(&mut buffer) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => seen.extend_from_slice(&buffer[..n]),
                        }
                    }
                    let _ = stream.write_all(
                        b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"x\"\r\n\
                          Content-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
            });
            Self {
                port,
                stop,
                thread: Some(thread),
            }
        }
    }

    impl Drop for Refuser {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    /// A folder somebody downloaded: its `.git` names a command in every
    /// place git would run one — and a partial clone beside it, whose lazy
    /// fetch would run one. Nothing this crate does runs any of them; and
    /// EVERY DOOR IS SHOWN TO OPEN, each by the plain git command that walks
    /// through it: a test of a door that stays shut is worth something only
    /// if the door opens.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn a_hostile_repository_runs_nothing() {
        use std::os::unix::ffi::OsStrExt as _;
        let tools = real();
        let dir = place("hostile");
        let work = dir.join("work");
        let bare = dir.join("remote.git");
        let doors = dir.join("doors");
        let marks = dir.join("marks");
        for d in [&work, &doors, &marks] {
            std::fs::create_dir_all(d).unwrap();
        }
        let web = Refuser::start();

        // THE DOORS, each a script that marks and then behaves.
        let d = |name: &str, then: &str| door(&doors, &marks, name, then);
        let cat = "exec cat";
        let clean = d("clean", cat);
        let clean_eq = d("clean-eq", cat);
        let clean_bytes = d("clean-bytes", cat);
        let clean_empty = d("clean-empty", cat);
        let smudge = d("smudge", cat);
        let process = d("process", "exit 1");
        let textconv = d("textconv", "exec cat \"$1\"");
        let diff_command = d("diff-command", "exit 0");
        let external = d("external", "exit 0");
        let fsmonitor = d("fsmonitor", "exit 1");
        let pager = d("pager", cat);
        let pager_log = d("pager-log", cat);
        let askpass = d("askpass", "echo someone");
        let credential = d("credential", "exit 0");
        let credential_url = d("credential-url", "exit 0");
        let ssh = d("ssh", "exit 1");
        let proxy = d("proxy", "exit 1");
        let alternates = d("alternates", "exit 0");
        let gpg = d("gpg", "exit 1");
        let upload_pack = d("upload-pack", "exec git-upload-pack \"$@\"");
        let lazy = d("lazy", "exec git-upload-pack \"$@\"");
        let ext = d("ext", "exit 1");
        let hook = d("hook", "exit 0");

        // THE REPOSITORY, written before its config turns hostile.
        fixture(
            &tools,
            &dir,
            &["init", "-q", "--bare", "-b", "main", "remote.git"],
        );
        fixture(&tools, &bare, &["config", "uploadpack.allowFilter", "true"]);
        fixture(&tools, &work, &["init", "-q", "-b", "main"]);
        let mut attributes = b"a.txt filter=clean\np.txt filter=process\ns.txt filter=smudge\n\
            e.txt filter=x=y\nn.txt filter=\nt.txt diff=tconv\nd.txt diff=dcmd\nf.txt filter="
            .to_vec();
        attributes.extend_from_slice(b"\xff\n");
        std::fs::write(work.join(".gitattributes"), attributes).unwrap();
        let files = ["a", "p", "s", "e", "f", "n", "t", "d", "x"];
        for f in files {
            std::fs::write(work.join(format!("{f}.txt")), format!("{f}\n")).unwrap();
        }
        fixture(&tools, &work, &["add", "-A"]);
        fixture(&tools, &work, &["commit", "-qm", "one"]);
        let one = fixture(&tools, &work, &["rev-parse", "HEAD"]);
        for f in ["a", "t", "d", "x"] {
            std::fs::write(work.join(format!("{f}.txt")), format!("{f}\n{f}\n")).unwrap();
        }
        fixture(&tools, &work, &["commit", "-qam", "two"]);
        let two = fixture(&tools, &work, &["rev-parse", "HEAD"]);
        fixture(
            &tools,
            &work,
            &["remote", "add", "origin", bare.to_str().unwrap()],
        );
        fixture(&tools, &work, &["push", "-q", "origin", "main"]);
        // A SIGNED commit on top (a signature git cannot check is still one
        // it hands to `gpg.program`), written as an object.
        let tree = fixture(&tools, &work, &["rev-parse", "HEAD^{tree}"]);
        let object = dir.join("signed");
        std::fs::write(
            &object,
            format!(
                "tree {tree}\nparent {two}\nauthor Ada <ada@example.org> 1700000000 +0000\n\
                 committer Ada <ada@example.org> 1700000000 +0000\n\
                 gpgsig -----BEGIN PGP SIGNATURE-----\n \n iQEzBAABCAAdFiEE\n \
                 -----END PGP SIGNATURE-----\n\nsigned\n"
            ),
        )
        .unwrap();
        let signed = fixture(
            &tools,
            &work,
            &[
                "hash-object",
                "-t",
                "commit",
                "-w",
                object.to_str().unwrap(),
            ],
        );
        fixture(&tools, &work, &["update-ref", "refs/heads/main", &signed]);
        // The remote moves on, so a fetch has refs to update (a hook's
        // moment).
        fixture(
            &tools,
            &dir,
            &["clone", "-q", bare.to_str().unwrap(), "other"],
        );
        let other = dir.join("other");
        std::fs::write(other.join("z.txt"), "z\n").unwrap();
        fixture(&tools, &other, &["add", "z.txt"]);
        fixture(&tools, &other, &["commit", "-qm", "elsewhere"]);
        fixture(&tools, &other, &["push", "-q"]);
        // A PARTIAL CLONE: its blobs are fetched when first read — the one
        // `z.txt` of its HEAD, for a start.
        fixture(
            &tools,
            &dir,
            &[
                "clone",
                "-q",
                "--filter=blob:none",
                "--no-checkout",
                &format!("file://{}", bare.display()),
                "partial",
            ],
        );
        let partial = dir.join("partial");
        let partial_head = fixture(&tools, &partial, &["rev-parse", "HEAD"]);

        // AND NOW THE CONFIG.
        let path = |p: &Path| p.to_str().unwrap().to_owned();
        let web_url = format!("http://127.0.0.1:{}/x.git", web.port);
        for (key, value) in [
            ("filter.clean.clean", path(&clean)),
            ("filter.smudge.smudge", path(&smudge)),
            ("filter.process.process", path(&process)),
            // git's `-c` would split this key at its `=`.
            ("filter.x=y.clean", path(&clean_eq)),
            ("diff.tconv.textconv", path(&textconv)),
            ("diff.dcmd.command", path(&diff_command)),
            ("diff.external", path(&external)),
            ("core.fsmonitor", path(&fsmonitor)),
            ("core.pager", path(&pager)),
            ("pager.log", path(&pager_log)),
            ("core.askPass", path(&askpass)),
            ("credential.helper", format!("!{}", path(&credential))),
            (
                &format!("credential.http://127.0.0.1:{}.helper", web.port),
                format!("!{}", path(&credential_url)),
            ),
            ("core.sshCommand", path(&ssh)),
            ("core.gitProxy", path(&proxy)),
            ("core.alternateRefsCommand", path(&alternates)),
            ("log.showSignature", "true".to_owned()),
            ("gpg.program", path(&gpg)),
            ("remote.origin.uploadpack", path(&upload_pack)),
            ("protocol.ext.allow", "always".to_owned()),
            ("remote.evil.url", format!("ext::{}", path(&ext))),
            ("remote.proxied.url", "git://127.0.0.1:1/x".to_owned()),
            ("remote.sshy.url", "ssh://127.0.0.1:1/x".to_owned()),
            ("remote.web.url", web_url),
        ] {
            fixture(&tools, &work, &["config", key, &value]);
        }
        // A driver whose name is not UTF-8, and one whose name is empty:
        // written as bytes, the way a downloaded file would carry them.
        let mut config = std::fs::read(work.join(".git/config")).unwrap();
        config.extend_from_slice(b"[filter \"\xff\"]\n\tclean = ");
        config.extend_from_slice(clean_bytes.as_os_str().as_bytes());
        config.extend_from_slice(b"\n[filter \"\"]\n\tclean = ");
        config.extend_from_slice(clean_empty.as_os_str().as_bytes());
        config.push(b'\n');
        std::fs::write(work.join(".git/config"), config).unwrap();
        std::fs::copy(&hook, work.join(".git/hooks/reference-transaction")).unwrap();
        std::fs::write(
            work.join(".git/objects/info/alternates"),
            format!("{}/objects\n", bare.display()),
        )
        .unwrap();
        fixture(
            &tools,
            &partial,
            &["config", "remote.origin.uploadpack", &path(&lazy)],
        );
        // A stat that no longer matches the index: a status must look at the
        // content, which is when a clean filter runs.
        let stale = |f: &str| {
            let file = std::fs::File::options()
                .append(true)
                .open(work.join(f))
                .unwrap();
            file.set_modified(std::time::SystemTime::UNIX_EPOCH)
                .unwrap();
        };
        let filtered = ["a.txt", "p.txt", "e.txt", "f.txt", "n.txt"];
        filtered.iter().for_each(|f| stale(f));
        wipe(&marks);

        // THROUGH THIS CRATE: every method, and nothing runs.
        let repo = opened(&tools, &work);
        let (one, two, signed) = (Oid(one), Oid(two), Oid(signed));
        assert_eq!(repo.head_commit().unwrap().unwrap().id, signed);
        let s = repo.status().unwrap();
        assert!(s.entries.is_empty(), "{:?}", s.entries);
        assert_eq!(repo.log(0, 10).unwrap().len(), 3);
        assert_eq!(repo.show(&signed).unwrap().message, "signed");
        assert_eq!(repo.show(&two).unwrap().files.len(), 4);
        for f in ["t.txt", "d.txt", "x.txt", "a.txt"] {
            let diff = repo.diff(&two, Path::new(f)).unwrap();
            let stem = &f[..1];
            assert!(diff.contains(&format!("+{stem}")), "{f}: {diff}");
        }
        assert_eq!(
            repo.file_history(Path::new("a.txt"), 0, 10).unwrap().len(),
            2
        );
        assert_eq!(repo.blame(Path::new("a.txt")).unwrap().len(), 2);
        assert_eq!(repo.blame(Path::new("t.txt")).unwrap()[1].commit, two);
        // THE WORK TREE'S DIFF of a file with a clean filter and of one with
        // a textconv driver, and an old version of each, read whole.
        for f in ["a.txt", "t.txt", "d.txt", "x.txt"] {
            let _ = repo.work_diff(Path::new(f), false).unwrap();
            let _ = repo.work_diff(Path::new(f), true).unwrap();
            let old = repo.blob(&two, Path::new(f), 1024).unwrap();
            assert_eq!(old, format!("{0}\n{0}\n", &f[..1]).as_bytes(), "{f}");
        }
        assert!(repo.refs().unwrap().len() >= 2);
        assert_eq!(repo.remotes().unwrap().len(), 5);
        // Every remote is tried: origin answers; ext, the dead `git://` and
        // `ssh://`, and the web server that wants a name do not.
        assert!(repo.fetch(None).is_err());
        let partial_repo = opened(&tools, &partial);
        assert!(
            partial_repo
                .diff(&Oid(partial_head.clone()), Path::new("z.txt"))
                .is_err(),
            "a blob that is not here is a refusal, never a fetch"
        );
        partial_repo.fetch(None).unwrap();
        let ran = marks_made(&marks);
        assert!(ran.is_empty(), "the repositories' config ran: {ran:?}");

        // AND EACH DOOR OPENS, by the plain git command that walks through it.
        let git = |args: &[&str]| {
            let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
            unhardened(&tools, &work, &args);
        };
        let opens = |what: &str, walk: &dyn Fn(), wanted: &[&str]| {
            wipe(&marks);
            walk();
            let ran = marks_made(&marks);
            for w in wanted {
                assert!(
                    ran.contains(*w),
                    "{what} did not run `{w}` (it ran {ran:?}): the fixture proves nothing"
                );
            }
        };
        let status_of = |f: &'static str| {
            move || {
                stale(f);
                git(&["status", "--porcelain", "--", f]);
            }
        };
        opens("a status", &status_of("a.txt"), &["fsmonitor", "clean"]);
        opens("a status", &status_of("p.txt"), &["process"]);
        opens("a status", &status_of("e.txt"), &["clean-eq"]);
        opens("a status", &status_of("f.txt"), &["clean-bytes"]);
        opens("a status", &status_of("n.txt"), &["clean-empty"]);
        opens(
            "a checkout",
            &|| {
                std::fs::remove_file(work.join("s.txt")).unwrap();
                git(&["checkout", "--", "s.txt"]);
            },
            &["smudge"],
        );
        opens(
            "a show",
            &|| git(&["show", &two.0, "--", "t.txt"]),
            &["textconv"],
        );
        opens(
            "a diff",
            &|| git(&["diff", &one.0, &two.0, "--", "d.txt"]),
            &["diff-command"],
        );
        opens(
            "a diff",
            &|| git(&["diff", &one.0, &two.0, "--", "x.txt"]),
            &["external"],
        );
        opens(
            "a show on a terminal",
            &|| on_a_terminal(&tools, &work, &["show", "-s", &two.0]),
            &["pager"],
        );
        opens(
            "a log on a terminal",
            &|| on_a_terminal(&tools, &work, &["log", "-1", &two.0]),
            &["pager-log"],
        );
        opens(
            "a log of a signed commit",
            &|| git(&["log", "-1", "--format=%H", &signed.0]),
            &["gpg"],
        );
        opens(
            "a fetch from a web server",
            &|| git(&["fetch", "web"]),
            &["askpass", "credential", "credential-url"],
        );
        opens("a fetch over ssh", &|| git(&["fetch", "sshy"]), &["ssh"]);
        opens(
            "a fetch over git://",
            &|| git(&["fetch", "proxied"]),
            &["proxy"],
        );
        opens("a fetch over ext::", &|| git(&["fetch", "evil"]), &["ext"]);
        opens(
            "a fetch",
            &|| git(&["fetch", "origin"]),
            &["upload-pack", "alternates"],
        );
        opens(
            "a ref update",
            &|| git(&["update-ref", "refs/control", "HEAD"]),
            &["hook"],
        );
        opens(
            "a read in a partial clone",
            &|| {
                let args: Vec<&OsStr> = ["show", "--format=", "--patch", partial_head.as_str()]
                    .into_iter()
                    .map(OsStr::new)
                    .collect();
                unhardened(&tools, &partial, &args);
            },
            &["lazy"],
        );
        drop(web);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_remote_nobody_answers_is_unreachable() {
        let tools = real();
        let dir = place("unreachable");
        fixture(&tools, &dir, &["init", "-q", "-b", "main"]);
        fixture(
            &tools,
            &dir,
            &["remote", "add", "origin", "https://127.0.0.1:1/x.git"],
        );
        let repo = opened(&tools, &dir);
        assert_eq!(repo.fetch(None).unwrap_err().why, Why::Unreachable);
        let _ = std::fs::remove_dir_all(dir);
    }
}
