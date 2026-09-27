//! The person's choices about their programs — `software.json`.
//!
//! One file, written through `ronoctua-store` (atomic, the PID in the scratch
//! name), and read back with `parsed()` before every write: the default that
//! `read()` falls back to is right for a reader and destroys the file for a
//! writer. Both doors watch it (`Notice::on_file`) — the file is the broadcast.
//!
//! TWO PROCESSES WRITE IT: a door, when the person turns a ring, and the CLI
//! the queue runs, when an unfree install remembers its consent. A writer
//! that laid a copy read minutes ago over the file would undo the other's
//! change without a trace, so every write is [`change`]: take the lock, read
//! the file as it is NOW, change ONE thing, write, let go.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::found::Prefer;
use crate::order::Traces;

/// The file's name under `$XDG_CONFIG_HOME/ronoctua[-dev]/`.
pub const FILE: &str = "software.json";

/// Everything the person chose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Preferences {
    /// Which source is recommended first.
    pub prefer: Prefer,
    /// Whether a search asks services on the network (Flathub, F-Droid,
    /// AppImageHub, dashboardicons). Off, only what is on this machine
    /// answers: the Nix index and the catalogues already fetched.
    pub online: bool,
    /// Per-source switches for the online part.
    pub sources: Sources,
    /// The unfree Nix packages the person said yes to, by attribute. An upgrade
    /// passes the flag only for these.
    pub unfree: BTreeSet<String>,
    /// Ask before downloading more than this many megabytes (0 = never ask).
    pub ask_above_mb: u64,
    /// What a trial may leave behind, by default.
    pub traces: Traces,
    /// Whether a sandboxed trial reaches the network, by default.
    pub trial_network: bool,
    /// The Chrome profile new web apps use; `None` = Chrome's last used.
    pub web_profile: Option<String>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            prefer: Prefer::NixFirst,
            online: true,
            sources: Sources::default(),
            unfree: BTreeSet::new(),
            ask_above_mb: 500,
            traces: Traces::Scratch,
            trial_network: true,
            web_profile: None,
        }
    }
}

/// Per-source switches for what goes on the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Sources {
    /// flathub.org's search API (the local catalogue answers when off).
    pub flathub: bool,
    /// f-droid.org's index.
    pub fdroid: bool,
    /// appimage.github.io's catalogue and GitHub's release pages.
    pub appimagehub: bool,
    /// dashboardicons.com, for web-app icons.
    pub dashboard: bool,
}

impl Default for Sources {
    fn default() -> Self {
        Self {
            flathub: true,
            fdroid: true,
            appimagehub: true,
            dashboard: true,
        }
    }
}

/// The store, under a config directory given as a parameter.
#[must_use]
pub fn store(config: &Path) -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(Some(config.join(ronoctua_xdg::name())), FILE)
}

/// What the person chose, or the defaults (and a log line) when the file is
/// missing or does not parse.
#[must_use]
pub fn read(config: &Path) -> Preferences {
    store(config).read()
}

/// Change one thing in the preferences as they are on disk NOW, under a lock
/// every writer takes, and return what was written. A file that does not
/// parse is never replaced.
///
/// # Errors
///
/// When the file exists and does not parse ([`crate::Refusal::WillNotParse`]
/// on screen), the lock cannot be taken, or the write fails.
pub fn change(config: &Path, what: impl FnOnce(&mut Preferences)) -> anyhow::Result<Preferences> {
    let store = store(config);
    let _held = match store.path() {
        Some(path) => Some(hold(&path)?),
        // A store that keeps nothing writes nothing, and has nothing to guard.
        None => None,
    };
    let Some(mut prefs) = store.parsed() else {
        anyhow::bail!("{FILE} will not parse, and writing would replace it");
    };
    what(&mut prefs);
    store.write(&prefs)?;
    Ok(prefs)
}

/// Write the whole preferences, under the same lock and refusal as
/// [`change`]. Only for a caller that really means every field.
///
/// # Errors
///
/// As [`change`].
pub fn keep(config: &Path, prefs: &Preferences) -> anyhow::Result<()> {
    change(config, |p| *p = prefs.clone()).map(|_| ())
}

/// The lock beside the file: `flock`, so a killed writer never leaves it shut.
fn hold(file: &Path) -> anyhow::Result<std::fs::File> {
    use rustix::fs::{FlockOperation, flock};
    let dir = file.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(format!("{FILE}.lock")))?;
    flock(&lock, FlockOperation::LockExclusive)?;
    Ok(lock)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-prefs-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_choice_is_kept_and_read_back() {
        let dir = scratch("keep");
        let mut p = Preferences::default();
        p.unfree.insert("google-chrome".into());
        p.prefer = Prefer::FlatpakFirst;
        keep(&dir, &p).unwrap();
        assert_eq!(read(&dir), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_does_not_parse_is_never_replaced() {
        let dir = scratch("broken");
        let file = dir.join(ronoctua_xdg::name()).join(FILE);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "{ not json").unwrap();
        assert!(keep(&dir, &Preferences::default()).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "{ not json");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_change_lands_on_what_is_on_disk_and_not_on_an_old_copy() {
        let dir = scratch("change");
        let old = read(&dir);
        // Another writer (the CLI remembering a consent) gets there first...
        change(&dir, |p| {
            p.unfree.insert("google-chrome".into());
        })
        .unwrap();
        // ...and this door turns a ring on the copy it read before.
        assert!(old.unfree.is_empty());
        let now = change(&dir, |p| p.online = false).unwrap();
        assert!(
            now.unfree.contains("google-chrome"),
            "the other writer's change survives"
        );
        assert!(!now.online);
        assert_eq!(read(&dir), now);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_field_takes_its_default() {
        let p: Preferences = serde_json::from_str(r#"{"online":false}"#).unwrap();
        assert!(!p.online);
        assert_eq!(p.ask_above_mb, 500);
        assert_eq!(p.prefer, Prefer::NixFirst);
    }
}
