//! Trying a Nix package without installing it: `nix build --out-link` into the
//! runtime directory (a GC root that lives as long as the session), then a
//! transient unit `ronoctua-try-<key>` with the traces the person chose.
//!
//! # Why an out-link in the runtime directory
//!
//! Because it is the one GC root that ends by itself. `nix run` and `nix
//! shell` root nothing: the store collection that every update run makes
//! would take the program from under a trial that is still open. An out-link
//! is an indirect root, and `$XDG_RUNTIME_DIR` is a tmpfs emptied at logout,
//! so the root dangles then and the next collection frees the package. That
//! IS "not permanent", with no timer and no bookkeeping of ours. Stopping the
//! trial removes the link at once.
//!
//! # Why a transient SERVICE, and why it runs our own CLI first
//!
//! A service can be listed, stopped and killed as a whole (`ExitType=cgroup`:
//! a program that forks and lets its first process exit is still the trial),
//! and `--collect` makes it vanish when it ends. What it runs is
//! `ronoctua-software trial-exec <record>` — which reads the trial's record
//! and `exec`s the program with its environment ([`exec`]). Two reasons:
//!
//! * a command-line program opens in OUR terminal, and the terminal is a
//!   program of ours that reads the person's theme and language from
//!   `$XDG_CONFIG_HOME`. Pointing the whole unit at a throwaway home would
//!   dress the terminal in the defaults; the throwaway home belongs to what
//!   runs INSIDE it, so the terminal runs `trial-exec`, which sets it.
//! * the Wayland display, the session's `PATH` and the login shell are the
//!   unit's own environment (the compositor imports `WAYLAND_DISPLAY` into the
//!   user manager), read where they are true — inside the unit — instead of
//!   copied from whichever process happened to ask for the trial.
//!
//! # A command-line program is a SHELL, not the program
//!
//! The terminal closes when its program exits, and most command-line
//! programs run without arguments print their usage and exit: `rg` alone
//! would flash a window. A trial of one opens the person's shell with the
//! package's `bin` first on `PATH` — what `nix shell` gives — and the record
//! keeps the command's name so the screen can say what to type. A `.desktop`
//! entry that asks for a terminal (`htop`) runs its program there instead.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::order::{Consent, Traces};
use crate::progress::{Phase, Report};
use crate::run::Call;
use crate::state::Trial;
use crate::{AppKey, Context, Done, Failure, Order, Outcome, Refusal, Roots};

/// Every trial's unit is `ronoctua-try-<key>.service`.
pub const UNIT_PREFIX: &str = "ronoctua-try-";
/// The record's file name, in the trial's runtime directory.
pub const RECORD: &str = "trial.json";
/// The CLI's subcommand the unit runs (`ronoctua-software trial-exec <record>`).
pub const EXEC: &str = "trial-exec";
/// The out-link's name in the trial's directory; Nix names the other outputs
/// `out-<output>` beside it.
pub const OUT: &str = "out";

/// What a trial is, written when it starts and read by [`exec`] inside the
/// unit and by [`trials`] for the screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Record {
    /// The nixpkgs attribute.
    pub attr: String,
    /// The name to show (data).
    pub name: String,
    /// What it may leave behind.
    pub traces: Traces,
    /// Whether a sandboxed trial reaches the network.
    pub network: bool,
    /// Its licence is not free (so keeping it opens the unfree door again).
    pub unfree: bool,
    /// When it started, seconds since the epoch.
    pub since: u64,
    /// The store paths of the built outputs.
    pub outputs: Vec<PathBuf>,
    /// The program and its arguments; empty means the person's shell.
    #[serde(default)]
    pub run: Vec<String>,
    /// Whether it runs inside a terminal.
    #[serde(default)]
    pub terminal: bool,
    /// The command a person types, for a command-line trial.
    #[serde(default)]
    pub program: Option<String>,
    /// The home it sees (`HOME` and every `XDG_*_HOME` under it) — a
    /// throwaway one unless its traces are the person's own home.
    pub home: PathBuf,
    /// Bubblewrap, for a sandboxed trial (resolved when it started, so the
    /// unit does not depend on its own environment to find it).
    #[serde(default)]
    pub bwrap: Option<PathBuf>,
}

/// Whether a trial key can name a unit and a directory: the output of
/// [`crate::slug_of`], so lower-case ASCII letters, digits and `-`.
#[must_use]
pub fn key_is_valid(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= 64
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The unit of a trial.
#[must_use]
pub fn unit_of(key: &str) -> String {
    format!("{UNIT_PREFIX}{key}.service")
}

/// Where the trials of this session are: `$XDG_RUNTIME_DIR/ronoctua[-dev]/
/// software/trials`. `None` without a runtime directory.
#[must_use]
pub fn trials_dir(roots: &Roots) -> Option<PathBuf> {
    Some(
        roots
            .runtime
            .as_ref()?
            .join(ronoctua_xdg::name())
            .join("software")
            .join("trials"),
    )
}

/// A trial's throwaway directory ON DISK (`~/.cache/ronoctua/software/try/
/// <key>`): the runtime directory is a tmpfs sized as a share of RAM, and a
/// program's caches are not something to hold in memory.
#[must_use]
pub fn scratch_of(roots: &Roots, key: &str) -> PathBuf {
    roots.ours_cache().join("try").join(key)
}

/// The throwaway home inside it.
#[must_use]
pub fn home_of(roots: &Roots, key: &str) -> PathBuf {
    scratch_of(roots, key).join("home")
}

/// `nix build` into the trial's out-link.
#[must_use]
pub fn build_argv(attr: &str, out_link: &Path, unfree: bool) -> Vec<String> {
    let mut argv: Vec<String> = ["build", "--log-format", "internal-json", "--out-link"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    argv.push(out_link.display().to_string());
    if unfree {
        argv.push("--impure".into());
    }
    argv.push("--".into());
    argv.push(crate::nix::installable(attr));
    argv
}

/// `systemd-run` of a trial: a transient user service in `app.slice` (where
/// the out-of-memory killer looks first), no new privileges, killed whole on
/// memory pressure, alive while any of its processes is, and unloaded when it
/// ends. It returns once the unit has started.
#[must_use]
pub fn launch_argv(key: &str, attr: &str, command: &[String]) -> Vec<String> {
    let mut argv = vec![
        "--user".to_owned(),
        format!("--unit={}", unit_of(key)),
        "--collect".to_owned(),
        "--slice=app.slice".to_owned(),
        format!("--description=ronoctua trial: {attr}"),
        "-p".to_owned(),
        "NoNewPrivileges=yes".to_owned(),
        "-p".to_owned(),
        "OOMPolicy=kill".to_owned(),
        "-p".to_owned(),
        "ExitType=cgroup".to_owned(),
        "--".to_owned(),
    ];
    argv.extend(command.iter().cloned());
    argv
}

/// What the unit runs: `trial-exec`, directly or inside our terminal.
#[must_use]
pub fn unit_command(software: &Path, record: &Path, terminal: Option<&Path>) -> Vec<String> {
    let exec = [
        software.display().to_string(),
        EXEC.to_owned(),
        record.display().to_string(),
    ];
    match terminal {
        Some(shell) => std::iter::once(shell.display().to_string())
            .chain(std::iter::once("terminal".to_owned()))
            .chain(exec)
            .collect(),
        None => exec.to_vec(),
    }
}

/// `systemctl --user stop` of a trial's unit.
#[must_use]
pub fn stop_argv(key: &str) -> Vec<String> {
    vec!["--user".into(), "stop".into(), "--".into(), unit_of(key)]
}

/// `systemctl --user is-active` of several trials' units: one line each, in
/// order.
#[must_use]
pub fn is_active_argv(keys: &[String]) -> Vec<String> {
    let mut argv = vec!["--user".to_owned(), "is-active".to_owned(), "--".to_owned()];
    argv.extend(keys.iter().map(|k| unit_of(k)));
    argv
}

/// What a trial runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct What {
    /// The program and its arguments; empty = the person's shell.
    pub run: Vec<String>,
    /// Inside a terminal.
    pub terminal: bool,
    /// The command to type, for a shell.
    pub program: Option<String>,
}

/// Decide what to run from what was built: a `.desktop` entry (its `Exec=`,
/// the program found in the package's own `bin` when it is a bare name),
/// preferring the one whose program is `main_program`; otherwise the shell,
/// with `main_program` — or the attribute's last segment — as the command to
/// type, when the package has it in a `bin`. `None` when there is nothing to
/// run at all (a library, a font).
#[must_use]
pub fn what_to_run(outputs: &[PathBuf], main_program: &str, attr: &str) -> Option<What> {
    let entries = entries_in(outputs);
    let wanted = [main_program, attr.rsplit('.').next().unwrap_or(attr)];
    let by_program = entries.iter().find(|e| {
        let base = e
            .run
            .first()
            .and_then(|p| p.rsplit('/').next())
            .unwrap_or_default();
        wanted.iter().any(|w| !w.is_empty() && *w == base)
    });
    if let Some(e) = by_program.or_else(|| entries.first()) {
        return Some(What {
            run: e.run.clone(),
            terminal: e.terminal,
            program: None,
        });
    }
    let command = wanted
        .iter()
        .find(|w| !w.is_empty() && in_bin(outputs, w).is_some())?;
    Some(What {
        run: Vec::new(),
        terminal: true,
        program: Some((*command).to_owned()),
    })
}

struct Entry {
    run: Vec<String>,
    terminal: bool,
}

fn in_bin(outputs: &[PathBuf], name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') {
        return None;
    }
    outputs
        .iter()
        .map(|o| o.join("bin").join(name))
        .find(|p| p.is_file())
}

/// The launchable `.desktop` entries of the outputs, sorted by file name.
fn entries_in(outputs: &[PathBuf]) -> Vec<Entry> {
    let mut files: Vec<PathBuf> = outputs
        .iter()
        .filter_map(|o| std::fs::read_dir(o.join("share/applications")).ok())
        .flat_map(|d| d.filter_map(Result::ok).map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "desktop"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|path| {
            use ronoctua_apps::desktop;
            let text = std::fs::read_to_string(path).ok()?;
            let fields = desktop::fields(&text);
            if desktop::raw_bytes(&fields, "Type") != Some("Application")
                || desktop::boolean(&fields, "NoDisplay")
                || desktop::boolean(&fields, "Hidden")
            {
                return None;
            }
            let name = desktop::raw_bytes(&fields, "Name").unwrap_or_default();
            let icon = desktop::raw_bytes(&fields, "Icon").unwrap_or_default();
            let (program, args) = ronoctua_apps::exec::assemble(
                desktop::raw_bytes(&fields, "Exec")?,
                ronoctua_apps::exec::Context {
                    icon,
                    name,
                    file: &path.to_string_lossy(),
                    open: &[],
                },
            )?;
            // A bare name is the package's own program, by path: the unit's
            // `PATH` does not have the trial in it, and a name found on the
            // person's `PATH` would run the installed copy instead.
            let program = if program.starts_with('/') {
                program
            } else {
                in_bin(outputs, &program)?.display().to_string()
            };
            let mut run = vec![program];
            run.extend(args);
            Some(Entry {
                run,
                terminal: desktop::boolean(&fields, "Terminal"),
            })
        })
        .collect()
}

/// The built outputs in a trial's directory (`out`, `out-man`, …), resolved
/// to their store paths — inside a sandbox the runtime directory is not there,
/// and the store is.
#[must_use]
pub fn outputs_in(trial_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(trial_dir) else {
        return Vec::new();
    };
    let mut links: Vec<(String, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            (name == OUT || name.starts_with("out-")).then(|| (name, e.path()))
        })
        .collect();
    links.sort();
    links
        .into_iter()
        .filter_map(|(_, p)| std::fs::canonicalize(p).ok())
        .collect()
}

/// The environment `trial-exec` finds in its unit.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env {
    /// `PATH`.
    pub path: Option<String>,
    /// `XDG_DATA_DIRS`.
    pub data_dirs: Option<String>,
    /// `WAYLAND_DISPLAY` (a name in the runtime directory, or a path).
    pub wayland: Option<String>,
    /// `XDG_RUNTIME_DIR`.
    pub runtime: Option<PathBuf>,
    /// `SHELL` — systemd sets it for every user service.
    pub shell: Option<String>,
}

impl Env {
    /// This process's. Read only by [`exec`], which runs inside the unit.
    #[must_use]
    pub fn of_process() -> Self {
        let var = |n: &str| std::env::var(n).ok().filter(|v| !v.is_empty());
        Self {
            path: var("PATH"),
            data_dirs: var("XDG_DATA_DIRS"),
            wayland: var("WAYLAND_DISPLAY"),
            runtime: var("XDG_RUNTIME_DIR").map(PathBuf::from),
            shell: var("SHELL"),
        }
    }
}

/// What [`exec`] replaces itself with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The program.
    pub program: String,
    /// Its arguments.
    pub args: Vec<String>,
    /// Variables to set.
    pub set: Vec<(String, String)>,
    /// The directory to start in, when not the unit's.
    pub dir: Option<PathBuf>,
}

/// The shell when a login shell is not known.
const FALLBACK_SHELL: &str = "/bin/sh";

/// `HOME` and every `XDG_*_HOME` under a home.
#[must_use]
pub fn scratch_vars(home: &Path) -> Vec<(String, String)> {
    let at = |rel: &str| {
        if rel.is_empty() {
            home.display().to_string()
        } else {
            home.join(rel).display().to_string()
        }
    };
    vec![
        ("HOME".to_owned(), at("")),
        ("XDG_CONFIG_HOME".to_owned(), at(".config")),
        ("XDG_DATA_HOME".to_owned(), at(".local/share")),
        ("XDG_STATE_HOME".to_owned(), at(".local/state")),
        ("XDG_CACHE_HOME".to_owned(), at(".cache")),
    ]
}

/// The package's `bin` and `share` first, then the session's.
fn search_paths(r: &Record, env: &Env) -> (String, String) {
    let join = |sub: &str, rest: Option<&str>| {
        let mut parts: Vec<String> = r
            .outputs
            .iter()
            .map(|o| o.join(sub))
            .filter(|p| p.is_dir())
            .map(|p| p.display().to_string())
            .collect();
        parts.extend(rest.map(str::to_owned));
        parts.join(":")
    };
    (
        join("bin", env.path.as_deref()),
        join("share", env.data_dirs.as_deref()),
    )
}

/// Bubblewrap's arguments for a sandboxed trial, up to (not including) the
/// `--` before the command: the store, the system and `/etc` read-only, a
/// fresh `/tmp`, `/dev` with the GPU, the throwaway home read-write, and of
/// the runtime directory only the Wayland and sound sockets, read-only. The
/// network only when allowed — and then systemd-resolved's directory, which
/// `/etc/resolv.conf` points into.
///
/// A graphical program also gets a session of its own (`--new-session`): with
/// the person's terminal as its controlling one, a sandboxed program could
/// push keystrokes into it (`TIOCSTI`). A program that runs IN our terminal
/// keeps it — that terminal is the trial's own, every reader of it is inside
/// the sandbox, and a shell without a controlling terminal has no job control.
#[must_use]
pub fn sandbox_args(r: &Record, env: &Env, path: &str, data_dirs: &str) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let mut push = |items: &[&str]| a.extend(items.iter().map(|s| (*s).to_owned()));
    push(&["--die-with-parent", "--unshare-all"]);
    if r.network {
        push(&["--share-net"]);
    }
    if !r.terminal {
        push(&["--new-session"]);
    }
    push(&[
        "--ro-bind",
        "/nix/store",
        "/nix/store",
        "--ro-bind",
        "/run/current-system",
        "/run/current-system",
        "--ro-bind",
        "/etc",
        "/etc",
        "--ro-bind-try",
        "/run/opengl-driver",
        "/run/opengl-driver",
    ]);
    if r.network {
        push(&[
            "--ro-bind-try",
            "/run/systemd/resolve",
            "/run/systemd/resolve",
        ]);
    }
    push(&[
        "--ro-bind-try",
        "/sys/dev",
        "/sys/dev",
        "--ro-bind-try",
        "/sys/devices",
        "/sys/devices",
        "--dev",
        "/dev",
        "--dev-bind-try",
        "/dev/dri",
        "/dev/dri",
        "--proc",
        "/proc",
        "--tmpfs",
        "/tmp",
    ]);
    let home = r.home.display().to_string();
    a.extend(["--bind".to_owned(), home.clone(), home.clone()]);
    let mut set: Vec<(String, String)> = scratch_vars(&r.home);
    if let Some(runtime) = &env.runtime {
        let rt = runtime.display().to_string();
        a.extend([
            "--perms".to_owned(),
            "0700".to_owned(),
            "--dir".to_owned(),
            rt.clone(),
        ]);
        let mut sockets: Vec<PathBuf> = Vec::new();
        if let Some(w) = &env.wayland {
            sockets.push(if w.starts_with('/') {
                PathBuf::from(w)
            } else {
                runtime.join(w)
            });
        }
        sockets.push(runtime.join("pipewire-0"));
        sockets.push(runtime.join("pulse/native"));
        for s in sockets {
            let s = s.display().to_string();
            a.extend(["--ro-bind-try".to_owned(), s.clone(), s]);
        }
        set.push(("XDG_RUNTIME_DIR".to_owned(), rt));
    }
    set.push(("PATH".to_owned(), path.to_owned()));
    set.push(("XDG_DATA_DIRS".to_owned(), data_dirs.to_owned()));
    for (k, v) in set {
        a.extend(["--setenv".to_owned(), k, v]);
    }
    a.extend(["--chdir".to_owned(), home]);
    a
}

/// What `trial-exec` becomes, from the record and the unit's environment.
///
/// # Errors
///
/// [`Refusal::SourceAbsent`] for a sandboxed trial whose record has no
/// bubblewrap.
pub fn plan(r: &Record, env: &Env) -> Result<Plan, Refusal> {
    let command: Vec<String> = if r.run.is_empty() {
        vec![
            env.shell
                .clone()
                .unwrap_or_else(|| FALLBACK_SHELL.to_owned()),
        ]
    } else {
        r.run.clone()
    };
    let (path, data_dirs) = search_paths(r, env);
    let found = vec![
        ("PATH".to_owned(), path.clone()),
        ("XDG_DATA_DIRS".to_owned(), data_dirs.clone()),
    ];
    let (program, args) = (command[0].clone(), command[1..].to_vec());
    Ok(match r.traces {
        Traces::Home => Plan {
            program,
            args,
            set: found,
            dir: None,
        },
        Traces::Scratch => {
            let mut set = scratch_vars(&r.home);
            set.extend(found);
            Plan {
                program,
                args,
                set,
                dir: Some(r.home.clone()),
            }
        }
        Traces::Sandbox => {
            let bwrap = r.bwrap.as_ref().ok_or(Refusal::SourceAbsent)?;
            let mut args = sandbox_args(r, env, &path, &data_dirs);
            args.push("--".to_owned());
            args.extend(command);
            Plan {
                program: bwrap.display().to_string(),
                args,
                set: Vec::new(),
                dir: None,
            }
        }
    })
}

/// `ronoctua-software trial-exec <record>`: runs INSIDE the trial's unit and
/// replaces itself with the trial's program. It returns only when that
/// fails.
///
/// This is the one place the crate starts a program without
/// [`crate::run::Call`]: `Call` is a CHILD with its streams read, and this
/// is the unit's own process becoming the program — its streams are the
/// unit's (or the terminal's), and nothing is left here to read them.
#[must_use]
pub fn exec(record: &Path) -> Failure {
    use std::os::unix::process::CommandExt as _;
    let Some(r) = read_record(record) else {
        return Failure {
            refusal: Some(Refusal::UnknownItem),
            trouble: format!("no trial at {}", record.display()),
        };
    };
    let plan = match plan(&r, &Env::of_process()) {
        Ok(p) => p,
        Err(refusal) => return Failure::refused(refusal),
    };
    let mut command = std::process::Command::new(&plan.program);
    command
        .args(&plan.args)
        .envs(plan.set.iter().map(|(k, v)| (k, v)));
    if let Some(dir) = &plan.dir {
        command.current_dir(dir);
    }
    let error = command.exec();
    Failure {
        refusal: Some(Refusal::ToolFailed),
        trouble: format!("could not start {}: {error}", plan.program),
    }
}

/// A trial's record, or `None`.
#[must_use]
pub fn read_record(path: &Path) -> Option<Record> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Which of `keys` have a unit alive. A READ; runs one program.
fn alive(ctx: &Context, keys: &[String]) -> Vec<bool> {
    let Some(systemctl) = ctx.tools.systemctl.as_deref() else {
        return vec![false; keys.len()];
    };
    if keys.is_empty() {
        return Vec::new();
    }
    // `is-active` exits non-zero when any unit is not active; the lines are
    // the answer either way.
    let Ok(ran) = Call::new(systemctl, is_active_argv(keys))
        .keep_stdout()
        .quietly(ctx)
    else {
        return vec![false; keys.len()];
    };
    let lines: Vec<&str> = ran.stdout.lines().map(str::trim).collect();
    if lines.len() != keys.len() {
        return vec![false; keys.len()];
    }
    lines
        .iter()
        .map(|l| matches!(*l, "active" | "activating" | "reloading"))
        .collect()
}

/// The trials of this session, running or ended, newest first. Runs one
/// program (`systemctl is-active`): call it on a thread.
#[must_use]
pub fn trials(ctx: &Context) -> Vec<Trial> {
    let Some(dir) = trials_dir(&ctx.roots) else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found: Vec<(String, Record)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let key = e
                .file_name()
                .into_string()
                .ok()
                .filter(|k| key_is_valid(k))?;
            Some((key, read_record(&e.path().join(RECORD))?))
        })
        .collect();
    found.sort_by(|a, b| b.1.since.cmp(&a.1.since).then_with(|| a.0.cmp(&b.0)));
    let keys: Vec<String> = found.iter().map(|(k, _)| k.clone()).collect();
    let running = alive(ctx, &keys);
    found
        .into_iter()
        .zip(running)
        .map(|((key, r), running)| Trial {
            key,
            attr: r.attr,
            name: r.name,
            running,
            since: Some(r.since),
            traces: r.traces,
            program: r.program,
            terminal: r.terminal,
        })
        .collect()
}

/// Run `Try`, `StopTrial` or `Keep`. Runs programs: call it on a thread.
pub fn run(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    match order {
        Order::Try {
            attr,
            traces,
            network,
            consent,
        } => try_it(ctx, attr, *traces, *network, *consent, report),
        Order::StopTrial { trial } => stop(ctx, trial, report),
        Order::Keep { trial } => keep(ctx, trial, report),
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

fn try_it(
    ctx: &Context,
    attr: &str,
    traces: Traces,
    network: bool,
    consent: Consent,
    report: &mut dyn Report,
) -> Done {
    let nix = crate::run::tool(ctx.tools.nix.as_ref())?;
    let systemd_run = crate::run::tool(ctx.tools.systemd_run.as_ref())?;
    let software = crate::run::tool(ctx.tools.software.as_ref())?;
    let bwrap = if traces == Traces::Sandbox {
        Some(crate::run::tool(ctx.tools.bwrap.as_ref())?.to_path_buf())
    } else {
        None
    };
    let unfree = crate::nix::vet(ctx, attr, consent)?;
    let key = crate::slug_of(attr);
    if !key_is_valid(&key) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let outcome = |rehearsed| Outcome {
        key: Some(AppKey::Nix {
            attr: attr.to_owned(),
        }),
        rehearsed,
        ..Outcome::default()
    };
    let dir = trials_dir(&ctx.roots).ok_or_else(|| Failure {
        refusal: Some(Refusal::SourceUnprepared),
        trouble: "XDG_RUNTIME_DIR is not set: a trial needs the session's runtime directory".into(),
    })?;
    if alive(ctx, std::slice::from_ref(&key)).first() == Some(&true) {
        // Already running: asking again is not a second copy.
        return Ok(outcome(false));
    }
    let trial_dir = dir.join(&key);
    report.step(Phase::Resolving, 0, 0, attr);
    if !ctx.rehearse {
        std::fs::create_dir_all(&trial_dir)
            .map_err(|e| Failure::trouble(format!("{}: {e}", trial_dir.display())))?;
    }
    let mut build = Call::new(nix, build_argv(attr, &trial_dir.join(OUT), unfree)).changing();
    if unfree {
        build = build.env(crate::nix::UNFREE_VARIABLE, "1");
    }
    match crate::nix::run_logged(ctx, build, report) {
        Ok(ran) if ran.rehearsed => return Ok(outcome(true)),
        Ok(_) => {}
        Err(e) => {
            // A build that failed leaves no directory to list as a trial.
            let _ = std::fs::remove_dir_all(&trial_dir);
            return Err(e);
        }
    }
    let outputs = outputs_in(&trial_dir);
    let row = ctx
        .tools
        .nix_index
        .as_deref()
        .and_then(|i| crate::nix_index::row(i, attr));
    let main_program = row.as_ref().map_or("", |r| r.program.as_str());
    let Some(what) = what_to_run(&outputs, main_program, attr) else {
        let _ = std::fs::remove_dir_all(&trial_dir);
        return Err(Failure::refused(Refusal::CannotTry));
    };
    let terminal = if what.terminal {
        Some(crate::run::tool(ctx.tools.shell.as_ref())?)
    } else {
        None
    };
    let home = if traces == Traces::Home {
        ctx.roots.home.clone()
    } else {
        home_of(&ctx.roots, &key)
    };
    if traces != Traces::Home {
        for (_, dir) in scratch_vars(&home) {
            std::fs::create_dir_all(&dir).map_err(|e| Failure::trouble(format!("{dir}: {e}")))?;
        }
    }
    let record = Record {
        attr: attr.to_owned(),
        name: row.as_ref().map_or_else(
            || attr.to_owned(),
            |r| {
                if r.pname.is_empty() {
                    attr.to_owned()
                } else {
                    r.pname.clone()
                }
            },
        ),
        traces,
        network,
        unfree,
        since: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs()),
        outputs,
        run: what.run,
        terminal: what.terminal,
        program: what.program,
        home,
        bwrap,
    };
    let bytes = serde_json::to_vec_pretty(&record).map_err(|e| Failure::trouble(e.to_string()))?;
    ronoctua_xdg::write_atomically(&trial_dir, RECORD, &bytes)
        .map_err(|e| Failure::trouble(format!("{}: {e}", trial_dir.display())))?;
    report.step(Phase::Starting, 0, 0, &record.name);
    let command = unit_command(software, &trial_dir.join(RECORD), terminal);
    let started = Call::new(systemd_run, launch_argv(&key, attr, &command))
        .changing()
        .quietly(ctx)
        .and_then(crate::run::Ran::ok);
    if let Err(e) = started {
        // Nothing runs: nothing is left behind either.
        let _ = std::fs::remove_dir_all(&trial_dir);
        let _ = std::fs::remove_dir_all(scratch_of(&ctx.roots, &key));
        return Err(e);
    }
    Ok(outcome(false))
}

fn stop(ctx: &Context, key: &str, report: &mut dyn Report) -> Done {
    if !key_is_valid(key) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let systemctl = crate::run::tool(ctx.tools.systemctl.as_ref())?;
    let trial_dir = trials_dir(&ctx.roots).map(|d| d.join(key));
    let attr = trial_dir
        .as_ref()
        .and_then(|d| read_record(&d.join(RECORD)))
        .map(|r| r.attr);
    report.step(Phase::Removing, 0, 0, attr.as_deref().unwrap_or(key));
    let ran = Call::new(systemctl, stop_argv(key))
        .changing()
        .quietly(ctx)?;
    let key_of = attr.map(|attr| AppKey::Nix { attr });
    if ran.rehearsed {
        return Ok(Outcome {
            key: key_of,
            rehearsed: true,
            ..Outcome::default()
        });
    }
    // A unit that is not loaded (it ended by itself and was collected) makes
    // `stop` fail; what matters is that nothing of it is running now.
    if ran.code != Some(0) && alive(ctx, &[key.to_owned()]).first() == Some(&true) {
        return Err(ran
            .ok()
            .err()
            .unwrap_or_else(|| Failure::refused(Refusal::ToolFailed)));
    }
    let _ = std::fs::remove_dir_all(scratch_of(&ctx.roots, key));
    if let Some(d) = trial_dir {
        let _ = std::fs::remove_dir_all(d);
    }
    Ok(Outcome {
        key: key_of,
        ..Outcome::default()
    })
}

fn keep(ctx: &Context, key: &str, report: &mut dyn Report) -> Done {
    if !key_is_valid(key) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let record = trials_dir(&ctx.roots)
        .and_then(|d| read_record(&d.join(key).join(RECORD)))
        .ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
    let installed = crate::nix::install(
        ctx,
        &record.attr,
        Consent {
            unfree: record.unfree,
            ..Consent::default()
        },
        report,
    )?;
    if !installed.rehearsed
        && let Err(e) = stop(ctx, key, report)
    {
        // Installed is what was asked; a trial left running is visible and
        // stoppable, and failing the whole order over it would lie.
        tracing::warn!(key, error = %e, "the kept trial could not be stopped");
    }
    Ok(installed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::Beat;
    use crate::{Tools, prefs::Preferences};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-trial-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    use crate::write_executable;

    /// A built package: `bin/<programs>` and, optionally, one `.desktop`.
    fn package(root: &Path, name: &str, programs: &[&str], desktop: Option<&str>) -> PathBuf {
        let out = root.join(format!("store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-{name}"));
        for p in programs {
            write_executable(&out.join("bin").join(p), "#!/bin/sh\n");
        }
        if let Some(text) = desktop {
            std::fs::create_dir_all(out.join("share/applications")).unwrap();
            std::fs::write(
                out.join("share/applications")
                    .join(format!("{name}.desktop")),
                text,
            )
            .unwrap();
        }
        out
    }

    fn record(traces: Traces) -> Record {
        Record {
            attr: "gimp".into(),
            name: "gimp".into(),
            traces,
            network: false,
            unfree: false,
            since: 1,
            outputs: vec![PathBuf::from(
                "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-gimp-3.0.8",
            )],
            run: vec!["/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-gimp-3.0.8/bin/gimp".into()],
            terminal: false,
            program: None,
            home: PathBuf::from("/home/p/.cache/ronoctua/software/try/gimp/home"),
            bwrap: Some(PathBuf::from("/nix/store/b-bubblewrap/bin/bwrap")),
        }
    }

    fn env() -> Env {
        Env {
            path: Some("/run/current-system/sw/bin".into()),
            data_dirs: Some("/run/current-system/sw/share".into()),
            wayland: Some("wayland-1".into()),
            runtime: Some(PathBuf::from("/run/user/1000")),
            shell: Some("/run/current-system/sw/bin/zsh".into()),
        }
    }

    #[test]
    fn the_build_and_the_launch_are_argument_vectors() {
        let out = Path::new("/run/user/1000/ronoctua/software/trials/gimp/out");
        assert_eq!(
            build_argv("gimp", out, false),
            [
                "build",
                "--log-format",
                "internal-json",
                "--out-link",
                "/run/user/1000/ronoctua/software/trials/gimp/out",
                "--",
                "nixpkgs#gimp"
            ]
        );
        assert!(
            build_argv("google-chrome", out, true)
                .windows(2)
                .any(|w| w == ["--impure", "--"])
        );
        let command = unit_command(
            Path::new("/s/bin/ronoctua-software"),
            Path::new("/r/trial.json"),
            None,
        );
        assert_eq!(
            command,
            ["/s/bin/ronoctua-software", "trial-exec", "/r/trial.json"]
        );
        assert_eq!(
            launch_argv("gimp", "gimp", &command),
            [
                "--user",
                "--unit=ronoctua-try-gimp.service",
                "--collect",
                "--slice=app.slice",
                "--description=ronoctua trial: gimp",
                "-p",
                "NoNewPrivileges=yes",
                "-p",
                "OOMPolicy=kill",
                "-p",
                "ExitType=cgroup",
                "--",
                "/s/bin/ronoctua-software",
                "trial-exec",
                "/r/trial.json"
            ]
        );
        let in_terminal = unit_command(
            Path::new("/s/bin/ronoctua-software"),
            Path::new("/r/trial.json"),
            Some(Path::new("/t/bin/ronoctua-shell")),
        );
        assert_eq!(
            in_terminal,
            [
                "/t/bin/ronoctua-shell",
                "terminal",
                "/s/bin/ronoctua-software",
                "trial-exec",
                "/r/trial.json"
            ]
        );
        assert_eq!(
            stop_argv("gimp"),
            ["--user", "stop", "--", "ronoctua-try-gimp.service"]
        );
        assert_eq!(
            is_active_argv(&["a".into(), "b".into()]),
            [
                "--user",
                "is-active",
                "--",
                "ronoctua-try-a.service",
                "ronoctua-try-b.service"
            ]
        );
    }

    #[test]
    fn a_key_names_a_unit_and_a_directory_and_nothing_else() {
        assert!(key_is_valid("gimp") && key_is_valid("python3packages-requests"));
        for bad in ["", "..", "a/b", "Gimp", "a.b", "a b", &"a".repeat(65)] {
            assert!(!key_is_valid(bad), "{bad}");
        }
        assert_eq!(
            crate::slug_of("python3Packages.requests"),
            "python3packages-requests"
        );
    }

    #[test]
    fn a_graphical_package_runs_its_desktop_entry_by_its_own_path() {
        let dir = scratch("desktop");
        let out = package(
            &dir,
            "gimp-3.0.8",
            &["gimp", "gimp-console"],
            Some("[Desktop Entry]\nType=Application\nName=GIMP\nIcon=gimp\nExec=gimp %U\n"),
        );
        let what = what_to_run(std::slice::from_ref(&out), "gimp", "gimp").unwrap();
        assert_eq!(what.run, [out.join("bin/gimp").display().to_string()]);
        assert!(!what.terminal && what.program.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_desktop_entry_that_asks_for_a_terminal_runs_its_program_there() {
        let dir = scratch("tui");
        let out = package(
            &dir,
            "htop-3.4",
            &["htop"],
            Some("[Desktop Entry]\nType=Application\nName=htop\nTerminal=true\nExec=htop\n"),
        );
        let what = what_to_run(std::slice::from_ref(&out), "htop", "htop").unwrap();
        assert_eq!(what.run, [out.join("bin/htop").display().to_string()]);
        assert!(what.terminal);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_command_line_package_opens_a_shell_that_has_it() {
        let dir = scratch("cli");
        let out = package(&dir, "ripgrep-15.1.0", &["rg"], None);
        let what = what_to_run(&[out], "rg", "ripgrep").unwrap();
        assert_eq!(
            what,
            What {
                run: Vec::new(),
                terminal: true,
                program: Some("rg".into())
            }
        );
        // A hidden entry is not a way in, and a library has nothing to run.
        let hidden = package(
            &dir,
            "lib-1",
            &[],
            Some("[Desktop Entry]\nType=Application\nName=x\nNoDisplay=true\nExec=x\n"),
        );
        assert_eq!(what_to_run(&[hidden], "", "lib"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_bare_exec_that_is_not_the_packages_own_is_not_run_from_the_persons_path() {
        let dir = scratch("foreign-exec");
        let out = package(
            &dir,
            "odd-1",
            &["odd"],
            Some("[Desktop Entry]\nType=Application\nName=Odd\nExec=firefox %u\n"),
        );
        let what = what_to_run(&[out], "odd", "odd").unwrap();
        assert_eq!(
            what.program.as_deref(),
            Some("odd"),
            "falls back to the shell with its own command"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_throwaway_trial_moves_every_home_and_nothing_else() {
        let p = plan(&record(Traces::Scratch), &env()).unwrap();
        assert_eq!(
            p.program,
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-gimp-3.0.8/bin/gimp"
        );
        let set: std::collections::BTreeMap<_, _> = p.set.iter().cloned().collect();
        let home = "/home/p/.cache/ronoctua/software/try/gimp/home";
        assert_eq!(set["HOME"], home);
        assert_eq!(set["XDG_CONFIG_HOME"], format!("{home}/.config"));
        assert_eq!(set["XDG_DATA_HOME"], format!("{home}/.local/share"));
        assert_eq!(set["XDG_STATE_HOME"], format!("{home}/.local/state"));
        assert_eq!(set["XDG_CACHE_HOME"], format!("{home}/.cache"));
        // The outputs are not on this disk, so only the session's path remains.
        assert_eq!(set["PATH"], "/run/current-system/sw/bin");
        assert_eq!(p.dir.as_deref(), Some(Path::new(home)));
        let own = plan(&record(Traces::Home), &env()).unwrap();
        assert!(
            own.set
                .iter()
                .all(|(k, _)| k == "PATH" || k == "XDG_DATA_DIRS"),
            "{:?}",
            own.set
        );
        assert_eq!(own.dir, None);
    }

    #[test]
    fn the_packages_own_bin_comes_first_on_the_path() {
        let dir = scratch("path");
        let out = package(&dir, "ripgrep-15.1.0", &["rg"], None);
        let r = Record {
            outputs: vec![out.clone()],
            run: Vec::new(),
            terminal: true,
            ..record(Traces::Home)
        };
        let p = plan(&r, &env()).unwrap();
        assert_eq!(
            p.program, "/run/current-system/sw/bin/zsh",
            "the person's shell"
        );
        let path = &p.set.iter().find(|(k, _)| k == "PATH").unwrap().1;
        assert_eq!(
            *path,
            format!("{}:/run/current-system/sw/bin", out.join("bin").display())
        );
        let no_shell = plan(
            &r,
            &Env {
                shell: None,
                ..env()
            },
        )
        .unwrap();
        assert_eq!(no_shell.program, "/bin/sh");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_sandbox_is_bubblewrap_with_exactly_these_doors() {
        let p = plan(&record(Traces::Sandbox), &env()).unwrap();
        assert_eq!(p.program, "/nix/store/b-bubblewrap/bin/bwrap");
        assert!(p.set.is_empty());
        let home = "/home/p/.cache/ronoctua/software/try/gimp/home";
        let expected: Vec<String> = [
            "--die-with-parent",
            "--unshare-all",
            "--new-session",
            "--ro-bind",
            "/nix/store",
            "/nix/store",
            "--ro-bind",
            "/run/current-system",
            "/run/current-system",
            "--ro-bind",
            "/etc",
            "/etc",
            "--ro-bind-try",
            "/run/opengl-driver",
            "/run/opengl-driver",
            "--ro-bind-try",
            "/sys/dev",
            "/sys/dev",
            "--ro-bind-try",
            "/sys/devices",
            "/sys/devices",
            "--dev",
            "/dev",
            "--dev-bind-try",
            "/dev/dri",
            "/dev/dri",
            "--proc",
            "/proc",
            "--tmpfs",
            "/tmp",
            "--bind",
            home,
            home,
            "--perms",
            "0700",
            "--dir",
            "/run/user/1000",
            "--ro-bind-try",
            "/run/user/1000/wayland-1",
            "/run/user/1000/wayland-1",
            "--ro-bind-try",
            "/run/user/1000/pipewire-0",
            "/run/user/1000/pipewire-0",
            "--ro-bind-try",
            "/run/user/1000/pulse/native",
            "/run/user/1000/pulse/native",
            "--setenv",
            "HOME",
            home,
            "--setenv",
            "XDG_CONFIG_HOME",
            &format!("{home}/.config"),
            "--setenv",
            "XDG_DATA_HOME",
            &format!("{home}/.local/share"),
            "--setenv",
            "XDG_STATE_HOME",
            &format!("{home}/.local/state"),
            "--setenv",
            "XDG_CACHE_HOME",
            &format!("{home}/.cache"),
            "--setenv",
            "XDG_RUNTIME_DIR",
            "/run/user/1000",
            "--setenv",
            "PATH",
            "/run/current-system/sw/bin",
            "--setenv",
            "XDG_DATA_DIRS",
            "/run/current-system/sw/share",
            "--chdir",
            home,
            "--",
            "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-gimp-3.0.8/bin/gimp",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        assert_eq!(p.args, expected);
        // No network: no `--share-net` and no resolver.
        assert!(
            !p.args
                .iter()
                .any(|a| a == "--share-net" || a == "/run/systemd/resolve")
        );
        let online = plan(
            &Record {
                network: true,
                ..record(Traces::Sandbox)
            },
            &env(),
        )
        .unwrap();
        assert_eq!(
            online.args[..3],
            ["--die-with-parent", "--unshare-all", "--share-net"]
        );
        assert!(online.args.windows(3).any(|w| w
            == [
                "--ro-bind-try",
                "/run/systemd/resolve",
                "/run/systemd/resolve"
            ]));
        // The person's home is never in it.
        assert!(!p.args.iter().any(|a| a == "/home/p" || a == "/home"));
        // A shell in our terminal keeps its controlling terminal.
        let shell = Record {
            run: Vec::new(),
            terminal: true,
            ..record(Traces::Sandbox)
        };
        let in_terminal = plan(&shell, &env()).unwrap();
        assert!(!in_terminal.args.iter().any(|a| a == "--new-session"));
        assert_eq!(
            in_terminal.args.last().map(String::as_str),
            Some("/run/current-system/sw/bin/zsh")
        );
        let without = Record {
            bwrap: None,
            ..record(Traces::Sandbox)
        };
        assert_eq!(plan(&without, &env()), Err(Refusal::SourceAbsent));
    }

    fn ctx(dir: &Path, tools: Tools) -> Context {
        Context {
            tools,
            roots: Roots::under(dir),
            prefs: Preferences::default(),
            rehearse: false,
        }
    }

    /// Fake `nix`, `systemd-run` and `systemctl`: the first builds a package
    /// into the out-link it is given; the others record their arguments.
    fn fakes(dir: &Path, desktop: bool) -> Tools {
        let pkg = package(
            dir,
            "gimp-3.0.8",
            &["gimp"],
            desktop.then_some("[Desktop Entry]\nType=Application\nName=GIMP\nExec=gimp\n"),
        );
        let calls = dir.join("calls");
        let nix = dir.join("bin/nix");
        write_executable(
            &nix,
            &format!(
                "#!/bin/sh\necho \"nix $*\" >> '{calls}'\nwhile [ \"$1\" != '--out-link' ]; do shift; done\nln -sfn '{pkg}' \"$2\"\n",
                calls = calls.display(),
                pkg = pkg.display()
            ),
        );
        let run = dir.join("bin/systemd-run");
        write_executable(
            &run,
            &format!(
                "#!/bin/sh\necho \"systemd-run $*\" >> '{}'\n",
                calls.display()
            ),
        );
        let ctl = dir.join("bin/systemctl");
        write_executable(
            &ctl,
            &format!(
                "#!/bin/sh\necho \"systemctl $*\" >> '{c}'\nif [ \"$2\" = is-active ]; then shift 3; for u in \"$@\"; do cat '{state}' 2>/dev/null || echo inactive; done; exit 3; fi\n",
                c = calls.display(),
                state = dir.join("state").display()
            ),
        );
        Tools {
            nix: Some(nix),
            systemd_run: Some(run),
            systemctl: Some(ctl),
            software: Some(PathBuf::from("/s/bin/ronoctua-software")),
            shell: Some(PathBuf::from("/t/bin/ronoctua-shell")),
            bwrap: Some(PathBuf::from("/b/bin/bwrap")),
            ..Tools::default()
        }
    }

    fn calls(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn a_trial_builds_records_launches_lists_and_stops_leaving_nothing() {
        let dir = scratch("whole");
        let c = ctx(&dir, fakes(&dir, true));
        let order = Order::Try {
            attr: "gimp".into(),
            traces: Traces::Scratch,
            network: true,
            consent: Consent::default(),
        };
        let mut beats: Vec<Beat> = Vec::new();
        let done = run(&c, &order, &mut beats).unwrap();
        assert_eq!(
            done.key,
            Some(AppKey::Nix {
                attr: "gimp".into()
            })
        );
        let trial_dir = trials_dir(&c.roots).unwrap().join("gimp");
        let calls_now = calls(&dir);
        assert!(calls_now.iter().any(|l| {
            l.starts_with("nix build --log-format internal-json --out-link ")
                && l.ends_with("-- nixpkgs#gimp")
        }));
        let launch = calls_now
            .iter()
            .find(|l| l.starts_with("systemd-run"))
            .unwrap();
        assert!(
            launch.ends_with(&format!(
                "-- /s/bin/ronoctua-software trial-exec {}",
                trial_dir.join(RECORD).display()
            )),
            "{launch}"
        );
        let record = read_record(&trial_dir.join(RECORD)).unwrap();
        assert!(!record.terminal && record.run[0].ends_with("/bin/gimp"));
        assert!(
            record.outputs[0].is_absolute() && !record.outputs[0].starts_with(&trial_dir),
            "resolved to the store"
        );
        let home = home_of(&c.roots, "gimp");
        assert!(home.join(".config").is_dir() && home.join(".cache").is_dir());
        assert!(beats.iter().any(|b| matches!(
            b,
            Beat::Step {
                phase: Phase::Starting,
                ..
            }
        )));

        std::fs::write(dir.join("state"), "active\n").unwrap();
        let listed = trials(&c);
        assert_eq!(listed.len(), 1);
        assert_eq!(
            (
                listed[0].key.as_str(),
                listed[0].attr.as_str(),
                listed[0].running
            ),
            ("gimp", "gimp", true)
        );
        // Asking again while it runs does not start a second one.
        let before = calls(&dir).len();
        run(&c, &order, &mut beats).unwrap();
        assert!(
            calls(&dir)[before..]
                .iter()
                .all(|l| !l.starts_with("systemd-run") && !l.starts_with("nix"))
        );

        std::fs::write(dir.join("state"), "inactive\n").unwrap();
        run(
            &c,
            &Order::StopTrial {
                trial: "gimp".into(),
            },
            &mut beats,
        )
        .unwrap();
        assert!(
            calls(&dir).contains(&"systemctl --user stop -- ronoctua-try-gimp.service".to_owned())
        );
        assert!(
            !trial_dir.exists() && !scratch_of(&c.roots, "gimp").exists(),
            "nothing left"
        );
        assert!(trials(&c).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_command_line_trial_opens_our_terminal() {
        let dir = scratch("terminal");
        let c = ctx(&dir, fakes(&dir, false));
        let order = Order::Try {
            attr: "gimp".into(),
            traces: Traces::Home,
            network: true,
            consent: Consent::default(),
        };
        run(&c, &order, &mut Vec::new()).unwrap();
        let launch = calls(&dir)
            .into_iter()
            .find(|l| l.starts_with("systemd-run"))
            .unwrap();
        assert!(
            launch
                .contains("-- /t/bin/ronoctua-shell terminal /s/bin/ronoctua-software trial-exec "),
            "{launch}"
        );
        let record = read_record(&trials_dir(&c.roots).unwrap().join("gimp").join(RECORD)).unwrap();
        assert_eq!(
            (record.terminal, record.program.as_deref()),
            (true, Some("gimp"))
        );
        assert_eq!(record.home, c.roots.home, "its own home: no throwaway one");
        assert!(!scratch_of(&c.roots, "gimp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_trial_without_a_runtime_directory_or_the_programs_is_refused() {
        let dir = scratch("refused");
        let mut c = ctx(&dir, fakes(&dir, true));
        c.roots.runtime = None;
        let order = Order::Try {
            attr: "gimp".into(),
            traces: Traces::Scratch,
            network: true,
            consent: Consent::default(),
        };
        assert_eq!(
            run(&c, &order, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::SourceUnprepared)
        );
        let c = ctx(
            &dir,
            Tools {
                systemd_run: None,
                ..fakes(&dir, true)
            },
        );
        assert_eq!(
            run(&c, &order, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::SourceAbsent)
        );
        let sandboxed = Order::Try {
            attr: "gimp".into(),
            traces: Traces::Sandbox,
            network: false,
            consent: Consent::default(),
        };
        let c = ctx(
            &dir,
            Tools {
                bwrap: None,
                ..fakes(&dir, true)
            },
        );
        assert_eq!(
            run(&c, &sandboxed, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::SourceAbsent)
        );
        assert!(
            calls(&dir).iter().all(|l| !l.starts_with("nix ")),
            "refused before building"
        );
        assert_eq!(
            run(
                &c,
                &Order::StopTrial {
                    trial: "../x".into()
                },
                &mut Vec::new()
            )
            .unwrap_err()
            .refusal,
            Some(Refusal::UnknownItem)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rehearsed_trial_builds_nothing_and_starts_nothing() {
        let dir = scratch("rehearsed");
        let mut c = ctx(&dir, fakes(&dir, true));
        c.rehearse = true;
        let order = Order::Try {
            attr: "gimp".into(),
            traces: Traces::Scratch,
            network: true,
            consent: Consent::default(),
        };
        assert!(run(&c, &order, &mut Vec::new()).unwrap().rehearsed);
        assert!(
            calls(&dir)
                .iter()
                .all(|l| l.starts_with("systemctl --user is-active")),
            "{:?}",
            calls(&dir)
        );
        assert!(!trials_dir(&c.roots).unwrap().join("gimp").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_build_leaves_nothing_and_says_why_in_nixpkgs_words() {
        let dir = scratch("failed-build");
        let tools = fakes(&dir, true);
        let refusal = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/nix/refused-unfree.jsonl");
        let nix = dir.join("bin/failing-nix");
        write_executable(
            &nix,
            &format!("#!/bin/sh\ncat '{}' >&2\nexit 1\n", refusal.display()),
        );
        let c = ctx(
            &dir,
            Tools {
                nix: Some(nix),
                ..tools
            },
        );
        let order = Order::Try {
            attr: "gimp".into(),
            traces: Traces::Scratch,
            network: true,
            consent: Consent::default(),
        };
        let err = run(&c, &order, &mut Vec::new()).unwrap_err();
        assert_eq!(err.refusal, Some(Refusal::UnfreeNeedsConsent));
        assert!(!trials_dir(&c.roots).unwrap().join("gimp").exists());
        assert!(calls(&dir).iter().all(|l| !l.starts_with("systemd-run")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn exec_says_why_when_there_is_no_trial() {
        let f = exec(Path::new("/nonexistent/trial.json"));
        assert_eq!(f.refusal, Some(Refusal::UnknownItem));
    }
}
