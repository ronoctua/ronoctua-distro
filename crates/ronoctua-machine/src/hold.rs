//! `hold <rev>` and `unhold`: the machine stays on the distro revision it
//! runs, and the update's `flake-update` step leaves the input alone while
//! the record says so.
//!
//! Only the revision the machine RUNS can be held. A lock written with
//! `--override-input` would name another revision for a while and then be
//! moved back by the first evaluation that compares it with `flake.nix`'s
//! address — a hold that does not hold. So the revision asked for has to be
//! the one the running system was built from (the lock the profile's system
//! carries or was kept for it, else the folder's), and the folder's lock is
//! put back on it if it had moved.

use ronoctua_person::state::Hold;

use crate::{DISTRO_INPUT, Machine, Refusal, Why, disk, guard, is_rev, say, trial};

/// `hold <rev>`.
pub(crate) fn hold(m: &Machine, rev: &str) -> Result<(), Refusal> {
    if !is_rev(rev) {
        say(
            3,
            &format!("{rev:?} is not a revision (7 to 64 lower-case hex digits)"),
        );
        return Err(Refusal::of(Why::Failed));
    }
    let _lock = guard(m)?;
    let p = &m.places;
    let mut state = disk::read_state(p)?;
    let file = p.dir.join("flake.lock");
    let now = disk::read_if_there(&file)?;
    let running = trial::running_lock(m)?;
    let Some(reference) = running.as_ref().or(now.as_ref()) else {
        say(
            3,
            &format!(
                "{} has no flake.lock: nothing names a revision",
                p.dir.display()
            ),
        );
        return Err(Refusal::of(Why::Unreadable));
    };
    let Some(locked) = disk::locked_rev(reference, DISTRO_INPUT) else {
        say(
            3,
            &format!("the lock names no revision of the input `{DISTRO_INPUT}`"),
        );
        return Err(Refusal::of(Why::Unreadable));
    };
    if !locked.starts_with(rev) {
        say(
            3,
            &format!("this machine runs {locked}; only the revision it runs can be held (held)"),
        );
        return Err(Refusal::of(Why::Held));
    }
    if now.as_ref() != Some(reference) {
        disk::write_atomically(&file, reference, 0o644).map_err(|e| disk::failed(&file, &e))?;
        say(6, "the lock is put back on the revision that runs");
    }
    state.hold = Some(Hold {
        rev: locked,
        since: disk::now(),
    });
    disk::write_state(p, &state)
}

/// `unhold`.
pub(crate) fn unhold(m: &Machine) -> Result<(), Refusal> {
    let _lock = guard(m)?;
    let mut state = disk::read_state(&m.places)?;
    state.hold = None;
    disk::write_state(&m.places, &state)
}
