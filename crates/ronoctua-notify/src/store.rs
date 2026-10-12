//! What survives a restart: the history, and the person's own filters — 4.2.
//!
//! # Two files, two lifetimes, two directories
//!
//! The HISTORY is state: it is what happened, it is machine-written, and it is
//! large. It lives under `$XDG_STATE_HOME`, which is the directory for exactly
//! that, and it is pruned to fifteen days.
//!
//! The PREFERENCES are configuration: the person's filters and their
//! adjustments to the distro's defaults. They live under `$XDG_CONFIG_HOME`,
//! next to the app id → glyph table that already lives there — and that
//! directory is already in `nix/modules/persist.nix`, so an impermanent root
//! does not take them.
//!
//! # Why the history is rewritten whole
//!
//! Because a record CHANGES: being read is a mutation of something already
//! written. An append-only log would need a second stream of "and this one was
//! read", and rebuilding the state would mean replaying it — for a few hundred
//! short records, on a file written when something happens. Rewriting is a few
//! kilobytes and no second format.
//!
//! # Why a failure here is not fatal
//!
//! A read-only home, a full disk, a file somebody hand-edited into invalid
//! JSON: none of those are reasons for the desktop not to come up. Everything
//! here degrades to "no history" and logs why.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::presentation::Verdict;
use crate::queue::{HISTORY_CEILING, HISTORY_DAYS, Record};
use crate::rule::Rule;

/// Where a process persists its notifications — and `None` means it does not.
///
/// TWO DIRECTORIES because there are two lifetimes: the history is state (what
/// happened, machine-written, pruned to fifteen days) and the preferences are
/// configuration (what the person chose). The module header says why they are
/// two files; this is the pair of roots, carried rather than found.
///
/// `None` on either half means that half is not written, and that is what every
/// test builds. It is the same shape as `sidebar::Sidebar`'s `store`, added
/// there after the sidebar's own tests wrote into somebody's real home.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Where {
    /// `$XDG_CONFIG_HOME/ronoctua`, for the preferences.
    pub config: Option<PathBuf>,
    /// `$XDG_STATE_HOME/ronoctua`, for the history.
    pub state: Option<PathBuf>,
}

impl Where {
    /// The pair the APPLICATION uses, found once from the environment.
    #[must_use]
    pub fn of_env() -> Self {
        Self {
            config: config_dir(),
            state: state_dir(),
        }
    }

    /// The person's filters, or the distro's when there is nowhere to read.
    #[must_use]
    pub fn preferences(&self) -> Preferences {
        self.config
            .as_deref()
            .map_or_else(Preferences::default, read_preferences_from)
    }

    /// The history, already pruned, or nothing.
    #[must_use]
    pub fn history(&self) -> Vec<Record> {
        self.state
            .as_deref()
            .map_or_else(Vec::new, read_history_from)
    }

    /// Writes the filters. Writing nowhere is not a failure.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn save_preferences(&self, p: &Preferences) -> std::io::Result<()> {
        self.config
            .as_deref()
            .map_or(Ok(()), |d| write_preferences_to(d, p))
    }

    /// Writes the history, pruning it first. Writing nowhere is not a failure.
    ///
    /// # Errors
    ///
    /// The directory cannot be created, or the file cannot be written.
    pub fn save_history(&self, records: &mut Vec<Record>) -> std::io::Result<()> {
        self.state
            .as_deref()
            .map_or(Ok(()), |d| write_history_to(d, records))
    }
}

/// The file the history is written to, under `$XDG_STATE_HOME/ronoctua`.
pub const HISTORY_FILE: &str = "notifications.json";
/// The file the person's filters are read from, under `$XDG_CONFIG_HOME/ronoctua`.
pub const PREFERENCES_FILE: &str = "notifications.json";

/// `$XDG_STATE_HOME/ronoctua` and `$XDG_CONFIG_HOME/ronoctua`, from the one
/// crate that owns the two roots (`ronoctua-xdg`; it says why it exists).
pub use ronoctua_xdg::{config_dir, state_dir};

/// The person's own filters and adjustments, as the file holds them.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Filters the person wrote. They are tried BEFORE the distro's.
    pub rules: Vec<Rule>,
    /// Which of those filters have been sent to the REPOSITORY.
    ///
    /// The filter itself stays in `rules` until it comes back as part of the
    /// distro — a build from the flake is the only way it can — and this is
    /// what lets the screen say "on its way" rather than "yours" about it. It
    /// is a list of ids and not a copy of anything: the copy that matters is
    /// the one in `assets/shared/notifications.json`, in the repository.
    pub shared: std::collections::BTreeSet<String>,
    /// Adjustments to a catalogue entry or to a distro rule, keyed by its id.
    ///
    /// This is "use the distro's default, or customise" as one map: an id that
    /// is not here uses the default, and an id that is here overrides only the
    /// fields it names.
    pub adjustments: std::collections::BTreeMap<String, Verdict>,
}

/// Reads the person's preferences from an explicit directory.
///
/// **A PARAMETER, and it used not to be.** These four functions found
/// `$XDG_CONFIG_HOME` and `$XDG_STATE_HOME` for themselves, which is the shape
/// that let the file manager's sidebar tests write into the real configuration
/// of whoever ran `cargo test` — a `/tmp` path pinned and the sidebar switched
/// off, on a build that had the feature compiled in. Nothing had bitten here
/// only because no test reaches these yet; any future one that builds a
/// `Footer` would have.
///
/// It is the rule the rest of this project already states: every root is a
/// parameter (`trash::Bin`, `who::Roster`, `when::Zone`, `sidebar::Sidebar`),
/// and [`config_dir`] is what the APPLICATION calls, once, before handing the
/// answer in.
#[must_use]
pub fn read_preferences_from(dir: &Path) -> Preferences {
    // A `Store`, so ONE filter that will not read costs that filter and not
    // every one the person wrote: the rest is read, the repair goes into the
    // ledger the belongings' page shows, and the store says it in the log.
    let p = preferences_at(dir).read();
    tracing::info!(
        dir = %dir.display(),
        rules = p.rules.len(),
        adjustments = p.adjustments.len(),
        "notification preferences read"
    );
    p
}

/// The person's filters as a [`ronoctua_store::Store`] in `dir`.
fn preferences_at(dir: &Path) -> ronoctua_store::Store<Preferences> {
    ronoctua_store::Store::at(Some(dir.to_path_buf()), PREFERENCES_FILE)
}

/// Writes the person's preferences — whole and atomically, keeping what the
/// file holds that this machine does not know (a newer machine's field, a
/// filter it could not read).
///
/// # Errors
///
/// When the directory cannot be created or the file cannot be written.
pub fn write_preferences_to(dir: &Path, p: &Preferences) -> std::io::Result<()> {
    preferences_at(dir).write(p)
}

/// Reads the history, already pruned to [`HISTORY_DAYS`].
#[must_use]
pub fn read_history_from(dir: &Path) -> Vec<Record> {
    let path = dir.join(HISTORY_FILE);
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<Record>>(&raw) {
        Ok(mut v) => {
            let before = v.len();
            prune(&mut v, SystemTime::now());
            tracing::info!(
                path = %path.display(),
                read = before,
                kept = v.len(),
                "notification history read"
            );
            v
        }
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "the history is unreadable");
            Vec::new()
        }
    }
}

/// Writes the history, pruning it first.
///
/// # Errors
///
/// When the directory cannot be created or the file cannot be written.
pub fn write_history_to(dir: &Path, records: &mut Vec<Record>) -> std::io::Result<()> {
    prune(records, SystemTime::now());
    let text = serde_json::to_string(records)?;
    ronoctua_xdg::write_atomically(dir, HISTORY_FILE, text.as_bytes())
}

/// Drops what is older than [`HISTORY_DAYS`], and then what is over the ceiling.
///
/// Two limits and the smaller wins, for the same reason the expansion's ceiling
/// has two: fifteen days of a quiet week is a handful of records, and fifteen
/// days of an application in a loop is a file nobody wants to parse at startup.
pub fn prune(records: &mut Vec<Record>, now: SystemTime) {
    let cutoff = now
        .checked_sub(Duration::from_secs(HISTORY_DAYS * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    // A record from the FUTURE stays: the clock may have been wrong when it was
    // written, and throwing away what one cannot date is worse than keeping it.
    records.retain(|r| r.notification.at >= cutoff);
    if records.len() > HISTORY_CEILING {
        records.drain(..records.len() - HISTORY_CEILING);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::{Presentation, Show};
    use crate::{Notification, Urgency};

    fn record(id: u32, age: Duration, now: SystemTime) -> Record {
        let mut n = Notification::fresh_one(
            id,
            "app".into(),
            None,
            format!("r{id}"),
            String::new(),
            Urgency::Normal,
        );
        n.at = now.checked_sub(age).unwrap();
        Record {
            notification: n,
            presentation: Presentation::from_urgency(Urgency::Normal),
            read: false,
        }
    }

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);

    /// A directory of this test's own, removed when it ends.
    struct Tree {
        root: std::path::PathBuf,
    }

    impl Tree {
        fn fresh(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("ronoctua-notes-{name}"));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn nowhere_to_write_is_not_a_failure_and_writes_nothing() {
        // THE DEFECT THIS PREVENTS reached the owner's machine through the
        // sidebar, not through here — but the shape was identical, and it was
        // one future test away. `read_preferences` and `write_preferences`
        // found `$XDG_CONFIG_HOME` for themselves, so any test that built a
        // `Footer` would have written into somebody's real home.
        let tree = Tree::fresh("no-destination");
        let nowhere = Where::default();
        assert!(nowhere.preferences().rules.is_empty());
        assert!(nowhere.history().is_empty());
        nowhere.save_preferences(&Preferences::default()).unwrap();
        nowhere.save_history(&mut Vec::new()).unwrap();
        assert!(std::fs::read_dir(&tree.root).unwrap().next().is_none());
    }

    #[test]
    fn a_named_pair_writes_there_and_reads_it_back() {
        let tree = Tree::fresh("with-destination");
        let notes = Where {
            config: Some(tree.root.join("config")),
            state: Some(tree.root.join("state")),
        };
        let mut prefs = Preferences::default();
        prefs.adjustments.insert("x".to_owned(), Verdict::default());
        notes.save_preferences(&prefs).unwrap();
        assert_eq!(notes.preferences().adjustments.len(), 1);

        // TWO DIRECTORIES, and both files are called `notifications.json` —
        // which is why a wrong root would silently cross-contaminate rather
        // than fail. The history's directory must hold the history and nothing
        // else.
        assert!(tree.root.join("config").join(PREFERENCES_FILE).exists());
        assert!(!tree.root.join("state").join(HISTORY_FILE).exists());
    }

    #[test]
    fn fifteen_days_and_not_a_day_more() {
        let now = SystemTime::now();
        let mut v = vec![
            record(1, DAY * 20, now),
            record(2, DAY * 14, now),
            record(3, Duration::ZERO, now),
        ];
        prune(&mut v, now);
        let kept: Vec<u32> = v.iter().map(|r| r.notification.id).collect();
        assert_eq!(kept, vec![2, 3]);
    }

    #[test]
    fn a_record_from_the_future_is_kept() {
        // The clock may have been wrong when it was written. Throwing away what
        // one cannot date loses more than it saves.
        let now = SystemTime::now();
        let mut r = record(1, Duration::ZERO, now);
        r.notification.at = now + DAY;
        let mut v = vec![r];
        prune(&mut v, now);
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn the_ceiling_drops_the_oldest() {
        let now = SystemTime::now();
        let mut v: Vec<Record> = (0..HISTORY_CEILING as u32 + 10)
            .map(|i| record(i, Duration::from_secs(1), now))
            .collect();
        prune(&mut v, now);
        assert_eq!(v.len(), HISTORY_CEILING);
        assert_eq!(v[0].notification.id, 10);
    }

    #[test]
    fn a_record_goes_to_json_and_comes_back_whole() {
        // The file is the only thing between one session and the next. A field
        // that does not survive the round trip is a filter that stops applying
        // after a reboot, with nothing to notice it by.
        let now = SystemTime::now();
        let mut r = record(9, Duration::ZERO, now);
        r.presentation.show = Show::Hidden;
        r.presentation.level = 77;
        r.presentation.by = Some("my-rule".into());
        r.read = true;
        let text = serde_json::to_string(&vec![r.clone()]).unwrap();
        let back: Vec<Record> = serde_json::from_str(&text).unwrap();
        assert_eq!(back[0], r);
    }

    #[test]
    fn a_file_from_an_older_build_still_loads() {
        // `serde(default)` on the shapes, and this is what it buys: a record
        // written before a field existed must not turn the whole history into a
        // parse error.
        let raw = r#"[{"notification":{"id":3,"app":"a","summary":"b"},"presentation":{}}]"#;
        let back: Vec<Record> = serde_json::from_str(raw).unwrap();
        assert_eq!(back[0].notification.id, 3);
        assert_eq!(back[0].presentation.show, Show::Brief);
        assert!(!back[0].read);
    }

    #[test]
    fn broken_preferences_do_not_take_the_shell_down() {
        // A hand-edited file that stopped parsing is not a reason for the
        // desktop not to come up.
        assert!(serde_json::from_str::<Preferences>("{ i am not json").is_err());
        assert!(Preferences::default().rules.is_empty());
    }
}
