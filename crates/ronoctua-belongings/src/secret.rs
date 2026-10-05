//! Private files (feature `prefs`): what a door and the CLI write that nobody
//! else may read — the passphrase's file, the token, the session.
//!
//! BORN PRIVATE: every file here is created `0600` (`create_new` + mode) and
//! renamed into place, never written and then `chmod`ed — a `chmod` after the
//! write leaves it readable between the two calls. What was there is
//! replaced whole.

use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

/// Writes a private file, `0600` from birth, through a draft beside it.
///
/// # Errors
///
/// It could not be written.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        make_private_dir(parent)?;
    }
    let draft = draft_of(path);
    let _ = std::fs::remove_file(&draft);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&draft)?;
    let written = f
        .write_all(bytes)
        .and_then(|()| f.sync_all())
        .and_then(|()| std::fs::rename(&draft, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

/// A folder only its owner enters (made, or narrowed to `0700`).
///
/// # Errors
///
/// It could not be made.
pub fn make_private_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))
}

/// The draft a file is written through, beside it: a dot name, which the
/// walk never keeps (`.*` is a draft).
#[must_use]
pub fn draft_of(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.belongings.{}.tmp", std::process::id()))
}

/// A secret a person typed, held only until it is written: never printed by
/// `Debug`, and its bytes overwritten when it is dropped.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Typed(String);

impl Typed {
    /// Holds `text`.
    #[must_use]
    pub const fn new(text: String) -> Self {
        Self(text)
    }

    /// The text, for the one write that needs it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Typed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Typed(<redacted>)")
    }
}

impl Drop for Typed {
    fn drop(&mut self) {
        let len = self.0.len();
        self.0.clear();
        self.0.extend(std::iter::repeat_n('\0', len));
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt as _;

    use super::*;

    #[test]
    fn a_private_file_is_private_from_its_first_byte_and_replaced_whole() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-belongings-secret-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let file = root.join("run/passphrase");
        write_private(&file, b"first").unwrap();
        write_private(&file, b"second").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"second");
        let meta = std::fs::metadata(&file).unwrap();
        assert_eq!(meta.mode() & 0o777, 0o600);
        assert_eq!(
            std::fs::metadata(root.join("run")).unwrap().mode() & 0o777,
            0o700
        );
        assert!(!draft_of(&file).exists(), "the draft was left behind");
        let typed = Typed::new("hunter2".to_owned());
        assert_eq!(format!("{typed:?}"), "Typed(<redacted>)");
        assert_eq!(typed.expose(), "hunter2");
        let _ = std::fs::remove_dir_all(root);
    }
}
