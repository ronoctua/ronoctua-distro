//! How the realizer exits, as systemd reads it.
//!
//! `ronoctua-part@.service` restarts on failure, ever later
//! (`RestartSteps=`, `RestartMaxDelaySec=`). Some endings must NOT be
//! retried on a clock: the part is not wanted, or the machine has no link and
//! the network coming up starts it again. Those exit with their own status,
//! and the unit's `RestartPreventExitStatus=` lists exactly [`NOT_RETRIED`]
//! (`parts-exit-codes-agree`).

/// Here: the wanted output is the profile's.
pub const DONE: u8 = 0;
/// A try that failed: systemd tries again, later each time.
pub const FAILED: u8 = 1;
/// A command line that is not one of the realizer's.
pub const USAGE: u8 = 2;
/// The plan does not want this part.
pub const UNWANTED: u8 = 10;
/// No link: the NetworkManager dispatcher starts the planner when one comes.
pub const OFFLINE: u8 = 11;
/// Root's lock of the records was not taken in time.
pub const BUSY: u8 = 12;

/// The statuses systemd must not answer with a restart.
pub const NOT_RETRIED: [u8; 4] = [USAGE, UNWANTED, OFFLINE, BUSY];

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a failed try is among the statuses never retried — the
    /// backoff would never run — or two endings share a status.
    #[test]
    fn a_failure_is_retried_and_the_rest_is_not() {
        assert!(!NOT_RETRIED.contains(&FAILED));
        assert!(!NOT_RETRIED.contains(&DONE));
        let mut all = vec![DONE, FAILED, USAGE, UNWANTED, OFFLINE, BUSY];
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 6);
    }
}
