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
//!   the one that runs): back to the system that RUNS. Whatever waited is
//!   dropped, and nothing that runs is switched: `boot` or `switch` alike
//!   only write the menu for it.
//! - OTHERWISE, the system of the generation before the profile's, as
//!   `nix-env --rollback` would — passing over the system whose trial
//!   failed, and every generation from the first that holds the profile's
//!   own system onwards (below). `switch` activates it now, `boot` from the
//!   next boot.
//!
//! # By a new generation, a copy
//!
//! The profile goes back by a NEW generation, a copy of the system gone back
//! to (`nix-env --set`, as the bless's way back does), never by moving onto
//! the old one (`--switch-generation`): that one stays OLDER than the one
//! undone, and a menu that boots the newest generation — lanzaboote's
//! `loader.conf` says `default nixos-*` — would boot the undone system again
//! at the next restart.
//!
//! So a generation may be a copy — a rollback's, or the bless's — and its
//! way back is its ORIGINAL's: the way back is chosen among the generations
//! older than the first that holds the profile's system. Two rollbacks walk
//! back (1 A, 2 B, 3 C: the first makes 4 B, the second 5 A, never C), a
//! copy of a copy goes back from the first of them, and a system set for
//! the next boot after the original and dropped, which never ran, is never
//! a way back. An apply that built a system again, the very toplevel of an
//! older generation, cannot be told from a copy: the generations between
//! are passed over too — a way back lost, never a wrong one.
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
    let Some((target, waited)) = way_back(m, &state, &generations, running.as_deref()) else {
        say(
            3,
            "the profile has no generation to go back to: nothing was changed",
        );
        return Err(Refusal::of(Why::Failed));
    };

    // THE PROFILE, on a new generation that is a copy of that system: the
    // newest, so a menu that boots the newest boots it too.
    if run::status(&nix_env, &argv::set_profile(&s.profile(), &target))? != Some(0) {
        say(3, "the system profile did not go back");
        return Err(Refusal::of(Why::Failed));
    }
    let on = profile_generation(m)
        .map_or_else(|| target.clone(), |n| format!("{target} as generation {n}"));

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
    if ok {
        trial::bless_way_back(m, &target);
    } else {
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
            format!("back on the system that runs ({on}); nothing waits for the next boot")
        }
        (false, Switch::Switch) => format!("back on {on}, now"),
        (false, Switch::Boot) => format!("back on {on} from the next boot"),
    })
}

/// Where the rollback goes: the toplevel, and whether a system waited for
/// the next boot (the way back is then the system that runs); `None` when
/// there is nowhere to go.
fn way_back(
    m: &Machine,
    state: &MachineState,
    generations: &[Generation],
    running: Option<&str>,
) -> Option<(String, bool)> {
    let s = &m.system;
    let profile = s.resolve(system::PROFILE)?;
    let waits = running.is_some_and(|r| !boot::tops_of(s, &profile).iter().any(|t| t == r));
    if waits {
        // THE SYSTEM THAT RUNS, by its generation's toplevel (it may run a
        // specialisation of it, and the profile takes the whole system).
        let top = trial::generation_toplevel(m, running?);
        return boot::generation_of_toplevel(generations, &top).map(|_| (top, true));
    }
    let current = profile_generation(m)?;
    // THE ORIGINAL: the first generation that holds the profile's system —
    // the profile's own, unless that is a copy.
    let original = generations
        .iter()
        .filter(|g| g.number <= current && g.toplevel.as_deref() == Some(profile.as_str()))
        .map(|g| g.number)
        .min()
        .unwrap_or(current);
    let failed = state
        .trial
        .as_ref()
        .filter(|t| t.outcome == TrialOutcome::Failed)
        .map(|t| t.toplevel.as_str());
    generations
        .iter()
        .rev()
        .filter(|g| g.number < original)
        .find_map(|g| {
            let top = g.toplevel.as_deref()?;
            (Some(top) != failed).then(|| (top.to_owned(), false))
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
