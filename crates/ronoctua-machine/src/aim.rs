//! `update-distro`: the update's `flake-update` step — the distro and
//! `nixpkgs` inputs moved forward, and a machine the installer AIMED put
//! back on the release first.
//!
//! # A local checkout
//!
//! On a machine whose lock names a LOCAL checkout of the distro (the
//! development machine: `git+file:`, `path:`, any `file:` source outside
//! the store), any process of the person's may commit there, and the
//! update runs as root with no password — the weekly routine, the
//! person's own start of its root half. So there it moves `nixpkgs` alone
//! ([`follows_a_checkout`]), and the person's commits come in only
//! through gestures that pass the sudo door (`.#apply-here`, a root
//! `nix flake update ronoctua`). And whatever the lock said before, a
//! move that leaves the distro input at another revision of a local
//! checkout — a `flake.nix` that names one its lock does not, which Nix
//! locks afresh — is put back and refused ([`Why::CheckoutMoved`]).
//!
//! # Aimed
//!
//! An installation reaches no network, so the installer aims the machine's
//! flake at its own tree (`inputs.ronoctua.url = "path:/nix/store/…"`,
//! `ronoctua_installer::plan::Origin`) and the machine keeps that pair. A
//! `nix flake update` of a `path:` input locks the same path again: the
//! machine would never follow the release. So the first update writes the
//! template's address back — read from that very tree's
//! `templates/machine/flake.nix`, never spelled here — and moves the inputs;
//! when the move does not go (no network), `flake.nix` is put back as it
//! was, for a `flake.nix` that disagrees with its lock is refused by every
//! evaluation that may not move the lock.

use std::path::Path;

use crate::{DISTRO_INPUT, Machine, Refusal, Why, argv, disk, guard, run, say, system};

/// The template, under the distro's tree.
pub const TEMPLATE_FLAKE: &str = "templates/machine/flake.nix";

/// The store tree a machine's `flake.nix` aims its distro input at, when
/// it does: the one `"path:/nix/store/…"` literal of the input's line. Pure.
#[must_use]
pub fn aimed_at(flake: &str) -> Option<&str> {
    let line = flake.lines().find(|l| {
        l.trim_start()
            .starts_with(&format!("inputs.{DISTRO_INPUT}.url"))
    })?;
    let start = line.find("\"path:/nix/store/")? + "\"path:".len();
    let rest = &line[start..];
    Some(&rest[..rest.find('"')?])
}

/// The address a template's flake gives the distro input: the literal of its
/// `inputs.<distro>.url` line. Pure.
#[must_use]
pub fn release_address(template: &str) -> Option<&str> {
    let line = template.lines().find(|l| {
        l.trim_start()
            .starts_with(&format!("inputs.{DISTRO_INPUT}.url"))
    })?;
    let start = line.find('"')? + 1;
    let rest = &line[start..];
    let address = &rest[..rest.find('"')?];
    (!address.starts_with("path:")).then_some(address)
}

/// `flake` with its distro input aimed at `release` instead of the store
/// tree it named: the one literal replaced, and nothing else. Pure.
#[must_use]
pub fn unaimed(flake: &str, release: &str) -> Option<String> {
    let tree = aimed_at(flake)?;
    let literal = format!("\"path:{tree}\"");
    (flake.matches(&literal).count() == 1)
        .then(|| flake.replacen(&literal, &format!("\"{release}\""), 1))
}

/// Whether a `flake.lock` names, for the distro input, a LOCAL checkout —
/// its `original` or its `locked` a `path` outside the store, or a source
/// whose `url` is a `file:` one outside the store (`git+file:`). A store
/// path is no checkout: nobody writes it, and an aimed machine names one.
/// Pure.
#[must_use]
pub fn follows_a_checkout(lock: &[u8]) -> bool {
    disk::input_node(lock, DISTRO_INPUT).is_some_and(|node| {
        ["original", "locked"]
            .iter()
            .any(|field| node.get(field).is_some_and(is_local))
    })
}

/// Whether one source of a lock's node is on this machine's disk, outside
/// the store.
fn is_local(source: &serde_json::Value) -> bool {
    let text = |k: &str| source.get(k).and_then(serde_json::Value::as_str);
    let outside_the_store = |path: &str| !path.starts_with("/nix/store/");
    if text("type") == Some("path") {
        return text("path").is_some_and(outside_the_store);
    }
    text("url")
        .and_then(|url| url.strip_prefix("file:"))
        .is_some_and(|rest| outside_the_store(rest.trim_start_matches("//")))
}

/// How the inputs moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Moved {
    /// The distro input and `nixpkgs`.
    Both,
    /// `nixpkgs` alone: the lock names a local checkout of the distro.
    NixpkgsOnly,
}

/// `nix flake update` in the folder `dir`: the distro input and `nixpkgs`,
/// or `nixpkgs` alone where its lock names a local checkout — and after it,
/// a distro input left at another revision of a local checkout put back
/// with the whole lock, and refused. What the update runs on the machine
/// directory, and its rehearsal on a copy.
pub(crate) fn move_inputs(nix: &Path, dir: &Path) -> Result<Moved, Refusal> {
    let file = dir.join("flake.lock");
    let before = disk::read_if_there(&file)?;
    let checkout = before.as_deref().is_some_and(follows_a_checkout);
    let ran = run::status(nix, &argv::update_distro(dir, checkout))?;
    let after = disk::read_if_there(&file)?;
    let distro = |lock: &Option<Vec<u8>>| {
        lock.as_deref()
            .and_then(|l| disk::input_node(l, DISTRO_INPUT))
            .and_then(|node| node.get("locked").cloned())
    };
    if after.as_deref().is_some_and(follows_a_checkout) && distro(&before) != distro(&after) {
        match &before {
            Some(bytes) => disk::write_atomically(&file, bytes, 0o644),
            None => std::fs::remove_file(&file),
        }
        .map_err(|e| disk::failed(&file, &e))?;
        say(
            3,
            "the distro input would have moved to another revision of a local checkout: the lock \
             is put back, and only the sudo door moves it (checkout-moved)",
        );
        return Err(Refusal::of(Why::CheckoutMoved));
    }
    if ran != Some(0) {
        return Err(Refusal::of(Why::Failed));
    }
    if checkout {
        say(
            6,
            "the distro input names a local checkout: nixpkgs alone moved, and the checkout's \
             commits come in only through the sudo door",
        );
        Ok(Moved::NixpkgsOnly)
    } else {
        Ok(Moved::Both)
    }
}

/// `update-distro`.
pub(crate) fn update_distro(m: &Machine) -> Result<String, Refusal> {
    let _lock = guard(m)?;
    let dir = &m.places.dir;
    let nix = system::needed(m.system.nix.as_ref(), system::NIX)?;
    if disk::read_state(&m.places)?.hold.is_some() {
        say(6, "held: the distro and nixpkgs inputs stay where they are");
        return Ok("held".to_owned());
    }
    let file = dir.join("flake.nix");
    let before = std::fs::read_to_string(&file).map_err(|e| disk::failed(&file, &e))?;
    let back = match aimed_at(&before) {
        None => None,
        Some(tree) => {
            let template = Path::new(tree).join(TEMPLATE_FLAKE);
            let text =
                std::fs::read_to_string(&template).map_err(|e| disk::failed(&template, &e))?;
            let Some(release) = release_address(&text) else {
                say(
                    3,
                    &format!("{} names no release address", template.display()),
                );
                return Err(Refusal::of(Why::Unreadable));
            };
            let Some(after) = unaimed(&before, release) else {
                say(
                    3,
                    &format!("{} names its tree more than once", file.display()),
                );
                return Err(Refusal::of(Why::Unreadable));
            };
            disk::write_atomically(&file, after.as_bytes(), 0o644)
                .map_err(|e| disk::failed(&file, &e))?;
            say(6, &format!("aimed at {tree}: following {release} again"));
            Some(before)
        }
    };
    let refusal = match move_inputs(&nix, dir) {
        Ok(Moved::Both) => return Ok("distro moved".to_owned()),
        Ok(Moved::NixpkgsOnly) => {
            return Ok(
                "nixpkgs moved; the distro stays at the checkout's locked revision".to_owned(),
            );
        }
        Err(refusal) => refusal,
    };
    if let Some(before) = back {
        disk::write_atomically(&file, before.as_bytes(), 0o644)
            .map_err(|e| disk::failed(&file, &e))?;
        say(
            4,
            "the distro input did not move: still aimed at the tree it was installed from",
        );
    }
    Err(refusal)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIMED: &str = "{\n  # follows the `release` branch\n  inputs.ronoctua.url = \"path:/nix/store/zzzz-source\";\n  outputs = { ronoctua, ... }: ronoctua.lib.machineFlake { dir = ./.; };\n}\n";
    const TEMPLATE: &str = "{\n  inputs.ronoctua.url = \"github:someone/distro/release\";\n}\n";

    /// FAILS IF: a lock whose distro input is on this machine's disk outside
    /// the store — `git+file:`, a bare `path:`, a `file:` tarball — is not
    /// a checkout, or the release, a store tree (an aimed machine) or a lock
    /// that does not read is taken for one.
    #[test]
    fn a_checkout_is_a_source_on_this_disk_outside_the_store() {
        let lock = |node: &str| {
            format!(
                r#"{{"nodes":{{"root":{{"inputs":{{"ronoctua":"distro"}}}},"distro":{node}}},"root":"root","version":7}}"#
            )
        };
        for local in [
            r#"{"original":{"type":"git","url":"file:///home/p/distro"},"locked":{"type":"git","url":"file:///home/p/distro","rev":"a"}}"#,
            r#"{"original":{"type":"path","path":"/home/p/distro"},"locked":{"type":"path","path":"/home/p/distro"}}"#,
            r#"{"original":{"type":"tarball","url":"file:/srv/distro.tar.gz"}}"#,
            // ONE OF THE TWO is enough: a lock written by hand.
            r#"{"original":{"type":"github","owner":"x","repo":"y"},"locked":{"type":"path","path":"/home/p/distro"}}"#,
        ] {
            assert!(follows_a_checkout(lock(local).as_bytes()), "{local}");
        }
        for not in [
            r#"{"original":{"type":"github","owner":"ronoctua","repo":"ronoctua-distro","ref":"release"}}"#,
            r#"{"original":{"type":"path","path":"/nix/store/zzzz-source"},"locked":{"type":"path","path":"/nix/store/zzzz-source"}}"#,
            r#"{"original":{"type":"git","url":"file:///nix/store/zzzz-source"}}"#,
            r#"{"original":{"type":"git","url":"https://example.org/distro.git"}}"#,
        ] {
            assert!(!follows_a_checkout(lock(not).as_bytes()), "{not}");
        }
        assert!(!follows_a_checkout(b"{"));
        assert!(!follows_a_checkout(
            br#"{"nodes":{"root":{}},"root":"root"}"#
        ));
    }

    #[test]
    fn an_aimed_machine_is_put_back_on_its_release_by_its_one_literal() {
        assert_eq!(aimed_at(AIMED), Some("/nix/store/zzzz-source"));
        assert_eq!(
            release_address(TEMPLATE),
            Some("github:someone/distro/release")
        );
        let back = unaimed(AIMED, "github:someone/distro/release").expect("unaimed");
        assert!(back.contains("inputs.ronoctua.url = \"github:someone/distro/release\";"));
        assert!(
            back.contains("# follows the `release` branch"),
            "nothing else changes"
        );
        // A machine that follows the release already is not touched, and a
        // template aimed at a tree gives no release address.
        assert_eq!(aimed_at(&back), None);
        assert_eq!(unaimed(&back, "github:x/y"), None);
        assert_eq!(release_address(AIMED), None);
    }
}
