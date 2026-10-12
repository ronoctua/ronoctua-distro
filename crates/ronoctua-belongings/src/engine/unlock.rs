//! The identity kept for the session (feature `crypt`): what opens `key.age`
//! on a run, keeping it, forgetting it ([`crate::unlock`] holds the keyring,
//! the marker and why).
//!
//! # What opens a run
//!
//! A passphrase the door wrote, when there is one: the person typed it, and
//! it is what they mean (a new one after a change, a first sync). Else the
//! identity this session kept, when the person keeps one
//! (`keep_unlocked`) — no scrypt, no question. A kept identity must open
//! THIS repository's index: another machine may have made it again since
//! (a new identity); one that does not is forgotten, and the
//! run is refused for want of a passphrase, which the door then asks.
//!
//! # When it is kept
//!
//! Right after a typed passphrase opened `key.age` — and after the first
//! sync of an empty repository made the identity — when `keep_unlocked` is
//! on; never refreshed by a run the kept identity opened (`unlockHours` is
//! counted from the typing). With `keep_unlocked` off, a run forgets what
//! was kept. An engine with no vault (the installer's verbs, on a system
//! that is not the person's) keeps nothing.

use std::path::{Path, PathBuf};

use age::secrecy::{ExposeSecret as _, SecretString};

use super::{Engine, KEY_FILE};
use crate::crypt::Keyring;
use crate::order::Beat;
use crate::places::Places;
use crate::unlock::{
    KEPT_SECONDS, Marker, PREFIX, Vault as _, description, instance_prefix, repository_tag, unmark,
};
use crate::{Refusal, Why};

/// What opens `key.age` on this run.
pub(super) enum Opening {
    /// A passphrase the person typed.
    Typed(SecretString),
    /// The identity the session kept.
    Kept(SecretString),
}

impl Engine<'_> {
    /// What opens this run: the passphrase a door wrote, else the identity
    /// the session kept for this repository.
    ///
    /// # Errors
    ///
    /// [`Why::NoPassphrase`] with neither — or with a passphrase's file that
    /// is not one a door wrote (it is never passed over for the kept
    /// identity: something is wrong there).
    pub(super) fn opening(&self) -> Result<Opening, Refusal> {
        let path = self.places.passphrase();
        let typed = super::read_secret(&path);
        if typed.is_ok() || std::fs::symlink_metadata(&path).is_ok() {
            return typed.map(Opening::Typed);
        }
        if self.prefs.keep_unlocked
            && let Some(vault) = self.vault
            && let Some(mut identity) = vault.take(&description(&self.prefs.repository))
        {
            return Ok(Opening::Kept(SecretString::from(std::mem::take(
                &mut *identity,
            ))));
        }
        typed.map(Opening::Typed)
    }

    /// The keyring `opening` opens: `key.age` with the passphrase, or the
    /// kept identity — which must open this repository's index.
    ///
    /// # Errors
    ///
    /// What opening `key.age` refuses; [`Why::NoPassphrase`] for a kept
    /// identity that is not this repository's (forgotten on the way).
    pub(super) fn open_ring(&self, opening: &Opening) -> Result<Keyring, Refusal> {
        match opening {
            Opening::Typed(passphrase) => Keyring::open(&self.read_repo(KEY_FILE)?, passphrase),
            Opening::Kept(identity) => {
                let ring = Keyring::from_secret(identity)
                    .ok()
                    .filter(|r| matches!(self.index_head(r), Ok(Some(_))));
                ring.ok_or_else(|| {
                    self.forget_kept();
                    Refusal::of(Why::NoPassphrase)
                })
            }
        }
    }

    /// After `opening` opened `ring`: keeps the identity for the session —
    /// when it was typed for, the person keeps one and this engine has a
    /// vault — or forgets what was kept when the person keeps none.
    pub(super) fn keep_for_the_session(&self, ring: &Keyring, opening: &Opening) {
        let Some(vault) = self.vault else {
            return;
        };
        if !self.prefs.keep_unlocked {
            self.forget_kept();
            return;
        }
        if matches!(opening, Opening::Kept(_)) {
            return;
        }
        let secret = ring.secret();
        let kept = vault.keep(
            &description(&self.prefs.repository),
            secret.expose_secret(),
            KEPT_SECONDS,
        );
        if kept {
            let until_s = self
                .now_s
                .saturating_add(i64::try_from(KEPT_SECONDS).unwrap_or(i64::MAX));
            let _ = Marker {
                format: crate::unlock::MARKER_FORMAT,
                repository: repository_tag(&self.prefs.repository),
                until_s,
            }
            .write(self.places);
        } else {
            unmark(self.places);
        }
    }

    /// Forgets the identity kept for this repository, and the marker.
    pub(super) fn forget_kept(&self) {
        if let Some(vault) = self.vault {
            vault.forget(&description(&self.prefs.repository));
        }
        unmark(self.places);
    }
}

/// The runtime folder every instance's marker lives under: this process's
/// `$XDG_RUNTIME_DIR`, else the user's own `/run/user/<uid>` (a system unit
/// running as the person — the sleep's — has no variable).
fn runtime_root() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|v| !v.is_empty())
        .map_or_else(
            || PathBuf::from("/run/user").join(rustix::process::getuid().as_raw().to_string()),
            PathBuf::from,
        )
}

/// Forgets EVERY identity the belongings keep for this user — every
/// repository, every instance — and every marker under `runtime`. What the
/// end of a session and a sleep do (`forget --everywhere`): the development
/// loop's copy runs inside the person's session, and ends with it. Says how
/// many keys went.
#[must_use]
pub fn forget_everywhere_under(vault: &dyn crate::unlock::Vault, runtime: &Path) -> usize {
    let gone = vault.forget(PREFIX);
    crate::unlock::unmark_everywhere(runtime);
    gone
}

/// `ronoctua-belongings forget --everywhere`: [`forget_everywhere_under`]
/// the kernel's keyring and this user's runtime folder — with no settings,
/// no home and no runtime variable needed (the sleep's system unit has none
/// of them).
#[must_use]
pub fn forget_everywhere() -> usize {
    forget_everywhere_under(&crate::unlock::Kernel::USER, &runtime_root())
}

/// Forgets the identities THIS instance keeps for this user (every
/// repository of `ronoctua`, or of `ronoctua-dev`: [`instance_prefix`]) and
/// this instance's marker under `runtime` — and nothing of another
/// instance's: the development loop turning `keep_unlocked` off never makes
/// the installed session ask its passphrase again. Says how many keys went.
#[must_use]
pub fn forget_here_under(vault: &dyn crate::unlock::Vault, runtime: &Path) -> usize {
    let gone = vault.forget(&instance_prefix());
    crate::unlock::unmark_here(runtime);
    gone
}

/// `ronoctua-belongings forget`: [`forget_here_under`] the kernel's keyring
/// and this user's runtime folder, with no settings and no home needed.
#[must_use]
pub fn forget_here() -> usize {
    forget_here_under(&crate::unlock::Kernel::USER, &runtime_root())
}

/// `forget`, as the queue orders it — when the person turns
/// `keep_unlocked` off: this instance's identities and the marker of the
/// runtime folder `places` lives in ([`forget_here_under`]). Never refused,
/// and it says nothing in `status.json`: forgetting is not a sync.
///
/// # Errors
///
/// None: what is not there is forgotten already.
#[allow(clippy::unnecessary_wraps)]
pub(super) fn forget(places: &Places, beat: &mut dyn FnMut(Beat)) -> Result<u32, Refusal> {
    let _ = beat;
    let _ = crate::unlock::Kernel::USER.forget(&instance_prefix());
    unmark(places);
    Ok(0)
}

// ── anchor: L10 ──
