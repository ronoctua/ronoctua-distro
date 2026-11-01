//! WHETHER THE SCREEN LOCKS BY ITSELF, and after how long — the person's
//! choice, `lock.json`, read where both its readers can reach it.
//!
//! # Why it lives here
//!
//! Two processes read it and they stand on nothing of each other: the footer,
//! which holds the idle watcher for the whole session and starts the lock
//! (`ronoctua_shell_footer::idle_notify`), and the security centre, which
//! offers it (`ronoctua_shell_security`, both of its doors). A choice two
//! crates use comes down to the kit, as `header` and `when::prefs` did.
//!
//! # Off by default, and a value it does not know is REFUSED
//!
//! The owner's decision (2026-10-31): the lock screen exists and has its key,
//! and locking by itself is something the person turns on. A file nobody
//! wrote, or one that says `"automatic": false`, never locks. A file the
//! store cannot read WHOLE — a minutes value outside [`MINUTES`], a word for
//! a number, a file that is not JSON — never locks either, and says so
//! ([`Armed::Refused`]): reading `"minutes": 7` as ten would be a lock at a
//! time nobody chose, and reading a broken file as "on" would be the same
//! guess the other way. The store keeps the file as it is
//! (`ronoctua_store::Store::write` puts back what it repaired) and its ledger
//! names the value, so nothing the person wrote is lost by being refused.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// The file, under `$XDG_CONFIG_HOME/ronoctua`.
pub const FILE: &str = "lock.json";

/// The waits the person can choose, in minutes — the ring the security
/// centre's row walks. Anything else in the file is refused.
pub const MINUTES: [u16; 7] = [1, 2, 5, 10, 15, 30, 60];

/// The wait a lock turned on starts at.
pub const DEFAULT_MINUTES: u16 = 10;

/// The choice as the file writes it: `{ "automatic": true, "minutes": 10 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Choice {
    /// Whether the screen locks by itself after [`Choice::minutes`] without
    /// use. OFF by default.
    pub automatic: bool,
    /// How long without use, in minutes — one of [`MINUTES`].
    pub minutes: u16,
}

impl Default for Choice {
    fn default() -> Self {
        Self {
            automatic: false,
            minutes: DEFAULT_MINUTES,
        }
    }
}

impl Choice {
    /// The same choice, the other way about locking by itself.
    #[must_use]
    pub const fn toggled(self) -> Self {
        Self {
            automatic: !self.automatic,
            minutes: self.minutes,
        }
    }

    /// The same choice, with the next wait of the ring (`forward`) or the one
    /// before it — and from a wait outside the ring, its first.
    #[must_use]
    pub fn stepped(self, forward: bool) -> Self {
        let at = MINUTES.iter().position(|m| *m == self.minutes);
        let next = match (at, forward) {
            (None, _) => 0,
            (Some(i), true) => (i + 1) % MINUTES.len(),
            (Some(i), false) => (i + MINUTES.len() - 1) % MINUTES.len(),
        };
        Self {
            automatic: self.automatic,
            minutes: MINUTES[next],
        }
    }
}

/// What the footer does with the file: nothing, a wait, or nothing because
/// the file was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Armed {
    /// The screen does not lock by itself.
    Off,
    /// It locks after this long without use.
    After(Duration),
    /// The file did not read whole: it does not lock by itself, and that is
    /// said, never guessed.
    Refused,
}

/// What a reading of the file arms — the one answer the footer acts on and
/// the security centre shows.
#[must_use]
pub fn armed_by(reading: &ronoctua_store::Reading<Choice>) -> Armed {
    if !reading.whole() || reading.trouble.is_some() {
        return Armed::Refused;
    }
    let c = reading.value;
    if !c.automatic {
        return Armed::Off;
    }
    if !MINUTES.contains(&c.minutes) {
        return Armed::Refused;
    }
    Armed::After(Duration::from_secs(u64::from(c.minutes) * 60))
}

/// A minutes value outside [`MINUTES`], refused — repaired in the read and
/// named in the ledger, never taken for a wait.
fn refused(c: &Choice) -> Vec<ronoctua_store::Refused> {
    if MINUTES.contains(&c.minutes) {
        return Vec::new();
    }
    vec![ronoctua_store::Refused {
        pointer: "/minutes".to_owned(),
        why: ronoctua_store::repair::Why::Invalid,
        kept: None,
    }]
}

/// The file as a store in `dir`, judged by [`refused`] on every read and
/// write.
#[must_use]
pub fn store_at(dir: Option<std::path::PathBuf>) -> ronoctua_store::Store<Choice> {
    ronoctua_store::Store::at(dir, FILE).checked(refused)
}

/// The file in the person's configuration.
#[must_use]
pub fn store() -> ronoctua_store::Store<Choice> {
    store_at(ronoctua_store::config_dir())
}

/// A watch on the file in `dir`, ringing `on_stir` from the watcher's thread
/// — how the footer hears a choice made in the security centre, with nothing
/// between them but the file. The program finds `dir`, once
/// (`ronoctua_store::config_dir()`), and hands it over: a function that writes
/// does not decide where (`persistence-takes-its-root-as-a-parameter`).
/// `None`, with a line in the log, when there is no configuration directory
/// or no inotify instance left: the choice then reaches the footer at its
/// next start, which is not silent.
pub fn watch<F>(
    dir: Option<std::path::PathBuf>,
    on_stir: F,
) -> Option<ronoctua_files::notice::Notice>
where
    F: Fn() -> bool + Send + 'static,
{
    let dir = dir?;
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(error = %e, "no configuration directory to watch lock.json in");
        return None;
    }
    match ronoctua_files::notice::Notice::on_file(
        &dir,
        FILE,
        ronoctua_files::notice::Pace::AtOnce,
        on_stir,
    ) {
        Ok(n) => Some(n),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "no watcher on lock.json; a choice reaches the footer at its next start"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ronoctua-lock-choice-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    /// FAILS IF: a machine nobody chose for locks by itself — the owner's
    /// default is off.
    #[test]
    fn nobody_chose_is_off() {
        let dir = scratch("absent");
        let store = store_at(Some(dir.clone()));
        assert_eq!(armed_by(&store.reading()), Armed::Off);
        assert!(!Choice::default().automatic);
        assert!(MINUTES.contains(&DEFAULT_MINUTES));
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a chosen wait is not the wait armed.
    #[test]
    fn a_choice_arms_its_minutes() {
        let dir = scratch("on");
        let store = store_at(Some(dir.clone()));
        store
            .write(&Choice {
                automatic: true,
                minutes: 5,
            })
            .expect("written");
        assert_eq!(
            armed_by(&store.reading()),
            Armed::After(Duration::from_secs(300))
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: a value this machine does not know becomes a plausible wait,
    /// or a broken file becomes "on" — refused, both, and the file kept.
    #[test]
    fn an_unknown_value_is_refused_and_never_defaulted() {
        let dir = scratch("refused");
        let store = store_at(Some(dir.clone()));
        std::fs::write(dir.join(FILE), br#"{"automatic":true,"minutes":7}"#).expect("seed");
        assert_eq!(armed_by(&store.reading()), Armed::Refused);
        std::fs::write(dir.join(FILE), br#"{"automatic":true,"minutes":"ten"}"#).expect("seed");
        assert_eq!(armed_by(&store.reading()), Armed::Refused);
        std::fs::write(dir.join(FILE), br#"{"automatic":tru"#).expect("seed");
        assert_eq!(armed_by(&store.reading()), Armed::Refused);
        // OFF STAYS OFF, whatever else the file says: nothing locks.
        std::fs::write(dir.join(FILE), br#"{"automatic":false,"minutes":10}"#).expect("seed");
        assert_eq!(armed_by(&store.reading()), Armed::Off);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// FAILS IF: the ring leaves the waits it offers, or loses its place.
    #[test]
    fn the_ring_walks_the_waits_both_ways() {
        let mut c = Choice::default();
        let mut seen = Vec::new();
        for _ in 0..MINUTES.len() {
            c = c.stepped(true);
            seen.push(c.minutes);
        }
        let mut sorted = seen.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, MINUTES.to_vec(), "every wait once, around");
        assert_eq!(c.minutes, DEFAULT_MINUTES, "and back where it began");
        assert_eq!(c.stepped(true).stepped(false), c);
        let odd = Choice {
            automatic: true,
            minutes: 7,
        };
        assert_eq!(odd.stepped(true).minutes, MINUTES[0]);
        assert!(Choice::default().toggled().automatic);
    }
}
