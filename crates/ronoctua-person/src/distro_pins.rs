//! `software/distro-pins.json` — the distro's AppImages this person follows
//! past the pins the distro tested.
//!
//! ```json
//! {"format": 1, "pins": [{"id": "cadcraft",
//!   "url": "https://github.com/storytold/cadcraft/releases/download/v0.9.0/cadcraft-0.9.0-linux-x86_64.AppImage",
//!   "hash": "sha256-…", "version": "0.9.0", "pinned-at": "2026-10-20T12:00:00Z"}]}
//! ```
//!
//! # Whose, and why it is data
//!
//! The distro pins every AppImage of its table at the release its gate
//! tested (`assets/distro-pins.json`), and only its owner moves those pins.
//! A person who turns on [`crate::options::Options::distro_apps_follow_releases`]
//! lets the routine update move them further on THEIR machines (owner's
//! decision, 2026-10-20): `ronoctua-software pins refresh` asks GitHub for the
//! newest release of each table entry that follows one (`follows.github`)
//! and writes it here — the person's data, in their mirror, signed with the
//! rest of their plain part and staged to root like `options.json`. Nothing
//! outside the table's repositories is ever named, and the update names no
//! program: the table says what follows, this file says where it is now.
//!
//! The machine's registry (`nix/modules/distro-apps.nix`) builds a followed
//! pin only while the option is on AND the pin is NEWER than the distro's
//! (`builtins.compareVersions`): when the owner moves the distro's pins past
//! it, the distro's wins again. A followed pin is the pin the AppImage's
//! PART is built at (`ronoctua_parts`): one that does not arrive keeps the
//! part's previous generation — the last that arrived, or the distro's.
//!
//! # One rule, twice
//!
//! A pin is judged against its table entry here ([`fault`]) and by `judge`
//! in `nix/distro-pins.nix` at the build, the two held to one table of cases
//! (`fixtures/distro-pin-cases.json`), each case naming the rule that refuses
//! it ([`Fault`]). The rules, in the order they are asked:
//!
//! 1. **id** — a slug naming a table entry that is an AppImage and follows a
//!    GitHub repository;
//! 2. **url** — exactly `https://github.com/<that repo>/releases/download/<tag>/<asset>`,
//!    the tag and the asset each one segment of `[A-Za-z0-9._+-]` (so no
//!    `/`, `?`, `#`, `\`, `%` or white space), and no `..` anywhere;
//! 3. **asset** — the asset's name matches the table's `assetPattern` (`*`
//!    globs only);
//! 4. **version** — the tag without one leading `v` before a digit (the
//!    distro's own tool's `version_of_tag`), containing a digit, and a store
//!    name's version (`[A-Za-z0-9._+-]`, at most 64): it names the download
//!    `<key>-<version>.AppImage`, the store name the distro's builder fetches
//!    it under (`nix/distro-downloads.nix`);
//! 5. **hash** — SRI SHA-256: `sha256-` and 44 characters of base64.
//!
//! The document's own [`Document::validate`] asks every rule that needs no
//! table — root's apply runs against the distro the machine runs NOW, and
//! the update moves the distro after it: the table's own questions are the
//! registry's, at the build, which drops a refused pin with a sentence
//! instead of stopping.

use serde::{Deserialize, Serialize};

use crate::declared::GithubReleases;
use crate::{Document, Refusal, Stamp, Why, check};

/// The hosts following asks: the API that answers which release is the
/// newest, and the one the download is fetched from. Turning the option on
/// lets them, and a host not let skips the following, said.
pub const HOSTS: [&str; 2] = ["api.github.com", "github.com"];

/// The file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct DistroPins {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The followed pins, one per table entry at most.
    pub pins: Vec<FollowedPin>,
}

impl Default for DistroPins {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            pins: Vec::new(),
        }
    }
}

/// One distro AppImage, followed to a newer release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FollowedPin {
    /// The table's id (`ronoctua.apps.<id>`).
    pub id: String,
    /// The release asset's address on GitHub.
    pub url: String,
    /// Its SRI SHA-256, as `fetchurl` takes it.
    pub hash: String,
    /// The release's version: the tag, without a leading `v`.
    pub version: String,
    /// When it was pinned — the later wins when two machines followed it.
    pub pinned_at: Stamp,
}

impl DistroPins {
    /// The followed pin of the table entry `id`.
    #[must_use]
    pub fn pin(&self, id: &str) -> Option<&FollowedPin> {
        self.pins.iter().find(|p| p.id == id)
    }
}

/// Why a followed pin is refused: the rule, in the order they are asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fault {
    /// The id names no AppImage of the table that follows a repository.
    Id,
    /// The address is not that repository's release download.
    Url,
    /// The asset's name does not match the table's pattern.
    Asset,
    /// The version is not the tag's, has no digit, or cannot name a store
    /// path.
    Version,
    /// The hash is not an SRI SHA-256.
    Hash,
    /// A second pin of the same id (the registry's word; the document's own
    /// refusal is [`Why::Duplicate`]).
    Duplicate,
}

impl Fault {
    /// Every one.
    pub const ALL: [Self; 6] = [
        Self::Id,
        Self::Url,
        Self::Asset,
        Self::Version,
        Self::Hash,
        Self::Duplicate,
    ];

    /// Its kebab-case name — the word the registry's judge answers too.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Url => "url",
            Self::Asset => "asset",
            Self::Version => "version",
            Self::Hash => "hash",
            Self::Duplicate => "duplicate",
        }
    }
}

/// The version a release tag is: the tag without ONE leading `v` (or `V`)
/// before a digit — `v1.2.3` is `1.2.3`, any other tag its own version. The
/// distro's pinning tool says the same (`nix/scripts/distro-pins.py`), so a
/// followed version and a distro pin compare like with like.
#[must_use]
pub fn version_of_tag(tag: &str) -> &str {
    let b = tag.as_bytes();
    if b.len() > 1 && matches!(b[0], b'v' | b'V') && b[1].is_ascii_digit() {
        &tag[1..]
    } else {
        tag
    }
}

/// The release download of `asset` under `tag` in `repo`.
#[must_use]
pub fn url_of(repo: &str, tag: &str, asset: &str) -> String {
    format!("https://github.com/{repo}/releases/download/{tag}/{asset}")
}

/// `*`-only glob over a whole name (`cadcraft-*-linux-x86_64.AppImage`).
#[must_use]
pub fn glob(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == name;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !name.starts_with(first) || name.len() < first.len() + last.len() || !name.ends_with(last) {
        return false;
    }
    let mut rest = &name[first.len()..name.len() - last.len()];
    for middle in &parts[1..parts.len() - 1] {
        match rest.find(middle) {
            Some(at) => rest = &rest[at + middle.len()..],
            None => return false,
        }
    }
    true
}

/// One segment of a tag or an asset: `[A-Za-z0-9._+-]`, at most 255 bytes.
fn segment(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 255
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-'))
}

/// One half of `owner/name`: `[A-Za-z0-9_.-]`, the table's own rule
/// (`followsProblems` in `nix/modules/distro-apps.nix`).
fn repo_half(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// A version a store name can carry, with a digit in it.
fn store_version(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 64
        && v.bytes().any(|c| c.is_ascii_digit())
        && v.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'+' | b'-'))
}

/// An SRI SHA-256: `sha256-`, 43 characters of base64 and one `=`.
fn sri_sha256(hash: &str) -> bool {
    hash.strip_prefix("sha256-").is_some_and(|b| {
        let b = b.as_bytes();
        b.len() == 44
            && b[43] == b'='
            && b[..43]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/'))
    })
}

/// The repository (`owner/name`), the tag and the asset of a release
/// download, when the address is one.
fn parts(url: &str) -> Option<(String, &str, &str)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let p: Vec<&str> = rest.split('/').collect();
    match p.as_slice() {
        [owner, name, "releases", "download", tag, asset]
            if repo_half(owner) && repo_half(name) && segment(tag) && segment(asset) =>
        {
            Some((format!("{owner}/{name}"), tag, asset))
        }
        _ => None,
    }
}

/// Why `pin` is refused against its table entry — `follows` is the
/// repository the table's entry of that id follows, `None` when the table
/// has no AppImage of that id or it follows nothing — or `None` when it is a
/// pin. The rules of the module's header, in their order. Pure.
#[must_use]
pub fn fault(pin: &FollowedPin, follows: Option<&GithubReleases>) -> Option<Fault> {
    let Some(follows) = follows.filter(|_| check::slug(&pin.id)) else {
        return Some(Fault::Id);
    };
    let Some((repo, tag, asset)) = parts(&pin.url).filter(|_| !pin.url.contains("..")) else {
        return Some(Fault::Url);
    };
    if repo != follows.repo {
        return Some(Fault::Url);
    }
    if !glob(&follows.asset_pattern, asset) {
        return Some(Fault::Asset);
    }
    if pin.version != version_of_tag(tag) || !store_version(&pin.version) {
        return Some(Fault::Version);
    }
    if !sri_sha256(&pin.hash) {
        return Some(Fault::Hash);
    }
    None
}

/// The rules [`fault`] asks that need no table: the pin judged against an
/// entry made of its own address — the repository it names, and every asset
/// — so only the shape of each field is asked. Pure.
#[must_use]
pub fn shape_fault(pin: &FollowedPin) -> Option<Fault> {
    let itself = parts(&pin.url).map(|(repo, _, _)| GithubReleases {
        repo,
        asset_pattern: "*".to_owned(),
    });
    match itself {
        Some(follows) => fault(pin, Some(&follows)),
        // No repository to judge it against: the id is still asked first.
        None if !check::slug(&pin.id) => Some(Fault::Id),
        None => Some(Fault::Url),
    }
}

impl Document for DistroPins {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        let mut seen = std::collections::BTreeSet::new();
        for (i, p) in self.pins.iter().enumerate() {
            if let Some(f) = shape_fault(p) {
                let (field, found) = match f {
                    Fault::Id | Fault::Duplicate => ("id", p.id.as_str()),
                    Fault::Url | Fault::Asset => ("url", p.url.as_str()),
                    Fault::Version => ("version", p.version.as_str()),
                    Fault::Hash => ("hash", p.hash.as_str()),
                };
                return Err(Refusal::new(
                    Why::Invalid,
                    format!("/pins/{i}/{field}"),
                    found,
                ));
            }
            if !seen.insert(p.id.as_str()) {
                return Err(Refusal::new(
                    Why::Duplicate,
                    format!("/pins/{i}/id"),
                    p.id.as_str(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared table: the table entries the cases name, in the shape of
    /// `assets/distro-apps.nix` (what the Nix judge reads), and the cases.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Shared {
        table: std::collections::BTreeMap<String, serde_json::Value>,
        cases: Vec<Case>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Case {
        case: String,
        pin: serde_json::Value,
        fault: Option<Fault>,
    }

    /// What the table's entry `id` follows, read as the Nix judge reads it:
    /// an AppImage with `follows.github`, or nothing.
    fn follows_of(shared: &Shared, id: &str) -> Option<GithubReleases> {
        let t = shared.table.get(id)?;
        if t.get("source")?.as_str()? != "appimage" {
            return None;
        }
        let g = t.get("follows")?.get("github")?;
        Some(GithubReleases {
            repo: g.get("repo")?.as_str()?.to_owned(),
            asset_pattern: g.get("assetPattern")?.as_str()?.to_owned(),
        })
    }

    /// FAILS IF: a pin the shared table calls good is refused, or one it
    /// refuses is taken or refused by another rule than the one it names —
    /// the registry's judge (`nix/distro-pins.nix`) reads the same table in
    /// `distro-apps-follow-only-when-newer`, so the two never disagree.
    #[test]
    fn a_pin_is_judged_as_the_shared_table_says() {
        let shared: Shared =
            serde_json::from_str(include_str!("../fixtures/distro-pin-cases.json"))
                .expect("the table");
        assert!(shared.cases.len() >= 20, "the table lost its cases");
        for f in Fault::ALL.into_iter().filter(|f| *f != Fault::Duplicate) {
            assert!(
                shared.cases.iter().any(|c| c.fault == Some(f)),
                "no case refused by the rule {}",
                f.id()
            );
        }
        assert!(shared.cases.iter().any(|c| c.fault.is_none()));
        for c in &shared.cases {
            let pin: FollowedPin = serde_json::from_value(c.pin.clone())
                .unwrap_or_else(|e| panic!("{}: not a pin: {e}", c.case));
            let follows = follows_of(&shared, &pin.id);
            assert_eq!(fault(&pin, follows.as_ref()), c.fault, "{}", c.case);
            // THE DOCUMENT'S OWN RULES never refuse what the table takes,
            // and refuse by the same rule whenever no table is needed.
            let shape = shape_fault(&pin);
            if c.fault.is_none() {
                assert_eq!(shape, None, "{}", c.case);
            }
            if matches!(c.fault, Some(Fault::Version | Fault::Hash)) {
                assert_eq!(shape, c.fault, "{}", c.case);
            }
        }
    }

    /// FAILS IF: a tag's version is not the distro tool's, or the
    /// verification's id does not name its table entry both ways.
    #[test]
    fn a_tag_is_its_version() {
        assert_eq!(version_of_tag("v0.9.0"), "0.9.0");
        assert_eq!(version_of_tag("V2"), "2");
        assert_eq!(version_of_tag("vintage-1"), "vintage-1");
        assert_eq!(version_of_tag("1.0"), "1.0");
        assert_eq!(version_of_tag("v"), "v");
        assert!(glob("a-*-x.AppImage", "a-1.0-x.AppImage"));
        assert!(!glob("a-*-x.AppImage", "a-1.0-y.AppImage"));
    }

    /// FAILS IF: the document takes a pin a table-free rule refuses, two
    /// pins of one id, or refuses a good one.
    #[test]
    fn the_document_refuses_by_its_own_rules() {
        let good = FollowedPin {
            id: "cadcraft".to_owned(),
            url: url_of(
                "storytold/cadcraft",
                "v0.9.0",
                "cadcraft-0.9.0-linux-x86_64.AppImage",
            ),
            hash: format!("sha256-{}=", "A".repeat(43)),
            version: "0.9.0".to_owned(),
            pinned_at: Stamp::parse("2026-10-20T12:00:00Z").unwrap(),
        };
        let doc = |pins: Vec<FollowedPin>| DistroPins {
            format: crate::FORMAT,
            pins,
        };
        assert!(doc(vec![good.clone()]).validate().is_ok());
        let twice = doc(vec![good.clone(), good.clone()])
            .validate()
            .unwrap_err();
        assert_eq!(
            (twice.why, twice.at.as_str()),
            (Why::Duplicate, "/pins/1/id")
        );
        let mut odd = good.clone();
        odd.version = "1.0".to_owned();
        let r = doc(vec![odd]).validate().unwrap_err();
        assert_eq!((r.why, r.at.as_str()), (Why::Invalid, "/pins/0/version"));
        let mut far = good;
        far.url = "https://example.org/cadcraft.AppImage".to_owned();
        assert_eq!(doc(vec![far]).validate().unwrap_err().at, "/pins/0/url");
        assert_eq!(
            DistroPins::default().pins.len(),
            0,
            "nothing followed is an empty list"
        );
    }
}
