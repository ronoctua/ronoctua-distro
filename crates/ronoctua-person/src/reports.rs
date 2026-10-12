//! What Nix writes back under `/etc/ronoctua/` — read by the software
//! centre, the update and the reconciler, never written by Rust.
//!
//! - `person-applied.json`: what was applied, what was skipped and why, what
//!   the declines did, which person brain modules were included or wait for
//!   an approval, and what root made of the person's signature (the key it
//!   trusts, the last refusal, what it kept back — `local/signer.json`).
//! - `distro-apps.json`: the distro's registry on this machine.
//! - `declared-android.json`: the Android apps to reconcile, as store apks.

use serde::{Deserialize, Serialize};

use crate::declared::{AndroidImage, Source};
use crate::local::SignatureRefused;
use crate::tree::KeptBack;
use crate::{Document, Refusal, Why, check};

/// `person-applied.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PersonApplied {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The machine's hostname.
    pub machine: String,
    /// The distro's revision the system was built from.
    pub distro_rev: String,
    /// The declared entries in the system.
    pub applied: Vec<String>,
    /// The declared entries left out, and why.
    pub skipped: Vec<Skipped>,
    /// What the declines did.
    pub declined: DeclinedReport,
    /// The person's brain modules.
    pub brain: BrainReport,
    /// What root made of the person's signature, as `local/signer.json`
    /// said when the system was built. A report of a build before the
    /// signature has none, which reads as no key pinned.
    #[serde(default)]
    pub signature: SignatureReport,
}

/// What root made of the person's signature (`local/signer.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SignatureReport {
    /// The fingerprint of the key this machine trusts; `None` when no key is
    /// pinned (the person's options and brain modules then wait).
    pub pinned: Option<String>,
    /// The highest counter of the person's an apply took.
    pub counter: u64,
    /// The last apply refused the staging for its signature: the machine
    /// kept the data it had.
    pub refused: Option<SignatureRefused>,
    /// What the last apply kept back, the machine's own copy kept.
    pub kept_back: Vec<KeptBack>,
}

/// Why a declared entry was left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkipWhy {
    /// nixpkgs has no such attribute.
    AttrMissing,
    /// The attribute is not a package.
    NotAPackage,
    /// Its `meta` refused it (unfree without consent, insecure, broken).
    RefusedByMeta,
    /// The update held it back (`local/held.json`).
    Held,
    /// This machine does not have its source (Flatpak or Android off).
    SourceOff,
    /// It is declared on other machines only.
    OtherMachine,
    /// The file it is in says a format this machine does not read.
    FormatUnknown,
}

impl SkipWhy {
    /// Every one.
    pub const ALL: [Self; 7] = [
        Self::AttrMissing,
        Self::NotAPackage,
        Self::RefusedByMeta,
        Self::Held,
        Self::SourceOff,
        Self::OtherMachine,
        Self::FormatUnknown,
    ];

    /// Its kebab-case name.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::AttrMissing => "attr-missing",
            Self::NotAPackage => "not-a-package",
            Self::RefusedByMeta => "refused-by-meta",
            Self::Held => "held",
            Self::SourceOff => "source-off",
            Self::OtherMachine => "other-machine",
            Self::FormatUnknown => "format-unknown",
        }
    }
}

/// One entry left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Skipped {
    /// The declared entry's id.
    pub id: String,
    /// Why.
    pub reason: SkipWhy,
    /// What Nix said, data (may be empty).
    pub detail: String,
}

/// What the declines did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DeclinedReport {
    /// Declined and removed.
    pub honoured: Vec<String>,
    /// Declined and kept: essential programs, with the reason's id
    /// (`software-essential-why-<reason>`).
    pub refused: Vec<RefusedDecline>,
    /// Declined ids the registry does not know.
    pub unknown: Vec<String>,
    /// Declined, removed from the registry's list, and still in
    /// `system.path` through another way in.
    pub still_present: Vec<String>,
}

/// A decline the registry refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RefusedDecline {
    /// The registry id.
    pub id: String,
    /// The essential reason's id.
    pub reason: String,
}

/// The person's brain modules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct BrainReport {
    /// Compiled into this machine's brain.
    pub included: Vec<String>,
    /// With a root part that waits for an approval.
    pub awaiting_approval: Vec<Awaiting>,
}

/// A module waiting for its approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Awaiting {
    /// The module id (`my-…`).
    pub id: String,
    /// The hash an approval must name.
    pub hash: String,
}

impl Document for PersonApplied {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        if !check::hostname(&self.machine) {
            return Err(Refusal::new(
                Why::Invalid,
                "/machine",
                self.machine.as_str(),
            ));
        }
        if !check::token(&self.distro_rev) {
            return Err(Refusal::new(
                Why::Invalid,
                "/distro-rev",
                self.distro_rev.as_str(),
            ));
        }
        for (i, id) in self.brain.included.iter().enumerate() {
            if !check::person_module(id) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("/brain/included/{i}"),
                    id.as_str(),
                ));
            }
        }
        for (i, a) in self.brain.awaiting_approval.iter().enumerate() {
            if !check::person_module(&a.id) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("/brain/awaiting-approval/{i}/id"),
                    a.id.as_str(),
                ));
            }
        }
        let signature = &self.signature;
        if let Some(fp) = &signature.pinned
            && !check::sha256_hex(fp)
        {
            return Err(Refusal::new(Why::Invalid, "/signature/pinned", fp.as_str()));
        }
        if let Some(r) = &signature.refused
            && !r.why.refuses_the_signature()
        {
            return Err(Refusal::new(
                Why::Invalid,
                "/signature/refused/why",
                r.why.id(),
            ));
        }
        if let Some(i) = signature.kept_back.iter().position(|k| !k.why.keeps_back()) {
            return Err(Refusal::new(
                Why::Invalid,
                format!("/signature/kept-back/{i}/why"),
                signature.kept_back[i].why.id(),
            ));
        }
        Ok(())
    }
}

/// `distro-apps.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DistroApps {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The registry.
    pub apps: Vec<DistroApp>,
}

/// One program of the distro's registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DistroApp {
    /// The registry id (`ronoctua.apps.<id>`).
    pub id: String,
    /// Where it comes from (`nix` for a package).
    pub source: Source,
    /// Its desktop files' ids.
    pub desktop_ids: Vec<String>,
    /// `None` when it can be declined; else the reason's id.
    pub essential: Option<String>,
    /// The `profile.nix` group it belongs to.
    pub group: String,
    /// Whether this machine's profile has it.
    pub on_this_machine: bool,
    /// Whether the person declined it here.
    pub declined_here: bool,
}

impl Document for DistroApps {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        let mut seen = std::collections::BTreeSet::new();
        for (i, a) in self.apps.iter().enumerate() {
            let at = format!("/apps/{i}");
            if !check::slug(&a.id) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/id"),
                    a.id.as_str(),
                ));
            }
            if !seen.insert(a.id.as_str()) {
                return Err(Refusal::new(
                    Why::Duplicate,
                    format!("{at}/id"),
                    a.id.as_str(),
                ));
            }
        }
        Ok(())
    }
}

/// `declared-android.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DeclaredAndroid {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The apks.
    pub apps: Vec<DeclaredApk>,
}

/// Whose declaration an apk comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Declarer {
    /// The distro's table.
    Distro,
    /// The person's `declared.json`.
    Person,
}

/// One apk to reconcile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DeclaredApk {
    /// `android:<package>`.
    pub id: String,
    /// The package name.
    pub package: String,
    /// The apk in the store.
    pub apk: String,
    /// Android's version code.
    pub version_code: u64,
    /// Whose declaration.
    pub from: Declarer,
    /// The image it is for.
    pub image: AndroidImage,
}

impl Document for DeclaredAndroid {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        let mut seen = std::collections::BTreeSet::new();
        for (i, a) in self.apps.iter().enumerate() {
            let at = format!("/apps/{i}");
            if a.id != format!("android:{}", a.package) || !check::android_package(&a.package) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/id"),
                    a.id.as_str(),
                ));
            }
            if !check::store_path(&a.apk) {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/apk"),
                    a.apk.as_str(),
                ));
            }
            if !seen.insert(a.id.as_str()) {
                return Err(Refusal::new(
                    Why::Duplicate,
                    format!("{at}/id"),
                    a.id.as_str(),
                ));
            }
        }
        Ok(())
    }
}
