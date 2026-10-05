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
//! what this machine had is encrypted into fresh random blob ids; the index
//! is rewritten; one commit on top of `origin/main`, pushed fast-forward. A
//! push the remote rejects because another machine pushed first sends the
//! whole thing back to the fetch, at most [`TRIES`] times ([`Why::Raced`]).
//! Then `last.json`, and the passphrase and the session are deleted.
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
//! Makes the identity, `key.age` with the passphrase typed, `FORMAT`, and
//! pushes everything this machine keeps.
//!
//! # Nothing blocks a screen
//!
//! This runs in the CLI, as a child of the queue: every call here may block
//! on the network or the disk.

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::path::{Path, PathBuf};

use age::secrecy::SecretString;
use ronoctua_git::{Credentials, Repo, Tools};

use crate::crypt::Keyring;
use crate::kept;
use crate::merge::{Blob, INDEX_VERSION, Index, under_a_root};
use crate::order::{Beat, Op, Phase};
use crate::places::Places;
use crate::plan::{Decision, Decisions, Kind, Plan, Side, Step, beside};
use crate::prefs::{Auth, Belongings, repository_is_plain};
use crate::running;
use crate::secret::{draft_of, make_private_dir, write_private};
use crate::status::{Known, Last, Outcome, Status};
use crate::walk::{self, Found};
use crate::{FORMAT, Refusal, Why};

/// How many times a push another machine beat is tried again.
pub const TRIES: usize = 3;

/// The branch the belongings live on.
pub const BRANCH: &str = "main";

/// The file in the repository saying what it is.
pub const FORMAT_FILE: &str = "FORMAT";
/// The identity, encrypted to the passphrase.
pub const KEY_FILE: &str = "key.age";
/// The index, encrypted to the identity.
pub const INDEX_FILE: &str = "index.age";
/// The folder of the blobs.
pub const BLOBS_DIR: &str = "blobs";

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
}

impl std::fmt::Debug for Engine<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("places", &self.places)
            .field("machine", &self.machine)
            .finish_non_exhaustive()
    }
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
/// token, which must exist.
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

/// A random blob id: 16 bytes, hexadecimal.
///
/// # Errors
///
/// [`Why::Failed`] when the system has no randomness to give.
fn random_id() -> Result<String, Refusal> {
    use ring::rand::SecureRandom as _;
    let mut bytes = [0u8; 16];
    ring::rand::SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| Refusal::of(Why::Failed))?;
    Ok(walk::hex(&bytes))
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

/// The base and this machine's side a plan was made from.
struct Sides<'s> {
    base: Option<&'s Last>,
    found: &'s Found,
}

/// What the three sides were, for one attempt.
struct Ground {
    repo: Repo,
    head: Option<String>,
}

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
        let done = self.passphrase().and_then(|p| {
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
        let decisions: Decisions =
            read_json(&self.places.decisions()).unwrap_or_else(|| plan.suggested());
        if !plan.decided(&decisions) {
            return Err(Refusal::of(Why::Undecided));
        }
        let done = self
            .passphrase()
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
            let old = self.passphrase()?;
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
                let ring = Keyring::open(&self.read_repo(KEY_FILE)?, &old)?;
                let key_age = ring.rewrap(&new, self.work)?;
                std::fs::write(self.places.repo().join(KEY_FILE), key_age)
                    .map_err(|_| Refusal::of(Why::Failed))?;
                beat(step(Phase::Pushing));
                if self.commit_and_push(&ground.repo)? {
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
        passphrase: &SecretString,
        decisions: Option<&Decisions>,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Done, Refusal> {
        for _ in 0..TRIES {
            match self.attempt(passphrase, decisions, beat)? {
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
        passphrase: &SecretString,
        decisions: Option<&Decisions>,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Attempt, Refusal> {
        beat(step(Phase::Fetching));
        let ground = self.ground()?;
        let Some(head) = ground.head.clone() else {
            // AN EMPTY REPOSITORY: this machine is the first, and its
            // passphrase becomes the repository's.
            self.fresh_clone()?;
            let ground = self.ground()?;
            let (ring, key_age) = Keyring::new(passphrase, self.work)?;
            let repo_dir = self.places.repo();
            std::fs::write(repo_dir.join(FORMAT_FILE), format!("{FORMAT}\n"))
                .and_then(|()| std::fs::write(repo_dir.join(KEY_FILE), key_age))
                .map_err(|_| Refusal::of(Why::Failed))?;
            let index = Index::fresh();
            beat(step(Phase::Reading));
            let roots = crate::merge::roots_after(None, &index, self.prefs);
            let found = walk::walk(self.places, self.prefs, &roots, &BTreeSet::new());
            let plan = crate::merge::plan(None, &found, &index, self.prefs);
            let sides = Sides {
                base: None,
                found: &found,
            };
            return self.carry(
                &ground,
                &ring,
                index,
                &plan,
                &Decisions::new(),
                &sides,
                beat,
            );
        };
        self.checkout(&ground.repo, &head)?;
        self.check_format()?;
        beat(step(Phase::Unlocking));
        let ring = Keyring::open(&self.read_repo(KEY_FILE)?, passphrase)?;
        let index = self.read_index(&ring)?;
        beat(step(Phase::Reading));
        let base =
            Last::read(&self.places.last()).filter(|l| l.repository == self.prefs.repository);
        let looked: BTreeSet<String> = base
            .as_ref()
            .map(|b| b.entries.keys().cloned().collect())
            .unwrap_or_default();
        // WHAT A PERSON ADDED ON ANY MACHINE, walked here as folders: a file new
        // in one of them is kept too.
        let roots = crate::merge::roots_after(base.as_ref(), &index, self.prefs);
        let found = walk::walk(self.places, self.prefs, &roots, &looked);
        let mut plan = crate::merge::plan(base.as_ref(), &found, &index, self.prefs);
        plan.remote_commit = Some(head);
        plan.made_s = self.now_s;
        let decided = decisions.is_some_and(|d| plan.decided(d));
        if plan.conflicts.is_empty() || decided {
            let none = Decisions::new();
            let sides = Sides {
                base: base.as_ref(),
                found: &found,
            };
            return self.carry(
                &ground,
                &ring,
                index,
                &plan,
                decisions.unwrap_or(&none),
                &sides,
                beat,
            );
        }
        // WAITING: every conflict listed, its text's other side (and the
        // diff) in the session. A decision the person made before a re-plan
        // is kept as the conflict's suggestion.
        if let Some(d) = decisions {
            for c in &mut plan.conflicts {
                if let Some(chosen) = d.get(&c.path).filter(|x| x.allowed(c)) {
                    c.suggested = Some(*chosen);
                }
            }
        }
        self.write_session(&ring, &index, &mut plan)?;
        Ok(Attempt::Waiting(plan))
    }

    /// The clone, fetched, and the repository's head.
    fn ground(&self) -> Result<Ground, Refusal> {
        let repo = self.clone_repo()?;
        repo.fetch(self.credentials.as_ref())
            .map_err(|r| Refusal::of(why_of(r)))?;
        let out = self.git(
            &repo,
            false,
            "for-each-ref",
            &["--format=%(objectname)", "refs/remotes/origin/main"],
        )?;
        let head = String::from_utf8_lossy(&out).trim().to_owned();
        Ok(Ground {
            repo,
            head: (!head.is_empty()).then_some(head),
        })
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

    /// Whether the work tree is a belongings repository this build reads.
    fn check_format(&self) -> Result<(), Refusal> {
        let said = std::fs::read_to_string(self.places.repo().join(FORMAT_FILE))
            .map_err(|_| Refusal::of(Why::NotOurs))?;
        if said.lines().next().map(str::trim) != Some(FORMAT) {
            return Err(Refusal::of(Why::NotOurs));
        }
        Ok(())
    }

    /// A file of the work tree, whole.
    fn read_repo(&self, name: &str) -> Result<Vec<u8>, Refusal> {
        std::fs::read(self.places.repo().join(name)).map_err(|_| Refusal::of(Why::Corrupt))
    }

    /// The index, decrypted.
    fn read_index(&self, ring: &Keyring) -> Result<Index, Refusal> {
        let plain = ring.open_bytes(&self.read_repo(INDEX_FILE)?)?;
        let index: Index = serde_json::from_slice(&plain).map_err(|_| Refusal::of(Why::Corrupt))?;
        if index.version > INDEX_VERSION {
            return Err(Refusal::of(Why::NotOurs));
        }
        Ok(index)
    }

    /// The session of a sync that waits: the plan, and for every conflicting
    /// text the repository's side and its diff against this machine's.
    fn write_session(&self, ring: &Keyring, index: &Index, plan: &mut Plan) -> Result<(), Refusal> {
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
            let Some(blob) = blobs.get(c.path.as_str()) else {
                continue;
            };
            let sealed = self.read_repo(&format!("{BLOBS_DIR}/{}.age", blob.id))?;
            let plain = ring.open_bytes(&sealed)?;
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
    fn diff(&self, here: &[u8], theirs: &Path) -> Option<Vec<u8>> {
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
            };
            if decision == Decision::KeepBoth && c.remote.is_some() {
                both.push(c.path.clone());
            }
            steps.push((c.path.clone(), step));
        }
        let pulls = steps
            .iter()
            .filter(|(_, s)| matches!(s, Step::Pull | Step::DeleteLocal))
            .count() as u64;
        let pushes = steps.iter().filter(|(_, s)| *s == Step::Push).count() as u64;

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
        let blobs_dir = self.places.repo().join(BLOBS_DIR);
        std::fs::create_dir_all(&blobs_dir).map_err(|_| Refusal::of(Why::Failed))?;
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
                    let Some(blob) = self.push(ring, path, &source, &blobs_dir)? else {
                        continue;
                    };
                    if let Some(old) = blobs.insert(path.clone(), blob) {
                        let _ = std::fs::remove_file(blobs_dir.join(format!("{}.age", old.id)));
                    }
                    counts.pushed += 1;
                }
                Step::DeleteRemote | Step::Unkeep => {
                    if let Some(old) = blobs.remove(path) {
                        let _ = std::fs::remove_file(blobs_dir.join(format!("{}.age", old.id)));
                    }
                    counts.deleted_there += 1;
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
        index.machines.insert(self.machine.clone(), self.now_s);
        let plain = serde_json::to_vec(&index).map_err(|_| Refusal::of(Why::Failed))?;
        std::fs::write(
            self.places.repo().join(INDEX_FILE),
            ring.seal_bytes(&plain)?,
        )
        .map_err(|_| Refusal::of(Why::Failed))?;

        beat(step(Phase::Pushing));
        if !self.commit_and_push(&ground.repo)? {
            return Ok(Attempt::Lost);
        }
        let commit =
            String::from_utf8_lossy(&self.git(&ground.repo, false, "rev-parse", &["HEAD"])?)
                .trim()
                .to_owned();

        // WHAT THE REPOSITORY HAD, HERE NOW — and what both keep, beside. A
        // path that could not be written keeps its OLD base below, so the
        // next sync pulls it again instead of reading this machine's old file
        // as a change to push over the other's.
        let mut unwritten: BTreeSet<String> = BTreeSet::new();
        let mut done = 0u64;
        for path in &both {
            let Some(blob) = theirs.get(path) else {
                continue;
            };
            let there = beside(path, &blob.machine);
            if let Some(dest) = self.places.resolve(&there) {
                match self.pull(ring, blob, &dest, &stamp, &there) {
                    Ok(()) => counts.pulled += 1,
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
                        Ok(()) => counts.pulled += 1,
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

        // THE NEW BASE: what the repository holds now, of what this machine
        // keeps — but the old one for what could not be written here.
        let old = base.map(|b| &b.entries);
        let mut entries: BTreeMap<String, Known> = index
            .entries
            .iter()
            .filter(|b| kept::kept_here(&b.path, self.prefs, under_a_root(&roots, &b.path)))
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
        let large = self
            .prefs
            .extra
            .iter()
            .map(|e| {
                (
                    e.clone(),
                    entries
                        .iter()
                        .filter(|(p, _)| crate::places::covers(e, p))
                        .map(|(_, k)| k.size_bytes)
                        .sum::<u64>(),
                )
            })
            .filter(|(_, bytes)| *bytes > kept::LARGE_BYTES)
            .collect();
        let last = Last {
            repository: self.prefs.repository.clone(),
            commit,
            at_s: self.now_s,
            entries,
            roots: crate::merge::followed(&roots, self.prefs),
        };
        let kept = u32::try_from(last.entries.len()).unwrap_or(u32::MAX);
        let text = serde_json::to_vec_pretty(&last).map_err(|_| Refusal::of(Why::Failed))?;
        write_private(&self.places.last(), &text).map_err(|_| Refusal::of(Why::Failed))?;
        rotate_replaced(&self.places.replaced());
        Ok(Attempt::Done(Outcome::Synced {
            pulled: counts.pulled,
            pushed: counts.pushed,
            deleted_here: counts.deleted_here,
            deleted_there: counts.deleted_there,
            kept,
            kept_bytes,
            large,
            unreadable: found.unreadable.iter().cloned().collect(),
            unwritten: unwritten.into_iter().collect(),
        }))
    }

    /// One commit of everything in the work tree, pushed fast-forward.
    /// `false` when another machine pushed first.
    fn commit_and_push(&self, repo: &Repo) -> Result<bool, Refusal> {
        self.git(repo, false, "add", &["-A"])?;
        let status = repo.status().map_err(|r| Refusal::of(why_of(r)))?;
        if status.entries.is_empty() {
            return Ok(true);
        }
        self.git(repo, false, "commit", &["-q", "-m", "belongings"])?;
        let before = self.git(
            repo,
            false,
            "for-each-ref",
            &["--format=%(objectname)", "refs/remotes/origin/main"],
        )?;
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
                repo.fetch(self.credentials.as_ref())
                    .map_err(|r| Refusal::of(why_of(r)))?;
                let after = self.git(
                    repo,
                    false,
                    "for-each-ref",
                    &["--format=%(objectname)", "refs/remotes/origin/main"],
                )?;
                if after != before {
                    Ok(false)
                } else {
                    Err(refusal)
                }
            }
        }
    }

    /// Writes what the repository has for `logical` at `dest`: what was there
    /// set aside first, the file through a draft born with its mode, a link
    /// as a link.
    fn pull(
        &self,
        ring: &Keyring,
        blob: &Blob,
        dest: &Path,
        stamp: &Path,
        logical: &str,
    ) -> Result<(), Refusal> {
        let failed = |_| Refusal::of(Why::Failed);
        let sealed = self
            .places
            .repo()
            .join(BLOBS_DIR)
            .join(format!("{}.age", blob.id));
        let mut from = std::fs::File::open(&sealed).map_err(|_| Refusal::of(Why::Corrupt))?;
        let private = blob.side.mode & 0o077 == 0;
        if let Some(parent) = dest.parent() {
            if private {
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(parent)
                    .map_err(failed)?;
            } else {
                std::fs::create_dir_all(parent).map_err(failed)?;
            }
        }
        self.set_aside(dest, stamp, logical)?;
        if blob.side.kind == Kind::Link {
            let mut target = Vec::new();
            ring.open_into(&mut from, &mut target)?;
            let target: std::ffi::OsString = std::os::unix::ffi::OsStringExt::from_vec(target);
            let target = PathBuf::from(target);
            let _ = std::fs::remove_file(dest);
            std::os::unix::fs::symlink(&target, dest).map_err(failed)?;
            return Ok(());
        }
        let draft = draft_of(dest);
        let _ = std::fs::remove_file(&draft);
        // BORN WITH ITS MODE: a private file is never readable, not even for
        // the time between a write and a `chmod`.
        let mut to = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(blob.side.mode & 0o7777 | 0o600)
            .open(&draft)
            .map_err(failed)?;
        let mut hashing = Hashing::new(&mut to);
        let opened = ring.open_into(&mut from, &mut hashing);
        let sha = hashing.finish();
        if let Err(r) = opened.and_then(|()| {
            if sha == blob.side.sha256 {
                Ok(())
            } else {
                Err(Refusal::of(Why::Corrupt))
            }
        }) {
            let _ = std::fs::remove_file(&draft);
            return Err(r);
        }
        to.sync_all().map_err(failed)?;
        drop(to);
        // THE EXACT MODE: the umask took bits the file had elsewhere.
        std::fs::set_permissions(
            &draft,
            std::fs::Permissions::from_mode(blob.side.mode & 0o7777),
        )
        .map_err(failed)?;
        std::fs::rename(&draft, dest).map_err(failed)
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

    /// Encrypts this machine's `logical` into a fresh blob; `None` when it is
    /// gone since the walk. Its "added" mark is the carrying's to set.
    fn push(
        &self,
        ring: &Keyring,
        logical: &str,
        source: &Path,
        blobs_dir: &Path,
    ) -> Result<Option<Blob>, Refusal> {
        let failed = |_| Refusal::of(Why::Failed);
        let Some(side) = walk::side_of(source).map_err(failed)? else {
            return Ok(None);
        };
        let id = random_id()?;
        let mut to = std::fs::File::create(blobs_dir.join(format!("{id}.age"))).map_err(failed)?;
        if side.kind == Kind::Link {
            let target = std::fs::read_link(source).map_err(failed)?;
            let bytes = std::os::unix::ffi::OsStrExt::as_bytes(target.as_os_str()).to_vec();
            ring.seal(&mut &bytes[..], &mut to)?;
        } else {
            let mut from = std::fs::File::open(source).map_err(failed)?;
            ring.seal(&mut from, &mut to)?;
        }
        to.sync_all().map_err(failed)?;
        Ok(Some(Blob {
            path: logical.to_owned(),
            id,
            side,
            machine: self.machine.clone(),
            added: false,
        }))
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

/// A writer that hashes what goes through it.
struct Hashing<'w> {
    to: &'w mut dyn std::io::Write,
    context: ring::digest::Context,
}

impl<'w> Hashing<'w> {
    fn new(to: &'w mut dyn std::io::Write) -> Self {
        Self {
            to,
            context: ring::digest::Context::new(&ring::digest::SHA256),
        }
    }

    fn finish(self) -> String {
        walk::hex(self.context.finish().as_ref())
    }
}

impl std::io::Write for Hashing<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.to.write(buf)?;
        self.context.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.to.flush()
    }
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
        Op::Abandon | Op::Keygen => Ok(0),
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
