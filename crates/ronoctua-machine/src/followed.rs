//! The GitHub branches the machine's flake follows, and the revision its
//! lock holds each at — what the update window asks GitHub about
//! (`ronoctua_software::heads`) to say whether an update is there, before
//! anything is built.
//!
//! # Read from the lock, never spelled here
//!
//! Which repository and which branch an input follows is its node's
//! `original` — what `flake.nix` wrote (`github:NixOS/nixpkgs/nixos-unstable`,
//! `github:ronoctua/ronoctua-distro/release`) — and the revision it is held
//! at, its `locked.rev`. Only the root's own two inputs are read,
//! [`DISTRO_INPUT`] and [`NIXPKGS_INPUT`]: the two the update moves. A root
//! input that `follows` another is that other's, and moves with it.
//!
//! # What is left out, what is not asked, and what is refused
//!
//! Left out is only what the update does not move: a local checkout of the
//! distro (`git+file:`, `path:` outside the store — there the update moves
//! nixpkgs alone, `crate::aim`) and an input pinned to a revision. Any other
//! input whose `original` is not a `github` one naming a `ref` is moved by
//! the update and follows no branch to ask about — the store tree an
//! installer aims the machine at, a repository's default branch, a registry
//! name, a tarball — and is [`Followed::Unasked`]: never said to be where the
//! lock holds it. One whose `original` IS a GitHub branch and whose node is
//! not what a lock of one is — a name GitHub would not give, a `locked` of
//! another kind or another repository, a revision that is not a commit's —
//! is [`Followed::Refused`]: never guessed.

use std::path::Path;

use serde_json::Value;

use crate::{DISTRO_INPUT, NIXPKGS_INPUT};

/// An input of the machine flake that may follow a branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Input {
    /// The distro ([`DISTRO_INPUT`]): its `release` branch.
    Distro,
    /// `nixpkgs` ([`NIXPKGS_INPUT`]): `nixos-unstable`, which the machine
    /// rolls.
    Nixpkgs,
}

impl Input {
    /// Every one, in the order they are read.
    pub const ALL: [Self; 2] = [Self::Distro, Self::Nixpkgs];

    /// Its name among the machine flake's inputs.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Distro => DISTRO_INPUT,
            Self::Nixpkgs => NIXPKGS_INPUT,
        }
    }
}

/// A GitHub branch an input follows, and where the lock holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branch {
    /// The input that follows it.
    pub input: Input,
    /// The repository's owner, as `original` names it.
    pub owner: String,
    /// The repository, as `original` names it.
    pub repo: String,
    /// The branch (`original.ref`).
    pub branch: String,
    /// The commit the lock holds it at (`locked.rev`): 40 lower-case hex
    /// digits.
    pub locked: String,
}

/// What the lock says of an input the update moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Followed {
    /// It follows this branch.
    Branch(Branch),
    /// Its `original` names a GitHub branch, and the rest of its node is not
    /// what a lock of one is.
    Refused(Input),
    /// The update moves it, and it follows no GitHub branch to ask about:
    /// whether it would move cannot be learnt before the update runs.
    Unasked(Input),
}

impl Followed {
    /// The input it is about.
    #[must_use]
    pub const fn input(&self) -> Input {
        match self {
            Self::Branch(branch) => branch.input,
            Self::Refused(input) | Self::Unasked(input) => *input,
        }
    }
}

/// Why the lock was not read, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unread {
    /// There is no lock: a machine with no flake of its own (the VM's, a
    /// test's, a photograph's room that planted none).
    NoLock,
    /// The lock is there and does not read: the file could not be read, or
    /// it is not a lock's JSON.
    Unreadable,
}

impl Unread {
    /// Every one.
    pub const ALL: [Self; 2] = [Self::NoLock, Self::Unreadable];

    /// Its kebab-case name, the suffix of the key a door says it with.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoLock => "no-lock",
            Self::Unreadable => "unreadable",
        }
    }
}

/// What the lock at `lock` (the machine directory's [`crate::LOCK`]) says
/// of the inputs the update moves, read now.
///
/// # Errors
///
/// [`Unread::NoLock`] when there is no file, [`Unread::Unreadable`] when it
/// cannot be read or is not a lock.
pub fn read(lock: &Path) -> Result<Vec<Followed>, Unread> {
    match std::fs::read(lock) {
        Ok(bytes) => of_lock(&bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Unread::NoLock),
        Err(_) => Err(Unread::Unreadable),
    }
}

/// What a lock's bytes say of the inputs the update moves, in
/// [`Input::ALL`]'s order. Pure.
///
/// # Errors
///
/// [`Unread::Unreadable`] for bytes that are not a lock: not JSON, or no
/// `nodes` with the `root` it names.
pub fn of_lock(lock: &[u8]) -> Result<Vec<Followed>, Unread> {
    let value: Value = serde_json::from_slice(lock).map_err(|_| Unread::Unreadable)?;
    let nodes = value
        .get("nodes")
        .filter(|n| n.is_object())
        .ok_or(Unread::Unreadable)?;
    let root = value
        .get("root")
        .and_then(Value::as_str)
        .and_then(|key| nodes.get(key))
        .ok_or(Unread::Unreadable)?;
    Ok(Input::ALL
        .into_iter()
        .filter_map(|input| {
            // A KEY OF ITS OWN: a `follows` path is another input's node.
            let key = root.get("inputs")?.get(input.name())?.as_str()?;
            let Some(node) = nodes.get(key) else {
                return Some(Followed::Refused(input));
            };
            followed(input, node)
        })
        .collect())
}

/// What one input's node says: `None` when the update does not move it.
fn followed(input: Input, node: &Value) -> Option<Followed> {
    // A LOCAL CHECKOUT OF THE DISTRO: the update moves nixpkgs alone there,
    // and what is committed in it comes in only through the sudo door.
    if input == Input::Distro && crate::aim::is_a_checkout(node) {
        return None;
    }
    let refused = Some(Followed::Refused(input));
    let unasked = Some(Followed::Unasked(input));
    let Some(original) = node.get("original").filter(|o| o.is_object()) else {
        return refused;
    };
    match text(original, "type") {
        Some(Some("github")) => {}
        Some(Some(_)) => return unasked,
        _ => return refused,
    }
    // PINNED to a revision: the update leaves it where it is.
    if original.get("rev").is_some() {
        return None;
    }
    let branch = match text(original, "ref") {
        // ON THE DEFAULT BRANCH: moved, and no branch named to ask about.
        None => return unasked,
        Some(Some(branch)) if is_branch(branch) => branch,
        Some(_) => return refused,
    };
    let (Some(Some(owner)), Some(Some(repo))) = (text(original, "owner"), text(original, "repo"))
    else {
        return refused;
    };
    let Some(locked) = node.get("locked") else {
        return refused;
    };
    // THE SAME REPOSITORY, as GitHub compares names: without case.
    let same = |field: &str, name: &str| {
        text(locked, field)
            .flatten()
            .is_some_and(|l| l.eq_ignore_ascii_case(name))
    };
    let rev = text(locked, "rev").flatten().filter(|r| is_commit(r));
    match rev {
        Some(rev)
            if is_name(owner)
                && is_name(repo)
                && text(locked, "type") == Some(Some("github"))
                && same("owner", owner)
                && same("repo", repo) =>
        {
            Some(Followed::Branch(Branch {
                input,
                owner: owner.to_owned(),
                repo: repo.to_owned(),
                branch: branch.to_owned(),
                locked: rev.to_owned(),
            }))
        }
        _ => refused,
    }
}

/// A field of a source: `None` when it is absent, `Some(None)` when it is
/// not text.
fn text<'a>(source: &'a Value, field: &str) -> Option<Option<&'a str>> {
    source.get(field).map(Value::as_str)
}

/// Whether `name` is an owner's or a repository's name as GitHub gives
/// them: letters, digits, `-`, `_` and `.`, at most 100, never `.` or `..`.
/// Pure.
#[must_use]
pub fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.len() <= 100
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// Whether `branch` is a branch's name that goes into an address as it is:
/// `/`-separated names of letters, digits, `-`, `_` and `.`, none empty,
/// `.` or `..`, at most 255 in all. Pure.
#[must_use]
pub fn is_branch(branch: &str) -> bool {
    branch.len() <= 255
        && branch.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        })
}

/// Whether `rev` is a commit's full name: 40 lower-case hex digits. Pure.
#[must_use]
pub fn is_commit(rev: &str) -> bool {
    rev.len() == 40
        && rev
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NIXPKGS_REV: &str = "f9a75f41be3a74eab55d3a7679d34ad6cc0da977";
    const DISTRO_REV: &str = "02c970a1bba56b8af8c5ff51349e84137aaadba2";

    /// A lock whose root has `nixpkgs` and `ronoctua`, their nodes as given.
    fn lock(nixpkgs: &str, ronoctua: &str) -> Vec<u8> {
        format!(
            r#"{{"nodes":{{"root":{{"inputs":{{"nixpkgs":"nixpkgs","ronoctua":"ronoctua"}}}},
            "nixpkgs":{nixpkgs},"ronoctua":{ronoctua}}},"root":"root","version":7}}"#
        )
        .into_bytes()
    }

    fn github(owner: &str, repo: &str, branch: &str, rev: &str) -> String {
        format!(
            r#"{{"original":{{"owner":"{owner}","ref":"{branch}","repo":"{repo}","type":"github"}},
            "locked":{{"lastModified":1,"narHash":"sha256-x","owner":"{owner}","repo":"{repo}",
            "rev":"{rev}","type":"github"}}}}"#
        )
    }

    fn branch(input: Input, owner: &str, repo: &str, name: &str, rev: &str) -> Followed {
        Followed::Branch(Branch {
            input,
            owner: owner.to_owned(),
            repo: repo.to_owned(),
            branch: name.to_owned(),
            locked: rev.to_owned(),
        })
    }

    /// FAILS IF: the branches of a machine flake as the template writes it
    /// are not both read — owner, repository and branch from `original`, the
    /// revision from `locked` — or are read in another order.
    #[test]
    fn the_templates_lock_follows_two_branches() {
        let bytes = lock(
            &github("NixOS", "nixpkgs", "nixos-unstable", NIXPKGS_REV),
            &github("ronoctua", "ronoctua-distro", "release", DISTRO_REV),
        );
        assert_eq!(
            of_lock(&bytes),
            Ok(vec![
                branch(
                    Input::Distro,
                    "ronoctua",
                    "ronoctua-distro",
                    "release",
                    DISTRO_REV
                ),
                branch(
                    Input::Nixpkgs,
                    "NixOS",
                    "nixpkgs",
                    "nixos-unstable",
                    NIXPKGS_REV
                ),
            ])
        );
    }

    /// FAILS IF: what the update does not move — a local checkout of the
    /// distro, a pinned revision, a `follows` of another input — is asked
    /// about, or what it moves and follows no GitHub branch — the
    /// installer's store tree, the default branch, an indirect input, a
    /// tarball, a nixpkgs on this machine's disk — is left out, where it
    /// would let every other input say "up to date".
    #[test]
    fn what_the_update_leaves_is_left_out_and_what_it_moves_unasked_is_said() {
        let nixpkgs = github("NixOS", "nixpkgs", "nixos-unstable", NIXPKGS_REV);
        let nixpkgs_branch = branch(
            Input::Nixpkgs,
            "NixOS",
            "nixpkgs",
            "nixos-unstable",
            NIXPKGS_REV,
        );
        for ronoctua in [
            // THE DEVELOPMENT MACHINE: a checkout of the distro, by its URL
            // or by its path — the update moves nixpkgs alone there.
            format!(
                r#"{{"original":{{"type":"git","url":"file:///home/x/ronoctua-distro"}},
                "locked":{{"rev":"{DISTRO_REV}","type":"git","url":"file:///home/x/ronoctua-distro"}}}}"#
            ),
            r#"{"original":{"path":"/home/x/ronoctua-distro","type":"path"},
                "locked":{"narHash":"sha256-y","path":"/home/x/ronoctua-distro","type":"path"}}"#
                .to_owned(),
            // PINNED to a revision: the update leaves it there.
            format!(
                r#"{{"original":{{"owner":"o","repo":"r","rev":"{DISTRO_REV}","type":"github"}},
                "locked":{{"owner":"o","repo":"r","rev":"{DISTRO_REV}","type":"github"}}}}"#
            ),
        ] {
            assert_eq!(
                of_lock(&lock(&nixpkgs, &ronoctua)),
                Ok(vec![nixpkgs_branch.clone()]),
                "{ronoctua}"
            );
        }
        for ronoctua in [
            // AIMED by the installer at its own tree, which the first update
            // points back at the release.
            r#"{"original":{"path":"/nix/store/abc-source","type":"path"},
                "locked":{"narHash":"sha256-y","path":"/nix/store/abc-source","type":"path"}}"#
                .to_owned(),
            // ON THE DEFAULT BRANCH, through the registry, from a tarball.
            format!(
                r#"{{"original":{{"owner":"o","repo":"r","type":"github"}},
                "locked":{{"owner":"o","repo":"r","rev":"{DISTRO_REV}","type":"github"}}}}"#
            ),
            r#"{"original":{"id":"ronoctua","type":"indirect"},"locked":{"type":"path"}}"#
                .to_owned(),
            r#"{"original":{"type":"tarball","url":"https://example.org/x.tar.gz"},
                "locked":{"narHash":"sha256-z","type":"tarball","url":"https://example.org/x.tar.gz"}}"#
                .to_owned(),
        ] {
            assert_eq!(
                of_lock(&lock(&nixpkgs, &ronoctua)),
                Ok(vec![Followed::Unasked(Input::Distro), nixpkgs_branch.clone()]),
                "{ronoctua}"
            );
        }
        // ONLY THE DISTRO'S CHECKOUT is left out: a nixpkgs on this
        // machine's disk is moved by the update all the same.
        let local = r#"{"original":{"path":"/home/x/nixpkgs","type":"path"},
            "locked":{"narHash":"sha256-w","path":"/home/x/nixpkgs","type":"path"}}"#;
        assert_eq!(
            of_lock(&lock(
                local,
                &github("ronoctua", "ronoctua-distro", "release", DISTRO_REV)
            ))
            .map(|all| all.into_iter().nth(1)),
            Ok(Some(Followed::Unasked(Input::Nixpkgs)))
        );
        // A `follows` at the root is the other input's, not one of its own.
        let follows = format!(
            r#"{{"nodes":{{"root":{{"inputs":{{"nixpkgs":["ronoctua","nixpkgs"],"ronoctua":"ronoctua"}}}},
            "ronoctua":{}}},"root":"root","version":7}}"#,
            github("ronoctua", "ronoctua-distro", "release", DISTRO_REV)
        );
        assert_eq!(
            of_lock(follows.as_bytes()).map(|all| all.iter().map(Followed::input).collect()),
            Ok(vec![Input::Distro])
        );
        // NO INPUTS at all: nothing followed, and nothing refused.
        let bare = br#"{"nodes":{"root":{}},"root":"root","version":7}"#;
        assert_eq!(of_lock(bare), Ok(Vec::new()));
    }

    /// FAILS IF: a node that says it follows a GitHub branch and is not what
    /// a lock of one is gets a guess — a short or upper-case revision, a
    /// locked source of another kind or repository, a name that would not go
    /// into an address — instead of a refusal of that input alone.
    #[test]
    fn a_strange_node_of_a_branch_is_refused_and_never_guessed() {
        let nixpkgs = github("NixOS", "nixpkgs", "nixos-unstable", NIXPKGS_REV);
        for ronoctua in [
            github("ronoctua", "ronoctua-distro", "release", "02c970a"),
            github(
                "ronoctua",
                "ronoctua-distro",
                "release",
                &DISTRO_REV.to_ascii_uppercase(),
            ),
            github("ronoctua", "ronoctua-distro", "release?x=1", DISTRO_REV),
            github("ronoctua", "ronoctua-distro", "a//b", DISTRO_REV),
            github("ronoctua", "ronoctua-distro", "../x", DISTRO_REV),
            github("ron octua", "ronoctua-distro", "release", DISTRO_REV),
            github("ronoctua", "..", "release", DISTRO_REV),
            format!(
                r#"{{"original":{{"owner":"ronoctua","ref":"release","repo":"ronoctua-distro","type":"github"}},
                "locked":{{"owner":"someone","repo":"ronoctua-distro","rev":"{DISTRO_REV}","type":"github"}}}}"#
            ),
            format!(
                r#"{{"original":{{"owner":"ronoctua","ref":"release","repo":"ronoctua-distro","type":"github"}},
                "locked":{{"rev":"{DISTRO_REV}","type":"git","url":"https://github.com/ronoctua/ronoctua-distro"}}}}"#
            ),
            r#"{"original":{"owner":"ronoctua","ref":"release","repo":"ronoctua-distro","type":"github"}}"#
                .to_owned(),
            r#"{"original":{"owner":"ronoctua","ref":7,"repo":"ronoctua-distro","type":"github"}}"#
                .to_owned(),
            r#"{"locked":{"type":"github"}}"#.to_owned(),
        ] {
            let read = of_lock(&lock(&nixpkgs, &ronoctua)).unwrap();
            assert_eq!(read[0], Followed::Refused(Input::Distro), "{ronoctua}");
            assert_eq!(read[1].input(), Input::Nixpkgs, "the other input is read");
            assert!(matches!(read[1], Followed::Branch(_)), "{ronoctua}");
        }
        // A NODE THE ROOT NAMES and the lock does not have.
        let missing = br#"{"nodes":{"root":{"inputs":{"nixpkgs":"gone"}}},"root":"root"}"#;
        assert_eq!(
            of_lock(missing),
            Ok(vec![Followed::Refused(Input::Nixpkgs)])
        );
        // THE OWNER'S CASE as `original` wrote it, the lock's as Nix did.
        let cased = format!(
            r#"{{"original":{{"owner":"nixos","ref":"nixos-unstable","repo":"nixpkgs","type":"github"}},
            "locked":{{"owner":"NixOS","repo":"nixpkgs","rev":"{NIXPKGS_REV}","type":"github"}}}}"#
        );
        let read = of_lock(&lock(&cased, "{}")).unwrap();
        assert_eq!(
            read,
            vec![
                Followed::Refused(Input::Distro),
                branch(
                    Input::Nixpkgs,
                    "nixos",
                    "nixpkgs",
                    "nixos-unstable",
                    NIXPKGS_REV
                ),
            ]
        );
    }

    /// FAILS IF: bytes that are not a lock read as one that follows nothing
    /// — an answer the window would draw as "nothing to learn" — or a
    /// machine with no lock is told apart from one whose lock is broken.
    #[test]
    fn a_lock_that_does_not_read_is_said_and_no_lock_is_another_thing() {
        for bad in [
            &b"{"[..],
            b"[]",
            b"{}",
            br#"{"nodes":{},"root":"root"}"#,
            br#"{"nodes":[],"root":"root"}"#,
            br#"{"nodes":{"root":{}}}"#,
        ] {
            assert_eq!(
                of_lock(bad),
                Err(Unread::Unreadable),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
        let dir = crate::tests::Scratch::new("followed");
        let file = dir.path(crate::LOCK);
        assert_eq!(read(&file), Err(Unread::NoLock));
        std::fs::write(&file, b"not a lock").unwrap();
        assert_eq!(read(&file), Err(Unread::Unreadable));
        std::fs::write(
            &file,
            lock(
                &github("NixOS", "nixpkgs", "nixos-unstable", NIXPKGS_REV),
                "{}",
            ),
        )
        .unwrap();
        assert_eq!(read(&file).map(|all| all.len()), Ok(2));
        let ids: std::collections::BTreeSet<_> = Unread::ALL.iter().map(|u| u.id()).collect();
        assert_eq!(ids.len(), Unread::ALL.len());
    }
}
