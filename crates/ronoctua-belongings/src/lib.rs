//! The belongings: the person's configuration, kept encrypted in a private
//! git repository they name, and restored on a new installation.
//!
//! # What the repository holds (format 2)
//!
//! ```text
//! FORMAT                  "ronoctua-belongings 2"                         (plain)
//! key.age                 the PQ identity (AGE-SECRET-KEY-PQ-1…) and the ML-DSA-65
//!                         signing key (a comment line), age-scrypt(passphrase)
//! index.age               the index's head {version, size_bytes, sha256, chunks}, sealed
//! chunks/<aa>/<id>.age    content-defined chunks of every sealed file AND of the index
//! plain/person/…          the person's declared data, in clear           (plain)
//! plain/brain-modules/…   the person's brain modules, in clear           (plain)
//! plain/signed.json       the manifest of the plain part: files, SHA-256, counter (plain)
//! plain/signed.sig        the person's ML-DSA-65 signature of it          (plain)
//! plain/signer.pub        the signer's public key, for a screen; no machine trusts it
//! RECOVER.md              how to restore with Go `age` ≥ 1.3 and `jq`   (plain)
//! .gitattributes          "* -text"                                       (plain)
//! ```
//!
//! Everything sealed is sealed to the person's post-quantum recipient
//! (`mlkem768x25519`, [`crypt`]); a chunk's name is a keyed hash of its
//! plaintext, written once ([`chunks`]), so no repository file reaches
//! GitHub's limit and an edit moves few chunks. Changing the passphrase
//! re-encrypts only `key.age`. What leaks is said on screen: how many chunks
//! and their sizes, which content repeats, the commits' times. The machine's
//! name is written only inside the index; a commit says `belongings`, and is
//! authored `ronoctua-belongings <noreply@localhost>`.
//!
//! Format 2 is the only one this build reads or writes ([`format_check`]): a
//! newer one is refused ([`Why::Newer`]) and a lower or unknown one is not
//! ours ([`Why::NotOurs`]) — never read as if it were this one.
//!
//! # Git is transport; the merge is ours
//!
//! Three-way per path: the base is `last.json`, ours the local files, theirs
//! the remote index. A change on one side is taken; on both, it is a
//! [`plan::Conflict`], and the person decides each ([`plan::Decision`]).
//! What is overwritten locally goes to `replaced/<stamp>/` (the last three):
//! the gesture can be undone. Nothing reaches the repository's host before
//! the person let it (`consent.json`): every fetch and push mints a
//! `ronoctua_consent::Pass` first ([`Why::NotLet`]).
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
//! [`prefs`], [`kept`], [`places`], [`plan`], [`status`], [`order`],
//! [`fleet`] and [`running`] are the shapes (feature `prefs`: serde and the
//! XDG roots — the shell, the queue and the installer). The engine
//! ([`engine`], [`merge`], [`crypt`], [`chunks`], [`walk`], feature `crypt`)
//! is the extracted CLI's, so `age` never enters the footer or the queue.

#![forbid(unsafe_code)]

#[cfg(feature = "crypt")]
pub mod crypt;
// Which place this machine's Emacs reads: what the page says when it is not
// the one the target `emacs` carries.
#[cfg(feature = "prefs")]
pub mod emacs;
#[cfg(feature = "crypt")]
pub mod engine;
#[cfg(feature = "prefs")]
pub mod kept;
pub mod key;
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

// THE 2026-10-05 WAVE'S MODULES, empty until their package fills them: each
// says what it will hold, so two packages never write the same file.
#[cfg(feature = "crypt")]
pub mod chunks;
#[cfg(feature = "prefs")]
pub mod inventory;
#[cfg(feature = "crypt")]
pub mod merge_json;
#[cfg(feature = "prefs")]
pub mod plain;
#[cfg(feature = "prefs")]
pub mod requests;
#[cfg(feature = "prefs")]
pub mod scope;
#[cfg(feature = "crypt")]
pub mod size;
#[cfg(feature = "prefs")]
pub mod unlock;
// ── anchor: L07 ──
// What the walk never keeps because of what a file IS: a link into the
// system's store, the access key under another name.
#[cfg(feature = "prefs")]
pub mod sift;
// ── anchor: L08 ──
#[cfg(feature = "prefs")]
pub mod fleet;

/// The format this build writes — and the only one it reads.
pub const FORMAT_VERSION: u32 = 2;

/// Whether a repository whose `FORMAT` says `format` is one this build
/// reads and writes. Pure.
///
/// # Errors
///
/// [`Why::Newer`] for a newer format (update this machine first), and
/// [`Why::NotOurs`] for a lower one: no build ever wrote a repository this
/// one could read below [`FORMAT_VERSION`].
pub const fn format_check(format: u32) -> Result<(), Refusal> {
    if format == FORMAT_VERSION {
        Ok(())
    } else if format > FORMAT_VERSION {
        Err(Refusal::of(Why::Newer))
    } else {
        Err(Refusal::of(Why::NotOurs))
    }
}

/// The format a repository's `FORMAT` file says it is: `ronoctua-belongings
/// <n>` on its first line. `None` when it is not a belongings repository.
/// Pure.
#[must_use]
pub fn format_of(text: &str) -> Option<u32> {
    let line = text.lines().next()?.trim();
    let n = line.strip_prefix("ronoctua-belongings ")?;
    (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        .then(|| n.parse().ok())
        .flatten()
}

/// The person's post-quantum recipient as the last sync of this machine
/// left it — what "to myself" seals to, with no passphrase asked. `None`
/// before a sync of format 2.
#[cfg(feature = "crypt")]
#[must_use]
pub fn pq_recipient() -> Option<ronoctua_pq::Recipient> {
    let places = places::Places::from_env()?;
    fleet::recipient(&places)?.parse().ok()
}
// ── anchor: L09 ──
// ── anchor: L10 ──
// What the files a sync brought offer to turn on, and saying yes to it
// (the predefined targets: SSH signing, a security key for the password).
#[cfg(feature = "offers")]
pub mod offers;
// ── anchor: L21 ──

/// The first line of the repository's `FORMAT` file ([`FORMAT_VERSION`]).
pub const FORMAT: &str = "ronoctua-belongings 2";

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
    /// The repository is not a belongings repository, or is of a format
    /// lower than this build's.
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
    /// The repository's host was not let in the person's `consent.json`:
    /// nothing was fetched or pushed (the git consent package).
    NotLet,
    /// The repository is a newer format than this machine reads: update this
    /// machine first.
    Newer,
    /// A file of the plain part is bigger than `plainMostKiB`.
    PlainTooLarge,
    /// The repository's host offered no post-quantum key exchange, and the
    /// belongings' SSH asks for one.
    NotPostQuantum,
}

impl Why {
    /// Every refusal, for the census of their sentences.
    pub const ALL: [Self; 19] = [
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
        Self::NotLet,
        Self::Newer,
        Self::PlainTooLarge,
        Self::NotPostQuantum,
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
            Self::NotLet => "not-let",
            Self::Newer => "newer",
            Self::PlainTooLarge => "plain-too-large",
            Self::NotPostQuantum => "not-post-quantum",
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

    /// FAILS IF: a repository of another format is taken as this one, or
    /// refused in silence — a lower one is not ours, a newer one asks for an
    /// update.
    #[test]
    fn only_this_format_is_read() {
        assert_eq!(format_check(FORMAT_VERSION), Ok(()));
        assert_eq!(
            format_check(FORMAT_VERSION + 1).unwrap_err().why,
            Why::Newer
        );
        for lower in 0..FORMAT_VERSION {
            assert_eq!(format_check(lower).unwrap_err().why, Why::NotOurs);
        }
    }

    #[test]
    fn the_format_file_says_its_number() {
        assert_eq!(format_of(FORMAT), Some(FORMAT_VERSION));
        assert_eq!(format_of("ronoctua-belongings 1\n"), Some(1));
        assert_eq!(format_of("ronoctua-belongings 17\nmore"), Some(17));
        for not in [
            "",
            "hello",
            "ronoctua-belongings",
            "ronoctua-belongings x",
            "ronoctua-belongings -1",
        ] {
            assert_eq!(format_of(not), None, "{not}");
        }
    }
}
