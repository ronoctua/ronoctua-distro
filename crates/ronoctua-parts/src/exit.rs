//! How the realizer and the planner exit, as systemd reads it.
//!
//! A PART THAT DOES NOT ARRIVE IS NEVER A FAILED UNIT. `switch-to-configuration`
//! counts every unit that failed while it ran — one in `auto-restart`
//! included — and exits 4, and an update reads that as a switch that went
//! wrong: one missing download would have made every update after it say it
//! failed (measured in `parts-are-isolated`). So every ending the record
//! already says is a SUCCESS to systemd (`SuccessExitStatus=` lists exactly
//! [`SETTLED`], `parts-rule-is-narrow`), and the next try is the planner's,
//! at the time the record's backoff gives ([`crate::record::Record::due_at`]),
//! by a timer armed only while a part waits for it. Only a command line that
//! is not one ([`USAGE`]) fails the unit: that is a defect, and it is seen.

/// Here: the wanted output is the profile's.
pub const DONE: u8 = 0;
/// A try that failed; the record says why and when the next one is.
pub const FAILED: u8 = 1;
/// A command line that is not one of the realizer's.
pub const USAGE: u8 = 2;
/// The plan does not want this part.
pub const UNWANTED: u8 = 10;
/// No link: the NetworkManager dispatcher starts the planner when one comes.
pub const OFFLINE: u8 = 11;
/// Root's lock of the records was not taken in time.
pub const BUSY: u8 = 12;
/// A metered link: the part waits for another network, or the person's yes.
pub const METERED: u8 = 13;

/// The statuses that end a unit as a success: the record says the rest.
pub const SETTLED: [u8; 5] = [FAILED, UNWANTED, OFFLINE, BUSY, METERED];

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a command line that is not one ends quietly, or a failed
    /// try fails the unit — and with it every switch it happens during — or
    /// two endings share a status.
    #[test]
    fn only_a_wrong_command_line_fails_the_unit() {
        assert!(SETTLED.contains(&FAILED));
        assert!(!SETTLED.contains(&USAGE));
        assert!(!SETTLED.contains(&DONE));
        let mut all = vec![DONE, FAILED, USAGE, UNWANTED, OFFLINE, BUSY, METERED];
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), 7);
    }
}
