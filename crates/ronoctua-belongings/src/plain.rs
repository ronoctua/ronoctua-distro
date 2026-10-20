//! The plain part (feature `prefs`): what the repository keeps IN CLEAR, under
//! `plain/`, beside the sealed part.
//!
//! # What is plain, and why
//!
//! The person's declared data (`state/person`: the programs they declared and
//! declined, each machine's record, their options) and their brain modules
//! (`config/brain-modules`) — the roots of the `plain` table of
//! `assets/belongings.nix` ([`crate::scope::plain_roots`]), each kept at `plain/<at>/…`. They are
//! read by the machine itself, BEFORE anybody types a passphrase: the update
//! fetches the declarations, the installer stages them before
//! `nixos-install`, the machine publishes its record after a switch. So they
//! travel with no identity (`Op::SyncPlain`, `plain pull`/`push`), and the
//! screen says that whoever can read the repository can read them.
//!
//! # Signed by the person
//!
//! Whoever can WRITE the repository — any machine's deploy key, the token,
//! the git host — could change them, and every machine would take code and
//! root's options at its next update. So the person SIGNS the plain part
//! (ML-DSA-65, the key born with the repository and kept in `key.age` beside
//! the identity): `plain/signed.json` names every file the signature covers
//! ([`signed`]) with its SHA-256, a counter that only grows, when and by
//! whom; `plain/signed.sig` is its signature; `plain/signer.pub` the public
//! key, for a screen to show — and nobody trusts that copy. A change of the
//! repository's to a signed file is taken by itself only when the signature
//! holds under the key THIS machine trusts — the one root pinned, or the one
//! the person's passphrase just opened — names that file as it is, and is
//! not older than one this machine saw. Anything else is HELD, with the
//! reason ([`HeldWhy`]), until it is signed or the person takes it on this
//! machine. The machines' records are not signed: each machine publishes its
//! own without the identity, and nothing reads them as configuration.
//!
//! # The git tree is its own index
//!
//! A plain file never enters `index.age`. Its base for the next three-way
//! merge is what the last run left in `last.json`'s plain section
//! ([`PlainBase`]): its SHA-256 (what "changed" compares) and git's own blob
//! id (where the base's CONTENT is read back from, `git cat-file`) — so the
//! plain part needs no cache and no key.
//!
//! # What is never plain
//!
//! A file past `plainMostKiB` ([`crate::kept::PLAIN_MOST_BYTES`]) is left out
//! on both sides and listed ([`PlainBase::too_large`], `Why::PlainTooLarge`
//! beside it on screen); a link, a socket or a pipe is not data and is never
//! carried; a draft (`.*`, `*.new`) is never kept, as anywhere else.
//!
//! `scope::plain_roots` and `Places::resolve` are what the machine package
//! reads: the scope is the one reader of the tables (`crate::scope`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use crate::kept::PlainRoot;
use crate::places::covers;

use crate::scope::plain_roots;

/// The repository's folder of the plain part.
pub const DIR: &str = "plain";

/// The repository's manifest of the plain part, under `plain/`
/// (`ronoctua_pq::sign::Manifest`) — the staged tree's name for it, since
/// the two have one shape.
pub const MANIFEST_FILE: &str = ronoctua_person::tree::MANIFEST;
/// Its detached signature, beside it.
pub const SIGNATURE_FILE: &str = ronoctua_person::tree::SIGNATURE;
/// The signer's public key, in clear beside them — for a screen to show; no
/// machine trusts it (`local/signer.json` is what root trusts).
pub const SIGNER_FILE: &str = "signer.pub";
/// Where this machine keeps the manifest the person signed of its own plain
/// part, under the state root (`ronoctua_person::names::SIGNED`).
pub const SIGNED_DIR: &str = ronoctua_person::names::SIGNED;

/// The path of a plain logical path under `plain/` (`person/options.json`),
/// the staged tree's and the manifest's. Pure.
#[must_use]
pub fn signed_path(logical: &str) -> Option<String> {
    repo_path(logical)?
        .strip_prefix(DIR)?
        .strip_prefix('/')
        .map(str::to_owned)
}

/// Whether the person's signature covers the plain logical path `logical`:
/// everything under the plain roots but the machines' records
/// (`ronoctua_person::tree::signed`, the one rule root holds a staging to).
/// Pure.
#[must_use]
pub fn signed(logical: &str) -> bool {
    signed_path(logical).is_some_and(|rel| ronoctua_person::tree::signed(&rel))
}

/// The logical path of the person's declined programs: the plain root kept
/// at `plain/person/` (`ronoctua_person::tree::PERSON`) joined with
/// `ronoctua_person::names::DECLINED` — `state/person/software/declined.json`.
/// `None` only for a build whose plain table lost the person's root. Pure.
#[must_use]
pub fn declined_logical() -> Option<String> {
    plain_roots()
        .iter()
        .find(|r| r.at == ronoctua_person::tree::PERSON)
        .map(|r| format!("{}/{}", r.path, ronoctua_person::names::DECLINED))
}

/// Why a change of the repository's to a signed file was not taken by
/// itself — each a sentence on screen (`belongings-held-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeldWhy {
    /// The repository's plain part carries no signature.
    Unsigned,
    /// There is no key to hold it to: this machine pinned none, and the
    /// person's identity is not open.
    NoKey,
    /// The signature is not the person's key's.
    BadSignature,
    /// The signature is the person's, over an older plain part than one
    /// this machine already took.
    RolledBack,
    /// The signature holds, and this file is not what it signed.
    NotAsSigned,
}

impl HeldWhy {
    /// Every one, for the census of their sentences.
    pub const ALL: [Self; 5] = [
        Self::Unsigned,
        Self::NoKey,
        Self::BadSignature,
        Self::RolledBack,
        Self::NotAsSigned,
    ];

    /// Its kebab-case name, the suffix of its sentence's key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Unsigned => "unsigned",
            Self::NoKey => "no-key",
            Self::BadSignature => "bad-signature",
            Self::RolledBack => "rolled-back",
            Self::NotAsSigned => "not-as-signed",
        }
    }
}

/// The highest counter of the person's this machine took or signed, and
/// the key it was under — a new key (the repository made again) counts from
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Seen {
    /// The signer's fingerprint.
    pub signer: String,
    /// Its highest counter.
    pub counter: u64,
}

impl Seen {
    /// What a manifest of `signer` must at least count: what was seen of
    /// that key, nothing for another.
    #[must_use]
    pub fn floor(&self, signer: &str) -> u64 {
        if self.signer == signer {
            self.counter
        } else {
            0
        }
    }

    /// This, having seen `counter` of `signer`.
    #[must_use]
    pub fn after(&self, signer: &str, counter: u64) -> Self {
        Self {
            signer: signer.to_owned(),
            counter: counter.max(self.floor(signer)),
        }
    }
}

// ── anchor: L09 ──
/// The plain root a logical path lies under, if one does. Pure.
#[must_use]
pub fn root_of(logical: &str) -> Option<&'static PlainRoot> {
    plain_roots().iter().find(|r| covers(r.path, logical))
}

/// Where a plain logical path is kept in the repository: `plain/<at>/<rest>`
/// (`state/person/options.json` → `plain/person/options.json`). `None` for a
/// path no plain root covers, or one that is not logical. Pure.
#[must_use]
pub fn repo_path(logical: &str) -> Option<String> {
    if !crate::places::is_logical(logical) {
        return None;
    }
    let root = root_of(logical)?;
    let rest = logical[root.path.len()..].trim_start_matches('/');
    Some(if rest.is_empty() {
        format!("{DIR}/{}", root.at)
    } else {
        format!("{DIR}/{}/{rest}", root.at)
    })
}

/// The logical path a repository path of the plain part is — the inverse of
/// [`repo_path`]. `None` for anything outside `plain/`, under a folder no root
/// of this build names (a newer distro's: left alone), or that climbs. Pure.
#[must_use]
pub fn logical_of_repo(relative: &str) -> Option<String> {
    let inside = relative.strip_prefix(DIR)?.strip_prefix('/')?;
    let root = plain_roots()
        .iter()
        .filter(|r| covers(r.at, inside))
        .max_by_key(|r| r.at.len())?;
    let rest = inside[root.at.len()..].trim_start_matches('/');
    let logical = if rest.is_empty() {
        root.path.to_owned()
    } else {
        format!("{}/{rest}", root.path)
    };
    (crate::places::is_logical(&logical) && root_of(&logical) == Some(root)).then_some(logical)
}

/// One plain file as the last run left it on both sides.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlainKnown {
    /// Its SHA-256, hexadecimal — what "changed" compares.
    pub sha256: String,
    /// git's blob id of it — where its content is read back from as the
    /// base of a merge (`git cat-file`).
    pub blob: String,
    /// Its size, in bytes.
    pub size_bytes: u64,
}

/// `last.json`'s plain section: the base of the plain part's next merge, and
/// what its last run left out.
///
/// It names its own repository because a plain run may be the first thing a
/// machine does with one (`sync-plain` before any sync): the rest of
/// `last.json` then stays empty, so nothing reads it as a sync of the sealed
/// part — the reminder still says "restore", the view still says "never".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PlainBase {
    /// The repository the base is of. Another one now is a first run.
    pub repository: String,
    /// Every plain file both sides had alike, by logical path.
    pub files: BTreeMap<String, PlainKnown>,
    /// The plain files both sides changed and no merge joined, left as each
    /// side has them — a run with nobody to ask (`sync-plain`, `plain pull`)
    /// leaves them for a sync with the person, whose plan lists them.
    pub conflicts: Vec<String>,
    /// The plain files left out for their size (past `plainMostKiB`), on
    /// either side.
    pub too_large: Vec<String>,
    /// The repository's changes to signed files this machine did not take,
    /// and why ([`HeldWhy`]) — left as this machine has them, and asked in a
    /// sync with the person.
    pub held: BTreeMap<String, HeldWhy>,
    /// This machine's changes to signed files that wait for a signature:
    /// the person's identity was not open to sign them, so they did not go.
    pub unsigned: Vec<String>,
    /// The highest counter of the person's this machine took or signed.
    pub seen: Seen,
    /// The public key the person's passphrase opened at the last run that
    /// opened `key.age` — what the view offers to pin, never the
    /// repository's copy of it. Empty before such a run.
    pub signer: String,
    /// Its fingerprint (the SHA-256 of the raw key, hexadecimal): what the
    /// view shows and compares with the machine's pin. Empty with no key.
    pub fingerprint: String,
}

impl PlainBase {
    /// The base when it is of `repository` — else none: a first run.
    #[must_use]
    pub fn of(&self, repository: &str) -> Option<&Self> {
        (self.repository == repository && !repository.is_empty()).then_some(self)
    }
}

/// What a plain run did — `plain pull --json` and `plain push --json` print
/// it, one line, for the machine package and the update.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Report {
    /// Plain files written or deleted here, by logical path.
    pub changed: Vec<String>,
    /// Plain files sent to the repository.
    pub pushed: Vec<String>,
    /// Changed on both sides and not joined: each left as this machine has
    /// it — conflicts never block.
    pub conflicts: Vec<String>,
    /// Left out for their size.
    pub too_large: Vec<String>,
    /// The repository's changes to signed files not taken, and why.
    pub held: BTreeMap<String, HeldWhy>,
    /// This machine's changes to signed files that wait for a signature.
    pub unsigned: Vec<String>,
    /// What became of the installer's choice (`plain --decline`), when it
    /// gave one.
    pub choice: Option<ChoiceThere>,
    /// The programs the installer's choice kept that the person declined on
    /// every machine: the person's own "everywhere", which a tick in the
    /// installer does not undo — they stay declined.
    pub kept_everywhere: Vec<String>,
}

/// What became of the installer's choice in the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChoiceThere {
    /// Said, signed and pushed by this run.
    Pushed,
    /// The repository said it already, and nothing was pushed: an attempt
    /// before — one whose push went in and was reported failed, too — took
    /// it up. Either way the choice is in the repository.
    AlreadyThere,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the plain table loses a root the machine package reads, or a
    /// logical path and its repository path stop being each other's.
    #[test]
    fn a_plain_path_and_its_place_in_the_repository_are_each_others() {
        assert!(
            plain_roots()
                .iter()
                .any(|r| r.path == "state/person" && r.at == "person"),
            "the person's data is not plain"
        );
        assert!(
            plain_roots()
                .iter()
                .any(|r| r.path == "config/brain-modules" && r.at == "brain-modules"),
            "the person's brain modules are not plain"
        );
        for (logical, there) in [
            (
                "state/person/software/declared.json",
                "plain/person/software/declared.json",
            ),
            (
                "config/brain-modules/my-tea/module.rs",
                "plain/brain-modules/my-tea/module.rs",
            ),
            ("state/person", "plain/person"),
        ] {
            assert_eq!(repo_path(logical).as_deref(), Some(there), "{logical}");
            assert_eq!(logical_of_repo(there).as_deref(), Some(logical), "{there}");
        }
        for not in [
            "config/theme.json",
            "state/personal/x",
            "state/person/../x",
            "home/person/x",
        ] {
            assert_eq!(repo_path(not), None, "{not}");
        }
        // THE DECLINES the installer signs in are the person's own file.
        let declined = declined_logical().unwrap();
        assert_eq!(declined, "state/person/software/declined.json");
        assert_eq!(
            repo_path(&declined).as_deref(),
            Some("plain/person/software/declined.json")
        );
        assert!(signed(&declined));
        for not in [
            "plain",
            "plain/",
            "plain/elsewhere/x",
            "plain/person/../x",
            "plainer/person/x",
            "chunks/ab/cd.age",
        ] {
            assert_eq!(logical_of_repo(not), None, "{not}");
        }
    }

    #[test]
    fn a_report_crosses_one_line_whole() {
        let report = Report {
            changed: vec!["state/person/options.json".to_owned()],
            pushed: vec![],
            conflicts: vec!["state/person/software/declared.json".to_owned()],
            too_large: vec!["config/brain-modules/my-big/blob.bin".to_owned()],
            held: BTreeMap::from([(
                "config/brain-modules/my-x/module.rs".to_owned(),
                HeldWhy::BadSignature,
            )]),
            unsigned: vec!["state/person/options.json".to_owned()],
            choice: Some(ChoiceThere::AlreadyThere),
            kept_everywhere: vec!["vlc".to_owned()],
        };
        let line = serde_json::to_string(&report).unwrap();
        assert!(!line.contains('\n'));
        assert!(line.contains("\"too-large\""), "{line}");
        assert!(line.contains("\"choice\":\"already-there\""), "{line}");
        assert_eq!(serde_json::from_str::<Report>(&line).unwrap(), report);
    }

    /// FAILS IF: the signature leaves a declaration, an option or a brain
    /// module out, covers a machine's record — or the belongings and root
    /// disagree on a path's place under `plain/`.
    #[test]
    fn the_signature_covers_the_persons_data_but_the_machines_records() {
        for yes in [
            "state/person/options.json",
            "state/person/software/declared.json",
            "state/person/software/icons/a.png",
            "config/brain-modules/my-tea/module.rs",
        ] {
            assert!(signed(yes), "{yes}");
        }
        for no in [
            "state/person/machines/laptop.json",
            "config/theme.json",
            "state/person-signed/signed.json",
            "home/.ssh/id_ed25519",
        ] {
            assert!(!signed(no), "{no}");
        }
        assert_eq!(
            signed_path("config/brain-modules/my-tea/module.rs").as_deref(),
            Some("brain-modules/my-tea/module.rs")
        );
        for why in HeldWhy::ALL {
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
    }

    /// FAILS IF: a counter of one key bars another key's manifests (a
    /// repository made again counts from nothing), or seeing a lower counter
    /// lowers the floor.
    #[test]
    fn a_counter_is_of_its_key() {
        let seen = Seen::default().after("aaaa", 7);
        assert_eq!(seen.floor("aaaa"), 7);
        assert_eq!(seen.floor("bbbb"), 0);
        assert_eq!(seen.after("aaaa", 3).counter, 7);
        assert_eq!(
            seen.after("bbbb", 3),
            Seen {
                signer: "bbbb".to_owned(),
                counter: 3
            }
        );
    }

    #[test]
    fn a_base_is_only_of_its_repository() {
        let base = PlainBase {
            repository: "git@example.org:a/b.git".to_owned(),
            ..PlainBase::default()
        };
        assert!(base.of("git@example.org:a/b.git").is_some());
        assert!(base.of("git@example.org:a/c.git").is_none());
        assert!(PlainBase::default().of("").is_none());
    }
}
