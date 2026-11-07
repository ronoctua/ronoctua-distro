//! What a machine writes and nobody has to read: the snippets a fresh install
//! filters, each with the sentence that says why it may.
//!
//! # A first draft of the person's list, and nothing more
//!
//! Each one is a filter like one the person typed ([`super::rules::Filter`]):
//! on the page, counting what it holds, and a tap turns it off. A line that is
//! a real failure is never one of them, whatever it looks like at a glance;
//! the tests hold this list against a table of such lines. A snippet is long
//! enough to name one line, because a short one would swallow the next
//! failure that happens to share two words with it.
//!
//! # Where each came from
//!
//! Read in a real journal, word for word, or in the strings of the program
//! that writes it. The few written from memory say `NOT SEEN HERE`: a wrong
//! word in one of those only means it never catches anything.
//!
//! The FIRST is the one that shipped alone before the others came, and its
//! words do not change: a person's file may already turn it off by them.

use super::super::key;

/// One snippet a fresh install filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shipped {
    /// The snippet: a literal piece of the line, as `<who>: <message>` reads.
    pub text: &'static str,
    /// The key of the sentence that says why it may be held.
    pub reason: &'static str,
}

const fn shipped(text: &'static str, reason: &'static str) -> Shipped {
    Shipped { text, reason }
}

/// The snippets, in the order they are tried.
pub const SHIPPED: &[Shipped] = &[
    shipped(
        "Ignoring duplicate name",
        key::JOURNAL_ERRORS_REASON_DUPLICATE_NAME,
    ),
    shipped(
        "Activation request for 'org.freedesktop.nm_dispatcher' failed.",
        key::JOURNAL_ERRORS_REASON_STOPPING_ECHO,
    ),
    shipped(
        "Activation request for 'org.bluez' failed.",
        key::JOURNAL_ERRORS_REASON_STOPPING_ECHO,
    ),
    shipped(
        "to send method call /midi/profile",
        key::JOURNAL_ERRORS_REASON_MIDI_PROFILE,
    ),
    shipped(
        "Expandable variables must be wrapped in {}",
        key::JOURNAL_ERRORS_REASON_PAM_ENV,
    ),
    shipped(
        "Returned feature report did not match the request",
        key::JOURNAL_ERRORS_REASON_PROBE,
    ),
    shipped(
        "watchdog did not stop!",
        key::JOURNAL_ERRORS_REASON_WATCHDOG_CLOSE,
    ),
    shipped(
        "CONFIG_IMA_DISABLE_HTABLE is disabled",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "v4l2loopback: loading out-of-tree module taints kernel",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "Cannot enable x2AVIC, AVIC is unsupported",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "command is advertised, but not supported",
        key::JOURNAL_ERRORS_REASON_BLUETOOTH_OPTIONAL,
    ),
    shipped(
        "i8042: Warning: Keylock active",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "Failed to set default system config for hci",
        key::JOURNAL_ERRORS_REASON_BLUETOOTH_OPTIONAL,
    ),
    shipped(
        "ACL packet for unknown connection handle",
        key::JOURNAL_ERRORS_REASON_BLUETOOTH_RACE,
    ),
    shipped(
        "i2c_dw_handle_tx_abort: lost arbitration",
        key::JOURNAL_ERRORS_REASON_BUS_CLASH,
    ),
    shipped(
        "Path Mic ACP LED is not a volume or mute control",
        key::JOURNAL_ERRORS_REASON_PROBE,
    ),
    shipped(
        "using unchecked data buffer",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "uses wireless extensions which will stop working",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "hrtimer: interrupt took",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "No Caching mode page found",
        key::JOURNAL_ERRORS_REASON_PROBE,
    ),
    shipped(
        "Assuming drive cache: write through",
        key::JOURNAL_ERRORS_REASON_PROBE,
    ),
    shipped(
        "not a multiple of preferred minimum block size",
        key::JOURNAL_ERRORS_REASON_PROBE,
    ),
    shipped(
        "due to active swapfile",
        key::JOURNAL_ERRORS_REASON_SWAPFILE,
    ),
    shipped(
        "Failed to adjust io pressure threshold, ignoring",
        key::JOURNAL_ERRORS_REASON_SAYS_IGNORING,
    ),
    shipped(
        "proceeding without lock: No such device or address",
        key::JOURNAL_ERRORS_REASON_TMPFILES_SOCKET,
    ),
    shipped(
        "/start is destructive (",
        key::JOURNAL_ERRORS_REASON_STOPPING_JOB,
    ),
    shipped(
        "/use_tempaddr', ignoring: No such file or directory",
        key::JOURNAL_ERRORS_REASON_INTERFACE_RENAMED,
    ),
    shipped(
        "/proxy_arp', ignoring: No such file or directory",
        key::JOURNAL_ERRORS_REASON_INTERFACE_RENAMED,
    ),
    shipped(
        "linklocal6: failed to get interface identifier; IPv6 cannot continue",
        key::JOURNAL_ERRORS_REASON_VPN_NO_ADDRESS,
    ),
    shipped("device (p2p-dev-", key::JOURNAL_ERRORS_REASON_P2P_UNUSED),
    shipped(
        "systemd-resolved.service: Using degraded feature set",
        key::JOURNAL_ERRORS_REASON_DNS_FALLBACK,
    ),
    shipped(
        "Can't connect to a11y bus",
        key::JOURNAL_ERRORS_REASON_NO_ACCESSIBILITY,
    ),
    shipped(
        "Realtime error: Could not get pidns for pid",
        key::JOURNAL_ERRORS_REASON_PORTAL_NAMESPACE,
    ),
    shipped(
        "Couldn't find existing drive object",
        key::JOURNAL_ERRORS_REASON_DEVICE_GONE,
    ),
    shipped(
        "libudev: received NULL device",
        key::JOURNAL_ERRORS_REASON_DEVICE_GONE,
    ),
    shipped(
        "systemd-logind.service: Failed to open /dev/input",
        key::JOURNAL_ERRORS_REASON_DEVICE_GONE,
    ),
    shipped(
        ": leaked proxy ",
        key::JOURNAL_ERRORS_REASON_PIPEWIRE_CLIENT,
    ),
    shipped(
        "See \"systemctl status",
        key::JOURNAL_ERRORS_REASON_SYSTEMCTL_ECHO,
    ),
    shipped(
        "failed because the control process exited with error code",
        key::JOURNAL_ERRORS_REASON_SYSTEMCTL_ECHO,
    ),
    shipped(
        "failed because a timeout was exceeded",
        key::JOURNAL_ERRORS_REASON_SYSTEMCTL_ECHO,
    ),
    shipped(
        "failed because a fatal signal was delivered",
        key::JOURNAL_ERRORS_REASON_SYSTEMCTL_ECHO,
    ),
    shipped(
        "ronoctua-git-clone-made-",
        key::JOURNAL_ERRORS_REASON_OWN_TEST,
    ),
    shipped("(udisks-test-hel)", key::JOURNAL_ERRORS_REASON_BUILD_TEST),
    shipped(
        "Error converting subject to JS object",
        key::JOURNAL_ERRORS_REASON_POLKIT_SUBJECT,
    ),
    shipped(
        "Couldn't initialize supplicant interface: Name owner lost",
        key::JOURNAL_ERRORS_REASON_STOPPING_ECHO,
    ),
    shipped(
        "lxcfs.service: Failed with result 'exit-code'",
        key::JOURNAL_ERRORS_REASON_LXCFS_STOP,
    ),
    shipped(
        " : a password is required ;",
        key::JOURNAL_ERRORS_REASON_SUDO_PROBE,
    ),
    shipped(
        ": Failed to process device, ignoring: Device or resource busy",
        key::JOURNAL_ERRORS_REASON_SAYS_IGNORING,
    ),
    shipped(
        "Set device flags return status: Invalid Parameters",
        key::JOURNAL_ERRORS_REASON_BLUETOOTH_OPTIONAL,
    ),
    shipped(
        "'s is in the view",
        key::JOURNAL_ERRORS_REASON_PARTS_OVERLAP,
    ),
    shipped(
        "is non-zero when fini",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    shipped(
        "is not a valid version, accepting anyway",
        key::JOURNAL_ERRORS_REASON_BOOT_VERSION,
    ),
    shipped(
        "BAP requires ISO Socket",
        key::JOURNAL_ERRORS_REASON_BLUETOOTH_OPTIONAL,
    ),
    shipped(
        "BlueZ system service is not available",
        key::JOURNAL_ERRORS_REASON_NO_BLUETOOTH,
    ),
    shipped(
        "could not set nice-level",
        key::JOURNAL_ERRORS_REASON_PIPEWIRE_PRIORITY,
    ),
    // NOT SEEN HERE
    shipped(
        "TPM interrupt not working, polling instead",
        key::JOURNAL_ERRORS_REASON_KERNEL_NOTE,
    ),
    // NOT SEEN HERE
    shipped(
        "ACPI BIOS Error (bug)",
        key::JOURNAL_ERRORS_REASON_FIRMWARE_TABLES,
    ),
    // NOT SEEN HERE
    shipped(
        "ACPI Error: Aborting method",
        key::JOURNAL_ERRORS_REASON_FIRMWARE_TABLES,
    ),
    // NOT SEEN HERE
    shipped(
        "couldn't check support for device",
        key::JOURNAL_ERRORS_REASON_NOT_A_MODEM,
    ),
];
