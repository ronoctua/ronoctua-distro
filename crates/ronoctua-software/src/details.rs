//! What a program's sheet shows beyond its search row, from its own source.
//!
//! | source | from |
//! |---|---|
//! | Nix | the index's row (one line of description, licence, site, command) |
//! | Flatpak | Flathub's appstream and summary answers (sizes with a missing runtime, permissions); the local catalogue offline; `flatpak info` for an installed app's permissions |
//! | AppImage | AppImageHub's kept feed, our sidecar of an installed one, and the GitHub release an install would take |
//! | Android | the verified F-Droid catalogue (summary in the person's language, anti-features, ARM only) |
//! | Web | the web app's own `.desktop` |
//!
//! Keeping a program across machines is not a line any more: it is the
//! person's declaration, one click away ([`crate::declare`]).

use serde::{Deserialize, Serialize};

use crate::found::{Badge, Picture};
use crate::search::Reach;
use crate::{AppKey, Context, Refusal};

/// One program's details.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct Details {
    /// Plain paragraphs. When the source has nothing longer than its one
    /// line (Nix, F-Droid's catalogue), that line — so it may repeat
    /// [`Details::summary`].
    pub description: Vec<String>,
    /// Bytes to download (with a Flatpak's missing runtime).
    pub download_bytes: Option<u64>,
    /// Bytes once installed (with a Flatpak's missing runtime; an installed
    /// AppImage's file).
    pub installed_bytes: Option<u64>,
    /// What it needs besides itself (a Flatpak runtime).
    pub needs: Option<String>,
    /// What it may reach (Flatpak permissions, Android permissions).
    pub permissions: Vec<String>,
    /// Anti-features (F-Droid), as their words.
    pub anti_features: Vec<String>,
    /// The Nix attribute or the app id, as a person would type it — a web
    /// app's address.
    pub reference: String,
    /// Its name, as its source gives it (data).
    pub name: String,
    /// One line on what it does (data).
    pub summary: String,
    /// The version its source offers now — what an install or an update would
    /// bring; empty when unknown. The installed one is the snapshot's.
    pub version: String,
    /// Its licence, as SPDX when known.
    pub license: Option<String>,
    /// Its site.
    pub homepage: Option<String>,
    /// Who publishes it.
    pub publisher: Option<String>,
    /// The command it runs (Nix `mainProgram`).
    pub program: Option<String>,
    /// Where its picture is.
    pub icon: Option<Picture>,
    /// What distinguishes it — never [`Badge::Installed`], which is the
    /// screen's to say from its snapshot.
    pub badges: Vec<Badge>,
}

/// The details of one program, in `language` where its source translates.
/// Runs programs and may reach the network: call it on a thread.
///
/// # Errors
///
/// The source's refusal.
pub fn of(ctx: &Context, key: &AppKey, language: &str) -> Result<Details, crate::Refusal> {
    of_with(ctx, key, language, &Reach::system())
}

/// [`of`], reaching through `reach`.
///
/// # Errors
///
/// [`Refusal::UnknownItem`] for what its source does not have;
/// [`Refusal::SourceAbsent`] for Nix without an index;
/// [`Refusal::SourceUnprepared`] for a Flatpak with no catalogue to read, or an
/// Android app before the F-Droid catalogue was fetched;
/// [`Refusal::DeclaredInTheFlake`] for a program of the system; and the
/// network's refusal when it was the only place to ask.
pub fn of_with(
    ctx: &Context,
    key: &AppKey,
    language: &str,
    reach: &Reach<'_>,
) -> Result<Details, Refusal> {
    match key {
        AppKey::Nix { attr } => nix(ctx, attr),
        AppKey::Flatpak { id } => flatpak(ctx, id, language, reach),
        AppKey::AppImage { key } => appimage(ctx, key, reach),
        AppKey::Android { package } => android(ctx, package, language),
        AppKey::Web { slug } => web(ctx, slug),
        AppKey::System { .. } => Err(Refusal::DeclaredInTheFlake),
    }
}

/// Lines as paragraphs: trimmed, and the empty ones left out.
fn paragraphs<I: IntoIterator<Item = String>>(lines: I) -> Vec<String> {
    lines
        .into_iter()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect()
}

// ─── Nix ────────────────────────────────────────────────────────────────────

fn nix(ctx: &Context, attr: &str) -> Result<Details, Refusal> {
    if !crate::nix::attr_is_valid(attr) {
        return Err(Refusal::UnknownItem);
    }
    let index = ctx
        .tools
        .nix_index
        .as_deref()
        .ok_or(Refusal::SourceAbsent)?;
    crate::nix_index::check(index)?;
    let row = crate::nix_index::row(index, attr).ok_or(Refusal::UnknownItem)?;
    let found = row.found(false);
    Ok(Details {
        description: paragraphs([row.description.clone()]),
        reference: row.attr,
        name: found.name,
        summary: found.summary,
        version: found.version,
        license: found.license,
        homepage: found.homepage,
        program: found.program,
        badges: found.badges,
        ..Details::default()
    })
}

// ─── Flatpak ────────────────────────────────────────────────────────────────

/// Flathub's answers when the person allows the network and Flathub; the local
/// catalogue otherwise, or when they fail. An INSTALLED app's permissions are
/// its own (`flatpak info --show-permissions`, overrides applied), whatever
/// Flathub's metadata says.
fn flatpak(ctx: &Context, id: &str, language: &str, reach: &Reach<'_>) -> Result<Details, Refusal> {
    if !crate::flatpak::valid_id(id) {
        return Err(Refusal::UnknownItem);
    }
    let installed =
        crate::flatpak::permissions(ctx, id).map(|k| crate::flatpak::permission_lines(&k));
    let online = ctx.prefs.online && ctx.prefs.sources.flathub;
    #[cfg(feature = "net")]
    let failed = if online {
        let installations = [
            ctx.roots.data.join("flatpak"),
            reach.flatpak_system.to_path_buf(),
        ];
        match crate::flathub::details_with(reach.net, &installations, id, language) {
            Ok(d) => return Ok(of_flathub(id, d, installed)),
            Err(r) => {
                tracing::debug!(
                    refusal = r.word(),
                    "Flathub did not answer; the local catalogue does"
                );
                Some(r)
            }
        }
    } else {
        None
    };
    #[cfg(not(feature = "net"))]
    let failed = {
        let _ = language;
        online.then_some(Refusal::Offline)
    };
    let catalogue = crate::search::local_catalogue(&ctx.roots, reach.flatpak_system);
    let entry = catalogue
        .as_deref()
        .and_then(|xml| crate::appstream::current(xml, &ctx.roots.ours_cache()).ok())
        .and_then(|c| c.apps.iter().find(|e| e.id == id).cloned());
    match (entry, installed) {
        (Some(e), permissions) => Ok(of_entry(&e, permissions.unwrap_or_default())),
        // Installed, and in no catalogue (a `.flatpakref`, a remote gone).
        (None, Some(permissions)) => Ok(Details {
            permissions,
            reference: id.to_owned(),
            name: id.to_owned(),
            badges: vec![Badge::Sandboxed],
            ..Details::default()
        }),
        (None, None) => Err(failed.unwrap_or(if catalogue.is_some() {
            Refusal::UnknownItem
        } else {
            Refusal::SourceUnprepared
        })),
    }
}

/// Flathub's details as the sheet's.
#[cfg(feature = "net")]
fn of_flathub(id: &str, d: crate::flathub::Details, installed: Option<Vec<String>>) -> Details {
    let mut badges = vec![Badge::Sandboxed];
    for (on, badge) in [
        (d.verified, Badge::Verified),
        (d.unfree, Badge::Unfree),
        (d.end_of_life, Badge::EndOfLife),
    ] {
        if on {
            badges.push(badge);
        }
    }
    Details {
        description: if d.description.is_empty() {
            paragraphs([d.summary.clone()])
        } else {
            d.description
        },
        download_bytes: d.download_bytes,
        installed_bytes: d.installed_bytes,
        needs: d.runtime_name.or(d.runtime),
        permissions: installed.unwrap_or(d.permissions),
        anti_features: Vec::new(),
        reference: id.to_owned(),
        name: d.name,
        summary: d.summary,
        version: d.version,
        license: d.license,
        homepage: d.homepage,
        publisher: d.publisher,
        program: None,
        icon: d.icon.map(|url| Picture::Url { url }),
        badges,
    }
}

/// A local catalogue entry as the sheet's. The catalogue keeps one line of
/// description (its summary) and no sizes.
fn of_entry(e: &crate::appstream::Entry, permissions: Vec<String>) -> Details {
    let found = crate::appstream::found_of(e, false);
    Details {
        description: paragraphs([e.summary.clone()]),
        needs: e.runtime.clone(),
        permissions,
        reference: e.id.clone(),
        name: found.name,
        summary: found.summary,
        version: found.version,
        license: found.license,
        homepage: found.homepage,
        publisher: found.publisher,
        icon: found.icon,
        badges: found.badges,
        ..Details::default()
    }
}

// ─── AppImage ───────────────────────────────────────────────────────────────

/// Our sidecar of an installed one, AppImageHub's kept feed (never fetched
/// here: a search keeps it), and the GitHub release an install or an update
/// would take — asked when the person allows the network and AppImageHub
/// (six hours kept, the hourly limit respected: `crate::github`), the kept
/// answer otherwise.
fn appimage(ctx: &Context, key: &str, reach: &Reach<'_>) -> Result<Details, Refusal> {
    let sidecar = crate::appimage::sidecar(&ctx.roots, key);
    let mut d = Details {
        reference: key.to_owned(),
        ..Details::default()
    };
    if let Some(s) = &sidecar {
        d.name.clone_from(&s.name);
        d.installed_bytes = std::fs::metadata(&s.file).ok().map(|m| m.len());
        d.icon = s.icon.clone().map(|path| Picture::File { path });
        if let crate::appimage::Provenance::Github(repo) = &s.source {
            d.homepage = Some(format!("https://github.com/{repo}"));
        }
    }
    #[cfg(feature = "net")]
    {
        let item = crate::appimagehub::item(&ctx.roots, key);
        if sidecar.is_none() && item.is_none() {
            return Err(Refusal::UnknownItem);
        }
        if let Some(i) = &item {
            let found = i.found(false);
            d.description = paragraphs(i.description.split("\n\n").map(str::to_owned));
            d.summary = found.summary;
            if d.name.is_empty() {
                d.name = found.name;
            }
            d.license = found.license;
            d.publisher = found.publisher;
            d.homepage = d.homepage.or(found.homepage);
            d.icon = d.icon.or(found.icon);
        }
        let repo = match sidecar.as_ref().map(|s| &s.source) {
            Some(crate::appimage::Provenance::Github(repo)) => Some(repo.clone()),
            _ => item.and_then(|i| i.github),
        };
        if let Some(repo) = repo {
            let pattern = sidecar.as_ref().and_then(|s| s.pattern.as_deref());
            let cache = crate::appimage::github_cache(&ctx.roots);
            let online = ctx.prefs.online && ctx.prefs.sources.appimagehub;
            let asset = if online {
                crate::github::latest_with(reach.net, &repo, &cache, pattern, reach.now)
                    .inspect_err(|f| tracing::debug!(%f, "no release for the sheet"))
                    .ok()
            } else {
                crate::github::kept(&cache, &repo)
                    .and_then(|k| k.release)
                    .and_then(|r| crate::github::choose(&r, pattern))
            };
            if let Some(a) = asset {
                d.version = a.tag;
                d.download_bytes = Some(a.size).filter(|s| *s > 0);
            }
        }
    }
    #[cfg(not(feature = "net"))]
    {
        let _ = reach;
        if sidecar.is_none() {
            return Err(Refusal::UnknownItem);
        }
    }
    Ok(d)
}

// ─── Android ────────────────────────────────────────────────────────────────

/// The verified F-Droid catalogue's entry, in `language`. An app installed
/// from a file is in our own record only, and says its name.
///
/// The compacted catalogue keeps each app's summary and not its description
/// or its permissions (`crate::fdroid::App`), so the description is that one
/// line and the permissions are empty.
fn android(ctx: &Context, package: &str, language: &str) -> Result<Details, Refusal> {
    let Some(app) = crate::fdroid::app(&ctx.roots, package) else {
        if let Some(r) = crate::android::record(&ctx.roots).remove(package) {
            return Ok(Details {
                reference: package.to_owned(),
                name: r.name,
                ..Details::default()
            });
        }
        return Err(if crate::fdroid::catalogue(&ctx.roots).is_some() {
            Refusal::UnknownItem
        } else {
            Refusal::SourceUnprepared
        });
    };
    let found = app.found(language, false);
    Ok(Details {
        description: paragraphs([found.summary.clone()]),
        download_bytes: found.download_bytes,
        anti_features: app.anti_features,
        reference: app.package,
        name: found.name,
        summary: found.summary,
        version: found.version,
        license: found.license,
        homepage: found.homepage,
        icon: found.icon,
        badges: found.badges,
        ..Details::default()
    })
}

// ─── Web ────────────────────────────────────────────────────────────────────

/// The web app's `.desktop`: its address is its reference.
fn web(ctx: &Context, slug: &str) -> Result<Details, Refusal> {
    let app = crate::web::read(&ctx.roots, slug).ok_or(Refusal::UnknownItem)?;
    Ok(Details {
        reference: app.url,
        name: app.name,
        icon: app.icon_file.map(|path| Picture::File { path }),
        ..Details::default()
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::search::tests::{context, plant_catalogue, plant_fdroid, scratch};

    #[cfg(feature = "net")]
    fn reach<'a>(net: &'a dyn crate::net::Fetch, system: &'a Path) -> Reach<'a> {
        Reach {
            net,
            flatpak_system: system,
            now: 1_790_000_000,
        }
    }

    #[cfg(not(feature = "net"))]
    fn reach(system: &Path) -> Reach<'_> {
        Reach {
            flatpak_system: system,
            now: 1_790_000_000,
        }
    }

    #[cfg(feature = "net")]
    mod online {
        use super::*;
        use crate::net::canned::Canned;
        use crate::search::tests::fixture;

        use crate::write_executable;

        fn of_key(
            ctx: &Context,
            key: AppKey,
            language: &str,
            net: &Canned,
            system: &Path,
        ) -> Result<Details, Refusal> {
            of_with(ctx, &key, language, &reach(net, system))
        }

        #[test]
        fn a_nix_package_is_its_index_row_and_the_line_that_keeps_it() {
            let root = scratch("details-nix");
            let ctx = context(&root);
            let net = Canned::default();
            let system = root.join("system");
            let nix =
                |attr: &str| of_key(&ctx, AppKey::Nix { attr: attr.into() }, "en", &net, &system);
            let rg = nix("ripgrep").unwrap();
            assert_eq!(rg.reference, "ripgrep");
            assert_eq!(rg.program.as_deref(), Some("rg"));
            assert_eq!(rg.license.as_deref(), Some("Unlicense"));
            assert_eq!(
                rg.homepage.as_deref(),
                Some("https://github.com/BurntSushi/ripgrep")
            );
            assert_eq!(rg.version, "15.1.0");
            assert_eq!(rg.description.len(), 1);
            assert!(
                nix("google-chrome")
                    .unwrap()
                    .badges
                    .contains(&Badge::Unfree)
            );
            assert_eq!(nix("no-such-package"), Err(Refusal::UnknownItem));
            assert_eq!(nix("../etc"), Err(Refusal::UnknownItem));
            let mut none = context(&root);
            none.tools.nix_index = None;
            assert_eq!(
                of_key(
                    &none,
                    AppKey::Nix {
                        attr: "hello".into()
                    },
                    "en",
                    &net,
                    &system
                ),
                Err(Refusal::SourceAbsent)
            );
            assert!(net.asked.borrow().is_empty(), "Nix asks nobody");
            let _ = std::fs::remove_dir_all(&root);
        }

        fn calculator_answers() -> Canned {
            Canned::new(vec![
                (
                    "https://flathub.org/api/v2/appstream/org.gnome.Calculator?locale=pt-BR",
                    200,
                    vec![],
                    fixture("flatpak/flathub-appstream-calculator.json"),
                ),
                (
                    "https://flathub.org/api/v2/summary/org.gnome.Calculator",
                    200,
                    vec![],
                    fixture("flatpak/flathub-summary-calculator.json"),
                ),
                (
                    "https://flathub.org/api/v2/summary/org.gnome.Platform",
                    200,
                    vec![],
                    fixture("flatpak/flathub-summary-platform.json"),
                ),
            ])
        }

        #[test]
        fn a_flatpak_counts_the_runtime_it_would_download_and_says_what_it_may_reach() {
            let root = scratch("details-flathub");
            let ctx = context(&root);
            let net = calculator_answers();
            let system = root.join("system");
            let key = AppKey::Flatpak {
                id: "org.gnome.Calculator".into(),
            };
            let d = of_key(&ctx, key.clone(), "pt_BR", &net, &system).unwrap();
            assert_eq!(d.name, "Calculator");
            assert_eq!(d.reference, "org.gnome.Calculator");
            // The app and GNOME 51, which neither installation has.
            assert_eq!(d.download_bytes, Some(1_921_122 + 432_185_831));
            assert_eq!(d.installed_bytes, Some(4_975_616 + 1_125_792_768));
            assert_eq!(
                d.needs.as_deref(),
                Some("GNOME Application Platform version 51")
            );
            assert!(!d.permissions.is_empty());
            assert!(!d.description.is_empty());
            assert_eq!(d.license.as_deref(), Some("GPL-3.0-or-later"));
            assert_eq!(
                d.homepage.as_deref(),
                Some("https://apps.gnome.org/Calculator")
            );
            assert!(d.badges.contains(&Badge::Sandboxed));
            assert!(!d.badges.contains(&Badge::Installed));
            // The runtime in the SYSTEM installation: nothing more to download.
            let active = system.join("runtime/org.gnome.Platform/x86_64/51/active");
            std::fs::create_dir_all(&active).unwrap();
            let net = calculator_answers();
            let d = of_key(&ctx, key, "pt_BR", &net, &system).unwrap();
            assert_eq!(d.download_bytes, Some(1_921_122));
            assert_eq!(
                net.asked.borrow().len(),
                2,
                "the runtime was not asked about"
            );
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn an_installed_flatpak_says_its_own_permissions() {
            let root = scratch("details-installed");
            let mut ctx = context(&root);
            let permissions = root.join("permissions.ini");
            std::fs::write(&permissions, fixture("flatpak/permissions.ini")).unwrap();
            let flatpak = root.join("flatpak");
            write_executable(
                &flatpak,
                &format!("#!/bin/sh\ncat '{}'\n", permissions.display()),
            );
            ctx.tools.flatpak = Some(flatpak);
            let net = calculator_answers();
            let system = root.join("system");
            let key = AppKey::Flatpak {
                id: "org.gnome.Calculator".into(),
            };
            let d = of_key(&ctx, key.clone(), "pt-BR", &net, &system).unwrap();
            let own = crate::flatpak::permission_lines(
                &String::from_utf8(fixture("flatpak/permissions.ini")).unwrap(),
            );
            assert_eq!(
                d.permissions, own,
                "overrides applied, not Flathub's metadata"
            );
            // Installed, offline, in no catalogue: still a sheet.
            ctx.prefs.online = false;
            let d = of_key(&ctx, key, "en", &Canned::default(), &system).unwrap();
            assert_eq!(d.permissions, own);
            assert_eq!(d.reference, "org.gnome.Calculator");
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn offline_a_flatpak_is_its_catalogue_entry_or_a_refusal_that_says_why() {
            let root = scratch("details-offline");
            let system = root.join("system");
            let net = Canned::default();
            let ctx = context(&root);
            let key = |id: &str| AppKey::Flatpak { id: id.into() };
            // No catalogue, and Flathub fails: the network's refusal.
            assert_eq!(
                of_key(&ctx, key("org.gnome.Calculator"), "en", &net, &system),
                Err(Refusal::Offline)
            );
            // No catalogue, and the network turned off: prepare it first.
            let mut off = context(&root);
            off.prefs.online = false;
            assert_eq!(
                of_key(&off, key("org.gnome.Calculator"), "en", &net, &system),
                Err(Refusal::SourceUnprepared)
            );
            plant_catalogue(&off.roots.data.join("flatpak"), 1_790_000_000);
            let d = of_key(&off, key("net.launchpad.Gnuclad"), "en", &net, &system).unwrap();
            assert_eq!(d.name, "Gnuclad");
            assert_eq!(d.description, std::slice::from_ref(&d.summary));
            assert_eq!(
                of_key(&off, key("org.example.Nothing"), "en", &net, &system),
                Err(Refusal::UnknownItem)
            );
            assert_eq!(
                of_key(&off, key("not an id"), "en", &net, &system),
                Err(Refusal::UnknownItem)
            );
            assert!(
                net.asked.borrow().len() == 1,
                "only the first, online, asked"
            );
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn an_appimage_is_its_catalogue_item_and_the_release_it_would_take() {
            let root = scratch("details-appimage");
            let ctx = context(&root);
            let system = root.join("system");
            let key = AppKey::AppImage {
                key: "audacity".into(),
            };
            assert_eq!(
                of_key(&ctx, key.clone(), "en", &Canned::default(), &system),
                Err(Refusal::UnknownItem),
                "nothing kept, nothing installed"
            );
            crate::appimagehub::refresh_with(
                &Canned::new(vec![(
                    crate::appimagehub::FEED,
                    200,
                    vec![],
                    fixture("appimage/feed-cut.json"),
                )]),
                &ctx.roots,
            )
            .unwrap();
            let net = Canned::new(vec![(
                "https://api.github.com/repos/audacity/audacity/releases/latest",
                200,
                vec![],
                fixture("appimage/github-audacity-latest.json"),
            )]);
            let d = of_key(&ctx, key.clone(), "en", &net, &system).unwrap();
            assert_eq!(d.name, "Audacity");
            assert_eq!(d.reference, "audacity");
            assert!(d.description.len() > 1, "the feed's paragraphs");
            assert_eq!(d.version, "Audacity-4.0.0");
            assert_eq!(d.download_bytes, Some(73_415_160));
            assert_eq!(
                d.homepage.as_deref(),
                Some("https://github.com/audacity/audacity")
            );
            // Turned off: the kept answer, and nobody asked.
            let mut off = context(&root);
            off.prefs.sources.appimagehub = false;
            let none = Canned::default();
            let d = of_key(&off, key, "en", &none, &system).unwrap();
            assert_eq!(d.download_bytes, Some(73_415_160));
            assert!(none.asked.borrow().is_empty());
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn an_installed_appimage_is_its_sidecar() {
            let root = scratch("details-sidecar");
            let mut ctx = context(&root);
            ctx.prefs.online = false;
            let file = ctx.roots.appimages().join("tool.AppImage");
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(&file, b"0123456789").unwrap();
            let sidecar = crate::appimage::Sidecar {
                key: "tool".into(),
                name: "Tool".into(),
                file,
                sha256: "0".repeat(64),
                update_info: None,
                source: crate::appimage::Provenance::File,
                tag: None,
                installed: 1,
                version: Some("2.0".into()),
                pattern: None,
                icon: None,
                verified: false,
            };
            let dir = crate::appimage::sidecars_dir(&ctx.roots).join("tool");
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join(crate::appimage::SIDECAR),
                serde_json::to_vec(&sidecar).unwrap(),
            )
            .unwrap();
            let d = of_key(
                &ctx,
                AppKey::AppImage { key: "tool".into() },
                "en",
                &Canned::default(),
                &root.join("system"),
            )
            .unwrap();
            assert_eq!((d.name.as_str(), d.installed_bytes), ("Tool", Some(10)));
            assert_eq!(d.version, "", "no release is known for a file");
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn an_android_app_is_its_verified_catalogue_entry_in_the_persons_language() {
            let root = scratch("details-android");
            let ctx = context(&root);
            let net = Canned::default();
            let system = root.join("system");
            let android = |package: &str, language: &str| {
                of_key(
                    &ctx,
                    AppKey::Android {
                        package: package.into(),
                    },
                    language,
                    &net,
                    &system,
                )
            };
            assert_eq!(
                android("org.fdroid.fdroid", "pt-BR"),
                Err(Refusal::SourceUnprepared)
            );
            plant_fdroid(&ctx.roots);
            let d = android("org.fdroid.fdroid", "pt-BR").unwrap();
            assert_eq!(d.name, "F-Droid");
            assert_eq!(
                d.summary,
                crate::search::tests::fdroid_summary("org.fdroid.fdroid", "pt-BR")
            );
            assert_eq!(d.description, std::slice::from_ref(&d.summary));
            assert_eq!(d.version, "1.23.2");
            assert!(d.download_bytes.is_some());
            let pipe = android("InfinityLoop1309.NewPipeEnhanced", "en").unwrap();
            assert_eq!(pipe.anti_features, ["NonFreeNet"]);
            assert!(pipe.badges.contains(&Badge::AntiFeatures));
            assert!(
                android("chat.simplex.app", "en")
                    .unwrap()
                    .badges
                    .contains(&Badge::ArmOnly)
            );
            assert_eq!(android("no.such.app", "en"), Err(Refusal::UnknownItem));
            assert!(net.asked.borrow().is_empty() && net.fetched.borrow().is_empty());
            let _ = std::fs::remove_dir_all(&root);
        }

        #[test]
        fn a_web_app_is_its_address_and_a_system_program_is_the_flakes() {
            let root = scratch("details-web");
            let ctx = context(&root);
            let net = Canned::default();
            let system = root.join("system");
            let apps = ctx.roots.applications();
            std::fs::create_dir_all(&apps).unwrap();
            std::fs::write(
                apps.join(crate::web::desktop_id("mail")),
                "[Desktop Entry]\nType=Application\nName=Mail\n\
                 X-Ronoctua-Web-Url=https://mail.example.org/\n\
                 X-Ronoctua-Web-Profile=Default\n",
            )
            .unwrap();
            let d = of_key(
                &ctx,
                AppKey::Web {
                    slug: "mail".into(),
                },
                "en",
                &net,
                &system,
            )
            .unwrap();
            assert_eq!(
                (d.name.as_str(), d.reference.as_str()),
                ("Mail", "https://mail.example.org/")
            );
            assert_eq!(
                of_key(
                    &ctx,
                    AppKey::Web {
                        slug: "nothing".into()
                    },
                    "en",
                    &net,
                    &system
                ),
                Err(Refusal::UnknownItem)
            );
            assert_eq!(
                of_key(
                    &ctx,
                    AppKey::System {
                        desktop_id: "firefox.desktop".into()
                    },
                    "en",
                    &net,
                    &system
                ),
                Err(Refusal::DeclaredInTheFlake)
            );
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[cfg(not(feature = "net"))]
    #[test]
    fn without_the_network_the_local_details_still_answer() {
        let root = scratch("details-local-only");
        let ctx = context(&root);
        let system = root.join("system");
        let d = of_with(
            &ctx,
            &AppKey::Nix {
                attr: "hello".into(),
            },
            "en",
            &reach(&system),
        )
        .unwrap();
        assert_eq!(d.reference, "hello");
        plant_catalogue(&ctx.roots.data.join("flatpak"), 1);
        let d = of_with(
            &ctx,
            &AppKey::Flatpak {
                id: "net.launchpad.Gnuclad".into(),
            },
            "en",
            &reach(&system),
        )
        .unwrap();
        assert_eq!(d.name, "Gnuclad");
        plant_fdroid(&ctx.roots);
        assert!(
            of_with(
                &ctx,
                &AppKey::Android {
                    package: "org.fdroid.fdroid".into()
                },
                "en",
                &reach(&system)
            )
            .is_ok()
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
