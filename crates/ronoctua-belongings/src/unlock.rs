//! The identity kept for the session (feature `prefs`; the keyring itself
//! behind feature `keyring`).
//!
//! # What is kept, and where
//!
//! A sync opens `key.age` with the passphrase the person typed — one scrypt —
//! and finds the IDENTITY inside (`AGE-SECRET-KEY-PQ-1…`). With
//! `keep_unlocked` (on by default) the CLI keeps that identity, never the
//! passphrase, in the kernel's keyring: a `user` key in the user keyring
//! (`@u`), described `ronoctua-belongings:<instance>:<16 hex of the
//! repository's SHA-256>`, with a kernel timeout of `unlockHours` ([`Vault`],
//! [`Kernel`]). The next sync finds it there and asks nothing. It never
//! touches a disk: not a file, not swap (the keyring is kernel memory).
//!
//! WHO CAN READ IT: every process of this user, for that window. Its
//! permissions give all to its possessor and a sight of it to the user, but
//! possession is no wall between two processes of the same uid — any of them
//! may link `@u` into its own session keyring (exactly what
//! `Kernel::possessed` does) and possess the key. What the keyring buys is
//! no file and no swap, and the timeout; other users never reach `@u`.
//!
//! # Doors read a marker, never the keyring
//!
//! The footer and the windows do not call `keyctl`. The CLI leaves a marker,
//! `runtime/unlocked` ([`Marker`]): which repository, and until when. A door
//! reads it ([`kept_for`]) to know whether a sync needs the passphrase; when
//! the marker lies (the kernel let the key go first), the sync is refused
//! for want of a passphrase and the door asks it — never a wrong answer,
//! at worst one question more.
//!
//! # It ends with the session
//!
//! At logout `ronoctua-belongings-forget.service` stops with the graphical
//! session and forgets (`ronoctua-belongings forget --everywhere`: every
//! instance's, the development loop's included); before the machine sleeps,
//! a system unit running as the person does the same; at most `unlockHours`
//! after it was typed, the kernel drops it by itself. A reboot empties the
//! keyring and the runtime folder alike. Turning `keep_unlocked` off forgets
//! it at the next run, and the queue's `forget` (a plain `forget` too)
//! forgets only THIS instance's ([`instance_prefix`], [`unmark_here`]): the
//! development loop never takes the installed session's.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::places::Places;

/// What every key the belongings keep is described as, before the instance.
pub const PREFIX: &str = "ronoctua-belongings:";

/// The marker's format.
pub const MARKER_FORMAT: u32 = 1;

/// How long the identity may stay in the kernel's keyring, in seconds (the
/// table's `unlockHours`).
pub const KEPT_SECONDS: u64 = crate::kept::UNLOCK_HOURS * 3600;

/// The repository, as a key's description and the marker name it: 16
/// hexadecimal characters of the SHA-256 of its address — the address itself
/// never sits in the keyring's listing. Pure.
#[must_use]
pub fn repository_tag(repository: &str) -> String {
    let sum = ronoctua_store::repair::sha256_hex(repository.trim().as_bytes());
    sum.chars().take(16).collect()
}

/// What every key THIS instance keeps (`ronoctua` or `ronoctua-dev`,
/// [`ronoctua_xdg::name`]) is described as, before the repository:
/// `ronoctua-belongings:<instance>:`. The colon ends the instance, so
/// `ronoctua` never covers `ronoctua-dev`.
#[must_use]
pub fn instance_prefix() -> String {
    format!("{PREFIX}{}:", ronoctua_xdg::name())
}

/// The description of the key that keeps the identity of `repository` for
/// this instance.
#[must_use]
pub fn description(repository: &str) -> String {
    format!("{}{}", instance_prefix(), repository_tag(repository))
}

/// `runtime/unlocked`: the identity of a repository is kept for this
/// session, until when. What a door reads instead of the keyring.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    /// [`MARKER_FORMAT`].
    pub format: u32,
    /// Which repository ([`repository_tag`]).
    pub repository: String,
    /// When the kernel lets it go at the latest, seconds since the epoch.
    pub until_s: i64,
}

impl Marker {
    /// The marker of `places`; `None` when there is none or it does not
    /// parse.
    #[must_use]
    pub fn read(places: &Places) -> Option<Self> {
        serde_json::from_slice(&std::fs::read(places.unlocked()).ok()?).ok()
    }

    /// Whether it says `repository` is kept at `now_s`. Pure.
    #[must_use]
    pub fn holds(&self, repository: &str, now_s: i64) -> bool {
        self.format == MARKER_FORMAT
            && self.repository == repository_tag(repository)
            && now_s < self.until_s
    }

    /// Writes it — `0600`, in the runtime folder.
    ///
    /// # Errors
    ///
    /// It could not be written.
    pub fn write(&self, places: &Places) -> std::io::Result<()> {
        let text = serde_json::to_vec(self).map_err(std::io::Error::other)?;
        crate::secret::write_private(&places.unlocked(), &text)
    }
}

/// Whether a sync of `repository` at `now_s` would find the identity kept —
/// what a door asks to know whether to ask the passphrase. A marker can say
/// yes after the kernel let the key go; the sync then refuses for want of a
/// passphrase, and the door asks it.
#[must_use]
pub fn kept_for(places: &Places, repository: &str, now_s: i64) -> bool {
    Marker::read(places).is_some_and(|m| m.holds(repository, now_s))
}

/// Deletes the marker of `places`.
pub fn unmark(places: &Places) {
    let _ = std::fs::remove_file(places.unlocked());
}

/// Deletes THIS instance's marker under the runtime folder `runtime` (the
/// user's `$XDG_RUNTIME_DIR`): `<instance>-belongings/unlocked`, and no other
/// instance's. Says whether there was one.
pub fn unmark_here(runtime: &Path) -> bool {
    let leaf = format!("{}-{}", ronoctua_xdg::name(), crate::places::RUNTIME_LEAF);
    std::fs::remove_file(runtime.join(leaf).join(crate::places::UNLOCKED_FILE)).is_ok()
}

/// Deletes the marker of EVERY instance under the runtime folder `runtime`
/// (`ronoctua-belongings/unlocked`, `ronoctua-dev-belongings/unlocked`…) —
/// what the end of a session or a sleep forgets. Says how many went.
pub fn unmark_everywhere(runtime: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(runtime) else {
        return 0;
    };
    // `<name>-belongings`, where the name is `ronoctua` or
    // `ronoctua-<instance>` (`ronoctua_xdg::runtime_file`).
    let leaf = format!("-{}", crate::places::RUNTIME_LEAF);
    entries
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(ronoctua_xdg::NAME) && n.ends_with(&leaf))
        })
        .filter(|e| std::fs::remove_file(e.path().join(crate::places::UNLOCKED_FILE)).is_ok())
        .count()
}

// ── anchor: L10 ──

/// Where the identity is kept: the kernel's keyring ([`Kernel`]) — or, in a
/// test, memory of its own.
#[cfg(feature = "keyring")]
pub trait Vault {
    /// Keeps `secret` under `description` for at most `seconds`. Says
    /// whether it is kept.
    fn keep(&self, description: &str, secret: &str, seconds: u64) -> bool;
    /// What is kept under `description`, if anything.
    fn take(&self, description: &str) -> Option<zeroize::Zeroizing<String>>;
    /// Forgets everything kept under a description that starts with
    /// `prefix`. Says how many went.
    fn forget(&self, prefix: &str) -> usize;
}

/// The kernel's keyring: the user keyring (`@u`) of this user, every
/// session of theirs alike — a test's own thread keyring instead.
#[cfg(feature = "keyring")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kernel {
    ring: linux_keyutils::KeyRingIdentifier,
}

/// The most links of a keyring read when forgetting.
#[cfg(feature = "keyring")]
const LINKS_MOST: usize = 512;

#[cfg(feature = "keyring")]
impl Kernel {
    /// The user keyring, `@u`.
    pub const USER: Self = Self {
        ring: linux_keyutils::KeyRingIdentifier::User,
    };

    /// This thread's own keyring — a test's, gone with the thread.
    #[cfg(test)]
    const fn thread() -> Self {
        Self {
            ring: linux_keyutils::KeyRingIdentifier::Thread,
        }
    }

    /// The keyring, POSSESSED by this process: a process possesses `@u` only
    /// through its session keyring. A session made by a service manager may
    /// not link it (one with none is given the user-session keyring, which
    /// does), so it is linked here — what a login session's `pam_keyinit`
    /// does for every process of the session, and what any process of the
    /// same user may do too (the module's header: who can read the key).
    fn possessed(self) -> Result<linux_keyutils::KeyRing, linux_keyutils::KeyError> {
        use linux_keyutils::{KeyRing, KeyRingIdentifier};
        let ring = KeyRing::from_special_id(self.ring, true)?;
        if self.ring == KeyRingIdentifier::User {
            let session = KeyRing::from_special_id(KeyRingIdentifier::Session, false)?;
            session.link_keyring(ring)?;
        }
        Ok(ring)
    }
}

#[cfg(feature = "keyring")]
impl Vault for Kernel {
    fn keep(&self, description: &str, secret: &str, seconds: u64) -> bool {
        use linux_keyutils::{KeyPermissionsBuilder, Permission};
        let Ok(ring) = self.possessed() else {
            return false;
        };
        let Ok(key) = ring.add_key(description, secret.as_bytes()) else {
            return false;
        };
        // ALL FOR ITS POSSESSOR, a sight of it for the user. Not a wall
        // between processes of the same user: any of them may link `@u` into
        // its own session keyring, as `possessed` does, and read the key for
        // the window. Other users are kept out by `@u` itself.
        let perms = KeyPermissionsBuilder::builder()
            .posessor(Permission::ALL)
            .user(Permission::VIEW)
            .build();
        let seconds = usize::try_from(seconds).unwrap_or(usize::MAX);
        if key.set_perms(perms).is_err() || key.set_timeout(seconds).is_err() {
            // NOT KEPT AS ASKED is not kept: no identity without its timeout.
            let _ = ring.unlink_key(key);
            let _ = key.invalidate();
            return false;
        }
        true
    }

    fn take(&self, description: &str) -> Option<zeroize::Zeroizing<String>> {
        let ring = self.possessed().ok()?;
        let key = ring.search(description).ok()?;
        let bytes = zeroize::Zeroizing::new(key.read_to_vec().ok()?);
        let text = std::str::from_utf8(&bytes).ok()?;
        Some(zeroize::Zeroizing::new(text.to_owned()))
    }

    fn forget(&self, prefix: &str) -> usize {
        // NO POSSESSION NEEDED: the user keyring is its owner's to read and
        // to unlink from, whoever started this (the sleep's system unit).
        let Ok(ring) = linux_keyutils::KeyRing::from_special_id(self.ring, false) else {
            return 0;
        };
        let Ok(links) = ring.get_links(LINKS_MOST) else {
            return 0;
        };
        let mut gone = 0;
        for node in links.iter() {
            let Some(key) = node.as_key() else {
                continue;
            };
            let ours = key
                .metadata()
                .is_ok_and(|m| m.get_description().starts_with(prefix));
            if ours && ring.unlink_key(key).is_ok() {
                // AND NOWHERE ELSE: a key linked somewhere it should not be
                // goes too (best effort: it needs the key's search right).
                let _ = key.invalidate();
                gone += 1;
            }
        }
        gone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: the repository's address sits in a key's description, two
    /// repositories share one, or the instance is not part of it.
    #[test]
    fn a_key_names_its_instance_and_hides_its_repository() {
        let a = description("git@example.org:someone/belongings.git");
        assert!(a.starts_with(PREFIX));
        assert!(a.contains(&format!(":{}:", ronoctua_xdg::name())));
        assert!(!a.contains("example.org"));
        assert_eq!(repository_tag(" git@example.org:a.git ").len(), 16);
        assert_ne!(a, description("git@example.org:someone/other.git"));
        assert_eq!(KEPT_SECONDS, crate::kept::UNLOCK_HOURS * 3600);
    }

    /// FAILS IF: a marker of another repository, an expired one or a broken
    /// one says "kept", or one that holds is not read back — or forgetting
    /// every instance's markers leaves one, or touches another file.
    #[test]
    fn the_marker_says_which_repository_and_until_when() {
        let root =
            std::env::temp_dir().join(format!("ronoctua-belongings-marker-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let p = Places::under(&root);
        let repo = "git@example.org:a/b.git";
        assert!(!kept_for(&p, repo, 0), "a marker that is not there");
        Marker {
            format: MARKER_FORMAT,
            repository: repository_tag(repo),
            until_s: 100,
        }
        .write(&p)
        .unwrap();
        assert!(kept_for(&p, repo, 99));
        assert!(!kept_for(&p, repo, 100), "expired");
        assert!(!kept_for(&p, "git@example.org:other.git", 1));
        std::fs::write(p.unlocked(), b"{").unwrap();
        assert!(!kept_for(&p, repo, 1), "a broken marker");

        // EVERY INSTANCE'S, under one runtime folder — and nothing else.
        let run = root.join("run");
        for leaf in ["ronoctua-belongings", "ronoctua-dev-belongings"] {
            std::fs::create_dir_all(run.join(leaf)).unwrap();
            std::fs::write(run.join(leaf).join(crate::places::UNLOCKED_FILE), b"{}").unwrap();
        }
        std::fs::create_dir_all(run.join("ronoctua-transfers")).unwrap();
        std::fs::write(run.join("ronoctua-transfers/unlocked"), b"x").unwrap();
        assert_eq!(unmark_everywhere(&run), 2);
        assert!(run.join("ronoctua-transfers/unlocked").exists());
        unmark(&p);
        let _ = std::fs::remove_dir_all(root);
    }

    /// FAILS IF: the kernel's keyring does not give back what it kept, keeps
    /// it readable by the user who does not possess it, forgets another
    /// description's key, or gives back a forgotten one. On this thread's
    /// own keyring: nothing of the person's is touched.
    #[cfg(feature = "keyring")]
    #[test]
    fn the_kernel_keeps_takes_and_forgets() {
        use linux_keyutils::{KeyRing, Permission};
        let vault = Kernel::thread();
        let ours = format!("{PREFIX}test:{}", std::process::id());
        let theirs = format!("someone-else:{}", std::process::id());
        assert!(vault.keep(&ours, "AGE-SECRET-KEY-PQ-1TEST", 60), "not kept");
        assert!(vault.keep(&theirs, "not ours", 60));
        assert_eq!(
            vault.take(&ours).as_deref().map(String::as_str),
            Some("AGE-SECRET-KEY-PQ-1TEST")
        );
        let ring = KeyRing::from_special_id(linux_keyutils::KeyRingIdentifier::Thread, false)
            .expect("the thread keyring");
        let key = ring.search(&ours).expect("the key");
        let perms = key.metadata().expect("described").get_perms().bits();
        assert_eq!(
            (perms >> 16) & 0xff,
            u32::from(Permission::VIEW.bits()),
            "the user (not possessing it) may read it"
        );
        assert_eq!(
            perms >> 24,
            u32::from(Permission::ALL.bits()),
            "its possessor may not do everything with it"
        );
        assert_eq!(vault.forget(PREFIX), 1);
        assert!(vault.take(&ours).is_none(), "forgotten and still there");
        assert!(vault.take(&theirs).is_some(), "another key was forgotten");
        assert_eq!(vault.forget("someone-else:"), 1);
    }
}
