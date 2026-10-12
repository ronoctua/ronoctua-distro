//! The installer's verbs (feature `crypt`): `restore` into a new home (pull
//! only, never push), `probe` an address and a credential, `init` an empty
//! repository, `verify-passphrase`.
//!
//! # The restore is a first sync that only takes
//!
//! The installer runs it AS the person (`setpriv`, their home and XDG roots
//! pointing at `/mnt/home/<user>`), with the places of that home
//! (`Places::for_home`), the passphrase in a `0600` file of its own runtime
//! folder and the access the person gave in `belongings-keys/`. It reads a
//! repository of this format (`Engine::open_repository`), writes every file the
//! table and the person's additions keep here — modes, links, the plain part
//! — and leaves `last.json` saying exactly that: the restore IS the base, so
//! the first sync of the new machine finds nothing to move. It never pushes:
//! a machine that does not exist yet has nothing to say to the others.
//!
//! # What the installer names
//!
//! A home being made has no settings yet, and the installer is not the
//! machine it installs: the verbs that take `--home` also take what the
//! installer knows ([`Named`]) — the repository (`--repository`, else
//! `belongings.json` of that home, which must then exist), the user an
//! HTTPS host asks for, and the machine's name (`--machine`, the hostname
//! of the new `host.nix`), which the index and `fleet.json` remember instead
//! of the live system's.
//!
//! # Probe, verify, init — the installer's questions
//!
//! `probe` answers "can this access read AND write the repository" with no
//! settings and no passphrase: `ls-remote`, then a push of an empty commit
//! with `--dry-run` from a scratch repository (the remote's own check of the
//! credential; nothing is sent). `verify-passphrase` opens `key.age` and
//! touches nothing. `init` is the first sync of an EMPTY repository (the
//! identity made with the passphrase typed twice); a repository somebody
//! filled meanwhile is only verified. Both leave the passphrase's file for
//! the restore when it opens, and delete it only when a refusal says it is
//! no good ([`tells_of_the_passphrase`]): a network that failed, an access
//! missing or a newer repository says nothing about it, and the installer
//! would ask again for no reason it could give. The restore deletes it
//! however it ends. Every network call holds a pass for the host, as a
//! sync's.
//!
//! # The key the installer pins
//!
//! A restore and an `init` that made the repository both open (or make)
//! `key.age` with the person's passphrase: each leaves the signing key's
//! public half in its runtime folder ([`SIGNER_OUT`], `0600`), and the
//! installer pins THAT key on the new machine (`ronoctua-person pin`) —
//! never the repository's copy of it, which anybody who can write the
//! repository could have replaced.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use age::secrecy::SecretString;
use ronoctua_git::{Credentials, Repo, Tools};
use serde::{Deserialize, Serialize};

use super::{
    AUTHOR_EMAIL, AUTHOR_NAME, BRANCH, Done, Engine, Ground, KEY_FILE, forget_secrets, read_prefs,
    ready, stamp_of, step, write_status,
};
use crate::crypt::Keyring;
use crate::kept;
use crate::merge::{roots_after, under_a_root};
use crate::order::{Beat, Phase};
use crate::places::Places;
use crate::prefs::{Auth, Belongings, repository_is_plain};
use crate::running;
use crate::secret::make_private_dir;
use crate::status::{Known, Last, Outcome};
use crate::{FORMAT_VERSION, Refusal, Why};

/// The branch a probe's dry run names: never written.
pub const PROBE_BRANCH: &str = "ronoctua-belongings-probe";

/// The file, in the runtime folder of an installer's restore or `init`,
/// where the signing key the passphrase opened is left for the installer to
/// pin.
pub const SIGNER_OUT: &str = crate::plain::SIGNER_FILE;

/// What `init` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Initialized {
    /// The repository was empty: the first sync made it, and this is how it
    /// ended.
    Made(Done),
    /// Another machine had made it: the passphrase opens it, of this format,
    /// and nothing was pushed.
    Verified {
        /// The repository's format.
        format: u32,
    },
}

/// What a probe found — `probe --json` prints it, one line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct Probed {
    /// The access reads the repository.
    pub read: bool,
    /// The access may push to it.
    pub write: bool,
    /// It has nothing yet: the installer asks the passphrase twice and
    /// runs `init`.
    pub empty: bool,
    /// Why one of the two failed.
    pub why: Option<Why>,
}

/// What the installer names on the command line for a home that is not this
/// process's — see the module's header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Named {
    /// The repository (`--repository`); `None`: `belongings.json` of that
    /// home says it.
    pub repository: Option<String>,
    /// The user name an HTTPS host asks for (`--user`).
    pub user: Option<String>,
    /// The machine the home is for (`--machine`); `None`: this process's.
    pub machine: Option<String>,
}

impl Named {
    /// The settings a run of these takes: `belongings.json` of `places`,
    /// unless the repository is named — then a fresh machine's, its access
    /// the address's kind (HTTPS: the token; else the deploy key, both in
    /// that home's `belongings-keys/`).
    #[must_use]
    pub fn prefs(&self, places: &Places) -> Belongings {
        let Some(repository) = self.repository.as_deref() else {
            return read_prefs(places);
        };
        let mut prefs = Belongings::with_repository(repository);
        prefs.auth = if prefs.repository.starts_with("https://") {
            Auth::Token
        } else {
            Auth::Ssh
        };
        prefs.user = self.user.clone().filter(|u| !u.is_empty());
        prefs
    }

    /// `engine`, for the machine named.
    fn name(&self, engine: &mut Engine<'_>) {
        if let Some(machine) = &self.machine {
            engine.machine.clone_from(machine);
        }
    }
}

/// Whether a refusal says the passphrase is no good — it does not open
/// `key.age`, its file is not one a door may have written (missing, a link,
/// readable by others), or the repository is not a belongings one at all —
/// and so its file goes, and the installer asks again. Pure.
#[must_use]
pub const fn tells_of_the_passphrase(why: Why) -> bool {
    matches!(why, Why::WrongPassphrase | Why::NoPassphrase | Why::NotOurs)
}

impl Engine<'_> {
    /// The restore into this engine's places — see the module's header.
    ///
    /// # Errors
    ///
    /// [`Why::Busy`], [`Why::NoPassphrase`], [`Why::Empty`],
    /// [`Why::WrongPassphrase`], [`Why::Newer`], [`Why::Corrupt`], and the
    /// network's. The passphrase's file is deleted however it ends — but
    /// for [`Why::Busy`].
    pub fn restore(&self, beat: &mut dyn FnMut(Beat)) -> Result<Outcome, Refusal> {
        let _held = running::hold(self.places)?;
        let done = self.passphrase().and_then(|p| self.restored(&p, beat));
        forget_secrets(self.places);
        if done.is_ok() {
            self.leave_signer();
        }
        let outcome = match &done {
            Ok(o) => o.clone(),
            Err(r) => Outcome::Refused { why: r.why },
        };
        write_status(self.places, self.now_s, outcome);
        done
    }

    /// Fetch, open (format 2), write what is kept here, and the base.
    fn restored(
        &self,
        passphrase: &SecretString,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<Outcome, Refusal> {
        beat(step(Phase::Fetching));
        let ground = self.ground()?;
        let Some(head) = ground.head.clone() else {
            return Err(Refusal::of(Why::Empty));
        };
        self.checkout(&ground.repo, &head)?;
        beat(step(Phase::Unlocking));
        let (ring, index) = self.open_repository(passphrase)?;
        self.remember_recipient(&ring);
        let roots = roots_after(None, &index, self.prefs);
        let stamp = self.places.replaced().join(stamp_of(self.now_s));
        // A TARGET NO MACHINE KEEPS any more is not brought to a new one.
        let dropped: BTreeSet<String> = index.dropped.keys().cloned().collect();
        let wanted: Vec<_> = index
            .entries
            .iter()
            .filter(|b| kept::kept_here(&b.path, self.prefs, under_a_root(&roots, &b.path)))
            .filter(|b| !kept::dropped_everywhere(&b.path, &dropped))
            .collect();
        let total = wanted.len() as u64;
        let mut pulled = 0u32;
        let mut unwritten: BTreeSet<String> = BTreeSet::new();
        // WHAT THE RESTORE BROUGHT: a private target's folders, the offers.
        let mut brought: Vec<String> = Vec::new();
        for (n, blob) in wanted.iter().enumerate() {
            beat(Beat::Step {
                phase: Phase::Pulling,
                done: n as u64,
                total,
            });
            let Some(dest) = self.places.resolve(&blob.path) else {
                continue;
            };
            // WHAT IS HERE ALREADY, ALIKE, stays as it is — its mode read as
            // the walk reads it (`kept::kept_mode`).
            let here = crate::walk::side_of(&dest).ok().flatten().map(|mut h| {
                if h.kind == crate::plan::Kind::File {
                    h.mode = kept::kept_mode(&blob.path, h.mode);
                }
                h
            });
            if here.is_some_and(|h| h.same_as(&blob.side)) {
                continue;
            }
            match self.pull(&ring, blob, &dest, &stamp, &blob.path) {
                Ok(()) => {
                    pulled += 1;
                    brought.push(blob.path.clone());
                }
                Err(_) => {
                    unwritten.insert(blob.path.clone());
                }
            }
        }
        self.restore_dirs(&brought, &index.dirs);
        // THE PLAIN PART (format 2's): every plain file of the repository
        // the person's signature vouches for — under the key their
        // passphrase just opened — here, its base what both now have; what
        // it does not vouch for is held for the first sync to ask.
        let mut merges = super::merging::Merges::default();
        let plain = self.plan_plain(self.plain_base(), &mut merges, None, Some(&ring))?;
        let landing = self.carry_plain(
            &plain,
            &crate::plan::Decisions::new(),
            &merges,
            false,
            Some(&ring),
        )?;
        let landed = self.land_plain(&plain, &landing, &merges, None);
        self.settle_signed(&plain, &landing);
        pulled += u32::try_from(landed.changed.len()).unwrap_or(u32::MAX);
        unwritten.extend(landed.unwritten.iter().cloned());
        unwritten.extend(landed.moved.iter().cloned());
        let plain_base = self.plain_base_after(&plain, &landing, &landed)?;

        let entries: BTreeMap<String, Known> = wanted
            .iter()
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
        let kept_bytes = entries.values().map(|k| k.size_bytes).sum();
        let mut last = Last {
            repository: self.prefs.repository.clone(),
            commit: head,
            at_s: self.now_s,
            entries,
            roots: crate::merge::followed(&roots, self.prefs),
            plain: plain_base,
            dropped,
            modified_s: BTreeMap::new(),
        };
        // WHEN EACH FILE HERE LAST CHANGED, as the restore leaves it alike
        // with its base (`inventory::stamp_sealed`).
        let written: BTreeSet<String> = brought.iter().chain(&landed.changed).cloned().collect();
        crate::inventory::stamp_sealed(self.places, &mut last, self.now_s, &written);
        crate::inventory::stamp_plain(self.places, &mut last, self.now_s, &written);
        let text = serde_json::to_vec_pretty(&last).map_err(|_| Refusal::of(Why::Failed))?;
        crate::secret::write_private(&self.places.last(), &text)
            .map_err(|_| Refusal::of(Why::Failed))?;
        self.keep_bases(&ring, &last);
        self.write_fleet(&index);
        Ok(Outcome::Synced {
            pulled,
            pushed: 0,
            deleted_here: 0,
            deleted_there: 0,
            kept: u32::try_from(last.entries.len()).unwrap_or(u32::MAX),
            kept_bytes,
            large: Vec::new(),
            unreadable: Vec::new(),
            unwritten: unwritten.into_iter().collect(),
            // WHAT THE RESTORE BROUGHT OFFERS, for the first session of the
            // new machine to draw ("no" first).
            offers: self.offers_after(&brought),
            repaired: Vec::new(),
            budget: None,
        })
    }

    /// ONE file of the sealed part at the repository's head, written at `to`
    /// `0600` (see [`take_named`]): only a FILE (never a link), checked
    /// against the SHA-256 the index names for it; `false` when the index
    /// names no `logical`. The passphrase's file is deleted however it
    /// ends — but for [`Why::Busy`].
    ///
    /// # Errors
    ///
    /// [`Why::Busy`], [`Why::NoPassphrase`], [`Why::Empty`],
    /// [`Why::WrongPassphrase`], [`Why::Newer`], [`Why::Corrupt`] (a link,
    /// or bytes that are not the index's), [`Why::Failed`] (not written),
    /// and the network's.
    pub fn take(
        &self,
        logical: &str,
        to: &Path,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<bool, Refusal> {
        let _held = running::hold(self.places)?;
        let done = self.passphrase().and_then(|p| {
            beat(step(Phase::Fetching));
            let ground = self.ground()?;
            let Some(head) = ground.head.clone() else {
                return Err(Refusal::of(Why::Empty));
            };
            self.checkout(&ground.repo, &head)?;
            beat(step(Phase::Unlocking));
            let (ring, index) = self.open_repository(&p)?;
            let Some(blob) = index.entries.iter().find(|b| b.path == logical) else {
                return Ok(false);
            };
            if blob.side.kind != crate::plan::Kind::File {
                return Err(Refusal::of(Why::Corrupt));
            }
            let bytes = zeroize::Zeroizing::new(self.read_blob(&ring, blob)?);
            if super::merging::sha256(&bytes) != blob.side.sha256 {
                return Err(Refusal::of(Why::Corrupt));
            }
            crate::secret::write_private(to, &bytes).map_err(|_| Refusal::of(Why::Failed))?;
            Ok(true)
        });
        forget_secrets(self.places);
        done
    }

    /// Whether the passphrase opens the repository's `key.age`, touching
    /// nothing — the repository's format when it does. Kept for the
    /// restore when it opens, and through every refusal that says nothing
    /// about it; deleted when one does ([`tells_of_the_passphrase`]).
    ///
    /// # Errors
    ///
    /// [`Why::NoPassphrase`], [`Why::Empty`], [`Why::WrongPassphrase`],
    /// [`Why::NotOurs`], [`Why::Newer`], and the network's.
    pub fn verify_passphrase(&self, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
        let _held = running::hold(self.places)?;
        let done = self.passphrase().and_then(|p| {
            beat(step(Phase::Fetching));
            let ground = self.ground()?;
            self.opens(&ground, &p, beat)
        });
        if let Err(r) = &done
            && tells_of_the_passphrase(r.why)
        {
            let _ = std::fs::remove_file(self.places.passphrase());
        }
        done
    }

    /// Whether `passphrase` opens `key.age` at the repository's head that
    /// `ground` fetched: the format.
    fn opens(
        &self,
        ground: &Ground,
        passphrase: &SecretString,
        beat: &mut dyn FnMut(Beat),
    ) -> Result<u32, Refusal> {
        let Some(head) = ground.head.clone() else {
            return Err(Refusal::of(Why::Empty));
        };
        self.checkout(&ground.repo, &head)?;
        let format = self.format()?;
        if format > FORMAT_VERSION {
            return Err(Refusal::of(Why::Newer));
        }
        beat(step(Phase::Unlocking));
        Keyring::open(&self.read_repo(KEY_FILE)?, passphrase)?;
        Ok(format)
    }

    /// The first sync of an EMPTY repository: the identity, `key.age` with
    /// the passphrase, and what this machine keeps, pushed. A repository
    /// that is not empty (another machine made it meanwhile) is only
    /// verified: nothing is pushed. Either way the passphrase's file stays
    /// for the restore that follows, and it goes only with a refusal that
    /// says it is no good. One lock from the first step to the last.
    ///
    /// # Errors
    ///
    /// As [`Engine::prepare`] for an empty repository, as
    /// [`Engine::verify_passphrase`] for another.
    pub fn init(&self, beat: &mut dyn FnMut(Beat)) -> Result<Initialized, Refusal> {
        let _held = running::hold(self.places)?;
        let done = self.passphrase().and_then(|p| {
            beat(step(Phase::Fetching));
            let ground = self.ground()?;
            if ground.head.is_some() {
                return self
                    .opens(&ground, &p, beat)
                    .map(|format| Initialized::Verified { format });
            }
            // THE FIRST SYNC, whole: a new plan, new decisions.
            let _ = std::fs::remove_dir_all(self.places.session());
            let _ = std::fs::remove_file(self.places.decisions());
            self.rounds(&super::Opening::Typed(p), None, beat)
                .map(Initialized::Made)
        });
        match &done {
            Ok(Initialized::Made(Done::Synced(outcome))) => {
                self.leave_signer();
                write_status(self.places, self.now_s, outcome.clone());
            }
            Ok(Initialized::Made(Done::Waiting(plan))) => write_status(
                self.places,
                self.now_s,
                Outcome::Waiting {
                    conflicts: u32::try_from(plan.conflicts.len()).unwrap_or(u32::MAX),
                },
            ),
            Ok(Initialized::Verified { .. }) => {}
            Err(r) => {
                if tells_of_the_passphrase(r.why) {
                    let _ = std::fs::remove_file(self.places.passphrase());
                }
                write_status(self.places, self.now_s, Outcome::Refused { why: r.why });
            }
        }
        done
    }

    /// Leaves the signing key the passphrase opened, as the run recorded it
    /// (`last.json`'s plain section), in the runtime folder for the
    /// installer to pin ([`SIGNER_OUT`]). Best effort: a key not left is a
    /// machine the person pins later, through the sudo door.
    fn leave_signer(&self) {
        let Some(key) = Last::read(&self.places.last())
            .map(|l| l.plain.signer)
            .filter(|k| !k.is_empty())
        else {
            return;
        };
        let out = self.places.runtime.join(SIGNER_OUT);
        let _ = std::fs::remove_file(&out);
        let _ = crate::secret::write_private(&out, format!("{key}\n").as_bytes());
    }

    /// Whether this engine's access reads and writes its repository — see
    /// the module's header. A scratch repository under the runtime folder
    /// is made and removed.
    ///
    /// # Errors
    ///
    /// [`Why::Failed`] when the scratch repository could not be made,
    /// [`Why::NoTools`]. What the remote says is the answer, not a refusal.
    pub fn probe(&self) -> Result<Probed, Refusal> {
        let scratch = self.places.probing();
        let _ = std::fs::remove_dir_all(&scratch);
        let probed = self.probed(&scratch);
        let _ = std::fs::remove_dir_all(&scratch);
        probed
    }

    /// The probe, in `scratch`.
    fn probed(&self, scratch: &Path) -> Result<Probed, Refusal> {
        make_private_dir(scratch).map_err(|_| Refusal::of(Why::Failed))?;
        let mut init = ronoctua_git::base(&self.tools, scratch, None)
            .map_err(|r| Refusal::of(super::why_of(r)))?;
        ronoctua_git::subcommand(&mut init, "init").args(["-q", "-b", BRANCH]);
        let ran = init.output().map_err(|_| Refusal::of(Why::NoTools))?;
        if !ran.status.success() {
            return Err(Refusal::of(Why::Failed));
        }
        let repo = Repo::discover(scratch)
            .ok_or(Refusal::of(Why::Failed))?
            .with_tools(self.tools.clone());
        for (key, value) in [
            ("user.name", AUTHOR_NAME),
            ("user.email", AUTHOR_EMAIL),
            ("commit.gpgsign", "false"),
        ] {
            self.git(&repo, false, "config", &[key, value])?;
        }
        let url = self.prefs.repository.trim();
        self.git(&repo, false, "remote", &["add", "origin", url])?;
        let mut probed = Probed::default();
        match self.git(&repo, true, "ls-remote", &["--heads", "origin"]) {
            Ok(heads) => {
                probed.read = true;
                let main = format!("refs/heads/{BRANCH}");
                probed.empty = !String::from_utf8_lossy(&heads)
                    .lines()
                    .any(|l| l.split_whitespace().nth(1) == Some(main.as_str()));
            }
            Err(r) => {
                probed.why = Some(r.why);
                return Ok(probed);
            }
        }
        self.git(
            &repo,
            false,
            "commit",
            &["-q", "--allow-empty", "-m", super::COMMIT_MESSAGE],
        )?;
        let target = format!("HEAD:refs/heads/{PROBE_BRANCH}");
        match self.git(
            &repo,
            true,
            "push",
            &[
                "--dry-run",
                "-q",
                "--receive-pack=git-receive-pack",
                "origin",
                target.as_str(),
            ],
        ) {
            Ok(_) => probed.write = true,
            // IT READ, SO IT WAS REACHED: a push the remote turns away is the
            // access refused (GitHub says a read-only key "could not read").
            Err(r) => {
                probed.why = Some(match r.why {
                    Why::Unreachable | Why::Failed | Why::NeedsAccess => Why::Refused,
                    other => other,
                });
            }
        }
        Ok(probed)
    }
}

/// The engine of one of the installer's questions (`verify-passphrase`,
/// `init`): refused before it starts with NOTHING deleted and nothing
/// written — a missing access or tool says nothing about the passphrase.
fn asking<'a>(places: &'a Places, prefs: &'a Belongings) -> Result<Engine<'a>, Refusal> {
    if !prefs.has_repository() || repository_is_plain(&prefs.repository).is_err() {
        return Err(Refusal::of(Why::NoRepository));
    }
    Engine::for_this_machine(places, prefs)
}

/// `restore`, as the queue orders it.
///
/// # Errors
///
/// As [`restore_named`].
pub(super) fn restore(places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    restore_named(places, &Named::default(), beat)
}

/// `restore --home --runtime`: into the home `places` are of, with what
/// the installer `named`.
///
/// # Errors
///
/// What [`Engine::restore`] refuses, and what refuses a run before it
/// starts (said in `status.json`, the passphrase deleted).
pub fn restore_named(
    places: &Places,
    named: &Named,
    beat: &mut dyn FnMut(Beat),
) -> Result<u32, Refusal> {
    let prefs = named.prefs(places);
    let mut engine = ready(places, &prefs)?;
    named.name(&mut engine);
    engine.restore(beat).map(|_| 0)
}

/// `probe`, as the queue orders it: this machine's settings and access.
/// What it found is the CLI's to print (`probe --json`); here a probe that
/// cannot read or write is the refusal it met.
///
/// # Errors
///
/// What [`Engine::probe`] refuses, and the remote's answer when it did not
/// both read and write.
pub(super) fn probe(places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    let _ = beat;
    let prefs = read_prefs(places);
    let engine = ready(places, &prefs)?;
    let probed = engine.probe()?;
    match probed.why {
        Some(why) if !(probed.read && probed.write) => Err(Refusal::of(why)),
        _ => Ok(0),
    }
}

/// The format of the repository the installer `named` (or `places`'
/// settings name), when the passphrase opens it — `verify-passphrase` on
/// the command line, which prints it.
///
/// # Errors
///
/// What [`Engine::verify_passphrase`] refuses, and what refuses it before
/// it starts (nothing deleted, nothing written).
pub fn format_opened(
    places: &Places,
    named: &Named,
    beat: &mut dyn FnMut(Beat),
) -> Result<u32, Refusal> {
    let prefs = named.prefs(places);
    asking(places, &prefs)?.verify_passphrase(beat)
}

/// `take --home --runtime --path <logical> --out <file>`: ONE file of the
/// sealed part of the repository the installer `named` (or `places`'
/// settings name), opened with the passphrase and written at `to`, `0600`
/// from its first byte — `false` when the repository keeps no such file.
///
/// What the installer needs BEFORE `nixos-install` that the restore only
/// brings after it: the person's push token, when their binary cache is
/// private — `nixos-install` reads it through a netrc of the installer's,
/// and the installed machine gets root's (`ronoctua-installer`'s plan).
///
/// # Errors
///
/// What [`Engine::take`] refuses, and what refuses it before it starts
/// (nothing deleted, nothing written).
pub fn take_named(
    places: &Places,
    named: &Named,
    logical: &str,
    to: &Path,
    beat: &mut dyn FnMut(Beat),
) -> Result<bool, Refusal> {
    let prefs = named.prefs(places);
    asking(places, &prefs)?.take(logical, to, beat)
}

/// `init`, as the queue orders it.
///
/// # Errors
///
/// As [`init_named`].
pub(super) fn init(places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    init_named(places, &Named::default(), beat).map(|done| match done {
        Initialized::Made(done) => super::conflicts_of(&done),
        Initialized::Verified { .. } => 0,
    })
}

/// `init --home --runtime`: the first sync of an empty repository from the
/// home `places` are of, with what the installer `named` — the machine's
/// name is what the index remembers.
///
/// # Errors
///
/// What [`Engine::init`] refuses, and what refuses it before it starts
/// (nothing deleted, nothing written).
pub fn init_named(
    places: &Places,
    named: &Named,
    beat: &mut dyn FnMut(Beat),
) -> Result<Initialized, Refusal> {
    let prefs = named.prefs(places);
    let mut engine = asking(places, &prefs)?;
    named.name(&mut engine);
    engine.init(beat)
}

/// `verify-passphrase`, as the queue orders it.
///
/// # Errors
///
/// What [`Engine::verify_passphrase`] refuses, and what refuses it before
/// it starts (nothing deleted, nothing written).
pub(super) fn verify_passphrase(
    places: &Places,
    beat: &mut dyn FnMut(Beat),
) -> Result<u32, Refusal> {
    let prefs = read_prefs(places);
    asking(places, &prefs)?.verify_passphrase(beat).map(|_| 0)
}

/// `probe --address <url> --credential <file>`: whether this access reads
/// and writes the repository, with no settings — the installer's question,
/// asked before the person's home exists. The access is the address's kind
/// ([`probe_access`]). The scratch repository lives in `places`' runtime
/// folder.
///
/// # Errors
///
/// [`Why::NoRepository`] for an address that is not plain, [`Why::NoTools`],
/// [`Why::NeedsAccess`] without the credential's file, [`Why::Failed`].
pub fn probe_address(
    places: &Places,
    address: &str,
    credential: &Path,
    user: Option<&str>,
    known_hosts: Option<&Path>,
) -> Result<Probed, Refusal> {
    let address = address.trim();
    if repository_is_plain(address).is_err() {
        return Err(Refusal::of(Why::NoRepository));
    }
    let tools = Tools::from_env().ok_or(Refusal::of(Why::NoTools))?;
    probe_with(places, address, credential, user, known_hosts, tools)
}

/// [`probe_address`] for an address already checked, with these tools.
pub(super) fn probe_with(
    places: &Places,
    address: &str,
    credential: &Path,
    user: Option<&str>,
    known_hosts: Option<&Path>,
    tools: Tools,
) -> Result<Probed, Refusal> {
    if !credential.is_file() {
        return Err(Refusal::of(Why::NeedsAccess));
    }
    let (prefs, credentials) = probe_access(address, credential, user, known_hosts);
    let engine = Engine {
        credentials: Some(credentials),
        ..Engine::for_plain(places, &prefs, tools)
    };
    engine.probe()
}

/// The settings and the access a probe of `address` takes: HTTPS reaches it
/// with the token in `credential` (answering `user` when the host asks),
/// anything else with the deploy key in `credential` — its known hosts
/// `known_hosts` when named, else the file beside it, as `belongings-keys/`
/// lays them out. Post-quantum only, as every belongings session. Pure.
#[must_use]
pub fn probe_access(
    address: &str,
    credential: &Path,
    user: Option<&str>,
    known_hosts: Option<&Path>,
) -> (Belongings, Credentials) {
    let mut prefs = Belongings::with_repository(address);
    prefs.user = user.filter(|u| !u.is_empty()).map(str::to_owned);
    let credentials = if prefs.repository.starts_with("https://") {
        prefs.auth = Auth::Token;
        Credentials::Token {
            file: credential.to_path_buf(),
            user: prefs.user.clone(),
            pq_only: true,
        }
    } else {
        prefs.auth = Auth::Ssh;
        let beside = credential.with_file_name(crate::places::KNOWN_HOSTS);
        Credentials::Ssh {
            key: credential.to_path_buf(),
            known_hosts: known_hosts.map_or(beside, Path::to_path_buf),
            pq_only: true,
        }
    };
    (prefs, credentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_probe_crosses_one_line_whole() {
        let probed = Probed {
            read: true,
            write: false,
            empty: true,
            why: Some(Why::Refused),
        };
        let line = serde_json::to_string(&probed).unwrap();
        assert_eq!(
            line,
            r#"{"read":true,"write":false,"empty":true,"why":"refused"}"#
        );
        assert_eq!(serde_json::from_str::<Probed>(&line).unwrap(), probed);
    }

    /// FAILS IF: a probe reaches the repository with another access than
    /// the address's kind, looks for the deploy key's known hosts anywhere
    /// but where it is told or beside the key, or loses the post-quantum
    /// narrowing.
    #[test]
    fn a_probe_takes_the_access_of_the_address_kind() {
        let key = Path::new("/run/installer/belongings-keys/deploy");
        let (prefs, ssh) = probe_access("git@example.org:a/b.git", key, Some("me"), None);
        assert_eq!(prefs.auth, Auth::Ssh);
        assert_eq!(prefs.repository, "git@example.org:a/b.git");
        assert_eq!(
            ssh,
            Credentials::Ssh {
                key: key.to_path_buf(),
                known_hosts: Path::new("/run/installer/belongings-keys/known_hosts").to_path_buf(),
                pq_only: true,
            }
        );
        let named = Path::new("/run/installer/hosts");
        let (_, ssh) = probe_access("ssh://git@example.org/a/b.git", key, None, Some(named));
        assert!(
            matches!(&ssh, Credentials::Ssh { known_hosts, .. } if known_hosts == named),
            "{ssh:?}"
        );
        let token = Path::new("/run/installer/belongings-keys/token");
        let (prefs, https) = probe_access(" https://example.org/a/b.git ", token, Some("me"), None);
        assert_eq!(prefs.auth, Auth::Token);
        assert_eq!(
            https,
            Credentials::Token {
                file: token.to_path_buf(),
                user: Some("me".to_owned()),
                pq_only: true,
            }
        );
    }

    /// FAILS IF: what the installer names does not reach the run — the
    /// repository and its access's kind, the user, the machine — or a home
    /// with no named repository stops reading its own `belongings.json`.
    #[test]
    fn what_the_installer_names_is_what_the_run_takes() {
        let root = std::env::temp_dir().join(format!("ronoctua-named-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let places = Places::for_home(&root.join("home"), &root.join("run"));
        std::fs::create_dir_all(&places.config).unwrap();
        std::fs::write(
            places.prefs(),
            Belongings::with_repository("git@example.org:from/file.git").to_file(),
        )
        .unwrap();
        assert_eq!(
            Named::default().prefs(&places).repository,
            "git@example.org:from/file.git"
        );
        let named = Named {
            repository: Some("https://example.org/a/b.git".to_owned()),
            user: Some("me".to_owned()),
            machine: Some("new-laptop".to_owned()),
        };
        let prefs = named.prefs(&places);
        assert_eq!(prefs.repository, "https://example.org/a/b.git");
        assert_eq!(prefs.auth, Auth::Token);
        assert_eq!(prefs.user.as_deref(), Some("me"));
        let tools = Tools {
            git: "/nonexistent/git".into(),
            ssh: None,
            askpass: None,
            pq_tls: None,
        };
        let mut engine = Engine::for_plain(&places, &prefs, tools);
        named.name(&mut engine);
        assert_eq!(engine.machine, "new-laptop");
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: a refusal that says nothing about the passphrase — the
    /// network, the access, a newer repository — deletes it, or one that
    /// says it is no good keeps it.
    #[test]
    fn only_a_refusal_about_the_passphrase_takes_it() {
        for why in [Why::WrongPassphrase, Why::NoPassphrase, Why::NotOurs] {
            assert!(tells_of_the_passphrase(why), "{why:?}");
        }
        for why in [
            Why::Unreachable,
            Why::NotLet,
            Why::NotPostQuantum,
            Why::NeedsAccess,
            Why::Refused,
            Why::NoTools,
            Why::Newer,
            Why::Empty,
            Why::Busy,
        ] {
            assert!(!tells_of_the_passphrase(why), "{why:?}");
        }
    }
}

// ── anchor: L09 ──
