//! The engine (feature `crypt`): what the extracted `ronoctua-belongings`
//! CLI runs as a child of the transfer queue (`Work::Belongings`).
//!
//! # A sync
//!
//! The passphrase arrives as a `0600` file a door wrote (`create_new`, in the
//! runtime directory), never on a command line. [`Engine::prepare`] fetches,
//! opens `key.age` with it (the one scrypt of the sync), decrypts the index,
//! walks this machine and plans ([`crate::merge`]). With no conflict it
//! carries the plan out at once; with conflicts it writes the plan, and the
//! repository's side of every conflicting TEXT (and its diff against this
//! machine's) into the session, and waits: the person decides each, a door
//! writes `decisions.json`, and [`Engine::apply`] runs.
//!
//! Carrying a plan out: what the repository had is written here (modes kept,
//! private files `0600` from birth, links as links), and what it overwrites
//! or deletes goes first to `replaced/<stamp>/` (the last three kept);
//! what this machine had is cut into chunks, and only those the repository
//! lacks are sealed and written ([`crate::chunks`]); the index is rewritten
//! (chunked too); the chunks THIS sync stopped naming are swept — and only
//! those: a chunk another machine pushed for an index it never pushed is
//! left to a compaction. One commit on top of `origin/main`, pushed
//! fast-forward — new chunks past `budget.pushMB` go first in commits of
//! their own, the index last, because GitHub refuses a push over 2 GiB. A
//! push the remote rejects because another machine pushed first sends the
//! whole thing back to the fetch, at most [`TRIES`] times ([`Why::Raced`]).
//! Then `last.json`, `fleet.json`, and the passphrase and the session are
//! deleted.
//!
//! # Nothing reaches the host before the person let it
//!
//! Every fetch and every push mints a `ronoctua_consent::Pass` for the
//! repository's host first, from `consent.json` as it is at that moment
//! ([`Engine::consent`]); without one the run is refused ([`Why::NotLet`])
//! and nothing was asked. Setting the address in the view lets its host, and
//! a host nobody let (an address from before the consent) is the view's
//! consent row.
//!
//! # One run at a time
//!
//! Every run of this machine works in one clone, so `prepare`, `apply`,
//! `abandon` and `rekey` each hold [`crate::running`]'s lock from their first
//! step to their last; one that finds it held is refused with
//! [`Why::Busy`] and touches nothing — not the secrets, not the status.
//!
//! # The first sync of an empty repository
//!
//! Makes the post-quantum identity, `key.age` with the passphrase typed,
//! `FORMAT`, `RECOVER.md`, and pushes everything this machine keeps. A
//! repository of a newer format than this build is refused ([`Why::Newer`]),
//! and one of an older format is not ours ([`Why::NotOurs`]): format 2 is
//! the first this build ever wrote.
//!
//! # Nothing blocks a screen
//!
//! This runs in the CLI, as a child of the queue: every call here may block
//! on the network or the disk.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};

use age::secrecy::SecretString;
use ronoctua_git::{Credentials, Repo, Tools};

use crate::chunks::{Store, Written};
use crate::crypt::Keyring;
use crate::fleet::{Fleet, Seen};
use crate::kept;
use crate::merge::{Blob, INDEX_VERSION, Index, referenced, under_a_root};
use crate::order::{Beat, Op, Phase};
use crate::places::Places;
use crate::plan::{Decision, Decisions, DecisionsFile, Plan, Side, Step, beside};
use crate::prefs::{Auth, Belongings, repository_is_plain};
use crate::running;
use crate::secret::{make_private_dir, write_private};
use crate::status::{Known, Last, Outcome, Status};
use crate::walk::{self, Found};
use crate::{Refusal, Why};

// THE ENGINE IN PARTS, one per package of the 2026-10-05 wave that grows it:
// the sealed part (index, blobs, keyring — the belongings-sealed package),
// the plain part and the merges (the belongings-plain-merge package), the
// compaction, the install-time restore, and the identity kept for the
// session. This file keeps the run itself: the lock, the rounds, the plan
// carried out, git.
mod compact;
mod merging;
mod plain;
mod restore;
mod sealed;
mod unlock;
// ── anchor: L08 ──
// THE PLAIN PART'S SIGNATURE: the key it is held to, what it vouches for,
// what goes up signed.
mod signing;
pub use compact::Compaction;
pub use sealed::{ATTRIBUTES, ATTRIBUTES_FILE, RECOVER, RECOVER_FILE};
pub use signing::{SignedHere, pinned_key, pinned_key_in, sign_verb};

/// The variable naming when the running distro was built (seconds since the
/// epoch; the flake's `lastModified`). Read at run time, so a new build
/// never recompiles anything; else [`BUILT_FILE`].
pub const BUILT_VARIABLE: &str = "RONOCTUA_BUILT_S";

/// Where the system writes the same number (`nix/modules/belongings.nix`):
/// a file, so a switch changes it without restarting the queue.
pub const BUILT_FILE: &str = "/etc/ronoctua/built-s";

/// When the running distro was built, seconds since the epoch — what the
/// index remembers of this machine beside its last sync (`Seen::built_s`).
/// `None` when nothing says.
#[must_use]
pub fn built_s() -> Option<i64> {
    let said = std::env::var(BUILT_VARIABLE)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| std::fs::read_to_string(BUILT_FILE).ok())?;
    said.trim().parse().ok().filter(|s: &i64| *s > 0)
}

/// This machine's engine for `prefs`, or the refusal of a run that never
/// started — said in `status.json`, and what a door typed for it deleted
/// (an order's way, [`refuse_unstarted`]).
fn ready<'a>(places: &'a Places, prefs: &'a Belongings) -> Result<Engine<'a>, Refusal> {
    let ready = if !prefs.has_repository() || repository_is_plain(&prefs.repository).is_err() {
        Err(Refusal::of(Why::NoRepository))
    } else {
        Engine::for_this_machine(places, prefs)
    };
    ready.map_err(|r| refuse_unstarted(places, now_s(), r.why))
}

/// How many conflicts a run leaves waiting.
fn conflicts_of(done: &Done) -> u32 {
    match done {
        Done::Synced(_) => 0,
        Done::Waiting(plan) => u32::try_from(plan.conflicts.len()).unwrap_or(u32::MAX),
    }
}
// ── anchor: L09 ──
// THE PLAIN PART ALONE AND THE INSTALLER'S VERBS, for the CLI's flags.
pub use plain::{Opened, plain_out, plain_verb};
pub use restore::{
    Initialized, Named, PROBE_BRANCH, Probed, SIGNER_OUT, format_opened, init_named, probe_access,
    probe_address, restore_named, take_named, tells_of_the_passphrase,
};
// ── anchor: L10 ──
// THE PREDEFINED TARGETS (requests, private folders, offers) and THE
// IDENTITY KEPT FOR THE SESSION.
mod targets;
use unlock::Opening;
pub use unlock::{forget_everywhere, forget_everywhere_under, forget_here, forget_here_under};

pub use merging::hunks_only;

/// How many times a push another machine beat is tried again.
pub const TRIES: usize = 3;

/// The branch the belongings live on.
pub const BRANCH: &str = "main";

/// The file in the repository saying what it is.
pub const FORMAT_FILE: &str = "FORMAT";
/// The identity and the signing key, encrypted to the passphrase.
pub const KEY_FILE: &str = crate::places::Places::KEY_AGE;
/// The index's head, sealed to the identity.
pub const INDEX_FILE: &str = "index.age";

/// How many `replaced/<stamp>/` folders are kept.
pub const REPLACED_KEPT: usize = 3;

/// The largest text a conflict's diff is made of, in bytes.
pub const TEXT_MOST_BYTES: u64 = 1024 * 1024;

/// The name and address every commit is authored with — never the machine's.
pub const AUTHOR_NAME: &str = "ronoctua-belongings";
/// See [`AUTHOR_NAME`].
pub const AUTHOR_EMAIL: &str = "noreply@localhost";

/// The variable naming `ssh-keygen`'s store path (`nix/rust.nix` compiles it
/// in; the environment wins).
pub const KEYGEN_VARIABLE: &str = "RONOCTUA_SSH_KEYGEN";

/// What a run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Done {
    /// Synced.
    Synced(Outcome),
    /// Prepared, and waiting for the person's decisions.
    Waiting(Plan),
}

/// One run of the belongings, on this machine.
pub struct Engine<'a> {
    /// Where everything is.
    pub places: &'a Places,
    /// What the person chose.
    pub prefs: &'a Belongings,
    /// git and ssh, by store path.
    pub tools: Tools,
    /// How the repository is reached; `None` for a local one (a test's).
    pub credentials: Option<Credentials>,
    /// This machine's name, written only inside the index.
    pub machine: String,
    /// scrypt's work factor for a new `key.age`; `None` is `age`'s choice
    /// (about a second here). A test hands a small one.
    pub work: Option<u8>,
    /// Now, seconds since the epoch.
    pub now_s: i64,
    /// Run right before every push — how a test makes another machine push
    /// first. `None` everywhere else.
    pub before_push: Option<&'a dyn Fn()>,
    /// What mints the pass for the repository's host — the person's
    /// `consent.json` ([`ronoctua_consent::pass`]) everywhere but a test,
    /// which hands its own consent file.
    pub consent: &'a dyn Fn(&str) -> Result<ronoctua_consent::Pass, ronoctua_consent::NotLet>,
    /// When this machine's distro was built ([`built_s`]), for the index.
    pub built_s: Option<i64>,
    /// The most new chunk bytes one push carries ([`kept::PUSH_MOST_BYTES`];
    /// a test hands a small one to see the batches).
    pub push_most_bytes: u64,
    /// Where the identity is kept for the session ([`crate::unlock`]): the
    /// kernel's keyring on this machine, a test's own memory — `None` for
    /// the installer's verbs, which run on a system that is not the
    /// person's and keep nothing.
    pub vault: Option<&'a dyn crate::unlock::Vault>,
    /// The signing key root pinned on this machine ([`pinned_key`]): what a
    /// run that did not open `key.age` holds the plain part to — `None`
    /// with none pinned (and for the installer's verbs, a system that is not
    /// the person's), or a test's own.
    pub pinned: Option<ronoctua_pq::sign::PublicKey>,
}

impl std::fmt::Debug for Engine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("places", &self.places)
            .field("machine", &self.machine)
            .finish_non_exhaustive()
    }
}

/// What the page says is past the "large" line (`kept::LARGE_BYTES`) among
/// `entries` — what a sync keeps: every folder or file the person added,
/// and every path of a target (an Emacs configuration that carries a folder
/// of data, a GnuPG keyring grown huge), each with its weight, and a path
/// that is both (an addition made at a target's path) once. Pure.
fn large_of(prefs: &Belongings, entries: &BTreeMap<String, Known>) -> Vec<(String, u64)> {
    let weight = |root: &str| {
        entries
            .iter()
            .filter(|(p, _)| crate::places::covers(root, p))
            .map(|(_, k)| k.size_bytes)
            .sum::<u64>()
    };
    let mut said = BTreeSet::new();
    prefs
        .extra
        .iter()
        .map(String::as_str)
        .chain(
            crate::scope::targets()
                .iter()
                .flat_map(|t| t.paths.iter().copied()),
        )
        .filter(|root| said.insert(*root))
        .map(|root| (root.to_owned(), weight(root)))
        .filter(|(_, bytes)| *bytes > kept::LARGE_BYTES)
        .collect()
}

/// Seconds since the epoch, now.
#[must_use]
pub fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// This machine's name (`uname`), for the index.
#[must_use]
pub fn machine_name() -> String {
    let name = rustix::system::uname();
    let name = name.nodename().to_string_lossy().into_owned();
    if name.is_empty() {
        "machine".to_owned()
    } else {
        name
    }
}

/// How `prefs` reaches its repository on this machine: the deploy key or the
/// token, which must exist. Either way the session is POST-QUANTUM ONLY
/// (`pq_only`) — over SSH the key exchanges, over HTTPS the TLS groups: what
/// travels is the person's whole encrypted home, and a server that offers no
/// post-quantum key agreement is [`Why::NotPostQuantum`] before anything is
/// sent.
///
/// # Errors
///
/// [`Why::NeedsAccess`] when the key or the token is not there.
pub fn credentials_of(places: &Places, prefs: &Belongings) -> Result<Credentials, Refusal> {
    match prefs.auth {
        Auth::Ssh => {
            let key = places.deploy_key();
            if !key.is_file() {
                return Err(Refusal::of(Why::NeedsAccess));
            }
            Ok(Credentials::Ssh {
                key,
                known_hosts: places.known_hosts(),
                pq_only: true,
            })
        }
        Auth::Token => {
            let file = places.token();
            if !file.is_file() {
                return Err(Refusal::of(Why::NeedsAccess));
            }
            Ok(Credentials::Token {
                file,
                user: prefs.user.clone().filter(|u| !u.is_empty()),
                pq_only: true,
            })
        }
    }
}

/// What a failed git call means for the belongings.
const fn why_of(refusal: ronoctua_git::Refusal) -> Why {
    use ronoctua_git::Why as G;
    match refusal.why {
        G::NeedsCredentials => Why::NeedsAccess,
        G::Refused => Why::Refused,
        G::Unreachable => Why::Unreachable,
        // A server with only the old SHA-1 exchanges answers a session that
        // is not narrowed this way; the belongings' always is, so for them it
        // is the same truth: no post-quantum key exchange was offered.
        G::NotPostQuantum | G::NoCommonKeyExchange => Why::NotPostQuantum,
        G::NoGit | G::NoSsh | G::NoAskpass | G::Unstartable => Why::NoTools,
        G::NotARepo | G::Failed | G::Garbled | G::TooLarge => Why::Failed,
    }
}

/// Reads a secret a door wrote: a regular file of ours that nobody else can
/// read, one trailing newline dropped.
///
/// # Errors
///
/// [`Why::NoPassphrase`] when it is missing, empty, a link, not ours, or
/// readable by somebody else.
pub fn read_secret(path: &Path) -> Result<SecretString, Refusal> {
    let refused = Refusal::of(Why::NoPassphrase);
    let meta = std::fs::symlink_metadata(path).map_err(|_| refused)?;
    let mine = rustix::process::getuid().as_raw();
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 || meta.uid() != mine {
        return Err(refused);
    }
    let mut text = std::fs::read_to_string(path).map_err(|_| refused)?;
    if text.ends_with('\n') {
        text.pop();
    }
    if text.is_empty() {
        return Err(refused);
    }
    Ok(crate::crypt::passphrase(text))
}

/// `YYYYMMDD-HHMMSS` in UTC, for `replaced/<stamp>`: sortable as text. Pure.
#[must_use]
pub fn stamp_of(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Keeps the newest [`REPLACED_KEPT`] folders of `replaced/`.
fn rotate_replaced(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut stamps: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    stamps.sort();
    let extra = stamps.len().saturating_sub(REPLACED_KEPT);
    for old in stamps.into_iter().take(extra) {
        let _ = std::fs::remove_dir_all(old);
    }
}

/// The base and this machine's side a plan was made from — and what its
/// merges joined and its plain part planned.
struct Sides<'s> {
    base: Option<&'s Last>,
    found: &'s Found,
    merges: &'s merging::Merges,
    plain: &'s plain::PlainPlan,
}

/// What the three sides were, for one attempt.
struct Ground {
    repo: Repo,
    head: Option<String>,
}

/// The ref that remembers the repository's head this clone last built on —
/// what tells a history another machine REWROTE (a compaction) from one that
/// only grew. A remote-tracking ref cannot say it: every fetch moves it (a
/// push that lost its race fetches too), and the next attempt would compare
/// the new head with itself.
pub const BUILT_ON_REF: &str = "refs/ronoctua/built-on";

/// The commit message of a sync: nothing of the machine, nothing of a path.
pub const COMMIT_MESSAGE: &str = "belongings";

/// The commit message of a batch of chunks sent ahead of the index.
pub const CHUNKS_MESSAGE: &str = "belongings: chunks";

/// How an attempt at carrying a plan out ended.
enum Attempt {
    Done(Outcome),
    Waiting(Plan),
    /// Another machine pushed first.
    Lost,
}

impl<'a> Engine<'a> {
    /// The engine for this machine: its tools, its access to the repository,
    /// its name, now.
    ///
    /// # Errors
    ///
    /// [`Why::NoRepository`], [`Why::NoTools`], [`Why::NeedsAccess`].
    pub fn for_this_machine(places: &'a Places, prefs: &'a Belongings) -> Result<Self, Refusal> {
        if !prefs.has_repository() {
            return Err(Refusal::of(Why::NoRepository));
        }
        let tools = Tools::from_env().ok_or(Refusal::of(Why::NoTools))?;
        let credentials = Some(credentials_of(places, prefs)?);
        Ok(Self {
            places,
            prefs,
            tools,
            credentials,
            machine: machine_name(),
            work: None,
            now_s: now_s(),
            before_push: None,
            consent: &ronoctua_consent::pass,
            built_s: built_s(),
            push_most_bytes: kept::PUSH_MOST_BYTES,
            vault: Some(&crate::unlock::Kernel::USER),
            pinned: pinned_key(),
        })
    }

    /// Fetches, decrypts and plans; with no conflict, carries the plan out.
    ///
    /// # Errors
    ///
    /// A [`Refusal`]; the passphrase's file is deleted with it — but for
    /// [`Why::Busy`], which leaves everything to the run that holds the lock.
    pub fn prepare(&self, beat: &mut dyn FnMut(Beat)) -> Result<Done, Refusal> {
        let _held = running::hold(self.places)?;
        // THE PASSPHRASE TYPED, or the identity the session kept (`unlock`).
        let done = self.opening().and_then(|p| {
            // A NEW PLAN, NEW DECISIONS: what was decided about the last one
            // is about paths that may not conflict any more.
            let _ = std::fs::remove_dir_all(self.places.session());
            let _ = std::fs::remove_file(self.places.decisions());
            self.rounds(&p, None, beat)
        });
        self.settle(&done);
        done
    }

    /// Carries the plan out with the person's decisions (`decisions.json`,
    /// else the plan's suggestions).
    ///
    /// # Errors
    ///
    /// [`Why::NoSession`] with nothing prepared (the passphrase's file is
    /// deleted: nothing waits for it), [`Why::Undecided`] with a conflict left
    /// undecided (the session and its passphrase wait on), and what
    /// [`Engine::prepare`] refuses.
    pub fn apply(&self, beat: &mut dyn FnMut(Beat)) -> Result<Done, Refusal> {
        let _held = running::hold(self.places)?;
        let Some(plan) = read_json::<Plan>(&self.places.plan()) else {
            forget_typed(self.places);
            return Err(Refusal::of(Why::NoSession));
        };
        // THE PATHS' DECISIONS AND THE FIELDS' (`DecisionsFile`, which also
        // reads a door's flat map of before).
        let decisions = DecisionsFile::read(&self.places.decisions())
            .unwrap_or_else(|| DecisionsFile::of_paths(plan.suggested()));
        if !plan.settled(&decisions) {
            return Err(Refusal::of(Why::Undecided));
        }
        let done = self
            .opening()
            .and_then(|p| self.rounds(&p, Some(&decisions), beat));
        self.settle(&done);
        done
    }

    /// Drops a prepared sync: the passphrase's files and the session.
    ///
    /// # Errors
    ///
    /// [`Why::Busy`] while a run holds the lock.
    pub fn abandon(&self) -> Result<(), Refusal> {
        abandon(self.places, self.now_s)
    }

    /// Changes the passphrase: `key.age` alone is encrypted again, to the
    /// new one (`new-passphrase`), and pushed. Both files are deleted.
    ///
    /// # Errors
    ///
    /// [`Why::WrongPassphrase`] when the old one does not open `key.age`,
    /// [`Why::Empty`] for a repository with no key yet, and the network's.
    pub fn rekey(&self, beat: &mut dyn FnMut(Beat)) -> Result<(), Refusal> {
        let _held = running::hold(self.places)?;
        let done = (|| {
            // THE OLD PASSPHRASE, typed: a kept identity changes no
            // passphrase.
            let old = Opening::Typed(self.passphrase()?);
            let new = read_secret(&self.places.new_passphrase())?;
            for _ in 0..TRIES {
                beat(step(Phase::Fetching));
                let ground = self.ground()?;
                let Some(head) = ground.head.as_deref() else {
                    return Err(Refusal::of(Why::Empty));
                };
                self.checkout(&ground.repo, head)?;
                self.check_format()?;
                beat(step(Phase::Unlocking));
                let ring = self.unlock(&old)?;
                let key_age = ring.rewrap(&new, self.work)?;
                std::fs::write(self.places.repo().join(KEY_FILE), key_age)
                    .map_err(|_| Refusal::of(Why::Failed))?;
                beat(step(Phase::Pushing));
                if self.commit_and_push(&ground.repo, &Written::default(), COMMIT_MESSAGE)? {
                    return Ok(());
                }
            }
            Err(Refusal::of(Why::Raced))
        })();
        let _ = std::fs::remove_file(self.places.new_passphrase());
        let _ = std::fs::remove_file(self.places.passphrase());
        match done {
            Ok(()) => write_status(self.places, self.now_s, Outcome::Rekeyed),
            Err(r) => write_status(self.places, self.now_s, Outcome::Refused { why: r.why }),
        }
        done
    }

    /// The passphrase the door wrote.
    fn passphrase(&self) -> Result<SecretString, Refusal> {
        read_secret(&self.places.passphrase())
    }

    /// Says how the run ended, and deletes what it no longer needs: a sync
    /// that waits for decisions keeps the passphrase and the session, anything
    /// else deletes both.
    fn settle(&self, done: &Result<Done, Refusal>) {
        match done {
            Ok(Done::Waiting(plan)) => write_status(
                self.places,
                self.now_s,
                Outcome::Waiting {
                    conflicts: u32::try_from(plan.conflicts.len()).unwrap_or(u32::MAX),
                },
            ),
            Ok(Done::Synced(outcome)) => {
                self.forget_secrets();
                write_status(self.places, self.now_s, outcome.clone());
            }
            Err(r) => {
                self.forget_secrets();
                write_status(self.places, self.now_s, Outcome::Refused { why: r.why });
            }
        }
    }

    /// Deletes the passphrase's files and the session.
    fn forget_secrets(&self) {
        forget_secrets(self.places);
    }

    /// Fetch, plan and carry out, again while another machine pushes first.
    fn rounds(
        &self,
        opening: &Opening,
        decisions: Option<&DecisionsFile>,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Done, Refusal> {
        for _ in 0..TRIES {
            match self.attempt(opening, decisions, beat)? {
                Attempt::Done(outcome) => return Ok(Done::Synced(outcome)),
                Attempt::Waiting(plan) => return Ok(Done::Waiting(plan)),
                Attempt::Lost => {}
            }
        }
        Err(Refusal::of(Why::Raced))
    }

    /// One attempt: fetch, open, plan, and carry out or wait.
    fn attempt(
        &self,
        opening: &Opening,
        decisions: Option<&DecisionsFile>,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Attempt, Refusal> {
        beat(step(Phase::Fetching));
        let ground = self.ground()?;
        let Some(head) = ground.head.clone() else {
            // AN EMPTY REPOSITORY: this machine is the first, and its
            // passphrase becomes the repository's — a TYPED one: an identity
            // kept for another repository at this address makes nothing.
            let Opening::Typed(passphrase) = opening else {
                self.forget_kept();
                return Err(Refusal::of(Why::NoPassphrase));
            };
            self.fresh_clone()?;
            let ground = self.ground()?;
            let (ring, key_age) = Keyring::new(passphrase, self.work)?;
            let repo_dir = self.places.repo();
            std::fs::write(repo_dir.join(KEY_FILE), key_age)
                .map_err(|_| Refusal::of(Why::Failed))?;
            self.write_plain_files()?;
            let mut index = Index::fresh();
            self.apply_requests(&mut index, &self.requested());
            beat(step(Phase::Reading));
            let roots = crate::merge::roots_after(None, &index, self.prefs);
            let found = walk::walk(
                &crate::scope::Scope::for_sync(self.prefs, &roots, &BTreeSet::new()),
                self.places,
            );
            let plan = crate::merge::plan(None, &found, &index, self.prefs);
            // THE PLAIN PART TOO, all of it new there — with NO BASE, as the
            // sealed part: a repository made again at the same address is
            // empty whatever `last.json` remembers of it, and a base would
            // read every plain file here as deleted there.
            let mut merges = merging::Merges::default();
            let plain = self.plan_plain(None, &mut merges, None, Some(&ring))?;
            let sides = Sides {
                base: None,
                found: &found,
                merges: &merges,
                plain: &plain,
            };
            let carried = self.carry(
                &ground,
                &ring,
                index,
                &plan,
                &Decisions::new(),
                &sides,
                beat,
            )?;
            // THE RECIPIENT ONLY ONCE ITS IDENTITY IS IN THE REPOSITORY: a
            // push refused (a deploy key added read-only) or beaten leaves
            // the identity nowhere — the next attempt makes another — and
            // "to myself" would seal files to it that nothing could open.
            if matches!(carried, Attempt::Done(_)) {
                self.remember_recipient(&ring);
                self.keep_for_the_session(&ring, opening);
            }
            return Ok(carried);
        };
        self.checkout(&ground.repo, &head)?;
        self.check_format()?;
        beat(step(Phase::Unlocking));
        let ring = self.unlock(opening)?;
        let mut index = self.read_index(&ring)?;
        // WHAT A DOOR ASKED (a target dropped or kept everywhere), in the
        // index this sync plans against and pushes.
        self.apply_requests(&mut index, &self.requested());
        beat(step(Phase::Reading));
        // A REPAIR THIS MACHINE MAY WRITE BY ITSELF — it runs the newest
        // distro of the person's machines (`fleet.json`) — is written now,
        // so this very sync sends the file it fixed (`inventory`).
        crate::inventory::repair_when_newest(self.places);
        let base =
            Last::read(&self.places.last()).filter(|l| l.repository == self.prefs.repository);
        let looked: BTreeSet<String> = base
            .as_ref()
            .map(|b| b.entries.keys().cloned().collect())
            .unwrap_or_default();
        // WHAT A PERSON ADDED ON ANY MACHINE, walked here as folders: a file new
        // in one of them is kept too.
        let roots = crate::merge::roots_after(base.as_ref(), &index, self.prefs);
        let found = walk::walk(
            &crate::scope::Scope::for_sync(self.prefs, &roots, &looked),
            self.places,
        );
        let mut plan = crate::merge::plan(base.as_ref(), &found, &index, self.prefs);
        plan.remote_commit = Some(head);
        plan.made_s = self.now_s;
        // WHAT BOTH SIDES CHANGED AND A MERGE CAN JOIN leaves the conflicts
        // here (`merging`), and the plain part's own conflicts join them
        // (`plain`): the person decides both alike.
        beat(step(Phase::Merging));
        let mut merges = self.merge_both(&ring, &index, base.as_ref(), &mut plan)?;
        let plain =
            self.plan_plain(self.plain_base(), &mut merges, Some(&mut plan), Some(&ring))?;
        let decided = decisions.is_some_and(|d| plan.settled(d));
        if plan.conflicts.is_empty() || decided {
            let settled = match decisions {
                Some(d) if decided => self.settle_decisions(&plan, d, &mut merges)?,
                _ => Decisions::new(),
            };
            let sides = Sides {
                base: base.as_ref(),
                found: &found,
                merges: &merges,
                plain: &plain,
            };
            return self.carry(&ground, &ring, index, &plan, &settled, &sides, beat);
        }
        // WAITING: every conflict listed, its text's other side (and the
        // diff) in the session. A decision the person made before a re-plan
        // is kept as the conflict's suggestion.
        if let Some(d) = decisions {
            for c in &mut plan.conflicts {
                if let Some(chosen) = d.paths.get(&c.path).filter(|x| x.allowed(c)) {
                    c.suggested = Some(*chosen);
                }
            }
        }
        self.write_session(&ring, &index, &mut plan)?;
        Ok(Attempt::Waiting(plan))
    }

    /// The clone, fetched, and the repository's head. A head that does not
    /// descend from the one this clone last built on ([`BUILT_ON_REF`]) is a
    /// history another machine rewrote (a compaction): the work tree moves to
    /// it and the old history is dropped here — in every run that fetches,
    /// whatever fetched in between.
    fn ground(&self) -> Result<Ground, Refusal> {
        let repo = self.clone_repo()?;
        // WHAT THIS CLONE LAST BUILT ON: the mark — or, in a clone from
        // before the mark, the remote's head as this fetch finds it.
        let was = match self.built_on(&repo)? {
            Some(mark) => Some(mark),
            None => self.origin_head(&repo)?,
        };
        self.fetch(&repo)?;
        let head = self.origin_head(&repo)?;
        if let Some(now) = head.as_deref() {
            let rewritten = was
                .as_deref()
                .is_some_and(|was| was != now && !self.is_ancestor(&repo, was, now));
            if rewritten {
                // NOTHING MAY STILL NAME THE OLD HISTORY when it is dropped:
                // the branch moves first, and the mark with it.
                self.checkout(&repo, now)?;
            }
            self.mark_built_on(&repo, now)?;
            if rewritten {
                self.shrink(&repo);
            }
        }
        Ok(Ground { repo, head })
    }

    /// The head this clone last built on ([`BUILT_ON_REF`]); `None` before
    /// the first.
    fn built_on(&self, repo: &Repo) -> Result<Option<String>, Refusal> {
        let out = self.git(
            repo,
            false,
            "for-each-ref",
            &["--format=%(objectname)", BUILT_ON_REF],
        )?;
        let mark = String::from_utf8_lossy(&out).trim().to_owned();
        Ok((!mark.is_empty()).then_some(mark))
    }

    /// Remembers `head` as the one this clone builds on.
    fn mark_built_on(&self, repo: &Repo, head: &str) -> Result<(), Refusal> {
        self.git(repo, false, "update-ref", &[BUILT_ON_REF, head])
            .map(|_| ())
    }

    /// `origin/main` as this clone has it; `None` before anything was
    /// fetched.
    fn origin_head(&self, repo: &Repo) -> Result<Option<String>, Refusal> {
        let out = self.git(
            repo,
            false,
            "for-each-ref",
            &["--format=%(objectname)", "refs/remotes/origin/main"],
        )?;
        let head = String::from_utf8_lossy(&out).trim().to_owned();
        Ok((!head.is_empty()).then_some(head))
    }

    /// Whether `older` is in `newer`'s history.
    fn is_ancestor(&self, repo: &Repo, older: &str, newer: &str) -> bool {
        self.git(repo, false, "merge-base", &["--is-ancestor", older, newer])
            .is_ok()
    }

    /// The pass for the repository's host, minted NOW from the person's
    /// consent — what every fetch and push holds while it asks. A repository
    /// on this machine (a folder; only a test's, since an order refuses one)
    /// asks no host and needs none.
    ///
    /// # Errors
    ///
    /// [`Why::NotLet`] when the person has not let the host — or the address
    /// names none.
    fn let_in(&self) -> Result<Option<ronoctua_consent::Pass>, Refusal> {
        let address = self.prefs.repository.trim();
        match crate::prefs::host_of(address) {
            Some(host) => (self.consent)(&host)
                .map(Some)
                .map_err(|_| Refusal::of(Why::NotLet)),
            None if address.starts_with("file://") || address.starts_with('/') => Ok(None),
            None => Err(Refusal::of(Why::NotLet)),
        }
    }

    /// Fetches the repository — on a pass for its host.
    fn fetch(&self, repo: &Repo) -> Result<(), Refusal> {
        let _pass = self.let_in()?;
        repo.fetch(self.credentials.as_ref())
            .map_err(|r| Refusal::of(why_of(r)))
    }

    /// The opened `key.age` of the work tree, which must be of format 2 —
    /// or the identity the session kept, which must open its index — and the
    /// person's recipient left where a door finds it, the identity kept for
    /// the session when it was typed for (`unlock`).
    fn unlock(&self, opening: &Opening) -> Result<Keyring, Refusal> {
        let ring = self.open_ring(opening)?;
        self.remember_recipient(&ring);
        self.keep_for_the_session(&ring, opening);
        Ok(ring)
    }

    /// Drops what a rewritten history left in this clone — the old commits,
    /// no ref names them any more (the branch, the remote's and
    /// [`BUILT_ON_REF`] are all on the new head) — so the clone shrinks as
    /// the repository did. Best effort: a clone that keeps them still syncs.
    fn shrink(&self, repo: &Repo) {
        let _ = self.git(repo, false, "reflog", &["expire", "--expire=now", "--all"]);
        let _ = self.git(repo, false, "gc", &["--quiet", "--prune=now"]);
    }

    /// The clone under `state/belongings/repo`, made when it is not there,
    /// with the repository as its `origin`.
    fn clone_repo(&self) -> Result<Repo, Refusal> {
        let dir = self.places.repo();
        if !dir.join(".git").is_dir() {
            self.fresh_clone()?;
        }
        let repo = Repo::discover(&dir)
            .filter(|r| same_dir(&r.root, &dir))
            .ok_or(Refusal::of(Why::Failed))?
            .with_tools(self.tools.clone());
        for (key, value) in [
            ("user.name", AUTHOR_NAME),
            ("user.email", AUTHOR_EMAIL),
            // THE PERSON'S OWN GIT CONFIGURATION IS NOT OURS to follow: a
            // global `commit.gpgsign` would ask a hardware key for every
            // sync.
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            // SEALED CHUNKS NEITHER COMPRESS NOR DELTA: a push would spend
            // minutes of CPU looking for what encryption took away.
            ("core.compression", "0"),
            ("pack.window", "0"),
        ] {
            self.git(&repo, false, "config", &[key, value])?;
        }
        let remotes = repo.remotes().map_err(|r| Refusal::of(why_of(r)))?;
        let url = self.prefs.repository.trim();
        match remotes.iter().find(|r| r.name == "origin") {
            Some(origin) if origin.fetch_url == url => {}
            Some(_) => {
                self.git(&repo, false, "remote", &["set-url", "origin", url])?;
            }
            None => {
                self.git(&repo, false, "remote", &["add", "origin", url])?;
            }
        }
        Ok(repo)
    }

    /// A new, empty clone (`git init`), what was there removed.
    fn fresh_clone(&self) -> Result<(), Refusal> {
        let dir = self.places.repo();
        let _ = std::fs::remove_dir_all(&dir);
        make_private_dir(&self.places.own()).map_err(|_| Refusal::of(Why::Failed))?;
        make_private_dir(&dir).map_err(|_| Refusal::of(Why::Failed))?;
        // THE ONE CALL NOT MADE THROUGH A `Repo`: there is none yet. It is
        // still the hardened command, and `init` reads no repository.
        let mut init =
            ronoctua_git::base(&self.tools, &dir, None).map_err(|r| Refusal::of(why_of(r)))?;
        ronoctua_git::subcommand(&mut init, "init").args(["-q", "-b", BRANCH]);
        let ran = init.output().map_err(|_| Refusal::of(Why::NoTools))?;
        if !ran.status.success() {
            return Err(Refusal::of(Why::Failed));
        }
        Ok(())
    }

    /// Runs a subcommand in the clone, with the credentials when `network`.
    fn git(
        &self,
        repo: &Repo,
        network: bool,
        name: &str,
        args: &[&str],
    ) -> Result<Vec<u8>, Refusal> {
        // A CALL THAT REACHES THE HOST holds a pass for it while it runs.
        let _pass = if network { self.let_in()? } else { None };
        let credentials = if network {
            self.credentials.as_ref()
        } else {
            None
        };
        repo.output(credentials, name, args, ronoctua_git::LISTING_MOST_BYTES)
            .map_err(|r| Refusal::of(why_of(r)))
    }

    /// The work tree at `head`, exactly: what an earlier attempt left there
    /// is thrown away.
    fn checkout(&self, repo: &Repo, head: &str) -> Result<(), Refusal> {
        self.git(repo, false, "checkout", &["-q", "-f", "-B", BRANCH, head])?;
        self.git(repo, false, "clean", &["-q", "-f", "-d", "-x"])?;
        Ok(())
    }

    /// Whether the work tree is a belongings repository this build syncs:
    /// format 2 ([`crate::format_check`]).
    fn check_format(&self) -> Result<(), Refusal> {
        crate::format_check(self.format()?)
    }

    /// A file of the work tree, whole.
    fn read_repo(&self, name: &str) -> Result<Vec<u8>, Refusal> {
        std::fs::read(self.places.repo().join(name)).map_err(|_| Refusal::of(Why::Corrupt))
    }

    /// Carries a plan out: writes here, encrypts there, commits, pushes, and
    /// leaves `last.json`.
    #[allow(clippy::too_many_arguments)]
    fn carry(
        &self,
        ground: &Ground,
        ring: &Keyring,
        mut index: Index,
        plan: &Plan,
        decisions: &Decisions,
        sides: &Sides<'_>,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Attempt, Refusal> {
        let (base, found) = (sides.base, sides.found);
        // THE ROOTS AS THEY WILL BE: the repository's, less what this machine
        // takes out, and what it added.
        let roots = crate::merge::roots_after(base, &index, self.prefs);
        let stamp = self.places.replaced().join(stamp_of(self.now_s));
        // WHAT THE REPOSITORY NAMED BEFORE THIS SYNC: the chunks of every
        // file and of the index itself. Of these, the ones the new index no
        // longer names are this sync's to sweep — and no others.
        let before = referenced(&index, self.index_head(ring)?.as_ref());
        let mut blobs: BTreeMap<String, Blob> = index
            .entries
            .drain(..)
            .map(|b| (b.path.clone(), b))
            .collect();
        let mut counts = Counts::default();
        let mut steps: Vec<(String, Step)> = plan
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.step))
            .collect();
        let mut both: Vec<String> = Vec::new();
        for c in &plan.conflicts {
            // A PLAIN PATH IS THE PLAIN PART'S to carry (`carry_plain`).
            if kept::plain_root(&c.path).is_some() {
                continue;
            }
            let decision = decisions
                .get(&c.path)
                .copied()
                .ok_or(Refusal::of(Why::Undecided))?;
            let step = match (decision, c.local.is_some(), c.remote.is_some()) {
                (Decision::KeepLocal, true, _) | (Decision::KeepBoth, true, _) => Step::Push,
                (Decision::KeepLocal, false, _) | (Decision::KeepBoth, false, _) => {
                    Step::DeleteRemote
                }
                (Decision::TakeRemote, _, true) => Step::Pull,
                (Decision::TakeRemote, _, false) => Step::DeleteLocal,
                // ── anchor: L09 ──
                // MERGED: what `settle_decisions` joined, written here and
                // sent — one with nothing joined is undecided.
                (Decision::TakeMergedOurs | Decision::TakeMergedTheirs, _, _) => {
                    if !sides.merges.joined.contains_key(&c.path) {
                        return Err(Refusal::of(Why::Undecided));
                    }
                    Step::Merge
                }
            };
            if decision == Decision::KeepBoth && c.remote.is_some() {
                both.push(c.path.clone());
            }
            steps.push((c.path.clone(), step));
        }
        let pulls = steps
            .iter()
            .filter(|(_, s)| matches!(s, Step::Pull | Step::DeleteLocal | Step::Merge))
            .count() as u64;
        let pushes = steps
            .iter()
            .filter(|(_, s)| matches!(s, Step::Push | Step::Merge))
            .count() as u64;

        // THE REPOSITORY'S SIDE OF WHAT BOTH KEEP, and of every pull, as it
        // is before this machine's blobs go in.
        let theirs: BTreeMap<String, Blob> = steps
            .iter()
            .filter(|(_, s)| *s == Step::Pull)
            .map(|(p, _)| p)
            .chain(both.iter())
            .filter_map(|p| blobs.get(p).map(|b| (p.clone(), b.clone())))
            .collect();

        // WHAT THIS MACHINE HAS, THERE — first, and nothing written here
        // until the push is in: a push another machine beat leaves this
        // machine exactly as it was, and the next attempt plans against the
        // same base.
        let repo_dir = self.places.repo();
        let aside = self.aside();
        let _ = std::fs::remove_dir_all(&aside);
        let store = Store::new(&repo_dir, ring, kept::CHUNK_SIZES).with_aside(&aside);
        let mut written = Written::default();
        let mut sent = 0u64;
        for (path, step) in &steps {
            match step {
                Step::Push => {
                    beat(Beat::Step {
                        phase: Phase::Encrypting,
                        done: sent,
                        total: pushes,
                    });
                    sent += 1;
                    let Some(source) = self.places.resolve(path) else {
                        continue;
                    };
                    let Some(blob) = self.push(&store, path, &source, &mut written)? else {
                        continue;
                    };
                    blobs.insert(path.clone(), blob);
                    counts.pushed += 1;
                }
                Step::DeleteRemote | Step::Unkeep => {
                    blobs.remove(path);
                    counts.deleted_there += 1;
                }
                Step::Merge => {
                    beat(Beat::Step {
                        phase: Phase::Encrypting,
                        done: sent,
                        total: pushes,
                    });
                    sent += 1;
                    let blob = self.push_joined(&store, path, sides.merges, &mut written)?;
                    blobs.insert(path.clone(), blob);
                    counts.pushed += 1;
                }
                _ => {}
            }
        }
        // A PERSON'S "ADDED" MARK FOLLOWS THE ROOTS: under one, added; its
        // root taken out, no longer.
        for (path, blob) in &mut blobs {
            blob.added = under_a_root(&roots, path);
        }
        index.version = INDEX_VERSION;
        index.entries = blobs.into_values().collect();
        index.roots.clone_from(&roots);
        // THE FOLDERS OF A PRIVATE TARGET, as this machine has them (`targets`).
        let previous = std::mem::take(&mut index.dirs);
        index.dirs = self.private_dirs(&index, &previous);
        index.machines.insert(
            self.machine.clone(),
            Seen {
                at_s: self.now_s,
                built_s: self.built_s,
            },
        );
        let head = self.write_index(&store, ring, &index, &mut written)?;
        // THE SWEEP: what this sync stopped naming. A chunk the new index
        // still names elsewhere (two files, one content) stays; one the
        // repository's side of a path both keep still needs — pulled beside,
        // after the push — waits aside until then.
        let after = referenced(&index, Some(&head));
        let read_later: BTreeSet<&str> = theirs
            .values()
            .flat_map(|b| &b.chunks)
            .map(|c| c.id.as_str())
            .collect();
        for id in before.difference(&after) {
            if read_later.contains(id.as_str()) {
                store.put_aside(id);
            } else {
                store.remove(id);
            }
        }
        self.write_plain_files()?;

        // THE PLAIN PART, beside the sealed one in the same commit (`plain`),
        // signed with the key the passphrase opened.
        let landing = self.carry_plain(sides.plain, decisions, sides.merges, true, Some(ring))?;
        beat(step(Phase::Pushing));
        if !self.commit_and_push(&ground.repo, &written, COMMIT_MESSAGE)? {
            return Ok(Attempt::Lost);
        }
        // WHAT A DOOR ASKED OF THIS SYNC, consumed under the lock once the
        // push is in.
        self.consume_requests(&index)?;
        let commit =
            String::from_utf8_lossy(&self.git(&ground.repo, false, "rev-parse", &["HEAD"])?)
                .trim()
                .to_owned();

        // WHAT THE REPOSITORY HAD, HERE NOW — and what both keep, beside. A
        // path that could not be written keeps its OLD base below, so the
        // next sync pulls it again instead of reading this machine's old file
        // as a change to push over the other's.
        let mut unwritten: BTreeSet<String> = BTreeSet::new();
        // WHAT THE REPOSITORY BROUGHT HERE: the folders of a private target
        // it landed in, and what it offers (`targets`).
        let mut landed: Vec<String> = Vec::new();
        // WHAT THIS RUN WROTE HERE: the files whose time it vouches for when
        // it records the base's (`inventory::stamp_sealed`).
        let mut written_here: BTreeSet<String> = BTreeSet::new();
        let mut done = 0u64;
        for path in &both {
            let Some(blob) = theirs.get(path) else {
                continue;
            };
            let there = beside(path, &blob.machine);
            if let Some(dest) = self.places.resolve(&there) {
                match self.pull(ring, blob, &dest, &stamp, &there) {
                    Ok(()) => {
                        counts.pulled += 1;
                        written_here.insert(there);
                    }
                    Err(_) => {
                        unwritten.insert(there);
                    }
                }
            }
        }
        for (path, step) in &steps {
            match step {
                Step::Pull => {
                    beat(Beat::Step {
                        phase: Phase::Pulling,
                        done,
                        total: pulls,
                    });
                    done += 1;
                    let written = theirs
                        .get(path)
                        .zip(self.places.resolve(path))
                        .map_or(Err(Refusal::of(Why::Corrupt)), |(blob, dest)| {
                            self.pull(ring, blob, &dest, &stamp, path)
                        });
                    match written {
                        Ok(()) => {
                            counts.pulled += 1;
                            landed.push(path.clone());
                        }
                        Err(_) => {
                            unwritten.insert(path.clone());
                        }
                    }
                }
                Step::Merge => {
                    done += 1;
                    match self.land_joined(path, sides.merges, Some(&stamp)) {
                        Ok(()) => {
                            counts.pulled += 1;
                            written_here.insert(path.clone());
                        }
                        Err(_) => {
                            unwritten.insert(path.clone());
                        }
                    }
                }
                Step::DeleteLocal => {
                    done += 1;
                    let gone = self.places.resolve(path).map_or(Ok(()), |dest| {
                        self.set_aside(&dest, &stamp, path).and_then(
                            |()| match std::fs::remove_file(&dest) {
                                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                                    Err(Refusal::of(Why::Failed))
                                }
                                _ => Ok(()),
                            },
                        )
                    });
                    match gone {
                        Ok(()) => counts.deleted_here += 1,
                        Err(_) => {
                            unwritten.insert(path.clone());
                        }
                    }
                }
                _ => {}
            }
        }

        // A PRIVATE TARGET'S FOLDERS, where a file of it just landed.
        self.restore_dirs(&landed, &index.dirs);
        let brought = landed;
        written_here.extend(brought.iter().cloned());

        // THE PLAIN PART, HERE NOW — what a decision overwrites set aside,
        // and a file that moved since the plan read it left alone: the
        // outcome lists both as not written, and the next sync takes them.
        let landed = self.land_plain(sides.plain, &landing, sides.merges, Some(&stamp));
        counts.pulled += u32::try_from(landed.changed.len()).unwrap_or(u32::MAX);
        counts.pushed += u32::try_from(landing.pushed.len()).unwrap_or(u32::MAX);
        let plain_base = self.plain_base_after(sides.plain, &landing, &landed)?;
        self.settle_signed(sides.plain, &landing);
        written_here.extend(landed.changed.iter().cloned());
        let plain_unwritten: Vec<String> = landed
            .unwritten
            .iter()
            .chain(&landed.moved)
            .cloned()
            .collect();

        // THE NEW BASE: what the repository holds now, of what this machine
        // keeps — but the old one for what could not be written here.
        let old = base.map(|b| &b.entries);
        // A TARGET NO MACHINE KEEPS is in no machine's base.
        let dropped: BTreeSet<String> = index.dropped.keys().cloned().collect();
        let mut entries: BTreeMap<String, Known> = index
            .entries
            .iter()
            .filter(|b| kept::kept_here(&b.path, self.prefs, under_a_root(&roots, &b.path)))
            .filter(|b| !kept::dropped_everywhere(&b.path, &dropped))
            .filter(|b| !unwritten.contains(&b.path))
            .map(|b| {
                (
                    b.path.clone(),
                    Known {
                        sha256: b.side.sha256.clone(),
                        kind: b.side.kind,
                        mode: b.side.mode,
                        size_bytes: b.side.size_bytes,
                        added: b.added,
                    },
                )
            })
            .collect();
        for path in &unwritten {
            if let Some(was) = old.and_then(|o| o.get(path)) {
                entries.insert(path.clone(), was.clone());
            }
        }
        let kept_bytes = entries.values().map(|k| k.size_bytes).sum();
        let large = large_of(self.prefs, &entries);
        let mut last = Last {
            repository: self.prefs.repository.clone(),
            commit,
            at_s: self.now_s,
            entries,
            roots: crate::merge::followed(&roots, self.prefs),
            plain: plain_base,
            dropped,
            modified_s: BTreeMap::new(),
        };
        // WHEN EACH FILE HERE LAST CHANGED, as this run leaves it alike with
        // its base: what a door's listing compares a `stat` with.
        crate::inventory::stamp_sealed(self.places, &mut last, self.now_s, &written_here);
        crate::inventory::stamp_plain(self.places, &mut last, self.now_s, &written_here);
        let kept = u32::try_from(last.entries.len()).unwrap_or(u32::MAX);
        let text = serde_json::to_vec_pretty(&last).map_err(|_| Refusal::of(Why::Failed))?;
        write_private(&self.places.last(), &text).map_err(|_| Refusal::of(Why::Failed))?;
        // THE BASES OF THE NEXT MERGES, sealed (`merging`).
        self.keep_bases(ring, &last);
        rotate_replaced(&self.places.replaced());
        let _ = std::fs::remove_dir_all(&aside);
        self.write_fleet(&index);
        let budget = self.measure(&ground.repo);
        Ok(Attempt::Done(Outcome::Synced {
            pulled: counts.pulled,
            pushed: counts.pushed,
            deleted_here: counts.deleted_here,
            deleted_there: counts.deleted_there,
            kept,
            kept_bytes,
            large,
            unreadable: found.unreadable.iter().cloned().collect(),
            unwritten: unwritten.into_iter().chain(plain_unwritten).collect(),
            // WHAT THE FILES THIS SYNC BROUGHT OFFER ("no" drawn first).
            offers: self.offers_after(&brought),
            // WHAT THE LEDGER HOLDS when the sync ends: a file this sync
            // wrote is listed once something reads it (the door watching it
            // does, at once), and the page reads the ledger itself.
            repaired: self.places.repaired(),
            budget,
        }))
    }

    /// `fleet.json`: the person's machines as the index now lists them —
    /// what a door reads to know whether this machine runs the newest
    /// distro. Best effort.
    fn write_fleet(&self, index: &Index) {
        let fleet = Fleet {
            this: self.machine.clone(),
            machines: index.machines.clone(),
        };
        if let Ok(text) = serde_json::to_vec_pretty(&fleet) {
            let _ = write_private(&self.places.fleet(), &text);
        }
    }

    /// What the repository weighs now: the tree, the history this clone
    /// holds (`git count-objects`), and whether a compaction of the table's
    /// `keepDays` would fold any of it. `None` when git could not say.
    fn measure(&self, repo: &Repo) -> Option<crate::status::Budget> {
        let said = self.git(repo, false, "count-objects", &["-v"]).ok()?;
        let history = crate::size::history_bytes(&String::from_utf8_lossy(&said))?;
        let folding = self
            .folding(
                repo,
                "HEAD",
                compact::cutoff_of(self.now_s, kept::KEEP_DAYS),
            )
            .ok()?;
        Some(crate::size::budget(
            crate::size::tree_bytes(&self.places.repo()),
            history,
            folding.folds(),
        ))
    }

    // ── anchor: L10 ──
    /// What a door asked of the next sync (`requests.json`), consumed after a
    /// successful push: what the pushed `index` settles leaves the file
    /// (`targets`). Never a refusal: the push is in, and a request left
    /// there is applied again, to the same effect, by the next sync.
    #[allow(clippy::unnecessary_wraps)]
    fn consume_requests(&self, index: &Index) -> Result<(), Refusal> {
        self.settle_requests(index);
        Ok(())
    }

    /// One commit of everything in the work tree, pushed fast-forward — the
    /// new chunks `written` first, in commits of their own, when they weigh
    /// more than one push may carry ([`crate::chunks::batches`]), and the
    /// index last. `false` when another machine pushed first.
    fn commit_and_push(
        &self,
        repo: &Repo,
        written: &Written,
        message: &str,
    ) -> Result<bool, Refusal> {
        for batch in crate::chunks::batches(written, self.push_most_bytes) {
            let mut args: Vec<&str> = vec!["--"];
            args.extend(batch.iter().map(String::as_str));
            self.git(repo, false, "add", &args)?;
            self.git(repo, false, "commit", &["-q", "-m", CHUNKS_MESSAGE])?;
            if !self.push_head(repo)? {
                return Ok(false);
            }
        }
        self.git(repo, false, "add", &["-A"])?;
        let status = repo.status().map_err(|r| Refusal::of(why_of(r)))?;
        if status.entries.is_empty() {
            // NOTHING NEW — but the batches above, when there were any,
            // went up: the head here is the remote's.
            return Ok(true);
        }
        self.git(repo, false, "commit", &["-q", "-m", message])?;
        self.push_head(repo)
    }

    /// Pushes this clone's head fast-forward. `false` when another machine
    /// pushed first.
    fn push_head(&self, repo: &Repo) -> Result<bool, Refusal> {
        let before = self.origin_head(repo)?;
        if let Some(first) = self.before_push {
            first();
        }
        let pushed = self.git(
            repo,
            true,
            "push",
            &[
                "-q",
                "--receive-pack=git-receive-pack",
                "origin",
                "HEAD:refs/heads/main",
            ],
        );
        match pushed {
            Ok(_) => Ok(true),
            Err(refusal) => {
                // WHETHER ANOTHER MACHINE PUSHED FIRST: its commit is there
                // now, and it was not before.
                self.fetch(repo)?;
                if self.origin_head(repo)? != before {
                    Ok(false)
                } else {
                    Err(refusal)
                }
            }
        }
    }

    /// Sets what is at `dest` aside into `replaced/<stamp>/<logical>`, when
    /// something is there: the gesture can be undone.
    fn set_aside(&self, dest: &Path, stamp: &Path, logical: &str) -> Result<(), Refusal> {
        let failed = |_| Refusal::of(Why::Failed);
        let Ok(meta) = std::fs::symlink_metadata(dest) else {
            return Ok(());
        };
        let kept = stamp.join(logical);
        if let Some(parent) = kept.parent() {
            make_private_dir(&self.places.replaced()).map_err(failed)?;
            std::fs::create_dir_all(parent).map_err(failed)?;
        }
        if meta.file_type().is_symlink() {
            let target = std::fs::read_link(dest).map_err(failed)?;
            let _ = std::fs::remove_file(&kept);
            std::os::unix::fs::symlink(target, &kept).map_err(failed)?;
        } else if meta.is_file() {
            std::fs::copy(dest, &kept).map_err(failed)?;
        }
        Ok(())
    }
}

/// Whether two paths are one folder.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// A step beat with no count.
const fn step(phase: Phase) -> Beat {
    Beat::Step {
        phase,
        done: 0,
        total: 0,
    }
}

/// What a sync moved.
#[derive(Debug, Default)]
struct Counts {
    pulled: u32,
    pushed: u32,
    deleted_here: u32,
    deleted_there: u32,
}

/// A JSON file, parsed; `None` when missing or broken.
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

/// Deletes the passphrase's files and the session.
fn forget_secrets(places: &Places) {
    let _ = std::fs::remove_file(places.passphrase());
    let _ = std::fs::remove_file(places.new_passphrase());
    let _ = std::fs::remove_file(places.decisions());
    let _ = std::fs::remove_dir_all(places.session());
}

/// Deletes what a door typed for a run that did not start: the passphrase —
/// unless a prepared sync waits, whose `apply` needs it — and always a new
/// passphrase.
fn forget_typed(places: &Places) {
    let _ = std::fs::remove_file(places.new_passphrase());
    if !places.plan().is_file() {
        let _ = std::fs::remove_file(places.passphrase());
    }
}

/// Drops a prepared sync: the passphrase's files and the session — nothing
/// else is needed to, not even the settings.
///
/// # Errors
///
/// [`Why::Busy`] while a run holds the lock: dropping its secrets under it
/// would end it half way.
pub fn abandon(places: &Places, now_s: i64) -> Result<(), Refusal> {
    let _held = running::hold(places)?;
    forget_secrets(places);
    write_status(places, now_s, Outcome::Abandoned);
    Ok(())
}

/// A run refused before it started — no repository, an address that is not
/// plain, no access, no tools: what a door typed for it is deleted (but a
/// waiting sync's passphrase) and the refusal is written, under the lock. When
/// another run holds the lock the answer is [`Why::Busy`], and nothing is
/// touched: the secrets are that run's.
#[must_use]
pub fn refuse_unstarted(places: &Places, now_s: i64, why: Why) -> Refusal {
    let held = running::hold(places);
    if let Err(r) = held
        && r.why == Why::Busy
    {
        return r;
    }
    forget_typed(places);
    write_status(places, now_s, Outcome::Refused { why });
    Refusal::of(why)
}

/// `belongings.json`, or the defaults (no repository) when it is missing or
/// broken — which a run then refuses with a sentence.
#[must_use]
pub fn read_prefs(places: &Places) -> Belongings {
    std::fs::read(places.prefs())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Carries out one order of the queue's, as the CLI does: the conflicts left
/// waiting, if any.
///
/// # Errors
///
/// The order's [`Refusal`]. Every refusal deletes what a door typed for the
/// run (but the passphrase a waiting sync keeps), and writes its sentence
/// into `status.json` — but [`Why::Busy`], which belongs to the run in
/// flight, and [`Why::Undecided`], whose session waits on.
pub fn order(op: Op, places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    match op {
        Op::Keygen => {
            beat(step(Phase::Keying));
            keygen(places)?;
            write_status(places, now_s(), Outcome::Keyed);
            return Ok(0);
        }
        // ABANDONING NEEDS NOTHING BUT THE PLACES: the passphrase and the
        // session go whatever the settings say.
        Op::Abandon => return abandon(places, now_s()).map(|()| 0),
        // THE 2026-10-05 WAVE'S ORDERS, each in the part that carries it
        // out.
        Op::Compact => return compact::compact(places, beat),
        Op::SyncPlain => return plain::sync_plain(places, beat),
        Op::Forget => return unlock::forget(places, beat),
        Op::Restore => return restore::restore(places, beat),
        Op::Probe => return restore::probe(places, beat),
        Op::Init => return restore::init(places, beat),
        Op::VerifyPassphrase => return restore::verify_passphrase(places, beat),
        // ── anchor: L08 ──
        // ── anchor: L09 ──
        Op::Sign => {
            let _ = beat;
            return signing::sign_verb(places).map(|_| 0);
        }
        // ── anchor: L10 ──
        // THE YES TO THE EXTENSIONS' OFFER: no repository, no passphrase.
        Op::VscodeExtensions => return crate::offers::install_extensions(places, beat),
        Op::Prepare | Op::Apply | Op::Rekey => {}
    }
    let prefs = read_prefs(places);
    // THE ADDRESS AGAIN, whatever wrote the file: a credential in it, `http`
    // or a local path is refused here too.
    let ready = if !prefs.has_repository() || repository_is_plain(&prefs.repository).is_err() {
        Err(Refusal::of(Why::NoRepository))
    } else {
        Engine::for_this_machine(places, &prefs)
    };
    let engine = match ready {
        Ok(engine) => engine,
        Err(r) => return Err(refuse_unstarted(places, now_s(), r.why)),
    };
    let waiting = |done: Done| match done {
        Done::Synced(_) => 0,
        Done::Waiting(plan) => u32::try_from(plan.conflicts.len()).unwrap_or(u32::MAX),
    };
    match op {
        Op::Prepare => engine.prepare(beat).map(waiting),
        Op::Apply => engine.apply(beat).map(waiting),
        Op::Rekey => engine.rekey(beat).map(|()| 0),
        Op::Abandon
        | Op::Keygen
        | Op::Compact
        | Op::SyncPlain
        | Op::Forget
        | Op::Restore
        | Op::Probe
        | Op::Init
        | Op::VerifyPassphrase
        | Op::Sign
        | Op::VscodeExtensions => Ok(0),
    }
}

/// Writes `status.json` — best effort: a status that could not be written is
/// the one thing there is nowhere to say.
pub fn write_status(places: &Places, at_s: i64, outcome: Outcome) {
    let status = Status { at_s, outcome };
    if let Ok(text) = serde_json::to_vec_pretty(&status) {
        let _ = write_private(&places.status(), &text);
    }
}

/// Makes the SSH deploy key (`ed25519`, no passphrase: the machine's own
/// access, `0600`) when there is none. Says whether it made one.
///
/// # Errors
///
/// [`Why::NoTools`] without `ssh-keygen`, [`Why::Failed`] when it failed.
pub fn keygen(places: &Places) -> Result<bool, Refusal> {
    let key = places.deploy_key();
    if key.is_file() {
        return Ok(false);
    }
    let program = std::env::var_os(KEYGEN_VARIABLE)
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(|| option_env!("RONOCTUA_SSH_KEYGEN").map(PathBuf::from))
        .ok_or(Refusal::of(Why::NoTools))?;
    make_private_dir(&places.keys()).map_err(|_| Refusal::of(Why::Failed))?;
    let _ = std::fs::remove_file(places.deploy_public());
    let ran = std::process::Command::new(program)
        .args(["-q", "-t", "ed25519", "-N", "", "-C", AUTHOR_NAME, "-f"])
        .arg(&key)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|_| Refusal::of(Why::NoTools))?;
    if !ran.status.success() || !key.is_file() {
        return Err(Refusal::of(Why::Failed));
    }
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| Refusal::of(Why::Failed))?;
    Ok(true)
}

/// The side a remote blob is, for a door's row.
#[must_use]
pub fn side_of_blob(blob: &Blob) -> Side {
    blob.side.clone()
}

#[cfg(test)]
mod tests;
