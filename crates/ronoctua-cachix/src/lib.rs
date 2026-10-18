//! The person's binary cache, on Cachix — the one crate that talks to it.
//!
//! # Whose cache
//!
//! The PERSON'S, never the distro's: the distro has no cache of its own, and
//! its ISO carries its seed. The person names their Cachix cache (its name,
//! its public signing key, public or private) in `options.json`
//! (`ronoctua_person::options::BinaryCache`), which travels in the plain part
//! of their belongings; `nix/modules/binary-cache.nix` reads root's validated
//! copy and adds the substituter before `cache.nixos.org`, and writes what a
//! push needs to know into [`SYSTEM_FILE`] ([`system::System`]).
//!
//! # What is here
//!
//! - [`token`]: the push token — a secret, kept `0600` in the sealed part of
//!   the belongings (`config/binary-cache-keys/push-token`), never in the
//!   plain part, an argument, a long-lived process's environment or a log;
//! - [`netrc`]: what root writes so the machine's Nix daemon may READ a
//!   private cache ([`NETRC`], `0600`, root's) — through the sudo door when
//!   the person sets the token, and by the installer;
//! - [`closure`]: what a push may carry — a PUBLIC cache only the distro's
//!   own outputs, never a path of the person's nor an unfree program; a
//!   private one the system's closure — minus what `cache.nixos.org` has;
//! - [`push`]: the push itself, the update's last step, as the person, only
//!   after an update that switched — and [`record`], what it said;
//! - [`settings`]: the person's own copy of their choice (the mirror of
//!   `options.json` the screen writes), and what root's netrc should be.
//!
//! # Nothing asks anybody anything
//!
//! The push runs inside the update with no terminal: `cachix` is given the
//! token in ITS environment alone (`CACHIX_AUTH_TOKEN`), and a host the
//! person did not let is a refusal ([`Why::NotLet`]), never a question.

#![forbid(unsafe_code)]

pub mod closure;
pub mod key;
pub mod netrc;
pub mod push;
pub mod record;
pub mod settings;
pub mod system;
pub mod token;

/// What the machine was built with, for the push: the cache, and the roots a
/// public one may receive. Written by `nix/modules/binary-cache.nix` (the
/// same path, `binary-cache-declared-once`), absent when the person names
/// no cache — and then the update's push declines, said (`no-cache`).
pub const SYSTEM_FILE: &str = "/etc/ronoctua/binary-cache.json";

/// The netrc the machine's Nix daemon reads for a PRIVATE cache
/// (`nix.settings.netrc-file`, `nix/modules/binary-cache.nix`): root's,
/// `0600`, in a folder of its own that survives the wipe
/// (`nix/modules/persist.nix`). The folder is `0755` so the screen can say
/// whether the file is there; the file is root's alone.
pub const NETRC: &str = "/var/lib/ronoctua-binary-cache/netrc";

/// The API host every push asks first.
pub const API_HOST: &str = "cachix.org";

/// The public cache a push subtracts: what it already serves is never
/// pushed again.
pub const UPSTREAM_HOST: &str = "cache.nixos.org";

/// Every host a push to the cache `name` reaches — what the screen lets in
/// the person's `consent.json` when they name the cache (naming it IS the
/// consent), and what a push holds a [`ronoctua_consent::Pass`] for.
#[must_use]
pub fn hosts_of(name: &str) -> [String; 3] {
    [
        API_HOST.to_owned(),
        format!("{name}.cachix.org"),
        UPSTREAM_HOST.to_owned(),
    ]
}

/// Why a push, a token or a netrc was refused, as a closed set — the screen
/// says each with `binary-cache-why-<id>`, and the update's step reads three
/// of them as news (`declines` in `steps.nix`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Why {
    /// The machine names no cache ([`SYSTEM_FILE`] absent, or not one this
    /// distro reads).
    NoCache,
    /// The person keeps no push token.
    NoToken,
    /// The token's file is readable by somebody else, or is not a file.
    TokenOpen,
    /// What was given is not a Cachix token.
    NotAToken,
    /// A host the push reaches is not let in `consent.json`.
    NotLet,
    /// No update switched the machine since the last push.
    NothingNew,
    /// `nix` or `cachix` is not where the environment says.
    NoTools,
    /// `nix` did not answer about the store.
    StoreFailed,
    /// `cachix` refused, or did not finish.
    CachixFailed,
    /// A file could not be written.
    Unwritable,
    /// The person's `options.json` is of a newer format than this machine
    /// reads: it is not rewritten.
    Newer,
    /// The person's `options.json` is there and does not read: it is not
    /// rewritten blind.
    Unreadable,
}

impl Why {
    /// Every one, in the order the keys are written.
    pub const ALL: [Self; 12] = [
        Self::NoCache,
        Self::NoToken,
        Self::TokenOpen,
        Self::NotAToken,
        Self::NotLet,
        Self::NothingNew,
        Self::NoTools,
        Self::StoreFailed,
        Self::CachixFailed,
        Self::Unwritable,
        Self::Newer,
        Self::Unreadable,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::NoCache => "no-cache",
            Self::NoToken => "no-token",
            Self::TokenOpen => "token-open",
            Self::NotAToken => "not-a-token",
            Self::NotLet => "not-let",
            Self::NothingNew => "nothing-new",
            Self::NoTools => "no-tools",
            Self::StoreFailed => "store-failed",
            Self::CachixFailed => "cachix-failed",
            Self::Unwritable => "unwritable",
            Self::Newer => "newer",
            Self::Unreadable => "unreadable",
        }
    }

    /// The one of this name.
    #[must_use]
    pub fn of_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|w| w.id() == id)
    }
}

impl std::fmt::Display for Why {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

impl std::error::Error for Why {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_why_crosses_by_its_name() {
        for why in Why::ALL {
            assert_eq!(Why::of_id(why.id()), Some(why));
            assert_eq!(
                serde_json::to_string(&why).unwrap(),
                format!("\"{}\"", why.id())
            );
        }
        assert_eq!(Why::of_id("nonsense"), None);
    }

    #[test]
    fn a_push_reaches_the_api_the_cache_and_the_upstream() {
        assert_eq!(
            hosts_of("someone"),
            [
                "cachix.org".to_owned(),
                "someone.cachix.org".to_owned(),
                "cache.nixos.org".to_owned()
            ]
        );
    }
}
