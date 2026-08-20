//! The one thing that runs as root — and it validates before it acts.
//!
//! # What it is for
//!
//! Four operations need privilege on a personal machine: changing an owner,
//! changing a group one does not belong to, changing the permissions of a file
//! one does not own, and telling the device which blocks are free. The file
//! manager never does any of them itself. It writes what it wants into a
//! directory it owns and asks a systemd unit to install it; this is that unit.
//!
//! It is the shape `ronoctua_rescue::edit` already uses to write `host.nix`,
//! and the reason is the same: the application runs as the person, the target
//! belongs to root, and the thing in between has to be small enough to read in
//! one sitting.
//!
//! # Why deleting and moving are NOT here
//!
//! Because they cannot be undone, and a root delete one key away from a listing
//! is how a file manager erases somebody's system. Whoever needs that opens the
//! root terminal the menu offers, where what is happening is visible.
//!
//! # What it refuses
//!
//! A verb that is not one of the four; a relative path; a path under
//! `/nix/store`, `/proc`, `/sys` or `/dev`; and a final component that is a
//! symlink. The last one is the interesting one: without `NOFOLLOW` the person
//! could be shown one file and a swap could make root change another.

use std::path::Path;

use anyhow::{Context, Result, bail};
use ronoctua_files::act::{STAGING, Staged, Verb};

/// Where a request may NOT point.
///
/// The store because it is read-only and immutable by design; the three
/// pseudo-filesystems because a `chmod` there is either meaningless or a way to
/// change how the kernel behaves. None of them is a place a file manager has
/// business in.
const FORBIDDEN: [&str; 4] = ["/nix/store", "/proc", "/sys", "/dev"];

fn main() -> Result<()> {
    let staging = std::env::args()
        .nth(1)
        .unwrap_or_else(|| STAGING.to_owned());
    let request = Staged::read_from(Path::new(&staging))
        .with_context(|| format!("ler o pedido em {staging}"))?;
    check(&request.path)?;
    act(&request)?;
    // The request is consumed. Left behind, the next `systemctl start` — from
    // anywhere — would replay it, which turns one authorisation into a standing
    // one.
    let _ = std::fs::remove_file(Path::new(&staging).join("verb"));
    let _ = std::fs::remove_file(Path::new(&staging).join("path"));
    let _ = std::fs::remove_file(Path::new(&staging).join("value"));
    Ok(())
}

/// Whether this path may be touched at all.
///
/// # Errors
///
/// When the path is relative, points into a forbidden tree, or its last
/// component is a symlink.
pub fn check(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        bail!("caminho relativo: {}", path.display());
    }
    // The PARENT is canonicalised and the last component is not: canonicalising
    // the whole path would resolve a final symlink, which is exactly what must
    // not happen — the entry the listing drew is the entry that gets changed.
    let parent = path.parent().unwrap_or(Path::new("/"));
    let real = parent
        .canonicalize()
        .with_context(|| format!("{} nao existe", parent.display()))?;
    for forbidden in FORBIDDEN {
        if real.starts_with(forbidden) {
            bail!("{} fica em {forbidden}, e isso nao se mexe", path.display());
        }
    }
    let here = real.join(path.file_name().unwrap_or_default());
    let meta = std::fs::symlink_metadata(&here)
        .with_context(|| format!("{} nao existe", here.display()))?;
    if meta.is_symlink() {
        bail!(
            "{} e um link simbolico; mude o alvo, que e o que voce esta vendo",
            here.display()
        );
    }
    Ok(())
}

/// Carries out one validated request.
fn act(request: &Staged) -> Result<()> {
    match request.verb {
        Verb::Chown => {
            let uid: u32 = request.value.parse().context("uid invalido")?;
            rustix::fs::chownat(
                rustix::fs::CWD,
                &request.path,
                Some(rustix::fs::Uid::from_raw(uid)),
                None,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .with_context(|| format!("mudar o dono de {}", request.path.display()))
        }
        Verb::Chgrp => {
            let gid: u32 = request.value.parse().context("gid invalido")?;
            rustix::fs::chownat(
                rustix::fs::CWD,
                &request.path,
                None,
                Some(rustix::fs::Gid::from_raw(gid)),
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )
            .with_context(|| format!("mudar o grupo de {}", request.path.display()))
        }
        Verb::Chmod => {
            let mode = u32::from_str_radix(&request.value, 8).context("modo invalido")?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                &request.path,
                std::fs::Permissions::from_mode(mode & 0o7777),
            )
            .with_context(|| format!("mudar a permissao de {}", request.path.display()))
        }
        Verb::Trim => {
            let fstrim = option_env!("RONOCTUA_FSTRIM")
                .context("RONOCTUA_FSTRIM nao foi definida em nix/rust.nix")?;
            let out = std::process::Command::new(fstrim)
                .arg(&request.path)
                .output()
                .context("rodar o fstrim")?;
            if out.status.success() {
                return Ok(());
            }
            bail!("{}", String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_relative_path_is_refused() {
        let e = check(Path::new("host.nix")).unwrap_err();
        assert!(e.to_string().contains("relativo"), "{e}");
    }

    #[test]
    fn the_store_and_the_pseudo_filesystems_are_refused() {
        // THE DEFECT THIS CATCHES: a `chmod` on `/proc/sys/...`, which is not a
        // file operation at all — it is changing how the kernel behaves,
        // through a door that exists to rename somebody's photographs.
        for path in ["/proc/self/status", "/sys/kernel/vmcoreinfo"] {
            let p = Path::new(path);
            if !p.exists() {
                continue;
            }
            let e = check(p).unwrap_err();
            assert!(e.to_string().contains("nao se mexe"), "{path}: {e}");
        }
    }

    #[test]
    fn a_symlink_is_refused_rather_than_followed() {
        // THE DEFECT THIS CATCHES, and it is the sharp one: the person is shown
        // a link and a swap between the click and the unit makes root act on
        // whatever it points at. The final component is never resolved.
        let dir = std::env::temp_dir().join(format!("ronoctua-root-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("alvo"), b"x").unwrap();
        std::os::unix::fs::symlink("alvo", dir.join("atalho")).unwrap();
        let e = check(&dir.join("atalho")).unwrap_err();
        assert!(e.to_string().contains("link simbolico"), "{e}");
        // And the real file behind it is fine.
        assert!(check(&dir.join("alvo")).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_path_that_does_not_exist_is_refused() {
        let e = check(Path::new("/tmp/isto-nao-existe-em-lugar-nenhum-42")).unwrap_err();
        assert!(e.to_string().contains("nao existe"), "{e}");
    }
}
