//! `software/declared.json` — the programs declared for this person.
//!
//! ```json
//! {"format": 1, "apps": [{"source": "nix", "id": "nix:ripgrep", "machines": null,
//!   "edited-at": "2026-10-05T12:00:00Z", "edited-on": "noctua", "attr": "ripgrep", "unfree": false}]}
//! ```
//!
//! An entry's `id` is `"<source>:<source-id>"` — the attribute, the Flatpak
//! id, the AppImage's key, the web app's slug, the Android package — and is
//! checked to say exactly that. `machines` is `null` for every machine of the
//! person, or the hostnames it is declared on.

use serde::{Deserialize, Serialize};

use crate::{Document, Refusal, Stamp, Why, check};

/// The file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Declared {
    /// Always [`crate::FORMAT`].
    pub format: u32,
    /// The entries, in the order the person declared them.
    pub apps: Vec<Entry>,
}

impl Default for Declared {
    fn default() -> Self {
        Self {
            format: crate::FORMAT,
            apps: Vec::new(),
        }
    }
}

/// Where a declared program comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// nixpkgs, into the person's `home.packages`.
    Nix,
    /// Flathub (or another remote), through nix-flatpak.
    Flatpak,
    /// An AppImage, wrapped by `appimageTools` on a pinned download.
    #[serde(rename = "appimage")]
    AppImage,
    /// A web app, as a desktop entry.
    Web,
    /// An Android app, a pinned apk the reconciler installs.
    Android,
}

impl Source {
    /// Every one.
    pub const ALL: [Self; 5] = [
        Self::Nix,
        Self::Flatpak,
        Self::AppImage,
        Self::Web,
        Self::Android,
    ];

    /// Its word, the first half of an entry's id.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Nix => "nix",
            Self::Flatpak => "flatpak",
            Self::AppImage => "appimage",
            Self::Web => "web",
            Self::Android => "android",
        }
    }
}

/// One declared program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "kebab-case")]
pub enum Entry {
    /// A nixpkgs package.
    Nix(NixApp),
    /// A Flatpak app.
    Flatpak(FlatpakApp),
    /// An AppImage.
    #[serde(rename = "appimage")]
    AppImage(AppImageApp),
    /// A web app.
    Web(WebApp),
    /// An Android app.
    Android(AndroidApp),
}

/// A nixpkgs package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct NixApp {
    /// `nix:<attr>`.
    pub id: String,
    /// `None` for every machine, or the hostnames.
    pub machines: Option<Vec<String>>,
    /// When it was last edited.
    pub edited_at: Stamp,
    /// Which machine edited it.
    pub edited_on: String,
    /// The attribute path, without `nixpkgs#`.
    pub attr: String,
    /// The person said yes to an unfree licence when declaring it.
    pub unfree: bool,
}

/// A Flatpak app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FlatpakApp {
    /// `flatpak:<app-id>`.
    pub id: String,
    /// `None` for every machine, or the hostnames.
    pub machines: Option<Vec<String>>,
    /// When it was last edited.
    pub edited_at: Stamp,
    /// Which machine edited it.
    pub edited_on: String,
    /// The reverse-DNS id.
    pub app_id: String,
    /// The remote (`flathub`).
    pub origin: String,
}

/// An AppImage's pinned download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Pin {
    /// `https://` only.
    pub url: String,
    /// SRI (`sha256-…`), as `fetchurl` takes it.
    pub hash: String,
    /// The version the download is.
    pub version: String,
    /// When the pin was taken.
    pub pinned_at: Stamp,
}

/// Where a new AppImage pin may be looked for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Follows {
    /// A GitHub repository's releases.
    pub github: GithubReleases,
}

/// A GitHub repository's releases, and which asset is the AppImage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GithubReleases {
    /// `owner/name`.
    pub repo: String,
    /// A glob over the asset's name (`*-x86_64.AppImage`).
    pub asset_pattern: String,
}

/// An AppImage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AppImageApp {
    /// `appimage:<key>`.
    pub id: String,
    /// `None` for every machine, or the hostnames.
    pub machines: Option<Vec<String>>,
    /// When it was last edited.
    pub edited_at: Stamp,
    /// Which machine edited it.
    pub edited_on: String,
    /// The folded, file-safe key (`obsidian`).
    pub key: String,
    /// The program's name, data (never translated).
    pub name: String,
    /// The download.
    pub pin: Pin,
    /// Where a newer pin is looked for, if anywhere.
    pub follows: Option<Follows>,
    /// The SHA-256 of its icon under `software/icons/`.
    pub icon: Option<String>,
}

/// A web app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WebApp {
    /// `web:<slug>`.
    pub id: String,
    /// `None` for every machine, or the hostnames.
    pub machines: Option<Vec<String>>,
    /// When it was last edited.
    pub edited_at: Stamp,
    /// Which machine edited it.
    pub edited_on: String,
    /// The slug its desktop entry is named after.
    pub slug: String,
    /// Its name, data.
    pub name: String,
    /// What it opens.
    pub url: String,
    /// The Chrome profile it opens in.
    pub profile: String,
    /// The SHA-256 of its icon under `software/icons/`.
    pub icon: Option<String>,
    /// Its freedesktop category (`Network`).
    pub category: String,
}

/// An apk's pinned download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct ApkPin {
    /// `https://` only.
    pub url: String,
    /// SRI (`sha256-…`).
    pub hash: String,
    /// The version name.
    pub version: String,
    /// Android's version code.
    pub version_code: u64,
}

/// Who vouches for an apk's hash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApkFrom {
    /// F-Droid's signed index.
    FDroid,
    /// A direct address the person gave (said unverified when declared).
    Direct,
}

/// Which Android system image an app is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AndroidImage {
    /// LineageOS with Google's apps.
    Gapps,
    /// LineageOS without Google.
    Vanilla,
}

/// An Android app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct AndroidApp {
    /// `android:<package>`.
    pub id: String,
    /// `None` for every machine, or the hostnames.
    pub machines: Option<Vec<String>>,
    /// When it was last edited.
    pub edited_at: Stamp,
    /// Which machine edited it.
    pub edited_on: String,
    /// The package name.
    pub package: String,
    /// The download.
    pub pin: ApkPin,
    /// Who vouches for it.
    pub from: ApkFrom,
    /// The image it is for.
    pub image: AndroidImage,
}

impl Entry {
    /// Its source.
    #[must_use]
    pub const fn source(&self) -> Source {
        match self {
            Self::Nix(_) => Source::Nix,
            Self::Flatpak(_) => Source::Flatpak,
            Self::AppImage(_) => Source::AppImage,
            Self::Web(_) => Source::Web,
            Self::Android(_) => Source::Android,
        }
    }

    /// `"<source>:<source-id>"`.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Nix(a) => &a.id,
            Self::Flatpak(a) => &a.id,
            Self::AppImage(a) => &a.id,
            Self::Web(a) => &a.id,
            Self::Android(a) => &a.id,
        }
    }

    /// What the source calls it.
    #[must_use]
    pub fn source_id(&self) -> &str {
        match self {
            Self::Nix(a) => &a.attr,
            Self::Flatpak(a) => &a.app_id,
            Self::AppImage(a) => &a.key,
            Self::Web(a) => &a.slug,
            Self::Android(a) => &a.package,
        }
    }

    /// The machines it is declared on; `None` for all.
    #[must_use]
    pub fn machines(&self) -> Option<&[String]> {
        match self {
            Self::Nix(a) => a.machines.as_deref(),
            Self::Flatpak(a) => a.machines.as_deref(),
            Self::AppImage(a) => a.machines.as_deref(),
            Self::Web(a) => a.machines.as_deref(),
            Self::Android(a) => a.machines.as_deref(),
        }
    }

    /// When it was last edited.
    #[must_use]
    pub const fn edited_at(&self) -> &Stamp {
        match self {
            Self::Nix(a) => &a.edited_at,
            Self::Flatpak(a) => &a.edited_at,
            Self::AppImage(a) => &a.edited_at,
            Self::Web(a) => &a.edited_at,
            Self::Android(a) => &a.edited_at,
        }
    }

    /// Which machine edited it last.
    #[must_use]
    pub fn edited_on(&self) -> &str {
        match self {
            Self::Nix(a) => &a.edited_on,
            Self::Flatpak(a) => &a.edited_on,
            Self::AppImage(a) => &a.edited_on,
            Self::Web(a) => &a.edited_on,
            Self::Android(a) => &a.edited_on,
        }
    }

    /// Whether it is declared on `host`.
    #[must_use]
    pub fn applies_to(&self, host: &str) -> bool {
        self.machines()
            .is_none_or(|hosts| hosts.iter().any(|h| h == host))
    }

    /// The same entry with what a later edit may settle alone — the two
    /// stamps and the pin — set to one fixed value, so two entries that
    /// differ only there compare equal ([`crate::merge::Keyed::minor_change`]).
    #[must_use]
    pub fn settled(&self) -> Self {
        let epoch = Stamp::from_unix_s(0);
        let mut e = self.clone();
        match &mut e {
            Self::Nix(a) => {
                a.edited_at = epoch;
                a.edited_on.clear();
            }
            Self::Flatpak(a) => {
                a.edited_at = epoch;
                a.edited_on.clear();
            }
            Self::AppImage(a) => {
                a.edited_at = epoch.clone();
                a.edited_on.clear();
                a.pin = Pin {
                    url: String::new(),
                    hash: String::new(),
                    version: String::new(),
                    pinned_at: epoch,
                };
            }
            Self::Web(a) => {
                a.edited_at = epoch;
                a.edited_on.clear();
            }
            Self::Android(a) => {
                a.edited_at = epoch;
                a.edited_on.clear();
                a.pin = ApkPin {
                    url: String::new(),
                    hash: String::new(),
                    version: String::new(),
                    version_code: 0,
                };
            }
        }
        e
    }

    fn validate(&self, at: &str) -> Result<(), Refusal> {
        let invalid = |field: &str, found: &str| {
            Err(Refusal::new(
                Why::Invalid,
                format!("{at}/{field}"),
                format!("{found:?}"),
            ))
        };
        let wanted = format!("{}:{}", self.source().word(), self.source_id());
        if self.id() != wanted {
            return invalid("id", self.id());
        }
        crate::declined::machines_are_valid(self.machines(), at)?;
        if !check::hostname(self.edited_on()) {
            return invalid("edited-on", self.edited_on());
        }
        match self {
            Self::Nix(a) => {
                if !check::nix_attr(&a.attr) {
                    return invalid("attr", &a.attr);
                }
            }
            Self::Flatpak(a) => {
                if !check::flatpak_id(&a.app_id) {
                    return invalid("app-id", &a.app_id);
                }
                if !check::remote_name(&a.origin) {
                    return invalid("origin", &a.origin);
                }
            }
            Self::AppImage(a) => {
                if !check::slug(&a.key) {
                    return invalid("key", &a.key);
                }
                if !check::name(&a.name) {
                    return invalid("name", &a.name);
                }
                pin_is_valid(
                    &a.pin.url,
                    &a.pin.hash,
                    &a.pin.version,
                    &format!("{at}/pin"),
                )?;
                if let Some(f) = &a.follows {
                    let repo_ok = f.github.repo.split_once('/').is_some_and(|(owner, name)| {
                        check::remote_name(owner) && check::remote_name(name)
                    });
                    if !repo_ok {
                        return invalid("follows/github/repo", &f.github.repo);
                    }
                    if !check::token(&f.github.asset_pattern) {
                        return invalid("follows/github/asset-pattern", &f.github.asset_pattern);
                    }
                }
                if let Some(icon) = a.icon.as_deref().filter(|i| !check::sha256_hex(i)) {
                    return invalid("icon", icon);
                }
            }
            Self::Web(a) => {
                if !check::slug(&a.slug) {
                    return invalid("slug", &a.slug);
                }
                if !check::name(&a.name) {
                    return invalid("name", &a.name);
                }
                if !check::web_url(&a.url) {
                    return invalid("url", &a.url);
                }
                if !check::name(&a.profile) {
                    return invalid("profile", &a.profile);
                }
                if !check::token(&a.category) {
                    return invalid("category", &a.category);
                }
                if let Some(icon) = a.icon.as_deref().filter(|i| !check::sha256_hex(i)) {
                    return invalid("icon", icon);
                }
            }
            Self::Android(a) => {
                if !check::android_package(&a.package) {
                    return invalid("package", &a.package);
                }
                pin_is_valid(
                    &a.pin.url,
                    &a.pin.hash,
                    &a.pin.version,
                    &format!("{at}/pin"),
                )?;
            }
        }
        Ok(())
    }
}

fn pin_is_valid(url: &str, hash: &str, version: &str, at: &str) -> Result<(), Refusal> {
    if !check::https_url(url) {
        return Err(Refusal::new(Why::Invalid, format!("{at}/url"), url));
    }
    if !check::sri(hash) {
        return Err(Refusal::new(Why::Invalid, format!("{at}/hash"), hash));
    }
    if !check::token(version) {
        return Err(Refusal::new(Why::Invalid, format!("{at}/version"), version));
    }
    Ok(())
}

impl Declared {
    /// The entry with this id.
    #[must_use]
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.apps.iter().find(|e| e.id() == id)
    }

    /// The entries declared on `host`.
    pub fn on<'a>(&'a self, host: &'a str) -> impl Iterator<Item = &'a Entry> + 'a {
        self.apps.iter().filter(move |e| e.applies_to(host))
    }
}

impl Document for Declared {
    fn format(&self) -> u32 {
        self.format
    }

    fn validate(&self) -> Result<(), Refusal> {
        let mut seen = std::collections::BTreeSet::new();
        for (i, entry) in self.apps.iter().enumerate() {
            let at = format!("/apps/{i}");
            entry.validate(&at)?;
            if !seen.insert(entry.id()) {
                return Err(Refusal::new(Why::Duplicate, format!("{at}/id"), entry.id()));
            }
        }
        Ok(())
    }
}
