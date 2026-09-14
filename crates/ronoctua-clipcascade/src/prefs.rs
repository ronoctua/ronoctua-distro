//! What this machine remembers about ClipCascade — and what it refuses to.
//!
//! # The password is never here
//!
//! What is kept is the DERIVED KEY, which is what the protocol actually uses:
//! the password goes into `Key::derive` once, at login, and nothing keeps it
//! afterwards. A key is not reversible into a password, so a person who reads
//! this file learns how to decrypt this account's clipboard and not how to log
//! into the server, change the password or reach anything else.
//!
//! It lives in `$XDG_STATE_HOME/ronoctua` at `0600` — the shape aria2's RPC
//! secret already has, and for the same reason its module gives: a file in a
//! directory nobody else may enter is the difference between a secret and a
//! secret-shaped string.
//!
//! **Nothing about it is in this repository** (invariant 13), and
//! `nix/scripts/scan-secrets.py` sweeps for the shape.
//!
//! # Why the address is a preference and not a Nix option
//!
//! Because it names somebody's machine. `nix/modules/secrets.nix` is explicit
//! that what is personal does not enter this repository even encrypted, and a
//! server address is as personal as an account. The module offers to RUN a
//! server; it does not say where anyone's is.

use serde::{Deserialize, Serialize};

/// What the file is called.
pub const FILE: &str = "clipcascade.json";

/// What the key file is called.
///
/// SEPARATE FROM THE REST, and deliberately: the preferences are ordinary JSON
/// a person may read and edit, and the key is thirty-two bytes at `0600`.
/// Putting it inside would make the whole file a secret, and then every
/// harmless setting in it would be one too.
pub const KEY_FILE: &str = "clipcascade.key";

/// What is remembered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Preferences {
    /// Should the clipboard sync at all?
    ///
    /// OFF BY DEFAULT. A clipboard that started crossing the network because
    /// a package was installed would be the opposite of what
    /// `docs/PENDING.md` spent a decision gate on.
    pub enabled: bool,
    /// Where the server is. Empty until somebody says.
    pub server: String,
    /// Who this machine logs in as.
    pub username: String,
    /// Should images cross the network, as well as text?
    pub images: bool,
    /// Should a notification be raised for what arrives?
    pub notify: bool,
    /// The largest entry that is worth sending, in bytes.
    ///
    /// Zero means the server's own limit, which it announces at `/max-size`.
    pub cap: u64,
    /// The salt that goes into the key, which is EVERY DEVICE'S and not the
    /// server's.
    ///
    /// Measured against the upstream: it is typed at login, it defaults to the
    /// empty string, and every machine on one installation must be given the
    /// same one or they derive different keys and read each other as noise.
    /// No endpoint serves it. See [`crate::cipher::Key::derive`].
    pub salt: String,
    /// How many PBKDF2 rounds, likewise the person's.
    pub rounds: u32,
    /// Which modeline mark to draw, if any.
    pub mark: Mark,
}

impl Default for Preferences {
    /// THE ROUNDS ARE NOT ZERO. Every other field's `Default` is the right
    /// answer for a machine nobody has set up, and this one's is the
    /// upstream's number: a file written without it, or one from an older
    /// version of this program, would otherwise derive a key against zero
    /// rounds and decrypt nothing — with an empty clipboard as the only
    /// symptom.
    fn default() -> Self {
        Self {
            enabled: false,
            server: String::new(),
            username: String::new(),
            images: false,
            notify: false,
            cap: 0,
            salt: String::new(),
            rounds: crate::cipher::ROUNDS,
            mark: Mark::default(),
        }
    }
}

/// What a door may change without touching the account.
///
/// SIX FIELDS AND NOT THE WHOLE RECORD: a door that sent its whole
/// [`Preferences`] back would carry the `server` and the `username` it read
/// some time ago, and a login that happened in between would be undone by
/// somebody turning a switch. It is the `Eyes` merge of `display.json`, in the
/// smallest form that says the same thing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// The salt that goes into the key.
    pub salt: String,
    /// How many PBKDF2 rounds.
    pub rounds: u32,
    /// Should images cross the network, as well as text?
    pub images: bool,
    /// Should what arrives raise a notification?
    pub notify: bool,
    /// The largest entry this machine will send, in bytes; zero is the
    /// server's own limit.
    pub cap: u64,
    /// Which modeline mark to draw, if any.
    pub mark: Mark,
}

/// When the modeline says something about the clipboard.
///
/// The same ring the Syncthing view carries, and for the same reason: the
/// owner chose the three, and `WhenOff` is the one that keeps the modeline
/// silent when everything is working.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mark {
    /// Never draw one.
    Never,
    /// Draw one when the clipboard is NOT syncing — off, or the server is
    /// unreachable. From the other side of the screen those are one fact.
    #[default]
    WhenOff,
    /// Draw one while it IS syncing.
    WhenOn,
}

impl Mark {
    /// The id, which is the stem of the text key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Never => "never",
            Self::WhenOff => "when-off",
            Self::WhenOn => "when-on",
        }
    }

    /// The ring, in the order it turns.
    pub const ALL: [Self; 3] = [Self::Never, Self::WhenOff, Self::WhenOn];

    /// Should a mark be drawn?
    ///
    /// A MACHINE THAT WAS NEVER SET UP LIGHTS NOTHING, whatever the ring says.
    /// It is the Syncthing mark's own rule, in its words: a mark that is lit on
    /// the first boot does not mean anything by the second day. `WhenOff` is
    /// about a sync that STOPPED, not about one that was never asked for.
    #[must_use]
    pub const fn shows(self, configured: bool, syncing: bool) -> bool {
        if !configured {
            return false;
        }
        match self {
            Self::Never => false,
            Self::WhenOff => !syncing,
            Self::WhenOn => syncing,
        }
    }
}

/// Where the preferences live.
#[must_use]
pub fn store() -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(ronoctua_store::config_dir(), FILE)
}

/// What is on disk, or the default.
#[must_use]
pub fn read() -> Preferences {
    store().read()
}

/// Writes them back.
///
/// # Errors
///
/// The file could not be written, or the one on disk will not parse — in which
/// case it is LEFT ALONE. The default is the right answer for a reader and
/// destruction for a writer; a `display.json` is what taught this.
pub fn write(prefs: &Preferences) -> std::io::Result<()> {
    let current = store();
    if current.path().is_some_and(|p| p.exists()) && current.parsed().is_none() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("{FILE} does not parse; it was left alone"),
        ));
    }
    current.write(prefs)
}

/// Where the derived key lives — `$XDG_STATE_HOME/ronoctua`.
///
/// STATE AND NOT CONFIG, because it is not a choice: it is what happened when
/// somebody logged in, and it is worthless on another machine.
#[must_use]
pub fn key_path() -> Option<std::path::PathBuf> {
    ronoctua_store::state_dir().map(|d| d.join(KEY_FILE))
}

/// The stored key, if this machine has logged in.
#[must_use]
pub fn key() -> Option<crate::cipher::Key> {
    let bytes = std::fs::read(key_path()?).ok()?;
    let sized: [u8; crate::cipher::KEY_LEN] = bytes.try_into().ok()?;
    Some(crate::cipher::Key::of_bytes(sized))
}

/// Writes the key, at `0600`.
///
/// # Errors
///
/// There is no state directory, or the file could not be written.
pub fn keep_key(key: &crate::cipher::Key) -> std::io::Result<()> {
    let path = key_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "there is no state directory to keep the key in",
        )
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // THE MODE IS SET BEFORE THE BYTES GO IN. A create-then-chmod writes the
    // key with the umask's permissions and only then narrows them, and the
    // secret is already in the file by that point — the lesson
    // `nix/modules/aria2.nix` states about `install -m600` versus a redirect.
    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(key.bytes())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, key.bytes())
    }
}

/// Forgets the key — what logging out means on this side.
pub fn forget_key() {
    if let Some(path) = key_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clipboard_does_not_cross_the_network_by_itself() {
        // A CLIPBOARD THAT STARTED SYNCING because a package was installed
        // would be the opposite of what `docs/PENDING.md` spent a decision
        // gate on. The owner turns it on; nothing else does.
        let fresh = Preferences::default();
        assert!(!fresh.enabled);
        assert!(fresh.server.is_empty());
        assert!(fresh.username.is_empty());
    }

    #[test]
    fn the_mark_is_silent_when_everything_is_working() {
        // THE RULE OF THE HOUSE: a modeline at rest says nothing. `WhenOff` is
        // the default, so a machine whose sync is up draws no mark at all.
        assert_eq!(Mark::default(), Mark::WhenOff);
        assert!(!Mark::default().shows(true, true));
        assert!(Mark::default().shows(true, false));
        assert!(!Mark::Never.shows(true, true));
        assert!(!Mark::Never.shows(true, false));
        assert!(Mark::WhenOn.shows(true, true));
        assert!(!Mark::WhenOn.shows(true, false));
        // AND A MACHINE THAT WAS NEVER SET UP LIGHTS NOTHING, whatever the
        // ring says: a mark lit on the first boot means nothing by the second
        // day. It is the Syncthing mark's rule, in its own words.
        for mark in Mark::ALL {
            assert!(
                !mark.shows(false, false),
                "{mark:?} lit with nothing set up"
            );
            assert!(!mark.shows(false, true), "{mark:?} lit with nothing set up");
        }
        // AND EVERY VALUE HAS ITS OWN WORD, because the ring says which is
        // chosen and two that shared an id would be one.
        let ids: std::collections::BTreeSet<_> = Mark::ALL.iter().map(|m| m.id()).collect();
        assert_eq!(ids.len(), Mark::ALL.len());
    }

    #[test]
    fn the_key_and_the_settings_are_two_files() {
        // Putting the key inside the preferences would make the whole file a
        // secret, and then every harmless setting in it would be one too.
        assert_ne!(FILE, KEY_FILE);
        assert!(FILE.ends_with(".json"));
        assert!(!KEY_FILE.ends_with(".json"));
    }
}
