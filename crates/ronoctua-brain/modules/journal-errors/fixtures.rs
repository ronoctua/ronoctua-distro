//! Journal lines, as `journalctl --output=json` writes them, for the tests.
//!
//! WRITTEN BY HAND in the shape the journal has: every field a string (the
//! timestamps and the priority too), `__CURSOR`, `__REALTIME_TIMESTAMP` and
//! `_BOOT_ID` on every line; a system service's stderr (`_TRANSPORT=stdout`,
//! `_SYSTEMD_UNIT`), a person's service from their manager
//! (`_SYSTEMD_USER_UNIT` under `user@1000.service`), the kernel's own
//! (`_TRANSPORT=kernel`), and a line with no `PRIORITY` at all, which journald
//! counts as `info`. Rust constants and not files: the Nix build of the crate
//! sees only `.rs`.

/// The second of the first line, as the journal's clock wrote it.
pub const AT: i64 = 1_790_000_000;

/// nginx cannot take port 80.
pub const NGINX_BIND_80: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f01;b=0123456789abcdef0123456789abcdef;m=1b2c3d4e;t=63a1b2c3d4e50;x=aa11bb22cc33dd44","__REALTIME_TIMESTAMP":"1790000000000000","__MONOTONIC_TIMESTAMP":"455884110","_BOOT_ID":"0123456789abcdef0123456789abcdef","_MACHINE_ID":"fedcba9876543210fedcba9876543210","_HOSTNAME":"machine","_TRANSPORT":"stdout","PRIORITY":"3","SYSLOG_FACILITY":"3","SYSLOG_IDENTIFIER":"nginx","_PID":"2211","_UID":"60","_GID":"60","_COMM":"nginx","_SYSTEMD_CGROUP":"/system.slice/nginx.service","_SYSTEMD_UNIT":"nginx.service","_SYSTEMD_SLICE":"system.slice","_SYSTEMD_INVOCATION_ID":"5e6f708192a3b4c5d6e7f8091a2b3c4d","_STREAM_ID":"9a8b7c6d5e4f30211203f4e5d6c7b8a9","MESSAGE":"bind() to 0.0.0.0:80 failed (98: Address in use)"}"#;

/// And a minute later, port 443: the same kind, other numbers.
pub const NGINX_BIND_443: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f02;b=0123456789abcdef0123456789abcdef;m=1ee81a4e;t=63a1b2c4b9e50;x=aa11bb22cc33dd45","__REALTIME_TIMESTAMP":"1790000060000000","__MONOTONIC_TIMESTAMP":"515884110","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"stdout","PRIORITY":"3","SYSLOG_FACILITY":"3","SYSLOG_IDENTIFIER":"nginx","_PID":"2215","_COMM":"nginx","_SYSTEMD_UNIT":"nginx.service","_SYSTEMD_SLICE":"system.slice","MESSAGE":"bind() to 0.0.0.0:443 failed (98: Address in use)"}"#;

/// A person's own service, in their journal.
pub const USER_SYNC: &str = r#"{"__CURSOR":"s=8e3f2a1b4c5d6e7f8091a2b3c4d5e6f7;i=7a10;b=0123456789abcdef0123456789abcdef;m=1c3d4e5f;t=63a1b2c3e8000;x=bb22cc33dd44ee55","__REALTIME_TIMESTAMP":"1790000030000000","__MONOTONIC_TIMESTAMP":"485884110","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"stdout","PRIORITY":"3","SYSLOG_FACILITY":"3","SYSLOG_IDENTIFIER":"syncthing","_PID":"3120","_UID":"1000","_GID":"100","_COMM":"syncthing","_SYSTEMD_CGROUP":"/user.slice/user-1000.slice/user@1000.service/app.slice/syncthing.service","_SYSTEMD_UNIT":"user@1000.service","_SYSTEMD_USER_UNIT":"syncthing.service","_SYSTEMD_USER_SLICE":"app.slice","_SYSTEMD_OWNER_UID":"1000","MESSAGE":"Failed to create folder root directory: mkdir /home/person/Sync: permission denied"}"#;

/// The kernel says a disk refused a write.
pub const KERNEL_IO: &str = r#"{"__CURSOR":"s=5c1f0e2a9b7d4c6e8f0a1b2c3d4e5f60;i=3001;b=0123456789abcdef0123456789abcdef;m=1d4e5f60;t=63a1b2c3f0000;x=cc33dd44ee55ff66","__REALTIME_TIMESTAMP":"1790000040000000","__MONOTONIC_TIMESTAMP":"495884110","_BOOT_ID":"0123456789abcdef0123456789abcdef","_SOURCE_MONOTONIC_TIMESTAMP":"495883002","_TRANSPORT":"kernel","PRIORITY":"3","SYSLOG_FACILITY":"0","SYSLOG_IDENTIFIER":"kernel","MESSAGE":"I/O error, dev sda, sector 1048576 op 0x1:(WRITE) flags 0x8800 phys_seg 1 prio class 2"}"#;

/// A warning: not an error.
pub const WARNING: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f03;b=0123456789abcdef0123456789abcdef;m=1ee81a50;t=63a1b2c4b9e51;x=aa11bb22cc33dd46","__REALTIME_TIMESTAMP":"1790000061000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"stdout","PRIORITY":"4","SYSLOG_IDENTIFIER":"nginx","_SYSTEMD_UNIT":"nginx.service","MESSAGE":"low on worker connections"}"#;

/// A line that names no priority: journald's `info`.
pub const INFO_NO_PRIORITY: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f04;b=0123456789abcdef0123456789abcdef;m=1ee81a51;t=63a1b2c4b9e52;x=aa11bb22cc33dd47","__REALTIME_TIMESTAMP":"1790000062000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"journal","SYSLOG_IDENTIFIER":"backup","MESSAGE":"failed to reach the server, will try again"}"#;

/// `dbus-broker` on the system bus: a name clashing between two service
/// directories, at `err` — the noise the shipped filter catches.
pub const DBUS_SYSTEM: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f05;b=0123456789abcdef0123456789abcdef;m=1ee81a60;t=63a1b2c4b9e60;x=aa11bb22cc33dd48","__REALTIME_TIMESTAMP":"1790000100000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"stdout","PRIORITY":"3","SYSLOG_IDENTIFIER":"dbus-broker-launch","_COMM":"dbus-broker-lau","_SYSTEMD_UNIT":"dbus-broker.service","MESSAGE":"Ignoring duplicate name 'org.freedesktop.UPower' in service file '/nix/store/abc-upower/share/dbus-1/system-services/org.freedesktop.UPower.service'"}"#;

/// The same on the person's bus, under `user@1000.service` — a second kind by
/// `who`, the one snippet catches both.
pub const DBUS_USER: &str = r#"{"__CURSOR":"s=8e3f2a1b4c5d6e7f8091a2b3c4d5e6f7;i=7a20;b=0123456789abcdef0123456789abcdef;m=1ee81a61;t=63a1b2c4b9e61;x=bb22cc33dd44ee56","__REALTIME_TIMESTAMP":"1790000101000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"stdout","PRIORITY":"3","SYSLOG_IDENTIFIER":"dbus-broker-launch","_COMM":"dbus-broker-lau","_SYSTEMD_UNIT":"user@1000.service","MESSAGE":"Ignoring duplicate name 'org.freedesktop.portal.Flatpak' in service file '/nix/store/def-flatpak/share/dbus-1/services/org.freedesktop.portal.Flatpak.service'"}"#;

/// A coredump whose message `journalctl --output=json` sent as `null`, being
/// over 4096 bytes: what is left are its short fields.
pub const COREDUMP_NO_MESSAGE: &str = r#"{"__CURSOR":"s=5c1f0e2a9b7d4c6e8f0a1b2c3d4e5f60;i=3101;b=0123456789abcdef0123456789abcdef;m=1d4e5f70;t=63a1b2c3f0100;x=cc33dd44ee55ff77","__REALTIME_TIMESTAMP":"1790000200000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"journal","PRIORITY":"2","SYSLOG_IDENTIFIER":"systemd-coredump","_SYSTEMD_UNIT":"systemd-coredump@3-1742-0.service","MESSAGE_ID":"fc2e22bc6ee647b6b90729ab34a250b1","COREDUMP_COMM":"pipewire","COREDUMP_SIGNAL_NAME":"SIGSEGV","MESSAGE":null}"#;

/// The first of the two lines of a unit that fails, which the book folds into
/// one row: `X: Failed with result 'exit-code'.`, at `warning`, with its result.
pub const UNIT_RESULT: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f10;b=0123456789abcdef0123456789abcdef;m=1ee81a70;t=63a1b2c4b9e70;x=aa11bb22cc33dd50","__REALTIME_TIMESTAMP":"1790000300000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"journal","PRIORITY":"4","SYSLOG_IDENTIFIER":"systemd","_SYSTEMD_UNIT":"init.scope","UNIT":"made-up.service","MESSAGE_ID":"d9b373ed55a64feb8242e02dbe79a49c","UNIT_RESULT":"exit-code","MESSAGE":"made-up.service: Failed with result 'exit-code'."}"#;

/// `Failed to start X.`, at `err`, with its job's result: the same failure.
pub const UNIT_START_FAILED: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f11;b=0123456789abcdef0123456789abcdef;m=1ee81a71;t=63a1b2c4b9e71;x=aa11bb22cc33dd51","__REALTIME_TIMESTAMP":"1790000300000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"journal","PRIORITY":"3","SYSLOG_IDENTIFIER":"systemd","_SYSTEMD_UNIT":"init.scope","UNIT":"made-up.service","MESSAGE_ID":"be02cf6855d2428ba40df7e9d022f03d","JOB_RESULT":"failed","MESSAGE":"Failed to start a made-up service."}"#;

/// A scope that failed, which is named by its program and folds into nothing.
pub const SCOPE_RESULT: &str = r#"{"__CURSOR":"s=7d2e1f0a3b4c5d6e7f8091a2b3c4d5e6;i=2f12;b=0123456789abcdef0123456789abcdef;m=1ee81a72;t=63a1b2c4b9e72;x=aa11bb22cc33dd52","__REALTIME_TIMESTAMP":"1790000301000000","_BOOT_ID":"0123456789abcdef0123456789abcdef","_TRANSPORT":"journal","PRIORITY":"4","SYSLOG_IDENTIFIER":"systemd","_SYSTEMD_UNIT":"init.scope","UNIT":"session-2.scope","MESSAGE_ID":"d9b373ed55a64feb8242e02dbe79a49c","UNIT_RESULT":"resources","MESSAGE":"session-2.scope: Failed with result 'resources'."}"#;

/// What a filter ought to catch: one line for each of `noise::SHIPPED`, in its
/// order, each `(who, message)` as the journal has them. Each is a line read in
/// a journal, whole, with its numbers and names changed, and its `who` too where
/// the real one is another module's to name. Where nothing was read, it is what
/// the strings of the program that writes it say; and for a snippet that says
/// `NOT SEEN HERE`, the memory it was written from, which proves only that it
/// reads as itself.
pub const SHIPPED_SAMPLES: &[(&str, &str)] = &[
    (
        "dbus-broker.service",
        "Ignoring duplicate name 'org.example.Made' in service file '/nix/store/x/a.service'",
    ),
    (
        "dbus-broker.service",
        "Activation request for 'org.freedesktop.nm_dispatcher' failed.",
    ),
    (
        "dbus-broker.service",
        "Activation request for 'org.bluez' failed.",
    ),
    (
        "dbus-broker.service",
        "A security policy denied :1.6 to send method call /midi/profile:org.bluez.GattProfile1.Release to :1.49.",
    ),
    (
        "session-2.scope",
        "pam_env(sudo:session): Expandable variables must be wrapped in {} <$XDG_DATA_HOME/a-folder> - ignoring",
    ),
    (
        "kernel",
        "hid-multitouch 0018:2C2F:0027.0003: Returned feature report did not match the request",
    ),
    ("kernel", "watchdog: watchdog0: watchdog did not stop!"),
    (
        "kernel",
        "device-mapper: core: CONFIG_IMA_DISABLE_HTABLE is disabled. Duplicate IMA measurements will not be recorded in the IMA log.",
    ),
    (
        "kernel",
        "v4l2loopback: loading out-of-tree module taints kernel.",
    ),
    (
        "kernel",
        "kvm_amd: [Firmware Bug]: Cannot enable x2AVIC, AVIC is unsupported",
    ),
    (
        "kernel",
        "Bluetooth: hci0: HCI Enhanced Setup Synchronous Connection command is advertised, but not supported.",
    ),
    ("kernel", "i8042: Warning: Keylock active"),
    (
        "bluetooth.service",
        "Failed to set default system config for hci0",
    ),
    (
        "kernel",
        "Bluetooth: hci0: ACL packet for unknown connection handle 3837",
    ),
    (
        "kernel",
        "i2c_designware AMDI0010:01: i2c_dw_handle_tx_abort: lost arbitration",
    ),
    (
        "wireplumber.service",
        "spa.alsa: Path Mic ACP LED is not a volume or mute control",
    ),
    ("kernel", "nvme nvme0: using unchecked data buffer"),
    (
        "kernel",
        "warning: `a-program' uses wireless extensions which will stop working for Wi-Fi 7 hardware; use nl80211",
    ),
    ("kernel", "hrtimer: interrupt took 2204 ns"),
    ("kernel", "sd 0:0:0:0: [sda] No Caching mode page found"),
    (
        "kernel",
        "sd 0:0:0:0: [sda] Assuming drive cache: write through",
    ),
    (
        "kernel",
        "sd 0:0:0:0: [sda] Optimal transfer size 33553920 bytes not a multiple of preferred minimum block size (4096 bytes)",
    ),
    (
        "kernel",
        "BTRFS warning (device dm-0): scrub: skipping scrub of block group 1104150528 due to active swapfile",
    ),
    (
        "systemd",
        "Failed to adjust io pressure threshold, ignoring: Device or resource busy",
    ),
    (
        "systemd-tmpfiles-setup.service",
        "Opening file \"/tmp/.X11-unix/X0\" failed, proceeding without lock: No such device or address",
    ),
    (
        "sys-devices-a-card.device",
        "sys-devices-a-card.device: Failed to enqueue SYSTEMD_USER_WANTS job, ignoring: Transaction for sound.target/start is destructive (sound.target has 'stop' job queued, but 'start' is included in transaction).",
    ),
    (
        "systemd-sysctl.service",
        "Couldn't write '2' to 'net/ipv6/conf/wlp1s0/use_tempaddr', ignoring: No such file or directory",
    ),
    (
        "systemd-sysctl.service",
        "Couldn't write '0' to 'net/ipv4/conf/wlp1s0/proxy_arp', ignoring: No such file or directory",
    ),
    (
        "NetworkManager.service",
        "<warn>  [1793143829.2889] device (a-vpn): linklocal6: failed to get interface identifier; IPv6 cannot continue",
    ),
    (
        "NetworkManager.service",
        "<warn>  [1793983125.1107] device (p2p-dev-wlp1s0): error setting IPv4 forwarding to '0': Success",
    ),
    (
        "systemd-resolved.service",
        "Using degraded feature set UDP instead of UDP+EDNS0 for DNS server 192.0.2.1.",
    ),
    (
        "WebKitWebProces",
        "Can't connect to a11y bus: Could not connect: No such file or directory",
    ),
    (
        "xdg-desktop-portal.service",
        "Realtime error: Could not get pidns for pid 2211: Could not fstatat ns/pid: Not a directory",
    ),
    (
        "udisks2.service",
        "Couldn't find existing drive object for device /sys/devices/pci0000:00/0000:00:08.3/usb8/8-1/block/sda (uevent action 'change', VPD '___II____II_9___')",
    ),
    ("dhcpcd.service", "libudev: received NULL device"),
    (
        "systemd-logind.service",
        concat!("Failed to open /dev/input", "/event17: No such device"),
    ),
    (
        "wireplumber.service",
        "pw.core: 0x6265f1387be0: leaked proxy 0x6265f13cfe90 id:3",
    ),
    (
        "a-program",
        "See \"systemctl status a-unit.service\" and \"journalctl -xeu a-unit.service\" for details.",
    ),
    (
        "a-program",
        "Job for a-unit.service failed because the control process exited with error code.",
    ),
    (
        "a-program",
        "Job for a-unit.service failed because a timeout was exceeded.",
    ),
    (
        "a-program",
        "Job for a-unit.service failed because a fatal signal was delivered to the control process.",
    ),
    (
        "git-daemon",
        "'/tmp/nix-shell.6BABLQ/ronoctua-git-clone-made-1234/served/missing.git' does not appear to be a git repository",
    ),
    (
        "systemd-coredump@0-1-4242_4243-0.service",
        "Process 4242 (udisks-test-hel) of user 30001 terminated abnormally without generating a coredump.",
    ),
    (
        "polkit.service",
        "Error converting subject to JS object: Process 2211 terminated",
    ),
    (
        "NetworkManager.service",
        "<error> [1793033073.2469] device (wlp1s0): Couldn't initialize supplicant interface: Name owner lost",
    ),
    (
        "lxcfs.service",
        "lxcfs.service: Failed with result 'exit-code'.",
    ),
    (
        "session-2.scope",
        "  person : a password is required ; PWD=/home/person/a-project ; USER=root ; COMMAND=/run/current-system/sw/bin/true",
    ),
    (
        "systemd-udevd.service",
        "zram0: Failed to process device, ignoring: Device or resource busy",
    ),
    (
        "bluetooth.service",
        "src/device.c:set_wake_allowed_complete() Set device flags return status: Invalid Parameters",
    ),
    (
        "a-part-mount.service",
        "bin/a-program is offered by a-part and person-a183da58-a-part: a-part's is in the view",
    ),
    (
        "kernel",
        "amdgpu 0000:c4:00.0: amdgpu: VM memory stats for proc a-program(547411) task a-program:cs0(547400) is non-zero when fini",
    ),
    (
        "systemd-logind.service",
        "/boot/an-entry-folder/nixos-0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef.conf:2: Version string 'Generation 66 NixOS Zokor 26.11.20261027.31ecbac (Linux 6.18.59), built on 2026-11-03' is not a valid version, accepting anyway.",
    ),
    (
        "bluetooth.service",
        "src/a-file.c:a_function() BAP requires ISO Socket which is not enabled",
    ),
    (
        "pipewire.service",
        "spa.bluez5: BlueZ system service is not available",
    ),
    (
        "pipewire.service",
        "mod.rt: could not set nice-level to -11: Permission denied",
    ),
    (
        "kernel",
        "tpm_tis MSFT0101:00: TPM interrupt not working, polling instead",
    ),
    (
        "kernel",
        "ACPI BIOS Error (bug): Could not resolve symbol [\\_SB.PCI0]",
    ),
    (
        "kernel",
        "ACPI Error: Aborting method \\_SB.PCI0 due to previous error",
    ),
    (
        "ModemManager.service",
        "<warn>  [device /sys/devices/x] couldn't check support for device: not supported",
    ),
];

/// What no shipped filter may hold: real failures, however close a snippet
/// comes to them. `(who, message)`, with every `who` made up where the real
/// one is another module's to name.
pub const REAL_FAILURES: &[(&str, &str)] = &[
    // A CRASH of what the machine cannot work without.
    (
        "systemd-coredump",
        "Process 2211 (pipewire) of user 1000 dumped core.",
    ),
    (
        "systemd-coredump",
        "Process 2212 (scroll) of user 1000 terminated abnormally with signal 11/SEGV, processing...",
    ),
    (
        "scroll.service",
        "Main process exited, code=dumped, status=11/SEGV",
    ),
    (
        "scroll.service",
        "scroll.service: Failed with result 'core-dump'.",
    ),
    (
        "scroll.service",
        "scroll.service: Failed with result 'exit-code'.",
    ),
    (
        "keyd.service",
        "keyd.service: Failed with result 'exit-code'.",
    ),
    // THE MACHINE KEEPING ITSELF UP TO DATE, failing.
    (
        "a-nightly-update.service",
        "a-nightly-update.service: Failed with result 'exit-code'.",
    ),
    (
        "a-nightly-update.service",
        "Failed to start the update of this machine.",
    ),
    (
        "flatpak-managed-install.service",
        "flatpak-managed-install.service: Failed with result 'exit-code'.",
    ),
    (
        "flatpak-managed-install.service",
        "error: Failed to install org.example.App: Could not resolve hostname",
    ),
    // WHAT WAS CLOSE TO A SNIPPET and is a failure all the same.
    (
        "dbus-broker.service",
        "Activation request for 'org.bluez' failed: The systemd unit 'dbus-org.bluez.service' could not be found.",
    ),
    ("a-program", "Job for a-unit.service canceled."),
    // THE CRASH THAT COULD NOT BE TOLD.
    (
        "systemd-coredump",
        "Failed to send coredump datagram: Message too long",
    ),
    // KEYS, NETWORK, DISK.
    ("ssh", "sshkey_sign: invalid format"),
    (
        "dnsmasq",
        "error binding DHCP socket to device incusbr0: Address already in use",
    ),
    (
        "systemd-journald.service",
        "File /var/log/journal/a/system.journal corrupted or uncleanly shut down, renaming and replacing.",
    ),
    (
        "kernel",
        "I/O error, dev nvme0n1, sector 1048576 op 0x1:(WRITE) flags 0x8800 phys_seg 1 prio class 2",
    ),
    (
        "kernel",
        "BTRFS warning (device dm-0): csum failed root 5 ino 257 off 4096",
    ),
    // FIRMWARE.
    (
        "kernel",
        "iwlwifi 0000:01:00.0: Direct firmware load for iwlwifi-so-a0.ucode failed with error -2",
    ),
    (
        "kernel",
        "rtw89_8852ce 0000:01:00.0: failed to load firmware",
    ),
    (
        "kernel",
        "amdgpu 0000:c4:00.0: firmware: failed to load amdgpu/vcn.bin (-2)",
    ),
    // AUTHENTICATION.
    (
        "session-2.scope",
        "pam_unix(sudo:auth): authentication failure; logname=person uid=1000 euid=0 tty=/dev/pts/1",
    ),
    (
        "session-2.scope",
        "person : 3 incorrect password attempts ; TTY=pts/1 ; PWD=/ ; USER=root ; COMMAND=/run/a-program",
    ),
    (
        "sshd",
        "Failed password for person from 192.0.2.1 port 22 ssh2",
    ),
    // WHAT MAY GO EITHER WAY, and is a person's to judge.
    (
        "wireplumber.service",
        "Failed to create 'api.alsa.acp.device' device",
    ),
    (
        "pipewire.service",
        "Caught PipeWire error: connection closed",
    ),
    (
        "NetworkManager.service",
        "<warn>  deleted default route via 192.0.2.1",
    ),
    ("bluetooth.service", "Failed to add UUID: Busy"),
];
