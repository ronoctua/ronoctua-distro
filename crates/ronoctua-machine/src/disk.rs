//! The files: the machine's record, the lock snapshots, a tree copied whole.
//!
//! Every file is written whole and renamed into place, so a reader — the
//! update's view, the person's publishing of the record — never sees half a
//! file, and a power cut leaves the old one or the new one.

use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use ronoctua_person::Stamp;
use ronoctua_person::state::MachineState;

use crate::{Places, Refusal, Why, say};

/// Now, as the files write it.
pub(crate) fn now() -> Stamp {
    let s = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    Stamp::from_unix_s(s)
}

/// A stamp as a file name's part: `2026-10-05T12:00:00Z` → `20261005T120000Z`.
pub(crate) fn compact(stamp: &Stamp) -> String {
    stamp
        .as_str()
        .chars()
        .filter(|c| *c != '-' && *c != ':')
        .collect()
}

/// The machine's record, its last apply whole from `last-apply.json` when
/// that is the apply `state.json` holds ([`MachineState::completed`]); the
/// default record when there is none yet.
pub(crate) fn read_state(places: &Places) -> Result<MachineState, Refusal> {
    let file = places.state_file();
    match std::fs::read(&file) {
        Ok(bytes) => ronoctua_person::parse::<MachineState>(&bytes)
            .map(|state| state.completed(&file))
            .map_err(|r| {
                say(3, &format!("{} does not read: {r}", file.display()));
                Refusal::of(Why::Unreadable)
            }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(MachineState::default()),
        Err(e) => {
            say(3, &format!("{}: {e}", file.display()));
            Err(Refusal::of(Why::Unreadable))
        }
    }
}

/// Writes the machine's record — `0644`: the person's half publishes a copy
/// of it, and the update's view reads it.
///
/// `state.json` gets only what the first published release reads, and the
/// last apply whole goes to `last-apply.json` beside it while `state.json`
/// cannot say it ([`MachineState::split`]): a boot trial's way back can land
/// on a system whose bless and apply would refuse `staged`. The file beside
/// is written FIRST and taken away LAST, so a reader between the two sees it
/// with the `state.json` of another apply, which it does not believe —
/// never `state.json`'s new apply told short of what it was.
///
/// A file beside that this does not read is left: a newer system wrote it,
/// and when it is that system's apply that `state.json` still holds — this
/// one blessed or held without applying — the newer system, booted again,
/// reads it whole. One that reads is of another apply (this one would have
/// joined it), and goes.
pub(crate) fn write_state(places: &Places, state: &MachineState) -> Result<(), Refusal> {
    std::fs::create_dir_all(&places.state).map_err(|e| failed(&places.state, &e))?;
    let file = places.state_file();
    let beside = file.with_file_name(ronoctua_person::names::LAST_APPLY);
    let (told, whole) = state.split();
    if let Some(whole) = &whole {
        write_atomically(&beside, ronoctua_person::to_json(whole).as_bytes(), 0o644)
            .map_err(|e| failed(&beside, &e))?;
    }
    write_atomically(&file, ronoctua_person::to_json(&told).as_bytes(), 0o644)
        .map_err(|e| failed(&file, &e))?;
    let ours = std::fs::read(&beside)
        .is_ok_and(|b| ronoctua_person::parse::<ronoctua_person::state::Whole>(&b).is_ok());
    if whole.is_none() && ours {
        std::fs::remove_file(&beside).map_err(|e| failed(&beside, &e))?;
    }
    Ok(())
}

/// `bytes` into `path`, by a sibling written whole, flushed and renamed.
pub(crate) fn write_atomically(path: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("not a file's path"))?
        .to_string_lossy()
        .into_owned();
    let draft = path.with_file_name(format!(".{name}.{}.new", std::process::id()));
    let _ = std::fs::remove_file(&draft);
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&draft)?;
        file.write_all(bytes)?;
        file.set_permissions(std::fs::Permissions::from_mode(mode))?;
        file.sync_all()?;
        std::fs::rename(&draft, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

/// A program or a file that failed, said; the refusal is [`Why::Failed`].
pub(crate) fn failed(path: &Path, e: &std::io::Error) -> Refusal {
    say(3, &format!("{}: {e}", path.display()));
    Refusal::of(Why::Failed)
}

/// The bytes of `path`, `None` when it is not there.
pub(crate) fn read_if_there(path: &Path) -> Result<Option<Vec<u8>>, Refusal> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => {
            say(3, &format!("{}: {e}", path.display()));
            Err(Refusal::of(Why::Unreadable))
        }
    }
}

/// The revision a `flake.lock` names for `input` (`nodes.<input>.locked.rev`).
#[must_use]
pub(crate) fn locked_rev(lock: &[u8], input: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(lock).ok()?;
    let nodes = value.get("nodes")?;
    let root = value.get("root")?.as_str()?;
    let key = nodes.get(root)?.get("inputs")?.get(input)?.as_str()?;
    nodes
        .get(key)?
        .get("locked")?
        .get("rev")?
        .as_str()
        .map(str::to_owned)
}

/// What a `flake.lock` locks the `nixpkgs` its system is built with at: the
/// root's own [`crate::NIXPKGS_INPUT`] (a machine flake that rolls it), else
/// the distro's ([`crate::DISTRO_INPUT`]'s own input of that name) — its
/// `locked.rev`, else its `locked.narHash`, else its whole `locked` (a `path:`
/// or a tarball locks no revision); `None` when the lock does not read or
/// names no such input.
#[must_use]
pub(crate) fn locked_nixpkgs(lock: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(lock).ok()?;
    let nodes = value.get("nodes")?;
    let root = value.get("root")?.as_str()?;
    let key = input_of(nodes, root, root, crate::NIXPKGS_INPUT).or_else(|| {
        let distro = input_of(nodes, root, root, crate::DISTRO_INPUT)?;
        input_of(nodes, root, &distro, crate::NIXPKGS_INPUT)
    })?;
    let locked = nodes.get(&key)?.get("locked")?;
    for field in ["rev", "narHash"] {
        if let Some(text) = locked.get(field).and_then(serde_json::Value::as_str) {
            return Some(format!("{field} {text}"));
        }
    }
    locked.is_object().then(|| locked.to_string())
}

/// The node the input `name` of the node `node` is, in a lock whose root is
/// `root`: a node's key, or a `follows` path from the root (a list of input
/// names), followed [`FOLLOWS_DEPTH`] steps deep at most — a path that goes
/// round in a circle names nothing.
fn input_of(nodes: &serde_json::Value, root: &str, node: &str, name: &str) -> Option<String> {
    input_within(nodes, root, node, name, FOLLOWS_DEPTH)
}

/// How deep a `follows` path is followed.
const FOLLOWS_DEPTH: usize = 8;

fn input_within(
    nodes: &serde_json::Value,
    root: &str,
    node: &str,
    name: &str,
    depth: usize,
) -> Option<String> {
    let depth = depth.checked_sub(1)?;
    match nodes.get(node)?.get("inputs")?.get(name)? {
        serde_json::Value::String(key) => Some(key.clone()),
        serde_json::Value::Array(path) => {
            let mut here = root.to_owned();
            for step in path {
                here = input_within(nodes, root, &here, step.as_str()?, depth)?;
            }
            Some(here)
        }
        _ => None,
    }
}

/// The lock snapshot called `name`, under the state folder.
pub(crate) fn snapshot(places: &Places, name: &str) -> PathBuf {
    places.state.join(name)
}

/// Writes the lock snapshot called `name` (the state folder made first).
pub(crate) fn keep_snapshot(places: &Places, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::create_dir_all(&places.state)?;
    write_atomically(&snapshot(places, name), bytes, 0o644)
}

/// Copies the tree at `from` into `into` (which must not exist): folders,
/// files with their mode, links as links. Anything else is refused — a
/// machine directory has no device or socket in it.
pub(crate) fn copy_tree(from: &Path, into: &Path, skip: &[&str]) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    std::fs::create_dir(into)?;
    std::fs::set_permissions(
        into,
        std::fs::Permissions::from_mode(meta.permissions().mode()),
    )?;
    let mut names: Vec<_> = std::fs::read_dir(from)?.collect::<Result<_, _>>()?;
    names.sort_by_key(std::fs::DirEntry::file_name);
    for entry in names {
        let name = entry.file_name();
        if skip.iter().any(|s| name == *s) {
            continue;
        }
        copy_one(&entry.path(), &into.join(&name))?;
    }
    Ok(())
}

/// Copies one entry, whatever it is.
pub(crate) fn copy_one(from: &Path, to: &Path) -> std::io::Result<()> {
    let meta = std::fs::symlink_metadata(from)?;
    let kind = meta.file_type();
    if kind.is_symlink() {
        std::os::unix::fs::symlink(std::fs::read_link(from)?, to)
    } else if kind.is_dir() {
        copy_tree(from, to, &[])
    } else if kind.is_file() {
        std::fs::copy(from, to)?;
        std::fs::set_permissions(
            to,
            std::fs::Permissions::from_mode(meta.permissions().mode()),
        )
    } else {
        Err(std::io::Error::other(format!(
            "{} is neither a file, a folder nor a link",
            from.display()
        )))
    }
}

/// Removes `path`, whatever it is; nothing to remove is fine.
pub(crate) fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The entries of a folder, by name, sorted.
pub(crate) fn names(dir: &Path) -> std::io::Result<Vec<std::ffi::OsString>> {
    let mut names: Vec<_> = std::fs::read_dir(dir)?
        .map(|e| e.map(|e| e.file_name()))
        .collect::<Result<_, _>>()?;
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_names_the_distro_by_its_root_input() {
        let lock = br#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua_2"}},
            "ronoctua_2":{"locked":{"rev":"613862cabc","type":"github"}}},"root":"root","version":7}"#;
        assert_eq!(locked_rev(lock, "ronoctua").as_deref(), Some("613862cabc"));
        assert_eq!(locked_rev(lock, "nixpkgs"), None);
        assert_eq!(locked_rev(b"{", "ronoctua"), None);
    }

    /// FAILS IF: the `nixpkgs` a system is built with is not found where a
    /// machine flake locks it — its own input, the distro's following it, or
    /// (a flake that does not roll it) the distro's own —, or a lock that
    /// names no revision (a tarball, a `path:`) reads as none at all.
    #[test]
    fn a_lock_names_the_nixpkgs_the_system_is_built_with() {
        // ROLLED by the machine, the distro following it.
        let rolled = br#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua","nixpkgs":"nixpkgs"}},
            "ronoctua":{"inputs":{"nixpkgs":["nixpkgs"]},"locked":{"rev":"aaaa"}},
            "nixpkgs":{"locked":{"rev":"1111","narHash":"sha256-x"}}},"root":"root","version":7}"#;
        assert_eq!(locked_nixpkgs(rolled).as_deref(), Some("rev 1111"));
        // THE DISTRO'S, on a machine flake that does not roll it.
        let pinned = br#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua"}},
            "ronoctua":{"inputs":{"nixpkgs":"nixpkgs_2"},"locked":{"rev":"aaaa"}},
            "nixpkgs_2":{"locked":{"rev":"2222"}}},"root":"root","version":7}"#;
        assert_eq!(locked_nixpkgs(pinned).as_deref(), Some("rev 2222"));
        // A TARBALL: its hash, then.
        let tarball = br#"{"nodes":{"root":{"inputs":{"nixpkgs":"n"}},
            "n":{"locked":{"narHash":"sha256-y","type":"tarball","url":"https://x"}}},"root":"root","version":7}"#;
        assert_eq!(locked_nixpkgs(tarball).as_deref(), Some("narHash sha256-y"));
        // NONE NAMED, or no lock.
        let none = br#"{"nodes":{"root":{"inputs":{"ronoctua":"ronoctua"}},
            "ronoctua":{"locked":{"rev":"aaaa"}}},"root":"root","version":7}"#;
        assert_eq!(locked_nixpkgs(none), None);
        assert_eq!(locked_nixpkgs(b"{"), None);
        // A FOLLOWS that goes round in a circle names nothing.
        let circle =
            br#"{"nodes":{"root":{"inputs":{"nixpkgs":["nixpkgs"]}}},"root":"root","version":7}"#;
        assert_eq!(locked_nixpkgs(circle), None);
    }

    #[test]
    fn a_stamp_becomes_a_name() {
        let s = Stamp::parse("2026-10-05T12:03:04Z").unwrap();
        assert_eq!(compact(&s), "20261005T120304Z");
    }

    #[test]
    fn a_tree_is_copied_with_its_modes_and_links_and_a_write_is_whole() {
        let root = crate::tests::Scratch::new("copy");
        let from = root.path("from");
        std::fs::create_dir_all(from.join("person/software")).unwrap();
        std::fs::write(from.join("host.nix"), b"{ }").unwrap();
        std::fs::set_permissions(
            from.join("host.nix"),
            std::fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        std::fs::write(from.join("person/software/declared.json"), b"[]").unwrap();
        std::os::unix::fs::symlink("host.nix", from.join("link")).unwrap();
        std::fs::write(from.join("skipped"), b"x").unwrap();
        let into = root.path("into");
        copy_tree(&from, &into, &["skipped"]).unwrap();
        assert_eq!(std::fs::read(into.join("host.nix")).unwrap(), b"{ }");
        let mode = std::fs::metadata(into.join("host.nix"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            std::fs::read_link(into.join("link")).unwrap(),
            Path::new("host.nix")
        );
        assert!(into.join("person/software/declared.json").is_file());
        assert!(!into.join("skipped").exists());
        assert!(copy_tree(&from, &into, &[]).is_err(), "never over a tree");

        let file = root.path("state.json");
        write_atomically(&file, b"one", 0o644).unwrap();
        write_atomically(&file, b"two", 0o644).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"two");
        let left: Vec<_> = names(&root.path("")).unwrap();
        assert!(
            left.iter().all(|n| !n.to_string_lossy().ends_with(".new")),
            "{left:?}"
        );
    }
}
