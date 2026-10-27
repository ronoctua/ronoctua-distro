//! The security centre's boundaries.
//!
//! # The sudo door
//!
//! There is no polkit agent in the session, and there will not be one. When
//! a person's gesture needs root (open the passwordless window, loosen the
//! firewall, enrol a finger, the file manager's root helper), the door
//! (feature `authority`, the shell's only) first tries `sudo -n` (an open
//! window, sudo's own timestamp), then either pipes a typed password to
//! `sudo -S -k -p "" -- <store>/ronoctua-security-root <verb>`
//! ([`authority::run`]) or talks to sudo on a terminal of its own, where a
//! security key or a finger may answer before the password
//! ([`authority::converse`]). It reuses the authority the person already
//! has, adds none, and sudo logs every use. The verbs are a closed set
//! ([`verb::RootVerb`]); the helper refuses uid 0 in `SUDO_UID`.
//!
//! # The root helper
//!
//! `[[bin]] ronoctua-security-root` (`src/bin/root.rs`, extracted by
//! `nix/rust.nix` as `securityRoot` so the units that run it do not carry the
//! desktop). Through the door it runs a [`verb::RootVerb`] for the person
//! named by `SUDO_UID`; from a unit or NetworkManager's dispatcher it runs a
//! [`verb::UnitVerb`] and nothing else. It answers on its standard output in
//! [`verb::Said`] lines, the last of which is the verdict.
//!
//! # The person's firewall
//!
//! The distro is on nftables. The person's table `inet ronoctua` decides and
//! MARKS packets; the declared `nixos-fw` obeys the marks ([`firewall`]).
//! Tightening is free (shields, by a narrow polkit YES on one unit);
//! loosening goes through the sudo door.
//!
//! # The passwordless window
//!
//! `/var/lib/ronoctua-sudo/relax`, one line — with sudo's own `NOTAFTER`
//! for a timed window, none for no limit — that survives a reboot and ends
//! at its end or by the person's gesture: a timer by the wall clock, and the
//! boot's look at it ([`relax`]).
//!
//! # Security keys
//!
//! Root's list of the keys `pam_u2f` trusts for sudo, and registering one by
//! a touch, as the person ([`u2f`]).
//!
//! # Facts
//!
//! Fingerprint readers and enrolled fingers ([`fprint`]); Secure Boot, the
//! TPM and fwupd's HSI level and updates, read only ([`platform`]); what
//! each channel does against a quantum computer ([`quantum`]: a pure table
//! over facts, what `ssh -G` says it would offer, by store path, and what
//! the build wrote).

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[cfg(feature = "authority")]
pub mod authority;
pub mod firewall;
pub mod fprint;
pub mod platform;
pub mod quantum;
pub mod relax;
pub mod shields;
pub mod tools;
pub mod u2f;
pub mod users;
pub mod verb;
pub mod vpn;

/// The variable naming the root helper's store path — the session's
/// (`nix/modules/security.nix`), read by the sudo door (`authority`) to run
/// it and by [`has_helper`] to say whether it is there.
pub const HELPER_VARIABLE: &str = "RONOCTUA_SECURITY_ROOT";

/// Whether the root helper exists where the session says — for a door that
/// should say so before asking for a password. It reads a variable and asks
/// whether a file is there: nothing runs, and no sudo door is linked, so a
/// view that only asks this needs no `authority`.
#[must_use]
pub fn has_helper() -> bool {
    std::env::var_os(HELPER_VARIABLE)
        .map(std::path::PathBuf::from)
        .is_some_and(|p| p.is_absolute() && p.is_file())
}

/// Why the centre could not do or read something, as a closed set
/// (`security-refusal-<id>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// This process was not given the root helper.
    NoHelper,
    /// sudo refused the password.
    WrongPassword,
    /// sudo wants a password and none was typed (no open window).
    NeedsPassword,
    /// The person is not allowed to do it: sudo says so, or the helper was
    /// not reached the way the verb must be (as root, through the door or
    /// from a unit).
    NotAllowed,
    /// A program could not be started.
    Unstartable,
    /// A program ended with a failure.
    Failed,
    /// What was handed over is not what it must be (a wish, a user name).
    Invalid,
    /// There is no fingerprint reader.
    NoReader,
    /// A fact could not be read.
    Unreadable,
    /// The fingerprint reader is claimed by another program.
    Busy,
    /// Nothing happened for too long (no finger on the reader).
    TimedOut,
    /// The person cancelled, and nothing was done.
    Cancelled,
    /// No security key is plugged in.
    NoKey,
    /// A window asked for root and the footer, whose sudo bar is where every
    /// root gesture of a screen is asked, did not answer (it is not running,
    /// or went away before it said how it ended).
    NoFooter,
}

impl Why {
    /// Every one, in the order the keys are written.
    pub const ALL: [Self; 14] = [
        Self::NoHelper,
        Self::WrongPassword,
        Self::NeedsPassword,
        Self::NotAllowed,
        Self::Unstartable,
        Self::Failed,
        Self::Invalid,
        Self::NoReader,
        Self::Unreadable,
        Self::Busy,
        Self::TimedOut,
        Self::Cancelled,
        Self::NoKey,
        Self::NoFooter,
    ];

    /// Its kebab-case name, the suffix of its key.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoHelper => "no-helper",
            Self::WrongPassword => "wrong-password",
            Self::NeedsPassword => "needs-password",
            Self::NotAllowed => "not-allowed",
            Self::Unstartable => "unstartable",
            Self::Failed => "failed",
            Self::Invalid => "invalid",
            Self::NoReader => "no-reader",
            Self::Unreadable => "unreadable",
            Self::Busy => "busy",
            Self::TimedOut => "timed-out",
            Self::Cancelled => "cancelled",
            Self::NoKey => "no-key",
            Self::NoFooter => "no-footer",
        }
    }

    /// The reason of this name; `None` for any other.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

/// A refusal of the centre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Refusal {
    /// Why.
    pub why: Why,
}

pub(crate) const fn refused(why: Why) -> Refusal {
    Refusal { why }
}

/// Seconds since the epoch, now.
pub(crate) fn now_unix_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Writes `bytes` to `path` whole: a draft beside it (a dot name, so sudo's
/// `@includedir` and anything else that lists the folder skips it), created
/// with `mode` already on, synced, and renamed over the old file.
pub(crate) fn write_whole(path: &std::path::Path, bytes: &[u8], mode: u32) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| std::io::Error::other("a path with no file name"))?;
    let draft = path.with_file_name(format!(".{name}.{}", std::process::id()));
    match std::fs::remove_file(&draft) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let written = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&draft)?;
        // The umask may have taken bits off; the mode is the one asked for,
        // set before a byte is in the file.
        file.set_permissions(std::os::unix::fs::PermissionsExt::from_mode(mode))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&draft, path)
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&draft);
    }
    written
}

#[cfg(test)]
pub(crate) mod scratch {
    //! Folders for the tests, each its own.

    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);

    /// A fresh, empty folder under the temporary directory, removed when
    /// dropped.
    pub struct Scratch(pub PathBuf);

    impl Scratch {
        pub fn new(what: &str) -> Self {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-security-{what}-{}-{n}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        /// A program here, written with `text` and made executable — with
        /// no child forked meanwhile (`crate::tools::FORKS`).
        pub fn program(&self, name: &str, text: &str) -> PathBuf {
            use std::os::unix::fs::PermissionsExt as _;
            let _alone = crate::tools::FORKS
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let path = self.0.join(name);
            std::fs::write(&path, text).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reason_has_one_name() {
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
        assert_eq!(Why::of_id("nope"), None);
    }

    #[test]
    fn a_whole_write_leaves_no_draft_and_the_mode_it_was_given() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = scratch::Scratch::new("whole");
        let path = dir.0.join("file");
        write_whole(&path, b"one", 0o640).unwrap();
        write_whole(&path, b"two", 0o640).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
        let names: Vec<_> = std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["file"], "the draft is gone");
    }
}
