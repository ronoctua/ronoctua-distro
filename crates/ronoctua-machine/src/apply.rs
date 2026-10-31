//! `apply`: build, switch, arm the boot trial — `apply --next-boot`, which
//! sets what built for the next boot instead of switching, and `apply
//! --rehearse`, which only measures.
//!
//! # For the next boot
//!
//! The update's apply sets the new system for the next boot
//! (`switch-to-configuration boot`): nothing that runs is stopped or
//! restarted, and the boot trial is armed for real, the next boot trying
//! the new system once ([`crate::Applying`] says why). Any other apply does
//! the same, unasked, while a system already waits for the next boot or the
//! folder's lock moved `nixpkgs` past the profile's system ([`switches_now`]).
//! The record says [`ApplyResult::Staged`] and names the system that waits
//! ([`LastApply::toplevel`]).
//!
//! # The machine directory describes the system the profile names
//!
//! The update moves the distro and `nixpkgs` inputs (`nix flake update
//! ronoctua nixpkgs`) BEFORE it applies, so the lock apply finds may already
//! name revisions nothing was built from. The lock the profile's system WAS
//! built from is known apart from the folder: the system carries it
//! (`etc/ronoctua/machine.lock`, put there by `nix/modules/machine.nix`), and
//! for a system that carries none —
//! the old layout's, right after a migration — a snapshot
//! ([`crate::APPLIED_LOCK`], written by the migration and every time the
//! profile takes a new system) says it. Every way out of an apply that did
//! not make a new system — a failed build, a refusal — puts it back. A failed
//! update leaves the folder as the machine runs, and the next apply builds
//! what runs, not what failed.
//!
//! # Sized, unit by unit, and its units and sources kept
//!
//! The build is [`build::system`]: the units the distro's plan describes
//! first, each by its class (`ronoctua-build-units`), then the system, sized
//! to the machine (`ronoctua-build-sizing`); after it worked the compiled
//! units it was built from are rooted for the next one ([`build::keep`]),
//! and so are the sources it was evaluated from ([`build::keep_sources`]).
//!
//! # What nobody may redistribute arrives with the first network
//!
//! A machine installed with no network carries the installer's marker
//! (`ronoctua_person::names::DEFERRED_HARDWARE`), and the hardware branches
//! whose packages nobody may redistribute stay off while it is there
//! (`nix/modules/host.nix`). Every apply that finds it builds WITHOUT it
//! first: when that works the marker stays gone and the hardware arrived;
//! when it does not (no network, a download that failed) the marker is put
//! back and the machine is built as it was — whole, those branches off —
//! and the journal says so.
//!
//! # A trial is judged before anything is built
//!
//! An apply that runs before this boot's bless (an update the boot's timer
//! starts at once) judges a trial an earlier boot tried, the way the bless
//! would ([`crate::bless`]): it must not arm a new trial against a last good
//! system that is not the last one, nor build again the system that did not
//! boot. That one is refused (`did-not-boot`) until the distro or `nixpkgs`
//! moves and another system is built.

use ronoctua_person::frontier::Download;
use ronoctua_person::state::{ApplyResult, LastApply, MachineState, TrialOutcome};

use crate::argv::{self, Switch};
use crate::build::{Built, Unbuilt};
use crate::loader::Loader;
use crate::system::{self, needed};
use crate::{
    APPLIED_LOCK, Applied, Applying, DISTRO_INPUT, Machine, NIXPKGS_INPUT, Refusal, Why, bless,
    boot, boots, build, disk, dry, guard, mirror, run, say, trial,
};

/// The attribute the machine flake builds, or a refusal.
pub(crate) fn attr(m: &Machine) -> Result<String, Refusal> {
    let attr = needed(m.system.attr.as_ref(), system::MACHINE_ATTR)?;
    if argv::is_attr(&attr) {
        Ok(attr)
    } else {
        say(
            3,
            &format!("{} is {attr:?}, not a user name", system::MACHINE_ATTR),
        );
        Err(Refusal::of(Why::Failed))
    }
}

/// The lock files of one apply: what the folder holds now, and what the
/// profile's system was built from.
struct Locks {
    file: std::path::PathBuf,
    now: Option<Vec<u8>>,
    running: Option<Vec<u8>>,
}

impl Locks {
    fn read(m: &Machine) -> Result<Self, Refusal> {
        let file = m.places.dir.join("flake.lock");
        let now = disk::read_if_there(&file)?;
        let running = trial::running_lock(m)?;
        Ok(Self { file, now, running })
    }

    /// Puts back the lock the profile's system was built from, when the
    /// folder holds another.
    fn put_back(&self) {
        let Some(running) = &self.running else {
            say(
                4,
                "the lock the running system was built from is not known: the folder keeps its own",
            );
            return;
        };
        if self.now.as_ref() != Some(running) {
            match disk::write_atomically(&self.file, running, 0o644) {
                Ok(()) => say(
                    6,
                    "the lock is put back: the machine directory describes the running system again",
                ),
                Err(e) => say(
                    3,
                    &format!(
                        "{}: the lock could not be put back: {e}",
                        self.file.display()
                    ),
                ),
            }
        }
    }

    /// The folder's lock is now the profile's system's.
    fn adopt(&self, m: &Machine) {
        if let Some(now) = &self.now
            && let Err(e) = disk::keep_snapshot(&m.places, APPLIED_LOCK, now)
        {
            say(4, &format!("the applied lock was not kept: {e}"));
        }
    }
}

/// Records how the apply ended, the system it set up (if it set one up)
/// and what its build could not fetch.
fn record(
    m: &Machine,
    state: &mut MachineState,
    result: ApplyResult,
    toplevel: Option<&str>,
    missing: Vec<Download>,
) -> Result<(), Refusal> {
    state.last_apply = Some(LastApply {
        at: disk::now(),
        result,
        toplevel: toplevel.map(str::to_owned),
        missing,
    });
    disk::write_state(&m.places, state)
}

/// Ends an apply that made nothing: the lock put back, the refusal recorded.
fn refuse(
    m: &Machine,
    locks: &Locks,
    state: &mut MachineState,
    result: ApplyResult,
    why: Why,
) -> Refusal {
    locks.put_back();
    let _ = record(m, state, result, None, Vec::new());
    Refusal::of(why)
}

/// Ends an apply whose build failed: the lock put back, and the refusal
/// recorded with the downloads it was missing — what the update's page
/// offers to hand in or bring on a stick.
fn unbuilt(m: &Machine, locks: &Locks, state: &mut MachineState, failed: Unbuilt) -> Refusal {
    locks.put_back();
    let _ = record(m, state, ApplyResult::BuildFailed, None, failed.missing);
    failed.refusal
}

/// `apply`, now or for the next boot (`how`: never a rehearsal, which is
/// [`rehearse`]).
pub(crate) fn apply(m: &Machine, how: Applying) -> Result<Applied, Refusal> {
    let _lock = guard(m)?;
    let (p, s) = (&m.places, &m.system);
    let attr = attr(m)?;
    let nix = needed(s.nix.as_ref(), system::NIX)?;
    let nix_env = needed(s.nix_env().as_ref(), system::NIX)?;
    let loader = needed(
        Loader::of(s).as_ref(),
        &format!(
            "{} (or, on GRUB, {})",
            system::BOOTCTL,
            system::GRUB_EDITENV
        ),
    )?;
    needed(s.sizing.as_ref(), system::BUILD_SIZING)?;
    needed(s.build_units.as_ref(), system::BUILD_UNITS)?;
    needed(s.keep_units.as_ref(), system::KEEP_UNITS)?;
    let limit = s.menu_limit()?;
    let mut state = disk::read_state(p)?;

    // A TRIAL AN EARLIER BOOT TRIED is judged first (it may put the last good
    // system's lock back in the folder, so the locks are read after it).
    if let Some(booted) = s.resolve(system::BOOTED)
        && bless::settle(m, &mut state, &booted)?.is_some_and(|o| o != TrialOutcome::Pending)
    {
        disk::write_state(p, &state)?;
    }
    let locks = Locks::read(m)?;

    // HELD: the folder must name the held revision, or nothing is built.
    if let Some(hold) = &state.hold {
        let locked = locks
            .now
            .as_deref()
            .and_then(|l| disk::locked_rev(l, DISTRO_INPUT));
        if !locked.as_deref().is_some_and(|r| r.starts_with(&hold.rev)) {
            say(
                3,
                &format!(
                    "this machine is held at {}, and its lock names {}: nothing was built (held)",
                    hold.rev,
                    locked.as_deref().unwrap_or("no revision")
                ),
            );
            return Err(refuse(
                m,
                &locks,
                &mut state,
                ApplyResult::Refused,
                Why::Held,
            ));
        }
    }

    // THE BUILD, which changes nothing but the store: the distro's units by
    // their class, then the system, sized ([`build`]). Any way it fails puts
    // the lock back.
    std::fs::create_dir_all(&p.run).map_err(|e| disk::failed(&p.run, &e))?;
    let out = p.run.join("next");
    disk::remove(&out).map_err(|e| disk::failed(&out, &e))?;
    // WHAT WAITED FOR A NETWORK, tried first (the module's header).
    let marker = p.dir.join(ronoctua_person::names::DEFERRED_HARDWARE);
    let deferred = disk::read_if_there(&marker)?;
    if deferred.is_some() {
        disk::remove(&marker).map_err(|e| disk::failed(&marker, &e))?;
    }
    // WHAT DID NOT COME is recorded with the result: the system's downloads
    // when its build failed, or the hardware's when the machine was built
    // without it again.
    let (made, missing) = match built(m, &nix, &attr, &out) {
        Ok(made) => {
            if deferred.is_some() {
                say(6, "the hardware that waited for a network arrived");
            }
            (made, Vec::new())
        }
        Err(waits) if deferred.is_some() => {
            let bytes = deferred.unwrap_or_default();
            disk::write_atomically(&marker, &bytes, 0o644)
                .map_err(|e| disk::failed(&marker, &e))?;
            say(
                4,
                "the hardware that waits for a network did not arrive (what nobody may \
                 redistribute is downloaded from its maker): built without it again",
            );
            disk::remove(&out).map_err(|e| disk::failed(&out, &e))?;
            match built(m, &nix, &attr, &out) {
                Ok(made) => (made, waits.missing),
                Err(failed) => return Err(unbuilt(m, &locks, &mut state, failed)),
            }
        }
        Err(failed) => return Err(unbuilt(m, &locks, &mut state, failed)),
    };
    // AND ITS UNITS, KEPT for the next build (the last build's released). A
    // build that worked is kept whatever happens to its switch: it is what
    // the next update builds on.
    if let Some(kept) = &made.kept {
        build::keep(m, kept);
    }
    // AND THE SOURCES IT WAS EVALUATED FROM, kept as its units are: an update
    // that finds them unchanged fetches none of them again — and so are the
    // downloads it was built from.
    let _ = build::keep_sources(m, &nix);
    if let Some(drv) = &made.drv {
        let _ = build::keep_downloads(m, &nix, drv);
    }
    drop(made);
    let Ok(built) = std::fs::canonicalize(&out) else {
        say(3, &format!("the build left no system at {}", out.display()));
        return Err(refuse(
            m,
            &locks,
            &mut state,
            ApplyResult::BuildFailed,
            Why::BuildFailed,
        ));
    };
    let toplevel = s.logical(&built);
    let made = Made {
        toplevel: &toplevel,
        missing,
        how,
    };
    let outcome = switch_to(m, &nix_env, &loader, limit, &locks, &mut state, made);
    let _ = disk::remove(&out);
    outcome
}

/// What a good build made: the system, and what it left missing — the
/// hardware that waits for a network, when it was built without it again —
/// and how the apply was asked to set it up.
struct Made<'a> {
    toplevel: &'a str,
    missing: Vec<Download>,
    how: Applying,
}

/// Whether an apply asked as `how` may switch to `toplevel` NOW: it was not
/// asked for the next boot, and either `toplevel` is what already runs (a
/// switch to it activates nothing new) or the system that runs is the
/// profile's (or one of its specialisations) — a system set for the next
/// boot stays for the next boot — and the folder's lock names the very
/// `nixpkgs` the profile's system was built from ([`disk::locked_nixpkgs`]).
/// A `nixpkgs` this cannot tell — a lock that names none, a profile's system
/// whose lock is not known — is no proof that nothing moved: that system is
/// set for the next boot too, and the journal says why. Pure but for the
/// specialisations, read off the systems.
pub(crate) fn switches_now(
    m: &Machine,
    how: Applying,
    toplevel: &str,
    systems: (Option<&str>, Option<&str>),
    locks: (Option<&[u8]>, Option<&[u8]>),
) -> bool {
    if how != Applying::Now {
        return false;
    }
    let (profile, running) = systems;
    if let Some(running) = running
        && boot::tops_of(&m.system, toplevel)
            .iter()
            .any(|t| t == running)
    {
        return true;
    }
    if let (Some(profile), Some(running)) = (profile, running)
        && !boot::tops_of(&m.system, profile)
            .iter()
            .any(|t| t == running)
    {
        say(
            6,
            "the system profile names a system that does not run yet: this one is set for the \
             next boot too, and nothing that runs is switched",
        );
        return false;
    }
    let (now, built_from) = locks;
    let nixpkgs = |lock: Option<&[u8]>| lock.and_then(disk::locked_nixpkgs);
    match (nixpkgs(now), nixpkgs(built_from)) {
        (Some(now), Some(then)) if now == then => true,
        (Some(now), Some(then)) => {
            say(
                6,
                &format!(
                    "the lock moved {NIXPKGS_INPUT} from {then} to {now}: the new system is set for \
                     the next boot, and nothing that runs is switched across it"
                ),
            );
            false
        }
        (None, _) => {
            say(
                6,
                &format!(
                    "the folder's lock names no {NIXPKGS_INPUT} this can read: whether it moved is \
                     not known, so the new system is set for the next boot"
                ),
            );
            false
        }
        (Some(_), None) => {
            say(
                6,
                &format!(
                    "the {NIXPKGS_INPUT} the profile's system was built from is not known: whether \
                     it moved is not either, so the new system is set for the next boot"
                ),
            );
            false
        }
    }
}

/// The machine's system built into `out` — and, when the build failed for
/// want of downloads the person's mirrors hold, built once more with what
/// they brought ([`mirror`]).
fn built(
    m: &Machine,
    nix: &std::path::Path,
    attr: &str,
    out: &std::path::Path,
) -> Result<Built, Unbuilt> {
    let failed = match build::system(m, nix, &m.places.dir, attr, out) {
        Err(failed) if !failed.missing.is_empty() => failed,
        other => return other,
    };
    let mirrors = mirror::named(&m.places.dir);
    if mirrors.is_empty() || mirror::fetched(nix, &failed.missing, &mirrors) == 0 {
        return Err(failed);
    }
    say(
        6,
        "the person's mirrors brought what the build was missing: it is built again",
    );
    disk::remove(out).map_err(|e| disk::failed(out, &e))?;
    build::system(m, nix, &m.places.dir, attr, out)
}

/// Everything after a good build: the menu's room, the profile, the switch
/// and the trial — and, recorded with them, what the build left missing.
fn switch_to(
    m: &Machine,
    nix_env: &std::path::Path,
    loader: &Loader,
    limit: u64,
    locks: &Locks,
    state: &mut MachineState,
    made: Made,
) -> Result<Applied, Refusal> {
    let Made {
        toplevel,
        missing,
        how,
    } = made;
    let s = &m.system;
    let profile = s.resolve(system::PROFILE);
    let running = s.resolve(system::CURRENT);

    // THE SYSTEM THAT DID NOT BOOT is not tried again: the next update would
    // switch into it, and the next restart fail the same way. A newer distro
    // revision, or a `nixpkgs` the machine rolled on, builds another system,
    // and that one is tried.
    if let Some(t) = &state.trial
        && t.outcome == TrialOutcome::Failed
        && t.toplevel == toplevel
    {
        say(
            3,
            "this is the system that did not start when it was tried (or was rolled back): it is \
             not tried again before the distro or nixpkgs moves on (did-not-boot)",
        );
        return Err(refuse(
            m,
            locks,
            state,
            ApplyResult::Refused,
            Why::DidNotBoot,
        ));
    }

    // NOTHING TO SWITCH: the profile and the running system already are it.
    if profile.as_deref() == Some(toplevel) && running.as_deref() == Some(toplevel) {
        locks.adopt(m);
        record(m, state, ApplyResult::Switched, Some(toplevel), missing)?;
        say(6, "the built system is the one running: nothing to switch");
        return Ok(Applied {
            result: ApplyResult::Switched,
            trial: state.trial.clone(),
            rehearsal: None,
        });
    }

    // THE WAY BACK. Before any bless, the system that booted this machine is
    // the last good one: it booted, and it is running this. The lock it was
    // built from is the one it carries, else the profile's kept one — taken
    // now, before the profile moves.
    if state.last_good.is_none()
        && let Some(booted) = s.resolve(system::BOOTED)
    {
        state.last_good = Some(trial::last_good_of(m, &booted, None));
        trial::keep_last_good_lock(m, &booted, locks.running.as_deref());
    }
    if let Some(last_good) = &state.last_good {
        let adds = profile.as_deref() != Some(toplevel);
        if boot::falls_out(&boot::generations(s), adds, limit, &last_good.toplevel) {
            say(
                3,
                "one more generation would push the last good system out of the boot menu: \
                 reboot into the system on trial first (last-good-would-fall-out)",
            );
            return Err(refuse(
                m,
                locks,
                state,
                ApplyResult::Refused,
                Why::LastGoodWouldFallOut,
            ));
        }
    }

    // NOW, OR FOR THE NEXT BOOT — decided before the profile moves, against
    // the system it names now and the lock that system was built from.
    let switch_how = if switches_now(
        m,
        how,
        toplevel,
        (profile.as_deref(), running.as_deref()),
        (locks.now.as_deref(), locks.running.as_deref()),
    ) {
        Switch::Switch
    } else {
        Switch::Boot
    };

    // THE PROFILE: from here the next boot is the new system, and the
    // folder's lock is the one it was built from.
    if run::status(nix_env, &argv::set_profile(&s.profile(), toplevel))? != Some(0) {
        say(3, "the system profile did not take the new system");
        return Err(refuse(
            m,
            locks,
            state,
            ApplyResult::SwitchFailed,
            Why::Failed,
        ));
    }
    locks.adopt(m);

    // THE SWITCH. 4 is "switched, and some units it started failed": the
    // new system runs, and that is not a failed apply. FOR THE NEXT BOOT
    // (`boot`) only the boot loader is written: nothing runs the new system
    // yet, and 0 is `staged`.
    let switch = s.real(toplevel).join("bin/switch-to-configuration");
    let result = match (switch_how, run::status(&switch, &argv::switch(switch_how))?) {
        (Switch::Boot, Some(0)) => ApplyResult::Staged,
        (Switch::Switch, Some(0)) => ApplyResult::Switched,
        (Switch::Switch, Some(4)) => ApplyResult::SwitchedWithFailures,
        _ => ApplyResult::SwitchFailed,
    };

    // THE TRIAL — armed even after a failed switch: the profile already
    // names the new system, and the next boot needs its way back most then.
    // Only a switch that returned 0 may bless at once a system that boots
    // like the last good one; any other — a system set for the next boot
    // above all, which nothing has run — is tried by a boot.
    let armed = trial::arm(m, loader, state, toplevel, result == ApplyResult::Switched);
    match &armed {
        Ok(t) => {
            state.trial = Some(t.clone());
            state.no_trial = None;
        }
        Err(why) => {
            state.trial = None;
            state.no_trial = Some(*why);
            boots::forget(m);
        }
    }
    record(m, state, result, Some(toplevel), missing)?;
    if result == ApplyResult::SwitchFailed {
        return Err(match switch_how {
            Switch::Switch => {
                say(
                    3,
                    "the switch failed; the next boot tries the new system with the way back armed",
                );
                Refusal::of(Why::SwitchFailed)
            }
            Switch::Boot => {
                say(
                    3,
                    "setting the new system for the next boot failed: nothing that runs changed, \
                     and the boot menu may not list it (next-boot-failed)",
                );
                Refusal::of(Why::NextBootFailed)
            }
        });
    }
    Ok(Applied {
        result,
        trial: armed.ok(),
        rehearsal: None,
    })
}

/// `apply --rehearse`: the machine directory copied under the run folder,
/// the distro and `nixpkgs` inputs moved THERE (as the update would, unless
/// held), and the build only measured. Nothing of the machine changes.
pub(crate) fn rehearse(m: &Machine) -> Result<Applied, Refusal> {
    let _lock = guard(m)?;
    let (p, s) = (&m.places, &m.system);
    let attr = attr(m)?;
    let nix = needed(s.nix.as_ref(), system::NIX)?;
    let state = disk::read_state(p)?;
    std::fs::create_dir_all(&p.run).map_err(|e| disk::failed(&p.run, &e))?;
    let copy = p.run.join("rehearsal");
    disk::remove(&copy).map_err(|e| disk::failed(&copy, &e))?;
    disk::copy_tree(&p.dir, &copy, &[crate::migrate::NEXT])
        .map_err(|e| disk::failed(&p.dir, &e))?;
    let measured = (|| {
        if state.hold.is_none() {
            if run::status(&nix, &argv::update_distro(&copy))? != Some(0) {
                say(
                    4,
                    "the distro and nixpkgs inputs did not move (no network?): measured with the \
                     lock as it is",
                );
            }
        } else {
            say(6, "held: the distro and nixpkgs inputs stay where they are");
        }
        let ran = run::capture(&nix, &argv::dry_run(&copy, &attr))?;
        eprint!("{}", ran.stderr);
        if ran.ok() {
            Ok(dry::rehearsal(&ran.stderr))
        } else {
            say(3, "the new system would not build (build-failed)");
            Err(Refusal::of(Why::BuildFailed))
        }
    })();
    let _ = disk::remove(&copy);
    let rehearsal = measured?;
    Ok(Applied {
        result: ApplyResult::Rehearsed,
        trial: None,
        rehearsal: Some(rehearsal),
    })
}
