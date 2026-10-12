//! The plain part's signature (feature `crypt`): which key this machine
//! holds the repository's plain part to, what the signature vouches for,
//! and the signing of what this machine sends ([`crate::plain`] says why).
//!
//! # Which key
//!
//! The person's passphrase opened `key.age` (a sync, a restore, `init`, the
//! identity the session kept): THAT signing key — the passphrase proves it
//! is theirs. Else the key root pinned on this machine
//! (`<machine dir>/local/signer.json`, read with `ronoctua-person`'s one
//! reader): a plain run asks nobody. Else none, and every change of the
//! repository's to a signed file is held ([`HeldWhy::NoKey`]). The
//! repository's own `plain/signer.pub` is never a key anybody trusts.
//!
//! # What goes up signed
//!
//! What a run leaves in the repository's `plain/` is signed by what this
//! machine vouches for: a file it holds itself (the person's own, or one
//! already taken under the signature), what it merged, what the person
//! decided to take — never a file of the repository's the signature did not
//! vouch for, which stays out of the new manifest and so stays held on every
//! machine. With the signing key at hand, a run that changes a signed file
//! signs anew (the counter one past every counter seen); without it, a
//! manifest this machine signed at a gesture (`sign`) still goes when it
//! covers exactly what goes; else this machine's changes to signed files
//! WAIT (`PlainBase::unsigned`) and the machines' records go alone.
//!
//! # Signing here, at a gesture
//!
//! `sign` ([`sign_verb`], `Op::Sign`) signs this machine's own plain part
//! with no network: the software centre before it stages a declaration, the
//! update before it stages, the view's "sign" for a brain module edited
//! here. It needs the identity — kept for the session, or the passphrase a
//! door wrote at that gesture — and never asks; without it, it is refused
//! (`no-passphrase`) and the change waits. What it signs is kept in
//! `state/person-signed/`, which a staging carries to root and a later push
//! carries up when it covers what goes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::str::FromStr as _;

use ronoctua_pq::sign::{Manifest, PublicKey, SigningKey};

use super::plain::{PlainFile, walk_here};
use super::{Engine, KEY_FILE};
use crate::crypt::Keyring;
use crate::places::Places;
use crate::plain::{
    DIR, HeldWhy, MANIFEST_FILE, PlainBase, SIGNATURE_FILE, SIGNER_FILE, Seen, signed, signed_path,
};
use crate::prefs::{Belongings, repository_is_plain};
use crate::running;
use crate::secret::{make_private_dir, write_private};
use crate::{Refusal, Why};

/// The most a manifest read from the repository or kept here may weigh.
const MANIFEST_MOST_BYTES: u64 = ronoctua_person::tree::caps::MANIFEST_MOST_BYTES;
/// The most its signature may.
const SIGNATURE_MOST_BYTES: u64 = ronoctua_person::tree::caps::SIGNATURE_MOST_BYTES;

/// The key root pinned on this machine (`local/signer.json` under the
/// machine directory), when one is — read as root reads it: the key must be
/// one and its fingerprint its own, else nothing is trusted.
#[must_use]
pub fn pinned_key() -> Option<PublicKey> {
    pinned_key_in(Path::new(env!("RONOCTUA_MACHINE_DIR")))
}

/// [`pinned_key`] of the machine directory `machine`.
#[must_use]
pub fn pinned_key_in(machine: &Path) -> Option<PublicKey> {
    let bytes = std::fs::read(machine.join(ronoctua_person::names::SIGNER)).ok()?;
    let pin: ronoctua_person::local::Signer = ronoctua_person::parse(&bytes).ok()?;
    let key = PublicKey::from_str(&pin.key).ok()?;
    (key.fingerprint() == pin.fingerprint).then_some(key)
}

/// A signed pair: the manifest's bytes and its signature's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Pair {
    /// The manifest, as written.
    pub json: Vec<u8>,
    /// The signature, in base64 with its line end.
    pub signature: Vec<u8>,
}

impl Pair {
    /// The pair in the folder `dir` (`signed.json`, `signed.sig`), when both
    /// are there, regular files under their caps.
    fn read_in(dir: &Path) -> Option<Self> {
        let read = |name: &str, most: u64| -> Option<Vec<u8>> {
            let path = dir.join(name);
            let meta = std::fs::symlink_metadata(&path).ok()?;
            if !meta.is_file() || meta.len() > most {
                return None;
            }
            std::fs::read(&path).ok()
        };
        Some(Self {
            json: read(MANIFEST_FILE, MANIFEST_MOST_BYTES)?,
            signature: read(SIGNATURE_FILE, SIGNATURE_MOST_BYTES)?,
        })
    }

    /// The manifest, when `key` signed it.
    fn open(&self, key: &PublicKey) -> Result<Manifest, HeldWhy> {
        ronoctua_pq::sign::open_manifest(&self.json, &self.signature, key)
            .map_err(|_| HeldWhy::BadSignature)
    }
}

/// What the repository's signature vouches for, on one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Vouched {
    /// The manifest the trusted key signed, not rolled back — or why not.
    pub manifest: Result<Manifest, HeldWhy>,
    /// Its pair as read, when it opened: what this machine keeps once it
    /// took what the repository had.
    pub pair: Option<Pair>,
}

impl Default for Vouched {
    fn default() -> Self {
        Self {
            manifest: Err(HeldWhy::Unsigned),
            pair: None,
        }
    }
}

impl Vouched {
    /// Whether the repository's change of the signed logical path `logical`
    /// to `there` (`None`: deleted there) is what the person signed — or the
    /// reason it is held.
    pub fn holds(&self, logical: &str, there: Option<&PlainFile>) -> Result<(), HeldWhy> {
        let manifest = self.manifest.as_ref().map_err(|why| *why)?;
        let rel = signed_path(logical).ok_or(HeldWhy::NotAsSigned)?;
        let listed = manifest.files.get(&rel);
        match (there, listed) {
            (Some(file), Some(sha)) if file.sha256 == *sha => Ok(()),
            (None, None) => Ok(()),
            _ => Err(HeldWhy::NotAsSigned),
        }
    }

    /// The counter the signature is at, when it holds.
    pub fn counter(&self) -> Option<u64> {
        self.manifest.as_ref().ok().map(|m| m.counter)
    }
}

/// How a run's push is signed.
pub(super) enum Signing<'k> {
    /// Nothing signed changed in what goes: the repository's signature stays.
    Unneeded,
    /// Signed anew with this key.
    With(&'k SigningKey),
    /// The manifest this machine signed at a gesture covers what goes: it
    /// goes as it is.
    Kept(Pair),
    /// No key, and nothing signed covers what goes: this machine's changes
    /// to signed files wait.
    Locked,
}

impl Engine<'_> {
    /// The key this run holds the plain part to: the one `ring` holds (the
    /// passphrase opened it), else the one root pinned.
    pub(super) fn trusted<'r>(&'r self, ring: Option<&'r Keyring>) -> Option<&'r PublicKey> {
        ring.map(|r| r.signing().public()).or(self.pinned.as_ref())
    }

    /// What the work tree's `plain/` signature vouches for under `trusted`,
    /// no lower than what this machine saw (`seen`).
    pub(super) fn vouched(&self, trusted: Option<&PublicKey>, seen: &Seen) -> Vouched {
        let dir = self.places.repo().join(DIR);
        let Some(pair) = Pair::read_in(&dir) else {
            return Vouched {
                manifest: Err(HeldWhy::Unsigned),
                pair: None,
            };
        };
        let Some(key) = trusted else {
            return Vouched {
                manifest: Err(HeldWhy::NoKey),
                pair: None,
            };
        };
        let manifest = pair.open(key).and_then(|m| {
            if m.counter < seen.floor(&m.signer) {
                Err(HeldWhy::RolledBack)
            } else {
                Ok(m)
            }
        });
        let pair = manifest.is_ok().then_some(pair);
        Vouched { manifest, pair }
    }

    /// The manifest this machine signed of its own plain part
    /// (`state/person-signed/`), when `trusted` signed it.
    pub(super) fn kept_signed(&self, trusted: Option<&PublicKey>) -> Option<(Pair, Manifest)> {
        let pair = Pair::read_in(&self.places.signed())?;
        let manifest = pair.open(trusted?).ok()?;
        Some((pair, manifest))
    }

    /// How a push whose signed files will be `going` (by path under
    /// `plain/`, with their SHA-256) is signed: not at all when the
    /// repository's signature already names exactly those; with `ring`'s
    /// key when there is one; with the manifest this machine signed at a
    /// gesture when it names exactly those and is newer than the
    /// repository's; else not, and this machine's changes wait.
    pub(super) fn signing<'k>(
        &self,
        going: &BTreeMap<String, String>,
        vouched: &Vouched,
        ring: Option<&'k Keyring>,
    ) -> Signing<'k> {
        if vouched.manifest.as_ref().is_ok_and(|m| m.files == *going) {
            return Signing::Unneeded;
        }
        if let Some(ring) = ring {
            return Signing::With(ring.signing());
        }
        let trusted = self.trusted(None);
        if let Some((pair, kept)) = self.kept_signed(trusted)
            && kept.files == *going
            && vouched.counter().is_none_or(|c| kept.counter > c)
        {
            return Signing::Kept(pair);
        }
        Signing::Locked
    }

    /// Signs `going` with `key`, one past every counter this machine knows
    /// of that key (`seen`, the repository's, the one kept here).
    pub(super) fn sign_going(
        &self,
        key: &SigningKey,
        going: &BTreeMap<String, String>,
        vouched: &Vouched,
        seen: &Seen,
    ) -> Result<(Pair, u64), Refusal> {
        let me = key.public().fingerprint();
        let kept = self
            .kept_signed(Some(key.public()))
            .map_or(0, |(_, m)| m.counter);
        let repo = vouched
            .manifest
            .as_ref()
            .ok()
            .filter(|m| m.signer == me)
            .map_or(0, |m| m.counter);
        let counter = seen.floor(&me).max(kept).max(repo).saturating_add(1);
        let manifest = Manifest {
            format: ronoctua_pq::sign::MANIFEST_FORMAT,
            counter,
            signed_at_s: self.now_s,
            signer: me,
            files: going.clone(),
        };
        let (json, signature) = ronoctua_pq::sign::sign_manifest(key, &manifest)
            .map_err(|_| Refusal::of(Why::Failed))?;
        Ok((Pair { json, signature }, counter))
    }

    /// Writes `pair` (and the signer's public key, for a screen) into the
    /// work tree's `plain/`, for the commit.
    pub(super) fn write_signed_there(&self, pair: &Pair, key: &PublicKey) -> Result<(), Refusal> {
        let dir = self.places.repo().join(DIR);
        let failed = |_| Refusal::of(Why::Failed);
        std::fs::create_dir_all(&dir).map_err(failed)?;
        std::fs::write(dir.join(MANIFEST_FILE), &pair.json).map_err(failed)?;
        std::fs::write(dir.join(SIGNATURE_FILE), &pair.signature).map_err(failed)?;
        std::fs::write(dir.join(SIGNER_FILE), format!("{key}\n")).map_err(failed)?;
        Ok(())
    }
}

/// What signing here did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", tag = "signed")]
pub enum SignedHere {
    /// This machine's plain part is signed now, at this counter.
    Signed {
        /// The manifest's counter.
        counter: u64,
    },
    /// What is kept here already names it as it is: nothing was signed.
    Unchanged,
}

impl Engine<'_> {
    /// Signs this machine's own plain part — under the run lock, with the
    /// identity the session kept or the passphrase a door wrote (kept for
    /// the session then, and its file deleted unless a prepared sync waits
    /// on it) — and keeps it in `state/person-signed/`. No network.
    ///
    /// # Errors
    ///
    /// [`Why::Busy`], [`Why::NoPassphrase`] with neither, [`Why::Empty`]
    /// before any sync made this machine's clone, what opening `key.age`
    /// refuses, [`Why::Failed`].
    pub fn sign_here(&self) -> Result<SignedHere, Refusal> {
        let _held = running::hold(self.places)?;
        let done = self.signed_here();
        super::forget_typed(self.places);
        done
    }

    fn signed_here(&self) -> Result<SignedHere, Refusal> {
        let opening = self.opening()?;
        if !self.places.repo().join(KEY_FILE).is_file() {
            return Err(Refusal::of(Why::Empty));
        }
        let ring = self.open_ring(&opening)?;
        self.keep_for_the_session(&ring, &opening);
        let key = ring.signing();
        let going = manifest_files(&walk_here(self.places).files);
        let kept = self.kept_signed(Some(key.public()));
        if kept.as_ref().is_some_and(|(_, m)| m.files == going) {
            return Ok(SignedHere::Unchanged);
        }
        let seen = self.plain_base().map(|b| b.seen).unwrap_or_default();
        let none = Vouched {
            manifest: Err(HeldWhy::Unsigned),
            pair: None,
        };
        let (pair, counter) = self.sign_going(key, &going, &none, &seen)?;
        keep_signed(self.places, &pair);
        Ok(SignedHere::Signed { counter })
    }
}

/// `sign`, as the CLI and the queue run it: this machine's settings, the
/// session's keyring, no network.
///
/// # Errors
///
/// [`Why::NoRepository`], [`Why::NoTools`], and what [`Engine::sign_here`]
/// refuses.
pub fn sign_verb(places: &Places) -> Result<SignedHere, Refusal> {
    let prefs: Belongings = super::read_prefs(places);
    if !prefs.has_repository() || repository_is_plain(&prefs.repository).is_err() {
        super::forget_typed(places);
        return Err(Refusal::of(Why::NoRepository));
    }
    let tools = ronoctua_git::Tools::from_env().ok_or(Refusal::of(Why::NoTools))?;
    let engine = Engine {
        vault: Some(&crate::unlock::Kernel::USER),
        pinned: pinned_key(),
        ..Engine::for_plain(places, &prefs, tools)
    };
    engine.sign_here()
}

/// Keeps `pair` as the manifest this machine signed or took of its plain
/// part — `0600` in a `0700` folder: it is the person's, not the world's.
/// Best effort: a copy not kept is a staging root keeps back until the next
/// run keeps one.
pub(super) fn keep_signed(places: &Places, pair: &Pair) {
    let dir = places.signed();
    if make_private_dir(&dir).is_err() {
        return;
    }
    let _ = write_private(&dir.join(MANIFEST_FILE), &pair.json);
    let _ = write_private(&dir.join(SIGNATURE_FILE), &pair.signature);
}

/// The signed files of a side (by logical path), as a manifest names them:
/// by path under `plain/`, with their SHA-256.
pub(super) fn manifest_files(files: &BTreeMap<String, PlainFile>) -> BTreeMap<String, String> {
    files
        .iter()
        .filter(|(logical, _)| signed(logical))
        .filter_map(|(logical, f)| signed_path(logical).map(|rel| (rel, f.sha256.clone())))
        .collect()
}

/// What a run vouches for of the work tree's signed files `there` (by
/// logical path): a file it holds `here` alike, one the repository's
/// signature names as it is, one this run wrote there (`sent`: from here,
/// or merged), one the person decided to take (`taken`). Anything else — a
/// change of the repository's this run held — stays out of what it signs.
pub(super) fn vouched_going(
    there: &BTreeMap<String, PlainFile>,
    here: &BTreeMap<String, PlainFile>,
    vouched: &Vouched,
    sent: &BTreeMap<String, String>,
    taken: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    there
        .iter()
        .filter(|(logical, _)| signed(logical))
        .filter(|(logical, f)| {
            here.get(*logical).is_some_and(|h| h.sha256 == f.sha256)
                || vouched.holds(logical, Some(f)).is_ok()
                || sent.get(*logical).is_some_and(|sha| *sha == f.sha256)
                || taken.contains(*logical)
        })
        .filter_map(|(logical, f)| signed_path(logical).map(|rel| (rel, f.sha256.clone())))
        .collect()
}

/// The plain base after a run, with what the run made of the signature:
/// held and why, waiting for a signature, the counter seen, the key the
/// passphrase opened.
pub(super) fn with_signature(
    mut base: PlainBase,
    held: BTreeMap<String, HeldWhy>,
    unsigned: Vec<String>,
    seen: Seen,
    signer: Option<&PublicKey>,
    before: Option<&PlainBase>,
) -> PlainBase {
    base.held = held;
    base.unsigned = unsigned;
    base.seen = seen;
    match signer {
        Some(key) => {
            base.signer = key.to_string();
            base.fingerprint = key.fingerprint();
        }
        None => {
            if let Some(before) = before {
                base.signer.clone_from(&before.signer);
                base.fingerprint.clone_from(&before.fingerprint);
            }
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(sha: &str) -> PlainFile {
        PlainFile {
            sha256: sha.to_owned(),
            size_bytes: 1,
            mode: 0o644,
        }
    }

    fn sha(text: &str) -> String {
        ronoctua_pq::sign::sha256_hex(text.as_bytes())
    }

    /// FAILS IF: a change of the repository's is taken for what the
    /// signature names when it is another content, a file it does not name,
    /// a deletion of one it names — or under a signature that did not hold.
    #[test]
    fn a_change_holds_only_as_signed() {
        let manifest = Manifest {
            format: ronoctua_pq::sign::MANIFEST_FORMAT,
            counter: 3,
            signed_at_s: 0,
            signer: "a".repeat(64),
            files: BTreeMap::from([("person/options.json".to_owned(), sha("signed"))]),
        };
        let v = Vouched {
            manifest: Ok(manifest),
            pair: None,
        };
        let options = "state/person/options.json";
        assert_eq!(v.holds(options, Some(&file(&sha("signed")))), Ok(()));
        assert_eq!(
            v.holds(options, Some(&file(&sha("tampered")))),
            Err(HeldWhy::NotAsSigned)
        );
        assert_eq!(
            v.holds(options, None),
            Err(HeldWhy::NotAsSigned),
            "deleted, and signed"
        );
        let module = "config/brain-modules/my-x/module.rs";
        assert_eq!(
            v.holds(module, Some(&file(&sha("x")))),
            Err(HeldWhy::NotAsSigned)
        );
        assert_eq!(
            v.holds(module, None),
            Ok(()),
            "a deletion the signature names"
        );
        let held = Vouched {
            manifest: Err(HeldWhy::RolledBack),
            pair: None,
        };
        assert_eq!(held.holds(options, None), Err(HeldWhy::RolledBack));
    }

    /// FAILS IF: what a run signs takes in a change of the repository's the
    /// signature did not vouch for — the one way a held file would come back
    /// signed by the person — or leaves out what this machine holds, sent,
    /// or the person took.
    #[test]
    fn a_signature_never_vouches_for_a_held_change() {
        let v = Vouched {
            manifest: Err(HeldWhy::BadSignature),
            pair: None,
        };
        let there = BTreeMap::from([
            ("state/person/options.json".to_owned(), file(&sha("held"))),
            (
                "state/person/software/declared.json".to_owned(),
                file(&sha("same")),
            ),
            (
                "config/brain-modules/my-x/module.rs".to_owned(),
                file(&sha("merged")),
            ),
            (
                "config/brain-modules/my-y/module.rs".to_owned(),
                file(&sha("taken")),
            ),
            (
                "state/person/machines/m.json".to_owned(),
                file(&sha("record")),
            ),
        ]);
        let here = BTreeMap::from([
            ("state/person/options.json".to_owned(), file(&sha("mine"))),
            (
                "state/person/software/declared.json".to_owned(),
                file(&sha("same")),
            ),
        ]);
        let sent = BTreeMap::from([(
            "config/brain-modules/my-x/module.rs".to_owned(),
            sha("merged"),
        )]);
        let taken = BTreeSet::from(["config/brain-modules/my-y/module.rs".to_owned()]);
        let going = vouched_going(&there, &here, &v, &sent, &taken);
        assert_eq!(
            going.keys().map(String::as_str).collect::<Vec<_>>(),
            [
                "brain-modules/my-x/module.rs",
                "brain-modules/my-y/module.rs",
                "person/software/declared.json",
            ]
        );
    }
}
