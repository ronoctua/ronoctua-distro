//! Android apps, inside Waydroid.
//!
//! # Who does what
//!
//! The IMAGE is root's: `waydroid init` refuses any other user, so preparing
//! is `systemctl start ronoctua-android-init@<GAPPS|VANILLA>.service` (a
//! oneshot the system declares, allowed to this person by a narrow polkit
//! rule), and so are erasing (`ronoctua-android-reset.service`) and reading
//! the device id Google asks for (`ronoctua-android-id.service`, which writes
//! [`Places::android_id`]). Everything else is the person's: the session runs
//! as a transient USER service, and apps go in and out with `waydroid app`.
//!
//! # What `waydroid` 1.6.3 answers (read in its source)
//!
//! * `status` prints `Session:\tRUNNING` or `Session:\tSTOPPED`.
//! * `app list` prints blocks of `Name: …`, `packageName: …`, `categories:`
//!   and tab-indented categories — and NO version, which is why this package
//!   keeps its own record ([`Recorded`]).
//! * `app install`, `app remove`, `prop get|set` need a running session, and
//!   when there is none they log `WayDroid session is stopped` and EXIT 0. So
//!   no exit code is believed: what they print is read ([`trouble_in`]), and
//!   an install or a removal is confirmed by listing the apps again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::found::Picture;
#[cfg(feature = "net")]
use crate::order::Scope;
use crate::order::{AndroidImage, Consent};
use crate::progress::{Phase, Report};
use crate::run::{self, Call};
use crate::state::{Android, Installed};
use crate::{AppKey, Context, Done, Failure, Order, Outcome, Refusal, Roots, Source};

/// The erase unit.
pub const RESET_UNIT: &str = "ronoctua-android-reset.service";
/// The unit that reads the device id for Google's uncertified-device page.
pub const ID_UNIT: &str = "ronoctua-android-id.service";
/// The transient user unit the session runs in.
pub const SESSION_UNIT: &str = "ronoctua-android-session";
/// Where Google registers an uncertified device, shown beside the id.
pub const UNCERTIFIED: &str = "https://www.google.com/android/uncertified";
/// Our record of what was installed and at which version.
pub const RECORD: &str = "android.json";
/// The most an APK may be.
pub const APK_MOST: u64 = 2 << 30;
/// The property that gives every app its own window.
pub const MULTI_WINDOWS: &str = "persist.waydroid.multi_windows";

/// The apps the image itself brings — Waydroid's own list of what it hides
/// from the launcher, and the Play Store. They are Android, not the person's
/// programs, and none of them can be removed.
pub const SYSTEM_APPS: [&str; 16] = [
    "com.android.calculator2",
    "com.android.camera2",
    "com.android.contacts",
    "com.android.deskclock",
    "com.android.documentsui",
    "com.android.email",
    "com.android.gallery3d",
    "com.android.inputmethod.latin",
    "com.android.settings",
    "com.google.android.gms",
    "org.lineageos.aperture",
    "org.lineageos.eleven",
    "org.lineageos.etar",
    "org.lineageos.jelly",
    "org.lineageos.recorder",
    "com.android.vending",
];

/// The init unit for an image: `ronoctua-android-init@GAPPS.service`.
#[must_use]
pub fn init_unit(image: AndroidImage) -> String {
    format!("ronoctua-android-init@{}.service", image.system_type())
}

/// Where the machine keeps what Android is — parameters, so a test hands a
/// scratch directory and the clock it may wait.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// Waydroid's root (`/var/lib/waydroid`): prepared = `waydroid.cfg` exists.
    pub waydroid: PathBuf,
    /// The file the id unit writes.
    pub android_id: PathBuf,
    /// How long a session may take to start.
    pub patience: Duration,
    /// How often its status is asked meanwhile.
    pub every: Duration,
}

impl Places {
    /// The machine's.
    #[must_use]
    pub fn system() -> Self {
        Self {
            waydroid: PathBuf::from("/var/lib/waydroid"),
            android_id: PathBuf::from("/run/ronoctua-android/android-id"),
            patience: Duration::from_secs(90),
            every: Duration::from_secs(2),
        }
    }
}

// ─── Argument vectors ───────────────────────────────────────────────────────

/// `systemctl start` of a system unit, never asking (polkit answers from the
/// rule; nothing in the session could show a question).
#[must_use]
pub fn start_argv(unit: &str) -> Vec<String> {
    vec!["start".into(), "--no-ask-password".into(), unit.into()]
}

/// `systemctl is-active` of a unit.
#[must_use]
pub fn is_active_argv(unit: &str) -> Vec<String> {
    vec!["is-active".into(), unit.into()]
}

/// The session as a transient user service in the apps' slice: it outlives
/// this process (the queue's job ends), its memory is the apps' and not the
/// desktop's, and `--collect` lets a failed one go without a `reset-failed`.
#[must_use]
pub fn session_start_argv(waydroid: &Path) -> Vec<String> {
    vec![
        "--user".into(),
        format!("--unit={SESSION_UNIT}"),
        "--collect".into(),
        "--slice=app.slice".into(),
        "--".into(),
        waydroid.display().to_string(),
        "session".into(),
        "start".into(),
    ]
}

/// `waydroid session stop`.
#[must_use]
pub fn session_stop_argv() -> Vec<String> {
    vec!["session".into(), "stop".into()]
}

/// `waydroid status`.
#[must_use]
pub fn status_argv() -> Vec<String> {
    vec!["status".into()]
}

/// `waydroid prop get persist.waydroid.multi_windows`.
#[must_use]
pub fn prop_get_argv() -> Vec<String> {
    vec!["prop".into(), "get".into(), MULTI_WINDOWS.into()]
}

/// `waydroid prop set persist.waydroid.multi_windows <true|false>`.
#[must_use]
pub fn prop_set_argv(multi: bool) -> Vec<String> {
    vec![
        "prop".into(),
        "set".into(),
        MULTI_WINDOWS.into(),
        if multi { "true" } else { "false" }.into(),
    ]
}

/// `waydroid app list`.
#[must_use]
pub fn app_list_argv() -> Vec<String> {
    vec!["app".into(), "list".into()]
}

/// `waydroid app install <apk>`.
#[must_use]
pub fn app_install_argv(apk: &Path) -> Vec<String> {
    vec!["app".into(), "install".into(), apk.display().to_string()]
}

/// `waydroid app remove <package>`.
#[must_use]
pub fn app_remove_argv(package: &str) -> Vec<String> {
    vec!["app".into(), "remove".into(), package.into()]
}

// ─── What waydroid prints ───────────────────────────────────────────────────

/// Whether `waydroid status` says a session runs.
#[must_use]
pub fn session_running(status: &str) -> bool {
    status.lines().any(|l| {
        l.split_once(':')
            .is_some_and(|(k, v)| k.trim() == "Session" && v.trim() == "RUNNING")
    })
}

/// Which image `waydroid.cfg` was made from (its `system_ota` names the
/// channel's JSON: `…/GAPPS.json`).
#[must_use]
pub fn image_of_config(text: &str) -> Option<AndroidImage> {
    let ota = text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "system_ota").then(|| v.trim().to_ascii_uppercase())
    })?;
    AndroidImage::ALL
        .into_iter()
        .find(|i| ota.contains(i.system_type()))
}

/// One app as `waydroid app list` names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listed {
    /// Its name (data).
    pub name: String,
    /// Its package.
    pub package: String,
    /// Its intent categories.
    pub categories: Vec<String>,
}

/// The apps out of `waydroid app list`.
#[must_use]
pub fn apps_of_list(text: &str) -> Vec<Listed> {
    let mut out: Vec<Listed> = Vec::new();
    let mut name: Option<String> = None;
    for line in text.lines() {
        if let Some(n) = line.strip_prefix("Name: ") {
            name = Some(n.trim().to_owned());
        } else if let Some(p) = line.strip_prefix("packageName: ") {
            let package = p.trim().to_owned();
            if is_package(&package) {
                out.push(Listed {
                    name: name.take().unwrap_or_else(|| package.clone()),
                    package,
                    categories: Vec::new(),
                });
            }
        } else if let Some(c) = line.strip_prefix('\t')
            && let Some(last) = out.last_mut()
        {
            last.categories.push(c.trim().to_owned());
        }
    }
    out
}

/// What went wrong, in waydroid's own words — it exits 0 on its errors.
#[must_use]
pub fn trouble_in(lines: &[String]) -> Option<Refusal> {
    let said = |needle: &str| lines.iter().any(|l| l.contains(needle));
    if said("WayDroid session is stopped") {
        Some(Refusal::SessionDown)
    } else if said("is not initialized") {
        Some(Refusal::SourceUnprepared)
    } else if said("Failed to") || said("ERROR:") || said("needs root access") {
        Some(Refusal::ToolFailed)
    } else {
        None
    }
}

/// A Java package name.
#[must_use]
pub fn is_package(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('-')
        && name
            .split('.')
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
}

// ─── Our record ─────────────────────────────────────────────────────────────

/// Where an installed app came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Came {
    /// F-Droid's catalogue: it is upgraded from there.
    Fdroid,
    /// A file the person gave.
    File,
}

/// What this package installed, since `waydroid app list` says no version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Recorded {
    /// Its name (data).
    pub name: String,
    /// The version code installed (0 when a file did not say).
    #[serde(default)]
    pub version_code: u64,
    /// The version name installed.
    #[serde(default)]
    pub version_name: String,
    /// The APK's SHA-256.
    #[serde(default)]
    pub sha256: String,
    /// Where it came from.
    pub source: Came,
}

/// The record, by package; empty when there is none or it does not parse.
#[must_use]
pub fn record(roots: &Roots) -> BTreeMap<String, Recorded> {
    std::fs::read(roots.ours_state().join(RECORD))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn keep_record(roots: &Roots, record: &BTreeMap<String, Recorded>) -> Result<(), Failure> {
    let bytes = serde_json::to_vec_pretty(record).map_err(|e| Failure::trouble(e.to_string()))?;
    ronoctua_xdg::write_atomically(&roots.ours_state(), RECORD, &bytes)
        .map_err(|e| Failure::trouble(format!("could not write {RECORD}: {e}")))
}

// ─── Reading the state ──────────────────────────────────────────────────────

fn waydroid(ctx: &Context) -> Result<&Path, Failure> {
    run::tool(ctx.tools.waydroid.as_ref())
}

/// `waydroid` with `args`, its output kept; a read.
fn ask(ctx: &Context, args: Vec<String>) -> Result<run::Ran, Failure> {
    Call::new(waydroid(ctx)?, args).keep_stdout().quietly(ctx)
}

/// How Android stands (prepared, session, image, the GAPPS device id). Runs
/// programs: call it on a thread.
#[must_use]
pub fn status(ctx: &Context) -> Android {
    status_in(ctx, &Places::system())
}

/// [`status`] with the machine's places as a parameter.
#[must_use]
pub fn status_in(ctx: &Context, places: &Places) -> Android {
    let support = ctx.tools.waydroid.is_some();
    let config = std::fs::read_to_string(places.waydroid.join("waydroid.cfg")).ok();
    let prepared = config.is_some();
    let preparing = ctx.tools.systemctl.as_ref().is_some_and(|systemctl| {
        AndroidImage::ALL.into_iter().any(|image| {
            Call::new(systemctl, is_active_argv(&init_unit(image)))
                .keep_stdout()
                .quietly(ctx)
                .is_ok_and(|r| matches!(r.stdout.trim(), "active" | "activating"))
        })
    });
    let session =
        support && prepared && ask(ctx, status_argv()).is_ok_and(|r| session_running(&r.stdout));
    let multi_window = if session {
        ask(ctx, prop_get_argv())
            .ok()
            .and_then(|r| match r.stdout.trim() {
                "true" => Some(true),
                "false" => Some(false),
                _ => None,
            })
    } else {
        None
    };
    let android_id = std::fs::read_to_string(&places.android_id)
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty() && t.len() <= 32 && t.bytes().all(|b| b.is_ascii_hexdigit()));
    Android {
        support,
        prepared,
        preparing,
        image: config.as_deref().and_then(image_of_config),
        session,
        multi_window,
        android_id,
    }
}

/// The apps inside Android (from `waydroid app list` while a session runs,
/// from our record otherwise).
#[must_use]
pub fn installed(ctx: &Context, android: &Android) -> Vec<Installed> {
    let recorded = record(&ctx.roots);
    let icons = ctx.roots.data.join("waydroid/data/icons");
    let row = |package: &str, name: &str, trouble: Option<Refusal>| {
        let mine = recorded.get(package);
        let icon = icons.join(format!("{package}.png"));
        Installed {
            key: AppKey::Android {
                package: package.to_owned(),
            },
            name: name.to_owned(),
            version: mine.map(|r| r.version_name.clone()).unwrap_or_default(),
            desktop_ids: vec![format!("waydroid.{package}.desktop")],
            size_bytes: None,
            update: mine.filter(|r| r.source == Came::Fdroid).and_then(|r| {
                let offered = crate::fdroid::app(&ctx.roots, package)?;
                (offered.version_code > r.version_code).then_some(offered.version_name)
            }),
            unfree: false,
            origin: crate::state::Origin::Mine,
            icon: icon.exists().then_some(Picture::File { path: icon }),
            trouble,
        }
    };
    if android.session
        && let Ok(ran) = ask(ctx, app_list_argv())
    {
        return apps_of_list(&ran.stdout)
            .into_iter()
            .filter(|a| !SYSTEM_APPS.contains(&a.package.as_str()))
            .map(|a| row(&a.package, &a.name, None))
            .collect();
    }
    recorded
        .iter()
        .map(|(package, r)| row(package, &r.name, Some(Refusal::SessionDown)))
        .collect()
}

/// The packages inside Android right now.
fn listed(ctx: &Context) -> Result<Vec<Listed>, Failure> {
    let ran = ask(ctx, app_list_argv())?;
    if let Some(refusal) = trouble_in(&ran.tail) {
        return Err(Failure {
            refusal: Some(refusal),
            trouble: ran.tail.join("\n"),
        });
    }
    Ok(apps_of_list(&ran.stdout))
}

// ─── Doing ──────────────────────────────────────────────────────────────────

/// `waydroid` with `args`, as a CHANGE (rehearsed in `.#dev`), its words read
/// for the failure its exit code does not tell.
fn tell(ctx: &Context, args: Vec<String>) -> Result<(), Failure> {
    let ran = Call::new(waydroid(ctx)?, args).changing().quietly(ctx)?;
    if let Some(refusal) = trouble_in(&ran.tail) {
        return Err(Failure {
            refusal: Some(refusal),
            trouble: ran.tail.join("\n"),
        });
    }
    ran.ok().map(|_| ())
}

/// Start a system unit and wait for it (a oneshot's start returns when it ends).
fn start_unit(ctx: &Context, unit: &str) -> Result<(), Failure> {
    let systemctl = run::tool(ctx.tools.systemctl.as_ref())?;
    Call::new(systemctl, start_argv(unit))
        .changing()
        .quietly(ctx)?
        .ok()
        .map(|_| ())
}

/// Whether a session runs now.
fn running(ctx: &Context) -> bool {
    ask(ctx, status_argv()).is_ok_and(|r| session_running(&r.stdout))
}

/// Start the session unless it runs, and wait until it does. Answers whether
/// it had to be started.
fn ensure_session(
    ctx: &Context,
    places: &Places,
    report: &mut dyn Report,
) -> Result<bool, Failure> {
    if !places.waydroid.join("waydroid.cfg").exists() {
        return Err(Failure::refused(Refusal::SourceUnprepared));
    }
    if running(ctx) {
        return Ok(false);
    }
    report.step(Phase::Starting, 0, 0, "android");
    let systemd_run = run::tool(ctx.tools.systemd_run.as_ref())?;
    Call::new(systemd_run, session_start_argv(waydroid(ctx)?))
        .changing()
        .quietly(ctx)?
        .ok()?;
    if ctx.rehearse {
        return Ok(true);
    }
    let started = std::time::Instant::now();
    loop {
        if running(ctx) {
            return Ok(true);
        }
        if started.elapsed() >= places.patience {
            return Err(Failure {
                refusal: Some(Refusal::SessionDown),
                trouble: format!(
                    "the session did not start in {} s",
                    places.patience.as_secs()
                ),
            });
        }
        std::thread::sleep(places.every);
    }
}

fn prepare(ctx: &Context, image: AndroidImage, places: &Places, report: &mut dyn Report) -> Done {
    report.step(Phase::Preparing, 0, 0, image.system_type());
    if !places.waydroid.join("waydroid.cfg").exists() {
        // Downloads about 1.2 GB and checks it; blocks until it is done. No
        // percentage is made up: the phase is the truth.
        start_unit(ctx, &init_unit(image))?;
        if !ctx.rehearse && !places.waydroid.join("waydroid.cfg").exists() {
            return Err(Failure::refused(Refusal::SourceUnprepared));
        }
    }
    ensure_session(ctx, places, report)?;
    tell(ctx, prop_set_argv(true))?;
    if image == AndroidImage::Gapps {
        // THE ID IS A HELP, NOT A STEP: a failure to read it leaves Android
        // working and the page without an id, which is what it would say.
        if let Err(f) = start_unit(ctx, ID_UNIT) {
            use std::io::Write as _;
            let _ = writeln!(std::io::stderr(), "the device id could not be read: {f}");
        }
    }
    Ok(Outcome::default())
}

fn install_file(
    ctx: &Context,
    path: &Path,
    consent: Consent,
    places: &Places,
    report: &mut dyn Report,
) -> Done {
    if !consent.unverified {
        return Err(Failure::refused(Refusal::UnverifiedNeedsConsent));
    }
    crate::fdroid::check_apk_file(path, APK_MOST).map_err(Failure::refused)?;
    ensure_session(ctx, places, report)?;
    let before: Vec<String> = listed(ctx)?.into_iter().map(|a| a.package).collect();
    report.step(Phase::Installing, 0, 0, &path.display().to_string());
    tell(ctx, app_install_argv(path))?;
    if ctx.rehearse {
        return Ok(Outcome {
            rehearsed: true,
            ..Outcome::default()
        });
    }
    let after = listed(ctx)?;
    // THE PACKAGE IS LEARNED BY WHAT APPEARED: an APK's name is not in its
    // file name, and reading its binary manifest is not worth a parser.
    let new: Vec<&Listed> = after
        .iter()
        .filter(|a| !before.contains(&a.package))
        .collect();
    let [app] = new.as_slice() else {
        return Ok(Outcome::default());
    };
    let mut kept = record(&ctx.roots);
    kept.insert(
        app.package.clone(),
        Recorded {
            name: app.name.clone(),
            version_code: 0,
            version_name: String::new(),
            sha256: run::sha256_of(path).unwrap_or_default(),
            source: Came::File,
        },
    );
    keep_record(&ctx.roots, &kept)?;
    Ok(Outcome {
        key: Some(AppKey::Android {
            package: app.package.clone(),
        }),
        desktop_ids: vec![format!("waydroid.{}.desktop", app.package)],
        ..Outcome::default()
    })
}

fn remove(ctx: &Context, package: &str, places: &Places, report: &mut dyn Report) -> Done {
    if !is_package(package) {
        return Err(Failure::refused(Refusal::UnknownItem));
    }
    if SYSTEM_APPS.contains(&package) {
        return Err(Failure::refused(Refusal::DeclaredInTheFlake));
    }
    ensure_session(ctx, places, report)?;
    report.step(Phase::Removing, 0, 0, package);
    tell(ctx, app_remove_argv(package))?;
    if !ctx.rehearse && listed(ctx)?.iter().any(|a| a.package == package) {
        return Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("{package} is still installed"),
        });
    }
    let mut kept = record(&ctx.roots);
    if kept.remove(package).is_some() {
        keep_record(&ctx.roots, &kept)?;
    }
    Ok(Outcome {
        key: Some(AppKey::Android {
            package: package.to_owned(),
        }),
        ..Outcome::default()
    })
}

fn erase(ctx: &Context, report: &mut dyn Report) -> Done {
    report.step(Phase::Removing, 0, 0, "android");
    if running(ctx) {
        tell(ctx, session_stop_argv())?;
    }
    start_unit(ctx, RESET_UNIT)?;
    if !ctx.rehearse {
        let _ = std::fs::remove_file(ctx.roots.ours_state().join(RECORD));
        let apks = ctx.roots.ours_cache().join("apk");
        if apks.exists() {
            let _ = crate::appimage::remove_tree(&apks);
        }
    }
    Ok(Outcome::default())
}

/// One window per app, or all of Android in one. The property lives inside
/// Android, so the session has to be running to be told; it takes effect when
/// the session starts again, which the screen says.
fn windows(ctx: &Context, multi: bool, places: &Places, report: &mut dyn Report) -> Done {
    if !places.waydroid.join("waydroid.cfg").exists() {
        return Err(Failure::refused(Refusal::SourceUnprepared));
    }
    ensure_session(ctx, places, report)?;
    tell(ctx, prop_set_argv(multi))?;
    Ok(Outcome::default())
}

fn session(ctx: &Context, on: bool, places: &Places, report: &mut dyn Report) -> Done {
    if on {
        ensure_session(ctx, places, report)?;
    } else if running(ctx) {
        report.step(Phase::Removing, 0, 0, "android");
        tell(ctx, session_stop_argv())?;
    }
    Ok(Outcome::default())
}

/// Run one order whose lane is Android: `Install` (F-Droid), `InstallFile`
/// (`.apk`), `Remove`, `Upgrade`, `Prepare`, `Refresh`, `AndroidSession`,
/// `AndroidErase`.
pub fn run(ctx: &Context, order: &Order, report: &mut dyn Report) -> Done {
    #[cfg(feature = "net")]
    {
        run_in(ctx, order, report, &crate::github::Net, &Places::system())
    }
    #[cfg(not(feature = "net"))]
    {
        match order {
            Order::Install { .. } | Order::Upgrade { .. } | Order::Refresh { .. } => {
                Err(Failure::refused(Refusal::Offline))
            }
            other => local(ctx, other, report, &Places::system()),
        }
    }
}

/// The orders that never reach the network.
fn local(ctx: &Context, order: &Order, report: &mut dyn Report, places: &Places) -> Done {
    if ctx.tools.waydroid.is_none() {
        return Err(Failure::refused(Refusal::SourceAbsent));
    }
    match order {
        Order::Prepare {
            source: Source::Android,
            image,
        } => prepare(ctx, image.unwrap_or_default(), places, report),
        Order::InstallFile { path, consent } => install_file(ctx, path, *consent, places, report),
        Order::Remove {
            key: AppKey::Android { package },
            ..
        } => remove(ctx, package, places, report),
        Order::AndroidSession { on } => session(ctx, *on, places, report),
        Order::AndroidErase => erase(ctx, report),
        Order::AndroidWindows { multi } => windows(ctx, *multi, places, report),
        _ => Err(Failure::refused(Refusal::UnknownItem)),
    }
}

/// [`run`] over any network and any places (a test's).
#[cfg(feature = "net")]
pub fn run_in(
    ctx: &Context,
    order: &Order,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
    places: &Places,
) -> Done {
    match order {
        Order::Refresh {
            source: Source::Android,
        } => {
            let count = crate::fdroid::refresh_with(net, &ctx.roots, report)?;
            Ok(Outcome {
                amount: Some(count as u64),
                ..Outcome::default()
            })
        }
        Order::Install {
            key: AppKey::Android { package },
            consent,
        } => {
            if ctx.tools.waydroid.is_none() {
                return Err(Failure::refused(Refusal::SourceAbsent));
            }
            let app = crate::fdroid::app(&ctx.roots, package).ok_or_else(|| {
                Failure::refused(if crate::fdroid::catalogue(&ctx.roots).is_some() {
                    Refusal::UnknownItem
                } else {
                    Refusal::SourceUnprepared
                })
            })?;
            install_app(ctx, &app, *consent, places, report, net)
        }
        Order::Upgrade { scope } => {
            if ctx.tools.waydroid.is_none() {
                return Err(Failure::refused(Refusal::SourceAbsent));
            }
            upgrade(ctx, scope, places, report, net)
        }
        other => local(ctx, other, report, places),
    }
}

/// Download, verify, install and record one F-Droid app. The consent
/// questions are answered before a byte is fetched.
#[cfg(feature = "net")]
fn install_app(
    ctx: &Context,
    app: &crate::fdroid::App,
    consent: Consent,
    places: &Places,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
) -> Done {
    if app.arm_only() && !consent.arm_only {
        return Err(Failure::refused(Refusal::ArmOnly));
    }
    let ask_above = ctx.prefs.ask_above_mb.saturating_mul(1 << 20);
    if ask_above > 0 && app.size > ask_above && !consent.big {
        return Err(Failure::refused(Refusal::BigNeedsConsent));
    }
    ensure_session(ctx, places, report)?;
    let name = app.name_in(crate::fdroid::DEFAULT_LOCALE);
    let dir = ctx.roots.ours_cache().join("apk");
    std::fs::create_dir_all(&dir)
        .map_err(|e| Failure::trouble(format!("could not create {}: {e}", dir.display())))?;
    let apk = dir.join(format!("{}.apk", app.sha256));
    let part = dir.join(format!("{}.apk.{}.part", app.sha256, std::process::id()));
    // NEITHER FILE STAYS: the APK is only the way in, and the next install
    // downloads the next version anyway.
    struct Gone(Vec<PathBuf>);
    impl Drop for Gone {
        fn drop(&mut self) {
            for p in &self.0 {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    let _gone = Gone(vec![part.clone(), apk.clone()]);
    let most = if app.size > 0 { app.size } else { APK_MOST };
    let mut last = 0u64;
    net.download(&app.apk_url(), most, &part, &mut |done, total| {
        if done >= last + (1 << 20) || Some(done) == total {
            last = done;
            report.step(Phase::Downloading, done, total.unwrap_or(app.size), &name);
        }
        true
    })?;
    report.step(Phase::Verifying, 0, 0, &name);
    let got = run::sha256_of(&part).map_err(|e| Failure::trouble(e.to_string()))?;
    if got != app.sha256 {
        return Err(Failure {
            refusal: Some(Refusal::Integrity),
            trouble: format!(
                "{}: sha256 {got}, the signed index says {}",
                app.apk, app.sha256
            ),
        });
    }
    std::fs::rename(&part, &apk).map_err(|e| Failure::trouble(e.to_string()))?;
    crate::fdroid::check_apk_file(&apk, APK_MOST).map_err(Failure::refused)?;
    report.step(Phase::Installing, 0, 0, &name);
    tell(ctx, app_install_argv(&apk))?;
    if !ctx.rehearse && !listed(ctx)?.iter().any(|a| a.package == app.package) {
        return Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: format!("{} did not appear", app.package),
        });
    }
    let mut kept = record(&ctx.roots);
    kept.insert(
        app.package.clone(),
        Recorded {
            name,
            version_code: app.version_code,
            version_name: app.version_name.clone(),
            sha256: app.sha256.clone(),
            source: Came::Fdroid,
        },
    );
    keep_record(&ctx.roots, &kept)?;
    Ok(Outcome {
        key: Some(AppKey::Android {
            package: app.package.clone(),
        }),
        desktop_ids: vec![format!("waydroid.{}.desktop", app.package)],
        ..Outcome::default()
    })
}

#[cfg(feature = "net")]
fn upgrade(
    ctx: &Context,
    scope: &Scope,
    places: &Places,
    report: &mut dyn Report,
    net: &dyn crate::github::Fetch,
) -> Done {
    let kept = record(&ctx.roots);
    let wanted: Vec<&String> = match scope {
        Scope::One {
            key: AppKey::Android { package },
        } => {
            if !kept.contains_key(package) {
                return Err(Failure::refused(Refusal::UnknownItem));
            }
            kept.keys().filter(|k| *k == package).collect()
        }
        Scope::Source {
            source: Source::Android,
        }
        | Scope::Everything => kept.keys().collect(),
        _ => return Err(Failure::refused(Refusal::UnknownItem)),
    };
    let due: Vec<crate::fdroid::App> = wanted
        .into_iter()
        .filter(|p| kept[*p].source == Came::Fdroid)
        .filter_map(|p| {
            crate::fdroid::app(&ctx.roots, p).filter(|a| a.version_code > kept[p].version_code)
        })
        .collect();
    if due.is_empty() {
        return Ok(Outcome {
            amount: Some(0),
            ..Outcome::default()
        });
    }
    // An update run that had to start Android stops it again: nobody asked
    // for it to keep running, and it costs memory for as long as it does.
    let started = ensure_session(ctx, places, report)?;
    let (mut moved, mut troubles) = (0u64, Vec::new());
    for app in &due {
        // Installed once, so the person already answered its questions.
        let consent = Consent {
            arm_only: true,
            big: true,
            ..Consent::default()
        };
        match install_app(ctx, app, consent, places, report, net) {
            Ok(_) => moved += 1,
            Err(f) => troubles.push(format!("{}: {f}", app.package)),
        }
    }
    if started {
        let _ = tell(ctx, session_stop_argv());
    }
    if troubles.is_empty() {
        Ok(Outcome {
            amount: Some(moved),
            ..Outcome::default()
        })
    } else {
        Err(Failure {
            refusal: Some(Refusal::ToolFailed),
            trouble: troubles.join("\n"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::Beat;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-android-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        crate::write_executable(&path, &format!("#!/bin/sh\n{body}"));
        path
    }

    /// A fake Waydroid kept in files: `state` holds RUNNING or STOPPED, `apps`
    /// the list; `app install` adds what `new-package` names, `app remove`
    /// takes a package out, and every call is logged.
    struct Fake {
        dir: PathBuf,
        ctx: Context,
        places: Places,
    }

    impl Fake {
        fn new(tag: &str, prepared: bool, running: bool) -> Self {
            let dir = scratch(tag);
            let log = dir.join("calls.log");
            let state = dir.join("state");
            let apps = dir.join("apps");
            std::fs::write(&state, if running { "RUNNING" } else { "STOPPED" }).unwrap();
            std::fs::write(&apps, "Name: Settings\npackageName: com.android.settings\ncategories:\n\tandroid.intent.category.LAUNCHER\n").unwrap();
            let waydroid = script(
                &dir,
                "fake-waydroid",
                &format!(
                    "echo \"waydroid $*\" >> '{log}'\ncase \"$1 $2\" in\n\
                     'status ') printf 'Session:\\t%s\\nVendor type:\\tMAINLINE\\n' \"$(cat '{state}')\";;\n\
                     'session start') echo RUNNING > '{state}';;\n\
                     'session stop') echo STOPPED > '{state}';;\n\
                     'prop get') echo true;;\n\
                     'prop set') ;;\n\
                     'app list') if [ \"$(cat '{state}')\" = RUNNING ]; then cat '{apps}'; else echo '[00:00:00] WayDroid session is stopped'; fi;;\n\
                     'app install') if [ \"$(cat '{state}')\" = RUNNING ]; then p=$(cat '{dir}/new-package'); printf 'Name: New\\npackageName: %s\\ncategories:\\n' \"$p\" >> '{apps}'; else echo '[00:00:00] WayDroid session is stopped'; fi;;\n\
                     'app remove') grep -v \"packageName: $3\" '{apps}' > '{apps}.new'; mv '{apps}.new' '{apps}';;\n\
                     esac\n",
                    log = log.display(),
                    state = state.display(),
                    apps = apps.display(),
                    dir = dir.display(),
                ),
            );
            let root = dir.join("var-lib-waydroid");
            std::fs::create_dir_all(&root).unwrap();
            if prepared {
                std::fs::write(root.join("waydroid.cfg"), "[waydroid]\nsystem_ota = https://ota.waydro.id/system/lineage/waydroid_x86_64/GAPPS.json\n").unwrap();
            }
            let systemctl = script(
                &dir,
                "fake-systemctl",
                &format!(
                    "echo \"systemctl $*\" >> '{log}'\ncase \"$1\" in\n\
                     is-active) echo inactive; exit 3;;\n\
                     start) case \"$3\" in ronoctua-android-init@*) printf '[waydroid]\\nsystem_ota = x/VANILLA.json\\n' > '{root}/waydroid.cfg';; esac;;\n\
                     esac\n",
                    log = log.display(),
                    root = root.display(),
                ),
            );
            // The transient unit runs its command right away, like systemd would.
            let systemd_run = script(
                &dir,
                "fake-systemd-run",
                &format!(
                    "echo \"systemd-run $*\" >> '{}'\nwhile [ \"$1\" != -- ]; do shift; done; shift; \"$@\"\n",
                    log.display()
                ),
            );
            let ctx = Context {
                tools: crate::Tools {
                    waydroid: Some(waydroid),
                    systemctl: Some(systemctl),
                    systemd_run: Some(systemd_run),
                    ..crate::Tools::default()
                },
                roots: Roots::under(&dir),
                prefs: crate::prefs::Preferences::default(),
                rehearse: false,
            };
            let places = Places {
                waydroid: root,
                android_id: dir.join("android-id"),
                patience: Duration::from_millis(300),
                every: Duration::from_millis(20),
            };
            Self { dir, ctx, places }
        }

        fn calls(&self) -> String {
            std::fs::read_to_string(self.dir.join("calls.log")).unwrap_or_default()
        }

        fn run(&self, order: &Order) -> Done {
            #[cfg(feature = "net")]
            {
                run_in(
                    &self.ctx,
                    order,
                    &mut Vec::<Beat>::new(),
                    &crate::github::canned::Canned::default(),
                    &self.places,
                )
            }
            #[cfg(not(feature = "net"))]
            {
                local(&self.ctx, order, &mut Vec::<Beat>::new(), &self.places)
            }
        }
    }

    impl Drop for Fake {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn every_argv_is_the_shape_the_units_and_waydroid_expect() {
        assert_eq!(
            init_unit(AndroidImage::Gapps),
            "ronoctua-android-init@GAPPS.service"
        );
        assert_eq!(
            init_unit(AndroidImage::Vanilla),
            "ronoctua-android-init@VANILLA.service"
        );
        assert_eq!(
            start_argv(RESET_UNIT),
            [
                "start",
                "--no-ask-password",
                "ronoctua-android-reset.service"
            ]
        );
        assert_eq!(
            session_start_argv(Path::new("/w/bin/waydroid")),
            [
                "--user",
                "--unit=ronoctua-android-session",
                "--collect",
                "--slice=app.slice",
                "--",
                "/w/bin/waydroid",
                "session",
                "start"
            ]
        );
        assert_eq!(
            prop_set_argv(true),
            ["prop", "set", "persist.waydroid.multi_windows", "true"]
        );
        assert_eq!(
            app_install_argv(Path::new("/c/a.apk")),
            ["app", "install", "/c/a.apk"]
        );
        assert_eq!(app_remove_argv("org.x"), ["app", "remove", "org.x"]);
    }

    #[test]
    fn waydroids_words_are_read_and_not_its_exit_code() {
        assert!(session_running("Session:\tRUNNING\nContainer:\tRUNNING\n"));
        assert!(!session_running(
            "Session:\tSTOPPED\nVendor type:\tMAINLINE\n"
        ));
        assert!(!session_running(
            "Waydroid is not initialized, run \"waydroid init\"\n"
        ));
        let list = apps_of_list(
            "Name: F-Droid\npackageName: org.fdroid.fdroid\ncategories:\n\tandroid.intent.category.LAUNCHER\n\
             [12:00:01] noise\nName: Bad\npackageName: -rf\ncategories:\n",
        );
        assert_eq!(
            list.len(),
            1,
            "a package name that is not one is not listed"
        );
        assert_eq!(list[0].name, "F-Droid");
        assert_eq!(list[0].categories, ["android.intent.category.LAUNCHER"]);
        assert_eq!(
            trouble_in(&["[10:00:00] WayDroid session is stopped".to_owned()]),
            Some(Refusal::SessionDown)
        );
        assert_eq!(
            trouble_in(&["[10:00:00] Failed to uninstall package: x".to_owned()]),
            Some(Refusal::ToolFailed)
        );
        assert_eq!(trouble_in(&["all good".to_owned()]), None);
        assert_eq!(
            image_of_config(
                "[waydroid]\nsystem_ota = https://ota.waydro.id/system/lineage/waydroid_x86_64/GAPPS.json\n"
            ),
            Some(AndroidImage::Gapps)
        );
        assert_eq!(image_of_config("system_ota = None\n"), None);
    }

    #[test]
    fn the_status_is_read_from_the_files_and_waydroid() {
        let fake = Fake::new("status", true, true);
        std::fs::write(&fake.places.android_id, "3f2a9c01b4\n").unwrap();
        let a = status_in(&fake.ctx, &fake.places);
        assert!(a.support && a.prepared && a.session && !a.preparing);
        assert_eq!(a.image, Some(AndroidImage::Gapps));
        assert_eq!(a.multi_window, Some(true));
        assert_eq!(a.android_id.as_deref(), Some("3f2a9c01b4"));
        let down = Fake::new("status-down", false, false);
        let b = status_in(&down.ctx, &down.places);
        assert!(b.support && !b.prepared && !b.session);
        assert!(
            !down.calls().contains("waydroid status"),
            "nothing to ask before it is prepared"
        );
    }

    #[test]
    fn preparing_runs_the_units_starts_the_session_and_asks_for_windows() {
        let fake = Fake::new("prepare", false, false);
        fake.run(&Order::Prepare {
            source: Source::Android,
            image: Some(AndroidImage::Gapps),
        })
        .unwrap();
        let calls = fake.calls();
        let at = |needle: &str| {
            calls
                .find(needle)
                .unwrap_or_else(|| panic!("{needle} missing from:\n{calls}"))
        };
        assert!(
            at("systemctl start --no-ask-password ronoctua-android-init@GAPPS.service")
                < at("systemd-run --user")
        );
        assert!(at("systemd-run --user") < at("prop set persist.waydroid.multi_windows true"));
        assert!(
            at("prop set") < at("systemctl start --no-ask-password ronoctua-android-id.service")
        );
        assert!(status_in(&fake.ctx, &fake.places).session);
    }

    #[test]
    fn a_session_that_never_starts_is_a_refusal_in_time() {
        let fake = Fake::new("never", true, false);
        // A systemd-run that starts nothing.
        let dead = script(&fake.dir, "fake-dead-run", "exit 0\n");
        let ctx = Context {
            tools: crate::Tools {
                systemd_run: Some(dead),
                ..fake.ctx.tools.clone()
            },
            ..fake.ctx.clone()
        };
        let f = local(
            &ctx,
            &Order::AndroidSession { on: true },
            &mut Vec::<Beat>::new(),
            &fake.places,
        )
        .unwrap_err();
        assert_eq!(f.refusal, Some(Refusal::SessionDown));
    }

    #[test]
    fn an_apk_file_is_installed_and_its_package_learned_by_what_appeared() {
        let fake = Fake::new("apk", true, false);
        std::fs::write(fake.dir.join("new-package"), "org.example.notes").unwrap();
        let apk = fake.dir.join("notes.apk");
        std::fs::write(
            &apk,
            zip_of(&[("AndroidManifest.xml", b"x"), ("classes.dex", b"dex")]),
        )
        .unwrap();
        let order = Order::InstallFile {
            path: apk.clone(),
            consent: Consent::default(),
        };
        assert_eq!(
            fake.run(&order).unwrap_err().refusal,
            Some(Refusal::UnverifiedNeedsConsent)
        );
        let order = Order::InstallFile {
            path: apk,
            consent: Consent {
                unverified: true,
                ..Consent::default()
            },
        };
        let done = fake.run(&order).unwrap();
        assert_eq!(
            done.key,
            Some(AppKey::Android {
                package: "org.example.notes".into()
            })
        );
        assert!(
            fake.calls().contains("session start"),
            "the session was started first"
        );
        let kept = record(&fake.ctx.roots);
        assert_eq!(kept["org.example.notes"].source, Came::File);
        let rows = installed(&fake.ctx, &status_in(&fake.ctx, &fake.places));
        assert_eq!(
            rows.len(),
            1,
            "the image's own Settings is not the person's"
        );
        assert_eq!(
            rows[0].key,
            AppKey::Android {
                package: "org.example.notes".into()
            }
        );
        let not = fake.dir.join("not.apk");
        std::fs::write(&not, zip_of(&[("readme.txt", b"x")])).unwrap();
        let order = Order::InstallFile {
            path: not,
            consent: Consent {
                unverified: true,
                ..Consent::default()
            },
        };
        assert_eq!(
            fake.run(&order).unwrap_err().refusal,
            Some(Refusal::NotAnApk)
        );
    }

    #[test]
    fn removing_is_confirmed_by_the_list_and_forgets_the_record() {
        let fake = Fake::new("remove", true, true);
        std::fs::write(fake.dir.join("new-package"), "org.example.notes").unwrap();
        let apk = fake.dir.join("notes.apk");
        std::fs::write(
            &apk,
            zip_of(&[("AndroidManifest.xml", b"x"), ("classes.dex", b"dex")]),
        )
        .unwrap();
        fake.run(&Order::InstallFile {
            path: apk,
            consent: Consent {
                unverified: true,
                ..Consent::default()
            },
        })
        .unwrap();
        fake.run(&Order::Remove {
            key: AppKey::Android {
                package: "org.example.notes".into(),
            },
            keep_data: true,
        })
        .unwrap();
        assert!(record(&fake.ctx.roots).is_empty());
        let system = Order::Remove {
            key: AppKey::Android {
                package: "com.android.settings".into(),
            },
            keep_data: true,
        };
        assert_eq!(
            fake.run(&system).unwrap_err().refusal,
            Some(Refusal::DeclaredInTheFlake)
        );
    }

    #[test]
    fn a_stopped_session_shows_the_record_with_its_trouble() {
        let fake = Fake::new("down", true, false);
        let mut kept = BTreeMap::new();
        kept.insert(
            "org.fdroid.fdroid".to_owned(),
            Recorded {
                name: "F-Droid".into(),
                version_code: 1,
                version_name: "1.0".into(),
                sha256: String::new(),
                source: Came::Fdroid,
            },
        );
        keep_record(&fake.ctx.roots, &kept).unwrap();
        let rows = installed(&fake.ctx, &status_in(&fake.ctx, &fake.places));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].trouble, Some(Refusal::SessionDown));
        assert_eq!(rows[0].version, "1.0");
    }

    #[test]
    fn erasing_stops_asks_root_and_forgets() {
        let fake = Fake::new("erase", true, true);
        keep_record(&fake.ctx.roots, &BTreeMap::new()).unwrap();
        fake.run(&Order::AndroidErase).unwrap();
        let calls = fake.calls();
        assert!(
            calls.find("session stop").unwrap()
                < calls
                    .find("start --no-ask-password ronoctua-android-reset.service")
                    .unwrap()
        );
        assert!(!fake.ctx.roots.ours_state().join(RECORD).exists());
    }

    #[test]
    fn windows_are_told_to_a_running_android_and_refused_before_one_exists() {
        let fake = Fake::new("windows", true, true);
        fake.run(&Order::AndroidWindows { multi: false }).unwrap();
        assert!(
            fake.calls()
                .contains("prop set persist.waydroid.multi_windows false"),
            "{}",
            fake.calls()
        );
        let bare = Fake::new("windows-bare", false, false);
        assert_eq!(
            bare.run(&Order::AndroidWindows { multi: true })
                .unwrap_err()
                .refusal,
            Some(Refusal::SourceUnprepared)
        );
        assert!(!bare.calls().contains("prop set"));
    }

    #[test]
    fn without_waydroid_every_order_says_so() {
        let mut fake = Fake::new("absent", true, true);
        fake.ctx.tools.waydroid = None;
        assert_eq!(
            fake.run(&Order::AndroidSession { on: true })
                .unwrap_err()
                .refusal,
            Some(Refusal::SourceAbsent)
        );
    }

    #[cfg(feature = "net")]
    mod from_fdroid {
        use super::*;
        use crate::github::canned::Canned;

        fn catalogue_with(fake: &Fake, apk: &[u8], nativecode: &[&str]) -> crate::fdroid::App {
            let app = crate::fdroid::App {
                package: "org.example.notes".into(),
                name: [("en-US".to_owned(), "Notes".to_owned())]
                    .into_iter()
                    .collect(),
                summary: BTreeMap::new(),
                icon: None,
                version_code: 7,
                version_name: "1.7".into(),
                apk: "/org.example.notes_7.apk".into(),
                sha256: run::sha256(apk),
                size: apk.len() as u64,
                nativecode: nativecode.iter().map(|s| (*s).to_owned()).collect(),
                min_sdk: Some(21),
                anti_features: Vec::new(),
                license: None,
                web_site: None,
            };
            let c = crate::fdroid::Catalogue {
                format: crate::fdroid::FORMAT,
                timestamp: 1,
                index_sha256: "c".repeat(64),
                apps: vec![app.clone()],
            };
            crate::fdroid::keep(&fake.ctx.roots, &c).unwrap();
            app
        }

        #[test]
        fn an_fdroid_app_is_verified_installed_recorded_and_its_apk_dropped() {
            let fake = Fake::new("fdroid", true, true);
            std::fs::write(fake.dir.join("new-package"), "org.example.notes").unwrap();
            let apk = zip_of(&[("AndroidManifest.xml", b"x"), ("classes.dex", b"dex")]);
            let app = catalogue_with(&fake, &apk, &[]);
            let net = Canned::default().serving(&app.apk_url(), apk);
            let order = Order::Install {
                key: AppKey::Android {
                    package: app.package.clone(),
                },
                consent: Consent::default(),
            };
            run_in(
                &fake.ctx,
                &order,
                &mut Vec::<Beat>::new(),
                &net,
                &fake.places,
            )
            .unwrap();
            let kept = record(&fake.ctx.roots);
            assert_eq!(
                (kept[&app.package].version_code, kept[&app.package].source),
                (7, Came::Fdroid)
            );
            assert_eq!(
                std::fs::read_dir(fake.ctx.roots.ours_cache().join("apk"))
                    .unwrap()
                    .count(),
                0,
                "the APK does not stay"
            );
        }

        #[test]
        fn a_tampered_apk_and_an_arm_only_one_are_refused() {
            let fake = Fake::new("fdroid-bad", true, true);
            let apk = zip_of(&[("AndroidManifest.xml", b"x"), ("classes.dex", b"dex")]);
            let app = catalogue_with(&fake, &apk, &[]);
            // The same size as the signed one: only its bytes differ.
            let net = Canned::default().serving(
                &app.apk_url(),
                zip_of(&[("AndroidManifest.xml", b"y"), ("classes.dex", b"evl")]),
            );
            let order = Order::Install {
                key: AppKey::Android {
                    package: app.package,
                },
                consent: Consent::default(),
            };
            let f = run_in(
                &fake.ctx,
                &order,
                &mut Vec::<Beat>::new(),
                &net,
                &fake.places,
            )
            .unwrap_err();
            assert_eq!(f.refusal, Some(Refusal::Integrity));
            assert!(!fake.calls().contains("app install"));
            catalogue_with(&fake, &apk, &["arm64-v8a"]);
            let f = run_in(
                &fake.ctx,
                &order,
                &mut Vec::<Beat>::new(),
                &Canned::default(),
                &fake.places,
            )
            .unwrap_err();
            assert_eq!(f.refusal, Some(Refusal::ArmOnly));
        }

        #[test]
        fn an_upgrade_reinstalls_what_the_catalogue_has_newer_and_stops_what_it_started() {
            let fake = Fake::new("fdroid-up", true, false);
            std::fs::write(fake.dir.join("new-package"), "org.example.notes").unwrap();
            let apk = zip_of(&[("AndroidManifest.xml", b"x"), ("classes.dex", b"dex")]);
            let app = catalogue_with(&fake, &apk, &[]);
            let mut kept = BTreeMap::new();
            kept.insert(
                app.package.clone(),
                Recorded {
                    name: "Notes".into(),
                    version_code: 3,
                    version_name: "1.3".into(),
                    sha256: String::new(),
                    source: Came::Fdroid,
                },
            );
            keep_record(&fake.ctx.roots, &kept).unwrap();
            let net = Canned::default().serving(&app.apk_url(), apk);
            let order = Order::Upgrade {
                scope: Scope::Source {
                    source: Source::Android,
                },
            };
            assert_eq!(
                run_in(
                    &fake.ctx,
                    &order,
                    &mut Vec::<Beat>::new(),
                    &net,
                    &fake.places
                )
                .unwrap()
                .amount,
                Some(1)
            );
            assert_eq!(record(&fake.ctx.roots)[&app.package].version_code, 7);
            assert!(
                fake.calls().trim_end().ends_with("waydroid session stop"),
                "{}",
                fake.calls()
            );
            assert_eq!(
                run_in(
                    &fake.ctx,
                    &order,
                    &mut Vec::<Beat>::new(),
                    &net,
                    &fake.places
                )
                .unwrap()
                .amount,
                Some(0)
            );
        }
    }

    /// A stored zip of `members`, enough for `ronoctua_zip::members`.
    pub(super) fn zip_of(members: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in members {
            let at = u32::try_from(out.len()).unwrap();
            let crc = 0u32;
            let len = u32::try_from(data.len()).unwrap();
            let n = u16::try_from(name.len()).unwrap();
            out.extend(b"PK\x03\x04");
            out.extend([20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            out.extend(crc.to_le_bytes());
            out.extend(len.to_le_bytes());
            out.extend(len.to_le_bytes());
            out.extend(n.to_le_bytes());
            out.extend(0u16.to_le_bytes());
            out.extend(name.as_bytes());
            out.extend(*data);
            central.extend(b"PK\x01\x02");
            central.extend([20, 0, 20, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            central.extend(crc.to_le_bytes());
            central.extend(len.to_le_bytes());
            central.extend(len.to_le_bytes());
            central.extend(n.to_le_bytes());
            central.extend([0u8; 12]);
            central.extend(at.to_le_bytes());
            central.extend(name.as_bytes());
        }
        let at = u32::try_from(out.len()).unwrap();
        let size = u32::try_from(central.len()).unwrap();
        let count = u16::try_from(members.len()).unwrap();
        out.extend(&central);
        out.extend(b"PK\x05\x06");
        out.extend([0u8; 4]);
        out.extend(count.to_le_bytes());
        out.extend(count.to_le_bytes());
        out.extend(size.to_le_bytes());
        out.extend(at.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out
    }
}
