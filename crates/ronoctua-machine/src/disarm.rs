//! `disarm`: the system profile was rolled back by hand (the rescue
//! console's rollback, `nix/modules/rescue.nix`), and the boot trial must
//! not take the machine back to what it left.
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
//!   (`did-not-boot`) before the distro moves on (a trial the profile still
//!   names keeps its way back, and nothing else is done);
//! - both variables are unset: `loader.conf`, which the rollback wrote, is
//!   the boot loader's choice again;
//! - the machine directory gets back the lock of the system the profile now
//!   names, when it is known, so a rebuild builds what the rollback chose.

use ronoctua_person::state::{Trial, TrialOutcome};

use crate::loader::Loader;
use crate::system;
use crate::{Machine, Refusal, boot, boots, disk, guard, say, trial};

/// `disarm`.
pub(crate) fn disarm(m: &Machine) -> Result<(), Refusal> {
    let _lock = guard(m)?;
    let (p, s) = (&m.places, &m.system);
    let mut state = disk::read_state(p)?;

    // THE TRIAL the profile no longer names was left; one it still names
    // keeps its way back.
    let profile = s.resolve(system::PROFILE);
    let left = match trial::pending(&state) {
        Some(t) => {
            let named = profile
                .as_deref()
                .is_some_and(|pr| boot::tops_of(s, &t.toplevel).iter().any(|top| top == pr));
            if named {
                say(
                    6,
                    "the profile still names the system on trial: its boot trial stays armed",
                );
                return Ok(());
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
        .and_then(|pr| trial::lock_for(m, &state, pr))
    {
        Some(lock) => trial::put_back(m, &lock),
        None => say(
            4,
            "the lock of the system the profile names is not known: the folder keeps its own",
        ),
    }
    if left {
        disk::write_state(p, &state)?;
    }
    say(6, "disarmed: the next boot is the one the boot menu names");
    Ok(())
}
