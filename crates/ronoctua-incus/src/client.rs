//! The daemon, as the doors and the queue's child ask it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ronoctua_consent::Pass;
use serde_json::{Map, Value, json};

use crate::rest::{self, Halt, Operation};
use crate::{
    Action, Console, Create, Image, Instance, Kind, LimitChange, Limits, Network, Offered, Pool,
    Profile, Project, Refusal, Remote, Snapshot, Source, State, Volume, Why, cli, events,
    is_instance_name, is_snapshot_name, parse,
};

/// How long an action waits for its operation before leaving it to the
/// daemon: a stop is given 30 s by the daemon, then forced.
const ACTION_PATIENCE: Duration = Duration::from_secs(300);

/// How long the daemon gives an instance to stop before it is forced.
const STOP_TIMEOUT_S: u64 = 30;

/// The daemon, through its socket (and its CLI, from the environment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Incus {
    /// The daemon's unix socket.
    pub socket: PathBuf,
    /// The CLI (`RONOCTUA_INCUS`), when given.
    pub cli: Option<PathBuf>,
}

/// An action's wait: the patience, nothing heard, nothing stopped.
fn patiently() -> (Option<Instant>, impl FnMut(&Operation) -> bool) {
    (Some(Instant::now() + ACTION_PATIENCE), |_: &Operation| true)
}

/// A followed call's refusal, for a caller that cannot stop it.
fn refusal(halt: Halt) -> Refusal {
    match halt {
        Halt::Refused(r) => r,
        Halt::Stopped => Refusal::of(Why::Failed),
    }
}

/// The path of an instance, refusing a name that is not one.
pub(crate) fn instance_path(name: &str) -> Result<String, Refusal> {
    if is_instance_name(name) {
        Ok(format!("/1.0/instances/{name}"))
    } else {
        Err(Refusal::of(Why::Invalid))
    }
}

/// The path of an instance's snapshot: the instance's, and the snapshot's
/// name percent-encoded as one segment (a name the daemon made may hold
/// `%`, `?`, `#` or any letter). Pure.
///
/// # Errors
///
/// [`Why::Invalid`] for an instance's or a snapshot's name Incus does not
/// take.
pub(crate) fn snapshot_path(name: &str, snapshot: &str) -> Result<String, Refusal> {
    let instance = instance_path(name)?;
    if !is_snapshot_name(snapshot) {
        return Err(Refusal::of(Why::Invalid));
    }
    Ok(format!(
        "{instance}/snapshots/{}",
        rest::path_segment(snapshot)
    ))
}

/// A fingerprint, or a refusal: it goes into a path.
fn fingerprint_path(fingerprint: &str) -> Result<String, Refusal> {
    if !fingerprint.is_empty()
        && fingerprint.len() <= 64
        && fingerprint.chars().all(|c| c.is_ascii_hexdigit())
    {
        Ok(format!("/1.0/images/{}", fingerprint.to_ascii_lowercase()))
    } else {
        Err(Refusal::of(Why::Invalid))
    }
}

/// A pool's or a profile's name as it goes into a path: Incus's own names
/// are letters, digits, `-`, `_` and `.`, and none is `.` or `..`.
fn plain_name(name: &str) -> Result<&str, Refusal> {
    let fits = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if !name.is_empty() && name.len() <= 63 && name.chars().all(fits) && name != "." && name != ".."
    {
        Ok(name)
    } else {
        Err(Refusal::of(Why::Invalid))
    }
}

/// The configuration keys a set of limits writes: a value, or `None` to take
/// the limit away. Pure.
///
/// # Errors
///
/// [`Why::Invalid`] for a limit of processes on a VM (its kernel counts its
/// own: the daemon refuses the key) or a zero.
pub(crate) fn limit_keys(
    limits: Limits,
    kind: Kind,
) -> Result<[(&'static str, Option<String>); 3], Refusal> {
    let positive = |n: Option<u64>| {
        n.map_or(Ok(()), |n| {
            if n == 0 {
                Err(Refusal::of(Why::Invalid))
            } else {
                Ok(())
            }
        })
    };
    positive(limits.cpu.map(u64::from))?;
    positive(limits.memory_bytes)?;
    positive(limits.processes)?;
    if kind == Kind::Vm && limits.processes.is_some() {
        return Err(Refusal::of(Why::Invalid));
    }
    Ok([
        ("limits.cpu", limits.cpu.map(|n| n.to_string())),
        (
            "limits.memory",
            limits.memory_bytes.map(|n| format!("{n}B")),
        ),
        (
            "limits.processes",
            match kind {
                Kind::Container => limits.processes.map(|n| n.to_string()),
                Kind::Vm => None,
            },
        ),
    ])
}

/// The ONE configuration key a change writes: a value, or `None` to take
/// the instance's own away. Pure.
///
/// # Errors
///
/// As [`limit_keys`].
pub(crate) fn limit_key(
    change: LimitChange,
    kind: Kind,
) -> Result<(&'static str, Option<String>), Refusal> {
    let (limits, which) = match change {
        LimitChange::Cpu(cpu) => (
            Limits {
                cpu,
                ..Limits::default()
            },
            0,
        ),
        LimitChange::MemoryBytes(memory_bytes) => (
            Limits {
                memory_bytes,
                ..Limits::default()
            },
            1,
        ),
        LimitChange::Processes(processes) => (
            Limits {
                processes,
                ..Limits::default()
            },
            2,
        ),
    };
    let [cpu, memory, processes] = limit_keys(limits, kind)?;
    Ok(match which {
        0 => cpu,
        1 => memory,
        _ => processes,
    })
}

/// Whether `passes` cover every one of `hosts`.
///
/// # Errors
///
/// [`Why::NotLet`] for the first host no pass covers (an empty one included:
/// an address that names no host).
pub(crate) fn covered(hosts: &[String], passes: &[Pass]) -> Result<(), Refusal> {
    if hosts
        .iter()
        .all(|host| passes.iter().any(|pass| pass.covers(host)))
    {
        Ok(())
    } else {
        Err(Refusal::of(Why::NotLet))
    }
}

/// `pass` — a door's, for a remote's address — and one `minter` answers for
/// each of `others` it does not cover: the hosts the remote's client asks
/// besides its address ([`crate::hosts_of`]). A host nobody let gets none,
/// and [`covered`] then refuses.
pub(crate) fn passes_for(
    pass: &Pass,
    others: &[String],
    minter: &dyn Fn(&str) -> Option<Pass>,
) -> Vec<Pass> {
    let mut passes = vec![pass.clone()];
    for host in others.iter().filter(|h| !pass.covers(h)) {
        if let Some(minted) = minter(host) {
            passes.push(minted);
        }
    }
    passes
}

/// The person's own consent file, at the moment of asking.
fn from_the_consent_file(host: &str) -> Option<Pass> {
    ronoctua_consent::pass(host).ok()
}

/// The `source` of a new instance. A remote's is built ONLY under passes
/// that cover every host its client asks ([`Source::needs_hosts`]) — the one
/// place this crate names a server for the daemon to reach.
///
/// # Errors
///
/// [`Why::NotLet`] for a remote one of whose hosts no pass covers (or no
/// pass).
pub(crate) fn source_of(source: &Source, passes: &[Pass]) -> Result<Value, Refusal> {
    match source {
        Source::Remote {
            server,
            protocol,
            alias,
        } => {
            covered(&source.needs_hosts(), passes)?;
            Ok(json!({
                "type": "image",
                "mode": "pull",
                "server": server,
                "protocol": protocol.word(),
                "alias": alias,
            }))
        }
        Source::Local { fingerprint } => {
            fingerprint_path(fingerprint)?;
            Ok(json!({ "type": "image", "fingerprint": fingerprint.to_ascii_lowercase() }))
        }
    }
}

/// An image copied from a remote: the request, built only under passes for
/// every host its client asks, never updating itself.
///
/// # Errors
///
/// [`Why::NotLet`] for a remote one of whose hosts no pass covers.
pub(crate) fn image_copy_of(
    passes: &[Pass],
    server: &str,
    protocol: crate::Protocol,
    alias: &str,
    kind: Option<Kind>,
) -> Result<Value, Refusal> {
    covered(&crate::hosts_of(server, protocol), passes)?;
    Ok(json!({
        "source": {
            "type": "image",
            "mode": "pull",
            "server": server,
            "protocol": protocol.word(),
            "alias": alias,
            "image_type": kind.map_or("", Kind::word),
        },
        // NOTHING UPDATES BY ITSELF: an image refreshed on the daemon's clock
        // is a request to a host nobody asked for at that moment.
        "auto_update": false,
    }))
}

impl Incus {
    /// The daemon of this machine, as the environment names it: the CLI from
    /// [`crate::VARIABLE`] (whose absence is a profile without Incus), the
    /// socket from [`crate::SOCKET_VARIABLE`] or the NixOS module's.
    ///
    /// FROM THE ENVIRONMENT ONLY, never compiled in: the CLI's closure would
    /// ride in every binary that links this crate, on every profile.
    ///
    /// # Errors
    ///
    /// [`Why::NoIncus`] without the CLI's variable.
    pub fn from_env() -> Result<Self, Refusal> {
        let cli = std::env::var_os(crate::VARIABLE)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .ok_or(Refusal::of(Why::NoIncus))?;
        let socket = std::env::var_os(crate::SOCKET_VARIABLE)
            .filter(|v| !v.is_empty())
            .map_or_else(|| PathBuf::from(crate::SOCKET), PathBuf::from);
        Ok(Self {
            socket,
            cli: Some(cli),
        })
    }

    /// Every instance of the default project, with its state and its
    /// snapshots (`recursion=2`).
    ///
    /// THE DEFAULT PROJECT ONLY: every action here names an instance by its
    /// name, and a name is an instance's only inside one project. Another
    /// project is the CLI's.
    ///
    /// # Errors
    ///
    /// [`Why::NoIncus`], [`Why::NotInGroup`], [`Why::Failed`].
    pub fn instances(&self) -> Result<Vec<Instance>, Refusal> {
        parse::instances_of(&rest::get(&self.socket, "/1.0/instances?recursion=2")?)
    }

    /// One instance, with its state and snapshots.
    ///
    /// # Errors
    ///
    /// As [`Incus::instances`]; [`Why::Gone`]; [`Why::Invalid`] for a name
    /// Incus does not take.
    pub fn instance(&self, name: &str) -> Result<Instance, Refusal> {
        let path = format!("{}?recursion=1", instance_path(name)?);
        parse::instance_of(&rest::get(&self.socket, &path)?)
    }

    /// The local images.
    ///
    /// # Errors
    ///
    /// As [`Incus::instances`].
    pub fn images(&self) -> Result<Vec<Image>, Refusal> {
        parse::images_of(&rest::get(&self.socket, "/1.0/images?recursion=1")?)
    }

    /// Deletes a local image.
    ///
    /// # Errors
    ///
    /// As [`Incus::instance`].
    pub fn delete_image(&self, fingerprint: &str) -> Result<(), Refusal> {
        let path = fingerprint_path(fingerprint)?;
        let (until, mut heard) = patiently();
        rest::operate(&self.socket, "DELETE", &path, None, until, &mut heard)
            .map(|_| ())
            .map_err(refusal)
    }

    /// The profiles.
    ///
    /// # Errors
    ///
    /// As [`Incus::instances`].
    pub fn profiles(&self) -> Result<Vec<Profile>, Refusal> {
        parse::profiles_of(&rest::get(&self.socket, "/1.0/profiles?recursion=1")?)
    }

    /// The networks.
    ///
    /// # Errors
    ///
    /// As [`Incus::instances`].
    pub fn networks(&self) -> Result<Vec<Network>, Refusal> {
        parse::networks_of(&rest::get(&self.socket, "/1.0/networks?recursion=1")?)
    }

    /// The storage pools.
    ///
    /// # Errors
    ///
    /// As [`Incus::instances`].
    pub fn pools(&self) -> Result<Vec<Pool>, Refusal> {
        parse::pools_of(&rest::get(&self.socket, "/1.0/storage-pools?recursion=1")?)
    }

    /// A pool's volumes.
    ///
    /// # Errors
    ///
    /// As [`Incus::instance`].
    pub fn volumes(&self, pool: &str) -> Result<Vec<Volume>, Refusal> {
        let path = format!(
            "/1.0/storage-pools/{}/volumes?recursion=1",
            plain_name(pool)?
        );
        parse::volumes_of(&rest::get(&self.socket, &path)?)
    }

    /// The projects.
    ///
    /// # Errors
    ///
    /// As [`Incus::instances`].
    pub fn projects(&self) -> Result<Vec<Project>, Refusal> {
        parse::projects_of(&rest::get(&self.socket, "/1.0/projects?recursion=1")?)
    }

    /// An instance's snapshots.
    ///
    /// # Errors
    ///
    /// As [`Incus::instance`].
    pub fn snapshots(&self, name: &str) -> Result<Vec<Snapshot>, Refusal> {
        let path = format!("{}/snapshots?recursion=1", instance_path(name)?);
        parse::snapshots_of(&rest::get(&self.socket, &path)?)
    }

    /// An instance's live state.
    ///
    /// # Errors
    ///
    /// As [`Incus::instance`].
    pub fn state(&self, name: &str) -> Result<State, Refusal> {
        let path = format!("{}/state", instance_path(name)?);
        parse::state_of(&rest::get(&self.socket, &path)?)
    }

    /// Tells an instance to do something, and waits for it to be done. A
    /// running (or frozen) instance told to go is stopped first, forced: the
    /// person already answered the question.
    ///
    /// # Errors
    ///
    /// As [`Incus::instance`]; [`Why::Busy`] when another operation holds
    /// it, or this one outlived the patience (it goes on, the daemon's).
    pub fn act(&self, name: &str, action: Action) -> Result<(), Refusal> {
        let path = instance_path(name)?;
        let (until, mut heard) = patiently();
        let Some(word) = action.state_word() else {
            if self.instance(name)?.status != crate::Status::Stopped {
                self.change_state(&path, "stop", true)?;
            }
            return rest::operate(&self.socket, "DELETE", &path, None, until, &mut heard)
                .map(|_| ())
                .map_err(refusal);
        };
        self.change_state(&path, word, false)
    }

    /// `PUT …/state`, followed to its end.
    fn change_state(&self, path: &str, word: &str, force: bool) -> Result<(), Refusal> {
        let body = json!({
            "action": word,
            "timeout": STOP_TIMEOUT_S,
            "force": force,
            "stateful": false,
        })
        .to_string();
        let (until, mut heard) = patiently();
        rest::operate(
            &self.socket,
            "PUT",
            &format!("{path}/state"),
            Some(&body),
            until,
            &mut heard,
        )
        .map(|_| ())
        .map_err(refusal)
    }

    /// Sets an instance's limits (`None` takes one away), against the
    /// version it read: a change made meanwhile refuses this one
    /// ([`Why::Busy`]) instead of being overwritten.
    ///
    /// ALL THREE are written into the instance's own configuration: a door
    /// that changes one limit asks [`Incus::set_limit`], which leaves the
    /// others — and a profile's — as they are.
    ///
    /// # Errors
    ///
    /// As [`Incus::instance`]; [`Why::Invalid`] for a zero, or a limit of
    /// processes on a VM.
    pub fn set_limits(&self, name: &str, limits: Limits) -> Result<(), Refusal> {
        self.put_config(name, |kind| Ok(limit_keys(limits, kind)?.to_vec()))
    }

    /// Sets ONE of an instance's limits in its own configuration, and
    /// nothing else: the other limits stay as they are, and a limit a
    /// profile gives is never copied into the instance (it would stop
    /// following the profile). `None` takes the instance's own away. Against
    /// the version it read, as [`Incus::set_limits`].
    ///
    /// # Errors
    ///
    /// As [`Incus::set_limits`].
    pub fn set_limit(&self, name: &str, change: LimitChange) -> Result<(), Refusal> {
        self.put_config(name, |kind| Ok(vec![limit_key(change, kind)?]))
    }

    /// Writes configuration keys into an instance's own configuration
    /// (`None` takes one away), against the version it read.
    fn put_config(
        &self,
        name: &str,
        keys: impl FnOnce(Kind) -> Result<Vec<(&'static str, Option<String>)>, Refusal>,
    ) -> Result<(), Refusal> {
        let path = instance_path(name)?;
        let reply = rest::call(&self.socket, "GET", &path, None, rest::READ)?;
        let etag = reply.header("etag").map(str::to_owned);
        let now = rest::sync(&reply)?;
        let kind = now
            .get("type")
            .and_then(Value::as_str)
            .and_then(Kind::of_word)
            .ok_or(Refusal::of(Why::Failed))?;
        let body = with_config(&now, &keys(kind)?)?.to_string();
        let mut headers = Vec::new();
        if let Some(etag) = etag.as_deref() {
            headers.push(("If-Match", etag));
        }
        let reply = rest::call_with(
            &self.socket,
            "PUT",
            &path,
            Some(&body),
            &headers,
            rest::READ,
        )?;
        if reply.status == 412 {
            return Err(Refusal::of(Why::Busy));
        }
        match rest::answered(&reply)? {
            rest::Answered::Sync(_) => Ok(()),
            rest::Answered::Async(id, _) => {
                let (until, mut heard) = patiently();
                rest::wait(&self.socket, &id, until, &mut heard)
                    .map(|_| ())
                    .map_err(refusal)
            }
        }
    }

    /// Takes a snapshot.
    ///
    /// # Errors
    ///
    /// As [`Incus::instance`]; [`Why::Invalid`] for a name the daemon does
    /// not take ([`crate::is_snapshot_name`]), or one taken.
    pub fn snapshot(&self, name: &str, snapshot: &str) -> Result<(), Refusal> {
        let path = format!("{}/snapshots", instance_path(name)?);
        if !is_snapshot_name(snapshot) {
            return Err(Refusal::of(Why::Invalid));
        }
        let body = json!({ "name": snapshot, "stateful": false }).to_string();
        let (until, mut heard) = patiently();
        rest::operate(&self.socket, "POST", &path, Some(&body), until, &mut heard)
            .map(|_| ())
            .map_err(refusal)
    }

    /// Restores a snapshot.
    ///
    /// # Errors
    ///
    /// As [`Incus::snapshot`].
    pub fn restore(&self, name: &str, snapshot: &str) -> Result<(), Refusal> {
        let path = instance_path(name)?;
        if !is_snapshot_name(snapshot) {
            return Err(Refusal::of(Why::Invalid));
        }
        let body = json!({ "restore": snapshot }).to_string();
        let (until, mut heard) = patiently();
        rest::operate(&self.socket, "PUT", &path, Some(&body), until, &mut heard)
            .map(|_| ())
            .map_err(refusal)
    }

    /// Deletes a snapshot.
    ///
    /// # Errors
    ///
    /// As [`Incus::snapshot`].
    pub fn delete_snapshot(&self, name: &str, snapshot: &str) -> Result<(), Refusal> {
        let path = snapshot_path(name, snapshot)?;
        let (until, mut heard) = patiently();
        rest::operate(&self.socket, "DELETE", &path, None, until, &mut heard)
            .map(|_| ())
            .map_err(refusal)
    }

    /// Creates an instance (not started) and waits for it. The daemon
    /// fetches a remote's image: `pass` is the door's for the remote's host,
    /// and every OTHER host its client asks ([`Source::needs_hosts`]: Docker
    /// Hub's two) must be let too — read from the person's `consent.json`
    /// here, at the moment of asking, as the queue's child reads all of
    /// them. Checked before the socket is opened.
    ///
    /// Minutes, for a remote image: a door queues [`crate::Job::Launch`]
    /// instead, and this is for the rest.
    ///
    /// # Errors
    ///
    /// [`Why::NotLet`] when a host of the source's is not let; as
    /// [`Incus::instance`] and [`Incus::set_limits`].
    pub fn create(&self, pass: &Pass, create: &Create) -> Result<(), Refusal> {
        let hosts = create.source.needs_hosts();
        // THE DOOR'S PASS IS THE ADDRESS'S: the file is read only for the
        // hosts after it.
        let others = hosts.get(1..).unwrap_or_default();
        let passes = passes_for(pass, others, &from_the_consent_file);
        self.create_with(&passes, create)
    }

    /// Creates an instance from a LOCAL image (not started): no host is
    /// asked, so no pass. A remote source is refused ([`Why::NotLet`]) —
    /// [`Incus::create`] is its door.
    ///
    /// # Errors
    ///
    /// As [`Incus::create`].
    pub fn create_local(&self, create: &Create) -> Result<(), Refusal> {
        self.create_with(&[], create)
    }

    /// Both creates.
    fn create_with(&self, passes: &[Pass], create: &Create) -> Result<(), Refusal> {
        let id = self.start_create(passes, create, false)?;
        let (until, mut heard) = patiently();
        rest::wait(&self.socket, &id, until, &mut heard)
            .map(|_| ())
            .map_err(refusal)
    }

    /// Submits a creation and answers its operation's id, for whoever
    /// follows it ([`crate::job::run`] does, with progress).
    ///
    /// # Errors
    ///
    /// As [`Incus::create`].
    pub(crate) fn start_create(
        &self,
        passes: &[Pass],
        create: &Create,
        start: bool,
    ) -> Result<String, Refusal> {
        if !is_instance_name(&create.name) {
            return Err(Refusal::of(Why::Invalid));
        }
        // THE PASSES FIRST, before anything is asked of the daemon.
        let source = source_of(&create.source, passes)?;
        let mut config = Map::new();
        for (key, value) in limit_keys(create.limits, create.kind)? {
            if let Some(value) = value {
                config.insert(key.to_owned(), Value::String(value));
            }
        }
        let mut body = json!({
            "name": create.name,
            "type": create.kind.word(),
            "source": source,
            "config": config,
            "start": start,
        });
        // AN EMPTY LIST WOULD MEAN NO PROFILE, and no root disk: absent is
        // the daemon's `default`.
        if !create.profiles.is_empty() {
            for profile in &create.profiles {
                plain_name(profile)?;
            }
            body["profiles"] = json!(create.profiles);
        }
        if create.kind == Kind::Vm
            && let Some(root) = self.vm_root(&create.profiles)?
        {
            body["devices"] = json!({ "root": root });
        }
        let reply = rest::call(
            &self.socket,
            "POST",
            "/1.0/instances",
            Some(&body.to_string()),
            rest::READ,
        )?;
        match rest::answered(&reply)? {
            rest::Answered::Async(id, _) => Ok(id),
            rest::Answered::Sync(_) => {
                tracing::warn!("incus created an instance with no operation");
                Err(Refusal::of(Why::Failed))
            }
        }
    }

    /// A new VM's root disk, asked to be without copy-on-write where its
    /// pool is btrfs ([`parse::nocow_root`], which says what is not proven):
    /// the root disk its profiles give it (the last one that has one wins,
    /// as the daemon applies them), on its pool's driver.
    fn vm_root(&self, profiles: &[String]) -> Result<Option<Map<String, Value>>, Refusal> {
        let named: Vec<&str> = if profiles.is_empty() {
            vec!["default"]
        } else {
            profiles.iter().map(String::as_str).collect()
        };
        let mut root = None;
        for profile in named {
            let value = rest::get(
                &self.socket,
                &format!("/1.0/profiles/{}", plain_name(profile)?),
            )?;
            if let Some(found) = parse::root_disk_of(&value) {
                root = Some(found);
            }
        }
        let Some(root) = root else { return Ok(None) };
        let Some(pool) = root.get("pool").and_then(Value::as_str) else {
            return Ok(None);
        };
        let driver = parse::driver_of(&rest::get(
            &self.socket,
            &format!("/1.0/storage-pools/{}", plain_name(pool)?),
        )?)?;
        Ok(parse::nocow_root(&root, &driver))
    }

    /// The events, while the returned value lives.
    ///
    /// # Errors
    ///
    /// As [`Incus::instances`].
    pub fn events(&self) -> Result<events::Events, Refusal> {
        events::Events::open(&self.socket)
    }

    /// The client's remotes that serve images.
    ///
    /// # Errors
    ///
    /// [`Why::NoIncus`] without the CLI; [`Why::Failed`] when it failed.
    pub fn remotes(&self) -> Result<Vec<Remote>, Refusal> {
        cli::remotes(self.cli()?)
    }

    /// A remote's image catalogue, for this machine's architecture. The CLI
    /// fetches it from the remote: `pass` must cover the remote's host, and
    /// every other host its client asks ([`crate::hosts_of`]) must be let in
    /// `consent.json` — checked before the CLI starts.
    ///
    /// # Errors
    ///
    /// [`Why::NotLet`] when a host of the remote's is not let;
    /// [`Why::Gone`] for a remote the client does not have; as
    /// [`Incus::remotes`].
    pub fn catalogue(&self, pass: &Pass, remote: &str) -> Result<Vec<Offered>, Refusal> {
        cli::catalogue(self.cli()?, pass, remote, &from_the_consent_file)
    }

    /// The command line for a console in our terminal: the CLI and its
    /// arguments.
    ///
    /// # Errors
    ///
    /// [`Why::NoIncus`] without the CLI; [`Why::Invalid`] for a name Incus
    /// does not take.
    pub fn console_command(&self, name: &str, console: Console) -> Result<Vec<String>, Refusal> {
        instance_path(name)?;
        self.command(crate::console_arguments(name, console))
    }

    /// The command line for a login shell in an instance.
    ///
    /// # Errors
    ///
    /// As [`Incus::console_command`].
    pub fn shell_command(&self, name: &str) -> Result<Vec<String>, Refusal> {
        instance_path(name)?;
        self.command(crate::shell_arguments(name))
    }

    /// The CLI, first, then `args`.
    fn command(&self, args: Vec<String>) -> Result<Vec<String>, Refusal> {
        let mut line = vec![self.cli()?.to_string_lossy().into_owned()];
        line.extend(args);
        Ok(line)
    }

    /// The CLI, or the refusal of a profile without it.
    fn cli(&self) -> Result<&std::path::Path, Refusal> {
        self.cli.as_deref().ok_or(Refusal::of(Why::NoIncus))
    }
}

/// An instance's writable half (`InstancePut`) out of what was read, with
/// these limits. Pure.
///
/// # Errors
///
/// As [`limit_keys`].
#[cfg(test)]
pub(crate) fn with_limits(now: &Value, limits: Limits, kind: Kind) -> Result<Value, Refusal> {
    with_config(now, &limit_keys(limits, kind)?)
}

/// An instance's writable half out of what was read, with these keys of
/// its own configuration set (`None` takes one away) — and every other key
/// as it was. Pure.
///
/// # Errors
///
/// [`Why::Failed`] for a configuration that is not an object.
pub(crate) fn with_config(
    now: &Value,
    keys: &[(&'static str, Option<String>)],
) -> Result<Value, Refusal> {
    let mut put = Map::new();
    for key in [
        "architecture",
        "config",
        "devices",
        "ephemeral",
        "profiles",
        "stateful",
        "description",
    ] {
        if let Some(value) = now.get(key) {
            put.insert(key.to_owned(), value.clone());
        }
    }
    let config = put
        .entry("config")
        .or_insert_with(|| Value::Object(Map::new()));
    if !config.is_object() {
        *config = Value::Object(Map::new());
    }
    let config = config.as_object_mut().ok_or(Refusal::of(Why::Failed))?;
    for (key, value) in keys {
        match value {
            Some(value) => {
                config.insert((*key).to_owned(), Value::String(value.clone()));
            }
            None => {
                config.remove(*key);
            }
        }
    }
    Ok(Value::Object(put))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Protocol, fixtures};

    fn pass_for(tag: &str, host: &str) -> Pass {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-incus-pass-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = ronoctua_consent::store_at(Some(dir.clone()));
        ronoctua_consent::let_hosts_in(&store, &[host], true).unwrap();
        let pass = ronoctua_consent::pass_in(&store, host).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        pass
    }

    /// An Incus whose socket is not there: any call that reached it would
    /// answer [`Why::NoIncus`], so a different answer was given BEFORE it.
    fn nowhere() -> Incus {
        Incus {
            socket: std::env::temp_dir()
                .join(format!("ronoctua-incus-absent-{}", std::process::id())),
            cli: None,
        }
    }

    fn remote_create() -> Create {
        Create {
            name: "web".to_owned(),
            kind: Kind::Container,
            source: Source::Remote {
                server: "https://images.linuxcontainers.org".to_owned(),
                protocol: Protocol::SimpleStreams,
                alias: "debian/13".to_owned(),
            },
            profiles: Vec::new(),
            limits: Limits::default(),
        }
    }

    /// FAILS IF: the daemon is asked to fetch a remote's image under a pass
    /// for another host, or with none — the refusal must come before the
    /// socket (the socket here is absent, so reaching it says `no-incus`).
    #[test]
    fn a_remote_image_is_asked_for_only_under_its_host_s_pass() {
        let incus = nowhere();
        let other = pass_for("other", "example.org");
        assert_eq!(
            incus.create(&other, &remote_create()),
            Err(Refusal::of(Why::NotLet))
        );
        assert_eq!(
            incus.create_local(&remote_create()),
            Err(Refusal::of(Why::NotLet)),
            "no pass at all"
        );
        let right = pass_for("right", "images.linuxcontainers.org");
        assert_eq!(
            incus.create(&right, &remote_create()),
            Err(Refusal::of(Why::NoIncus)),
            "with its pass, the daemon is asked"
        );
        assert_eq!(
            image_copy_of(
                std::slice::from_ref(&other),
                "https://images.linuxcontainers.org",
                Protocol::SimpleStreams,
                "x",
                None
            ),
            Err(Refusal::of(Why::NotLet))
        );
        let copy = image_copy_of(
            std::slice::from_ref(&right),
            "https://images.linuxcontainers.org",
            Protocol::SimpleStreams,
            "x",
            Some(Kind::Vm),
        )
        .unwrap();
        assert_eq!(
            copy["auto_update"],
            json!(false),
            "nothing updates by itself"
        );
        assert_eq!(copy["source"]["image_type"], json!("virtual-machine"));
        // AND A LOCAL IMAGE ASKS NOBODY.
        let local = Create {
            source: Source::Local {
                fingerprint: "A1B2".to_owned(),
            },
            ..remote_create()
        };
        assert_eq!(incus.create_local(&local), Err(Refusal::of(Why::NoIncus)));
        assert_eq!(
            source_of(&local.source, &[]).unwrap(),
            json!({ "type": "image", "fingerprint": "a1b2" })
        );
    }

    /// FAILS IF: an image is pulled from Docker Hub — whose client asks
    /// `registry-1.docker.io` and `auth.docker.io` by its own constants —
    /// under the consent of `docker.io` alone.
    #[test]
    fn an_oci_pull_needs_every_host_its_client_asks() {
        let hub = Source::Remote {
            server: "https://docker.io".to_owned(),
            protocol: Protocol::Oci,
            alias: "nginx".to_owned(),
        };
        let address = pass_for("hub", "docker.io");
        assert_eq!(
            source_of(&hub, std::slice::from_ref(&address)),
            Err(Refusal::of(Why::NotLet)),
            "the address's pass alone"
        );
        let (registry, auth) = (
            pass_for("registry", "registry-1.docker.io"),
            pass_for("auth", "auth.docker.io"),
        );
        assert_eq!(
            source_of(&hub, &[address.clone(), registry.clone()]),
            Err(Refusal::of(Why::NotLet)),
            "two of three"
        );
        let all = [address.clone(), registry.clone(), auth.clone()];
        assert_eq!(
            source_of(&hub, &all).unwrap()["server"],
            "https://docker.io"
        );
        assert_eq!(
            image_copy_of(&all[..2], "https://docker.io", Protocol::Oci, "nginx", None),
            Err(Refusal::of(Why::NotLet))
        );
        assert!(image_copy_of(&all, "https://docker.io", Protocol::Oci, "nginx", None).is_ok());

        // THE OTHER HOSTS ARE MINTED, the address's never: a door's pass is
        // the address's, and the file is asked only for the rest.
        let asked = std::cell::RefCell::new(Vec::new());
        let minter = |host: &str| {
            asked.borrow_mut().push(host.to_owned());
            [registry.clone(), auth.clone()]
                .into_iter()
                .find(|p| p.covers(host))
        };
        let hosts = hub.needs_hosts();
        let passes = passes_for(&address, &hosts[1..], &minter);
        assert_eq!(
            asked.borrow().as_slice(),
            ["registry-1.docker.io", "auth.docker.io"]
        );
        assert!(source_of(&hub, &passes).is_ok());
        let refusing = |_: &str| None;
        let passes = passes_for(&address, &hosts[1..], &refusing);
        assert_eq!(source_of(&hub, &passes), Err(Refusal::of(Why::NotLet)));
    }

    /// FAILS IF: a snapshot the daemon made with a name freer than an
    /// instance's (`pre_upgrade`) is refused, or its name reaches the path
    /// unencoded.
    #[test]
    fn a_snapshot_of_the_daemon_s_naming_is_restored_and_deleted() {
        let incus = nowhere();
        for name in ["pre_upgrade", "v1.0", "2026-10-05_12:30"] {
            assert_eq!(
                incus.restore("web", name),
                Err(Refusal::of(Why::NoIncus)),
                "{name} reached the daemon"
            );
            assert_eq!(
                incus.delete_snapshot("web", name),
                Err(Refusal::of(Why::NoIncus)),
                "{name} reached the daemon"
            );
        }
        assert_eq!(
            snapshot_path("web", "v1.0 #2?").unwrap_err(),
            Refusal::of(Why::Invalid),
            "a space is the daemon's refusal too"
        );
        assert_eq!(
            snapshot_path("web", "a%b?c#d").unwrap(),
            "/1.0/instances/web/snapshots/a%25b%3Fc%23d"
        );
        assert_eq!(
            snapshot_path("web", "pre_upgrade").unwrap(),
            "/1.0/instances/web/snapshots/pre_upgrade"
        );
        assert_eq!(
            incus.delete_snapshot("web", ".."),
            Err(Refusal::of(Why::Invalid))
        );
    }

    #[test]
    fn a_name_that_is_not_one_never_reaches_a_path() {
        let incus = nowhere();
        assert_eq!(incus.instance("../1.0"), Err(Refusal::of(Why::Invalid)));
        assert_eq!(
            incus.act("a/b", Action::Start),
            Err(Refusal::of(Why::Invalid))
        );
        assert_eq!(incus.snapshot("web", "x/y"), Err(Refusal::of(Why::Invalid)));
        assert_eq!(incus.volumes(".."), Err(Refusal::of(Why::Invalid)));
        assert_eq!(incus.delete_image("../x"), Err(Refusal::of(Why::Invalid)));
        assert_eq!(
            incus.instance("web"),
            Err(Refusal::of(Why::NoIncus)),
            "a good name is asked"
        );
        assert_eq!(
            incus.remotes(),
            Err(Refusal::of(Why::NoIncus)),
            "no CLI here"
        );
        assert_eq!(
            incus.console_command("web", Console::Text),
            Err(Refusal::of(Why::NoIncus))
        );
        let with_cli = Incus {
            cli: Some("/nix/store/x-incus/bin/incus".into()),
            ..nowhere()
        };
        assert_eq!(
            with_cli.console_command("vm1", Console::Vga).unwrap(),
            [
                "/nix/store/x-incus/bin/incus",
                "console",
                "vm1",
                "--type=vga"
            ]
        );
        assert_eq!(
            with_cli.shell_command("web").unwrap(),
            [
                "/nix/store/x-incus/bin/incus",
                "exec",
                "web",
                "--",
                "su",
                "-l"
            ]
        );
        assert_eq!(with_cli.shell_command("-x"), Err(Refusal::of(Why::Invalid)));
    }

    /// FAILS IF: changing one limit writes the others too — a profile's
    /// limit copied into the instance stops following the profile, and a
    /// processor set (`0-1`) rewritten as a count stops pinning — or `None`
    /// takes away more than the one asked.
    #[test]
    fn one_limit_changed_leaves_the_others_and_the_profiles_as_they_are() {
        let now =
            serde_json::from_str::<Value>(fixtures::INSTANCE_PLAIN).unwrap()["metadata"].clone();
        let put = with_config(
            &now,
            &[limit_key(LimitChange::MemoryBytes(Some(1 << 30)), Kind::Container).unwrap()],
        )
        .unwrap();
        assert_eq!(put["config"]["limits.memory"], json!("1073741824B"));
        assert_eq!(
            put["config"]["limits.cpu"],
            json!("0-1"),
            "the processors' set is left as it was"
        );
        assert_eq!(put["config"]["limits.processes"], json!("500"));
        let put = with_config(
            &now,
            &[limit_key(LimitChange::Processes(None), Kind::Container).unwrap()],
        )
        .unwrap();
        assert!(
            put["config"].get("limits.processes").is_none(),
            "taken away"
        );
        assert_eq!(put["config"]["limits.cpu"], json!("0-1"), "and only it");
        assert!(
            put["config"].get("limits.memory").is_none(),
            "nothing written that was not asked"
        );
        assert_eq!(
            limit_key(LimitChange::Processes(Some(10)), Kind::Vm),
            Err(Refusal::of(Why::Invalid)),
            "a VM's kernel counts its own processes"
        );
        assert_eq!(
            limit_key(LimitChange::Cpu(Some(0)), Kind::Container),
            Err(Refusal::of(Why::Invalid))
        );
        assert_eq!(
            limit_key(LimitChange::Cpu(Some(4)), Kind::Vm),
            Ok(("limits.cpu", Some("4".to_owned())))
        );
    }

    #[test]
    fn limits_are_written_in_the_daemon_s_words_and_a_none_takes_one_away() {
        let now =
            serde_json::from_str::<Value>(fixtures::INSTANCE_PLAIN).unwrap()["metadata"].clone();
        let put = with_limits(
            &now,
            Limits {
                cpu: Some(4),
                memory_bytes: Some(1 << 30),
                processes: None,
            },
            Kind::Container,
        )
        .unwrap();
        assert_eq!(put["config"]["limits.cpu"], json!("4"));
        assert_eq!(put["config"]["limits.memory"], json!("1073741824B"));
        assert!(
            put["config"].get("limits.processes").is_none(),
            "None took it away"
        );
        assert_eq!(
            put["config"]["image.description"],
            json!("Debian trixie amd64 (20261004_05:24)"),
            "the rest of the configuration is kept"
        );
        assert!(
            put.get("expanded_config").is_none(),
            "only the writable half goes back"
        );
        assert!(put.get("status").is_none());
        assert_eq!(
            with_limits(
                &now,
                Limits {
                    processes: Some(10),
                    ..Limits::default()
                },
                Kind::Vm
            ),
            Err(Refusal::of(Why::Invalid)),
            "a VM's kernel counts its own processes"
        );
        assert_eq!(
            limit_keys(
                Limits {
                    cpu: Some(0),
                    ..Limits::default()
                },
                Kind::Container
            ),
            Err(Refusal::of(Why::Invalid))
        );
    }
}
