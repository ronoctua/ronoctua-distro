//! `seed --root DIR`: the parts an installation medium carries, into the
//! store of the machine it just installed under `DIR`.
//!
//! The installer runs it after `nixos-install` (`crate::plan` of
//! `ronoctua-installer`, `INST_STEP_SEED_PARTS`). The machine's own plan
//! (`/etc/ronoctua/parts-plan.json` of the system under `DIR`) names each
//! part's output; every one this medium's store holds is copied over, so the
//! first boot's realizer finds it valid and makes it the part's profile
//! with no build and no network (`realize`'s seeding). A part the medium
//! does not hold downloads later, like any other.
//!
//! IT NEVER FAILS THE INSTALLATION: a plan it cannot read, or a copy that
//! does not go, is said and ends [`exit::DONE`] — the machine is whole
//! without its parts.

use std::path::{Path, PathBuf};

use ronoctua_parts::{exit, names};

use crate::{Usage, beside_nix, disk, program, say};

/// The most symlinks one path may cross before it is given up on.
const HOPS_MOST: usize = 40;

pub fn run(args: &[String]) -> Result<u8, Usage> {
    let [flag, root] = args else {
        return Err(Usage);
    };
    if flag != "--root" {
        return Err(Usage);
    }
    let root = Path::new(root);
    // THE MACHINE'S `/etc`, as its activation left it under `DIR`: each
    // entry a link to `/etc/static`, a link into the store.
    let Some(plan) = within(root, names::PLAN).and_then(|at| ronoctua_parts::plan::read(&at))
    else {
        say(
            4,
            &format!("no parts plan under {}: nothing seeded", root.display()),
        );
        println!("seeded 0");
        return Ok(exit::DONE);
    };
    let here: Vec<String> = plan
        .parts
        .values()
        .map(|w| w.out.clone())
        .filter(|out| {
            ronoctua_parts::plan::is_store_path(out)
                && disk::run_quiet(&[
                    beside_nix("nix-store"),
                    "--check-validity".to_owned(),
                    out.clone(),
                ])
        })
        .collect();
    if here.is_empty() {
        println!("seeded 0");
        return Ok(exit::DONE);
    }
    // `--no-check-sigs`: the medium's store is the one the whole system was
    // just copied from, as `nixos-install` copies it.
    let mut copy = vec![
        program("RONOCTUA_NIX"),
        "copy".to_owned(),
        "--no-check-sigs".to_owned(),
        "--to".to_owned(),
        format!("local?root={}", root.display()),
    ];
    copy.extend(here.iter().cloned());
    let said = disk::run(&copy);
    if said.ok {
        println!("seeded {}", here.len());
    } else {
        say(
            4,
            &format!("the parts were not copied: {}", said.err.trim()),
        );
        println!("seeded 0");
    }
    Ok(exit::DONE)
}

/// `path` as the system under `root` resolves it: every symlink on the way
/// followed, an absolute target taken under `root` again. `None` when a
/// component is missing or the links go round.
pub fn within(root: &Path, path: &str) -> Option<PathBuf> {
    let mut todo: Vec<String> = path
        .split('/')
        .filter(|c| !c.is_empty())
        .rev()
        .map(str::to_owned)
        .collect();
    let mut at = PathBuf::new();
    let mut hops = 0;
    while let Some(component) = todo.pop() {
        match component.as_str() {
            "." => continue,
            ".." => {
                at.pop();
                continue;
            }
            _ => at.push(&component),
        }
        let real = root.join(&at);
        let Ok(meta) = std::fs::symlink_metadata(&real) else {
            return None;
        };
        if meta.file_type().is_symlink() {
            hops += 1;
            if hops > HOPS_MOST {
                return None;
            }
            let target = std::fs::read_link(&real).ok()?;
            at.pop();
            if target.is_absolute() {
                at = PathBuf::new();
            }
            for c in target
                .to_string_lossy()
                .split('/')
                .filter(|c| !c.is_empty())
                .rev()
            {
                todo.push(c.to_owned());
            }
        }
    }
    Some(root.join(at))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn a_path_is_resolved_inside_its_root() {
        let dir = std::env::temp_dir().join(format!("parts-seed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = dir.join("nix/store/abc-etc/etc/ronoctua");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("parts-plan.json"), "{}").unwrap();
        // The machine's `/etc` as an activation leaves it: the entry a link
        // to `/etc/static`, and `/etc/static` a link into the store — both
        // absolute, so both are taken under the root again.
        std::fs::create_dir_all(dir.join("etc/ronoctua")).unwrap();
        symlink("/nix/store/abc-etc/etc", dir.join("etc/static")).unwrap();
        symlink(
            "/etc/static/ronoctua/parts-plan.json",
            dir.join("etc/ronoctua/parts-plan.json"),
        )
        .unwrap();
        assert_eq!(
            within(&dir, names::PLAN),
            Some(dir.join("nix/store/abc-etc/etc/ronoctua/parts-plan.json"))
        );
        assert_eq!(within(&dir, "/etc/ronoctua/absent.json"), None);
        // Links that go round are given up on.
        symlink("loop-b", dir.join("loop-a")).unwrap();
        symlink("loop-a", dir.join("loop-b")).unwrap();
        assert_eq!(within(&dir, "/loop-a"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
