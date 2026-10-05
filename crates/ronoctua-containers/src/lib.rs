//! The person's containers — the one crate that runs `podman`.
//!
//! # Rootless, Docker-compatible
//!
//! Podman runs as the person (`docker` is podman; the rootless socket is
//! `$XDG_RUNTIME_DIR/podman/podman.sock`; compose is `podman-compose`). What
//! the person pulls, builds and runs lives in their home — the programs
//! exception of invariant 6, never `/etc`.
//!
//! # The CLI in JSON
//!
//! Lists by `--format json` (`ps -a`, `pod ps`, `images`, `volume ls`,
//! `network ls`), compose projects grouped by their labels
//! ([`compose_of`]); actions by [`Action`] (removing with `-f` and pruning
//! only after a question with "no" first — the door's); `logs -f`, `stats`
//! and `events` as children only while shown ([`Lines`], killed when
//! dropped); `inspect` as text for the door to highlight; `login` with the
//! password on stdin. Pull, build and compose go on the transfer queue
//! (`Work::Container`); a shell is the terminal's ([`exec_arguments`]).
//!
//! # The brain's mark costs nothing
//!
//! crun keeps a folder per running container under [`runtime_dir`]; the
//! brain's `containers` module watches it with inotify — no process.
//!
//! # The shapes are podman's own
//!
//! Every parser here reads what the pinned podman 5.8.2 PRINTED (the
//! fixtures in `fixtures.rs`, captured from a throwaway store), not what its
//! documentation promises: `ps` gives `Created` as seconds and `PodName`
//! empty; `pod ps`, `volume ls` and `network ls` give RFC 3339 times;
//! `network ls` alone is lower-case; `images` has no `Dangling`. A pod's
//! infra container is its plumbing and is not listed.
//!
//! # Blocks
//!
//! Every call runs podman to its end (~30–60 ms): a thread, never the event
//! loop. The streams ([`Lines`]) block on their next line.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use ronoctua_consent::keys::Secret;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod parse;
mod queue;

pub use parse::{Details, Event, Mount, Usage, details_of};
pub use queue::{Job, Step, containerfile_in, step_of, tag_of_folder};

/// The variable naming podman's store path.
pub const VARIABLE: &str = "RONOCTUA_PODMAN";

/// The label compose writes on a container: its project.
pub const COMPOSE_PROJECT: &str = "com.docker.compose.project";
/// The label compose writes: the project's folder.
pub const COMPOSE_WORKING_DIR: &str = "com.docker.compose.project.working_dir";
/// The label compose writes: the project's files, comma-separated.
pub const COMPOSE_CONFIG_FILES: &str = "com.docker.compose.project.config_files";
/// The label compose writes: the service.
pub const COMPOSE_SERVICE: &str = "com.docker.compose.service";

/// A container's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Created, never started.
    Created,
    /// Running.
    Running,
    /// Paused.
    Paused,
    /// Stopped (exited).
    Exited,
    /// Being stopped or removed.
    Stopping,
    /// A state this build does not know.
    Unknown,
}

impl State {
    /// The state podman's `State` says.
    #[must_use]
    pub fn of(said: &str) -> Self {
        match said.trim().to_ascii_lowercase().as_str() {
            "created" | "configured" | "initialized" => Self::Created,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "exited" | "stopped" => Self::Exited,
            "stopping" | "removing" => Self::Stopping,
            _ => Self::Unknown,
        }
    }

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Exited => "exited",
            Self::Stopping => "stopping",
            Self::Unknown => "unknown",
        }
    }
}

/// A published port.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Port {
    /// The address it listens on (`None`: every address).
    pub host_ip: Option<String>,
    /// The port on this machine.
    pub host_port: u16,
    /// The port inside the container.
    pub container_port: u16,
    /// `tcp`, `udp` or `sctp`.
    pub protocol: String,
}

/// What compose's labels say of a container.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ComposeLabels {
    /// The project.
    pub project: String,
    /// The project's folder.
    pub working_dir: Option<PathBuf>,
    /// The project's files.
    pub config_files: Vec<PathBuf>,
    /// The service this container is.
    pub service: Option<String>,
}

/// A container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Container {
    /// Its id.
    pub id: String,
    /// Its names.
    pub names: Vec<String>,
    /// Its image.
    pub image: String,
    /// Its state.
    pub state: State,
    /// podman's own status line ("Up 3 hours") — data.
    pub status: String,
    /// When it was created, seconds since the epoch.
    pub created_s: i64,
    /// Its published ports.
    pub ports: Vec<Port>,
    /// Its pod, when in one.
    pub pod: Option<String>,
    /// Its compose project, when compose made it.
    pub compose: Option<ComposeLabels>,
    /// Every label.
    pub labels: BTreeMap<String, String>,
}

/// A pod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pod {
    /// Its id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// podman's status — data.
    pub status: String,
    /// Its containers' ids.
    pub containers: Vec<String>,
    /// When it was created, seconds since the epoch.
    pub created_s: i64,
}

/// An image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Image {
    /// Its id.
    pub id: String,
    /// Its names (`docker.io/library/nginx:latest`).
    pub names: Vec<String>,
    /// Its size, in bytes.
    pub size_bytes: u64,
    /// When it was created, seconds since the epoch.
    pub created_s: i64,
    /// Whether no name points at it.
    pub dangling: bool,
    /// How many containers use it.
    pub containers: u32,
}

/// A volume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    /// Its name.
    pub name: String,
    /// Its driver.
    pub driver: String,
    /// Where it is on disk.
    pub mountpoint: PathBuf,
    /// When it was created, seconds since the epoch.
    pub created_s: i64,
}

/// A network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Network {
    /// Its id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Its driver.
    pub driver: String,
    /// Its subnets, in CIDR.
    pub subnets: Vec<String>,
    /// Whether its DNS is on.
    pub dns_enabled: bool,
}

/// A compose project, grouped from its containers' labels.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComposeProject {
    /// Its name.
    pub name: String,
    /// Its folder, when a container says.
    pub working_dir: Option<PathBuf>,
    /// Its files, when a container says.
    pub config_files: Vec<PathBuf>,
    /// Its containers' ids.
    pub containers: Vec<String>,
}

/// Everything, for the app's kinds column (with counts).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Listing {
    /// The containers, stopped ones too.
    pub containers: Vec<Container>,
    /// The pods.
    pub pods: Vec<Pod>,
    /// The images.
    pub images: Vec<Image>,
    /// The volumes.
    pub volumes: Vec<Volume>,
    /// The networks.
    pub networks: Vec<Network>,
    /// The compose projects.
    pub compose: Vec<ComposeProject>,
}

/// The compose projects the containers' labels name, by name. Pure.
#[must_use]
pub fn compose_of(containers: &[Container]) -> Vec<ComposeProject> {
    let mut projects: BTreeMap<&str, ComposeProject> = BTreeMap::new();
    for container in containers {
        let Some(labels) = &container.compose else {
            continue;
        };
        let project = projects
            .entry(labels.project.as_str())
            .or_insert_with(|| ComposeProject {
                name: labels.project.clone(),
                working_dir: None,
                config_files: Vec::new(),
                containers: Vec::new(),
            });
        if project.working_dir.is_none() {
            project.working_dir.clone_from(&labels.working_dir);
        }
        if project.config_files.is_empty() {
            project.config_files.clone_from(&labels.config_files);
        }
        project.containers.push(container.id.clone());
    }
    projects.into_values().collect()
}

/// What prune removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Prune {
    /// Stopped containers, unused networks, dangling images, build cache.
    System,
    /// Dangling images.
    Images,
    /// Unused volumes.
    Volumes,
}

/// Something done to the person's containers. Removing with `force` and
/// pruning come only after a question with "no" first — the door asks.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum Action {
    /// Start a container.
    Start {
        /// Its id or name.
        id: String,
    },
    /// Stop a container.
    Stop {
        /// Its id or name.
        id: String,
    },
    /// Restart a container.
    Restart {
        /// Its id or name.
        id: String,
    },
    /// Pause a container.
    Pause {
        /// Its id or name.
        id: String,
    },
    /// Unpause a container.
    Unpause {
        /// Its id or name.
        id: String,
    },
    /// Remove a container.
    Remove {
        /// Its id or name.
        id: String,
        /// Even while running.
        force: bool,
    },
    /// Start a pod's containers.
    StartPod {
        /// Its id or name.
        id: String,
    },
    /// Stop a pod's containers.
    StopPod {
        /// Its id or name.
        id: String,
    },
    /// Restart a pod's containers.
    RestartPod {
        /// Its id or name.
        id: String,
    },
    /// Remove a pod.
    RemovePod {
        /// Its id or name.
        id: String,
        /// Even while running.
        force: bool,
    },
    /// Remove an image.
    RemoveImage {
        /// Its id or name.
        id: String,
        /// Even while a container uses it.
        force: bool,
    },
    /// Remove a volume.
    RemoveVolume {
        /// Its name.
        name: String,
    },
    /// Remove a network.
    RemoveNetwork {
        /// Its name.
        name: String,
    },
    /// Prune.
    Prune {
        /// What.
        what: Prune,
    },
}

impl Action {
    /// podman's arguments for it. Pure. A name always follows `--`, so no
    /// name is read as an option.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        let on = |verb: &[&str], force: bool, id: &str| {
            let mut args: Vec<String> = verb.iter().map(|&s| s.to_owned()).collect();
            if force {
                args.push("--force".to_owned());
            }
            args.push("--".to_owned());
            args.push(id.to_owned());
            args
        };
        match self {
            Self::Start { id } => on(&["start"], false, id),
            Self::Stop { id } => on(&["stop"], false, id),
            Self::Restart { id } => on(&["restart"], false, id),
            Self::Pause { id } => on(&["pause"], false, id),
            Self::Unpause { id } => on(&["unpause"], false, id),
            Self::Remove { id, force } => on(&["rm"], *force, id),
            Self::StartPod { id } => on(&["pod", "start"], false, id),
            Self::StopPod { id } => on(&["pod", "stop"], false, id),
            Self::RestartPod { id } => on(&["pod", "restart"], false, id),
            Self::RemovePod { id, force } => on(&["pod", "rm"], *force, id),
            Self::RemoveImage { id, force } => on(&["rmi"], *force, id),
            Self::RemoveVolume { name } => on(&["volume", "rm"], false, name),
            Self::RemoveNetwork { name } => on(&["network", "rm"], false, name),
            Self::Prune { what } => {
                let scope = match what {
                    Prune::System => "system",
                    Prune::Images => "image",
                    Prune::Volumes => "volume",
                };
                vec![scope.to_owned(), "prune".to_owned(), "--force".to_owned()]
            }
        }
    }
}

/// What `podman exec` runs in the terminal for a shell in a container: bash
/// when it has one, sh otherwise. Pure.
#[must_use]
pub fn exec_arguments(id: &str) -> Vec<String> {
    [
        "exec",
        "-it",
        id,
        "sh",
        "-c",
        "command -v bash >/dev/null && exec bash || exec sh",
    ]
    .map(str::to_owned)
    .to_vec()
}

/// crun's state folder for the person: one folder per running container.
/// `None` without `XDG_RUNTIME_DIR`.
#[must_use]
pub fn runtime_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map(|d| PathBuf::from(d).join("crun"))
}

/// Why podman answered nothing, as a closed set (`containers-why-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// This process was not given podman (containers are off in this
    /// profile).
    NoPodman,
    /// podman could not be started.
    Unstartable,
    /// podman ended with a failure.
    Failed,
    /// podman answered something that is not the JSON asked for.
    Garbled,
    /// No such container, image, volume, network or pod.
    NotFound,
    /// The registry refused the login.
    Refused,
    /// The answer is larger than anything shown.
    TooLarge,
}

impl Why {
    /// Every refusal, for the keys' census.
    pub const ALL: [Self; 7] = [
        Self::NoPodman,
        Self::Unstartable,
        Self::Failed,
        Self::Garbled,
        Self::NotFound,
        Self::Refused,
        Self::TooLarge,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoPodman => "no-podman",
            Self::Unstartable => "unstartable",
            Self::Failed => "failed",
            Self::Garbled => "garbled",
            Self::NotFound => "not-found",
            Self::Refused => "refused",
            Self::TooLarge => "too-large",
        }
    }
}

/// A refusal of podman.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

const fn refused(why: Why) -> Refusal {
    Refusal { why }
}

/// What a failed podman said, as a refusal — MEASURED on the pinned
/// podman, one wording per kind of thing. Pure.
#[must_use]
fn verdict(err: &str) -> Why {
    let says = |words: &[&str]| words.iter().any(|w| err.contains(w));
    if says(&[
        "no such container",
        "no such object",
        "no such volume",
        "no such pod",
        "network not found",
        "image not known",
    ]) {
        Why::NotFound
    } else if says(&[
        "unauthorized",
        "authentication required",
        "invalid username/password",
        "incorrect username or password",
    ]) {
        Why::Refused
    } else {
        Why::Failed
    }
}

/// A failed podman's refusal — and podman's own reason, WRITTEN TO THE LOG
/// where the sentence on screen for [`Why::Failed`] sends the person:
/// a refusal is a closed set of sentences, and the words that say which
/// volume is in use, or which name is taken, are podman's.
///
/// Only the verb is logged with them, never the rest of the arguments, and
/// only the last [`SAID_MOST_BYTES`] of stderr: podman ends with its
/// `Error: …` line, which is the reason.
fn failed(args: &[&str], err: &str) -> Refusal {
    let why = verdict(err);
    tracing::warn!(
        verb = args.iter().take_while(|a| !a.starts_with('-')).take(2).copied().collect::<Vec<_>>().join(" "),
        why = ?why,
        said = said_tail(err),
        "podman refused"
    );
    refused(why)
}

/// How much of a failed podman's stderr goes to the log.
const SAID_MOST_BYTES: usize = 512;

/// The end of what a failed podman said, cut on a character's boundary.
fn said_tail(err: &str) -> &str {
    let err = err.trim();
    let mut from = err.len().saturating_sub(SAID_MOST_BYTES);
    while !err.is_char_boundary(from) {
        from += 1;
    }
    &err[from..]
}

/// The most a listing or an inspect may be.
pub const ANSWER_MOST_BYTES: usize = 32 * 1024 * 1024;
/// The most one streamed JSON document (a stats round) may be.
pub const DOCUMENT_MOST_BYTES: usize = 1024 * 1024;
/// The most stderr kept to read a refusal from.
const ERR_MOST_BYTES: usize = 64 * 1024;

/// A child's output, a line at a time; dropping it kills the child — a log
/// or a stats stream exists only while shown.
#[derive(Debug)]
pub struct Lines {
    held: Held,
    out: Reader,
}

/// The reading half of [`Lines`], once split: it ends when the child does.
#[derive(Debug)]
pub struct Reader {
    out: BufReader<std::io::PipeReader>,
}

/// The owning half of [`Lines`], once split: dropping it kills the child,
/// and the [`Reader`] on its thread then sees the end.
///
/// # Why a split
///
/// A reader blocks on its next line, so it lives on a thread; and the door
/// that decides the stream is no longer shown lives on the loop. Holding the
/// child where the decision is made is what makes "only while shown" one
/// `drop` — no flag a blocked reader would read only after the next line.
#[derive(Debug)]
pub struct Held {
    child: Option<Child>,
}

impl Drop for Held {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Held {
    /// The child's process id.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.child.as_ref().map_or(0, Child::id)
    }
}

impl Reader {
    /// The next line, without its end; `None` when the child ended. Blocks:
    /// read it on a thread.
    pub fn next_line(&mut self) -> Option<String> {
        let mut line = String::new();
        match self.out.read_line(&mut line) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(line.trim_end_matches(['\n', '\r']).to_owned()),
        }
    }

    /// The next whole JSON document, however many lines podman spread it
    /// over (`stats --format json` indents one array per round); `None`
    /// when the child ended or a document grew past
    /// [`DOCUMENT_MOST_BYTES`]. Blocks.
    pub fn next_document(&mut self) -> Option<Value> {
        let mut text = String::new();
        loop {
            let line = self.next_line()?;
            text.push_str(&line);
            text.push('\n');
            if text.len() > DOCUMENT_MOST_BYTES {
                return None;
            }
            let closes = matches!(line.trim_end().chars().last(), Some(']' | '}'));
            if closes && let Ok(value) = serde_json::from_str::<Value>(&text) {
                return Some(value);
            }
        }
    }
}

impl Lines {
    /// The next line, without its end; `None` when the child ended. Blocks:
    /// read it on a thread.
    pub fn next_line(&mut self) -> Option<String> {
        self.out.next_line()
    }

    /// The two halves: the reader for a thread, the child for whoever
    /// decides how long it lives.
    #[must_use]
    pub fn split(self) -> (Reader, Held) {
        (self.out, self.held)
    }

    /// The next whole JSON document — see [`Reader::next_document`].
    pub fn next_document(&mut self) -> Option<Value> {
        self.out.next_document()
    }

    /// The child's process id, for a caller that waits on it elsewhere.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.held.id()
    }
}

/// podman, by its store path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Podman {
    /// Where podman is.
    pub program: PathBuf,
}

/// What a finished podman said.
struct Ran {
    ok: bool,
    out: Vec<u8>,
    err: String,
}

impl Podman {
    /// podman as this process was given it; `None` without [`VARIABLE`].
    ///
    /// FROM THE ENVIRONMENT ONLY, never compiled in: podman's closure
    /// (conmon, crun, netavark, passt…) would then ride in every binary that
    /// links this crate, on every profile. `nix/modules/containers.nix` sets
    /// it for the session and the footer where containers are on; elsewhere
    /// its absence is [`Why::NoPodman`].
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::var_os(VARIABLE)
            .filter(|v| !v.is_empty())
            .map(|p| Self {
                program: PathBuf::from(p),
            })
    }

    /// The command for a queue's job: this podman with the job's
    /// arguments, in the folder it runs in. Its streams and its process
    /// group are the queue's to set: the queue owns the child, its cancel
    /// and its end.
    #[must_use]
    pub fn job_command(&self, job: &Job) -> Command {
        let mut command = Command::new(&self.program);
        command.args(job.arguments());
        if let Some(dir) = job.working_dir() {
            command.current_dir(dir);
        }
        command
    }

    /// What a terminal runs for a shell inside container `id`: this podman
    /// and [`exec_arguments`] — the program first, as the terminal takes it.
    #[must_use]
    pub fn exec_line(&self, id: &str) -> (String, Vec<String>) {
        (
            self.program.to_string_lossy().into_owned(),
            exec_arguments(id),
        )
    }

    /// podman with `args`, to its end; `stdin` written first when given.
    fn run(&self, args: &[&str], stdin: Option<&[u8]>) -> Result<Ran, Refusal> {
        let mut child = Command::new(&self.program)
            .args(args)
            .stdin(if stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| refused(Why::Unstartable))?;
        if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
            // A podman that stopped before reading says why on its exit.
            let _ = pipe.write_all(bytes);
        }
        let (Some(mut out), Some(mut err)) = (child.stdout.take(), child.stderr.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(refused(Why::Unstartable));
        };
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
            .take(u64::try_from(ANSWER_MOST_BYTES).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut bytes);
        let too_large = bytes.len() > ANSWER_MOST_BYTES;
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

    /// podman's JSON answer to `args`.
    fn json(&self, args: &[&str]) -> Result<Value, Refusal> {
        let ran = self.run(args, None)?;
        if !ran.ok {
            return Err(failed(args, &ran.err));
        }
        serde_json::from_slice(&ran.out).map_err(|_| refused(Why::Garbled))
    }

    /// A child whose output is read a line at a time — stdout, and stderr
    /// too into the same pipe when `with_stderr` (a log: a container's
    /// stderr is podman's stderr, and many services log only there).
    fn lines(&self, args: &[&str], with_stderr: bool) -> Result<Lines, Refusal> {
        let (reader, writer) = std::io::pipe().map_err(|_| refused(Why::Unstartable))?;
        let stderr = if with_stderr {
            Stdio::from(writer.try_clone().map_err(|_| refused(Why::Unstartable))?)
        } else {
            Stdio::null()
        };
        let mut command = Command::new(&self.program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(writer)
            .stderr(stderr);
        let spawned = command.spawn();
        // The command holds the pipe's writing ends: dropped now, or the
        // reader would never see the end when the child does.
        drop(command);
        let child = spawned.map_err(|_| refused(Why::Unstartable))?;
        Ok(Lines {
            held: Held { child: Some(child) },
            out: Reader {
                out: BufReader::new(reader),
            },
        })
    }

    /// Everything: containers, pods, images, volumes, networks, compose.
    ///
    /// # Errors
    ///
    /// [`Why::Garbled`] for an answer that is not the JSON podman prints,
    /// and what podman refuses.
    pub fn list(&self) -> Result<Listing, Refusal> {
        let garbled = refused(Why::Garbled);
        let containers =
            parse::containers(&self.json(&["ps", "--all", "--format", "json"])?).ok_or(garbled)?;
        let pods = parse::pods(&self.json(&["pod", "ps", "--format", "json"])?).ok_or(garbled)?;
        let images = parse::images(&self.json(&["images", "--format", "json"])?).ok_or(garbled)?;
        let volumes =
            parse::volumes(&self.json(&["volume", "ls", "--format", "json"])?).ok_or(garbled)?;
        let networks =
            parse::networks(&self.json(&["network", "ls", "--format", "json"])?).ok_or(garbled)?;
        let compose = compose_of(&containers);
        Ok(Listing {
            containers,
            pods,
            images,
            volumes,
            networks,
            compose,
        })
    }

    /// Does `action`.
    ///
    /// # Errors
    ///
    /// [`Why::NotFound`], [`Why::Failed`].
    pub fn act(&self, action: &Action) -> Result<(), Refusal> {
        let args = action.arguments();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let ran = self.run(&args, None)?;
        if ran.ok {
            Ok(())
        } else {
            Err(failed(&args, &ran.err))
        }
    }

    /// `podman inspect` of one thing, as podman wrote it (JSON, for the door
    /// to highlight).
    ///
    /// # Errors
    ///
    /// [`Why::NotFound`], [`Why::Failed`].
    pub fn inspect(&self, id: &str) -> Result<String, Refusal> {
        if id.is_empty() {
            return Err(refused(Why::NotFound));
        }
        let ran = self.run(&["inspect", "--format", "json", "--", id], None)?;
        if !ran.ok {
            return Err(failed(&["inspect"], &ran.err));
        }
        Ok(String::from_utf8_lossy(&ran.out).into_owned())
    }

    /// A container's log, the last `tail_lines` and then as it grows —
    /// what it wrote to stdout AND to stderr, in one stream in the order
    /// podman passed them on (podman prints a container's stderr on its
    /// own). What podman itself says, a container gone, is a last line.
    ///
    /// # Errors
    ///
    /// [`Why::Unstartable`].
    pub fn logs(&self, id: &str, tail_lines: u32) -> Result<Lines, Refusal> {
        self.lines(
            &[
                "logs",
                "--follow",
                "--tail",
                &tail_lines.to_string(),
                "--",
                id,
            ],
            true,
        )
    }

    /// Every running container's usage, one JSON document every
    /// `interval_s` seconds — read with [`Lines::next_document`] and
    /// [`parse_stats`].
    ///
    /// # Errors
    ///
    /// [`Why::Unstartable`].
    pub fn stats(&self, interval_s: u32) -> Result<Lines, Refusal> {
        // stderr apart: a warning in the middle of a document would break it.
        self.lines(
            &[
                "stats",
                "--format",
                "json",
                "--interval",
                &interval_s.to_string(),
            ],
            false,
        )
    }

    /// podman's events, one JSON object per line, while the app is open —
    /// read with [`parse_event`].
    ///
    /// # Errors
    ///
    /// [`Why::Unstartable`].
    pub fn events(&self) -> Result<Lines, Refusal> {
        self.lines(&["events", "--format", "json"], false)
    }

    /// Logs in to a registry, the password on stdin (`--password-stdin`):
    /// never an argument.
    ///
    /// # Errors
    ///
    /// [`Why::Refused`], [`Why::Failed`].
    pub fn login(&self, registry: &str, user: &str, password: &Secret) -> Result<(), Refusal> {
        if registry.is_empty() || user.is_empty() || password.is_empty() {
            return Err(refused(Why::Refused));
        }
        let mut line = password.expose().as_bytes().to_vec();
        line.push(b'\n');
        let ran = self.run(
            &[
                "login",
                "--username",
                user,
                "--password-stdin",
                "--",
                registry,
            ],
            Some(&line),
        );
        line.fill(0);
        let ran = ran?;
        if ran.ok {
            Ok(())
        } else {
            Err(failed(&["login"], &ran.err))
        }
    }
}

/// One round of [`Podman::stats`], as usage rows. `None` for what is not
/// one. Pure.
#[must_use]
pub fn parse_stats(document: &Value) -> Option<Vec<Usage>> {
    parse::stats(document)
}

/// One line of [`Podman::events`]. `None` for what is not one. Pure.
#[must_use]
pub fn parse_event(line: &str) -> Option<Event> {
    parse::event(line)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    use super::*;

    fn container(id: &str, project: Option<&str>) -> Container {
        Container {
            id: id.to_owned(),
            names: vec![id.to_owned()],
            image: "docker.io/library/nginx:latest".to_owned(),
            state: State::of("running"),
            status: String::new(),
            created_s: 0,
            ports: Vec::new(),
            pod: None,
            compose: project.map(|p| ComposeLabels {
                project: p.to_owned(),
                working_dir: Some(PathBuf::from("/home/someone/site")),
                config_files: vec![PathBuf::from("/home/someone/site/compose.yaml")],
                service: None,
            }),
            labels: BTreeMap::new(),
        }
    }

    #[test]
    fn actions_are_arguments_and_projects_are_labels() {
        assert_eq!(
            Action::Remove {
                id: "web".to_owned(),
                force: true
            }
            .arguments(),
            ["rm", "--force", "--", "web"]
        );
        assert_eq!(
            Action::Prune {
                what: Prune::Volumes
            }
            .arguments(),
            ["volume", "prune", "--force"]
        );
        let projects = compose_of(&[
            container("a1", Some("site")),
            container("b1", None),
            container("a2", Some("site")),
        ]);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].containers, ["a1", "a2"]);
        assert_eq!(State::of("Exited"), State::Exited);
        assert_eq!(exec_arguments("web")[2], "web");
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len());
    }

    #[test]
    fn a_failure_is_read_as_podman_worded_it() {
        for said in [
            "Error: no container with ID or name \"x\" found: no such container",
            "Error: no such object: \"x\"",
            "Error: no volume with name \"x\" found: no such volume",
            "Error: unable to find network with name or ID x: network not found",
            "Error: x: image not known",
            "Error: no pod with name or ID x found: no such pod",
        ] {
            assert_eq!(verdict(said), Why::NotFound, "{said}");
        }
        assert_eq!(
            verdict("Error: logging into \"r\": invalid username/password"),
            Why::Refused
        );
        assert_eq!(verdict("Error: something else"), Why::Failed);
        // WHAT GOES TO THE LOG is the end of what podman said, whole
        // characters only.
        let long = format!("{}Error: volume web is being used", "é".repeat(400));
        let tail = said_tail(&long);
        assert!(tail.ends_with("Error: volume web is being used"), "{tail}");
        assert!(tail.len() <= SAID_MOST_BYTES);
    }

    /// ONE TEST AT A TIME STARTS A PROGRAM HERE. A stand-in is a script
    /// written and then run; a `fork` in another test's thread between the
    /// write and the `exec` inherits the file open for writing, and the
    /// `exec` fails with "text file busy". Serialised, it cannot.
    static FORKS: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        FORKS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A stand-in podman: it writes down its argv and stdin, and answers as
    /// the fixture says (the real podman is the VM's: `tests/containers-vm.nix`).
    fn stand_in(dir: &Path, body: &str) -> Podman {
        let program = dir.join("podman");
        std::fs::write(
            &program,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{0}/argv'\n{body}\n",
                dir.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        Podman { program }
    }

    fn place(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-containers-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_listing_is_five_questions_and_the_projects_are_grouped() {
        let _one = one_at_a_time();
        let dir = place("list");
        for (name, text) in [
            ("ps", fixtures::CONTAINERS),
            ("pod", fixtures::PODS),
            ("images", fixtures::IMAGES),
            ("volume", fixtures::VOLUMES),
            ("network", fixtures::NETWORKS),
        ] {
            std::fs::write(dir.join(format!("{name}.json")), text).unwrap();
        }
        let podman = stand_in(&dir, &format!("cat '{}/'\"$1\".json", dir.display()));
        let listing = podman.list().unwrap();
        assert_eq!(listing.containers.len(), 2);
        assert_eq!(listing.pods.len(), 1);
        assert_eq!(listing.images.len(), 1);
        assert_eq!(listing.volumes.len(), 1);
        assert_eq!(listing.networks.len(), 1);
        assert_eq!(listing.compose.len(), 1);
        assert_eq!(listing.compose[0].name, "site");

        let garbled = stand_in(&dir, "echo 'not json'");
        assert_eq!(garbled.list().unwrap_err().why, Why::Garbled);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_action_says_what_podman_said() {
        let _one = one_at_a_time();
        let dir = place("act");
        let podman = stand_in(
            &dir,
            "echo 'Error: no container with ID or name \"x\" found: no such container' >&2; exit 125",
        );
        let stop = Action::Stop {
            id: "-x".to_owned(),
        };
        assert_eq!(podman.act(&stop).unwrap_err().why, Why::NotFound);
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        assert_eq!(
            argv, "stop\n--\n-x\n",
            "a name after `--` is never a switch"
        );
        let fine = stand_in(&dir, "exit 0");
        fine.act(&stop).unwrap();
        assert_eq!(fine.inspect("").unwrap_err().why, Why::NotFound);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_login_password_goes_to_stdin_and_never_to_argv() {
        let _one = one_at_a_time();
        let dir = place("login");
        let podman = stand_in(&dir, &format!("cat > '{}/stdin'", dir.display()));
        podman
            .login("ghcr.io", "someone", &Secret::new("hunter2 but long"))
            .unwrap();
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        assert!(!argv.contains("hunter2"), "{argv}");
        assert!(argv.lines().any(|a| a == "--password-stdin"));
        assert_eq!(
            std::fs::read_to_string(dir.join("stdin")).unwrap(),
            "hunter2 but long\n"
        );
        let refusing = stand_in(
            &dir,
            "cat > /dev/null; echo 'Error: logging into \"ghcr.io\": invalid username/password' >&2; exit 125",
        );
        assert_eq!(
            refusing
                .login("ghcr.io", "someone", &Secret::new("wrong"))
                .unwrap_err()
                .why,
            Why::Refused
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stream_lives_only_while_it_is_held() {
        let _one = one_at_a_time();
        let dir = place("stream");
        let round = r#"[
 {
  "id": "abc",
  "name": "web",
  "cpu_percent": "1.50%",
  "mem_usage": "4.1MB / 33.29GB",
  "mem_percent": "0.01%",
  "net_io": "5.19kB / 1.31kB",
  "block_io": "0B / 0B",
  "pids": "2"
 }
]"#;
        std::fs::write(dir.join("round"), round).unwrap();
        let podman = stand_in(
            &dir,
            &format!(
                "while true; do cat '{0}/round'; echo; sleep 0.05; done",
                dir.display()
            ),
        );
        let mut stats = podman.stats(2).unwrap();
        let pid = stats.id();
        for _ in 0..2 {
            let document = stats.next_document().unwrap();
            let usage = parse_stats(&document).unwrap();
            assert_eq!(usage[0].cpu, "1.50%");
        }
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        assert_eq!(argv, "stats\n--format\njson\n--interval\n2\n");
        drop(stats);
        let proc = PathBuf::from(format!("/proc/{pid}"));
        assert!(
            !proc.exists()
                || std::fs::read_to_string(proc.join("stat")).is_ok_and(|s| s.contains(") Z ")),
            "the stream's podman is gone once it is dropped"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// A log is both streams: podman prints a container's stderr on its
    /// own stderr, and nginx, most runtimes and every crash write there.
    #[test]
    fn a_log_is_what_the_container_wrote_on_both_streams() {
        let _one = one_at_a_time();
        let dir = place("logs");
        let podman = stand_in(
            &dir,
            "echo 'GET / 200'\necho 'worker died' >&2\necho 'GET /x 404'\nexec sleep 30",
        );
        let mut logs = podman.logs("web", 100).unwrap();
        let mut seen = Vec::new();
        for _ in 0..3 {
            seen.push(logs.next_line().unwrap());
        }
        assert_eq!(seen, ["GET / 200", "worker died", "GET /x 404"], "in order");
        let argv = std::fs::read_to_string(dir.join("argv")).unwrap();
        assert_eq!(argv, "logs\n--follow\n--tail\n100\n--\nweb\n");
        drop(logs);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// SPLIT, the child lives as long as its HELD half: dropped on the
    /// loop, the reader blocked on its thread sees the end.
    #[test]
    fn a_split_stream_ends_when_its_held_half_is_dropped() {
        let _one = one_at_a_time();
        let dir = place("split");
        let podman = stand_in(&dir, "echo first\nexec sleep 30");
        let (mut reader, held) = podman.logs("web", 10).unwrap().split();
        assert_eq!(reader.next_line().as_deref(), Some("first"));
        let (tx, rx) = std::sync::mpsc::channel();
        let thread = std::thread::spawn(move || {
            let _ = tx.send(reader.next_line());
        });
        drop(held);
        let ended = rx
            .recv_timeout(std::time::Duration::from_secs(10))
            .expect("the reader was never told the end");
        assert_eq!(ended, None);
        thread.join().unwrap();
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_job_runs_the_one_podman_in_its_folder() {
        let podman = Podman {
            program: PathBuf::from("/nix/store/x-podman/bin/podman"),
        };
        let job = Job::ComposeUp {
            project: "site".to_owned(),
            files: vec![PathBuf::from("/home/someone/site/compose.yaml")],
        };
        let command = podman.job_command(&job);
        assert_eq!(command.get_program(), "/nix/store/x-podman/bin/podman");
        assert_eq!(
            command.get_current_dir(),
            Some(std::path::Path::new("/home/someone/site"))
        );
        let (program, args) = podman.exec_line("web");
        assert_eq!(program, "/nix/store/x-podman/bin/podman");
        assert_eq!(args, exec_arguments("web"));
        assert_eq!(
            Action::StopPod {
                id: "duo".to_owned()
            }
            .arguments(),
            ["pod", "stop", "--", "duo"]
        );
    }

    #[test]
    fn without_podman_there_is_nothing_to_run() {
        let _one = one_at_a_time();
        let missing = Podman {
            program: PathBuf::from("/nonexistent/podman"),
        };
        assert_eq!(missing.list().unwrap_err().why, Why::Unstartable);
        assert_eq!(missing.logs("x", 10).unwrap_err().why, Why::Unstartable);
        assert!(runtime_dir().is_none_or(|d| d.ends_with("crun")));
    }
}

// Declared LAST, after the tests: the checks cut a file at its first
// `#[cfg(test)] mod`, and everything after it is not the product.
#[cfg(test)]
mod fixtures;
