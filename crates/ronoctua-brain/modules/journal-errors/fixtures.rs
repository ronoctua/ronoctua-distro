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
