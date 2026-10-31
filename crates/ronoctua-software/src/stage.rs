//! Handing the person's declaration to the machine, and to their repository.
//!
//! # Three steps, three owners
//!
//! 1. **Stage** ([`stage`]). The person lays their mirror — `person/` and
//!    `brain-modules/`, the repository's `plain/` shape — in root's staging
//!    folder (`/run/ronoctua-person/staged`, theirs, `0700`), with what they
//!    signed of it (`signed.json`, `signed.sig`: root holds the staging to
//!    the key it pinned) and a [`ronoctua_person::tree::NONCE`] written LAST
//!    and the old one removed FIRST: the contract of `ronoctua-person apply`
//!    (`docs/ARCHITECTURE.md`, L06, L09). Before it lays anything, [`apply`]
//!    asks the belongings to sign the mirror as it is now (`sign`, no
//!    network, never a question): with the identity the session keeps, the
//!    change just made arrives signed; without it, root keeps its copy of
//!    what is not signed yet, and the change waits for the person's
//!    passphrase.
//! 2. **Apply** ([`apply`]). `systemctl start ronoctua-person-apply.service`
//!    (one narrow polkit rule says yes, nothing asks), and the verdict read
//!    back: a verdict whose nonce is not ours answers another staging (or an
//!    earlier run, when the start ran none), so the unit is started again;
//!    `busy` means the update is building — an answer, not a failure: the
//!    mirror keeps the change, and the machine takes it at the apply after
//!    that update's, which staged the mirror before it took the machine.
//!    The update's `person-stage` hands the mirror over the same way
//!    ([`hand_over`], `person stage`), without signing: its `person-sign`
//!    came first. [`rebuild`] then starts `ronoctua-update-apply.service`
//!    when the change needs the system rebuilt (a program removed, a
//!    decline): exit 75 of its `flock` is the update holding the machine,
//!    and the change waits for the same next apply.
//! 3. **Publish** ([`publish`]). `ronoctua-belongings plain push`, which
//!    carries the declaration to the person's repository. Not having one, or
//!    the network being down, is not a failure of the declaration: the next
//!    sync carries it.
//!
//! Every argument vector is a pure function; the programs come from
//! [`crate::Tools`] and the places from [`crate::System`], so a test hands
//! both.

use std::io::Write as _;
use std::path::Path;

use ronoctua_person::names;
use ronoctua_person::tree::{self, Outcome as TreeOutcome, TreeWhy, Verdict};

use crate::progress::{Phase, Report};
use crate::run::{Call, tool};
use crate::{Context, Failure, Refusal, Roots};

/// How many times the applier is started for one staging before giving up:
/// each start that answered somebody else's staging is one more try.
pub const TRIES: usize = 3;

/// The exit code of the update's `flock` when the update holds the machine
/// (`ronoctua.machine.applyCommand`, `--conflict-exit-code 75`).
pub const UPDATE_BUSY: i32 = 75;

/// `systemctl start` of a system unit, never asking.
#[must_use]
pub fn start_argv(unit: &str) -> Vec<String> {
    crate::android::start_argv(unit)
}

/// `systemctl show` of a unit's main process's exit status.
#[must_use]
pub fn exit_status_argv(unit: &str) -> Vec<String> {
    vec![
        "show".into(),
        "--property=ExecMainStatus".into(),
        "--value".into(),
        unit.into(),
    ]
}

/// `ronoctua-belongings plain push --json`.
#[must_use]
pub fn push_argv() -> Vec<String> {
    vec!["plain".into(), "push".into(), "--json".into()]
}

/// `ronoctua-belongings sign --json`: the mirror signed here, no network.
#[must_use]
pub fn sign_argv() -> Vec<String> {
    vec!["sign".into(), "--json".into()]
}

/// A nonce for one staging: 24 letters and digits, from the kernel's
/// randomness (`tree::nonce_of`'s grammar).
#[must_use]
pub fn nonce() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut bytes = [0u8; 24];
    if aws_lc_rs::rand::fill(&mut bytes).is_err() {
        // THE CLOCK AND THE PID, when the randomness is not there: a nonce
        // only has to differ from the previous staging's.
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
            ^ u128::from(std::process::id());
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = u8::try_from((seed >> ((i % 16) * 8)) & 0xff).unwrap_or(0);
        }
    }
    bytes
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(*b) % ALPHABET.len()]))
        .collect()
}

/// Remove what a folder holds, keeping the folder (root's tmpfiles made it
/// the person's, `0700`, and a new one would be the person's default mode).
fn empty(folder: &Path) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(folder) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(folder)?;
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    for e in entries {
        let e = e?;
        let path = e.path();
        if e.file_type()?.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

/// A file only its owner reads, written whole.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)
}

/// Copy a tree of regular files and folders; links, sockets, pipes and
/// devices are left out (the applier refuses them, and a link would lead it
/// somewhere else).
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(from) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for e in entries {
        let e = e?;
        let kind = e.file_type()?;
        let target = to.join(e.file_name());
        if kind.is_dir() {
            std::fs::create_dir_all(&target)?;
            copy_tree(&e.path(), &target)?;
        } else if kind.is_file() {
            write_private(&target, &std::fs::read(e.path())?)?;
        }
    }
    Ok(())
}

/// What the mirror holds that a staging carries, checked as root would read
/// it — the whole of `person stage --check`.
///
/// # Errors
///
/// [`Refusal::DeclarationUnreadable`] for a document that does not read.
pub fn check(roots: &Roots) -> Result<(usize, usize), Failure> {
    let declared = crate::declaration::read_declared(roots).map_err(|why| Failure {
        refusal: Some(Refusal::DeclarationUnreadable),
        trouble: format!("{}: {}", names::DECLARED, why.id()),
    })?;
    let declined = crate::declaration::read_declined(roots).map_err(|why| Failure {
        refusal: Some(Refusal::DeclarationUnreadable),
        trouble: format!("{}: {}", names::DECLINED, why.id()),
    })?;
    // THE DISTRO PINS THEY FOLLOW never hold the declarations back: one that
    // does not read here is staged as it is, and root keeps it back with its
    // reason while everything else arrives.
    if let Err(e) = crate::follow::read(roots) {
        tracing::warn!(error = %e, "the followed distro pins do not read; root keeps them back");
    }
    Ok((declared.apps.len(), declined.apps.len()))
}

/// Lay the person's mirror in `staged`, in the shape root's applier reads:
/// the old nonce out first, the tree, the icons the entries name, the brain
/// modules, and a new nonce last. Answers the nonce.
///
/// A mirror whose documents do not read is refused before anything is laid:
/// a staging root would refuse is a staging that takes the person's
/// declarations away from nobody, but that says nothing true either.
///
/// # Errors
///
/// [`Refusal::DeclarationUnreadable`], or a file that could not be written.
pub fn stage(roots: &Roots, staged: &Path) -> Result<String, Failure> {
    check(roots)?;
    let wrote = |e: std::io::Error| {
        Failure::trouble(format!("could not stage into {}: {e}", staged.display()))
    };
    // THE OLD NONCE FIRST: a staging half laid must never carry it.
    match std::fs::remove_file(staged.join(tree::NONCE)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(wrote(e)),
    }
    empty(staged).map_err(wrote)?;
    let mirror = crate::declaration::mirror(roots);
    let person = staged.join(tree::PERSON);
    for rel in [
        names::DECLARED,
        names::DECLINED,
        names::OPTIONS,
        names::DISTRO_PINS,
    ] {
        match std::fs::read(mirror.join(rel)) {
            Ok(bytes) => write_private(&person.join(rel), &bytes).map_err(wrote)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(wrote(e)),
        }
    }
    // A STAGING WITH NO DECLARATION AT ALL is still a person's: an empty
    // folder is what every boot leaves, and the applier refuses it as
    // nothing staged.
    std::fs::create_dir_all(person.join("software")).map_err(wrote)?;
    // ONLY THE ICONS AN ENTRY NAMES: the applier reads those, by name, and
    // nothing else of the folder.
    if let Ok(declared) = crate::declaration::read_declared(roots) {
        for entry in &declared.apps {
            let icon = match entry {
                ronoctua_person::declared::Entry::AppImage(a) => a.icon.as_ref(),
                ronoctua_person::declared::Entry::Web(a) => a.icon.as_ref(),
                _ => None,
            };
            let Some(sha) = icon else { continue };
            let from = crate::declaration::icon_path(roots, sha);
            let to = person.join(names::ICONS).join(format!("{sha}.png"));
            if to.exists() {
                continue;
            }
            match std::fs::read(&from) {
                Ok(bytes) => write_private(&to, &bytes).map_err(wrote)?,
                Err(e) => {
                    return Err(Failure {
                        refusal: Some(Refusal::DeclarationRefused),
                        trouble: format!("the icon {} an entry names: {e}", from.display()),
                    });
                }
            }
        }
    }
    copy_tree(
        &crate::declaration::brain_modules(roots),
        &staged.join(tree::BRAIN_MODULES),
    )
    .map_err(wrote)?;
    // WHAT THE PERSON SIGNED of the mirror, as the belongings keep it: root
    // takes from the staging what it vouches for, under the key it pinned.
    let signed = crate::declaration::signed(roots);
    for name in [tree::MANIFEST, tree::SIGNATURE] {
        match std::fs::read(signed.join(name)) {
            Ok(bytes) => write_private(&staged.join(name), &bytes).map_err(wrote)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(wrote(e)),
        }
    }
    let nonce = nonce();
    write_private(&staged.join(tree::NONCE), nonce.as_bytes()).map_err(wrote)?;
    Ok(nonce)
}

/// What root's applier did with our staging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// The machine has the declaration now.
    Applied,
    /// It already had it.
    Unchanged,
    /// The update holds the machine: the mirror keeps the change, and it
    /// arrives with the apply after that update's.
    Busy,
}

/// The applier's verdict, when the file reads.
fn verdict(path: &Path) -> Option<Verdict> {
    let bytes = std::fs::read(path).ok()?;
    ronoctua_person::parse(&bytes).ok()
}

/// What a verdict says to the staging `nonce`, whose applier was started at
/// `since`: `None` when it answers another staging, or an earlier run (start
/// again).
///
/// A verdict with our nonce is ours whenever it was written (a run already
/// going may have read our staging). A `busy` carries no nonce — it is said
/// before the staging is read — so it is ours only when its run began at or
/// after our start: the file is root's and stays between runs, and an
/// earlier update's `busy` would otherwise answer a start that never ran
/// the applier at all.
///
/// # Errors
///
/// [`Refusal::DeclarationRefused`] with the applier's reason and place.
pub fn read_verdict(
    v: &Verdict,
    nonce: &str,
    since: &ronoctua_person::Stamp,
) -> Option<Result<Applied, Failure>> {
    if v.nonce.as_deref() != Some(nonce) {
        // BUSY is said before the nonce was read: it is the answer to every
        // staging made while the update holds the lock — of a run that began
        // after ours was asked for.
        if let Some(r) = &v.refused
            && r.why == TreeWhy::Busy
            && v.nonce.is_none()
            && v.at >= *since
        {
            return Some(Ok(Applied::Busy));
        }
        return None;
    }
    Some(match v.outcome {
        TreeOutcome::Applied => Ok(Applied::Applied),
        TreeOutcome::Unchanged => Ok(Applied::Unchanged),
        TreeOutcome::Refused => match &v.refused {
            Some(r) if r.why == TreeWhy::Busy => Ok(Applied::Busy),
            Some(r) => Err(Failure {
                refusal: Some(Refusal::DeclarationRefused),
                trouble: format!("{} at {}: {}", r.why.id(), r.at, r.detail),
            }),
            None => Err(Failure::refused(Refusal::DeclarationRefused)),
        },
    })
}

/// The applier's reason, when `failure` is its refusal of a staging
/// ([`read_verdict`] writes it first: `<why> at <where>: <detail>`) — the
/// word the update's step ends with (`refused <why>`). `None` for any other
/// failure. Pure.
#[must_use]
pub fn refused_why(failure: &Failure) -> Option<TreeWhy> {
    if failure.refusal != Some(Refusal::DeclarationRefused) {
        return None;
    }
    let (word, _) = failure.trouble.split_once(" at ")?;
    TreeWhy::try_from(word.to_owned()).ok()
}

/// Sign the mirror here, stage it and have root apply it — the software
/// centre's gesture.
///
/// # Errors
///
/// What staging refused, [`Refusal::SourceAbsent`] without `systemctl`, and
/// the applier's refusal.
pub fn apply(ctx: &Context, report: &mut dyn Report) -> Result<Applied, Failure> {
    applied(ctx, report, true)
}

/// Stage the mirror AS IT IS SIGNED NOW and have root apply it — the update's
/// `person-stage`, whose own step signed it just before (`person-sign`, which
/// declines and is said when the session keeps no identity). Without the
/// apply, the update's root half verifies and builds the declarations the
/// machine had, and what the person declared on their other machines waits
/// for a gesture in the software centre.
///
/// # Errors
///
/// As [`apply`].
pub fn hand_over(ctx: &Context, report: &mut dyn Report) -> Result<Applied, Failure> {
    applied(ctx, report, false)
}

/// [`apply`], signing first or not.
fn applied(ctx: &Context, report: &mut dyn Report, signing: bool) -> Result<Applied, Failure> {
    let systemctl = tool(ctx.tools.systemctl.as_ref())?;
    report.step(Phase::Applying, 0, 0, "");
    if ctx.rehearse {
        let _ = writeln!(
            std::io::stderr(),
            "rehearse: stage into {} and start {}",
            ctx.roots.system.staged().display(),
            ronoctua_person::root::APPLY_UNIT
        );
        return Ok(Applied::Unchanged);
    }
    let staged = ctx.roots.system.staged();
    let unit = ronoctua_person::root::APPLY_UNIT;
    if signing {
        sign(ctx);
    }
    // WHY THE LAST START RAN NO APPLIER, when it did not (a start limit, a
    // unit that does not load): no verdict answers such a start, and the one
    // in the file is an earlier run's.
    let mut unstarted: Option<String> = None;
    for _ in 0..TRIES {
        let nonce = stage(&ctx.roots, &staged)?;
        let since = crate::declaration::now_stamp();
        // A FAILED START is also the refusal's exit 1: the verdict says why,
        // when this start wrote one.
        let ran = Call::new(systemctl, start_argv(unit))
            .changing()
            .quietly(ctx)?;
        let read = verdict(&ctx.roots.system.verdict());
        let fresh = read.as_ref().is_some_and(|v| v.at >= since);
        let Some(v) = read else {
            if ran.code == Some(0) {
                return Err(Failure {
                    refusal: Some(Refusal::DeclarationRefused),
                    trouble: format!(
                        "{unit} said nothing in {}",
                        ctx.roots.system.verdict().display()
                    ),
                });
            }
            unstarted = Some(ran.tail.join(" | "));
            continue;
        };
        if !fresh && v.nonce.as_deref() != Some(nonce.as_str()) && ran.code != Some(0) {
            unstarted = Some(ran.tail.join(" | "));
            continue;
        }
        // THIS START RAN ONE: what an earlier try said of its start is old.
        unstarted = None;
        if let Some(said) = read_verdict(&v, &nonce, &since) {
            // A BRAIN MODULE THE APPLIER LEFT OUT is not the declaration's
            // failure: the machine kept the copy it had (or none), and the
            // journal says which and why — the brain's page is where the
            // person sees it.
            for out in v
                .left_out
                .iter()
                .filter(|_| v.nonce.as_deref() == Some(&nonce))
            {
                tracing::warn!(
                    module = %out.module,
                    why = out.why.id(),
                    at = %out.at,
                    kept = out.kept,
                    "a brain module of the person's was left out by the applier"
                );
            }
            return said;
        }
    }
    Err(Failure {
        refusal: Some(Refusal::DeclarationRefused),
        trouble: unstarted.map_or_else(
            || format!("{TRIES} stagings were answered by others' verdicts"),
            |why| format!("{unit} ran no applier: {why}"),
        ),
    })
}

/// What a rebuild came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rebuilt {
    /// The machine runs the change.
    Switched,
    /// The change is in a system set for the NEXT BOOT (`ronoctua-machine`'s
    /// `staged`: an update's system waited for a restart, or `nixpkgs`
    /// moved): the machine runs it from the next restart, and until then
    /// runs what it ran — so nothing the old system still runs (a Flatpak's
    /// data) is taken away yet.
    Staged,
    /// The update holds the machine: the change waits for the apply after
    /// that update's.
    Busy,
}

/// The machine's record (`state.json`), when it reads.
#[must_use]
pub fn machine_state(machine: &Path) -> Option<ronoctua_person::state::MachineState> {
    let bytes = std::fs::read(machine.join(names::STATE)).ok()?;
    ronoctua_person::parse(&bytes).ok()
}

/// What a finished `ronoctua-update-apply.service` says: its main process's
/// exit, and the machine's record of its last apply.
///
/// # Errors
///
/// [`Refusal::ApplyFailed`] with the record's result.
pub fn rebuilt_of(
    status: Option<i32>,
    state: Option<&ronoctua_person::state::MachineState>,
) -> Result<Rebuilt, Failure> {
    use ronoctua_person::state::ApplyResult;
    let result = state.and_then(|s| s.last_apply.as_ref()).map(|a| a.result);
    match status {
        // SET FOR THE NEXT BOOT: the apply did its work, and the machine
        // runs it from the restart.
        Some(0) if result == Some(ApplyResult::Staged) => Ok(Rebuilt::Staged),
        Some(0) => Ok(Rebuilt::Switched),
        Some(UPDATE_BUSY) => Ok(Rebuilt::Busy),
        _ => {
            match result {
                // SWITCHED, SOME UNIT FAILED: the system runs the change.
                Some(ApplyResult::SwitchedWithFailures) => Ok(Rebuilt::Switched),
                Some(r) => Err(Failure {
                    refusal: Some(Refusal::ApplyFailed),
                    trouble: format!(
                        "{}: {}",
                        ronoctua_machine::UPDATE_APPLY_UNIT,
                        serde_json::to_string(&r)
                            .unwrap_or_default()
                            .trim_matches('"')
                    ),
                }),
                None => Err(Failure {
                    refusal: Some(Refusal::ApplyFailed),
                    trouble: format!(
                        "{} exited with {}",
                        ronoctua_machine::UPDATE_APPLY_UNIT,
                        status.map_or_else(|| "nothing".to_owned(), |s| s.to_string())
                    ),
                }),
            }
        }
    }
}

/// Rebuild the machine from its directory, inputs unmoved — after a change
/// only a rebuild carries out (a program removed, a decline, one brought
/// back). Blocks until the build and the switch are done.
///
/// # Errors
///
/// [`Refusal::SourceAbsent`] without `systemctl`, [`Refusal::ApplyFailed`].
pub fn rebuild(ctx: &Context, report: &mut dyn Report) -> Result<Rebuilt, Failure> {
    let systemctl = tool(ctx.tools.systemctl.as_ref())?;
    report.step(Phase::Applying, 0, 0, ronoctua_machine::UPDATE_APPLY_UNIT);
    let unit = ronoctua_machine::UPDATE_APPLY_UNIT;
    let ran = Call::new(systemctl, start_argv(unit))
        .changing()
        .quietly(ctx)?;
    if ran.rehearsed {
        return Ok(Rebuilt::Switched);
    }
    if ran.code == Some(0) {
        return rebuilt_of(Some(0), machine_state(&ctx.roots.system.machine).as_ref());
    }
    let status = Call::new(systemctl, exit_status_argv(unit))
        .keep_stdout()
        .quietly(ctx)
        .ok()
        .and_then(|r| r.stdout.trim().parse::<i32>().ok());
    rebuilt_of(status, machine_state(&ctx.roots.system.machine).as_ref())
}

/// What a publish came to: the push's own report, or why it did not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    /// Pushed (the belongings' JSON report, data).
    Pushed(String),
    /// Not now, and why (English, for the journal): no belongings' CLI here,
    /// no repository, the network. The next sync carries it.
    Later(String),
}

/// Asks the belongings to sign the mirror as it is now, before it is staged
/// — best effort, and never a question: without the identity the session
/// keeps (`refused no-passphrase`), root keeps its copy of what is not
/// signed yet, and the change goes signed with the next sync the person
/// opens. The journal says which.
fn sign(ctx: &Context) {
    let Some(belongings) = ctx.tools.belongings.as_deref() else {
        return;
    };
    match Call::new(belongings, sign_argv())
        .changing()
        .keep_stdout()
        .quietly(ctx)
    {
        Ok(ran) => tracing::info!(said = %ran.stdout.trim(), "the mirror was offered for signing"),
        Err(f) => tracing::info!(why = %f, "the mirror was not signed"),
    }
}

/// Carry the declaration to the person's repository.
#[must_use]
pub fn publish(ctx: &Context, report: &mut dyn Report) -> Published {
    let Some(belongings) = ctx.tools.belongings.as_deref() else {
        return Published::Later("no belongings CLI in this environment".into());
    };
    report.step(Phase::Publishing, 0, 0, "");
    match Call::new(belongings, push_argv())
        .changing()
        .keep_stdout()
        .quietly(ctx)
    {
        Ok(ran) if ran.code == Some(0) || ran.rehearsed => {
            Published::Pushed(ran.stdout.trim().to_owned())
        }
        Ok(ran) => Published::Later(ran.tail.join(" | ")),
        Err(f) => Published::Later(f.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::Tools;
    use ronoctua_person::Stamp;
    use ronoctua_person::declared::{Declared, Entry, WebApp};
    use ronoctua_person::tree::VerdictRefusal;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ronoctua-software-stage-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn web(slug: &str, icon: Option<String>) -> Entry {
        Entry::Web(WebApp {
            id: format!("web:{slug}"),
            machines: None,
            edited_at: Stamp::parse("2026-10-05T12:00:00Z").unwrap(),
            edited_on: "noctua".into(),
            slug: slug.into(),
            name: "Mail".into(),
            url: "https://mail.example.org/".into(),
            profile: "Default".into(),
            icon,
            category: "Network".into(),
        })
    }

    /// FAILS IF: the staging is not the shape root's applier reads — the
    /// documents under `person/`, ONLY the icons an entry names, the brain
    /// modules with their links left out, an old staging's leftovers gone,
    /// and a nonce of the applier's grammar written last.
    #[test]
    fn a_staging_is_the_shape_the_applier_reads() {
        let root = scratch("shape");
        let roots = Roots::under(&root);
        let png = b"\x89PNG\r\n\x1a\nnamed".to_vec();
        let sha = crate::declaration::keep_icon(&roots, &png).unwrap();
        let other = crate::declaration::keep_icon(&roots, b"\x89PNG\r\n\x1a\nunnamed").unwrap();
        crate::declaration::change_declared(&roots, |d| {
            crate::declaration::upsert(d, web("mail", Some(sha.clone())));
            Ok(())
        })
        .unwrap();
        let module = crate::declaration::brain_modules(&roots).join("my-notes");
        std::fs::create_dir_all(module.join("i18n")).unwrap();
        std::fs::write(module.join("module.rs"), b"pub fn new() {}").unwrap();
        std::fs::write(module.join("i18n/en-GB.ftl"), b"x = y").unwrap();
        std::os::unix::fs::symlink("/etc/passwd", module.join("stolen")).unwrap();
        let staged = roots.system.staged();
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(staged.join("leftover"), b"old").unwrap();
        let nonce = stage(&roots, &staged).unwrap();
        assert!(tree::nonce_of(nonce.as_bytes()).is_ok(), "{nonce}");
        assert_eq!(
            std::fs::read_to_string(staged.join(tree::NONCE)).unwrap(),
            nonce
        );
        assert!(staged.join("person").join(names::DECLARED).is_file());
        assert!(
            !staged.join("person").join(names::DECLINED).exists(),
            "nothing declined"
        );
        assert!(
            !staged.join("person").join(names::DISTRO_PINS).exists(),
            "nothing followed"
        );
        assert_eq!(
            std::fs::read(
                staged
                    .join("person")
                    .join(names::ICONS)
                    .join(format!("{sha}.png"))
            )
            .unwrap(),
            png
        );
        assert!(
            !staged
                .join("person")
                .join(names::ICONS)
                .join(format!("{other}.png"))
                .exists()
        );
        assert!(
            staged
                .join("brain-modules/my-notes/i18n/en-GB.ftl")
                .is_file()
        );
        assert!(std::fs::symlink_metadata(staged.join("brain-modules/my-notes/stolen")).is_err());
        assert!(!staged.join("leftover").exists());
        assert!(
            !staged.join(tree::MANIFEST).exists(),
            "a signature from nowhere"
        );
        // WHAT THE PERSON SIGNED travels with the staging, byte for byte.
        let signed = crate::declaration::signed(&roots);
        std::fs::create_dir_all(&signed).unwrap();
        std::fs::write(signed.join(tree::MANIFEST), b"{\"the\": \"manifest\"}").unwrap();
        std::fs::write(signed.join(tree::SIGNATURE), b"c2lnbmF0dXJl\n").unwrap();
        stage(&roots, &staged).unwrap();
        assert_eq!(
            std::fs::read(staged.join(tree::MANIFEST)).unwrap(),
            b"{\"the\": \"manifest\"}"
        );
        assert_eq!(
            std::fs::read(staged.join(tree::SIGNATURE)).unwrap(),
            b"c2lnbmF0dXJl\n"
        );
        assert_eq!(
            signed,
            roots.state.join(ronoctua_xdg::name()).join(names::SIGNED),
            "the stager looks for the signature where the belongings keep it"
        );
        // THE DISTRO PINS THEY FOLLOW travel as they are — one that does not
        // read too: root keeps it back, and the declarations still arrive.
        // Written here and not read from `ronoctua-person`'s fixtures: a unit
        // of this crate is built from this crate's sources alone.
        let followed: &[u8] = br#"{
  "format": 1,
  "pins": [
    {
      "id": "cadcraft",
      "url": "https://github.com/storytold/cadcraft/releases/download/v0.9.0/cadcraft-0.9.0-linux-x86_64.AppImage",
      "hash": "sha256-JPd+NyvwcjP3JnJFi8aRNdVLyfa7Y3ntc3hwp8EVK9I=",
      "version": "0.9.0",
      "pinned-at": "2026-10-20T12:00:00Z"
    }
  ]
}
"#;
        std::fs::write(crate::follow::path(&roots), followed).unwrap();
        stage(&roots, &staged).unwrap();
        assert_eq!(
            std::fs::read(staged.join("person").join(names::DISTRO_PINS)).unwrap(),
            followed
        );
        std::fs::write(crate::follow::path(&roots), b"{\"format\": 9}").unwrap();
        stage(&roots, &staged).expect("followed pins that do not read hold nothing back");
        assert_eq!(
            std::fs::read(staged.join("person").join(names::DISTRO_PINS)).unwrap(),
            b"{\"format\": 9}"
        );
        std::fs::remove_file(crate::follow::path(&roots)).unwrap();
        // A SECOND STAGING has another nonce.
        assert_ne!(stage(&roots, &staged).unwrap(), nonce);
        // A MIRROR THAT DOES NOT READ is never staged.
        std::fs::write(
            crate::declaration::mirror(&roots).join(names::DECLARED),
            b"{",
        )
        .unwrap();
        assert_eq!(
            stage(&roots, &staged).unwrap_err().refusal,
            Some(Refusal::DeclarationUnreadable)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    fn verdict_of(nonce: Option<&str>, outcome: TreeOutcome, why: Option<TreeWhy>) -> Verdict {
        Verdict {
            format: 1,
            at: Stamp::parse("2026-10-05T12:00:00Z").unwrap(),
            nonce: nonce.map(str::to_owned),
            outcome,
            refused: why.map(|why| VerdictRefusal {
                why,
                at: "person/software/declared.json#/apps/0".into(),
                detail: "too long".into(),
            }),
            ignored: Vec::new(),
            left_out: Vec::new(),
            kept_back: Vec::new(),
        }
    }

    /// FAILS IF: another staging's verdict is taken for ours, busy is a
    /// failure, an earlier run's busy answers a later start, or a refusal
    /// loses its reason.
    #[test]
    fn a_verdict_answers_its_own_staging_only() {
        let ours = "aaaaaaaaaaaa";
        let since = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
        let later = Stamp::parse("2026-10-05T12:00:01Z").unwrap();
        assert_eq!(
            read_verdict(
                &verdict_of(Some(ours), TreeOutcome::Applied, None),
                ours,
                &since
            ),
            Some(Ok(Applied::Applied))
        );
        // OUR NONCE answers us even from a run that began before our start
        // (it was already going, and read our staging).
        assert_eq!(
            read_verdict(
                &verdict_of(Some(ours), TreeOutcome::Applied, None),
                ours,
                &later
            ),
            Some(Ok(Applied::Applied))
        );
        assert_eq!(
            read_verdict(
                &verdict_of(Some("bbbbbbbbbbbb"), TreeOutcome::Applied, None),
                ours,
                &since
            ),
            None
        );
        assert_eq!(
            read_verdict(
                &verdict_of(None, TreeOutcome::Refused, Some(TreeWhy::Busy)),
                ours,
                &since
            ),
            Some(Ok(Applied::Busy))
        );
        // AN EARLIER RUN'S BUSY, left in root's file, answers nothing now.
        assert_eq!(
            read_verdict(
                &verdict_of(None, TreeOutcome::Refused, Some(TreeWhy::Busy)),
                ours,
                &later
            ),
            None
        );
        let refused = read_verdict(
            &verdict_of(Some(ours), TreeOutcome::Refused, Some(TreeWhy::TooLarge)),
            ours,
            &since,
        )
        .unwrap()
        .unwrap_err();
        assert_eq!(refused.refusal, Some(Refusal::DeclarationRefused));
        assert!(refused.trouble.contains("too-large"), "{}", refused.trouble);
    }

    /// FAILS IF: the update holding the machine is a failure, a switch with a
    /// failed unit is not a switch, a failed build loses its word, or an
    /// apply set for the next boot is taken for a switch — what the old
    /// system still runs (a Flatpak's data) would be erased under it.
    #[test]
    fn a_rebuild_says_what_the_machine_did() {
        use ronoctua_person::state::{ApplyResult, LastApply, MachineState};
        assert_eq!(rebuilt_of(Some(0), None), Ok(Rebuilt::Switched));
        assert_eq!(rebuilt_of(Some(UPDATE_BUSY), None), Ok(Rebuilt::Busy));
        let with = |result| MachineState {
            last_apply: Some(LastApply {
                at: Stamp::parse("2026-10-05T12:00:00Z").unwrap(),
                result,
                toplevel: None,
                missing: Vec::new(),
            }),
            ..MachineState::default()
        };
        assert_eq!(
            rebuilt_of(Some(0), Some(&with(ApplyResult::Staged))),
            Ok(Rebuilt::Staged)
        );
        assert_eq!(
            rebuilt_of(Some(0), Some(&with(ApplyResult::Switched))),
            Ok(Rebuilt::Switched)
        );
        assert_eq!(
            rebuilt_of(Some(4), Some(&with(ApplyResult::SwitchedWithFailures))),
            Ok(Rebuilt::Switched)
        );
        let failed = rebuilt_of(Some(1), Some(&with(ApplyResult::BuildFailed))).unwrap_err();
        assert_eq!(failed.refusal, Some(Refusal::ApplyFailed));
        assert!(
            failed.trouble.contains("build-failed"),
            "{}",
            failed.trouble
        );
        assert_eq!(
            rebuilt_of(None, None).unwrap_err().refusal,
            Some(Refusal::ApplyFailed)
        );
    }

    /// FAILS IF: the applier is not started by its unit with no question, or
    /// a verdict of another staging is taken as ours.
    #[test]
    fn the_applier_is_started_and_its_verdict_read() {
        let root = scratch("apply");
        let roots = Roots::under(&root);
        crate::declaration::change_declared(&roots, |d: &mut Declared| {
            crate::declaration::upsert(d, web("mail", None));
            Ok(())
        })
        .unwrap();
        let bin = root.join("bin");
        let log = root.join("calls.log");
        // SYSTEMCTL, as root's applier would answer: the verdict of the
        // nonce it finds, written beside the staging.
        crate::write_executable(
            &bin.join("systemctl"),
            &format!(
                "#!/bin/sh\n\
                 echo \"systemctl $*\" >> {log}\n\
                 n=$(cat {staged}/nonce)\n\
                 printf '{{\"format\":1,\"at\":\"2026-10-05T12:00:00Z\",\"nonce\":\"%s\",\"outcome\":\"applied\",\"refused\":null,\"ignored\":[],\"left-out\":[]}}' \"$n\" > {verdict}\n",
                log = log.display(),
                staged = roots.system.staged().display(),
                verdict = roots.system.verdict().display()
            ),
        );
        let ctx = Context {
            tools: Tools {
                systemctl: Some(bin.join("systemctl")),
                ..Tools::default()
            },
            roots,
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        assert_eq!(apply(&ctx, &mut Vec::new()), Ok(Applied::Applied));
        let calls = std::fs::read_to_string(&log).unwrap();
        assert_eq!(
            calls.trim(),
            format!(
                "systemctl start --no-ask-password {}",
                ronoctua_person::root::APPLY_UNIT
            )
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the update's hand-over signs (its own step signed just
    /// before), or the software centre's gesture does not — or either stages
    /// without starting the applier.
    #[test]
    fn the_update_hands_over_what_its_own_step_signed() {
        let root = scratch("hand-over");
        let roots = Roots::under(&root);
        crate::declaration::change_declared(&roots, |d: &mut Declared| {
            crate::declaration::upsert(d, web("mail", None));
            Ok(())
        })
        .unwrap();
        let bin = root.join("bin");
        let log = root.join("calls.log");
        crate::write_executable(
            &bin.join("systemctl"),
            &format!(
                "#!/bin/sh\n\
                 echo \"systemctl $*\" >> {log}\n\
                 n=$(cat {staged}/nonce)\n\
                 printf '{{\"format\":1,\"at\":\"2026-10-05T12:00:00Z\",\"nonce\":\"%s\",\"outcome\":\"unchanged\",\"refused\":null,\"ignored\":[],\"left-out\":[]}}' \"$n\" > {verdict}\n",
                log = log.display(),
                staged = roots.system.staged().display(),
                verdict = roots.system.verdict().display()
            ),
        );
        crate::write_executable(
            &bin.join("belongings"),
            &format!(
                "#!/bin/sh\necho \"belongings $*\" >> {log}\necho '{{\"signed\":\"unchanged\"}}'\n",
                log = log.display()
            ),
        );
        let ctx = Context {
            tools: Tools {
                systemctl: Some(bin.join("systemctl")),
                belongings: Some(bin.join("belongings")),
                ..Tools::default()
            },
            roots,
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        let start = format!(
            "systemctl start --no-ask-password {}",
            ronoctua_person::root::APPLY_UNIT
        );
        assert_eq!(hand_over(&ctx, &mut Vec::new()), Ok(Applied::Unchanged));
        assert_eq!(std::fs::read_to_string(&log).unwrap().trim(), start);
        std::fs::remove_file(&log).unwrap();
        assert_eq!(apply(&ctx, &mut Vec::new()), Ok(Applied::Unchanged));
        assert_eq!(
            std::fs::read_to_string(&log).unwrap().trim(),
            format!("belongings sign --json\n{start}")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: a start that ran no applier (the unit's start limit, a
    /// unit that does not load) is answered by the busy an earlier update
    /// left in root's file — the order would say it was done — or its
    /// refusal does not say that the applier never ran.
    #[test]
    fn a_start_that_ran_no_applier_is_a_refusal() {
        let root = scratch("unstarted");
        let roots = Roots::under(&root);
        crate::declaration::change_declared(&roots, |d: &mut Declared| {
            crate::declaration::upsert(d, web("mail", None));
            Ok(())
        })
        .unwrap();
        // AN EARLIER UPDATE'S BUSY, of an hour ago.
        let old = Verdict {
            at: Stamp::parse("2026-10-05T11:00:00Z").unwrap(),
            ..verdict_of(None, TreeOutcome::Refused, Some(TreeWhy::Busy))
        };
        std::fs::create_dir_all(&roots.system.person_run).unwrap();
        std::fs::write(roots.system.verdict(), ronoctua_person::to_json(&old)).unwrap();
        let bin = root.join("bin");
        crate::write_executable(
            &bin.join("systemctl"),
            "#!/bin/sh
echo 'Job for ronoctua-person-apply.service failed: start-limit-hit' >&2
exit 1
",
        );
        let ctx = Context {
            tools: Tools {
                systemctl: Some(bin.join("systemctl")),
                ..Tools::default()
            },
            roots,
            prefs: crate::prefs::Preferences::default(),
            rehearse: false,
        };
        let refused = apply(&ctx, &mut Vec::new()).unwrap_err();
        assert_eq!(refused.refusal, Some(Refusal::DeclarationRefused));
        assert!(
            refused.trouble.contains("ran no applier")
                && refused.trouble.contains("start-limit-hit"),
            "{}",
            refused.trouble
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// FAILS IF: the applier's reason is lost on the way to the update's
    /// step — or a failure that is not the applier's is given one.
    #[test]
    fn the_appliers_reason_is_its_word() {
        let since = Stamp::parse("2026-10-05T12:00:00Z").unwrap();
        for why in [TreeWhy::NotSigned, TreeWhy::RolledBack, TreeWhy::TooLarge] {
            let failure = read_verdict(
                &verdict_of(Some("aaaaaaaaaaaa"), TreeOutcome::Refused, Some(why)),
                "aaaaaaaaaaaa",
                &since,
            )
            .unwrap()
            .unwrap_err();
            assert_eq!(refused_why(&failure), Some(why), "{}", failure.trouble);
        }
        assert_eq!(refused_why(&Failure::trouble("not-signed at x: y")), None);
        assert_eq!(
            refused_why(&Failure::refused(Refusal::DeclarationRefused)),
            None
        );
    }

    #[test]
    fn the_argument_vectors_are_the_programs_own() {
        assert_eq!(
            exit_status_argv("x.service"),
            ["show", "--property=ExecMainStatus", "--value", "x.service"]
        );
        assert_eq!(push_argv(), ["plain", "push", "--json"]);
        assert_eq!(
            start_argv(ronoctua_machine::UPDATE_APPLY_UNIT),
            [
                "start",
                "--no-ask-password",
                "ronoctua-update-apply.service"
            ]
        );
    }
}
