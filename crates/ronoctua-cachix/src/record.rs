//! What the last push said — `$XDG_STATE_HOME/ronoctua/binary-cache-push.json`,
//! written by every push and every refusal of one, read by the screen ("the
//! last push and what it said").
//!
//! It also holds the stamp of the machine's apply the push followed
//! (`applied-at`), which is how the next run knows whether an update
//! switched the machine since: a push follows a SUCCESSFUL update, once.

use serde::{Deserialize, Serialize};

use std::path::{Path, PathBuf};

use ronoctua_person::Stamp;
use ronoctua_person::options::Visibility;

use crate::Why;

/// The file's name, under the state root.
pub const FILE: &str = "binary-cache-push.json";

/// How a push ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Outcome {
    /// Pushed — `paths` of them, `bytes` before compression; none when the
    /// cache and `cache.nixos.org` had everything.
    Pushed {
        /// How many paths went.
        paths: u64,
        /// Their NARs' size, in bytes.
        bytes: u64,
    },
    /// A rehearsal: this is what would have gone.
    Rehearsed {
        /// How many paths would go.
        paths: u64,
        /// Their NARs' size, in bytes.
        bytes: u64,
    },
    /// Refused, and why.
    Refused {
        /// Why.
        why: Why,
    },
}

/// One push. (No `deny_unknown_fields`: serde does not take it beside a
/// `flatten`ed field.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Record {
    /// When it ended.
    pub at: Stamp,
    /// The cache it was for — `None` when the machine names none.
    pub cache: Option<String>,
    /// Public or private, when there was a cache.
    pub visibility: Option<Visibility>,
    /// The machine's apply this push followed, when it pushed.
    pub applied_at: Option<Stamp>,
    /// How it ended.
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// The file: the last push.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Pushes {
    /// The last one, if there was one.
    pub last: Option<Record>,
}

impl Pushes {
    /// The apply the last PUSH followed — a refusal follows none, and keeps
    /// the one before it.
    #[must_use]
    pub fn followed(&self) -> Option<&Stamp> {
        self.last.as_ref().and_then(|r| r.applied_at.as_ref())
    }

    /// `self` after `record`: a refusal keeps the apply the last push
    /// followed, so a refused run does not count as having pushed it.
    #[must_use]
    pub fn after(&self, mut record: Record) -> Self {
        if record.applied_at.is_none() {
            record.applied_at = self.followed().cloned();
        }
        Self { last: Some(record) }
    }
}

/// This instance's file — `None` with no home.
///
/// WRITTEN WHOLE, by the push alone, and never through a `Store`: a store
/// keeps the keys a value lacks (a newer machine's field), and an outcome
/// that changed shape would keep the old one's `paths` beside the new one's
/// `why`. Nobody else writes it, so there is nothing to merge.
#[must_use]
pub fn path() -> Option<PathBuf> {
    ronoctua_xdg::state_dir().map(|d| d.join(FILE))
}

/// The pushes kept at `path` — none when the file is not there or is not
/// this shape (a screen says "never", a push pushes).
#[must_use]
pub fn read_at(path: &Path) -> Pushes {
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

/// Writes `pushes` at `path`, whole and atomically.
///
/// # Errors
///
/// The folder could not be made, or the file written.
pub fn write_at(path: &Path, pushes: &Pushes) -> std::io::Result<()> {
    let (Some(dir), Some(file)) = (path.parent(), path.file_name().and_then(|f| f.to_str())) else {
        return Err(std::io::ErrorKind::InvalidInput.into());
    };
    std::fs::create_dir_all(dir)?;
    let mut text = serde_json::to_string_pretty(pushes).map_err(std::io::Error::other)?;
    text.push('\n');
    ronoctua_xdg::write_atomically(dir, file, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a refusal forgets which apply the last push followed (the
    /// next run would push again what it pushed), or the file's shape moves.
    #[test]
    fn a_refusal_keeps_the_apply_the_last_push_followed() {
        let pushed = Record {
            at: Stamp::from_unix_s(100),
            cache: Some("someone".to_owned()),
            visibility: Some(Visibility::Public),
            applied_at: Some(Stamp::from_unix_s(90)),
            outcome: Outcome::Pushed { paths: 3, bytes: 9 },
        };
        let after = Pushes::default().after(pushed);
        let refused = Record {
            at: Stamp::from_unix_s(200),
            cache: Some("someone".to_owned()),
            visibility: Some(Visibility::Public),
            applied_at: None,
            outcome: Outcome::Refused { why: Why::NoToken },
        };
        let later = after.after(refused);
        assert_eq!(later.followed(), Some(&Stamp::from_unix_s(90)));
        let text = serde_json::to_string(&later).unwrap();
        assert!(
            text.contains(r#""outcome":"refused","why":"no-token""#),
            "{text}"
        );
        let back: Pushes = serde_json::from_str(&text).unwrap();
        assert_eq!(back, later);
        let root =
            std::env::temp_dir().join(format!("ronoctua-cachix-record-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let file = root.join("state").join(FILE);
        assert_eq!(read_at(&file), Pushes::default());
        write_at(&file, &later).unwrap();
        assert_eq!(read_at(&file), later);
        let _ = std::fs::remove_dir_all(&root);
    }
}
