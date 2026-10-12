//! What the machine was built with — [`crate::SYSTEM_FILE`], written by
//! `nix/modules/binary-cache.nix` from root's validated copy of the person's
//! `options.json`.
//!
//! ```json
//! {"format": 1,
//!  "cache": {"name": "someone", "public-key": "someone.cachix.org-1:…=", "visibility": "public"},
//!  "public-roots": ["/nix/store/…-ronoctua-shell", …],
//!  "unfree": ["google-chrome", "unrar", …],
//!  "android-unfree": false}
//! ```
//!
//! # The public roots are the DISTRO's
//!
//! `public-roots` is the distro's own seed — what its ISO carries (the
//! compositor, the shell, the fonts, the icons, the index) — as the DISTRO
//! evaluates it, never as this machine does: a store path is a function of
//! everything that went into it, so a path the distro's evaluation gives
//! cannot hold the person's host name, their user, their `/etc`, their
//! declared programs or their brain modules. A machine whose shell carries
//! the person's brain modules builds ANOTHER shell, which is not on this
//! list, and its public cache simply does not receive it. The paths are
//! written without their string context, so the list costs the system no
//! reference: a root this machine never built is not valid here, and is
//! skipped ([`crate::push`]).
//!
//! `unfree` is what `nix/release-unfree.nix` calls unfree among the names
//! this machine's packages carry, by name without the version — and with
//! `android-unfree` (Android on: nixpkgs lets every unfree package in) every
//! name that begins with `android` too.

use serde::{Deserialize, Serialize};

use ronoctua_person::options::BinaryCache;
use ronoctua_person::{Document, Refusal, Why as PersonWhy};

use crate::Why;

/// The file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct System {
    /// Always `ronoctua_person::FORMAT`.
    pub format: u32,
    /// The cache the machine was built with.
    pub cache: BinaryCache,
    /// The distro's own outputs, which a PUBLIC cache may receive.
    #[serde(default)]
    pub public_roots: Vec<String>,
    /// The unfree names, without their versions.
    #[serde(default)]
    pub unfree: Vec<String>,
    /// Whether every name beginning with `android` is unfree too.
    #[serde(default)]
    pub android_unfree: bool,
}

impl Document for System {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if let Some(at) = self.cache.fault() {
            return Err(Refusal::new(
                PersonWhy::Invalid,
                at,
                self.cache.name.as_str(),
            ));
        }
        if let Some(bad) = self.public_roots.iter().find(|p| !is_store_path(p)) {
            return Err(Refusal::new(
                PersonWhy::Invalid,
                "/public-roots",
                bad.as_str(),
            ));
        }
        if let Some(bad) = self.unfree.iter().find(|n| n.is_empty() || n.contains('/')) {
            return Err(Refusal::new(PersonWhy::Invalid, "/unfree", bad.as_str()));
        }
        Ok(())
    }
}

impl System {
    /// The file at `path`.
    ///
    /// # Errors
    ///
    /// [`Why::NoCache`] when it is not there, or is not one this distro
    /// reads — a machine with no cache, said, never a cache guessed at.
    pub fn read_at(path: &std::path::Path) -> Result<Self, Why> {
        let bytes = std::fs::read(path).map_err(|_| Why::NoCache)?;
        ronoctua_person::parse::<Self>(&bytes).map_err(|_| Why::NoCache)
    }
}

/// Whether `path` names a store object: `/nix/store/` and a base name of a
/// 32-character hash, a dash and a name, nothing below it. Pure.
#[must_use]
pub fn is_store_path(path: &str) -> bool {
    let Some(base) = path.strip_prefix("/nix/store/") else {
        return false;
    };
    let b = base.as_bytes();
    b.len() > 33
        && b[32] == b'-'
        && b[..32]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && b[33..]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || b"+-._?=".contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "someone.cachix.org-1:QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ=";

    /// FAILS IF: the machine's file with a cache that is not one, a root
    /// that is not a store path or an unknown field is read as a cache —
    /// or a good one is not.
    #[test]
    fn the_machines_file_is_read_whole_or_not_at_all() {
        let good = format!(
            r#"{{"format": 1, "cache": {{"name": "someone", "public-key": "{KEY}", "visibility": "public"}},
               "public-roots": ["/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-ronoctua-shell"],
               "unfree": ["google-chrome"], "android-unfree": true}}"#
        );
        let s: System = ronoctua_person::parse(good.as_bytes()).unwrap();
        assert_eq!(s.cache.name, "someone");
        assert!(s.android_unfree);
        for bad in [
            good.replace("someone\"", "Some_one\""),
            good.replace("/nix/store/aaaa", "/etc/aaaa"),
            good.replace("\"unfree\"", "\"unfree-too\""),
            good.replace("\"format\": 1", "\"format\": 9"),
        ] {
            assert!(
                ronoctua_person::parse::<System>(bad.as_bytes()).is_err(),
                "{bad}"
            );
        }
        let root =
            std::env::temp_dir().join(format!("ronoctua-cachix-system-{}", std::process::id()));
        assert_eq!(
            System::read_at(&root.join("absent.json")),
            Err(Why::NoCache)
        );
    }

    #[test]
    fn a_store_path_is_a_hash_and_a_name() {
        assert!(is_store_path(
            "/nix/store/0v9l3jn4d335zrrn57bvfl585q6i7kya-bash-5.3p9-info"
        ));
        for bad in [
            "/nix/store/short-bash",
            "/nix/store/0v9l3jn4d335zrrn57bvfl585q6i7kya-bash/bin/bash",
            "/nix/store/0V9L3JN4D335ZRRN57BVFL585Q6I7KYA-bash",
            "/etc/0v9l3jn4d335zrrn57bvfl585q6i7kya-bash",
            "/nix/store/0v9l3jn4d335zrrn57bvfl585q6i7kya-",
        ] {
            assert!(!is_store_path(bad), "{bad}");
        }
    }
}
