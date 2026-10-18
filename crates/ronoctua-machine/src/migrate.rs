//! `migrate`: the old layout (a copy of the whole distro in the machine
//! directory) becomes the machine's own flake — and only if the new one
//! builds.
//!
//! # The order, and why
//!
//! 1. The new flake is made BESIDE the old one, in `<dir>/.next`: the
//!    template's `flake.nix`, and what is the machine's own copied byte for
//!    byte — `host.nix`, `facter.json`, the secrets (`secrets/secrets.yaml`
//!    becomes `secrets.yaml`). Then it is locked and BUILT — with a copy of
//!    the person's `person/` and root's `local/` beside it, so it builds
//!    what the machine will. Any failure removes `.next` and nothing else:
//!    the old layout is untouched, and the machine updates as before.
//! 2. The old layout is COPIED to `<state>/legacy-<stamp>` (one is kept),
//!    before anything is removed from it.
//! 3. The old entries leave the folder and the new ones are moved in —
//!    all but `person/` and `local/`, which never move: only
//!    `ronoctua-person` writes them (`one-person-boundary`), so the folder
//!    keeps its own and the build's copies are dropped.
//! 4. The new lock is kept as the one the folder goes back to
//!    ([`crate::APPLIED_LOCK`]): the system that runs carries no machine
//!    lock, and this one is known to build.
//!
//! The folder itself is never renamed. With the root wiped at boot it is a
//! bind mount from `/persist`, and a mount point cannot be renamed; its
//! ENTRIES can, and `.next` is inside it so that every move is a rename on
//! one filesystem. If step 3 fails half way, the legacy copy is put back.

use std::path::Path;

use crate::system::{self, needed};
use crate::{Machine, Migrated, Refusal, Why, apply, argv, disk, guard, run, say};

/// Where the new flake is made, inside the machine directory.
pub(crate) const NEXT: &str = ".next";

/// What says a folder is the OLD layout: the distro's own modules in it.
pub const OLD_LAYOUT_MARK: &str = "nix/modules/base.nix";

/// What stays in the machine directory through a migration: the person's
/// data and root's record of it, which only `ronoctua-person` writes.
const STAYS: [&str; 2] = ["person", "local"];

/// The prefix of the folder the old layout is kept in.
pub const LEGACY_PREFIX: &str = "legacy-";

/// `migrate`.
pub(crate) fn migrate(m: &Machine) -> Result<Migrated, Refusal> {
    let _lock = guard(m)?;
    let (p, s) = (&m.places, &m.system);
    let dir = &p.dir;
    if !dir.join(OLD_LAYOUT_MARK).is_file() {
        say(
            6,
            &format!(
                "{} is not the old layout: nothing to migrate",
                dir.display()
            ),
        );
        return Err(Refusal::of(Why::NothingToMigrate));
    }
    if !dir.join("host.nix").is_file() {
        say(
            3,
            &format!(
                "{} has no host.nix: there is no machine to carry over",
                dir.display()
            ),
        );
        return Err(Refusal::of(Why::Unreadable));
    }
    let attr = apply::attr(m)?;
    let nix = needed(s.nix.as_ref(), system::NIX)?;
    let template = needed(s.template.as_ref(), system::TEMPLATE)?;

    let next = dir.join(NEXT);
    disk::remove(&next).map_err(|e| disk::failed(&next, &e))?;
    let made = make_next(m, &next, &template, &nix, &attr);
    if let Err(r) = made {
        let _ = disk::remove(&next);
        say(3, "the old layout stays as it was");
        return Err(r);
    }

    // THE OLD LAYOUT, copied whole before anything leaves it.
    std::fs::create_dir_all(&p.state).map_err(|e| disk::failed(&p.state, &e))?;
    let stamp = disk::compact(&disk::now());
    let legacy = p.state.join(format!("{LEGACY_PREFIX}{stamp}"));
    let partial = p.state.join(format!(".{LEGACY_PREFIX}{stamp}.partial"));
    let _ = disk::remove(&partial);
    let _ = disk::remove(&legacy);
    if let Err(e) =
        disk::copy_tree(dir, &partial, &[NEXT]).and_then(|()| std::fs::rename(&partial, &legacy))
    {
        let _ = disk::remove(&partial);
        let _ = disk::remove(&next);
        say(
            3,
            "the old layout could not be set aside; it stays as it was",
        );
        return Err(disk::failed(&legacy, &e));
    }

    // THE SWAP.
    if let Err(e) = swap(dir, &next) {
        say(
            3,
            &format!("the swap failed half way ({e}); the old layout is put back"),
        );
        if let Err(e) = restore(dir, &legacy) {
            say(
                3,
                &format!(
                    "and putting it back failed too ({e}): it is whole in {}",
                    legacy.display()
                ),
            );
        }
        return Err(Refusal::of(Why::Failed));
    }

    // ONE LEGACY IS KEPT, and the old lock snapshots described a flake that
    // is gone.
    if let Ok(names) = disk::names(&p.state) {
        for name in names {
            let name = name.to_string_lossy();
            if name.starts_with(LEGACY_PREFIX) && p.state.join(&*name) != legacy {
                let _ = disk::remove(&p.state.join(&*name));
            }
        }
    }
    for old in [crate::APPLIED_LOCK, crate::LAST_GOOD_LOCK] {
        let _ = disk::remove(&disk::snapshot(p, old));
    }
    // THE LOCK IT BUILT is the folder's baseline. The system that runs was
    // built from the old layout and carries no machine lock; this one is the
    // lock known to build here, and an apply that fails puts it back — the
    // update moves the input right after the migration.
    match std::fs::read(dir.join("flake.lock")) {
        Ok(lock) => {
            if let Err(e) = disk::keep_snapshot(p, crate::APPLIED_LOCK, &lock) {
                say(4, &format!("the migrated lock was not kept: {e}"));
            }
        }
        Err(e) => say(4, &format!("the migrated flake has no lock to keep: {e}")),
    }
    say(
        6,
        &format!("migrated; the old layout is in {}", legacy.display()),
    );
    Ok(Migrated { legacy })
}

/// The new flake in `next`, locked and built.
fn make_next(
    m: &Machine,
    next: &Path,
    template: &Path,
    nix: &Path,
    attr: &str,
) -> Result<(), Refusal> {
    let dir = &m.places.dir;
    std::fs::create_dir(next).map_err(|e| disk::failed(next, &e))?;
    let written = std::fs::read_to_string(template.join("flake.nix"))
        .map_err(|e| disk::failed(&template.join("flake.nix"), &e))?;
    let flake = match &m.system.distro {
        Some(url) => with_distro(&written, url).ok_or_else(|| {
            say(
                3,
                "the template does not name the distro input on one line of its own",
            );
            Refusal::of(Why::Failed)
        })?,
        None => written,
    };
    disk::write_atomically(&next.join("flake.nix"), flake.as_bytes(), 0o644)
        .map_err(|e| disk::failed(next, &e))?;

    // What is the machine's own, byte for byte.
    for (from, to) in [
        ("host.nix", "host.nix"),
        ("facter.json", "facter.json"),
        ("secrets.yaml", "secrets.yaml"),
        ("secrets/secrets.yaml", "secrets.yaml"),
        ("person", "person"),
        ("local", "local"),
    ] {
        let source = dir.join(from);
        if std::fs::symlink_metadata(&source).is_ok()
            && std::fs::symlink_metadata(next.join(to)).is_err()
        {
            disk::copy_one(&source, &next.join(to)).map_err(|e| disk::failed(&source, &e))?;
        }
    }

    if run::status(nix, &argv::lock(next))? != Some(0) {
        say(
            3,
            "the new machine flake could not be locked (build-failed)",
        );
        return Err(Refusal::of(Why::BuildFailed));
    }
    std::fs::create_dir_all(&m.places.run).map_err(|e| disk::failed(&m.places.run, &e))?;
    let out = m.places.run.join("migrated");
    let _ = disk::remove(&out);
    // Sized and in phases, as an apply builds it ([`crate::build`]); its
    // units are kept by the apply that follows, which builds the same system.
    let built = crate::build::system(m, nix, next, attr, &out);
    let _ = disk::remove(&out);
    if let Err(refused) = built {
        say(3, "the new machine flake does not build (build-failed)");
        return Err(Refusal::of(match refused.why {
            Why::Failed => Why::Failed,
            _ => Why::BuildFailed,
        }));
    }
    Ok(())
}

/// The template's `flake.nix` with the distro input at `url`: the one line
/// `inputs.<input>.url = "…";`, and only when there is exactly one.
#[must_use]
pub fn with_distro(flake: &str, url: &str) -> Option<String> {
    let head = format!("inputs.{}.url = \"", crate::DISTRO_INPUT);
    let lines: Vec<&str> = flake.lines().collect();
    let at: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.trim_start().starts_with(&head) && l.trim_end().ends_with("\";"))
        .map(|(i, _)| i)
        .collect();
    let [only] = at.as_slice() else {
        return None;
    };
    if url.contains(['"', '\\', '$', '\n']) {
        return None;
    }
    let indent: String = lines[*only]
        .chars()
        .take_while(|c| c.is_whitespace())
        .collect();
    let mut out: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
    out[*only] = format!("{indent}{head}{url}\";");
    let mut text = out.join("\n");
    if flake.ends_with('\n') {
        text.push('\n');
    }
    Some(text)
}

/// The old entries out, the new ones in, `next` gone.
fn swap(dir: &Path, next: &Path) -> std::io::Result<()> {
    for name in disk::names(dir)? {
        if name != NEXT && !STAYS.iter().any(|s| name == *s) {
            disk::remove(&dir.join(&name))?;
        }
    }
    for name in disk::names(next)? {
        if STAYS.iter().any(|s| name == *s) {
            disk::remove(&next.join(&name))?;
        } else {
            std::fs::rename(next.join(&name), dir.join(&name))?;
        }
    }
    std::fs::remove_dir(next)
}

/// The folder as `legacy` holds it (`person/` and `local/` never left).
fn restore(dir: &Path, legacy: &Path) -> std::io::Result<()> {
    for name in disk::names(dir)? {
        if !STAYS.iter().any(|s| name == *s) {
            disk::remove(&dir.join(&name))?;
        }
    }
    for name in disk::names(legacy)? {
        if !STAYS.iter().any(|s| name == *s) {
            disk::copy_one(&legacy.join(&name), &dir.join(&name))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "{\n  inputs.ronoctua.url = \"github:ronoctua/ronoctua-distro/release\";\n  outputs = x: x;\n}\n";

    #[test]
    fn the_distro_input_is_moved_on_its_own_line_only() {
        let moved = with_distro(TEMPLATE, "path:/nix/store/abc-source").unwrap();
        assert!(moved.contains("  inputs.ronoctua.url = \"path:/nix/store/abc-source\";\n"));
        assert!(moved.ends_with("}\n"));
        assert_eq!(moved.lines().count(), TEMPLATE.lines().count());
        assert_eq!(with_distro("{ }\n", "path:/x"), None, "no such line");
        let twice = format!("{TEMPLATE}  inputs.ronoctua.url = \"github:a/b\";\n");
        assert_eq!(with_distro(&twice, "path:/x"), None, "two lines");
        assert_eq!(with_distro(TEMPLATE, "path:/x\";evil"), None);
    }
}
