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

use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use crate::presentation::Verdict;
use crate::queue::{HISTORY_CEILING, HISTORY_DAYS, Record};
use crate::rule::Rule;

/// The file the history is written to, under `$XDG_STATE_HOME/ronoctua`.
pub const HISTORY_FILE: &str = "notifications.json";
/// The file the person's filters are read from, under `$XDG_CONFIG_HOME/ronoctua`.
pub const PREFERENCES_FILE: &str = "notifications.json";

/// `$XDG_STATE_HOME/ronoctua`, or `~/.local/state/ronoctua`.
#[must_use]
pub fn state_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("state"))
        })?;
    Some(base.join("ronoctua"))
}

/// `$XDG_CONFIG_HOME/ronoctua`, or `~/.config/ronoctua`.
#[must_use]
pub fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("ronoctua"))
}

/// The person's own filters and adjustments, as the file holds them.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Filters the person wrote. They are tried BEFORE the distro's.
    pub rules: Vec<Rule>,
    /// Adjustments to a catalogue entry or to a distro rule, keyed by its id.
    ///
    /// This is "use the distro's default, or customise" as one map: an id that
    /// is not here uses the default, and an id that is here overrides only the
    /// fields it names.
    pub adjustments: std::collections::BTreeMap<String, Verdict>,
}

/// Reads the person's preferences. A missing or broken file is an empty one.
#[must_use]
pub fn read_preferences() -> Preferences {
    let Some(path) = config_dir().map(|d| d.join(PREFERENCES_FILE)) else {
        return Preferences::default();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Preferences::default();
    };
    match serde_json::from_str::<Preferences>(&raw) {
        Ok(p) => {
            tracing::info!(
                path = %path.display(),
                rules = p.rules.len(),
                adjustments = p.adjustments.len(),
                "preferencias de notificacao lidas"
            );
            p
        }
        Err(e) => {
            // LOUD, and not silent. A hand-edited file that stopped parsing
            // means every filter the person wrote is gone, and the symptom
            // would be notifications reappearing with no explanation.
            tracing::error!(
                path = %path.display(),
                error = %e,
                "preferencias de notificacao ilegiveis; seguindo com as da distro"
            );
            Preferences::default()
        }
    }
}

/// Writes the person's preferences.
///
/// # Errors
///
/// When the directory cannot be created or the file cannot be written.
pub fn write_preferences(p: &Preferences) -> std::io::Result<()> {
    let dir = config_dir().ok_or_else(|| std::io::Error::other("sem HOME nem XDG_CONFIG_HOME"))?;
    std::fs::create_dir_all(&dir)?;
    let text = serde_json::to_string_pretty(p)?;
    write_atomically(&dir.join(PREFERENCES_FILE), &text)
}

/// Reads the history, already pruned to [`HISTORY_DAYS`].
#[must_use]
pub fn read_history() -> Vec<Record> {
    let Some(path) = state_dir().map(|d| d.join(HISTORY_FILE)) else {
        return Vec::new();
    };
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    match serde_json::from_str::<Vec<Record>>(&raw) {
        Ok(mut v) => {
            let before = v.len();
            prune(&mut v, SystemTime::now());
            tracing::info!(
                path = %path.display(),
                lidas = before,
                mantidas = v.len(),
                "historico de notificacoes lido"
            );
            v
        }
        Err(e) => {
            tracing::error!(path = %path.display(), error = %e, "historico ilegivel");
            Vec::new()
        }
    }
}

/// Writes the history, pruning it first.
///
/// # Errors
///
/// When the directory cannot be created or the file cannot be written.
pub fn write_history(records: &mut Vec<Record>) -> std::io::Result<()> {
    prune(records, SystemTime::now());
    let dir = state_dir().ok_or_else(|| std::io::Error::other("sem HOME nem XDG_STATE_HOME"))?;
    std::fs::create_dir_all(&dir)?;
    let text = serde_json::to_string(records)?;
    write_atomically(&dir.join(HISTORY_FILE), &text)
}

/// Writes through a temporary file in the same directory, then renames.
///
/// A history half written is a history that will not parse, and the next start
/// would silently begin from nothing. `rename` inside one filesystem is atomic,
/// so either the old file is there or the new one is.
fn write_atomically(path: &std::path::Path, text: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
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
        r.presentation.by = Some("minha-regra".into());
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
        assert!(serde_json::from_str::<Preferences>("{ nao e json").is_err());
        assert!(Preferences::default().rules.is_empty());
    }
}
