//! `disarm`: the system profile was rolled back by hand, and the boot trial
//! must not take the machine back to what it left. The rescue console's
//! rollback is [`crate::rollback`], which does this itself; the verb stays
//! for a profile moved by anything else.
//!
//! A trial armed by an apply is two EFI variables: `LoaderEntryOneShot` on
//! the new system and `LoaderEntryDefault` on the last good one. Both outlive
//! a rollback — `nix-env --rollback` and `switch-to-configuration` write the
//! profile and `loader.conf`, never the variables — so without this the next
//! boot would still try the system the person rolled away from, and every
//! boot after it would come up on the default. So:
//!
//! - a trial the profile no longer names was LEFT by the person, and is
//!   recorded `failed`: the next apply does not switch into it again
//!   (`did-not-boot`) before the distro or `nixpkgs` moves on (a trial the
//!   profile still names keeps its way back, and nothing else is done);
//! - both variables are unset: `loader.conf`, which the rollback wrote, is
//!   the boot loader's choice again;
//! - the machine directory gets back the lock of the system the profile now
//!   names, when it is known, so a rebuild builds what the rollback chose.

use ronoctua_person::state::{MachineState, Trial, TrialOutcome};

use crate::loader::Loader;
use crate::system;
use crate::{Machine, Refusal, boot, boots, disk, guard, say, trial};

/// `disarm`.
pub(crate) fn disarm(m: &Machine) -> Result<(), Refusal> {
    let _lock = guard(m)?;
    let mut state = disk::read_state(&m.places)?;
    match after_rollback(m, &mut state) {
        Disarmed::KeptArmed => Ok(()),
        Disarmed::Done { left } => {
            if left {
                disk::write_state(&m.places, &state)?;
            }
            Ok(())
        }
    }
}

/// What [`after_rollback`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Disarmed {
    /// The profile still names the system on trial: its way back stays, and
    /// nothing else was done.
    KeptArmed,
    /// The variables are unset and the folder describes what the profile
    /// names; `left` when a pending trial was recorded `failed`.
    Done {
        /// Whether the record changed.
        left: bool,
    },
}

/// What a profile moved by hand leaves to undo, root's lock held: the trial
/// it left recorded `failed`, the boot loader's variables unset, and the
/// lock of what the profile names put back in the folder.
pub(crate) fn after_rollback(m: &Machine, state: &mut MachineState) -> Disarmed {
    let s = &m.system;

    // THE TRIAL the profile no longer names was left; one it still names
    // keeps its way back.
    let profile = s.resolve(system::PROFILE);
    let left = match trial::pending(state) {
        Some(t) => {
            let named = profile
                .as_deref()
                .is_some_and(|pr| boot::tops_of(s, &t.toplevel).iter().any(|top| top == pr));
            if named {
                say(
                    6,
                    "the profile still names the system on trial: its boot trial stays armed",
                );
                return Disarmed::KeptArmed;
            }
            say(
                4,
                &format!(
                    "the trial {} was rolled away from: it is not tried again",
                    t.toplevel
                ),
            );
            state.trial = Some(Trial {
                outcome: TrialOutcome::Failed,
                ..t
            });
            boots::forget(m);
            crate::sleep::forget(m);
            true
        }
        None => false,
    };

    // THE VARIABLES, whatever armed them.
    match Loader::of(s) {
        Some(loader) => {
            if loader.entries().is_some() {
                loader.clear();
            }
        }
        None => say(
            4,
            &format!(
                "neither {} nor {} is set: the boot loader's variables are left as they are",
                system::BOOTCTL,
                system::GRUB_EDITENV
            ),
        ),
    }

    // THE FOLDER describes what the profile names again.
    match profile
        .as_deref()
        .and_then(|pr| trial::lock_for(m, state, pr))
    {
        Some(lock) => trial::put_back(m, &lock),
        None => say(
            4,
            "the lock of the system the profile names is not known: the folder keeps its own",
        ),
    }
    say(6, "disarmed: the next boot is the one the boot menu names");
    Disarmed::Done { left }
}
