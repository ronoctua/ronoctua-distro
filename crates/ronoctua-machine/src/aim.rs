//! `update-distro`: the update's `flake-update` step — the distro and
//! `nixpkgs` inputs moved forward, and a machine the installer AIMED put
//! back on the release first.
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
    if run::status(&nix, &argv::update_distro(dir))? == Some(0) {
        return Ok("distro moved".to_owned());
    }
    if let Some(before) = back {
        disk::write_atomically(&file, before.as_bytes(), 0o644)
            .map_err(|e| disk::failed(&file, &e))?;
        say(
            4,
            "the distro input did not move: still aimed at the tree it was installed from",
        );
    }
    Err(Refusal::of(Why::Failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AIMED: &str = "{\n  # follows the `release` branch\n  inputs.ronoctua.url = \"path:/nix/store/zzzz-source\";\n  outputs = { ronoctua, ... }: ronoctua.lib.machineFlake { dir = ./.; };\n}\n";
    const TEMPLATE: &str = "{\n  inputs.ronoctua.url = \"github:someone/distro/release\";\n}\n";

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
