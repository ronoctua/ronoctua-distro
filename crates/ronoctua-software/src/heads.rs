//! The heads of the GitHub branches the machine's flake follows, against the
//! revisions its lock holds them at — what the update window's line says
//! when it says whether an update is there, learnt before anything is built.
//!
//! # Which branches
//!
//! The lock's own words (`ronoctua_machine::followed`): each input of the
//! machine flake whose `original` is a GitHub branch — nixpkgs'
//! `nixos-unstable`, the distro's `release` — with the revision `locked`
//! holds. A local checkout of the distro, which the update does not move,
//! is left out; an input the update moves that follows no branch is said as
//! [`Unlearnt::Unasked`], never as the same.
//!
//! # Asked as the machine's own update
//!
//! The owner's exception of 2026-11-08 (`AGENTS.md`): learning whether those
//! branches moved is the machine keeping itself at the newest version, like
//! the Nix that moves them from the same API, so it asks no consent. Every
//! request goes on the declared updater's pass (`NET`, `UPDATER`, kept to
//! this module), which is minted for `api.github.com` alone. This module is
//! the only one that asks as it.
//!
//! # One question per branch, and the cheapest one
//!
//! `GET /repos/{owner}/{repo}/commits/{branch}` with `Accept:
//! application/vnd.github.sha` answers the bare commit the branch is at, no
//! JSON around it. The head is kept [`FRESH`] seconds, one file per input
//! (so the cache is never more files than `Input::ALL`), and asked again
//! with its `ETag`: a 304 keeps it. The sixty requests an hour GitHub
//! answers unauthenticated are the follow of the distro's AppImages' too
//! (`crate::follow`): the same folder ([`crate::appimage::github_cache`])
//! remembers until when a spent allowance stays spent, and until then
//! nobody asks.
//!
//! # Never a plausible answer
//!
//! A head that could not be learnt is that branch's refusal — offline,
//! rate-limited, refused, a reply that is not a commit — never "the same",
//! and never a kept head past [`FRESH`]: an old head against a lock the
//! update has moved since would say the wrong thing either way.
//!
//! # Every function here that asks blocks
//!
//! On the network: call [`look`] on a THREAD, never on a drawing one.

use std::path::Path;

/// The machine directory's lock, by its name beside `flake.nix` — what a door
/// hands [`look`].
pub use ronoctua_machine::LOCK;
use ronoctua_machine::followed;
/// The lock's words, re-exported: a door reads the heads in the types the
/// machine reads its lock in, without linking the machine crate itself.
pub use ronoctua_machine::followed::{Branch, Followed, Input, Unread};
use serde::{Deserialize, Serialize};

use crate::net::Fetch;
use crate::{Failure, Refusal};

/// The declared updater the heads are asked as — this module's alone: a
/// public one would let any crate ask GitHub on its pass.
const UPDATER: ronoctua_consent::Updater = ronoctua_consent::Updater::MachineBranches;

/// The network the heads are asked through: the updater's pass, never the
/// person's consent.
const NET: crate::net::UpdaterNet = crate::net::UpdaterNet(UPDATER);

/// How long a kept head is said without asking again: a quarter of an hour.
/// A window opened twice in a row asks once, and an hour of them spends four
/// of the sixty questions per branch at most.
pub const FRESH: u64 = 15 * 60;

/// How long a kept head is said without asking again when the person asks
/// for the check again ([`look_again`]): a minute. Asking again is asking
/// GitHub, not the copy kept a quarter of an hour ago — and a hand that
/// asks twice in a row still spends one question.
pub const AGAIN: u64 = 60;

/// The most a reply may be: the commit is 40 bytes, and GitHub's refusals
/// (a JSON `message` and a link to its documentation) a few hundred.
pub const MOST: u64 = 4 << 10;

/// The format of a kept head, in its file name: a new shape never reads an
/// old file as if it were its own.
pub const FORMAT: u32 = 1;

/// Where a branch stands against the lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Standing {
    /// The branch is at another commit than the lock holds: the update has
    /// something to bring.
    Moved,
    /// The branch is where the lock holds it.
    Same,
}

/// A branch's head, as GitHub said it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// The branch, and where the lock holds it.
    pub branch: Branch,
    /// The commit the branch is at: 40 lower-case hex digits.
    pub sha: String,
}

impl Head {
    /// Where the branch stands against the lock.
    #[must_use]
    pub fn standing(&self) -> Standing {
        if self.sha == self.branch.locked {
            Standing::Same
        } else {
            Standing::Moved
        }
    }
}

/// What was learnt of one input the update moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heard {
    /// The input.
    pub input: Input,
    /// Its branch's head, or why it was not learnt.
    pub head: Result<Head, Unlearnt>,
}

/// Why an input's head was not learnt — whose the fault is, so a door never
/// blames GitHub for this machine's lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unlearnt {
    /// GitHub was asked, and did not answer with a commit: its refusal.
    Asked(Failure),
    /// The lock's node of the input names a GitHub branch and is not one
    /// this reads (`Followed::Refused`): nobody was asked, and what is wrong
    /// is this machine's lock.
    Lock,
    /// The update moves the input, and it follows no GitHub branch
    /// (`Followed::Unasked`): there is nobody to ask, and it is never said
    /// to be where the lock holds it.
    Unasked,
}

/// THE SHELL'S ENTRY: the heads of every GitHub branch the lock at `lock`
/// follows (the machine directory's `ronoctua_machine::LOCK`), asked on the
/// declared updater's pass, kept under `cache_dir`
/// ([`crate::appimage::github_cache`] of the session's roots). Blocks on the
/// network: call it on a thread.
///
/// # Errors
///
/// The lock's own refusal ([`Unread`]): there is none, or it does not read.
/// What GitHub could not answer is each [`Heard`]'s.
pub fn look(lock: &Path, cache_dir: &Path) -> Result<Vec<Heard>, Unread> {
    let followed = followed::read(lock)?;
    Ok(look_with(
        &NET,
        &followed,
        cache_dir,
        crate::appimage::now(),
    ))
}

/// [`look`], asked again by the person: a head kept longer than [`AGAIN`]
/// is asked of GitHub again (conditionally, with its `ETag`), not said from
/// the copy kept for [`FRESH`]. Blocks on the network: call it on a thread.
///
/// # Errors
///
/// [`look`]'s.
pub fn look_again(lock: &Path, cache_dir: &Path) -> Result<Vec<Heard>, Unread> {
    let followed = followed::read(lock)?;
    Ok(look_within(
        &NET,
        &followed,
        cache_dir,
        crate::appimage::now(),
        AGAIN,
    ))
}

/// [`look`] over the lock's branches already read, any network and the
/// clock, in the lock's order.
#[must_use]
pub fn look_with(net: &dyn Fetch, followed: &[Followed], cache_dir: &Path, now: u64) -> Vec<Heard> {
    look_within(net, followed, cache_dir, now, FRESH)
}

/// [`look_with`], a kept head said without asking for `fresh` seconds.
#[must_use]
pub fn look_within(
    net: &dyn Fetch,
    followed: &[Followed],
    cache_dir: &Path,
    now: u64,
    fresh: u64,
) -> Vec<Heard> {
    followed
        .iter()
        .map(|f| Heard {
            input: f.input(),
            head: match f {
                Followed::Branch(branch) => {
                    head_within(net, branch, cache_dir, now, fresh).map_err(Unlearnt::Asked)
                }
                Followed::Refused(_) => Err(Unlearnt::Lock),
                Followed::Unasked(_) => Err(Unlearnt::Unasked),
            },
        })
        .collect()
}

/// One branch's head: kept while [`FRESH`], else asked (conditionally, when
/// one is kept). Blocks on the network: call it on a thread.
///
/// # Errors
///
/// [`Refusal::RateLimited`] while GitHub's allowance is spent (nobody is
/// asked), [`Refusal::UnknownItem`] for a repository or a branch GitHub does
/// not have, [`Refusal::ToolFailed`] for a branch that is not one this asks
/// about or a reply that is not a commit, and the network's refusals
/// ([`Refusal::Offline`] first among them).
pub fn head_with(
    net: &dyn Fetch,
    branch: &Branch,
    cache_dir: &Path,
    now: u64,
) -> Result<Head, Failure> {
    head_within(net, branch, cache_dir, now, FRESH)
}

/// [`head_with`], a kept head said without asking for `fresh` seconds.
///
/// # Errors
///
/// [`head_with`]'s.
pub fn head_within(
    net: &dyn Fetch,
    branch: &Branch,
    cache_dir: &Path,
    now: u64,
    fresh: u64,
) -> Result<Head, Failure> {
    let named = format!("{}/{}@{}", branch.owner, branch.repo, branch.branch);
    // THE ADDRESS IS BUILT FROM THESE NAMES: checked again here, whoever
    // built the branch.
    if !(followed::is_name(&branch.owner)
        && followed::is_name(&branch.repo)
        && followed::is_branch(&branch.branch)
        && followed::is_commit(&branch.locked))
    {
        return Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("{named} is not a branch to ask about"),
        });
    }
    let head = |sha: String| Head {
        branch: branch.clone(),
        sha,
    };
    let before = kept(cache_dir, branch);
    if let Some(k) = before
        .as_ref()
        .filter(|k| k.when <= now && now - k.when < fresh)
    {
        return Ok(head(k.sha.clone()));
    }
    let until = crate::github::limited_until(cache_dir);
    if until > now {
        return Err(Failure {
            refusal: Some(Refusal::RateLimited),
            trouble: format!("api.github.com asked to wait until {until}"),
        });
    }
    let url = format!(
        "https://api.github.com/repos/{}/{}/commits/{}",
        branch.owner, branch.repo, branch.branch
    );
    let mut headers = vec![
        ("Accept", "application/vnd.github.sha"),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ];
    if let Some(tag) = before.as_ref().and_then(|k| k.etag.as_deref()) {
        headers.push(("If-None-Match", tag));
    }
    let got = net.get(&url, &headers, MOST)?;
    let reply = &got.reply;
    if let Some(until) = crate::github::spent_until(reply, now) {
        let _ = ronoctua_xdg::write_atomically(
            cache_dir,
            crate::github::LIMIT_FILE,
            until.to_string().as_bytes(),
        );
        return Err(Failure {
            refusal: Some(Refusal::RateLimited),
            trouble: format!("api.github.com answered {} for {named}", reply.status),
        });
    }
    if reply.status == 304 {
        let Some(mut k) = before else {
            return Err(Failure {
                refusal: Some(Refusal::ToolFailed),
                trouble: format!("api.github.com answered 304 for {named}, and nothing is kept"),
            });
        };
        k.when = now;
        keep(cache_dir, branch.input, &k);
        return Ok(head(k.sha));
    }
    // A BRANCH GITHUB DOES NOT HAVE is 422 ("No commit found for SHA"), a
    // repository it does not have 404.
    if matches!(reply.status, 404 | 422) {
        return Err(Failure {
            refusal: Some(Refusal::UnknownItem),
            trouble: format!("api.github.com answered {} for {named}", reply.status),
        });
    }
    if let Some(refusal) = crate::net::refusal_of_status(reply) {
        return Err(Failure {
            refusal: Some(refusal),
            trouble: format!("api.github.com answered {} for {named}", reply.status),
        });
    }
    let sha = std::str::from_utf8(&reply.body)
        .ok()
        .map(|text| text.trim_ascii())
        .filter(|text| followed::is_commit(text))
        .ok_or_else(|| Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("api.github.com's reply for {named} is not a commit"),
        })?
        .to_owned();
    keep(
        cache_dir,
        branch.input,
        &Kept {
            owner: branch.owner.clone(),
            repo: branch.repo.clone(),
            branch: branch.branch.clone(),
            sha: sha.clone(),
            etag: reply.header("etag").map(str::to_owned),
            when: now,
        },
    );
    Ok(head(sha))
}

/// What is kept of one input's head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Kept {
    /// The repository's owner it was asked of.
    owner: String,
    /// The repository.
    repo: String,
    /// The branch.
    branch: String,
    /// Its head then.
    sha: String,
    /// The reply's `ETag`, for the next conditional request.
    #[serde(default)]
    etag: Option<String>,
    /// When it was asked (seconds since the epoch).
    when: u64,
}

/// The kept file of an input: one per input, whatever branch it follows.
fn kept_name(input: Input) -> String {
    format!("head-v{FORMAT}-{}.json", input.name())
}

/// The kept head of `branch`'s input, when it is that branch's and a
/// commit's; anything else kept there is another question's.
fn kept(cache_dir: &Path, branch: &Branch) -> Option<Kept> {
    let bytes = std::fs::read(cache_dir.join(kept_name(branch.input))).ok()?;
    serde_json::from_slice::<Kept>(&bytes).ok().filter(|k| {
        k.owner == branch.owner
            && k.repo == branch.repo
            && k.branch == branch.branch
            && followed::is_commit(&k.sha)
    })
}

fn keep(cache_dir: &Path, input: Input, kept: &Kept) {
    let bytes = serde_json::to_vec(kept).unwrap_or_default();
    if let Err(e) = ronoctua_xdg::write_atomically(cache_dir, &kept_name(input), &bytes) {
        tracing::debug!(error = %e, "the branch's head could not be kept");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::canned::Canned;

    const LOCKED: &str = "f9a75f41be3a74eab55d3a7679d34ad6cc0da977";
    const NEWER: &str = "0123456789abcdef0123456789abcdef01234567";
    const NIXPKGS: &str = "https://api.github.com/repos/NixOS/nixpkgs/commits/nixos-unstable";
    const DISTRO: &str = "https://api.github.com/repos/ronoctua/ronoctua-distro/commits/release";

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-heads-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn nixpkgs(locked: &str) -> Branch {
        Branch {
            input: Input::Nixpkgs,
            owner: "NixOS".to_owned(),
            repo: "nixpkgs".to_owned(),
            branch: "nixos-unstable".to_owned(),
            locked: locked.to_owned(),
        }
    }

    fn distro(locked: &str) -> Branch {
        Branch {
            input: Input::Distro,
            owner: "ronoctua".to_owned(),
            repo: "ronoctua-distro".to_owned(),
            branch: "release".to_owned(),
            locked: locked.to_owned(),
        }
    }

    fn header(net: &Canned, at: usize, name: &str) -> Option<String> {
        net.asked.borrow()[at]
            .1
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }

    /// FAILS IF: a branch that moved past the lock is said to be where the
    /// lock holds it (or the other way round), the question is not the bare
    /// commit's, or the inputs come back in another order than the lock's.
    #[test]
    fn a_moved_branch_and_a_still_one_are_told_apart() {
        let dir = scratch("standing");
        let net = Canned::new(vec![
            (
                DISTRO,
                200,
                vec![("ETag", "\"d\"")],
                LOCKED.as_bytes().to_vec(),
            ),
            (
                NIXPKGS,
                200,
                vec![("ETag", "\"n\"")],
                format!("{NEWER}\n").into_bytes(),
            ),
        ]);
        let heard = look_with(
            &net,
            &[
                Followed::Branch(distro(LOCKED)),
                Followed::Branch(nixpkgs(LOCKED)),
            ],
            &dir,
            1_000,
        );
        assert_eq!(heard.len(), 2);
        assert_eq!(heard[0].input, Input::Distro);
        let same = heard[0].head.as_ref().unwrap();
        assert_eq!(same.standing(), Standing::Same);
        assert_eq!(heard[1].input, Input::Nixpkgs);
        let moved = heard[1].head.as_ref().unwrap();
        assert_eq!(moved.sha, NEWER);
        assert_eq!(moved.standing(), Standing::Moved);
        for at in 0..2 {
            assert_eq!(
                header(&net, at, "Accept").as_deref(),
                Some("application/vnd.github.sha")
            );
            assert_eq!(header(&net, at, "If-None-Match"), None, "nothing was kept");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a head is asked again while it is fresh, a stale one is
    /// asked without its `ETag`, a 304 does not keep it, or a kept head is
    /// judged against the lock it was asked under rather than the lock now.
    #[test]
    fn a_kept_head_is_asked_again_conditionally_and_a_304_keeps_it() {
        let dir = scratch("kept");
        let net = Canned::new(vec![(
            NIXPKGS,
            200,
            vec![("ETag", "W/\"one\"")],
            NEWER.as_bytes().to_vec(),
        )]);
        let first = head_with(&net, &nixpkgs(LOCKED), &dir, 1_000).unwrap();
        assert_eq!(first.standing(), Standing::Moved);
        // FRESH: nobody is asked again.
        let none = Canned::new(Vec::new());
        let again = head_with(&none, &nixpkgs(LOCKED), &dir, 1_000 + 60).unwrap();
        assert_eq!(again.sha, NEWER);
        assert!(none.asked.borrow().is_empty());
        // STALE: asked with the `ETag`; the 304 keeps the head, and the lock
        // the update moved since is what it is judged against.
        let net = Canned::new(vec![(NIXPKGS, 304, vec![], Vec::new())]);
        let later = head_with(&net, &nixpkgs(NEWER), &dir, 1_000 + FRESH + 1).unwrap();
        assert_eq!(later.sha, NEWER);
        assert_eq!(later.standing(), Standing::Same);
        assert_eq!(
            header(&net, 0, "If-None-Match").as_deref(),
            Some("W/\"one\"")
        );
        // …and the 304 made it fresh again.
        let none = Canned::new(Vec::new());
        head_with(&none, &nixpkgs(NEWER), &dir, 1_000 + FRESH + 2).unwrap();
        assert!(none.asked.borrow().is_empty());
        // A KEPT HEAD OF ANOTHER BRANCH is not this one's.
        let mut other = nixpkgs(LOCKED);
        other.branch = "nixos-25.11".to_owned();
        let net = Canned::new(Vec::new());
        assert_eq!(
            head_with(&net, &other, &dir, 1_000 + FRESH + 3)
                .unwrap_err()
                .refusal,
            Some(Refusal::Offline)
        );
        assert_eq!(header(&net, 0, "If-None-Match"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a check the person asks for again is answered from a head
    /// kept longer than [`AGAIN`] — the quarter of an hour of [`FRESH`] —
    /// or one asked again within that minute is asked of GitHub twice.
    #[test]
    fn asked_again_a_head_older_than_a_minute_is_asked_again() {
        let dir = scratch("again");
        let net = Canned::new(vec![(
            NIXPKGS,
            200,
            vec![("ETag", "\"one\"")],
            NEWER.as_bytes().to_vec(),
        )]);
        head_with(&net, &nixpkgs(LOCKED), &dir, 1_000).unwrap();
        // WITHIN THE MINUTE: said from the copy, as a first look would.
        let none = Canned::new(Vec::new());
        let followed = [Followed::Branch(nixpkgs(LOCKED))];
        let heard = look_within(&none, &followed, &dir, 1_000 + AGAIN - 1, AGAIN);
        assert_eq!(heard[0].head.as_ref().unwrap().sha, NEWER);
        assert!(none.asked.borrow().is_empty());
        // PAST IT, and well inside FRESH: asked again, with the `ETag`.
        let net = Canned::new(vec![(NIXPKGS, 304, vec![], Vec::new())]);
        let heard = look_within(&net, &followed, &dir, 1_000 + AGAIN + 1, AGAIN);
        assert_eq!(heard[0].head.as_ref().unwrap().sha, NEWER);
        assert_eq!(net.asked.borrow().len(), 1, "the kept head was said");
        assert_eq!(header(&net, 0, "If-None-Match").as_deref(), Some("\"one\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a head that could not be learnt is answered with a
    /// plausible one — a stale kept head while offline, "the same" for a
    /// reply that is not a commit, a 304 nobody asked for — instead of the
    /// branch's refusal, or a spent allowance is forgotten.
    #[test]
    fn what_could_not_be_learnt_is_a_refusal() {
        let dir = scratch("refused");
        let refusal = |net: &Canned, branch: &Branch, now: u64| {
            head_with(net, branch, &dir, now).unwrap_err().refusal
        };
        // OFFLINE, nothing kept — and then with a stale head kept.
        assert_eq!(
            refusal(&Canned::new(Vec::new()), &nixpkgs(LOCKED), 10),
            Some(Refusal::Offline)
        );
        let net = Canned::new(vec![(NIXPKGS, 200, vec![], LOCKED.as_bytes().to_vec())]);
        head_with(&net, &nixpkgs(LOCKED), &dir, 10).unwrap();
        assert_eq!(
            refusal(&Canned::new(Vec::new()), &nixpkgs(LOCKED), 10 + FRESH),
            Some(Refusal::Offline),
            "a stale head was said"
        );
        // NOT A COMMIT: JSON, a short one, an upper-case one, nothing.
        for body in [
            br#"{"sha":"f9a75f41be3a74eab55d3a7679d34ad6cc0da977"}"#.to_vec(),
            b"f9a75f4".to_vec(),
            LOCKED.to_ascii_uppercase().into_bytes(),
            Vec::new(),
        ] {
            let net = Canned::new(vec![(DISTRO, 200, vec![], body.clone())]);
            assert_eq!(
                refusal(&net, &distro(LOCKED), 10),
                Some(Refusal::ToolFailed),
                "{}",
                String::from_utf8_lossy(&body)
            );
        }
        // A 304 TO A QUESTION THAT WAS NOT CONDITIONAL.
        let net = Canned::new(vec![(DISTRO, 304, vec![], Vec::new())]);
        assert_eq!(
            refusal(&net, &distro(LOCKED), 10),
            Some(Refusal::ToolFailed)
        );
        // NO SUCH BRANCH, NO SUCH REPOSITORY.
        for status in [422, 404] {
            let net = Canned::new(vec![(DISTRO, status, vec![], b"{}".to_vec())]);
            assert_eq!(
                refusal(&net, &distro(LOCKED), 10),
                Some(Refusal::UnknownItem)
            );
        }
        // NOT A BRANCH TO ASK ABOUT: nothing is sent.
        let mut strange = distro(LOCKED);
        strange.branch = "release?per_page=1".to_owned();
        let net = Canned::new(Vec::new());
        assert_eq!(refusal(&net, &strange, 10), Some(Refusal::ToolFailed));
        assert!(net.asked.borrow().is_empty());
        // SPENT: refused, remembered, and until the reset nobody is asked.
        let net = Canned::new(vec![(
            DISTRO,
            403,
            vec![
                ("X-RateLimit-Remaining", "0"),
                ("X-RateLimit-Reset", "5000"),
            ],
            b"{}".to_vec(),
        )]);
        assert_eq!(
            refusal(&net, &distro(LOCKED), 100),
            Some(Refusal::RateLimited)
        );
        let net = Canned::new(Vec::new());
        assert_eq!(
            refusal(&net, &distro(LOCKED), 200),
            Some(Refusal::RateLimited)
        );
        assert!(net.asked.borrow().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a node of the lock that is not one this reads, or an input
    /// that follows no GitHub branch, is asked about, dropped instead of
    /// said, or said as GitHub's refusal — the lock's fault is this
    /// machine's, and an input nobody can ask is never the same.
    #[test]
    fn what_the_lock_cannot_ask_is_said_and_never_asked() {
        let dir = scratch("node");
        let net = Canned::new(Vec::new());
        let heard = look_with(
            &net,
            &[
                Followed::Refused(Input::Distro),
                Followed::Unasked(Input::Nixpkgs),
            ],
            &dir,
            10,
        );
        assert_eq!(
            heard,
            [
                Heard {
                    input: Input::Distro,
                    head: Err(Unlearnt::Lock),
                },
                Heard {
                    input: Input::Nixpkgs,
                    head: Err(Unlearnt::Unasked),
                },
            ]
        );
        assert!(net.asked.borrow().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: the heads are asked on the person's consent, or the
    /// updater's pass reaches any host but the API — the download host its
    /// sibling updater asks included —, refused before any socket.
    #[test]
    fn the_heads_are_asked_of_the_api_alone_as_the_declared_updater() {
        assert_eq!(UPDATER.hosts(), ["api.github.com"]);
        let pass = crate::net::Asker::Updater(UPDATER)
            .pass(NIXPKGS)
            .unwrap_or_else(|f| panic!("{f}"));
        assert!(crate::net::first_host_is(&pass, NIXPKGS));
        for elsewhere in [
            "https://github.com/NixOS/nixpkgs/archive/nixos-unstable.tar.gz",
            "https://never-let.invalid/x",
        ] {
            let refused = NET
                .get(elsewhere, &[], MOST)
                .expect_err("another host was asked");
            assert_eq!(refused.refusal, Some(Refusal::NotLet), "{elsewhere}");
        }
    }
}
