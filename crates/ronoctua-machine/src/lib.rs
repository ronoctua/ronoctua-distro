//! The machine's own flake and its life — the one crate that runs `bootctl`,
//! reads the EFI variables and writes the machine directory.
//!
//! # The machine directory
//!
//! Each installed machine has its own flake (`flake.nix`, `flake.lock`,
//! `host.nix`, `facter.json`, `person/`, `local/`) with the distro as an
//! input that follows the `release` branch. Its path reaches a program by
//! `RONOCTUA_MACHINE_DIR` (one name: `machine-dir-has-one-name`), and
//! [`Places::dir`] carries it from there.
//!
//! # The verbs ([`Verb`], the CLI's and the units')
//!
//! - `migrate`: old layout (a copy of the distro) → the machine's own flake,
//!   only when the new one builds; the old goes to `legacy-<stamp>`.
//! - `apply [--rehearse]`: build → switch → arm the boot trial; a failed
//!   build puts back the lock the running system was built from, so the
//!   directory always describes the running system. The build is the
//!   distro's units, each by its class, then the system, sized to the
//!   machine, and keeps the compiled units it was built from for the next
//!   one ([`build`]).
//! - `bless`: after a good boot, the trial becomes the last good generation;
//!   after a bad one, back to the last good.
//! - `disarm`: the profile was rolled back by hand (the rescue console); the
//!   trial it left is not tried again, and the EFI variables are unset.
//! - `hold <rev>` / `unhold`.
//!
//! # Who runs what
//!
//! Every program this crate starts is named by its store path, through the
//! environment the units give it ([`System::from_env`]): `nix` (and the
//! `nix-env` beside it), `bootctl`, the template of a machine flake, the
//! build sizing, what builds the units and what keeps them. Nothing is looked up on a
//! `PATH`. The words handed to each are built by the pure functions of
//! [`argv`].
//!
//! # The boot trial
//!
//! NixOS 26.05 has no boot counting, so the way back is ours. After a switch
//! — unless it returned 0 and the kernel, initrd and kernel parameters are
//! the last good generation's — `bootctl set-default` names the LAST GOOD
//! entry and `bootctl set-oneshot` the NEW one: the next boot tries the new
//! system once, and any boot after that — a panic (`panic=10`), a stage 1
//! that failed (`boot.panic_on_fail`), a stage 2 that does not reach the
//! default target in time (`ronoctua-boot-trial-deadline.service`), a hang
//! the hardware watchdog ends — comes back to the last good one. `bless`
//! (`ronoctua-boot-bless.service`, a minute after the system's default
//! target) reads which of the two the first boot after the arming one booted
//! ([`boots`]).

#![forbid(unsafe_code)]

pub mod aim;
mod apply;
pub mod argv;
mod bless;
pub mod boot;
pub mod boots;
pub mod build;
mod disarm;
mod disk;
pub mod dry;
mod hold;
mod loader;
mod migrate;
mod run;
mod system;
#[cfg(test)]
mod tests;
mod trial;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub use ronoctua_person::state::{
    ApplyResult, Hold, LastApply, LastGood, MachineState, NoTrial, Trial, TrialOutcome,
};
pub use system::{Env, System};

/// Where the machine's record lives (`state.json`, `legacy-<stamp>`, the
/// lock snapshots), when `RONOCTUA_MACHINE_STATE` does not say otherwise.
pub const STATE_DIR: &str = "/var/lib/ronoctua-machine";

/// Where the built system's out-link and a rehearsal's copy live — the
/// update's own folder under `/run`, when `RONOCTUA_MACHINE_RUN` does not say
/// otherwise.
pub const RUN_DIR: &str = "/run/ronoctua-update";

/// How long the bless waits for root's lock of the person's data: an update
/// holds it for its whole root half, builds included. The judgement can
/// wait; what booted was written down before ([`boots`]).
pub const BLESS_LOCK_WAIT_S: u64 = 4 * 60 * 60;

/// The unit that blesses (or rolls back) the boot trial after a boot.
pub const BLESS_UNIT: &str = "ronoctua-boot-bless.service";

/// The unit that applies the machine directory without moving its inputs —
/// what the software centre starts after a declaration, through one narrow
/// polkit rule.
pub const UPDATE_APPLY_UNIT: &str = "ronoctua-update-apply.service";

/// The input of the machine flake that is the distro.
pub const DISTRO_INPUT: &str = "ronoctua";

/// The lock the running system was built from (a copy of `flake.lock`,
/// under [`Places::state`]).
pub const APPLIED_LOCK: &str = "applied.lock";

/// The lock the last good generation was built from.
pub const LAST_GOOD_LOCK: &str = "last-good.lock";

/// Why a verb did not do what it was asked, as a closed set
/// (`machine-refused-<id>` on screen).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// It runs as root only.
    NotRoot,
    /// Another apply or migration holds the update's lock.
    Busy,
    /// There is no old layout to migrate.
    NothingToMigrate,
    /// The new system did not build; nothing changed.
    BuildFailed,
    /// The switch failed.
    SwitchFailed,
    /// The system built is the one whose boot trial failed (or that was
    /// rolled away from): it is not tried again before the distro moves on.
    DidNotBoot,
    /// The last good generation would fall out of the boot menu.
    LastGoodWouldFallOut,
    /// The machine is held at a revision, and the lock names another.
    Held,
    /// A file of the machine directory does not read.
    Unreadable,
    /// A program failed.
    Failed,
}

impl Why {
    /// Every one.
    pub const ALL: [Self; 10] = [
        Self::NotRoot,
        Self::Busy,
        Self::NothingToMigrate,
        Self::BuildFailed,
        Self::SwitchFailed,
        Self::DidNotBoot,
        Self::LastGoodWouldFallOut,
        Self::Held,
        Self::Unreadable,
        Self::Failed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NotRoot => "not-root",
            Self::Busy => "busy",
            Self::NothingToMigrate => "nothing-to-migrate",
            Self::BuildFailed => "build-failed",
            Self::SwitchFailed => "switch-failed",
            Self::DidNotBoot => "did-not-boot",
            Self::LastGoodWouldFallOut => "last-good-would-fall-out",
            Self::Held => "held",
            Self::Unreadable => "unreadable",
            Self::Failed => "failed",
        }
    }

    /// The one whose [`Why::id`] is `id`.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

impl Refusal {
    /// The refusal of `why`.
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

/// What the CLI is asked to do — a closed set, read back by [`Verb::parse`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Verb {
    /// Old layout → the machine's own flake.
    Migrate,
    /// Build, switch and arm the boot trial; with `rehearse`, nothing changes.
    Apply {
        /// Only measure (a copy under `/run`, `nix build --dry-run`).
        rehearse: bool,
    },
    /// After the boot: bless the trial, or go back to the last good one.
    Bless,
    /// After a rollback by hand: the trial left is not tried again.
    Disarm,
    /// Hold the distro at a revision.
    Hold {
        /// The revision (hex).
        rev: String,
    },
    /// Stop holding.
    Unhold,
    /// Root the sources the machine's flake was evaluated from.
    KeepSources,
    /// The distro input moved forward — an aimed machine put back on the
    /// release first (`aim`).
    UpdateDistro,
}

impl Verb {
    /// The CLI's arguments for it. Pure.
    #[must_use]
    pub fn arguments(&self) -> Vec<String> {
        match self {
            Self::Migrate => vec!["migrate".to_owned()],
            Self::Apply { rehearse: false } => vec!["apply".to_owned()],
            Self::Apply { rehearse: true } => vec!["apply".to_owned(), "--rehearse".to_owned()],
            Self::Bless => vec!["bless".to_owned()],
            Self::Disarm => vec!["disarm".to_owned()],
            Self::Hold { rev } => vec!["hold".to_owned(), rev.clone()],
            Self::Unhold => vec!["unhold".to_owned()],
            Self::KeepSources => vec!["keep-sources".to_owned()],
            Self::UpdateDistro => vec!["update-distro".to_owned()],
        }
    }

    /// The verb of the CLI's arguments; `None` for anything else. A revision
    /// is 7 to 64 lower-case hex digits. Pure.
    #[must_use]
    pub fn parse(args: &[String]) -> Option<Self> {
        let words: Vec<&str> = args.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["migrate"] => Some(Self::Migrate),
            ["apply"] => Some(Self::Apply { rehearse: false }),
            ["apply", "--rehearse"] => Some(Self::Apply { rehearse: true }),
            ["bless"] => Some(Self::Bless),
            ["disarm"] => Some(Self::Disarm),
            ["hold", rev] if is_rev(rev) => Some(Self::Hold {
                rev: (*rev).to_owned(),
            }),
            ["unhold"] => Some(Self::Unhold),
            ["keep-sources"] => Some(Self::KeepSources),
            ["update-distro"] => Some(Self::UpdateDistro),
            _ => None,
        }
    }
}

/// Whether `rev` is 7 to 64 lower-case hex digits.
#[must_use]
pub fn is_rev(rev: &str) -> bool {
    (7..=64).contains(&rev.len())
        && rev
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Where the machine's things are — parameters, never looked up here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The machine directory (`RONOCTUA_MACHINE_DIR`).
    pub dir: PathBuf,
    /// `/var/lib/ronoctua-machine`: `state.json`, `legacy-<stamp>`.
    pub state: PathBuf,
    /// `/run/ronoctua-update`: the out-link of the built system, the
    /// rehearsal's copy.
    pub run: PathBuf,
}

impl Places {
    /// The places the environment names: `RONOCTUA_MACHINE_DIR` (required),
    /// `RONOCTUA_MACHINE_STATE` ([`STATE_DIR`] when unset) and
    /// `RONOCTUA_MACHINE_RUN` ([`RUN_DIR`] when unset).
    ///
    /// # Errors
    ///
    /// The name of the variable that is missing.
    pub fn from_env(env: &Env) -> Result<Self, String> {
        Ok(Self {
            dir: env
                .path(system::MACHINE_DIR)
                .ok_or_else(|| format!("{} is not set", system::MACHINE_DIR))?,
            state: env
                .path(system::MACHINE_STATE)
                .unwrap_or_else(|| PathBuf::from(STATE_DIR)),
            run: env
                .path(system::MACHINE_RUN)
                .unwrap_or_else(|| PathBuf::from(RUN_DIR)),
        })
    }

    /// `state.json`.
    #[must_use]
    pub fn state_file(&self) -> PathBuf {
        self.state.join(ronoctua_person::names::STATE)
    }
}

/// What a migration did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migrated {
    /// Where the old layout went.
    pub legacy: PathBuf,
}

/// What an apply did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    /// How it ended.
    pub result: ApplyResult,
    /// The boot trial it armed, if any.
    pub trial: Option<Trial>,
    /// For a rehearsal: bytes to download and derivations to build.
    pub rehearsal: Option<Rehearsal>,
}

/// What a rehearsal measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rehearsal {
    /// Bytes the update would download.
    pub download_bytes: u64,
    /// Derivations it would build here.
    pub builds: u64,
}

/// The places and the system together: what every verb runs against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Machine {
    /// Where the machine's things are.
    pub places: Places,
    /// The programs and the system's own links.
    pub system: System,
}

impl Machine {
    /// The machine the process environment describes.
    #[must_use]
    pub fn from_env(places: Places) -> Self {
        Self {
            places,
            system: System::from_env(&Env::of_process()),
        }
    }

    /// Runs one verb, and says in one line what happened.
    ///
    /// # Errors
    ///
    /// The verb's refusal.
    pub fn run(&self, verb: &Verb) -> Result<String, Refusal> {
        match verb {
            Verb::Migrate => self
                .migrate()
                .map(|m| format!("migrated; the old layout is in {}", m.legacy.display())),
            Verb::Apply { rehearse } => self.apply(*rehearse).map(|a| {
                let no_trial = self.state().ok().and_then(|s| s.no_trial);
                verdict(&a, no_trial)
            }),
            Verb::Bless => self.bless().map(|o| format!("trial {}", outcome_word(o))),
            Verb::Disarm => self.disarm().map(|()| "disarmed".to_owned()),
            Verb::Hold { rev } => self.hold(rev).map(|()| format!("held at {rev}")),
            Verb::Unhold => self.unhold().map(|()| "not held".to_owned()),
            Verb::UpdateDistro => aim::update_distro(self),
            Verb::KeepSources => self.keep_sources().map(|kept| {
                if kept {
                    "sources kept".to_owned()
                } else {
                    "sources not kept".to_owned()
                }
            }),
        }
    }

    /// `migrate`.
    ///
    /// # Errors
    ///
    /// [`Why::NotRoot`], [`Why::Busy`], [`Why::NothingToMigrate`],
    /// [`Why::Unreadable`] (no `host.nix`), [`Why::BuildFailed`] (the old
    /// layout untouched), [`Why::Failed`].
    pub fn migrate(&self) -> Result<Migrated, Refusal> {
        migrate::migrate(self)
    }

    /// `apply [--rehearse]`.
    ///
    /// # Errors
    ///
    /// [`Why::NotRoot`], [`Why::Busy`], [`Why::Held`],
    /// [`Why::BuildFailed`] (the lock put back), [`Why::DidNotBoot`],
    /// [`Why::LastGoodWouldFallOut`], [`Why::SwitchFailed`],
    /// [`Why::Unreadable`], [`Why::Failed`].
    pub fn apply(&self, rehearse: bool) -> Result<Applied, Refusal> {
        if rehearse {
            apply::rehearse(self)
        } else {
            apply::apply(self)
        }
    }

    /// `bless`.
    ///
    /// # Errors
    ///
    /// [`Why::NotRoot`], [`Why::Busy`], [`Why::Unreadable`], [`Why::Failed`].
    pub fn bless(&self) -> Result<TrialOutcome, Refusal> {
        bless::bless(self)
    }

    /// `disarm`.
    ///
    /// # Errors
    ///
    /// [`Why::NotRoot`], [`Why::Busy`], [`Why::Unreadable`], [`Why::Failed`].
    pub fn disarm(&self) -> Result<(), Refusal> {
        disarm::disarm(self)
    }

    /// `hold <rev>`.
    ///
    /// # Errors
    ///
    /// [`Why::NotRoot`], [`Why::Busy`], [`Why::Held`] (the lock names
    /// another revision), [`Why::Unreadable`], [`Why::Failed`].
    pub fn hold(&self, rev: &str) -> Result<(), Refusal> {
        hold::hold(self, rev)
    }

    /// `unhold`.
    ///
    /// # Errors
    ///
    /// [`Why::NotRoot`], [`Why::Busy`], [`Why::Unreadable`], [`Why::Failed`].
    pub fn unhold(&self) -> Result<(), Refusal> {
        hold::unhold(self)
    }

    /// `keep-sources`: roots the sources the machine's flake was evaluated
    /// from — the flake, the distro and every input — that the store holds,
    /// as the apply does after every build ([`build::keep_sources`]). Run at
    /// every boot, so a machine an installation made (which ran no apply) has
    /// them held from its first boot. Whether they were kept: a list that
    /// could not be made is said, and is no refusal.
    ///
    /// NOT WAITED FOR: whoever holds the machine now is an apply, which
    /// roots its own sources after its build, or a verb that changes none.
    ///
    /// # Errors
    ///
    /// [`Why::NotRoot`], [`Why::Failed`] (the lock unopenable, no `nix`).
    pub fn keep_sources(&self) -> Result<bool, Refusal> {
        must_be_root(self)?;
        let _lock = match take_lock(self, 0) {
            Ok(lock) => lock,
            Err(refused) if refused.why == Why::Busy => {
                say(
                    6,
                    "the machine is busy: an apply keeps the sources of its own build",
                );
                return Ok(false);
            }
            Err(refused) => return Err(refused),
        };
        let nix = system::needed(self.system.nix.as_ref(), system::NIX)?;
        Ok(build::keep_sources(self, &nix))
    }

    /// The machine's record, as `state.json` holds it (the default record
    /// when there is none yet).
    ///
    /// # Errors
    ///
    /// [`Why::Unreadable`].
    pub fn state(&self) -> Result<MachineState, Refusal> {
        disk::read_state(&self.places)
    }
}

/// The one line an apply ends with.
fn verdict(applied: &Applied, no_trial: Option<NoTrial>) -> String {
    if let Some(r) = applied.rehearsal {
        return format!(
            "rehearsed: {} bytes to download, {} derivations to build",
            r.download_bytes, r.builds
        );
    }
    let result = match applied.result {
        ApplyResult::Switched => "switched",
        ApplyResult::SwitchedWithFailures => "switched, some units failed",
        ApplyResult::BuildFailed => "build failed",
        ApplyResult::SwitchFailed => "switch failed",
        ApplyResult::Rehearsed => "rehearsed",
        ApplyResult::Refused => "refused",
    };
    match (&applied.trial, no_trial) {
        (Some(t), _) => format!("{result}; trial {}", outcome_word(t.outcome)),
        (None, Some(why)) => format!("{result}; no boot trial ({})", why.id()),
        (None, None) => format!("{result}; no boot trial"),
    }
}

/// A trial outcome as the CLI says it.
const fn outcome_word(outcome: TrialOutcome) -> &'static str {
    match outcome {
        TrialOutcome::Pending => "pending",
        TrialOutcome::Good => "good",
        TrialOutcome::Failed => "failed",
        TrialOutcome::BootIdentical => "boot-identical",
    }
}

/// Runs one verb against the machine the environment describes.
///
/// # Errors
///
/// The verb's refusal.
pub fn run(verb: &Verb, places: &Places) -> Result<(), Refusal> {
    Machine::from_env(places.clone()).run(verb).map(|_| ())
}

/// `migrate`.
///
/// # Errors
///
/// See [`Machine::migrate`].
pub fn migrate(places: &Places) -> Result<Migrated, Refusal> {
    Machine::from_env(places.clone()).migrate()
}

/// `apply [--rehearse]`.
///
/// # Errors
///
/// See [`Machine::apply`].
pub fn apply(places: &Places, rehearse: bool) -> Result<Applied, Refusal> {
    Machine::from_env(places.clone()).apply(rehearse)
}

/// `bless`.
///
/// # Errors
///
/// See [`Machine::bless`].
pub fn bless(places: &Places) -> Result<TrialOutcome, Refusal> {
    Machine::from_env(places.clone()).bless()
}

/// `disarm`.
///
/// # Errors
///
/// See [`Machine::disarm`].
pub fn disarm(places: &Places) -> Result<(), Refusal> {
    Machine::from_env(places.clone()).disarm()
}

/// `hold <rev>`.
///
/// # Errors
///
/// See [`Machine::hold`].
pub fn hold(places: &Places, rev: &str) -> Result<(), Refusal> {
    Machine::from_env(places.clone()).hold(rev)
}

/// `unhold`.
///
/// # Errors
///
/// See [`Machine::unhold`].
pub fn unhold(places: &Places) -> Result<(), Refusal> {
    Machine::from_env(places.clone()).unhold()
}

/// `keep-sources`.
///
/// # Errors
///
/// See [`Machine::keep_sources`].
pub fn keep_sources(places: &Places) -> Result<bool, Refusal> {
    Machine::from_env(places.clone()).keep_sources()
}

/// The machine's record, as `state.json` holds it.
///
/// # Errors
///
/// [`Why::Unreadable`].
pub fn state(places: &Places) -> Result<MachineState, Refusal> {
    disk::read_state(places)
}

/// Root, and root's lock of the person's data — what every verb that may
/// change the machine takes first.
///
/// The lock is the one the person's applier, the update's verification and
/// the approval take (`ronoctua_person::root::LOCK`): a build reads the
/// person's data, and an apply between the verification and the switch
/// would hand the build an entry nobody tried. Under the update's
/// `ronoctua-person hold` it is found on the standard input, already held;
/// alone (the rescue's rebuild, `ronoctua-update-apply.service`) it is
/// taken, and whoever holds it is waited for a while, then refused as busy.
fn guard(m: &Machine) -> Result<ronoctua_person::root::Lock, Refusal> {
    must_be_root(m)?;
    take_lock(m, ronoctua_person::root::LOCK_WAIT_S)
}

/// Root, or a refusal: on the real system, nobody else changes it.
fn must_be_root(m: &Machine) -> Result<(), Refusal> {
    if m.system.is_the_real_one() && !rustix::process::geteuid().is_root() {
        say(3, "this changes the machine, and only root may (not-root)");
        return Err(Refusal::of(Why::NotRoot));
    }
    Ok(())
}

/// Root's lock of the person's data, waited for at most `wait_s` seconds.
fn take_lock(m: &Machine, wait_s: u64) -> Result<ronoctua_person::root::Lock, Refusal> {
    let path = m.system.person_lock();
    ronoctua_person::root::Lock::take(&path, std::time::Duration::from_secs(wait_s)).map_err(
        |refused| {
            say(3, &format!("{}: {refused}", path.display()));
            Refusal::of(match refused {
                ronoctua_person::root::LockRefused::Busy => Why::Busy,
                _ => Why::Failed,
            })
        },
    )
}

/// What a step said, on standard error with the journal's priority prefix:
/// `<6>` for what happened, `<4>` for what could not, `<3>` for a refusal.
fn say(priority: u8, what: &str) {
    eprintln!("<{priority}>{what}");
}

#[cfg(test)]
mod verbs {
    use super::*;

    #[test]
    fn every_verb_crosses_and_nothing_else_does() {
        for verb in [
            Verb::Migrate,
            Verb::Apply { rehearse: false },
            Verb::Apply { rehearse: true },
            Verb::Bless,
            Verb::Disarm,
            Verb::Hold {
                rev: "613862c".to_owned(),
            },
            Verb::Unhold,
            Verb::KeepSources,
            Verb::UpdateDistro,
        ] {
            assert_eq!(Verb::parse(&verb.arguments()), Some(verb));
        }
        for bad in [
            &["hold", "HEAD"][..],
            &["hold", "abc"],
            &["apply", "--force"],
            &["rm", "-rf"],
            &[],
        ] {
            let args: Vec<String> = bad.iter().map(|&s| s.to_owned()).collect();
            assert_eq!(Verb::parse(&args), None, "{bad:?}");
        }
    }

    #[test]
    fn every_why_has_one_id() {
        let ids: std::collections::BTreeSet<_> = Why::ALL.iter().map(|w| w.id()).collect();
        assert_eq!(ids.len(), Why::ALL.len());
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
            let json = serde_json::to_string(&why).unwrap();
            assert_eq!(json, format!("\"{}\"", why.id()), "the wire says the id");
        }
    }
}
