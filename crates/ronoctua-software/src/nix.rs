//! The user's Nix profile: `nix profile add|remove|upgrade|list|wipe-history|rollback`.
//!
//! Every argument vector is a pure function below (`add_argv`, …) so a test
//! reads it without running anything; the functions that run take a
//! [`Context`] and never look up a program by name.
//!
//! # The profile is read, never remembered
//!
//! What is installed is what `nix profile list --json` says (manifest
//! version 3: `elements` is an OBJECT keyed by the element's name — the
//! manual's page on `manifest.json` still shows the old array, and the source,
//! `src/nix/profile.cc`, is what was read). The generations are the
//! `profile-N-link` symlinks beside the profile, each a store path with its own
//! `manifest.json` — `nix profile history` has no JSON form, and reading the
//! files is what it does itself.
//!
//! # Which Nix
//!
//! The pinned Nix is 2.34.8 (`nixVersions.stable` in the system's nixpkgs),
//! and `nix profile --help` there lists `add` (the 2.35 manual keeps `install`
//! only as an alias). Measured on that binary, not assumed.
//!
//! # Unfree, once per package
//!
//! nixpkgs refuses an unfree package in pure evaluation, and the one door it
//! leaves is `NIXPKGS_ALLOW_UNFREE=1` read by `builtins.getEnv` — which pure
//! evaluation hides, hence `--impure` beside it. The flag goes ONLY on the
//! calls about packages the person said yes to (the order's consent, or the
//! remembered list in `software.json`), so the yes for one program is never
//! a yes for all of them. An upgrade repeats it for those elements and only
//! for those: evaluating an unfree package without it fails, whatever was
//! installed before.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::found::Picture;
use crate::nix_index::Row;
use crate::nix_log::{Log, Pace};
use crate::order::{Consent, Scope};
use crate::progress::{Phase, Report};
use crate::run::{Call, Ran};
use crate::state::{Generation, Installed, Origin};
use crate::{AppKey, Context, Done, Failure, Order, Outcome, Refusal, Roots, Source};

/// The flake every order installs from: the system registry points it at the
/// system's own nixpkgs, locked (`nix/modules/base.nix`), so what a person
/// installs is built from the revision the index was made of.
pub const FLAKE: &str = "nixpkgs";
/// The variable nixpkgs' `check-meta.nix` reads to allow an unfree package.
pub const UNFREE_VARIABLE: &str = "NIXPKGS_ALLOW_UNFREE";
/// How old a generation must be before `Tidy` deletes it. The system's own
/// collection uses the same thirty days (`nix.gc.options`).
pub const KEEP_HISTORY: &str = "30d";

/// Whether an attribute may be handed to Nix: `^[A-Za-z0-9_+.-]+$`, with no
/// empty segment and no leading `-`. The installable is always behind `--`
/// and `nixpkgs#`, so this is not what stops a flag — it is what stops a
/// flake reference, a path or a `^` output selector from riding in on a name.
#[must_use]
pub fn attr_is_valid(attr: &str) -> bool {
    !attr.is_empty()
        && !attr.starts_with('-')
        && attr.split('.').all(|seg| !seg.is_empty())
        && attr
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'+' | b'.' | b'-'))
}

/// `nixpkgs#<attr>`.
#[must_use]
pub fn installable(attr: &str) -> String {
    format!("{FLAKE}#{attr}")
}

/// `nix profile add` — structured progress, `--impure` only for an unfree
/// package the person said yes to.
#[must_use]
pub fn add_argv(attr: &str, unfree: bool) -> Vec<String> {
    let mut argv = words(&["profile", "add", "--log-format", "internal-json"]);
    if unfree {
        argv.push("--impure".into());
    }
    argv.push("--".into());
    argv.push(installable(attr));
    argv
}

/// `nix profile remove` of one element, by name — or by store path when the
/// name is all digits, which Nix 2.34 reads as a (removed) index.
#[must_use]
pub fn remove_argv(element: &str) -> Vec<String> {
    let mut argv = words(&["profile", "remove", "--log-format", "internal-json", "--"]);
    argv.push(element.to_owned());
    argv
}

/// `nix profile upgrade` of the named elements.
#[must_use]
pub fn upgrade_argv(elements: &[String], unfree: bool) -> Vec<String> {
    let mut argv = words(&["profile", "upgrade", "--log-format", "internal-json"]);
    if unfree {
        argv.push("--impure".into());
    }
    argv.push("--".into());
    argv.extend(elements.iter().cloned());
    argv
}

/// `nix profile list --json`.
#[must_use]
pub fn list_argv() -> Vec<String> {
    words(&["profile", "list", "--json"])
}

/// `nix profile wipe-history --older-than 30d`.
#[must_use]
pub fn tidy_argv() -> Vec<String> {
    words(&["profile", "wipe-history", "--older-than", KEEP_HISTORY])
}

/// `nix profile rollback --to <n>`.
#[must_use]
pub fn rollback_argv(generation: u64) -> Vec<String> {
    let mut argv = words(&["profile", "rollback", "--to"]);
    argv.push(generation.to_string());
    argv
}

fn words(w: &[&str]) -> Vec<String> {
    w.iter().map(|s| (*s).to_owned()).collect()
}

/// One element of a profile manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Element {
    /// Its name in the manifest — what `remove` and `upgrade` take.
    pub name: String,
    /// `legacyPackages.x86_64-linux.hello`, when it came from a flake.
    pub attr_path: Option<String>,
    /// The flake reference as typed (`flake:nixpkgs`).
    pub original_url: Option<String>,
    /// The store paths it put in the profile (one per output).
    pub store_paths: Vec<String>,
    /// Whether it is linked into the profile.
    pub active: bool,
}

impl Element {
    /// The nixpkgs attribute it was installed as: the attribute path without
    /// `legacyPackages.<system>.` or `packages.<system>.` (`python3Packages.
    /// requests` keeps its dot). `None` for a store path installed by hand.
    #[must_use]
    pub fn attr(&self) -> Option<&str> {
        let path = self.attr_path.as_deref()?;
        let mut parts = path.splitn(3, '.');
        match (parts.next(), parts.next(), parts.next()) {
            (Some("legacyPackages" | "packages"), Some(_system), Some(rest)) => Some(rest),
            _ => Some(path),
        }
    }

    /// Whether it came from nixpkgs (the registry's `nixpkgs`, or any
    /// reference that names it), so its attribute means what the index says.
    #[must_use]
    pub fn from_nixpkgs(&self) -> bool {
        self.original_url.as_deref().is_some_and(|u| {
            u == "flake:nixpkgs"
                || u.starts_with("flake:nixpkgs/")
                || u.starts_with("flake:nixpkgs?")
                || u.to_ascii_lowercase().contains("nixos/nixpkgs")
        })
    }

    /// The key it answers to: its nixpkgs attribute, or its name.
    #[must_use]
    pub fn key(&self) -> AppKey {
        AppKey::Nix {
            attr: self.attr().unwrap_or(&self.name).to_owned(),
        }
    }

    /// The store name of its main output (`hello-2.12.3`): the shortest of
    /// its paths' names, since the other outputs append `-man`, `-bin`, ….
    #[must_use]
    pub fn store_name(&self) -> String {
        self.store_paths
            .iter()
            .map(|p| crate::nix_log::store_name(p))
            .min_by_key(|n| (n.len(), n.clone()))
            .unwrap_or_default()
    }
}

/// A profile manifest, versions 1 to 3.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// The elements, by name.
    pub elements: BTreeMap<String, Element>,
}

#[derive(Debug, Deserialize)]
struct RawManifest {
    #[serde(default)]
    version: u64,
    #[serde(default)]
    elements: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawElement {
    #[serde(default)]
    store_paths: Vec<String>,
    #[serde(default = "yes")]
    active: bool,
    #[serde(default)]
    attr_path: Option<String>,
    #[serde(default)]
    original_url: Option<String>,
    /// Version 1's spelling.
    #[serde(default)]
    original_uri: Option<String>,
}

const fn yes() -> bool {
    true
}

impl Manifest {
    /// A manifest's text, or `None` when it is not one. Version 3 keys the
    /// elements by name; versions 1 and 2 are an array, and the name is then
    /// derived the way Nix derives it (`getNameFromURL`): the last segment of
    /// the attribute path unless it is `default`, else the store name.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let raw: RawManifest = serde_json::from_str(text).ok()?;
        if !(1..=3).contains(&raw.version) {
            return None;
        }
        let mut elements = BTreeMap::new();
        let mut add = |name: String, e: RawElement| {
            let mut unique = name.clone();
            let mut i = 1;
            while elements.contains_key(&unique) {
                unique = format!("{name}-{i}");
                i += 1;
            }
            let element = Element {
                name: unique.clone(),
                attr_path: e.attr_path.filter(|a| !a.is_empty()),
                original_url: e.original_url.or(e.original_uri).filter(|u| !u.is_empty()),
                store_paths: e.store_paths,
                active: e.active,
            };
            elements.insert(unique, element);
        };
        match raw.elements {
            serde_json::Value::Object(map) => {
                for (name, value) in map {
                    add(name, serde_json::from_value(value).ok()?);
                }
            }
            serde_json::Value::Array(list) => {
                for value in list {
                    let e: RawElement = serde_json::from_value(value).ok()?;
                    let name = e
                        .attr_path
                        .as_deref()
                        .and_then(|a| a.rsplit('.').next())
                        .filter(|last| !last.is_empty() && *last != "default")
                        .map(str::to_owned)
                        .or_else(|| e.store_paths.first().map(|p| crate::nix_log::store_name(p)))
                        .unwrap_or_default();
                    add(name, e);
                }
            }
            serde_json::Value::Null => {}
            _ => return None,
        }
        Some(Self { elements })
    }

    /// The element installed as `attr` — by its attribute, or by its name
    /// when nothing was installed under that attribute.
    #[must_use]
    pub fn find(&self, attr: &str) -> Option<&Element> {
        self.elements
            .values()
            .find(|e| e.attr() == Some(attr))
            .or_else(|| self.elements.get(attr))
    }
}

/// The desktop-file ids under a store path's `share/applications`
/// (`org.gimp.GIMP.desktop`; a subdirectory joins with `-`, as the
/// freedesktop spec says).
#[must_use]
pub fn desktop_ids_in(prefix: &Path) -> Vec<String> {
    desktop_files_in(prefix)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

/// The same, with where each one is, sorted by id.
fn desktop_files_in(prefix: &Path) -> Vec<(String, PathBuf)> {
    let mut out = Vec::new();
    collect_desktop_files(&prefix.join("share/applications"), "", 0, &mut out);
    out.sort();
    out
}

fn collect_desktop_files(dir: &Path, prefix: &str, depth: usize, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        let path = entry.path();
        if path.is_dir() {
            if depth < 2 {
                collect_desktop_files(&path, &format!("{prefix}{name}-"), depth + 1, out);
            }
        } else if name.ends_with(".desktop") {
            out.push((format!("{prefix}{name}"), path));
        }
    }
}

/// The `Icon=` of the first entry that has one, for the installed row's
/// picture: a theme name, or a file when the entry gives a path.
fn icon_in(files: &[(String, PathBuf)]) -> Option<Picture> {
    files.iter().find_map(|(_, path)| {
        let text = std::fs::read_to_string(path).ok()?;
        let fields = ronoctua_apps::desktop::fields(&text);
        let icon = ronoctua_apps::desktop::raw_bytes(&fields, "Icon")?.trim();
        if icon.is_empty() {
            None
        } else if icon.starts_with('/') {
            Some(Picture::File {
                path: PathBuf::from(icon),
            })
        } else {
            Some(Picture::Named {
                name: icon.to_owned(),
            })
        }
    })
}

/// The version of an element, as its store path names it: the store name
/// after `<pname>-` when it starts with it (`hello-2.12.3` → `2.12.3`),
/// otherwise the whole store name. Never compared or parsed further (inv. 5).
#[must_use]
pub fn version_of(store_name: &str, pname: &str) -> String {
    store_name
        .strip_prefix(pname)
        .and_then(|rest| rest.strip_prefix('-'))
        .filter(|v| !v.is_empty())
        .unwrap_or(store_name)
        .to_owned()
}

/// What a manifest holds, as installed rows. `rows` are the index's rows of
/// the elements from nixpkgs, by attribute.
#[must_use]
pub fn installed_of(manifest: &Manifest, rows: &BTreeMap<String, Row>) -> Vec<Installed> {
    manifest
        .elements
        .values()
        .map(|e| {
            let row = e
                .attr()
                .filter(|_| e.from_nixpkgs())
                .and_then(|a| rows.get(a));
            let pname = row.map_or(e.name.as_str(), |r| r.pname.as_str());
            let store_name = e.store_name();
            let version = version_of(&store_name, pname);
            // An update is only claimed when the version is a version (the
            // prefix came off) and the index — the revision the next upgrade
            // moves to — names another one.
            let update = row
                .filter(|r| !r.version.is_empty() && r.version != version && version != store_name)
                .map(|r| r.version.clone());
            let mut desktop_ids = Vec::new();
            let mut icon = None;
            for p in &e.store_paths {
                let files = desktop_files_in(Path::new(p));
                if icon.is_none() {
                    icon = icon_in(&files);
                }
                desktop_ids.extend(files.into_iter().map(|(id, _)| id));
            }
            Installed {
                key: e.key(),
                name: e.name.clone(),
                version,
                desktop_ids,
                size_bytes: None,
                update,
                unfree: row.is_some_and(|r| r.unfree),
                origin: Origin::Mine,
                icon,
                trouble: None,
            }
        })
        .collect()
}

/// The profile's manifest, as `nix profile list --json` gives it. A READ: it
/// runs in a rehearsal too.
///
/// # Errors
///
/// No Nix, or Nix failed, or its answer is not a manifest.
pub fn manifest(ctx: &Context) -> Result<Manifest, Failure> {
    let nix = crate::run::tool(ctx.tools.nix.as_ref())?;
    let ran = Call::new(nix, list_argv())
        .keep_stdout()
        .quietly(ctx)?
        .ok()?;
    Manifest::parse(ran.stdout.trim()).ok_or_else(|| Failure {
        refusal: Some(Refusal::ToolFailed),
        trouble: "nix profile list --json: not a manifest".into(),
    })
}

/// What the profile holds (manifest v3, read through `nix profile list --json`).
/// Runs a program: call it on a thread.
#[must_use]
pub fn installed(ctx: &Context) -> Vec<Installed> {
    let manifest = match manifest(ctx) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(error = %e, "the Nix profile could not be listed");
            return Vec::new();
        }
    };
    let rows = ctx
        .tools
        .nix_index
        .as_deref()
        .map_or_else(BTreeMap::new, |index| {
            let attrs: Vec<&str> = manifest
                .elements
                .values()
                .filter(|e| e.from_nixpkgs())
                .filter_map(Element::attr)
                .collect();
            crate::nix_index::rows(index, &attrs)
        });
    installed_of(&manifest, &rows)
}

/// The profile link and the directory its generations are in.
///
/// Nix's own answer (`getDefaultProfile`): `~/.nix-profile` when it is a link
/// (its target is the profile), else `$XDG_STATE_HOME/nix/profile` (the
/// `use-xdg-base-directories` spelling), else
/// `$XDG_STATE_HOME/nix/profiles/profile`.
#[must_use]
pub fn profile_link(roots: &Roots) -> PathBuf {
    let absolute = |from: &Path, target: PathBuf| {
        if target.is_absolute() {
            target
        } else {
            from.parent().unwrap_or(from).join(target)
        }
    };
    for link in [
        roots.home.join(".nix-profile"),
        roots.state.join("nix/profile"),
    ] {
        if let Ok(target) = std::fs::read_link(&link) {
            return absolute(&link, target);
        }
    }
    roots.state.join("nix/profiles/profile")
}

/// The generation a `<name>-<N>-link` file name is, for a profile `name`.
#[must_use]
pub fn generation_number(file: &str, profile: &str) -> Option<u64> {
    file.strip_prefix(profile)?
        .strip_prefix('-')?
        .strip_suffix("-link")?
        .parse()
        .ok()
}

/// What changed between two manifests, as data: `+gimp -vlc ~hello` — added,
/// removed, and moved to another store path (an upgrade).
#[must_use]
pub fn summary(before: Option<&Manifest>, after: &Manifest) -> String {
    let empty = Manifest::default();
    let before = before.unwrap_or(&empty);
    let mut out = Vec::new();
    for (name, e) in &after.elements {
        match before.elements.get(name) {
            None => out.push(format!("+{name}")),
            Some(old) if old.store_paths != e.store_paths => out.push(format!("~{name}")),
            Some(_) => {}
        }
    }
    for name in before.elements.keys() {
        if !after.elements.contains_key(name) {
            out.push(format!("-{name}"));
        }
    }
    out.join(" ")
}

/// The generations of the profile behind `link`, newest first. Reads files,
/// runs nothing: `nix profile history` has no JSON, and it reads these same
/// links and manifests.
#[must_use]
pub fn generations_at(link: &Path) -> Vec<Generation> {
    let (Some(dir), Some(profile)) = (link.parent(), link.file_name().and_then(|n| n.to_str()))
    else {
        return Vec::new();
    };
    let current = std::fs::read_link(link).ok().and_then(|t| {
        t.file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| generation_number(n, profile))
    });
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<(u64, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            Some((generation_number(&name, profile)?, e.path()))
        })
        .collect();
    found.sort_by_key(|(n, _)| *n);
    let mut previous: Option<Manifest> = None;
    let mut out = Vec::with_capacity(found.len());
    for (number, path) in found {
        let when = std::fs::symlink_metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let manifest = std::fs::read_to_string(path.join("manifest.json"))
            .ok()
            .and_then(|t| Manifest::parse(&t));
        let summary = manifest
            .as_ref()
            .map_or_else(String::new, |m| summary(previous.as_ref(), m));
        out.push(Generation {
            number,
            when,
            current: current == Some(number),
            summary,
        });
        previous = manifest;
    }
    out.reverse();
    out
}

/// The profile's generations, newest first.
#[must_use]
pub fn generations(ctx: &Context) -> Vec<Generation> {
    generations_at(&profile_link(&ctx.roots))
}

/// Run one order whose lane is Nix: `Install`, `Remove`, `Upgrade` (one or
/// the source), `Tidy`, `Rollback`. Any other order is `Refusal::UnknownItem`.
/// Runs programs: call it on a thread (the CLI does).
pub fn run(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    match order {
        Order::Install {
            key: AppKey::Nix { attr },
            consent,
        } => install(ctx, attr, *consent, report),
        Order::Remove {
            key: AppKey::Nix { attr },
            ..
        } => remove(ctx, attr, report),
        Order::Upgrade {
            scope: Scope::One {
                key: AppKey::Nix { attr },
            },
        } => upgrade(ctx, Some(attr), report),
        Order::Upgrade {
            scope:
                Scope::Source {
                    source: Source::Nix,
                }
                | Scope::Everything,
        } => upgrade(ctx, None, report),
        Order::Tidy {
            source: Source::Nix,
        } => tidy(ctx, report),
        Order::Rollback { generation } => rollback(ctx, *generation, report),
        Order::Install {
            key: AppKey::System { .. },
            ..
        }
        | Order::Remove {
            key: AppKey::System { .. },
            ..
        }
        | Order::Upgrade {
            scope: Scope::One {
                key: AppKey::System { .. },
            },
        } => Err(Failure::refused(Refusal::DeclaredInTheFlake)),
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

/// What the index says about an attribute before anything runs: it exists,
/// nothing refuses it, and its licence is either free or already said yes to.
/// Returns whether it is unfree (so the call needs the flag). Without an
/// index the attribute is only checked for shape and Nix is the judge — it
/// refuses broken, insecure and unfree packages itself, in words
/// [`crate::nix_log::classify`] reads.
///
/// # Errors
///
/// The refusal, as a value.
pub fn vet(ctx: &Context, attr: &str, consent: Consent) -> Result<bool, Failure> {
    if !attr_is_valid(attr) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let Some(index) = ctx.tools.nix_index.as_deref() else {
        return Ok(ctx.prefs.unfree.contains(attr));
    };
    let row =
        crate::nix_index::row(index, attr).ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
    if let Some(refusal) = row.refusal() {
        return Err(Failure::refused(refusal));
    }
    if row.unfree && !consent.unfree && !ctx.prefs.unfree.contains(attr) {
        return Err(Failure::refused(Refusal::UnfreeNeedsConsent));
    }
    Ok(row.unfree)
}

/// A Nix call with the unfree door opened when asked.
fn nix_call<'a>(nix: &'a Path, argv: Vec<String>, unfree: bool) -> Call<'a> {
    let call = Call::new(nix, argv).changing();
    if unfree {
        call.env(UNFREE_VARIABLE, "1")
    } else {
        call
    }
}

/// Run a Nix call that prints internal-json, reporting its progress; a bad
/// exit is the log's failure.
///
/// # Errors
///
/// The failure, in Nix's words.
pub fn run_logged(ctx: &Context, call: Call<'_>, report: &mut dyn Report) -> Result<Ran, Failure> {
    let mut log = Log::new();
    let mut pace = Pace::new();
    let ran = {
        let mut each = crate::nix_log::follow(&mut log, &mut pace, report);
        call.run(ctx, &mut each)?
    };
    if ran.code == Some(0) || ran.rehearsed {
        Ok(ran)
    } else {
        Err(log.failure(&ran.tail))
    }
}

/// Install one package into the person's profile (also how a kept trial
/// becomes installed: the same store path, already realised).
///
/// # Errors
///
/// A refusal before anything runs, or Nix's failure.
pub fn install(ctx: &Context, attr: &str, consent: Consent, report: &mut dyn Report) -> Done {
    let nix = crate::run::tool(ctx.tools.nix.as_ref())?;
    let unfree = vet(ctx, attr, consent)?;
    report.step(Phase::Resolving, 0, 0, attr);
    let ran = run_logged(ctx, nix_call(nix, add_argv(attr, unfree), unfree), report)?;
    let key = AppKey::Nix {
        attr: attr.to_owned(),
    };
    if ran.rehearsed {
        return Ok(Outcome {
            key: Some(key),
            rehearsed: true,
            ..Outcome::default()
        });
    }
    report.step(Phase::Integrating, 0, 0, attr);
    if unfree {
        remember_unfree(&ctx.roots, attr);
    }
    let desktop_ids = manifest(ctx)
        .ok()
        .and_then(|m| {
            m.find(attr).map(|e| {
                e.store_paths
                    .iter()
                    .flat_map(|p| desktop_ids_in(Path::new(p)))
                    .collect()
            })
        })
        .unwrap_or_default();
    Ok(Outcome {
        key: Some(key),
        desktop_ids,
        amount: None,
        rehearsed: false,
    })
}

/// Add `attr` to the remembered unfree consents, so its upgrades pass the
/// flag. Never over a preferences file that does not parse (`parsed()`, not
/// `read()`: a writer that took the default would erase the person's file).
fn remember_unfree(roots: &Roots, attr: &str) {
    if let Err(e) = crate::prefs::change(&roots.config, |p| {
        p.unfree.insert(attr.to_owned());
    }) {
        tracing::warn!(error = %e, "the unfree consent could not be remembered");
    }
}

/// Take one package out of the profile. `keep_data` has nothing to act on
/// here: a Nix package keeps what it saved in the person's own dotfiles,
/// under names no manifest records, and guessing them would be deleting files
/// this crate never wrote. The store path itself goes at the next collection.
fn remove(ctx: &Context, attr: &str, report: &mut dyn Report) -> Done {
    let nix = crate::run::tool(ctx.tools.nix.as_ref())?;
    let manifest = manifest(ctx)?;
    let element = manifest
        .find(attr)
        .ok_or_else(|| Failure::refused(Refusal::UnknownItem))?;
    report.step(Phase::Removing, 0, 0, &element.name);
    let target = if element.name.bytes().all(|b| b.is_ascii_digit()) {
        element
            .store_paths
            .first()
            .cloned()
            .unwrap_or_else(|| element.name.clone())
    } else {
        element.name.clone()
    };
    let ran = run_logged(ctx, nix_call(nix, remove_argv(&target), false), report)?;
    Ok(Outcome {
        key: Some(AppKey::Nix {
            attr: attr.to_owned(),
        }),
        rehearsed: ran.rehearsed,
        ..Outcome::default()
    })
}

/// The element names to upgrade, split by whether they need the unfree door.
/// Elements installed from a store path are left out: Nix only warns about
/// them, and there is nothing to move them to.
#[must_use]
pub fn upgrade_plan(
    manifest: &Manifest,
    only: Option<&str>,
    unfree_rows: &dyn Fn(&str) -> bool,
    consented: &std::collections::BTreeSet<String>,
) -> (Vec<String>, Vec<String>) {
    let mut free = Vec::new();
    let mut unfree = Vec::new();
    for e in manifest.elements.values() {
        let Some(attr) = e.attr() else { continue };
        if only.is_some_and(|o| o != attr && o != e.name) {
            continue;
        }
        let needs = consented.contains(attr) || (e.from_nixpkgs() && unfree_rows(attr));
        if needs {
            unfree.push(e.name.clone())
        } else {
            free.push(e.name.clone())
        }
    }
    (free, unfree)
}

fn upgrade(ctx: &Context, only: Option<&str>, report: &mut dyn Report) -> Done {
    let nix = crate::run::tool(ctx.tools.nix.as_ref())?;
    let before = manifest(ctx)?;
    let index = ctx.tools.nix_index.clone();
    let unfree_rows = |attr: &str| {
        index
            .as_deref()
            .and_then(|i| crate::nix_index::row(i, attr))
            .is_some_and(|r| r.unfree)
    };
    let (free, unfree) = upgrade_plan(&before, only, &unfree_rows, &ctx.prefs.unfree);
    let key = only.map(|a| AppKey::Nix { attr: a.to_owned() });
    if only.is_some() && free.is_empty() && unfree.is_empty() {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    let mut rehearsed = false;
    for (names, door) in [(free, false), (unfree, true)] {
        if names.is_empty() {
            continue;
        }
        report.step(Phase::Resolving, 0, 0, &names.join(" "));
        rehearsed |=
            run_logged(ctx, nix_call(nix, upgrade_argv(&names, door), door), report)?.rehearsed;
    }
    if rehearsed {
        return Ok(Outcome {
            key,
            rehearsed: true,
            ..Outcome::default()
        });
    }
    // How many moved: the elements whose store paths are not what they were.
    let moved = manifest(ctx).map_or(0, |after| {
        after
            .elements
            .iter()
            .filter(|(name, e)| {
                before
                    .elements
                    .get(*name)
                    .is_some_and(|b| b.store_paths != e.store_paths)
            })
            .count()
    });
    Ok(Outcome {
        key,
        desktop_ids: Vec::new(),
        amount: Some(moved as u64),
        rehearsed: false,
    })
}

fn tidy(ctx: &Context, report: &mut dyn Report) -> Done {
    let nix = crate::run::tool(ctx.tools.nix.as_ref())?;
    let link = profile_link(&ctx.roots);
    let count = || generations_at(&link).len();
    let before = count();
    report.step(Phase::Removing, 0, 0, "");
    let ran = Call::new(nix, tidy_argv()).changing().quietly(ctx)?.ok()?;
    if ran.rehearsed {
        return Ok(Outcome {
            rehearsed: true,
            ..Outcome::default()
        });
    }
    Ok(Outcome {
        amount: Some(before.saturating_sub(count()) as u64),
        ..Outcome::default()
    })
}

fn rollback(ctx: &Context, generation: u64, report: &mut dyn Report) -> Done {
    let nix = crate::run::tool(ctx.tools.nix.as_ref())?;
    if !generations(ctx).iter().any(|g| g.number == generation) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    report.step(Phase::Installing, 0, 0, &generation.to_string());
    let ran = Call::new(nix, rollback_argv(generation))
        .changing()
        .quietly(ctx)?
        .ok()?;
    Ok(Outcome {
        amount: Some(generation),
        rehearsed: ran.rehearsed,
        ..Outcome::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::Beat;
    use crate::{Tools, prefs::Preferences};

    fn fixture(name: &str) -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/nix")
            .join(name);
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-nix-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A fake `nix`: a `/bin/sh` script that records its arguments and its
    /// unfree variable, and answers `profile list --json` with `list`.
    fn fake_nix(dir: &Path, list: &str, exit: i32) -> PathBuf {
        let listing = dir.join("list.json");
        std::fs::write(&listing, list).unwrap();
        let script = dir.join("nix");
        write_executable(
            &script,
            &format!(
                "#!/bin/sh\necho \"$* | ${{NIXPKGS_ALLOW_UNFREE:-}}\" >> '{calls}'\n\
                 if [ \"$1 $2\" = 'profile list' ]; then cat '{listing}'; exit 0; fi\n\
                 cat '{log}' >&2\nexit {exit}\n",
                calls = dir.join("calls").display(),
                listing = listing.display(),
                log = dir.join("log").display(),
            ),
        );
        std::fs::write(dir.join("log"), "").unwrap();
        script
    }

    use crate::write_executable;

    fn calls(dir: &Path) -> Vec<String> {
        std::fs::read_to_string(dir.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn index(dir: &Path) -> PathBuf {
        let path = dir.join("index.tsv");
        std::fs::write(
            &path,
            "#ronoctua-software-nix-index\t1\t4\n\
             alexandria\talexandria\t0.13.1\tb\talexandria\t\t\tBooks\n\
             gimp\tgimp\t3.0.8\t\tgimp\tGPL-3.0-or-later\t\tImages\n\
             google-chrome\tgoogle-chrome\t151.0.7922.71\tu\tgoogle-chrome-stable\tunfree\t\tBrowser\n\
             hello\thello\t2.12.3\t\thello\tGPL-3.0-or-later\t\tGreeting\n",
        )
        .unwrap();
        path
    }

    fn ctx(dir: &Path, nix: Option<PathBuf>) -> Context {
        Context {
            tools: Tools {
                nix,
                nix_index: Some(index(dir)),
                ..Tools::default()
            },
            roots: Roots::under(dir),
            prefs: Preferences::default(),
            rehearse: false,
        }
    }

    #[test]
    fn every_argument_vector_ends_its_flags_before_what_it_names() {
        assert_eq!(
            add_argv("hello", false),
            [
                "profile",
                "add",
                "--log-format",
                "internal-json",
                "--",
                "nixpkgs#hello"
            ]
        );
        assert_eq!(
            add_argv("google-chrome", true),
            [
                "profile",
                "add",
                "--log-format",
                "internal-json",
                "--impure",
                "--",
                "nixpkgs#google-chrome"
            ]
        );
        assert_eq!(
            remove_argv("hello"),
            [
                "profile",
                "remove",
                "--log-format",
                "internal-json",
                "--",
                "hello"
            ]
        );
        assert_eq!(
            upgrade_argv(&["a".into(), "b".into()], true),
            [
                "profile",
                "upgrade",
                "--log-format",
                "internal-json",
                "--impure",
                "--",
                "a",
                "b"
            ]
        );
        assert_eq!(list_argv(), ["profile", "list", "--json"]);
        assert_eq!(
            tidy_argv(),
            ["profile", "wipe-history", "--older-than", "30d"]
        );
        assert_eq!(rollback_argv(7), ["profile", "rollback", "--to", "7"]);
    }

    #[test]
    fn an_attribute_is_a_name_and_never_a_reference() {
        for good in [
            "hello",
            "python3Packages.requests",
            "gtk+3",
            "_7zz",
            "nixVersions.nix_2_34",
            "a-b",
        ] {
            assert!(attr_is_valid(good), "{good}");
        }
        for bad in [
            "",
            "-flag",
            "a..b",
            ".a",
            "a.",
            "github:x/y",
            "hello^out",
            "../x",
            "a b",
            "a#b",
            "ação",
        ] {
            assert!(!attr_is_valid(bad), "{bad}");
        }
    }

    #[test]
    fn a_version_three_manifest_is_read_by_name() {
        let m = Manifest::parse(&fixture("manifest-v3.json")).unwrap();
        assert_eq!(
            m.elements.keys().collect::<Vec<_>>(),
            ["figlet", "google-chrome", "hello", "my-tool", "requests"]
        );
        let hello = &m.elements["hello"];
        assert_eq!(hello.attr(), Some("hello"));
        assert!(hello.from_nixpkgs());
        assert_eq!(hello.store_name(), "hello-2.12.3");
        assert_eq!(
            m.elements["requests"].attr(),
            Some("python3Packages.requests")
        );
        assert_eq!(m.find("python3Packages.requests").unwrap().name, "requests");
        // Installed by store path: no attribute, found by its name.
        let tool = &m.elements["my-tool"];
        assert_eq!(tool.attr(), None);
        assert_eq!(
            tool.key(),
            AppKey::Nix {
                attr: "my-tool".into()
            }
        );
        assert!(
            !m.elements["figlet"].from_nixpkgs(),
            "another flake's attribute is not nixpkgs'"
        );
        assert!(!m.elements["figlet"].active);
    }

    #[test]
    fn an_old_manifest_is_read_too_and_names_its_elements_like_nix() {
        let v2 = r#"{"version":2,"elements":[
            {"active":true,"attrPath":"legacyPackages.x86_64-linux.hello","originalUrl":"flake:nixpkgs","url":"x","outputs":null,"priority":5,"storePaths":["/nix/store/6i6xl6bmcpxqd51m8nlva40d5c1bhndx-hello-2.12.3"]},
            {"active":true,"attrPath":"packages.x86_64-linux.default","originalUrl":"github:o/r","url":"x","storePaths":["/nix/store/x46pli2va967nknmvq1s9dy55lcwwdka-tool-1.0"]},
            {"active":true,"attrPath":"legacyPackages.x86_64-linux.hello","originalUrl":"flake:nixpkgs","url":"x","storePaths":["/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-hello-2.12.3"]}]}"#;
        let m = Manifest::parse(v2).unwrap();
        assert_eq!(
            m.elements.keys().collect::<Vec<_>>(),
            ["hello", "hello-1", "tool-1.0"]
        );
        let v1 = r#"{"version":1,"elements":[{"active":true,"attrPath":"legacyPackages.x86_64-linux.hello","originalUri":"flake:nixpkgs","uri":"x","storePaths":[]}]}"#;
        assert!(Manifest::parse(v1).unwrap().elements["hello"].from_nixpkgs());
        assert_eq!(Manifest::parse(r#"{"version":9,"elements":{}}"#), None);
        assert_eq!(Manifest::parse("not json"), None);
    }

    #[test]
    fn the_installed_rows_carry_version_update_unfree_and_launcher_ids() {
        let dir = scratch("installed");
        let store = dir.join("store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-gimp-3.0.6");
        std::fs::create_dir_all(store.join("share/applications/kde")).unwrap();
        std::fs::write(
            store.join("share/applications/gimp.desktop"),
            "[Desktop Entry]\nType=Application\nName=GIMP\nIcon=gimp\nExec=gimp %U\n",
        )
        .unwrap();
        std::fs::write(
            store.join("share/applications/kde/extra.desktop"),
            "[Desktop Entry]\n",
        )
        .unwrap();
        let manifest = format!(
            r#"{{"version":3,"elements":{{
              "gimp":{{"active":true,"attrPath":"legacyPackages.x86_64-linux.gimp","originalUrl":"flake:nixpkgs","url":"x","outputs":null,"priority":5,"storePaths":["{}"]}},
              "google-chrome":{{"active":true,"attrPath":"legacyPackages.x86_64-linux.google-chrome","originalUrl":"flake:nixpkgs","url":"x","outputs":null,"priority":5,"storePaths":["/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-google-chrome-151.0.7922.71"]}}}}}}"#,
            store.display()
        );
        let m = Manifest::parse(&manifest).unwrap();
        let index = index(&dir);
        let rows = crate::nix_index::rows(&index, &["gimp", "google-chrome"]);
        let rows_of = installed_of(&m, &rows);
        let gimp = rows_of.iter().find(|r| r.name == "gimp").unwrap();
        assert_eq!(
            gimp.key,
            AppKey::Nix {
                attr: "gimp".into()
            }
        );
        assert_eq!(gimp.version, "3.0.6");
        assert_eq!(gimp.update.as_deref(), Some("3.0.8"));
        assert_eq!(gimp.desktop_ids, ["gimp.desktop", "kde-extra.desktop"]);
        assert_eq!(
            gimp.icon,
            Some(Picture::Named {
                name: "gimp".into()
            })
        );
        assert!(!gimp.unfree);
        let chrome = rows_of.iter().find(|r| r.name == "google-chrome").unwrap();
        assert!(chrome.unfree);
        assert_eq!(chrome.update, None, "same version, no update");
        assert_eq!(chrome.origin, Origin::Mine);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_version_is_what_follows_the_name_and_nothing_is_guessed() {
        assert_eq!(version_of("hello-2.12.3", "hello"), "2.12.3");
        assert_eq!(
            version_of("python3.13-requests-2.32.5", "requests"),
            "python3.13-requests-2.32.5"
        );
        assert_eq!(version_of("hello", "hello"), "hello");
    }

    #[test]
    fn generations_are_read_from_the_links_newest_first_with_what_changed() {
        let dir = scratch("generations");
        let roots = Roots::under(&dir);
        let profiles = roots.state.join("nix/profiles");
        std::fs::create_dir_all(&profiles).unwrap();
        let gens = [
            (
                1,
                r#"{"version":3,"elements":{"hello":{"active":true,"storePaths":["/s/a-hello-1"]}}}"#,
            ),
            (
                2,
                r#"{"version":3,"elements":{"hello":{"active":true,"storePaths":["/s/a-hello-1"]},"gimp":{"active":true,"storePaths":["/s/b-gimp-3"]}}}"#,
            ),
            (
                4,
                r#"{"version":3,"elements":{"gimp":{"active":true,"storePaths":["/s/c-gimp-3.1"]},"vlc":{"active":true,"storePaths":["/s/d-vlc-3"]}}}"#,
            ),
        ];
        for (n, manifest) in gens {
            let store = dir.join(format!("store/gen{n}"));
            std::fs::create_dir_all(&store).unwrap();
            std::fs::write(store.join("manifest.json"), manifest).unwrap();
            std::os::unix::fs::symlink(&store, profiles.join(format!("profile-{n}-link"))).unwrap();
        }
        std::os::unix::fs::symlink("profile-2-link", profiles.join("profile")).unwrap();
        // A stray file that only looks like one.
        std::fs::write(profiles.join("profile-x-link"), "").unwrap();
        std::fs::create_dir_all(&roots.home).unwrap();
        std::os::unix::fs::symlink(profiles.join("profile"), roots.home.join(".nix-profile"))
            .unwrap();
        assert_eq!(profile_link(&roots), profiles.join("profile"));
        let found = generations_at(&profile_link(&roots));
        assert_eq!(
            found.iter().map(|g| g.number).collect::<Vec<_>>(),
            [4, 2, 1]
        );
        assert_eq!(
            found.iter().map(|g| g.current).collect::<Vec<_>>(),
            [false, true, false]
        );
        assert_eq!(found[2].summary, "+hello");
        assert_eq!(found[1].summary, "+gimp");
        assert_eq!(found[0].summary, "~gimp +vlc -hello");
        assert!(found[0].when > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_any_link_the_profile_is_where_nix_would_make_it() {
        let dir = scratch("nolink");
        let roots = Roots::under(&dir);
        assert_eq!(
            profile_link(&roots),
            roots.state.join("nix/profiles/profile")
        );
        assert!(generations_at(&profile_link(&roots)).is_empty());
        assert_eq!(generation_number("profile-12-link", "profile"), Some(12));
        assert_eq!(generation_number("profile-12", "profile"), None);
        assert_eq!(generation_number("other-3-link", "profile"), None);
    }

    #[test]
    fn a_broken_or_unknown_package_is_refused_before_nix_runs() {
        let dir = scratch("vet");
        let nix = fake_nix(&dir, "{}", 0);
        let c = ctx(&dir, Some(nix));
        let mut beats: Vec<Beat> = Vec::new();
        let install = |attr: &str, consent| Order::Install {
            key: AppKey::Nix { attr: attr.into() },
            consent,
        };
        for (attr, refusal) in [
            ("alexandria", Refusal::Broken),
            ("not-in-the-index", Refusal::UnknownItem),
            ("github:x/y", Refusal::UnknownItem),
            ("google-chrome", Refusal::UnfreeNeedsConsent),
        ] {
            let err = run(&c, &install(attr, Consent::default()), &mut beats).unwrap_err();
            assert_eq!(err.refusal, Some(refusal), "{attr}");
        }
        assert!(calls(&dir).is_empty(), "nothing ran: {:?}", calls(&dir));
        let system = Order::Remove {
            key: AppKey::System {
                desktop_id: "firefox.desktop".into(),
            },
            keep_data: true,
        };
        assert_eq!(
            run(&c, &system, &mut beats).unwrap_err().refusal,
            Some(Refusal::DeclaredInTheFlake)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unfree_package_with_consent_opens_the_door_and_remembers_it() {
        let dir = scratch("unfree");
        let list = r#"{"version":3,"elements":{"google-chrome":{"active":true,"attrPath":"legacyPackages.x86_64-linux.google-chrome","originalUrl":"flake:nixpkgs","storePaths":[]}}}"#;
        let nix = fake_nix(&dir, list, 0);
        std::fs::write(dir.join("log"), fixture("build-hello.jsonl")).unwrap();
        let c = ctx(&dir, Some(nix));
        let mut beats: Vec<Beat> = Vec::new();
        let order = Order::Install {
            key: AppKey::Nix {
                attr: "google-chrome".into(),
            },
            consent: Consent {
                unfree: true,
                ..Consent::default()
            },
        };
        let done = run(&c, &order, &mut beats).unwrap();
        assert_eq!(
            done.key,
            Some(AppKey::Nix {
                attr: "google-chrome".into()
            })
        );
        let first = calls(&dir);
        assert_eq!(
            first[0],
            "profile add --log-format internal-json --impure -- nixpkgs#google-chrome | 1"
        );
        assert!(
            crate::prefs::read(&c.roots.config)
                .unfree
                .contains("google-chrome"),
            "remembered"
        );
        assert!(beats.iter().any(|b| matches!(
            b,
            Beat::Step {
                phase: Phase::Downloading,
                total: 279_624,
                ..
            }
        )));
        // A free one never gets the door.
        let order = Order::Install {
            key: AppKey::Nix {
                attr: "hello".into(),
            },
            consent: Consent::default(),
        };
        run(&c, &order, &mut beats).unwrap();
        assert!(
            calls(&dir)
                .iter()
                .any(|l| l == "profile add --log-format internal-json -- nixpkgs#hello | ")
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failure_says_why_in_nixs_words() {
        let dir = scratch("fails");
        let nix = fake_nix(&dir, "{}", 1);
        std::fs::write(dir.join("log"), fixture("refused-conflict.jsonl")).unwrap();
        let c = ctx(&dir, Some(nix));
        let order = Order::Install {
            key: AppKey::Nix {
                attr: "hello".into(),
            },
            consent: Consent::default(),
        };
        let err = run(&c, &order, &mut Vec::new()).unwrap_err();
        assert_eq!(err.refusal, Some(Refusal::Conflict));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_takes_the_element_whose_attribute_it_is() {
        let dir = scratch("remove");
        let nix = fake_nix(&dir, &fixture("manifest-v3.json"), 0);
        let c = ctx(&dir, Some(nix));
        let remove = |attr: &str| Order::Remove {
            key: AppKey::Nix { attr: attr.into() },
            keep_data: true,
        };
        run(&c, &remove("python3Packages.requests"), &mut Vec::new()).unwrap();
        assert_eq!(
            calls(&dir).last().unwrap(),
            "profile remove --log-format internal-json -- requests | "
        );
        let err = run(&c, &remove("vlc"), &mut Vec::new()).unwrap_err();
        assert_eq!(err.refusal, Some(Refusal::UnknownItem));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_upgrade_opens_the_door_only_for_the_unfree_elements() {
        let m = Manifest::parse(&fixture("manifest-v3.json")).unwrap();
        let unfree_rows = |a: &str| a == "google-chrome";
        let none = std::collections::BTreeSet::new();
        let (free, unfree) = upgrade_plan(&m, None, &unfree_rows, &none);
        // `my-tool` came from a store path: there is nothing to upgrade it to.
        assert_eq!(free, ["figlet", "hello", "requests"]);
        assert_eq!(unfree, ["google-chrome"]);
        let (free, unfree) = upgrade_plan(&m, Some("hello"), &unfree_rows, &none);
        assert_eq!((free, unfree), (vec!["hello".to_owned()], vec![]));
        // A remembered consent counts even when the index lost the flag.
        let consented = ["hello".to_owned()].into_iter().collect();
        let (_, unfree) = upgrade_plan(&m, Some("hello"), &|_: &str| false, &consented);
        assert_eq!(unfree, ["hello"]);

        let dir = scratch("upgrade");
        let nix = fake_nix(&dir, &fixture("manifest-v3.json"), 0);
        let c = ctx(&dir, Some(nix));
        let done = run(
            &c,
            &Order::Upgrade {
                scope: Scope::Source {
                    source: Source::Nix,
                },
            },
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(done.amount, Some(0), "the fake moved nothing");
        let made = calls(&dir);
        assert!(
            made.contains(
                &"profile upgrade --log-format internal-json -- figlet hello requests | "
                    .to_owned()
            ),
            "{made:?}"
        );
        assert!(
            made.contains(
                &"profile upgrade --log-format internal-json --impure -- google-chrome | 1"
                    .to_owned()
            ),
            "{made:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_rehearsal_runs_nothing_that_changes_the_profile() {
        let dir = scratch("rehearse");
        let nix = fake_nix(&dir, &fixture("manifest-v3.json"), 0);
        let mut c = ctx(&dir, Some(nix));
        c.rehearse = true;
        let order = Order::Install {
            key: AppKey::Nix {
                attr: "hello".into(),
            },
            consent: Consent::default(),
        };
        let done = run(&c, &order, &mut Vec::new()).unwrap();
        assert!(done.rehearsed);
        let order = Order::Remove {
            key: AppKey::Nix {
                attr: "hello".into(),
            },
            keep_data: true,
        };
        assert!(run(&c, &order, &mut Vec::new()).unwrap().rehearsed);
        // Only the READ ran (the list, to find the element).
        assert!(
            calls(&dir)
                .iter()
                .all(|l| l.starts_with("profile list --json")),
            "{:?}",
            calls(&dir)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn without_nix_the_source_is_absent() {
        let dir = scratch("absent");
        let c = ctx(&dir, None);
        let order = Order::Install {
            key: AppKey::Nix {
                attr: "hello".into(),
            },
            consent: Consent::default(),
        };
        assert_eq!(
            run(&c, &order, &mut Vec::new()).unwrap_err().refusal,
            Some(Refusal::SourceAbsent)
        );
        assert!(installed(&c).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
