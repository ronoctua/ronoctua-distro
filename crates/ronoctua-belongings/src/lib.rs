//! The belongings: the person's configuration, kept encrypted in a private
//! git repository they name, and restored on a new installation.
//!
//! # What the repository holds
//!
//! ```text
//! FORMAT          "ronoctua-belongings 1"                        (plain)
//! key.age         an X25519 identity, age-scrypt(passphrase)     (one KDF per sync)
//! index.age       {version, entries:[{id, path, kind, mode, sha256, size, …}], machines}
//! blobs/<id>.age  one per file; random ids hide the names and paths
//! ```
//!
//! Recoverable without the distro (`age -d key.age > id; age -d -i id
//! index.age`); changing the passphrase re-encrypts only `key.age`. What leaks
//! is said on screen: how many files, their sizes, the commits' times. The
//! machine's name is written only inside the index; a commit says
//! `belongings`, and is authored `ronoctua-belongings <noreply@localhost>`.
//!
//! # Git is transport; the merge is ours
//!
//! Three-way per path: the base is `last.json`, ours the local files, theirs
//! the remote index. A change on one side is taken; on both, it is a
//! [`plan::Conflict`], and the person decides each ([`plan::Decision`]).
//! What is overwritten locally goes to `replaced/<stamp>/` (the last three):
//! the gesture can be undone.
//!
//! # Paths are logical
//!
//! `config/theme.json`, `state/software/history.jsonl`, `home/notes/todo.md`:
//! the first word is a ROOT ([`places::Root`]), resolved on each machine
//! against its own directories — the `.#dev` loop's second copy of the system
//! included. The repository never holds a machine's absolute path.
//!
//! # Layers
//!
//! [`prefs`], [`kept`], [`places`], [`plan`], [`status`], [`order`] and
//! [`running`] are the shapes (feature `prefs`: serde and the XDG roots — the
//! shell, the queue and the installer). The engine ([`engine`], [`merge`], [`crypt`], [`walk`],
//! feature `crypt`) is the extracted CLI's, so `age` never enters the footer
//! or the queue.

#![forbid(unsafe_code)]

#[cfg(feature = "crypt")]
pub mod crypt;
#[cfg(feature = "crypt")]
pub mod engine;
#[cfg(feature = "prefs")]
pub mod kept;
#[cfg(feature = "crypt")]
pub mod merge;
#[cfg(feature = "prefs")]
pub mod order;
#[cfg(feature = "prefs")]
pub mod places;
#[cfg(feature = "prefs")]
pub mod plan;
#[cfg(feature = "prefs")]
pub mod prefs;
#[cfg(feature = "prefs")]
pub mod running;
#[cfg(feature = "prefs")]
pub mod secret;
#[cfg(feature = "prefs")]
pub mod status;
#[cfg(feature = "crypt")]
pub mod walk;

/// The first line of the repository's `FORMAT` file.
pub const FORMAT: &str = "ronoctua-belongings 1";

/// Why a sync did not happen, as a closed set — each with a sentence on
/// screen (`belongings-refused-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// No repository is configured.
    NoRepository,
    /// The passphrase's file is missing, or somebody else can read it.
    NoPassphrase,
    /// The passphrase does not open `key.age`.
    WrongPassphrase,
    /// The remote wants access that was not given (no key, no token).
    NeedsAccess,
    /// The remote refused the key or the token.
    Refused,
    /// The remote could not be reached.
    Unreachable,
    /// Another machine pushed three times in a row while this one prepared.
    Raced,
    /// The repository is not a belongings repository (or a newer format).
    NotOurs,
    /// The repository is empty: there is no passphrase to change yet (the
    /// first sync sets it).
    Empty,
    /// Something in it does not decrypt or does not parse.
    Corrupt,
    /// A conflict has no decision (or one it may not have).
    Undecided,
    /// There is nothing prepared to apply or abandon.
    NoSession,
    /// This machine was not given git or `ssh-keygen`.
    NoTools,
    /// A local file could not be read or written.
    Failed,
    /// Another run of the belongings holds this machine's lock: two at once
    /// would each clean the other's work out of the clone
    /// ([`running`]).
    Busy,
}

impl Why {
    /// Every refusal, for the census of their sentences.
    pub const ALL: [Self; 15] = [
        Self::NoRepository,
        Self::NoPassphrase,
        Self::WrongPassphrase,
        Self::NeedsAccess,
        Self::Refused,
        Self::Unreachable,
        Self::Raced,
        Self::NotOurs,
        Self::Empty,
        Self::Corrupt,
        Self::Undecided,
        Self::NoSession,
        Self::NoTools,
        Self::Failed,
        Self::Busy,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoRepository => "no-repository",
            Self::NoPassphrase => "no-passphrase",
            Self::WrongPassphrase => "wrong-passphrase",
            Self::NeedsAccess => "needs-access",
            Self::Refused => "refused",
            Self::Unreachable => "unreachable",
            Self::Raced => "raced",
            Self::NotOurs => "not-ours",
            Self::Empty => "empty",
            Self::Corrupt => "corrupt",
            Self::Undecided => "undecided",
            Self::NoSession => "no-session",
            Self::NoTools => "no-tools",
            Self::Failed => "failed",
            Self::Busy => "busy",
        }
    }

    /// The one named `id`, as [`Why::id`] writes it.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal of the belongings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// A refusal for `why`.
    #[must_use]
    pub const fn of(why: Why) -> Self {
        Self { why }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.why.id())
    }
}

impl std::error::Error for Refusal {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_refusal_has_an_id_that_names_it_back() {
        let mut ids: Vec<&str> = Why::ALL.iter().map(|w| w.id()).collect();
        for w in Why::ALL {
            assert_eq!(Why::of_id(w.id()), Some(w));
            let text = serde_json::to_string(&w).unwrap();
            assert_eq!(
                text,
                format!("\"{}\"", w.id()),
                "the wire and the key differ"
            );
        }
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), Why::ALL.len());
    }
}
