//! The person's translation keys: `$XDG_CONFIG_HOME/ronoctua/words-keys/`,
//! one `0600` file each, through [`ronoctua_consent::keys::Dir`].
//!
//! A key is a [`Secret`]: it goes into DeepL's `DeepL-Auth-Key` header or
//! LibreTranslate's JSON body, and nowhere else — never argv, never a query
//! string, never a log line.

use ronoctua_consent::keys::{Dir, Secret};
use serde::{Deserialize, Serialize};

/// The folder's name, under the config root.
pub const DIR: &str = "words-keys";

/// One of the person's translation keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Key {
    /// DeepL's authentication key.
    #[serde(rename = "deepl")]
    DeepL,
    /// A LibreTranslate server's API key.
    #[serde(rename = "libretranslate")]
    LibreTranslate,
}

impl Key {
    /// Every key, in the order a settings page lists them.
    pub const ALL: [Self; 2] = [Self::DeepL, Self::LibreTranslate];

    /// Its file's name in [`DIR`] — and its id.
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::DeepL => "deepl",
            Self::LibreTranslate => "libretranslate",
        }
    }
}

/// Why a key was not kept or not read, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyTrouble {
    /// There is no home to keep it in.
    NoHome,
    /// Nothing was typed.
    Empty,
    /// It has a line break or another control character in it: in DeepL's
    /// header it would start a header nobody wrote.
    Malformed,
    /// The file is there and could not be read (or is not a key).
    Unreadable,
    /// The folder or the file could not be written.
    Unwritable,
}

/// The folder, or `None` with no home.
#[must_use]
pub fn dir() -> Option<Dir> {
    Dir::in_config(DIR)
}

/// The key, when the person kept one.
///
/// # Errors
///
/// [`KeyTrouble::NoHome`], [`KeyTrouble::Unreadable`].
pub fn read(key: Key) -> Result<Option<Secret>, KeyTrouble> {
    dir()
        .ok_or(KeyTrouble::NoHome)?
        .read(key.file())
        .map_err(|_| KeyTrouble::Unreadable)
}

/// Keep this key (trimmed), replacing the one kept before.
///
/// # Errors
///
/// [`KeyTrouble::NoHome`], and as [`keep_in`].
pub fn keep(key: Key, secret: &Secret) -> Result<(), KeyTrouble> {
    keep_in(&dir().ok_or(KeyTrouble::NoHome)?, key, secret)
}

/// [`keep`], in a given folder.
///
/// # Errors
///
/// [`KeyTrouble::Empty`], [`KeyTrouble::Malformed`] for what was typed,
/// [`KeyTrouble::Unwritable`].
pub fn keep_in(dir: &Dir, key: Key, secret: &Secret) -> Result<(), KeyTrouble> {
    let typed = secret.expose().trim();
    if typed.is_empty() {
        return Err(KeyTrouble::Empty);
    }
    if typed.chars().any(char::is_control) {
        return Err(KeyTrouble::Malformed);
    }
    dir.keep(key.file(), &Secret::new(typed))
        .map_err(|_| KeyTrouble::Unwritable)
}

/// Forget this key. Forgetting one never kept is not a failure.
///
/// # Errors
///
/// [`KeyTrouble::NoHome`], [`KeyTrouble::Unwritable`].
pub fn forget(key: Key) -> Result<(), KeyTrouble> {
    dir()
        .ok_or(KeyTrouble::NoHome)?
        .forget(key.file())
        .map_err(|_| KeyTrouble::Unwritable)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::MetadataExt as _;

    use super::*;

    /// FAILS IF: a translation key is kept readable by others, or one that
    /// would start a header of its own is kept at all.
    #[test]
    fn a_key_is_kept_private_trimmed_and_never_with_a_line_break() {
        let dir = Dir::at(
            std::env::temp_dir().join(format!("ronoctua-words-keys-{}", std::process::id())),
        );
        let _ = std::fs::remove_dir_all(dir.path());
        keep_in(&dir, Key::DeepL, &Secret::new("  abc:fx\n")).unwrap();
        let file = dir.path().join(Key::DeepL.file());
        assert_eq!(std::fs::metadata(&file).unwrap().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(dir.path()).unwrap().mode() & 0o777, 0o700);
        assert_eq!(dir.read("deepl").unwrap().unwrap().expose(), "abc:fx");
        assert_eq!(
            keep_in(&dir, Key::LibreTranslate, &Secret::new("a\r\nX-Evil: 1")),
            Err(KeyTrouble::Malformed)
        );
        assert_eq!(
            keep_in(&dir, Key::LibreTranslate, &Secret::new("   ")),
            Err(KeyTrouble::Empty)
        );
        assert!(!dir.path().join("libretranslate").exists());
        assert!(
            dir.path()
                .ends_with(format!("ronoctua-words-keys-{}", std::process::id()))
        );
        assert_eq!(DIR, "words-keys");
        let _ = std::fs::remove_dir_all(dir.path());
    }
}
