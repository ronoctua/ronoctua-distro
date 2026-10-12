//! Making this machine match the declaration — and touching nothing but what
//! its own ledger says it did.
//!
//! # What Nix cannot do, this does
//!
//! Nix puts the person's declared programs in the system: a package in their
//! `home.packages`, a Flatpak on nix-flatpak's list, an AppImage wrapped, a
//! web app's launcher entry. Two things it cannot do, and this module does
//! them, as the person, after a system was applied:
//!
//! * **Android's apps.** Android is a container that exists only while the
//!   person's session runs it; Nix fetches each declared apk into the store by
//!   its pin and lists it in `/etc/ronoctua/declared-android.json`, and here it
//!   goes in — `waydroid app install <the store's apk>` — while Android runs.
//!   When it does not run, nothing happens and the screen says the app WAITS
//!   ([`crate::declaration::Declarations::android_pending`]); the next
//!   reconcile (the update's, or the session's start) puts it in.
//! * **The copy a declared program replaces.** A program installed by hand
//!   and then declared is on this machine twice once its declared copy is
//!   applied: a Nix package in the profile AND in `home.packages`, an
//!   AppImage in `~/Applications` AND wrapped, a web app's entry in
//!   `~/.local/share/applications` shadowing the system's one of the same
//!   name. Once `person-applied.json` says the declared copy IS in the
//!   system, the hand-made one goes ([`Shadow`]).
//!
//! # The ledger, and why nothing else is touched
//!
//! `$XDG_STATE_HOME/ronoctua/software/declared-applied.json` lists the
//! Android apps this module put in (or found already there, at the declared
//! version, and took charge of). It is the ONLY record of what this module
//! may take out: an app that leaves the declaration is removed from Android
//! when — and only when — the ledger has it. An app the person installed by
//! hand, from F-Droid, from the Play Store or from a file, is never in the
//! ledger and never removed here. A copy a declared program replaces is
//! removed only when the declared one is reported applied; one whose
//! declaration was skipped (a missing attribute, a held entry) stays, because
//! nothing replaces it. `reconcile-touches-only-the-ledger` proves the four,
//! with fake programs.
//!
//! Every decision is a pure function of what was read ([`plan`]); [`run`]
//! reads, decides and does, each source in its own lane.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ronoctua_person::reports::DeclaredApk;
use serde::{Deserialize, Serialize};

use crate::progress::{Phase, Report};
use crate::{AppKey, Context, Done, Failure, Order, Outcome, Source};

/// The ledger's name, under our state directory.
pub const LEDGER: &str = "declared-applied.json";

/// The ledger's format.
pub const FORMAT: u32 = 1;

/// One Android app the ledger answers for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Kept {
    /// `android:<package>`.
    pub id: String,
    /// The package.
    pub package: String,
    /// The version code the declaration had when it went in.
    pub version_code: u64,
    /// The store apk it went in from (`/nix/store/…`), or empty for one found
    /// already there and taken charge of.
    #[serde(default)]
    pub apk: String,
}

/// What this module did, and so what it may undo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Ledger {
    /// Always [`FORMAT`].
    pub format: u32,
    /// The Android apps it answers for.
    #[serde(default)]
    pub android: Vec<Kept>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self {
            format: FORMAT,
            android: Vec::new(),
        }
    }
}

impl Ledger {
    /// Whether the package is in, at this version or a later one.
    #[must_use]
    pub fn has(&self, package: &str, version_code: u64) -> bool {
        self.android
            .iter()
            .any(|k| k.package == package && k.version_code >= version_code)
    }
}

/// The ledger as it is. One that is missing or does not read is EMPTY — and
/// an empty ledger is the safe answer: it lets nothing be removed.
#[must_use]
pub fn ledger(roots: &crate::Roots) -> Ledger {
    let path = roots.ours_state().join(LEDGER);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Ledger>(&bytes)
            .ok()
            .filter(|l| l.format == FORMAT)
            .unwrap_or_else(|| {
                tracing::warn!(file = %path.display(), "the reconciler's ledger does not read: it is taken as empty");
                Ledger::default()
            }),
        Err(_) => Ledger::default(),
    }
}

/// Write the ledger, whole and atomically.
fn keep(dir: &Path, ledger: &Ledger) -> Result<(), Failure> {
    let mut text = serde_json::to_string_pretty(ledger)
        .map_err(|e| Failure::trouble(format!("the ledger: {e}")))?;
    text.push('\n');
    std::fs::create_dir_all(dir)
        .and_then(|()| ronoctua_xdg::write_atomically(dir, LEDGER, text.as_bytes()))
        .map_err(|e| Failure::trouble(format!("could not write {LEDGER}: {e}")))
}

/// A copy a declared program replaces, which goes once the declared one is
/// applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shadow {
    /// A Nix package in the person's profile, by attribute.
    Nix(String),
    /// An AppImage of `~/Applications`, by key.
    AppImage(String),
    /// A web app's entry of `~/.local/share/applications`, by slug.
    Web(String),
}

impl Shadow {
    /// Its source.
    #[must_use]
    pub const fn source(&self) -> Source {
        match self {
            Self::Nix(_) => Source::Nix,
            Self::AppImage(_) => Source::AppImage,
            Self::Web(_) => Source::Web,
        }
    }

    /// The key of the copy that goes.
    #[must_use]
    pub fn key(&self) -> AppKey {
        match self {
            Self::Nix(attr) => AppKey::Nix { attr: attr.clone() },
            Self::AppImage(key) => AppKey::AppImage { key: key.clone() },
            Self::Web(slug) => AppKey::Web { slug: slug.clone() },
        }
    }
}

/// What the person installed by hand, as the lists say it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ByHand {
    /// The attributes in the Nix profile.
    pub nix: BTreeSet<String>,
    /// The keys of `~/Applications`' AppImages.
    pub appimages: BTreeSet<String>,
    /// The slugs of the person's own web app entries.
    pub web: BTreeSet<String>,
}

/// What a reconcile is to do.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    /// Declared apks to put in Android.
    pub install: Vec<DeclaredApk>,
    /// Declared apks already in Android at their version or a later one:
    /// into the ledger, with nothing installed.
    pub adopt: Vec<DeclaredApk>,
    /// Apps the ledger put in that nobody declares any more.
    pub remove: Vec<Kept>,
    /// Apps the ledger has that are no longer in Android (taken out by hand):
    /// out of the ledger, with nothing removed.
    pub forget: Vec<Kept>,
    /// Copies a declared program replaces.
    pub shadows: Vec<Shadow>,
    /// Android does not run: what it is to have waits.
    pub waiting: bool,
}

/// What a reconcile is to do — pure.
///
/// * `declared`: the apks this machine's Android is to have.
/// * `android`: what is inside Android now, by package, with the version
///   code our record knows (0: not known) — `None` when Android does not run
///   (or this machine has none), and then nothing of Android moves.
/// * `applied`: the ids `person-applied.json` says the system has.
/// * `by_hand`: what the lists say the person installed themselves.
/// * `only`: one source, or every one.
#[must_use]
pub fn plan(
    declared: &[DeclaredApk],
    ledger: &Ledger,
    android: Option<&BTreeMap<String, u64>>,
    applied: &[String],
    by_hand: &ByHand,
    only: Option<Source>,
) -> Plan {
    let wants = |s: Source| only.is_none_or(|o| o == s);
    let mut p = Plan::default();
    if wants(Source::Android) {
        match android {
            None => {
                p.waiting = declared
                    .iter()
                    .any(|d| !ledger.has(&d.package, d.version_code))
            }
            Some(inside) => {
                for d in declared {
                    if ledger.has(&d.package, d.version_code) && inside.contains_key(&d.package) {
                        continue;
                    }
                    match inside.get(&d.package) {
                        // THERE ALREADY, at its version or a later one (or
                        // one our record does not know): taken charge of.
                        // Installing over a newer one would be a downgrade
                        // Android refuses.
                        Some(&have) if have == 0 || have >= d.version_code => {
                            p.adopt.push(d.clone());
                        }
                        _ => p.install.push(d.clone()),
                    }
                }
                for k in &ledger.android {
                    if declared.iter().any(|d| d.package == k.package) {
                        continue;
                    }
                    if inside.contains_key(&k.package) {
                        p.remove.push(k.clone());
                    } else {
                        p.forget.push(k.clone());
                    }
                }
            }
        }
    }
    for id in applied {
        let Some((source, rest)) = id.split_once(':') else {
            continue;
        };
        let shadow = match source {
            "nix" if wants(Source::Nix) && by_hand.nix.contains(rest) => {
                Shadow::Nix(rest.to_owned())
            }
            "appimage" if wants(Source::AppImage) && by_hand.appimages.contains(rest) => {
                Shadow::AppImage(rest.to_owned())
            }
            "web" if wants(Source::Web) && by_hand.web.contains(rest) => {
                Shadow::Web(rest.to_owned())
            }
            _ => continue,
        };
        p.shadows.push(shadow);
    }
    p
}

/// Take a source's lane, saying so when it has to wait.
fn lane(
    ctx: &Context,
    source: Source,
    report: &mut dyn Report,
) -> Result<crate::lock::Lane, Failure> {
    crate::lock::take(&crate::lock::directory(&ctx.roots), source, || {
        report.step(Phase::Waiting, 0, 0, source.word());
    })
    .map_err(|e| Failure::trouble(format!("could not take the {} lane: {e}", source.word())))
}

/// What the person installed by hand, read from the lists of the sources
/// this reconcile reaches.
fn by_hand(ctx: &Context, only: Option<Source>) -> ByHand {
    let wants = |s: Source| only.is_none_or(|o| o == s);
    ByHand {
        nix: if wants(Source::Nix) && ctx.tools.nix.is_some() {
            crate::nix::manifest(ctx)
                .map(|m| {
                    m.elements
                        .values()
                        .filter(|e| e.from_nixpkgs())
                        .filter_map(|e| e.attr().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            BTreeSet::new()
        },
        appimages: if wants(Source::AppImage) {
            crate::appimage::sidecars(&ctx.roots)
                .into_iter()
                .map(|s| s.key)
                .collect()
        } else {
            BTreeSet::new()
        },
        web: if wants(Source::Web) {
            let declared = crate::declaration::read_declared(&ctx.roots).ok();
            crate::web::list(&ctx.roots)
                .into_iter()
                .filter(|w| {
                    !waits_for_build(
                        w,
                        declared.as_ref(),
                        crate::web::read_system(&ctx.roots, &w.slug).as_ref(),
                    )
                })
                .map(|w| w.slug)
                .collect()
        } else {
            BTreeSet::new()
        },
    }
}

/// Whether the person's own entry of a web app is a change the system has
/// not built yet — and so is no copy a declared one replaces, though the
/// report says the declared one is applied: the editor changed a declared
/// web app (it is written here and declared again,
/// [`crate::declare::edit_declared_web`]), and until the next apply the
/// system's entry is the old one. Taking the person's away then would bring
/// the old address back until the update.
///
/// It waits when it IS what the declaration now says (name, address,
/// profile) and the system's entry is not that; and when the declaration
/// does not read here, because then nobody can tell. A person's entry that
/// the declaration does not say is a stale copy, and goes. Pure.
#[must_use]
pub fn waits_for_build(
    mine: &crate::web::WebApp,
    declared: Option<&ronoctua_person::declared::Declared>,
    system: Option<&crate::web::WebApp>,
) -> bool {
    use ronoctua_person::declared::Entry;
    let Some(declared) = declared else {
        return true;
    };
    let id = format!("{}:{}", Source::Web.word(), mine.slug);
    let Some(Entry::Web(e)) = declared.entry(&id) else {
        return false;
    };
    let url = crate::web::canonical_url(&mine.url).unwrap_or_else(|_| mine.url.clone());
    let says = url == e.url && mine.name == e.name && mine.profile == e.profile;
    let built =
        system.is_some_and(|s| s.url == e.url && s.name == e.name && s.profile == e.profile);
    says && !built
}

/// What is inside Android now, by package, with the version our record
/// knows; `None` when Android does not run here.
fn inside_android(ctx: &Context) -> Option<BTreeMap<String, u64>> {
    ctx.tools.waydroid.as_ref()?;
    if !crate::android::running(ctx) {
        return None;
    }
    let record = crate::android::record(&ctx.roots);
    let listed = crate::android::listed(ctx)
        .map_err(|e| tracing::warn!(error = %e, "Android's apps could not be listed"))
        .ok()?;
    Some(
        listed
            .into_iter()
            .map(|l| {
                let version = record.get(&l.package).map_or(0, |r| r.version_code);
                (l.package, version)
            })
            .collect(),
    )
}

/// Reconcile this machine with the declaration: one source, or all.
///
/// Changes are counted in the outcome's amount. In a rehearsal every program
/// is only printed, and the ledger is not written.
///
/// # Errors
///
/// The first failure of a change; what was done before it stays done, and
/// is in the ledger.
pub fn run(ctx: &Context, only: Option<Source>, report: &mut dyn Report) -> Done {
    let declared = crate::declaration::declared_android(&ctx.roots)
        .map(|d| d.apps)
        .unwrap_or_default();
    let applied = crate::declaration::person_applied(&ctx.roots)
        .map(|r| r.applied)
        .unwrap_or_default();
    let mut ledger = ledger(&ctx.roots);
    let dir = ctx.roots.ours_state();
    let mut changed: u64 = 0;
    report.step(Phase::Reconciling, 0, 0, only.map_or("", Source::word));

    // THE COPIES A DECLARED PROGRAM REPLACES, each in its source's lane, by
    // that source's own removal — so a rehearsal prints it, and the screen's
    // history says it.
    let hands = by_hand(ctx, only);
    let shadows = plan(&[], &ledger, None, &applied, &hands, only).shadows;
    for shadow in shadows {
        let _lane = lane(ctx, shadow.source(), report)?;
        let order = Order::Remove {
            key: shadow.key(),
            keep_data: true,
        };
        let done = match shadow.source() {
            Source::Nix => crate::nix::run(ctx, &order, report),
            Source::AppImage => crate::appimage::run(ctx, &order, report),
            _ => crate::web::run(ctx, &order, report),
        };
        done?;
        changed += 1;
    }

    // ANDROID, in its lane.
    if only.is_none_or(|s| s == Source::Android) && ctx.tools.waydroid.is_some() {
        let _lane = lane(ctx, Source::Android, report)?;
        let inside = inside_android(ctx);
        let p = plan(
            &declared,
            &ledger,
            inside.as_ref(),
            &[],
            &ByHand::default(),
            Some(Source::Android),
        );
        let total = (p.install.len() + p.remove.len()) as u64;
        let mut step = 0;
        for d in &p.install {
            step += 1;
            report.step(Phase::Installing, step, total, &d.package);
            crate::android::install_apk(ctx, Path::new(&d.apk))?;
            take(&mut ledger, d, &d.apk);
            changed += 1;
            if !ctx.rehearse {
                keep(&dir, &ledger)?;
            }
        }
        for d in &p.adopt {
            take(&mut ledger, d, "");
        }
        for k in &p.remove {
            step += 1;
            report.step(Phase::Removing, step, total, &k.package);
            crate::android::remove_package(ctx, &k.package)?;
            ledger.android.retain(|x| x.package != k.package);
            changed += 1;
            if !ctx.rehearse {
                keep(&dir, &ledger)?;
            }
        }
        for k in &p.forget {
            ledger.android.retain(|x| x.package != k.package);
        }
        if !ctx.rehearse && (!p.adopt.is_empty() || !p.forget.is_empty()) {
            keep(&dir, &ledger)?;
        }
    }
    Ok(Outcome {
        key: None,
        desktop_ids: Vec::new(),
        amount: Some(changed),
        rehearsed: ctx.rehearse,
    })
}

/// The ledger answers for `d` now.
fn take(ledger: &mut Ledger, d: &DeclaredApk, apk: &str) {
    ledger.android.retain(|k| k.package != d.package);
    ledger.android.push(Kept {
        id: d.id.clone(),
        package: d.package.clone(),
        version_code: d.version_code,
        apk: apk.to_owned(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Roots, Tools};
    use ronoctua_person::declared::AndroidImage;
    use ronoctua_person::reports::Declarer;

    fn apk(package: &str, version_code: u64) -> DeclaredApk {
        DeclaredApk {
            id: format!("android:{package}"),
            package: package.into(),
            apk: format!("/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-{package}.apk"),
            version_code,
            from: Declarer::Person,
            image: AndroidImage::Gapps,
        }
    }

    fn kept(package: &str, version_code: u64) -> Kept {
        Kept {
            id: format!("android:{package}"),
            package: package.into(),
            version_code,
            apk: String::new(),
        }
    }

    fn web_app(url: &str) -> crate::web::WebApp {
        crate::web::WebApp {
            slug: "mail".into(),
            name: "Mail".into(),
            url: url.into(),
            profile: "Default".into(),
            icon: None,
            icon_file: None,
            category: Some("Network".into()),
        }
    }

    /// FAILS IF: the person's entry of a declared web app they just changed
    /// (written here, declared again) is taken away while the system's entry
    /// is still the old one — the old address would come back until the
    /// update — or a stale entry the declaration no longer says is kept, or
    /// one is taken away while the declaration does not read.
    #[test]
    fn a_changed_declared_web_app_waits_for_its_build() {
        use ronoctua_person::declared::{Declared, Entry, WebApp};
        let declared = Declared {
            apps: vec![Entry::Web(WebApp {
                id: "web:mail".into(),
                machines: None,
                edited_at: ronoctua_person::Stamp::parse("2026-10-05T12:00:00Z").unwrap(),
                edited_on: "noctua".into(),
                slug: "mail".into(),
                name: "Mail".into(),
                url: "https://new.example.org/".into(),
                profile: "Default".into(),
                icon: None,
                category: "Network".into(),
            })],
            ..Declared::default()
        };
        let new = web_app("https://new.example.org/");
        let old = web_app("https://old.example.org/");
        // CHANGED HERE, NOT BUILT: it waits.
        assert!(waits_for_build(&new, Some(&declared), Some(&old)));
        assert!(waits_for_build(&new, Some(&declared), None));
        // BUILT: it goes.
        assert!(!waits_for_build(&new, Some(&declared), Some(&new)));
        // STALE (the declaration moved on, here or elsewhere): it goes.
        assert!(!waits_for_build(&old, Some(&declared), Some(&new)));
        // NOT DECLARED: the plan never takes it anyway; nothing waits.
        assert!(!waits_for_build(&new, Some(&Declared::default()), None));
        // A DECLARATION THAT DOES NOT READ: nobody can tell, nothing goes.
        assert!(waits_for_build(&old, None, Some(&new)));
    }

    /// FAILS IF: a declared app goes in twice, one already there at its
    /// version is reinstalled (or downgraded), one the ledger did not put in
    /// is removed, or Android down moves anything.
    #[test]
    fn android_moves_only_by_the_ledger() {
        let declared = [
            apk("org.declared.new", 3),
            apk("org.declared.there", 5),
            apk("org.declared.old", 9),
        ];
        let ledger = Ledger {
            format: FORMAT,
            android: vec![
                kept("org.gone", 1),
                kept("org.gone.byhand", 1),
                kept("org.declared.old", 2),
            ],
        };
        let inside: BTreeMap<String, u64> = [
            ("org.declared.there", 7),
            ("org.declared.old", 2),
            ("org.gone", 1),
            ("org.mine.byhand", 4),
        ]
        .into_iter()
        .map(|(p, v)| (p.to_owned(), v))
        .collect();
        let p = plan(
            &declared,
            &ledger,
            Some(&inside),
            &[],
            &ByHand::default(),
            None,
        );
        let names = |l: &[DeclaredApk]| l.iter().map(|d| d.package.clone()).collect::<Vec<_>>();
        assert_eq!(names(&p.install), ["org.declared.new", "org.declared.old"]);
        assert_eq!(names(&p.adopt), ["org.declared.there"]);
        assert_eq!(
            p.remove
                .iter()
                .map(|k| k.package.as_str())
                .collect::<Vec<_>>(),
            ["org.gone"]
        );
        assert_eq!(
            p.forget
                .iter()
                .map(|k| k.package.as_str())
                .collect::<Vec<_>>(),
            ["org.gone.byhand"]
        );
        assert!(!p.waiting);
        // DOWN: nothing moves, and what is not in yet waits.
        let down = plan(&declared, &ledger, None, &[], &ByHand::default(), None);
        assert!(down.install.is_empty() && down.remove.is_empty() && down.adopt.is_empty());
        assert!(down.waiting);
    }

    /// FAILS IF: a hand-made copy goes while its declared one is not applied,
    /// one nobody declared goes, or a reconcile of one source reaches another.
    #[test]
    fn a_hand_made_copy_goes_only_once_its_declared_one_is_applied() {
        let hands = ByHand {
            nix: ["ripgrep", "fd", "kept-by-hand"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            appimages: ["obsidian"].into_iter().map(str::to_owned).collect(),
            web: ["mail"].into_iter().map(str::to_owned).collect(),
        };
        let applied = [
            "nix:ripgrep",
            "appimage:obsidian",
            "web:mail",
            "flatpak:org.x.Y",
        ]
        .map(str::to_owned);
        let p = plan(&[], &Ledger::default(), None, &applied, &hands, None);
        assert_eq!(
            p.shadows,
            [
                Shadow::Nix("ripgrep".into()),
                Shadow::AppImage("obsidian".into()),
                Shadow::Web("mail".into())
            ]
        );
        let only_nix = plan(
            &[],
            &Ledger::default(),
            None,
            &applied,
            &hands,
            Some(Source::Nix),
        );
        assert_eq!(only_nix.shadows, [Shadow::Nix("ripgrep".into())]);
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-reconcile-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A tree of files, by relative path → contents, for "nothing else was
    /// touched".
    fn files(root: &Path, skip: &[&Path]) -> BTreeMap<String, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
                let path = e.path();
                if skip.iter().any(|s| path.starts_with(s)) {
                    continue;
                }
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let rel = path.strip_prefix(root).unwrap().display().to_string();
                    out.insert(rel, std::fs::read(&path).unwrap_or_default());
                }
            }
        }
        out
    }

    /// RECONCILE TOUCHES ONLY THE LEDGER, with fake programs that write down
    /// how they were called:
    ///
    /// * a declared apk not yet inside goes in, from the store, and into the
    ///   ledger;
    /// * an app the ledger put in and nobody declares goes out;
    /// * an app installed by hand — inside Android, not in the ledger — is
    ///   never removed;
    /// * a Nix package in the profile goes once its declared copy is
    ///   applied, and one whose declaration was skipped stays;
    /// * no file of the person's changes but the ledger; and with Android
    ///   down, nothing goes in, nothing out, and nothing is written.
    ///
    /// FAILS IF any of those does not hold.
    #[test]
    fn reconcile_touches_only_the_ledger() {
        let root = scratch("ledger");
        let roots = Roots::under(&root);
        let bin = root.join("bin");
        let log = root.join("calls.log");
        let listing = root.join("listing.txt");
        let running = root.join("running");
        // WAYDROID: `status` from a flag file, `app list` from a file, and
        // every change written down.
        crate::write_executable(
            &bin.join("waydroid"),
            &format!(
                "#!/bin/sh\n\
                 case \"$1 $2\" in\n\
                 'status '*) if [ -e {running} ]; then printf 'Session:\\tRUNNING\\n'; else printf 'Session:\\tSTOPPED\\n'; fi ;;\n\
                 'app list') cat {listing} ;;\n\
                 *) echo \"waydroid $*\" >> {log} ;;\n\
                 esac\n",
                running = running.display(),
                listing = listing.display(),
                log = log.display()
            ),
        );
        // NIX: a profile with three packages; every change written down.
        let manifest = r#"{"version":3,"elements":{
            "ripgrep":{"active":true,"attrPath":"legacyPackages.x86_64-linux.ripgrep","originalUrl":"flake:nixpkgs","storePaths":["/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-ripgrep-14"]},
            "fd":{"active":true,"attrPath":"legacyPackages.x86_64-linux.fd","originalUrl":"flake:nixpkgs","storePaths":["/nix/store/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-fd-10"]},
            "hello":{"active":true,"attrPath":"legacyPackages.x86_64-linux.hello","originalUrl":"flake:nixpkgs","storePaths":["/nix/store/cccccccccccccccccccccccccccccccc-hello-2"]}}}"#;
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("manifest.json"), manifest).unwrap();
        crate::write_executable(
            &bin.join("nix"),
            &format!(
                "#!/bin/sh\n\
                 if [ \"$2\" = list ]; then cat {m}; else echo \"nix $*\" >> {log}; fi\n",
                m = root.join("manifest.json").display(),
                log = log.display()
            ),
        );
        // THE SYSTEM'S REPORTS: ripgrep applied, fd skipped; two apks.
        let reports = &roots.system.reports;
        std::fs::create_dir_all(reports).unwrap();
        std::fs::write(
            reports.join(ronoctua_person::names::PERSON_APPLIED),
            r#"{"format":1,"machine":"noctua","distro-rev":"abc1234","applied":["nix:ripgrep","android:org.declared.new"],
               "skipped":[{"id":"nix:fd","reason":"attr-missing","detail":""}],
               "declined":{"honoured":[],"refused":[],"unknown":[],"still-present":[]},
               "brain":{"included":[],"awaiting-approval":[]}}"#,
        )
        .unwrap();
        let declared = ronoctua_person::reports::DeclaredAndroid {
            format: 1,
            apps: vec![apk("org.declared.new", 3)],
        };
        std::fs::write(
            reports.join(ronoctua_person::names::DECLARED_ANDROID),
            ronoctua_person::to_json(&declared),
        )
        .unwrap();
        // THE LEDGER put `org.gone` in earlier; `org.mine` is the person's.
        let state = roots.ours_state();
        keep(
            &state,
            &Ledger {
                format: FORMAT,
                android: vec![kept("org.gone", 1)],
            },
        )
        .unwrap();
        std::fs::write(
            &listing,
            "Name: Gone\npackageName: org.gone\nName: Mine\npackageName: org.mine\n",
        )
        .unwrap();
        // SOMETHING OF THE PERSON'S beside it, which must not move.
        std::fs::create_dir_all(roots.home.join("Documents")).unwrap();
        std::fs::write(roots.home.join("Documents/notes.txt"), b"mine").unwrap();
        let ctx = Context {
            tools: Tools {
                nix: Some(bin.join("nix")),
                waydroid: Some(bin.join("waydroid")),
                ..Tools::default()
            },
            roots: roots.clone(),
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        let ledger_path = state.join(LEDGER);
        let lanes = crate::lock::directory(&roots);

        // ANDROID DOWN: the hand-made Nix copy goes; Android waits.
        let before = files(&roots.home, &[&ledger_path, &lanes]);
        let ledger_before = std::fs::read(&ledger_path).unwrap();
        let done = run(&ctx, None, &mut Vec::new()).unwrap();
        assert_eq!(done.amount, Some(1));
        let calls = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            calls.contains("nix profile remove") && calls.contains("ripgrep"),
            "{calls}"
        );
        assert!(
            !calls.contains(" fd") && !calls.contains("hello"),
            "{calls}"
        );
        assert!(!calls.contains("waydroid"), "Android was down: {calls}");
        assert_eq!(std::fs::read(&ledger_path).unwrap(), ledger_before);
        assert_eq!(files(&roots.home, &[&ledger_path, &lanes]), before);

        // ANDROID UP: the declared one in, the ledger's one out, the
        // person's own untouched.
        std::fs::write(&log, b"").unwrap();
        std::fs::write(&running, b"").unwrap();
        run(&ctx, Some(Source::Android), &mut Vec::new()).unwrap();
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(
            calls.contains("waydroid app install /nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-org.declared.new.apk"),
            "{calls}"
        );
        assert!(calls.contains("waydroid app remove org.gone"), "{calls}");
        assert!(
            !calls.contains("org.mine"),
            "the person's own app was touched: {calls}"
        );
        assert!(
            !calls.contains("nix "),
            "an Android reconcile reached Nix: {calls}"
        );
        let after = ledger(&roots);
        assert_eq!(
            after.android,
            [Kept {
                id: "android:org.declared.new".into(),
                package: "org.declared.new".into(),
                version_code: 3,
                apk: "/nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-org.declared.new.apk".into(),
            }]
        );
        assert_eq!(files(&roots.home, &[&ledger_path, &lanes]), before);
        let _ = std::fs::remove_dir_all(&root);
    }
}
