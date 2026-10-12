//! The push token — the one secret of the person's cache.
//!
//! # Where it lives
//!
//! In the person's configuration root, in a `0700` folder of private files
//! (`ronoctua_consent::keys::Dir`): `config/binary-cache-keys/push-token`,
//! `0600` from its first byte. The belongings keep the whole `config` tree
//! SEALED, so the token travels encrypted to the person's other machines and
//! comes back `0600` (a file of owner-only bits is restored born with them,
//! in a `0700` folder); it is never under a plain root
//! (`belongings-token-is-sealed` in `ronoctua-belongings`'s tests).
//!
//! It is never an argument, never in the environment of a long-lived
//! process, never in a log: [`read_from`] hands back a
//! [`ronoctua_consent::keys::Secret`], which prints as `<redacted>` and is
//! wiped when dropped, and the push gives it to `cachix` in that CHILD's
//! environment alone.
//!
//! # For root, staged
//!
//! A PRIVATE cache needs the token to be READ, by the machine's Nix daemon,
//! from a netrc only root writes ([`crate::netrc`]). The person's gesture
//! stages a copy in their runtime folder ([`stage`], `0600`, a fixed name),
//! the sudo door's helper takes it ([`crate::netrc::take_staged`], which
//! deletes it), and the gesture deletes it again however it ended
//! ([`unstage`]).

use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use ronoctua_consent::keys::{Dir, MOST_BYTES, Secret, write_private_file};

use crate::Why;

/// The folder of private files under the configuration root.
pub const DIR: &str = "binary-cache-keys";

/// The token's file in it.
pub const FILE: &str = "push-token";

/// The staged copy's name, in the person's runtime folder
/// (`/run/user/<uid>`): fixed, so root's helper looks in one place and
/// nowhere else.
pub const STAGED: &str = "ronoctua-binary-cache.token";

/// The longest token taken, in bytes: a Cachix token is a JWT of a few
/// hundred characters, and anything near this is not one.
pub const MOST_TOKEN_BYTES: usize = 4096;

/// This instance's folder — `None` with no home.
#[must_use]
pub fn dir() -> Option<Dir> {
    Dir::in_config(DIR)
}

/// The token's logical path in the belongings (`config/…`).
#[must_use]
pub fn logical() -> String {
    format!("config/{DIR}/{FILE}")
}

/// Whether `text` may be a Cachix token: a JWT's characters (base64url and
/// dots), sixteen or more, at most [`MOST_TOKEN_BYTES`]. A space, a quote, a
/// line break — anything a netrc or an environment would read as something
/// else — is refused. Pure.
#[must_use]
pub fn is_token(text: &str) -> bool {
    let b = text.as_bytes();
    (16..=MOST_TOKEN_BYTES).contains(&b.len())
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// Keeps `token` (trimmed) in `dir`, replacing the one there.
///
/// # Errors
///
/// [`Why::NotAToken`] for text that is not one; [`Why::Unwritable`].
pub fn keep(dir: &Dir, token: &Secret) -> Result<(), Why> {
    let trimmed = Secret::new(token.expose().trim());
    if !is_token(trimmed.expose()) {
        return Err(Why::NotAToken);
    }
    dir.keep(FILE, &trimmed).map_err(|_| Why::Unwritable)
}

/// Forgets the token. Forgetting one that is not there is not a failure.
///
/// # Errors
///
/// [`Why::Unwritable`].
pub fn forget(dir: &Dir) -> Result<(), Why> {
    dir.forget(FILE).map_err(|_| Why::Unwritable)
}

/// Whether a token is kept (a file there, whatever it holds — the screen
/// says "kept"; [`read_from`] says whether it is good).
#[must_use]
pub fn is_kept(dir: &Dir) -> bool {
    std::fs::symlink_metadata(dir.path().join(FILE)).is_ok_and(|m| m.len() > 0)
}

/// The token kept in `dir`, or `None` when there is none.
///
/// # Errors
///
/// [`Why::TokenOpen`] when the file is not a file of the person's alone
/// (a link, a folder, bits for the group or others) — it is not read;
/// [`Why::NotAToken`] when what it holds is not one; [`Why::Unwritable`]
/// when it cannot be read at all.
pub fn read_from(dir: &Dir) -> Result<Option<Secret>, Why> {
    let path = dir.path().join(FILE);
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Why::Unwritable),
    };
    if !meta.file_type().is_file()
        || meta.permissions().mode() & 0o077 != 0
        || rustix::process::getuid().as_raw() != meta.uid()
    {
        return Err(Why::TokenOpen);
    }
    match dir.read(FILE) {
        Ok(Some(secret)) if is_token(secret.expose()) => Ok(Some(secret)),
        Ok(Some(_)) => Err(Why::NotAToken),
        Ok(None) => Ok(None),
        Err(_) => Err(Why::Unwritable),
    }
}

/// The person's runtime folder as root finds it: `/run/user/<uid>`.
#[must_use]
pub fn runtime_of(uid: u32) -> PathBuf {
    PathBuf::from(format!("/run/user/{uid}"))
}

/// This person's runtime folder as root finds it — where [`stage`] puts the
/// copy the sudo door's helper takes.
#[must_use]
pub fn my_runtime() -> PathBuf {
    runtime_of(rustix::process::getuid().as_raw())
}

/// Stages `token` for root's helper in the runtime folder `runtime` —
/// `0600` from its first byte, replacing a copy left there.
///
/// # Errors
///
/// [`Why::NotAToken`]; [`Why::Unwritable`] when the folder is not there or
/// the copy could not be written.
pub fn stage(runtime: &Path, token: &Secret) -> Result<(), Why> {
    if !is_token(token.expose()) {
        return Err(Why::NotAToken);
    }
    if !runtime.is_dir() || token.expose().len() > MOST_BYTES {
        return Err(Why::Unwritable);
    }
    write_private_file(&runtime.join(STAGED), token.expose().as_bytes())
        .map_err(|_| Why::Unwritable)
}

/// Deletes the staged copy, if one is left.
pub fn unstage(runtime: &Path) {
    let _ = std::fs::remove_file(runtime.join(STAGED));
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    const TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJqdGkiOiIxIn0.c2lnbmF0dXJl_-";

    fn scratch(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "ronoctua-cachix-token-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    /// FAILS IF: text that a netrc or an environment would read as more
    /// than a token is taken, or a JWT is refused.
    #[test]
    fn a_token_is_a_jwts_characters_and_nothing_else() {
        assert!(is_token(TOKEN));
        for bad in [
            "",
            "short",
            "has space in it but long enough",
            "line\nbreak.aaaaaaaaaaaaaaaa",
            "quote\"aaaaaaaaaaaaaaaaaaaa",
            "machine x.cachix.org password y",
        ] {
            assert!(!is_token(bad), "{bad:?}");
        }
        assert!(!is_token(&"a".repeat(MOST_TOKEN_BYTES + 1)));
    }

    /// FAILS IF: the token is kept with bits for anybody else, a token
    /// file somebody opened up is read, or forgetting leaves it there.
    #[test]
    fn the_token_is_private_from_its_first_byte_and_an_open_one_is_refused() {
        let root = scratch("keep");
        let dir = Dir::at(root.join(DIR));
        assert_eq!(read_from(&dir), Ok(None));
        assert!(!is_kept(&dir));
        assert_eq!(keep(&dir, &Secret::new("nope")), Err(Why::NotAToken));
        keep(&dir, &Secret::new(format!("  {TOKEN}\n"))).unwrap();
        let mode = std::fs::metadata(dir.path().join(FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert!(is_kept(&dir));
        assert_eq!(read_from(&dir).unwrap().unwrap().expose(), TOKEN);
        std::fs::set_permissions(
            dir.path().join(FILE),
            std::fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert_eq!(read_from(&dir).map(|s| s.is_some()), Err(Why::TokenOpen));
        forget(&dir).unwrap();
        assert!(!is_kept(&dir));
        forget(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the staged copy is born readable, or survives `unstage`.
    #[test]
    fn the_staged_copy_is_private_and_goes() {
        let root = scratch("stage");
        stage(&root, &Secret::new(TOKEN)).unwrap();
        let staged = root.join(STAGED);
        assert_eq!(
            std::fs::metadata(&staged).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(std::fs::read_to_string(&staged).unwrap(), TOKEN);
        unstage(&root);
        assert!(!staged.exists());
        assert_eq!(
            stage(&root.join("absent"), &Secret::new(TOKEN)),
            Err(Why::Unwritable)
        );
        assert_eq!(stage(&root, &Secret::new("x y")), Err(Why::NotAToken));
        let _ = std::fs::remove_dir_all(&root);
    }
}
