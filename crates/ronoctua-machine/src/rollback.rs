//! `rollback switch | boot`: the rescue console's way back
//! (`nix/modules/rescue.nix`, `ronoctua-rescue-rollback@<how>.service`).
//!
//! # Back to a system that ran
//!
//! `nix-env --rollback` goes to the profile's PREVIOUS generation, and since
//! applies set their system for the next boot that one may never have run:
//! two applies before a restart leave two generations nobody booted, and the
//! one before the newest is the first of them — on another `nixpkgs`,
//! perhaps, and with no boot trial to bring a machine it does not start
//! back. So the way back is chosen here, by what lasts:
//!
//! - WHEN A SYSTEM WAITS for the next boot (the profile names another than
//!   the one that runs): back to the system that RUNS — its newest
//!   generation, no new one made. Whatever waited is dropped, and nothing
//!   that runs is switched: `boot` or `switch` alike only write the menu for
//!   it.
//! - OTHERWISE, the generation before the profile's, as `nix-env --rollback`
//!   would — passing over one that is the system that runs again (a way back
//!   the bless made) and the system whose trial failed, neither of which is
//!   a way back. `switch` activates it now, `boot` from the next boot.
//!
//! Then, as [`crate::disarm`] says, the trial left is recorded `failed`, the
//! boot loader's variables go, and the folder gets back the lock of the
//! system the profile names. Root's lock of the person's data is held
//! throughout, as every verb that moves the profile holds it.

use ronoctua_person::state::{MachineState, TrialOutcome};

use crate::argv::{self, Switch};
use crate::boot::{self, Generation};
use crate::system::{self, needed};
use crate::{Machine, Refusal, Why, disarm, disk, guard, run, say, trial};

/// `rollback switch` (`now`) or `rollback boot`; the line it ends with.
pub(crate) fn rollback(m: &Machine, now: bool) -> Result<String, Refusal> {
    let _lock = guard(m)?;
    let (p, s) = (&m.places, &m.system);
    let nix_env = needed(s.nix_env().as_ref(), system::NIX)?;
    let mut state = disk::read_state(p)?;
    let generations = boot::generations(s);
    let running = s.resolve(system::CURRENT);
    let Some((number, target, waited)) = way_back(m, &state, &generations, running.as_deref())
    else {
        say(
            3,
            "the profile has no generation to go back to: nothing was changed",
        );
        return Err(Refusal::of(Why::Failed));
    };

    // THE PROFILE, on that generation: none is made.
    if run::status(&nix_env, &argv::switch_generation(&s.profile(), number))? != Some(0) {
        say(3, "the system profile did not go back");
        return Err(Refusal::of(Why::Failed));
    }

    // THE TRIAL LEFT, the variables and the folder.
    let _ = disarm::after_rollback(m, &mut state);
    if waited {
        // Nothing waits any more: no system without a trial is coming.
        state.no_trial = None;
    }
    disk::write_state(p, &state)?;

    // THE SWITCH: none into the system that runs.
    let how = if now && !waited {
        Switch::Switch
    } else {
        Switch::Boot
    };
    let switch = s.real(&target).join("bin/switch-to-configuration");
    let ok = matches!(
        (how, run::status(&switch, &argv::switch(how))?),
        (Switch::Switch, Some(0 | 4)) | (Switch::Boot, Some(0))
    );
    if !ok {
        say(
            3,
            &format!("the profile is back on {target}, and its switch failed"),
        );
        return Err(Refusal::of(match how {
            Switch::Switch => Why::SwitchFailed,
            Switch::Boot => Why::NextBootFailed,
        }));
    }
    Ok(match (waited, how) {
        (true, _) => {
            format!("back on the system that runs ({target}); nothing waits for the next boot")
        }
        (false, Switch::Switch) => format!("back on generation {number} ({target}), now"),
        (false, Switch::Boot) => {
            format!("back on generation {number} ({target}) from the next boot")
        }
    })
}

/// Where the rollback goes: the generation's number, its toplevel, and
/// whether a system waited for the next boot (the way back is then the
/// system that runs); `None` when there is nowhere to go.
fn way_back(
    m: &Machine,
    state: &MachineState,
    generations: &[Generation],
    running: Option<&str>,
) -> Option<(u64, String, bool)> {
    let s = &m.system;
    let profile = s.resolve(system::PROFILE)?;
    let waits = running.is_some_and(|r| !boot::tops_of(s, &profile).iter().any(|t| t == r));
    if waits {
        // THE SYSTEM THAT RUNS, by its generation's toplevel (it may run a
        // specialisation of it).
        let top = trial::generation_toplevel(m, running?);
        let number = boot::generation_of_toplevel(generations, &top)?;
        return Some((number, top, true));
    }
    let current = profile_generation(m)?;
    let failed = state
        .trial
        .as_ref()
        .filter(|t| t.outcome == TrialOutcome::Failed)
        .map(|t| t.toplevel.as_str());
    generations
        .iter()
        .rev()
        .filter(|g| g.number < current)
        .find_map(|g| {
            let top = g.toplevel.as_deref()?;
            (top != profile && Some(top) != failed).then(|| (g.number, top.to_owned(), false))
        })
}

/// The number of the generation the system profile names
/// (`system-<n>-link`).
fn profile_generation(m: &Machine) -> Option<u64> {
    let link = std::fs::read_link(m.system.profile()).ok()?;
    link.file_name()?
        .to_str()?
        .strip_prefix("system-")?
        .strip_suffix("-link")?
        .parse()
        .ok()
}
