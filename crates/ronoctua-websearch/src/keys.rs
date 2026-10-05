//! The person's search key: Brave's, `$XDG_CONFIG_HOME/ronoctua/websearch-keys/brave`,
//! one `0600` file through [`ronoctua_consent::keys::Dir`].
//!
//! The key is a [`Secret`]: it goes into Brave's `X-Subscription-Token`
//! header and nowhere else — never argv, never a query string, never a log.

use ronoctua_consent::keys::{Dir, Secret};

use crate::{BRAVE_KEY, KEYS_DIR};

/// Why a key was not kept, as a closed set (`fast-access-web-key-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyTrouble {
    /// There is no home to keep it in.
    NoHome,
    /// Nothing was typed.
    Empty,
    /// It has a line break or another control character in it: in a header
    /// it would start a header nobody wrote.
    Malformed,
    /// The folder or the file could not be written.
    Unwritable,
}

impl KeyTrouble {
    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoHome => "no-home",
            Self::Empty => "empty",
            Self::Malformed => "malformed",
            Self::Unwritable => "unwritable",
        }
    }
}

/// The folder, or `None` with no home.
#[must_use]
pub fn dir() -> Option<Dir> {
    Dir::in_config(KEYS_DIR)
}

/// Whether the person kept a Brave key — asked by a door to draw the row
/// that searches with it, never to read the key.
#[must_use]
pub fn has_brave() -> bool {
    dir().is_some_and(|d| d.read(BRAVE_KEY).ok().flatten().is_some())
}

/// Keeps the Brave key (trimmed), replacing the one kept before.
///
/// # Errors
///
/// [`KeyTrouble::NoHome`], and as [`keep_in`].
pub fn keep_brave(secret: &Secret) -> Result<(), KeyTrouble> {
    keep_in(&dir().ok_or(KeyTrouble::NoHome)?, secret)
}

/// [`keep_brave`], in a given folder.
///
/// # Errors
///
/// [`KeyTrouble::Empty`], [`KeyTrouble::Malformed`] for what was typed,
/// [`KeyTrouble::Unwritable`].
pub fn keep_in(dir: &Dir, secret: &Secret) -> Result<(), KeyTrouble> {
    let typed = secret.expose().trim();
    if typed.is_empty() {
        return Err(KeyTrouble::Empty);
    }
    if typed.chars().any(char::is_control) {
        return Err(KeyTrouble::Malformed);
    }
    dir.keep(BRAVE_KEY, &Secret::new(typed))
        .map_err(|_| KeyTrouble::Unwritable)
}

/// Forgets the Brave key. Forgetting one never kept is not a failure.
///
/// # Errors
///
/// [`KeyTrouble::NoHome`], [`KeyTrouble::Unwritable`].
pub fn forget_brave() -> Result<(), KeyTrouble> {
    dir()
        .ok_or(KeyTrouble::NoHome)?
        .forget(BRAVE_KEY)
        .map_err(|_| KeyTrouble::Unwritable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt as _;

    #[test]
    fn a_key_is_kept_private_and_a_broken_one_refused() {
        let dir = Dir::at(
            std::env::temp_dir().join(format!("ronoctua-websearch-keep-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(dir.path());
        assert_eq!(keep_in(&dir, &Secret::new("  ")), Err(KeyTrouble::Empty));
        assert_eq!(
            keep_in(&dir, &Secret::new("a\nb")),
            Err(KeyTrouble::Malformed)
        );
        keep_in(&dir, &Secret::new(" k3y ")).unwrap();
        let file = dir.path().join(BRAVE_KEY);
        let mode = std::fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        assert_eq!(dir.read(BRAVE_KEY).unwrap().unwrap().expose(), "k3y");
        let _ = std::fs::remove_dir_all(dir.path());
    }
}
