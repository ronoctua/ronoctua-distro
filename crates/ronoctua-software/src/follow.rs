//! The distro's AppImages the person follows past its pins — the second half
//! of `pins refresh`.
//!
//! # What it does, and when
//!
//! With `distro-apps-follow-releases` on in the person's options (their
//! mirror's `options.json`, `ronoctua_person::options`), the update's
//! `person-pins` step asks GitHub for the newest release of every AppImage of
//! the distro's table that follows a repository, and writes it into the
//! person's `software/distro-pins.json` ([`ronoctua_person::distro_pins`]) —
//! signed with their plain part and staged to root like the rest. The
//! machine's registry builds a followed pin only while the option is on and
//! it is newer than the distro's; root's verification holds one that does
//! not build. With the option OFF the file goes ([`forget`]), and the
//! machine builds the distro's pins at once anyway.
//!
//! # Where the table comes from
//!
//! The system's own report, `/etc/ronoctua/distro-apps.json`: each AppImage
//! with its key, the repository and pattern it follows, the distro's pin and
//! why the verification held a followed one — no Nix evaluation at run time.
//! A report of a build before the followed pins knows none: then nothing is
//! asked and nothing is dropped.
//!
//! # The same GitHub, the same downloads
//!
//! The newest release is [`crate::github::latest_with`]'s — the latest, else
//! the newest of the recent ones with a matching asset; drafts and
//! pre-releases never; kept six hours and asked again with its `ETag`, so the
//! sixty questions an hour GitHub answers unauthenticated are not spent by a
//! routine update. A tag without a digit moves under the same name and is
//! never pinned. The download is fetched under THE SAME store name the
//! distro's builder fetches it under (`<key>-<version>.AppImage`,
//! `nix/distro-downloads.nix`), checked against GitHub's digest, so the build
//! finds the bytes. Every pin is judged by the one rule
//! ([`ronoctua_person::distro_pins::fault`]) before it is written.
//!
//! # Only where the machine would take it, and judged before it is fetched
//!
//! The person's options and followed pins reach root only signed: a machine
//! that trusts no key of theirs (`person-applied.json` says none is pinned)
//! never builds a followed pin, so nothing is asked or downloaded there
//! ([`Looked::no_signer`]). And a release is judged by the one rule — its
//! address, its asset, its version — with a stand-in hash BEFORE its bytes
//! are fetched: only a release that can pass is downloaded.
//!
//! # Asked only of the hosts the person let
//!
//! [`ronoctua_person::distro_pins::HOSTS`] — the API and the download's host —
//! both in the person's consent, or nothing is asked: the AppImages keep the
//! pins they had and the step says so ([`NOT_LET`]). Turning the option on
//! is the gesture that lets them (the update's page).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ronoctua_person::distro_pins::{DistroPins, FollowedPin};
use ronoctua_person::options::Options;
use ronoctua_person::tree::caps;
use ronoctua_person::{Document as _, names};

use crate::{Failure, Refusal, Roots};

/// The refusal word `pins refresh` ends with when a host following asks is
/// not let — the update's step reads it as news (`declines`), and says its
/// sentence, the update's own.
pub const NOT_LET: &str = "github-not-let";

/// The person's followed pins, in their mirror.
#[must_use]
pub fn path(roots: &Roots) -> PathBuf {
    crate::declaration::mirror(roots).join(names::DISTRO_PINS)
}

/// The person's options, in their mirror.
#[must_use]
pub fn options_path(roots: &Roots) -> PathBuf {
    crate::declaration::mirror(roots).join(names::OPTIONS)
}

/// Whether the person's options say to follow: `Some(false)` without the
/// file, `None` when it does not read here (a newer machine's, a broken one)
/// — then the followed pins are left as they are, neither moved nor dropped.
#[must_use]
pub fn following(roots: &Roots) -> Option<bool> {
    match std::fs::read(options_path(roots)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Some(false),
        Err(_) => None,
        Ok(bytes) => ronoctua_person::parse::<Options>(&bytes)
            .ok()
            .map(|o| o.distro_apps_follow_releases),
    }
}

/// The bytes of the followed pins, when the file is there.
fn bytes_at(path: &Path) -> Result<Option<Vec<u8>>, Failure> {
    use std::io::Read as _;
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Failure::trouble(format!("{}: {e}", path.display()))),
    };
    let mut bytes = Vec::new();
    file.take(caps::DISTRO_PINS_MOST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| Failure::trouble(format!("{}: {e}", path.display())))?;
    if bytes.len() as u64 > caps::DISTRO_PINS_MOST_BYTES {
        return Err(unreadable(ronoctua_person::Why::Invalid));
    }
    Ok(Some(bytes))
}

/// The refusal of a file of followed pins that does not read here.
fn unreadable(why: ronoctua_person::Why) -> Failure {
    Failure {
        refusal: Some(Refusal::DeclarationUnreadable),
        trouble: format!("{}: {}", names::DISTRO_PINS, why.id()),
    }
}

/// The person's followed pins as they are now — none when there is no file.
///
/// # Errors
///
/// [`Refusal::DeclarationUnreadable`] for a file that does not read here.
pub fn read(roots: &Roots) -> Result<DistroPins, Failure> {
    match bytes_at(&path(roots))? {
        None => Ok(DistroPins::default()),
        Some(bytes) => ronoctua_person::parse(&bytes).map_err(|r| unreadable(r.why)),
    }
}

/// What one look decided of one followed AppImage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// This pin, newer than the one written.
    Set(FollowedPin),
    /// No followed pin: the distro's caught up, or the table no longer
    /// follows it.
    Drop,
}

/// What one look found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Looked {
    /// By table id.
    pub changes: BTreeMap<String, Change>,
    /// A host following asks was not let: nothing was asked.
    pub not_let: bool,
    /// This machine trusts no signing key of the person's: it would never
    /// take a followed pin, and nothing was asked.
    pub no_signer: bool,
}

/// The hash a release is judged with before its bytes are fetched: an SRI
/// SHA-256 of the right shape, so only its address, asset and version are
/// asked.
#[cfg(feature = "net")]
const STAND_IN_HASH: &str = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

/// How many times a change is made again over a file that moved between its
/// reading and its writing (the belongings land a merge without our lane).
const CHANGE_TRIES: usize = 5;

/// Writes `changes` into the followed pins AS THEY ARE NOW — the caller holds
/// the declaration's lane — the list kept in the order of its ids, so two
/// machines that followed the same releases write the same bytes. Answers
/// how many moved.
///
/// # Errors
///
/// A file that does not read here (never written over), a result that would
/// not validate, a failed write.
pub fn apply(roots: &Roots, changes: &BTreeMap<String, Change>) -> Result<u64, Failure> {
    if changes.is_empty() {
        return Ok(0);
    }
    let file = path(roots);
    for _ in 0..CHANGE_TRIES {
        let before = bytes_at(&file)?;
        let mut doc: DistroPins = match &before {
            None => DistroPins::default(),
            Some(bytes) => ronoctua_person::parse(bytes).map_err(|r| unreadable(r.why))?,
        };
        let mut moved = 0;
        for (id, change) in changes {
            let at = doc.pins.iter().position(|p| p.id == *id);
            match (change, at) {
                (Change::Set(pin), Some(i)) if doc.pins[i] != *pin => {
                    doc.pins[i] = pin.clone();
                    moved += 1;
                }
                (Change::Set(pin), None) => {
                    doc.pins.push(pin.clone());
                    moved += 1;
                }
                (Change::Drop, Some(i)) => {
                    doc.pins.remove(i);
                    moved += 1;
                }
                _ => {}
            }
        }
        if moved == 0 {
            return Ok(0);
        }
        doc.pins.sort_by(|a, b| a.id.cmp(&b.id));
        doc.validate().map_err(|r| Failure {
            refusal: Some(Refusal::DeclarationRefused),
            trouble: r.to_string(),
        })?;
        // STILL WHAT WAS READ, or the change is made again over what is there.
        if bytes_at(&file).ok() != Some(before) {
            continue;
        }
        let folder = file
            .parent()
            .ok_or_else(|| Failure::trouble(format!("{} is not a file", file.display())))?;
        std::fs::create_dir_all(folder)
            .and_then(|()| {
                ronoctua_xdg::write_atomically(
                    folder,
                    &file.file_name().unwrap_or_default().to_string_lossy(),
                    ronoctua_person::to_json(&doc).as_bytes(),
                )
            })
            .map_err(|e| Failure::trouble(format!("could not write {}: {e}", file.display())))?;
        return Ok(moved);
    }
    Err(Failure::trouble(format!(
        "{} changed under every one of {CHANGE_TRIES} changes",
        names::DISTRO_PINS
    )))
}

/// The followed pins gone — the person does not follow any more. Whether
/// there was a file.
///
/// # Errors
///
/// A file that could not be removed.
pub fn forget(roots: &Roots) -> Result<bool, Failure> {
    let file = path(roots);
    match std::fs::remove_file(&file) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Failure::trouble(format!(
            "could not remove {}: {e}",
            file.display()
        ))),
    }
}

/// The ids the registry follows a repository for — `None` for a report that
/// knows no AppImage's pins (a build before them): nothing is then asked,
/// and nothing dropped. Pure.
#[must_use]
pub fn followable(registry: &ronoctua_person::reports::DistroApps) -> Option<BTreeSet<String>> {
    let images = registry.apps.iter().filter(|a| a.appimage.is_some());
    let mut ids = BTreeSet::new();
    let mut any = false;
    for app in images {
        any = true;
        if app.appimage.as_ref().is_some_and(|i| i.follows.is_some()) {
            ids.insert(app.id.clone());
        }
    }
    any.then_some(ids)
}

/// A pin of an AppImage's asset under the builder's name, as
/// [`crate::pins::appimage::pin_as`] takes it: the key, the version the
/// download is named by, the asset — what [`look_with`] is handed (a
/// test's, without the network or Nix).
#[cfg(feature = "net")]
pub type PinOf<'a> = dyn FnMut(
        &str,
        &str,
        &crate::github::Asset,
        ronoctua_person::Stamp,
        &mut dyn crate::progress::Report,
    ) -> Result<ronoctua_person::declared::Pin, Failure>
    + 'a;

/// Looks for the newest release of every AppImage the registry follows a
/// repository for, on this machine — nothing is written: the downloads run
/// with no lane held, and [`apply`] writes what changed.
#[cfg(feature = "net")]
pub fn look(
    ctx: &crate::Context,
    net: &dyn crate::github::Fetch,
    report: &mut dyn crate::progress::Report,
) -> Looked {
    // NO KEY PINNED, NO FOLLOWING: root keeps the person's signed-only
    // documents out of such a machine, and a download it never builds is
    // bandwidth for nothing.
    let pinned = crate::declaration::person_applied(&ctx.roots)
        .is_some_and(|r| r.signature.pinned.is_some());
    if !pinned {
        tracing::info!("this machine trusts no signing key of the person's: nothing is followed");
        return Looked {
            no_signer: true,
            ..Looked::default()
        };
    }
    let Some(registry) = crate::declaration::distro_apps(&ctx.roots) else {
        tracing::warn!("no registry report of this machine: no distro AppImage is followed");
        return Looked::default();
    };
    let doc = match read(&ctx.roots) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(error = %e, "the followed pins do not read here; they are left as they are");
            return Looked::default();
        }
    };
    look_with(
        &LookAt {
            registry: &registry,
            doc: &doc,
            cache: &crate::appimage::github_cache(&ctx.roots),
            allows: &ronoctua_consent::allows,
            now: crate::appimage::now(),
            stamp: crate::declaration::now_stamp(),
        },
        net,
        report,
        &mut |key, version, asset, stamp, report| {
            crate::pins::appimage::pin_as(ctx, key, version, asset, None, stamp, report)
        },
    )
}

/// What a look reads: the registry's report, the followed pins as they are,
/// where GitHub's answers are kept, which hosts the person let, and the
/// clock.
#[cfg(feature = "net")]
pub struct LookAt<'a> {
    /// `/etc/ronoctua/distro-apps.json`.
    pub registry: &'a ronoctua_person::reports::DistroApps,
    /// The followed pins now.
    pub doc: &'a DistroPins,
    /// GitHub's kept answers (`crate::appimage::github_cache`).
    pub cache: &'a Path,
    /// Whether the person let a host.
    pub allows: &'a dyn Fn(&str) -> bool,
    /// Seconds since the epoch, for the kept answers.
    pub now: u64,
    /// The stamp a new pin carries.
    pub stamp: ronoctua_person::Stamp,
}

/// [`look`], over what it reads and with the pin of an asset handed in.
#[cfg(feature = "net")]
pub fn look_with(
    at: &LookAt<'_>,
    net: &dyn crate::github::Fetch,
    report: &mut dyn crate::progress::Report,
    pin_of: &mut PinOf<'_>,
) -> Looked {
    use ronoctua_person::distro_pins::{self, HOSTS};

    use crate::pins::Look;
    use crate::progress::Phase;

    let mut looked = Looked::default();
    let Some(followable) = followable(at.registry) else {
        return looked;
    };
    let allowed = HOSTS.iter().all(|h| (at.allows)(h));
    for app in &at.registry.apps {
        let Some(image) = &app.appimage else { continue };
        let Some(follows) = &image.follows else {
            continue;
        };
        // ON THIS MACHINE ONLY: another machine of the person's may carry
        // what this one does not, and its pin stays as that one wrote it.
        if !app.on_this_machine || app.declined_here {
            continue;
        }
        if !allowed {
            looked.not_let = true;
            continue;
        }
        report.step(Phase::Resolving, 0, 0, &app.id);
        let latest = match crate::github::latest_with(
            net,
            &follows.repo,
            at.cache,
            Some(&follows.asset_pattern),
            at.now,
        ) {
            Ok(a) => a,
            Err(e) => {
                tracing::warn!(error = %e, app = app.id, "the newest release was not asked");
                continue;
            }
        };
        // A TAG WITH NO NUMBER MOVES under the same name: never pinned.
        if !latest.tag.bytes().any(|b| b.is_ascii_digit()) {
            tracing::warn!(
                app = app.id,
                tag = latest.tag,
                "the newest release's tag moves"
            );
            continue;
        }
        let current = at.doc.pin(&app.id);
        // THE DISTRO CAUGHT UP: its own pin is this release.
        if latest.url == image.distro_url {
            if current.is_some() {
                looked.changes.insert(app.id.clone(), Change::Drop);
            }
            continue;
        }
        if let Some(c) = current
            && crate::pins::look(&c.url, &latest.url, image.held) == Look::Nothing
        {
            continue;
        }
        let version = distro_pins::version_of_tag(&latest.tag);
        // JUDGED BEFORE IT IS FETCHED: a release this machine would refuse
        // costs no download.
        let judged = FollowedPin {
            id: app.id.clone(),
            url: latest.url.clone(),
            hash: STAND_IN_HASH.to_owned(),
            version: version.to_owned(),
            pinned_at: at.stamp.clone(),
        };
        if let Some(fault) = distro_pins::fault(&judged, Some(follows)) {
            tracing::warn!(
                app = app.id,
                fault = fault.id(),
                url = judged.url,
                "the newest release is not one this machine builds"
            );
            continue;
        }
        let pin = match pin_of(&image.key, version, &latest, at.stamp.clone(), report) {
            Ok(p) => FollowedPin {
                id: app.id.clone(),
                url: p.url,
                hash: p.hash,
                version: p.version,
                pinned_at: p.pinned_at,
            },
            Err(e) => {
                tracing::warn!(error = %e, app = app.id, "the newest release was not pinned");
                continue;
            }
        };
        if let Some(fault) = distro_pins::fault(&pin, Some(follows)) {
            tracing::warn!(
                app = app.id,
                fault = fault.id(),
                url = pin.url,
                "the newest release is not one this machine builds"
            );
            continue;
        }
        // THE SAME BYTES at the same address: nothing moved.
        if current.is_some_and(|c| c.url == pin.url && c.hash == pin.hash) {
            continue;
        }
        looked.changes.insert(app.id.clone(), Change::Set(pin));
    }
    // A PIN THE TABLE NO LONGER FOLLOWS is nobody's to build.
    for p in &at.doc.pins {
        if !followable.contains(&p.id) {
            looked.changes.insert(p.id.clone(), Change::Drop);
        }
    }
    looked
}

#[cfg(all(test, feature = "net"))]
mod tests {
    use super::*;
    use ronoctua_person::Stamp;
    use ronoctua_person::declared::{GithubReleases, Pin};
    use ronoctua_person::local::HeldWhy;
    use ronoctua_person::reports::{DistroApp, DistroAppImage, DistroApps, PinFrom};

    const RELEASES: &str = "https://github.com/storytold";

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-follow-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn image_url(app: &str, version: &str) -> String {
        format!(
            "{RELEASES}/{app}/releases/download/v{version}/{app}-{version}-linux-x86_64.AppImage"
        )
    }

    /// One AppImage of the registry, following `storytold/<id>`, the
    /// distro's pin at `distro`.
    fn app(id: &str, distro: &str, follows: bool) -> DistroApp {
        DistroApp {
            id: id.to_owned(),
            name: None,
            source: ronoctua_person::declared::Source::AppImage,
            desktop_ids: vec![format!("ronoctua-appimage-{id}.desktop")],
            essential: None,
            group: "crafts".to_owned(),
            on_this_machine: true,
            declined_here: false,
            appimage: Some(DistroAppImage {
                key: id.to_owned(),
                follows: follows.then(|| GithubReleases {
                    repo: format!("storytold/{id}"),
                    asset_pattern: format!("{id}-*-linux-x86_64.AppImage"),
                }),
                pin: PinFrom::Distro,
                version: distro.to_owned(),
                distro_version: distro.to_owned(),
                distro_url: image_url(id, distro),
                followed_version: None,
                held: None,
                waiting: false,
            }),
        }
    }

    /// GitHub's `releases/latest` of `storytold/<app>`, at `tag`, with its
    /// x86_64 image, the aarch64 one and the zsync beside them.
    fn latest(app: &str, tag: &str) -> (String, Vec<u8>) {
        let version = tag.trim_start_matches('v');
        let asset = |name: String| {
            serde_json::json!({
                "name": name,
                "browser_download_url": format!("{RELEASES}/{app}/releases/download/{tag}/{name}"),
                "size": 7,
                "digest": format!("sha256:{}", "a".repeat(64)),
            })
        };
        let body = serde_json::json!({
            "tag_name": tag,
            "draft": false,
            "prerelease": false,
            "assets": [
                asset(format!("{app}-{version}-linux-x86_64.AppImage")),
                asset(format!("{app}-{version}-linux-x86_64.AppImage.zsync")),
                asset(format!("{app}-{version}-linux-aarch64.AppImage")),
            ],
        });
        (
            format!("https://api.github.com/repos/storytold/{app}/releases/latest"),
            serde_json::to_vec(&body).unwrap(),
        )
    }

    /// GitHub, answering each of `answers` once.
    fn github(answers: &[(String, Vec<u8>)]) -> crate::github::canned::Canned {
        crate::github::canned::Canned::new(
            answers
                .iter()
                .map(|(url, body)| (url.as_str(), 200, vec![], body.clone()))
                .collect(),
        )
    }

    fn followed(id: &str, version: &str) -> FollowedPin {
        FollowedPin {
            id: id.to_owned(),
            url: image_url(id, version),
            hash: format!("sha256-{}=", "B".repeat(43)),
            version: version.to_owned(),
            pinned_at: Stamp::parse("2026-10-19T12:00:00Z").unwrap(),
        }
    }

    /// FAILS IF, over canned GitHub answers and no Nix: a newer release is
    /// not pinned under the version the distro's builder names it by, an
    /// AppImage the distro caught up keeps a followed pin, one already
    /// followed to the newest release is downloaded again, one held for a
    /// download that changed is not fetched again, a moving tag or another
    /// machine's AppImage is pinned, a pin of an id the table no longer
    /// follows stays, or GitHub is asked without the person's consent.
    #[test]
    fn the_distro_appimages_follow_their_newest_release() {
        let dir = scratch("look");
        let mut held = app("heldcraft", "0.8.0", true);
        if let Some(i) = held.appimage.as_mut() {
            i.held = Some(HeldWhy::UpstreamChanged);
        }
        let mut elsewhere = app("othercraft", "0.8.0", true);
        elsewhere.on_this_machine = false;
        let registry = DistroApps {
            format: 1,
            apps: vec![
                app("newcraft", "0.8.0", true),
                app("caughtcraft", "0.9.0", true),
                app("samecraft", "0.8.0", true),
                held,
                app("movingcraft", "0.8.0", true),
                // A TAG THE JUDGE REFUSES (a `~` in the address): never downloaded.
                app("tildecraft", "0.8.0", true),
                elsewhere,
                app("byhand", "1.0", false),
            ],
            followed: ronoctua_person::reports::FollowedReport::default(),
        };
        let doc = DistroPins {
            format: 1,
            pins: vec![
                followed("caughtcraft", "0.9.0"),
                followed("samecraft", "0.9.0"),
                followed("heldcraft", "0.9.0"),
                followed("othercraft", "0.9.0"),
                followed("gonecraft", "0.9.0"),
            ],
        };
        let net = github(&[
            latest("newcraft", "v0.9.0"),
            latest("caughtcraft", "v0.9.0"),
            latest("samecraft", "v0.9.0"),
            latest("heldcraft", "v0.9.0"),
            latest("movingcraft", "nightly"),
            latest("tildecraft", "2.0~rc"),
        ]);
        let stamp = Stamp::parse("2026-10-20T12:00:00Z").unwrap();
        let asked: std::cell::RefCell<Vec<(String, String)>> = std::cell::RefCell::default();
        let mut pin_of = |key: &str,
                          version: &str,
                          asset: &crate::github::Asset,
                          stamp: Stamp,
                          _: &mut dyn crate::progress::Report|
         -> Result<Pin, Failure> {
            asked
                .borrow_mut()
                .push((key.to_owned(), version.to_owned()));
            Ok(Pin {
                url: asset.url.clone(),
                hash: format!("sha256-{}=", "C".repeat(43)),
                version: version.to_owned(),
                pinned_at: stamp,
            })
        };
        let all = |_: &str| true;
        let at = LookAt {
            registry: &registry,
            doc: &doc,
            cache: &dir,
            allows: &all,
            now: 1_000,
            stamp: stamp.clone(),
        };
        let looked = look_with(&at, &net, &mut Vec::new(), &mut pin_of);
        assert!(!looked.not_let);
        assert_eq!(
            *asked.borrow(),
            [
                ("newcraft".to_owned(), "0.9.0".to_owned()),
                ("heldcraft".to_owned(), "0.9.0".to_owned())
            ],
            "only the new and the held one are downloaded, under the builder's version"
        );
        let set = |id: &str| match looked.changes.get(id) {
            Some(Change::Set(p)) => Some(p.clone()),
            _ => None,
        };
        let new = set("newcraft").expect("the newer release is pinned");
        assert_eq!(new.url, image_url("newcraft", "0.9.0"));
        assert_eq!((new.version.as_str(), &new.pinned_at), ("0.9.0", &stamp));
        assert!(set("heldcraft").is_some(), "the held one is fetched again");
        assert_eq!(looked.changes.get("caughtcraft"), Some(&Change::Drop));
        assert_eq!(looked.changes.get("gonecraft"), Some(&Change::Drop));
        for kept in [
            "samecraft",
            "movingcraft",
            "tildecraft",
            "othercraft",
            "byhand",
        ] {
            assert!(!looked.changes.contains_key(kept), "{kept}: {looked:?}");
        }
        // WRITTEN, in the order of the ids, and judged as the machine will.
        let root = scratch("apply");
        let roots = Roots::under(&root);
        std::fs::create_dir_all(path(&roots).parent().unwrap()).unwrap();
        std::fs::write(path(&roots), ronoctua_person::to_json(&doc)).unwrap();
        assert_eq!(apply(&roots, &looked.changes).unwrap(), 4);
        let after = read(&roots).unwrap();
        let ids: Vec<&str> = after.pins.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["heldcraft", "newcraft", "othercraft", "samecraft"]);
        assert_eq!(apply(&roots, &looked.changes).unwrap(), 0, "the same again");
        // NOT LET: GitHub is not asked at all, and nothing moves.
        let none = crate::github::canned::Canned::new(Vec::new());
        let only_the_api = |h: &str| h == "api.github.com";
        let not_let = scratch("not-let");
        let at = LookAt {
            allows: &only_the_api,
            cache: &not_let,
            ..at
        };
        let refused = look_with(&at, &none, &mut Vec::new(), &mut pin_of);
        assert!(refused.not_let);
        assert!(none.asked.borrow().is_empty());
        assert!(
            refused.changes.values().all(|c| *c == Change::Drop),
            "only what the table stopped following goes: {refused:?}"
        );
        // OFF: the file goes.
        assert!(forget(&roots).unwrap());
        assert!(!forget(&roots).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&not_let);
    }

    /// FAILS IF: a machine that trusts no signing key of the person's —
    /// where root would never take a followed pin — asks GitHub anything.
    #[test]
    fn a_machine_with_no_signer_follows_nothing() {
        let dir = scratch("no-signer");
        let roots = Roots::under(&dir);
        std::fs::create_dir_all(&roots.system.reports).unwrap();
        let report = ronoctua_person::reports::PersonApplied {
            format: 1,
            machine: "noctua".into(),
            distro_rev: "abc1234".into(),
            applied: Vec::new(),
            skipped: Vec::new(),
            declined: Default::default(),
            brain: Default::default(),
            signature: Default::default(),
        };
        std::fs::write(
            roots
                .system
                .reports
                .join(ronoctua_person::names::PERSON_APPLIED),
            ronoctua_person::to_json(&report),
        )
        .unwrap();
        let ctx = crate::Context {
            tools: crate::Tools::default(),
            roots,
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        let net = crate::github::canned::Canned::new(Vec::new());
        let looked = look(&ctx, &net, &mut Vec::new());
        assert!(looked.no_signer && looked.changes.is_empty());
        assert!(net.asked.borrow().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// FAILS IF: a report of a build before the followed pins — no AppImage
    /// says its pins — makes a look ask anything or drop a pin another
    /// machine wrote; or options that do not read are taken as "off".
    #[test]
    fn an_older_report_and_unreadable_options_change_nothing() {
        let mut old = app("cadcraft", "0.8.0", true);
        old.appimage = None;
        let registry = DistroApps {
            format: 1,
            apps: vec![old],
            followed: ronoctua_person::reports::FollowedReport::default(),
        };
        assert_eq!(followable(&registry), None);
        let doc = DistroPins {
            format: 1,
            pins: vec![followed("cadcraft", "0.9.0")],
        };
        let net = crate::github::canned::Canned::new(Vec::new());
        let all = |_: &str| true;
        let dir = scratch("old");
        let at = LookAt {
            registry: &registry,
            doc: &doc,
            cache: &dir,
            allows: &all,
            now: 1,
            stamp: Stamp::parse("2026-10-20T12:00:00Z").unwrap(),
        };
        let looked = look_with(&at, &net, &mut Vec::new(), &mut |_, _, _, _, _| {
            panic!("nothing is pinned")
        });
        assert_eq!(looked, Looked::default());
        assert!(net.asked.borrow().is_empty());
        let roots = Roots::under(&dir);
        assert_eq!(following(&roots), Some(false), "no options, no following");
        std::fs::create_dir_all(options_path(&roots).parent().unwrap()).unwrap();
        std::fs::write(options_path(&roots), "{\"format\": 9}").unwrap();
        assert_eq!(following(&roots), None);
        std::fs::write(
            options_path(&roots),
            "{\"format\": 1, \"ssh-signing\": null, \"distro-apps-follow-releases\": true}",
        )
        .unwrap();
        assert_eq!(following(&roots), Some(true));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
