//! Reading podman's JSON — pure, so every shape is tested against what the
//! pinned podman printed (`fixtures.rs`).
//!
//! Tolerant of what may be missing (`null` where a list is empty, a field a
//! newer podman drops) and strict about what must be there: a container
//! without an id is not a container, and the whole answer is refused.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    COMPOSE_CONFIG_FILES, COMPOSE_PROJECT, COMPOSE_SERVICE, COMPOSE_WORKING_DIR, ComposeLabels,
    Container, Image, Network, Pod, Port, State, Volume,
};

/// The most host ports one published range is spread into: a range of a
/// thousand ports is a range, not a thousand rows.
pub(crate) const RANGE_MOST_PORTS: u16 = 64;

fn text<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key)?.as_str()
}

fn texts(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn labels(v: &Value) -> BTreeMap<String, String> {
    v.get("Labels")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's algorithm).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Seconds since the epoch of an RFC 3339 time
/// (`2026-09-30T06:33:07.067241604+01:00`, `…Z`); `None` for anything else.
/// Pure.
#[must_use]
pub(crate) fn rfc3339(t: &str) -> Option<i64> {
    let (date, rest) = t.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (d.next()??, d.next()??, d.next()??);
    let clock_end = rest.find(['Z', 'z', '+', '-']).unwrap_or(rest.len());
    let (clock, zone) = rest.split_at(clock_end);
    let clock = clock.split('.').next()?;
    let mut c = clock.split(':').map(|p| p.parse::<i64>().ok());
    let (hour, minute, second) = (c.next()??, c.next()??, c.next()??);
    let offset = match zone {
        "Z" | "z" | "" => 0,
        _ => {
            let sign = if zone.starts_with('-') { -1 } else { 1 };
            let (h, m) = zone.get(1..)?.split_once(':')?;
            sign * (h.parse::<i64>().ok()? * 3600 + m.parse::<i64>().ok()? * 60)
        }
    };
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && (0..24).contains(&hour)
        && (0..60).contains(&minute)
        && (0..=60).contains(&second);
    valid.then(|| {
        days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second - offset
    })
}

/// A time podman gives as seconds or as RFC 3339.
fn seconds(v: &Value, key: &str) -> i64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => rfc3339(s).unwrap_or(0),
        _ => 0,
    }
}

fn ports(v: &Value) -> Vec<Port> {
    let Some(all) = v.get("Ports").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for p in all {
        let number = |key| {
            p.get(key)
                .and_then(Value::as_u64)
                .and_then(|n| u16::try_from(n).ok())
        };
        let (Some(host), Some(inside)) = (number("host_port"), number("container_port")) else {
            continue;
        };
        let range = number("range").unwrap_or(1).clamp(1, RANGE_MOST_PORTS);
        let host_ip = text(p, "host_ip")
            .filter(|ip| !ip.is_empty())
            .map(str::to_owned);
        let protocol = text(p, "protocol").unwrap_or("tcp").to_owned();
        for n in 0..range {
            let (Some(host_port), Some(container_port)) =
                (host.checked_add(n), inside.checked_add(n))
            else {
                break;
            };
            out.push(Port {
                host_ip: host_ip.clone(),
                host_port,
                container_port,
                protocol: protocol.clone(),
            });
        }
    }
    out
}

fn compose(labels: &BTreeMap<String, String>) -> Option<ComposeLabels> {
    let project = labels.get(COMPOSE_PROJECT)?.clone();
    let working_dir = labels
        .get(COMPOSE_WORKING_DIR)
        .filter(|d| !d.is_empty())
        .map(PathBuf::from);
    let given: Vec<PathBuf> = labels
        .get(COMPOSE_CONFIG_FILES)
        .map(|f| {
            f.split(',')
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    Some(ComposeLabels {
        project,
        config_files: resolved_files(working_dir.as_deref(), &given),
        working_dir,
        service: labels.get(COMPOSE_SERVICE).cloned(),
    })
}

/// The project's files as paths that hold wherever they are used. Pure.
///
/// # Why compose's label cannot be used as it is
///
/// `podman-compose` writes `config_files` AS IT WAS GIVEN THEM — usually the
/// bare `compose.yaml` it found in the folder it was run from — and
/// `working_dir` as the first file's folder, made real
/// (`fixtures::COMPOSE_CONTAINERS`). A relative file handed back to compose
/// from any other folder names nothing; its parent, an empty path, is no
/// folder to run in.
///
/// # Where compose was run, from what it wrote
///
/// The folder it was run in is `working_dir` less the folders the FIRST file
/// named on its way (`shop/compose.yaml` was given from `shop`'s parent), and
/// every relative file is that folder's. A first file that climbs (`..`) or
/// is absolute while another is relative leaves that folder unknown, and
/// then NO file is answered: a project run with some of its files is a
/// different project, and the door says it does not know where the files are
/// rather than act on part of them.
#[must_use]
pub(crate) fn resolved_files(working_dir: Option<&Path>, given: &[PathBuf]) -> Vec<PathBuf> {
    let ran_in = given.first().filter(|f| f.is_relative()).and_then(|first| {
        let mut at = working_dir?.to_path_buf();
        for part in first.parent()?.components() {
            match part {
                Component::Normal(_) => {
                    if !at.pop() {
                        return None;
                    }
                }
                Component::CurDir => {}
                _ => return None,
            }
        }
        Some(at)
    });
    let mut out = Vec::with_capacity(given.len());
    for file in given {
        if file.is_absolute() {
            out.push(file.clone());
        } else if let Some(folder) = &ran_in {
            // `components` drops a `./`, which `join` would keep.
            out.push(folder.join(file).components().collect());
        } else {
            return Vec::new();
        }
    }
    out
}

/// `ps --all --format json`, the pods' infra containers left out. Pure.
#[must_use]
pub(crate) fn containers(v: &Value) -> Option<Vec<Container>> {
    let mut out = Vec::new();
    for c in v.as_array()? {
        if c.get("IsInfra").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let labels = labels(c);
        out.push(Container {
            id: text(c, "Id")?.to_owned(),
            names: texts(c, "Names"),
            image: text(c, "Image").unwrap_or_default().to_owned(),
            state: State::of(text(c, "State").unwrap_or_default()),
            status: text(c, "Status").unwrap_or_default().to_owned(),
            created_s: seconds(c, "Created"),
            ports: ports(c),
            pod: text(c, "Pod").filter(|p| !p.is_empty()).map(str::to_owned),
            compose: compose(&labels),
            labels,
        });
    }
    Some(out)
}

/// `pod ps --format json`, each pod's infra container left out of its
/// list. Pure.
#[must_use]
pub(crate) fn pods(v: &Value) -> Option<Vec<Pod>> {
    let mut out = Vec::new();
    for p in v.as_array()? {
        let infra = text(p, "InfraId").unwrap_or_default();
        let containers = p
            .get("Containers")
            .and_then(Value::as_array)
            .map(|all| {
                all.iter()
                    .filter_map(|c| text(c, "Id"))
                    .filter(|id| *id != infra)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        out.push(Pod {
            id: text(p, "Id")?.to_owned(),
            name: text(p, "Name").unwrap_or_default().to_owned(),
            status: text(p, "Status").unwrap_or_default().to_owned(),
            containers,
            created_s: seconds(p, "Created"),
        });
    }
    Some(out)
}

/// `images --format json`. An image no name points at is dangling. Pure.
#[must_use]
pub(crate) fn images(v: &Value) -> Option<Vec<Image>> {
    let mut out = Vec::new();
    for i in v.as_array()? {
        let mut names = texts(i, "Names");
        if names.is_empty() {
            names = texts(i, "RepoTags");
        }
        out.push(Image {
            id: text(i, "Id")?.to_owned(),
            dangling: names.is_empty(),
            names,
            size_bytes: i.get("Size").and_then(Value::as_u64).unwrap_or(0),
            created_s: seconds(i, "Created"),
            containers: i
                .get("Containers")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0),
        });
    }
    Some(out)
}

/// `volume ls --format json`. Pure.
#[must_use]
pub(crate) fn volumes(v: &Value) -> Option<Vec<Volume>> {
    let mut out = Vec::new();
    for x in v.as_array()? {
        out.push(Volume {
            name: text(x, "Name")?.to_owned(),
            driver: text(x, "Driver").unwrap_or_default().to_owned(),
            mountpoint: PathBuf::from(text(x, "Mountpoint").unwrap_or_default()),
            created_s: seconds(x, "CreatedAt"),
        });
    }
    Some(out)
}

/// `network ls --format json` — netavark's lower-case keys. Pure.
#[must_use]
pub(crate) fn networks(v: &Value) -> Option<Vec<Network>> {
    let mut out = Vec::new();
    for n in v.as_array()? {
        let subnets = n
            .get("subnets")
            .and_then(Value::as_array)
            .map(|all| {
                all.iter()
                    .filter_map(|s| text(s, "subnet").map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        out.push(Network {
            id: text(n, "id")?.to_owned(),
            name: text(n, "name")?.to_owned(),
            driver: text(n, "driver").unwrap_or_default().to_owned(),
            subnets,
            dns_enabled: n
                .get("dns_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        });
    }
    Some(out)
}

/// One container's usage in a round of `stats --format json` — podman's
/// own figures, as it words them ("0.12%", "4.1MB / 33.29GB"): data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// The container's id.
    pub id: String,
    /// Its name.
    pub name: String,
    /// Processor use.
    pub cpu: String,
    /// Memory used, of the limit.
    pub memory: String,
    /// Memory, as a share of the limit.
    pub memory_share: String,
    /// Network in and out.
    pub network: String,
    /// Disk in and out.
    pub disk: String,
    /// Processes.
    pub pids: String,
}

/// A round of `stats --format json`. Pure.
#[must_use]
pub(crate) fn stats(v: &Value) -> Option<Vec<Usage>> {
    let field = |u: &Value, key| text(u, key).unwrap_or_default().to_owned();
    v.as_array()?
        .iter()
        .map(|u| {
            Some(Usage {
                id: text(u, "id")?.to_owned(),
                name: field(u, "name"),
                cpu: field(u, "cpu_percent"),
                memory: field(u, "mem_usage"),
                memory_share: field(u, "mem_percent"),
                network: field(u, "net_io"),
                disk: field(u, "block_io"),
                pids: field(u, "pids"),
            })
        })
        .collect()
}

/// One of podman's events — what the app refreshes on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// What it happened to: `container`, `pod`, `image`, `volume`,
    /// `network`.
    pub kind: String,
    /// What happened: `create`, `start`, `died`, `remove`, `pull`…
    pub status: String,
    /// The thing's id, when it has one.
    pub id: Option<String>,
    /// Its name, when it has one.
    pub name: Option<String>,
}

/// A line of `events --format json`. Pure.
#[must_use]
pub(crate) fn event(line: &str) -> Option<Event> {
    let v: Value = serde_json::from_str(line).ok()?;
    let owned = |key| text(&v, key).filter(|s| !s.is_empty()).map(str::to_owned);
    Some(Event {
        kind: owned("Type")?,
        status: owned("Status")?,
        id: owned("ID"),
        name: owned("Name"),
    })
}

/// A mount of a container, as `inspect` says it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mount {
    /// `bind`, `volume`, `tmpfs`…
    pub kind: String,
    /// The folder or the volume's name on this machine.
    pub source: String,
    /// Where it appears inside.
    pub destination: String,
    /// Whether the container may write it.
    pub writable: bool,
}

/// What `inspect` adds to a listing's row — for the detail's tabs. Every
/// field is what podman printed (data); what it did not print is empty.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Details {
    /// What runs: the entry point and its arguments, or the image's command.
    pub command: Vec<String>,
    /// When it last started, seconds since the epoch.
    pub started_s: Option<i64>,
    /// When it last ended, seconds since the epoch.
    pub finished_s: Option<i64>,
    /// How its last run ended, when it ended.
    pub exit_code: Option<i64>,
    /// How many times podman restarted it.
    pub restarts: u64,
    /// Its restart policy's name (`no`, `always`, `on-failure`…).
    pub restart_policy: Option<String>,
    /// The folder it runs in, inside.
    pub working_dir: Option<String>,
    /// Whom it runs as, inside.
    pub user: Option<String>,
    /// Its mounts.
    pub mounts: Vec<Mount>,
    /// Its environment, `NAME` and value, in podman's order.
    pub env: Vec<(String, String)>,
    /// The networks it is on, and its address on each.
    pub networks: Vec<(String, String)>,
    /// An image's architecture and system (`amd64`, `linux`).
    pub platform: Option<String>,
}

/// The first thing of an `inspect --format json` answer, read for
/// [`Details`]. `None` for what is not such an answer. Pure.
#[must_use]
pub fn details_of(answer: &str) -> Option<Details> {
    let all: Value = serde_json::from_str(answer).ok()?;
    let v = all.as_array()?.first()?;
    let config = v.get("Config");
    let state = v.get("State");
    let owned_texts = |x: Option<&Value>| -> Vec<String> {
        match x {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect(),
            Some(Value::String(s)) if !s.is_empty() => vec![s.clone()],
            _ => Vec::new(),
        }
    };
    // WHAT RUNS: a container says its path and arguments; an image, its
    // entry point and command.
    let mut command = owned_texts(v.get("Path"));
    command.extend(owned_texts(v.get("Args")));
    if command.is_empty() {
        command = owned_texts(config.and_then(|c| c.get("Entrypoint")));
        command.extend(owned_texts(config.and_then(|c| c.get("Cmd"))));
    }
    let moment = |x: Option<&Value>| {
        x.and_then(Value::as_str)
            .and_then(rfc3339)
            // podman's "never": the year one.
            .filter(|&s| s > 0)
    };
    let nonempty = |x: Option<&Value>| {
        x.and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let env = config
        .and_then(|c| c.get("Env"))
        .and_then(Value::as_array)
        .map(|all| {
            all.iter()
                .filter_map(Value::as_str)
                .map(|pair| match pair.split_once('=') {
                    Some((k, v)) => (k.to_owned(), v.to_owned()),
                    None => (pair.to_owned(), String::new()),
                })
                .collect()
        })
        .unwrap_or_default();
    let mounts = v
        .get("Mounts")
        .and_then(Value::as_array)
        .map(|all| {
            all.iter()
                .map(|m| Mount {
                    kind: text(m, "Type").unwrap_or_default().to_owned(),
                    source: text(m, "Name")
                        .filter(|n| !n.is_empty() && text(m, "Type") == Some("volume"))
                        .or_else(|| text(m, "Source"))
                        .unwrap_or_default()
                        .to_owned(),
                    destination: text(m, "Destination").unwrap_or_default().to_owned(),
                    writable: m.get("RW").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let networks = v
        .get("NetworkSettings")
        .and_then(|n| n.get("Networks"))
        .and_then(Value::as_object)
        .map(|all| {
            all.iter()
                .map(|(name, n)| {
                    (
                        name.clone(),
                        text(n, "IPAddress").unwrap_or_default().to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let platform = match (text(v, "Architecture"), text(v, "Os")) {
        (Some(a), Some(o)) if !a.is_empty() && !o.is_empty() => Some(format!("{o}/{a}")),
        _ => None,
    };
    Some(Details {
        command,
        started_s: moment(state.and_then(|s| s.get("StartedAt"))),
        finished_s: moment(state.and_then(|s| s.get("FinishedAt"))),
        exit_code: state
            .filter(|s| s.get("Running").and_then(Value::as_bool) == Some(false))
            .and_then(|s| s.get("ExitCode"))
            .and_then(Value::as_i64),
        restarts: v.get("RestartCount").and_then(Value::as_u64).unwrap_or(0),
        restart_policy: nonempty(
            v.get("HostConfig")
                .and_then(|h| h.get("RestartPolicy"))
                .and_then(|r| r.get("Name")),
        ),
        working_dir: nonempty(config.and_then(|c| c.get("WorkingDir"))),
        user: nonempty(config.and_then(|c| c.get("User"))),
        mounts,
        env,
        networks,
        platform,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_container_is_read_as_podman_printed_it() {
        let all = containers(&json(fixtures::CONTAINERS)).unwrap();
        assert_eq!(all.len(), 2, "the pod's infra container is not listed");
        let web = all.iter().find(|c| c.names == ["web"]).unwrap();
        assert_eq!(web.state, State::Created);
        assert_eq!(web.image, "localhost/tiny:1");
        assert!(web.created_s > 1_790_000_000);
        assert_eq!(web.pod, None);
        assert_eq!(web.ports.len(), 3, "80, and the range 5000–5001");
        assert!(web.ports.contains(&Port {
            host_ip: None,
            host_port: 5001,
            container_port: 5001,
            protocol: "udp".to_owned()
        }));
        assert!(
            web.ports
                .iter()
                .any(|p| p.host_port == 8080 && p.container_port == 80)
        );
        let compose = web.compose.as_ref().unwrap();
        assert_eq!(compose.project, "site");
        assert_eq!(compose.service.as_deref(), Some("web"));
        assert_eq!(
            compose.config_files,
            [PathBuf::from("/home/x/site/compose.yaml")]
        );
        let inpod = all.iter().find(|c| c.names == ["inpod"]).unwrap();
        let pods = pods(&json(fixtures::PODS)).unwrap();
        assert_eq!(
            inpod.pod.as_ref(),
            Some(&pods[0].id),
            "`Pod` is the pod's id"
        );
        assert!(inpod.compose.is_none() && inpod.labels.is_empty());
    }

    #[test]
    fn a_compose_project_names_files_that_hold_from_anywhere() {
        // AS THE PINNED podman-compose WROTE THEM: a bare `compose.yaml`, and
        // two files given from the folder above, under a name of their own.
        let all = containers(&json(fixtures::COMPOSE_CONTAINERS)).unwrap();
        let projects = crate::compose_of(&all);
        let site = projects.iter().find(|p| p.name == "site").unwrap();
        assert_eq!(
            site.config_files,
            [PathBuf::from("/home/x/site/compose.yaml")]
        );
        assert_eq!(site.working_dir.as_deref(), Some(Path::new("/home/x/site")));
        let duo = projects.iter().find(|p| p.name == "duo").unwrap();
        assert_eq!(
            duo.config_files,
            [
                PathBuf::from("/home/x/shop/compose.yaml"),
                PathBuf::from("/home/x/shop/compose.override.yaml")
            ]
        );
        // EVERY FILE A JOB IS GIVEN IS ONE, and the folder it runs in exists
        // as a path — never the empty parent of a bare name.
        let job = crate::Job::ComposeDown {
            project: duo.name.clone(),
            files: duo.config_files.clone(),
        };
        assert_eq!(job.working_dir(), Some(PathBuf::from("/home/x/shop")));
    }

    #[test]
    fn a_file_whose_folder_is_unknown_is_no_file() {
        let dir = Some(Path::new("/home/x/site"));
        let files = |all: &[&str]| all.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(
            resolved_files(dir, &files(&["./compose.yaml"])),
            files(&["/home/x/site/compose.yaml"])
        );
        assert_eq!(
            resolved_files(dir, &files(&["/srv/a.yaml"])),
            files(&["/srv/a.yaml"])
        );
        // A CLIMB, a relative file after an absolute one, or no folder at
        // all: nothing, never a guess.
        assert!(resolved_files(dir, &files(&["../compose.yaml"])).is_empty());
        assert!(resolved_files(dir, &files(&["/srv/a.yaml", "b.yaml"])).is_empty());
        assert!(resolved_files(None, &files(&["compose.yaml"])).is_empty());
        // A FOLDER SHALLOWER THAN THE PATH that was given cannot be.
        assert!(resolved_files(Some(Path::new("/")), &files(&["a/b/c.yaml"])).is_empty());
    }

    #[test]
    fn pods_images_volumes_and_networks_are_read() {
        let p = pods(&json(fixtures::PODS)).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].name, "duo");
        assert_eq!(p[0].containers.len(), 1, "without its infra container");
        let infra = containers(&json(fixtures::CONTAINERS)).unwrap();
        assert_eq!(
            p[0].containers,
            infra
                .iter()
                .filter(|c| c.names == ["inpod"])
                .map(|c| c.id.clone())
                .collect::<Vec<_>>()
        );
        assert!(p[0].created_s > 1_790_000_000, "RFC 3339 at +01:00");
        let i = images(&json(fixtures::IMAGES)).unwrap();
        assert_eq!(i[0].names, ["localhost/tiny:1"]);
        assert!(!i[0].dangling);
        assert_eq!(i[0].size_bytes, 11_455);
        assert_eq!(i[0].containers, 2);
        let v = volumes(&json(fixtures::VOLUMES)).unwrap();
        assert_eq!(v[0].name, "v1");
        assert!(v[0].mountpoint.ends_with("volumes/v1/_data"));
        assert!(v[0].created_s > 1_790_000_000);
        let n = networks(&json(fixtures::NETWORKS)).unwrap();
        assert_eq!(n[0].name, "podman");
        assert_eq!(n[0].subnets, ["10.88.0.0/16"]);
        assert!(!n[0].dns_enabled);
        assert_eq!(containers(&json("{}")), None);
        assert_eq!(
            containers(&json("[{\"Names\":[]}]")),
            None,
            "no id, no answer"
        );
    }

    #[test]
    fn times_ports_stats_and_events() {
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339("1970-01-01T01:00:00+01:00"), Some(0));
        assert_eq!(rfc3339("1969-12-31T23:00:00.5-01:00"), Some(0));
        assert_eq!(rfc3339("yesterday"), None);
        let many = json(
            r#"{"Ports":[{"host_ip":"127.0.0.1","container_port":1,"host_port":1000,"range":5000,"protocol":"tcp"}]}"#,
        );
        let p = ports(&many);
        assert_eq!(p.len(), usize::from(RANGE_MOST_PORTS));
        assert_eq!(p[0].host_ip.as_deref(), Some("127.0.0.1"));
        let round = json(
            r#"[{"id":"abc","name":"web","cpu_percent":"0.12%","mem_usage":"4.1MB / 33.29GB","mem_percent":"0.01%","net_io":"5.19kB / 1.31kB","block_io":"0B / 0B","pids":"2"}]"#,
        );
        let u = stats(&round).unwrap();
        assert_eq!(u[0].memory, "4.1MB / 33.29GB");
        assert_eq!(
            event(
                r#"{"ID":"abc","Image":"x","Name":"web","Status":"start","Time":"2026-09-30T06:33:07+01:00","Type":"container"}"#
            ),
            Some(Event {
                kind: "container".to_owned(),
                status: "start".to_owned(),
                id: Some("abc".to_owned()),
                name: Some("web".to_owned())
            })
        );
        assert_eq!(event("not json"), None);
        let seen = event(fixtures::EVENT).unwrap();
        assert_eq!(
            (seen.kind.as_str(), seen.status.as_str()),
            ("container", "create")
        );
        assert!(seen.id.is_some() && seen.name.is_some());
    }

    #[test]
    fn an_inspect_is_read_for_the_detail() {
        let d = details_of(fixtures::INSPECT).unwrap();
        assert_eq!(d.command, ["nginx", "-g", "daemon off;"]);
        assert_eq!(d.exit_code, None, "a running container has no ending yet");
        assert!(d.started_s.is_some_and(|s| s > 0));
        assert_eq!(d.finished_s, None, "the year one is podman's never");
        assert_eq!(d.restart_policy.as_deref(), Some("always"));
        assert_eq!(d.mounts.len(), 2);
        assert_eq!(d.mounts[0].source, "/home/someone/site/html");
        assert!(!d.mounts[0].writable);
        assert_eq!(d.mounts[1].source, "site-data", "a volume by its name");
        assert_eq!(
            d.env
                .iter()
                .find(|(k, _)| k == "API_TOKEN")
                .map(|(_, v)| v.as_str()),
            Some("s3cr3t=with=equals")
        );
        assert_eq!(d.networks, [("podman".to_owned(), "10.88.0.7".to_owned())]);
        assert_eq!(details_of("[]"), None);
        assert_eq!(details_of("not json"), None);
    }
}
