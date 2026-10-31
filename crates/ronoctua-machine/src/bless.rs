//! `bless`: the boot reached the system's default target and stayed up a
//! while (`ronoctua-boot-bless.service`) — which system booted?
//!
//! # A trial is judged by the first boot after the one that armed it
//!
//! Not by the boot the bless happens to run in, and not by the clock
//! ([`crate::boots`]):
//!
//! - In the boot that ARMED it — the first switch that brings the bless unit
//!   starts it at once, and a person may restart it — the trial has not been
//!   tried, and is left alone.
//! - The first LATER boot that lasts to the bless writes down what it booted,
//!   BEFORE the bless waits for root's lock of the person's data. A bless that
//!   then waits long (an update holds the lock) or never gets it (the machine
//!   is turned off) has still said it, and whoever judges next — the next
//!   bless, or an apply — judges by that boot, not by its own: a trial that
//!   booted well is never called failed because the boot AFTER it came up on
//!   the EFI default.
//!
//! Then:
//!
//! - That boot booted the trial: it is the last good system now, and the EFI
//!   default is unset — `loader.conf` names it already.
//! - It booted another system: the trial did not boot (the one-shot was spent
//!   on it, and the boot after came back to the default). The trial is
//!   `failed`, the system profile goes back to what booted
//!   (`switch-to-configuration boot` writes the menu for it), and the lock
//!   that system was built from goes back into the machine directory — so the
//!   next apply builds what runs, and refuses to build the failed system
//!   again (`did-not-boot`): only a NEW system tries again, built after the
//!   distro or `nixpkgs` moved.
//! - No trial waiting: whatever booted and lasted is the last good one.

use ronoctua_person::state::{MachineState, Trial, TrialOutcome};

use crate::argv::{self, Switch};
use crate::loader::Loader;
use crate::system::{self, needed};
use crate::{
    LAST_GOOD_LOCK, Machine, Refusal, Why, boot, boots, disk, must_be_root, run, say, sleep,
    take_lock, trial,
};

/// `bless`.
pub(crate) fn bless(m: &Machine) -> Result<TrialOutcome, Refusal> {
    must_be_root(m)?;
    let (p, s) = (&m.places, &m.system);
    let Some(booted) = s.resolve(system::BOOTED) else {
        say(
            3,
            &format!("{} leads nowhere: nothing to bless", system::BOOTED),
        );
        return Err(Refusal::of(Why::Failed));
    };

    // WHAT THIS BOOT BOOTED, written down before any wait.
    if let (Ok(state), Some(boot)) = (disk::read_state(p), s.this_boot()) {
        note_this_boot(m, &state, &boot, &booted);
    }

    let _lock = take_lock(m, s.bless_wait_s()?)?;
    let mut state = disk::read_state(p)?;
    let outcome = match settle(m, &mut state, &booted)? {
        Some(outcome) => outcome,
        None => {
            // NO TRIAL WAITING: what booted and lasted is the last good one.
            if state.last_good.as_ref().map(|g| g.toplevel.as_str()) != Some(booted.as_str()) {
                let listed = Loader::of(s).as_ref().and_then(Loader::entries);
                state.last_good = Some(trial::last_good_of(m, &booted, listed.as_deref()));
                if trial::profile_names(m, &trial::generation_toplevel(m, &booted)) {
                    let running = trial::running_lock(m).ok().flatten();
                    trial::keep_last_good_lock(m, &booted, running.as_deref());
                }
                say(
                    6,
                    &format!("{booted} booted and lasted: it is the last good system"),
                );
            }
            // The system a switch made without a trial has booted: the time
            // it had no way back is over.
            if state.no_trial.is_some()
                && trial::profile_names(m, &trial::generation_toplevel(m, &booted))
            {
                state.no_trial = None;
            }
            TrialOutcome::Good
        }
    };
    disk::write_state(p, &state)?;
    Ok(outcome)
}

/// Writes down, for the trial waiting, that this boot booted `booted` — when
/// this is a later boot than the one that armed it, and no earlier one has.
fn note_this_boot(m: &Machine, state: &MachineState, boot: &str, booted: &str) {
    let Some(t) = trial::pending(state) else {
        return;
    };
    // A ONE-SHOT PUT ASIDE FOR A HIBERNATION: this boot did not try it, and
    // the judgement puts it back ([`sleep::lost_resume`]).
    if sleep::put_aside_for(m, &t) {
        return;
    }
    let armed = boots::armed(m, &t.toplevel).unwrap_or_else(|| boots::unknown(&t.toplevel));
    if !armed_here(m, &armed, &t, boot, booted) {
        boots::note(m, &armed, boot, booted);
    }
}

/// Whether the trial `t` was armed in this boot: the record says so; or, with
/// no record of the boot that armed it, another system booted and either
/// the system that runs IS the trial — it was switched to in this boot — or
/// the loader still holds the one-shot that tries it ([`untried`]) — it was
/// set for the next boot, and no boot has read it yet.
fn armed_here(m: &Machine, armed: &boots::Armed, t: &Trial, boot: &str, booted: &str) -> bool {
    if boots::armed_in(armed, boot) {
        return true;
    }
    boots::in_unknown_boot(armed)
        && !boot::tops_of(&m.system, &t.toplevel)
            .iter()
            .any(|top| top == booted)
        && (m.system.resolve(system::CURRENT).as_deref() == Some(t.toplevel.as_str())
            || untried(m, t))
}

/// Whether the loader still holds the one-shot that tries `t` — its entry,
/// as the menu names it. Both loaders forget a one-shot the moment a boot
/// reads it, so one still set has not been tried: a trial set for the next
/// boot (`apply --next-boot`), judged in the boot that set it, whose record
/// of that boot was lost, runs the system it ran before — and is not the
/// trial that failed.
fn untried(m: &Machine, t: &Trial) -> bool {
    let Some(loader) = Loader::of(&m.system) else {
        return false;
    };
    let Some(oneshot) = loader.oneshot().filter(|o| !o.is_empty()) else {
        return false;
    };
    loader
        .entries()
        .is_some_and(|listed| boot::entry_of(&listed, &t.toplevel) == Some(oneshot.as_str()))
}

/// Judges the trial waiting, if there is one: `None` when none waits; the
/// trial's outcome otherwise (`pending` while it has not been tried). Run by
/// the bless and, before anything else, by an apply — an update that runs
/// before this boot's bless must not build on a judgement nobody made.
/// Root's lock of the person's data is held.
pub(crate) fn settle(
    m: &Machine,
    state: &mut MachineState,
    booted: &str,
) -> Result<Option<TrialOutcome>, Refusal> {
    let Some(t) = trial::pending(state) else {
        return Ok(None);
    };
    let s = &m.system;
    let Some(boot) = s.this_boot() else {
        say(
            4,
            "this boot's identity cannot be read: the trial is not judged",
        );
        return Ok(Some(TrialOutcome::Pending));
    };
    let armed = boots::armed(m, &t.toplevel).unwrap_or_else(|| boots::unknown(&t.toplevel));
    if armed_here(m, &armed, &t, &boot, booted) {
        say(
            6,
            "the trial was armed during this boot: it has not been tried yet",
        );
        return Ok(Some(TrialOutcome::Pending));
    }
    // ITS ONE-SHOT PUT ASIDE FOR A HIBERNATION never put back: no boot tried
    // it, and it is armed again for the next ([`sleep`]).
    if sleep::lost_resume(m, &t, &boot) {
        return Ok(Some(TrialOutcome::Pending));
    }
    let tried = boots::note(m, &armed, &boot, booted);
    if tried.boot != boot {
        say(
            6,
            &format!(
                "judged by the boot that first tried it ({}), not by this one",
                tried.boot
            ),
        );
    }
    let loader = Loader::of(s);
    let listed = loader.as_ref().and_then(Loader::entries);
    let outcome = if boot::tops_of(s, &t.toplevel).contains(&tried.booted) {
        state.last_good = Some(trial::last_good_of(m, &t.toplevel, listed.as_deref()));
        state.trial = Some(Trial {
            outcome: TrialOutcome::Good,
            ..t.clone()
        });
        if listed.is_some()
            && let Some(l) = &loader
        {
            l.set_default("");
        }
        let running = trial::running_lock(m).ok().flatten();
        trial::keep_last_good_lock(m, &t.toplevel, running.as_deref());
        say(
            6,
            &format!(
                "the trial {} booted: it is the last good system",
                t.toplevel
            ),
        );
        TrialOutcome::Good
    } else {
        let back_to = match &state.last_good {
            Some(g) if boot::tops_of(s, &g.toplevel).contains(&tried.booted) => g.toplevel.clone(),
            _ => trial::generation_toplevel(m, &tried.booted),
        };
        // The lock to go back to, found before the record moves.
        let lock = trial::lock_for(m, state, &back_to);
        state.trial = Some(Trial {
            outcome: TrialOutcome::Failed,
            ..t.clone()
        });
        go_back(m, &back_to)?;
        if listed.is_some()
            && let Some(l) = &loader
        {
            l.set_default("");
        }
        if state.last_good.as_ref().map(|g| g.toplevel.as_str()) != Some(back_to.as_str()) {
            state.last_good = Some(trial::last_good_of(m, &back_to, listed.as_deref()));
        }
        match lock {
            Some(lock) => {
                trial::put_back(m, &lock);
                if let Err(e) = disk::keep_snapshot(&m.places, LAST_GOOD_LOCK, &lock) {
                    say(4, &format!("the last good lock was not kept: {e}"));
                }
                say(
                    6,
                    "the lock is put back: the machine directory describes the system that runs",
                );
            }
            None => say(
                4,
                "no lock of the system that runs is known: the machine directory keeps its own",
            ),
        }
        say(
            4,
            &format!(
                "the trial {} did not boot: this machine is back on {back_to}",
                t.toplevel
            ),
        );
        TrialOutcome::Failed
    };
    boots::forget(m);
    sleep::forget(m);
    Ok(Some(outcome))
}

/// The system profile back on `toplevel`, and the boot menu written for it.
fn go_back(m: &Machine, toplevel: &str) -> Result<(), Refusal> {
    let s = &m.system;
    if s.resolve(system::PROFILE).as_deref() == Some(toplevel) {
        return Ok(());
    }
    let nix = needed(s.nix_env().as_ref(), system::NIX)?;
    if run::status(&nix, &argv::set_profile(&s.profile(), toplevel))? != Some(0) {
        say(3, "the system profile could not go back to what booted");
        return Err(Refusal::of(Why::Failed));
    }
    let switch = s.real(toplevel).join("bin/switch-to-configuration");
    if run::status(&switch, &argv::switch(Switch::Boot))? != Some(0) {
        say(
            4,
            "the boot menu was not written again for the system that booted",
        );
    }
    Ok(())
}
