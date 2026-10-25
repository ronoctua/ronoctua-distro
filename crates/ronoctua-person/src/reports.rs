//! What Nix writes back under `/etc/ronoctua/` — read by the software
//! centre, the update and the reconciler, never written by Rust.
//!
//! - `person-applied.json`: what was applied, what was skipped and why, what
//!   the declines did, which person brain modules were included or wait for
//!   an approval, and what root made of the person's signature (the key it
//!   trusts, the last refusal, what it kept back — `local/signer.json`).
//! - `distro-apps.json`: the distro's registry on this machine — and, for
//!   each AppImage of its table, which pin built it: the distro's, or the
//!   one the person follows past it (`crate::distro_pins`).
//! - `declared-android.json`: the Android apps to reconcile, as store apks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::declared::{AndroidImage, GithubReleases, Source};
use crate::distro_pins::Fault;
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
    /// The declared entries the system took: in it, or made parts.
    pub applied: Vec<String>,
    /// Which part each applied entry that is one became, by the entry's id
    /// (`nix/modules/person.nix`): whether it is HERE is the part's record
    /// (`ronoctua_parts::record`). A report of a build before parts has
    /// none.
    #[serde(default)]
    pub parts: BTreeMap<String, String>,
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
    /// The update's verification held it back — said by a system built
    /// before the person's packages were parts, and read so a report of
    /// one a rollback boots stays readable; never written now.
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
    /// What the registry made of the person's followed pins. A report of a
    /// build before them has none, which reads as not following.
    #[serde(default)]
    pub followed: FollowedReport,
}

/// What the registry made of the person's followed pins
/// (`software/distro-pins.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FollowedReport {
    /// Whether the person's options say to follow — as root's copy said
    /// when the system was built.
    pub on: bool,
    /// The followed pins the registry refused and dropped — the distro's pin
    /// built in their place.
    pub refused: Vec<FollowedRefused>,
}

/// A followed pin the registry refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FollowedRefused {
    /// The id it named, as written (`unknown` when it named none).
    pub id: String,
    /// The rule that refused it.
    pub fault: Fault,
    /// The registry's sentence, English, for the journal.
    pub detail: String,
}

/// Whose pin built a distro AppImage here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PinFrom {
    /// The distro's, as its gate tested it.
    Distro,
    /// A pin the person follows, newer than the distro's and built by the
    /// update's verification: in the person's own profile.
    Followed,
}

/// A distro AppImage on this machine: its key, what it follows, and which
/// pin built it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DistroAppImage {
    /// The key it is wrapped as (`ronoctua-appimage-<key>`), and its
    /// download's store name begins with.
    pub key: String,
    /// The repository whose releases it follows, `None` for a pin moved by
    /// hand alone.
    pub follows: Option<GithubReleases>,
    /// Whose pin its part is built at — a followed one is the person's own
    /// download, in the part and never in the system.
    pub pin: PinFrom,
    /// The version built.
    pub version: String,
    /// The distro's pin: its version.
    pub distro_version: String,
    /// The distro's pin: its address — what a followed pin is not needed
    /// past.
    pub distro_url: String,
    /// The person's followed pin the registry took (the option on, the pin
    /// valid), whether newer or not; `None` without one.
    pub followed_version: Option<String>,
}

/// One program of the distro's registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DistroApp {
    /// The registry id (`ronoctua.apps.<id>`).
    pub id: String,
    /// The program's name, when the registry gives one (the table's for an
    /// AppImage or a web app, the entry's own `name` for a package) — what
    /// a screen says of a program whose launcher entry is not on this
    /// machine (declined before it was ever installed). A report of a build
    /// before the field has none.
    #[serde(default)]
    pub name: Option<String>,
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
    /// For an AppImage of the distro's table: its key and its pins. A report
    /// of a build before them has none.
    #[serde(default)]
    pub appimage: Option<DistroAppImage>,
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
            if let Some(name) = &a.name
                && !is_name(name)
            {
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("{at}/name"),
                    name.as_str(),
                ));
            }
            if let Some(image) = &a.appimage {
                let wrong = if !check::slug(&image.key) {
                    Some(("key", image.key.as_str()))
                } else if !check::token(&image.version) {
                    Some(("version", image.version.as_str()))
                } else if !check::token(&image.distro_version) {
                    Some(("distro-version", image.distro_version.as_str()))
                } else if !check::https_url(&image.distro_url) {
                    Some(("distro-url", image.distro_url.as_str()))
                } else {
                    image
                        .followed_version
                        .as_deref()
                        .filter(|v| !check::token(v))
                        .map(|v| ("followed-version", v))
                };
                if let Some((field, found)) = wrong {
                    return Err(Refusal::new(
                        Why::Invalid,
                        format!("{at}/appimage/{field}"),
                        found,
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Whether `name` is one a screen may draw as a program's name: no control
/// character — C0, DEL and C1 (`char::is_control`, Unicode's `Cc`) — and
/// not white space alone (Unicode's `White_Space`, what `str::trim` takes
/// off: U+00A0 included). ONE RULE with the registry's `isName`
/// (`nix/modules/distro-apps.nix`), which refuses the same names at
/// evaluation; `distro-app-sources-are-checked` plants the edges there.
#[must_use]
pub fn is_name(name: &str) -> bool {
    !name.trim().is_empty() && !name.chars().any(char::is_control)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(app: &str) -> String {
        format!(r#"{{"format": 1, "apps": [{app}]}}"#)
    }

    const LIBREOFFICE: &str = r#"{"id": "libreoffice", "source": "nix",
        "desktop-ids": ["startcenter.desktop"], "essential": null, "group": "office",
        "on-this-machine": true, "declined-here": true"#;

    /// FAILS IF: a report of a build before the followed pins stops reading,
    /// or an AppImage's pins are not read whole — whose pin built it, both
    /// versions, the hold — or a key that is not a slug reaches a screen.
    #[test]
    fn an_appimage_says_whose_pin_built_it() {
        let older: DistroApps =
            crate::parse(registry(&format!("{LIBREOFFICE}}}")).as_bytes()).unwrap();
        assert_eq!(older.followed, FollowedReport::default());
        assert_eq!(older.apps[0].appimage, None);
        let golden: DistroApps =
            crate::parse(include_str!("../fixtures/good/distro-apps.json").as_bytes()).unwrap();
        let cad = golden
            .apps
            .iter()
            .find_map(|a| a.appimage.as_ref())
            .expect("the golden report has an AppImage");
        assert_eq!(cad.pin, PinFrom::Followed);
        assert_eq!(
            (cad.version.as_str(), cad.distro_version.as_str()),
            ("0.9.0", "0.8.0")
        );
        assert!(golden.followed.on);
        assert_eq!(golden.followed.refused[0].fault, Fault::Asset);
        let mut bad = golden;
        if let Some(a) = bad.apps.iter_mut().find_map(|a| a.appimage.as_mut()) {
            a.key = "Bad Key".to_owned();
        }
        let refused = crate::parse::<DistroApps>(crate::to_json(&bad).as_bytes()).unwrap_err();
        assert_eq!(refused.at, "/apps/2/appimage/key");
    }

    /// FAILS IF: the registry's name is not read, a report of a build before
    /// the name stops reading (its programs would lose their page), or a
    /// name that is not one line of text reaches a screen.
    #[test]
    fn a_program_is_named_by_the_registry_or_not_at_all() {
        let named: DistroApps = crate::parse(
            registry(&format!(r#"{LIBREOFFICE}, "name": "LibreOffice"}}"#)).as_bytes(),
        )
        .unwrap();
        assert_eq!(named.apps[0].name.as_deref(), Some("LibreOffice"));
        let older: DistroApps =
            crate::parse(registry(&format!("{LIBREOFFICE}}}")).as_bytes()).unwrap();
        assert_eq!(older.apps[0].name, None);
        for bad in [
            r#""""#,
            r#""  ""#,
            r#""Libre\nOffice""#,
            r#""Libre\tOffice""#,
            r#""\u00a0""#,
            r#""\u3000 \u2028""#,
            r#""Libre\u0080Office""#,
            r#""Libre\u0085Office""#,
            r#""Libre\u009fOffice""#,
            r#""Libre\u007fOffice""#,
        ] {
            let refused = crate::parse::<DistroApps>(
                registry(&format!(r#"{LIBREOFFICE}, "name": {bad}}}"#)).as_bytes(),
            )
            .unwrap_err();
            assert_eq!(
                (refused.why, refused.at.as_str()),
                (Why::Invalid, "/apps/0/name"),
                "{bad}"
            );
        }
        // THE EDGES THE REGISTRY'S RULE AGREES ON: a non-breaking space
        // inside a name and letters past ASCII are a name.
        for good in ["Libre\u{a0}Office", "Café ☕", "Ünïcödé"] {
            assert!(is_name(good), "{good}");
        }
    }
}
