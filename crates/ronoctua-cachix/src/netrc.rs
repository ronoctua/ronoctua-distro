//! The netrc root keeps for a PRIVATE cache — how the machine's Nix daemon
//! reads it.
//!
//! # Why root's, and written by a gesture
//!
//! A private cache answers only who shows the token, and what downloads from
//! a substituter is the Nix DAEMON, which runs as root and reads
//! `nix.settings.netrc-file` ([`crate::NETRC`], set by
//! `nix/modules/binary-cache.nix` only for a private cache). So the token is
//! copied into a file of root's, `0600`, and written only by:
//!
//! - the sudo door's helper (`binary-cache-netrc <name>`, a `RootVerb` whose
//!   name is checked), when the person sets the token or makes the cache
//!   private — it takes the copy the gesture staged ([`take_staged`]);
//! - the installer, which read the token out of the sealed part with the
//!   passphrase, before `nixos-install` (`ronoctua-installer`).
//!
//! And removed ([`remove`], `binary-cache-netrc-remove`) when the token goes
//! or the cache stops being private: a token nothing reads is a token less
//! on the disk.
//!
//! # What it says
//!
//! One line, as Cachix asks for a private cache: `machine
//! <name>.cachix.org password <token>`. Built in memory that is wiped
//! ([`text`]); the host and the token are each checked first, so the line
//! can never say more than that.

use std::io::Read as _;
use std::os::unix::fs::DirBuilderExt as _;
use std::path::Path;

use ronoctua_consent::keys::{Secret, write_private_file};
use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use zeroize::Zeroizing;

use crate::Why;
use crate::token::{MOST_TOKEN_BYTES, STAGED, is_token};

/// The netrc's one line for the cache `name` (checked: a Cachix name) and
/// `token` (checked: a token's characters).
///
/// # Errors
///
/// [`Why::NoCache`] for a name that is not a cache's, [`Why::NotAToken`]
/// for a token that is not one.
pub fn text(name: &str, token: &Secret) -> Result<Zeroizing<String>, Why> {
    if !ronoctua_person::options::is_cache_name(name) {
        return Err(Why::NoCache);
    }
    if !is_token(token.expose()) {
        return Err(Why::NotAToken);
    }
    Ok(Zeroizing::new(format!(
        "machine {name}.cachix.org password {}\n",
        token.expose()
    )))
}

/// Writes the netrc for `name` at `path`: its folder made `0755` when it is
/// not there (never through a link), the file `0600` from its first byte,
/// replacing the old one whole.
///
/// # Errors
///
/// What [`text`] refuses; [`Why::Unwritable`] when the folder is a link or
/// not a folder, or the file could not be written.
pub fn write(path: &Path, name: &str, token: &Secret) -> Result<(), Why> {
    let line = text(name, token)?;
    let folder = path.parent().ok_or(Why::Unwritable)?;
    match std::fs::symlink_metadata(folder) {
        Ok(m) if m.file_type().is_dir() => {}
        Ok(_) => return Err(Why::Unwritable),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o755)
                .create(folder)
                .map_err(|_| Why::Unwritable)?;
        }
        Err(_) => return Err(Why::Unwritable),
    }
    write_private_file(path, line.as_bytes()).map_err(|_| Why::Unwritable)
}

/// Removes the netrc. Removing one that is not there is not a failure.
///
/// # Errors
///
/// [`Why::Unwritable`].
pub fn remove(path: &Path) -> Result<(), Why> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(Why::Unwritable),
        _ => Ok(()),
    }
}

/// Whether the netrc is there — what the screen says of a private cache,
/// asked of the folder (`0755`) without reading the file.
#[must_use]
pub fn present(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

/// ROOT'S HALF: the token the person `uid` staged in their runtime folder
/// `runtime` ([`crate::token::stage`]), taken and deleted.
///
/// The folder is opened as a folder, never through a link, and must be
/// `uid`'s; the copy is opened relative to it with `O_NOFOLLOW`, and must be
/// a regular file of `uid`'s with no bits for anybody else, at most a
/// token's size. Whatever it holds is checked as a token.
///
/// # Errors
///
/// [`Why::NoToken`] when nothing is staged; [`Why::TokenOpen`] when the
/// folder or the copy is not what it must be; [`Why::NotAToken`].
pub fn take_staged(runtime: &Path, uid: u32) -> Result<Secret, Why> {
    let folder = rustix::fs::open(
        runtime,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|e| {
        if e == rustix::io::Errno::NOENT {
            Why::NoToken
        } else {
            Why::TokenOpen
        }
    })?;
    let at = rustix::fs::fstat(&folder).map_err(|_| Why::TokenOpen)?;
    if at.st_uid != uid {
        return Err(Why::TokenOpen);
    }
    let file = match rustix::fs::openat(
        &folder,
        STAGED,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(f) => f,
        Err(rustix::io::Errno::NOENT) => return Err(Why::NoToken),
        Err(_) => return Err(Why::TokenOpen),
    };
    let stat = rustix::fs::fstat(&file).map_err(|_| Why::TokenOpen)?;
    let size = usize::try_from(stat.st_size).unwrap_or(usize::MAX);
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile
        || stat.st_uid != uid
        || stat.st_mode & 0o077 != 0
        || stat.st_nlink != 1
        || size > MOST_TOKEN_BYTES
    {
        return Err(Why::TokenOpen);
    }
    // TAKEN: whatever it holds, the copy goes now.
    let _ = rustix::fs::unlinkat(&folder, STAGED, AtFlags::empty());
    let mut bytes = Zeroizing::new(Vec::with_capacity(size));
    std::fs::File::from(file)
        .take(u64::try_from(MOST_TOKEN_BYTES + 1).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|_| Why::TokenOpen)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Why::NotAToken)?;
    let token = text.trim();
    if is_token(token) {
        Ok(Secret::new(token))
    } else {
        Err(Why::NotAToken)
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJqdGkiOiIxIn0.c2lnbmF0dXJl_-";

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-cachix-netrc-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn me() -> u32 {
        rustix::process::getuid().as_raw()
    }

    /// FAILS IF: the line says anything but the cache's host and the token,
    /// or takes a name or a token that would let it say more.
    #[test]
    fn the_line_is_the_host_and_the_token_and_nothing_else() {
        let line = text("someone", &Secret::new(TOKEN)).unwrap();
        assert_eq!(
            line.as_str(),
            format!("machine someone.cachix.org password {TOKEN}\n")
        );
        assert_eq!(
            text("x\nmachine evil", &Secret::new(TOKEN)).map(|_| ()),
            Err(Why::NoCache)
        );
        assert_eq!(
            text("someone", &Secret::new("a b\nmachine evil.org")).map(|_| ()),
            Err(Why::NotAToken)
        );
    }

    /// FAILS IF: the netrc is born readable, its folder is made through a
    /// link, or removing it twice fails.
    #[test]
    fn the_netrc_is_private_from_its_first_byte_and_goes_on_request() {
        let root = scratch("write");
        let path = root.join("var/lib/ronoctua-binary-cache/netrc");
        write(&path, "someone", &Secret::new(TOKEN)).unwrap();
        assert!(present(&path));
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        remove(&path).unwrap();
        remove(&path).unwrap();
        assert!(!present(&path));
        let linked = root.join("linked");
        std::os::unix::fs::symlink(root.join("elsewhere"), &linked).unwrap();
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        assert_eq!(
            write(&linked.join("netrc"), "someone", &Secret::new(TOKEN)),
            Err(Why::Unwritable)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: root takes a staged copy that is a link, opened up, not the
    /// person's or not a token — or leaves a taken copy behind.
    #[test]
    fn only_the_persons_own_private_copy_is_taken_and_it_is_spent() {
        let root = scratch("staged");
        assert_eq!(take_staged(&root, me()).map(|_| ()), Err(Why::NoToken));
        crate::token::stage(&root, &Secret::new(TOKEN)).unwrap();
        assert_eq!(
            take_staged(&root, me().wrapping_add(1)).map(|_| ()),
            Err(Why::TokenOpen),
            "another person's folder"
        );
        assert_eq!(take_staged(&root, me()).unwrap().expose(), TOKEN);
        assert!(!root.join(STAGED).exists(), "the copy is spent");
        // OPENED UP: refused, and not read.
        crate::token::stage(&root, &Secret::new(TOKEN)).unwrap();
        std::fs::set_permissions(root.join(STAGED), std::fs::Permissions::from_mode(0o640))
            .unwrap();
        assert_eq!(take_staged(&root, me()).map(|_| ()), Err(Why::TokenOpen));
        std::fs::remove_file(root.join(STAGED)).unwrap();
        // A LINK where the copy goes: never followed.
        std::fs::write(root.join("secret"), TOKEN).unwrap();
        std::os::unix::fs::symlink(root.join("secret"), root.join(STAGED)).unwrap();
        assert_eq!(take_staged(&root, me()).map(|_| ()), Err(Why::TokenOpen));
        std::fs::remove_file(root.join(STAGED)).unwrap();
        // NOT A TOKEN: refused, and spent all the same.
        write_private_file(&root.join(STAGED), b"machine evil.org password x").unwrap();
        assert_eq!(take_staged(&root, me()).map(|_| ()), Err(Why::NotAToken));
        assert!(!root.join(STAGED).exists());
        // THE FOLDER A LINK: never followed.
        let linked = root.join("linked-run");
        std::os::unix::fs::symlink(&root, &linked).unwrap();
        crate::token::stage(&root, &Secret::new(TOKEN)).unwrap();
        assert_eq!(take_staged(&linked, me()).map(|_| ()), Err(Why::TokenOpen));
        let _ = std::fs::remove_dir_all(&root);
    }
}
