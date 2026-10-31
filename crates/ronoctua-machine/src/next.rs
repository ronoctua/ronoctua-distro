//! Whether a system waits for the next boot — what every reader of the
//! machine asks (the update's page and its restart, the software centre, the
//! parts' plan of the next boot, the binary cache's and the distro's lines).
//!
//! # From what lasts until the restart
//!
//! The answer is read off the two links that say it, and nothing else: the
//! system PROFILE names the system the next boot starts, `/run/current-system`
//! the one that runs. A system waits when the profile names one whose
//! generation the running system is not (neither its toplevel nor one of its
//! specialisations). Never from the record's last apply: a trial that failed
//! (the bless puts the profile back), a rollback by hand, or a later apply
//! that built nothing (a refusal, a failed build — the profile still names
//! what waits) would each make that answer wrong, one way or the other.
//!
//! The record says one thing more: whether the restart TRIES the system once
//! with the way back armed — its trial is pending — or boots it with no way
//! back but the menu (`no-trial`).

use std::path::Path;

use ronoctua_person::state::MachineState;

use crate::system::{self, Env, System};
use crate::{boot, trial};

/// A system set for the next boot that does not run yet ([`next_boot`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextBoot {
    /// Its toplevel, as the system names it (`/nix/store/…`): the folder
    /// whose `etc/ronoctua` holds its own reports.
    pub toplevel: String,
    /// Whether the restart tries it once with the way back armed (the
    /// record's trial is pending on it); `false` when no trial could be
    /// armed, and the restart boots it with no way back but the menu.
    pub guarded: bool,
}

/// The system the next boot starts, when it is not the one that runs — the
/// one the system profile names, when the running system is neither it nor
/// one of its specialisations; `None` when nothing waits, or when either
/// link leads nowhere (nothing is known to wait). `root` is where the
/// system's own paths are (`/`, or a test's or a photograph's folder);
/// `state` is the machine's record, which says whether the restart tries it
/// with the way back armed.
#[must_use]
pub fn next_boot(root: &Path, state: Option<&MachineState>) -> Option<NextBoot> {
    let s = System::from_env(&Env::of(&[(system::SYSROOT, &root.display().to_string())]));
    let profile = s.resolve(system::PROFILE)?;
    let running = s.resolve(system::CURRENT)?;
    if !profile.starts_with("/nix/store/") || boot::tops_of(&s, &profile).contains(&running) {
        return None;
    }
    let guarded = state
        .and_then(trial::pending)
        .is_some_and(|t| t.toplevel == profile);
    Some(NextBoot {
        toplevel: profile,
        guarded,
    })
}
