//! What a door asks of the next sync (feature `prefs`).
//!
//! `requests.json` (`state/belongings/requests.json`): `DropEverywhere(id)`,
//! `KeepEverywhere(id)` and `Compact { keep_days }`, consumed under the run
//! lock after a successful push (`engine`'s `consume_requests` hook).
//!
//! # Why a file, and not an argument of the order
//!
//! "Not on any machine" changes the REPOSITORY, and only a sync with the
//! passphrase (or the identity the session keeps) can write it: the door
//! that hears the gesture has neither. So it writes what was asked here and
//! queues a sync; the sync reads the file under its lock, applies what it
//! asks to the index it pushes (the index's `dropped`), and — once the push
//! is in — takes out what the pushed index now says is done
//! ([`Requests::unsettled`]). A request a door added while the sync ran is
//! not in what the sync read, so it stays for the next one; a push another
//! machine beat consumes nothing.
//!
//! # One answer per target
//!
//! A later request about a target replaces an earlier one ([`Requests::ask`]):
//! "nowhere" then "everywhere again" is "everywhere again". `Compact` is the
//! history's, read by the compaction (`Op::Compact`) and consumed by it.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::kept;

/// The file's format.
pub const REQUESTS_FORMAT: u32 = 1;

/// One request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ask", rename_all = "kebab-case")]
pub enum Request {
    /// Stop keeping a predefined target on EVERY machine: its copies leave
    /// the repository, and the other machines forget theirs.
    DropEverywhere {
        /// The target's id.
        target: String,
    },
    /// Keep a target dropped everywhere on every machine again.
    KeepEverywhere {
        /// The target's id.
        target: String,
    },
    /// Compact the history to its last `keep_days` days.
    Compact {
        /// The days of history kept.
        keep_days: u64,
    },
}

impl Request {
    /// The target it is about, if any.
    #[must_use]
    pub fn target(&self) -> Option<&str> {
        match self {
            Self::DropEverywhere { target } | Self::KeepEverywhere { target } => {
                Some(target.as_str())
            }
            Self::Compact { .. } => None,
        }
    }

    /// Whether it may be asked: a target this build knows (a request about
    /// another could never be satisfied, and would wait for ever).
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.target()
            .is_none_or(|t| kept::target_by_id(t).is_some())
    }
}

/// `requests.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Requests {
    /// [`REQUESTS_FORMAT`].
    pub format: u32,
    /// What is asked, oldest first.
    pub requests: Vec<Request>,
}

impl Default for Requests {
    fn default() -> Self {
        Self {
            format: REQUESTS_FORMAT,
            requests: Vec::new(),
        }
    }
}

impl Requests {
    /// The file; nothing asked when it is missing. A file that does not parse
    /// asks nothing either — and is never rewritten blind: [`Requests::ask`]
    /// on it starts from what it could read.
    #[must_use]
    pub fn read(path: &Path) -> Self {
        std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .map(|mut r| {
                r.requests.retain(Request::is_known);
                r
            })
            .unwrap_or_default()
    }

    /// Asks `request`, in place of an earlier one about the same target (or
    /// an earlier `Compact`). Says whether it was taken: a target this build
    /// does not know is refused.
    pub fn ask(&mut self, request: Request) -> bool {
        if !request.is_known() {
            return false;
        }
        self.requests
            .retain(|r| match (r.target(), request.target()) {
                (Some(a), Some(b)) => a != b,
                (None, None) => false,
                _ => true,
            });
        self.requests.push(request);
        true
    }

    /// The days a compaction was asked to keep, if one was.
    #[must_use]
    pub fn compact_days(&self) -> Option<u64> {
        self.requests.iter().rev().find_map(|r| match r {
            Request::Compact { keep_days } => Some(*keep_days),
            _ => None,
        })
    }

    /// What is still asked once a sync pushed an index that drops the
    /// targets in `dropped`: a "nowhere" whose target it drops is done, an
    /// "everywhere again" whose target it keeps is done. A compaction is the
    /// compaction's to consume. Pure.
    #[must_use]
    pub fn unsettled(&self, dropped: &std::collections::BTreeSet<String>) -> Self {
        Self {
            format: REQUESTS_FORMAT,
            requests: self
                .requests
                .iter()
                .filter(|r| match r {
                    Request::DropEverywhere { target } => !dropped.contains(target),
                    Request::KeepEverywhere { target } => dropped.contains(target),
                    Request::Compact { .. } => true,
                })
                .cloned()
                .collect(),
        }
    }

    /// What is still asked once a compaction ran: everything but it. Pure.
    #[must_use]
    pub fn without_compaction(&self) -> Self {
        Self {
            format: REQUESTS_FORMAT,
            requests: self
                .requests
                .iter()
                .filter(|r| !matches!(r, Request::Compact { .. }))
                .cloned()
                .collect(),
        }
    }

    /// Writes the file — `0600`, through a draft; deleted when nothing is
    /// asked any more.
    ///
    /// # Errors
    ///
    /// It could not be written or deleted.
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if self.requests.is_empty() {
            return match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            };
        }
        let mut text = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        text.push(b'\n');
        crate::secret::write_private(path, &text)
    }
}

// ── anchor: L10 ──

#[cfg(test)]
mod tests {
    use super::*;

    fn drop(t: &str) -> Request {
        Request::DropEverywhere {
            target: t.to_owned(),
        }
    }

    fn keep(t: &str) -> Request {
        Request::KeepEverywhere {
            target: t.to_owned(),
        }
    }

    /// FAILS IF: two answers about one target both stay (the sync would do
    /// both), an unknown target is taken, a compaction replaces a target's
    /// request, or what a pushed index settled is not consumed — or what it
    /// did not settle is.
    #[test]
    fn one_answer_per_target_and_only_what_was_done_is_consumed() {
        let mut r = Requests::default();
        assert!(r.ask(drop("ssh")));
        assert!(r.ask(drop("gnupg")));
        assert!(r.ask(Request::Compact { keep_days: 7 }));
        assert!(r.ask(keep("ssh")), "everywhere again replaces nowhere");
        assert!(!r.ask(drop("nonsense")), "a target nobody declared");
        assert_eq!(
            r.requests,
            [
                drop("gnupg"),
                Request::Compact { keep_days: 7 },
                keep("ssh")
            ]
        );
        assert_eq!(r.compact_days(), Some(7));

        // THE INDEX PUSHED drops gnupg (done) and ssh (not yet kept again).
        let dropped = ["gnupg".to_owned(), "ssh".to_owned()].into_iter().collect();
        let left = r.unsettled(&dropped);
        assert_eq!(
            left.requests,
            [Request::Compact { keep_days: 7 }, keep("ssh")]
        );
        assert_eq!(left.without_compaction().requests, [keep("ssh")]);
    }

    /// FAILS IF: the file does not come back whole, a broken one asks
    /// something, an unknown target in it survives, or an empty one is left
    /// on disk.
    #[test]
    fn the_file_crosses_whole_and_goes_when_empty() {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-belongings-requests-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("requests.json");
        assert_eq!(Requests::read(&path), Requests::default());
        let mut r = Requests::default();
        r.ask(drop("ssh"));
        r.write(&path).unwrap();
        assert_eq!(Requests::read(&path), r);
        std::fs::write(
            &path,
            r#"{"format":1,"requests":[{"ask":"drop-everywhere","target":"ssh"},{"ask":"drop-everywhere","target":"gone"}]}"#,
        )
        .unwrap();
        assert_eq!(Requests::read(&path).requests, [drop("ssh")]);
        std::fs::write(&path, b"{ not json").unwrap();
        assert_eq!(Requests::read(&path), Requests::default());
        Requests::default().write(&path).unwrap();
        assert!(!path.exists(), "an empty request file stayed");
        let _ = std::fs::remove_dir_all(dir);
    }
}
