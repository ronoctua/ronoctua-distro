//! The person's keys: `$XDG_CONFIG_HOME/ronoctua/player-keys/`, one file each
//! — the player's, and the security centre's VirusTotal key, in the same
//! folder (it keeps its name: renaming it would lose every key kept) and told
//! apart by [`Key::scope`].
//!
//! # Private from the first byte
//!
//! The folder is `0700` and every file is `0600` — and `0600` from its
//! CREATION, not after it: a file written and then chmodded holds the key in
//! a readable file for as long as the two calls are apart (the defect of the
//! player's `remote::keep`, which this does not copy). A key is written into
//! a draft opened with the mode already on it, synced, and renamed over the
//! old one, so a reader sees the whole old key or the whole new one, and a
//! key written over a file somebody left readable leaves no readable copy.
//!
//! # Never printed
//!
//! A key is a [`Secret`], whose `Debug` says only that it is one and which
//! has no `Display` and no `Serialize`: it cannot reach a log line, the
//! player's `State` or the wire by accident. It travels in a header (TMDB's
//! token as `Bearer`, OpenSubtitles' as `Api-Key`) — never in argv, and never
//! in a query string, with one exception the service leaves no way around:
//! SubDL takes its key only as `api_key=` (see [`crate::subdl`]).
//!
//! # Per instance
//!
//! The folder is under [`ronoctua_xdg::config_dir`], so the `.#dev` loop keeps
//! keys of its own and never reads the installed session's.
//!
//! # Where the private files live now
//!
//! [`Secret`], [`write_private_file`] and the `0700` folder ([`Dir`]) were
//! written here first; they moved to `ronoctua_consent::keys` so every crate
//! with a key keeps it the same way, and are re-exported here so the player's
//! paths still name them. What stays is the player's own: its [`Key`]s, their
//! checks and their refusals.

use std::path::{Path, PathBuf};

pub use ronoctua_consent::keys::{Dir, Secret, write_private_file};

use serde::{Deserialize, Serialize};

use crate::{Refusal, Why};

/// The folder's name, under the config root.
pub const DIR: &str = "player-keys";

/// The longest key, token or password kept. OpenSubtitles' and SubDL's keys
/// are 32 characters and TMDB's read token about 250; four kibibytes is a
/// bound and not a guess.
pub const MOST_BYTES: usize = 4096;

/// One of the person's keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    /// OpenSubtitles' API key (`Api-Key`).
    #[serde(rename = "opensubtitles")]
    OpenSubtitles,
    /// An OpenSubtitles account: the user name, a line break, the password.
    /// Optional: it raises the day's downloads from what the key alone gets.
    #[serde(rename = "opensubtitles-account")]
    OpenSubtitlesAccount,
    /// TMDB's API read access token (`Authorization: Bearer`).
    #[serde(rename = "tmdb")]
    Tmdb,
    /// SubDL's API key.
    #[serde(rename = "subdl")]
    SubDl,
    /// VirusTotal's API key (`x-apikey`) — the security centre's.
    #[serde(rename = "virustotal")]
    VirusTotal,
}

impl Key {
    /// Every key, the player's in the order its Internet page lists them,
    /// then the security centre's. A page lists its own ([`Key::scope`]).
    pub const ALL: [Self; 5] = [
        Self::OpenSubtitles,
        Self::OpenSubtitlesAccount,
        Self::Tmdb,
        Self::SubDl,
        Self::VirusTotal,
    ];

    /// Whose it is — which page lists it.
    #[must_use]
    pub const fn scope(self) -> crate::Scope {
        self.service().scope()
    }

    /// Its file's name in [`DIR`] — and its id.
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Self::OpenSubtitles => "opensubtitles",
            Self::OpenSubtitlesAccount => "opensubtitles-account",
            Self::Tmdb => "tmdb",
            Self::SubDl => "subdl",
            Self::VirusTotal => "virustotal",
        }
    }

    /// The service it opens.
    #[must_use]
    pub const fn service(self) -> crate::Service {
        match self {
            Self::OpenSubtitles | Self::OpenSubtitlesAccount => crate::Service::OpenSubtitles,
            Self::Tmdb => crate::Service::Tmdb,
            Self::SubDl => crate::Service::SubDl,
            Self::VirusTotal => crate::Service::VirusTotal,
        }
    }
}

/// Why a key was not kept or not read, as a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeyTrouble {
    /// Neither `XDG_CONFIG_HOME` nor `HOME` is set: there is no home to keep
    /// it in.
    NoHome,
    /// Nothing was typed.
    Empty,
    /// It has a line break or another control character in it — it would
    /// start a header nobody wrote — or an account without its two lines.
    Malformed,
    /// Longer than [`MOST_BYTES`].
    TooLong,
    /// The folder or the file could not be written.
    Unwritable,
    /// The file is there and could not be read.
    Unreadable,
}

/// An account: a user name and its password.
#[derive(Clone, PartialEq, Eq)]
pub struct Account {
    /// The user name.
    pub user: String,
    /// The password.
    pub password: Secret,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The user name is the person's too: a log line needs neither.
        f.write_str("Account(<redacted>)")
    }
}

/// `<config>/player-keys`, or `None` with no home.
#[must_use]
pub fn dir() -> Option<PathBuf> {
    ronoctua_xdg::config_dir().map(|d| d.join(DIR))
}

/// Whether the person has kept this key.
#[must_use]
pub fn has(key: Key) -> bool {
    dir().is_some_and(|d| has_in(&d, key))
}

/// The key, when the person kept one.
///
/// # Errors
///
/// [`KeyTrouble::NoHome`], or [`KeyTrouble::Unreadable`] /
/// [`KeyTrouble::Malformed`] for a file that is there and cannot be used.
pub fn read(key: Key) -> Result<Option<Secret>, KeyTrouble> {
    read_in(&dir().ok_or(KeyTrouble::NoHome)?, key)
}

/// The key a service needs, or the refusal a lookup gives without it —
/// [`Why::NoKey`] for no key (or no home), [`Why::KeyRefused`] for a file
/// that cannot be used.
///
/// # Errors
///
/// As above.
pub fn needed(key: Key) -> Result<Secret, Refusal> {
    let refused = |why| Refusal::new(key.service(), why);
    match read(key) {
        Ok(Some(secret)) => Ok(secret),
        Ok(None) | Err(KeyTrouble::NoHome) => Err(refused(Why::NoKey)),
        Err(_) => Err(refused(Why::KeyRefused)),
    }
}

/// Keep this key, replacing the one kept before.
///
/// # Errors
///
/// [`KeyTrouble::Empty`], [`KeyTrouble::Malformed`], [`KeyTrouble::TooLong`]
/// for what was typed, [`KeyTrouble::NoHome`], [`KeyTrouble::Unwritable`].
pub fn keep(key: Key, secret: &Secret) -> Result<(), KeyTrouble> {
    keep_in(&dir().ok_or(KeyTrouble::NoHome)?, key, secret)
}

/// Forget this key. Forgetting one that was never kept is not a failure.
///
/// # Errors
///
/// [`KeyTrouble::NoHome`], [`KeyTrouble::Unwritable`].
pub fn forget(key: Key) -> Result<(), KeyTrouble> {
    forget_in(&dir().ok_or(KeyTrouble::NoHome)?, key)
}

/// The OpenSubtitles account, when the person kept one.
///
/// # Errors
///
/// As [`read`].
pub fn account() -> Result<Option<Account>, KeyTrouble> {
    account_in(&dir().ok_or(KeyTrouble::NoHome)?)
}

/// Keep the OpenSubtitles account.
///
/// # Errors
///
/// As [`keep`]; a user name with a line break in it is
/// [`KeyTrouble::Malformed`].
pub fn keep_account(account: &Account) -> Result<(), KeyTrouble> {
    keep_account_in(&dir().ok_or(KeyTrouble::NoHome)?, account)
}

/// A key as typed, trimmed and checked. Pure.
///
/// # Errors
///
/// [`KeyTrouble::Empty`], [`KeyTrouble::TooLong`], [`KeyTrouble::Malformed`].
pub fn checked(text: &str) -> Result<&str, KeyTrouble> {
    let text = text.trim();
    if text.is_empty() {
        return Err(KeyTrouble::Empty);
    }
    if text.len() > MOST_BYTES {
        return Err(KeyTrouble::TooLong);
    }
    if text.chars().any(char::is_control) {
        return Err(KeyTrouble::Malformed);
    }
    Ok(text)
}

fn has_in(dir: &Path, key: Key) -> bool {
    matches!(read_in(dir, key), Ok(Some(_)))
}

fn read_in(dir: &Path, key: Key) -> Result<Option<Secret>, KeyTrouble> {
    let Some(text) = read_file(&dir.join(key.file()))? else {
        return Ok(None);
    };
    if key == Key::OpenSubtitlesAccount {
        // The account's file is two lines; `read` hands it back whole, and
        // `account` splits it.
        return Ok(Some(Secret::new(text)));
    }
    Ok(Some(Secret::new(
        checked(&text).map_err(|_| KeyTrouble::Malformed)?,
    )))
}

/// A key file's text; `None` when there is none (or it is empty).
fn read_file(path: &Path) -> Result<Option<String>, KeyTrouble> {
    match std::fs::read(path) {
        Ok(bytes) if bytes.len() > MOST_BYTES * 2 + 2 => Err(KeyTrouble::Malformed),
        Ok(bytes) => {
            let text = String::from_utf8(bytes).map_err(|_| KeyTrouble::Malformed)?;
            Ok((!text.trim().is_empty()).then_some(text))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(KeyTrouble::Unreadable),
    }
}

fn keep_in(dir: &Path, key: Key, secret: &Secret) -> Result<(), KeyTrouble> {
    if key == Key::OpenSubtitlesAccount {
        // An account goes through `keep_account`, which checks both lines.
        return Err(KeyTrouble::Malformed);
    }
    let text = checked(secret.expose())?;
    write_private(dir, key.file(), text.as_bytes())
}

fn forget_in(dir: &Path, key: Key) -> Result<(), KeyTrouble> {
    match std::fs::remove_file(dir.join(key.file())) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(KeyTrouble::Unwritable),
    }
}

fn account_in(dir: &Path) -> Result<Option<Account>, KeyTrouble> {
    let Some(text) = read_file(&dir.join(Key::OpenSubtitlesAccount.file()))? else {
        return Ok(None);
    };
    let mut lines = text.lines();
    let user = checked(lines.next().unwrap_or_default()).map_err(|_| KeyTrouble::Malformed)?;
    let password = lines.next().unwrap_or_default();
    // The password is kept as typed (spaces are a password's to have); only a
    // line break or nothing is refused.
    if password.is_empty() || password.chars().any(char::is_control) {
        return Err(KeyTrouble::Malformed);
    }
    Ok(Some(Account {
        user: user.to_owned(),
        password: Secret::new(password),
    }))
}

fn keep_account_in(dir: &Path, account: &Account) -> Result<(), KeyTrouble> {
    let user = checked(&account.user)?;
    let password = account.password.expose();
    if password.is_empty() {
        return Err(KeyTrouble::Empty);
    }
    if password.len() > MOST_BYTES {
        return Err(KeyTrouble::TooLong);
    }
    if password.chars().any(char::is_control) {
        return Err(KeyTrouble::Malformed);
    }
    let text = format!("{user}\n{password}\n");
    write_private(dir, Key::OpenSubtitlesAccount.file(), text.as_bytes())
}

/// The folder, `0700`: created so, or tightened to it when it was there
/// ([`Dir::make`]).
fn private_dir(dir: &Path) -> Result<(), KeyTrouble> {
    Dir::at(dir.to_path_buf())
        .make()
        .map_err(|_| KeyTrouble::Unwritable)
}

/// `bytes` into `dir/file`, private from its first byte: the folder `0700`,
/// a draft opened `0600`, written, synced and renamed over the file.
fn write_private(dir: &Path, file: &str, bytes: &[u8]) -> Result<(), KeyTrouble> {
    private_dir(dir)?;
    write_private_file(&dir.join(file), bytes).map_err(|_| KeyTrouble::Unwritable)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};

    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-lookup-keys-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join(DIR)
    }

    fn mode(path: &Path) -> u32 {
        std::fs::metadata(path).unwrap().mode() & 0o777
    }

    #[test]
    fn a_key_is_kept_private_and_read_back() {
        let dir = scratch("keep");
        assert!(!has_in(&dir, Key::Tmdb));
        keep_in(&dir, Key::Tmdb, &Secret::new("  eyJhbGciOi.token \n")).unwrap();
        assert!(has_in(&dir, Key::Tmdb));
        assert_eq!(mode(&dir), 0o700, "the folder");
        assert_eq!(mode(&dir.join("tmdb")), 0o600, "the file");
        assert_eq!(
            read_in(&dir, Key::Tmdb).unwrap().unwrap().expose(),
            "eyJhbGciOi.token",
            "trimmed"
        );
        // No draft is left behind.
        let left: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, ["tmdb"]);
        forget_in(&dir, Key::Tmdb).unwrap();
        assert!(!has_in(&dir, Key::Tmdb));
        forget_in(&dir, Key::Tmdb).expect("forgetting twice is not a failure");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn a_key_written_over_a_readable_file_leaves_no_readable_copy() {
        let dir = scratch("over");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        let file = dir.join("subdl");
        std::fs::write(&file, "old").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        keep_in(&dir, Key::SubDl, &Secret::new("new-key")).unwrap();
        assert_eq!(mode(&file), 0o600);
        assert_eq!(mode(&dir), 0o700, "a folder made by hand is tightened");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "new-key");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn a_key_that_would_start_a_header_is_refused() {
        let dir = scratch("refused");
        assert_eq!(
            keep_in(&dir, Key::Tmdb, &Secret::new("abc\r\nX-Injected: 1")),
            Err(KeyTrouble::Malformed)
        );
        assert_eq!(
            keep_in(&dir, Key::Tmdb, &Secret::new("   ")),
            Err(KeyTrouble::Empty)
        );
        assert_eq!(
            keep_in(&dir, Key::Tmdb, &Secret::new("k".repeat(MOST_BYTES + 1))),
            Err(KeyTrouble::TooLong)
        );
        assert!(!dir.join("tmdb").exists(), "nothing refused was written");
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn an_account_is_two_lines_and_private() {
        let dir = scratch("account");
        assert_eq!(account_in(&dir).unwrap(), None);
        let a = Account {
            user: "someone".into(),
            password: Secret::new("pass word"),
        };
        keep_account_in(&dir, &a).unwrap();
        assert_eq!(mode(&dir.join("opensubtitles-account")), 0o600);
        assert_eq!(account_in(&dir).unwrap(), Some(a));
        let bad = Account {
            user: "a\nb".into(),
            password: Secret::new("p"),
        };
        assert_eq!(keep_account_in(&dir, &bad), Err(KeyTrouble::Malformed));
        assert_eq!(
            keep_in(&dir, Key::OpenSubtitlesAccount, &Secret::new("x")),
            Err(KeyTrouble::Malformed),
            "an account goes through keep_account"
        );
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn a_secret_never_prints() {
        let s = Secret::new("the-key-itself");
        assert_eq!(format!("{s:?}"), "Secret(<redacted>)");
        let a = Account {
            user: "someone".into(),
            password: Secret::new("hunter2"),
        };
        let shown = format!("{a:?}");
        assert!(
            !shown.contains("hunter2") && !shown.contains("someone"),
            "{shown}"
        );
    }

    #[test]
    fn a_missing_key_is_no_key_and_the_folder_is_the_instances() {
        let dir = scratch("missing");
        assert_eq!(read_in(&dir, Key::OpenSubtitles), Ok(None));
        assert!(dir.ends_with("player-keys"));
        for key in Key::ALL {
            assert_eq!(
                serde_json::to_string(&key).unwrap(),
                format!("\"{}\"", key.file())
            );
        }
    }
}
