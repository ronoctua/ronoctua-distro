//! What a sync learned that is not secret, for the doors (feature `prefs`):
//! the person's machines as the index lists them (`fleet.json`) and the
//! person's post-quantum recipient (`recipient`), both under
//! `state/belongings/`.
//!
//! # Why a door needs them
//!
//! The index is sealed: only the CLI, with the passphrase, reads it. A door
//! that must know "does this machine run the newest distro of the person's
//! machines?" (a lenient store's repair is written by itself only then) or
//! "what does 'to myself' seal to?" (encrypt-anything) reads what the last
//! sync left here instead — each written whole, after the sync that read it.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::places::Places;

/// One machine, as the index remembers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Seen {
    /// When it last synced, seconds since the epoch.
    pub at_s: i64,
    /// When the distro it runs was built (the flake's `lastModified`),
    /// seconds since the epoch; `None` when that machine did not say.
    pub built_s: Option<i64>,
}

/// `fleet.json`: the person's machines, and which one this is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Fleet {
    /// This machine's name in the index.
    pub this: String,
    /// Every machine that synced.
    pub machines: BTreeMap<String, Seen>,
}

impl Fleet {
    /// Reads `fleet.json`; `None` when there is none or it does not parse.
    #[must_use]
    pub fn read(path: &Path) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(path).ok()?).ok()
    }

    /// Whether this machine runs the newest distro among the person's
    /// machines: its build is known, and no other machine's is later. Pure.
    #[must_use]
    pub fn newest_here(&self) -> bool {
        let Some(mine) = self.machines.get(&self.this).and_then(|s| s.built_s) else {
            return false;
        };
        self.machines
            .iter()
            .filter(|(name, _)| **name != self.this)
            .all(|(_, seen)| seen.built_s.is_none_or(|theirs| theirs <= mine))
    }
}

/// The person's post-quantum recipient (`age1pq1…`) the last sync left,
/// public — what "to myself" seals to without asking the passphrase. `None`
/// before a sync of format 2, or for text that is not one.
#[must_use]
pub fn recipient(places: &Places) -> Option<String> {
    let text = std::fs::read_to_string(places.recipient()).ok()?;
    let text = text.trim();
    let fits = text.starts_with("age1pq1")
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
    fits.then(|| text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seen(built_s: Option<i64>) -> Seen {
        Seen { at_s: 1, built_s }
    }

    #[test]
    fn the_newest_is_the_machine_no_other_is_later_than() {
        let mut fleet = Fleet {
            this: "laptop".to_owned(),
            machines: [
                ("laptop".to_owned(), seen(Some(200))),
                ("desk".to_owned(), seen(Some(100))),
                ("old".to_owned(), seen(None)),
            ]
            .into_iter()
            .collect(),
        };
        assert!(fleet.newest_here());
        fleet.machines.insert("desk".to_owned(), seen(Some(300)));
        assert!(!fleet.newest_here(), "another machine runs a later distro");
        fleet.machines.insert("laptop".to_owned(), seen(None));
        assert!(!fleet.newest_here(), "a build this machine did not say");
        assert!(!Fleet::default().newest_here());
    }

    /// FAILS IF: a machine whose index entry says no build stops reading.
    #[test]
    fn a_machine_that_did_not_say_its_build_reads_as_unknown() {
        let text = r#"{"this":"a","machines":{"a":{"at_s":5}}}"#;
        let fleet: Fleet = serde_json::from_str(text).unwrap();
        assert_eq!(fleet.machines["a"], seen(None).with_at(5));
    }

    impl Seen {
        const fn with_at(mut self, at_s: i64) -> Self {
            self.at_s = at_s;
            self
        }
    }
}
