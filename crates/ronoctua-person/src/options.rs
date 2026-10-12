//! `options.json` — the person's opt-ins that travel to every machine.
//!
//! ```json
//! {"format": 1, "ssh-signing": null,
//!  "binary-cache": {"name": "someone", "public-key": "someone.cachix.org-1:…=",
//!                   "visibility": "public"}}
//! ```
//!
//! The belongings' targets offer them after a restore ("no" first); the Nix
//! side reads them through `ronoctua.person.*`.
//!
//! WHETHER A SECURITY KEY ANSWERS SUDO IS NOT HERE (owner, 2026-10-11): it is
//! root's list of the person's keys (`ronoctua_security::u2f::AUTHFILE`),
//! written through the sudo door and holding at once, with no rebuild — a
//! choice that changes how root authenticates never travels as data.
//!
//! # The binary cache is the person's
//!
//! `binary-cache` names the person's OWN Cachix cache — never the distro's,
//! which has none: its name, its public signing key and who may read it. It
//! travels in the plain part of their belongings like every opt-in, and
//! `nix/modules/binary-cache.nix` reads root's validated copy of it and adds
//! the substituter BEFORE `cache.nixos.org`. The token that pushes (and that
//! reads a private cache) is a secret and is never here: it lives in the
//! sealed part (`ronoctua_cachix::token`).
//!
//! The two validators — this one and the Nix module's — are held to ONE
//! table of cases (`fixtures/binary-cache-cases.json`): a value one of them
//! takes and the other refuses would be a cache the screen accepts and the
//! machine ignores, or the reverse.

use serde::{Deserialize, Serialize};

use crate::{Document, Refusal, Why, check};

/// The file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Options {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// Commits signed by an SSH key, if the person said yes.
    pub ssh_signing: Option<SshSigning>,
    /// The person's own binary cache, if they named one. Absent in a file
    /// written before it existed, which reads as none.
    #[serde(default)]
    pub binary_cache: Option<BinaryCache>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            ssh_signing: None,
            binary_cache: None,
        }
    }
}

/// Signing commits with an SSH key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SshSigning {
    /// The public key's path, relative to home (`.ssh/id_ed25519.pub`).
    pub key: String,
}

/// The person's Cachix cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BinaryCache {
    /// The cache's name: its address is `https://<name>.cachix.org`.
    pub name: String,
    /// Its signing key, exactly as cachix.org shows it:
    /// `<name>.cachix.org-1:<44 characters of base64>`.
    pub public_key: String,
    /// Who may read it.
    pub visibility: Visibility,
}

impl BinaryCache {
    /// The cache's address, as `nix.conf` names a substituter.
    #[must_use]
    pub fn substituter(&self) -> String {
        format!("https://{}", self.host())
    }

    /// The host the cache answers on — and the one a netrc names.
    #[must_use]
    pub fn host(&self) -> String {
        format!("{}.cachix.org", self.name)
    }

    /// Whether the cache is private: reading it needs the token.
    #[must_use]
    pub const fn is_private(&self) -> bool {
        matches!(self.visibility, Visibility::Private)
    }

    /// Why the cache is refused, as the JSON pointer of the value at fault —
    /// `None` when it is one. The Nix module asks the same three questions
    /// (`nix/modules/binary-cache.nix`).
    #[must_use]
    pub fn fault(&self) -> Option<&'static str> {
        if !is_cache_name(&self.name) {
            Some("/binary-cache/name")
        } else if !is_key_of(&self.name, &self.public_key) {
            Some("/binary-cache/public-key")
        } else {
            None
        }
    }
}

/// Who may read the cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    /// Anybody: Cachix's free plan. It receives only the distro's own
    /// outputs — never a path that carries the person's data, never an
    /// unfree program (`ronoctua_cachix::push`).
    Public,
    /// Only who holds the token: the machine's Nix reads it through a netrc
    /// root writes, and it may receive the system's whole closure.
    Private,
}

impl Visibility {
    /// Both, in the order the screen offers them.
    pub const ALL: [Self; 2] = [Self::Public, Self::Private];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

/// Whether `name` is a Cachix cache's name: lower-case letters, digits and
/// dashes, starting with a letter or a digit, at most 63 — the host label of
/// `<name>.cachix.org`. Pure.
#[must_use]
pub fn is_cache_name(name: &str) -> bool {
    let b = name.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
}

/// Whether `key` is the signing key cachix.org shows for the cache `name`:
/// `<name>.cachix.org-1:` and an ed25519 public key in base64 — 43
/// characters of the alphabet and one `=`. Pure.
#[must_use]
pub fn is_key_of(name: &str, key: &str) -> bool {
    let Some(rest) = key
        .strip_prefix(name)
        .and_then(|r| r.strip_prefix(".cachix.org-1:"))
    else {
        return false;
    };
    let b = rest.as_bytes();
    b.len() == 44
        && b[43] == b'='
        && b[..43]
            .iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/')
}

impl Document for Options {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if let Some(s) = &self.ssh_signing {
            let relative = !s.key.starts_with('/') && !s.key.split('/').any(|p| p == "..");
            if !relative || !check::token(&s.key) {
                return Err(Refusal::new(
                    Why::Invalid,
                    "/ssh-signing/key",
                    s.key.as_str(),
                ));
            }
        }
        if let Some(cache) = &self.binary_cache
            && let Some(at) = cache.fault()
        {
            let found = if at.ends_with("/name") {
                cache.name.as_str()
            } else {
                cache.public_key.as_str()
            };
            return Err(Refusal::new(Why::Invalid, at, found));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One case of the table both validators are held to.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        case: String,
        #[serde(rename = "binary-cache")]
        binary_cache: serde_json::Value,
        valid: bool,
    }

    /// FAILS IF: a cache the shared table calls valid is refused, or one it
    /// calls invalid is taken — the Nix module reads the same table
    /// (`binary-cache-declared-once`), so the two never disagree.
    #[test]
    fn the_cache_is_judged_as_the_shared_table_says() {
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("../fixtures/binary-cache-cases.json"))
                .expect("the table");
        assert!(cases.len() >= 10, "the table lost its cases");
        assert!(cases.iter().any(|c| c.valid) && cases.iter().any(|c| !c.valid));
        for c in cases {
            let file = serde_json::json!({
                "format": crate::FORMAT,
                "ssh-signing": null,
                "binary-cache": c.binary_cache,
            });
            let read = crate::parse::<Options>(file.to_string().as_bytes());
            assert_eq!(read.is_ok(), c.valid, "{}: {read:?}", c.case);
            if let Err(r) = read {
                assert!(
                    matches!(r.why, Why::Invalid | Why::Shape),
                    "{}: {r}",
                    c.case
                );
            }
        }
    }

    /// FAILS IF: a file written before the cache existed stops reading, or
    /// the cache's address and host are not the name's.
    #[test]
    fn an_older_file_reads_as_no_cache_and_the_address_is_the_names() {
        let old: Options = crate::parse(br#"{"format": 1, "ssh-signing": null}"#).unwrap();
        assert_eq!(old.binary_cache, None);
        let cache = BinaryCache {
            name: "someone".to_owned(),
            public_key: format!("someone.cachix.org-1:{}=", "Q".repeat(43)),
            visibility: Visibility::Private,
        };
        assert_eq!(cache.substituter(), "https://someone.cachix.org");
        assert_eq!(cache.host(), "someone.cachix.org");
        assert!(cache.is_private());
        assert_eq!(cache.fault(), None);
        for v in Visibility::ALL {
            assert_eq!(
                serde_json::to_string(&v).unwrap(),
                format!("\"{}\"", v.id())
            );
        }
    }
}
