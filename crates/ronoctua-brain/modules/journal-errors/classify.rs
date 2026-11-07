//! How grave a line is: what the journal's fields say first, and its text only
//! for what no field says.
//!
//! # Why fields first
//!
//! Because a field is what the program meant and a text is what a word
//! happens to look like. A unit that ended badly says so in `UNIT_RESULT` and
//! `JOB_RESULT`, a crash in its `MESSAGE_ID`, and neither needs reading.
//!
//! # Why two tables
//!
//! The syslog priority is all most lines have, and at `warning` it says little:
//! the kernel writes a failing disk and an old driver at the same level. So
//! a few sentences lift a warning to [`Severity::Notable`]. The kernel's own
//! ([`KERNEL_SAYS`]) are asked only of a line the kernel wrote, because the same
//! words elsewhere mean something else: X's `fatal I/O error` is a client
//! whose display went away, and no disk is involved. The ones that mean the
//! same whoever says them are in [`ANYONE_SAYS`].
//!
//! A text here is asked of a `<who>: <line>` already case folded, the one a
//! filter is asked first: what a filter holds never gets this far.

use ronoctua_journal::{ERR, INFO, Priority};

use crate::module::Entry;

use super::rules::Severity;

/// `MESSAGE_ID` of systemd-coredump's report of a crash.
const COREDUMP: &str = "fc2e22bc6ee647b6b90729ab34a250b1";

/// The mildest syslog priority that is still [`Severity::Critical`]: `crit`,
/// and `alert` and `emerg` above it.
const CRITICAL_UNTIL: Priority = ERR - 1;

/// What the kernel says when something is really failing. Asked only of a
/// line of `_TRANSPORT=kernel`, case folded.
const KERNEL_SAYS: &[&str] = &[
    // The disk.
    "i/o error",
    "critical medium error",
    "critical target error",
    "blk_update_request",
    "controller is down",
    // The file system.
    "csum failed",
    "checksum error",
    "parent transid",
    "corrupt",
    "forced readonly",
    "transaction aborted",
    "ext4-fs error",
    "remounting filesystem read-only",
    // The memory.
    "out of memory",
    "killed process",
    "page allocation failure",
    "slab extension vector",
    // The kernel itself. `: bug:` and not `bug:`, which `debug:` carries.
    "segfault at",
    "general protection fault",
    ": bug:",
    "oops:",
    "kernel bug",
    "call trace",
    "blocked for more than",
    "soft lockup",
    "hard lockup",
    "detected stalls",
    // The hardware.
    "mce:",
    "machine check",
    "[hardware error]",
    "edac",
    "aer: uncorrected",
    "critical temperature",
    "clock throttled",
    "gpu reset",
    "amdgpu_job_timedout",
];

/// What means the same whoever says it, case folded.
const ANYONE_SAYS: &[&str] = &[
    "no space left on device",
    "read-only file system",
    "corrupted or uncleanly shut down",
];

/// What a program writes at `alert` about a person and not about the machine:
/// sudo puts a mistyped password above a failing disk. It may still be someone
/// else at the keyboard, so it is worth a look, but the red is the machine's.
const A_PERSON_TYPED: &[&str] = &[
    "auth could not identify password",
    "incorrect password attempt",
];

/// How grave an entry is. `hay` is its `<who>: <line>`, case folded.
#[must_use]
pub fn severity(entry: &Entry, hay: &str) -> Severity {
    let field = |name: &str| entry.fields.get(name).map(String::as_str);
    // A CRASH IS CRITICAL WHATEVER ITS PRIORITY: systemd writes a unit's
    // `core-dump` result at `warning`, and that line is the crash. The line
    // before it (`Main process exited, code=dumped`) names no result, and
    // stays what its priority says.
    if field("MESSAGE_ID") == Some(COREDUMP) || field("UNIT_RESULT") == Some("core-dump") {
        return Severity::Critical;
    }
    let says = |table: &[&str]| table.iter().any(|needle| hay.contains(needle));
    let priority = entry.priority.unwrap_or(INFO);
    if priority <= CRITICAL_UNTIL {
        return if says(A_PERSON_TYPED) {
            Severity::Notable
        } else {
            Severity::Critical
        };
    }
    if priority == ERR {
        return Severity::Error;
    }
    let ended_badly =
        field("UNIT_RESULT").is_some() || field("JOB_RESULT").is_some_and(|r| r != "done");
    let kernel = field("_TRANSPORT") == Some("kernel");
    if ended_badly || (kernel && says(KERNEL_SAYS)) || says(ANYONE_SAYS) {
        Severity::Notable
    } else {
        Severity::Warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(priority: Priority, fields: &[(&str, &str)]) -> Entry {
        Entry {
            priority: Some(priority),
            fields: fields
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            ..Entry::default()
        }
    }

    /// A line of the kernel, as the page would ask about it.
    fn kernel(line: &str) -> Severity {
        severity(
            &entry(4, &[("_TRANSPORT", "kernel")]),
            &format!("kernel: {line}").to_lowercase(),
        )
    }

    /// A line of a program, as the page would ask about it.
    fn program(line: &str) -> Severity {
        severity(
            &entry(4, &[("_TRANSPORT", "stdout")]),
            &format!("a.service: {line}").to_lowercase(),
        )
    }

    #[test]
    fn the_syslog_priority_says_it_when_nothing_else_does() {
        for priority in 0..=2 {
            assert_eq!(
                severity(&entry(priority, &[]), ""),
                Severity::Critical,
                "the priority {priority}"
            );
        }
        assert_eq!(severity(&entry(ERR, &[]), ""), Severity::Error);
        assert_eq!(severity(&entry(4, &[]), ""), Severity::Warning);
    }

    #[test]
    fn a_mistyped_password_is_worth_a_look_and_is_no_machine_failing() {
        for line in [
            "sudo: pam_unix(sudo:auth): auth could not identify password for [someone]",
            "sudo: someone : 3 incorrect password attempts ; USER=root",
            "sudo: someone : 1 incorrect password attempt ; USER=root",
        ] {
            let entry = entry(1, &[]);
            assert_eq!(severity(&entry, line), Severity::Notable, "{line}");
        }
        let failing = entry(1, &[]);
        assert_eq!(
            severity(&failing, "kernel: no irq handler for 1.2"),
            Severity::Critical
        );
    }

    #[test]
    fn a_crash_is_critical_at_any_priority() {
        let report = entry(4, &[("MESSAGE_ID", COREDUMP)]);
        assert_eq!(severity(&report, ""), Severity::Critical);
        let result = entry(4, &[("UNIT_RESULT", "core-dump")]);
        assert_eq!(severity(&result, ""), Severity::Critical);
    }

    #[test]
    fn a_unit_that_ended_badly_is_worth_a_look() {
        let exited = entry(4, &[("UNIT_RESULT", "exit-code")]);
        assert_eq!(severity(&exited, ""), Severity::Notable);
        let job = entry(4, &[("JOB_RESULT", "timeout")]);
        assert_eq!(severity(&job, ""), Severity::Notable);
        // A JOB THAT WENT WELL says nothing is wrong, however it is written.
        let done = entry(4, &[("JOB_RESULT", "done")]);
        assert_eq!(severity(&done, ""), Severity::Warning);
    }

    #[test]
    fn x11_saying_fatal_io_error_is_not_a_disk() {
        // THE TRAP THE TABLE'S `_TRANSPORT` IS FOR: the same two words in the
        // kernel are a disk failing, and here a client whose display went away.
        let line = "XIO: fatal I/O error 11 (Resource temporarily unavailable) on X server :0";
        assert_eq!(program(line), Severity::Warning);
        assert_eq!(
            kernel("I/O error, dev nvme0n1, sector 1048576 op 0x1:(WRITE)"),
            Severity::Notable
        );
    }

    #[test]
    fn a_scrub_skipping_the_swapfile_is_not_a_disk_failing() {
        // With the shipped filter off this is what would be left of it.
        let line = "BTRFS warning (device nvme0n1p2): scrub: skipping a block group \
                    due to active swapfile";
        assert_eq!(kernel(line), Severity::Warning);
    }

    #[test]
    fn a_keylock_warning_is_not_a_lockup() {
        assert_eq!(kernel("i8042: Warning: Keylock active"), Severity::Warning);
        assert_eq!(
            kernel("watchdog: BUG: soft lockup - CPU#3 stuck for 22s!"),
            Severity::Notable
        );
    }

    #[test]
    fn code_dumped_stays_what_its_priority_says_and_the_result_is_the_crash() {
        // TWO LINES OF ONE CRASH, a row each until they fold: a program's
        // exit is not news by itself, the result that follows it is.
        let exited = entry(4, &[("_TRANSPORT", "journal"), ("CODE_FUNC", "x")]);
        let hay = "a.service: main process exited, code=dumped, status=11/segv";
        assert_eq!(severity(&exited, hay), Severity::Warning);
        let result = entry(4, &[("UNIT_RESULT", "core-dump")]);
        assert_eq!(
            severity(&result, "a.service: failed with result 'core-dump'."),
            Severity::Critical
        );
    }

    #[test]
    fn a_kernel_line_at_debug_is_not_a_kernel_bug() {
        assert_eq!(
            kernel("usb 1-2: debug: reading a descriptor twice"),
            Severity::Warning
        );
        assert_eq!(
            kernel("BUG: unable to handle page fault for address"),
            Severity::Notable
        );
    }

    #[test]
    fn what_means_the_same_from_anyone_is_asked_of_anyone() {
        assert_eq!(
            program("could not write the state: No space left on device"),
            Severity::Notable
        );
        assert_eq!(
            program("the journal file is corrupted or uncleanly shut down, renaming"),
            Severity::Notable
        );
    }
}
