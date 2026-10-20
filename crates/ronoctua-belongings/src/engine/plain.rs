//! The plain part, carried (feature `crypt`): the person's declared data and
//! brain modules, in clear under `plain/` of the repository
//! ([`crate::plain`]).
//!
//! # Three sides, as for the sealed part
//!
//! The BASE is `last.json`'s plain section (each file's SHA-256 and git's
//! blob of it), OURS the files under the plain roots here, THEIRS the work
//! tree's `plain/` at the repository's head. Per path the table of
//! `crate::merge` holds; a change on both sides is merged when a merge can
//! (`engine/merging.rs`: the person's software lists by id, JSON by field,
//! text by hunk), the base read back from git (`cat-file`) — no key needed.
//! An EMPTY repository has no base whatever `last.json` remembers of its
//! address (a repository made again at the same place): the run is a first
//! one, and nothing here is deleted.
//!
//! # With a person, and without
//!
//! In a SYNC (`prepare`/`apply`) the plain part rides in the same commit as
//! the sealed one, and what clashes is a conflict of the plan like any other:
//! the person decides it. A PLAIN RUN — `sync-plain` (the queue, the
//! automatic watch), `plain pull`/`plain push` (the update and the machine
//! package) — asks nobody: no identity, no passphrase, `index.age` and every
//! chunk untouched. What clashes there is LEFT as each side has it, its base
//! kept, and named ([`crate::plain::PlainBase::conflicts`]); the next sync
//! with the person asks. Conflicts never block. A plain run speaks in
//! `status.json` only where no run a door started has spoken
//! ([`Engine::sync_plain`]).
//!
//! # Taken only as the person signed it
//!
//! The plain part is signed (`crate::plain`, `super::signing`): a change of
//! the repository's to a signed file — a declaration, the options, a brain
//! module — is pulled, merged or deleted here only when the signature holds
//! under the key this run trusts and names that file as it is. Anything
//! else is a conflict with no suggestion and no merge, its reason kept
//! ([`crate::plain::HeldWhy`]): left in a plain run, asked in a sync, until
//! it is signed or the person takes it on THIS machine. What this machine
//! sends is signed by what it vouches for — with the key at hand, or the
//! manifest it signed at a gesture — and when neither covers it, its changes
//! to signed files wait (`unsigned`) while the machines' records go.
//!
//! # Read once, and nothing written over a file that moved
//!
//! This machine's side is read ONCE, by the walk the plan is made of
//! ([`PlainPlan::here`]); the base after the run is computed from that plan
//! and from what the run did, never from a second walk. What a push sent is
//! read when it is sent, and what it carried — not what the file holds by
//! the time the push is in — is that path's base ([`Landing::sent`]): a file
//! changed again meanwhile is a change of this machine's the next run sends.
//! And before anything lands here, the destination must still hold what the
//! plan read: one the software centre or the machine's record wrote in
//! between is left alone ([`Landed::moved`]), its base kept, so the next run
//! sees a change on both sides and merges or asks.
//!
//! # Nothing written here before the push is in
//!
//! As the sealed part: a push another machine beat leaves this machine as it
//! was, and the next attempt merges against the same base. A plain run sets
//! nothing aside (`replaced/` is the sealed sync's): it only takes what the
//! repository changed over what this machine did not, or a merge holding
//! both — nothing of this machine's is lost. A sync with the person sets
//! aside what a decision overwrites, as for any path.
//!
//! # What is never plain
//!
//! A link, a socket or a pipe is not data; a draft (`.*`, `*.new`) is never
//! kept; a file past `plainMostKiB` is left out on BOTH sides and named
//! ([`crate::plain::PlainBase::too_large`]), and its base kept; a folder this
//! machine cannot read is left alone on both sides.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use ronoctua_git::{Credentials, Repo, Tools};
use ronoctua_person::choice::Choice;
use ronoctua_person::declined::Declined;

use super::merging::{Joined, Merges, Three, Tried, write_with_mode};
use super::signing::{self, Pair, Signing, Vouched};
use super::{COMMIT_MESSAGE, Engine, TRIES, step};
use crate::chunks::Written;
use crate::crypt::Keyring;
use crate::kept;
use crate::order::{Beat, Phase};
use crate::places::{Places, covers};
use crate::plain::{HeldWhy, PlainBase, PlainKnown, Report, Seen, signed};
use crate::plan::{Conflict, Decision, Decisions, Kind, Plan, Side, Step};
use crate::prefs::{Auth, Belongings, repository_is_plain};
use crate::running;
use crate::status::{Last, Outcome, Status};
use crate::walk;
use crate::{Refusal, Why};

/// How deep a walk of the plain part goes.
const DEEPEST: usize = 64;

/// The mode a plain file pulled here takes when there was none: the
/// person's data, readable as any file they write.
const PLAIN_MODE: u32 = 0o644;

/// One plain file as one side has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PlainFile {
    /// Its SHA-256.
    pub sha256: String,
    /// Its size, in bytes.
    pub size_bytes: u64,
    /// Its permission bits.
    pub mode: u32,
}

/// One side of the plain part, read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PlainSide {
    /// Every plain file, by logical path.
    pub files: BTreeMap<String, PlainFile>,
    /// Left out for their size.
    pub too_large: BTreeSet<String>,
    /// Paths (files or folders) that could not be read.
    pub unreadable: BTreeSet<String>,
}

/// What the plain part's merge planned.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct PlainPlan {
    /// The base it was made against.
    pub base: Option<PlainBase>,
    /// This machine's plain files as the plan read them — what a land
    /// checks its destination against, and what the base after the run is
    /// computed from: the only read of this side.
    pub here: BTreeMap<String, PlainFile>,
    /// The work tree's, at the repository's head.
    pub there: BTreeMap<String, PlainFile>,
    /// The paths moved without a question.
    pub steps: Vec<(String, Step)>,
    /// The paths both sides changed and no merge joined.
    pub conflicts: Vec<String>,
    /// Left out for their size, on either side.
    pub too_large: BTreeSet<String>,
    /// Left alone: they could not be read here.
    pub unreadable: BTreeSet<String>,
    /// What the repository's signature vouches for, under the key this run
    /// trusts.
    pub vouched: Vouched,
    /// The highest counter of the person's this machine saw before the run.
    pub seen: Seen,
    /// The repository's changes to signed files held, and why.
    pub held: BTreeMap<String, HeldWhy>,
    /// The signing key's public half, when the person's passphrase opened
    /// `key.age` for this run — what the view offers to pin.
    pub signer: Option<ronoctua_pq::sign::PublicKey>,
}

/// What lands here once the push is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Land {
    /// The repository's file.
    Theirs,
    /// Deleted.
    Delete,
    /// What a merge joined.
    Joined,
}

/// What a push carried of one plain file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Sent {
    /// The SHA-256 of the bytes sent.
    pub sha256: String,
    /// How many, in bytes.
    pub size_bytes: u64,
}

impl Sent {
    /// What `bytes` are, sent.
    fn of(bytes: &[u8]) -> Self {
        Self {
            sha256: super::merging::sha256(bytes),
            size_bytes: bytes.len() as u64,
        }
    }
}

/// What a carrying of the plain part did to the work tree, and what it
/// leaves to land here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Landing {
    /// What lands here, in order.
    pub lands: Vec<(String, Land)>,
    /// What went to the repository.
    pub pushed: Vec<String>,
    /// What each path sent carried — `None` for one deleted there: its base
    /// once the push is in, whatever the file here holds by then.
    pub sent: BTreeMap<String, Option<Sent>>,
    /// The conflicts a decision settled.
    pub decided: BTreeSet<String>,
    /// The signature that went up with the push, and its counter — what
    /// this machine keeps as its own once the push is in.
    pub signed: Option<(Pair, u64)>,
    /// This machine's changes to signed files that did not go: nothing
    /// covered them with a signature.
    pub unsigned: Vec<String>,
}

/// What landing did here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Landed {
    /// Written or deleted here.
    pub changed: Vec<String>,
    /// What could not be: its base stays, and the next run tries again.
    pub unwritten: BTreeSet<String>,
    /// What this machine changed after the plan read it (the software
    /// centre, the machine's record): left as it is, its base kept — the
    /// next run sees a change on both sides.
    pub moved: BTreeSet<String>,
}

/// What a path's base becomes after a run.
enum After<'s> {
    /// What both sides hold now: this SHA-256, of this size.
    Holds(&'s str, u64),
    /// Nothing, on either side.
    Gone,
    /// The base it had: a change is still a change next time.
    Kept,
}

impl Engine<'_> {
    /// The plain base `last.json` keeps for this repository; `None` before
    /// any run with it.
    pub(super) fn plain_base(&self) -> Option<PlainBase> {
        Last::read(&self.places.last()).and_then(|l| l.plain.of(&self.prefs.repository).cloned())
    }

    /// Plans the plain part against the work tree (checked out at the
    /// repository's head) and `base` — `None` for a first run, which deletes
    /// nothing: steps, merges into `merges`, and — with a `plan`, a sync's —
    /// every conflict as a conflict of it. A plain conflict names no
    /// machine: no plain commit says which one wrote it.
    ///
    /// A change of the repository's to a signed file is planned only when
    /// the repository's signature holds under the key this run trusts —
    /// `ring`'s, when the person's passphrase opened `key.age`, else the one
    /// root pinned — and names that file as it is: anything else becomes a
    /// conflict with no suggestion and no merge, its reason kept
    /// ([`HeldWhy`]).
    pub(super) fn plan_plain(
        &self,
        base: Option<PlainBase>,
        merges: &mut Merges,
        plan: Option<&mut Plan>,
        ring: Option<&Keyring>,
    ) -> Result<PlainPlan, Refusal> {
        let here = walk_here(self.places);
        let there = walk_there(&self.places.repo());
        let seen = base.as_ref().map(|b| b.seen.clone()).unwrap_or_default();
        let vouched = self.vouched(self.trusted(ring), &seen);
        let mut out = PlainPlan {
            too_large: here.too_large.union(&there.too_large).cloned().collect(),
            unreadable: here.unreadable.clone(),
            base: base.clone(),
            signer: ring.map(|r| r.signing().public().clone()),
            seen,
            ..PlainPlan::default()
        };
        let mut conflicts: Vec<Conflict> = Vec::new();
        let mut paths: BTreeSet<&str> = here.files.keys().map(String::as_str).collect();
        paths.extend(there.files.keys().map(String::as_str));
        if let Some(b) = &base {
            paths.extend(b.files.keys().map(String::as_str));
        }
        for path in paths {
            if out.too_large.contains(path) || out.unreadable.iter().any(|u| covers(u, path)) {
                continue;
            }
            let l = here.files.get(path);
            let r = there.files.get(path);
            let b = base.as_ref().and_then(|b| b.files.get(path));
            if sha(l) == sha(r) {
                if l.is_some() {
                    out.steps.push((path.to_owned(), Step::Same));
                }
                continue;
            }
            // WHAT THE PERSON SIGNED, AND ONLY THAT, comes from the
            // repository by itself: a change of the repository's to a
            // signed file the signature does not vouch for is asked in every
            // run, with no suggestion and no merge.
            let held = if signed(path) {
                vouched.holds(path, r).err()
            } else {
                None
            };
            let conflict = |suggested: Option<Decision>, why: Option<HeldWhy>| Conflict {
                path: path.to_owned(),
                local: l.map(side_of_file),
                remote: r.map(side_of_file),
                suggested,
                held: why,
                ..Conflict::default()
            };
            if base.is_none() {
                // A FIRST RUN WITH THIS REPOSITORY: nothing is deleted, and
                // what differs starts as "take the repository's".
                match (l, r) {
                    (Some(_), None) => out.steps.push((path.to_owned(), Step::Push)),
                    (None, Some(_)) if held.is_none() => {
                        out.steps.push((path.to_owned(), Step::Pull));
                    }
                    _ => {
                        if let Some(why) = held {
                            out.held.insert(path.to_owned(), why);
                        }
                        conflicts.push(conflict(
                            held.is_none().then_some(Decision::TakeRemote),
                            held,
                        ));
                    }
                }
                continue;
            }
            let base_sha = b.map(|b| b.sha256.as_str());
            let changed_here = sha(l) != base_sha;
            let changed_there = sha(r) != base_sha;
            let step = match (changed_here, changed_there, l, r) {
                (_, true, _, _) if held.is_some() => {
                    if let Some(why) = held {
                        out.held.insert(path.to_owned(), why);
                    }
                    conflicts.push(conflict(None, held));
                    continue;
                }
                (false, true, _, Some(_)) => Step::Pull,
                (false, true, _, None) => Step::DeleteLocal,
                (true, false, Some(_), _) => Step::Push,
                (true, false, None, _) => Step::DeleteRemote,
                _ => {
                    // BOTH CHANGED: joined when a merge can, else asked.
                    let mut c = conflict(None, None);
                    if let Some(three) = self.plain_three(path, b, l)? {
                        match self.merge_three(path, &three)? {
                            Tried::Joined(bytes) => {
                                merges.joined.insert(
                                    path.to_owned(),
                                    Joined {
                                        bytes,
                                        mode: three.mode,
                                    },
                                );
                                out.steps.push((path.to_owned(), Step::Merge));
                                continue;
                            }
                            Tried::Clashed(merging) => {
                                c.merge = Some(merging);
                                merges.three.insert(path.to_owned(), three);
                            }
                            Tried::Not => {}
                        }
                    }
                    conflicts.push(c);
                    continue;
                }
            };
            out.steps.push((path.to_owned(), step));
        }
        out.conflicts = conflicts.iter().map(|c| c.path.clone()).collect();
        if let Some(plan) = plan {
            plan.conflicts.extend(conflicts);
        }
        out.here = here.files;
        out.there = there.files;
        out.vouched = vouched;
        Ok(out)
    }

    /// The three sides of a plain path both changed, when all three are
    /// there: the base from git (`cat-file`), this machine's file, the work
    /// tree's.
    fn plain_three(
        &self,
        logical: &str,
        base: Option<&PlainKnown>,
        here: Option<&PlainFile>,
    ) -> Result<Option<Three>, Refusal> {
        let (Some(base), Some(here)) = (base, here) else {
            return Ok(None);
        };
        let Some(there) = crate::plain::repo_path(logical) else {
            return Ok(None);
        };
        let Ok(theirs) = std::fs::read(self.places.repo().join(there)) else {
            return Ok(None);
        };
        let Some(ours) = self
            .places
            .resolve(logical)
            .and_then(|p| std::fs::read(p).ok())
            .filter(|o| super::merging::sha256(o) == here.sha256)
        else {
            return Ok(None);
        };
        let Some(base_bytes) = self
            .blob_bytes(&base.blob)
            .filter(|b| super::merging::sha256(b) == base.sha256)
        else {
            return Ok(None);
        };
        Ok(Some(Three {
            base: base_bytes,
            ours,
            theirs,
            mode: here.mode,
        }))
    }

    /// A blob of the clone, whole; `None` when it is not there (a history
    /// compacted past it) or the id is not one.
    fn blob_bytes(&self, id: &str) -> Option<Vec<u8>> {
        let hex = |s: &str| s.bytes().all(|b| b.is_ascii_hexdigit());
        if !(id.len() == 40 || id.len() == 64) || !hex(id) {
            return None;
        }
        let repo = Repo::discover(&self.places.repo())?.with_tools(self.tools.clone());
        self.git(&repo, false, "cat-file", &["blob", id]).ok()
    }

    /// Carries the plain plan into the work tree — what this machine had,
    /// and what a decision or a merge made, for the commit (only when
    /// `push`), signed (`ring`'s key, or the manifest this machine signed at
    /// a gesture) or, when nothing signs it, this machine's changes to
    /// signed files taken back out to wait — and says what lands here once
    /// the push is in, and what each path sent carried.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when the work tree could not be written or signed.
    pub(super) fn carry_plain(
        &self,
        plain: &PlainPlan,
        decisions: &Decisions,
        merges: &Merges,
        push: bool,
        ring: Option<&Keyring>,
    ) -> Result<Landing, Refusal> {
        let mut landing = Landing::default();
        let mut moves: Vec<(String, Step)> = plain.steps.clone();
        // WHAT THE PERSON DECIDED TO TAKE of the repository's: vouched for
        // by that decision, whatever the signature said.
        let mut taken: BTreeSet<String> = BTreeSet::new();
        for path in &plain.conflicts {
            let Some(d) = decisions.get(path) else {
                continue;
            };
            if *d == Decision::TakeRemote {
                taken.insert(path.clone());
            }
            let here = plain.here.contains_key(path);
            let there = plain.there.contains_key(path);
            let step = match d {
                Decision::KeepLocal | Decision::KeepBoth if here => Step::Push,
                Decision::KeepLocal | Decision::KeepBoth => Step::DeleteRemote,
                Decision::TakeRemote if there => Step::Pull,
                Decision::TakeRemote => Step::DeleteLocal,
                Decision::TakeMergedOurs | Decision::TakeMergedTheirs => Step::Merge,
            };
            landing.decided.insert(path.clone());
            moves.push((path.clone(), step));
        }
        let failed = |_| Refusal::of(Why::Failed);
        for (path, step) in moves {
            let Some(there) = crate::plain::repo_path(&path).map(|p| self.places.repo().join(p))
            else {
                continue;
            };
            match step {
                Step::Push if push => {
                    // THE FILE AS IT IS NOW — and what it carries is its
                    // base (`sent`): changed again after this read, it is a
                    // change of this machine's the next run sends. Gone, or
                    // no longer a plain file, nothing is sent and its base
                    // stays.
                    let Some(bytes) = self.places.resolve(&path).and_then(|p| plain_bytes(&p))
                    else {
                        continue;
                    };
                    write_with_mode(&there, &bytes, PLAIN_MODE).map_err(failed)?;
                    landing.sent.insert(path.clone(), Some(Sent::of(&bytes)));
                    landing.pushed.push(path);
                }
                Step::DeleteRemote if push => {
                    match std::fs::remove_file(&there) {
                        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                            return Err(Refusal::of(Why::Failed));
                        }
                        _ => {}
                    }
                    landing.sent.insert(path.clone(), None);
                    landing.pushed.push(path);
                }
                Step::Pull => landing.lands.push((path, Land::Theirs)),
                Step::DeleteLocal => landing.lands.push((path, Land::Delete)),
                Step::Merge => {
                    if push {
                        let joined = merges
                            .joined
                            .get(&path)
                            .ok_or(Refusal::of(Why::Undecided))?;
                        write_with_mode(&there, &joined.bytes, PLAIN_MODE).map_err(failed)?;
                        landing
                            .sent
                            .insert(path.clone(), Some(Sent::of(&joined.bytes)));
                        landing.pushed.push(path.clone());
                    }
                    landing.lands.push((path, Land::Joined));
                }
                _ => {}
            }
        }
        if push {
            self.sign_or_wait(plain, &mut landing, ring, &taken)?;
        }
        Ok(landing)
    }

    /// Signs what this run leaves in the work tree's `plain/` — or, when
    /// nothing can, takes this machine's changes to signed files back out of
    /// it (they wait, `Landing::unsigned`). What is signed is what this run
    /// vouches for (`signing::vouched_going`): a change of the repository's
    /// it held stays out, and so stays held everywhere.
    fn sign_or_wait(
        &self,
        plain: &PlainPlan,
        landing: &mut Landing,
        ring: Option<&Keyring>,
        taken: &BTreeSet<String>,
    ) -> Result<(), Refusal> {
        let sent: BTreeMap<String, String> = landing
            .sent
            .iter()
            .filter_map(|(p, s)| s.as_ref().map(|s| (p.clone(), s.sha256.clone())))
            .collect();
        let now = walk_there(&self.places.repo());
        let mut going =
            signing::vouched_going(&now.files, &plain.here, &plain.vouched, &sent, taken);
        // A CHANGE OF THE REPOSITORY'S THIS RUN HELD, and nobody decided:
        // what is signed for that path is this machine's own file — the
        // person's — so every other machine keeps holding the repository's,
        // and this one's root takes its own.
        for path in plain.held.keys().filter(|p| !landing.decided.contains(*p)) {
            if let (Some(file), Some(rel)) = (plain.here.get(path), crate::plain::signed_path(path))
            {
                going.insert(rel, file.sha256.clone());
            }
        }
        match self.signing(&going, &plain.vouched, ring) {
            Signing::Unneeded => {}
            Signing::With(key) => {
                let (pair, counter) = self.sign_going(key, &going, &plain.vouched, &plain.seen)?;
                self.write_signed_there(&pair, key.public())?;
                landing.signed = Some((pair, counter));
            }
            Signing::Kept(pair) => {
                let key = self.trusted(None).ok_or(Refusal::of(Why::Failed))?;
                let counter = ronoctua_pq::sign::open_manifest(&pair.json, &pair.signature, key)
                    .map_err(|_| Refusal::of(Why::Failed))?
                    .counter;
                self.write_signed_there(&pair, key)?;
                landing.signed = Some((pair, counter));
            }
            Signing::Locked => {
                let waiting: Vec<String> = landing
                    .pushed
                    .iter()
                    .filter(|p| signed(p))
                    .cloned()
                    .collect();
                self.take_back_there(plain, &waiting)?;
                landing.pushed.retain(|p| !waiting.contains(p));
                landing.sent.retain(|p, _| !waiting.contains(p));
                landing.unsigned = waiting;
            }
        }
        Ok(())
    }

    /// Puts the work tree's `paths` back as the repository's head has them
    /// (what `plain` read there), or removes them when it has none: this
    /// run's writes, taken back.
    fn take_back_there(&self, plain: &PlainPlan, paths: &[String]) -> Result<(), Refusal> {
        if paths.is_empty() {
            return Ok(());
        }
        let repo = Repo::discover(&self.places.repo())
            .ok_or(Refusal::of(Why::Failed))?
            .with_tools(self.tools.clone());
        for path in paths {
            let Some(rel) = crate::plain::repo_path(path) else {
                continue;
            };
            if plain.there.contains_key(path) {
                self.git(&repo, false, "checkout", &["HEAD", "--", rel.as_str()])?;
            } else {
                match std::fs::remove_file(self.places.repo().join(&rel)) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        return Err(Refusal::of(Why::Failed));
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Keeps, once a run's push is in and what the repository had landed,
    /// the manifest that now covers this machine's plain part best: the one
    /// this run signed, else the repository's when it held and is not older
    /// than the one kept here — so a staging carries a signature root holds.
    pub(super) fn settle_signed(&self, plain: &PlainPlan, landing: &Landing) {
        if let Some((pair, _)) = &landing.signed {
            signing::keep_signed(self.places, pair);
            return;
        }
        let Some(pair) = plain.vouched.pair.as_ref() else {
            return;
        };
        let repo = plain.vouched.counter().unwrap_or(0);
        let kept = self
            .kept_signed(self.trusted_of(plain))
            .map(|(_, m)| m.counter);
        if kept.is_none_or(|k| repo >= k) {
            signing::keep_signed(self.places, pair);
        }
    }

    /// The key `plain` was vouched with: the passphrase's, else the pin.
    fn trusted_of<'p>(&'p self, plain: &'p PlainPlan) -> Option<&'p ronoctua_pq::sign::PublicKey> {
        plain.signer.as_ref().or(self.pinned.as_ref())
    }

    /// Lands what the repository had, here — after the push — on every
    /// destination that still holds what `plain` read (else it is
    /// [`Landed::moved`] and left alone). With a `stamp` (a sync with the
    /// person) what is overwritten or deleted is set aside first.
    pub(super) fn land_plain(
        &self,
        plain: &PlainPlan,
        landing: &Landing,
        merges: &Merges,
        stamp: Option<&Path>,
    ) -> Landed {
        let mut out = Landed::default();
        for (path, land) in &landing.lands {
            let Some(dest) = self.places.resolve(path) else {
                continue;
            };
            // STILL WHAT THE PLAN READ: a file written here since (the
            // software centre, the machine's record) is not written over.
            let read = plain.here.get(path).map(|f| f.sha256.as_str());
            if !holds(&dest, read) {
                out.moved.insert(path.clone());
                continue;
            }
            let aside = |engine: &Self| match stamp {
                Some(stamp) => engine.set_aside(&dest, stamp, path),
                None => Ok(()),
            };
            let done = match land {
                Land::Theirs => {
                    let there = crate::plain::repo_path(path).map(|p| self.places.repo().join(p));
                    let bytes = there.and_then(|t| std::fs::read(t).ok());
                    let mode = plain.here.get(path).map_or(PLAIN_MODE, |f| f.mode);
                    match bytes {
                        Some(bytes) => aside(self).and_then(|()| {
                            write_with_mode(&dest, &bytes, mode)
                                .map_err(|_| Refusal::of(Why::Failed))
                        }),
                        None => Err(Refusal::of(Why::Failed)),
                    }
                }
                Land::Delete => aside(self).and_then(|()| match std::fs::remove_file(&dest) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                        Err(Refusal::of(Why::Failed))
                    }
                    _ => Ok(()),
                }),
                Land::Joined => self.land_joined(path, merges, stamp),
            };
            match done {
                Ok(()) => out.changed.push(path.clone()),
                Err(_) => {
                    out.unwritten.insert(path.clone());
                }
            }
        }
        out
    }

    /// The plain base after a run, from its plan and what it did — never
    /// from a second walk: what both sides held alike, what a push carried
    /// ([`Landing::sent`]), what landed here (the repository's side; for a
    /// merge only pulled, the repository's side too, the file here holding
    /// it now) — each with git's blob of it at `HEAD` — and the OLD base of
    /// every other path (a conflict left, a change of this machine's not
    /// sent, what could not be written, what moved since the plan read it,
    /// what is too large or unreadable), so it is still a change next time.
    pub(super) fn plain_base_after(
        &self,
        plain: &PlainPlan,
        landing: &Landing,
        landed: &Landed,
    ) -> Result<PlainBase, Refusal> {
        let repo = Repo::discover(&self.places.repo())
            .ok_or(Refusal::of(Why::Failed))?
            .with_tools(self.tools.clone());
        let listed = self.git(
            &repo,
            false,
            "ls-tree",
            &["-r", "-z", "HEAD", "--", crate::plain::DIR],
        )?;
        let blobs = blobs_of(&listed);
        let old = plain.base.as_ref();
        let left: BTreeSet<&str> = plain
            .conflicts
            .iter()
            .filter(|c| !landing.decided.contains(*c))
            .map(String::as_str)
            .collect();
        let lands: BTreeMap<&str, &Land> =
            landing.lands.iter().map(|(p, l)| (p.as_str(), l)).collect();
        let changed: BTreeSet<&str> = landed.changed.iter().map(String::as_str).collect();
        let theirs = |path: &str| {
            plain
                .there
                .get(path)
                .map_or(After::Kept, |f| After::Holds(&f.sha256, f.size_bytes))
        };
        let mut paths: BTreeSet<&str> = plain.here.keys().map(String::as_str).collect();
        paths.extend(plain.there.keys().map(String::as_str));
        paths.extend(plain.too_large.iter().map(String::as_str));
        if let Some(old) = old {
            paths.extend(old.files.keys().map(String::as_str));
        }
        let mut files = BTreeMap::new();
        for path in paths {
            let after = if plain.too_large.contains(path)
                || plain.unreadable.iter().any(|u| covers(u, path))
                || left.contains(path)
            {
                After::Kept
            } else if let Some(land) = lands.get(path) {
                match (changed.contains(path), land) {
                    // NOT LANDED (unwritten, or moved since the plan read it):
                    // even a merge this run pushed keeps the old base, so
                    // the next run joins this machine's newer file with it.
                    (false, _) => After::Kept,
                    (true, Land::Theirs) => theirs(path),
                    (true, Land::Delete) => After::Gone,
                    (true, Land::Joined) => match landing.sent.get(path) {
                        Some(Some(s)) => After::Holds(&s.sha256, s.size_bytes),
                        // A MERGE ONLY PULLED: the file here holds the
                        // repository's side, which is its base now; the
                        // next push sends only this machine's difference.
                        _ => theirs(path),
                    },
                }
            } else if let Some(sent) = landing.sent.get(path) {
                sent.as_ref()
                    .map_or(After::Gone, |s| After::Holds(&s.sha256, s.size_bytes))
            } else {
                match (plain.here.get(path), plain.there.get(path)) {
                    (Some(l), Some(r)) if l.sha256 == r.sha256 => theirs(path),
                    (None, None) => After::Gone,
                    _ => After::Kept,
                }
            };
            let was = || old.and_then(|o| o.files.get(path)).cloned();
            let now = match after {
                After::Holds(sha256, size_bytes) => crate::plain::repo_path(path)
                    .and_then(|r| blobs.get(&r).cloned())
                    .map(|blob| PlainKnown {
                        sha256: sha256.to_owned(),
                        blob,
                        size_bytes,
                    })
                    .or_else(was),
                After::Gone => None,
                After::Kept => was(),
            };
            if let Some(k) = now {
                files.insert(path.to_owned(), k);
            }
        }
        // WHAT THE RUN MADE OF THE SIGNATURE: what it held (and is still
        // left), what waits for one, the counter now seen.
        let held = plain
            .held
            .iter()
            .filter(|(p, _)| left.contains(p.as_str()))
            .map(|(p, w)| (p.clone(), *w))
            .collect();
        let mut seen = plain.seen.clone();
        if let Ok(m) = &plain.vouched.manifest {
            seen = seen.after(&m.signer, m.counter);
        }
        if let Some((pair, counter)) = &landing.signed
            && let Ok(m) = serde_json::from_slice::<ronoctua_pq::sign::Manifest>(&pair.json)
        {
            seen = seen.after(&m.signer, *counter);
        }
        // WAITING FOR A SIGNATURE: what this push took back out, and what
        // waited before and is still this machine's change against the base
        // now (a plain pull tries nothing, and the change waits on).
        let pending = |p: &str| {
            plain.here.get(p).map(|f| f.sha256.as_str())
                != files.get(p).map(|k: &PlainKnown| k.sha256.as_str())
        };
        let unsigned: Vec<String> = landing
            .unsigned
            .iter()
            .cloned()
            .chain(plain.base.iter().flat_map(|b| b.unsigned.iter().cloned()))
            .filter(|p| pending(p))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Ok(signing::with_signature(
            PlainBase {
                repository: self.prefs.repository.clone(),
                files,
                conflicts: left.iter().map(|c| (*c).to_owned()).collect(),
                too_large: plain.too_large.iter().cloned().collect(),
                ..PlainBase::default()
            },
            held,
            unsigned,
            seen,
            plain.signer.as_ref(),
            plain.base.as_ref(),
        ))
    }

    /// The plain part ALONE: fetch, merge, and — when `push` — commit and
    /// push; no identity, no passphrase, the sealed part untouched. What
    /// clashes is left and named. Under the run lock; `status.json` is left
    /// to the runs a door started.
    ///
    /// # Errors
    ///
    /// [`Why::Busy`], [`Why::Empty`] for a repository the first sync has not
    /// made yet (its identity comes first), [`Why::NotOurs`] and
    /// [`Why::Newer`], [`Why::Raced`], and the network's.
    pub fn plain_part(&self, push: bool, beat: &mut dyn FnMut(Beat)) -> Result<Report, Refusal> {
        let _held = running::hold(self.places)?;
        self.plain_rounds(push, beat)
    }

    /// `sync-plain`: the plain part alone, pushed, under the run lock — and
    /// how it ended said in `status.json` only where no run a door started
    /// has said anything: none yet, or another plain run. A sealed sync's
    /// ending — a failure, a plan that waits for the person, what it synced
    /// and what it could not write — is what the door shows and the
    /// reminder's mark reads, and a plain run never covers it; nor is a
    /// plain run's refusal a sync that failed (the queue's row and the
    /// journal say it). Nothing a door typed is touched.
    ///
    /// # Errors
    ///
    /// As [`Engine::plain_part`].
    pub fn sync_plain(&self, beat: &mut dyn FnMut(Beat)) -> Result<Report, Refusal> {
        let _held = running::hold(self.places)?;
        let done = self.plain_rounds(true, beat);
        if let Ok(report) = &done {
            let file = self.places.status();
            let unsaid = !file.exists()
                || Status::read(&file)
                    .is_some_and(|s| matches!(s.outcome, Outcome::PlainSynced { .. }));
            if unsaid {
                super::write_status(
                    self.places,
                    self.now_s,
                    Outcome::PlainSynced {
                        pulled: count(report.changed.len()),
                        pushed: count(report.pushed.len()),
                    },
                );
            }
        }
        done
    }

    /// The rounds of a plain run, the lock already held: again while another
    /// machine pushes first.
    fn plain_rounds(&self, push: bool, beat: &mut dyn FnMut(Beat)) -> Result<Report, Refusal> {
        for _ in 0..TRIES {
            beat(step(Phase::Fetching));
            let ground = self.ground()?;
            let Some(head) = ground.head.clone() else {
                return Err(Refusal::of(Why::Empty));
            };
            self.checkout(&ground.repo, &head)?;
            self.check_format()?;
            // THE IDENTITY THE SESSION KEPT, when there is one: it vouches
            // and signs; a plain run never asks for a passphrase.
            let kept = self.kept_ring();
            beat(step(Phase::Merging));
            let mut merges = Merges::default();
            let plain = self.plan_plain(self.plain_base(), &mut merges, None, kept.as_ref())?;
            let landing =
                self.carry_plain(&plain, &Decisions::new(), &merges, push, kept.as_ref())?;
            if push && (!landing.pushed.is_empty() || landing.signed.is_some()) {
                beat(step(Phase::Pushing));
                if !self.commit_and_push(&ground.repo, &Written::default(), COMMIT_MESSAGE)? {
                    continue;
                }
            }
            beat(step(Phase::Pulling));
            let landed = self.land_plain(&plain, &landing, &merges, None);
            let base = self.plain_base_after(&plain, &landing, &landed)?;
            self.settle_signed(&plain, &landing);
            let written: BTreeSet<String> = landed.changed.iter().cloned().collect();
            self.keep_plain_base(&base, &written);
            return Ok(Report {
                changed: landed.changed,
                pushed: landing.pushed,
                conflicts: base.conflicts,
                too_large: base.too_large,
                held: base.held,
                unsigned: base.unsigned,
                ..Report::default()
            });
        }
        Err(Refusal::of(Why::Raced))
    }

    /// The keyring of the identity the session kept for this repository,
    /// when the person keeps one and it opens this repository's index
    /// (else it is forgotten, as a sync forgets it) — never a passphrase.
    fn kept_ring(&self) -> Option<Keyring> {
        if !self.prefs.keep_unlocked {
            return None;
        }
        let mut identity = self
            .vault?
            .take(&crate::unlock::description(&self.prefs.repository))?;
        let secret = age::secrecy::SecretString::from(std::mem::take(&mut *identity));
        self.open_ring(&super::Opening::Kept(secret)).ok()
    }

    /// Writes `base` as `last.json`'s plain section, the rest as it was —
    /// with when each plain file here last changed, as this run leaves it
    /// (`inventory::stamp_plain`; `written`: what it wrote here).
    fn keep_plain_base(&self, base: &PlainBase, written: &BTreeSet<String>) {
        let mut last = Last::read(&self.places.last()).unwrap_or_default();
        last.plain = base.clone();
        crate::inventory::stamp_plain(self.places, &mut last, self.now_s, written);
        if let Ok(text) = serde_json::to_vec_pretty(&last) {
            let _ = crate::secret::write_private(&self.places.last(), &text);
        }
    }

    // ── anchor: L09 ──
}

/// A plain file's SHA-256, when there is the file.
fn sha(f: Option<&PlainFile>) -> Option<&str> {
    f.map(|f| f.sha256.as_str())
}

/// The side a plain file is, for a conflict's row.
fn side_of_file(f: &PlainFile) -> Side {
    Side {
        size_bytes: f.size_bytes,
        modified_s: 0,
        sha256: f.sha256.clone(),
        kind: Kind::File,
        mode: f.mode,
    }
}

/// The bytes of a plain file to send: a regular file (never what a link
/// points at) within `plainMostKiB`. `None` otherwise.
fn plain_bytes(path: &Path) -> Option<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > kept::PLAIN_MOST_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    (bytes.len() as u64 <= kept::PLAIN_MOST_BYTES).then_some(bytes)
}

/// Whether `dest` holds what a plan read there: the file of that SHA-256,
/// or nothing at all when `read` is `None`. Anything else (another content,
/// a link, a folder, a read that fails) is a destination that moved.
fn holds(dest: &Path, read: Option<&str>) -> bool {
    match std::fs::symlink_metadata(dest) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => read.is_none(),
        Err(_) => false,
        Ok(meta) if !meta.is_file() => false,
        Ok(_) => read.is_some_and(|sha| {
            std::fs::File::open(dest)
                .and_then(|mut f| walk::sha256_of(&mut f))
                .is_ok_and(|now| now == sha)
        }),
    }
}

/// This machine's plain part: every file under the plain roots.
pub(super) fn walk_here(places: &Places) -> PlainSide {
    let mut side = PlainSide::default();
    for root in crate::scope::plain_roots() {
        if let Some(at) = places.resolve(root.path) {
            visit(root.path, &at, &mut side, 0);
        }
    }
    side
}

/// The work tree's plain part: every file under `plain/` of a root this
/// build knows (a newer distro's root is left alone).
pub(super) fn walk_there(repo: &Path) -> PlainSide {
    let mut side = PlainSide::default();
    for root in crate::scope::plain_roots() {
        let at = repo.join(crate::plain::DIR).join(root.at);
        visit(root.path, &at, &mut side, 0);
    }
    side
}

/// One path of a plain walk: a folder walked, a regular file read; anything
/// else (a link, a socket) is not data.
fn visit(logical: &str, at: &Path, side: &mut PlainSide, depth: usize) {
    let Ok(meta) = std::fs::symlink_metadata(at) else {
        return;
    };
    if meta.is_dir() {
        if depth >= DEEPEST {
            return;
        }
        let Ok(entries) = std::fs::read_dir(at) else {
            side.unreadable.insert(logical.to_owned());
            return;
        };
        let mut names: Vec<_> = entries.flatten().map(|e| e.file_name()).collect();
        names.sort();
        for name in names {
            let Some(name) = name.to_str() else {
                continue;
            };
            let child = format!("{logical}/{name}");
            if crate::places::is_logical(&child) {
                visit(&child, &at.join(name), side, depth + 1);
            }
        }
        return;
    }
    if !meta.is_file() || !kept::is_plain(logical) {
        return;
    }
    if meta.len() > kept::PLAIN_MOST_BYTES {
        side.too_large.insert(logical.to_owned());
        return;
    }
    match std::fs::File::open(at).and_then(|mut f| walk::sha256_of(&mut f)) {
        Ok(sha256) => {
            side.files.insert(
                logical.to_owned(),
                PlainFile {
                    sha256,
                    size_bytes: meta.len(),
                    mode: meta.permissions().mode() & 0o7777,
                },
            );
        }
        Err(_) => {
            side.unreadable.insert(logical.to_owned());
        }
    }
}

/// The blobs `ls-tree -r -z` lists, by path. Pure.
fn blobs_of(listed: &[u8]) -> BTreeMap<String, String> {
    listed
        .split(|b| *b == 0)
        .filter_map(|record| {
            let record = std::str::from_utf8(record).ok()?;
            let (head, path) = record.split_once('\t')?;
            let mut fields = head.split(' ');
            let (_mode, kind, id) = (fields.next()?, fields.next()?, fields.next()?);
            (kind == "blob").then(|| (path.to_owned(), id.to_owned()))
        })
        .collect()
}

/// The engine of a plain run: this machine's settings, tools and access.
///
/// # Errors
///
/// [`Why::NoRepository`], [`Why::NoTools`], [`Why::NeedsAccess`] — said by
/// the caller; nothing a door typed is touched (it is a sealed run's), and
/// nothing is written in `status.json` (a plain run's refusal is not a
/// sync's).
fn plain_engine<'a>(places: &'a Places, prefs: &'a Belongings) -> Result<Engine<'a>, Refusal> {
    if !prefs.has_repository() || repository_is_plain(&prefs.repository).is_err() {
        return Err(Refusal::of(Why::NoRepository));
    }
    Engine::for_this_machine(places, prefs)
}

/// `sync-plain`, as the queue and the automatic watch order it
/// ([`Engine::sync_plain`]).
///
/// # Errors
///
/// What [`Engine::sync_plain`] refuses, and what refuses a run before it
/// starts — neither written anywhere.
pub(super) fn sync_plain(places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    let prefs = super::read_prefs(places);
    plain_engine(places, &prefs)?.sync_plain(beat).map(|_| 0)
}

/// `plain pull` / `plain push`: the plain part alone, for the update and the
/// machine package — the report is the answer (`--json`), and `status.json`
/// is left to the runs a door started.
///
/// # Errors
///
/// As [`sync_plain`].
pub fn plain_verb(places: &Places, push: bool) -> Result<Report, Refusal> {
    let prefs = super::read_prefs(places);
    plain_engine(places, &prefs)?.plain_part(push, &mut |_| {})
}

/// A count, for an outcome.
fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// What the installer hands [`plain_out`] beside the address and the access:
/// the passphrase's file (`0600`, consumed), where the signing key the
/// passphrase opened is written for the installer to pin, and the person's
/// choice of the distro's programs for the machine being installed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Opened<'a> {
    /// The passphrase's file; `None`: nothing is verified, and root's
    /// applier holds the copy to whatever key the machine pins.
    pub passphrase: Option<&'a Path>,
    /// Where the signing key's public half goes (`0600`, new).
    pub signer: Option<&'a Path>,
    /// The installer's choice (`ronoctua_person::choice::Choice`): said in
    /// the repository's `declined.json`, signed with the key the passphrase
    /// opens and pushed before the copy — only with the passphrase: a
    /// decline is signed, or not made.
    pub choice: Option<&'a Path>,
}

/// `plain --repository <url> --keys <dir> --out <dir> [--user <name>] [--passphrase <file>
/// --signer <file> [--decline <file>]]`: the plain part of a repository,
/// copied into `out` (`out/<at>/…`, the repository's `plain/` layout, with
/// its manifest and signature) — for the installer, which stages the
/// person's declarations before `nixos-install`, with no home and no
/// settings. The access is the address's kind: a deploy key (`keys/deploy`,
/// with `keys/known_hosts`) for SSH, a token (`keys/token`) for HTTPS. With
/// the passphrase, `key.age` is opened, the signature must hold under ITS
/// signing key, and that key's public half is written to `signer` — the key
/// the installer pins, read from the passphrase and never from the
/// repository's copy. The clone is made beside `out` and removed, however it
/// ends.
///
/// Nothing is pushed but the person's CHOICE for the machine being installed
/// (`choice`, [`Choice`]): read before any network, said in the repository's
/// `declined.json` ([`Choice::apply`]: each program left out declined on
/// that machine alone, each one kept brought back on it — a decline of every
/// machine stays, and the report names it), which must be as the person
/// signed it; signed with the key the passphrase opened (the counter one
/// past the repository's) and pushed, again on top of another machine's
/// push, BEFORE the copy: the copy then carries it signed, and a machine
/// that pins that key takes it. A choice the repository says already pushes
/// nothing ([`crate::plain::ChoiceThere::AlreadyThere`]), so an installation
/// tried again changes nothing — and one that ticks now what an earlier
/// attempt left out brings it back.
///
/// # Errors
///
/// [`Why::NoRepository`] for an address that is not plain, [`Why::NoTools`],
/// [`Why::NeedsAccess`] without the access file, [`Why::Empty`],
/// [`Why::Newer`] (the repository's, or its `declined.json`'s),
/// [`Why::NoPassphrase`] (a choice without one included),
/// [`Why::WrongPassphrase`], [`Why::Corrupt`] for a plain part whose
/// signature does not hold under the key the passphrase opened — or whose
/// `declined.json` is not as it signed —, [`Why::Raced`] when another machine
/// pushed first every time, [`Why::ChoiceUnread`] for a choice that does not
/// read, [`Why::CopyUnwritten`] when `out` or `signer` could not be written,
/// [`Why::Failed`] for a file of the scratch clone, and the network's (on a
/// pass for the host, as every fetch).
pub fn plain_out(
    address: &str,
    keys: &Path,
    out: &Path,
    user: Option<&str>,
    opened: Opened<'_>,
) -> Result<Report, Refusal> {
    let address = address.trim();
    if repository_is_plain(address).is_err() {
        if let Some(p) = opened.passphrase {
            let _ = std::fs::remove_file(p);
        }
        return Err(Refusal::of(Why::NoRepository));
    }
    let tools = Tools::from_env().ok_or(Refusal::of(Why::NoTools))?;
    copy_out(address, keys, out, user, tools, opened)
}

/// [`plain_out`] for an address already checked, with these tools. The
/// passphrase's file goes however it ends.
pub(super) fn copy_out(
    address: &str,
    keys: &Path,
    out: &Path,
    user: Option<&str>,
    tools: Tools,
    opened: Opened<'_>,
) -> Result<Report, Refusal> {
    let passphrase = opened.passphrase.map(|p| {
        let read = super::read_secret(p);
        let _ = std::fs::remove_file(p);
        read
    });
    let passphrase = passphrase.transpose()?;
    // THE CHOICE, read and checked before any network: a decline is
    // signed with the key the passphrase opens, or not made.
    let choice = opened.choice.map(read_choice).transpose()?;
    if choice.is_some() && passphrase.is_none() {
        return Err(Refusal::of(Why::NoPassphrase));
    }
    let mut prefs = Belongings::with_repository(address);
    prefs.user = user.map(str::to_owned);
    let credentials = access_of(address, keys, user, &mut prefs)?;
    let scratch = scratch_beside(out);
    let _ = std::fs::remove_dir_all(&scratch);
    let places = Places {
        home: scratch.clone(),
        config: scratch.join("config"),
        state: scratch.join("state"),
        runtime: scratch.join("run"),
    };
    let engine = Engine {
        credentials: Some(credentials),
        ..Engine::for_plain(&places, &prefs, tools)
    };
    let copied = engine.copy_plain(out, passphrase.as_ref(), opened.signer, choice.as_ref());
    let _ = std::fs::remove_dir_all(&scratch);
    copied
}

/// The choice the installer wrote: a regular file within the plain part's
/// cap, and a [`Choice`] this distro reads.
fn read_choice(path: &Path) -> Result<Choice, Refusal> {
    let bytes = plain_bytes(path).ok_or(Refusal::of(Why::ChoiceUnread))?;
    ronoctua_person::parse(&bytes).map_err(|_| Refusal::of(Why::ChoiceUnread))
}

/// What saying the installer's choice in the repository came to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ChosenThere {
    /// Said, signed and pushed.
    Pushed,
    /// The repository said it already: nothing pushed.
    Unchanged,
    /// Another machine pushed first — or the push went in and was reported
    /// failed: the round goes again on top of what is there now.
    Lost,
}

impl Engine<'_> {
    /// Fetches into this engine's clone and copies the repository's plain
    /// part into `out` (`out/<at>/…`, its manifest and signature beside) —
    /// what [`plain_out`] does in a clone of its own. With `passphrase`, the
    /// signature must hold under the key it opens, whose public half goes to
    /// `signer`; with `choice` too, the choice is said, signed and pushed
    /// first ([`Self::choose_there`]), again while another machine pushes
    /// first.
    ///
    /// # Errors
    ///
    /// As [`plain_out`].
    pub(super) fn copy_plain(
        &self,
        out: &Path,
        passphrase: Option<&age::secrecy::SecretString>,
        signer: Option<&Path>,
        choice: Option<&Choice>,
    ) -> Result<Report, Refusal> {
        if choice.is_some() && passphrase.is_none() {
            return Err(Refusal::of(Why::NoPassphrase));
        }
        for _ in 0..TRIES {
            let ground = self.ground()?;
            let Some(head) = ground.head.clone() else {
                return Err(Refusal::of(Why::Empty));
            };
            self.checkout(&ground.repo, &head)?;
            if self.format()? > crate::FORMAT_VERSION {
                return Err(Refusal::of(Why::Newer));
            }
            // THE KEY THE PASSPHRASE OPENS, and the signature held to it: the
            // installer pins that key, never the repository's copy of it. A
            // plain part that key did not sign at all is refused; a file of
            // one it signed that is not as signed is left out, and named.
            let mut vouched = None;
            let mut pushed = Vec::new();
            let mut chosen = None;
            let mut kept_everywhere = Vec::new();
            if let Some(passphrase) = passphrase {
                let ring = Keyring::open(&self.read_repo(super::KEY_FILE)?, passphrase)?;
                let key = ring.signing().public();
                let mut v = self.vouched(Some(key), &Seen::default());
                if v.manifest.is_err() {
                    return Err(Refusal::of(Why::Corrupt));
                }
                if let Some(signer) = signer {
                    let _ = std::fs::remove_file(signer);
                    crate::secret::write_private(signer, format!("{key}\n").as_bytes())
                        .map_err(|_| Refusal::of(Why::CopyUnwritten))?;
                }
                if let Some(choice) = choice {
                    let (there, everywhere) =
                        self.choose_there(&ground.repo, ring.signing(), &v, choice)?;
                    kept_everywhere = everywhere;
                    match there {
                        ChosenThere::Lost => continue,
                        ChosenThere::Unchanged => {
                            // ALREADY SAID THERE — by an attempt before this
                            // one too, whose push went in and was reported
                            // failed: the choice is up all the same.
                            chosen = Some(crate::plain::ChoiceThere::AlreadyThere);
                        }
                        ChosenThere::Pushed => {
                            // WHAT WENT UP is the signature the copy carries.
                            v = self.vouched(Some(key), &Seen::default());
                            if v.manifest.is_err() {
                                return Err(Refusal::of(Why::Corrupt));
                            }
                            pushed.extend(crate::plain::declined_logical());
                            chosen = Some(crate::plain::ChoiceThere::Pushed);
                        }
                    }
                }
                vouched = Some(v);
            }
            let mut report = self.copy_there(out, vouched.as_ref())?;
            report.pushed = pushed;
            report.choice = chosen;
            report.kept_everywhere = kept_everywhere;
            return Ok(report);
        }
        Err(Refusal::of(Why::Raced))
    }

    /// Copies the work tree's plain part into `out`, its manifest and
    /// signature beside — leaving out, and naming, a signed file `vouched`
    /// does not hold as signed.
    fn copy_there(&self, out: &Path, vouched: Option<&Vouched>) -> Result<Report, Refusal> {
        // THE MANIFEST AND ITS SIGNATURE travel with the copy: root's
        // applier holds it to the key the machine pinned.
        for name in [crate::plain::MANIFEST_FILE, crate::plain::SIGNATURE_FILE] {
            let from = self.places.repo().join(crate::plain::DIR).join(name);
            if let Ok(bytes) = std::fs::read(&from) {
                write_with_mode(&out.join(name), &bytes, PLAIN_MODE)
                    .map_err(|_| Refusal::of(Why::CopyUnwritten))?;
            }
        }
        let there = walk_there(&self.places.repo());
        let mut report = Report {
            too_large: there.too_large.iter().cloned().collect(),
            ..Report::default()
        };
        for (path, file) in &there.files {
            let Some(rel) = crate::plain::repo_path(path) else {
                continue;
            };
            if let Some(v) = vouched
                && signed(path)
                && let Err(why) = v.holds(path, Some(file))
            {
                report.held.insert(path.clone(), why);
                continue;
            }
            let inside = rel
                .strip_prefix(crate::plain::DIR)
                .map_or(rel.as_str(), |r| r.trim_start_matches('/'));
            let bytes = std::fs::read(self.places.repo().join(&rel))
                .map_err(|_| Refusal::of(Why::Corrupt))?;
            write_with_mode(&out.join(inside), &bytes, PLAIN_MODE)
                .map_err(|_| Refusal::of(Why::CopyUnwritten))?;
            report.changed.push(path.clone());
        }
        Ok(report)
    }

    /// Says `choice` in the work tree's `declined.json` ([`Choice::apply`])
    /// and pushes it, signed with `key` — with the kept ids the person
    /// declined on every machine, which stay. The file there must be what
    /// the person signed
    /// (`vouched`, under that key's signature) — or absent and unsigned:
    /// signing over content the person did not sign would launder it for
    /// every machine. What goes up is signed one past the repository's
    /// counter, over exactly what the person's manifest named and the
    /// joined file: a change of the repository's it does not hold — a file
    /// changed, added or DELETED without the person — keeps the person's
    /// line (or none), and so stays held everywhere. Nothing changed (the
    /// choice said there already): nothing pushed.
    fn choose_there(
        &self,
        repo: &Repo,
        key: &ronoctua_pq::sign::SigningKey,
        vouched: &Vouched,
        choice: &Choice,
    ) -> Result<(ChosenThere, Vec<String>), Refusal> {
        let logical = crate::plain::declined_logical().ok_or(Refusal::of(Why::Failed))?;
        let rel = crate::plain::repo_path(&logical).ok_or(Refusal::of(Why::Failed))?;
        let path = self.places.repo().join(&rel);
        let bytes = match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => return Err(Refusal::of(Why::Corrupt)),
            Ok(_) => Some(plain_bytes(&path).ok_or(Refusal::of(Why::Corrupt))?),
        };
        let file = bytes.as_ref().map(|b| PlainFile {
            sha256: super::merging::sha256(b),
            size_bytes: b.len() as u64,
            mode: PLAIN_MODE,
        });
        if vouched.holds(&logical, file.as_ref()).is_err() {
            return Err(Refusal::of(Why::Corrupt));
        }
        let mut going = vouched
            .manifest
            .as_ref()
            .map_err(|_| Refusal::of(Why::Corrupt))?
            .files
            .clone();
        let mut joined = match &bytes {
            None => Declined::default(),
            Some(b) => ronoctua_person::parse::<Declined>(b).map_err(|r| {
                Refusal::of(if r.why == ronoctua_person::Why::Newer {
                    Why::Newer
                } else {
                    Why::Corrupt
                })
            })?,
        };
        let applied = choice.apply(&mut joined);
        if !applied.changed {
            return Ok((ChosenThere::Unchanged, applied.everywhere));
        }
        let text = ronoctua_person::to_json(&joined);
        write_with_mode(&path, text.as_bytes(), PLAIN_MODE)
            .map_err(|_| Refusal::of(Why::Failed))?;
        let signed_as = crate::plain::signed_path(&logical).ok_or(Refusal::of(Why::Failed))?;
        going.insert(signed_as, super::merging::sha256(text.as_bytes()));
        let (pair, _) = self.sign_going(key, &going, vouched, &Seen::default())?;
        self.write_signed_there(&pair, key.public())?;
        if !self.commit_and_push(repo, &Written::default(), COMMIT_MESSAGE)? {
            return Ok((ChosenThere::Lost, applied.everywhere));
        }
        Ok((ChosenThere::Pushed, applied.everywhere))
    }
}

/// The access an address takes from `keys`: a token for HTTPS, else the
/// deploy key — and `prefs.auth` set to match.
fn access_of(
    address: &str,
    keys: &Path,
    user: Option<&str>,
    prefs: &mut Belongings,
) -> Result<Credentials, Refusal> {
    if address.starts_with("https://") {
        prefs.auth = Auth::Token;
        let file = keys.join(crate::places::TOKEN_FILE);
        if !file.is_file() {
            return Err(Refusal::of(Why::NeedsAccess));
        }
        return Ok(Credentials::Token {
            file,
            user: user.filter(|u| !u.is_empty()).map(str::to_owned),
            pq_only: true,
        });
    }
    prefs.auth = Auth::Ssh;
    let key = keys.join(crate::places::DEPLOY_KEY);
    if !key.is_file() {
        return Err(Refusal::of(Why::NeedsAccess));
    }
    Ok(Credentials::Ssh {
        key,
        known_hosts: keys.join(crate::places::KNOWN_HOSTS),
        pq_only: true,
    })
}

/// A scratch folder beside `path`, named after it and this process.
fn scratch_beside(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .map_or_else(|| "plain".to_owned(), |n| n.to_string_lossy().into_owned());
    path.with_file_name(format!(".{name}.belongings.{}", std::process::id()))
}

impl<'a> Engine<'a> {
    /// An engine for `places` and `prefs` with these tools and no access yet
    /// — the installer's verbs, which name their access on the command line.
    pub(super) fn for_plain(places: &'a Places, prefs: &'a Belongings, tools: Tools) -> Self {
        Self {
            places,
            prefs,
            tools,
            credentials: None,
            machine: super::machine_name(),
            work: None,
            now_s: super::now_s(),
            before_push: None,
            consent: &ronoctua_consent::pass,
            built_s: super::built_s(),
            push_most_bytes: kept::PUSH_MOST_BYTES,
            // THE INSTALLER'S VERBS run on a system that is not the
            // person's: nothing is kept for a session there, and no key of
            // the person's is pinned.
            vault: None,
            pinned: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the installer's copy of the plain part reaches the
    /// repository with another access than the address's kind, looks for
    /// the deploy key's known hosts anywhere but beside it, or goes on
    /// without the access file.
    #[test]
    fn the_installer_copy_takes_the_access_of_the_address_kind() {
        let keys =
            std::env::temp_dir().join(format!("ronoctua-plain-access-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&keys);
        std::fs::create_dir_all(&keys).unwrap();
        let mut prefs = Belongings::default();
        assert_eq!(
            access_of("git@example.org:a/b.git", &keys, None, &mut prefs),
            Err(Refusal::of(Why::NeedsAccess))
        );
        assert_eq!(
            access_of("https://example.org/a/b.git", &keys, None, &mut prefs),
            Err(Refusal::of(Why::NeedsAccess))
        );
        std::fs::write(keys.join(crate::places::DEPLOY_KEY), "key").unwrap();
        std::fs::write(keys.join(crate::places::TOKEN_FILE), "token").unwrap();
        let ssh = access_of("git@example.org:a/b.git", &keys, Some("me"), &mut prefs).unwrap();
        assert_eq!(prefs.auth, Auth::Ssh);
        assert_eq!(
            ssh,
            Credentials::Ssh {
                key: keys.join("deploy"),
                known_hosts: keys.join("known_hosts"),
                pq_only: true,
            }
        );
        let token =
            access_of("https://example.org/a/b.git", &keys, Some("me"), &mut prefs).unwrap();
        assert_eq!(prefs.auth, Auth::Token);
        assert_eq!(
            token,
            Credentials::Token {
                file: keys.join("token"),
                user: Some("me".to_owned()),
                pq_only: true,
            }
        );
        assert_eq!(
            scratch_beside(Path::new("/mnt/etc/nixos/person")),
            PathBuf::from(format!(
                "/mnt/etc/nixos/.person.belongings.{}",
                std::process::id()
            ))
        );
        let _ = std::fs::remove_dir_all(keys);
    }

    /// FAILS IF: `ls-tree -z` is read wrong — a path with a space, or a
    /// submodule taken for a blob.
    #[test]
    fn the_blobs_of_a_listing_are_read_by_path() {
        let listed =
            b"100644 blob 1111111111111111111111111111111111111111\tplain/person/a b.json\0\
160000 commit 2222222222222222222222222222222222222222\tplain/person/sub\0";
        let blobs = blobs_of(listed);
        assert_eq!(blobs.len(), 1);
        assert_eq!(
            blobs.get("plain/person/a b.json").map(String::as_str),
            Some("1111111111111111111111111111111111111111")
        );
    }

    /// FAILS IF: a destination that moved since the plan read it — another
    /// content, a link where a file was, a file where there was none — is
    /// taken for the one the plan read.
    #[test]
    fn a_destination_holds_only_what_the_plan_read() {
        let dir = std::env::temp_dir().join(format!("ronoctua-plain-holds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("options.json");
        assert!(holds(&file, None), "nothing there, nothing read");
        std::fs::write(&file, b"one").unwrap();
        let one = super::super::merging::sha256(b"one");
        assert!(holds(&file, Some(&one)));
        assert!(!holds(&file, None), "a file born since the plan read none");
        std::fs::write(&file, b"two").unwrap();
        assert!(!holds(&file, Some(&one)), "another content");
        std::fs::remove_file(&file).unwrap();
        assert!(!holds(&file, Some(&one)), "deleted since");
        std::os::unix::fs::symlink(dir.join("elsewhere"), &file).unwrap();
        assert!(!holds(&file, None), "a link where nothing was");
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ── anchor: L09 ──
