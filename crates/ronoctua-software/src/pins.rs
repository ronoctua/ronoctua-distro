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

use std::path::Path;

use ronoctua_person::declared::{ApkFrom, ApkPin};

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
    Ok((of_app(&app)?, ApkFrom::FDroid))
}

/// The pin of the version a verified index offers of an app: its address in
/// the repository, the SHA-256 the signed index gives as SRI, its version
/// name and code. The one shape of an F-Droid pin, the person's
/// ([`android`]) and the distro's ([`signed`]).
///
/// # Errors
///
/// [`Refusal::Integrity`] for a hash that is not one.
pub fn of_app(app: &crate::fdroid::App) -> Result<ApkPin, Failure> {
    let hash = sri_of_hex(&app.sha256.to_ascii_lowercase())
        .ok_or_else(|| Failure::refused(Refusal::Integrity))?;
    let version = if app.version_name.is_empty() {
        app.version_code.to_string()
    } else {
        app.version_name.clone()
    };
    Ok(ApkPin {
        url: format!(
            "{}/{}",
            crate::fdroid::REPO,
            app.apk.trim_start_matches('/')
        ),
        hash,
        version: version_of(&version),
        version_code: app.version_code,
    })
}

/// The distro's apk pins held against F-Droid's SIGNED index — what
/// `ronoctua-fdroid-pins` answers `nix run .#distro-pins`
/// (`nix/scripts/distro-pins.py`).
///
/// # Why a program, and why it reads files
///
/// The distro's pins were taken from F-Droid's JSON API, which nobody signs,
/// and hashed from whatever `f-droid.org` served that day. The index is
/// signed by a key F-Droid keeps off its web servers, and the chain from
/// that signature to an apk's SHA-256 is already checked here
/// ([`crate::fdroid::verify_jar`], `aws-lc-rs`, the one provider of the
/// primitives) — so the release tool asks this program, and no second
/// implementation of RSA is written in Python. The tool downloads
/// `entry.jar` and the index it names; this reads them as files, so it never
/// touches the network and is proved offline with planted ones.
///
/// # What it answers
///
/// Per package asked, the version the signed index offers — the
/// catalogue's own rule ([`crate::fdroid::compact_index`]), as a pin
/// ([`of_app`]) — and how the pin the distro holds stands against it
/// ([`signed::standing`]). A jar that does not verify against the pinned
/// certificate, or an index that is not the one its signed entry names, is
/// refused whole: nothing is answered from it.
pub mod signed {
    use std::collections::BTreeMap;
    use std::path::Path;

    use ronoctua_person::declared::ApkPin;
    use serde::{Deserialize, Serialize};

    use crate::{Failure, Refusal};

    /// How a pinned apk stands against the version the signed index offers.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case")]
    pub enum Standing {
        /// Nothing is pinned yet: the signed version is the pin.
        New,
        /// The pin IS the signed version: address, hash and version code.
        Same,
        /// The pin is another version code: the signed one replaces it.
        Moves,
        /// The pin claims the version code the index signs, with another
        /// address or another hash: the index does not vouch for it.
        Disagrees,
        /// The signed index has no such package.
        Unlisted,
    }

    /// How `pinned` stands against `signed`, the version the verified index
    /// offers. Pure.
    #[must_use]
    pub fn standing(pinned: Option<&ApkPin>, signed: Option<&ApkPin>) -> Standing {
        match (pinned, signed) {
            (_, None) => Standing::Unlisted,
            (None, Some(_)) => Standing::New,
            (Some(p), Some(s)) if p.version_code != s.version_code => Standing::Moves,
            (Some(p), Some(s)) if p.url == s.url && p.hash == s.hash => Standing::Same,
            (Some(_), Some(_)) => Standing::Disagrees,
        }
    }

    /// One package's answer.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case", deny_unknown_fields)]
    pub struct Answered {
        /// How the pin asked about stands.
        pub standing: Standing,
        /// The version the signed index offers, as a pin; `None` when it
        /// lists no such package.
        pub signed: Option<ApkPin>,
    }

    /// What the program prints for `pins`.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "kebab-case", deny_unknown_fields)]
    pub struct Answer {
        /// When the signed entry was published (milliseconds since the
        /// epoch).
        pub timestamp: u64,
        /// The SHA-256 of the index the answers come from, as the signed
        /// entry names it.
        pub index_sha256: String,
        /// By package.
        pub apps: BTreeMap<String, Answered>,
    }

    /// Hold each pin of `pinned` (by package; `None`: nothing pinned yet)
    /// against the index at `index`, once `jar` verified against the
    /// certificate pinned as `pin` and `index` is the one it names.
    ///
    /// # Errors
    ///
    /// [`Refusal::Integrity`] for a jar or an index that does not verify,
    /// `TooLarge` above the ceilings, `ToolFailed` for something that is not
    /// an index.
    pub fn answer(
        jar: &[u8],
        pin: &str,
        index: &Path,
        pinned: &BTreeMap<String, Option<ApkPin>>,
    ) -> Result<Answer, Failure> {
        let (entry, apps) = crate::fdroid::signed_apps(jar, pin, index)?;
        let mut answered = BTreeMap::new();
        for (package, held) in pinned {
            let offered = apps
                .binary_search_by(|a| a.package.as_str().cmp(package))
                .ok()
                .map(|at| super::of_app(&apps[at]))
                .transpose()?;
            answered.insert(
                package.clone(),
                Answered {
                    standing: standing(held.as_ref(), offered.as_ref()),
                    signed: offered,
                },
            );
        }
        Ok(Answer {
            timestamp: entry.timestamp,
            index_sha256: entry.index.sha256.to_ascii_lowercase(),
            apps: answered,
        })
    }

    /// What `ronoctua-fdroid-pins` prints when its command line is not one.
    pub const USAGE: &str = "\
usage: ronoctua-fdroid-pins entry <entry.jar>
       ronoctua-fdroid-pins pins <entry.jar> <index-v2.json> <pinned.json>

Checks F-Droid's signed index against the certificate pinned in the distro,
reading files and never the network.

  entry  verify entry.jar and print the entry it vouches for (the index's
         name, SHA-256 and size), as one JSON object.
  pins   verify entry.jar, check that index-v2.json is the index it names,
         and hold each pin of pinned.json ({\"<package>\": <pin> or null})
         against the version the index offers: one JSON object, each
         package's standing (new, same, moves, disagrees, unlisted) and the
         signed pin.

Exit status: 0 answered, 1 refused (a sentence on standard error), 2 a wrong
command line.
";

    /// The program, as a function of its arguments and the certificate it
    /// trusts: the line it prints, or the exit status and the sentence.
    ///
    /// # Errors
    ///
    /// `(2, usage)` for a wrong command line; `(1, sentence)` for a file that
    /// does not read or a jar, an index or a pins file that is refused.
    pub fn run(args: &[String], pin: &str) -> Result<String, (u8, String)> {
        let read = |path: &str, most: u64| -> Result<Vec<u8>, (u8, String)> {
            let size = std::fs::metadata(path)
                .map_err(|e| (1, format!("{path}: {e}")))?
                .len();
            if size > most {
                return Err((
                    1,
                    format!("{path} is {size} bytes, above the {most} it may be"),
                ));
            }
            std::fs::read(path).map_err(|e| (1, format!("{path}: {e}")))
        };
        // THE JAR FIRST, on its own: a refusal says which file broke the
        // chain — the signature, or the index it names.
        let verified = |jar: &str| -> Result<crate::fdroid::Entry, (u8, String)> {
            let bytes = read(jar, crate::fdroid::ENTRY_MOST)?;
            crate::fdroid::verify_jar_with(&bytes, pin).map_err(|r| (1, jar_refused(jar, r)))
        };
        match args {
            [verb, jar] if verb == "entry" => {
                let entry = verified(jar)?;
                let line = serde_json::json!({
                    "timestamp": entry.timestamp,
                    "index": {
                        "name": entry.index.name,
                        "sha256": entry.index.sha256.to_ascii_lowercase(),
                        "size": entry.index.size,
                    },
                });
                Ok(line.to_string())
            }
            [verb, jar, index, pinned] if verb == "pins" => {
                verified(jar)?;
                let asked: BTreeMap<String, Option<ApkPin>> =
                    serde_json::from_slice(&read(pinned, crate::fdroid::ENTRY_MOST)?).map_err(
                        |e| {
                            (
                                1,
                                format!("{pinned} is not {{\"<package>\": <pin> or null}}: {e}"),
                            )
                        },
                    )?;
                let bytes = read(jar, crate::fdroid::ENTRY_MOST)?;
                let answer = answer(&bytes, pin, Path::new(index), &asked)
                    .map_err(|f| (1, index_refused(index, &f)))?;
                serde_json::to_string(&answer).map_err(|e| (1, e.to_string()))
            }
            _ => Err((2, USAGE.to_owned())),
        }
    }

    /// The sentence a refused `entry.jar` is said in.
    fn jar_refused(jar: &str, refusal: Refusal) -> String {
        match refusal {
            Refusal::TooLarge => format!("{jar} names an index larger than F-Droid's may be"),
            _ => format!(
                "{jar} does not verify against F-Droid's signing certificate pinned in the distro: \
                 nothing is answered from it"
            ),
        }
    }

    /// The sentence a refused index is said in (its jar verified).
    fn index_refused(index: &str, f: &Failure) -> String {
        match f.refusal {
            Some(Refusal::Integrity) => format!(
                "{index} is not the index F-Droid's signed entry names ({}): nothing is answered from it",
                f.trouble
            ),
            Some(Refusal::ToolFailed) => format!("{index} is not an F-Droid index"),
            _ => format!("{index}: {f}"),
        }
    }
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
            crate::net::Asker::Person,
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
    /// distro's builder fetches it under (`nix/distro-downloads.nix`) — and
    /// downloaded on `asker`'s pass: the person's, or the declared updater's
    /// that follows the distro's AppImages (`crate::follow`).
    ///
    /// # Errors
    ///
    /// As [`pin`].
    #[allow(clippy::too_many_arguments)]
    pub fn pin_as(
        ctx: &Context,
        asker: crate::net::Asker,
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
                // to a host the asker does not let (`net::Asker::pass`, the
                // same pass every download of this crate takes).
                let _pass = asker.pass(&asset.url)?;
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
    /// The same address, fetched AGAIN for its hash: the entry's part could
    /// not fetch its download there — changed under that address, or gone
    /// — and no newer release is there to move to: a pin that stayed would
    /// fail on every machine for ever.
    Again,
}

/// What to do about an AppImage pinned at `pinned`, whose newest release is
/// at `latest`, when its part could not fetch the pinned download
/// (`missed`, [`crate::parts::misses`]). Pure.
#[must_use]
pub fn look(pinned: &str, latest: &str, missed: bool) -> Look {
    if latest != pinned {
        Look::Newer
    } else if missed {
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
    let applied = crate::declaration::person_applied(&ctx.roots);
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
                let missed = crate::parts::of_entry(applied.as_ref(), &a.id)
                    .is_some_and(|part| crate::parts::misses(&ctx.roots, part, &a.pin.url));
                if look(&a.pin.url, &asset.url, missed) == Look::Nothing {
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
        assert_eq!(look(a, b, false), Look::Newer);
        assert_eq!(look(a, b, true), Look::Newer);
        assert_eq!(look(a, a, false), Look::Nothing);
        assert_eq!(look(a, a, true), Look::Again);
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
        use ronoctua_person::reports::PersonApplied;
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
        let report = PersonApplied {
            format: 1,
            machine: "noctua".into(),
            distro_rev: "abc1234".into(),
            applied: vec![
                "appimage:held".into(),
                "appimage:kept".into(),
                "appimage:older".into(),
            ],
            parts: [
                ("appimage:held", "person-0000000a-held"),
                ("appimage:kept", "person-0000000b-kept"),
            ]
            .into_iter()
            .map(|(e, p)| (e.to_owned(), p.to_owned()))
            .collect(),
            skipped: Vec::new(),
            declined: Default::default(),
            brain: Default::default(),
            signature: Default::default(),
        };
        // THE PART OF `held` COULD NOT FETCH its pinned download: the
        // address serves other bytes now.
        let stuck = ronoctua_parts::record::Record::new("person-0000000a-held")
            .planned_entry(&"a".repeat(64))
            .failed(
                ronoctua_parts::record::Why::BuildFailed,
                "hash mismatch",
                at.clone(),
                true,
            )
            .missing(vec![ronoctua_person::frontier::Download {
                drv: "/nix/store/0123456789abcdfghijklmnpqrsvwxyz-held.AppImage.drv".into(),
                out: "/nix/store/zyxwvsrqpnmlkjihgfdcba9876543210-held.AppImage".into(),
                name: "held-4.0.0.AppImage".into(),
                hash: old_hash.into(),
                mode: ronoctua_person::frontier::Mode::Flat,
                urls: vec![AUDACITY.into()],
            }]);
        std::fs::create_dir_all(&roots.system.parts).unwrap();
        std::fs::write(
            roots.system.parts.join("person-0000000a-held.json"),
            ronoctua_person::to_json(&stuck),
        )
        .unwrap();
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
            "the one neither stuck nor old is not fetched"
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

    /// F-DROID'S SIGNED INDEX, PLANTED: an `entry.jar` signed the way F-Droid
    /// signs its own (a PKCS#7 block over the `.SF`, no signed attributes,
    /// RSA and SHA-256) by a key made here, its `entry.json` naming the cut
    /// index of the fixtures by its SHA-256 and size. The certificate is
    /// only what the verifier reads of one — the hash of its bytes is the pin,
    /// and the RSA key is taken from it; nobody asks its own signature.
    mod signed_index {
        use std::collections::BTreeMap;
        use std::path::PathBuf;
        use std::sync::OnceLock;

        use aws_lc_rs::rsa::{KeyPair as RsaKey, KeySize};
        use aws_lc_rs::signature::KeyPair as _;
        use ronoctua_person::declared::ApkPin;

        use super::super::signed::{Answer, Standing, answer, run, standing};
        use super::super::sri_of_hex;
        use crate::Refusal;

        const RSA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01];
        const SHA256: &[u8] = &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01];
        const SIGNED_DATA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x02];
        const DATA: &[u8] = &[0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x07, 0x01];
        const COMMON_NAME: &[u8] = &[0x55, 0x04, 0x03];

        /// What the cut index offers of F-Droid's own client: the stable
        /// 1.23.2, never the 2.0 betas above it.
        const FDROID_SHA256: &str =
            "985f5181d48bb6bafd54083a048b391271e0ab28385881cc41294fb01a222762";

        fn der(tag: u8, content: &[u8]) -> Vec<u8> {
            let n = content.len();
            let mut out = vec![tag];
            if n < 0x80 {
                out.push(u8::try_from(n).unwrap());
            } else {
                let bytes = u32::try_from(n).unwrap().to_be_bytes();
                let skip = bytes.iter().take_while(|b| **b == 0).count();
                out.push(0x80 | u8::try_from(4 - skip).unwrap());
                out.extend(&bytes[skip..]);
            }
            out.extend(content);
            out
        }

        fn seq(parts: &[Vec<u8>]) -> Vec<u8> {
            der(0x30, &parts.concat())
        }

        fn algorithm(oid: &[u8]) -> Vec<u8> {
            seq(&[der(0x06, oid), der(0x05, &[])])
        }

        /// Two keys, made once: F-Droid's stand-in and another's.
        fn keys() -> &'static (RsaKey, RsaKey) {
            static KEYS: OnceLock<(RsaKey, RsaKey)> = OnceLock::new();
            KEYS.get_or_init(|| {
                (
                    RsaKey::generate(KeySize::Rsa2048).unwrap(),
                    RsaKey::generate(KeySize::Rsa2048).unwrap(),
                )
            })
        }

        fn certificate(key: &RsaKey, name: &str) -> Vec<u8> {
            let mut bits = vec![0u8];
            bits.extend(key.public_key().as_ref());
            let who = seq(&[der(
                0x31,
                &seq(&[der(0x06, COMMON_NAME), der(0x0C, name.as_bytes())]),
            )]);
            let tbs = seq(&[
                der(0xA0, &der(0x02, &[2])),
                der(0x02, &[7]),
                algorithm(RSA),
                who.clone(),
                seq(&[der(0x17, b"260101000000Z"), der(0x17, b"360101000000Z")]),
                who,
                seq(&[algorithm(RSA), der(0x03, &bits)]),
            ]);
            seq(&[tbs, algorithm(RSA), der(0x03, &[0])])
        }

        /// The hex SHA-256 of a certificate: what is pinned.
        fn pin_of(certificate: &[u8]) -> String {
            crate::run::sha256(certificate)
        }

        /// A `.RSA` block: `certificate` carried, `sf` signed by `key`.
        fn block(key: &RsaKey, certificate: &[u8], sf: &[u8]) -> Vec<u8> {
            let mut signature = vec![0; key.public_modulus_len()];
            key.sign(
                &aws_lc_rs::signature::RSA_PKCS1_SHA256,
                &aws_lc_rs::rand::SystemRandom::new(),
                sf,
                &mut signature,
            )
            .unwrap();
            let signer = seq(&[
                der(0x02, &[1]),
                seq(&[seq(&[]), der(0x02, &[7])]),
                algorithm(SHA256),
                algorithm(RSA),
                der(0x04, &signature),
            ]);
            let data = seq(&[
                der(0x02, &[1]),
                der(0x31, &algorithm(SHA256)),
                seq(&[der(0x06, DATA)]),
                der(0xA0, certificate),
                der(0x31, &signer),
            ]);
            seq(&[der(0x06, SIGNED_DATA), der(0xA0, &data)])
        }

        fn digest(bytes: &[u8]) -> String {
            crate::fdroid::base64(
                aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, bytes).as_ref(),
            )
        }

        /// An `entry.jar` vouching for `entry`, signed by `key` and carrying
        /// `certificate`.
        fn jar(key: &RsaKey, certificate: &[u8], entry: &[u8]) -> Vec<u8> {
            let manifest = format!(
                "Manifest-Version: 1.0\r\nCreated-By: a test\r\n\r\nName: entry.json\r\nSHA-256-Digest: {}\r\n\r\n",
                digest(entry)
            );
            let sf = format!(
                "Signature-Version: 1.0\r\nSHA-256-Digest-Manifest: {}\r\nCreated-By: a test\r\n\r\n",
                digest(manifest.as_bytes())
            );
            let block = block(key, certificate, sf.as_bytes());
            crate::android::tests::zip_of(&[
                ("META-INF/MANIFEST.MF", manifest.as_bytes()),
                ("META-INF/TEST.SF", sf.as_bytes()),
                ("META-INF/TEST.RSA", &block),
                ("entry.json", entry),
            ])
        }

        /// The cut index of the fixtures.
        fn cut() -> PathBuf {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/android/index-v2-cut.json")
        }

        /// The `entry.json` that names `index`.
        fn entry_naming(index: &std::path::Path) -> Vec<u8> {
            let bytes = std::fs::read(index).unwrap();
            serde_json::json!({
                "timestamp": 1_761_436_800_000u64,
                "version": 20002,
                "index": {
                    "name": "/index-v2.json",
                    "sha256": crate::run::sha256(&bytes),
                    "size": bytes.len(),
                    "numPackages": 5,
                },
            })
            .to_string()
            .into_bytes()
        }

        /// A room of the test's own.
        fn room(tag: &str) -> PathBuf {
            let dir = std::env::temp_dir().join(format!(
                "ronoctua-software-signed-{}-{tag}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            dir
        }

        /// The pin the signed cut offers of F-Droid's client.
        fn fdroid_signed() -> ApkPin {
            ApkPin {
                url: "https://f-droid.org/repo/org.fdroid.fdroid_1023052.apk".into(),
                hash: sri_of_hex(FDROID_SHA256).unwrap(),
                version: "1.23.2".into(),
                version_code: 1_023_052,
            }
        }

        fn asked(pins: &[(&str, Option<ApkPin>)]) -> BTreeMap<String, Option<ApkPin>> {
            pins.iter()
                .map(|(p, pin)| ((*p).to_owned(), pin.clone()))
                .collect()
        }

        fn standings(a: &Answer) -> BTreeMap<&str, Standing> {
            a.apps
                .iter()
                .map(|(p, x)| (p.as_str(), x.standing))
                .collect()
        }

        /// FAILS IF: a pin is not what the signed index offers — the address in
        /// the repository, the SHA-256 the index signs, the version the
        /// catalogue's rule picks (the stable one, never a beta) — or a pin
        /// the distro holds is not told against it: the same, another version
        /// (it moves), none yet, a package the index does not list.
        #[test]
        fn a_pin_is_what_the_signed_index_offers() {
            let (fdroid, _) = keys();
            let cert = certificate(fdroid, "F-Droid");
            let signed = jar(fdroid, &cert, &entry_naming(&cut()));
            let older = ApkPin {
                url: "https://f-droid.org/repo/chat.simplex.app_1.apk".into(),
                hash: sri_of_hex(&"a".repeat(64)).unwrap(),
                version: "0.1".into(),
                version_code: 1,
            };
            let a = answer(
                &signed,
                &pin_of(&cert),
                &cut(),
                &asked(&[
                    ("org.fdroid.fdroid", Some(fdroid_signed())),
                    ("com.aurora.store", None),
                    ("chat.simplex.app", Some(older)),
                    ("no.such.app", None),
                ]),
            )
            .unwrap();
            assert_eq!(
                a.apps["org.fdroid.fdroid"].signed,
                Some(fdroid_signed()),
                "the stable version, by the index's own hash"
            );
            assert_eq!(
                standings(&a),
                [
                    ("chat.simplex.app", Standing::Moves),
                    ("com.aurora.store", Standing::New),
                    ("no.such.app", Standing::Unlisted),
                    ("org.fdroid.fdroid", Standing::Same),
                ]
                .into_iter()
                .collect()
            );
            assert_eq!(a.apps["no.such.app"].signed, None);
            assert_eq!(a.timestamp, 1_761_436_800_000);
            assert_eq!(
                a.index_sha256,
                crate::run::sha256(&std::fs::read(cut()).unwrap())
            );
        }

        /// FAILS IF: a pin that claims the version code the index signs, with
        /// another hash or another address, is not told as disagreeing — or
        /// one that is the signed version is.
        #[test]
        fn a_pin_that_claims_the_signed_version_with_other_bytes_disagrees() {
            let (fdroid, _) = keys();
            let cert = certificate(fdroid, "F-Droid");
            let signed = jar(fdroid, &cert, &entry_naming(&cut()));
            let other_hash = ApkPin {
                hash: sri_of_hex(&"b".repeat(64)).unwrap(),
                ..fdroid_signed()
            };
            let other_url = ApkPin {
                url: "https://mirror.example/org.fdroid.fdroid_1023052.apk".into(),
                ..fdroid_signed()
            };
            for (what, pinned) in [("another hash", other_hash), ("another address", other_url)] {
                let a = answer(
                    &signed,
                    &pin_of(&cert),
                    &cut(),
                    &asked(&[("org.fdroid.fdroid", Some(pinned.clone()))]),
                )
                .unwrap();
                assert_eq!(
                    a.apps["org.fdroid.fdroid"].standing,
                    Standing::Disagrees,
                    "{what}"
                );
                assert_eq!(a.apps["org.fdroid.fdroid"].signed, Some(fdroid_signed()));
                assert_eq!(
                    standing(Some(&pinned), Some(&fdroid_signed())),
                    Standing::Disagrees,
                    "{what}"
                );
            }
            assert_eq!(
                standing(Some(&fdroid_signed()), Some(&fdroid_signed())),
                Standing::Same
            );
            let renamed = ApkPin {
                version: "renamed".into(),
                ..fdroid_signed()
            };
            assert_eq!(
                standing(Some(&renamed), Some(&fdroid_signed())),
                Standing::Same,
                "a version NAME is no part of the bytes"
            );
            assert_eq!(standing(Some(&fdroid_signed()), None), Standing::Unlisted);
        }

        /// FAILS IF: an index that is not the one the signed entry names —
        /// one byte changed — is answered from, or an `entry.json` changed
        /// after it was signed is believed.
        #[test]
        fn a_tampered_index_or_entry_is_refused() {
            let dir = room("tampered");
            let (fdroid, _) = keys();
            let cert = certificate(fdroid, "F-Droid");
            let signed = jar(fdroid, &cert, &entry_naming(&cut()));
            let mut bytes = std::fs::read(cut()).unwrap();
            let at = bytes.iter().position(|b| *b == b'9').unwrap();
            bytes[at] = b'8';
            let tampered = dir.join("index-v2.json");
            std::fs::write(&tampered, &bytes).unwrap();
            let f = answer(
                &signed,
                &pin_of(&cert),
                &tampered,
                &asked(&[("org.fdroid.fdroid", None)]),
            )
            .unwrap_err();
            assert_eq!(f.refusal, Some(Refusal::Integrity), "{f}");
            // AN ENTRY CHANGED AFTER THE SIGNATURE: the signed parts of the
            // honest jar, with an `entry.json` that names the tampered index.
            let honest = ronoctua_zip::members(&signed).unwrap();
            let kept = |name: &str| {
                ronoctua_zip::member_bytes(&signed, ronoctua_zip::part(&honest, name).unwrap())
                    .unwrap()
            };
            let (manifest, sf, rsa) = (
                kept("META-INF/MANIFEST.MF"),
                kept("META-INF/TEST.SF"),
                kept("META-INF/TEST.RSA"),
            );
            let lying = entry_naming(&tampered);
            let forged = crate::android::tests::zip_of(&[
                ("META-INF/MANIFEST.MF", &manifest),
                ("META-INF/TEST.SF", &sf),
                ("META-INF/TEST.RSA", &rsa),
                ("entry.json", &lying),
            ]);
            let f = answer(
                &forged,
                &pin_of(&cert),
                &tampered,
                &asked(&[("org.fdroid.fdroid", None)]),
            )
            .unwrap_err();
            assert_eq!(f.refusal, Some(Refusal::Integrity), "{f}");
            let _ = std::fs::remove_dir_all(&dir);
        }

        /// FAILS IF: a jar signed by another key is believed — carrying its
        /// own certificate (not the pinned one), or carrying the pinned
        /// certificate with a signature that key did not make.
        #[test]
        fn a_jar_signed_by_another_certificate_is_refused() {
            let (fdroid, other) = keys();
            let pinned = certificate(fdroid, "F-Droid");
            let theirs = certificate(other, "Someone");
            let entry = entry_naming(&cut());
            let asked = asked(&[("org.fdroid.fdroid", None)]);
            for (what, forged) in [
                ("their own certificate", jar(other, &theirs, &entry)),
                ("the pinned certificate", jar(other, &pinned, &entry)),
            ] {
                let f = answer(&forged, &pin_of(&pinned), &cut(), &asked).unwrap_err();
                assert_eq!(f.refusal, Some(Refusal::Integrity), "{what}: {f}");
            }
            assert!(
                answer(
                    &jar(fdroid, &pinned, &entry),
                    &pin_of(&pinned),
                    &cut(),
                    &asked
                )
                .is_ok(),
                "the planted jar itself verifies"
            );
            assert!(
                answer(
                    &jar(fdroid, &pinned, &entry),
                    &pin_of(&theirs),
                    &cut(),
                    &asked
                )
                .is_err(),
                "against another pin, the same jar does not"
            );
        }

        /// FAILS IF: the program does not answer what the library does, its
        /// refusals do not say which file broke the chain, or a wrong command
        /// line is not told as one.
        #[test]
        fn the_program_answers_from_files_and_says_its_refusals() {
            let dir = room("program");
            let (fdroid, other) = keys();
            let cert = certificate(fdroid, "F-Droid");
            let jar_at = dir.join("entry.jar");
            std::fs::write(&jar_at, jar(fdroid, &cert, &entry_naming(&cut()))).unwrap();
            let forged_at = dir.join("forged.jar");
            std::fs::write(
                &forged_at,
                jar(other, &certificate(other, "Someone"), &entry_naming(&cut())),
            )
            .unwrap();
            let pinned_at = dir.join("pinned.json");
            std::fs::write(&pinned_at, r#"{"org.fdroid.fdroid": null}"#).unwrap();
            let text = |p: &PathBuf| p.display().to_string();
            let pin = pin_of(&cert);

            let entry: serde_json::Value =
                serde_json::from_str(&run(&["entry".into(), text(&jar_at)], &pin).unwrap())
                    .unwrap();
            assert_eq!(entry["index"]["name"], "/index-v2.json");
            assert_eq!(
                entry["index"]["size"],
                std::fs::metadata(cut()).unwrap().len()
            );

            let line = run(
                &["pins".into(), text(&jar_at), text(&cut()), text(&pinned_at)],
                &pin,
            )
            .unwrap();
            let a: Answer = serde_json::from_str(&line).unwrap();
            assert_eq!(a.apps["org.fdroid.fdroid"].standing, Standing::New);
            assert_eq!(a.apps["org.fdroid.fdroid"].signed, Some(fdroid_signed()));
            assert!(
                line.contains(r#""version-code":1023052"#) && line.contains(r#""standing":"new""#),
                "the pin's own fields, as the distro's file writes them: {line}"
            );

            let (code, said) = run(&["entry".into(), text(&forged_at)], &pin).unwrap_err();
            assert_eq!(code, 1);
            assert!(
                said.contains("does not verify against F-Droid's signing certificate"),
                "{said}"
            );
            let mut bytes = std::fs::read(cut()).unwrap();
            bytes.push(b'\n');
            let longer = dir.join("index-v2.json");
            std::fs::write(&longer, bytes).unwrap();
            let (code, said) = run(
                &[
                    "pins".into(),
                    text(&jar_at),
                    text(&longer),
                    text(&pinned_at),
                ],
                &pin,
            )
            .unwrap_err();
            assert_eq!(code, 1);
            assert!(
                said.contains("is not the index F-Droid's signed entry names"),
                "{said}"
            );

            assert_eq!(run(&["what".into()], &pin).unwrap_err().0, 2);
            assert_eq!(run(&[], &pin).unwrap_err().0, 2);
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
