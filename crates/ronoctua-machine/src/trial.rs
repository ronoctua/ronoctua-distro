//! Arming the boot trial, what a generation is recorded as, and the locks
//! the systems were built from.

use ronoctua_person::reports::PersonApplied;
use ronoctua_person::state::{LastGood, MachineState, NoTrial, Trial, TrialOutcome};

use crate::boot::{self, Entry};
use crate::loader::Loader;
use crate::system::{self, PROFILE};
use crate::{APPLIED_LOCK, LAST_GOOD_LOCK, Machine, Refusal, boots, disk, say};

/// The trial the record holds, when it is still waiting for its boot.
pub(crate) fn pending(state: &MachineState) -> Option<Trial> {
    state
        .trial
        .clone()
        .filter(|t| t.outcome == TrialOutcome::Pending)
}

/// The distro revision `toplevel` was built from, as its
/// `/etc/ronoctua/person-applied.json` says; `unknown` when it says nothing.
pub(crate) fn rev_of(m: &Machine, toplevel: &str) -> String {
    let file = m
        .system
        .real(toplevel)
        .join("etc/ronoctua")
        .join(ronoctua_person::names::PERSON_APPLIED);
    std::fs::read(file)
        .ok()
        .and_then(|bytes| ronoctua_person::parse::<PersonApplied>(&bytes).ok())
        .map_or_else(|| "unknown".to_owned(), |r| r.distro_rev)
}

/// `toplevel` as the last good generation, blessed now: its entry from the
/// menu when `bootctl` listed it, else the one its generation's number
/// gives.
pub(crate) fn last_good_of(m: &Machine, toplevel: &str, listed: Option<&[Entry]>) -> LastGood {
    let entry = listed
        .and_then(|e| boot::entry_of(e, toplevel))
        .map(str::to_owned)
        .or_else(|| {
            boot::generation_of_toplevel(&boot::generations(&m.system), toplevel)
                .map(|n| boot::entry_of_generation(&m.system, n))
        })
        .unwrap_or_else(|| "unknown".to_owned());
    LastGood {
        toplevel: toplevel.to_owned(),
        entry,
        distro_rev: rev_of(m, toplevel),
        blessed_at: disk::now(),
    }
}

/// The lock the system the profile names was built from: the one that
/// system carries, else the snapshot kept when the profile last took a
/// system ([`APPLIED_LOCK`]: a system built before the machine flake
/// carries none — the old layout's, right after a migration).
///
/// # Errors
///
/// [`crate::Why::Unreadable`]: the snapshot is there and does not read.
pub(crate) fn running_lock(m: &Machine) -> Result<Option<Vec<u8>>, Refusal> {
    let carried = m
        .system
        .resolve(PROFILE)
        .and_then(|profile| m.system.lock_of(&profile));
    match carried {
        Some(lock) => Ok(Some(lock)),
        None => disk::read_if_there(&disk::snapshot(&m.places, APPLIED_LOCK)),
    }
}

/// The lock `toplevel` was built from: the one it carries, else — when it is
/// the last good system — the snapshot kept for it.
pub(crate) fn lock_for(m: &Machine, state: &MachineState, toplevel: &str) -> Option<Vec<u8>> {
    m.system.lock_of(toplevel).or_else(|| {
        state
            .last_good
            .as_ref()
            .filter(|g| g.toplevel == toplevel)
            .and_then(|_| std::fs::read(disk::snapshot(&m.places, LAST_GOOD_LOCK)).ok())
    })
}

/// `toplevel` is the last good system now: the lock it was built from — the
/// one it carries, else `fallback` (the lock the profile's system was built
/// from) — is kept as the last good one's.
pub(crate) fn keep_last_good_lock(m: &Machine, toplevel: &str, fallback: Option<&[u8]>) {
    let carried = m.system.lock_of(toplevel);
    let Some(bytes) = carried.as_deref().or(fallback) else {
        say(
            4,
            "the last good system's lock is not known: a trial that fails keeps the folder's",
        );
        return;
    };
    if let Err(e) = disk::keep_snapshot(&m.places, LAST_GOOD_LOCK, bytes) {
        say(4, &format!("the last good lock was not kept: {e}"));
    }
}

/// Puts `lock` in the machine directory and keeps it as the profile's
/// system's: the folder describes the system the profile names again.
pub(crate) fn put_back(m: &Machine, lock: &[u8]) {
    for to in [
        m.places.dir.join("flake.lock"),
        disk::snapshot(&m.places, APPLIED_LOCK),
    ] {
        if std::fs::read(&to).ok().as_deref() == Some(lock) {
            continue;
        }
        if let Err(e) = disk::write_atomically(&to, lock, 0o644) {
            say(4, &format!("{}: {e}", to.display()));
        }
    }
}

/// Arms the boot trial of `toplevel`, the system just switched to, against
/// the last good one: the trial, or why there is none (said).
///
/// After a switch that returned 0 (`clean`), a system that boots like the
/// last good one (same kernel, initrd and parameters) needs no reboot to be
/// trusted: it is the last good one at once (`boot-identical`), and any
/// trial still armed is disarmed. After any other switch — 4, some units
/// failed; anything else, the activation failed — nothing proved the new
/// system runs well, and only a real trial does.
pub(crate) fn arm(
    m: &Machine,
    loader: &Loader,
    state: &mut MachineState,
    toplevel: &str,
    clean: bool,
) -> Result<Trial, NoTrial> {
    let Some(last_good) = state.last_good.clone() else {
        say(
            4,
            "no last good generation is known: no boot trial this time (no-last-good)",
        );
        return Err(NoTrial::NoLastGood);
    };
    if clean && boot::same_boot(&m.system, toplevel, &last_good.toplevel) {
        let listed = loader.entries();
        if listed.is_some() {
            loader.clear();
        }
        state.last_good = Some(last_good_of(m, toplevel, listed.as_deref()));
        // The profile names it already: its kept lock is this one's.
        let applied = std::fs::read(disk::snapshot(&m.places, APPLIED_LOCK)).ok();
        keep_last_good_lock(m, toplevel, applied.as_deref());
        boots::forget(m);
        say(
            6,
            "the new system switched cleanly and boots like the last good one: blessed at once",
        );
        return Ok(Trial {
            toplevel: toplevel.to_owned(),
            outcome: TrialOutcome::BootIdentical,
        });
    }
    let Some(listed) = loader.entries() else {
        say(
            4,
            &format!(
                "no boot trial: {} cannot be read or written (no-efi-variables)",
                loader.variables()
            ),
        );
        return Err(NoTrial::NoEfiVariables);
    };
    let Some(new) = boot::entry_of(&listed, toplevel) else {
        say(
            4,
            "no boot trial: the boot menu has no entry for the new system (not-in-menu)",
        );
        loader.clear();
        return Err(NoTrial::NotInMenu);
    };
    let Some(old) = boot::entry_of(&listed, &last_good.toplevel) else {
        say(
            4,
            "no boot trial: the boot menu no longer lists the last good system (not-in-menu)",
        );
        loader.clear();
        return Err(NoTrial::NotInMenu);
    };
    let unwritten = || {
        say(
            4,
            &format!(
                "no boot trial: {} cannot be written (no-efi-variables)",
                loader.variables()
            ),
        );
        Err(NoTrial::NoEfiVariables)
    };
    if !loader.set_default(old) {
        return unwritten();
    }
    if !loader.set_oneshot(new) {
        loader.set_default("");
        return unwritten();
    }
    if let Some(g) = state.last_good.as_mut() {
        g.entry = old.to_owned();
    }
    boots::arm(m, toplevel, m.system.this_boot().as_deref());
    say(
        6,
        &format!("boot trial armed: the next boot tries {new} once, and falls back to {old}"),
    );
    Ok(Trial {
        toplevel: toplevel.to_owned(),
        outcome: TrialOutcome::Pending,
    })
}

/// The generation `booted` belongs to: its own toplevel, or the toplevel of
/// the newest generation whose specialisation it is.
pub(crate) fn generation_toplevel(m: &Machine, booted: &str) -> String {
    boot::generations(&m.system)
        .iter()
        .rev()
        .filter_map(|g| g.toplevel.as_deref())
        .find(|top| boot::tops_of(&m.system, top).iter().any(|t| t == booted))
        .map_or_else(|| booted.to_owned(), str::to_owned)
}

/// Whether the profile names `toplevel` (or one of its specialisations).
pub(crate) fn profile_names(m: &Machine, toplevel: &str) -> bool {
    m.system
        .resolve(system::PROFILE)
        .is_some_and(|p| boot::tops_of(&m.system, toplevel).contains(&p))
}
