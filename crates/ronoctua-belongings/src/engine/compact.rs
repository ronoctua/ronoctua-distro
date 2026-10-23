//! The history's compaction (feature `crypt`) — offered by the view ("no"
//! drawn first), never automatic.
//!
//! # Why
//!
//! A sync never deletes a version: every file every machine pushed stays in
//! the history. The budget ([`crate::size`]) says when the history weighs
//! too much; a compaction keeps `budget.keepDays` of it and drops the rest.
//!
//! # How
//!
//! Under the run lock: fetch the head H; the tree of the last commit at or
//! before the cutoff becomes a NEW ROOT (`commit-tree`), and every later
//! commit is written again on top of it with its message and its dates kept
//! — a commit written again as it was has the same id, so an unchanged
//! history is the same history, and `keepDays = 0` is the head's tree alone.
//! Then the chunks the head's tree holds and its index does not name — the
//! orphans of another machine's lost push — are swept in one more commit,
//! and the result goes up with `--force-with-lease=refs/heads/main:H`: a push
//! another machine made after the fetch breaks the lease, and the whole
//! compaction starts over from the fetch ([`TRIES`] times). Locally, the
//! reflog is expired and `gc --prune=now` drops the old objects.
//!
//! The engine never reads a version from the history, so nothing it could
//! restore is lost.
//!
//! # Nothing to fold is said as such
//!
//! A history all younger than the cutoff, with no orphan, is
//! left as it was, and the status says so ([`Outcome::NothingToCompact`]) —
//! never "compacted" over a history that did not change. The budget asks for
//! a compaction only when one would fold something
//! ([`crate::status::Budget::foldable`]).
//!
//! # The other machines
//!
//! Nothing changes for their merge: the base is `last.json`'s content
//! hashes, and the content did not change. A machine whose fetched head does
//! not descend from the one its clone last built on drops the old history
//! the same way (`Engine::ground`). GitHub keeps unreachable objects until
//! its own collection — the screen says so.

use std::collections::BTreeSet;

use ronoctua_git::{Oid, Recommit, Repo, Signature};

use super::{Engine, TRIES, forget_typed, step, write_status};
use crate::chunks::{self, Store};
use crate::kept;
use crate::merge::referenced;
use crate::order::{Beat, Phase};
use crate::places::Places;
use crate::running;
use crate::status::{Last, Outcome};
use crate::{Refusal, Why};

/// The commit message of the sweep a compaction ends with.
pub const SWEPT_MESSAGE: &str = "belongings: compaction";

/// The format the history is read with: NUL between fields and between
/// commits (`-z`), raw dates, the whole message.
const LOG_FORMAT: &str = "--format=%H%x00%T%x00%an%x00%ae%x00%ad%x00%cn%x00%ce%x00%cd%x00%B";

/// How many fields one commit of [`LOG_FORMAT`] is.
const FIELDS: usize = 9;

/// One commit of the history, first parents only, oldest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    /// Its id.
    pub id: Oid,
    /// The tree it holds.
    pub tree: Oid,
    /// Its message, without the line ends git adds.
    pub message: String,
    /// Who wrote it, and when.
    pub author: Signature,
    /// Who committed it, and when.
    pub committer: Signature,
}

impl Line {
    /// When it was committed, seconds since the epoch.
    #[must_use]
    pub fn committed_s(&self) -> i64 {
        self.committer
            .date
            .split_once(' ')
            .and_then(|(s, _)| s.parse().ok())
            .unwrap_or(0)
    }
}

/// The history `git log -z --first-parent --reverse --date=raw` printed in
/// [`LOG_FORMAT`]. `None` when it is not that. Pure.
#[must_use]
pub fn lineage(out: &[u8]) -> Option<Vec<Line>> {
    let text = std::str::from_utf8(out).ok()?;
    let mut fields: Vec<&str> = text.split('\0').collect();
    if fields.last().is_some_and(|f| f.trim().is_empty()) {
        fields.pop();
    }
    if !fields.len().is_multiple_of(FIELDS) {
        return None;
    }
    fields
        .chunks(FIELDS)
        .map(|f| {
            // `-z` puts the commits' NUL where a line end would be: the id of
            // every commit but the first begins its field.
            let id = f[0].trim_start_matches('\n');
            let line = Line {
                id: Oid(id.to_owned()),
                tree: Oid(f[1].to_owned()),
                author: Signature {
                    name: f[2].to_owned(),
                    email: f[3].to_owned(),
                    date: f[4].to_owned(),
                },
                committer: Signature {
                    name: f[5].to_owned(),
                    email: f[6].to_owned(),
                    date: f[7].to_owned(),
                },
                message: f[8].trim_end_matches('\n').to_owned(),
            };
            let whole = |o: &Oid| o.0.len() >= 40 && o.0.bytes().all(|b| b.is_ascii_hexdigit());
            (whole(&line.id) && whole(&line.tree)).then_some(line)
        })
        .collect()
}

/// Which commit's tree becomes the new root: the last one committed at or
/// before `cutoff_s`. `None` when every commit is newer — and a root at `0`
/// folds nothing. Pure.
#[must_use]
pub fn root_at(lines: &[Line], cutoff_s: i64) -> Option<usize> {
    lines.iter().rposition(|l| l.committed_s() <= cutoff_s)
}

/// Where a compaction would root the history, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folding {
    /// The first-parent history, oldest first.
    pub lines: Vec<Line>,
    /// The new root ([`root_at`]).
    pub root: Option<usize>,
}

impl Folding {
    /// Whether a compaction would drop any commit.
    #[must_use]
    pub fn folds(&self) -> bool {
        self.root.is_some_and(|r| r > 0)
    }
}

/// What a compaction did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compaction {
    /// The history was rewritten, or its orphans swept, and pushed.
    Done {
        /// What the clone weighed before, in bytes.
        before_bytes: u64,
        /// And after.
        after_bytes: u64,
    },
    /// Nothing was old enough to fold or orphaned: the
    /// history was left as it was.
    Nothing,
}

impl Engine<'_> {
    /// Compacts the history to its last `keep_days` days, under the run lock.
    ///
    /// # Errors
    ///
    /// [`Why::NotOurs`] and [`Why::Newer`] for a repository of another
    /// format, [`Why::Empty`] for a repository
    /// with nothing in it, [`Why::Raced`] when other machines kept pushing,
    /// and what a sync refuses.
    pub fn compact(
        &self,
        keep_days: u64,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Compaction, Refusal> {
        let _held = running::hold(self.places)?;
        let done = self
            .opening()
            .and_then(|p| self.compacted(&p, keep_days, beat));
        // WHAT WAS TYPED FOR IT goes — but a waiting sync's passphrase.
        forget_typed(self.places);
        let outcome = match &done {
            Ok(Compaction::Done {
                before_bytes,
                after_bytes,
            }) => Outcome::Compacted {
                before_bytes: *before_bytes,
                after_bytes: *after_bytes,
            },
            Ok(Compaction::Nothing) => Outcome::NothingToCompact { keep_days },
            Err(r) => Outcome::Refused { why: r.why },
        };
        write_status(self.places, self.now_s, outcome);
        done
    }

    /// The compaction, tried again while other machines push.
    fn compacted(
        &self,
        opening: &super::Opening,
        keep_days: u64,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Compaction, Refusal> {
        let cutoff_s = cutoff_of(self.now_s, keep_days);
        for _ in 0..TRIES {
            beat(step(Phase::Fetching));
            let ground = self.ground()?;
            let Some(head) = ground.head.clone() else {
                return Err(Refusal::of(Why::Empty));
            };
            let before = self.clone_bytes(&ground.repo);
            self.checkout(&ground.repo, &head)?;
            self.check_format()?;
            beat(step(Phase::Unlocking));
            let ring = self.unlock(opening)?;
            let index = self.read_index(&ring)?;
            let named = referenced(&index, self.index_head(&ring)?.as_ref());

            beat(step(Phase::Compacting));
            let tip = self.rewritten(&ground.repo, &head, cutoff_s)?;
            if tip != head {
                self.checkout(&ground.repo, &tip)?;
            }
            // THE ORPHANS: chunks the tree holds and the index does not name.
            let repo_dir = self.places.repo();
            let store = Store::new(&repo_dir, &ring, kept::CHUNK_SIZES);
            let orphans: BTreeSet<String> = store.on_disk().difference(&named).cloned().collect();
            for id in &orphans {
                store.remove(id);
            }
            let mut tip = Oid(tip);
            if !orphans.is_empty() {
                self.git(
                    &ground.repo,
                    false,
                    "add",
                    &["-A", "--", chunks::CHUNKS_DIR],
                )?;
                self.git(&ground.repo, false, "commit", &["-q", "-m", SWEPT_MESSAGE])?;
                tip = Oid(self.rev(&ground.repo, "HEAD")?);
            }
            if tip.0 == head {
                // NOTHING TO FOLD AND NOTHING TO SWEEP: the history is what
                // the budget asks for already, and nothing went up.
                return Ok(Compaction::Nothing);
            }
            beat(step(Phase::Pushing));
            if !self.force(&ground.repo, &tip, &head)? {
                continue;
            }
            // THIS CLONE BUILDS ON THE NEW HISTORY now: the next fetch finds
            // it rewritten by nobody else.
            self.mark_built_on(&ground.repo, &tip.0)?;
            self.shrink(&ground.repo);
            self.recommitted(&tip);
            let after = self.clone_bytes(&ground.repo);
            return Ok(Compaction::Done {
                before_bytes: before,
                after_bytes: after,
            });
        }
        Err(Refusal::of(Why::Raced))
    }

    /// The first-parent history up to `head`, oldest first, and where a
    /// compaction at `cutoff_s` would root it — the one reading both the
    /// compaction and the budget ([`crate::status::Budget::foldable`]) take.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when git could not say.
    pub(super) fn folding(
        &self,
        repo: &Repo,
        head: &str,
        cutoff_s: i64,
    ) -> Result<Folding, Refusal> {
        let out = repo
            .output(
                None,
                "log",
                &[
                    "-z",
                    "--first-parent",
                    "--reverse",
                    "--date=raw",
                    LOG_FORMAT,
                    head,
                    "--",
                ],
                ronoctua_git::LISTING_MOST_BYTES,
            )
            .map_err(|_| Refusal::of(Why::Failed))?;
        let lines = lineage(&out).ok_or(Refusal::of(Why::Failed))?;
        let root = root_at(&lines, cutoff_s);
        Ok(Folding { lines, root })
    }

    /// The history up to `head` folded at `cutoff_s`: the id of the new tip
    /// (`head` itself when nothing is old enough to fold).
    fn rewritten(&self, repo: &Repo, head: &str, cutoff_s: i64) -> Result<String, Refusal> {
        let Folding { lines, root, .. } = self.folding(repo, head, cutoff_s)?;
        let Some(root) = root else {
            return Ok(head.to_owned());
        };
        let mut parent: Option<Oid> = None;
        for line in &lines[root..] {
            let id = repo
                .commit_tree(&Recommit {
                    tree: line.tree.clone(),
                    parent: parent.clone(),
                    message: line.message.clone(),
                    author: line.author.clone(),
                    committer: line.committer.clone(),
                })
                .map_err(|_| Refusal::of(Why::Failed))?;
            parent = Some(id);
        }
        Ok(parent.map_or_else(|| head.to_owned(), |p| p.0))
    }

    /// Forces `tip` up over `head` — on a pass, and only while the remote is
    /// still `head`. `false` when another machine pushed after the fetch.
    fn force(&self, repo: &Repo, tip: &Oid, head: &str) -> Result<bool, Refusal> {
        if let Some(first) = self.before_push {
            first();
        }
        let pushed = {
            let _pass = self.let_in()?;
            repo.push_force_with_lease(
                self.credentials.as_ref(),
                "origin",
                tip,
                super::BRANCH,
                &Oid(head.to_owned()),
            )
        };
        match pushed {
            Ok(()) => Ok(true),
            Err(refusal) => {
                self.fetch(repo)?;
                if self.origin_head(repo)?.as_deref() == Some(head) {
                    Err(Refusal::of(super::why_of(refusal)))
                } else {
                    Ok(false)
                }
            }
        }
    }

    /// A ref's commit id.
    fn rev(&self, repo: &Repo, name: &str) -> Result<String, Refusal> {
        let out = self.git(repo, false, "rev-parse", &["--verify", name])?;
        Ok(String::from_utf8_lossy(&out).trim().to_owned())
    }

    /// What the clone weighs (its `objects` folder), in bytes; `0` when git
    /// could not say where that is.
    fn clone_bytes(&self, repo: &Repo) -> u64 {
        crate::size::objects_of(repo).map_or(0, |objects| crate::size::history_bytes(&objects))
    }

    /// `last.json` names the new tip: the content it remembers is the same.
    fn recommitted(&self, tip: &Oid) {
        let Some(mut last) =
            Last::read(&self.places.last()).filter(|l| l.repository == self.prefs.repository)
        else {
            return;
        };
        tip.0.clone_into(&mut last.commit);
        if let Ok(text) = serde_json::to_vec_pretty(&last) {
            let _ = crate::secret::write_private(&self.places.last(), &text);
        }
    }
}

/// The cutoff of a compaction that keeps `keep_days` at `now_s`, seconds
/// since the epoch. Pure.
#[must_use]
pub fn cutoff_of(now_s: i64, keep_days: u64) -> i64 {
    now_s.saturating_sub(i64::try_from(keep_days.saturating_mul(86_400)).unwrap_or(i64::MAX))
}

/// `compact`, as the queue orders it: the settings, this machine's engine,
/// and the compaction to the table's `budget.keepDays`. A door that asks
/// for another number does it through `requests.json` (`Compact{keep_days}`,
/// read here, and taken out once a compaction ran on it).
///
/// # Errors
///
/// What [`Engine::compact`] refuses, and what refuses a run before it
/// starts.
pub(super) fn compact(places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    let prefs = super::read_prefs(places);
    let engine = super::ready(places, &prefs)?;
    // THE DAYS A DOOR ASKED FOR, else the table's — and the request goes
    // once a compaction ran on it (refused, it stays for the next).
    let asked = crate::requests::Requests::read(&places.requests());
    let keep_days = asked.compact_days().unwrap_or(kept::KEEP_DAYS);
    let done = engine.compact(keep_days, beat);
    if done.is_ok() && asked.compact_days().is_some() {
        let now = crate::requests::Requests::read(&places.requests());
        let _ = now.without_compaction().write(&places.requests());
    }
    done.map(|_| 0)
}

// ── anchor: L08 ──

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: char, at_s: i64) -> Line {
        let signed = Signature {
            name: "ronoctua-belongings".to_owned(),
            email: "noreply@localhost".to_owned(),
            date: format!("{at_s} +0000"),
        };
        Line {
            id: Oid(id.to_string().repeat(40)),
            tree: Oid("e".repeat(40)),
            message: "belongings".to_owned(),
            author: signed.clone(),
            committer: signed,
        }
    }

    #[test]
    fn the_history_reads_as_git_prints_it() {
        let a = "a".repeat(40);
        let t = "e".repeat(40);
        let out = format!(
            "{a}\0{t}\0ronoctua-belongings\0noreply@localhost\01700000000 +0000\0\
             ronoctua-belongings\0noreply@localhost\01700000100 +0000\0belongings\n\n\0\
             \n{b}\0{t}\0x\0y\01700000200 +0000\0x\0y\01700000300 -0300\0belongings: compaction\n\0",
            b = "b".repeat(40)
        );
        let lines = lineage(out.as_bytes()).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].message, "belongings");
        assert_eq!(lines[0].committed_s(), 1_700_000_100);
        assert_eq!(lines[1].id.0, "b".repeat(40));
        assert_eq!(lines[1].committer.date, "1700000300 -0300");
        assert_eq!(lines[1].message, "belongings: compaction");
        assert!(lineage(b"not\0a\0log").is_none());
    }

    /// FAILS IF: the new root is not the last commit at the cutoff, a
    /// history newer than the cutoff is folded at all, or `keepDays = 0`
    /// keeps more than the head.
    #[test]
    fn the_root_is_the_last_commit_at_the_cutoff() {
        let lines = [line('a', 100), line('b', 200), line('c', 300)];
        assert_eq!(root_at(&lines, 250), Some(1));
        assert_eq!(root_at(&lines, 200), Some(1), "at the cutoff is kept");
        assert_eq!(root_at(&lines, 99), None, "nothing is old enough");
        assert_eq!(root_at(&lines, 1000), Some(2), "keepDays 0: the head alone");
        assert_eq!(root_at(&[], 1000), None);
    }

    /// FAILS IF: a history that is not old enough is said to fold, or a
    /// root at its first commit is — the budget would ask for a compaction
    /// that drops nothing.
    #[test]
    fn only_a_root_after_the_first_commit_folds() {
        let lines = [line('a', 100), line('b', 200), line('c', 300)];
        let folds = |root| Folding {
            lines: lines.to_vec(),
            root,
        };
        assert!(!folds(root_at(&lines, 99)).folds());
        assert!(
            !folds(root_at(&lines, 150)).folds(),
            "a root at the first commit folds nothing"
        );
        assert!(folds(root_at(&lines, 250)).folds());
    }

    #[test]
    fn the_cutoff_counts_days_back() {
        assert_eq!(cutoff_of(1_000_000, 0), 1_000_000);
        assert_eq!(cutoff_of(1_000_000, 2), 1_000_000 - 2 * 86_400);
        assert!(cutoff_of(5, u64::MAX) < 0);
    }
}
