//! The daemon's JSON — and the CLI's — read into this crate's shapes.
//!
//! Every reader is pure and tolerant of what the daemon leaves out or sends
//! as `null` (a stopped VM's `network`, an image without aliases); it is NOT
//! tolerant of a value it does not know: an instance of a type that is
//! neither a container nor a VM refuses the listing ([`Why::Failed`], logged)
//! instead of being drawn as one of the two.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::{
    Image, Instance, Kind, Limits, Network, Offered, Pool, Profile, Project, Protocol, Refusal,
    Remote, Snapshot, State, Status, Volume, Why,
};

/// `null` read as the type's default.
fn nullable<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

/// A shape this build does not read, logged and refused.
fn garbled(what: &str, e: &dyn std::fmt::Display) -> Refusal {
    tracing::warn!(what, error = %e, "incus answered a shape this build does not read");
    Refusal::of(Why::Failed)
}

/// Reads `T` out of a value.
fn read<'a, T: Deserialize<'a>>(value: &'a Value, what: &str) -> Result<T, Refusal> {
    T::deserialize(value).map_err(|e| garbled(what, &e))
}

#[derive(Debug, Deserialize)]
struct RawInstance {
    name: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    status_code: i64,
    #[serde(default)]
    project: String,
    #[serde(default)]
    description: String,
    #[serde(default, deserialize_with = "nullable")]
    profiles: Vec<String>,
    #[serde(default)]
    created_at: String,
    #[serde(default, deserialize_with = "nullable")]
    expanded_config: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "nullable")]
    config: BTreeMap<String, String>,
    #[serde(default)]
    state: Option<RawState>,
    #[serde(default, deserialize_with = "nullable")]
    snapshots: Vec<RawSnapshot>,
}

#[derive(Debug, Default, Deserialize)]
struct RawState {
    #[serde(default, deserialize_with = "nullable")]
    disk: BTreeMap<String, RawUsage>,
    #[serde(default, deserialize_with = "nullable")]
    memory: RawUsage,
    #[serde(default, deserialize_with = "nullable")]
    network: BTreeMap<String, RawNic>,
    #[serde(default)]
    processes: i64,
    #[serde(default, deserialize_with = "nullable")]
    cpu: RawUsage,
}

#[derive(Debug, Default, Deserialize)]
struct RawUsage {
    #[serde(default)]
    usage: i64,
}

#[derive(Debug, Default, Deserialize)]
struct RawNic {
    #[serde(default, deserialize_with = "nullable")]
    addresses: Vec<RawAddress>,
    #[serde(rename = "type", default)]
    kind: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawAddress {
    #[serde(default)]
    family: String,
    #[serde(default)]
    address: String,
    #[serde(default)]
    scope: String,
}

#[derive(Debug, Deserialize)]
struct RawSnapshot {
    name: String,
    #[serde(default)]
    created_at: String,
}

/// A non-negative count out of the daemon's signed one (`-1` is "unknown").
fn count(n: i64) -> u64 {
    u64::try_from(n).unwrap_or(0)
}

impl RawState {
    fn state(&self) -> State {
        let mut state = State {
            cpu_ns: count(self.cpu.usage),
            memory_bytes: count(self.memory.usage),
            processes: count(self.processes),
            disk_bytes: self.disk.get("root").map_or(0, |d| count(d.usage)),
            ..State::default()
        };
        // THE LOOPBACK IS NOT AN ADDRESS of the instance's, and a link-local
        // one is not one a person can reach it at.
        for nic in self.network.values().filter(|n| n.kind != "loopback") {
            for a in nic.addresses.iter().filter(|a| a.scope == "global") {
                match a.family.as_str() {
                    "inet" => state.ipv4.push(a.address.clone()),
                    "inet6" => state.ipv6.push(a.address.clone()),
                    _ => {}
                }
            }
        }
        state
    }
}

impl RawSnapshot {
    fn snapshot(self) -> Snapshot {
        // SOME ANSWERS NAME IT WITH ITS INSTANCE in front (`web/snap0`).
        let name = self
            .name
            .rsplit('/')
            .next()
            .unwrap_or(&self.name)
            .to_owned();
        Snapshot {
            name,
            created_at: self.created_at,
        }
    }
}

impl RawInstance {
    fn instance(self) -> Result<Instance, Refusal> {
        let kind =
            Kind::of_word(&self.kind).ok_or_else(|| garbled("an instance's type", &self.kind))?;
        let config = if self.expanded_config.is_empty() {
            &self.config
        } else {
            &self.expanded_config
        };
        Ok(Instance {
            limits: limits_of(config),
            // THE INSTANCE'S OWN: what a profile sets is not here.
            own_limits: limits_of(&self.config),
            image: config.get("image.description").cloned().unwrap_or_default(),
            name: self.name,
            kind,
            status: Status::of_code(self.status_code),
            project: self.project,
            description: self.description,
            profiles: self.profiles,
            created_at: self.created_at,
            state: self.state.as_ref().map(RawState::state),
            snapshots: self
                .snapshots
                .into_iter()
                .map(RawSnapshot::snapshot)
                .collect(),
        })
    }
}

/// The instances of a listing.
///
/// # Errors
///
/// [`Why::Failed`] for a shape this build does not read.
pub(crate) fn instances_of(value: &Value) -> Result<Vec<Instance>, Refusal> {
    read::<Vec<RawInstance>>(value, "the instances")?
        .into_iter()
        .map(RawInstance::instance)
        .collect()
}

/// One instance.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn instance_of(value: &Value) -> Result<Instance, Refusal> {
    read::<RawInstance>(value, "an instance")?.instance()
}

/// An instance's state.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn state_of(value: &Value) -> Result<State, Refusal> {
    Ok(read::<RawState>(value, "an instance's state")?.state())
}

/// An instance's snapshots.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn snapshots_of(value: &Value) -> Result<Vec<Snapshot>, Refusal> {
    Ok(read::<Vec<RawSnapshot>>(value, "the snapshots")?
        .into_iter()
        .map(RawSnapshot::snapshot)
        .collect())
}

#[derive(Debug, Deserialize)]
struct RawImage {
    fingerprint: String,
    #[serde(default, deserialize_with = "nullable")]
    aliases: Vec<RawAlias>,
    #[serde(default, deserialize_with = "nullable")]
    properties: BTreeMap<String, String>,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    size: i64,
    #[serde(default)]
    architecture: String,
}

#[derive(Debug, Deserialize)]
struct RawAlias {
    name: String,
}

impl RawImage {
    fn kind(&self) -> Result<Kind, Refusal> {
        Kind::of_word(&self.kind).ok_or_else(|| garbled("an image's type", &self.kind))
    }
}

/// The local images.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn images_of(value: &Value) -> Result<Vec<Image>, Refusal> {
    read::<Vec<RawImage>>(value, "the images")?
        .into_iter()
        .map(|raw| {
            Ok(Image {
                kind: raw.kind()?,
                aliases: raw.aliases.into_iter().map(|a| a.name).collect(),
                description: raw
                    .properties
                    .get("description")
                    .cloned()
                    .unwrap_or_default(),
                fingerprint: raw.fingerprint,
                size_bytes: count(raw.size),
            })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct RawNamed {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    managed: bool,
    #[serde(default)]
    driver: String,
}

/// The profiles.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn profiles_of(value: &Value) -> Result<Vec<Profile>, Refusal> {
    Ok(read::<Vec<RawNamed>>(value, "the profiles")?
        .into_iter()
        .map(|r| Profile {
            name: r.name,
            description: r.description,
        })
        .collect())
}

/// The networks.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn networks_of(value: &Value) -> Result<Vec<Network>, Refusal> {
    Ok(read::<Vec<RawNamed>>(value, "the networks")?
        .into_iter()
        .map(|r| Network {
            name: r.name,
            managed: r.managed,
            kind: r.kind,
        })
        .collect())
}

/// The storage pools.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn pools_of(value: &Value) -> Result<Vec<Pool>, Refusal> {
    Ok(read::<Vec<RawNamed>>(value, "the pools")?
        .into_iter()
        .map(|r| Pool {
            name: r.name,
            driver: r.driver,
        })
        .collect())
}

/// A pool's driver.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn driver_of(value: &Value) -> Result<String, Refusal> {
    Ok(read::<RawNamed>(value, "a pool")?.driver)
}

/// A pool's volumes.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn volumes_of(value: &Value) -> Result<Vec<Volume>, Refusal> {
    Ok(read::<Vec<RawNamed>>(value, "the volumes")?
        .into_iter()
        .map(|r| Volume {
            name: r.name,
            kind: r.kind,
        })
        .collect())
}

/// The projects.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn projects_of(value: &Value) -> Result<Vec<Project>, Refusal> {
    Ok(read::<Vec<RawNamed>>(value, "the projects")?
        .into_iter()
        .map(|r| Project {
            name: r.name,
            description: r.description,
        })
        .collect())
}

/// A profile's root disk: the device that is a disk at `/`.
#[must_use]
pub(crate) fn root_disk_of(profile: &Value) -> Option<serde_json::Map<String, Value>> {
    profile
        .get("devices")?
        .as_object()?
        .values()
        .filter_map(Value::as_object)
        .find(|d| {
            d.get("type").and_then(Value::as_str) == Some("disk")
                && d.get("path").and_then(Value::as_str) == Some("/")
        })
        .cloned()
}

/// A VM's root disk, ASKED to be no-copy-on-write where its pool is btrfs:
/// the profile's own device with `initial.btrfs.compression = none`. Pure.
///
/// # Why this and not `chattr +C` on a folder
///
/// The `+C` of a file is set while it is empty, from its folder's, or never
/// (a `chattr +C` on a file with data changes nothing, and says nothing).
/// Every volume of a btrfs pool is a SUBVOLUME, and a subvolume's root takes
/// no flag from the folder it is made in (nor from its parent subvolume): a
/// `+C` on `virtual-machines/` would reach no VM's disk. Both measured on a
/// `compress=zstd` btrfs. So only the daemon, which writes the disk, can
/// write it without copy-on-write, and its knob is the volume's
/// compression. Containers keep theirs.
///
/// # What is NOT proven
///
/// A VM made from an image on a btrfs pool is normally a SNAPSHOT of the
/// image's cached volume (`images/<fingerprint>`), and a snapshot keeps the
/// flags of the file it shares (measured): this override reaches that disk
/// only if the image's volume was written without copy-on-write, which is
/// the daemon's to decide and needs a VM — nested KVM, which the test VM
/// does not have — to see (`docs/PENDING.md`).
#[must_use]
pub(crate) fn nocow_root(
    root: &serde_json::Map<String, Value>,
    driver: &str,
) -> Option<serde_json::Map<String, Value>> {
    if driver != "btrfs" {
        return None;
    }
    let mut root = root.clone();
    root.insert(
        "initial.btrfs.compression".to_owned(),
        Value::String("none".to_owned()),
    );
    Some(root)
}

/// The limits a configuration says. Pure.
#[must_use]
pub(crate) fn limits_of(config: &BTreeMap<String, String>) -> Limits {
    Limits {
        cpu: config.get("limits.cpu").and_then(|v| cpus_of(v)),
        memory_bytes: config.get("limits.memory").and_then(|v| size_of(v)),
        processes: config
            .get("limits.processes")
            .and_then(|v| v.trim().parse().ok()),
    }
}

/// How many CPUs `limits.cpu` gives: a count (`2`) or a set (`0-1,4` is
/// three). Pure.
#[must_use]
pub(crate) fn cpus_of(value: &str) -> Option<u32> {
    let value = value.trim();
    if !value.contains(['-', ',']) {
        return value.parse().ok().filter(|n| *n > 0);
    }
    let mut total = 0u32;
    for part in value.split(',') {
        let (from, to) = part.split_once('-').unwrap_or((part, part));
        let (from, to): (u32, u32) = (from.trim().parse().ok()?, to.trim().parse().ok()?);
        total = total.checked_add(to.checked_sub(from)? + 1)?;
    }
    Some(total)
}

/// Bytes, out of the daemon's size (`512MiB`, `2GB`, `1048576`); `None` for
/// a percentage of the host or anything else. Pure.
#[must_use]
pub(crate) fn size_of(value: &str) -> Option<u64> {
    let value = value.trim();
    let at = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    let (number, suffix) = value.split_at(at);
    let number: u64 = number.parse().ok()?;
    let unit: u64 = match suffix.trim() {
        "" | "B" | "bytes" => 1,
        "kB" => 1000,
        "MB" => 1000u64.pow(2),
        "GB" => 1000u64.pow(3),
        "TB" => 1000u64.pow(4),
        "PB" => 1000u64.pow(5),
        "EB" => 1000u64.pow(6),
        "KiB" => 1 << 10,
        "MiB" => 1 << 20,
        "GiB" => 1 << 30,
        "TiB" => 1 << 40,
        "PiB" => 1 << 50,
        "EiB" => 1 << 60,
        _ => return None,
    };
    number.checked_mul(unit)
}

#[derive(Debug, Deserialize)]
struct RawRemote {
    #[serde(rename = "Addrs", default, deserialize_with = "nullable")]
    addrs: Vec<String>,
    #[serde(rename = "Addr", default)]
    addr: Option<String>,
    #[serde(rename = "Protocol", default)]
    protocol: String,
    #[serde(rename = "Public", default)]
    public: bool,
}

/// The remotes that serve images, out of `remote list --format json` (a map
/// by name, the Go struct's field names), by name.
///
/// `local` and any other daemon (`incus` protocol) are left out: this machine
/// is read through its socket, and another daemon's images are another
/// daemon's.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn remotes_of(bytes: &[u8]) -> Result<Vec<Remote>, Refusal> {
    let raw: BTreeMap<String, RawRemote> =
        serde_json::from_slice(bytes).map_err(|e| garbled("the remotes", &e))?;
    Ok(raw
        .into_iter()
        .filter_map(|(name, r)| {
            let protocol = Protocol::of_word(&r.protocol)?;
            let address = r.addrs.into_iter().next().or(r.addr)?;
            Some(Remote {
                name,
                address,
                protocol,
                public: r.public,
            })
        })
        .collect())
}

/// A remote's catalogue for this architecture, out of `image list
/// --format json`: one entry per alias and kind, by its SHORTEST alias
/// (`debian/13`, not `debian/13/default/amd64`), sorted. An image with no
/// alias cannot be asked for by one, and is left out.
///
/// # Errors
///
/// As [`instances_of`].
pub(crate) fn catalogue_of(bytes: &[u8], architecture: &str) -> Result<Vec<Offered>, Refusal> {
    let raw: Vec<RawImage> =
        serde_json::from_slice(bytes).map_err(|e| garbled("the catalogue", &e))?;
    let mut seen = BTreeMap::new();
    for image in raw.into_iter().filter(|i| i.architecture == architecture) {
        let Some(alias) = image
            .aliases
            .iter()
            .map(|a| a.name.as_str())
            .min_by(|a, b| a.len().cmp(&b.len()).then(a.cmp(b)))
            .map(str::to_owned)
        else {
            continue;
        };
        let kind = image.kind()?;
        seen.entry((alias.clone(), kind))
            .or_insert_with(|| Offered {
                alias,
                description: image
                    .properties
                    .get("description")
                    .cloned()
                    .unwrap_or_default(),
                kind,
                fingerprint: image.fingerprint.clone(),
                size_bytes: count(image.size),
            });
    }
    Ok(seen.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn metadata(envelope: &str) -> Value {
        serde_json::from_str::<Value>(envelope).unwrap()["metadata"].take()
    }

    #[test]
    fn the_instances_read_whole() {
        let all = instances_of(&metadata(fixtures::INSTANCES)).unwrap();
        assert_eq!(all.len(), 2);
        let web = &all[0];
        assert_eq!(web.name, "web");
        assert_eq!((web.kind, web.status), (Kind::Container, Status::Running));
        assert_eq!(web.profiles, ["default", "small"]);
        assert_eq!(web.description, "the web server");
        assert_eq!(web.image, "Debian trixie amd64 (20261004_05:24)");
        assert_eq!(
            web.limits,
            Limits {
                cpu: Some(2),
                memory_bytes: Some(512 << 20),
                processes: Some(500)
            },
            "the EXPANDED configuration: the memory comes from a profile"
        );
        assert_eq!(
            web.own_limits,
            Limits {
                cpu: Some(2),
                memory_bytes: None,
                processes: None
            },
            "its OWN configuration: the profile's memory is not its own"
        );
        let state = web.state.as_ref().unwrap();
        assert_eq!(
            state.ipv4,
            ["10.218.42.17"],
            "the loopback is not an address"
        );
        assert_eq!(
            state.ipv6,
            ["fd42:4242:4242:4242:1266:6aff:fe12:3456"],
            "nor a link-local one"
        );
        assert_eq!(state.cpu_ns, 1_234_567_890);
        assert_eq!(state.memory_bytes, 52_428_800);
        assert_eq!((state.processes, state.disk_bytes), (12, 7_340_032));
        assert_eq!(
            web.snapshots
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>(),
            ["before-upgrade", "after-upgrade"],
            "a name with its instance in front is the snapshot's own"
        );
        let vm = &all[1];
        assert_eq!((vm.kind, vm.status), (Kind::Vm, Status::Stopped));
        let state = vm.state.as_ref().unwrap();
        assert_eq!(
            (state.processes, state.ipv4.len(), state.disk_bytes),
            (0, 0, 0),
            "a stopped VM's nulls and -1 read as nothing"
        );
        assert!(vm.snapshots.is_empty());
        assert_eq!(vm.limits.memory_bytes, Some(2 << 30));
    }

    #[test]
    fn a_type_this_build_does_not_know_refuses_the_listing() {
        let mut all = metadata(fixtures::INSTANCES);
        all[1]["type"] = Value::String("microvm".to_owned());
        assert_eq!(instances_of(&all), Err(Refusal::of(Why::Failed)));
        assert_eq!(
            instances_of(&Value::String("no".to_owned())),
            Err(Refusal::of(Why::Failed))
        );
    }

    #[test]
    fn one_instance_its_state_and_its_snapshots() {
        let web = instance_of(&metadata(fixtures::INSTANCE_PLAIN)).unwrap();
        assert_eq!(web.state, None, "asked without recursion, no state");
        assert_eq!(web.limits.cpu, Some(2));
        let state = state_of(&metadata(fixtures::STATE)).unwrap();
        assert_eq!(state.ipv4, ["10.218.42.17"]);
        assert_eq!(state.processes, 9);
        let snaps = snapshots_of(&metadata(fixtures::SNAPSHOTS)).unwrap();
        assert_eq!(snaps[0].name, "before-upgrade");
        assert_eq!(snaps[0].created_at, "2026-10-05T12:30:00.5Z");
    }

    #[test]
    fn the_other_lists_read_whole() {
        let images = images_of(&metadata(fixtures::IMAGES)).unwrap();
        assert_eq!(images[0].aliases, ["debian-13"]);
        assert_eq!(
            images[0].description,
            "Debian trixie amd64 (20261004_05:24)"
        );
        assert_eq!(
            (images[0].kind, images[0].size_bytes),
            (Kind::Container, 98_765_432)
        );
        assert!(images[1].aliases.is_empty(), "null aliases are none");
        assert_eq!(images[1].kind, Kind::Vm);
        let profiles = profiles_of(&metadata(fixtures::PROFILES)).unwrap();
        assert_eq!(profiles[0].name, "default");
        assert_eq!(profiles[0].description, "Default Incus profile");
        let networks = networks_of(&metadata(fixtures::NETWORKS)).unwrap();
        assert!(networks[0].managed && networks[0].kind == "bridge");
        assert!(!networks[1].managed);
        let pools = pools_of(&metadata(fixtures::POOLS)).unwrap();
        assert_eq!(pools[0].driver, "btrfs");
        assert_eq!(
            driver_of(&metadata(fixtures::POOL_DEFAULT)).unwrap(),
            "btrfs"
        );
        let volumes = volumes_of(&metadata(fixtures::VOLUMES)).unwrap();
        assert_eq!(
            volumes.iter().map(|v| v.kind.as_str()).collect::<Vec<_>>(),
            ["container", "virtual-machine", "custom"]
        );
        let projects = projects_of(&metadata(fixtures::PROJECTS)).unwrap();
        assert_eq!(projects[0].name, "default");
    }

    #[test]
    fn a_vm_on_btrfs_gets_its_disk_without_copy_on_write() {
        let profile = metadata(fixtures::PROFILE_DEFAULT);
        let root = root_disk_of(&profile).unwrap();
        assert_eq!(root.get("pool").and_then(Value::as_str), Some("default"));
        let nocow = nocow_root(&root, "btrfs").unwrap();
        assert_eq!(
            nocow
                .get("initial.btrfs.compression")
                .and_then(Value::as_str),
            Some("none")
        );
        assert_eq!(
            nocow.get("path").and_then(Value::as_str),
            Some("/"),
            "the rest is kept"
        );
        assert_eq!(nocow_root(&root, "dir"), None, "only btrfs has the knob");
        assert_eq!(root_disk_of(&metadata(fixtures::PROJECTS)), None);
    }

    #[test]
    fn limits_read_the_daemon_s_numbers() {
        assert_eq!(cpus_of("2"), Some(2));
        assert_eq!(cpus_of("0-1"), Some(2));
        assert_eq!(cpus_of("0-1,4"), Some(3));
        assert_eq!(cpus_of("1,3,5"), Some(3));
        assert_eq!(cpus_of("3-1"), None);
        assert_eq!(cpus_of("0"), None);
        assert_eq!(cpus_of("x"), None);
        assert_eq!(size_of("512MiB"), Some(512 << 20));
        assert_eq!(size_of("2GB"), Some(2_000_000_000));
        assert_eq!(size_of("1048576"), Some(1 << 20));
        assert_eq!(size_of("4096B"), Some(4096));
        assert_eq!(
            size_of("50%"),
            None,
            "a share of the host is no number of bytes"
        );
        assert_eq!(
            size_of("9999999999EiB"),
            None,
            "and an overflow is no number either"
        );
    }

    #[test]
    fn the_remotes_are_the_image_servers() {
        let remotes = remotes_of(fixtures::REMOTES.as_bytes()).unwrap();
        assert_eq!(
            remotes,
            [
                Remote {
                    name: "docker".to_owned(),
                    address: "https://docker.io".to_owned(),
                    protocol: Protocol::Oci,
                    public: true,
                },
                Remote {
                    name: "images".to_owned(),
                    address: "https://images.linuxcontainers.org".to_owned(),
                    protocol: Protocol::SimpleStreams,
                    public: true,
                },
            ],
            "`local` is this machine, read through its socket"
        );
        // AN OLDER CLIENT wrote one `Addr`.
        let older = remotes_of(br#"{"images":{"Addr":"https://images.linuxcontainers.org","Protocol":"simplestreams","Public":true}}"#)
            .unwrap();
        assert_eq!(older[0].address, "https://images.linuxcontainers.org");
        assert_eq!(remotes_of(b"not json"), Err(Refusal::of(Why::Failed)));
    }

    #[test]
    fn the_catalogue_is_one_entry_per_alias_and_kind_of_this_architecture() {
        let offered = catalogue_of(fixtures::CATALOGUE.as_bytes(), "x86_64").unwrap();
        assert_eq!(
            offered
                .iter()
                .map(|o| (o.alias.as_str(), o.kind))
                .collect::<Vec<_>>(),
            [
                ("debian/13", Kind::Container),
                ("debian/13", Kind::Vm),
                ("debian/13/cloud", Kind::Container),
            ],
            "the shortest alias, both kinds, and no arm64 image"
        );
        assert_eq!(offered[1].size_bytes, 398_765_432);
        assert_eq!(
            offered[0].description,
            "Debian trixie amd64 (20261004_05:24)"
        );
    }
}
