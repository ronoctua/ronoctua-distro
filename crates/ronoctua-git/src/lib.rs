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
//! # Post-quantum only, where it is asked for
//!
//! `pq_only: true` — the belongings' credentials, both kinds — lets the
//! session key be agreed only post-quantum: a server that offers no such
//! exchange is refused before anything is sent ([`Why::NotPostQuantum`]),
//! because what travels there is the person's whole encrypted home, and a
//! recording of a classically keyed session is readable the day a quantum
//! computer is.
//!
//! - **SSH** says [`PQ_KEX`] on ssh's command line (`-o KexAlgorithms=`),
//!   which no configuration file can widen.
//! - **HTTPS** is libcurl over OpenSSL, inside git, and git has no setting
//!   for the key-exchange groups. OpenSSL does: `OPENSSL_CONF` names a file
//!   whose `system_default` section every TLS context of the process starts
//!   from, and the one [`Tools::pq_tls`] names (`nix/pq-tls.nix`) lists only
//!   the hybrid ML-KEM groups, TLS 1.3 and up (and git's own floor,
//!   `GIT_SSL_VERSION`, says TLS 1.3 too). MEASURED on the pinned git:
//!   a server with X25519 alone, with the NIST curves alone, or with TLS 1.2
//!   alone is refused (`alert handshake failure`); one with
//!   `X25519MLKEM768` or `SecP384r1MLKEM1024` is reached.
//!
//! **OpenSSL ignores a configuration it cannot read, SILENTLY** — a missing
//! file, a relative path, a group name it does not know: the narrowing is
//! simply gone and every server is reached (measured, all three). So [`base`]
//! refuses a post-quantum-only token when the file is not there, and
//! `belongings-https-is-pq-only` proves the file the build ships refuses a
//! classical server with the git the build ships.
//!
//! # Read-only where the person reads
//!
//! The file manager's line and browser ([`Repo::status`], [`Repo::log`],
//! [`Repo::show`], [`Repo::diff`], [`Repo::blame`]…) change nothing; the one
//! network call, [`Repo::fetch`], never prompts — a remote that needs
//! credentials it was not given is a refusal that says so. Every call blocks
//! until git ends: a thread, never the event loop.
//!
//! # And the one that reaches a new host
//!
//! [`Repo::clone`] (feature `clone`) makes a repository from an address, and
//! the host it reaches is one nobody let yet: so it takes a
//! [`ronoctua_consent::Pass`] for that host, and an address the pass does not
//! cover starts nothing — nor does a redirect take the clone to another host
//! (`http.followRedirects=false`, for the address itself too). The host is
//! where the address LEADS: a clone keeps the person's settings, and their
//! `url.<base>.insteadOf` can send it elsewhere — so the repository is made
//! first and read where its fetch will run ([`Repo::prepare_clone`]), and
//! only then fetched ([`Prepared::fetch`]).

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
/// The variable naming the OpenSSL configuration a post-quantum-only HTTPS
/// session runs git under (`nix/rust.nix` compiles it in; the environment
/// wins).
pub const PQ_TLS_VARIABLE: &str = "RONOCTUA_GIT_PQ_TLS";
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

/// A clone slower than this many bytes a second…
#[cfg(feature = "clone")]
pub const CLONE_LOW_SPEED_BYTES_PER_S: u32 = 1024;
/// …for this many seconds in a row is given up: git over HTTPS has no
/// deadline of its own, and a stalled line would hold the clone — and the
/// thread waiting on it — for ever, with nothing said.
#[cfg(feature = "clone")]
pub const CLONE_LOW_SPEED_S: u32 = 60;

/// The key exchanges a post-quantum-only SSH session may agree, in ssh's
/// order of preference: ML-KEM-768 with X25519 (OpenSSH 9.9, the standard
/// name), then Streamlined NTRU Prime with X25519 under both its names
/// (OpenSSH 8.5 on). sntrup761 STAYS: GitHub answers it today and only
/// offers ML-KEM from 2026-10-14, and a list of ML-KEM alone would refuse the
/// host most belongings live on.
///
/// The same three open the system's list (`assets/ssh.nix`, `postQuantum`) —
/// `belongings-ssh-is-pq-only` holds the two together and proves the list
/// against a real `sshd`.
pub const PQ_KEX: [&str; 3] = [
    "mlkem768x25519-sha256",
    "sntrup761x25519-sha512",
    "sntrup761x25519-sha512@openssh.com",
];

/// What ssh prints when the server offered no key exchange it was allowed —
/// for a post-quantum-only session, a server with no post-quantum one; for
/// any other, a server with only what this machine no longer agrees (the
/// SHA-1 exchanges).
pub const NO_COMMON_KEX: &str = "no matching key exchange method found";

/// What git prints (curl's words over OpenSSL's) when a TLS server would not
/// complete a handshake — for a post-quantum-only session, a server with no
/// hybrid group ([`Tools::pq_tls`]). `alert handshake failure` is the one
/// MEASURED (a server with X25519 alone, the NIST curves alone, or TLS 1.2
/// alone); `insufficient security` is the alert a server answers when the
/// groups offered are below its policy, and the last two say a server that
/// stops short of TLS 1.3, where the hybrids live.
pub const PQ_TLS_REFUSED: [&str; 4] = [
    "alert handshake failure",
    "alert insufficient security",
    "alert protocol version",
    "unsupported protocol",
];

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
    /// The OpenSSL configuration a post-quantum-only HTTPS session runs git
    /// under — the hybrid ML-KEM groups alone, TLS 1.3 and up
    /// (`nix/pq-tls.nix`) — when given.
    pub pq_tls: Option<PathBuf>,
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
            pq_tls: program_path(PQ_TLS_VARIABLE, option_env!("RONOCTUA_GIT_PQ_TLS")),
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
        /// Only [`PQ_KEX`] may key the session: a server with none of them is
        /// [`Why::NotPostQuantum`].
        pq_only: bool,
    },
    /// HTTPS with a token kept in a `0600` file; only its path travels.
    Token {
        /// The token's file.
        file: PathBuf,
        /// The user name a prompt is answered with, when the host wants one.
        user: Option<String>,
        /// Only the hybrid ML-KEM groups may key the TLS session
        /// ([`Tools::pq_tls`]), and only [`PQ_KEX`] an SSH one: a server with
        /// none of them is [`Why::NotPostQuantum`].
        pq_only: bool,
    },
}

impl Credentials {
    /// Whether the session may be keyed only post-quantum.
    #[must_use]
    pub const fn pq_only(&self) -> bool {
        match self {
            Self::Ssh { pq_only, .. } | Self::Token { pq_only, .. } => *pq_only,
        }
    }
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
    /// A post-quantum-only session met a server that offers no post-quantum
    /// key exchange ([`PQ_KEX`], or no hybrid TLS group): nothing was sent.
    NotPostQuantum,
    /// The SSH server offers only key exchanges this machine no longer
    /// agrees (the SHA-1 ones): it was reached, and nothing was sent.
    NoCommonKeyExchange,
}

impl Why {
    /// Every refusal, for the keys' census.
    pub const ALL: [Self; 13] = [
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
        Self::NotPostQuantum,
        Self::NoCommonKeyExchange,
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
            Self::NotPostQuantum => "not-post-quantum",
            Self::NoCommonKeyExchange => "no-common-key-exchange",
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
/// credentials' environment, with neither the global nor the system
/// configuration when there are credentials (no rewrite of the address the
/// consent named). Add the subcommand with [`subcommand`].
///
/// # Errors
///
/// [`Why::NoSsh`] for SSH credentials without ssh, [`Why::NoAskpass`] for a
/// token without the helper, [`Why::Unstartable`] for a post-quantum-only
/// token without the configuration that narrows its TLS ([`Tools::pq_tls`]).
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
    // A REMOTE REACHED WITH OUR ACCESS (the belongings' deploy key or token)
    // reads no configuration but the repository's own. The consent was given
    // for the host of the address the caller named, and the person's global
    // or the system's `url.<base>.insteadOf` / `pushInsteadOf` would take the
    // access — and what is pushed — to a host nobody let, after the pass was
    // checked. Nothing else of those files is needed here: the SSH command,
    // the credentials and a commit's identities are all said by this call. A
    // call with no access of ours (the git view, on the person's own
    // projects) keeps the person's configuration, rewrites included.
    if credentials.is_some() {
        command
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1");
    }
    // ALWAYS SET: the environment beats the repository's `core.sshCommand`.
    let ssh = tools
        .ssh
        .as_deref()
        .map_or_else(|| "ssh".to_owned(), |s| quoted(s.as_os_str()));
    let timeout = format!("-o ConnectTimeout={SSH_CONNECT_TIMEOUT_S}");
    // ON THE COMMAND LINE, where it beats every configuration file: the list
    // a post-quantum-only session may agree, for SSH.
    let kex = if credentials.is_some_and(Credentials::pq_only) {
        format!(" -o KexAlgorithms={}", PQ_KEX.join(","))
    } else {
        String::new()
    };
    match credentials {
        Some(Credentials::Ssh {
            key, known_hosts, ..
        }) => {
            if tools.ssh.is_none() {
                return Err(refused(Why::NoSsh));
            }
            // `-F none` READS NO CONFIGURATION, the system's included — and
            // the key exchanges were said above, so nothing a file says can
            // widen them.
            command.env(
                "GIT_SSH_COMMAND",
                format!(
                    "{ssh} -i {} -o IdentitiesOnly=yes -o BatchMode=yes -o UserKnownHostsFile={} \
                     -o StrictHostKeyChecking=accept-new {timeout}{kex} -F none",
                    quoted(key.as_os_str()),
                    quoted(known_hosts.as_os_str()),
                ),
            );
        }
        Some(Credentials::Token {
            file,
            user,
            pq_only,
        }) => {
            let askpass = tools.askpass.as_deref().ok_or(refused(Why::NoAskpass))?;
            if *pq_only {
                // NOT THERE IS NOT "NO NARROWING": OpenSSL would read a
                // missing file as no configuration at all, and every server
                // would be reached. A store path is absolute and does not
                // move; anything else is refused before git starts.
                let narrowing = tools
                    .pq_tls
                    .as_deref()
                    .filter(|p| p.is_absolute() && p.is_file())
                    .ok_or(refused(Why::Unstartable))?;
                // AND GIT'S OWN FLOOR, TLS 1.3, where the hybrids live: curl
                // sets the lowest version on top of OpenSSL's, and a
                // repository's `http.sslVersion` or a session's
                // `GIT_SSL_VERSION` could otherwise lower it. The
                // environment beats the configuration.
                command
                    .env("OPENSSL_CONF", narrowing)
                    .env("GIT_SSL_VERSION", "tlsv1.3");
            }
            command
                .env(
                    "GIT_SSH_COMMAND",
                    format!("{ssh} -o BatchMode=yes {timeout}{kex}"),
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
    /// Its exit code; `None` when a signal ended it.
    code: Option<i32>,
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
        code: status.code(),
        out: bytes,
        err,
    })
}

/// [`run`], with git stopped when it has not ended within the limit given —
/// the refusal then the [`Why`] given with it. A clone's (feature `clone`):
/// every other call is bounded by what it reads.
///
/// THE CLOCK IS ONE THREAD, armed only while git runs: it waits on a channel
/// the call closes when git ended, and kills git at the limit; nothing polls
/// while git writes. Only after git closed its output does this ask, in short
/// steps, whether it ended — the moment between the two is short, and a
/// blocking wait there would hold the child where the clock cannot kill it.
#[cfg(feature = "clone")]
fn run_until(
    mut command: Command,
    most: usize,
    limit: Option<(std::time::Duration, Why)>,
) -> Result<Ran, Refusal> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
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
    let child = Arc::new(Mutex::new(child));
    let late = Arc::new(AtomicBool::new(false));
    let (ended, ending) = std::sync::mpsc::channel::<()>();
    let clock = limit.map(|(within, _)| {
        let child = Arc::clone(&child);
        let late = Arc::clone(&late);
        std::thread::spawn(move || {
            if ending.recv_timeout(within) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
                late.store(true, Ordering::SeqCst);
                if let Ok(mut c) = child.lock() {
                    let _ = c.kill();
                }
            }
        })
    });
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
    if too_large && let Ok(mut c) = child.lock() {
        let _ = c.kill();
    }
    drop(out);
    let status = loop {
        let polled = child.lock().map_err(|_| refused(Why::Failed))?.try_wait();
        match polled {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(20)),
            Err(_) => return Err(refused(Why::Failed)),
        }
    };
    drop(ended);
    if let Some(clock) = clock {
        let _ = clock.join();
    }
    let err = errors.join().unwrap_or_default();
    if let Some((_, why)) = limit
        && late.load(Ordering::SeqCst)
    {
        return Err(refused(why));
    }
    if too_large {
        return Err(refused(Why::TooLarge));
    }
    read.map_err(|_| refused(Why::Failed))?;
    Ok(Ran {
        ok: status.success(),
        code: status.code(),
        out: bytes,
        err,
    })
}

/// Where a failed call went — which decides what its stderr can mean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// This machine only: a "permission denied" is the file system's.
    Local,
    /// A remote, with the key exchanges this machine agrees.
    Remote,
    /// A remote, over a session keyed only post-quantum
    /// ([`Credentials::pq_only`]).
    PostQuantumOnly,
}

/// Where `name`, run with `credentials`, goes. Pure.
fn reach_of(credentials: Option<&Credentials>, name: &str) -> Reach {
    match credentials {
        Some(c) if c.pq_only() => Reach::PostQuantumOnly,
        Some(_) => Reach::Remote,
        None if name == "fetch" => Reach::Remote,
        None => Reach::Local,
    }
}

/// What a failed git's stderr means, for a call that went where `reach`
/// says (only a remote's "permission denied" is the remote's). Pure.
#[must_use]
fn verdict(err: &str, reach: Reach) -> Why {
    let says = |words: &[&str]| words.iter().any(|w| err.contains(w));
    let narrowed = reach == Reach::PostQuantumOnly;
    if says(&["not a git repository"]) {
        Why::NotARepo
    } else if reach == Reach::Local {
        Why::Failed
    } else if says(&[NO_COMMON_KEX]) {
        // BEFORE "unreachable": git closes the same sentence with "Could not
        // read from remote repository", and the host WAS reached. A
        // post-quantum-only session refused it for that; any other session
        // offered everything this machine agrees, and the host had none of it.
        if narrowed {
            Why::NotPostQuantum
        } else {
            Why::NoCommonKeyExchange
        }
    } else if narrowed && says(&PQ_TLS_REFUSED) {
        // Before "unable to access", which curl writes in front of it: the
        // server was reached and would not agree a hybrid group.
        Why::NotPostQuantum
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

/// A whole commit id as an argument (never an abbreviation, never a name),
/// or [`Why::Garbled`].
fn full_oid(commit: &Oid) -> Result<&str, Refusal> {
    parse::is_oid(&commit.0, false)
        .then_some(commit.0.as_str())
        .ok_or(refused(Why::Garbled))
}

/// Who wrote or committed a commit, and when — as `--date=raw` prints it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Signature {
    /// The name.
    pub name: String,
    /// The address.
    pub email: String,
    /// `<seconds since the epoch> <±hhmm>`.
    pub date: String,
}

impl Signature {
    /// Whether git would write it back as it is: no line end, no NUL, no
    /// angle bracket in the name or the address, and a raw date.
    #[must_use]
    pub fn fits(&self) -> bool {
        let plain = |t: &str| !t.contains(['\n', '\0', '<', '>']);
        let date = self.date.split_once(' ').is_some_and(|(s, z)| {
            !s.is_empty()
                && s.bytes().all(|b| b.is_ascii_digit())
                && z.len() == 5
                && z.starts_with(['+', '-'])
                && z[1..].bytes().all(|b| b.is_ascii_digit())
        });
        plain(&self.name) && plain(&self.email) && date
    }
}

/// A commit written again over another parent ([`Repo::commit_tree`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recommit {
    /// The tree it holds.
    pub tree: Oid,
    /// Its parent; `None` for a new root.
    pub parent: Option<Oid>,
    /// Its message, as it was.
    pub message: String,
    /// Its author, as it was.
    pub author: Signature,
    /// Its committer, as it was.
    pub committer: Signature,
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

/// The transports an address may name for a clone, by scheme.
#[cfg(feature = "clone")]
const CLONE_SCHEMES: [&str; 4] = ["https", "http", "git", "ssh"];

/// The host a remote's address names, as consent keeps it
/// ([`ronoctua_consent::normal`]): `https://host/…`, `http://`, `git://`,
/// `ssh://[user@]host[:port]/…`, and SSH's short form `[user@]host:path`.
/// `None` for an address that names no host — a local path, `file://`, a
/// remote helper's `<transport>::<address>` (`ext::`, `fd::`, `gcrypt::…`),
/// an option (`-…`) — or a host consent would not keep. Pure.
#[cfg(feature = "clone")]
#[must_use]
pub fn host_of(url: &str) -> Option<String> {
    if url.starts_with('-') || url.chars().any(char::is_whitespace) || through_a_helper(url) {
        return None;
    }
    let authority = if let Some((scheme, rest)) = url.split_once("://") {
        if !CLONE_SCHEMES.contains(&scheme.to_ascii_lowercase().as_str()) {
            return None;
        }
        rest.split('/').next()?
    } else {
        // `host:path` is SSH only when no `/` comes before the colon (git's
        // own rule); `./a:b` and `/x:y` are paths.
        let (before, _) = url.split_once(':')?;
        if before.contains('/') || before.is_empty() {
            return None;
        }
        before
    };
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    ronoctua_consent::normal(host)
}

/// Whether git hands `url` to a remote helper — `<transport>::<address>`,
/// the transport a letter and then letters, digits, `+`, `-` and `.` (git's
/// own rule, `is_urlschemechar`): a program named after the transport
/// reaches whatever it likes, so there is no host to ask, and never one named
/// after the helper. Pure.
#[cfg(feature = "clone")]
fn through_a_helper(url: &str) -> bool {
    url.split_once("::").is_some_and(|(transport, _)| {
        transport
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
            && transport
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    })
}

/// The most bytes an address git answers with is read as (a few KiB is
/// already no address).
#[cfg(feature = "clone")]
const ADDRESS_MOST_BYTES: usize = 64 * 1024;

/// How long a step of a clone that reads and writes only this machine may
/// take (`init`, `config`, `ls-remote --get-url`), in seconds: a
/// configuration that includes a file nobody ever finishes writing (a FIFO)
/// would hold it, and the person's question, for ever.
#[cfg(feature = "clone")]
pub const CLONE_LOCAL_MOST_S: u64 = 120;

/// How long the fetch and the checkout may take, in seconds. A stalled
/// HTTPS line is given up sooner ([`CLONE_LOW_SPEED_S`]); SSH and `git://`
/// have no such limit of git's, and this is theirs.
#[cfg(feature = "clone")]
pub const CLONE_FETCH_MOST_S: u64 = 3600;

/// WHERE GIT READS THE PERSON'S SETTINGS for a call with no access of ours —
/// the clone's: what this process inherited (their home, the system's file),
/// in the product; a home and a system file of a test's own, planted.
#[cfg(feature = "clone")]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Settings {
    planted: Option<(PathBuf, PathBuf)>,
}

#[cfg(feature = "clone")]
impl Settings {
    /// The person's own: what this process inherited.
    #[must_use]
    pub fn persons() -> Self {
        Self::default()
    }

    /// A test's: `home` is the home git reads (its `.gitconfig`, and
    /// `.config/git/config` under it) and `system` the system's file — never
    /// those of the machine running the test.
    #[must_use]
    pub fn planted(home: &Path, system: &Path) -> Self {
        Self {
            planted: Some((home.to_path_buf(), system.to_path_buf())),
        }
    }

    /// Says them on a [`base`] command.
    fn on(&self, command: &mut Command) {
        if let Some((home, system)) = &self.planted {
            command
                .env("HOME", home)
                .env_remove("XDG_CONFIG_HOME")
                .env_remove("GIT_CONFIG_GLOBAL")
                .env_remove("GIT_CONFIG_NOSYSTEM")
                .env("GIT_CONFIG_SYSTEM", system);
        }
    }
}

/// Where a clone of `url` would go, read from `/`: the address with the
/// person's own rewrites applied (`url.<base>.insteadOf`, from their global
/// configuration and the system's — a call with no access of ours reads
/// both, [`base`]), as git itself expands it — `git ls-remote --get-url`,
/// which asks the network nothing. What `LEADER / c` asks about before the
/// folder is known: the common rewrite `https://github.com/` →
/// `git@github.com:` keeps the host, and another can send the clone to one
/// the person never let. `pushInsteadOf` is a push's, never a clone's.
///
/// The CLONE reads it again where it will fetch ([`Repo::prepare_clone`]):
/// a rule only a file included for the new folder holds
/// (`includeIf "gitdir:…"`, `"hasconfig:remote.*.url:…"`) is not seen from
/// here.
///
/// # Errors
///
/// [`Why::Garbled`] for an address that names no host, or an answer that
/// is not one address (empty, with a control character, an option);
/// [`Why::NoGit`]… as [`base`]; [`Why::Failed`] when git could not read the
/// configuration, or did not in [`CLONE_LOCAL_MOST_S`].
#[cfg(feature = "clone")]
pub fn effective_url(tools: &Tools, url: &str) -> Result<String, Refusal> {
    effective_url_in(tools, &Settings::persons(), url)
}

/// [`effective_url`], with the person's settings read where `settings`
/// says (a test plants its own).
///
/// # Errors
///
/// As [`effective_url`].
#[cfg(feature = "clone")]
pub fn effective_url_in(tools: &Tools, settings: &Settings, url: &str) -> Result<String, Refusal> {
    if host_of(url).is_none() {
        return Err(refused(Why::Garbled));
    }
    let mut command = base(tools, Path::new("/"), None)?;
    settings.on(&mut command);
    subcommand(&mut command, "ls-remote").args(["--get-url", "--", url]);
    let ran = run_until(
        command,
        ADDRESS_MOST_BYTES,
        Some((
            std::time::Duration::from_secs(CLONE_LOCAL_MOST_S),
            Why::Failed,
        )),
    )?;
    if !ran.ok {
        return Err(refused(verdict(&ran.err, Reach::Local)));
    }
    one_address(ran.out)
}

/// What git answered for `--get-url`, when it is one address.
#[cfg(feature = "clone")]
fn one_address(out: Vec<u8>) -> Result<String, Refusal> {
    let text = String::from_utf8(out).map_err(|_| refused(Why::Garbled))?;
    let effective = text.strip_suffix('\n').unwrap_or(&text);
    if effective.is_empty() || effective.starts_with('-') || effective.chars().any(char::is_control)
    {
        return Err(refused(Why::Garbled));
    }
    Ok(effective.to_owned())
}

/// A CLONE MADE READY, and nothing reached ([`Repo::prepare_clone`]): the
/// folder is a repository whose `origin` is the address — every URL the
/// person's settings gave a remote `origin` let go first —, and where its
/// fetch goes was read THERE, with every file the person's settings include
/// for that folder. [`Prepared::fetch`] fetches and checks out with a pass
/// for that host; [`Prepared::abandon`] takes back what was made.
#[cfg(feature = "clone")]
#[derive(Debug)]
#[must_use]
pub struct Prepared {
    tools: Tools,
    settings: Settings,
    url: String,
    effective: String,
    into: PathBuf,
    /// Whether the folder itself was made (else it was there, empty).
    made: bool,
}

#[cfg(feature = "clone")]
impl Prepared {
    /// Where the fetch goes: the address as the person's settings send it
    /// from the new repository.
    #[must_use]
    pub fn effective(&self) -> &str {
        &self.effective
    }

    /// The host the fetch reaches ([`host_of`]) — `None` for a path or a
    /// remote helper, which no pass covers.
    #[must_use]
    pub fn host(&self) -> Option<String> {
        host_of(&self.effective)
    }

    /// A [`base`] call in `at` with the person's settings, kept inside the
    /// new folder when it runs there: a `.git` gone missing never sends git
    /// up to a repository that holds the folder.
    fn call(&self, at: &Path) -> Result<Command, Refusal> {
        let mut command = base(&self.tools, at, None)?;
        self.settings.on(&mut command);
        if at == self.into {
            ceiling(&mut command, &self.into);
        }
        Ok(command)
    }

    /// A step in the new repository that reads and writes only this
    /// machine, to its end.
    fn local(&self, name: &str, args: &[&str]) -> Result<Ran, Refusal> {
        let mut command = self.call(&self.into)?;
        subcommand(&mut command, name).args(args);
        let ran = run_until(
            command,
            ADDRESS_MOST_BYTES,
            Some((
                std::time::Duration::from_secs(CLONE_LOCAL_MOST_S),
                Why::Failed,
            )),
        )?;
        if ran.ok {
            Ok(ran)
        } else {
            Err(refused(verdict(&ran.err, Reach::Local)))
        }
    }

    /// Makes the repository, gives it `origin`, and reads where its fetch
    /// goes. Nothing of the network is asked.
    fn ready(&self) -> Result<String, Refusal> {
        let mut init = self.call(Path::new("/"))?;
        subcommand(&mut init, "init")
            .args(["--quiet", "--template=", "--"])
            .arg(&self.into);
        let ran = run_until(
            init,
            ADDRESS_MOST_BYTES,
            Some((
                std::time::Duration::from_secs(CLONE_LOCAL_MOST_S),
                Why::Failed,
            )),
        )?;
        if !ran.ok {
            return Err(refused(verdict(&ran.err, Reach::Local)));
        }
        // EVERY URL THE PERSON'S SETTINGS GAVE `origin` GOES FIRST — an empty
        // value resets the list —, then the address: a global
        // `remote.origin.url` would otherwise be the first, and the fetch's.
        self.local("config", &["--", "remote.origin.url", ""])?;
        self.local("config", &["--add", "--", "remote.origin.url", &self.url])?;
        self.local(
            "config",
            &[
                "--",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ],
        )?;
        // WHERE THE FETCH GOES, read where it runs: the files the person's
        // settings include for this folder (`gitdir:`) or for this address
        // (`hasconfig:remote.*.url:`) are read here, as the fetch reads them.
        one_address(self.local("ls-remote", &["--get-url", "origin"])?.out)
    }

    /// The second half of the clone: with a pass for the host the fetch goes
    /// to, `origin` fetched — its branches and its tags — and its default
    /// branch checked out and tracked, as `git clone` leaves a repository;
    /// `origin` the address alone then. Hardened like every call ([`base`]),
    /// no submodule, the remote's own `git-upload-pack`, and:
    ///
    /// * PINNED to where it was found to lead: `url.<effective>.insteadOf`
    ///   for the whole address, through the environment (`GIT_CONFIG_KEY_0`),
    ///   never `-c`, which splits at an address's first `=` — settings
    ///   changed since [`Repo::prepare_clone`] read them do not move it;
    /// * NO REDIRECT, to any host: `http.followRedirects=false`, and the same
    ///   for the effective address itself (`http.<effective>.followRedirects`,
    ///   read last and as specific as a rule of the person's can be) — git's
    ///   default follows the first redirect anywhere, and a rule of the
    ///   person's for that address would win over the plain key;
    /// * A LOW-SPEED LIMIT ([`CLONE_LOW_SPEED_BYTES_PER_S`] for
    ///   [`CLONE_LOW_SPEED_S`]) over HTTPS, and [`CLONE_FETCH_MOST_S`] for all.
    ///
    /// # Errors
    ///
    /// [`Why::Garbled`] when the pass does not cover the host — nothing is
    /// asked; [`Why::NeedsCredentials`], [`Why::Refused`],
    /// [`Why::Unreachable`] (a fetch that did not end in time among them) and
    /// [`Why::Failed`] from what git said. Whatever the refusal, what
    /// [`Repo::prepare_clone`] made goes.
    pub fn fetch(self, pass: &ronoctua_consent::Pass) -> Result<Repo, Refusal> {
        match self.fetched(pass) {
            Ok(repo) => Ok(repo),
            Err(refusal) => {
                self.abandon();
                Err(refusal)
            }
        }
    }

    fn fetched(&self, pass: &ronoctua_consent::Pass) -> Result<Repo, Refusal> {
        if !self.host().is_some_and(|host| pass.covers(&host)) {
            return Err(refused(Why::Garbled));
        }
        let limit = std::time::Duration::from_secs(CLONE_FETCH_MOST_S);
        let ran = run_until(
            self.fetch_command()?,
            LISTING_MOST_BYTES,
            Some((limit, Why::Unreachable)),
        )?;
        if !ran.ok {
            return Err(refused(verdict(&ran.err, Reach::Remote)));
        }
        // THE REMOTE'S DEFAULT BRANCH, which the fetch named
        // (`refs/remotes/origin/HEAD`), checked out and tracked; a remote with
        // no branch leaves the empty repository `git clone` leaves.
        let mut head = self.call(&self.into)?;
        subcommand(&mut head, "symbolic-ref").args(["--quiet", "refs/remotes/origin/HEAD"]);
        let local = std::time::Duration::from_secs(CLONE_LOCAL_MOST_S);
        let named = run_until(head, ADDRESS_MOST_BYTES, Some((local, Why::Failed)))?;
        let named = String::from_utf8_lossy(&named.out).trim().to_owned();
        if let Some(branch) = named.strip_prefix("refs/remotes/origin/") {
            let mut checkout = self.call(&self.into)?;
            subcommand(&mut checkout, "checkout")
                .args([
                    "--quiet",
                    "--no-recurse-submodules",
                    "-b",
                    branch,
                    "--track",
                ])
                .arg(&named);
            let ran = run_until(checkout, LISTING_MOST_BYTES, Some((limit, Why::Failed)))?;
            if !ran.ok {
                return Err(refused(verdict(&ran.err, Reach::Local)));
            }
        }
        self.local(
            "config",
            &["--replace-all", "--", "remote.origin.url", &self.url],
        )?;
        Ok(Repo {
            root: self.into.clone(),
            git_dir: self.into.join(".git"),
            tools: Some(self.tools.clone()),
        })
    }

    /// The fetch's command — what it carries is said at [`Prepared::fetch`].
    fn fetch_command(&self) -> Result<Command, Refusal> {
        let mut command = self.call(&self.into)?;
        command
            .args(["-c", "http.followRedirects=false", "-c"])
            .arg(format!("http.lowSpeedLimit={CLONE_LOW_SPEED_BYTES_PER_S}"))
            .arg("-c")
            .arg(format!("http.lowSpeedTime={CLONE_LOW_SPEED_S}"));
        let mut pins = vec![(
            format!("url.{}.insteadOf", self.effective),
            self.url.clone(),
        )];
        if over_http(&self.effective) {
            pins.push((
                format!("http.{}.followRedirects", self.effective),
                "false".to_owned(),
            ));
        }
        command.env("GIT_CONFIG_COUNT", pins.len().to_string());
        for (n, (key, value)) in pins.into_iter().enumerate() {
            command
                .env(format!("GIT_CONFIG_KEY_{n}"), key)
                .env(format!("GIT_CONFIG_VALUE_{n}"), value);
        }
        subcommand(&mut command, "fetch").args([
            "--quiet",
            "--no-recurse-submodules",
            "--upload-pack=git-upload-pack",
            "--tags",
            "origin",
        ]);
        Ok(command)
    }

    /// Takes back what [`Repo::prepare_clone`] made: the folder, when it was
    /// made, else what is in it (it was empty). Nothing was reached.
    pub fn abandon(self) {
        unmake(&self.into, self.made);
    }
}

/// Whether an address is fetched over HTTP(S) — where a redirect can be.
#[cfg(feature = "clone")]
fn over_http(url: &str) -> bool {
    url.split_once("://")
        .is_some_and(|(scheme, _)| matches!(scheme.to_ascii_lowercase().as_str(), "http" | "https"))
}

/// Removes what a clone made in `into`: the folder when it made it, else
/// what is in it — it was empty. Never follows a link.
#[cfg(feature = "clone")]
fn unmake(into: &Path, made: bool) {
    if made {
        let _ = std::fs::remove_dir_all(into);
        return;
    }
    let Ok(entries) = std::fs::read_dir(into) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
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
            return Err(refused(verdict(&ran.err, Reach::Local)));
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
        let ran = self.ran(credentials, name, args, None, most)?;
        if ran.ok {
            Ok(ran.out)
        } else {
            Err(refused(verdict(&ran.err, reach_of(credentials, name))))
        }
    }

    /// Runs `name` with `args`; `signed` is the author and the committer a
    /// `commit-tree` writes ([`Repo::commit_tree`]) — the six variables git
    /// reads them from, and no other.
    fn ran<S: AsRef<OsStr>>(
        &self,
        credentials: Option<&Credentials>,
        name: &str,
        args: &[S],
        signed: Option<(&Signature, &Signature)>,
        most: usize,
    ) -> Result<Ran, Refusal> {
        let mut command = self.command(credentials)?;
        subcommand(&mut command, name).args(args);
        if let Some((author, committer)) = signed {
            command
                .env("GIT_AUTHOR_NAME", &author.name)
                .env("GIT_AUTHOR_EMAIL", &author.email)
                .env("GIT_AUTHOR_DATE", &author.date)
                .env("GIT_COMMITTER_NAME", &committer.name)
                .env("GIT_COMMITTER_EMAIL", &committer.email)
                .env("GIT_COMMITTER_DATE", &committer.date);
        }
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
            None,
            LISTING_MOST_BYTES,
        )?;
        if ran.ok {
            return Ok(true);
        }
        match verdict(&ran.err, Reach::Local) {
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
            None,
            LISTING_MOST_BYTES,
        )?;
        if !has.ok {
            return match verdict(&has.err, Reach::Local) {
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
            None,
            LISTING_MOST_BYTES,
        )?;
        // No match is exit 1 with nothing said: no remotes.
        if !ran.ok && !ran.err.is_empty() {
            return Err(refused(verdict(&ran.err, Reach::Local)));
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

    /// Writes a commit of `commit.tree` over `commit.parent` (none: a new
    /// root) with its message and both signatures as they were — what a
    /// history is rewritten with (`commit-tree`, the belongings'
    /// compaction). Gives back the new commit's id.
    ///
    /// # Errors
    ///
    /// [`Why::Garbled`] for an id that is not one, or a name, an address, a
    /// date or a message git would read as something else; and what git
    /// refuses.
    pub fn commit_tree(&self, commit: &Recommit) -> Result<Oid, Refusal> {
        let tree = full_oid(&commit.tree)?;
        let mut args: Vec<&str> = vec![tree];
        if let Some(parent) = &commit.parent {
            args.extend(["-p", full_oid(parent)?]);
        }
        if commit.message.contains('\0') || !commit.author.fits() || !commit.committer.fits() {
            return Err(refused(Why::Garbled));
        }
        args.extend(["-m", commit.message.as_str()]);
        let ran = self.ran(
            None,
            "commit-tree",
            &args,
            Some((&commit.author, &commit.committer)),
            LISTING_MOST_BYTES,
        )?;
        if !ran.ok {
            return Err(refused(verdict(&ran.err, Reach::Local)));
        }
        let id = String::from_utf8_lossy(&ran.out).trim().to_owned();
        parse::is_oid(&id, false)
            .then_some(Oid(id))
            .ok_or(refused(Why::Garbled))
    }

    /// Pushes `local` to `branch` of the remote `remote` over whatever is
    /// there, but ONLY while the remote's branch is still `expected`
    /// (`--force-with-lease`): a push another machine made after `expected`
    /// was fetched refuses this one instead of being lost. The remote's own
    /// `git-receive-pack`, never a command the repository's config names.
    ///
    /// # Errors
    ///
    /// [`Why::Garbled`] for an id, a remote or a branch that is not one; a
    /// lease another push broke is a refusal like any other, and the caller
    /// fetches to see why.
    pub fn push_force_with_lease(
        &self,
        credentials: Option<&Credentials>,
        remote: &str,
        local: &Oid,
        branch: &str,
        expected: &Oid,
    ) -> Result<(), Refusal> {
        let named = |name: &str| {
            !name.is_empty()
                && !name.starts_with('-')
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b))
                && !name.contains("..")
        };
        if !named(remote) || !named(branch) {
            return Err(refused(Why::Garbled));
        }
        let lease = format!(
            "--force-with-lease=refs/heads/{branch}:{}",
            full_oid(expected)?
        );
        let refspec = format!("{}:refs/heads/{branch}", full_oid(local)?);
        self.output(
            credentials,
            "push",
            &[
                "-q",
                "--receive-pack=git-receive-pack",
                lease.as_str(),
                remote,
                refspec.as_str(),
            ],
            LISTING_MOST_BYTES,
        )
        .map(|_| ())
    }

    /// Clones `url` into `into` — the one call of this crate that makes a
    /// repository, and so the one that takes a [`ronoctua_consent::Pass`]:
    /// the person let the host the fetch goes to, or nothing is asked
    /// (`LEADER / c`, the distro's own repository). [`Repo::prepare_clone`]
    /// with the person's settings, then [`Prepared::fetch`]. The tools come
    /// from the environment.
    ///
    /// # Errors
    ///
    /// As [`Repo::clone_with`], and [`Why::NoGit`] without git.
    #[cfg(feature = "clone")]
    pub fn clone(pass: &ronoctua_consent::Pass, url: &str, into: &Path) -> Result<Self, Refusal> {
        let tools = Tools::from_env().ok_or(refused(Why::NoGit))?;
        Self::clone_with(&tools, pass, url, into)
    }

    /// [`Repo::clone`], with these tools (a test's).
    ///
    /// # Errors
    ///
    /// What [`Repo::prepare_clone`] and [`Prepared::fetch`] refuse.
    #[cfg(feature = "clone")]
    pub fn clone_with(
        tools: &Tools,
        pass: &ronoctua_consent::Pass,
        url: &str,
        into: &Path,
    ) -> Result<Self, Refusal> {
        Self::prepare_clone(tools, &Settings::persons(), url, into)?.fetch(pass)
    }

    /// The first half of a clone, which reaches nothing: `into` made a
    /// repository (`init`, no template) whose `origin` is `url`, and where
    /// its fetch goes read there ([`Prepared::effective`]).
    ///
    /// WHY NOT `git clone`. A clone keeps the person's settings, and three of
    /// them send it somewhere only the new repository knows (all measured on
    /// the pinned git): a rewrite in a file included for the new folder
    /// (`includeIf "gitdir:…"`) or for the address (`"hasconfig:remote.*.url:…"`),
    /// which `ls-remote --get-url` from anywhere else does not read and which
    /// beats any rule of ours for the same whole address (the first read wins
    /// a tie); and a global `remote.origin.url`, which `git clone` reads
    /// before the address it was given. Made first, the repository is where
    /// the fetch will run, read as it will read it — and `origin`'s URL list
    /// is reset before the address goes in.
    ///
    /// # Errors
    ///
    /// [`Why::Garbled`] for an address that names no host ([`host_of`]) or a
    /// folder that is not an absolute path, and [`Why::Failed`] for a folder
    /// that is there and not empty — nothing made or touched; what a step
    /// refuses (git could not make or read the repository, or not in
    /// [`CLONE_LOCAL_MOST_S`]), and then what was made goes.
    #[cfg(feature = "clone")]
    pub fn prepare_clone(
        tools: &Tools,
        settings: &Settings,
        url: &str,
        into: &Path,
    ) -> Result<Prepared, Refusal> {
        if host_of(url).is_none() || !into.is_absolute() {
            return Err(refused(Why::Garbled));
        }
        let made = match std::fs::symlink_metadata(into) {
            Err(_) => true,
            Ok(meta)
                if meta.is_dir()
                    && std::fs::read_dir(into).is_ok_and(|mut e| e.next().is_none()) =>
            {
                false
            }
            Ok(_) => return Err(refused(Why::Failed)),
        };
        let mut prepared = Prepared {
            tools: tools.clone(),
            settings: settings.clone(),
            url: url.to_owned(),
            effective: String::new(),
            into: into.to_path_buf(),
            made,
        };
        match prepared.ready() {
            Ok(effective) => {
                prepared.effective = effective;
                Ok(prepared)
            }
            Err(refusal) => {
                prepared.abandon();
                Err(refusal)
            }
        }
    }
}

/// How [`merge_file`] settles a hunk both sides changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Favor {
    /// It does not: the hunk is a conflict, counted.
    None,
    /// This side's lines (`--ours`).
    Ours,
    /// The other side's (`--theirs`).
    Theirs,
    /// Both sides' lines, ours first (`--union`) — a list whose lines are
    /// each a fact (`known_hosts`, `allowed_signers`).
    Union,
}

/// What [`merge_file`] made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMerge {
    /// The merged text — with conflict markers where `conflicts` says, when
    /// nothing was favoured.
    pub bytes: Vec<u8>,
    /// How many hunks clash (0: a clean merge). git counts at most 127.
    pub conflicts: u32,
}

/// The names of the three sides inside the scratch folder.
const MERGE_SIDES: [&str; 3] = ["ours", "base", "theirs"];

/// A three-way merge of texts, hunk by hunk — `git merge-file -p`, through
/// the hardened command ([`base`]): `ours` and `theirs` joined over `base`,
/// each hunk both changed settled by `favor` or counted as a conflict.
///
/// The three are written into `scratch` — a folder of the caller's that
/// nobody else reads (they may be a person's files), made `0700` beforehand —
/// born `0600`, and deleted before this returns, whatever it returns. No
/// repository is read: `merge-file` takes three files.
///
/// # Errors
///
/// [`Why::NoGit`]… as [`base`]; [`Why::Failed`] when git could not merge
/// them (a binary file, a file it could not read) or the files could not be
/// written.
pub fn merge_file(
    tools: &Tools,
    scratch: &Path,
    base_text: &[u8],
    ours: &[u8],
    theirs: &[u8],
    favor: Favor,
) -> Result<FileMerge, Refusal> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let paths: Vec<PathBuf> = MERGE_SIDES.iter().map(|n| scratch.join(n)).collect();
    let written = paths
        .iter()
        .zip([ours, base_text, theirs])
        .try_for_each(|(path, bytes)| {
            let _ = std::fs::remove_file(path);
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)?
                .write_all(bytes)
        });
    let merged = written.map_err(|_| refused(Why::Failed)).and_then(|()| {
        let mut command = base(tools, scratch, None)?;
        subcommand(&mut command, "merge-file").arg("-p");
        match favor {
            Favor::None => {}
            Favor::Ours => {
                command.arg("--ours");
            }
            Favor::Theirs => {
                command.arg("--theirs");
            }
            Favor::Union => {
                command.arg("--union");
            }
        }
        // THE LABELS of the markers: fixed words, never a path.
        command.args(["-L", "ours", "-L", "base", "-L", "theirs"]);
        command.args(MERGE_SIDES);
        run(command, LISTING_MOST_BYTES)
    });
    for path in &paths {
        let _ = std::fs::remove_file(path);
    }
    let ran = merged?;
    // THE EXIT CODE IS THE COUNT of conflicting hunks (at most 127); a
    // negative one — 255 — is a failure, and so is a signal.
    match ran.code {
        Some(n) if (0..=127).contains(&n) => Ok(FileMerge {
            bytes: ran.out,
            conflicts: u32::try_from(n).unwrap_or(0),
        }),
        _ => Err(refused(Why::Failed)),
    }
}

/// Whether `key` may name a setting: `section.name` (a subsection between
/// them allowed), ASCII, nothing git would read as an option or a pattern.
/// Pure.
fn a_setting(key: &str) -> bool {
    let fits = !key.is_empty()
        && key.len() <= 256
        && !key.starts_with(['-', '.'])
        && !key.ends_with('.')
        && key.contains('.')
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./@".contains(&b));
    fits && !key.contains("..")
}

/// A `git config --global` call for the person whose home is `home`: their
/// `~/.gitconfig` (or `~/.config/git/config` when only that one exists),
/// whatever this process's own variables say — `HOME` and
/// `XDG_CONFIG_HOME` from `home`, no `GIT_CONFIG_GLOBAL`. Run from `/`, so no
/// repository's configuration is read along.
fn global_config(tools: &Tools, home: &Path) -> Result<Command, Refusal> {
    let mut command = base(tools, Path::new("/"), None)?;
    command
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env_remove("GIT_CONFIG_GLOBAL");
    subcommand(&mut command, "config").arg("--global");
    Ok(command)
}

/// Sets `key` to `value` in the person's GLOBAL git configuration file,
/// every value it had replaced. The hardened command ([`base`]), no
/// repository, no prompt. (The belongings' "sign commits with this SSH key"
/// writes its settings into a file of its own, [`set_in_file`], and only
/// the include into this one, [`add_global`].)
///
/// # Errors
///
/// [`Why::Garbled`] for a key that is not a setting's name or a value with
/// a line break or a NUL in it; [`Why::NoGit`]…, and [`Why::Failed`] when git
/// could not write the file.
pub fn set_global(tools: &Tools, home: &Path, key: &str, value: &str) -> Result<(), Refusal> {
    if !a_setting(key) || value.contains(['\n', '\r', '\0']) {
        return Err(refused(Why::Garbled));
    }
    let mut command = global_config(tools, home)?;
    command.args(["--replace-all", "--", key, value]);
    let ran = run(command, LISTING_MOST_BYTES)?;
    if ran.ok {
        Ok(())
    } else {
        Err(refused(verdict(&ran.err, Reach::Local)))
    }
}

/// The value of `key` in the person's global git configuration — the last
/// one when it has several; `None` when it is not set. What the files it
/// INCLUDES (`include.path`) say counts, as it counts for git when it
/// commits: `git config --global` alone reads one file.
///
/// # Errors
///
/// [`Why::Garbled`] for a key that is not a setting's name; [`Why::NoGit`]…,
/// and [`Why::Failed`] when git could not read the file.
pub fn get_global(tools: &Tools, home: &Path, key: &str) -> Result<Option<String>, Refusal> {
    if !a_setting(key) {
        return Err(refused(Why::Garbled));
    }
    let mut command = global_config(tools, home)?;
    command.args(["--includes", "--get", "--", key]);
    let ran = run(command, LISTING_MOST_BYTES)?;
    match ran.code {
        Some(0) => Ok(Some(
            String::from_utf8_lossy(&ran.out)
                .trim_end_matches(['\n', '\r'])
                .to_owned(),
        )),
        // `--get` SAYS 1 for a key that is not set.
        Some(1) => Ok(None),
        _ => Err(refused(verdict(&ran.err, Reach::Local))),
    }
}

/// Adds `value` to the key `key` of the person's GLOBAL git configuration
/// file — a key that holds several (`include.path`) — unless that file has
/// it already: every other value stays, and asking twice adds it once.
///
/// # Errors
///
/// As [`set_global`].
pub fn add_global(tools: &Tools, home: &Path, key: &str, value: &str) -> Result<(), Refusal> {
    if !a_setting(key) || value.contains(['\n', '\r', '\0']) {
        return Err(refused(Why::Garbled));
    }
    let mut command = global_config(tools, home)?;
    command.args(["--get-all", "--", key]);
    let ran = run(command, LISTING_MOST_BYTES)?;
    let there = match ran.code {
        Some(0) => String::from_utf8_lossy(&ran.out)
            .lines()
            .any(|line| line == value),
        // `--get-all` SAYS 1 for a key that is not set.
        Some(1) => false,
        _ => return Err(refused(verdict(&ran.err, Reach::Local))),
    };
    if there {
        return Ok(());
    }
    let mut command = global_config(tools, home)?;
    command.args(["--add", "--", key, value]);
    let ran = run(command, LISTING_MOST_BYTES)?;
    if ran.ok {
        Ok(())
    } else {
        Err(refused(verdict(&ran.err, Reach::Local)))
    }
}

/// Sets `key` to `value` in the git configuration file at `file` (`git
/// config --file`, an absolute path), every value it had replaced — a file
/// of settings the global configuration includes. git keeps the mode of a
/// file it rewrites, so one the caller made private stays private.
///
/// # Errors
///
/// [`Why::Garbled`] for a key that is not a setting's name, a value with a
/// line break or a NUL in it, or a path that is not absolute;
/// [`Why::NoGit`]…, and [`Why::Failed`] when git could not write the file.
pub fn set_in_file(tools: &Tools, file: &Path, key: &str, value: &str) -> Result<(), Refusal> {
    if !a_setting(key) || value.contains(['\n', '\r', '\0']) || !file.is_absolute() {
        return Err(refused(Why::Garbled));
    }
    let mut command = base(tools, Path::new("/"), None)?;
    command.env_remove("GIT_CONFIG_GLOBAL");
    subcommand(&mut command, "config")
        .arg("--file")
        .arg(file)
        .args(["--replace-all", "--", key, value]);
    let ran = run(command, LISTING_MOST_BYTES)?;
    if ran.ok {
        Ok(())
    } else {
        Err(refused(verdict(&ran.err, Reach::Local)))
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
            pq_tls: None,
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
            pq_only: false,
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
            pq_only: false,
        };
        let mut command = base(&fake_tools(), Path::new("/r"), Some(&token)).unwrap();
        subcommand(&mut command, "status");
        let env = envs(&command);
        assert!(
            !env.contains_key("OPENSSL_CONF"),
            "a session that is not narrowed leaves OpenSSL as it is"
        );
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

    /// FAILS IF: a post-quantum-only session leaves ssh free to agree a
    /// classical key, or one that is not asks for the narrower list.
    #[test]
    fn a_post_quantum_only_session_names_only_post_quantum_exchanges() {
        let key = |pq_only| Credentials::Ssh {
            key: PathBuf::from("/k"),
            known_hosts: PathBuf::from("/h"),
            pq_only,
        };
        let ssh_of = |credentials: &Credentials| {
            let command = base(&fake_tools(), Path::new("/r"), Some(credentials)).unwrap();
            envs(&command)["GIT_SSH_COMMAND"].clone().unwrap()
        };
        let narrow = ssh_of(&key(true));
        let list = narrow
            .split_whitespace()
            .find_map(|word| word.strip_prefix("KexAlgorithms="))
            .unwrap_or_default();
        let said: Vec<&str> = list.split(',').collect();
        assert_eq!(said, PQ_KEX, "{narrow}");
        assert!(
            narrow.ends_with(" -F none"),
            "no configuration file can widen it: {narrow}"
        );
        assert!(!ssh_of(&key(false)).contains("KexAlgorithms"));
        for name in PQ_KEX {
            assert!(
                name.starts_with("mlkem") || name.starts_with("sntrup"),
                "{name} is not a post-quantum exchange"
            );
        }
    }

    /// FAILS IF: a post-quantum-only token reaches git without the file
    /// that narrows its TLS — OpenSSL would read none, silently, and agree a
    /// classical key with anybody — or its ssh is not narrowed too.
    #[test]
    fn a_post_quantum_only_token_runs_git_under_the_narrowing() {
        // A FOLDER OF ITS OWN NAME: the TLS server test's is `pq-tls`, and
        // the two run at once.
        let dir = place("narrowing-file");
        let file = dir.join("pq-tls.cnf");
        std::fs::write(&file, "openssl_conf = openssl_init\n").unwrap();
        let token = |pq_only| Credentials::Token {
            file: PathBuf::from("/run/token"),
            user: None,
            pq_only,
        };
        let with = |pq_tls: Option<PathBuf>| Tools {
            pq_tls,
            ..fake_tools()
        };

        let command = base(
            &with(Some(file.clone())),
            Path::new("/r"),
            Some(&token(true)),
        )
        .unwrap();
        let env = envs(&command);
        assert_eq!(
            env["OPENSSL_CONF"].as_deref(),
            Some(file.to_str().unwrap()),
            "git's curl reads the narrowing"
        );
        assert_eq!(
            env["GIT_SSL_VERSION"].as_deref(),
            Some("tlsv1.3"),
            "and git's own floor cannot be lowered below the hybrids"
        );
        let ssh = env["GIT_SSH_COMMAND"].clone().unwrap();
        assert!(
            ssh.contains(&format!(" -o KexAlgorithms={}", PQ_KEX.join(","))),
            "an ssh remote reached with a token is narrowed too: {ssh}"
        );

        // NOTHING TO NARROW WITH IS A REFUSAL, never a session that is not.
        for missing in [
            None,
            Some(dir.join("gone.cnf")),
            Some(PathBuf::from("pq-tls.cnf")),
        ] {
            assert_eq!(
                base(&with(missing.clone()), Path::new("/r"), Some(&token(true)))
                    .unwrap_err()
                    .why,
                Why::Unstartable,
                "{missing:?}"
            );
        }
        // And a token that is not narrowed needs none.
        let command = base(&with(None), Path::new("/r"), Some(&token(false))).unwrap();
        assert!(!envs(&command).contains_key("OPENSSL_CONF"));
        assert!(!envs(&command).contains_key("GIT_SSL_VERSION"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failure_is_read_as_one_refusal() {
        assert_eq!(
            verdict("fatal: not a git repository (or any parent)", Reach::Local),
            Why::NotARepo
        );
        assert_eq!(
            verdict(
                "fatal: could not read Username for 'https://h': terminal prompts disabled",
                Reach::Remote
            ),
            Why::NeedsCredentials
        );
        assert_eq!(
            verdict(
                "git@h: Permission denied (publickey).\nfatal: Could not read from remote repository.",
                Reach::Remote
            ),
            Why::Refused,
            "refused before unreachable: both lines come together"
        );
        assert_eq!(
            verdict(
                "fatal: unable to access 'https://h/': Could not resolve host: h",
                Reach::Remote
            ),
            Why::Unreachable
        );
        assert_eq!(
            verdict("error: open(\"x\"): Permission denied", Reach::Local),
            Why::Failed,
            "a local permission is not the remote's"
        );
    }

    /// FAILS IF: a server that would not agree a key is blamed on the wrong
    /// thing — called "not post-quantum" when the session never asked for
    /// that (the file manager's own fetch), or "unreachable" when it was
    /// reached and refused only what a post-quantum-only session may agree.
    #[test]
    fn a_key_exchange_refusal_says_whose_it_is() {
        // OpenSSH's own words, and git's line after them: the host was
        // reached, and offered nothing this session may agree.
        let ssh = "Unable to negotiate with 140.82.121.4 port 22: no matching key exchange \
                   method found. Their offer: diffie-hellman-group14-sha1\r\nfatal: Could not \
                   read from remote repository.";
        assert_eq!(
            verdict(ssh, Reach::PostQuantumOnly),
            Why::NotPostQuantum,
            "not post-quantum before unreachable"
        );
        assert_eq!(
            verdict(ssh, Reach::Remote),
            Why::NoCommonKeyExchange,
            "a session that is not narrowed offered everything: the server has only old ones"
        );
        // curl's words over OpenSSL's, MEASURED on the pinned git against a
        // server with X25519 alone.
        let tls = "fatal: unable to access 'https://127.0.0.1:26103/x.git/': TLS connect error: \
                   error:0A000410:SSL routines::ssl/tls alert handshake failure";
        assert_eq!(verdict(tls, Reach::PostQuantumOnly), Why::NotPostQuantum);
        assert_eq!(
            verdict(tls, Reach::Remote),
            Why::Unreachable,
            "a handshake that failed for a session that was not narrowed is not ours to explain"
        );

        // WHERE A CALL GOES decides which of these it can be.
        let key = |pq_only| Credentials::Ssh {
            key: PathBuf::from("/k"),
            known_hosts: PathBuf::from("/h"),
            pq_only,
        };
        let token = |pq_only| Credentials::Token {
            file: PathBuf::from("/t"),
            user: None,
            pq_only,
        };
        assert_eq!(reach_of(Some(&key(true)), "push"), Reach::PostQuantumOnly);
        assert_eq!(
            reach_of(Some(&token(true)), "fetch"),
            Reach::PostQuantumOnly
        );
        assert_eq!(reach_of(Some(&key(false)), "push"), Reach::Remote);
        assert_eq!(reach_of(Some(&token(false)), "push"), Reach::Remote);
        assert_eq!(
            reach_of(None, "fetch"),
            Reach::Remote,
            "the file manager's fetch"
        );
        assert_eq!(reach_of(None, "status"), Reach::Local);
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

    /// FAILS IF: a remote reached with our access follows a rewrite of the
    /// address from the person's global configuration or the system's
    /// (`url.<base>.insteadOf`, `pushInsteadOf`) — the consent named the
    /// address's host, and git would take the access and what is pushed
    /// elsewhere — or a call with no access of ours stops following them
    /// (the git view reaches the person's projects their way). git itself
    /// says where it would go (`remote get-url`, which expands both).
    #[test]
    fn our_access_follows_no_rewrite_of_the_address() {
        let tools = real();
        let dir = place("insteadof");
        let work = dir.join("work");
        std::fs::create_dir_all(&work).unwrap();
        fixture(&tools, &work, &["init", "-q", "-b", "main"]);
        let address = "git@example.org:me/belongings.git";
        fixture(&tools, &work, &["remote", "add", "origin", address]);
        // THE PERSON'S GLOBAL FILE, read through the home, and THE SYSTEM'S,
        // through the variable git reads it from — each sending the host
        // somewhere else.
        let rewrite = |to: &str| {
            format!(
                "[url \"{to}\"]\n\tinsteadOf = git@example.org:\n\tpushInsteadOf = git@example.org:\n"
            )
        };
        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".gitconfig"), rewrite("file:///elsewhere/")).unwrap();
        let system = dir.join("system-gitconfig");
        std::fs::write(&system, rewrite("file:///system/")).unwrap();
        let bare_home = dir.join("bare-home");
        std::fs::create_dir_all(&bare_home).unwrap();
        let ours = Credentials::Ssh {
            key: dir.join("deploy"),
            known_hosts: dir.join("known_hosts"),
            pq_only: true,
        };
        // Where git would go for `origin` (`--push`: to push), with the
        // person's home `home` and the system's file `system`.
        let get_url =
            |credentials: Option<&Credentials>, push: bool, home: &Path, system: &Path| {
                let mut command = base(&tools, &work, credentials).unwrap();
                if credentials.is_none() {
                    // What this test process inherited is not the person's.
                    command
                        .env_remove("GIT_CONFIG_GLOBAL")
                        .env_remove("GIT_CONFIG_NOSYSTEM");
                }
                command
                    .env("HOME", home)
                    .env_remove("XDG_CONFIG_HOME")
                    .env("GIT_CONFIG_SYSTEM", system);
                subcommand(&mut command, "remote").arg("get-url");
                if push {
                    command.arg("--push");
                }
                let out = command.arg("origin").output().unwrap();
                assert!(
                    out.status.success(),
                    "{}",
                    String::from_utf8_lossy(&out.stderr)
                );
                String::from_utf8_lossy(&out.stdout).trim().to_owned()
            };
        let none = Path::new("/dev/null");
        for push in [false, true] {
            // WITH OUR ACCESS: the address, whatever either file says.
            assert_eq!(
                get_url(Some(&ours), push, &home, none),
                address,
                "global, push {push}"
            );
            assert_eq!(
                get_url(Some(&ours), push, &bare_home, &system),
                address,
                "system, push {push}"
            );
            // WITHOUT: the person's own rewrites hold — and so the files
            // above do say something.
            assert_eq!(
                get_url(None, push, &home, none),
                "file:///elsewhere/me/belongings.git",
                "global, push {push}"
            );
            assert_eq!(
                get_url(None, push, &bare_home, &system),
                "file:///system/me/belongings.git",
                "system, push {push}"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
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

    // ---- against a real sshd -----------------------------------------

    /// An `sshd` of this test's own, on the loopback, agreeing only `kex`,
    /// stopped when dropped.
    struct Sshd {
        child: std::process::Child,
        port: u16,
    }

    impl Drop for Sshd {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// A program the `belongings-ssh-is-pq-only` check hands this test by
    /// its store path. Missing, the test FAILS: it is never run by a plain
    /// `cargo test` (it is ignored there), and run by the check it must run.
    fn given(variable: &str) -> PathBuf {
        let path = std::env::var_os(variable)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                panic!("{variable} is not set: `belongings-ssh-is-pq-only` runs this test")
            });
        assert!(path.is_absolute(), "{variable} is not a store path");
        path
    }

    /// An ed25519 key at `path`, no passphrase.
    fn key_at(path: &Path) {
        let made = Command::new(given("RONOCTUA_SSH_KEYGEN"))
            .args(["-q", "-t", "ed25519", "-N", "", "-C", "test", "-f"])
            .arg(path)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );
    }

    fn sshd(dir: &Path, kex: &str) -> Sshd {
        let host_key = dir.join("host_ed25519");
        if !host_key.exists() {
            key_at(&host_key);
        }
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .unwrap()
            .port();
        let config = dir.join(format!("sshd_config-{port}"));
        // NOBODY CAN LOG IN: no authorized key, no password. The session is
        // refused AFTER the key exchange, which is the half under test.
        std::fs::write(
            &config,
            format!(
                "ListenAddress 127.0.0.1\nPort {port}\nHostKey {}\nKexAlgorithms {kex}\n\
                 PidFile none\nStrictModes no\nUsePAM no\nPasswordAuthentication no\n\
                 KbdInteractiveAuthentication no\nAuthorizedKeysFile none\n",
                host_key.display()
            ),
        )
        .unwrap();
        let log = std::fs::File::create(dir.join(format!("sshd-{port}.log"))).unwrap();
        let child = Command::new(given("RONOCTUA_SSHD"))
            .args(["-D", "-e", "-f"])
            .arg(&config)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()
            .unwrap();
        let server = Sshd { child, port };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(
                std::time::Instant::now() < deadline,
                "sshd did not listen: {}",
                std::fs::read_to_string(dir.join(format!("sshd-{port}.log"))).unwrap_or_default()
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        server
    }

    /// FAILS IF: the belongings' session (`pq_only`) agrees a classical key
    /// with a server that offers nothing else, the refusal reads as anything
    /// but `NotPostQuantum`, a server offering ML-KEM — or only sntrup761, as
    /// GitHub until 2026-10-14 — is refused, or the list drifts from the
    /// system's (`RONOCTUA_SSH_PQ_KEX`, out of `assets/ssh.nix`).
    ///
    /// Ignored in a plain `cargo test`, which has no `sshd`; the
    /// `belongings-ssh-is-pq-only` check runs it on its own, with one.
    #[test]
    #[ignore = "needs sshd: run by the belongings-ssh-is-pq-only check"]
    fn a_post_quantum_only_session_refuses_a_classical_server() {
        let system = std::env::var("RONOCTUA_SSH_PQ_KEX")
            .expect("RONOCTUA_SSH_PQ_KEX is not set: `belongings-ssh-is-pq-only` runs this test");
        assert_eq!(
            system.split(',').collect::<Vec<_>>(),
            PQ_KEX,
            "assets/ssh.nix"
        );

        let tools = real();
        assert!(tools.ssh.is_some(), "RONOCTUA_SSH is not set");
        let dir = place("pq-only");
        let client_key = dir.join("client_ed25519");
        key_at(&client_key);
        let credentials = |pq_only| Credentials::Ssh {
            key: client_key.clone(),
            known_hosts: dir.join("known_hosts"),
            pq_only,
        };
        let work = dir.join("work");
        std::fs::create_dir_all(&work).unwrap();
        fixture(&tools, &work, &["init", "-q", "-b", "main"]);
        fixture(
            &tools,
            &work,
            &["remote", "add", "origin", "ssh://127.0.0.1:1/x.git"],
        );
        let fetch_from = |server: &Sshd, pq_only: bool| {
            fixture(
                &tools,
                &work,
                &[
                    "remote",
                    "set-url",
                    "origin",
                    &format!("ssh://127.0.0.1:{}/x.git", server.port),
                ],
            );
            opened(&tools, &work)
                .fetch(Some(&credentials(pq_only)))
                .unwrap_err()
                .why
        };

        let classical = sshd(&dir, "curve25519-sha256");
        assert_eq!(fetch_from(&classical, true), Why::NotPostQuantum);
        assert_eq!(
            fetch_from(&classical, false),
            Why::Refused,
            "the same server is reached by a session that is not narrowed: \
             the refusal above is the narrowing's"
        );
        drop(classical);

        for kex in [
            "mlkem768x25519-sha256",
            "sntrup761x25519-sha512@openssh.com",
        ] {
            let quantum = sshd(&dir, kex);
            assert_eq!(
                fetch_from(&quantum, true),
                Why::Refused,
                "{kex}: the key was agreed and only the login refused"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    // ---- against a real TLS server -----------------------------------

    /// An `openssl s_server` of this test's own, on the loopback, agreeing
    /// only what `limits` allows, answering every request with a page —
    /// stopped when dropped.
    struct TlsServer {
        child: std::process::Child,
        port: u16,
    }

    impl Drop for TlsServer {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    /// `openssl`, under no configuration but its own defaults: the test's
    /// server and certificate are never shaped by the machine running it.
    fn openssl() -> Command {
        let mut command = Command::new(given("RONOCTUA_OPENSSL"));
        command
            .env("OPENSSL_CONF", "/dev/null")
            .stdin(Stdio::null());
        command
    }

    /// A P-256 certificate for `127.0.0.1` and its key, in `dir`, made once:
    /// `(cert.pem, key.pem)`.
    fn certificate(dir: &Path) -> (PathBuf, PathBuf) {
        let (cert, key) = (dir.join("cert.pem"), dir.join("key.pem"));
        if !cert.exists() {
            let made = openssl()
                .args(["req", "-x509", "-newkey", "ec", "-pkeyopt"])
                .args(["ec_paramgen_curve:P-256", "-days", "2", "-nodes"])
                .args([
                    "-subj",
                    "/CN=127.0.0.1",
                    "-addext",
                    "subjectAltName=IP:127.0.0.1",
                ])
                .arg("-keyout")
                .arg(&key)
                .arg("-out")
                .arg(&cert)
                .output()
                .unwrap();
            assert!(
                made.status.success(),
                "{}",
                String::from_utf8_lossy(&made.stderr)
            );
        }
        (cert, key)
    }

    fn tls_server(dir: &Path, limits: &[&str]) -> TlsServer {
        let (cert, key) = certificate(dir);
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .unwrap()
            .port();
        let child = openssl()
            .args(["s_server", "-quiet", "-www", "-accept"])
            .arg(format!("127.0.0.1:{port}"))
            .arg("-cert")
            .arg(&cert)
            .arg("-key")
            .arg(&key)
            .args(limits)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let server = TlsServer { child, port };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(
                std::time::Instant::now() < deadline,
                "openssl s_server {limits:?} did not listen"
            );
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        server
    }

    /// FAILS IF: the belongings' token (`pq_only`) agrees a classical key
    /// with a TLS server that offers nothing else — X25519 alone, the NIST
    /// curves alone, TLS 1.2 alone — the refusal reads as anything but
    /// `NotPostQuantum`, or a server offering a hybrid ML-KEM group is
    /// refused. The configuration is the one the build ships
    /// (`RONOCTUA_GIT_PQ_TLS`), with the git the build ships: OpenSSL ignores
    /// a configuration it cannot use without a word, so only this shows the
    /// narrowing is there.
    ///
    /// Ignored in a plain `cargo test`, which has no `openssl` program; the
    /// `belongings-https-is-pq-only` check runs it on its own, with one.
    #[test]
    #[ignore = "needs openssl s_server: run by the belongings-https-is-pq-only check"]
    fn a_post_quantum_only_token_refuses_a_classical_tls_server() {
        let tools = real();
        let narrowing = tools
            .pq_tls
            .clone()
            .expect("RONOCTUA_GIT_PQ_TLS is not set: the build ships the narrowing");
        assert!(narrowing.is_file(), "{}", narrowing.display());
        let dir = place("pq-tls");
        let token = dir.join("token");
        std::fs::write(&token, b"a-token\n").unwrap();
        std::fs::set_permissions(&token, std::fs::Permissions::from_mode(0o600)).unwrap();
        let credentials = |pq_only| Credentials::Token {
            file: token.clone(),
            user: None,
            pq_only,
        };
        let work = dir.join("work");
        std::fs::create_dir_all(&work).unwrap();
        fixture(&tools, &work, &["init", "-q", "-b", "main"]);
        fixture(
            &tools,
            &work,
            &["remote", "add", "origin", "https://127.0.0.1:1/x.git"],
        );
        let fetch_from = |server: &TlsServer, pq_only: bool| {
            fixture(
                &tools,
                &work,
                &[
                    "remote",
                    "set-url",
                    "origin",
                    &format!("https://127.0.0.1:{}/x.git", server.port),
                ],
            );
            opened(&tools, &work)
                .fetch(Some(&credentials(pq_only)))
                .unwrap_err()
                .why
        };
        // THE TEST'S OWN AUTHORITY, in the repository: the certificate
        // verifies, so what is left to fail is the key exchange.
        let (cert, _) = certificate(&dir);
        fixture(
            &tools,
            &work,
            &["config", "http.sslCAInfo", cert.to_str().unwrap()],
        );

        // REACHED is what a page that is no repository answers: git got past
        // TLS, asked for `info/refs`, and read HTML.
        let reached = Why::Failed;
        for limits in [
            &["-groups", "X25519"][..],
            &["-groups", "P-256:P-384"][..],
            &["-tls1_2"][..],
        ] {
            let classical = tls_server(&dir, limits);
            assert_eq!(
                fetch_from(&classical, true),
                Why::NotPostQuantum,
                "{limits:?}"
            );
            assert_eq!(
                fetch_from(&classical, false),
                reached,
                "{limits:?}: the same server is reached by a token that is not narrowed — \
                 the refusal above is the narrowing's"
            );
        }
        for group in ["X25519MLKEM768", "SecP384r1MLKEM1024"] {
            let quantum = tls_server(&dir, &["-groups", group]);
            assert_eq!(
                fetch_from(&quantum, true),
                reached,
                "{group}: the key was agreed, and only the page was not a repository"
            );
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a rewritten commit loses its tree, message or dates, a
    /// lease another push broke still forces, or a name that is not one
    /// becomes an argument.
    #[test]
    fn a_history_is_rewritten_and_forced_only_under_its_lease() {
        let tools = real();
        let dir = place("rewrite");
        let bare = dir.join("remote.git");
        std::fs::create_dir_all(&bare).unwrap();
        fixture(&tools, &bare, &["init", "-q", "--bare", "-b", "main"]);
        let work = dir.join("work");
        std::fs::create_dir_all(&work).unwrap();
        fixture(&tools, &work, &["init", "-q", "-b", "main"]);
        for n in 0..3 {
            std::fs::write(work.join("f"), format!("{n}")).unwrap();
            fixture(&tools, &work, &["add", "-A"]);
            fixture(&tools, &work, &["commit", "-q", "-m", &format!("c{n}")]);
        }
        fixture(
            &tools,
            &work,
            &["remote", "add", "origin", &bare.to_string_lossy()],
        );
        fixture(&tools, &work, &["push", "-q", "origin", "main"]);
        let head = Oid(fixture(&tools, &work, &["rev-parse", "HEAD"]));
        let tree = Oid(fixture(&tools, &work, &["rev-parse", "HEAD^{tree}"]));
        let repo = opened(&tools, &work);

        // THE HEAD'S TREE AS A NEW ROOT, its message and dates kept.
        let signed = Signature {
            name: "Ada".to_owned(),
            email: "ada@example.org".to_owned(),
            date: "1700000000 +0000".to_owned(),
        };
        let root = repo
            .commit_tree(&Recommit {
                tree: tree.clone(),
                parent: None,
                message: "c2".to_owned(),
                author: signed.clone(),
                committer: signed.clone(),
            })
            .unwrap();
        assert_ne!(root, head);
        assert_eq!(
            fixture(
                &tools,
                &work,
                &["log", "--format=%T|%s|%ad|%cd", "--date=raw", &root.0]
            ),
            format!("{}|c2|1700000000 +0000|1700000000 +0000", tree.0),
            "one commit, the same tree, message and dates"
        );

        // A LEASE ON THE HEAD IT FETCHED forces the new root up.
        repo.push_force_with_lease(None, "origin", &root, "main", &head)
            .unwrap();
        assert_eq!(fixture(&tools, &bare, &["rev-parse", "main"]), root.0);
        // AND A LEASE ANOTHER PUSH BROKE refuses, leaving the other's push.
        assert!(
            repo.push_force_with_lease(None, "origin", &head, "main", &head)
                .is_err()
        );
        assert_eq!(fixture(&tools, &bare, &["rev-parse", "main"]), root.0);

        // NOTHING THAT IS NOT A NAME becomes an argument.
        for (remote, branch) in [("--exec=x", "main"), ("origin", "-x"), ("origin", "a..b")] {
            assert_eq!(
                repo.push_force_with_lease(None, remote, &root, branch, &root)
                    .unwrap_err()
                    .why,
                Why::Garbled
            );
        }
        let short = Oid(root.0[..8].to_owned());
        assert_eq!(
            repo.push_force_with_lease(None, "origin", &short, "main", &root)
                .unwrap_err()
                .why,
            Why::Garbled
        );
        let forged = Signature {
            name: "Ada\nCommitter: Eve".to_owned(),
            ..signed.clone()
        };
        let refused = repo.commit_tree(&Recommit {
            tree,
            parent: Some(root),
            message: "x".to_owned(),
            author: forged,
            committer: signed,
        });
        assert_eq!(refused.unwrap_err().why, Why::Garbled);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: two changes in different hunks are not joined, a hunk both
    /// changed is not counted, a favour does not settle it to its side, the
    /// union does not keep both lines, or the three sides outlive the call.
    #[test]
    fn merge_file_joins_hunks_and_counts_the_clashes() {
        let tools = real();
        let dir = place("merge-file");
        let base_text = b"one\ntwo\nthree\nfour\nfive\nsix\nseven\n";
        let ours = b"ONE\ntwo\nthree\nfour\nfive\nsix\nseven\n";
        let theirs = b"one\ntwo\nthree\nfour\nfive\nsix\nSEVEN\n";
        let clean = merge_file(&tools, &dir, base_text, ours, theirs, Favor::None).unwrap();
        assert_eq!(clean.conflicts, 0);
        assert_eq!(clean.bytes, b"ONE\ntwo\nthree\nfour\nfive\nsix\nSEVEN\n");

        let theirs = b"uno\ntwo\nthree\nfour\nfive\nsix\nSEVEN\n";
        let clash = merge_file(&tools, &dir, base_text, ours, theirs, Favor::None).unwrap();
        assert_eq!(clash.conflicts, 1);
        assert!(String::from_utf8_lossy(&clash.bytes).contains("<<<<<<< ours"));
        let mine = merge_file(&tools, &dir, base_text, ours, theirs, Favor::Ours).unwrap();
        assert_eq!(mine.conflicts, 0);
        assert_eq!(mine.bytes, b"ONE\ntwo\nthree\nfour\nfive\nsix\nSEVEN\n");
        let other = merge_file(&tools, &dir, base_text, ours, theirs, Favor::Theirs).unwrap();
        assert_eq!(other.bytes, b"uno\ntwo\nthree\nfour\nfive\nsix\nSEVEN\n");
        let both = merge_file(&tools, &dir, base_text, ours, theirs, Favor::Union).unwrap();
        assert_eq!(
            both.bytes,
            b"ONE\nuno\ntwo\nthree\nfour\nfive\nsix\nSEVEN\n"
        );
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            0,
            "the three sides outlived the merge"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a global setting is written anywhere but the person's
    /// `~/.gitconfig` (this process's `HOME` or `GIT_CONFIG_GLOBAL` would
    /// win), a second write leaves two values, an unset key is an error, or
    /// a key git would read as an option or a value with a line break in it
    /// is taken.
    #[test]
    fn a_global_setting_is_the_persons_and_replaces_itself() {
        let tools = real();
        let home = place("global-config");
        set_global(&tools, &home, "gpg.format", "ssh").unwrap();
        set_global(&tools, &home, "gpg.format", "ssh").unwrap();
        set_global(&tools, &home, "user.signingkey", "~/.ssh/id_ed25519.pub").unwrap();
        let file = std::fs::read_to_string(home.join(".gitconfig")).unwrap();
        assert_eq!(file.matches("format = ssh").count(), 1, "{file}");
        assert_eq!(
            get_global(&tools, &home, "user.signingkey")
                .unwrap()
                .as_deref(),
            Some("~/.ssh/id_ed25519.pub")
        );
        assert_eq!(get_global(&tools, &home, "commit.gpgsign").unwrap(), None);
        for key in ["--add", "nosection", "a..b", ".x", "a.b c"] {
            assert_eq!(
                set_global(&tools, &home, key, "v").unwrap_err().why,
                Why::Garbled,
                "{key}"
            );
        }
        assert_eq!(
            set_global(&tools, &home, "user.name", "a\n[core]\n\tpager = x")
                .unwrap_err()
                .why,
            Why::Garbled
        );
        let _ = std::fs::remove_dir_all(home);
    }

    /// FAILS IF: a value added to a key that holds several replaces the
    /// others or is added twice; a setting in a file of its own is not
    /// written there, or the file's private mode is lost; what an included
    /// file says is not read as the person's — or an include whose file is
    /// missing is an error (a machine without the file reads nothing from
    /// it).
    #[test]
    fn an_included_file_counts_and_is_added_once() {
        use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
        let tools = real();
        let home = place("global-include");
        add_global(&tools, &home, "include.path", "~/.other.inc").unwrap();
        add_global(&tools, &home, "include.path", "~/.ssh/signing.inc").unwrap();
        add_global(&tools, &home, "include.path", "~/.ssh/signing.inc").unwrap();
        let file = std::fs::read_to_string(home.join(".gitconfig")).unwrap();
        assert_eq!(file.matches("path = ").count(), 2, "{file}");
        assert!(file.contains("~/.other.inc"), "{file}");
        // MISSING: nothing read from it, and no error.
        assert_eq!(get_global(&tools, &home, "commit.gpgsign").unwrap(), None);
        std::fs::create_dir_all(home.join(".ssh")).unwrap();
        let inc = home.join(".ssh/signing.inc");
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&inc)
            .unwrap();
        set_in_file(&tools, &inc, "commit.gpgsign", "true").unwrap();
        set_in_file(&tools, &inc, "commit.gpgsign", "true").unwrap();
        let text = std::fs::read_to_string(&inc).unwrap();
        assert_eq!(text.matches("gpgsign = true").count(), 1, "{text}");
        assert_eq!(
            std::fs::metadata(&inc).unwrap().permissions().mode() & 0o777,
            0o600,
            "git widened the file it rewrote"
        );
        assert_eq!(
            get_global(&tools, &home, "commit.gpgsign")
                .unwrap()
                .as_deref(),
            Some("true"),
            "what the included file says is not the person's"
        );
        assert_eq!(
            set_in_file(&tools, Path::new("relative.inc"), "a.b", "c")
                .unwrap_err()
                .why,
            Why::Garbled
        );
        let _ = std::fs::remove_dir_all(home);
    }

    // ---- the clone -----------------------------------------------------

    #[cfg(feature = "clone")]
    #[test]
    fn an_address_names_its_host_or_none() {
        for (url, host) in [
            (
                "https://github.com/ronoctua/ronoctua-distro.git",
                Some("github.com"),
            ),
            ("HTTPS://GitHub.com./a", Some("github.com")),
            (
                "https://user:token@Example.org:8443/a.git",
                Some("example.org"),
            ),
            ("ssh://git@example.org:22/a.git", Some("example.org")),
            ("git://127.0.0.1:9418/a.git", Some("127.0.0.1")),
            (
                "git@github.com:ronoctua/ronoctua-distro.git",
                Some("github.com"),
            ),
            ("github.com:a/b", Some("github.com")),
            ("file:///srv/a.git", None),
            ("/srv/a.git", None),
            ("./a:b", None),
            ("ext::sh -c touch% /tmp/x", None),
            // A REMOTE HELPER'S, whatever follows: never a host named after
            // the helper.
            ("ext::true", None),
            ("fd::17", None),
            ("gcrypt::git@example.org:a.git", None),
            ("gcrypt::rsync://example.org/a", None),
            ("persistent-https::example.org/a", None),
            // `::` after a `/` or a `:` is no helper's.
            ("https://example.org/a::b", Some("example.org")),
            ("--upload-pack=touch x", None),
            ("ftp://example.org/a", None),
            ("https://[::1]/a", None),
            ("https:///a", None),
        ] {
            assert_eq!(host_of(url).as_deref(), host, "{url}");
        }
    }

    /// A pass for `host`, over a consent file of this test's own.
    #[cfg(feature = "clone")]
    fn pass_for(dir: &Path, host: &str) -> ronoctua_consent::Pass {
        let store = ronoctua_consent::store_at(Some(dir.join("consent")));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        ronoctua_consent::pass_in(&store, host).unwrap()
    }

    /// A folder of a clone test's own, under the temporary directory (run
    /// with `TMPDIR` pointing where scratch belongs), removed when dropped —
    /// on a panic too.
    #[cfg(feature = "clone")]
    struct Scratch(PathBuf);

    #[cfg(feature = "clone")]
    impl Scratch {
        fn new(name: &str) -> Self {
            Self(place(name))
        }
    }

    #[cfg(feature = "clone")]
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(feature = "clone")]
    #[test]
    fn a_clone_asks_only_the_host_its_pass_lets() {
        let tools = real();
        let scratch = Scratch::new("clone-refused");
        let dir = &scratch.0;
        let other = pass_for(dir, "example.org");
        let into = dir.join("into");
        for (url, into) in [
            // The pass is another host's.
            (
                "https://github.com/ronoctua/ronoctua-distro.git",
                into.as_path(),
            ),
            // No host at all: a path is never what a pass lets.
            ("/srv/a.git", into.as_path()),
            ("file:///srv/a.git", into.as_path()),
            // An option where the address goes.
            ("--upload-pack=touch x", into.as_path()),
            // A folder that is not absolute.
            ("https://example.org/a.git", Path::new("relative")),
        ] {
            let refusal = Repo::clone_with(&tools, &other, url, into).unwrap_err();
            assert_eq!(refusal.why, Why::Garbled, "{url}");
        }
        assert!(!into.exists(), "a refused clone made its folder");
        assert!(!Path::new("relative").exists());
    }

    /// A clone made ready by hand over `effective`, for a look at its
    /// commands — nothing is run.
    #[cfg(feature = "clone")]
    fn ready_over(effective: &str) -> Prepared {
        Prepared {
            tools: fake_tools(),
            settings: Settings::persons(),
            url: "https://github.com/ronoctua/ronoctua-distro.git".to_owned(),
            effective: effective.to_owned(),
            into: PathBuf::from("/home/ada/ronoctua-distro"),
            made: true,
        }
    }

    /// The variables a command sets.
    #[cfg(feature = "clone")]
    fn envs_of(command: &Command) -> BTreeMap<String, String> {
        command
            .get_envs()
            .filter_map(|(k, v)| {
                Some((
                    k.to_string_lossy().into_owned(),
                    v?.to_string_lossy().into_owned(),
                ))
            })
            .collect()
    }

    /// FAILS IF: the fetch's command stops refusing a redirect — the plain
    /// key, and the one for the address it goes to, which a rule of the
    /// person's for that address would otherwise beat —, loses its low-speed
    /// limit, or is not pinned to where the address was found to lead.
    #[cfg(feature = "clone")]
    #[test]
    fn the_fetch_follows_no_redirect_gives_up_on_a_stalled_line_and_is_pinned() {
        let effective = "https://mirror.example.org/github/ronoctua/ronoctua-distro.git";
        let command = ready_over(effective).fetch_command().unwrap();
        let args: Vec<String> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let fetch_at = args.iter().position(|a| a == "fetch").unwrap();
        for pair in [
            "http.followRedirects=false".to_owned(),
            format!("http.lowSpeedLimit={CLONE_LOW_SPEED_BYTES_PER_S}"),
            format!("http.lowSpeedTime={CLONE_LOW_SPEED_S}"),
        ] {
            let at = args.iter().position(|a| *a == pair).unwrap_or_else(|| {
                panic!("{pair} is not in {args:?}");
            });
            // A `-c` pair is git's, so it goes before the subcommand.
            assert!(at < fetch_at && args[at - 1] == "-c", "{args:?}");
        }
        assert!(
            args.ends_with(&["--tags".to_owned(), "origin".to_owned()]),
            "{args:?}"
        );
        let envs = envs_of(&command);
        assert_eq!(envs.get("GIT_CONFIG_COUNT").map(String::as_str), Some("2"));
        assert_eq!(
            envs.get("GIT_CONFIG_KEY_0").map(String::as_str),
            Some(&*format!("url.{effective}.insteadOf"))
        );
        assert_eq!(
            envs.get("GIT_CONFIG_VALUE_0").map(String::as_str),
            Some("https://github.com/ronoctua/ronoctua-distro.git")
        );
        assert_eq!(
            envs.get("GIT_CONFIG_KEY_1").map(String::as_str),
            Some(&*format!("http.{effective}.followRedirects"))
        );
        assert_eq!(
            envs.get("GIT_CONFIG_VALUE_1").map(String::as_str),
            Some("false")
        );
        // OVER SSH THERE IS NO REDIRECT to refuse: the pin alone.
        let by_ssh = ready_over("git@github.com:ronoctua/ronoctua-distro.git")
            .fetch_command()
            .unwrap();
        assert_eq!(
            envs_of(&by_ssh).get("GIT_CONFIG_COUNT").map(String::as_str),
            Some("1")
        );
    }

    /// A home of a test's own under `dir`, its `.gitconfig` saying `config`.
    #[cfg(feature = "clone")]
    fn home_saying(dir: &Path, name: &str, config: &str) -> PathBuf {
        let home = dir.join(name);
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join(".gitconfig"), config).unwrap();
        home
    }

    /// FAILS IF: where a clone would go, read from `/`, is not the person's
    /// own rewrite of the address — from their global file or the system's
    /// —, a push's rewrite counts for a clone, or an address that names no
    /// host is read at all. The three cases `LEADER / c` asks about: no
    /// rewrite, a rewrite that keeps the host (https → ssh), one that sends
    /// it to another host.
    #[cfg(feature = "clone")]
    #[test]
    fn where_an_address_leads_is_the_persons_rewrite_of_it() {
        let tools = real();
        let scratch = Scratch::new("effective");
        let dir = &scratch.0;
        let address = "https://github.com/ronoctua/ronoctua-distro.git";
        let none = Path::new("/dev/null");
        let leads = |home: &Path, system: &Path| {
            effective_url_in(&tools, &Settings::planted(home, system), address).unwrap()
        };
        // NO REWRITE: the address itself.
        let bare = home_saying(dir, "bare", "");
        assert_eq!(leads(&bare, none), address);
        // THE SAME HOST, by another way: the common https → ssh.
        let ssh = home_saying(
            dir,
            "ssh",
            "[url \"git@github.com:\"]\n\tinsteadOf = https://github.com/\n",
        );
        let to = leads(&ssh, none);
        assert_eq!(to, "git@github.com:ronoctua/ronoctua-distro.git");
        assert_eq!(host_of(&to).as_deref(), Some("github.com"));
        // ANOTHER HOST.
        let mirror = home_saying(
            dir,
            "mirror",
            "[url \"https://mirror.example.org/github/\"]\n\tinsteadOf = https://github.com/\n",
        );
        let to = leads(&mirror, none);
        assert_eq!(
            to,
            "https://mirror.example.org/github/ronoctua/ronoctua-distro.git"
        );
        assert_eq!(host_of(&to).as_deref(), Some("mirror.example.org"));
        // A PUSH'S REWRITE is a push's.
        let push = home_saying(
            dir,
            "push",
            "[url \"git@elsewhere.example.org:\"]\n\tpushInsteadOf = https://github.com/\n",
        );
        assert_eq!(leads(&push, none), address);
        // THE SYSTEM'S FILE counts, as it does for the clone.
        let system = dir.join("system-gitconfig");
        std::fs::write(
            &system,
            "[url \"ssh://git@forge.example.net/\"]\n\tinsteadOf = https://github.com/\n",
        )
        .unwrap();
        assert_eq!(
            host_of(&leads(&bare, &system)).as_deref(),
            Some("forge.example.net")
        );
        // A REWRITE TO A PATH, or TO A REMOTE HELPER, leads to no host — the
        // clone refuses both.
        for (to, rule) in [
            ("/srv/mirror/", "/srv/mirror/ronoctua/ronoctua-distro.git"),
            (
                "gcrypt::git@example.org:",
                "gcrypt::git@example.org:ronoctua/ronoctua-distro.git",
            ),
        ] {
            let home = home_saying(
                dir,
                "nowhere",
                &format!("[url \"{to}\"]\n\tinsteadOf = https://github.com/\n"),
            );
            let led = leads(&home, none);
            assert_eq!(led, rule);
            assert_eq!(host_of(&led), None, "{led}");
        }
        // AN ADDRESS THAT NAMES NO HOST is never read.
        for url in [
            "/srv/a.git",
            "--upload-pack=touch x",
            "file:///srv/a.git",
            "ext::true",
        ] {
            assert_eq!(
                effective_url(&tools, url).unwrap_err().why,
                Why::Garbled,
                "{url}"
            );
        }
    }

    /// FAILS IF: a remote that answers with a redirect to ANOTHER host takes
    /// the clone there — with the person's settings saying nothing of
    /// redirects, or saying to follow them for this very address
    /// (`http.<url>.followRedirects = true`, which beats a plain
    /// `-c http.followRedirects=false`; measured). A real HTTP exchange on
    /// the loopback: the first host (127.0.0.1, the one the pass lets)
    /// answers every request with a 302 to a second (127.0.0.2), which counts
    /// who reaches it and hangs up.
    #[cfg(feature = "clone")]
    #[test]
    fn a_redirect_to_another_host_is_not_followed() {
        use std::io::{Read as _, Write as _};
        use std::sync::atomic::{AtomicUsize, Ordering};
        let tools = real();
        let scratch = Scratch::new("clone-redirect");
        let dir = &scratch.0;
        let elsewhere = Elsewhere::listening();
        let elsewhere_port = elsewhere.port;
        let first = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = first.local_addr().unwrap().port();
        let served = std::sync::Arc::new(AtomicUsize::new(0));
        let counted = std::sync::Arc::clone(&served);
        std::thread::spawn(move || {
            for stream in first.incoming() {
                let Ok(mut stream) = stream else { continue };
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);
                counted.fetch_add(1, Ordering::SeqCst);
                let _ = write!(
                    stream,
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.2:{elsewhere_port}/distro.git/info/refs?service=git-upload-pack\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
        });
        let pass = pass_for(dir, "127.0.0.1");
        let url = format!("http://127.0.0.1:{port}/distro.git");
        let none = Path::new("/dev/null");
        for (name, config) in [
            ("silent", String::new()),
            (
                "follows",
                format!(
                    "[http \"http://127.0.0.1:{port}/distro.git\"]\n\tfollowRedirects = true\n"
                ),
            ),
        ] {
            let home = home_saying(dir, name, &config);
            let into = dir.join(format!("into-{name}"));
            let settings = Settings::planted(&home, none);
            let refusal = Repo::prepare_clone(&tools, &settings, &url, &into)
                .and_then(|p| p.fetch(&pass))
                .unwrap_err();
            assert!(
                served.load(Ordering::SeqCst) >= 1,
                "{name}: the first host was never asked: {refusal:?}"
            );
            assert_eq!(
                elsewhere.reached(),
                0,
                "{name}: the clone followed the redirect to a host nobody let"
            );
            assert!(!into.exists(), "{name}: a failed clone left its folder");
        }
    }

    /// FAILS IF: a step of a clone that does not end is not stopped at its
    /// limit — and said as the limit's refusal —, or one that ends is.
    /// A program of the test's own in git's place, which never ends.
    #[cfg(feature = "clone")]
    #[test]
    fn a_step_that_does_not_end_is_stopped() {
        let scratch = Scratch::new("clone-stalled");
        let dir = &scratch.0;
        let stalls = dir.join("stalls");
        let status = Command::new("/bin/sh")
            .arg("-c")
            .arg("printf '#!/bin/sh\\nwhile :; do :; done\\n' > \"$1\" && chmod 755 \"$1\"")
            .arg("sh")
            .arg(&stalls)
            .status()
            .unwrap();
        assert!(status.success());
        let started = std::time::Instant::now();
        let mut stalling = Command::new(&stalls);
        stalling.stdin(Stdio::null());
        let refusal = run_until(
            stalling,
            ADDRESS_MOST_BYTES,
            Some((std::time::Duration::from_millis(300), Why::Unreachable)),
        )
        .err()
        .expect("a step that never ends ended");
        assert_eq!(refusal.why, Why::Unreachable);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "it waited {:?}",
            started.elapsed()
        );
        let mut ends = Command::new("/bin/sh");
        ends.args(["-c", "exit 0"]).stdin(Stdio::null());
        let ran = run_until(
            ends,
            ADDRESS_MOST_BYTES,
            Some((std::time::Duration::from_secs(30), Why::Unreachable)),
        )
        .unwrap();
        assert!(ran.ok);
    }

    /// A real `git://` remote on a free port of the loopback, serving
    /// `base` — and it lives IN THIS PROCESS: a thread of the test holds the
    /// listening socket, and each connection is served by `git-daemon
    /// --inetd`, which this thread waits for and which ends with its
    /// connection. A listening daemon of its own would outlive a test that
    /// is killed (it did once, reparented, holding every descriptor the
    /// test had); a thread cannot. Dropped — on a panic too — the thread is
    /// woken, stops, and the port closes.
    #[cfg(feature = "clone")]
    struct Remote {
        port: u16,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        serving: Option<std::thread::JoinHandle<()>>,
    }

    #[cfg(feature = "clone")]
    impl Remote {
        fn serving(tools: &Tools, base: &Path) -> Self {
            use std::os::fd::OwnedFd;
            use std::sync::atomic::Ordering;
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let exec_path = Command::new(&tools.git)
                .arg("--exec-path")
                .output()
                .unwrap();
            let core = PathBuf::from(String::from_utf8(exec_path.stdout).unwrap().trim());
            let daemon = core.join("git-daemon");
            assert!(daemon.is_file(), "{} is not there", daemon.display());
            let base = base.to_path_buf();
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stopped = std::sync::Arc::clone(&stop);
            let serving = std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let Ok(reading) = stream.try_clone() else {
                        continue;
                    };
                    // The command and its copies of the socket go at the end
                    // of this statement, so the connection ends when the
                    // child does.
                    let child = Command::new(&daemon)
                        .args([
                            "--inetd",
                            "--export-all",
                            "--timeout=30",
                            "--init-timeout=10",
                        ])
                        .arg(format!("--base-path={}", base.display()))
                        .arg(&base)
                        .env("GIT_CONFIG_GLOBAL", "/dev/null")
                        .env("GIT_CONFIG_NOSYSTEM", "1")
                        // `git-daemon` runs `git upload-pack` by NAME, and,
                        // unlike `git` itself, never puts its own folder on
                        // the PATH: without this the server is git's only
                        // where the test's PATH happens to carry git (a
                        // workstation), and the build sandbox's does not.
                        .env("PATH", &core)
                        .stdin(Stdio::from(OwnedFd::from(reading)))
                        .stdout(Stdio::from(OwnedFd::from(stream)))
                        .stderr(Stdio::null())
                        .spawn();
                    if let Ok(mut child) = child {
                        let _ = child.wait();
                    }
                }
            });
            Self {
                port,
                stop,
                serving: Some(serving),
            }
        }
    }

    #[cfg(feature = "clone")]
    impl Drop for Remote {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            // Wakes the thread out of `accept`; it sees the flag and stops.
            let _ = std::net::TcpStream::connect(("127.0.0.1", self.port));
            if let Some(serving) = self.serving.take() {
                let _ = serving.join();
            }
        }
    }

    #[cfg(feature = "clone")]
    #[test]
    fn a_clone_makes_the_repository_and_runs_nothing_of_it() {
        let tools = real();
        let scratch = Scratch::new("clone-made");
        let dir = &scratch.0;
        let source = dir.join("source");
        std::fs::create_dir_all(&source).unwrap();
        fixture(&tools, &source, &["init", "-q", "-b", "main"]);
        std::fs::write(source.join("flake.nix"), "{ }\n").unwrap();
        // A HOSTILE CHECKOUT: an attribute naming a filter driver. A clone
        // reads no configuration of the remote's, so the driver has no
        // command — and the hook a template would bring is not brought.
        std::fs::write(source.join(".gitattributes"), "* filter=evil\n").unwrap();
        fixture(&tools, &source, &["add", "-A"]);
        fixture(&tools, &source, &["commit", "-q", "-m", "first"]);
        let bare = dir.join("served/distro.git");
        fixture(
            &tools,
            dir,
            &[
                "clone",
                "-q",
                "--bare",
                source.to_str().unwrap(),
                bare.to_str().unwrap(),
            ],
        );
        let remote = Remote::serving(&tools, &dir.join("served"));
        let pass = pass_for(dir, "127.0.0.1");
        let url = format!("git://127.0.0.1:{}/distro.git", remote.port);

        // Into a folder whose parents do not exist yet.
        let into = dir.join("home/work/distro");
        let repo = Repo::clone_with(&tools, &pass, &url, &into).unwrap();
        assert_eq!(repo.root, into);
        assert_eq!(
            std::fs::read_to_string(into.join("flake.nix")).unwrap(),
            "{ }\n"
        );
        assert!(repo.head_commit().unwrap().is_some());
        let hooks: Vec<_> = std::fs::read_dir(into.join(".git/hooks"))
            .map(|d| d.flatten().collect())
            .unwrap_or_default();
        assert!(hooks.is_empty(), "the clone brought hooks: {hooks:?}");

        // THE SAME FOLDER AGAIN: it is there and not empty, and git says so —
        // a refusal, and what was there stays.
        let again = Repo::clone_with(&tools, &pass, &url, &into).unwrap_err();
        assert_eq!(again.why, Why::Failed);
        assert!(into.join("flake.nix").exists());

        // A REPOSITORY THE REMOTE DOES NOT HAVE: a refusal, and no half
        // folder left behind.
        let missing = format!("git://127.0.0.1:{}/missing.git", remote.port);
        let lost = dir.join("lost");
        assert!(Repo::clone_with(&tools, &pass, &missing, &lost).is_err());
        assert!(!lost.exists(), "a failed clone left its folder");

        // A REMOTE THAT IS NOT THERE AT ALL: unreachable, said as such.
        let port = remote.port;
        drop(remote);
        let gone = format!("git://127.0.0.1:{port}/distro.git");
        let refusal = Repo::clone_with(&tools, &pass, &gone, &lost).unwrap_err();
        assert_eq!(refusal.why, Why::Unreachable, "{refusal:?}");
        assert!(!lost.exists(), "a failed clone left its folder");
    }

    /// A host nobody let, on `127.0.0.2`: it counts who reaches it and hangs
    /// up at once (a clone that reached it fails at once instead of waiting
    /// for an answer). Dropped, the thread is woken and stops.
    #[cfg(feature = "clone")]
    struct Elsewhere {
        port: u16,
        reached: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
        serving: Option<std::thread::JoinHandle<()>>,
    }

    #[cfg(feature = "clone")]
    impl Elsewhere {
        fn listening() -> Self {
            use std::sync::atomic::Ordering;
            let listener = std::net::TcpListener::bind("127.0.0.2:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let reached = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let counted = std::sync::Arc::clone(&reached);
            let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let stopped = std::sync::Arc::clone(&stop);
            let serving = std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stopped.load(Ordering::SeqCst) {
                        break;
                    }
                    if stream.is_ok() {
                        counted.fetch_add(1, Ordering::SeqCst);
                    }
                }
            });
            Self {
                port,
                reached,
                stop,
                serving: Some(serving),
            }
        }

        fn reached(&self) -> usize {
            self.reached.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[cfg(feature = "clone")]
    impl Drop for Elsewhere {
        fn drop(&mut self) {
            self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
            let _ = std::net::TcpStream::connect(("127.0.0.2", self.port));
            if let Some(serving) = self.serving.take() {
                let _ = serving.join();
            }
        }
    }

    /// The bare repository a clone test serves, with one commit.
    #[cfg(feature = "clone")]
    fn served_distro(tools: &Tools, dir: &Path) -> PathBuf {
        let source = dir.join("source");
        std::fs::create_dir_all(&source).unwrap();
        fixture(tools, &source, &["init", "-q", "-b", "main"]);
        std::fs::write(source.join("flake.nix"), "{ }\n").unwrap();
        fixture(tools, &source, &["add", "-A"]);
        fixture(tools, &source, &["commit", "-q", "-m", "first"]);
        let bare = dir.join("served/distro.git");
        fixture(
            tools,
            dir,
            &[
                "clone",
                "-q",
                "--bare",
                source.to_str().unwrap(),
                bare.to_str().unwrap(),
            ],
        );
        dir.join("served")
    }

    /// FAILS IF: a clone whose address the person's settings send to another
    /// host goes there with the pass of the address's host — the consent
    /// named one host and git reaches another — or does not go there with
    /// that host's own pass; the repository's `origin` stops being the
    /// address the clone was given; or a rewrite that keeps the host (only
    /// the way changes, as https → ssh) does not go on with its pass.
    #[cfg(feature = "clone")]
    #[test]
    fn a_clone_goes_where_the_address_leads_and_only_with_that_hosts_pass() {
        let tools = real();
        let scratch = Scratch::new("clone-rewritten");
        let dir = &scratch.0;
        let none = Path::new("/dev/null");
        let address = "git://distro.example.invalid/distro.git";
        // SENT ELSEWHERE, with the address's host let: nothing reaches it.
        let elsewhere = Elsewhere::listening();
        let away = home_saying(
            dir,
            "away",
            &format!(
                "[url \"git://127.0.0.2:{}/\"]\n\tinsteadOf = git://distro.example.invalid/\n",
                elsewhere.port
            ),
        );
        let named = pass_for(dir, "distro.example.invalid");
        let into = dir.join("into");
        let prepared =
            Repo::prepare_clone(&tools, &Settings::planted(&away, none), address, &into).unwrap();
        assert_eq!(prepared.host().as_deref(), Some("127.0.0.2"));
        let refusal = prepared.fetch(&named).unwrap_err();
        assert_eq!(refusal.why, Why::Garbled);
        assert_eq!(
            elsewhere.reached(),
            0,
            "the clone reached a host nobody let"
        );
        assert!(!into.exists(), "a refused clone left its folder");
        // SENT TO A SERVED HOST, with THAT host let: the clone is made.
        let remote = Remote::serving(&tools, &served_distro(&tools, dir));
        let served = home_saying(
            dir,
            "served-home",
            &format!(
                "[url \"git://127.0.0.1:{}/\"]\n\tinsteadOf = git://distro.example.invalid/\n",
                remote.port
            ),
        );
        let let_there = pass_for(dir, "127.0.0.1");
        let repo = Repo::prepare_clone(&tools, &Settings::planted(&served, none), address, &into)
            .and_then(|p| p.fetch(&let_there))
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(into.join("flake.nix")).unwrap(),
            "{ }\n"
        );
        assert!(repo.head_commit().unwrap().is_some());
        // ITS `origin` IS THE ADDRESS, alone: the person's rewrite goes on
        // working; and the branch tracks it.
        assert_eq!(
            fixture(&tools, &into, &["config", "--get-all", "remote.origin.url"]).trim(),
            address
        );
        assert_eq!(
            fixture(&tools, &into, &["config", "branch.main.remote"]).trim(),
            "origin"
        );
        // THE SAME HOST BY ANOTHER WAY (as https → ssh): the address's own
        // host's pass, and the clone goes on.
        let same = format!("http://127.0.0.1:{}/distro.git", remote.port);
        let by_git = home_saying(
            dir,
            "same-host",
            &format!(
                "[url \"git://127.0.0.1:{0}/\"]\n\tinsteadOf = http://127.0.0.1:{0}/\n",
                remote.port
            ),
        );
        let again = dir.join("again");
        Repo::prepare_clone(&tools, &Settings::planted(&by_git, none), &same, &again)
            .and_then(|p| p.fetch(&let_there))
            .unwrap();
        assert!(again.join("flake.nix").exists());
    }

    /// FAILS IF: the clone does not go where the person's settings send it
    /// FROM THE NEW REPOSITORY — read there, as its fetch reads them —, but
    /// where they send the bare address from anywhere else. Five cases, each
    /// measured on the pinned git: no rewrite; a global `remote.origin.url`,
    /// which a plain `git clone` reads before the address it was given; a
    /// rewrite of the WHOLE address in a file included for the new folder
    /// (`includeIf "gitdir:…"`) or for the address
    /// (`"hasconfig:remote.*.url:…"`) — a tie a rule of ours for the same
    /// address loses, the first read winning —; and the same with a SHORTER
    /// part of the address, which a whole-address rule of ours would have
    /// beaten, leaving the person's settings unread. Where the fetch goes is
    /// said, and only its host's pass fetches: nothing else is reached.
    #[cfg(feature = "clone")]
    #[test]
    fn a_clone_reads_where_it_goes_in_the_new_repository() {
        let tools = real();
        let scratch = Scratch::new("clone-in-place");
        let dir = &scratch.0;
        let none = Path::new("/dev/null");
        let address = "git://distro.example.invalid/distro.git";
        let remote = Remote::serving(&tools, &served_distro(&tools, dir));
        let elsewhere = Elsewhere::listening();
        let served = format!("git://127.0.0.1:{}/", remote.port);
        let away = format!("git://127.0.0.2:{}/", elsewhere.port);
        let unconditional =
            format!("[url \"{served}\"]\n\tinsteadOf = git://distro.example.invalid/\n");
        let let_served = pass_for(dir, "127.0.0.1");
        let work = dir.join("work");
        let into = work.join("distro");
        let included = dir.join("included.gitconfig");
        for (case, config, rule) in [
            ("no rewrite", String::new(), None),
            (
                "a global origin",
                format!("[remote \"origin\"]\n\turl = {away}elsewhere.git\n"),
                None,
            ),
            (
                "the whole address, for the folder",
                format!(
                    "{unconditional}[includeIf \"gitdir:{}/\"]\n\tpath = {}\n",
                    work.display(),
                    included.display()
                ),
                Some(address.to_owned()),
            ),
            (
                "the whole address, for the address",
                format!(
                    "{unconditional}[includeIf \"hasconfig:remote.*.url:git://distro.example.invalid/**\"]\n\tpath = {}\n",
                    included.display()
                ),
                Some(address.to_owned()),
            ),
            (
                "a shorter part, for the folder",
                format!(
                    "{unconditional}[includeIf \"gitdir:{}/\"]\n\tpath = {}\n",
                    work.display(),
                    included.display()
                ),
                Some("git://distro.example.invalid/distro".to_owned()),
            ),
        ] {
            if let Some(rule) = &rule {
                std::fs::write(
                    &included,
                    format!("[url \"{away}\"]\n\tinsteadOf = {rule}\n"),
                )
                .unwrap();
            }
            let home = home_saying(dir, "home", &config);
            let settings = Settings::planted(&home, none);
            // THE SERVED ADDRESS ITSELF where no rule rewrites it.
            let url = if rule.is_some() {
                address.to_owned()
            } else {
                format!("{served}distro.git")
            };
            let prepared = Repo::prepare_clone(&tools, &settings, &url, &into).unwrap();
            if rule.is_some() {
                // ELSEWHERE, said — and not fetched with the served host's pass.
                assert_eq!(prepared.host().as_deref(), Some("127.0.0.2"), "{case}");
                let refusal = prepared.fetch(&let_served).unwrap_err();
                assert_eq!(refusal.why, Why::Garbled, "{case}");
                assert!(!into.exists(), "{case}: a refused clone left its folder");
            } else {
                assert_eq!(prepared.host().as_deref(), Some("127.0.0.1"), "{case}");
                prepared.fetch(&let_served).unwrap();
                assert!(into.join("flake.nix").exists(), "{case}");
                std::fs::remove_dir_all(&into).unwrap();
            }
            assert_eq!(
                elsewhere.reached(),
                0,
                "{case}: a host nobody let was reached"
            );
        }
    }
}
