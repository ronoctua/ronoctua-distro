//! What a key, a token or a password is kept in.
//!
//! # Private from the first byte
//!
//! A folder is `0700` and every file in it is `0600` — and `0600` from its
//! CREATION, not after it: a file written and then chmodded holds the secret
//! in a readable file for as long as the two calls are apart. A file is
//! written into a draft opened with the mode already on it (`create_new` +
//! `mode`), synced, and renamed over the old one, so a reader sees the whole
//! old file or the whole new one.
//!
//! # Never printed, and wiped
//!
//! A [`Secret`]'s `Debug` says only that it is one, and it has no `Display`
//! and no `Serialize`: it cannot reach a log line, a `State` or a wire by
//! accident. Its bytes are overwritten with zeros when it is dropped. It
//! travels in a header, a stdin or a `0600` file — never in argv, and never
//! in a query string (SubDL is the one exception the service leaves, and the
//! screen says so).
//!
//! # Where these came from
//!
//! `ronoctua-lookup/src/keys.rs` wrote them first, for the player's keys;
//! they are here so every crate with a key (words, websearch, the
//! belongings, PAM, the sudo door) keeps it the same way. The lookup crate
//! re-exports [`Secret`], [`write_private_file`] and [`Dir`], so the player's
//! paths (`ronoctua_lookup::keys::Secret`) still name them.

use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use zeroize::Zeroize as _;

/// The longest secret [`Dir::read`] accepts, in bytes: a key, a token, a
/// passphrase or an account's two lines. A bound, not a guess — a file larger
/// than this is not a secret somebody typed.
pub const MOST_BYTES: usize = 16 * 1024;

/// Text that must never be printed: a key, a token, a password.
///
/// `Debug` names the type and nothing else; there is no `Display` and no
/// `Serialize`. Read it with [`Secret::expose`], at the one place it is put
/// into a header or a pipe. Its bytes are zeroed when it is dropped (and every
/// clone's, when that one is).
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Text to keep secret.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    /// The text itself — for the header or the pipe it goes into, and nowhere
    /// else.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Whether it is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// A folder of private files: `0700`, each file `0600`.
///
/// It only names the folder; [`Dir::make`] creates it (or tightens one that
/// was there), and the three verbs below make it first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dir(PathBuf);

impl Dir {
    /// The folder at `path`.
    #[must_use]
    pub const fn at(path: PathBuf) -> Self {
        Self(path)
    }

    /// `<config root>/<name>` — `words-keys`, `player-keys` — or `None` with
    /// no home. Per instance: the `.#dev` loop keeps keys of its own.
    #[must_use]
    pub fn in_config(name: &str) -> Option<Self> {
        ronoctua_store::config_dir().map(|d| Self(d.join(name)))
    }

    /// Where it is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Creates the folder `0700`, or tightens it to `0700` when it was there
    /// (a folder somebody made by hand may be `0755`).
    ///
    /// # Errors
    ///
    /// The folder or its parent could not be created or changed.
    pub fn make(&self) -> std::io::Result<()> {
        if let Some(parent) = self.0.parent() {
            std::fs::create_dir_all(parent)?;
        }
        match std::fs::DirBuilder::new().mode(0o700).create(&self.0) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o700))
    }

    /// Keeps `secret` as `file`, replacing what was there.
    ///
    /// # Errors
    ///
    /// The folder could not be made, or the file written.
    pub fn keep(&self, file: &str, secret: &Secret) -> std::io::Result<()> {
        self.make()?;
        write_private_file(
            &self.0.join(checked_name(file)?),
            secret.expose().as_bytes(),
        )
    }

    /// The secret kept as `file`, trimmed; `None` when there is none (or it is
    /// empty).
    ///
    /// # Errors
    ///
    /// The file is there and could not be read, is not UTF-8, or is larger
    /// than [`MOST_BYTES`] (`InvalidData`).
    pub fn read(&self, file: &str) -> std::io::Result<Option<Secret>> {
        let mut bytes = match std::fs::read(self.0.join(checked_name(file)?)) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e),
        };
        if bytes.len() > MOST_BYTES {
            bytes.zeroize();
            return Err(std::io::ErrorKind::InvalidData.into());
        }
        let mut text = match String::from_utf8(bytes) {
            Ok(t) => t,
            Err(e) => {
                e.into_bytes().zeroize();
                return Err(std::io::ErrorKind::InvalidData.into());
            }
        };
        let kept = text.trim();
        let secret = (!kept.is_empty()).then(|| Secret::new(kept));
        text.zeroize();
        Ok(secret)
    }

    /// Forgets `file`. Forgetting one that was never kept is not a failure.
    ///
    /// # Errors
    ///
    /// The file is there and could not be removed.
    pub fn forget(&self, file: &str) -> std::io::Result<()> {
        match std::fs::remove_file(self.0.join(checked_name(file)?)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }
}

/// A file's name inside a [`Dir`]: one component, never a path, never hidden
/// (a leading dot is what a draft is named).
fn checked_name(file: &str) -> std::io::Result<&str> {
    let fits = !file.is_empty() && !file.starts_with('.') && !file.contains('/');
    if fits {
        Ok(file)
    } else {
        Err(std::io::ErrorKind::InvalidInput.into())
    }
}

/// A draft beside `file`, created `0600` — before its first byte.
fn open_draft(dir: &Path, file: &str) -> std::io::Result<(PathBuf, std::fs::File)> {
    let draft = dir.join(format!(".{file}.{}.draft", std::process::id()));
    let open = || {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&draft)
    };
    let handle = match open() {
        Ok(f) => f,
        // A draft left by a process that died with this pid: it is ours to
        // replace, and `create_new` still refuses to write into it.
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            std::fs::remove_file(&draft)?;
            open()?
        }
        Err(e) => return Err(e),
    };
    Ok((draft, handle))
}

/// `bytes` into the file at `path`, private from its FIRST byte: a draft
/// beside it opened `0600` (`create_new` with the mode on the creation, never
/// a `chmod` after the bytes are in), written, synced and renamed over it. A
/// reader sees the whole old file or the whole new one, and a file somebody
/// left readable is replaced by one that never was.
///
/// The folder must exist, and its mode is its own ([`Dir::make`] makes one
/// `0700`).
///
/// # Errors
///
/// The draft could not be created, written, synced or renamed; no draft is
/// left behind.
pub fn write_private_file(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
        return Err(std::io::ErrorKind::InvalidInput.into());
    };
    let (draft, mut handle) = open_draft(dir, file)?;
    let written = handle
        .write_all(bytes)
        .and_then(|()| handle.sync_all())
        .and_then(|()| std::fs::rename(&draft, path));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&draft);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt as _;

    use super::*;

    #[test]
    fn a_secret_never_prints_and_a_key_is_private_from_its_first_byte() {
        let secret = Secret::new("hunter2");
        assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
        assert_eq!(secret.expose(), "hunter2");

        let root =
            std::env::temp_dir().join(format!("ronoctua-consent-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = Dir::at(root.join("words-keys"));
        assert_eq!(dir.read("deepl").unwrap(), None);
        dir.keep("deepl", &Secret::new("  abc:fx \n")).unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().mode() & 0o777;
        assert_eq!(mode(dir.path()), 0o700, "the folder");
        assert_eq!(mode(&dir.path().join("deepl")), 0o600, "the file");
        assert_eq!(dir.read("deepl").unwrap().unwrap().expose(), "abc:fx");
        assert!(
            dir.keep("../escape", &secret).is_err(),
            "one component only"
        );
        assert!(dir.keep(".draft", &secret).is_err(), "never a hidden name");
        dir.forget("deepl").unwrap();
        dir.forget("deepl")
            .expect("forgetting twice is not a failure");
        assert_eq!(dir.read("deepl").unwrap(), None);
        let _ = std::fs::remove_dir_all(root);
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-consent-private-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().mode() & 0o777
    }

    /// THE DEFECT THIS PREVENTS: writing and then chmodding, which leaves the
    /// secret in a readable file between the two calls.
    #[test]
    fn a_draft_is_private_before_its_first_byte() {
        let dir = scratch("draft");
        let (draft, handle) = open_draft(&dir, "tmdb").unwrap();
        assert_eq!(handle.metadata().unwrap().len(), 0, "nothing written yet");
        assert_eq!(mode(&draft), 0o600, "and already private");
        drop(handle);
        // A draft a dead process left with this pid is replaced, not written into.
        let (again, _) = open_draft(&dir, "tmdb").unwrap();
        assert_eq!(again, draft);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a file written through the public writer is readable by
    /// anyone for a moment — or stays readable when it replaced one that was.
    #[test]
    fn a_private_file_is_private_from_its_first_byte_and_replaces_a_readable_one() {
        let dir = scratch("public");
        let file = dir.join("peers.json");
        std::fs::write(&file, "old").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        write_private_file(&file, b"{\"hosts\":[]}").unwrap();
        assert_eq!(mode(&file), 0o600);
        assert_eq!(std::fs::read(&file).unwrap(), b"{\"hosts\":[]}");
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["peers.json"], "no draft is left");
        assert!(write_private_file(Path::new("/"), b"x").is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_folder_made_by_hand_is_tightened_and_a_huge_file_is_not_a_secret() {
        let root = scratch("tight");
        let dir = Dir::at(root.join("keys"));
        std::fs::create_dir_all(dir.path()).unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        dir.make().unwrap();
        assert_eq!(mode(dir.path()), 0o700);
        std::fs::write(dir.path().join("big"), vec![b'k'; MOST_BYTES + 1]).unwrap();
        assert_eq!(
            dir.read("big").unwrap_err().kind(),
            std::io::ErrorKind::InvalidData
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
