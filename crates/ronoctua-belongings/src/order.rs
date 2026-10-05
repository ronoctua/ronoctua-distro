//! What the queue carries (`Work::Belongings`), and the lines the CLI prints
//! while it works (feature `prefs`).
//!
//! The queue starts `ronoctua-belongings <op> --progress` as a child — the
//! `software` shape: a program in its own process group, read line by line —
//! so `age` never enters the queue. The CLI reads everything else (the
//! passphrase's file, the plan, the decisions) from the places both sides
//! know ([`crate::places`]): nothing secret crosses the queue's socket.

use serde::{Deserialize, Serialize};

use crate::Why;

/// What to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Op {
    /// Fetch, decrypt and plan; with no conflict, apply at once.
    Prepare,
    /// Apply the plan with the person's decisions (`decisions.json`).
    Apply,
    /// Drop a prepared sync: the passphrase's file and the session.
    Abandon,
    /// Make the SSH deploy key (`config/belongings-keys/deploy`).
    Keygen,
    /// Change the passphrase: re-encrypt `key.age` alone.
    Rekey,
}

impl Op {
    /// Every one, for the census of their texts.
    pub const ALL: [Self; 5] = [
        Self::Prepare,
        Self::Apply,
        Self::Abandon,
        Self::Keygen,
        Self::Rekey,
    ];

    /// The CLI's word for it, and the suffix of its text key.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Prepare => "prepare",
            Self::Apply => "apply",
            Self::Abandon => "abandon",
            Self::Keygen => "keygen",
            Self::Rekey => "rekey",
        }
    }

    /// The one the CLI's word names.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|o| o.word() == word)
    }
}

/// Where inside a run it is, for the queue's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// Reaching the repository.
    Fetching,
    /// Opening `key.age` (the one scrypt of a sync).
    Unlocking,
    /// Reading this machine's files.
    Reading,
    /// Writing what the repository had.
    Pulling,
    /// Encrypting what this machine has.
    Encrypting,
    /// Sending it.
    Pushing,
    /// Making a key.
    Keying,
}

impl Phase {
    /// Every one, for the census of their texts.
    pub const ALL: [Self; 7] = [
        Self::Fetching,
        Self::Unlocking,
        Self::Reading,
        Self::Pulling,
        Self::Encrypting,
        Self::Pushing,
        Self::Keying,
    ];

    /// Its word, the suffix of its text key.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Fetching => "fetching",
            Self::Unlocking => "unlocking",
            Self::Reading => "reading",
            Self::Pulling => "pulling",
            Self::Encrypting => "encrypting",
            Self::Pushing => "pushing",
            Self::Keying => "keying",
        }
    }

    /// The one named `word`.
    #[must_use]
    pub fn of_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.word() == word)
    }
}

/// The first field of every line the CLI prints for the queue: what is not
/// one of these is chatter.
pub const MARK: &str = "BELONGINGS";

/// One line the CLI prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Beat {
    /// Working: a phase, and how far along it (items).
    Step {
        /// Where.
        phase: Phase,
        /// How many done.
        done: u64,
        /// Of how many; `0` unknown.
        total: u64,
    },
    /// Done; the conflicts left for the person, if any.
    Done {
        /// How many conflicts wait.
        conflicts: u32,
    },
    /// Not done, and why.
    Failed(Why),
}

impl Beat {
    /// The line, without its newline.
    #[must_use]
    pub fn line(&self) -> String {
        match self {
            Self::Step { phase, done, total } => {
                format!("{MARK}\tstep\t{}\t{done}\t{total}", phase.word())
            }
            Self::Done { conflicts } => format!("{MARK}\tdone\t{conflicts}"),
            Self::Failed(why) => format!("{MARK}\tfailed\t{}", why.id()),
        }
    }

    /// The beat a line is, if it is one.
    #[must_use]
    pub fn of_line(line: &str) -> Option<Self> {
        let mut fields = line.trim_end_matches(['\r', '\n']).split('\t');
        if fields.next()? != MARK {
            return None;
        }
        let beat = match fields.next()? {
            "step" => Self::Step {
                phase: Phase::of_word(fields.next()?)?,
                done: fields.next()?.parse().ok()?,
                total: fields.next()?.parse().ok()?,
            },
            "done" => Self::Done {
                conflicts: fields.next()?.parse().ok()?,
            },
            "failed" => Self::Failed(Why::of_id(fields.next()?)?),
            _ => return None,
        };
        fields.next().is_none().then_some(beat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_beat_is_one_line_read_back_whole() {
        for beat in [
            Beat::Step {
                phase: Phase::Encrypting,
                done: 3,
                total: 9,
            },
            Beat::Done { conflicts: 2 },
            Beat::Failed(Why::WrongPassphrase),
        ] {
            let line = beat.line();
            assert!(!line.contains('\n'));
            assert_eq!(Beat::of_line(&line), Some(beat));
        }
        assert_eq!(Beat::of_line("git: fatal: something"), None);
        assert_eq!(Beat::of_line("BELONGINGS\tfailed\tno-such"), None);
        assert_eq!(Beat::of_line("BELONGINGS\tdone\t1\textra"), None);
        for op in Op::ALL {
            assert_eq!(Op::of_word(op.word()), Some(op));
        }
    }
}
