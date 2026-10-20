//! `local/held.json` and `local/approved.json` — what only root writes, under
//! the machine directory, and never syncs.
//!
//! ```json
//! {"format": 1, "held": [{"id": "nix:broken", "fingerprint": "…", "reason": "eval-failed",
//!   "since": "2026-10-05T12:00:00Z"}]}
//! {"format": 1, "modules": {"my-reminders": {"hash": "sha256-…", "approved-at": "2026-10-05T12:00:00Z"}}}
//! ```
//!
//! `local/signer.json` is the person's signing key this machine trusts —
//! pinned by the installer, or by the person through the sudo door — the
//! highest counter of theirs an apply took, and what the last apply made of
//! the signature ([`Signer`]).
//!
//! `local/verified.json` is the other side of the hold, for the distro's
//! AppImages the person follows past its pins ([`Verified`]): root's verify
//! records each one it BUILT, at the fingerprint it built it at, and the
//! registry uses a followed pin only while this record names it — an
//! allow-list, so a pin nobody verified (the software centre's apply, the
//! rescue's rebuild, a verify that did not run) is simply not used yet.
//!
//! A held entry is skipped by the Nix consumer until its fingerprint (which
//! only Nix computes: the `personFingerprints` output) changes. An approval
//! names the hash `nix/person-hash.nix` computes — the single implementation;
//! the root helper asks it through `nix eval` — and a module edited after its
//! approval no longer matches and is not imported.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::tree::{KeptBack, TreeWhy};
use crate::{Document, Refusal, Stamp, Why, check};

/// `local/held.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Held {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The entries held back.
    pub held: Vec<HeldEntry>,
}

impl Default for Held {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            held: Vec::new(),
        }
    }
}

/// Why an entry is held back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HeldWhy {
    /// Its evaluation failed in isolation.
    EvalFailed,
    /// Its download could not be fetched.
    FetchFailed,
    /// Its download is there with a different hash.
    UpstreamChanged,
    /// It evaluated and did not build.
    BuildFailed,
}

impl HeldWhy {
    /// Every one.
    pub const ALL: [Self; 4] = [
        Self::EvalFailed,
        Self::FetchFailed,
        Self::UpstreamChanged,
        Self::BuildFailed,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::EvalFailed => "eval-failed",
            Self::FetchFailed => "fetch-failed",
            Self::UpstreamChanged => "upstream-changed",
            Self::BuildFailed => "build-failed",
        }
    }
}

/// One held entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct HeldEntry {
    /// The declared entry's id.
    pub id: String,
    /// What Nix computed of it when it failed.
    pub fingerprint: String,
    /// Why.
    pub reason: HeldWhy,
    /// Since when.
    pub since: Stamp,
}

impl Document for Held {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        let mut seen = std::collections::BTreeSet::new();
        for (i, h) in self.held.iter().enumerate() {
            let at = format!("/held/{i}");
            if !h.id.contains(':') || !check::token(&h.id) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/id"),
                    h.id.as_str(),
                ));
            }
            if !check::token(&h.fingerprint) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/fingerprint"),
                    h.fingerprint.as_str(),
                ));
            }
            if !seen.insert(h.id.as_str()) {
                return Err(Refusal::new(
                    Why::Duplicate,
                    format!("{at}/id"),
                    h.id.as_str(),
                ));
            }
        }
        Ok(())
    }
}

/// `local/verified.json`: the followed distro AppImages root's verify built,
/// each at the fingerprint it built it at, with its pin — so the registry
/// can build the LAST verified one while a newer one waits or is held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Verified {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// One per followed AppImage, by its verification id.
    pub verified: Vec<VerifiedEntry>,
}

impl Default for Verified {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            verified: Vec::new(),
        }
    }
}

/// One followed AppImage root's verify built.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct VerifiedEntry {
    /// `distro-appimage:<id>` ([`crate::distro_pins::entry_id`]).
    pub id: String,
    /// What Nix computed of it when it built.
    pub fingerprint: String,
    /// The pin it built: the release's address.
    pub url: String,
    /// Its SRI hash.
    pub hash: String,
    /// Its version.
    pub version: String,
    /// When it was first built at this fingerprint.
    pub since: Stamp,
}

impl Document for Verified {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        let mut seen = std::collections::BTreeSet::new();
        for (i, v) in self.verified.iter().enumerate() {
            let at = format!("/verified/{i}");
            let wrong = if crate::distro_pins::of_entry(&v.id).is_none() {
                Some(("id", v.id.as_str()))
            } else if !check::token(&v.fingerprint) {
                Some(("fingerprint", v.fingerprint.as_str()))
            } else if !check::https_url(&v.url) {
                Some(("url", v.url.as_str()))
            } else if !check::sri(&v.hash) {
                Some(("hash", v.hash.as_str()))
            } else if !check::token(&v.version) {
                Some(("version", v.version.as_str()))
            } else {
                None
            };
            if let Some((field, found)) = wrong {
                return Err(Refusal::new(Why::Invalid, format!("{at}/{field}"), found));
            }
            if !seen.insert(v.id.as_str()) {
                return Err(Refusal::new(
                    Why::Duplicate,
                    format!("{at}/id"),
                    v.id.as_str(),
                ));
            }
        }
        Ok(())
    }
}

/// `local/approved.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Approved {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// By person module id (`my-…`).
    pub modules: BTreeMap<String, Approval>,
}

impl Default for Approved {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            modules: BTreeMap::new(),
        }
    }
}

/// One approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Approval {
    /// What `nix/person-hash.nix` said of the module's folder.
    pub hash: String,
    /// When the person approved it.
    pub approved_at: Stamp,
}

impl Document for Approved {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        for (id, a) in &self.modules {
            let at = format!("/modules/{id}");
            if !check::person_module(id) {
                return Err(Refusal::new(Why::Invalid, at, id.as_str()));
            }
            if !check::token(&a.hash) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/hash"),
                    a.hash.as_str(),
                ));
            }
        }
        Ok(())
    }
}

/// How a pinned public key is written: this, then the raw key in unpadded
/// base64 (`ronoctua_pq::sign::PUBLIC_KEY_PREFIX`; the two are held equal by
/// the binary's tests, which link both).
pub const KEY_PREFIX: &str = "ronoctua-mldsa65-1";

/// The base64 characters of an ML-DSA-65 public key's 1952 bytes, unpadded.
pub const KEY_BASE64_CHARS: usize = 2603;

/// Whether `key` is written as a pinned key is: the prefix, then exactly the
/// base64 characters of a key of the right length — the shape, which the
/// sudo door's verb checks before it hands the key over; that the bytes ARE
/// a key is the binary's to say, when it pins. Pure.
#[must_use]
pub fn key_is_plain(key: &str) -> bool {
    key.strip_prefix(KEY_PREFIX).is_some_and(|body| {
        body.len() == KEY_BASE64_CHARS
            && body
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
    })
}

/// `local/signer.json`: the person's signing key this machine trusts, and
/// what the last apply made of their signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Signer {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The public key ([`key_is_plain`]).
    pub key: String,
    /// The SHA-256 of its raw bytes, hexadecimal: what a manifest names its
    /// signer by.
    pub fingerprint: String,
    /// When it was pinned.
    pub pinned_at: Stamp,
    /// The highest manifest counter an apply took under it: a lower one is
    /// an older tree played again ([`TreeWhy::RolledBack`]).
    pub counter: u64,
    /// The last apply refused the staging for its signature, and why.
    pub refused: Option<SignatureRefused>,
    /// What the last apply kept back because the signature did not vouch
    /// for it.
    pub kept_back: Vec<KeptBack>,
}

/// Why, where and when the last apply refused a staging for its signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SignatureRefused {
    /// When.
    pub at: Stamp,
    /// [`TreeWhy::NotSigned`], [`TreeWhy::BadSignature`] or
    /// [`TreeWhy::RolledBack`].
    pub why: TreeWhy,
    /// What was found, English, for the journal.
    pub detail: String,
}

impl Signer {
    /// A key pinned at `at`: no counter taken yet, nothing refused.
    #[must_use]
    pub fn pinned(key: String, fingerprint: String, at: Stamp) -> Self {
        Self {
            format: crate::FORMAT,
            key,
            fingerprint,
            pinned_at: at,
            counter: 0,
            refused: None,
            kept_back: Vec::new(),
        }
    }
}

impl Document for Signer {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if !key_is_plain(&self.key) {
            return Err(Refusal::new(Why::Invalid, "/key", "not a pinned key"));
        }
        if !check::sha256_hex(&self.fingerprint) {
            return Err(Refusal::new(
                Why::Invalid,
                "/fingerprint",
                self.fingerprint.as_str(),
            ));
        }
        if let Some(r) = &self.refused
            && !r.why.refuses_the_signature()
        {
            return Err(Refusal::new(Why::Invalid, "/refused/why", r.why.id()));
        }
        if let Some(i) = self.kept_back.iter().position(|k| !k.why.keeps_back()) {
            return Err(Refusal::new(
                Why::Invalid,
                format!("/kept-back/{i}/why"),
                self.kept_back[i].why.id(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: a key cut short, one character too many, a character
    /// outside base64, padding or another prefix passes for a pinned key's
    /// shape — or the shape of a real one is refused.
    #[test]
    fn a_pinned_key_has_one_shape() {
        let body = "A".repeat(KEY_BASE64_CHARS);
        assert!(key_is_plain(&format!("{KEY_PREFIX}{body}")));
        assert!(key_is_plain(&format!(
            "{KEY_PREFIX}{}+/9",
            "a".repeat(KEY_BASE64_CHARS - 3)
        )));
        for bad in [
            format!("{KEY_PREFIX}{}", "A".repeat(KEY_BASE64_CHARS - 1)),
            format!("{KEY_PREFIX}{body}A"),
            format!("{KEY_PREFIX}{}=", "A".repeat(KEY_BASE64_CHARS - 1)),
            format!("{KEY_PREFIX}{} ", "A".repeat(KEY_BASE64_CHARS - 1)),
            format!("ronoctua-mldsa87-1{body}"),
            format!(" {KEY_PREFIX}{body}"),
            String::new(),
        ] {
            assert!(!key_is_plain(&bad), "{bad:.30}");
        }
    }

    /// FAILS IF: a pin whose refusal is not a signature's, or whose kept-back
    /// file is kept for another reason, is written or read.
    #[test]
    fn a_pin_says_only_what_a_signature_can_say() {
        let key = format!("{KEY_PREFIX}{}", "A".repeat(KEY_BASE64_CHARS));
        let at = Stamp::parse("2026-10-11T12:00:00Z").unwrap();
        let mut pin = Signer::pinned(key, "a".repeat(64), at.clone());
        assert!(pin.validate().is_ok());
        pin.refused = Some(SignatureRefused {
            at,
            why: TreeWhy::Link,
            detail: String::new(),
        });
        assert_eq!(pin.validate().unwrap_err().at, "/refused/why");
        pin.refused = None;
        pin.kept_back.push(KeptBack {
            at: "person/options.json".to_owned(),
            why: TreeWhy::BadSignature,
        });
        assert_eq!(pin.validate().unwrap_err().at, "/kept-back/0/why");
        pin.kept_back.clear();
        pin.fingerprint = "A".repeat(64);
        assert_eq!(pin.validate().unwrap_err().at, "/fingerprint");
    }
}
