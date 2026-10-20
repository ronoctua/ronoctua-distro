//! The pins of a declaration — the exact download a declared AppImage or
//! Android app is — and looking for newer ones.
//!
//! # Why a pin, and what it is
//!
//! A declaration is built on the person's other machines months later; an
//! address that serves new bytes under the same name would build something
//! else there, or nothing. So a declared download is an `https://` address
//! AND the SRI hash of what it served (`fetchurl`'s pair), and moving it is a
//! deliberate act: the update's `person-pins` step and the screen's "look for
//! a newer download" ([`refresh`]), both through the declaration.
//!
//! # Where the hash comes from, cheapest first
//!
//! * **The bytes already here.** An AppImage this crate integrated keeps its
//!   SHA-256 in its sidecar; when the release asset IS those bytes (GitHub's
//!   digest says so, or the tag is the one it was installed from) the pin is
//!   that hash, and the file goes into the store under the name the builder
//!   fetches it as (`nix store add --mode flat`) — the next rebuild here
//!   downloads nothing.
//! * **The catalogue's.** F-Droid's signed index carries each apk's SHA-256,
//!   which is the verified hash (`fdroid::App::sha256`).
//! * **A download.** Otherwise `nix store prefetch-file --json` fetches the
//!   asset under that same name and answers its hash — checked against
//!   GitHub's digest when there is one, and only once the person let the
//!   asset's host (a `Pass`, as every download of this crate).

use std::collections::BTreeMap;
use std::path::Path;

use ronoctua_person::declared::{ApkFrom, ApkPin};
use ronoctua_person::local::HeldWhy;
use ronoctua_person::reports::{PersonApplied, SkipWhy};

use crate::{Failure, Refusal, Roots};

/// The SRI form of a SHA-256 given in hex (`sha256-<base64>`); `None` for
/// anything that is not 64 hex digits.
#[must_use]
pub fn sri_of_hex(hex: &str) -> Option<String> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let bytes: Vec<u8> = (0..32)
        .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16))
        .collect::<Result<_, _>>()
        .ok()?;
    Some(format!("sha256-{}", crate::fdroid::base64(&bytes)))
}

/// A version as a pin may carry it (`ronoctua-person`'s token): the tag or
/// the version name, unless it has spaces, quotes or is too long.
#[must_use]
pub fn version_of(raw: &str) -> String {
    let raw = raw.trim();
    let ok = !raw.is_empty()
        && raw.len() <= 64
        && raw
            .bytes()
            .all(|c| c.is_ascii_graphic() && c != b'"' && c != b'\\');
    if ok {
        raw.to_owned()
    } else {
        "pinned".to_owned()
    }
}

/// The store name a declared AppImage's download is fetched under — the
/// builder's own (`nix/declared-apps/appimage.nix`, `versionOf`): the key,
/// the version when a store name can carry it, else `pinned`.
#[must_use]
pub fn store_name(key: &str, version: &str) -> String {
    let ok = !version.is_empty()
        && version.len() <= 64
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'));
    let version = if ok { version } else { "pinned" };
    format!("{key}-{version}.AppImage")
}

/// The asset pattern a release's next AppImage is looked for by: its name
/// with the version replaced by `*` (`Obsidian-1.6.7.AppImage` →
/// `Obsidian-*.AppImage`); the exact name when the version is not in it.
#[must_use]
pub fn pattern_of(asset: &str, tag: &str) -> String {
    let bare = tag.trim_start_matches(['v', 'V']);
    if !bare.is_empty() && asset.contains(bare) {
        asset.replacen(bare, "*", 1)
    } else {
        asset.to_owned()
    }
}

/// `nix store prefetch-file --json`, under the builder's name, checked
/// against a hash when one is known.
#[must_use]
pub fn prefetch_argv(url: &str, name: &str, expected: Option<&str>) -> Vec<String> {
    let mut argv: Vec<String> = [
        "store",
        "prefetch-file",
        "--json",
        "--hash-type",
        "sha256",
        "--name",
        name,
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if let Some(hash) = expected {
        argv.push("--expected-hash".into());
        argv.push(hash.to_owned());
    }
    argv.push("--".into());
    argv.push(url.to_owned());
    argv
}

/// `nix store add --mode flat`: a file already here, into the store under the
/// name the builder fetches it as — the same path a `fetchurl` of it makes.
#[must_use]
pub fn store_add_argv(name: &str, file: &Path) -> Vec<String> {
    vec![
        "store".into(),
        "add".into(),
        "--mode".into(),
        "flat".into(),
        "--hash-algo".into(),
        "sha256".into(),
        "--name".into(),
        name.into(),
        "--".into(),
        file.display().to_string(),
    ]
}

/// The hash `prefetch-file --json` answered.
#[must_use]
pub fn hash_of_prefetch(json: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(json.trim()).ok()?;
    let hash = v.get("hash")?.as_str()?;
    hash.starts_with("sha256-").then(|| hash.to_owned())
}

/// The pin of an Android app as F-Droid's kept catalogue offers it now: the
/// apk's address in the repository and the hash the signed index gives.
///
/// # Errors
///
/// [`Refusal::NothingToPin`] for a package the catalogue does not have (an
/// app from a file, or from the Play Store), [`Refusal::Integrity`] for a
/// hash that is not one.
pub fn android(roots: &Roots, package: &str) -> Result<(ApkPin, ApkFrom), Failure> {
    let app = crate::fdroid::app(roots, package).ok_or_else(|| Failure {
        refusal: Some(Refusal::NothingToPin),
        trouble: format!("F-Droid's catalogue here has no {package}"),
    })?;
    let hash = sri_of_hex(&app.sha256.to_ascii_lowercase())
        .ok_or_else(|| Failure::refused(Refusal::Integrity))?;
    let version = if app.version_name.is_empty() {
        app.version_code.to_string()
    } else {
        app.version_name.clone()
    };
    Ok((
        ApkPin {
            url: format!(
                "{}/{}",
                crate::fdroid::REPO,
                app.apk.trim_start_matches('/')
            ),
            hash,
            version: version_of(&version),
            version_code: app.version_code,
        },
        ApkFrom::FDroid,
    ))
}

/// The pin of an integrated AppImage, and where newer ones are looked for.
#[cfg(feature = "net")]
pub mod appimage {
    use ronoctua_person::Stamp;
    use ronoctua_person::declared::{Follows, GithubReleases, Pin};

    use super::{Failure, Refusal};
    use super::{hash_of_prefetch, pattern_of, prefetch_argv, sri_of_hex, store_add_argv};
    use super::{store_name, version_of};
    use crate::Context;
    use crate::appimage::{Provenance, Sidecar};
    use crate::github::{Asset, Fetch};
    use crate::progress::{Phase, Report};
    use crate::run::{Call, tool};

    /// The asset a sidecar's repository offers now, the pattern it is
    /// followed by, and the repository.
    ///
    /// # Errors
    ///
    /// [`Refusal::NothingToPin`] for an AppImage that came from a file (no
    /// repository), and GitHub's refusals.
    pub fn asset_of(
        ctx: &Context,
        s: &Sidecar,
        net: &dyn Fetch,
        now: u64,
    ) -> Result<(Asset, String, String), Failure> {
        let Provenance::Github(repo) = &s.source else {
            return Err(Failure {
                refusal: Some(Refusal::NothingToPin),
                trouble: format!("{} came from a file", s.key),
            });
        };
        let asset = crate::github::latest_with(
            net,
            repo,
            &crate::appimage::github_cache(&ctx.roots),
            s.pattern.as_deref(),
            now,
        )?;
        let pattern = s
            .pattern
            .clone()
            .unwrap_or_else(|| pattern_of(&asset.name, &asset.tag));
        Ok((asset, pattern, repo.clone()))
    }

    /// The pin of `asset` for the AppImage `key`, given the bytes this machine
    /// has of it (`here`: the integrated file and its SHA-256, when they are
    /// the asset's).
    ///
    /// # Errors
    ///
    /// No Nix, a download that failed, [`Refusal::Integrity`] for one that
    /// is not what GitHub's digest says.
    pub fn pin(
        ctx: &Context,
        key: &str,
        asset: &Asset,
        here: Option<(&std::path::Path, &str)>,
        pinned_at: Stamp,
        report: &mut dyn Report,
    ) -> Result<Pin, Failure> {
        pin_as(
            ctx,
            key,
            &version_of(&asset.tag),
            asset,
            here,
            pinned_at,
            report,
        )
    }

    /// [`pin`], under the `version` the builder names the download by — a
    /// distro AppImage's is the tag without its `v`
    /// (`ronoctua_person::distro_pins::version_of_tag`), the name the
    /// distro's builder fetches it under (`nix/distro-downloads.nix`).
    ///
    /// # Errors
    ///
    /// As [`pin`].
    pub fn pin_as(
        ctx: &Context,
        key: &str,
        version: &str,
        asset: &Asset,
        here: Option<(&std::path::Path, &str)>,
        pinned_at: Stamp,
        report: &mut dyn Report,
    ) -> Result<Pin, Failure> {
        let nix = tool(ctx.tools.nix.as_ref())?;
        let version = version.to_owned();
        let name = store_name(key, &version);
        let digest = asset.sha256().and_then(|h| sri_of_hex(&h));
        let hash = match here {
            Some((file, sha256)) => {
                let hash =
                    sri_of_hex(sha256).ok_or_else(|| Failure::refused(Refusal::Integrity))?;
                report.step(Phase::Declaring, 0, 0, &name);
                // INTO THE STORE under the builder's name: the next rebuild
                // here finds it. A store that refuses is no loss but a
                // download later, and the journal says why.
                if let Err(e) = Call::new(nix, store_add_argv(&name, file))
                    .changing()
                    .quietly(ctx)
                    .and_then(crate::run::Ran::ok)
                {
                    tracing::warn!(error = %e, "an AppImage already here could not go into the store");
                }
                hash
            }
            None => {
                // THE HOST IS ASKED FIRST: Nix downloads it, and nothing goes
                // to a host the person has not let (`net::pass_for`, the
                // same pass every download of this crate takes).
                let _pass = crate::net::pass_for(&asset.url)?;
                report.step(Phase::Downloading, 0, asset.size, &asset.name);
                let ran = Call::new(nix, prefetch_argv(&asset.url, &name, digest.as_deref()))
                    .changing()
                    .keep_stdout()
                    .quietly(ctx)?;
                if ran.rehearsed {
                    digest.unwrap_or_default()
                } else if ran.code != Some(0) {
                    let integrity = ran.tail.iter().any(|l| l.contains("hash mismatch"));
                    return Err(Failure {
                        refusal: Some(if integrity {
                            Refusal::Integrity
                        } else {
                            Refusal::ToolFailed
                        }),
                        trouble: ran.tail.join("\n"),
                    });
                } else {
                    hash_of_prefetch(&ran.stdout).ok_or_else(|| Failure {
                        refusal: Some(Refusal::ToolFailed),
                        trouble: "nix store prefetch-file --json: no sha256 hash".into(),
                    })?
                }
            }
        };
        Ok(Pin {
            url: asset.url.clone(),
            hash,
            version,
            pinned_at,
        })
    }

    /// Where an AppImage of `repo` is followed.
    #[must_use]
    pub fn follows(repo: &str, pattern: &str) -> Follows {
        Follows {
            github: GithubReleases {
                repo: repo.to_owned(),
                asset_pattern: pattern.to_owned(),
            },
        }
    }

    /// Whether the bytes this machine has ARE the asset: GitHub's digest
    /// says so, or (no digest) it is the tag they were installed from.
    #[must_use]
    pub fn is_here(s: &Sidecar, asset: &Asset) -> bool {
        match asset.sha256() {
            Some(digest) => digest == s.sha256,
            None => s.tag.as_deref() == Some(asset.tag.as_str()),
        }
    }
}

/// What a look at a declared AppImage's newest release finds to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Nothing: the newest release is the pinned download.
    Nothing,
    /// A newer release, at another address.
    Newer,
    /// The same address, fetched AGAIN for its hash: the system held the
    /// entry because the download changed under that address
    /// (`upstream-changed`) or could not be fetched (`fetch-failed`), and
    /// no newer release is there to move to — a pin that stayed would be
    /// held on every machine for ever.
    Again,
}

/// What to do about an AppImage pinned at `pinned`, whose newest release is
/// at `latest`, held by the system for `held` (or not held). Pure.
#[must_use]
pub fn look(pinned: &str, latest: &str, held: Option<HeldWhy>) -> Look {
    if latest != pinned {
        Look::Newer
    } else if matches!(held, Some(HeldWhy::UpstreamChanged | HeldWhy::FetchFailed)) {
        Look::Again
    } else {
        Look::Nothing
    }
}

/// Whether F-Droid's offer moves a pinned apk: a newer version code, or
/// the same one REBUILT — another hash under the same version, which a pin
/// that stayed could never fetch again. Pure.
#[must_use]
pub fn apk_moves(pinned: &ApkPin, offered: &ApkPin) -> bool {
    offered.version_code > pinned.version_code
        || (offered.version_code == pinned.version_code && offered.hash != pinned.hash)
}

/// The entries the system held back, and why — `person-applied.json`'s
/// `held` skips, whose detail is the hold's reason. Pure.
#[must_use]
pub fn held_of(applied: Option<&PersonApplied>) -> BTreeMap<String, HeldWhy> {
    applied
        .map(|r| {
            r.skipped
                .iter()
                .filter(|s| s.reason == SkipWhy::Held)
                .filter_map(|s| {
                    HeldWhy::ALL
                        .into_iter()
                        .find(|h| h.id() == s.detail.trim())
                        .map(|h| (s.id.clone(), h))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Look for newer pins of every declared download, and answer the entries
/// whose pin moved — nothing is written: the downloads a new pin takes run
/// with no lane held, and [`take`] then writes them under the declaration's.
///
/// An AppImage follows its GitHub releases (`follows`) — and one the system
/// held because its download changed or vanished under the pinned address
/// is fetched again there when no newer release exists ([`look`]); an
/// Android app from F-Droid follows the catalogue, fetched again first,
/// rebuilt apks included ([`apk_moves`]). An entry with nowhere to look
/// stays as it is, and so does one whose source cannot be asked now
/// (offline, not let): the next look asks again.
///
/// # Errors
///
/// [`Refusal::DeclarationUnreadable`].
#[cfg(feature = "net")]
pub fn newer(
    ctx: &crate::Context,
    net: &dyn crate::github::Fetch,
    host: &str,
    report: &mut dyn crate::progress::Report,
) -> Result<Vec<ronoctua_person::declared::Entry>, Failure> {
    newer_with(ctx, net, host, report, &mut |a, asset, stamp, report| {
        appimage::pin(ctx, &a.key, asset, None, stamp, report)
    })
}

/// A pin of an AppImage's asset, as [`appimage::pin`] takes it: what
/// [`newer_with`] is handed (a test's, without the network).
#[cfg(feature = "net")]
pub type PinOf<'a> = dyn FnMut(
        &ronoctua_person::declared::AppImageApp,
        &crate::github::Asset,
        ronoctua_person::Stamp,
        &mut dyn crate::progress::Report,
    ) -> Result<ronoctua_person::declared::Pin, Failure>
    + 'a;

/// [`newer`], with the pin of an asset handed in.
///
/// # Errors
///
/// As [`newer`].
#[cfg(feature = "net")]
pub fn newer_with(
    ctx: &crate::Context,
    net: &dyn crate::github::Fetch,
    host: &str,
    report: &mut dyn crate::progress::Report,
    pin_of: &mut PinOf<'_>,
) -> Result<Vec<ronoctua_person::declared::Entry>, Failure> {
    use ronoctua_person::declared::Entry;

    use crate::progress::Phase;

    let now = crate::appimage::now();
    let declared = crate::declaration::read_declared(&ctx.roots).map_err(|why| Failure {
        refusal: Some(Refusal::DeclarationUnreadable),
        trouble: format!("{}: {}", ronoctua_person::names::DECLARED, why.id()),
    })?;
    let held = held_of(crate::declaration::person_applied(&ctx.roots).as_ref());
    let android_here = declared
        .apps
        .iter()
        .any(|e| matches!(e, Entry::Android(a) if a.from == ApkFrom::FDroid));
    if android_here && let Err(e) = crate::fdroid::refresh_with(net, &ctx.roots, report) {
        tracing::warn!(error = %e, "F-Droid's catalogue was not fetched again; its pins stay");
    }
    let stamp = crate::declaration::now_stamp();
    let mut moved: Vec<Entry> = Vec::new();
    for entry in &declared.apps {
        report.step(Phase::Resolving, 0, 0, entry.source_id());
        match entry {
            Entry::AppImage(a) => {
                let Some(f) = &a.follows else { continue };
                let latest = crate::github::latest_with(
                    net,
                    &f.github.repo,
                    &crate::appimage::github_cache(&ctx.roots),
                    Some(&f.github.asset_pattern),
                    now,
                );
                let asset = match latest {
                    Ok(asset) => asset,
                    Err(e) => {
                        tracing::warn!(error = %e, entry = a.id, "no newer pin asked");
                        continue;
                    }
                };
                if look(&a.pin.url, &asset.url, held.get(&a.id).copied()) == Look::Nothing {
                    continue;
                }
                let pin = match pin_of(a, &asset, stamp.clone(), report) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(error = %e, entry = a.id, "a newer pin was not taken");
                        continue;
                    }
                };
                // THE SAME BYTES at the same address: nothing moved, and the
                // entry is left as it was (a fetch that failed for a moment
                // is tried again by the next update's verification).
                if pin.url == a.pin.url && pin.hash == a.pin.hash {
                    continue;
                }
                let mut next = a.clone();
                next.pin = pin;
                next.edited_at = stamp.clone();
                host.clone_into(&mut next.edited_on);
                moved.push(Entry::AppImage(next));
            }
            Entry::Android(a) if a.from == ApkFrom::FDroid => {
                let Ok((pin, _)) = android(&ctx.roots, &a.package) else {
                    continue;
                };
                if !apk_moves(&a.pin, &pin) {
                    continue;
                }
                let mut next = a.clone();
                next.pin = pin;
                next.edited_at = stamp.clone();
                host.clone_into(&mut next.edited_on);
                moved.push(Entry::Android(next));
            }
            _ => {}
        }
    }
    Ok(moved)
}

/// Write the pins [`newer`] found into the declaration AS IT IS NOW — the
/// caller holds the declaration's lane. Only the pin and the stamps of an
/// entry move: one forgotten meanwhile stays forgotten, and one whose
/// machines another door changed meanwhile keeps them. Answers how many
/// moved.
///
/// # Errors
///
/// [`crate::declaration::change_declared`]'s.
pub fn take(roots: &Roots, moved: &[ronoctua_person::declared::Entry]) -> Result<u64, Failure> {
    if moved.is_empty() {
        return Ok(0);
    }
    let mut count = 0;
    crate::declaration::change_declared(roots, |doc| {
        // AGAIN FROM NOTHING on every try: a try made over a file that moved
        // is made again (`change_declared`).
        count = 0;
        for newer in moved {
            if let Some(entry) = doc.apps.iter_mut().find(|e| e.id() == newer.id())
                && repinned(entry, newer)
            {
                count += 1;
            }
        }
        Ok(())
    })?;
    Ok(count)
}

/// `entry` with `newer`'s pin and stamps, when the two pins differ.
fn repinned(
    entry: &mut ronoctua_person::declared::Entry,
    newer: &ronoctua_person::declared::Entry,
) -> bool {
    use ronoctua_person::declared::Entry;
    match (entry, newer) {
        (Entry::AppImage(e), Entry::AppImage(n)) if e.pin != n.pin => {
            e.pin = n.pin.clone();
            e.edited_at = n.edited_at.clone();
            e.edited_on.clone_from(&n.edited_on);
            true
        }
        (Entry::Android(e), Entry::Android(n)) if e.pin != n.pin => {
            e.pin = n.pin.clone();
            e.edited_at = n.edited_at.clone();
            e.edited_on.clone_from(&n.edited_on);
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FAILS IF: writing a newer pin moves anything but the pin and the
    /// stamps — machines another door changed meanwhile are kept — or brings
    /// back an entry forgotten meanwhile.
    #[test]
    fn a_newer_pin_moves_only_the_pin() {
        use ronoctua_person::Stamp;
        use ronoctua_person::declared::{AppImageApp, Entry, Pin};
        let root = std::env::temp_dir().join(format!(
            "ronoctua-software-pins-take-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let roots = Roots::under(&root);
        let at = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
        let later = Stamp::parse("2026-10-06T12:00:00Z").unwrap();
        let app = |key: &str, version: &str, machines: Option<Vec<String>>, when: &Stamp| {
            Entry::AppImage(AppImageApp {
                id: format!("appimage:{key}"),
                machines,
                edited_at: when.clone(),
                edited_on: "noctua".into(),
                key: key.into(),
                name: "App".into(),
                pin: Pin {
                    url: format!("https://example.org/{key}-{version}.AppImage"),
                    hash: "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=".into(),
                    version: version.into(),
                    pinned_at: when.clone(),
                },
                follows: None,
                icon: None,
            })
        };
        crate::declaration::change_declared(&roots, |d| {
            crate::declaration::upsert(d, app("kept", "1", Some(vec!["laptop".into()]), &at));
            Ok(())
        })
        .unwrap();
        // FOUND NEWER while the person, meanwhile, declared it on no machine
        // of the list it had, and a second one was forgotten.
        let moved = [
            app("kept", "2", None, &later),
            app("forgotten", "2", None, &later),
        ];
        assert_eq!(take(&roots, &moved).unwrap(), 1);
        let doc = crate::declaration::read_declared(&roots).unwrap();
        assert_eq!(doc.apps.len(), 1, "a forgotten entry came back");
        let Entry::AppImage(a) = &doc.apps[0] else {
            panic!("not an AppImage");
        };
        assert_eq!(a.pin.version, "2");
        assert_eq!(a.edited_at, later);
        assert_eq!(
            a.machines,
            Some(vec!["laptop".to_owned()]),
            "the machines moved"
        );
        // THE SAME PIN AGAIN is no change.
        assert_eq!(take(&roots, &moved).unwrap(), 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    fn pin(url: &str, hash: &str, code: u64) -> ApkPin {
        ApkPin {
            url: url.into(),
            hash: hash.into(),
            version: "1".into(),
            version_code: code,
        }
    }

    /// FAILS IF: an F-Droid apk rebuilt under the same version code (another
    /// hash) does not move its pin — the pin would never be fetched again —
    /// or an older or identical offer moves it.
    #[test]
    fn an_apk_moves_to_a_newer_code_or_a_rebuild_of_its_own() {
        let url = "https://f-droid.org/repo/org.fdroid.fdroid_1021050.apk";
        let pinned = pin(url, "sha256-A", 1_021_050);
        assert!(apk_moves(
            &pinned,
            &pin(
                "https://f-droid.org/repo/org.fdroid.fdroid_1021051.apk",
                "sha256-B",
                1_021_051
            )
        ));
        assert!(
            apk_moves(&pinned, &pin(url, "sha256-B", 1_021_050)),
            "rebuilt"
        );
        assert!(
            !apk_moves(&pinned, &pin(url, "sha256-A", 1_021_050)),
            "the same"
        );
        assert!(
            !apk_moves(&pinned, &pin(url, "sha256-B", 1_021_049)),
            "older"
        );
    }

    /// FAILS IF: an AppImage held because its download changed (or
    /// vanished) under the pinned address is not fetched again there when no
    /// newer release exists, a newer release is not taken, or one that is
    /// neither is downloaded for nothing.
    #[test]
    fn a_held_download_is_fetched_again_at_its_address() {
        let a = "https://x.example/A-1.AppImage";
        let b = "https://x.example/A-2.AppImage";
        assert_eq!(look(a, b, None), Look::Newer);
        assert_eq!(look(a, b, Some(HeldWhy::UpstreamChanged)), Look::Newer);
        assert_eq!(look(a, a, None), Look::Nothing);
        assert_eq!(look(a, a, Some(HeldWhy::UpstreamChanged)), Look::Again);
        assert_eq!(look(a, a, Some(HeldWhy::FetchFailed)), Look::Again);
        assert_eq!(look(a, a, Some(HeldWhy::BuildFailed)), Look::Nothing);
        assert_eq!(look(a, a, Some(HeldWhy::EvalFailed)), Look::Nothing);
    }

    /// The canned GitHub feed: audacity's latest release, whose x86_64
    /// AppImage is at [`AUDACITY`].
    #[cfg(feature = "net")]
    const AUDACITY: &str = "https://github.com/audacity/audacity/releases/download/Audacity-4.0.0/audacity-linux-4.0.0-x86_64.AppImage";

    #[cfg(feature = "net")]
    fn feed() -> crate::github::canned::Canned {
        crate::github::canned::Canned::new(vec![(
            "https://api.github.com/repos/audacity/audacity/releases/latest",
            200,
            vec![],
            std::fs::read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/appimage/github-audacity-latest.json"),
            )
            .unwrap(),
        )])
    }

    /// FAILS IF, over a canned GitHub feed: an entry pinned at the newest
    /// release's own address and held as `upstream-changed` is not pinned
    /// again at that address with the hash it serves now; one not held is
    /// downloaded at all; one pinned at an older address is not moved; or
    /// the same bytes again count as a move.
    #[cfg(feature = "net")]
    #[test]
    fn newer_pins_follow_the_feed_and_mend_a_held_one() {
        use ronoctua_person::Stamp;
        use ronoctua_person::declared::{AppImageApp, Entry, Pin};
        use ronoctua_person::reports::Skipped;
        let root = std::env::temp_dir().join(format!(
            "ronoctua-software-pins-newer-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let roots = Roots::under(&root);
        let at = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
        let old_hash = "sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=";
        let app = |key: &str, url: &str| {
            Entry::AppImage(AppImageApp {
                id: format!("appimage:{key}"),
                machines: None,
                edited_at: at.clone(),
                edited_on: "noctua".into(),
                key: key.into(),
                name: "Audacity".into(),
                pin: Pin {
                    url: url.into(),
                    hash: old_hash.into(),
                    version: "Audacity-4.0.0".into(),
                    pinned_at: at.clone(),
                },
                follows: Some(appimage::follows(
                    "audacity/audacity",
                    "audacity-linux-*-x86_64.AppImage",
                )),
                icon: None,
            })
        };
        crate::declaration::change_declared(&roots, |d| {
            crate::declaration::upsert(d, app("held", AUDACITY));
            crate::declaration::upsert(d, app("kept", AUDACITY));
            crate::declaration::upsert(
                d,
                app(
                    "older",
                    "https://github.com/audacity/audacity/releases/download/Audacity-3.7.0/audacity-linux-3.7.0-x86_64.AppImage",
                ),
            );
            Ok(())
        })
        .unwrap();
        let mut report = PersonApplied {
            format: 1,
            machine: "noctua".into(),
            distro_rev: "abc1234".into(),
            applied: vec!["appimage:kept".into(), "appimage:older".into()],
            skipped: vec![Skipped {
                id: "appimage:held".into(),
                reason: SkipWhy::Held,
                detail: "upstream-changed".into(),
            }],
            declined: Default::default(),
            brain: Default::default(),
            signature: Default::default(),
        };
        std::fs::create_dir_all(&roots.system.reports).unwrap();
        let write_report = |r: &PersonApplied| {
            std::fs::write(
                roots
                    .system
                    .reports
                    .join(ronoctua_person::names::PERSON_APPLIED),
                ronoctua_person::to_json(r),
            )
            .unwrap();
        };
        write_report(&report);
        let ctx = crate::Context {
            tools: crate::Tools::default(),
            roots: roots.clone(),
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        // WHAT THE ADDRESS SERVES NOW, as the download would hash it.
        let new_hash = "sha256-d2Zjv7QHvkQjLBk4QCzeQNpGZcfm6B7bW3DjsU6LlbQ=";
        let mut asked: Vec<String> = Vec::new();
        let mut pin_of = |a: &AppImageApp,
                          asset: &crate::github::Asset,
                          stamp: Stamp,
                          _: &mut dyn crate::progress::Report|
         -> Result<Pin, Failure> {
            asked.push(a.key.clone());
            Ok(Pin {
                url: asset.url.clone(),
                hash: new_hash.into(),
                version: asset.tag.clone(),
                pinned_at: stamp,
            })
        };
        let moved = newer_with(&ctx, &feed(), "noctua", &mut Vec::new(), &mut pin_of).unwrap();
        assert_eq!(
            asked,
            ["held", "older"],
            "the one neither held nor old is not fetched"
        );
        let by = |key: &str| {
            moved.iter().find_map(|e| match e {
                Entry::AppImage(a) if a.key == key => Some(a.clone()),
                _ => None,
            })
        };
        let held = by("held").expect("the held one is pinned again");
        assert_eq!(
            (held.pin.url.as_str(), held.pin.hash.as_str()),
            (AUDACITY, new_hash)
        );
        assert_eq!(by("older").expect("moved").pin.url, AUDACITY);
        assert!(by("kept").is_none());
        // THE SAME BYTES AGAIN (a fetch that failed for a moment): no move.
        report.skipped[0].detail = "fetch-failed".into();
        write_report(&report);
        let mut same = |a: &AppImageApp,
                        asset: &crate::github::Asset,
                        stamp: Stamp,
                        _: &mut dyn crate::progress::Report|
         -> Result<Pin, Failure> {
            Ok(Pin {
                url: asset.url.clone(),
                hash: if a.key == "held" {
                    old_hash.into()
                } else {
                    new_hash.into()
                },
                version: asset.tag.clone(),
                pinned_at: stamp,
            })
        };
        let moved = newer_with(&ctx, &feed(), "noctua", &mut Vec::new(), &mut same).unwrap();
        assert!(
            moved.iter().all(|e| e.id() != "appimage:held"),
            "the same bytes moved the pin"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_hex_sha256_is_its_sri() {
        // The SHA-256 of nothing, both ways people write it.
        assert_eq!(
            sri_of_hex("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855")
                .as_deref(),
            Some("sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=")
        );
        assert_eq!(sri_of_hex("abc"), None);
        assert_eq!(sri_of_hex(&"g".repeat(64)), None);
    }

    /// FAILS IF: the name an AppImage goes into the store under is not the
    /// one the builder fetches it as — the rebuild would download it again.
    #[test]
    fn an_appimage_is_stored_under_the_builders_name() {
        assert_eq!(store_name("obsidian", "v1.6.7"), "obsidian-v1.6.7.AppImage");
        assert_eq!(store_name("x", "1 2"), "x-pinned.AppImage");
        assert_eq!(store_name("x", ""), "x-pinned.AppImage");
        assert_eq!(version_of(" v1.2 "), "v1.2");
        assert_eq!(version_of("a \"b\""), "pinned");
    }

    #[test]
    fn a_pattern_follows_the_next_release() {
        assert_eq!(
            pattern_of("Obsidian-1.6.7.AppImage", "v1.6.7"),
            "Obsidian-*.AppImage"
        );
        assert_eq!(pattern_of("tool.AppImage", "v2"), "tool.AppImage");
    }

    #[test]
    fn the_programs_are_asked_as_nix_takes_them() {
        assert_eq!(
            prefetch_argv("https://x/a.AppImage", "a-1.AppImage", Some("sha256-X")),
            [
                "store",
                "prefetch-file",
                "--json",
                "--hash-type",
                "sha256",
                "--name",
                "a-1.AppImage",
                "--expected-hash",
                "sha256-X",
                "--",
                "https://x/a.AppImage"
            ]
        );
        assert_eq!(
            store_add_argv("a-1.AppImage", Path::new("/h/Applications/a.AppImage")),
            [
                "store",
                "add",
                "--mode",
                "flat",
                "--hash-algo",
                "sha256",
                "--name",
                "a-1.AppImage",
                "--",
                "/h/Applications/a.AppImage"
            ]
        );
        assert_eq!(
            hash_of_prefetch(r#"{"hash":"sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=","storePath":"/nix/store/x"}"#)
                .as_deref(),
            Some("sha256-47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU=")
        );
        assert_eq!(hash_of_prefetch(r#"{"hash":"md5-x"}"#), None);
    }
}
