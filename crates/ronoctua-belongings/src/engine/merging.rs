//! The merges: what both sides changed, joined where a merge can join it —
//! JSON field by field ([`crate::merge_json`]), text hunk by hunk
//! (`git merge-file`, [`ronoctua_git::merge_file`]) — and the session of a
//! sync that waits (the repository's side of every conflicting text and its
//! diff).
//!
//! # A merge needs a base
//!
//! Three sides, never two: with no base a change on both sides cannot be told
//! from a change on one, and a merge would guess. The sealed part's base is
//! what the last sync left, kept SEALED to the identity in
//! `state/belongings/base/<sha256>.age` ([`Engine::keep_bases`]) — only text
//! of at most [`TEXT_MOST_BYTES`] (JSON is text), never a path whose target
//! says `choose`, pruned to what `last.json` names after every sync. The
//! plain part's base is git's own blob of it (`engine/plain.rs`). No base
//! here — a first sync, a file never kept as text, a cache lost — is a
//! conflict, never a guess.
//!
//! # What a merge does to a conflict
//!
//! A clean merge takes the path out of the conflicts: the joined file is
//! written here and pushed ([`Step::Merge`]). A merge that clashes leaves the
//! conflict and says where ([`Conflict::merge`]): a JSON document's fields,
//! each decided on its own or all one way, or a text's hunks, with the two
//! merged decisions ("merge, keeping this machine's where they clash" and
//! the repository's). Each attempt merges again from the three sides it
//! read: nothing merged survives a process, so `apply` re-merges what
//! `prepare` merged, against the repository as it is then.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ronoctua_git::Favor;

use super::{Engine, TEXT_MOST_BYTES};
use crate::chunks::{Store, Written};
use crate::crypt::Keyring;
use crate::kept::{self, MergeRule};
use crate::merge::{Blob, Index};
use crate::merge_json::{self, Pick};
use crate::plan::{Conflict, Decision, Decisions, DecisionsFile, Entry, Kind, Merging, Plan, Step};
use crate::secret::{draft_of, make_private_dir, write_private};
use crate::status::Last;
use crate::walk;
use crate::{Refusal, Why};

impl Engine<'_> {
    /// The session of a sync that waits: the plan, and for every conflicting
    /// text the repository's side and its diff against this machine's — a
    /// sealed path's opened from its chunks, a plain one's read from the work
    /// tree.
    pub(super) fn write_session(
        &self,
        ring: &Keyring,
        index: &Index,
        plan: &mut Plan,
    ) -> Result<(), Refusal> {
        let failed = |_| Refusal::of(Why::Failed);
        let theirs_dir = self.places.session().join(crate::places::THEIRS_DIR);
        make_private_dir(&theirs_dir).map_err(failed)?;
        let blobs = index.by_path();
        for (n, c) in plan.conflicts.iter_mut().enumerate() {
            let (Some(local), Some(remote)) = (&c.local, &c.remote) else {
                continue;
            };
            if local.kind != Kind::File
                || remote.kind != Kind::File
                || local.size_bytes > TEXT_MOST_BYTES
                || remote.size_bytes > TEXT_MOST_BYTES
            {
                continue;
            }
            // THE REPOSITORY'S SIDE: opened from its chunks (`sealed`), or —
            // a plain path — as the work tree has it.
            let plain = if kept::plain_root(&c.path).is_some() {
                let Some(there) = crate::plain::repo_path(&c.path) else {
                    continue;
                };
                match std::fs::read(self.places.repo().join(there)) {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                }
            } else {
                let Some(blob) = blobs.get(c.path.as_str()) else {
                    continue;
                };
                self.read_blob(ring, blob)?
            };
            let Some(here) = self
                .places
                .resolve(&c.path)
                .and_then(|p| walk::head(&p, TEXT_MOST_BYTES))
            else {
                continue;
            };
            if !walk::is_text(&plain) || !walk::is_text(&here) {
                continue;
            }
            let name = n.to_string();
            let theirs = self.places.theirs(&name);
            write_private(&theirs, &plain).map_err(failed)?;
            if let Some(diff) = self.diff(&here, &theirs) {
                write_private(&self.places.theirs(&format!("{name}.diff")), &diff)
                    .map_err(failed)?;
            }
            c.text = true;
            c.theirs = Some(name);
        }
        let text = serde_json::to_vec_pretty(plan).map_err(|_| Refusal::of(Why::Failed))?;
        write_private(&self.places.plan(), &text).map_err(failed)
    }

    /// The unified diff from this machine's text to the repository's — git's
    /// own (`diff --no-index`), through the hardened command. `None` when git
    /// could not make one.
    pub(super) fn diff(&self, here: &[u8], theirs: &Path) -> Option<Vec<u8>> {
        // THIS MACHINE'S SIDE AS IT WAS READ, beside the other in the session:
        // the diff is of the two the person decides between.
        let mine = theirs.with_extension("here");
        write_private(&mine, here).ok()?;
        let mut command = ronoctua_git::base(&self.tools, &self.places.repo(), None).ok()?;
        ronoctua_git::subcommand(&mut command, "diff")
            .args(["--no-index", "--no-color", "-U3", "--"])
            .arg(&mine)
            .arg(theirs);
        let ran = command.output().ok();
        let _ = std::fs::remove_file(&mine);
        let ran = ran?;
        // `--no-index` exits 1 when the two differ: that is the answer.
        (ran.status.code() == Some(1) || ran.status.success()).then(|| hunks_only(&ran.stdout))
    }

    /// Joins what both sides changed and a merge can join: a clean merge
    /// leaves the conflicts for a [`Step::Merge`]; one that clashes stays a
    /// conflict that says where ([`Conflict::merge`]). Gives back what it
    /// merged, for the carrying and for the person's merged decisions.
    pub(super) fn merge_both(
        &self,
        ring: &Keyring,
        index: &Index,
        base: Option<&Last>,
        plan: &mut Plan,
    ) -> Result<Merges, Refusal> {
        let mut merges = Merges::default();
        // A FIRST SYNC HAS NO BASE: what differs is the restore's question.
        let Some(base) = base else {
            return Ok(merges);
        };
        let blobs = index.by_path();
        let mut kept_conflicts = Vec::with_capacity(plan.conflicts.len());
        for mut c in std::mem::take(&mut plan.conflicts) {
            let Some(three) = self.sealed_sides(ring, &blobs, base, &c)? else {
                kept_conflicts.push(c);
                continue;
            };
            self.offer(&mut merges, &mut c, three, plan, &mut kept_conflicts)?;
        }
        plan.conflicts = kept_conflicts;
        Ok(merges)
    }

    /// Merges `three` for `c`: a clean merge becomes a [`Step::Merge`] of
    /// `plan`, a merge that clashes keeps `c` (saying where) in `conflicts`,
    /// and one that cannot be made keeps it as it was.
    pub(super) fn offer(
        &self,
        merges: &mut Merges,
        c: &mut Conflict,
        three: Three,
        plan: &mut Plan,
        conflicts: &mut Vec<Conflict>,
    ) -> Result<(), Refusal> {
        match self.merge_three(&c.path, &three)? {
            Tried::Joined(bytes) => {
                merges.joined.insert(
                    c.path.clone(),
                    Joined {
                        bytes,
                        mode: three.mode,
                    },
                );
                plan.entries.push(Entry {
                    path: c.path.clone(),
                    step: Step::Merge,
                });
            }
            Tried::Clashed(merging) => {
                c.merge = Some(merging);
                merges.three.insert(c.path.clone(), three);
                conflicts.push(std::mem::take(c));
            }
            Tried::Not => conflicts.push(std::mem::take(c)),
        }
        Ok(())
    }

    /// The three sides of a sealed conflict, when a merge may be tried:
    /// both sides files of text at most [`TEXT_MOST_BYTES`], read as the
    /// plan saw them, and a base in the sealed cache.
    fn sealed_sides(
        &self,
        ring: &Keyring,
        blobs: &BTreeMap<&str, &Blob>,
        base: &Last,
        c: &Conflict,
    ) -> Result<Option<Three>, Refusal> {
        let (Some(local), Some(remote)) = (&c.local, &c.remote) else {
            return Ok(None);
        };
        let small = |s: &crate::plan::Side| s.kind == Kind::File && s.size_bytes <= TEXT_MOST_BYTES;
        if !small(local) || !small(remote) || kept::merge_rule(&c.path) == MergeRule::Choose {
            return Ok(None);
        }
        let Some(known) = base.entries.get(&c.path).filter(|k| k.kind == Kind::File) else {
            return Ok(None);
        };
        let Some(base_bytes) = self.base_of(ring, &known.sha256) else {
            return Ok(None);
        };
        let Some(blob) = blobs.get(c.path.as_str()) else {
            return Ok(None);
        };
        let theirs = self.read_blob(ring, blob)?;
        let ours = self
            .places
            .resolve(&c.path)
            .and_then(|p| walk::head(&p, TEXT_MOST_BYTES + 1));
        let Some(ours) = ours.filter(|o| sha256(o) == local.sha256) else {
            return Ok(None);
        };
        if sha256(&theirs) != remote.sha256 {
            return Ok(None);
        }
        Ok(Some(Three {
            base: base_bytes,
            ours,
            theirs,
            mode: if known.mode == local.mode {
                remote.mode
            } else {
                local.mode
            },
        }))
    }

    /// What a merge of three sides of `logical` comes to: JSON by field when
    /// it is a JSON document, else text by hunk — or nothing, for what is
    /// not text.
    pub(super) fn merge_three(&self, logical: &str, three: &Three) -> Result<Tried, Refusal> {
        if ![&three.base, &three.ours, &three.theirs]
            .iter()
            .all(|b| walk::is_text(b))
        {
            return Ok(Tried::Not);
        }
        if let Some(joined) =
            merge_json::merge(logical, &three.base, &three.ours, &three.theirs, &|_| None)
        {
            return Ok(if joined.clashes.is_empty() {
                Tried::Joined(joined.bytes)
            } else {
                Tried::Clashed(Merging::Json {
                    fields: joined
                        .clashes
                        .iter()
                        .map(merge_json::Clash::conflict)
                        .collect(),
                })
            });
        }
        let favor = match kept::merge_rule(logical) {
            MergeRule::Union => Favor::Union,
            MergeRule::Auto | MergeRule::Choose => Favor::None,
        };
        let Some(merged) = self.merge_text(three, favor) else {
            return Ok(Tried::Not);
        };
        Ok(if merged.conflicts == 0 {
            Tried::Joined(merged.bytes)
        } else {
            Tried::Clashed(Merging::Text {
                hunks: merged.conflicts,
            })
        })
    }

    /// `git merge-file` over the three sides, in the runtime folder (they
    /// are the person's files). `None` when git could not merge them.
    fn merge_text(&self, three: &Three, favor: Favor) -> Option<ronoctua_git::FileMerge> {
        let scratch = self.places.merging();
        make_private_dir(&scratch).ok()?;
        let merged = ronoctua_git::merge_file(
            &self.tools,
            &scratch,
            &three.base,
            &three.ours,
            &three.theirs,
            favor,
        );
        let _ = std::fs::remove_dir(&scratch);
        merged.ok()
    }

    /// The decisions a carrying takes, from what the person decided: a
    /// merged decision — or every field of a JSON conflict — becomes the
    /// joined file of that conflict ([`Merges::joined`]), carried as a
    /// merged decision. The rest as decided.
    ///
    /// # Errors
    ///
    /// [`Why::Undecided`] for a conflict with no decision it may take, or a
    /// merge that no longer joins.
    pub(super) fn settle_decisions(
        &self,
        plan: &Plan,
        file: &DecisionsFile,
        merges: &mut Merges,
    ) -> Result<Decisions, Refusal> {
        let mut out = Decisions::new();
        for c in &plan.conflicts {
            let decided = file.paths.get(&c.path).copied().filter(|d| d.allowed(c));
            match decided {
                Some(d @ (Decision::TakeMergedOurs | Decision::TakeMergedTheirs)) => {
                    let side = if d == Decision::TakeMergedOurs {
                        Pick::Ours
                    } else {
                        Pick::Theirs
                    };
                    self.join_picked(merges, &c.path, &|_| Some(side))?;
                    out.insert(c.path.clone(), d);
                }
                Some(d) => {
                    out.insert(c.path.clone(), d);
                }
                None if file.fields_settle(c) => {
                    self.join_picked(merges, &c.path, &|pointer| {
                        file.field(&c.path, pointer).map(|d| {
                            if d == Decision::TakeRemote {
                                Pick::Theirs
                            } else {
                                Pick::Ours
                            }
                        })
                    })?;
                    // EVERY FIELD DECIDED is a merged file like the others:
                    // the carrying writes what was joined.
                    out.insert(c.path.clone(), Decision::TakeMergedOurs);
                }
                None => return Err(Refusal::of(Why::Undecided)),
            }
        }
        Ok(out)
    }

    /// Joins a conflict's three sides with each clash taking `pick`'s side,
    /// into [`Merges::joined`].
    fn join_picked(
        &self,
        merges: &mut Merges,
        logical: &str,
        pick: &dyn Fn(&str) -> Option<Pick>,
    ) -> Result<(), Refusal> {
        let undecided = Refusal::of(Why::Undecided);
        let three = merges.three.get(logical).ok_or(undecided)?;
        let bytes = if let Some(joined) =
            merge_json::merge(logical, &three.base, &three.ours, &three.theirs, pick)
        {
            joined.bytes
        } else {
            let favor = match pick("") {
                Some(Pick::Theirs) => Favor::Theirs,
                Some(Pick::Ours) => Favor::Ours,
                None => return Err(undecided),
            };
            let merged = self.merge_text(three, favor).ok_or(undecided)?;
            if merged.conflicts > 0 {
                return Err(undecided);
            }
            merged.bytes
        };
        let mode = three.mode;
        merges
            .joined
            .insert(logical.to_owned(), Joined { bytes, mode });
        Ok(())
    }

    /// A merged file, there: its chunks written, its blob for the index.
    ///
    /// # Errors
    ///
    /// [`Why::Undecided`] when nothing was merged for it, and what writing
    /// chunks refuses.
    pub(super) fn push_joined(
        &self,
        store: &Store<'_>,
        logical: &str,
        merges: &Merges,
        written: &mut Written,
    ) -> Result<Blob, Refusal> {
        let joined = merges
            .joined
            .get(logical)
            .ok_or(Refusal::of(Why::Undecided))?;
        let put = store.put_bytes(&joined.bytes, written)?;
        Ok(Blob {
            path: logical.to_owned(),
            side: crate::plan::Side {
                size_bytes: put.size_bytes,
                modified_s: self.now_s,
                sha256: put.sha256,
                kind: Kind::File,
                mode: joined.mode,
            },
            machine: self.machine.clone(),
            added: false,
            chunks: put.chunks,
        })
    }

    /// A merged file, here: what was there set aside (when `stamp` says
    /// where), the file through a draft born with its mode.
    ///
    /// # Errors
    ///
    /// [`Why::Undecided`] when nothing was merged for it, [`Why::Failed`]
    /// when it could not be written.
    pub(super) fn land_joined(
        &self,
        logical: &str,
        merges: &Merges,
        stamp: Option<&Path>,
    ) -> Result<(), Refusal> {
        let joined = merges
            .joined
            .get(logical)
            .ok_or(Refusal::of(Why::Undecided))?;
        let dest = self
            .places
            .resolve(logical)
            .ok_or(Refusal::of(Why::Failed))?;
        if let Some(stamp) = stamp {
            self.set_aside(&dest, stamp, logical)?;
        }
        write_with_mode(&dest, &joined.bytes, joined.mode).map_err(|_| Refusal::of(Why::Failed))
    }

    /// Keeps the base of every path `last` names that a merge may need —
    /// text of at most [`TEXT_MOST_BYTES`], whose target does not say
    /// `choose` — SEALED to the identity, and drops every other. Read from
    /// this machine's file, which the sync just made equal to the index's.
    /// Best effort: a base missing is a conflict later, never a guess.
    pub(super) fn keep_bases(&self, ring: &Keyring, last: &Last) {
        let dir = self.places.base_cache();
        if make_private_dir(&dir).is_err() {
            return;
        }
        let wanted: BTreeMap<&str, &str> = last
            .entries
            .iter()
            .filter(|(path, k)| {
                k.kind == Kind::File
                    && k.size_bytes <= TEXT_MOST_BYTES
                    && kept::merge_rule(path) != MergeRule::Choose
            })
            .map(|(path, k)| (k.sha256.as_str(), path.as_str()))
            .collect();
        let mut kept_now: BTreeSet<String> = BTreeSet::new();
        for (sha, path) in &wanted {
            let file = base_file(&dir, sha);
            if file.is_file() {
                kept_now.insert((*sha).to_owned());
                continue;
            }
            let Some(bytes) = self
                .places
                .resolve(path)
                .and_then(|p| walk::head(&p, TEXT_MOST_BYTES + 1))
            else {
                continue;
            };
            if sha256(&bytes) != *sha || !walk::is_text(&bytes) {
                continue;
            }
            if let Ok(sealed) = ring.seal_bytes(&bytes)
                && write_private(&file, &sealed).is_ok()
            {
                kept_now.insert((*sha).to_owned());
            }
        }
        // PRUNED TO WHAT `last.json` NAMES: a base of no path is nothing's.
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let keep = name
                .to_str()
                .and_then(|n| n.strip_suffix(BASE_SUFFIX))
                .is_some_and(|sha| kept_now.contains(sha));
            if !keep {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// The base whose SHA-256 is `sha`, opened from the sealed cache and
    /// checked against it. `None` when it is not kept or does not open.
    pub(super) fn base_of(&self, ring: &Keyring, sha: &str) -> Option<Vec<u8>> {
        if !crate::chunks::is_id(sha) {
            return None;
        }
        let sealed = std::fs::read(base_file(&self.places.base_cache(), sha)).ok()?;
        let plain = ring.open_bytes(&sealed).ok()?;
        (sha256(&plain) == sha).then_some(plain)
    }

    // ── anchor: L09 ──
}

/// The suffix of a sealed base: `<sha256>.age`.
pub const BASE_SUFFIX: &str = ".age";

/// The sealed base of the content whose SHA-256 is `sha`.
fn base_file(dir: &Path, sha: &str) -> std::path::PathBuf {
    dir.join(format!("{sha}{BASE_SUFFIX}"))
}

/// The SHA-256 of bytes, hexadecimal.
pub(super) fn sha256(bytes: &[u8]) -> String {
    walk::sha256_of(&mut &bytes[..]).unwrap_or_default()
}

/// Writes `bytes` at `dest` through a draft born with `mode`, then renamed
/// over it: a reader never sees half of it, and a private file is never
/// readable on the way.
pub(super) fn write_with_mode(dest: &Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let draft = draft_of(dest);
    let _ = std::fs::remove_file(&draft);
    let written = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode & 0o7777 | 0o600)
            .open(&draft)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        std::fs::set_permissions(&draft, std::fs::Permissions::from_mode(mode & 0o7777))?;
        std::fs::rename(&draft, dest)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

/// The three sides of a path both changed, read, and the mode a merge of
/// them takes (this machine's when it changed it, else the repository's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Three {
    /// What both came from.
    pub base: Vec<u8>,
    /// This machine's.
    pub ours: Vec<u8>,
    /// The repository's.
    pub theirs: Vec<u8>,
    /// The mode.
    pub mode: u32,
}

/// What a merge came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Tried {
    /// Joined, nothing clashed.
    Joined(Vec<u8>),
    /// It clashes there.
    Clashed(Merging),
    /// It could not be made (not text, git failed).
    Not,
}

/// A merged file and its mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Joined {
    /// The bytes.
    pub bytes: Vec<u8>,
    /// Its mode.
    pub mode: u32,
}

/// What the merges of one attempt made — never kept past it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Merges {
    /// Every merged file a [`Step::Merge`] or a merged decision carries, by
    /// logical path.
    pub joined: BTreeMap<String, Joined>,
    /// The three sides of every conflict a merge reached, for a merged
    /// decision.
    pub three: BTreeMap<String, Three>,
}

/// A diff from its first hunk on. git's header names the two files it was
/// made of — the session's own, under the runtime folder — and never the
/// path the person decides about: the door's title says that, and which side
/// is which. Pure.
#[must_use]
pub fn hunks_only(diff: &[u8]) -> Vec<u8> {
    let mut at = 0;
    for line in diff.split_inclusive(|b| *b == b'\n') {
        if line.starts_with(b"@@") {
            return diff[at..].to_vec();
        }
        at += line.len();
    }
    Vec::new()
}

// ── anchor: L09 ──
